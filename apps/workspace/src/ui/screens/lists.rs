//! Os oito ecrãs de lista.
//!
//! Unidades, Ideias, Projectos, Bibliografia, Dados, Agentes, Membros e Audit
//! Log partilham exactamente o mesmo componente de tabela
//! (`design/README.md` §6.4). As grelhas de colunas são as do design.
//!
//! # Dados
//!
//! Todo o conteúdo vem do Ocinye Core. Onde o Core ainda não tem endpoint — os
//! agentes de IA, por exemplo — a lista aparece vazia com a explicação real, em
//! vez de dados de demonstração fixos no código.

use leptos::prelude::*;
use serde_json::Value;

use crate::ui::components::{
    button, data_table, Button, Cell, Column, ListTab, Table, Tone, Variant,
};
use crate::ui::shell::Viewer;
use ocinye_contracts::bibliography::BibliographyReview;
use ocinye_contracts::Permission;

/// Descrição de um ecrã de lista.
pub struct ListScreen {
    /// Título da página.
    pub title: &'static str,
    /// Subtítulo.
    ///
    /// `String` e não `&'static str` porque um ecrã com recorte por unidade diz
    /// **qual**: quem filtra por «Energia» deve ver o nome ali, e não o
    /// subtítulo genérico que descreve a lista inteira.
    pub subtitle: String,
    /// Rótulo da acção primária, quando o ecrã tem uma.
    ///
    /// `None` significa que **não há operação** por trás — não que ela esteja
    /// indisponível. O Audit Log é assim: o Core não expõe exportação do
    /// registo, e um botão «Exportar» declarado indisponível prometeria uma
    /// funcionalidade que não está por vir. Um controlo que não representa
    /// nada não pertence à interface.
    pub action: Option<&'static str>,
    /// Destino da acção primária. `None` quando o ecrã ainda não existe.
    pub action_href: Option<&'static str>,
    /// A permissão que a acção primária exige.
    ///
    /// A acção aparece sempre que exista, e é declarada indisponível a quem não
    /// tem a permissão — esconder fazia a interface mudar de forma consoante
    /// quem olha.
    pub action_permission: Permission,
    /// Uma acção secundária, quando o ecrã tem uma ferramenta a oferecer.
    ///
    /// Rótulo e destino. Segue a mesma regra da primária — aparece sempre e é
    /// declarada indisponível a quem não tem a permissão — porque esconder faz
    /// a interface mudar de forma consoante quem olha.
    pub secondary: Option<(&'static str, &'static str)>,
    /// A tabela.
    pub table: Table,
}

/// Renderiza um ecrã de lista.
/// A razão dada quando a acção existe e é a pessoa que não lhe chega.
const SEM_AUTORIZACAO: &str = "Não tem autorização para esta acção.";

pub fn list_screen(viewer: &Viewer, screen: ListScreen) -> impl IntoView {
    let ListScreen {
        title,
        subtitle,
        action,
        action_href,
        action_permission,
        secondary,
        table,
    } = screen;

    let may_act = viewer.can(action_permission);

    view! {
        <div class="oc-page">
            <div class="oc-head">
                <div class="oc-head__text">
                    <h1>{title}</h1>
                    <p>{subtitle}</p>
                </div>
                <div class="oc-head__actions">
                    // A acção aparece sempre. Escondê-la a quem não tem a
                    // permissão fazia a interface mudar de forma consoante quem
                    // olha, e quem não a via não ficava a saber que existe nem
                    // porque não a tem.
                    {secondary.map(|(label, href)| {
                        button(if may_act {
                            Button::new(label, Variant::Secondary).href(href)
                        } else {
                            Button::new(label, Variant::Secondary)
                                .unavailable_because(SEM_AUTORIZACAO)
                        })
                    })}
                    {action.map(|label| {
                        button(if may_act {
                            action_href.map_or_else(
                                || Button::new(label, Variant::Primary).not_yet_available(),
                                |href| Button::new(label, Variant::Primary).href(href),
                            )
                        } else {
                            Button::new(label, Variant::Primary)
                                .unavailable_because(SEM_AUTORIZACAO)
                        })
                    })}
                </div>
            </div>
            {data_table(table)}
        </div>
    }
}

/// Lê um campo de texto, com um travessão quando falta.
fn text(row: &Value, key: &str) -> String {
    row.get(key)
        .and_then(Value::as_str)
        .unwrap_or("—")
        .to_owned()
}

/// Lê um inteiro como texto.
fn number(row: &Value, key: &str) -> String {
    row.get(key)
        .and_then(Value::as_i64)
        .map_or_else(|| "—".to_owned(), |n| n.to_string())
}

/// Extrai a lista de itens de uma resposta paginada do Core.
fn items(payload: &Value) -> Vec<Value> {
    payload
        .get("items")
        .and_then(Value::as_array)
        .or_else(|| payload.as_array())
        .cloned()
        .unwrap_or_default()
}

/// O texto de contagem do rodapé.
fn footer(payload: &Value, shown: usize, singular: &str, plural: &str) -> String {
    let total = payload
        .get("total")
        .and_then(Value::as_i64)
        .unwrap_or_else(|| i64::try_from(shown).unwrap_or(0));

    if total == 0 {
        return format!("0 {plural}");
    }
    let noun = if total == 1 { singular } else { plural };
    format!("1–{shown} de {total} {noun}")
}

/// O recorte activo de uma lista de workspaces.
#[derive(Debug, Default, Clone)]
pub struct Slice {
    /// Apenas aqueles em que o membro participa.
    pub mine: bool,
    /// A unidade escolhida, quando há uma.
    pub unit_id: Option<String>,
    /// As unidades que o membro pode usar como recorte.
    pub units: Vec<(String, String)>,
    /// O recorte por unidade foi pedido e ainda não há escolha.
    pub awaiting_unit: bool,
}

impl Slice {
    /// O destino da tab «Da Unidade».
    ///
    /// # Nunca «a primeira»
    ///
    /// > **The Workspace never invents a primary Unit.**
    ///
    /// Sem unidades, o recorte não existe e a tab diz porquê. Com uma, não há
    /// ambiguidade e ela escolhe-se sozinha — obrigar a escolher entre uma
    /// opção é cerimónia. Com várias, a escolha é do membro: nem a primeira,
    /// nem a mais antiga, nem a de nome alfabeticamente primeiro. Qualquer
    /// dessas heurísticas seria uma unidade principal inventada, e uma
    /// instituição não tem unidade principal só porque uma consulta precisa de
    /// uma.
    fn unit_tab(&self, base: &str) -> ListTab {
        match self.units.len() {
            0 => ListTab::missing(
                "Da Unidade",
                "Não pertence a nenhuma unidade que possa usar como recorte.",
            ),
            _ if self.unit_id.is_some() || self.awaiting_unit => ListTab::current("Da Unidade"),
            _ => ListTab::to("Da Unidade", format!("{base}?unit=true")),
        }
    }

    /// O nome da unidade escolhida.
    fn unit_name(&self) -> Option<&str> {
        let escolhida = self.unit_id.as_deref()?;
        self.units
            .iter()
            .find(|(id, _)| id == escolhida)
            .map(|(_, nome)| nome.as_str())
    }
}

/// O selector de unidade, quando o recorte por unidade está em jogo.
fn unit_selector(slice: &Slice, base: &str) -> impl IntoView {
    if slice.units.len() < 2 && !slice.awaiting_unit {
        return ().into_any();
    }

    let base = base.to_owned();
    let escolhida = slice.unit_id.clone();
    let opcoes: Vec<_> = slice
        .units
        .iter()
        .map(|(id, nome)| {
            let activa = escolhida.as_deref() == Some(id.as_str());
            view! {
                <option value=id.clone() selected=activa>
                    {nome.clone()}
                </option>
            }
        })
        .collect();

    view! {
        // Um `GET` normal: a escolha vai para o URL, e um endereço de unidade
        // continua a ser essa unidade quando alguém o guarda ou partilha.
        <form class="oc-unit-pick" method="get" action=base>
            <label class="oc-field__label" for="unit_id">"UNIDADE"</label>
            <select class="oc-select" id="unit_id" name="unit_id">
                <option value="" disabled=true selected=escolhida.is_none()>
                    "Escolha uma unidade…"
                </option>
                {opcoes}
            </select>
            {button(Button::new("Aplicar", Variant::Secondary))}
        </form>
    }
    .into_any()
}

