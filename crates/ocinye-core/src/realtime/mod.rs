//! O plano realtime: propagação, presença e `typing`.
//!
//! # O que este módulo **não** decide
//!
//! Nada. Não autoriza, não persiste, e não é fonte de verdade sobre coisa
//! nenhuma (ADR-0012). O que ele faz é levar depressa a quem está a ouvir uma
//! notícia que o PostgreSQL já guardou — e guardar, com prazo de validade, duas
//! coisas que não merecem uma tabela: quem está, e quem está a escrever.
//!
//! # Persistir primeiro, publicar depois
//!
//! Sempre por esta ordem, e nunca em paralelo. Se o `publish` falhar depois do
//! `commit`, a operação continua verdadeira e chega ao cliente no `reconnect`,
//! lida da base. Não há compensação a fazer, porque não há nada por desfazer.

pub mod events;
pub mod presence;

use std::collections::BTreeSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use redis::AsyncCommands;
use uuid::Uuid;

use events::{Channel, ServerEvent};

/// Quanto tempo se espera pelo Redis ao arrancar.
///
/// # Porque com limite
///
/// Porque o gestor de ligações do `redis` volta a tentar sozinho, e sem limite
/// um endereço que não responde prende o arranque — e a suite, que passou a
/// levar oito minutos num teste que devia levar um segundo. Quem não responde
/// depressa não responde: o plano fica degradado e o Ocinye sobe na mesma.
const LIGACAO_MAXIMA: std::time::Duration = std::time::Duration::from_secs(3);
use presence::{Presence, PRESENCE_TTL_SECONDS, TYPING_TTL_SECONDS};

/// O plano realtime desta instalação.
///
/// # Porque nenhum método devolve erro ao chamador
///
/// Porque uma falha de propagação **não é uma falha da operação**. Se o Redis
/// não responder depois de a mensagem estar guardada, devolver erro faria o
/// handler dizer à pessoa que o envio falhou — e ele não falhou. O que se perde
/// é a chegada instantânea; a mensagem está lá, e aparece ao recarregar.
///
/// A falha regista-se e muda o estado de prontidão. Não sobe.
pub struct Realtime {
    /// Ausente quando esta instalação não tem Redis configurado.
    ///
    /// Nunca fatal: sem Redis, o Ocinye continua inteiro e o que se perde é
    /// propagação instantânea, presença e `typing` (ADR-0012 §9).
    ligacao: Option<redis::aio::ConnectionManager>,
    /// O endereço, guardado para abrir escutas novas.
    ///
    /// Uma ligação de `pub/sub` não pode fazer mais nada enquanto escuta, por
    /// isso cada socket precisa da sua — e para a abrir é preciso o endereço.
    url: Option<String>,
    /// Se a última operação contra o Redis correu bem.
    ///
    /// É isto que a prontidão lê, e o que faz a interface dizer honestamente
    /// que o tempo real está em baixo em vez de mostrar uma lista parada.
    saudavel: Arc<AtomicBool>,
}

impl std::fmt::Debug for Realtime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Realtime")
            .field("configurado", &self.ligacao.is_some())
            .field("saudavel", &self.saudavel.load(Ordering::Relaxed))
            .finish()
    }
}

