//! Ocinye Mensagens — a aplicação.
//!
//! # O que esta superfície é, e o que não é
//!
//! É onde uma pessoa da Ocinye fala com outra sem sair do Workspace. Duas
//! colunas: as conversas à esquerda, a conversa aberta à direita. A navegação
//! institucional continua a ser a barra lateral do Ocinye — esta lista é
//! navegação **de dentro do módulo**, e por isso é mais leve e mais densa do
//! que ela.
//!
//! Não é um cartão por mensagem, não são balões grandes, e não é uma tabela.
//! Uma conversa lê-se de relance: quem falou, quando, e o quê.

use chrono::{DateTime, NaiveDate, Utc};
use leptos::prelude::*;
use ocinye_contracts::temporal::TimeZoneName;
use serde_json::Value;
use uuid::Uuid;

use crate::ui::components::{avatar, empty_state, AvatarSize, Button, EmptyState};
use crate::ui::icon::{icon, Icon};

/// A rota canónica do módulo.
///
/// Escrita uma vez. Um caminho repetido por dez sítios é um caminho que muda em
/// nove deles quando alguém o altera.
pub const ROUTE: &str = "/messages";

/// O caminho de uma conversa.
#[must_use]
pub fn conversation_path(id: Uuid) -> String {
    format!("{ROUTE}/{id}")
}

/// Iniciais de um nome, para o avatar.
fn iniciais(nome: &str) -> String {
    let mut letras = nome
        .split_whitespace()
        .filter_map(|parte| parte.chars().next())
        .map(|c| c.to_uppercase().to_string());
    let primeira = letras.next().unwrap_or_default();
    let ultima = letras.next_back().unwrap_or_default();
    format!("{primeira}{ultima}")
}

fn texto<'a>(valor: &'a Value, campo: &str) -> &'a str {
    valor.get(campo).and_then(Value::as_str).unwrap_or_default()
}

fn inteiro(valor: &Value, campo: &str) -> i64 {
    valor.get(campo).and_then(Value::as_i64).unwrap_or(0)
}

fn instante(valor: &Value, campo: &str) -> Option<DateTime<Utc>> {
    valor
        .get(campo)
        .and_then(Value::as_str)
        .and_then(|t| DateTime::parse_from_rfc3339(t).ok())
        .map(|t| t.with_timezone(&Utc))
}

/// A hora de uma mensagem, na zona de quem olha.
fn hora(quando: DateTime<Utc>, zona: TimeZoneName) -> String {
    crate::ui::tempo::hora_civil(quando, zona)
        .format("%H:%M")
        .to_string()
}

/// Quando uma conversa foi tocada, dita como uma pessoa a diria.
///
/// Hoje mostra a hora; ontem diz «Ontem»; esta semana diz o dia; o resto mostra
/// a data. Um carimbo completo em todas as linhas seria ruído numa lista que se
/// lê de relance.
fn quando_curto(quando: DateTime<Utc>, hoje: NaiveDate, zona: TimeZoneName) -> String {
    let dia = crate::ui::tempo::dia_civil(quando, zona);
    let diferenca = (hoje - dia).num_days();
    match diferenca {
        0 => hora(quando, zona),
        1 => "Ontem".to_owned(),
        2..=6 => crate::ui::tempo::dia_da_semana_curto(dia).to_owned(),
        _ => dia.format("%d/%m/%Y").to_string(),
    }
}

/// O separador de um dia, dentro da conversa.
fn separador_do_dia(dia: NaiveDate, hoje: NaiveDate) -> String {
    match (hoje - dia).num_days() {
        0 => "Hoje".to_owned(),
        1 => "Ontem".to_owned(),
        2..=6 => crate::ui::tempo::dia_da_semana(dia).to_owned(),
        _ => crate::ui::tempo::data_por_extenso(dia),
    }
}

/// O ponto de presença de alguém.
///
/// # Porque não é só cor
///
/// Porque um ponto colorido sozinho não diz nada a quem não distingue as cores,
/// e nada a quem não conhece a convenção. O `title` e o texto para leitores de
/// ecrã dizem-no por extenso.
fn presenca(pessoa: &Value) -> impl IntoView {
    let estado = pessoa
        .get("presence")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
    let rotulo = pessoa
        .get("presence_label")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();

    // Sem tempo real não há presença nenhuma para mostrar. Um ponto cinzento a
    // dizer «Offline» seria uma afirmação que ninguém verificou.
    (!estado.is_empty()).then(|| {
        let classe = format!("oc-presenca oc-presenca--{estado}");
        view! {
            <span class=classe title=rotulo.clone() data-oc="presenca">
                <span class="oc-sr">{rotulo.clone()}</span>
            </span>
        }
    })
}

// ── A lista de conversas ────────────────────────────────────────────────

