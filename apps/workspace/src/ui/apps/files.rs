//! D004 · Ficheiros. O espaço de ficheiros do Ocinye, não o do anfitrião.
//!
//! Lista densa ou grelha, caminho, pesquisa desta pasta, selecção múltipla com
//! barra de acções, inspector (detalhes, pré-visualização segura, versões),
//! bandeja de envios. Arrastar é sempre uma alternativa, nunca o único caminho:
//! «Enviar» abre o selector; «Mover» é um formulário.

use leptos::prelude::*;

use super::{empty, frame, kind_icon, load_state, more, nav, nye, primary, search};
use crate::i18n::{t, tf};
use crate::ui::components::icon;
use crate::ui::view_models::{
    FileDetailsVm, FileItemVm, FileKind, FilePreviewVm, FilesSection, FilesSortKey, FilesView,
    FilesVm, UploadState, UploadVm,
};

fn sort_head(
    vm: &FilesVm,
    key: FilesSortKey,
    label: &'static str,
    class: &'static str,
) -> impl IntoView {
    let (cur, desc) = vm.sort;
    let on = cur == key;
    let name = match key {
        FilesSortKey::Name => "name",
        FilesSortKey::Modified => "modified",
        FilesSortKey::Size => "size",
        FilesSortKey::Kind => "kind",
    };
    let next = if on && !desc { "desc" } else { "asc" };
    let sort = if !on {
        "none"
    } else if desc {
        "descending"
    } else {
        "ascending"
    };
    view! {
        <th scope="col" class=class aria-sort=sort>
            <a class="oc-files-sort" href=format!("?sort={name}&dir={next}")>
                {t(label)}
                {on.then(|| icon(if desc { "chev-d" } else { "chev-u" }))}
            </a>
        </th>
    }
}

fn check(item: &FileItemVm) -> impl IntoView {
    view! {
        <label class="oc-files-check">
            <span class="oc-sr">{tf("files.select", &[("name", item.name.as_str())])}</span>
            <input type="checkbox" name="item" value=item.id.clone() checked=item.selected data-part="files-check" />
        </label>
    }
}

fn row(item: &FileItemVm, can_write: bool) -> impl IntoView {
    let folder = item.kind == FileKind::Folder;
    view! {
        <tr class="oc-files-row" data-part="files-item" data-kind=if folder { "folder" } else { "file" } aria-selected=if item.selected { "true" } else { "false" } data-open=item.open.then_some("")>
            <td class="oc-files-c-check">{check(item)}</td>
            <td class="oc-files-c-name">
                <a class="oc-files-name" href=item.href.clone() data-part="files-open" draggable=can_write.then_some("true") data-id=item.id.clone()>
                    <span class="oc-files-ic" data-kind=if folder { "folder" } else { "file" } aria-hidden="true">{icon(kind_icon(item.kind))}</span>
                    <span class="oc-files-name__text">{item.name.clone()}</span>
                    {item.favourite.then(|| view! { <span class="oc-files-flag" title=t("files.fav")>{icon("star-fill")}<span class="oc-sr">{t("files.fav")}</span></span> })}
                    {item.shared.then(|| view! { <span class="oc-files-flag" title=t("files.shared")>{icon("share")}<span class="oc-sr">{t("files.shared")}</span></span> })}
                </a>
            </td>
            <td class="oc-files-c-kind">{item.kind_label.clone()}</td>
            <td class="oc-files-c-mod">{item.modified.clone()}</td>
            <td class="oc-files-c-size">{item.size.clone().unwrap_or_else(|| "—".into())}</td>
            <td class="oc-files-c-ctx">{item.context.clone().unwrap_or_default()}</td>
        </tr>
    }
}

fn tile(item: &FileItemVm, can_write: bool) -> impl IntoView {
    let folder = item.kind == FileKind::Folder;
    view! {
        <li class="oc-files-tile" data-part="files-item" data-kind=if folder { "folder" } else { "file" } aria-selected=if item.selected { "true" } else { "false" } data-open=item.open.then_some("")>
            {check(item)}
            <a class="oc-files-tile__link" href=item.href.clone() data-part="files-open" draggable=can_write.then_some("true") data-id=item.id.clone()>
                <span class="oc-files-tile__thumb" aria-hidden="true">
                    {match item.thumb.clone() {
                        Some(src) => view! { <img src=src alt="" loading="lazy" /> }.into_any(),
                        None => icon(kind_icon(item.kind)).into_any(),
                    }}
                </span>
                <span class="oc-files-tile__name">{item.name.clone()}</span>
                <span class="oc-files-tile__meta">{item.size.clone().map_or_else(|| item.modified.clone(), |s| format!("{s} · {}", item.modified))}</span>
            </a>
        </li>
    }
}

