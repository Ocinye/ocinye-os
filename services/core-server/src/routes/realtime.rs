//! A fronteira realtime: um socket por ligação, autorizado a cada acto.
//!
//! # A decisão que este ficheiro existe para impor
//!
//! Todo o resto do Ocinye autoriza por pedido. Um pedido chega, o Core resolve
//! o principal, decide, responde, esquece — e entre dois pedidos há sempre onde
//! uma revogação acontecer.
//!
//! Um socket dura horas. Se a autoridade fosse resolvida quando ele abre, uma
//! pessoa removida de uma conversa às 10h continuaria a recebê-la às 18h. Não
//! por defeito nenhum: porque nada voltou a perguntar.
//!
//! Por isso este socket guarda **quem** — o identificador da pessoa e o da
//! sessão — e nunca **o que ela pode**. Antes de subscrever, pergunta. Antes de
//! entregar, pergunta outra vez (ADR-0012 §4).
//!
//! # O que não entra por aqui
//!
//! Nada durável. Enviar uma mensagem é uma Core Operation, e entra pela porta
//! que todas as outras usam. Este canal transporta subscrições, batimentos,
//! declarações de estado e `typing` — tudo efémero, tudo com prazo.

use std::collections::BTreeSet;
use std::time::Duration;

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::response::Response;
use axum::routing::get;
use axum::Router;
use ocinye_core::modules::identity;
use ocinye_core::realtime::events::{Channel, ClientCommand, ServerEvent};
use ocinye_core::realtime::presence::{self, Presence, HEARTBEAT_SECONDS};
use ocinye_core::realtime::Escuta;
use sqlx::PgPool;
use uuid::Uuid;

use crate::extract::CurrentPrincipal;
use crate::state::AppState;

/// A rota do plano realtime.
pub fn routes() -> Router<AppState> {
    Router::new().route("/realtime", get(abrir))
}

/// Quanto tempo o servidor espera por um comando antes de reavaliar tudo.
///
/// Não é um `timeout` de inactividade: é o relógio que faz a autoridade ser
/// reavaliada mesmo num socket em que ninguém escreve nada.
const RELOGIO: Duration = Duration::from_secs(HEARTBEAT_SECONDS);

/// Abre o socket.
///
/// A extracção do principal acontece **antes** do `upgrade`: um socket que
/// abrisse primeiro e autenticasse depois teria uma janela em que existe sem
/// dono.
async fn abrir(
    State(state): State<AppState>,
    CurrentPrincipal(principal): CurrentPrincipal,
    upgrade: WebSocketUpgrade,
) -> Response {
    let person_id = principal.person_id;
    // Uma ligação, e não uma pessoa: três separadores abertos são três destes
    // e uma pessoa só (ADR-0012 §8).
    let ligacao_id = Uuid::new_v4();

    upgrade.on_upgrade(move |socket| conduzir(state, socket, person_id, ligacao_id))
}

/// O estado de uma ligação aberta.
///
/// # O que aqui **não** está
///
/// Nenhuma permissão, nenhum papel, nenhum `Principal`. O que está guardado é
/// o que a pessoa **pediu** para ouvir; o que ela pode ouvir pergunta-se de cada
/// vez.
struct Ligacao {
    person_id: Uuid,
    ligacao_id: Uuid,
    /// Os tópicos que esta ligação pediu e obteve.
    ///
    /// O que ela **pode** ouvir não está aqui: pergunta-se antes de cada
    /// entrega, porque entre a subscrição e o evento pode ter havido uma
    /// remoção (ADR-0012 §4).
    pedidos: BTreeSet<String>,
}