/// Uma linha da lista.
fn linha_da_conversa(
    conversa: &Value,
    aberta: Option<Uuid>,
    hoje: NaiveDate,
    zona: TimeZoneName,
) -> impl IntoView {
    let id = texto(conversa, "id").to_owned();
    let titulo = texto(conversa, "title").to_owned();
    let por_ler = inteiro(conversa, "unread");
    let mencoes = inteiro(conversa, "unread_mentions");
    let ultima = texto(conversa, "last_body").to_owned();
    let quando = instante(conversa, "last_at").map(|q| quando_curto(q, hoje, zona));
    let grupo = texto(conversa, "kind") == "group";
    let activa = aberta.is_some_and(|a| a.to_string() == id);

    // Três pistas, e não só a cor: peso da letra, um marcador, e a contagem.
    // Uma pessoa que não distinga cores continua a ver o que falta ler.
    let classe = if por_ler > 0 {
        "oc-conversa oc-conversa--por-ler"
    } else {
        "oc-conversa"
    };

    let outro = conversa.get("other").cloned().unwrap_or(Value::Null);

    view! {
        <a
            class=classe
            href=format!("{ROUTE}/{id}")
            aria-current=activa.then_some("page")
            data-oc="conversa"
            data-oc-id=id
        >
            <span class="oc-conversa__quem">
                {if grupo {
                    view! {
                        <span class="oc-conversa__grupo" aria-hidden="true">
                            {icon(Icon::Units, 15)}
                        </span>
                    }
                        .into_any()
                } else {
                    view! {
                        <span class="oc-conversa__avatar">
                            {avatar(
                                &ocinye_contracts::AvatarChoice::Initials,
                                &iniciais(&titulo),
                                AvatarSize::Small,
                            )}
                            {presenca(&outro)}
                        </span>
                    }
                        .into_any()
                }}
            </span>

            <span class="oc-conversa__corpo">
                <span class="oc-conversa__topo">
                    <span class="oc-conversa__titulo">{titulo}</span>
                    {quando.map(|q| view! { <span class="oc-conversa__quando">{q}</span> })}
                </span>
                <span class="oc-conversa__fundo">
                    <span class="oc-conversa__ultima">{ultima}</span>
                    {(por_ler > 0)
                        .then(|| {
                            let etiqueta = if mencoes > 0 {
                                format!("{por_ler} por ler, com menção")
                            } else {
                                format!("{por_ler} por ler")
                            };
                            let classe = if mencoes > 0 {
                                "oc-conversa__contagem oc-conversa__contagem--mencao"
                            } else {
                                "oc-conversa__contagem"
                            };
                            view! {
                                <span class=classe title=etiqueta.clone()>
                                    <span class="oc-sr">{etiqueta.clone()}</span>
                                    <span aria-hidden="true">
                                        {if por_ler > 99 {
                                            "99+".to_owned()
                                        } else {
                                            por_ler.to_string()
                                        }}
                                    </span>
                                </span>
                            }
                        })}
                </span>
            </span>
        </a>
    }
}

/// O que a página das Mensagens precisa de saber.
pub struct MessagingPage<'a> {
    /// As conversas desta pessoa.
    pub conversations: &'a [Value],
    /// A conversa aberta, quando há uma.
    pub open: Option<&'a Value>,
    /// As mensagens dela, da mais antiga para a mais recente.
    pub messages: &'a [Value],
    /// Quem está a olhar.
    pub me: Uuid,
    /// A zona de quem está a olhar.
    pub zona: TimeZoneName,
    /// Se a assistência do Ocinye está disponível.
    ///
    /// Falsa não esconde a aplicação: esconde o botão que prometeria melhorar
    /// um texto e falharia depois.
    pub ai: bool,
    /// Se o tempo real está a funcionar.
    pub realtime: bool,
    /// Um erro a mostrar, quando a leitura falhou.
    pub failure: Option<String>,
}

/// A aplicação Mensagens.
pub fn messaging(page: &MessagingPage<'_>) -> impl IntoView {
    let MessagingPage {
        conversations,
        open,
        messages,
        me,
        zona,
        ai,
        realtime,
        failure,
    } = page;
    let zona = *zona;
    let me = *me;
    let ai = *ai;
    let realtime = *realtime;

    let hoje = crate::ui::tempo::hoje_civil(Utc::now(), zona);
    let aberta = open
        .and_then(|c| c.get("id"))
        .and_then(Value::as_str)
        .and_then(|s| Uuid::parse_str(s).ok());

    view! {
        <div class="oc-page oc-page--messaging" data-oc="mensagens">
            <div class="oc-msg" data-oc-aberta=aberta.map(|a| a.to_string())>
                <aside class="oc-msg__lista" aria-label="Conversas">
                    <header class="oc-msg__lista-topo">
                        <h1 class="oc-msg__titulo">"Mensagens"</h1>
                        <div class="oc-msg__lista-accoes">
                            <button
                                type="button"
                                class="oc-btn oc-btn--icone"
                                data-oc="nova-conversa"
                                aria-haspopup="dialog"
                                title="Nova conversa"
                            >
                                <span class="oc-sr">"Nova conversa"</span>
                                {icon(Icon::Plus, 15)}
                            </button>
                        </div>
                    </header>

                    {(!realtime)
                        .then(|| {
                            view! {
                                <p class="oc-msg__degradado" data-oc="realtime-degradado">
                                    "As actualizações em tempo real estão indisponíveis. \
                                     O histórico continua completo; recarregue para ver o \
                                     que chegou."
                                </p>
                            }
                        })}

                    <div class="oc-msg__conversas" role="list">
                        {if conversations.is_empty() {
                            view! {
                                <p class="oc-msg__sem-conversas">
                                    "Ainda não falou com ninguém por aqui."
                                </p>
                            }
                                .into_any()
                        } else {
                            conversations
                                .iter()
                                .map(|c| linha_da_conversa(c, aberta, hoje, zona))
                                .collect_view()
                                .into_any()
                        }}
                    </div>
                </aside>

                {nova_conversa()}

                <section class="oc-msg__conversa" aria-label="Conversa">
                    {match (failure.as_deref(), open) {
                        (Some(razao), _) => aviso(razao).into_any(),
                        (None, None) => sem_conversa(conversations.is_empty()).into_any(),
                        (None, Some(conversa)) => {
                            aberta_view(conversa, messages, me, hoje, zona, ai).into_any()
                        }
                    }}
                </section>
            </div>
        </div>
    }
}

