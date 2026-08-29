//! Ficheiros — os ficheiros institucionais de um Research Workspace.
//!
//! # O que este ecrã é
//!
//! Um sítio para arrumar, carregar, navegar, ver, versionar e descarregar
//! ficheiros. Nada mais.
//!
//! # O que este ecrã não faz
//!
//! Não atribui significado institucional. Carregar um PDF aqui não cria um
//! Document, um Dataset nem uma Source: cria um ficheiro, que é o que a pessoa
//! fez. Afirmar conhecimento é um acto separado, e continua a sê-lo.
//!
//! As pastas também não decidem nada. Mudar um ficheiro RESTRICTED para uma
//! pasta chamada «Público» muda onde ele aparece na navegação e mais nada — a
//! classificação continua a ser a do ficheiro, composta com a do ambiente.

use leptos::prelude::*;
use serde_json::Value;

use crate::ui::components::{
    classification_badge, data_table, empty_state, Cell, Column, EmptyState, Table,
};
use crate::ui::icon::{icon, Icon};

fn text(row: &Value, key: &str) -> String {
    row.get(key)
        .and_then(Value::as_str)
        .unwrap_or("—")
        .to_owned()
}

fn number(row: &Value, key: &str) -> i64 {
    row.get(key).and_then(Value::as_i64).unwrap_or(0)
}

/// Um tamanho legível.
///
/// Não é cosmética: `4823718` não diz nada a quem está a decidir se descarrega
/// um ficheiro numa ligação fraca, e «4,8 MB» diz.
#[must_use]
pub fn tamanho(bytes: i64) -> String {
    #[allow(clippy::cast_precision_loss)]
    let b = bytes as f64;
    if bytes < 1024 {
        return format!("{bytes} B");
    }
    if bytes < 1024 * 1024 {
        return format!("{:.0} KB", b / 1024.0);
    }
    if bytes < 1024 * 1024 * 1024 {
        return format!("{:.1} MB", b / (1024.0 * 1024.0));
    }
    format!("{:.1} GB", b / (1024.0 * 1024.0 * 1024.0))
}

/// O que a página de navegação precisa de saber.
pub struct FilesView {
    /// Os ambientes que este membro alcança, para escolher onde está.
    pub workspaces: Vec<(String, String)>,
    /// O ambiente aberto, se algum.
    pub workspace_id: Option<String>,
    /// O nome do ambiente aberto.
    pub workspace_name: String,
    /// A pasta aberta, se alguma.
    pub folder_id: Option<String>,
    /// Da raiz até à pasta actual.
    pub path: Vec<Value>,
    /// As pastas dentro da pasta actual.
    pub folders: Vec<Value>,
    /// Os ficheiros dentro da pasta actual.
    pub files: Vec<Value>,
    /// Se este membro pode carregar ficheiros.
    pub may_upload: bool,
    /// Uma mensagem a mostrar, vinda da operação anterior.
    pub notice: Option<(bool, String)>,
}