fn preview(p: &FilePreviewVm) -> AnyView {
    match p {
        FilePreviewVm::Image { src } => view! { <figure class="oc-files-prev" data-kind="image"><img src=src.clone() alt="" /></figure> }.into_any(),
        FilePreviewVm::Pdf { src } => view! {
            <figure class="oc-files-prev" data-kind="pdf">
                <iframe src=src.clone() title=t("files.preview") sandbox=""></iframe>
            </figure>
        }
        .into_any(),
        FilePreviewVm::Text { text, lang, truncated } => view! {
            <figure class="oc-files-prev" data-kind="text">
                <pre data-lang=lang.clone()><code>{text.clone()}</code></pre>
                {truncated.then(|| view! { <figcaption>{t("files.preview.truncated")}</figcaption> })}
            </figure>
        }
        .into_any(),
        FilePreviewVm::Unsupported => view! {
            <figure class="oc-files-prev" data-kind="none"><p>{icon("eye")}<span>{t("files.preview.none")}</span></p></figure>
        }
        .into_any(),
    }
}

fn details(d: &FileDetailsVm, can_write: bool) -> impl IntoView {
    let it = &d.item;
    let kv = |k: &'static str, v: String| view! { <div class="oc-app-kv"><dt>{t(k)}</dt><dd>{v}</dd></div> };
    view! {
        <div class="oc-app-insp" data-part="files-details">
            <header class="oc-app-insp__head">
                <span class="oc-files-ic" data-kind=if it.kind == FileKind::Folder { "folder" } else { "file" } aria-hidden="true">{icon(kind_icon(it.kind))}</span>
                <h2 class="oc-app-insp__title">{it.name.clone()}</h2>
                <a class="oc-app__icon" href="?" data-oc="app-insp-close" aria-label=t("app.details.close")>{icon("close")}</a>
            </header>
            {d.preview.as_ref().map(preview)}
            <div class="oc-app-insp__actions">
                {d.download_href.clone().map(|h| view! { <a class="oc-app-btn" href=h>{icon("download")}{t("files.download")}</a> })}
                {d.nye.as_ref().map(nye)}
            </div>
            <dl class="oc-app-kvs">
                {kv("files.col.kind", it.kind_label.clone())}
                {it.size.clone().map(|s| kv("files.col.size", s))}
                {kv("files.col.modified", it.modified.clone())}
                {d.created.clone().map(|c| kv("files.created", c))}
                {d.owner.clone().map(|o| kv("files.owner", o))}
                {it.context.clone().map(|c| kv("files.col.context", c))}
                {(!d.shared_with.is_empty()).then(|| kv("files.shared_with", d.shared_with.join(", ")))}
            </dl>
            {can_write.then(|| view! {
                <form class="oc-app-inline" method="post" action=format!("/files/{}/rename", it.id)>
                    <label class="oc-app-field"><span>{t("files.rename")}</span><input name="name" value=it.name.clone() required="" /></label>
                    <button type="submit" class="oc-app-btn">{t("files.rename.do")}</button>
                </form>
            })}
            <section class="oc-files-versions" aria-labelledby="oc-files-v-t">
                <h3 class="oc-app-insp__sub" id="oc-files-v-t">{t("files.versions")}</h3>
                {match &d.versions {
                    None => view! { <p class="oc-app-note">{t("files.versions.none")}</p> }.into_any(),
                    Some(vs) => view! {
                        <ol class="oc-files-vlist">
                            {vs.iter().map(|v| view! {
                                <li class="oc-files-v" data-current=v.current.then_some("")>
                                    <span class="oc-files-v__label">{v.label.clone()}{v.current.then(|| view! { <span class="oc-app-tag">{t("files.versions.current")}</span> })}</span>
                                    <span class="oc-files-v__meta">{[Some(v.at.clone()), v.author.clone(), Some(v.size.clone())].into_iter().flatten().collect::<Vec<_>>().join(" · ")}</span>
                                    <span class="oc-files-v__act">
                                        <a class="oc-app__icon" href=v.download_href.clone() aria-label=tf("files.versions.download", &[("v", v.label.as_str())])>{icon("download")}</a>
                                        {v.restore_action.clone().map(|a| view! {
                                            <form method="post" action=a><button type="submit" class="oc-app__icon" aria-label=tf("files.versions.restore", &[("v", v.label.as_str())])>{icon("restart")}</button></form>
                                        })}
                                    </span>
                                </li>
                            }).collect_view()}
                        </ol>
                    }.into_any(),
                }}
            </section>
        </div>
    }
}