/// O diálogo de começar uma conversa.
///
/// # Porque uma superfície e não uma página
///
/// Porque começar a falar com alguém é um gesto, e não uma tarefa. Levar a
/// pessoa a outro ecrã para escolher um nome fá-la perder o sítio onde estava.
///
/// # Porque a pesquisa é do servidor
///
/// Porque uma instituição não cabe num `select`, e carregá-la inteira para
/// filtrar no browser seria mandar a lista de toda a gente para cada pessoa que
/// abre as Mensagens.
fn nova_conversa() -> impl IntoView {
    view! {
        <div class="oc-msg__nova" data-oc="nova-conversa-dialogo" hidden>
            <div class="oc-msg__nova-fundo" data-oc="fechar-nova"></div>

            <div
                class="oc-msg__nova-caixa"
                role="dialog"
                aria-modal="true"
                aria-labelledby="oc-nova-titulo"
            >
                <header class="oc-msg__nova-topo">
                    <h2 class="oc-msg__nova-titulo" id="oc-nova-titulo">"Nova conversa"</h2>
                    <button
                        type="button"
                        class="oc-msg__accao"
                        data-oc="fechar-nova"
                        title="Fechar"
                    >
                        <span class="oc-sr">"Fechar"</span>
                        <span aria-hidden="true">"×"</span>
                    </button>
                </header>

                // Directa ou grupo. A escolha muda o que o formulário pede: um
                // grupo precisa de nome, uma directa é com uma pessoa.
                <div class="oc-msg__nova-modo" role="tablist" aria-label="Tipo de conversa">
                    <button
                        type="button"
                        class="oc-msg__modo oc-msg__modo--activo"
                        role="tab"
                        aria-selected="true"
                        data-oc="modo"
                        data-oc-modo="directa"
                    >
                        "Com uma pessoa"
                    </button>
                    <button
                        type="button"
                        class="oc-msg__modo"
                        role="tab"
                        aria-selected="false"
                        data-oc="modo"
                        data-oc-modo="grupo"
                    >
                        "Grupo"
                    </button>
                </div>

                <div class="oc-msg__nova-grupo" data-oc="campo-nome" hidden>
                    <label class="oc-campo">
                        <span class="oc-campo__rotulo">"Nome do grupo"</span>
                        <input
                            class="oc-entrada"
                            type="text"
                            data-oc="nome-do-grupo"
                            placeholder="Projecto Energia"
                            maxlength="120"
                        />
                    </label>
                </div>

                <label class="oc-campo">
                    <span class="oc-campo__rotulo">"Procurar uma pessoa"</span>
                    <input
                        class="oc-entrada"
                        type="search"
                        data-oc="procurar-pessoa"
                        placeholder="Nome ou endereço institucional…"
                        autocomplete="off"
                        role="combobox"
                        aria-expanded="false"
                        aria-controls="oc-nova-resultados"
                    />
                </label>

                // Quem já foi escolhido, para um grupo.
                <div class="oc-msg__escolhidos" data-oc="escolhidos" hidden></div>

                <div
                    class="oc-msg__resultados"
                    id="oc-nova-resultados"
                    data-oc="resultados"
                    role="listbox"
                    aria-label="Pessoas"
                ></div>

                <p class="oc-msg__nova-estado" data-oc="estado-da-procura">
                    "Escreva pelo menos duas letras."
                </p>

                <footer class="oc-msg__nova-accoes">
                    <button
                        type="button"
                        class="oc-btn oc-btn--secondary oc-btn--sm"
                        data-oc="fechar-nova"
                    >
                        "Cancelar"
                    </button>
                    <button
                        type="button"
                        class="oc-btn oc-btn--primary oc-btn--sm"
                        data-oc="criar-conversa"
                        disabled
                    >
                        "Começar"
                    </button>
                </footer>
            </div>
        </div>
    }
}

/// O que se mostra quando a leitura falhou.
///
/// Distinto do estado vazio de propósito: uma lista vazia diz «não há nada», e
/// um erro diz «não consegui ler». Confundi-los faz uma pessoa concluir que
/// perdeu conversas.
fn aviso(razao: &str) -> impl IntoView {
    let razao = razao.to_owned();
    view! {
        <div class="oc-msg__vazio" data-oc="erro">
            {empty_state(EmptyState {
                icon: Icon::Messaging,
                title: "Não foi possível ler as conversas".to_owned(),
                body: razao,
                actions: Vec::new(),
                small: false,
            })}
        </div>
    }
}

/// Nenhuma conversa aberta.
fn sem_conversa(primeira_vez: bool) -> impl IntoView {
    let (titulo, corpo) = if primeira_vez {
        (
            "Comece uma conversa",
            "As Mensagens são o sítio onde se fala com colegas sem sair do \
             Workspace. Procure alguém e escreva.",
        )
    } else {
        (
            "Escolha uma conversa",
            "As suas conversas estão à esquerda. Abra uma para continuar, ou \
             comece outra.",
        )
    };

    view! {
        <div class="oc-msg__vazio">
            {empty_state(EmptyState {
                icon: Icon::Messaging,
                title: titulo.to_owned(),
                body: corpo.to_owned(),
                actions: vec![
                    Button::new("Nova conversa", crate::ui::components::button::Variant::Primary)
                        .with_action("nova-conversa"),
                ],
                small: false,
            })}
        </div>
    }
}

// ── A conversa aberta ───────────────────────────────────────────────────

fn aberta_view(
    conversa: &Value,
    mensagens: &[Value],
    me: Uuid,
    hoje: NaiveDate,
    zona: TimeZoneName,
    ai: bool,
) -> impl IntoView {
    let id = texto(conversa, "id").to_owned();
    let titulo = texto(conversa, "title").to_owned();
    let grupo = texto(conversa, "kind") == "group";
    let participantes: Vec<Value> = conversa
        .get("participants")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let governa = matches!(texto(conversa, "role"), "owner" | "administrator");

    let outro = participantes
        .iter()
        .find(|p| texto(p, "id") != me.to_string())
        .cloned()
        .unwrap_or(Value::Null);

    let lido_ate = instante(conversa, "last_read_at");

    view! {
        <div class="oc-msg__painel" data-oc="conversa-aberta" data-oc-id=id.clone()>
            {cabecalho(&titulo, grupo, &outro, &participantes, governa, &id)}

            <div class="oc-msg__fluxo" data-oc="fluxo" tabindex="0" role="log" aria-live="polite">
                {if mensagens.is_empty() {
                    view! {
                        <p class="oc-msg__primeira">
                            "Ainda não há mensagens. Escreva a primeira."
                        </p>
                    }
                        .into_any()
                } else {
                    fluxo(mensagens, me, hoje, zona, lido_ate).into_any()
                }}
            </div>

            <p class="oc-msg__escrita" data-oc="a-escrever" hidden></p>

            {composer(&id, ai, &participantes, me)}
        </div>
    }
}