/// O ecrã de Ficheiros.
#[allow(clippy::too_many_lines)]
pub fn files(view: FilesView) -> impl IntoView {
    let FilesView {
        workspaces,
        workspace_id,
        workspace_name,
        folder_id,
        path,
        folders,
        files,
        may_upload,
        notice,
    } = view;

    let Some(workspace_id) = workspace_id else {
        return escolher_ambiente(workspaces).into_any();
    };

    let base = format!("/files?workspace={workspace_id}");
    let aqui = folder_id
        .as_ref()
        .map_or_else(|| base.clone(), |folder| format!("{base}&folder={folder}"));

    let trilho = trilho_de_pastas(&base, &path, &workspace_name);

    let linhas: Vec<(Option<String>, Vec<Cell>)> = folders
        .iter()
        .map(|pasta| {
            let id = text(pasta, "id");
            (
                Some(format!("{base}&folder={id}")),
                vec![
                    Cell::Primary(format!("📁 {}", text(pasta, "name"))),
                    Cell::Text("Pasta".to_owned()),
                    Cell::Empty,
                    Cell::Empty,
                    Cell::Empty,
                ],
            )
        })
        .chain(files.iter().map(|ficheiro| {
            let id = text(ficheiro, "id");
            (
                Some(format!("/files/{id}")),
                vec![
                    Cell::Primary(text(ficheiro, "name")),
                    Cell::Text(text(ficheiro, "content_type")),
                    Cell::Classification(text(ficheiro, "classification")),
                    Cell::Mono(tamanho(number(ficheiro, "size_bytes"))),
                    Cell::Mono(format!("v{}", number(ficheiro, "versions"))),
                ],
            )
        }))
        .collect();

    let vazio = linhas.is_empty();
    let total = linhas.len();

    view! {
        <div class="oc-page">
            <div class="oc-head">
                <div class="oc-head__text">
                    <h1>"Ficheiros"</h1>
                    <p>
                        "Os ficheiros institucionais de "
                        {workspace_name.clone()}
                        ". Arrumar não é classificar."
                    </p>
                </div>
                {seletor_de_ambiente(&workspaces, &workspace_id)}
            </div>

            {notice.map(|(ok, mensagem)| aviso(ok, &mensagem))}

            {trilho}

            {may_upload
                .then(|| barra_de_accoes(&workspace_id, folder_id.as_deref(), &aqui))}

            {if vazio {
                empty_state(EmptyState {
                    icon: Icon::Files,
                    title: "Ainda não há nada aqui".to_owned(),
                    body: "Esta pasta está vazia. Carregue um ficheiro ou crie uma pasta \
                           para começar a arrumar."
                        .to_owned(),
                    actions: vec![],
                    small: false,
                })
                    .into_any()
            } else {
                data_table(Table {
                    tabs: vec![],
                    search: "Filtrar ficheiros",
                    truncated: false,
                    shape: "oc-table--files",
                    columns: vec![
                        Column::new("Nome"),
                        Column::new("Tipo"),
                        Column::new("Classificação"),
                        Column::right("Tamanho"),
                        Column::right("Versões"),
                    ],
                    rows: linhas,
                    footer: format!("{total} a mostrar"),
                    previous: None,
                    next: None,
                    empty: "Esta pasta está vazia.",
                })
                    .into_any()
            }}
        </div>
    }
    .into_any()
}

/// Sem ambiente escolhido não há ficheiros para mostrar.
///
/// Não se escolhe um por omissão: «o primeiro da lista» é uma escolha que a
/// interface faz por alguém, e num ecrã onde a classificação depende do
/// ambiente essa escolha silenciosa é a errada.
fn escolher_ambiente(workspaces: Vec<(String, String)>) -> impl IntoView {
    view! {
        <div class="oc-page">
            <div class="oc-head">
                <div class="oc-head__text">
                    <h1>"Ficheiros"</h1>
                    <p>"Escolha o ambiente cujos ficheiros quer ver."</p>
                </div>
            </div>

            {if workspaces.is_empty() {
                empty_state(EmptyState {
                    icon: Icon::Files,
                    title: "Não alcança nenhum ambiente".to_owned(),
                    body: "Os ficheiros institucionais vivem dentro de Research Workspaces. \
                           Quando pertencer a um, aparece aqui."
                        .to_owned(),
                    actions: vec![],
                    small: false,
                })
                    .into_any()
            } else {
                view! {
                    <div class="oc-grid oc-grid--3">
                        {workspaces
                            .into_iter()
                            .map(|(id, nome)| {
                                view! {
                                    <a
                                        class="oc-card oc-card--clickable oc-card__body \
                                               oc-card__body--block"
                                        href=format!("/files?workspace={id}")
                                    >
                                        <div class="oc-t-meta">"AMBIENTE"</div>
                                        <div class="oc-t-strong oc-mt-5">{nome}</div>
                                    </a>
                                }
                            })
                            .collect_view()}
                    </div>
                }
                    .into_any()
            }}
        </div>
    }
}