/// Conduz uma ligação até ela fechar.
async fn conduzir(state: AppState, mut socket: WebSocket, person_id: Uuid, ligacao_id: Uuid) {
    let mut ligacao = Ligacao {
        person_id,
        ligacao_id,
        pedidos: BTreeSet::new(),
    };

    // O primeiro batimento, para que a pessoa apareça online sem esperar pelo
    // relógio.
    state.realtime.batimento(person_id, ligacao_id, true).await;

    // A interface tem de saber logo se o tempo real está a funcionar. Sem isto,
    // mostraria uma conversa parada com ar de normalidade.
    let inicial = ServerEvent::RealtimeDegraded {
        activo: state.realtime.saudavel(),
    };
    if enviar(&mut socket, &inicial).await.is_err() {
        return;
    }

    // A escuta desta ligação. Sem Redis não há nenhuma, e o socket continua a
    // servir batimentos e declarações — degradado, e não partido.
    let mut escuta = state.realtime.escutar().await;

    loop {
        // Três coisas podem acontecer a seguir: o cliente fala, o Redis
        // entrega, ou o relógio bate. A última existe para que a autoridade
        // seja reavaliada mesmo num socket onde ninguém diz nada.
        let acontecimento = tokio::select! {
            recebido = socket.recv() => Acontecimento::Cliente(recebido),
            entregue = proxima_entrega(&mut escuta) => Acontecimento::Redis(entregue),
            () = tokio::time::sleep(RELOGIO) => Acontecimento::Relogio,
        };

        match acontecimento {
            Acontecimento::Relogio => {
                if !continua_autorizado(&state.pool, person_id).await {
                    // A conta deixou de estar activa. Fechar é a única resposta:
                    // manter o socket seria manter uma autoridade que já não
                    // existe.
                    break;
                }
                state.realtime.batimento(person_id, ligacao_id, false).await;
            }
            Acontecimento::Redis(None) => {
                // A escuta caiu. O socket sobrevive: o cliente reconcilia com o
                // PostgreSQL ao reconectar, e não perde nada durável.
                escuta = None;
                let aviso = ServerEvent::RealtimeDegraded { activo: false };
                if enviar(&mut socket, &aviso).await.is_err() {
                    break;
                }
            }
            Acontecimento::Redis(Some((canal, carga))) => {
                if !ligacao.pedidos.contains(&canal.topico()) {
                    continue;
                }
                // A pergunta outra vez, e não a resposta de há uma hora. É isto
                // que faz uma remoção retirar acesso no mesmo instante.
                if !pode_ouvir(&state.pool, person_id, canal).await {
                    ligacao.pedidos.remove(&canal.topico());
                    if let Some(escuta) = escuta.as_mut() {
                        escuta.cancelar(canal).await;
                    }
                    continue;
                }
                if socket.send(Message::Text(carga.into())).await.is_err() {
                    break;
                }
            }
            Acontecimento::Cliente(None | Some(Err(_))) => break,
            Acontecimento::Cliente(Some(Ok(Message::Close(_)))) => break,
            Acontecimento::Cliente(Some(Ok(
                Message::Ping(_) | Message::Pong(_) | Message::Binary(_),
            ))) => {}
            Acontecimento::Cliente(Some(Ok(Message::Text(texto)))) => {
                if !continua_autorizado(&state.pool, person_id).await {
                    break;
                }
                let Ok(comando) = serde_json::from_str::<ClientCommand>(&texto) else {
                    // Um comando que não existe é recusado, e não ignorado: o
                    // socket tem contrato, e um cliente que fala outra língua
                    // tem de o saber.
                    continue;
                };
                if executar(&state, &mut ligacao, comando, escuta.as_mut())
                    .await
                    .is_err()
                {
                    break;
                }
            }
        }
    }

    // Um adeus é mais rápido do que o TTL — mas é o TTL a garantia, porque a
    // maioria das ligações morre sem se despedir.
    state.realtime.largar(person_id, ligacao_id).await;
}

/// O que pode acontecer a seguir numa ligação aberta.
enum Acontecimento {
    /// O cliente disse alguma coisa, ou desapareceu.
    Cliente(Option<Result<Message, axum::Error>>),
    /// O Redis entregou um evento, ou a escuta caiu.
    Redis(Option<(Channel, String)>),
    /// Passou o tempo, e é altura de reavaliar.
    Relogio,
}

/// Espera pela próxima entrega.
///
/// Quando não há escuta, nunca termina — e o `select!` fica com os outros dois
/// ramos, que é o comportamento certo de um socket sem tempo real.
async fn proxima_entrega(escuta: &mut Option<Escuta>) -> Option<(Channel, String)> {
    match escuta {
        None => std::future::pending().await,
        Some(escuta) => escuta.proxima().await,
    }
}

/// A conta continua a poder trabalhar?
///
/// Chamado antes de cada acto e a cada volta do relógio. É a resposta a
/// «Identity may persist. Authority must be re-established.»
async fn continua_autorizado(pool: &PgPool, person_id: Uuid) -> bool {
    match identity::person_by_id(pool, person_id).await {
        Ok(Some(pessoa)) => match identity::principal_for_person(pool, &pessoa).await {
            Ok(principal) => principal.is_active,
            Err(_) => false,
        },
        // Não encontrada, desactivada, ou a base não respondeu. Nos três casos a
        // resposta é a mesma: não se entrega. Falhar aberto aqui seria entregar
        // conversas por causa de uma indisponibilidade.
        _ => false,
    }
}

