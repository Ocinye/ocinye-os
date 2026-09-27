//! A tabela institucional.
//!
//! Oito ecrãs de lista partilham este componente — Unidades, Ideias, Projectos,
//! Bibliografia, Dados, Agentes, Membros e Audit Log. Nenhum deles define uma
//! tabela própria (`design/README.md` §6.4).
//!
//! A grelha de colunas vem do design tal como está; as larguras não são
//! recalculadas aqui.

use leptos::prelude::*;

use super::badge::{badge, Tone};
use super::progress::progress_bar;

/// Uma coluna.
pub struct Column {
    /// Rótulo em maiúsculas, mono.
    pub label: &'static str,
    /// Alinhamento à direita, para valores numéricos.
    pub right: bool,
}

impl Column {
    /// Uma coluna alinhada à esquerda.
    #[must_use]
    pub const fn new(label: &'static str) -> Self {
        Self {
            label,
            right: false,
        }
    }

    /// Uma coluna numérica, alinhada à direita.
    #[must_use]
    pub const fn right(label: &'static str) -> Self {
        Self { label, right: true }
    }
}

/// O conteúdo de uma célula.
pub enum Cell {
    /// A primeira célula da linha: mais escura e com mais peso.
    Primary(String),
    /// Texto secundário.
    Text(String),
    /// Código, data, DOI, versão ou identificador.
    Mono(String),
    /// Um estado, com ponto e texto.
    Badge(String, Tone),
    /// Uma classificação institucional.
    Classification(String),
    /// Progresso, em percentagem.
    Progress(u8),
    /// Sem valor. Renderiza um travessão em vez de um espaço vazio, para que
    /// se distinga de uma célula que falhou a carregar.
    Empty,
}

impl Cell {
    /// A célula da tabela do D1. A primeira coluna leva a ligação da linha,
    /// para o teclado e para quem lê com um leitor de ecrã.
    fn render(self, right: bool, href: Option<&str>) -> AnyView {
        let class = if right { "ods-num" } else { "" };
        let conteudo = match self {
            Self::Primary(text) => match href {
                Some(h) => view! { <a href=h.to_owned()>{text}</a> }.into_any(),
                None => view! { <b>{text}</b> }.into_any(),
            },
            Self::Text(text) | Self::Mono(text) => text.into_any(),
            Self::Badge(label, tone) => badge(label, tone).into_any(),
            Self::Classification(value) => super::badge::classification_badge(&value).into_any(),
            Self::Progress(pct) => progress_bar(pct).into_any(),
            Self::Empty => "—".into_any(),
        };
        view! { <td class=class>{conteudo}</td> }.into_any()
    }
}

/// Uma tabela completa: barra de controlo, cabeçalho, linhas e rodapé.
pub struct Table {
    /// Tabs da barra de controlo.
    pub tabs: Vec<ListTab>,
    /// O nome plural do que a lista contém, para o campo de filtro.
    pub search: &'static str,
    /// Se o Core tem mais linhas do que as que vieram.
    ///
    /// Muda o que o campo de filtro promete. Sem isto, o campo diz «Pesquisar
    /// datasets…» sobre as cinquenta linhas que a página recebeu, e quem
    /// escrever o nome do quinquagésimo primeiro conclui que ele não existe.
    pub truncated: bool,
    /// A forma da tabela: o sufixo da classe `oc-table--…` que declara as
    /// colunas e a largura mínima na folha de estilos.
    ///
    /// As colunas não vêm num atributo `style` porque a CSP do Workspace
    /// declara `style-src 'self'` sem `'unsafe-inline'`, e o browser descarta
    /// esse atributo antes de pintar. Sem colunas, `display: grid` cai para uma
    /// só e o cabeçalho empilha-se por cima das linhas.
    pub shape: &'static str,
    /// As colunas.
    pub columns: Vec<Column>,
    /// As linhas. Cada uma pode ter um destino.
    pub rows: Vec<(Option<String>, Vec<Cell>)>,
    /// Texto de contagem no rodapé.
    pub footer: String,
    /// A página anterior, quando existe.
    pub previous: Option<String>,
    /// A página seguinte, quando existe.
    pub next: Option<String>,
    /// Mensagem quando não há linhas.
    pub empty: &'static str,
}

/// O que uma tab da barra pode ser.
#[derive(Debug, Clone)]
pub enum TabState {
    /// O recorte que está a ser mostrado.
    Current,
    /// Um recorte real, alcançável neste destino.
    Available(String),
    /// Uma capacidade que o produto ainda não tem, com a razão à vista.
    NotImplemented(&'static str),
}

/// Uma tab da barra de controlo de uma lista.
#[derive(Debug, Clone)]
pub struct ListTab {
    /// O rótulo.
    pub label: &'static str,
    /// O que ela é.
    pub state: TabState,
}

impl ListTab {
    /// O recorte actual.
    #[must_use]
    pub const fn current(label: &'static str) -> Self {
        Self {
            label,
            state: TabState::Current,
        }
    }

    /// Um recorte real, com o destino que o produz.
    #[must_use]
    pub fn to(label: &'static str, query: impl Into<String>) -> Self {
        Self {
            label,
            state: TabState::Available(query.into()),
        }
    }