fn seletor_de_ambiente(workspaces: &[(String, String)], actual: &str) -> impl IntoView {
    let opcoes = workspaces
        .iter()
        .map(|(id, nome)| {
            let escolhido = id == actual;
            view! {
                <option value=id.clone() selected=escolhido>
                    {nome.clone()}
                </option>
            }
        })
        .collect_view();

    view! {
        <form class="oc-head__aside" method="get" action="/files">
            <label class="oc-sr" for="oc-files-workspace">"Ambiente"</label>
            <select
                class="oc-select"
                id="oc-files-workspace"
                name="workspace"
                data-autosubmit="1"
            >
                {opcoes}
            </select>
            <noscript>
                <button class="oc-btn oc-btn--ghost" type="submit">"Ver"</button>
            </noscript>
        </form>
    }
}

/// O trilho de pastas.
///
/// Cada degrau é uma pasta real e navegável. A raiz chama-se pelo nome do
/// ambiente porque é isso que ela é: o ambiente, e não uma pasta chamada
/// «raiz» que ninguém criou.
fn trilho_de_pastas(base: &str, path: &[Value], workspace_name: &str) -> impl IntoView {
    let raiz = base.to_owned();
    let degraus = path
        .iter()
        .map(|pasta| {
            let id = text(pasta, "id");
            let nome = text(pasta, "name");
            view! {
                <span class="oc-crumbs__sep" aria-hidden="true">"/"</span>
                <a class="oc-crumbs__step" href=format!("{base}&folder={id}")>{nome}</a>
            }
        })
        .collect_view();

    view! {
        <nav class="oc-crumbs oc-mb-5" aria-label="Pastas">
            <a class="oc-crumbs__step" href=raiz>
                {icon(Icon::Folder, 14)}
                {workspace_name.to_owned()}
            </a>
            {degraus}
        </nav>
    }
}

/// Carregar e criar pasta, lado a lado.
///
/// Os dois formulários funcionam sem JavaScript: o `input[type=file]` submete,
/// e o campo de nome cria a pasta. A zona de largada é enriquecimento por cima
/// do que já funciona — se o browser não colaborar, o botão continua lá.
fn barra_de_accoes(workspace_id: &str, folder_id: Option<&str>, regresso: &str) -> impl IntoView {
    let folder = folder_id.unwrap_or_default().to_owned();
    view! {
        <section class="oc-files__bar oc-mb-5">
            <form
                class="oc-drop"
                method="post"
                action="/files/upload"
                enctype="multipart/form-data"
                data-drop="1"
            >
                <input type="hidden" name="workspace_id" value=workspace_id.to_owned() />
                <input type="hidden" name="folder_id" value=folder.clone() />
                <input type="hidden" name="return_to" value=regresso.to_owned() />

                <div class="oc-drop__face">
                    {icon(Icon::Files, 20)}
                    <div>
                        <div class="oc-t-strong">"Largue ficheiros aqui"</div>
                        <div class="oc-t-caption--muted">
                            "ou escolha do computador. Ficam nesta pasta."
                        </div>
                    </div>
                </div>

                <div class="oc-drop__controls">
                    <label class="oc-sr" for="oc-files-file">"Ficheiro"</label>
                    <input
                        class="oc-input"
                        id="oc-files-file"
                        type="file"
                        name="file"
                        required
                        data-drop-input="1"
                    />

                    <label class="oc-sr" for="oc-files-class">"Classificação"</label>
                    <select class="oc-select" id="oc-files-class" name="classification">
                        <option value="">"Herdar do ambiente"</option>
                        <option value="PUBLIC">"Público"</option>
                        <option value="INTERNAL">"Interno"</option>
                        <option value="CONFIDENTIAL">"Confidencial"</option>
                        <option value="RESTRICTED">"Restrito"</option>
                    </select>

                    <button class="oc-btn oc-btn--primary" type="submit">"Carregar"</button>
                </div>

                <div class="oc-drop__tray" data-drop-tray="1" hidden></div>
            </form>

            <form class="oc-card oc-files__folder" method="post" action="/files/folder">
                <input type="hidden" name="workspace_id" value=workspace_id.to_owned() />
                <input type="hidden" name="parent_id" value=folder />
                <input type="hidden" name="return_to" value=regresso.to_owned() />
                <div class="oc-card__body">
                    <label class="oc-label" for="oc-files-folder">"Nova pasta"</label>
                    <input
                        class="oc-input"
                        id="oc-files-folder"
                        type="text"
                        name="name"
                        maxlength="128"
                        required
                        placeholder="Ensaios de Março"
                    />
                    <button class="oc-btn oc-btn--ghost" type="submit">"Criar pasta"</button>
                </div>
            </form>
        </section>
    }
}

