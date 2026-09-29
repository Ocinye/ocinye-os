//! D004 · Notas. Uma aplicação de escrita: lista à esquerda, folha branca ao centro.
//!
//! Modelo do editor: texto estruturado (Markdown restrito) num `textarea`; a
//! barra insere sintaxe (título, negrito, itálico, lista, ligação, código,
//! citação). Nunca HTML guardado ou interpretado. Gravação explícita com
//! `base_revision` (o Core devolve conflito se outra revisão entrou); sem
//! gravação automática, porque o Core não a oferece. Alterações por guardar:
//! o formulário reporta `data-dirty`; o fecho é o diálogo do gestor (D002).

use leptos::prelude::*;

use super::{doc_form_id, empty, error, frame, load_state, more, nav, nye, save_state, search};
use crate::i18n::{t, tf};
use crate::ui::components::icon;
use crate::ui::view_models::{AppSaveState, NoteEditorVm, NotesVm};

fn tool(md: &'static str, ic: &'static str, key: &'static str) -> impl IntoView {
    view! { <button type="button" class="oc-app__icon" data-oc="md" data-md=md aria-label=t(key) title=t(key)>{icon(ic)}</button> }
}

fn editor(e: &NoteEditorVm) -> impl IntoView {
    let form_id = doc_form_id("notes", &e.id);
    let conflict = matches!(
        e.save,
        AppSaveState::Failed(crate::ui::view_models::AppError::Conflict)
    );
    view! {
        <form class="oc-notes-doc" id=form_id.clone() method="post" action=e.save_action.clone() data-oc="app-doc" data-state=if matches!(e.save, AppSaveState::Dirty) { "dirty" } else { "clean" }>
            <input type="hidden" name="base_revision" value=e.revision.to_string() />
            <div class="oc-notes-doc__bar" role="toolbar" aria-label=t("notes.format") aria-controls="oc-notes-body">
                {(!e.read_only).then(|| view! {
                    {tool("h", "type2", "notes.fmt.heading")}
                    {tool("b", "bold", "notes.fmt.bold")}
                    {tool("i", "italic", "notes.fmt.italic")}
                    {tool("ul", "list", "notes.fmt.list")}
                    {tool("a", "link", "notes.fmt.link")}
                    {tool("code", "code", "notes.fmt.code")}
                    {tool("q", "quote", "notes.fmt.quote")}
                })}
                <span class="oc-app__spacer"></span>
                {save_state(&e.save)}
                {e.nye.as_ref().map(nye)}
                {(!e.read_only).then(|| view! {
                    <button type="submit" class="oc-app-primary" data-oc="app-save" disabled=matches!(e.save, AppSaveState::Saving)>{icon("check")}<span>{t("notes.save")}</span></button>
                })}
                <details class="oc-app-menu">
                    <summary class="oc-app__icon" aria-label=t("app.more_actions")>{icon("more")}</summary>
                    <div class="oc-app-menu__list" role="menu">
                        {e.revisions_href.clone().map(|h| view! { <a role="menuitem" href=h>{icon("history")}{t("notes.revisions")}</a> })}
                        {e.trash_action.clone().map(|a| view! { <button role="menuitem" type="submit" formaction=a formnovalidate="">{icon("trash")}{t("notes.to_trash")}</button> })}
                    </div>
                </details>
            </div>
            {conflict.then(|| view! {
                <p class="oc-app-note" data-tone="warn" role="alert">{icon("warning")}<span>{t("notes.conflict")}</span></p>
            })}
            {e.read_only.then(|| view! { <p class="oc-app-note">{icon("lock")}<span>{t("notes.read_only")}</span></p> })}
            <div class="oc-notes-sheet">
                <label class="oc-sr" for="oc-notes-title">{t("notes.title")}</label>
                <input id="oc-notes-title" class="oc-notes-title" name="title" value=e.title.clone() placeholder=t("notes.untitled") readonly=e.read_only autocomplete="off" />
                <label class="oc-sr" for="oc-notes-body">{t("notes.body")}</label>
                <textarea id="oc-notes-body" class="oc-notes-body" name="body" readonly=e.read_only placeholder=t("notes.body.placeholder") data-part="notes-body" spellcheck="true">{crate::text::rcdata(&e.body)}</textarea>
            </div>
            {e.stats.clone().map(|s| view! { <p class="oc-notes-stats">{s}</p> })}
        </form>
    }
}