impl Realtime {
    /// Liga-se ao Redis, ou fica sem ele.
    ///
    /// # Porque não falha o arranque
    ///
    /// Porque o correio, a investigação, o conhecimento e a governação não têm
    /// nada que ver com tempo real. Deitar a instituição abaixo porque um
    /// serviço de coordenação efémera não responde seria uma avaria
    /// auto-infligida.
    pub async fn connect(url: &str) -> Self {
        let saudavel = Arc::new(AtomicBool::new(false));

        if url.trim().is_empty() {
            tracing::info!("o plano realtime não está configurado nesta instalação");
            return Self {
                ligacao: None,
                url: None,
                saudavel,
            };
        }

        match redis::Client::open(url) {
            Ok(cliente) => match tokio::time::timeout(
                LIGACAO_MAXIMA,
                redis::aio::ConnectionManager::new(cliente),
            )
            .await
            .unwrap_or_else(|_| {
                Err(redis::RedisError::from((
                    redis::ErrorKind::IoError,
                    "o Redis não respondeu a tempo",
                )))
            }) {
                Ok(gestor) => {
                    saudavel.store(true, Ordering::Relaxed);
                    tracing::info!(endpoint = %sem_credenciais(url), "plano realtime ligado");
                    Self {
                        ligacao: Some(gestor),
                        url: Some(url.to_owned()),
                        saudavel,
                    }
                }
                Err(erro) => {
                    // O anfitrião e o porto, e nunca as credenciais que possam
                    // vir no endereço.
                    //
                    // O comentário dizia «o endereço» e nada era registado. Sem
                    // ele, uma instalação configurada para `6380` com o Redis a
                    // responder em `6379` diz apenas «não respondeu» — e quem
                    // administra vai procurar um serviço em baixo quando o que
                    // há é um número trocado. Foi assim que esta instalação
                    // passou dias com o tempo real degradado.
                    tracing::warn!(
                        cause = %erro,
                        endpoint = %sem_credenciais(url),
                        "o plano realtime não respondeu; o tempo real fica degradado"
                    );
                    // O endereço fica guardado.
                    //
                    // É o que distingue «esta instalação não tem tempo real» de
                    // «tem, e está em baixo» — e mandar quem administra
                    // procurar configuração em falta quando o que há é um
                    // serviço parado é fazê-lo perder tempo no sítio errado.
                    Self {
                        ligacao: None,
                        url: Some(url.to_owned()),
                        saudavel,
                    }
                }
            },
            Err(erro) => {
                // Um endereço que não se lê é configuração **errada**, e não
                // configuração ausente. Quem a escreveu tem de a ver.
                tracing::warn!(cause = %erro, "endereço de Redis inválido");
                Self {
                    ligacao: None,
                    url: Some(url.to_owned()),
                    saudavel,
                }
            }
        }
    }