fn aviso(ok: bool, mensagem: &str) -> impl IntoView {
    let classe = if ok {
        "oc-note oc-note--ok"
    } else {
        "oc-note oc-note--bad"
    };
    view! { <p class=classe role="status">{mensagem.to_owned()}</p> }
}

// ── O ficheiro, visto de perto ──────────────────────────────────────────────

/// O que a página de um ficheiro precisa de saber.
pub struct FileDetailView {
    /// O ficheiro, como o Core o descreve.
    pub file: Value,
    /// O histórico completo, da versão mais recente para a mais antiga.
    pub versions: Vec<Value>,
    /// O conteúdo, quando é texto e cabe. `None` quando não se pode mostrar.
    pub preview: Preview,
    /// Se este membro pode carregar uma versão nova.
    pub may_upload: bool,
    /// Uma mensagem a mostrar, vinda da operação anterior.
    pub notice: Option<(bool, String)>,
}

/// O que se pode honestamente mostrar do conteúdo.
///
/// # Porque não há uma caixa cinzenta a fingir
///
/// Uma pré-visualização que falha em silêncio ensina que o ficheiro está
/// corrompido. Estes três estados são distintos e a interface distingue-os: ou
/// se mostra o conteúdo, ou se diz que o tipo não se mostra aqui, ou se diz que
/// é grande de mais para mostrar inteiro.
pub enum Preview {
    /// Texto, tal como está guardado.
    Text(String),
    /// O tipo não se pré-visualiza nesta superfície.
    UnsupportedType(String),
    /// Cabe no formato, mas não no ecrã.
    TooLarge(i64),
    /// Não se conseguiu ler o conteúdo agora.
    Unavailable(String),
}