    /// Um recorte que o produto ainda não sabe fazer.
    ///
    /// A razão é obrigatória e é mostrada: «não está disponível» sem dizer
    /// porquê é a mesma frase para uma capacidade em falta, uma configuração em
    /// falta e uma avaria — e essas três pedem coisas diferentes a quem lê.
    #[must_use]
    pub const fn missing(label: &'static str, reason: &'static str) -> Self {
        Self {
            label,
            state: TabState::NotImplemented(reason),
        }
    }
}

/// Renderiza a tabela.
pub fn data_table(table: Table) -> impl IntoView {
    let Table {
        tabs,
        search,
        truncated,
        shape,
        columns,
        rows,
        footer,
        previous,
        next,
        empty,
    } = table;

    // O campo diz o que faz. Quando a página traz tudo, filtrar a página é
    // filtrar a lista, e dizer «Filtrar unidades…» é verdade. Quando não traz,
    // a diferença tem de aparecer: filtrar cinquenta linhas de duzentas não é
    // pesquisar duzentas, e quem não encontrar o que procura merece saber
    // porquê.
    let rotulo = if truncated {
        crate::i18n::tf("table.filter_page", &[("noun", search)])
    } else {
        crate::i18n::tf("table.filter", &[("noun", search)])
    };

    let column_count = columns.len();
    let alignment: Vec<bool> = columns.iter().map(|c| c.right).collect();
    let is_empty = rows.is_empty();

    view! {
        <section class="ods-widget-surface" data-oc="table" data-shape=shape data-dense="false">
            <div class="ods-app__toolbar">
                <div class="ods-tabs" role="tablist" aria-label=crate::i18n::t("table.slices_aria")>
                    {tabs
                        .into_iter()
                        .map(|tab| match tab.state {
                            TabState::Current => view! {
                                <span class="ods-tabs__tab" data-part="tab" role="tab" aria-selected="true">
                                    {tab.label}
                                </span>
                            }
                            .into_any(),
                            TabState::Available(query) => view! {
                                <a class="ods-tabs__tab" data-part="tab" role="tab" aria-selected="false" href=query>
                                    {tab.label}
                                </a>
                            }
                            .into_any(),
                            TabState::NotImplemented(razao) => view! {
                                <span
                                    class="ods-tabs__tab"
                                    data-part="tab unavailable"
                                    role="tab"
                                    aria-selected="false"
                                    aria-disabled="true"
                                    title=razao
                                >
                                    {tab.label}
                                </span>
                            }
                            .into_any(),
                        })
                        .collect_view()}
                </div>
                <span class="ods-app__toolbar-spacer"></span>
                <label class="ods-search">
                    {crate::ui::ods::icone("search", "")}
                    <span class="ods-sr-only">{rotulo.clone()}</span>
                    <input
                        id="table-search"
                        class="ods-search__input"
                        type="search"
                        data-oc="table-filter"
                        autocomplete="off"
                        placeholder=rotulo
                    />
                </label>
                <button
                    type="button"
                    class="ods-btn ods-btn--ghost ods-btn--sm"
                    data-oc="density"
                    aria-pressed="false"
                    title=crate::i18n::t("table.toggle_density")
                >
                    {crate::i18n::t("table.density")}
                </button>
            </div>

            // O cabeçalho fica mesmo vazia: diz que colunas a lista teria, e o
            // vazio diz-se por baixo dele.
            <table class="ods-table">
                <thead>
                    <tr>
                        {columns
                            .into_iter()
                            .map(|column| {
                                let class = if column.right { "ods-num" } else { "" };
                                view! { <th scope="col" class=class>{column.label}</th> }
                            })
                            .collect_view()}
                    </tr>
                </thead>
                <tbody>
                    {rows
                        .into_iter()
                        .map(|(href, cells)| {
                            let alignment = alignment.clone();
                            let destino = href.clone();
                            let celulas = cells
                                .into_iter()
                                .enumerate()
                                .map(|(i, cell)| {
                                    cell.render(
                                        alignment.get(i).copied().unwrap_or(false),
                                        if i == 0 { destino.as_deref() } else { None },
                                    )
                                })
                                .collect_view();
                            // A linha inteira abre o destino (`app.js`,
                            // `data-oc-href`); a ligação real é a da primeira
                            // célula.
                            view! { <tr data-oc="table-row" data-oc-href=href>{celulas}</tr> }
                        })
                        .collect_view()}
                </tbody>
            </table>
            {is_empty.then(|| view! {
                <div class="ods-empty">
                    <span class="ods-empty__icon">{crate::ui::ods::icone("grid", "ods-icon--lg")}</span>
                    <p class="ods-empty__body">{empty}</p>
                </div>
            })}

            <div class="ods-d12-pager">
                {previous.map(|href| view! {
                    <a class="ods-btn ods-btn--sm" href=href rel="prev">{crate::i18n::t("table.previous")}</a>
                })}
                <span data-oc="table-count">{footer}</span>
                {next.map(|href| view! {
                    <a class="ods-btn ods-btn--sm" href=href rel="next">{crate::i18n::t("table.next")}</a>
                })}
            </div>
        </section>
    }
    .into_any()
    .attr("data-columns", column_count.to_string())
}