/// Os recortes de Ideias.
///
/// # O que mudou aqui
///
/// «Minhas» estava esbatida, com a razão «este recorte da lista ainda não está
/// disponível». Deixou de ser verdade no passo 6, quando o Core passou a aceitar
/// `mine=true` — e uma tab que declara indisponível uma capacidade que existe é
/// o inverso exacto da UI morta: em vez de prometer o que não faz, esconde o
/// que faz.
///
/// Os outros recortes continuam declarados, e agora dizem **porquê** cada um
/// falta. «Da Unidade» não é uma omissão de ligação: o Core filtra por *uma*
/// unidade, e um membro pode pertencer a várias — qual delas seria «a unidade»
/// é uma decisão de produto, não um parâmetro esquecido.
fn ideas_tabs(slice: &Slice) -> Vec<ListTab> {
    let noutro = slice.mine || slice.unit_id.is_some() || slice.awaiting_unit;
    vec![
        if noutro {
            ListTab::to("Todas", "/ideas")
        } else {
            ListTab::current("Todas")
        },
        if slice.mine {
            ListTab::current("Minhas")
        } else {
            ListTab::to("Minhas", "/ideas?mine=true")
        },
        slice.unit_tab("/ideas"),
        ListTab::missing("Seguidas", "Seguir ideias ainda não existe no Ocinye OS."),
        ListTab::missing(
            "Arquivadas",
            "Recortar por estado ainda não é uma consulta do Core.",
        ),
    ]
}

/// Os recortes de Projectos. Mesma história das Ideias.
fn projects_tabs(slice: &Slice) -> Vec<ListTab> {
    let noutro = slice.mine || slice.unit_id.is_some() || slice.awaiting_unit;
    vec![
        if noutro {
            ListTab::to("Todos", "/projects")
        } else {
            ListTab::current("Todos")
        },
        if slice.mine {
            ListTab::current("Meus")
        } else {
            ListTab::to("Meus", "/projects?mine=true")
        },
        slice.unit_tab("/projects"),
        ListTab::missing(
            "Concluídos",
            "Recortar por estado ainda não é uma consulta do Core.",
        ),
    ]
}

/// Os destinos das páginas vizinha, quando existem.
///
/// # O que a paginação é, e o que não pode ser
///
/// > **Pagination changes location inside an authorised result set; it never
/// > changes the authorised result set.**
///
/// A página viaja no URL, junto com os filtros que já lá estavam: mudar de
/// página não pode perder o recorte. Um `?page=2` que esquecesse `mine=true`
/// devolveria a segunda página da instituição inteira, e quem a lesse concluiria
/// que participa em coisas em que não participa.
///
/// Cada lado só existe quando existe: um «anterior» na primeira página é um
/// controlo que promete um sítio que não há.
fn pager(
    payload: &Value,
    base: &str,
    filtros: &[(&str, String)],
) -> (Option<String>, Option<String>) {
    let numero = |chave: &str| payload.get(chave).and_then(Value::as_i64);
    let (Some(pagina), Some(paginas)) = (numero("page"), numero("total_pages")) else {
        // Sem a forma de página, a resposta não é paginada — e inventar
        // controlos por cima dela seria prometer páginas que não existem.
        return (None, None);
    };

    let destino = |n: i64| {
        let mut query: Vec<String> = filtros
            .iter()
            .map(|(chave, valor)| format!("{chave}={valor}"))
            .collect();
        if n > 1 {
            query.push(format!("page={n}"));
        }
        if query.is_empty() {
            base.to_owned()
        } else {
            format!("{base}?{}", query.join("&"))
        }
    };

    (
        (pagina > 1).then(|| destino(pagina - 1)),
        (pagina < paginas).then(|| destino(pagina + 1)),
    )
}

/// O rodapé de contagem de uma página.
fn footer_paginado(payload: &Value, shown: usize, singular: &str, plural: &str) -> String {
    let total = payload.get("total").and_then(Value::as_i64);
    let pagina = payload.get("page").and_then(Value::as_i64).unwrap_or(1);
    let tamanho = payload
        .get("page_size")
        .and_then(Value::as_i64)
        .unwrap_or(0);

    let Some(total) = total else {
        return footer(payload, shown, singular, plural);
    };
    if total == 0 {
        return format!("0 {plural}");
    }

    let primeiro = (pagina - 1) * tamanho + 1;
    let ultimo = primeiro + i64::try_from(shown).unwrap_or(0) - 1;
    let noun = if total == 1 { singular } else { plural };
    format!("{primeiro}–{ultimo} de {total} {noun}")
}

/// Se o Core tem mais linhas do que as que chegaram.
fn truncated(payload: &Value, shown: usize) -> bool {
    payload
        .get("total")
        .and_then(Value::as_i64)
        .is_some_and(|total| total > i64::try_from(shown).unwrap_or(i64::MAX))
}

/// Uma data ISO reduzida a `AAAA-MM-DD`.
fn day(row: &Value, key: &str) -> String {
    row.get(key)
        .and_then(Value::as_str)
        .map_or_else(|| "—".to_owned(), |value| value.chars().take(10).collect())
}

// ── Unidades ─────────────────────────────────────────────────────────────

/// Unidades.
pub fn units(viewer: &Viewer, payload: &Value) -> impl IntoView {
    let rows = items(payload);
    let shown = rows.len();

    let table = Table {
        tabs: vec![
            ListTab::current("Todas"),
            ListTab::missing(
                "Minhas",
                "As unidades a que pertence ainda não são um recorte desta lista.",
            ),
            ListTab::missing("Seguidas", "Seguir unidades ainda não existe no Ocinye OS."),
            ListTab::missing(
                "Arquivadas",
                "Ver apenas as unidades arquivadas ainda não é um recorte desta lista.",
            ),
        ],
        search: "unidades",
        truncated: truncated(payload, shown),
        shape: "units",
        columns: vec![
            Column::new("UNIDADE"),
            Column::new("CÓDIGO"),
            Column::new("RESPONSÁVEL"),
            Column::right("MEMBROS"),
            Column::right("IDEIAS"),
            Column::right("PROJECTOS"),
            Column::new("ESTADO"),
        ],
        rows: rows
            .iter()
            .map(|row| {
                let id = text(row, "id");
                let status = text(row, "status");
                (
                    Some(format!("/units/{id}")),
                    vec![
                        Cell::Primary(text(row, "name")),
                        Cell::Mono(text(row, "code")),
                        Cell::Text(text(row, "lead")),
                        Cell::Mono(number(row, "members")),
                        Cell::Mono(number(row, "ideas")),
                        Cell::Mono(number(row, "projects")),
                        Cell::Badge(status.clone(), Tone::of(&status)),
                    ],
                )
            })
            .collect(),
        footer: footer(payload, shown, "unidade", "unidades"),
        // O Core devolve estas inteiras: não há segunda página para onde ir.
        previous: None,
        next: None,
        empty:
            "Ainda não existem unidades. Uma unidade é criada por um administrador da organização.",
    };

    list_screen(
        viewer,
        ListScreen {
            title: "Unidades",
            subtitle: "Todas as unidades institucionais da Ocinye.".to_owned(),
            action: Some("Nova Unidade"),
            action_href: Some("/units/new"),
            action_permission: Permission::UnitsCreate,
            secondary: None,
            table,
        },
    )
}

// ── Ideias ───────────────────────────────────────────────────────────────