/// A página de um ficheiro.
#[allow(clippy::too_many_lines)]
pub fn file_detail(view: FileDetailView) -> impl IntoView {
    let FileDetailView {
        file,
        versions,
        preview,
        may_upload,
        notice,
    } = view;

    let id = text(&file, "id");
    let nome = text(&file, "name");
    let workspace_id = text(&file, "workspace_id");
    let corrente = versions.first().cloned().unwrap_or(Value::Null);

    let linhas: Vec<(Option<String>, Vec<Cell>)> = versions
        .iter()
        .map(|v| {
            let vid = text(v, "id");
            (
                Some(format!("/file-versions/{vid}/download")),
                vec![
                    Cell::Primary(format!("v{}", number(v, "sequence"))),
                    Cell::Text(text(v, "created_by")),
                    Cell::Mono(text(v, "created_at").chars().take(10).collect()),
                    Cell::Mono(tamanho(number(v, "size_bytes"))),
                    Cell::Mono(text(v, "checksum_sha256").chars().take(12).collect()),
                ],
            )
        })
        .collect();

    let contagem = versions.len();

    view! {
        <div class="oc-page">
            <div class="oc-head">
                <div class="oc-head__text">
                    <h1>{nome.clone()}</h1>
                    <p>
                        "Ficheiro institucional em "
                        <a href=format!("/files?workspace={workspace_id}")>
                            {text(&file, "workspace_name")}
                        </a>
                    </p>
                </div>
                <div class="oc-head__aside">
                    {classification_badge(&text(&file, "classification"))}
                    <a class="oc-btn oc-btn--primary" href=format!("/files/{id}/download")>
                        "Descarregar"
                    </a>
                </div>
            </div>

            {notice.map(|(ok, mensagem)| aviso(ok, &mensagem))}

            <div class="oc-split">
                <section class="oc-card">
                    <div class="oc-card__head"><h2>"Conteúdo"</h2></div>
                    <div class="oc-card__body">{previsualizacao(preview)}</div>
                </section>

                <section class="oc-card">
                    <div class="oc-card__head"><h2>"Detalhes"</h2></div>
                    <div class="oc-card__body">
                        {detalhe("Tipo", &text(&corrente, "content_type"))}
                        {detalhe("Tamanho", &tamanho(number(&corrente, "size_bytes")))}
                        {detalhe("Versões", &contagem.to_string())}
                        {detalhe(
                            "Classificação efectiva",
                            &text(&file, "classification"),
                        )}
                        {detalhe("Ambiente", &text(&file, "workspace_name"))}
                        {detalhe(
                            "Classificação do ambiente",
                            &text(&file, "workspace_classification"),
                        )}
                        {detalhe("Soma SHA-256", &text(&corrente, "checksum_sha256"))}
                    </div>
                </section>
            </div>

            {may_upload
                .then(|| {
                    view! {
                        <section class="oc-card oc-mt-5">
                            <div class="oc-card__head"><h2>"Carregar nova versão"</h2></div>
                            <div class="oc-card__body">
                                <p class="oc-t-caption--muted">
                                    "A versão actual não é substituída. Fica no histórico, \
                                     citável exactamente como está."
                                </p>
                                <form
                                    method="post"
                                    action=format!("/files/{id}/version")
                                    enctype="multipart/form-data"
                                >
                                    <label class="oc-sr" for="oc-version-file">"Ficheiro"</label>
                                    <input
                                        class="oc-input"
                                        id="oc-version-file"
                                        type="file"
                                        name="file"
                                        required
                                    />
                                    <button class="oc-btn oc-btn--primary" type="submit">"Carregar versão"</button>
                                </form>
                            </div>
                        </section>
                    }
                })}

            <section class="oc-mt-5">
                <h2 class="oc-t-strong oc-mb-5">"Histórico de versões"</h2>
                {data_table(Table {
                    tabs: vec![],
                    search: "Filtrar versões",
                    truncated: false,
                    shape: "oc-table--versions",
                    columns: vec![
                        Column::new("Versão"),
                        Column::new("Por"),
                        Column::new("Quando"),
                        Column::right("Tamanho"),
                        Column::right("Soma"),
                    ],
                    rows: linhas,
                    footer: format!("{contagem} versões"),
                    previous: None,
                    next: None,
                    empty: "Este ficheiro ainda não tem versões.",
                })}
            </section>
        </div>
    }
}

fn detalhe(rotulo: &str, valor: &str) -> impl IntoView {
    view! {
        <div class="oc-kv">
            <span class="oc-kv__k">{rotulo.to_owned()}</span>
            <span class="oc-kv__v">{valor.to_owned()}</span>
        </div>
    }
}

fn previsualizacao(preview: Preview) -> impl IntoView {
    match preview {
        Preview::Text(conteudo) => view! {
            <pre class="oc-pre" tabindex="0">{conteudo}</pre>
        }
        .into_any(),
        Preview::UnsupportedType(tipo) => view! {
            <div class="oc-note">
                <p class="oc-t-strong">"Sem pré-visualização para " {tipo}</p>
                <p class="oc-t-caption--muted">
                    "Esta superfície mostra ficheiros de texto. Descarregue o ficheiro \
                     para o abrir na aplicação que o lê."
                </p>
            </div>
        }
        .into_any(),
        Preview::TooLarge(bytes) => view! {
            <div class="oc-note">
                <p class="oc-t-strong">"Grande de mais para mostrar aqui"</p>
                <p class="oc-t-caption--muted">
                    {format!(
                        "São {}. Descarregue o ficheiro para o ver inteiro.",
                        tamanho(bytes),
                    )}
                </p>
            </div>
        }
        .into_any(),
        Preview::Unavailable(razao) => view! {
            <div class="oc-note oc-note--bad">
                <p class="oc-t-strong">"Não foi possível ler o conteúdo"</p>
                <p class="oc-t-caption--muted">{razao}</p>
            </div>
        }
        .into_any(),
    }
}