fn upload(u: &UploadVm) -> impl IntoView {
    let (state, label, bar): (&str, String, Option<(u32, u32)>) = match &u.state {
        UploadState::Queued => ("queued", t("files.up.queued").into(), None),
        UploadState::Checking => ("checking", t("files.up.checking").into(), None),
        UploadState::Sending { text, parts } => ("sending", text.clone(), Some(*parts)),
        UploadState::Done => ("done", t("files.up.done").into(), None),
        UploadState::Conflict => ("conflict", t("files.up.conflict").into(), None),
        UploadState::Failed(e) => ("failed", t(&format!("{}.title", e.key())).into(), None),
        UploadState::Cancelled => ("cancelled", t("files.up.cancelled").into(), None),
    };
    let busy = matches!(
        u.state,
        UploadState::Queued | UploadState::Checking | UploadState::Sending { .. }
    );
    let retry = matches!(u.state, UploadState::Failed(_) | UploadState::Cancelled);
    view! {
        <li class="oc-files-up" data-part="files-upload" data-id=u.id.clone() data-state=state>
            <span class="oc-files-up__name">{u.name.clone()}<span class="oc-files-up__size">{u.size.clone()}</span></span>
            {match bar {
                Some((d, n)) => view! { <progress max=n.to_string() value=d.to_string() aria-label=u.name.clone()></progress> }.into_any(),
                None if busy => view! { <progress aria-label=u.name.clone()></progress> }.into_any(),
                None => ().into_any(),
            }}
            <span class="oc-files-up__state" role="status">{label}</span>
            {busy.then(|| view! { <button type="button" class="oc-app__icon" data-oc="files-up-cancel" data-id=u.id.clone() aria-label=tf("files.up.cancel", &[("name", u.name.as_str())])>{icon("close")}</button> })}
            {retry.then(|| view! { <button type="button" class="oc-app__icon" data-oc="files-up-retry" data-id=u.id.clone() aria-label=tf("files.up.retry", &[("name", u.name.as_str())])>{icon("refresh")}</button> })}
            {(u.state == UploadState::Conflict).then(|| view! {
                <span class="oc-files-up__choice">
                    <button type="button" class="oc-app-btn" data-oc="files-up-keep" data-id=u.id.clone()>{t("files.up.keep_both")}</button>
                    <button type="button" class="oc-app-btn" data-oc="files-up-version" data-id=u.id.clone()>{t("files.up.new_version")}</button>
                </span>
            })}
        </li>
    }
}

