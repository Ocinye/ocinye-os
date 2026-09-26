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
/// A razão dada quando a acção existe e é a pessoa que não lhe chega. É uma
/// função, e não uma `const`: uma `const` não pode chamar `crate::i18n::t`, que
/// resolve o idioma no momento da renderização.
fn sem_autorizacao() -> &'static str {
    crate::i18n::t("lists.action.unauthorised")
}

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
                                .unavailable_because(sem_autorizacao())
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
                                .unavailable_because(sem_autorizacao())
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

/// Quem agiu, com quem responde quando são identidades diferentes.
///
/// # Porque não chega o nome de quem executou
///
/// Porque uma operação administrativa é executada por uma identidade
/// privilegiada, e uma linha que dissesse apenas «Fidel Admin» perde a pessoa
/// que responde por ela. Uma auditoria que não sabe dizer quem responde não é
/// uma auditoria.
///
/// E não é o contrário: mostrar só «Fidel Monteiro» apagaria o facto de aquilo
/// ter sido feito com autoridade de plataforma, que é precisamente o que uma
/// revisão precisa de ver.
///
/// Para uma pessoa comum não há segunda camada, e a linha fica como sempre
/// esteve — acrescentar «(em nome de si próprio)» a toda a gente seria ruído a
/// esconder os casos que importam.
fn actor_da_auditoria(row: &Value) -> String {
    let quem_executou = text(row, "actor_name");
    match row.get("actor_on_behalf_of").and_then(Value::as_str) {
        Some(dono) if !dono.is_empty() => format!("{quem_executou} · por {dono}"),
        _ => quem_executou,
    }
}

/// Lê um inteiro como texto.
fn number(row: &Value, key: &str) -> String {
    row.get(key)
        .and_then(Value::as_i64)
        .map_or_else(|| "—".to_owned(), |n| n.to_string())
}

/// Lê uma lista de strings (por exemplo, as áreas de investigação de uma unidade).
fn string_list(row: &Value, key: &str) -> Vec<String> {
    row.get(key)
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(Value::as_str)
                .filter(|s| !s.is_empty())
                .map(ToOwned::to_owned)
                .collect()
        })
        .unwrap_or_default()
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
                crate::i18n::t("lists.slice.unit"),
                crate::i18n::t("lists.slice.unit.none"),
            ),
            _ if self.unit_id.is_some() || self.awaiting_unit => {
                ListTab::current(crate::i18n::t("lists.slice.unit"))
            }
            _ => ListTab::to(
                crate::i18n::t("lists.slice.unit"),
                format!("{base}?unit=true"),
            ),
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
            <label class="oc-field__label" for="unit_id">{crate::i18n::t("lists.unit_pick.label")}</label>
            <select class="oc-select" id="unit_id" name="unit_id">
                <option value="" disabled=true selected=escolhida.is_none()>
                    {crate::i18n::t("lists.unit_pick.placeholder")}
                </option>
                {opcoes}
            </select>
            {button(Button::new(crate::i18n::t("lists.unit_pick.apply"), Variant::Secondary))}
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
            ListTab::to(crate::i18n::t("lists.tab.all_f"), "/ideas")
        } else {
            ListTab::current(crate::i18n::t("lists.tab.all_f"))
        },
        if slice.mine {
            ListTab::current(crate::i18n::t("lists.tab.mine_f"))
        } else {
            ListTab::to(crate::i18n::t("lists.tab.mine_f"), "/ideas?mine=true")
        },
        slice.unit_tab("/ideas"),
        ListTab::missing(
            crate::i18n::t("lists.tab.followed_f"),
            crate::i18n::t("lists.slice.followed_ideas"),
        ),
        ListTab::missing(
            crate::i18n::t("lists.tab.archived_f"),
            crate::i18n::t("lists.slice.state_not_query"),
        ),
    ]
}