    /// Um plano realtime que não existe.
    ///
    /// Para instalações sem Redis e para testes que medem outra coisa. Aceita
    /// tudo e não propaga nada — que é exactamente o comportamento correcto de
    /// uma instalação sem tempo real, e não um sítio por preencher.
    #[must_use]
    pub fn ausente() -> Self {
        Self {
            ligacao: None,
            url: None,
            saudavel: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Abre um canal de escuta, próprio desta ligação.
    ///
    /// # Porque uma ligação por socket, e não uma partilhada
    ///
    /// Porque o `pub/sub` do Redis subscreve por ligação: uma partilhada
    /// obrigaria a ouvir **tudo** e a filtrar dentro do processo, e cada Core
    /// receberia todos os eventos de toda a instituição para deitar fora quase
    /// todos. Subscrever só o que cada pessoa pediu é mais ligações e muito
    /// menos tráfego.
    ///
    /// Devolve `None` quando não há Redis — e o socket funciona na mesma, sem
    /// entregar nada, que é o que uma instalação sem tempo real faz.
    pub async fn escutar(&self) -> Option<Escuta> {
        let url = self.url.clone()?;
        let cliente = redis::Client::open(url).ok()?;
        match cliente.get_async_pubsub().await {
            Ok(pubsub) => {
                let (sink, stream) = pubsub.split();
                Some(Escuta { sink, stream })
            }
            Err(erro) => {
                self.saudavel.store(false, Ordering::Relaxed);
                tracing::warn!(cause = %erro, "não foi possível abrir uma escuta realtime");
                None
            }
        }
    }

    /// Se esta instalação tem tempo real configurado.
    ///
    /// Distinto de [`Self::saudavel`]: uma instalação sem Redis fez uma escolha,
    /// e uma com Redis em baixo tem uma avaria. Dizer o mesmo às duas mandaria
    /// quem administra procurar um problema que não existe.
    #[must_use]
    pub fn configurado(&self) -> bool {
        self.url.is_some()
    }

    /// Se o tempo real está a funcionar agora.
    #[must_use]
    pub fn saudavel(&self) -> bool {
        self.ligacao.is_some() && self.saudavel.load(Ordering::Relaxed)
    }

    /// Anota o resultado de uma operação, e devolve-o.
    fn anota<T>(&self, resultado: redis::RedisResult<T>) -> Option<T> {
        match resultado {
            Ok(valor) => {
                self.saudavel.store(true, Ordering::Relaxed);
                Some(valor)
            }
            Err(erro) => {
                self.saudavel.store(false, Ordering::Relaxed);
                tracing::warn!(cause = %erro, "o plano realtime falhou uma operação");
                None
            }
        }
    }

    /// Anuncia um evento num canal.
    ///
    /// Chamar isto **depois** do `commit`, sempre.
    pub async fn publish(&self, canal: Channel, evento: &ServerEvent) {
        let Some(mut ligacao) = self.ligacao.clone() else {
            return;
        };
        let Ok(carga) = serde_json::to_string(evento) else {
            // Um evento que não serializa é um defeito de programação, não uma
            // falha de infraestrutura: não marca o plano como doente.
            tracing::error!("um evento realtime não serializou");
            return;
        };

        let resultado: redis::RedisResult<()> = ligacao.publish(canal.topico(), carga).await;
        self.anota(resultado);
    }

    // ── Presença ────────────────────────────────────────────────────────

    /// Regista que esta ligação desta pessoa está viva.
    ///
    /// # Porque a chave leva a ligação e não só a pessoa
    ///
    /// Porque três separadores abertos são três sockets e **uma** pessoa
    /// (ADR-0012 §8). Com uma chave por pessoa, fechar um separador apagaria a
    /// presença enquanto os outros dois continuavam ligados.
    pub async fn batimento(&self, person_id: Uuid, ligacao_id: Uuid, activo: bool) {
        let Some(mut redis) = self.ligacao.clone() else {
            return;
        };
        let chave = format!("oc:rt:vivo:{person_id}:{ligacao_id}");
        let valor = if activo { "activo" } else { "parado" };

        let resultado: redis::RedisResult<()> =
            redis.set_ex(chave, valor, PRESENCE_TTL_SECONDS).await;
        self.anota(resultado);
    }

    /// Larga uma ligação, sem esperar pelo TTL.
    ///
    /// Um adeus educado é mais rápido do que o prazo — mas o prazo é que é a
    /// garantia, porque a maioria das ligações morre sem se despedir.
    pub async fn largar(&self, person_id: Uuid, ligacao_id: Uuid) {
        let Some(mut redis) = self.ligacao.clone() else {
            return;
        };
        let resultado: redis::RedisResult<()> = redis
            .del(format!("oc:rt:vivo:{person_id}:{ligacao_id}"))
            .await;
        self.anota(resultado);
    }

    /// Guarda o estado que a pessoa declarou.
    ///
    /// Sem TTL: uma declaração é uma intenção, e não um sinal de vida. Quem se
    /// pôs em «Não incomodar» não quer voltar a «Disponível» porque passaram
    /// quarenta e cinco segundos.
    pub async fn declarar(&self, person_id: Uuid, estado: Option<Presence>) {
        let Some(mut redis) = self.ligacao.clone() else {
            return;
        };
        let chave = format!("oc:rt:declarado:{person_id}");
        let resultado: redis::RedisResult<()> = match estado {
            Some(estado) => redis.set(chave, estado.as_str()).await,
            None => redis.del(chave).await,
        };
        self.anota(resultado);
    }

    /// Os sinais que o Redis conhece sobre uma pessoa.
    ///
    /// O sinal do Calendar não vem daqui — é acrescentado por quem tem
    /// autorização para o perguntar, e chega como um booleano e nada mais.
    pub async fn sinais(&self, person_id: Uuid) -> presence::Sinais {
        let Some(mut redis) = self.ligacao.clone() else {
            return presence::Sinais::default();
        };

        let declarado: Option<String> = self
            .anota(redis.get(format!("oc:rt:declarado:{person_id}")).await)
            .flatten();

        let vivas: Vec<String> = self
            .anota(redis.keys(format!("oc:rt:vivo:{person_id}:*")).await)
            .unwrap_or_default();

        let mut activo = false;
        for chave in &vivas {
            let valor: Option<String> = self.anota(redis.get(chave).await).flatten();
            if valor.as_deref() == Some("activo") {
                activo = true;
                break;
            }
        }

        presence::Sinais {
            declarado: declarado.as_deref().and_then(Presence::parse),
            em_compromisso: false,
            ligado: !vivas.is_empty(),
            activo,
        }
    }

    // ── `typing` ────────────────────────────────────────────────────────

    /// Marca, ou desmarca, que esta pessoa está a escrever.
    pub async fn a_escrever(&self, conversation_id: Uuid, person_id: Uuid, sim: bool) {
        let Some(mut redis) = self.ligacao.clone() else {
            return;
        };
        let chave = format!("oc:rt:escreve:{conversation_id}:{person_id}");
        let resultado: redis::RedisResult<()> = if sim {
            redis.set_ex(chave, "1", TYPING_TTL_SECONDS).await
        } else {
            redis.del(chave).await
        };
        self.anota(resultado);
    }

    /// Quem está a escrever numa conversa, agora.
    pub async fn quem_escreve(&self, conversation_id: Uuid) -> BTreeSet<Uuid> {
        let Some(mut redis) = self.ligacao.clone() else {
            return BTreeSet::new();
        };

        let chaves: Vec<String> = self
            .anota(
                redis
                    .keys(format!("oc:rt:escreve:{conversation_id}:*"))
                    .await,
            )
            .unwrap_or_default();

        chaves
            .iter()
            .filter_map(|chave| chave.rsplit(':').next())
            .filter_map(|id| Uuid::parse_str(id).ok())
            .collect()
    }
}

/// Uma escuta aberta, própria de uma ligação.
///
/// # Porque o Redis não sai daqui
///
/// Para que o `core-server` não precise de nomear o `redis`. A fronteira do
/// socket decide **quem ouve o quê**; como isso viaja é assunto deste módulo, e
/// trocar o Redis por outra coisa não devia obrigar a tocar na autorização.
pub struct Escuta {
    sink: redis::aio::PubSubSink,
    stream: redis::aio::PubSubStream,
}

impl std::fmt::Debug for Escuta {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Escuta")
    }
}

impl Escuta {
    /// Passa a receber este canal.
    ///
    /// Devolve `false` quando a escuta caiu. Não é um erro a subir: quem chama
    /// trata isso como degradação do tempo real, e não como falha da operação
    /// que o desencadeou.
    pub async fn subscrever(&mut self, canal: Channel) -> bool {
        self.sink.subscribe(canal.topico()).await.is_ok()
    }