fn cabecalho(
    titulo: &str,
    grupo: bool,
    outro: &Value,
    participantes: &[Value],
    governa: bool,
    id: &str,
) -> impl IntoView {
    let titulo = titulo.to_owned();
    let quantos = participantes.len();
    let outro = outro.clone();
    let id = id.to_owned();

    view! {
        <header class="oc-msg__cabecalho">
            <div class="oc-msg__identidade">
                {avatar(
                    &ocinye_contracts::AvatarChoice::Initials,
                    &iniciais(&titulo),
                    AvatarSize::Medium,
                )}
                <div class="oc-msg__quem">
                    <h2 class="oc-msg__nome">{titulo.clone()}</h2>
                    {if grupo {
                        view! {
                            <p class="oc-msg__estado">
                                {format!(
                                    "{quantos} {}",
                                    if quantos == 1 { "participante" } else { "participantes" },
                                )}
                            </p>
                        }
                            .into_any()
                    } else {
                        let rotulo = outro
                            .get("presence_label")
                            .and_then(Value::as_str)
                            .unwrap_or_default()
                            .to_owned();
                        view! {
                            <p class="oc-msg__estado">
                                {presenca(&outro)}
                                <span>{rotulo}</span>
                            </p>
                        }
                            .into_any()
                    }}
                </div>
            </div>

            {grupo
                .then(|| {
                    view! {
                        <button
                            type="button"
                            class="oc-btn oc-btn--secondary oc-btn--sm"
                            data-oc="detalhes-do-grupo"
                            aria-expanded="false"
                            aria-controls=format!("detalhes-{id}")
                        >
                            "Detalhes"
                        </button>
                    }
                })}
        </header>

        {grupo
            .then(|| {
                detalhes_do_grupo(&id, participantes, governa)
            })}
    }
}

/// O painel de detalhes de um grupo.
///
/// Contextual, ao lado da conversa: quem lá está, e o que se pode fazer. Não é
/// uma página administrativa — gerir um grupo acontece onde ele se lê.
fn detalhes_do_grupo(id: &str, participantes: &[Value], governa: bool) -> impl IntoView {
    let id = id.to_owned();
    let linhas: Vec<Value> = participantes.to_vec();

    view! {
        <aside class="oc-msg__detalhes" id=format!("detalhes-{id}") data-oc="detalhes" hidden>
            <h3 class="oc-msg__detalhes-titulo">"Participantes"</h3>
            <ul class="oc-msg__participantes">
                {linhas
                    .into_iter()
                    .map(|p| {
                        let nome = texto(&p, "name").to_owned();
                        let quem = texto(&p, "id").to_owned();
                        view! {
                            <li class="oc-msg__participante">
                                {avatar(
                                    &ocinye_contracts::AvatarChoice::Initials,
                                    &iniciais(&nome),
                                    AvatarSize::Small,
                                )}
                                <span class="oc-msg__participante-nome">{nome.clone()}</span>
                                {presenca(&p)}
                                {governa
                                    .then(|| {
                                        view! {
                                            <button
                                                type="button"
                                                class="oc-btn oc-btn--ghost oc-btn--sm"
                                                data-oc="retirar"
                                                data-oc-quem=quem
                                                title=format!("Retirar {}", nome.clone())
                                            >
                                                "Retirar"
                                            </button>
                                        }
                                    })}
                            </li>
                        }
                    })
                    .collect_view()}
            </ul>

            <div class="oc-msg__detalhes-accoes">
                {governa
                    .then(|| {
                        view! {
                            <button
                                type="button"
                                class="oc-btn oc-btn--secondary oc-btn--sm"
                                data-oc="acrescentar-membro"
                            >
                                "Acrescentar alguém"
                            </button>
                        }
                    })}
                <button type="button" class="oc-btn oc-btn--ghost oc-btn--sm" data-oc="sair">
                    "Sair do grupo"
                </button>
            </div>
        </aside>
    }
}

/// O fluxo de mensagens.
///
/// # Porque agrupa
///
/// Porque três frases seguidas da mesma pessoa são uma intervenção, e repetir o
/// nome e a hora em cada uma transforma uma conversa numa lista de registos.
fn fluxo(
    mensagens: &[Value],
    me: Uuid,
    hoje: NaiveDate,
    zona: TimeZoneName,
    lido_ate: Option<DateTime<Utc>>,
) -> impl IntoView {
    /// Quanto tempo separa duas intervenções da mesma pessoa.
    const AGRUPA_ATE: i64 = 5;

    let mut blocos: Vec<AnyView> = Vec::new();
    let mut dia_anterior: Option<NaiveDate> = None;
    let mut autor_anterior: Option<String> = None;
    let mut instante_anterior: Option<DateTime<Utc>> = None;
    let mut ja_marcou_novas = false;

    for mensagem in mensagens {
        let quando = instante(mensagem, "created_at").unwrap_or_else(Utc::now);
        let dia = crate::ui::tempo::dia_civil(quando, zona);
        let autor = texto(mensagem, "author_id").to_owned();

        if dia_anterior != Some(dia) {
            blocos.push(
                view! {
                    <div class="oc-msg__dia" role="separator">
                        <span>{separador_do_dia(dia, hoje)}</span>
                    </div>
                }
                .into_any(),
            );
            dia_anterior = Some(dia);
            autor_anterior = None;
        }

        // A primeira que ainda não estava lida.
        if !ja_marcou_novas && autor != me.to_string() {
            if let Some(lido) = lido_ate {
                if quando > lido {
                    blocos.push(
                        view! {
                            <div class="oc-msg__novas" role="separator" data-oc="novas">
                                <span>"Novas mensagens"</span>
                            </div>
                        }
                        .into_any(),
                    );
                    ja_marcou_novas = true;
                    autor_anterior = None;
                }
            }
        }

        let seguida = autor_anterior.as_deref() == Some(autor.as_str())
            && instante_anterior
                .is_some_and(|anterior| (quando - anterior).num_minutes() < AGRUPA_ATE)
            && mensagem.get("reply_to").is_none();

        blocos.push(mensagem_view(mensagem, me, seguida, zona).into_any());
        autor_anterior = Some(autor);
        instante_anterior = Some(quando);
    }

    view! { <div class="oc-msg__blocos">{blocos}</div> }
}