/// Ideias.
pub fn ideas(viewer: &Viewer, payload: &Value, slice: Slice) -> impl IntoView {
    let rows = items(payload);
    let shown = rows.len();

    // O recorte activo viaja com a página: um `?page=2` que
    // esquecesse `mine=true` devolveria a segunda página da
    // instituição inteira.
    // Todo o recorte activo viaja com a página, e não só o «minhas»: uma
    // segunda página que largasse a unidade devolveria a segunda página da
    // instituição inteira sob um cabeçalho que diz o nome de uma unidade.
    let mut filtros: Vec<(&str, String)> = Vec::new();
    if slice.mine {
        filtros.push(("mine", "true".to_owned()));
    }
    if let Some(unit_id) = slice.unit_id.clone() {
        filtros.push(("unit_id", unit_id));
    }
    let (anterior, seguinte) = pager(payload, "/ideas", &filtros);

    // Pedido o recorte por unidade sem escolha feita, não há consulta nenhuma
    // por trás — e uma lista vazia diria «esta unidade não tem nada», que é uma
    // afirmação sobre uma unidade que ainda não foi escolhida.
    if slice.awaiting_unit {
        return view! {
            <div class="oc-page">
                <div class="oc-head">
                    <div class="oc-head__text">
                        <h1>"Ideias"</h1>
                        <p>"Escolha a unidade cujo trabalho quer ver."</p>
                    </div>
                </div>
                {unit_selector(&slice, "/ideas")}
                {crate::ui::components::empty_state(crate::ui::components::EmptyState {
                    icon: crate::ui::icon::Icon::Units,
                    title: "Nenhuma unidade escolhida".to_owned(),
                    body: "O Ocinye OS não escolhe uma unidade por si. Escolha acima qual delas \
                           quer ver."
                        .to_owned(),
                    actions: Vec::new(),
                    small: false,
                })}
            </div>
        }
        .into_any();
    }

    let table = Table {
        tabs: ideas_tabs(&slice),
        search: "ideias",
        truncated: truncated(payload, shown),
        shape: "ideas",
        columns: vec![
            Column::new("TÍTULO"),
            Column::new("UNIDADE"),
            Column::new("RESPONSÁVEL"),
            Column::new("ESTADO"),
            Column::new("PRIORIDADE"),
            Column::new("CLASSIFICAÇÃO"),
            Column::right("ACTUALIZADA"),
        ],
        rows: rows
            .iter()
            .map(|row| {
                let id = text(row, "id");
                let state = text(row, "state");
                let priority = text(row, "priority");
                (
                    Some(format!("/ideas/{id}")),
                    vec![
                        Cell::Primary(text(row, "title")),
                        Cell::Mono(text(row, "unit_code")),
                        Cell::Text(text(row, "lead")),
                        Cell::Badge(state.clone(), Tone::of(&state)),
                        if priority == "—" {
                            Cell::Empty
                        } else {
                            Cell::Badge(priority.clone(), Tone::of(&priority))
                        },
                        Cell::Classification(text(row, "classification")),
                        Cell::Mono(day(row, "updated_at")),
                    ],
                )
            })
            .collect(),
        footer: footer_paginado(payload, shown, "ideia", "ideias"),
        previous: anterior,
        next: seguinte,
        // O subtítulo do ecrã já diz o que é uma ideia; repeti-lo aqui não
        // acrescenta nada. O que falta ao ecrã vazio é de onde parte uma.
        empty: "Ainda não existem ideias. Uma ideia pertence a uma unidade e é o ponto de partida da investigação.",
    };

    list_screen(
        viewer,
        ListScreen {
            title: "Ideias",
            // Quem filtra por uma unidade deve ver qual: o subtítulo genérico
            // descreve a lista inteira, e a lista deixou de ser inteira.
            subtitle: slice.unit_name().map_or_else(
                || "Uma ideia é explorada antes de se tornar projecto.".to_owned(),
                |unidade| format!("Unidade: {unidade}."),
            ),
            action: Some("Nova Ideia"),
            action_href: Some("/ideas/new"),
            action_permission: Permission::IdeasCreate,
            secondary: None,
            table,
        },
    )
    .into_any()
}

// ── Projectos ────────────────────────────────────────────────────────────

/// Projectos.
pub fn projects(viewer: &Viewer, payload: &Value, slice: Slice) -> impl IntoView {
    let rows = items(payload);
    let shown = rows.len();

    // O recorte activo viaja com a página: um `?page=2` que
    // esquecesse `mine=true` devolveria a segunda página da
    // instituição inteira.
    // Todo o recorte activo viaja com a página, e não só o «minhas»: uma
    // segunda página que largasse a unidade devolveria a segunda página da
    // instituição inteira sob um cabeçalho que diz o nome de uma unidade.
    let mut filtros: Vec<(&str, String)> = Vec::new();
    if slice.mine {
        filtros.push(("mine", "true".to_owned()));
    }
    if let Some(unit_id) = slice.unit_id.clone() {
        filtros.push(("unit_id", unit_id));
    }
    let (anterior, seguinte) = pager(payload, "/projects", &filtros);

    // Pedido o recorte por unidade sem escolha feita, não há consulta nenhuma
    // por trás — e uma lista vazia diria «esta unidade não tem nada», que é uma
    // afirmação sobre uma unidade que ainda não foi escolhida.
    if slice.awaiting_unit {
        return view! {
            <div class="oc-page">
                <div class="oc-head">
                    <div class="oc-head__text">
                        <h1>"Projectos"</h1>
                        <p>"Escolha a unidade cujo trabalho quer ver."</p>
                    </div>
                </div>
                {unit_selector(&slice, "/projects")}
                {crate::ui::components::empty_state(crate::ui::components::EmptyState {
                    icon: crate::ui::icon::Icon::Units,
                    title: "Nenhuma unidade escolhida".to_owned(),
                    body: "O Ocinye OS não escolhe uma unidade por si. Escolha acima qual delas \
                           quer ver."
                        .to_owned(),
                    actions: Vec::new(),
                    small: false,
                })}
            </div>
        }
        .into_any();
    }

    let table = Table {
        tabs: projects_tabs(&slice),
        search: "projectos",
        truncated: truncated(payload, shown),
        shape: "projects",
        columns: vec![
            Column::new("CÓDIGO"),
            Column::new("PROJECTO"),
            Column::new("UNIDADE"),
            Column::new("RESPONSÁVEL"),
            Column::new("ESTADO"),
            Column::new("PROGRESSO"),
            Column::new("INÍCIO"),
            Column::new("FIM"),
        ],
        rows: rows
            .iter()
            .map(|row| {
                let id = text(row, "id");
                let state = text(row, "state");
                let progress = row
                    .get("progress")
                    .and_then(Value::as_i64)
                    .and_then(|p| u8::try_from(p).ok());
                (
                    Some(format!("/projects/{id}")),
                    vec![
                        Cell::Mono(text(row, "code")),
                        Cell::Primary(text(row, "title")),
                        Cell::Mono(text(row, "unit_code")),
                        Cell::Text(text(row, "lead")),
                        Cell::Badge(state.clone(), Tone::of(&state)),
                        progress.map_or(Cell::Empty, Cell::Progress),
                        Cell::Mono(day(row, "started_at")),
                        Cell::Mono(day(row, "completed_at")),
                    ],
                )
            })
            .collect(),
        footer: footer_paginado(payload, shown, "projecto", "projectos"),
        previous: anterior,
        next: seguinte,
        empty: "Ainda não existem projectos. Um projecto nasce da promoção de uma ideia.",
    };

    list_screen(
        viewer,
        ListScreen {
            title: "Projectos",
            // Quem filtra por uma unidade deve ver qual: o subtítulo genérico
            // descreve a lista inteira, e a lista deixou de ser inteira.
            subtitle: slice.unit_name().map_or_else(
                || "Projectos institucionais em execução e planeamento.".to_owned(),
                |unidade| format!("Unidade: {unidade}."),
            ),
            action: Some("Novo Projecto"),
            action_href: Some("/projects/new"),
            action_permission: Permission::ProjectsCreate,
            secondary: None,
            table,
        },
    )
    .into_any()
}

// ── Bibliografia ─────────────────────────────────────────────────────────

/// Bibliografia.
pub fn bibliography(viewer: &Viewer, payload: &Value) -> impl IntoView {
    let rows = items(payload);
    let shown = rows.len();

    let (anterior, seguinte) = pager(payload, "/bibliography", &[]);

    let table = Table {
        tabs: vec![
            ListTab::current("Todas"),
            ListTab::missing(
                "Minhas",
                "As referências que criou ainda não são um recorte desta lista.",
            ),
            ListTab::missing(
                "Da Unidade",
                "Filtrar a bibliografia por unidade ainda não é um recorte desta lista.",
            ),
            ListTab::missing("Favoritas", "Marcar favoritos ainda não existe no Ocinye OS."),
        ],
        search: "referências",
        truncated: truncated(payload, shown),
        shape: "bibliography",
        columns: vec![
            Column::new("TÍTULO"),
            Column::new("AUTORES"),
            Column::new("ANO"),
            Column::new("ORIGEM"),
            Column::new("TIPO"),
            Column::new("DOI"),
            Column::right("CITAÇÕES"),
        ],
        rows: rows
            .iter()
            .map(|row| {
                let authors = row
                    .get("authors")
                    .and_then(Value::as_array)
                    .map(|list| {
                        list.iter()
                            .filter_map(Value::as_str)
                            .collect::<Vec<_>>()
                            .join("; ")
                    })
                    .filter(|joined| !joined.is_empty())
                    .unwrap_or_else(|| "—".to_owned());

                (
                    None,
                    vec![
                        Cell::Primary(text(row, "title")),
                        Cell::Text(authors),
                        Cell::Mono(number(row, "year")),
                        Cell::Text(text(row, "container_title")),
                        Cell::Text(text(row, "source_type")),
                        Cell::Mono(text(row, "doi")),
                        Cell::Mono(number(row, "citations")),
                    ],
                )
            })
            .collect(),
        footer: footer_paginado(payload, shown, "referência", "referências"),
        previous: anterior,
        next: seguinte,
        empty: "Ainda não há referências. A bibliografia é acrescentada dentro de um Research Workspace.",
    };

    list_screen(
        viewer,
        ListScreen {
            title: "Bibliografia",
            subtitle: "Referências ligadas a ideias, projectos e unidades.".to_owned(),
            action: Some("Nova Referência"),
            action_href: Some("/bibliography/new"),
            action_permission: Permission::BibliographyCreate,
            secondary: Some(("Ferramentas", "/bibliography/tools")),
            table,
        },
    )
}