/// Quem pode ouvir este canal, agora.
///
/// Público para ser medido: é a regra de autorização do plano realtime inteiro,
/// e uma regra que só se pudesse exercitar levantando um browser seria uma regra
/// que ninguém exercita.
///
/// # Porque isto é um `match` exaustivo
///
/// Para que acrescentar um canal novo não compile até alguém decidir quem o
/// pode ouvir. Com uma `String` e uma consulta genérica, um canal novo nasceria
/// aberto por omissão (ADR-0012 §4).
pub async fn pode_ouvir(pool: &PgPool, person_id: Uuid, canal: Channel) -> bool {
    match canal {
        // Só a própria. O canal pessoal leva menções e contagens por ler.
        Channel::Person { id } => id == person_id,
        // Só quem participa **agora**. Conhecer o identificador não basta, e
        // nunca bastou.
        Channel::Conversation { id } => sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM conversation_participants
                  WHERE conversation_id = $1 AND person_id = $2 AND left_at IS NULL",
        )
        .bind(id)
        .bind(person_id)
        .fetch_one(pool)
        .await
        .map(|quantos| quantos > 0)
        .unwrap_or(false),
    }
}

/// Executa um comando do cliente.
async fn executar(
    state: &AppState,
    ligacao: &mut Ligacao,
    comando: ClientCommand,
    escuta: Option<&mut Escuta>,
) -> Result<(), ()> {
    let mut escuta = escuta;
    match comando {
        ClientCommand::Subscribe { canal } => {
            if pode_ouvir(&state.pool, ligacao.person_id, canal).await {
                if let Some(escuta) = escuta.as_mut() {
                    if !escuta.subscrever(canal).await {
                        return Ok(());
                    }
                }
                ligacao.pedidos.insert(canal.topico());
            }
            // Uma subscrição recusada não fecha o socket nem se explica. Dizer
            // «não pertences a essa conversa» confirmaria que ela existe, a
            // quem estivesse a adivinhar identificadores.
        }
        ClientCommand::Unsubscribe { canal } => {
            if let Some(escuta) = escuta.as_mut() {
                escuta.cancelar(canal).await;
            }
            ligacao.pedidos.remove(&canal.topico());
        }
        ClientCommand::Heartbeat => {
            state
                .realtime
                .batimento(ligacao.person_id, ligacao.ligacao_id, true)
                .await;
        }
        ClientCommand::Declare { estado } => {
            // `Offline` não se declara: seria dizer que não se está enquanto se
            // está, e a presença deixaria de significar nada.
            let guardado = match estado {
                Presence::Offline => None,
                Presence::Disponivel | Presence::Ausente => None,
                outro => Some(outro),
            };
            state.realtime.declarar(ligacao.person_id, guardado).await;

            let sinais = state.realtime.sinais(ligacao.person_id).await;
            let resolvido = presence::resolver(sinais);
            state
                .realtime
                .publish(
                    Channel::Person {
                        id: ligacao.person_id,
                    },
                    &ServerEvent::PresenceChanged {
                        person_id: ligacao.person_id,
                        estado: resolvido,
                    },
                )
                .await;
        }
        ClientCommand::Typing {
            conversation_id,
            a_escrever,
        } => {
            let canal = Channel::Conversation {
                id: conversation_id,
            };
            // Reverificado aqui, e não confiado na subscrição: entre subscrever
            // e escrever pode ter havido uma remoção.
            if !pode_ouvir(&state.pool, ligacao.person_id, canal).await {
                return Ok(());
            }
            state
                .realtime
                .a_escrever(conversation_id, ligacao.person_id, a_escrever)
                .await;
            state
                .realtime
                .publish(
                    canal,
                    &ServerEvent::TypingChanged {
                        conversation_id,
                        person_id: ligacao.person_id,
                        a_escrever,
                    },
                )
                .await;
        }
    }

    Ok(())
}

/// Envia um evento pelo socket.
async fn enviar(socket: &mut WebSocket, evento: &ServerEvent) -> Result<(), ()> {
    let Ok(texto) = serde_json::to_string(evento) else {
        return Err(());
    };
    socket
        .send(Message::Text(texto.into()))
        .await
        .map_err(|_| ())
}