fn mensagem_view(mensagem: &Value, me: Uuid, seguida: bool, zona: TimeZoneName) -> impl IntoView {
    let id = texto(mensagem, "id").to_owned();
    let autor = texto(mensagem, "author_id").to_owned();
    let nome = texto(mensagem, "author_name").to_owned();
    let corpo = texto(mensagem, "body").to_owned();
    let quando = instante(mensagem, "created_at").unwrap_or_else(Utc::now);
    let editada = mensagem.get("edited_at").is_some_and(|v| !v.is_null());
    let minha = autor == me.to_string();

    let citada = mensagem.get("reply_to").cloned().unwrap_or(Value::Null);
    let reaccoes: Vec<Value> = mensagem
        .get("reactions")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    let mut classe = String::from("oc-msg__mensagem");
    if minha {
        classe.push_str(" oc-msg__mensagem--minha");
    }
    if seguida {
        classe.push_str(" oc-msg__mensagem--seguida");
    }

    view! {
        <article class=classe data-oc="mensagem" data-oc-id=id.clone() data-oc-autor=autor>
            {(!seguida)
                .then(|| {
                    view! {
                        <span class="oc-msg__mensagem-avatar" aria-hidden="true">
                            {avatar(
                                &ocinye_contracts::AvatarChoice::Initials,
                                &iniciais(&nome),
                                AvatarSize::Small,
                            )}
                        </span>
                    }
                })}

            <div class="oc-msg__mensagem-corpo">
                {(!seguida)
                    .then(|| {
                        view! {
                            <p class="oc-msg__mensagem-topo">
                                <span class="oc-msg__mensagem-autor">{nome.clone()}</span>
                                <time class="oc-msg__mensagem-hora">{hora(quando, zona)}</time>
                            </p>
                        }
                    })}

                {(!citada.is_null())
                    .then(|| {
                        let alvo = texto(&citada, "id").to_owned();
                        let quem = texto(&citada, "author_name").to_owned();
                        let excerto = texto(&citada, "excerpt").to_owned();
                        view! {
                            <a
                                class="oc-msg__citada"
                                href=format!("#mensagem-{alvo}")
                                data-oc="citada"
                                data-oc-alvo=alvo
                            >
                                <span class="oc-msg__citada-quem">{quem}</span>
                                <span class="oc-msg__citada-texto">{excerto}</span>
                            </a>
                        }
                    })}

                // Texto. Nunca `inner_html`: uma mensagem é escrita por uma
                // pessoa, e o que ela escrever não pode virar estrutura na
                // página de quem a lê.
                <p class="oc-msg__texto" id=format!("mensagem-{id}")>
                    {corpo}
                </p>

                {editada
                    .then(|| view! { <span class="oc-msg__editada">"editada"</span> })}

                {(!reaccoes.is_empty())
                    .then(|| {
                        view! {
                            <div class="oc-msg__reaccoes">
                                {reaccoes
                                    .into_iter()
                                    .map(|r| {
                                        let emoji = texto(&r, "emoji").to_owned();
                                        let quantas = inteiro(&r, "count");
                                        let minha = r
                                            .get("mine")
                                            .and_then(Value::as_bool)
                                            .unwrap_or(false);
                                        let classe = if minha {
                                            "oc-msg__reaccao oc-msg__reaccao--minha"
                                        } else {
                                            "oc-msg__reaccao"
                                        };
                                        view! {
                                            <button
                                                type="button"
                                                class=classe
                                                data-oc="reagir"
                                                data-oc-emoji=emoji.clone()
                                                aria-pressed=minha.to_string()
                                            >
                                                <span aria-hidden="true">{emoji.clone()}</span>
                                                <span class="oc-msg__reaccao-conta">
                                                    {quantas.to_string()}
                                                </span>
                                            </button>
                                        }
                                    })
                                    .collect_view()}
                            </div>
                        }
                    })}
            </div>

            // As acções aparecem ao passar o rato e ao focar, e não estão
            // sempre lá: uma barra permanente por mensagem enche a conversa de
            // botões que ninguém pediu.
            <div class="oc-msg__accoes" data-oc="accoes-da-mensagem">
                <button
                    type="button"
                    class="oc-msg__accao"
                    data-oc="responder"
                    title="Responder"
                >
                    <span class="oc-sr">"Responder"</span>
                    {icon(Icon::Reply, 14)}
                </button>
                <button
                    type="button"
                    class="oc-msg__accao"
                    data-oc="abrir-reaccoes"
                    title="Reagir"
                    aria-haspopup="menu"
                >
                    <span class="oc-sr">"Reagir"</span>
                    <span aria-hidden="true">"☺"</span>
                </button>
                <button type="button" class="oc-msg__accao" data-oc="copiar" title="Copiar">
                    <span class="oc-sr">"Copiar o texto"</span>
                    {icon(Icon::Archive, 14)}
                </button>
            </div>
        </article>
    }
}

// ── O composer ──────────────────────────────────────────────────────────

/// Os emoji que a paleta oferece.
///
/// Um conjunto pequeno e útil. Uma biblioteca inteira dentro do composer é uma
/// segunda aplicação — e o teclado do sistema já escreve qualquer emoji.
const EMOJI: [(&str, &str); 12] = [
    ("👍", "gosto"),
    ("❤️", "coração"),
    ("😀", "sorriso"),
    ("😂", "riso"),
    ("🎉", "festa"),
    ("👀", "a ver"),
    ("✅", "feito"),
    ("🙏", "obrigado"),
    ("🔥", "fogo"),
    ("💡", "ideia"),
    ("⚠️", "atenção"),
    ("🤝", "acordo"),
];