/// A aplicação Notas.
pub fn app(vm: &NotesVm) -> AnyView {
    let has_doc = vm.editor.is_some() || vm.editor_error.is_some();
    let toolbar = view! {
        <h2 class="oc-app__title">{t("nav.notes")}</h2>
        <span class="oc-app__spacer"></span>
        {vm.create_action.clone().map(|a| view! {
            <form method="post" action=a><button type="submit" class="oc-app-primary">{icon("plus")}<span>{t("notes.new")}</span></button></form>
        })}
    }
    .into_any();
    let list = match load_state(vm.load, 7) {
        Some(s) => s,
        None if vm.items.is_empty() && !vm.query.is_empty() => empty("search", "notes.empty.search", None).into_any(),
        None if vm.items.is_empty() => empty("notes", "notes.empty", vm.create_action.is_some().then_some("notes.empty.hint")).into_any(),
        None => view! {
            <ul class="oc-notes-list" data-part="app-list" aria-label=t("notes.list")>
                {vm.items.iter().map(|n| view! {
                    <li>
                        <a class="oc-notes-item" href=n.href.clone() aria-current=n.active.then_some("page")>
                            <span class="oc-notes-item__title">{n.title.clone()}</span>
                            <span class="oc-notes-item__meta">
                                {[Some(n.modified.clone()), n.context.clone()].into_iter().flatten().collect::<Vec<_>>().join(" · ")}
                                {n.shared.then(|| view! { " · "{icon("share")}<span class="oc-sr">{t("notes.shared")}</span> })}
                            </span>
                        </a>
                    </li>
                }).collect_view()}
            </ul>
        }
        .into_any(),
    };
    let side = view! {
        {nav(&vm.nav)}
        {search(String::from("/notes"), &vm.query, "notes.search")}
        {list}
        {more(&vm.page)}
    }
    .into_any();
    let main = match (&vm.editor, vm.editor_error) {
        (_, Some(e)) => error(e).into_any(),
        (Some(ed), None) => editor(ed).into_any(),
        (None, None) => empty("edit", "notes.none_open", None).into_any(),
    };
    let html = frame(
        "notes",
        t("nav.notes").to_owned(),
        toolbar,
        Some(side),
        main,
        None,
    );
    if has_doc {
        // A mesma marcação; o CSS mostra a folha no móvel e a lista fica na gaveta.
        return view! { <div class="oc-app-wrap" data-doc="">{html}</div> }.into_any();
    }
    let _ = tf;
    html
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::testing::assert_contracts;
    use crate::ui::view_models::{AppError, AppLoad, AppPageVm};

    fn ed(save: AppSaveState) -> NoteEditorVm {
        NoteEditorVm {
            id: "n1".into(),
            title: "Notas de campo".into(),
            body: "# Torre 2\n<script>x</script>".into(),
            revision: 4,
            save_action: "/notes/n1/gravar".into(),
            save,
            read_only: false,
            trash_action: Some("/notes/n1/apagar".into()),
            revisions_href: None,
            nye: None,
            stats: None,
        }
    }

    fn vm(e: Option<NoteEditorVm>) -> NotesVm {
        NotesVm {
            nav: vec![],
            query: String::new(),
            items: vec![],
            load: AppLoad::Ready,
            page: AppPageVm::default(),
            editor: e,
            editor_error: None,
            create_action: Some("/notes/new".into()),
        }
    }

    #[test]
    fn o_editor_e_texto_e_nunca_html() {
        let html = app(&vm(Some(ed(AppSaveState::Dirty)))).to_html();
        assert_contracts(&html);
        assert!(html.contains("&lt;script&gt;") && !html.contains("<script>x"));
        assert!(
            html.contains(r#"name="base_revision" value="4""#)
                && html.contains(r#"data-state="dirty""#)
        );
        assert!(html.contains(r#"id="oc-notes-doc-n1""#));
    }

    #[test]
    fn conflito_e_revogada_sao_estados_tipados() {
        let html = app(&vm(Some(ed(AppSaveState::Failed(AppError::Conflict))))).to_html();
        assert!(html.contains(t("notes.conflict")));
        let mut v = vm(None);
        v.editor_error = Some(AppError::Revoked);
        assert!(app(&v)
            .to_html()
            .contains(r#"data-error="app.err.revoked""#));
    }
}