/// Os recortes de Projectos. Mesma história das Ideias.
fn projects_tabs(slice: &Slice) -> Vec<ListTab> {
    let noutro = slice.mine || slice.unit_id.is_some() || slice.awaiting_unit;
    vec![
        if noutro {
            ListTab::to(crate::i18n::t("lists.tab.all_m"), "/projects")
        } else {
            ListTab::current(crate::i18n::t("lists.tab.all_m"))
        },
        if slice.mine {
            ListTab::current(crate::i18n::t("lists.tab.mine_m"))
        } else {
            ListTab::to(crate::i18n::t("lists.tab.mine_m"), "/projects?mine=true")
        },
        slice.unit_tab("/projects"),
        ListTab::missing(
            crate::i18n::t("lists.tab.completed_m"),
            crate::i18n::t("lists.slice.state_not_query"),
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
/// A célula «Unidade» da lista de membros, a partir do array `units` que o Core
/// devolve (`[{code, name}, …]`, vazio quando o membro não pertence a nenhuma).
///
/// Não há unidade principal (CLAUDE.md §34.3): com uma, mostra-se o nome; com
/// duas ou mais, a contagem — eleger uma seria inventar uma hierarquia que o
/// domínio recusa. Sem nenhuma, a célula fica «—» (`Cell::Empty`).
fn unit_cell(row: &Value) -> Cell {
    let unidades = row.get("units").and_then(Value::as_array);
    match unidades.map(Vec::as_slice) {
        Some([]) | None => Cell::Empty,
        Some([uma]) => {
            let nome = uma
                .get("name")
                .and_then(Value::as_str)
                .or_else(|| uma.get("code").and_then(Value::as_str))
                .unwrap_or("");
            if nome.is_empty() {
                Cell::Empty
            } else {
                Cell::Text(nome.to_owned())
            }
        }
        Some(varias) => Cell::Text(format!("{} unidades", varias.len())),
    }
}

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
            ListTab::current(crate::i18n::t("lists.tab.all_f")),
            ListTab::missing(
                crate::i18n::t("lists.tab.mine_f"),
                crate::i18n::t("lists.units.mine_none"),
            ),
            ListTab::missing(
                crate::i18n::t("lists.tab.followed_f"),
                crate::i18n::t("lists.units.followed_none"),
            ),
            ListTab::missing(
                crate::i18n::t("lists.tab.archived_f"),
                crate::i18n::t("lists.units.archived_none"),
            ),
        ],
        search: crate::i18n::t("lists.noun.units"),
        truncated: truncated(payload, shown),
        shape: "units",
        columns: vec![
            Column::new(crate::i18n::t("lists.col.unit")),
            Column::new(crate::i18n::t("lists.col.code")),
            Column::new(crate::i18n::t("lists.col.lead")),
            Column::right(crate::i18n::t("lists.col.members")),
            Column::right(crate::i18n::t("lists.col.ideas")),
            Column::right(crate::i18n::t("lists.col.projects")),
            Column::new(crate::i18n::t("lists.col.state")),
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
        empty: crate::i18n::t("lists.units.empty"),
    };

    list_screen(
        viewer,
        ListScreen {
            title: crate::i18n::t("nav.units"),
            subtitle: crate::i18n::t("lists.units.subtitle").to_owned(),
            action: Some(crate::i18n::t("lists.new.unit")),
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
                        <h1>{crate::i18n::t("nav.ideas")}</h1>
                        <p>{crate::i18n::t("lists.choose_unit_prompt")}</p>
                    </div>
                </div>
                {unit_selector(&slice, "/ideas")}
                {crate::ui::components::empty_state(crate::ui::components::EmptyState {
                    icon: crate::ui::icon::Icon::Units,
                    title: crate::i18n::t("lists.no_unit_chosen.title").to_owned(),
                    body: crate::i18n::t("lists.no_unit_chosen.body").to_owned(),
                    actions: Vec::new(),
                    small: false,
                })}
            </div>
        }
        .into_any();
    }

    let table = Table {
        tabs: ideas_tabs(&slice),
        search: crate::i18n::t("lists.noun.ideas"),
        truncated: truncated(payload, shown),
        shape: "ideas",
        columns: vec![
            Column::new(crate::i18n::t("lists.col.title")),
            Column::new(crate::i18n::t("lists.col.unit")),
            Column::new(crate::i18n::t("lists.col.lead")),
            Column::new(crate::i18n::t("lists.col.state")),
            Column::new(crate::i18n::t("lists.col.priority")),
            Column::new(crate::i18n::t("lists.col.classification")),
            Column::right(crate::i18n::t("lists.col.updated")),
        ],
        rows: rows
            .iter()
            .map(|row| {
                // A lista de Ideias é servida por `/workspaces?kind=idea`: cada
                // linha é um Research Workspace, e `id` é o id do **ambiente**,
                // não o da ideia. Ligar a `/ideas/{id}` daria esse id de ambiente
                // a uma rota que espera um id de ideia — e caía em «Página não
                // encontrada». O ambiente abre-se directamente, como na Home.
                let id = text(row, "id");
                let state = text(row, "state");
                let priority = text(row, "priority");
                (
                    Some(format!("/workspaces/{id}")),
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
        empty: crate::i18n::t("lists.ideas.empty"),
    };

    list_screen(
        viewer,
        ListScreen {
            title: crate::i18n::t("nav.ideas"),
            // Quem filtra por uma unidade deve ver qual: o subtítulo genérico
            // descreve a lista inteira, e a lista deixou de ser inteira.
            subtitle: slice.unit_name().map_or_else(
                || crate::i18n::t("lists.ideas.subtitle").to_owned(),
                |unidade| crate::i18n::tf("lists.subtitle.unit", &[("unit", unidade)]),
            ),
            action: Some(crate::i18n::t("create.idea")),
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
                        <h1>{crate::i18n::t("nav.projects")}</h1>
                        <p>{crate::i18n::t("lists.choose_unit_prompt")}</p>
                    </div>
                </div>
                {unit_selector(&slice, "/projects")}
                {crate::ui::components::empty_state(crate::ui::components::EmptyState {
                    icon: crate::ui::icon::Icon::Units,
                    title: crate::i18n::t("lists.no_unit_chosen.title").to_owned(),
                    body: crate::i18n::t("lists.no_unit_chosen.body").to_owned(),
                    actions: Vec::new(),
                    small: false,
                })}
            </div>
        }
        .into_any();
    }

    let table = Table {
        tabs: projects_tabs(&slice),
        search: crate::i18n::t("lists.noun.projects"),
        truncated: truncated(payload, shown),
        shape: "projects",
        columns: vec![
            Column::new(crate::i18n::t("lists.col.code")),
            Column::new(crate::i18n::t("lists.col.project")),
            Column::new(crate::i18n::t("lists.col.unit")),
            Column::new(crate::i18n::t("lists.col.lead")),
            Column::new(crate::i18n::t("lists.col.state")),
            Column::new(crate::i18n::t("lists.col.progress")),
            Column::new(crate::i18n::t("lists.col.start")),
            Column::new(crate::i18n::t("lists.col.end")),
        ],
        rows: rows
            .iter()
            .map(|row| {
                // Como a lista de Ideias: `/workspaces?kind=project` devolve
                // ambientes, e `id` é o id do ambiente, não o do projecto. Ligar
                // a `/projects/{id}` daria um id de ambiente a uma rota que espera
                // um id de projecto — o mesmo 404 da lista de Ideias.
                let id = text(row, "id");
                let state = text(row, "state");
                let progress = row
                    .get("progress")
                    .and_then(Value::as_i64)
                    .and_then(|p| u8::try_from(p).ok());
                (
                    Some(format!("/workspaces/{id}")),
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
        empty: crate::i18n::t("lists.projects.empty"),
    };

    list_screen(
        viewer,
        ListScreen {
            title: crate::i18n::t("nav.projects"),
            // Quem filtra por uma unidade deve ver qual: o subtítulo genérico
            // descreve a lista inteira, e a lista deixou de ser inteira.
            subtitle: slice.unit_name().map_or_else(
                || crate::i18n::t("lists.projects.subtitle").to_owned(),
                |unidade| crate::i18n::tf("lists.subtitle.unit", &[("unit", unidade)]),
            ),
            action: Some(crate::i18n::t("create.project")),
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
            ListTab::current(crate::i18n::t("lists.tab.all_f")),
            ListTab::missing(
                crate::i18n::t("lists.tab.mine_f"),
                crate::i18n::t("lists.biblio.mine_none"),
            ),
            ListTab::missing(
                crate::i18n::t("lists.slice.unit"),
                crate::i18n::t("lists.biblio.unit_none"),
            ),
            ListTab::missing(
                crate::i18n::t("lists.tab.favourites_f"),
                crate::i18n::t("lists.favourites_none"),
            ),
        ],
        search: crate::i18n::t("lists.noun.sources"),
        truncated: truncated(payload, shown),
        shape: "bibliography",
        columns: vec![
            Column::new(crate::i18n::t("lists.col.title")),
            Column::new(crate::i18n::t("lists.col.authors")),
            Column::new(crate::i18n::t("lists.col.year")),
            Column::new(crate::i18n::t("lists.col.origin")),
            Column::new(crate::i18n::t("lists.col.type")),
            Column::new("DOI"),
            Column::right(crate::i18n::t("lists.col.citations")),
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
        empty: crate::i18n::t("lists.biblio.empty"),
    };

    list_screen(
        viewer,
        ListScreen {
            title: crate::i18n::t("nav.bibliography"),
            subtitle: crate::i18n::t("lists.biblio.subtitle").to_owned(),
            action: Some(crate::i18n::t("create.reference")),
            action_href: Some("/bibliography/new"),
            action_permission: Permission::BibliographyCreate,
            secondary: Some((crate::i18n::t("lists.tools.link"), "/bibliography/tools")),
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
            ListTab::current(crate::i18n::t("lists.tab.all_m")),
            ListTab::missing(
                crate::i18n::t("lists.tab.mine_m"),
                crate::i18n::t("lists.datasets.mine_none"),
            ),
            ListTab::missing(
                crate::i18n::t("lists.slice.unit"),
                crate::i18n::t("lists.datasets.unit_none"),
            ),
            ListTab::missing(
                crate::i18n::t("lists.tab.favourites_m"),
                crate::i18n::t("lists.favourites_none"),
            ),
        ],
        search: crate::i18n::t("lists.noun.datasets"),
        truncated: truncated(payload, shown),
        shape: "datasets",
        columns: vec![
            Column::new(crate::i18n::t("lists.col.name")),
            Column::new(crate::i18n::t("lists.col.lead")),
            Column::new(crate::i18n::t("lists.col.registered")),
            Column::new(crate::i18n::t("lists.col.version")),
            Column::right(crate::i18n::t("lists.col.size")),
            Column::new(crate::i18n::t("lists.col.type")),
            Column::new(crate::i18n::t("lists.col.classification")),
            Column::new(crate::i18n::t("lists.col.access")),
        ],
        rows: rows
            .iter()
            .map(|row| {
                let state = text(row, "state");
                let id = text(row, "id");
                (
                    Some(format!("/datasets/{id}")),
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
        empty: crate::i18n::t("lists.datasets.empty"),
    };

    list_screen(
        viewer,
        ListScreen {
            title: crate::i18n::t("nav.data"),
            subtitle: crate::i18n::t("lists.datasets.subtitle").to_owned(),
            action: Some(crate::i18n::t("create.dataset")),
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
            ListTab::current(crate::i18n::t("lists.tab.all_m")),
            ListTab::missing(
                crate::i18n::t("lists.tab.mine_m"),
                crate::i18n::t("lists.agents.mine_none"),
            ),
            ListTab::missing(
                crate::i18n::t("lists.slice.unit"),
                crate::i18n::t("lists.agents.unit_none"),
            ),
            ListTab::missing(
                crate::i18n::t("lists.agents.tab_institutional"),
                crate::i18n::t("lists.agents.institutional_none"),
            ),
        ],
        search: crate::i18n::t("lists.noun.agents"),
        truncated: truncated(payload, shown),
        // «UTILIZAÇÃO» foi retirada: não existe contagem de utilizações no
        // Core, e uma coluna sem fonte é uma estatística inventada (§60).
        shape: "agents",
        columns: vec![
            Column::new(crate::i18n::t("lists.col.agent")),
            Column::new(crate::i18n::t("lists.col.purpose")),
            Column::new(crate::i18n::t("lists.col.state")),
            Column::new(crate::i18n::t("lists.col.scope")),
            Column::new(crate::i18n::t("lists.col.capability")),
            Column::right(crate::i18n::t("lists.col.created")),
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
                let id = text(row, "id");
                (
                    Some(format!("/ai/agents/{id}")),
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
        empty: crate::i18n::t("lists.agents.empty"),
    };

    list_screen(
        viewer,
        ListScreen {
            title: crate::i18n::t("nav.agents"),
            subtitle: crate::i18n::t("lists.agents.subtitle").to_owned(),
            action: Some(crate::i18n::t("lists.new.agent")),
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
            ListTab::current(crate::i18n::t("lists.members.tab")),
            ListTab::missing(
                crate::i18n::t("lists.members.tab_roles"),
                crate::i18n::t("lists.members.roles_none"),
            ),
            ListTab::to(crate::i18n::t("nav.units"), "/units"),
            // «Acessos», e não «Convites»: no Ocinye não há convite por email
            // (ADR-0103). O acesso provisiona-se com uma credencial temporária,
            // e isso gere-se hoje no separador «Segurança» de cada membro — dar
            // acesso, reemitir uma credencial expirada. Uma vista de todos os
            // acessos num só ecrã ainda não existe.
            ListTab::missing(
                crate::i18n::t("lists.members.tab_access"),
                crate::i18n::t("lists.members.access_none"),
            ),
            ListTab::missing(
                crate::i18n::t("lists.members.tab_services"),
                crate::i18n::t("lists.members.services_none"),
            ),
        ],
        search: crate::i18n::t("lists.noun.members"),
        truncated: truncated(payload, shown),
        shape: "members",
        columns: vec![
            Column::new(crate::i18n::t("lists.col.name")),
            Column::new(crate::i18n::t("lists.col.email")),
            Column::new(crate::i18n::t("lists.col.unit")),
            // «Posição», e não «Função»: a coluna mostra a posição institucional
            // (Fundador, Director), que não concede acesso. Chamá-la «Função»
            // sugeria o papel técnico, que é outra dimensão (ADR-0100).
            Column::new(crate::i18n::t("lists.col.position")),
            Column::new(crate::i18n::t("lists.col.registered")),
            Column::new(crate::i18n::t("lists.col.state")),
            Column::right(crate::i18n::t("lists.col.activity")),
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
                        // A pertença factual do membro. Não há unidade principal
                        // (CLAUDE.md §34.3): com uma, mostra-se o nome; com
                        // várias, a contagem — nunca se elege uma. Sem nenhuma,
                        // a célula fica «—».
                        unit_cell(row),
                        // A posição institucional, em português e pela mesma
                        // tradução do resto da Administração. Não concede
                        // permissões, e a interface não sugere que conceda.
                        Cell::Text(super::administration::position_label(
                            row.get("institutional_position")
                                .and_then(Value::as_str)
                                .unwrap_or(""),
                        )),
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
        empty: crate::i18n::t("lists.members.empty"),
    };

    list_screen(
        viewer,
        ListScreen {
            title: crate::i18n::t("lists.members.tab"),
            subtitle: crate::i18n::t("lists.members.subtitle").to_owned(),
            // «Adicionar», não «Convidar»: sob o ADR-0103 não há convite por email —
            // o administrador cria a conta e entrega uma credencial temporária.
            action: Some(crate::i18n::t("lists.new.member")),
            action_href: Some("/admin/members/new"),
            action_permission: Permission::MembersCreate,
            // A configuração da Instância (ADR-0014), para quem a governa.
            secondary: viewer
                .can(Permission::OrganisationManage)
                .then(|| (crate::i18n::t("admin.instance.link"), "/admin/instance")),
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
            ListTab::current(crate::i18n::t("knowledge.tab.all")),
            ListTab::missing(
                crate::i18n::t("lists.audit.tab_auth"),
                crate::i18n::t("lists.audit.category_none"),
            ),
            ListTab::missing(
                crate::i18n::t("nav.data"),
                crate::i18n::t("lists.audit.category_none"),
            ),
            ListTab::missing(
                crate::i18n::t("lists.audit.tab_permissions"),
                crate::i18n::t("lists.audit.category_none"),
            ),
            ListTab::missing(
                crate::i18n::t("lists.audit.tab_ai"),
                crate::i18n::t("lists.audit.category_none"),
            ),
        ],
        search: crate::i18n::t("lists.noun.events"),
        truncated: truncated(payload, shown),
        shape: "audit",
        columns: vec![
            Column::new(crate::i18n::t("lists.col.date")),
            Column::new(crate::i18n::t("lists.col.user")),
            Column::new(crate::i18n::t("lists.col.action")),
            Column::new(crate::i18n::t("lists.col.resource")),
            Column::new(crate::i18n::t("lists.col.context")),
            Column::new(crate::i18n::t("lists.col.outcome")),
            Column::new(crate::i18n::t("lists.col.correlation_id")),
        ],
        rows: rows
            .iter()
            .map(|row| {
                // O Core regista `action` e `resource_type` separadamente; a
                // notação técnica do design é a composição dos dois.
                let action = format!("{}.{}", text(row, "resource_type"), text(row, "action"));
                let outcome = match text(row, "outcome").as_str() {
                    "success" => ("OK", Tone::Ok),
                    "denied" => (crate::i18n::t("lists.audit.denied"), Tone::Err),
                    _ => (crate::i18n::t("lists.audit.warn"), Tone::Warn),
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
                        Cell::Text(actor_da_auditoria(row)),
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
        empty: crate::i18n::t("lists.audit.empty"),
    };

    list_screen(
        viewer,
        ListScreen {
            title: crate::i18n::t("nav.audit"),
            subtitle: crate::i18n::t("lists.audit.subtitle").to_owned(),
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
            <label class="oc-field__label" for="destino">{crate::i18n::t("lists.research_workspace")}</label>
            <select class="oc-select" id="destino" name="workspace_id" required>
                {opcoes
                    .into_iter()
                    .map(|(id, rotulo)| view! { <option value=id>{rotulo}</option> })
                    .collect_view()}
            </select>
            <p class="oc-field__hint">
                {crate::i18n::t("lists.destination.hint")}
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
            title: crate::i18n::tf("lists.no_destination.title", &[("what", o_que)]),
            body: crate::i18n::t("lists.no_destination.body").to_owned(),
            actions: vec![
                Button::new(crate::i18n::t("lists.see_units"), Variant::Secondary).href("/units"),
            ],
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
                    <h1>{crate::i18n::t("create.reference")}</h1>
                    <p>
                        {crate::i18n::t("lists.new_source.intro")}
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
                            section_head(crate::i18n::t("lists.new_source.section"), None, None),
                            view! {
                                {workspace_destination(workspaces)}
                                {text_field(
                                    "src-title",
                                    crate::i18n::t("lists.field.title"),
                                    "title",
                                    crate::i18n::t("lists.ph.work_title"),
                                    "text",
                                )}
                                {text_field(
                                    "src-authors",
                                    crate::i18n::t("lists.field.authors"),
                                    "authors",
                                    crate::i18n::t("lists.ph.semicolon"),
                                    "text",
                                )}
                                {text_field("src-year", crate::i18n::t("lists.field.year"), "year", crate::i18n::t("lists.ph.year"), "text")}
                                {text_field(
                                    "src-container",
                                    crate::i18n::t("lists.field.publication"),
                                    "container_title",
                                    crate::i18n::t("lists.ph.venue"),
                                    "text",
                                )}
                                {text_field("src-doi", "DOI", "doi", crate::i18n::t("lists.ph.doi"), "text")}
                                {textarea(
                                    "src-abstract",
                                    crate::i18n::t("lists.field.abstract"),
                                    "abstract_text",
                                    crate::i18n::t("lists.ph.abstract_work"),
                                    92,
                                )}
                                {select(
                                    "src-classification",
                                    crate::i18n::t("lists.field.classification"),
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
                                Button::new(crate::i18n::t("action.cancel"), Variant::Secondary)
                                    .href("/bibliography"),
                            )}
                            {button(Button::new(crate::i18n::t("lists.create.reference_btn"), Variant::Gold))}
                        </div>
                    </form>
                }
                    .into_any()
            } else {
                no_destination(crate::i18n::t("lists.noun.sources")).into_any()
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
                    <h1>{crate::i18n::t("create.dataset")}</h1>
                    <p>
                        {crate::i18n::t("lists.new_dataset.intro")}
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
                            section_head(crate::i18n::t("lists.new_dataset.section"), None, None),
                            view! {
                                {workspace_destination(workspaces)}
                                {text_field(
                                    "ds-code",
                                    crate::i18n::t("lists.field.code"),
                                    "code",
                                    crate::i18n::t("lists.ph.dataset_code"),
                                    "text",
                                )}
                                {text_field("ds-title", crate::i18n::t("lists.field.title"), "title", crate::i18n::t("lists.ph.dataset_name"), "text")}
                                {textarea(
                                    "ds-description",
                                    crate::i18n::t("lists.field.description"),
                                    "description",
                                    crate::i18n::t("lists.ph.dataset_desc"),
                                    92,
                                )}
                                {text_field(
                                    "ds-keywords",
                                    crate::i18n::t("lists.field.keywords"),
                                    "keywords",
                                    crate::i18n::t("lists.ph.comma_separated"),
                                    "text",
                                )}
                                {text_field(
                                    "ds-restrictions",
                                    crate::i18n::t("lists.field.usage_restrictions"),
                                    "usage_restrictions",
                                    crate::i18n::t("lists.ph.usage_limits"),
                                    "text",
                                )}
                                {select(
                                    "ds-classification",
                                    crate::i18n::t("lists.field.classification"),
                                    "classification",
                                    vec![
                                        ("INTERNAL".to_owned(), true),
                                        ("CONFIDENTIAL".to_owned(), true),
                                        ("RESTRICTED".to_owned(), true),
                                    ],
                                )}
                                <p class="oc-muted oc-t-caption--muted">
                                    {crate::i18n::t("lists.new_dataset.class_note")}
                                </p>
                            },
                        )}

                        <div class="oc-row--end oc-gap-5 oc-mt-8">
                            {button(Button::new(crate::i18n::t("action.cancel"), Variant::Secondary).href("/datasets"))}
                            {button(Button::new(crate::i18n::t("lists.create.dataset_btn"), Variant::Gold))}
                        </div>
                    </form>
                }
                    .into_any()
            } else {
                no_destination(crate::i18n::t("lists.noun.datasets")).into_any()
            }}
        </div>
    }
}

/// O formulário de criação de uma tarefa.
///
/// Uma tarefa pertence a um Research Workspace — a mesma unidade de contexto que
/// governa ideias, referências e datasets. Sem nenhum ambiente onde criar, o
/// formulário diz onde a filiação se obtém, em vez de se declarar indisponível.
///
/// O responsável **não** se escolhe aqui: quem pode ser atribuído depende do
/// ambiente escolhido, e resolve-se no detalhe da tarefa. A tarefa nasce por
/// atribuir, e é uma atribuição legítima.
pub fn new_task(workspaces: &Value, error: Option<String>) -> impl IntoView {
    use crate::ui::components::{
        card, section_head, select_labelled, text_field, textarea, SelectOption,
    };

    let tem_destino = destinations(workspaces) > 0;

    view! {
        <div class="oc-page oc-page--narrow">
            <div class="oc-head">
                <div class="oc-head__text">
                    <h1>{crate::i18n::t("create.task")}</h1>
                    <p>
                        {crate::i18n::t("lists.new_task.intro")}
                    </p>
                </div>
            </div>

            {error
                .map(|message| {
                    view! { <div class="oc-card oc-alert" role="alert">{message}</div> }
                })}

            {if tem_destino {
                view! {
                    <form method="post" action="/tasks/new">
                        {card(
                            section_head(crate::i18n::t("lists.new_task.section"), None, None),
                            view! {
                                {workspace_destination(workspaces)}
                                {text_field(
                                    "task-title",
                                    crate::i18n::t("lists.field.title"),
                                    "title",
                                    crate::i18n::t("lists.ph.task_title"),
                                    "text",
                                )}
                                {textarea(
                                    "task-description",
                                    crate::i18n::t("lists.field.description"),
                                    "description",
                                    crate::i18n::t("lists.ph.task_desc"),
                                    92,
                                )}
                                {select_labelled(
                                    "task-priority",
                                    crate::i18n::t("lists.field.priority"),
                                    "priority",
                                    vec![
                                        SelectOption::new("normal", crate::i18n::t("task.priority.normal")).selected(true),
                                        SelectOption::new("low", crate::i18n::t("task.priority.low")),
                                        SelectOption::new("high", crate::i18n::t("task.priority.high")),
                                        SelectOption::new("critical", crate::i18n::t("lists.priority.critical")),
                                    ],
                                )}
                                {text_field("task-due", crate::i18n::t("lists.field.due"), "due_on", "", "date")}
                                <p class="oc-muted oc-t-caption--muted">
                                    {crate::i18n::t("lists.new_task.responsible_note")}
                                </p>
                            },
                        )}

                        <div class="oc-row--end oc-gap-5 oc-mt-8">
                            {button(Button::new(crate::i18n::t("action.cancel"), Variant::Secondary).href("/my-work"))}
                            {button(Button::new(crate::i18n::t("lists.create.task_btn"), Variant::Gold))}
                        </div>
                    </form>
                }
                    .into_any()
            } else {
                no_destination(crate::i18n::t("lists.noun.tasks")).into_any()
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
                    <h1>{crate::i18n::t("create.project")}</h1>
                    <p>
                        {crate::i18n::t("lists.new_project.intro")}
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
                            section_head(crate::i18n::t("lists.new_project.section"), None, None),
                            view! {
                                <div class="oc-field">
                                    <label class="oc-field__label" for="promote-idea">
                                        {crate::i18n::t("lists.new_project.eligible_idea")}
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
                                        {crate::i18n::t("lists.new_project.eligible_hint")}
                                    </p>
                                </div>

                                {text_field(
                                    "project-code",
                                    crate::i18n::t("lists.field.project_code"),
                                    "code",
                                    crate::i18n::t("lists.ph.project_code"),
                                    "text",
                                )}
                                {text_field(
                                    "project-title",
                                    crate::i18n::t("lists.field.title"),
                                    "title",
                                    crate::i18n::t("lists.ph.project_title"),
                                    "text",
                                )}
                                {textarea(
                                    "project-objectives",
                                    crate::i18n::t("lists.field.objectives"),
                                    "objectives",
                                    crate::i18n::t("lists.ph.objectives"),
                                    92,
                                )}
                            },
                        )}

                        <div class="oc-row--end oc-gap-5 oc-mt-8">
                            {button(Button::new(crate::i18n::t("action.cancel"), Variant::Secondary).href("/projects"))}
                            {button(Button::new(crate::i18n::t("lists.promote_btn"), Variant::Gold))}
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
                        title: crate::i18n::t("lists.new_project.none_title").to_owned(),
                        body: crate::i18n::t("lists.new_project.none_body").to_owned(),
                        actions: vec![
                            Button::new(crate::i18n::t("lists.see_ideas"), Variant::Secondary)
                                .href("/ideas"),
                        ],
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
    use crate::ui::components::{card, section_head, textarea};

    view! {
        <div class="oc-page oc-page--narrow">
            <div class="oc-head">
                <div class="oc-head__text">
                    <h1>{crate::i18n::t("lists.new.unit")}</h1>
                    <p>
                        {crate::i18n::t("lists.new_unit.intro")}
                    </p>
                </div>
            </div>

            {error
                .map(|message| {
                    view! { <div class="oc-card oc-alert" role="alert">{message}</div> }
                })}

            // Sem campo de código: o código é institucional e gerado. Quem cria
            // uma unidade dá-lhe um nome; o Ocinye atribui o identificador.
            <form method="post" action="/units/new">
                {card(
                    section_head(crate::i18n::t("lists.new_unit.section"), None, None),
                    view! {
                        <div class="oc-field">
                            <label class="oc-field__label" for="unit-name">{crate::i18n::t("lists.field.name")}</label>
                            <input
                                class="oc-input"
                                id="unit-name"
                                name="name"
                                type="text"
                                placeholder=crate::i18n::t("lists.ph.unit_name")
                                autocomplete="off"
                                data-oc-code-source
                            />
                        </div>

                        // O código gerado, mostrado a quem cria. Sem JavaScript,
                        // fica a explicação; com JavaScript, o código previsto
                        // aparece aqui à medida que o nome é escrito.
                        <div class="oc-field">
                            <label class="oc-field__label" for="unit-code-preview">{crate::i18n::t("lists.field.code")}</label>
                            <output
                                class="oc-code-preview"
                                id="unit-code-preview"
                                data-oc-code-preview
                                data-oc-code-endpoint="/units/code-suggestion"
                            >
                                {crate::i18n::t("lists.new_unit.code_generated")}
                            </output>
                        </div>

                        {textarea(
                            "unit-description",
                            crate::i18n::t("lists.field.description"),
                            "description",
                            crate::i18n::t("lists.ph.unit_investigates"),
                            92,
                        )}

                        // As áreas de investigação. Base: um campo de texto com
                        // valores separados por vírgulas, que funciona sem
                        // JavaScript. Com JavaScript, o app.js promove-o a fichas.
                        <div class="oc-field">
                            <label class="oc-field__label" for="unit-areas">
                                {crate::i18n::t("lists.field.research_areas")}
                            </label>
                            <input
                                class="oc-input"
                                id="unit-areas"
                                name="research_areas"
                                type="text"
                                placeholder=crate::i18n::t("lists.ph.comma_separated")
                                autocomplete="off"
                                data-oc-chips
                                data-oc-chips-hint="Escreva uma área e prima Enter."
                            />
                        </div>
                    },
                )}

                <div class="oc-row--end oc-gap-5 oc-mt-8">
                    {button(Button::new(crate::i18n::t("action.cancel"), Variant::Secondary).href("/units"))}
                    {button(Button::new(crate::i18n::t("lists.create.unit_btn"), Variant::Gold))}
                </div>
            </form>
        </div>
    }
}

/// O formulário de edição de uma unidade.
///
/// O código **não** é editável — é identidade institucional e aparece em
/// citações; renomear nunca renumera. Por isso é mostrado, e não pedido. Os
/// campos vêm preenchidos com o que já lá está: editar não é começar do zero.
pub fn edit_unit(unit: &Value, error: Option<String>) -> impl IntoView {
    use crate::ui::components::{card, field_with_value, section_head, textarea_with_value};

    let id = text(unit, "id");
    let code = text(unit, "code");
    let code_hidden = code.clone();
    let name = text(unit, "name");
    let description = text(unit, "description");
    let areas = string_list(unit, "research_areas").join(", ");
    let action = format!("/units/{id}/edit");

    view! {
        <div class="oc-page oc-page--narrow">
            <div class="oc-head">
                <div class="oc-head__text">
                    <h1>{crate::i18n::t("lists.edit.unit")}</h1>
                    <p>{crate::i18n::t("lists.edit_unit.subtitle")}</p>
                </div>
            </div>

            {error
                .map(|message| {
                    view! { <div class="oc-card oc-alert" role="alert">{message}</div> }
                })}

            <form method="post" action=action>
                {card(
                    section_head(crate::i18n::t("lists.new_unit.section"), None, None),
                    view! {
                        <div class="oc-field">
                            <label class="oc-field__label" for="unit-code-fixed">{crate::i18n::t("lists.field.code")}</label>
                            <input
                                class="oc-input oc-input--readonly"
                                id="unit-code-fixed"
                                type="text"
                                value=code
                                readonly
                                aria-describedby="unit-code-note"
                            />
                            // Round-trip do código para o re-render de erro; nunca vai ao Core.
                            <input type="hidden" name="code" value=code_hidden />
                            <p class="oc-field__note" id="unit-code-note">
                                {crate::i18n::t("lists.edit_unit.code_note")}
                            </p>
                        </div>

                        {field_with_value(
                            "unit-name",
                            crate::i18n::t("lists.field.name"),
                            "name",
                            crate::i18n::t("lists.ph.unit_name"),
                            "text",
                            name,
                        )}
                        {textarea_with_value(
                            "unit-description",
                            crate::i18n::t("lists.field.description"),
                            "description",
                            crate::i18n::t("lists.ph.unit_investigates"),
                            92,
                            description,
                        )}

                        <div class="oc-field">
                            <label class="oc-field__label" for="unit-areas">
                                {crate::i18n::t("lists.field.research_areas")}
                            </label>
                            <input
                                class="oc-input"
                                id="unit-areas"
                                name="research_areas"
                                type="text"
                                value=areas
                                placeholder=crate::i18n::t("lists.ph.comma_separated")
                                autocomplete="off"
                                data-oc-chips
                                data-oc-chips-hint="Escreva uma área e prima Enter."
                            />
                        </div>
                    },
                )}

                <div class="oc-row--end oc-gap-5 oc-mt-8">
                    {button(Button::new(crate::i18n::t("action.cancel"), Variant::Secondary).href(format!("/units/{id}")))}
                    {button(Button::new(crate::i18n::t("action.save"), Variant::Gold))}
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
                    <h1>{crate::i18n::t("create.idea")}</h1>
                    <p>
                        {crate::i18n::t("lists.new_idea.intro")}
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
                            section_head(crate::i18n::t("lists.new_idea.section"), None, None),
                            view! {
                                // O selector usa o mesmo rótulo composto que a
                                // lista de unidades; o valor submetido é o id.
                                {units_select(&unit_rows)}
                                {text_field("idea-title", crate::i18n::t("lists.field.title"), "title", crate::i18n::t("lists.ph.idea_title"), "text")}
                                {textarea(
                                    "idea-question",
                                    crate::i18n::t("lists.field.research_question"),
                                    "research_question",
                                    crate::i18n::t("lists.ph.research_question"),
                                    64,
                                )}
                                {textarea(
                                    "idea-hypothesis",
                                    crate::i18n::t("lists.field.hypothesis"),
                                    "hypothesis",
                                    crate::i18n::t("lists.ph.hypothesis"),
                                    64,
                                )}
                                {textarea(
                                    "idea-motivation",
                                    crate::i18n::t("lists.field.motivation"),
                                    "motivation",
                                    crate::i18n::t("lists.ph.motivation"),
                                    64,
                                )}
                                {textarea("idea-summary", crate::i18n::t("lists.field.summary"), "summary", crate::i18n::t("lists.ph.idea_summary"), 92)}
                                {text_field(
                                    "idea-keywords",
                                    crate::i18n::t("lists.field.keywords"),
                                    "keywords",
                                    crate::i18n::t("lists.ph.comma_separated"),
                                    "text",
                                )}
                                {select(
                                    "idea-classification",
                                    crate::i18n::t("lists.field.classification"),
                                    "classification",
                                    vec![
                                        ("INTERNAL".to_owned(), true),
                                        ("CONFIDENTIAL".to_owned(), true),
                                        ("RESTRICTED".to_owned(), true),
                                    ],
                                )}
                                <p class="oc-muted oc-t-caption--muted" >
                                    {crate::i18n::t("lists.new_idea.class_note")}
                                </p>
                            },
                        )}

                        <div class="oc-row--end oc-gap-5 oc-mt-8" >
                            {button(Button::new(crate::i18n::t("action.cancel"), Variant::Secondary).href("/ideas"))}
                            {button(Button::new(crate::i18n::t("lists.create.idea_btn"), Variant::Gold))}
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
                            <h3>{crate::i18n::t("lists.new_idea.no_units_title")}</h3>
                            <p>
                                {crate::i18n::t("lists.new_idea.no_units_body")}
                            </p>
                            <div class="oc-empty__actions">
                                {button(Button::new(crate::i18n::t("lists.see_units_lc"), Variant::Secondary).href("/units"))}
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
            <label class="oc-field__label" for="idea-unit">{crate::i18n::t("lists.field.unit")}</label>
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
                    <h1>{crate::i18n::t("lists.tools.title")}</h1>
                    <p>
                        {crate::i18n::t("lists.tools.intro")}
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
                            section_head(crate::i18n::t("lists.tools.section"), None, None),
                            view! {
                                {workspace_destination(workspaces)}
                                <div class="oc-field">
                                    <label class="oc-field__label" for="bibtex">"BibTeX"</label>
                                    <textarea
                                        class="oc-textarea"
                                        id="bibtex"
                                        name="bibtex"
                                        rows="12"
                                        placeholder=crate::i18n::t("lists.ph.bibtex")
                                    >{escrito}</textarea>
                                </div>
                                <div class="oc-actions">
                                    <button type="submit" class="oc-btn oc-btn--navy">
                                        {crate::i18n::t("lists.tools.validate")}
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
                            title: crate::i18n::t("lists.tools.none_title").to_owned(),
                            body: crate::i18n::t("lists.tools.none_body").to_owned(),
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
        crate::i18n::tf(
            "lists.review.all_readable",
            &[("count", &lidas.to_string())],
        )
    } else {
        crate::i18n::tf(
            "lists.review.some_unread",
            &[
                ("read", &lidas.to_string()),
                ("unread", &por_ler.to_string()),
            ],
        )
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
                section_head(crate::i18n::t("lists.review.section"), None, None),
                view! {
                    <p class="oc-t-body">
                        {badge(
                            if completa {
                                crate::i18n::t("lists.review.readable")
                            } else {
                                crate::i18n::t("lists.review.problems")
                            },
                            if completa { Tone::Ok } else { Tone::Gold },
                        )}
                        " "
                        {resumo}
                    </p>

                    {(!ilegiveis.is_empty())
                        .then(|| {
                            view! {
                                <div class="oc-mt-4">
                                    <p class="oc-t-strong">{crate::i18n::t("lists.review.unreadable_head")}</p>
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
                                    <p class="oc-t-strong">{crate::i18n::t("lists.review.read_head")}</p>
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
                            {crate::i18n::t("lists.review.normalised")}
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
    pub(super) fn viewer() -> Viewer {
        Viewer {
            pinned: crate::ui::apps::default_pins(),
            inactive_apps: Vec::new(),
            resolucao: crate::ui::shell::ResolucaoSessao::Resolvida,
            sessao_privilegiada: false,
            administra: false,
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
            pinned: crate::ui::apps::default_pins(),
            inactive_apps: Vec::new(),
            resolucao: crate::ui::shell::ResolucaoSessao::Resolvida,
            sessao_privilegiada: false,
            administra: false,
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
                html.contains(sem_autorizacao()),
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

    /// As listas de Ideias e Projectos são servidas por `/workspaces?kind=…`: o
    /// `id` de cada linha é o do **ambiente**. A linha tem de ligar a
    /// `/workspaces/{id}` — ligar a `/ideas/{id}` ou `/projects/{id}` daria esse
    /// id de ambiente a uma rota que espera um id de ideia/projecto, e caía em
    /// «Página não encontrada» (defeito da aceitação em produção).
    #[test]
    fn a_linha_de_ideia_ou_projecto_liga_ao_ambiente() {
        let ws = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa";
        let payload = json!({
            "items": [{ "id": ws, "code": "UCS-001-IDEA-001", "title": "Nzayilu",
                        "state": "discovery", "classification": "INTERNAL" }],
            "total": 1
        });
        let ideias = ideas(&viewer(), &payload, Slice::default()).to_html();
        assert!(
            ideias.contains(&format!("href=\"/workspaces/{ws}\"")),
            "a linha da ideia não liga ao ambiente"
        );
        assert!(
            !ideias.contains(&format!("/ideas/{ws}")),
            "a linha da ideia liga a /ideas/{{ambiente}} — a rota errada (404)"
        );

        let projectos = projects(&viewer(), &payload, Slice::default()).to_html();
        assert!(
            projectos.contains(&format!("href=\"/workspaces/{ws}\"")),
            "a linha do projecto não liga ao ambiente"
        );
        assert!(
            !projectos.contains(&format!("/projects/{ws}")),
            "a linha do projecto liga a /projects/{{ambiente}} — a rota errada (404)"
        );
    }

    /// A coluna «Unidade» da lista de membros reflecte a pertença real, sem
    /// inventar uma unidade principal (CLAUDE.md §34.3): com uma, o nome; com
    /// várias, a contagem; sem nenhuma, «—».
    #[test]
    fn a_coluna_de_unidade_reflecte_a_pertenca() {
        let payload = json!({
            "items": [
                { "id": "00000000-0000-0000-0000-000000000001", "full_name": "Sem Unidade",
                  "email": "s@ocinye.com", "status": "active", "units": [] },
                { "id": "00000000-0000-0000-0000-000000000002", "full_name": "Uma Unidade",
                  "email": "u@ocinye.com", "status": "active",
                  "units": [{ "code": "ESC", "name": "Energia e Sistemas Computacionais" }] },
                { "id": "00000000-0000-0000-0000-000000000003", "full_name": "Duas Unidades",
                  "email": "d@ocinye.com", "status": "active",
                  "units": [{ "code": "ESC", "name": "Energia" }, { "code": "BIO", "name": "Bio" }] },
            ],
            "total": 3
        });
        let html = members(&viewer(), &payload).to_html();
        // Com uma, o nome — nunca o UUID.
        assert!(
            html.contains("Energia e Sistemas Computacionais"),
            "uma unidade devia mostrar-se pelo nome"
        );
        // Com várias, a contagem honesta — sem eleger uma principal.
        assert!(
            html.contains("2 unidades"),
            "várias unidades resumem-se pela contagem, sem inventar uma principal"
        );
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

    /// A auditoria diz quem executou **e** quem responde.
    #[test]
    fn a_auditoria_resolve_as_duas_camadas_de_quem_agiu() {
        let linha = json!({
            "actor_name": "Fidel Admin",
            "actor_identity_kind": "privileged",
            "actor_on_behalf_of": "Fidel Monteiro"
        });
        let lido = actor_da_auditoria(&linha);
        assert!(
            lido.contains("Fidel Admin"),
            "perdeu-se qual identidade executou: {lido}"
        );
        assert!(
            lido.contains("Fidel Monteiro"),
            "perdeu-se quem responde pela identidade privilegiada: {lido}"
        );
    }

    /// Uma pessoa comum não ganha uma segunda camada inventada.
    ///
    /// Acrescentar «em nome de si próprio» a toda a gente seria ruído a esconder
    /// exactamente os casos que a coluna existe para destacar.
    #[test]
    fn uma_pessoa_comum_aparece_como_sempre_apareceu() {
        for linha in [
            json!({"actor_name": "Ana Fernandes", "actor_identity_kind": "human"}),
            json!({"actor_name": "Ana Fernandes", "actor_on_behalf_of": null}),
            json!({"actor_name": "Ana Fernandes", "actor_on_behalf_of": ""}),
        ] {
            assert_eq!(
                actor_da_auditoria(&linha),
                "Ana Fernandes",
                "uma pessoa comum ganhou uma camada que não existe: {linha}"
            );
        }
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
        assert!(html.contains("RESTRITO"));
    }

    #[test]
    fn as_dimensoes_sao_legiveis() {
        assert_eq!(size(&json!({"n": 512}), "n"), "512 B");
        assert_eq!(size(&json!({"n": 2048}), "n"), "2.0 kB");
        assert_eq!(size(&json!({"n": 5_368_709_120_i64}), "n"), "5.0 GB");
        assert_eq!(size(&json!({}), "n"), "—");
    }

    /// A «Nova Unidade» não pede um código: ele é gerado.
    #[test]
    fn nova_unidade_nao_pede_codigo_e_anuncia_que_e_gerado() {
        let html = new_unit(None).to_html();
        // Não há campo submissível `name="code"`.
        assert!(
            !html.contains("name=\"code\""),
            "a criação não pode pedir um código: ele é gerado"
        );
        // Há o alvo da pré-visualização e a fonte (o nome).
        assert!(html.contains("data-oc-code-preview"));
        assert!(html.contains("data-oc-code-source"));
        // As áreas são um campo promovível a fichas, com o nome que o Core lê.
        assert!(html.contains("data-oc-chips"));
        assert!(html.contains("name=\"research_areas\""));
    }

    /// A «Nova Tarefa» resolve o ambiente e oferece os campos canónicos.
    #[test]
    fn nova_tarefa_resolve_ambiente_e_oferece_os_campos() {
        // Com um ambiente onde criar, há formulário para /tasks/new.
        let com = json!({
            "items": [
                {"id": "22222222-2222-2222-2222-222222222222",
                 "code": "P-001", "title": "Projecto", "may_create": true}
            ]
        });
        let html = new_task(&com, None).to_html();
        assert!(html.contains("action=\"/tasks/new\""));
        assert!(html.contains("name=\"workspace_id\""));
        assert!(html.contains("name=\"title\""));
        assert!(html.contains("name=\"priority\""));
        assert!(html.contains("name=\"due_on\""));

        // Sem ambiente, um estado accionável — nunca «indisponível».
        let sem = json!({"items": []});
        let vazio = new_task(&sem, None).to_html();
        assert!(vazio.contains("Não tem onde criar tarefas"));
        assert!(!vazio.contains("action=\"/tasks/new\""));
    }

    /// Editar mostra o código como fixo e traz os campos preenchidos.
    #[test]
    fn editar_unidade_mostra_o_codigo_fixo_e_preenche_os_campos() {
        let unit = json!({
            "id": "11111111-1111-1111-1111-111111111111",
            "code": "UCS-001",
            "name": "Computação e Sistemas",
            "description": "Descrição existente",
            "research_areas": ["Sistemas distribuídos", "Engenharia de software"],
        });
        let html = edit_unit(&unit, None).to_html();
        // O código aparece, mas só-de-leitura — não é um campo que se submeta
        // como mutável (vai num hidden apenas para re-render de erro).
        assert!(html.contains("UCS-001"));
        assert!(html.contains("readonly"));
        // Os campos vêm preenchidos.
        assert!(html.contains("Computação e Sistemas"));
        assert!(html.contains("Descrição existente"));
        assert!(html.contains("Sistemas distribuídos, Engenharia de software"));
        // O formulário aponta para a rota de edição.
        assert!(html.contains("/units/11111111-1111-1111-1111-111111111111/edit"));
    }
}

/// Um ecrã, uma língua: os ecrãs de lista e um formulário de criação em francês,
/// com marcas francesas presentes e o chrome português ausente. A prova segue o
/// mesmo molde das outras migrações (ai.rs, settings.rs): renderiza dentro de
/// `with_locale(Locale::Fr, …)` e afirma sobre o HTML resultante.
#[cfg(test)]
mod pureza_i18n {
    use super::tests::viewer;
    use super::*;
    use crate::i18n::{with_locale, Locale};
    use serde_json::json;

    /// Confirma que todas as marcas francesas aparecem e nenhuma portuguesa.
    fn so_frances(html: &str, francesas: &[&str], portuguesas: &[&str]) {
        for fr in francesas {
            assert!(html.contains(fr), "fr: falta «{fr}»");
        }
        for pt in portuguesas {
            assert!(!html.contains(pt), "fr: chrome português «{pt}»");
        }
    }

    #[tokio::test]
    async fn as_unidades_nao_misturam_linguas() {
        let payload = json!({"items": [], "total": 0});
        let fr = with_locale(Locale::Fr, async { units(&viewer(), &payload).to_html() }).await;
        so_frances(
            &fr,
            &[
                "Nouvelle unité",
                "Toutes les unités institutionnelles",
                "RESPONSABLE",
                "MEMBRES",
            ],
            &[
                "Nova Unidade",
                "Todas as unidades institucionais",
                "RESPONSÁVEL",
                "MEMBROS",
            ],
        );
    }

    #[tokio::test]
    async fn as_ideias_nao_misturam_linguas() {
        let payload = json!({"items": [], "total": 0});
        let fr = with_locale(Locale::Fr, async {
            ideas(&viewer(), &payload, Slice::default()).to_html()
        })
        .await;
        so_frances(
            &fr,
            &["Nouvelle idée", "TITRE", "Toutes", "De l’unité"],
            &["Nova Ideia", "TÍTULO", "Todas", "Da Unidade"],
        );
    }

    #[tokio::test]
    async fn os_datasets_nao_misturam_linguas() {
        let payload = json!({"items": [], "total": 0});
        let fr = with_locale(Locale::Fr, async {
            datasets(&viewer(), &payload).to_html()
        })
        .await;
        so_frances(
            &fr,
            &["Nouveau jeu de données", "TAILLE", "VERSION"],
            &["Novo Dataset", "TAMANHO"],
        );
    }

    #[tokio::test]
    async fn os_membros_nao_misturam_linguas() {
        let payload = json!({"items": [], "total": 0});
        let fr = with_locale(Locale::Fr, async { members(&viewer(), &payload).to_html() }).await;
        so_frances(
            &fr,
            &["Ajouter un utilisateur", "Rôles", "Services"],
            &["Adicionar Utilizador", "Funções", "Membros"],
        );
    }

    #[tokio::test]
    async fn o_audit_nao_mistura_linguas() {
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
        let fr = with_locale(Locale::Fr, async { audit(&viewer(), &payload).to_html() }).await;
        so_frances(
            &fr,
            &[
                "Journal d’audit",
                "Authentification",
                "REFUSÉ",
                "ID DE CORRÉLATION",
            ],
            &["Audit Log", "Autenticação", "NEGADO", "CORRELATION ID"],
        );
    }

    #[tokio::test]
    async fn o_formulario_de_ideia_nao_mistura_linguas() {
        let units = json!({
            "items": [
                {"id": "11111111-1111-1111-1111-111111111111", "code": "UCS-001", "name": "Unité"}
            ]
        });
        let fr = with_locale(Locale::Fr, async { new_idea(&units, None).to_html() }).await;
        so_frances(
            &fr,
            &[
                "Nouvelle idée",
                "Question de recherche",
                "Créer l’idée",
                "Annuler",
            ],
            &[
                "Nova Ideia",
                "Pergunta de investigação",
                "Criar Ideia",
                "Cancelar",
            ],
        );
    }
}