/// A aplicação Ficheiros.
pub fn app(vm: &FilesVm) -> AnyView {
    let trash = vm.section == FilesSection::Trash;
    let n_sel = vm.items.iter().filter(|i| i.selected).count();
    let view_attr = match vm.view {
        FilesView::List => "list",
        FilesView::Grid => "grid",
    };
    let toolbar = view! {
        <nav class="oc-files-crumbs" aria-label=t("files.path")>
            <ol>
                {vm.crumbs.iter().enumerate().map(|(i, c)| {
                    let last = i + 1 == vm.crumbs.len();
                    view! { <li><a href=c.href.clone() aria-current=last.then_some("page")>{c.label.clone()}</a></li> }
                }).collect_view()}
            </ol>
        </nav>
        <span class="oc-app__spacer"></span>
        {search(String::new(), &vm.query, "files.search")}
        <div class="oc-app-seg" role="group" aria-label=t("files.view")>
            <a class="oc-app__icon" href="?view=list" aria-current=(vm.view == FilesView::List).then_some("true") aria-label=t("files.view.list")>{icon("list")}</a>
            <a class="oc-app__icon" href="?view=grid" aria-current=(vm.view == FilesView::Grid).then_some("true") aria-label=t("files.view.grid")>{icon("grid")}</a>
        </div>
        {(vm.can_write && !trash).then(|| view! {
            <form class="oc-files-newfolder" method="post" action="/files/folder">
                <input type="hidden" name="parent" value=vm.folder_id.clone().unwrap_or_default() />
                <label class="oc-sr" for="oc-files-nf">{t("files.new_folder.name")}</label>
                <input id="oc-files-nf" name="name" placeholder=t("files.new_folder.name") required="" data-part="files-nf" hidden />
                <button type="button" class="oc-app-btn" data-oc="files-new-folder">{icon("folder")}<span>{t("files.new_folder")}</span></button>
            </form>
            <label class="oc-app-primary oc-files-upload">
                {icon("upload")}<span>{t("files.upload")}</span>
                <input type="file" multiple="" class="oc-sr" data-oc="files-upload" data-folder=vm.folder_id.clone().unwrap_or_default() />
            </label>
        })}
    }
    .into_any();
    let side = view! {
        {nav(&vm.nav)}
        {(!vm.uploads.is_empty()).then(|| view! {
            <section class="oc-files-tray" aria-labelledby="oc-files-tray-t">
                <h2 class="oc-app-side__title" id="oc-files-tray-t">{t("files.uploads")}</h2>
                <ul>{vm.uploads.iter().map(upload).collect_view()}</ul>
            </section>
        })}
    }
    .into_any();
    let body = match load_state(vm.load, 8) {
        Some(s) => s,
        None if vm.items.is_empty() => {
            if !vm.query.is_empty() {
                view! { {empty("search", "files.empty.search", None)} }.into_any()
            } else if trash {
                view! { {empty("trash", "files.empty.trash", None)} }.into_any()
            } else {
                view! { {empty("folder", "files.empty.folder", vm.can_write.then_some("files.empty.folder.hint"))} }.into_any()
            }
        }
        None => match vm.view {
            FilesView::List => view! {
                <table class="oc-files-table" data-part="files-list" aria-label=t("files.items")>
                    <thead>
                        <tr>
                            <th scope="col" class="oc-files-c-check"><label class="oc-files-check"><span class="oc-sr">{t("files.select_all")}</span><input type="checkbox" data-oc="files-check-all" /></label></th>
                            {sort_head(vm, FilesSortKey::Name, "files.col.name", "oc-files-c-name")}
                            {sort_head(vm, FilesSortKey::Kind, "files.col.kind", "oc-files-c-kind")}
                            {sort_head(vm, FilesSortKey::Modified, "files.col.modified", "oc-files-c-mod")}
                            {sort_head(vm, FilesSortKey::Size, "files.col.size", "oc-files-c-size")}
                            <th scope="col" class="oc-files-c-ctx">{t("files.col.context")}</th>
                        </tr>
                    </thead>
                    <tbody>{vm.items.iter().map(|i| row(i, vm.can_write)).collect_view()}</tbody>
                </table>
            }
            .into_any(),
            FilesView::Grid => view! {
                <ul class="oc-files-grid" data-part="files-list" aria-label=t("files.items")>
                    {vm.items.iter().map(|i| tile(i, vm.can_write)).collect_view()}
                </ul>
            }
            .into_any(),
        },
    };
    let main = view! {
        <form class="oc-files-sel" method="post" action="/files/selection" data-part="files-selbar" hidden={n_sel == 0}>
            <span class="oc-files-sel__n" role="status" data-part="files-sel-n">{tf("files.sel.n", &[("n", n_sel.to_string().as_str())])}</span>
            {(!trash).then(|| view! {
                <button type="submit" class="oc-app-btn" name="op" value="download">{icon("download")}<span>{t("files.download")}</span></button>
                {vm.can_write.then(|| view! {
                    <button type="submit" class="oc-app-btn" name="op" value="move">{icon("folder")}<span>{t("files.move")}</span></button>
                    <button type="submit" class="oc-app-btn" name="op" value="favourite">{icon("star")}<span>{t("files.fav.toggle")}</span></button>
                    <button type="submit" class="oc-app-btn" name="op" value="trash">{icon("trash")}<span>{t("files.to_trash")}</span></button>
                })}
            })}
            {trash.then(|| view! {
                <button type="submit" class="oc-app-btn" name="op" value="restore">{icon("restart")}<span>{t("files.restore")}</span></button>
                // D004.1: sem fluxo governado de eliminação definitiva no Core — a acção
                // mostra-se indisponível, sem `name`/`value`, e nunca submete.
                <button type="button" class="oc-app-btn oc-app-btn--danger" disabled="" aria-disabled="true" aria-describedby="oc-files-purge-why" data-part="files-purge-unavailable">{icon("trash")}<span>{t("files.purge")}</span></button>
                <span id="oc-files-purge-why" class="oc-files-sel__why">{t("files.purge.unavailable")}</span>
            })}
            <span class="oc-app__spacer"></span>
            <button type="button" class="oc-app-btn" data-oc="files-sel-clear">{t("files.sel.clear")}</button>
        </form>
        {trash.then(|| view! { <p class="oc-app-note oc-files-trashnote">{icon("trash")}<span>{t("files.trash.note")}</span></p> })}
        <div class="oc-files-drop" data-part="files-drop" data-folder=vm.folder_id.clone().unwrap_or_default() data-view=view_attr>
            {body}
            <p class="oc-files-drop__hint" aria-hidden="true">{icon("upload")}<span>{t("files.drop")}</span></p>
        </div>
        {more(&vm.page)}
    }
    .into_any();
    let insp = vm
        .details
        .as_ref()
        .map(|d| details(d, vm.can_write && !trash).into_any());
    let html = frame(
        "files",
        t("nav.files").to_owned(),
        toolbar,
        Some(side),
        main,
        insp,
    );
    if vm.preview_open {
        if let Some(p) = vm.details.as_ref().and_then(|d| d.preview.clone()) {
            let name = vm
                .details
                .as_ref()
                .map(|d| d.item.name.clone())
                .unwrap_or_default();
            return view! {
                <div class="oc-app" data-oc="app" data-app="files" data-preview="">
                    <div class="oc-app__bar" role="toolbar" aria-label=name.clone()>
                        <a class="oc-app__icon" href="?" aria-label=t("files.preview.back")>{icon("chev-l")}</a>
                        <h2 class="oc-app__title">{name.clone()}</h2>
                    </div>
                    <div class="oc-app__main">{preview(&p)}</div>
                </div>
            }
            .into_any();
        }
    }
    let _ = primary;
    html
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::testing::assert_contracts;
    use crate::ui::view_models::{AppLoad, AppPageVm};

    fn item(name: &str, kind: FileKind, selected: bool) -> FileItemVm {
        FileItemVm {
            id: name.into(),
            name: name.into(),
            kind,
            kind_label: "PDF".into(),
            size: Some("2,4 MB".into()),
            modified: "há 2 h".into(),
            context: None,
            href: format!("/files/{name}"),
            thumb: None,
            favourite: false,
            shared: false,
            selected,
            open: false,
        }
    }

    fn vm() -> FilesVm {
        FilesVm {
            section: FilesSection::Mine,
            nav: vec![],
            crumbs: vec![],
            view: FilesView::List,
            sort: (FilesSortKey::Name, false),
            query: String::new(),
            items: vec![
                item("a.pdf", FileKind::Pdf, true),
                item("b", FileKind::Folder, false),
            ],
            load: AppLoad::Ready,
            page: AppPageVm::default(),
            details: None,
            uploads: vec![],
            folder_id: Some("f".into()),
            can_write: true,
            preview_open: false,
        }
    }

    #[test]
    fn lista_com_ordenacao_e_seleccao_sem_armazenamento() {
        let html = app(&vm()).to_html();
        assert_contracts(&html);
        assert!(
            html.contains(r#"aria-sort="ascending""#) && html.contains(r#"aria-selected="true""#)
        );
        for leak in ["bucket", "s3://", "garage", "/etc/", "C:\\"] {
            assert!(!html.to_lowercase().contains(leak), "{leak}");
        }
    }

    #[test]
    fn sem_escrita_nao_ha_enviar_nem_mover() {
        let mut v = vm();
        v.can_write = false;
        let html = app(&v).to_html();
        assert!(!html.contains(r#"data-oc="files-upload""#) && !html.contains(r#"value="move""#));
    }

    #[test]
    fn o_progresso_so_tem_valor_quando_medido() {
        let mut v = vm();
        v.uploads = vec![
            UploadVm {
                id: "u1".into(),
                name: "x".into(),
                size: "4 GB".into(),
                state: UploadState::Checking,
            },
            UploadVm {
                id: "u2".into(),
                name: "y".into(),
                size: "4 GB".into(),
                state: UploadState::Sending {
                    text: "1 de 4 GB".into(),
                    parts: (3, 12),
                },
            },
        ];
        let html = app(&v).to_html();
        assert!(html.contains(r#"max="12" value="3""#));
        assert!(html.contains(r#"<progress aria-label="x"></progress>"#));
    }

    #[test]
    fn d004_1_eliminar_definitivamente_nunca_submete() {
        let mut v = vm();
        v.section = FilesSection::Trash;
        let html = app(&v).to_html();
        assert_contracts(&html);
        assert!(!html.contains(r#"value="purge""#));
        assert!(
            html.contains(r#"data-part="files-purge-unavailable""#)
                && html.contains(t("files.purge.unavailable"))
        );
    }
}