// ── Dados ────────────────────────────────────────────────────────────────

/// Dimensão legível a partir de bytes.
fn size(row: &Value, key: &str) -> String {
    let Some(bytes) = row.get(key).and_then(Value::as_i64) else {
        return "—".to_owned();
    };

    const UNITS: [&str; 5] = ["B", "kB", "MB", "GB", "TB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }

    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

/// Datasets.
pub fn datasets(viewer: &Viewer, payload: &Value) -> impl IntoView {
    let rows = items(payload);
    let shown = rows.len();

    let (anterior, seguinte) = pager(payload, "/datasets", &[]);

    let table = Table {
        tabs: vec![
            ListTab::current("Todos"),
            ListTab::missing(
                "Meus",
                "Os datasets que criou ainda não são um recorte desta lista.",
            ),
            ListTab::missing(
                "Da Unidade",
                "Filtrar datasets por unidade ainda não é um recorte desta lista.",
            ),
            ListTab::missing("Favoritos", "Marcar favoritos ainda não existe no Ocinye OS."),
        ],
        search: "datasets",
        truncated: truncated(payload, shown),
        shape: "datasets",
        columns: vec![
            Column::new("NOME"),
            Column::new("RESPONSÁVEL"),
            Column::new("REGISTO"),
            Column::new("VERSÃO"),
            Column::right("TAMANHO"),
            Column::new("TIPO"),
            Column::new("CLASSIFICAÇÃO"),
            Column::new("ACESSO"),
        ],
        rows: rows
            .iter()
            .map(|row| {
                let state = text(row, "state");
                (
                    None,
                    vec![
                        Cell::Primary(text(row, "title")),
                        Cell::Text(text(row, "responsible")),
                        Cell::Mono(day(row, "created_at")),
                        Cell::Mono(text(row, "latest_version")),
                        Cell::Mono(size(row, "size_bytes")),
                        Cell::Text(text(row, "origin")),
                        Cell::Classification(text(row, "classification")),
                        Cell::Badge(state.clone(), Tone::of(&state)),
                    ],
                )
            })
            .collect(),
        footer: footer_paginado(payload, shown, "dataset", "datasets"),
        previous: anterior,
        next: seguinte,
        empty: "Ainda não há datasets catalogados. Um dataset é catalogado dentro de um Research Workspace.",
    };

    list_screen(
        viewer,
        ListScreen {
            title: "Dados",
            subtitle: "Datasets institucionais com versão, proveniência e classificação."
                .to_owned(),
            action: Some("Novo Dataset"),
            action_href: Some("/datasets/new"),
            action_permission: Permission::DatasetsCreate,
            secondary: None,
            table,
        },
    )
}

// ── Agentes ──────────────────────────────────────────────────────────────

/// Agentes de IA.
///
/// Sem nó de IA enrolado não existem agentes activos, e a lista diz porquê em
/// vez de mostrar exemplos.
pub fn agents(viewer: &Viewer, payload: &Value) -> impl IntoView {
    let rows = items(payload);
    let shown = rows.len();

    let table = Table {
        tabs: vec![
            ListTab::current("Todos"),
            ListTab::missing(
                "Meus",
                "Os agentes que criou ainda não são um recorte desta lista.",
            ),
            ListTab::missing(
                "Da Unidade",
                "Filtrar agentes por unidade ainda não é um recorte desta lista.",
            ),
            ListTab::missing(
                "Institucionais",
                "Distinguir agentes institucionais dos pessoais ainda não é um recorte desta lista.",
            ),
        ],
        search: "agentes",
        truncated: truncated(payload, shown),
        // «UTILIZAÇÃO» foi retirada: não existe contagem de utilizações no
        // Core, e uma coluna sem fonte é uma estatística inventada (§60).
        shape: "agents",
        columns: vec![
            Column::new("AGENTE"),
            Column::new("PROPÓSITO"),
            Column::new("ESTADO"),
            Column::new("ÂMBITO"),
            Column::new("CAPACIDADE"),
            Column::right("CRIADO"),
        ],
        rows: rows
            .iter()
            .map(|row| {
                let state = text(row, "state");
                let scope = text(row, "scope");
                // O rótulo vem do Core, que o deriva da disponibilidade real:
                // «Configurado — sem capacidade disponível» e não «activo».
                let label = row
                    .get("state_label")
                    .and_then(Value::as_str)
                    .unwrap_or(&state)
                    .to_owned();
                (
                    None,
                    vec![
                        Cell::Primary(text(row, "name")),
                        Cell::Text(text(row, "purpose")),
                        Cell::Badge(label, Tone::of(&state)),
                        Cell::Badge(scope.clone(), Tone::of(&scope)),
                        Cell::Mono(text(row, "capability")),
                        Cell::Mono(day(row, "created_at")),
                    ],
                )
            })
            .collect(),
        footer: footer(payload, shown, "agente", "agentes"),
        // O Core devolve estas inteiras: não há segunda página para onde ir.
        previous: None,
        next: None,
        // §41: um agente é definido por capacidade, não por modelo — por isso
        // «Novo Agente» está activo mesmo sem nó. O que falta é onde correr,
        // e é isso que a frase diz, sem contradizer o botão ao lado.
        empty: "Ainda não existem agentes. Um agente é definido por capacidade; só responderá quando existir um nó de IA da Ocinye.",
    };

    list_screen(
        viewer,
        ListScreen {
            title: "Agentes",
            subtitle: "Agentes de IA criados e configurados pelos membros.".to_owned(),
            action: Some("Novo Agente"),
            action_href: Some("/ai/agents/new"),
            action_permission: Permission::AgentsCreatePersonal,
            secondary: None,
            table,
        },
    )
}

// ── Membros ──────────────────────────────────────────────────────────────

/// Administração › Membros.
pub fn members(viewer: &Viewer, payload: &Value) -> impl IntoView {
    let rows = items(payload);
    let shown = rows.len();

    let (anterior, seguinte) = pager(payload, "/admin", &[]);

    let table = Table {
        tabs: vec![
            ListTab::current("Membros"),
            ListTab::missing(
                "Funções",
                "Gerir funções por ecrã próprio ainda não existe.",
            ),
            ListTab::to("Unidades", "/units"),
            ListTab::missing(
                "Convites",
                "A gestão de convites ainda não tem ecrã próprio.",
            ),
            ListTab::missing("Serviços", "A administração de serviços ainda não existe."),
        ],
        search: "membros",
        truncated: truncated(payload, shown),
        shape: "members",
        columns: vec![
            Column::new("NOME"),
            Column::new("E-MAIL"),
            Column::new("UNIDADE"),
            Column::new("FUNÇÃO"),
            Column::new("REGISTO"),
            Column::new("ESTADO"),
            Column::right("ACTIVIDADE"),
        ],
        rows: rows
            .iter()
            .map(|row| {
                let status = text(row, "status");
                // A linha leva ao detalhe do membro. O ecrã existia e nada lhe
                // ligava: um endpoint implementado e inalcançável pela
                // interface (briefing §3).
                let href = row
                    .get("id")
                    .and_then(Value::as_str)
                    .map(|id| format!("/admin/members/{id}"));
                (
                    href,
                    vec![
                        Cell::Primary(text(row, "full_name")),
                        Cell::Mono(text(row, "email")),
                        Cell::Mono(text(row, "unit_code")),
                        // A posição institucional é mostrada para atribuição.
                        // Não concede permissões, e a interface não sugere que
                        // conceda.
                        Cell::Text(text(row, "institutional_position")),
                        Cell::Mono(day(row, "created_at")),
                        Cell::Badge(status.clone(), Tone::of(&status)),
                        Cell::Mono(day(row, "last_seen_at")),
                    ],
                )
            })
            .collect(),
        footer: footer_paginado(payload, shown, "membro", "membros"),
        previous: anterior,
        next: seguinte,
        // O estado vazio só aparece com zero linhas, e o rodapé diz «0 membros»
        // ao lado. «para além de si» afirmaria uma adesão que a contagem nega.
        empty: "Ainda não há membros registados.",
    };

    list_screen(
        viewer,
        ListScreen {
            title: "Membros",
            subtitle: "Administração · membros da instituição.".to_owned(),
            // «Adicionar», não «Convidar»: sob o ADR-0103 não há convite por email —
            // o administrador cria a conta e entrega uma credencial temporária.
            action: Some("Adicionar Utilizador"),
            action_href: Some("/admin/members/new"),
            action_permission: Permission::MembersCreate,
            secondary: None,
            table,
        },
    )
}

// ── Audit Log ────────────────────────────────────────────────────────────

/// Audit Log.
///
/// **Não é um feed de actividade.** Notação técnica de acção, recurso, contexto,
/// resultado e correlation ID (`design/README.md` §6.4).
pub fn audit(viewer: &Viewer, payload: &Value) -> impl IntoView {
    let rows = items(payload);
    let shown = rows.len();

    let (anterior, seguinte) = pager(payload, "/audit", &[]);

    let table = Table {
        tabs: vec![
            ListTab::current("Tudo"),
            ListTab::missing(
                "Autenticação",
                "Recortar o registo por categoria ainda não é uma consulta do Core.",
            ),
            ListTab::missing(
                "Dados",
                "Recortar o registo por categoria ainda não é uma consulta do Core.",
            ),
            ListTab::missing(
                "Permissões",
                "Recortar o registo por categoria ainda não é uma consulta do Core.",
            ),
            ListTab::missing(
                "IA",
                "Recortar o registo por categoria ainda não é uma consulta do Core.",
            ),
        ],
        search: "eventos",
        truncated: truncated(payload, shown),
        shape: "audit",
        columns: vec![
            Column::new("DATA"),
            Column::new("UTILIZADOR"),
            Column::new("ACÇÃO"),
            Column::new("RECURSO"),
            Column::new("CONTEXTO"),
            Column::new("RESULTADO"),
            Column::new("CORRELATION ID"),
        ],
        rows: rows
            .iter()
            .map(|row| {
                // O Core regista `action` e `resource_type` separadamente; a
                // notação técnica do design é a composição dos dois.
                let action = format!("{}.{}", text(row, "resource_type"), text(row, "action"));
                let outcome = match text(row, "outcome").as_str() {
                    "success" => ("OK", Tone::Ok),
                    "denied" => ("NEGADO", Tone::Err),
                    _ => ("AVISO", Tone::Warn),
                };
                let correlation: String = text(row, "correlation_id").chars().take(18).collect();

                (
                    None,
                    vec![
                        Cell::Mono(
                            text(row, "occurred_at")
                                .chars()
                                .take(19)
                                .collect::<String>(),
                        ),
                        Cell::Text(text(row, "actor_name")),
                        Cell::Mono(action),
                        Cell::Mono(text(row, "resource_id")),
                        Cell::Classification(text(row, "classification")),
                        Cell::Badge(outcome.0.to_owned(), outcome.1),
                        Cell::Mono(correlation),
                    ],
                )
            })
            .collect(),
        footer: footer_paginado(payload, shown, "evento", "eventos"),
        previous: anterior,
        next: seguinte,
        empty: "Sem eventos de auditoria para os filtros aplicados.",
    };

    list_screen(
        viewer,
        ListScreen {
            title: "Audit Log",
            subtitle: "Registo técnico e imutável de operações do Ocinye OS.".to_owned(),
            // Sem acção: o Core não expõe exportação do registo de auditoria.
            action: None,
            action_href: None,
            action_permission: Permission::AuditView,
            secondary: None,
            table,
        },
    )
}

// ── Nova ideia ───────────────────────────────────────────────────────────

/// O selector de destino de uma criação institucional.
///
/// # Porque uma página global precisa disto
///
/// Fontes e datasets pertencem a um Research Workspace. Os ecrãs
/// `Bibliografia` e `Dados` são institucionais, e por isso a criação a partir
/// deles tem de perguntar **onde**. Criar sem âmbito não é possível, e escolher
/// um por omissão seria decidir em silêncio onde o trabalho de alguém aterra.
///
/// A lista traz só workspaces onde a criação seria aceite — `may_create`, vindo
/// do Core. Oferecer um destino que o Core recusaria seria um botão para uma
/// recusa, descoberta depois de preencher o formulário inteiro.
///
/// Continua a ser apenas *affordance*. O Core resolve o workspace outra vez na
/// submissão e volta a autorizar; um identificador escrito à mão não passa por
/// aqui ser bonito.
fn workspace_destination(workspaces: &Value) -> impl IntoView {
    let opcoes: Vec<(String, String)> = items(workspaces)
        .iter()
        .filter(|row| {
            row.get("may_create")
                .and_then(Value::as_bool)
                .unwrap_or(false)
        })
        .map(|row| {
            (
                text(row, "id"),
                format!("{} · {}", text(row, "code"), text(row, "title")),
            )
        })
        .collect();

    view! {
        <div class="oc-field">
            <label class="oc-field__label" for="destino">"Research Workspace"</label>
            <select class="oc-select" id="destino" name="workspace_id" required>
                {opcoes
                    .into_iter()
                    .map(|(id, rotulo)| view! { <option value=id>{rotulo}</option> })
                    .collect_view()}
            </select>
            <p class="oc-field__hint">
                "Só aparecem ambientes onde tem autorização para criar."
            </p>
        </div>
    }
}

/// Quantos destinos de criação o membro tem.
fn destinations(workspaces: &Value) -> usize {
    items(workspaces)
        .iter()
        .filter(|row| {
            row.get("may_create")
                .and_then(Value::as_bool)
                .unwrap_or(false)
        })
        .count()
}

/// O estado de quem não tem onde criar.
fn no_destination(o_que: &'static str) -> impl IntoView {
    view! {
        {crate::ui::components::empty_state(crate::ui::components::EmptyState {
            icon: crate::ui::icon::Icon::EmptyState,
            title: format!("Não tem onde criar {o_que}"),
            body: "Estes artefactos pertencem a um Research Workspace, e não pertence a \
                   nenhum onde possa criar. A filiação é concedida por quem gere a unidade."
                .to_owned(),
            actions: vec![Button::new("Ver Unidades", Variant::Secondary).href("/units")],
            small: false,
        })}
    }
}

/// O formulário de criação de uma referência bibliográfica.
pub fn new_source(workspaces: &Value, error: Option<String>) -> impl IntoView {
    use crate::ui::components::{card, section_head, select, text_field, textarea};

    let tem_destino = destinations(workspaces) > 0;

    view! {
        <div class="oc-page oc-page--narrow">
            <div class="oc-head">
                <div class="oc-head__text">
                    <h1>"Nova Referência"</h1>
                    <p>
                        "Uma referência pertence ao Research Workspace onde a investigação
                         que a cita acontece."
                    </p>
                </div>
            </div>

            {error
                .map(|message| {
                    view! { <div class="oc-card oc-alert" role="alert">{message}</div> }
                })}

            {if tem_destino {
                view! {
                    <form method="post" action="/bibliography/new">
                        {card(
                            section_head("A REFERÊNCIA", None, None),
                            view! {
                                {workspace_destination(workspaces)}
                                {text_field(
                                    "src-title",
                                    "Título",
                                    "title",
                                    "Título da obra",
                                    "text",
                                )}
                                {text_field(
                                    "src-authors",
                                    "Autores",
                                    "authors",
                                    "separados por ponto e vírgula",
                                    "text",
                                )}
                                {text_field("src-year", "Ano", "year", "Ex.: 2024", "text")}
                                {text_field(
                                    "src-container",
                                    "Publicação",
                                    "container_title",
                                    "Revista, conferência ou colecção",
                                    "text",
                                )}
                                {text_field("src-doi", "DOI", "doi", "10.xxxx/xxxxx", "text")}
                                {textarea(
                                    "src-abstract",
                                    "Resumo",
                                    "abstract_text",
                                    "Resumo da obra",
                                    92,
                                )}
                                {select(
                                    "src-classification",
                                    "Classificação",
                                    "classification",
                                    vec![
                                        ("INTERNAL".to_owned(), true),
                                        ("CONFIDENTIAL".to_owned(), true),
                                        ("RESTRICTED".to_owned(), true),
                                    ],
                                )}
                            },
                        )}

                        <div class="oc-row--end oc-gap-5 oc-mt-8">
                            {button(
                                Button::new("Cancelar", Variant::Secondary).href("/bibliography"),
                            )}
                            {button(Button::new("Criar Referência", Variant::Gold))}
                        </div>
                    </form>
                }
                    .into_any()
            } else {
                no_destination("referências").into_any()
            }}
        </div>
    }
}

/// O formulário de criação de um dataset.
pub fn new_dataset(workspaces: &Value, error: Option<String>) -> impl IntoView {
    use crate::ui::components::{card, section_head, select, text_field, textarea};

    let tem_destino = destinations(workspaces) > 0;

    view! {
        <div class="oc-page oc-page--narrow">
            <div class="oc-head">
                <div class="oc-head__text">
                    <h1>"Novo Dataset"</h1>
                    <p>
                        "Um dataset pertence ao Research Workspace que o produz ou o usa,
                         e herda dele o contexto institucional."
                    </p>
                </div>
            </div>

            {error
                .map(|message| {
                    view! { <div class="oc-card oc-alert" role="alert">{message}</div> }
                })}

            {if tem_destino {
                view! {
                    <form method="post" action="/datasets/new">
                        {card(
                            section_head("O DATASET", None, None),
                            view! {
                                {workspace_destination(workspaces)}
                                {text_field(
                                    "ds-code",
                                    "Código",
                                    "code",
                                    "Ex.: DS-0001",
                                    "text",
                                )}
                                {text_field("ds-title", "Título", "title", "Nome do conjunto", "text")}
                                {textarea(
                                    "ds-description",
                                    "Descrição",
                                    "description",
                                    "O que o conjunto contém e como foi obtido",
                                    92,
                                )}
                                {text_field(
                                    "ds-keywords",
                                    "Palavras-chave",
                                    "keywords",
                                    "separadas por vírgulas",
                                    "text",
                                )}
                                {text_field(
                                    "ds-restrictions",
                                    "Restrições de uso",
                                    "usage_restrictions",
                                    "Limites de utilização, quando existam",
                                    "text",
                                )}
                                {select(
                                    "ds-classification",
                                    "Classificação",
                                    "classification",
                                    vec![
                                        ("INTERNAL".to_owned(), true),
                                        ("CONFIDENTIAL".to_owned(), true),
                                        ("RESTRICTED".to_owned(), true),
                                    ],
                                )}
                                <p class="oc-muted oc-t-caption--muted">
                                    "A classificação do dataset pode ser mais restrita do que a
                                     do ambiente, e governa quem o alcança."
                                </p>
                            },
                        )}

                        <div class="oc-row--end oc-gap-5 oc-mt-8">
                            {button(Button::new("Cancelar", Variant::Secondary).href("/datasets"))}
                            {button(Button::new("Criar Dataset", Variant::Gold))}
                        </div>
                    </form>
                }
                    .into_any()
            } else {
                no_destination("datasets").into_any()
            }}
        </div>
    }
}

/// O ecrã de promoção de uma ideia a projecto.
///
/// # Porque não é um formulário de criação
///
/// O Ocinye Core não tem `POST /projects`. Um projecto nasce da promoção de uma
/// ideia que chegou a `project_candidate`, e a promoção leva consigo o Research
/// Workspace inteiro — bibliografia, notas, documentos, tudo o que foi reunido
/// enquanto se explorava. Um formulário de raiz criaria um projecto sem
/// proveniência, e perderia isso.
///
/// O selector oferece apenas ideias que a promoção aceitaria hoje, filtradas
/// pelo Core (`?promotable=true`). Oferecer uma que ele recusasse seria um botão
/// para uma recusa — e a recusa só apareceria depois de escolher e submeter.
/// A garantia continua a viver no Core, que valida outra vez.
pub fn new_project(
    candidates: &Value,
    preferido: Option<&str>,
    error: Option<String>,
) -> impl IntoView {
    use crate::ui::components::{card, section_head, text_field, textarea};

    let rows = items(candidates);
    let has_candidates = !rows.is_empty();

    view! {
        <div class="oc-page oc-page--narrow">
            <div class="oc-head">
                <div class="oc-head__text">
                    <h1>"Novo Projecto"</h1>
                    <p>
                        "Um projecto nasce da promoção de uma ideia. O Research Workspace
                         acompanha-a, com tudo o que foi reunido enquanto se explorava."
                    </p>
                </div>
            </div>

            {error
                .map(|message| {
                    view! { <div class="oc-card oc-alert" role="alert">{message}</div> }
                })}

            {if has_candidates {
                let preferido = preferido.unwrap_or_default().to_owned();
                let opcoes: Vec<(String, String, bool)> = rows
                    .iter()
                    .map(|row| {
                        let id = text(row, "id");
                        let escolhida = id == preferido;
                        let rotulo = format!("{} · {}", text(row, "code"), text(row, "title"));
                        (id, rotulo, escolhida)
                    })
                    .collect();

                view! {
                    <form method="post" action="/projects/new">
                        {card(
                            section_head("A IDEIA A PROMOVER", None, None),
                            view! {
                                <div class="oc-field">
                                    <label class="oc-field__label" for="promote-idea">
                                        "Ideia elegível"
                                    </label>
                                    <select
                                        class="oc-select"
                                        id="promote-idea"
                                        name="workspace_id"
                                        required
                                    >
                                        {opcoes
                                            .into_iter()
                                            .map(|(id, rotulo, escolhida)| {
                                                view! {
                                                    <option value=id selected=escolhida>
                                                        {rotulo}
                                                    </option>
                                                }
                                            })
                                            .collect_view()}
                                    </select>
                                    <p class="oc-field__hint">
                                        "Só aparecem ideias em estado de candidatura a projecto,
                                         dentro do que lhe está acessível."
                                    </p>
                                </div>

                                {text_field(
                                    "project-code",
                                    "Código do projecto",
                                    "code",
                                    "Ex.: PPEC-2026-001",
                                    "text",
                                )}
                                {text_field(
                                    "project-title",
                                    "Título",
                                    "title",
                                    "Deixe vazio para manter o título da ideia",
                                    "text",
                                )}
                                {textarea(
                                    "project-objectives",
                                    "Objectivos",
                                    "objectives",
                                    "O que o projecto se propõe alcançar",
                                    92,
                                )}
                            },
                        )}

                        <div class="oc-row--end oc-gap-5 oc-mt-8">
                            {button(Button::new("Cancelar", Variant::Secondary).href("/projects"))}
                            {button(Button::new("Promover a Projecto", Variant::Gold))}
                        </div>
                    </form>
                }
                    .into_any()
            } else {
                // Sem candidatas não há o que promover, e um formulário vazio
                // seria uma promessa que a operação não pode cumprir.
                view! {
                    {crate::ui::components::empty_state(crate::ui::components::EmptyState {
                        icon: crate::ui::icon::Icon::EmptyState,
                        title: "Não existem ideias elegíveis para promoção".to_owned(),
                        body: "Um projecto nasce de uma ideia que chegou a candidatura. \
                               Nenhuma das ideias a que tem acesso está nesse estado."
                            .to_owned(),
                        actions: vec![Button::new("Ver Ideias", Variant::Secondary).href("/ideas")],
                        small: false,
                    })}
                }
                    .into_any()
            }}
        </div>
    }
}

/// O formulário de criação de uma unidade.
///
/// Sem este ecrã o Ocinye OS não se consegue povoar: uma unidade é o âmbito em
/// que uma Ideia nasce, e sem nenhuma o botão «Nova Ideia» não teria onde
/// colocar o que criasse. O Core já aceitava `POST /api/v1/units`; era o
/// Workspace que não lhe chegava, e «Nova Unidade» era um botão sem destino.
pub fn new_unit(error: Option<String>) -> impl IntoView {
    use crate::ui::components::{card, section_head, text_field, textarea};

    view! {
        <div class="oc-page oc-page--narrow">
            <div class="oc-head">
                <div class="oc-head__text">
                    <h1>"Nova Unidade"</h1>
                    <p>
                        "Uma unidade é o âmbito institucional onde a investigação acontece.
                         As ideias, os projectos e as filiações vivem dentro de uma."
                    </p>
                </div>
            </div>

            {error
                .map(|message| {
                    view! { <div class="oc-card oc-alert" role="alert">{message}</div> }
                })}

            <form method="post" action="/units/new">
                {card(
                    section_head("A UNIDADE", None, None),
                    view! {
                        {text_field(
                            "unit-code",
                            "Código",
                            "code",
                            "Ex.: UENR-001",
                            "text",
                        )}
                        {text_field(
                            "unit-name",
                            "Nome",
                            "name",
                            "Ex.: Unidade de Energias Renováveis",
                            "text",
                        )}
                        {textarea(
                            "unit-description",
                            "Descrição",
                            "description",
                            "O que esta unidade investiga",
                            92,
                        )}
                        {text_field(
                            "unit-areas",
                            "Áreas de investigação",
                            "research_areas",
                            "separadas por vírgulas",
                            "text",
                        )}
                    },
                )}

                <div class="oc-row--end oc-gap-5 oc-mt-8">
                    {button(Button::new("Cancelar", Variant::Secondary).href("/units"))}
                    {button(Button::new("Criar Unidade", Variant::Gold))}
                </div>
            </form>
        </div>
    }
}

/// O formulário de criação de uma ideia.
///
/// Só o título e a unidade são obrigatórios. Uma ideia em `Discovery` tem
/// direito a ser magra: exigir uma especificação completa transformaria
/// investigação exploratória em papelada de projecto.
pub fn new_idea(units: &Value, error: Option<String>) -> impl IntoView {
    use crate::ui::components::{card, section_head, select, text_field, textarea};

    let unit_rows = items(units);
    let has_units = !unit_rows.is_empty();

    view! {
        <div class="oc-page oc-page--narrow">
            <div class="oc-head">
                <div class="oc-head__text">
                    <h1>"Nova Ideia"</h1>
                    <p>
                        "Uma ideia é exploratória. Nem todas se tornam projectos, e isso é um
                         desfecho legítimo."
                    </p>
                </div>
            </div>

            {error
                .map(|message| {
                    view! {
                        <div
                            class="oc-card oc-alert"
                            role="alert"
                        >
                            {message}
                        </div>
                    }
                })}

            {if has_units {
                view! {
                    <form method="post" action="/ideas/new">
                        {card(
                            section_head("A IDEIA", None, None),
                            view! {
                                // O selector usa o mesmo rótulo composto que a
                                // lista de unidades; o valor submetido é o id.
                                {units_select(&unit_rows)}
                                {text_field("idea-title", "Título", "title", "O que se quer investigar", "text")}
                                {textarea(
                                    "idea-question",
                                    "Pergunta de investigação",
                                    "research_question",
                                    "Que pergunta é que isto responde",
                                    64,
                                )}
                                {textarea(
                                    "idea-hypothesis",
                                    "Hipótese",
                                    "hypothesis",
                                    "A hipótese, quando já existe uma",
                                    64,
                                )}
                                {textarea(
                                    "idea-motivation",
                                    "Motivação",
                                    "motivation",
                                    "Porque é que isto importa à instituição",
                                    64,
                                )}
                                {textarea("idea-summary", "Resumo", "summary", "Resumo da ideia", 92)}
                                {text_field(
                                    "idea-keywords",
                                    "Palavras-chave",
                                    "keywords",
                                    "separadas por vírgulas",
                                    "text",
                                )}
                                {select(
                                    "idea-classification",
                                    "Classificação",
                                    "classification",
                                    vec![
                                        ("INTERNAL".to_owned(), true),
                                        ("CONFIDENTIAL".to_owned(), true),
                                        ("RESTRICTED".to_owned(), true),
                                    ],
                                )}
                                <p class="oc-muted oc-t-caption--muted" >
                                    "A classificação governa tudo o que for acrescentado a este
                                     Research Workspace."
                                </p>
                            },
                        )}

                        <div class="oc-row--end oc-gap-5 oc-mt-8" >
                            {button(Button::new("Cancelar", Variant::Secondary).href("/ideas"))}
                            {button(Button::new("Criar Ideia", Variant::Gold))}
                        </div>
                    </form>
                }
                    .into_any()
            } else {
                // Sem unidades não há onde colocar uma ideia. Dizê-lo é melhor
                // do que apresentar um formulário que falharia na submissão.
                view! {
                    <section class="oc-card">
                        <div class="oc-empty">
                            <h3>"Ainda não existem unidades"</h3>
                            <p>
                                "Uma ideia pertence sempre a uma unidade científica. Peça a um
                                 administrador que crie a primeira."
                            </p>
                            <div class="oc-empty__actions">
                                {button(Button::new("Ver unidades", Variant::Secondary).href("/units"))}
                            </div>
                        </div>
                    </section>
                }
                    .into_any()
            }}
        </div>
    }
}

/// O selector de unidade, com o identificador como valor submetido.
fn units_select(units: &[Value]) -> impl IntoView {
    let options: Vec<(String, String)> = units
        .iter()
        .map(|unit| {
            (
                text(unit, "id"),
                format!("{} — {}", text(unit, "code"), text(unit, "name")),
            )
        })
        .collect();

    view! {
        <div class="oc-field">
            <label class="oc-field__label" for="idea-unit">"Unidade"</label>
            <select class="oc-select" id="idea-unit" name="unit_id" required>
                {options
                    .into_iter()
                    .map(|(id, label)| view! { <option value=id>{label}</option> })
                    .collect_view()}
            </select>
        </div>
    }
}

/// Ferramentas bibliográficas: validar e normalizar BibTeX.
///
/// # Porque não se fala aqui de WebAssembly
///
/// Porque quem usa isto está a preparar uma bibliografia, e não a executar
/// código. O isolamento é uma decisão de engenharia do Ocinye OS, e a página
/// diz apenas o que importa a quem lê: que a leitura acontece aqui dentro e não
/// consulta serviço nenhum.
///
/// # Porque o resultado é texto e não marcação
///
/// O que entra é conteúdo não confiável — alguém colou-o de um sítio qualquer.
/// Aparece numa área de texto, escapado pelo Leptos, e nunca interpretado.
pub fn bibliography_tools(
    workspaces: &Value,
    bibtex: &str,
    revisao: Option<&BibliographyReview>,
    error: Option<String>,
) -> impl IntoView {
    use crate::ui::components::{card, empty_state, section_head, EmptyState};

    let tem_destino = destinations(workspaces) > 0;
    let escrito = bibtex.to_owned();

    view! {
        <div class="oc-page oc-page--narrow">
            <div class="oc-head">
                <div class="oc-head__text">
                    <h1>"Ferramentas bibliográficas"</h1>
                    <p>
                        "Valida a estrutura de referências BibTeX e produz uma versão
                         normalizada. A leitura acontece no Ocinye OS, sem consultar
                         serviços externos: nenhum DOI é verificado e nenhuma referência
                         é confirmada."
                    </p>
                </div>
            </div>

            {error
                .map(|message| {
                    view! { <div class="oc-card oc-alert" role="alert">{message}</div> }
                })}

            {if tem_destino {
                view! {
                    <form class="oc-form" method="post" action="/bibliography/tools">
                        {card(
                            section_head("BIBLIOGRAFIA", None, None),
                            view! {
                                {workspace_destination(workspaces)}
                                <div class="oc-field">
                                    <label class="oc-field__label" for="bibtex">"BibTeX"</label>
                                    <textarea
                                        class="oc-textarea"
                                        id="bibtex"
                                        name="bibtex"
                                        rows="12"
                                        placeholder="@article{chave, title = {…}, author = {…}, year = {…}}"
                                    >{escrito}</textarea>
                                </div>
                                <div class="oc-actions">
                                    <button type="submit" class="oc-btn oc-btn--navy">
                                        "Validar e normalizar"
                                    </button>
                                </div>
                            },
                        )}
                    </form>
                }
                    .into_any()
            } else {
                view! {
                    <div class="oc-card">
                        {empty_state(EmptyState {
                            title: "Sem Research Workspace onde trabalhar".to_owned(),
                            body: "Rever bibliografia acontece dentro de um ambiente de \
                                   investigação onde possa acrescentar referências."
                                .to_owned(),
                            actions: Vec::new(),
                            small: false,
                            icon: crate::ui::icon::Icon::EmptyState,
                        })}
                    </div>
                }
                    .into_any()
            }}

            {revisao.map(resultado_da_revisao)}
        </div>
    }
}

/// O que a revisão devolveu, como quem lê o vê.
fn resultado_da_revisao(revisao: &BibliographyReview) -> impl IntoView {
    use crate::ui::components::{badge, card, section_head};

    let lidas = revisao.read_count();
    let por_ler = revisao.unreadable.len();
    let completa = revisao.is_complete();

    let resumo = if completa {
        format!("{lidas} referência(s) lidas, todas legíveis.")
    } else {
        format!("{lidas} referência(s) lidas · {por_ler} por ler.")
    };

    let ilegiveis: Vec<String> = revisao.unreadable.clone();
    let normalizado = revisao.normalized.clone();
    let entradas: Vec<(String, String, String)> = revisao
        .entries
        .iter()
        .map(|entrada| {
            (
                entrada.citation_key.clone(),
                entrada.entry_type.clone(),
                entrada.title.clone().unwrap_or_else(|| "—".to_owned()),
            )
        })
        .collect();

    view! {
        <div class="oc-mt-6" data-oc="revisao">
            {card(
                section_head("RESULTADO", None, None),
                view! {
                    <p class="oc-t-body">
                        {badge(
                            if completa { "Legível" } else { "Com problemas" },
                            if completa { Tone::Ok } else { Tone::Gold },
                        )}
                        " "
                        {resumo}
                    </p>

                    {(!ilegiveis.is_empty())
                        .then(|| {
                            view! {
                                <div class="oc-mt-4">
                                    <p class="oc-t-strong">"Não foi possível ler:"</p>
                                    <ul class="oc-list">
                                        {ilegiveis
                                            .into_iter()
                                            .map(|excerto| view! { <li>{excerto}</li> })
                                            .collect::<Vec<_>>()}
                                    </ul>
                                </div>
                            }
                        })}

                    {(!entradas.is_empty())
                        .then(|| {
                            view! {
                                <div class="oc-mt-4">
                                    <p class="oc-t-strong">"Referências lidas:"</p>
                                    <ul class="oc-list">
                                        {entradas
                                            .into_iter()
                                            .map(|(chave, tipo, titulo)| {
                                                view! {
                                                    <li>
                                                        <code>{chave}</code>
                                                        " · " {tipo} " · " {titulo}
                                                    </li>
                                                }
                                            })
                                            .collect::<Vec<_>>()}
                                    </ul>
                                </div>
                            }
                        })}

                    <div class="oc-mt-5">
                        <label class="oc-field__label" for="normalizado">
                            "BibTeX normalizado"
                        </label>
                        <textarea
                            class="oc-textarea"
                            id="normalizado"
                            rows="12"
                            readonly
                            data-oc="normalizado"
                        >{normalizado}</textarea>
                    </div>
                },
            )}
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Um membro que pode tudo, para os testes que verificam a tabela e não a
    /// filtragem por permissão.
    fn viewer() -> Viewer {
        Viewer {
            zona: "UTC".to_owned().try_into().expect("fuso conhecido"),
            avatar: ocinye_contracts::AvatarChoice::Initials,
            email: Some("jmanuel@ocinye.com".to_owned()),
            session_expires_in: Some(std::time::Duration::from_secs(8 * 3600)),
            name: "Teste".to_owned(),
            organisation: "Ocinye".to_owned(),
            core_status: crate::ui::shell::CoreStatus::Ok,
            temporal: Vec::new(),
            temporal_failure: None,
            unread: 0,
            modules: Vec::new(),
            capabilities: Permission::all()
                .into_iter()
                .map(|p| p.as_str().to_owned())
                .collect(),
        }
    }

    /// Um membro sem permissão nenhuma.
    fn viewer_sem_permissoes() -> Viewer {
        Viewer {
            zona: "UTC".to_owned().try_into().expect("fuso conhecido"),
            avatar: ocinye_contracts::AvatarChoice::Initials,
            email: Some("jmanuel@ocinye.com".to_owned()),
            session_expires_in: Some(std::time::Duration::from_secs(8 * 3600)),
            name: "Teste".to_owned(),
            organisation: "Ocinye".to_owned(),
            core_status: crate::ui::shell::CoreStatus::Ok,
            temporal: Vec::new(),
            temporal_failure: None,
            unread: 0,
            modules: Vec::new(),
            capabilities: Vec::new(),
        }
    }

    use serde_json::json;

    #[test]
    fn sem_permissao_a_accao_primaria_e_visivel_mas_nao_e_accionavel() {
        // A política mudou por decisão da instituição: as acções deixam de
        // desaparecer a quem não as pode usar. Uma interface que muda de forma
        // consoante quem olha esconde a própria existência da acção, e quem não
        // a vê não fica a saber que existe nem porque não a tem.
        //
        // O que **não** pode mudar é a segunda metade: visível não é
        // accionável. O botão não tem destino, está marcado como desactivado, e
        // diz porquê. É a diferença entre declarar uma recusa e oferecer uma.
        let sem = viewer_sem_permissoes();
        let payload = json!({"items": [], "total": 0});

        for (ecra, html) in [
            ("agentes", agents(&sem, &payload).to_html()),
            ("membros", members(&sem, &payload).to_html()),
            ("ideias", ideas(&sem, &payload, Slice::default()).to_html()),
        ] {
            assert!(
                html.contains("oc-btn--primary"),
                "{ecra}: a acção primária desapareceu em vez de se declarar"
            );
            assert!(
                html.contains("oc-unavailable") && html.contains("aria-disabled=\"true\""),
                "{ecra}: a acção aparece sem estar marcada como indisponível"
            );
            assert!(
                html.contains(SEM_AUTORIZACAO),
                "{ecra}: a acção não diz porque está indisponível"
            );
            assert!(
                !html.contains(r#"class="oc-btn oc-btn--primary" href="#),
                "{ecra}: a acção continua a levar a algum lado sem a permissão"
            );
        }
    }

    #[test]
    fn com_a_permissao_a_accao_aparece_e_leva_ao_ecra_certo() {
        let payload = json!({"items": [], "total": 0});
        let html = agents(&viewer(), &payload).to_html();
        assert!(html.contains("Novo Agente"));
        assert!(html.contains(r#"href="/ai/agents/new""#));

        let html = members(&viewer(), &payload).to_html();
        assert!(html.contains(r#"href="/admin/members/new""#));
    }

    #[test]
    fn uma_lista_vazia_explica_porque_esta_vazia() {
        let html = agents(&viewer(), &json!({"items": [], "total": 0})).to_html();
        // A explicação tem de nomear o que falta — o nó — e não pode contradizer
        // o botão «Novo Agente», que fica activo porque um agente se define por
        // capacidade (§41).
        assert!(html.contains("quando existir um nó de IA da Ocinye"));
        assert!(!html.contains("precisa de um modelo"));
    }

    #[test]
    fn a_contagem_concorda_em_singular_e_plural() {
        assert_eq!(
            footer(&json!({"total": 1}), 1, "ideia", "ideias"),
            "1–1 de 1 ideia"
        );
        assert_eq!(
            footer(&json!({"total": 86}), 8, "ideia", "ideias"),
            "1–8 de 86 ideias"
        );
        assert_eq!(
            footer(&json!({"total": 0}), 0, "ideia", "ideias"),
            "0 ideias"
        );
    }

    #[test]
    fn o_audit_usa_notacao_tecnica_e_nao_prosa() {
        let payload = json!({
            "items": [{
                "occurred_at": "2026-08-22T03:14:00Z",
                "actor_name": "João Manuel",
                "action": "read",
                "resource_type": "dataset",
                "outcome": "denied",
                "classification": "RESTRICTED",
                "correlation_id": "9c1f4b2a-77de-4c11"
            }],
            "total": 1
        });
        let html = audit(&viewer(), &payload).to_html();
        assert!(html.contains("dataset.read"));
        assert!(html.contains("NEGADO"));
        assert!(html.contains("RESTRICTED"));
    }

    #[test]
    fn as_dimensoes_sao_legiveis() {
        assert_eq!(size(&json!({"n": 512}), "n"), "512 B");
        assert_eq!(size(&json!({"n": 2048}), "n"), "2.0 kB");
        assert_eq!(size(&json!({"n": 5_368_709_120_i64}), "n"), "5.0 GB");
        assert_eq!(size(&json!({}), "n"), "—");
    }
}
