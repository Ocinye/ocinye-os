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
use crate::ui::icon::{icon, Icon};

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
    fn render(self, right: bool) -> AnyView {
        let align = if right {
            "oc-cell oc-cell--r"
        } else {
            "oc-cell"
        };

        match self {
            Self::Primary(text) => {
                view! { <div class=format!("{align} oc-cell--first")>{text}</div> }.into_any()
            }
            Self::Text(text) => {
                view! { <div class=format!("{align} oc-cell--text")>{text}</div> }.into_any()
            }
            Self::Mono(text) => {
                view! { <div class=format!("{align} oc-cell--mono")>{text}</div> }.into_any()
            }
            Self::Badge(label, tone) => {
                view! { <div class=align>{badge(label, tone)}</div> }.into_any()
            }
            Self::Classification(value) => {
                view! { <div class=align>{super::badge::classification_badge(&value)}</div> }
                    .into_any()
            }
            Self::Progress(pct) => view! { <div class=align>{progress_bar(pct)}</div> }.into_any(),
            Self::Empty => {
                view! { <div class=format!("{align} oc-cell--text")>"—"</div> }.into_any()
            }
        }
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
        format!("Filtrar {search} nesta página…")
    } else {
        format!("Filtrar {search}…")
    };

    let column_count = columns.len();
    let alignment: Vec<bool> = columns.iter().map(|c| c.right).collect();
    let is_empty = rows.is_empty();

    view! {
        <section class=format!("oc-card oc-table oc-table--{shape}") data-dense="false">
            <div class="oc-table__bar">
                // Os separadores de filtro eram `<button role="tab">` sem
                // handler: clicar não fazia nada. O Core ainda não expõe estes
                // recortes — «Minhas», «Da Unidade», «Arquivadas» — como
                // parâmetros de consulta, por isso são declarados indisponíveis
                // em vez de fingirem uma escolha (briefing §2C, §95).
                <div class="oc-tabs" role="tablist" aria-label="Recortes da lista">
                    {tabs
                        .into_iter()
                        .map(|tab| match tab.state {
                            TabState::Current => {
                                view! {
                                    <span class="oc-tab" role="tab" aria-selected="true">
                                        {tab.label}
                                    </span>
                                }
                                    .into_any()
                            }
                            TabState::Available(query) => {
                                view! {
                                    <a
                                        class="oc-tab"
                                        role="tab"
                                        aria-selected="false"
                                        href=query
                                    >
                                        {tab.label}
                                    </a>
                                }
                                    .into_any()
                            }
                            // Não é «indisponível»: é uma capacidade que o
                            // produto ainda não tem. A Ajuda distingue os dois
                            // estados para o membro, e a barra tem de os
                            // distinguir também — «volta daqui a pouco» e
                            // «ainda não existe» pedem coisas diferentes a quem
                            // está à espera.
                            TabState::NotImplemented(razao) => {
                                view! {
                                    <span
                                        class="oc-tab oc-unavailable"
                                        role="tab"
                                        aria-selected="false"
                                        aria-disabled="true"
                                        title=razao
                                    >
                                        {tab.label}
                                    </span>
                                }
                                    .into_any()
                            }
                        })
                        .collect_view()}
                </div>

                <div class="oc-spacer"></div>

                // A pesquisa da lista é local e funciona sem rede: filtra as
                // linhas já renderizadas. Antes era um `<input>` fora de
                // qualquer formulário e sem handler — escrever e carregar em
                // Enter não fazia nada (briefing §3).
                <div class="oc-table__search">
                    {icon(Icon::Search, 14)}
                    <label class="oc-sr" for="table-search">{rotulo.clone()}</label>
                    <input
                        id="table-search"
                        type="search"
                        data-oc="table-filter"
                        autocomplete="off"
                        placeholder=rotulo
                    />
                </div>

                // «Filtrar» foi retirado: era um botão sem handler e sem painel
                // por trás. Volta quando o Core aceitar filtros na consulta.
                <button
                    type="button"
                    class="oc-table__filter"
                    data-oc="density"
                    aria-pressed="false"
                    title="Alternar densidade das linhas"
                >
                    "Densidade"
                </button>
            </div>

            <div class="oc-table__scroll">
                <div class="oc-table__head" role="row">
                    {columns
                        .into_iter()
                        .map(|column| {
                            let class = if column.right { "oc-th--r" } else { "" };
                            view! { <span role="columnheader" class=class>{column.label}</span> }
                        })
                        .collect_view()}
                </div>

                {if is_empty {
                    view! {
                        <div class="oc-empty" >
                            <div class="oc-empty__tile oc-empty__tile--sm">
                                {icon(Icon::EmptyState, 22)}
                            </div>
                            <p>{empty}</p>
                        </div>
                    }
                        .into_any()
                } else {
                    rows.into_iter()
                        .map(|(href, cells)| {
                            // As células são construídas dentro de cada ramo:
                            // uma vista Leptos consome-se uma só vez.
                            let alignment = alignment.clone();
                            let render_cells = move || {
                                cells
                                    .into_iter()
                                    .enumerate()
                                    .map(|(i, cell)| {
                                        cell.render(alignment.get(i).copied().unwrap_or(false))
                                    })
                                    .collect_view()
                            };

                            match href {
                                // A linha é um `<a>`, não um `<div>` com um
                                // handler: assim funciona com o teclado, com o
                                // clique do meio e sem JavaScript.
                                Some(href) => {
                                    view! {
                                        <a class="oc-table__row" role="row" href=href>
                                            {render_cells()}
                                        </a>
                                    }
                                        .into_any()
                                }
                                None => {
                                    view! {
                                        <div class="oc-table__row" role="row">
                                            {render_cells()}
                                        </div>
                                    }
                                        .into_any()
                                }
                            }
                        })
                        .collect_view()
                        .into_any()
                }}
            </div>

            // Paginação real.
            //
            // Os controlos tinham sido retirados porque eram botões sem
            // handler: o «seguinte» aparecia activo e não levava a lado nenhum.
            // Entretanto o Workspace pedia uma página só, e o rodapé dizia
            // honestamente «1–50 de 213» — o que era verdade e não resolvia
            // nada: a linha 51 continuava inalcançável.
            //
            // Agora os destinos são URLs que o servidor reconhece, e carregam
            // consigo os filtros activos: mudar de página não é mudar de
            // consulta.
            //
            // Cada lado só aparece quando existe. Um «anterior» na primeira
            // página é um controlo que promete um sítio que não há.
            <div class="oc-table__foot">
                {previous
                    .map(|href| {
                        view! {
                            <a class="oc-page-link" href=href rel="prev">
                                "← Anterior"
                            </a>
                        }
                    })}
                <span class="oc-table__count">{footer}</span>
                {next
                    .map(|href| {
                        view! {
                            <a class="oc-page-link" href=href rel="next">
                                "Seguinte →"
                            </a>
                        }
                    })}
            </div>
        </section>
    }
    .into_any()
    .attr("data-columns", column_count.to_string())
}