    /// Deixa de receber este canal.
    pub async fn cancelar(&mut self, canal: Channel) {
        let _ = self.sink.unsubscribe(canal.topico()).await;
    }

    /// A próxima entrega, ou `None` quando a escuta acabou.
    ///
    /// Devolve o canal já tipado: quem entrega precisa dele para reverificar a
    /// autorização, e uma `String` obrigaria a adivinhar a regra.
    pub async fn proxima(&mut self) -> Option<(Channel, String)> {
        loop {
            let mensagem = futures::StreamExt::next(&mut self.stream).await?;
            let topico = mensagem.get_channel_name().to_owned();
            let Ok(carga) = mensagem.get_payload::<String>() else {
                continue;
            };
            // Um tópico que não corresponde a nenhum canal conhecido é
            // descartado. Chegar aqui significaria que alguém publicou fora do
            // contrato, e adivinhar a regra seria pior do que ignorar.
            if let Some(canal) = canal_do_topico(&topico) {
                return Some((canal, carga));
            }
        }
    }
}

/// Lê o canal a partir do tópico por onde ele viajou.
fn canal_do_topico(topico: &str) -> Option<Channel> {
    let id = |resto: &str| Uuid::parse_str(resto).ok();
    if let Some(resto) = topico.strip_prefix("oc:rt:conversa:") {
        return id(resto).map(|id| Channel::Conversation { id });
    }
    if let Some(resto) = topico.strip_prefix("oc:rt:pessoa:") {
        return id(resto).map(|id| Channel::Person { id });
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn um_plano_ausente_aceita_tudo_e_nao_se_diz_saudavel() {
        let plano = Realtime::ausente();
        assert!(!plano.saudavel());
    }

    #[tokio::test]
    async fn sem_redis_nada_rebenta() {
        // O que uma instalação sem tempo real tem de fazer é continuar a
        // funcionar. Se qualquer destas chamadas entrasse em pânico, o correio
        // e a investigação cairiam por causa de um serviço que não têm.
        let plano = Realtime::ausente();
        let id = Uuid::from_u128(1);

        plano
            .publish(
                Channel::Person { id },
                &ServerEvent::RealtimeDegraded { activo: false },
            )
            .await;
        plano.batimento(id, Uuid::from_u128(2), true).await;
        plano.declarar(id, Some(Presence::Ocupado)).await;
        plano.a_escrever(id, id, true).await;

        assert!(plano.quem_escreve(id).await.is_empty());
        assert!(!plano.sinais(id).await.ligado);
    }

    #[tokio::test]
    async fn um_endereco_invalido_degrada_em_vez_de_rebentar() {
        let plano = Realtime::connect("isto-não-é-um-url").await;
        assert!(!plano.saudavel());
    }

    #[tokio::test]
    async fn configurado_e_em_baixo_nao_e_o_mesmo_que_por_configurar() {
        // Um endereço escrito e um serviço parado é uma **avaria**. Dizer «não
        // configurado» manda quem administra procurar configuração em falta que
        // está lá, e não olhar para o serviço que está parado.
        let em_baixo = Realtime::connect("redis://127.0.0.1:1").await;
        assert!(
            em_baixo.configurado(),
            "um endereço escrito continua escrito depois de a ligação falhar"
        );
        assert!(!em_baixo.saudavel());

        // Sem endereço nenhum, é uma escolha desta instalação.
        let ausente = Realtime::connect("").await;
        assert!(!ausente.configurado());
        assert!(!ausente.saudavel());
    }
}

/// O anfitrião e o porto de um endereço, sem o que estiver antes do `@`.
///
/// Um `redis://` pode levar utilizador e palavra-passe. Registar o endereço
/// inteiro poria credenciais no diário; não registar nada deixa quem administra
/// sem saber a que porto a instalação estava a bater. O que interessa ao
/// diagnóstico é exactamente o que é seguro dizer.
fn sem_credenciais(url: &str) -> String {
    let sem_esquema = url.split("://").nth(1).unwrap_or(url);
    let autoridade = sem_esquema.split('/').next().unwrap_or(sem_esquema);
    autoridade
        .rsplit('@')
        .next()
        .unwrap_or(autoridade)
        .to_owned()
}

#[cfg(test)]
mod endereco {
    use super::sem_credenciais;

    /// O que se regista diz o porto e não diz a palavra-passe.
    #[test]
    fn o_endereco_registado_nao_leva_credenciais() {
        assert_eq!(sem_credenciais("redis://localhost:6380"), "localhost:6380");
        assert_eq!(sem_credenciais("redis://redis:6379/0"), "redis:6379");
        assert_eq!(
            sem_credenciais("redis://ocinye:supersegredo@cache.interno:6379"),
            "cache.interno:6379"
        );
        // Sem esquema continua a ser legível, e continua a não dizer o segredo.
        assert_eq!(sem_credenciais("user:pw@host:6379"), "host:6379");
    }
}