fn composer(id: &str, ai: bool, participantes: &[Value], me: Uuid) -> impl IntoView {
    let id = id.to_owned();
    let outros: Vec<Value> = participantes
        .iter()
        .filter(|p| texto(p, "id") != me.to_string())
        .cloned()
        .collect();

    view! {
        <div class="oc-msg__composer" data-oc="composer" data-oc-conversa=id.clone()>
            // O estado de resposta, quando a pessoa escolheu responder a uma.
            <div class="oc-msg__resposta" data-oc="a-responder" hidden>
                <div class="oc-msg__resposta-texto">
                    <span class="oc-msg__resposta-quem" data-oc="resposta-quem"></span>
                    <span class="oc-msg__resposta-excerto" data-oc="resposta-excerto"></span>
                </div>
                <button
                    type="button"
                    class="oc-msg__accao"
                    data-oc="cancelar-resposta"
                    title="Deixar de responder"
                >
                    <span class="oc-sr">"Deixar de responder"</span>
                    <span aria-hidden="true">"×"</span>
                </button>
            </div>

            // A sugestão do Ocinye, quando há uma. O original fica por baixo,
            // recuperável, até alguém escolher.
            <div class="oc-msg__sugestao" data-oc="sugestao" hidden>
                <p class="oc-msg__sugestao-topo">
                    <span class="oc-msg__sugestao-marca" aria-hidden="true">"✦"</span>
                    <span data-oc="sugestao-titulo">"Sugestão"</span>
                </p>
                <p class="oc-msg__sugestao-texto" data-oc="sugestao-texto"></p>
                <div class="oc-msg__sugestao-accoes">
                    <button
                        type="button"
                        class="oc-btn oc-btn--primary oc-btn--sm"
                        data-oc="usar-sugestao"
                    >
                        "Usar sugestão"
                    </button>
                    <button
                        type="button"
                        class="oc-btn oc-btn--ghost oc-btn--sm"
                        data-oc="manter-original"
                    >
                        "Manter o original"
                    </button>
                </div>
            </div>

            <div class="oc-msg__caixa">
                <label class="oc-sr" for="oc-msg-texto">"Escrever mensagem"</label>
                <textarea
                    id="oc-msg-texto"
                    class="oc-msg__entrada"
                    data-oc="texto"
                    rows="1"
                    placeholder="Escrever mensagem…"
                    aria-describedby="oc-msg-ajuda"
                ></textarea>

                <div class="oc-msg__ferramentas">
                    <button
                        type="button"
                        class="oc-msg__ferramenta"
                        data-oc="abrir-emoji"
                        aria-haspopup="dialog"
                        aria-expanded="false"
                        title="Emoji"
                    >
                        <span class="oc-sr">"Escolher um emoji"</span>
                        <span aria-hidden="true">"☺"</span>
                    </button>

                    {ai
                        .then(|| {
                            view! {
                                <div class="oc-msg__assist">
                                    <button
                                        type="button"
                                        class="oc-msg__ferramenta oc-msg__ferramenta--assist"
                                        data-oc="abrir-assist"
                                        aria-haspopup="menu"
                                        aria-expanded="false"
                                        title="Ocinye"
                                    >
                                        <span class="oc-sr">
                                            "Pedir ajuda ao Ocinye"
                                        </span>
                                        <span aria-hidden="true">"✦"</span>
                                    </button>
                                    <div class="oc-msg__assist-menu" data-oc="assist-menu" hidden>
                                        {[
                                            ("corrigir", "Corrigir"),
                                            ("melhorar", "Melhorar"),
                                            ("formal", "Mais formal"),
                                            ("curto", "Mais curto"),
                                            ("claro", "Mais claro"),
                                            ("traduzir", "Traduzir"),
                                        ]
                                            .map(|(chave, rotulo)| {
                                                view! {
                                                    <button
                                                        type="button"
                                                        class="oc-msg__assist-item"
                                                        data-oc="assist"
                                                        data-oc-accao=chave
                                                    >
                                                        {rotulo}
                                                    </button>
                                                }
                                            })
                                            .collect_view()}
                                    </div>
                                </div>
                            }
                        })}

                    <button
                        type="button"
                        class="oc-btn oc-btn--primary oc-btn--sm oc-msg__enviar"
                        data-oc="enviar"
                    >
                        "Enviar"
                    </button>
                </div>
            </div>

            <p class="oc-msg__ajuda" id="oc-msg-ajuda">
                <kbd class="oc-kbd">"Enter"</kbd>
                " envia · "
                <kbd class="oc-kbd">"Shift"</kbd>
                "+"
                <kbd class="oc-kbd">"Enter"</kbd>
                " muda de linha · "
                <kbd class="oc-kbd">"@"</kbd>
                " menciona"
            </p>

            // A paleta de emoji, ancorada ao composer.
            <div
                class="oc-msg__emoji"
                data-oc="emoji"
                role="dialog"
                aria-label="Emoji"
                hidden
            >
                {EMOJI
                    .map(|(caracter, nome)| {
                        view! {
                            <button
                                type="button"
                                class="oc-msg__emoji-item"
                                data-oc="emoji-item"
                                data-oc-emoji=caracter
                                title=nome
                            >
                                <span aria-hidden="true">{caracter}</span>
                                <span class="oc-sr">{nome}</span>
                            </button>
                        }
                    })
                    .collect_view()}
            </div>

            // As pessoas que se podem mencionar nesta conversa, e mais nenhuma.
            // Mencionar não dá acesso: quem não participa não está aqui, e o
            // Core recusa na mesma.
            <div class="oc-msg__mencoes" data-oc="mencoes" role="listbox" hidden>
                {outros
                    .into_iter()
                    .map(|p| {
                        let nome = texto(&p, "name").to_owned();
                        let quem = texto(&p, "id").to_owned();
                        view! {
                            <button
                                type="button"
                                class="oc-msg__mencao"
                                role="option"
                                aria-selected="false"
                                data-oc="mencao"
                                data-oc-quem=quem
                                data-oc-nome=nome.clone()
                            >
                                {avatar(
                                    &ocinye_contracts::AvatarChoice::Initials,
                                    &iniciais(&nome),
                                    AvatarSize::Small,
                                )}
                                <span>{nome.clone()}</span>
                            </button>
                        }
                    })
                    .collect_view()}
            </div>
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn zona() -> TimeZoneName {
        "UTC".to_owned().try_into().expect("fuso conhecido")
    }

    fn dia(a: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(a, m, d).expect("data")
    }

    #[test]
    fn as_iniciais_sao_a_primeira_e_a_ultima() {
        assert_eq!(iniciais("Fidel Monteiro"), "FM");
        // Um nome só dá uma letra, e não a mesma repetida.
        assert_eq!(iniciais("Ana"), "A");
        assert_eq!(iniciais("Maria da Conceição Silva"), "MS");
        assert_eq!(iniciais(""), "");
    }

    #[test]
    fn a_lista_diz_o_tempo_como_uma_pessoa_o_diria() {
        let hoje = dia(2026, 3, 12);
        let z = zona();
        let em = |texto: &str| {
            DateTime::parse_from_rfc3339(texto)
                .expect("instante")
                .with_timezone(&Utc)
        };

        assert_eq!(quando_curto(em("2026-03-12T09:30:00Z"), hoje, z), "09:30");
        assert_eq!(quando_curto(em("2026-03-11T09:30:00Z"), hoje, z), "Ontem");
        // Cinco de Março de 2026 é uma quinta-feira.
        assert_eq!(quando_curto(em("2026-03-08T09:30:00Z"), hoje, z), "Dom");
        assert_eq!(
            quando_curto(em("2025-12-01T09:30:00Z"), hoje, z),
            "01/12/2025"
        );
    }

    #[test]
    fn os_separadores_do_dia_estao_em_portugues() {
        let hoje = dia(2026, 3, 12);
        assert_eq!(separador_do_dia(hoje, hoje), "Hoje");
        assert_eq!(separador_do_dia(dia(2026, 3, 11), hoje), "Ontem");
        // Nada disto sai em inglês, e o teste é o que o garante.
        for d in [dia(2026, 3, 8), dia(2025, 12, 1)] {
            let texto = separador_do_dia(d, hoje);
            for ingles in [
                "Monday",
                "Tuesday",
                "Wednesday",
                "Thursday",
                "Friday",
                "Saturday",
                "Sunday",
                "January",
                "March",
                "December",
                "Yesterday",
                "Today",
            ] {
                assert!(
                    !texto.contains(ingles),
                    "«{texto}» tem inglês perdido lá dentro"
                );
            }
        }
    }

    #[test]
    fn a_hora_mostrada_e_a_de_quem_olha() {
        let quando = DateTime::parse_from_rfc3339("2026-03-11T22:30:00Z")
            .expect("instante")
            .with_timezone(&Utc);
        let tbilisi: TimeZoneName = "Asia/Tbilisi".to_owned().try_into().expect("fuso");
        assert_eq!(hora(quando, tbilisi), "02:30");
        assert_eq!(hora(quando, zona()), "22:30");
    }

    #[test]
    fn uma_mensagem_hostil_nao_cria_estrutura_na_pagina() {
        // A propriedade é esta, e não «o script não corre»: um `<script>` posto
        // por `innerHTML` nunca corre, e uma asserção sobre isso passaria com a
        // injecção presente.
        let mensagem = serde_json::json!({
            "id": "11111111-1111-1111-1111-111111111111",
            "author_id": "22222222-2222-2222-2222-222222222222",
            "author_name": "Alguém",
            "body": "<script>alert(1)</script><img src=x onerror=alert(1)>",
            "created_at": "2026-03-12T09:30:00Z",
        });

        let html = mensagem_view(&mensagem, Uuid::from_u128(3), false, zona()).to_html();

        assert!(
            !html.contains("<script>") && !html.contains("<img src=x"),
            "o texto de uma mensagem virou estrutura: {html}"
        );
        assert!(
            html.contains("&lt;script&gt;"),
            "o texto devia continuar lá, como texto"
        );
    }

    #[test]
    fn sem_assistencia_nao_ha_botao_que_a_prometa() {
        let vazio: Vec<Value> = Vec::new();
        let sem = composer("c", false, &vazio, Uuid::from_u128(1)).to_html();
        let com = composer("c", true, &vazio, Uuid::from_u128(1)).to_html();

        assert!(
            !sem.contains("abrir-assist"),
            "sem inferência, o composer prometeu melhorar um texto"
        );
        assert!(com.contains("abrir-assist"));
        // E o composer continua utilizável nos dois casos.
        assert!(sem.contains("data-oc=\"enviar\""));
        assert!(sem.contains("data-oc=\"abrir-emoji\""));
    }

    #[test]
    fn as_duas_maneiras_de_comecar_uma_conversa_estao_ligadas() {
        // O `+` da lista e o botão do estado vazio. O segundo ficou desenhado e
        // mudo: era um `Button` sem destino e sem gancho, e um botão assim
        // submete um formulário que não existe.
        let vazio: Vec<Value> = Vec::new();
        let html = messaging(&MessagingPage {
            conversations: &vazio,
            open: None,
            messages: &vazio,
            me: Uuid::from_u128(1),
            zona: zona(),
            ai: false,
            realtime: true,
            failure: None,
        })
        .to_html();

        assert_eq!(
            html.matches(r#"data-oc="nova-conversa""#).count(),
            2,
            "as duas entradas para começar uma conversa têm de estar ligadas"
        );
        // E o diálogo que elas abrem existe na página.
        assert!(html.contains(r#"data-oc="nova-conversa-dialogo""#));
        assert!(html.contains(r#"data-oc="procurar-pessoa""#));
    }

    #[test]
    fn nenhum_botao_do_modulo_fica_desenhado_e_mudo() {
        // Um `<button type="submit">` sem formulário à volta é um botão que não
        // faz nada. Aqui, cada botão ou tem gancho, ou submete um formulário
        // que existe — e este módulo não tem formulários.
        let vazio: Vec<Value> = Vec::new();
        let html = messaging(&MessagingPage {
            conversations: &vazio,
            open: None,
            messages: &vazio,
            me: Uuid::from_u128(1),
            zona: zona(),
            ai: false,
            realtime: true,
            failure: None,
        })
        .to_html();

        assert!(
            !html.contains(r#"type="submit""#),
            "há um botão a submeter um formulário que não existe neste módulo"
        );
    }

    #[test]
    fn uma_conversa_por_ler_diz_o_por_ler_de_mais_de_uma_maneira() {
        let conversa = serde_json::json!({
            "id": "11111111-1111-1111-1111-111111111111",
            "kind": "direct",
            "title": "Ana Silva",
            "unread": 3,
            "unread_mentions": 0,
            "last_body": "Combinado",
            "last_at": "2026-03-12T09:30:00Z",
        });
        let html = linha_da_conversa(&conversa, None, dia(2026, 3, 12), zona()).to_html();

        // Peso, marcador e contagem — e não só cor.
        assert!(html.contains("oc-conversa--por-ler"));
        assert!(html.contains("oc-conversa__contagem"));
        assert!(html.contains("3 por ler"));
    }

    #[test]
    fn uma_conversa_lida_nao_traz_contagem_nenhuma() {
        let conversa = serde_json::json!({
            "id": "11111111-1111-1111-1111-111111111111",
            "kind": "direct",
            "title": "Ana Silva",
            "unread": 0,
            "unread_mentions": 0,
        });
        let html = linha_da_conversa(&conversa, None, dia(2026, 3, 12), zona()).to_html();
        assert!(!html.contains("oc-conversa__contagem"));
        assert!(!html.contains("oc-conversa--por-ler"));
    }

    #[test]
    fn sem_tempo_real_nao_se_afirma_presenca_nenhuma() {
        // Um ponto cinzento a dizer «Offline» seria uma afirmação que ninguém
        // verificou. Ausente é ausente.
        let sem = serde_json::json!({"id": "x", "name": "Ana"});
        // Uma vista ausente rende um marcador vazio, e não uma cadeia vazia:
        // a asserção é sobre não haver ponto de presença nenhum.
        assert!(!presenca(&sem).to_html().contains("oc-presenca"));

        let com = serde_json::json!({
            "id": "x", "name": "Ana",
            "presence": "ocupado", "presence_label": "Ocupado",
        });
        let html = presenca(&com).to_html();
        assert!(html.contains("oc-presenca--ocupado"));
        assert!(html.contains("Ocupado"));
    }

    #[test]
    fn a_minha_mensagem_e_a_do_outro_ficam_de_lados_diferentes() {
        // O lado é o que faz uma conversa ler-se sem procurar o nome em cada
        // linha. A marca fica no HTML; é o CSS que a põe à direita, e o portão
        // da folha de estilo é que garante que a regra existe.
        let eu = Uuid::from_u128(1);
        let outro = Uuid::from_u128(2);

        let minha = serde_json::json!({
            "id": "a", "author_id": eu.to_string(), "author_name": "Eu",
            "body": "sou eu", "created_at": "2026-03-12T09:30:00Z"});
        let dele = serde_json::json!({
            "id": "b", "author_id": outro.to_string(), "author_name": "Outro",
            "body": "sou outro", "created_at": "2026-03-12T09:31:00Z"});

        assert!(
            mensagem_view(&minha, eu, false, zona())
                .to_html()
                .contains("oc-msg__mensagem--minha"),
            "a minha mensagem não ficou marcada como minha"
        );
        assert!(
            !mensagem_view(&dele, eu, false, zona())
                .to_html()
                .contains("oc-msg__mensagem--minha"),
            "a mensagem de outra pessoa ficou marcada como minha"
        );
    }

    #[test]
    fn o_fluxo_agrupa_o_mesmo_autor_e_separa_os_dias() {
        let autor = "22222222-2222-2222-2222-222222222222";
        let mensagens: Vec<Value> = vec![
            serde_json::json!({
                "id": "a", "author_id": autor, "author_name": "Ana",
                "body": "primeira", "created_at": "2026-03-11T09:30:00Z"}),
            serde_json::json!({
                "id": "b", "author_id": autor, "author_name": "Ana",
                "body": "segunda", "created_at": "2026-03-11T09:31:00Z"}),
            serde_json::json!({
                "id": "c", "author_id": autor, "author_name": "Ana",
                "body": "outro dia", "created_at": "2026-03-12T09:30:00Z"}),
        ];

        let html = fluxo(
            &mensagens,
            Uuid::from_u128(9),
            dia(2026, 3, 12),
            zona(),
            None,
        )
        .to_html();

        // A segunda vem agrupada; a terceira não, porque mudou o dia.
        assert_eq!(html.matches("oc-msg__mensagem--seguida").count(), 1);
        assert!(html.contains("Hoje"));
        assert!(html.contains("Ontem"));
    }

    #[test]
    fn o_separador_de_novas_mensagens_aparece_no_sitio() {
        let outro = "22222222-2222-2222-2222-222222222222";
        let lido = DateTime::parse_from_rfc3339("2026-03-12T09:00:00Z")
            .expect("instante")
            .with_timezone(&Utc);
        let mensagens: Vec<Value> = vec![
            serde_json::json!({
                "id": "a", "author_id": outro, "author_name": "Ana",
                "body": "lida", "created_at": "2026-03-12T08:00:00Z"}),
            serde_json::json!({
                "id": "b", "author_id": outro, "author_name": "Ana",
                "body": "por ler", "created_at": "2026-03-12T10:00:00Z"}),
        ];

        let html = fluxo(
            &mensagens,
            Uuid::from_u128(9),
            dia(2026, 3, 12),
            zona(),
            Some(lido),
        )
        .to_html();
        assert_eq!(html.matches("Novas mensagens").count(), 1);
        // Antes da que está por ler, e não no fim.
        let marca = html.find("Novas mensagens").expect("marca");
        let por_ler = html.find("por ler").expect("mensagem");
        assert!(marca < por_ler);
    }
}
