//! D005 · Dados. DESIGN_LOCKED.
//!
//! Um dataset é um recurso lógico — código, origem, licença, restrições de uso,
//! responsável, classificação — com **versões** imutáveis depois de
//! publicadas, cada uma com a sua proveniência e os seus ficheiros por caminho
//! lógico. Não é Ficheiros, e a vista não navega pastas nem mostra chaves de
//! objecto. Não há pré-visualização de conteúdo: o browser nunca carrega o
//! dataset.

use leptos::prelude::*;

use super::res::{
    dataset_state, detail_head, detail_or, input, keywords, kv, links_section, list, prose, select,
    state_tag, textarea, two_pane, version_state,
};
use super::{doc_form_id, empty, error, frame, nye};
use crate::i18n::t;
use crate::ui::components::icon;
use crate::ui::view_models::{DatasetFormVm, DatasetVersionVm, DatasetVm, DatasetsVm};

fn version(v: &DatasetVersionVm) -> impl IntoView {
    view! {
        <section class="oc-res-sec oc-res-version" data-part="dataset-version">
            <header class="oc-res-version__head">
                <h3 class="oc-res-sec__title">{t("data.version")}{" "}{v.label.clone()}</h3>
                {state_tag(version_state(v.status))}
                {v.published.clone().map(|p| view! { <span class="oc-res-version__at">{p}</span> })}
                {v.totals.clone().map(|s| view! { <span class="oc-res-version__at">{s}</span> })}
            </header>
            {v.withdrawn_reason.clone().map(|r| view! { <p class="oc-app-note" data-tone="warn">{icon("warning")}<span><strong>{t("data.version.withdrawn_reason")}</strong>{" "}{r}</span></p> })}
            <dl class="oc-res-meta">
                {kv("data.version.derived_from", v.derived_from.clone())}
            </dl>
            {prose("data.provenance", v.provenance.as_ref())}
            {prose("data.version.notes", v.notes.as_ref())}
            <h4 class="oc-res-sub">{t("data.files")}</h4>
            {if v.files.is_empty() {
                view! { <p class="oc-res-sec__empty">{t("data.files.empty")}</p> }.into_any()
            } else {
                view! {
                    <ul class="oc-res-files">
                        {v.files.iter().map(|f| {
                            let inner = view! { {icon("files")}<span class="oc-res-files__path">{f.path.clone()}</span><span class="oc-res-files__size">{f.size.clone()}</span> };
                            match f.href.clone() {
                                Some(h) => view! { <li><a class="oc-res-files__row" href=h>{inner}</a></li> }.into_any(),
                                None => view! { <li><span class="oc-res-files__row">{inner}</span></li> }.into_any(),
                            }
                        }).collect_view()}
                    </ul>
                }.into_any()
            }}
            <p class="oc-res-sec__empty">{icon("eye")}{" "}{t("data.preview.none")}</p>
            <div class="oc-res-trans__row">
                {v.add_file_action.clone().map(|a| view! {
                    <form method="post" action=a enctype="multipart/form-data" class="oc-res-upload">
                        <label class="oc-app-btn oc-files-upload">{icon("upload")}<span>{t("data.files.add")}</span><input type="file" name="file" class="oc-sr" required="" /></label>
                        <input type="text" name="path" class="oc-res-upload__path" placeholder=t("data.files.path") aria-label=t("data.files.path") required="" />
                        <button type="submit" class="oc-app-btn">{t("data.files.add.do")}</button>
                    </form>
                })}
                {v.publish_action.clone().map(|a| view! {
                    <form method="post" action=a><button type="submit" class="oc-app-primary">{icon("check")}<span>{t("data.version.publish")}</span></button></form>
                })}
            </div>
        </section>
    }
}

fn detail(d: &DatasetVm, back: String) -> impl IntoView {
    let open = d.versions.iter().find(|v| v.open);
    view! {
        <article class="oc-res-doc" data-part="dataset">
            {detail_head(back, Some(d.code.clone()), d.title.clone(), Some(dataset_state(d.state)), Some(d.classification))}
            <div class="oc-res-actions">{d.nye.as_ref().map(nye)}</div>
            <dl class="oc-res-meta">
                {kv("data.origin", Some(d.origin.clone()))}
                {kv("data.licence", d.licence.clone())}
                {kv("data.responsible", d.responsible.clone())}
                {kv("data.acquired", d.acquired.clone())}
            </dl>
            {d.usage_restrictions.clone().map(|u| view! { <p class="oc-app-note" data-tone="warn">{icon("lock")}<span><strong>{t("data.restrictions")}</strong>{" "}{u}</span></p> })}
            {keywords(&d.keywords)}
            {prose("data.description", d.description.as_ref())}
            {d.workspace.as_ref().map(|w| links_section("res.workspace", std::slice::from_ref(w), None, None))}
            <section class="oc-res-sec">
                <h3 class="oc-res-sec__title">{t("data.versions")}</h3>
                {if d.versions.is_empty() {
                    view! { <p class="oc-res-sec__empty">{t("data.versions.empty")}</p> }.into_any()
                } else {
                    view! {
                        <nav class="oc-res-vtabs" aria-label=t("data.versions")>
                            {d.versions.iter().map(|v| view! {
                                <a class="oc-res-vtabs__tab" href=v.href.clone() aria-current=v.open.then_some("true")>{v.label.clone()}<span class="oc-sr">{" · "}{t(version_state(v.status).key)}</span></a>
                            }).collect_view()}
                        </nav>
                    }.into_any()
                }}
                {d.new_version_action.clone().map(|a| view! {
                    <details class="oc-res-newver">
                        <summary class="oc-app-btn">{icon("plus")}<span>{t("data.version.new")}</span></summary>
                        <form method="post" action=a class="oc-res-form">
                            {input("label", "data.version.label", "", "text", true)}
                            {textarea("provenance", "data.provenance", "", 3)}
                            {textarea("notes", "data.version.notes", "", 2)}
                            <div class="oc-res-form__foot"><span class="oc-app__spacer"></span><button type="submit" class="oc-app-primary">{t("data.version.create")}</button></div>
                        </form>
                    </details>
                })}
            </section>
            {open.map(version)}
            {links_section("res.links", &d.links, None, None)}
        </article>
    }
}

fn form(f: &DatasetFormVm) -> impl IntoView {
    let id = doc_form_id("datasets", "new");
    view! {
        <form class="oc-res-form" id=id method="post" action=f.action.clone() data-oc="app-doc" data-state="clean" aria-labelledby="oc-res-title">
            <header class="oc-res-head">
                <a class="oc-app__icon oc-res-back" href=f.cancel_href.clone() aria-label=t("res.back")>{icon("chev-l")}</a>
                <div class="oc-res-head__text"><h2 class="oc-res-head__title" id="oc-res-title">{t("data.new")}</h2></div>
            </header>
            {f.error.map(error)}
            {select("workspace", "res.workspace", &f.workspaces, true)}
            <div class="oc-app-row2">
                {input("code", "data.code", &f.code, "text", true)}
                {select("origin", "data.origin", &f.origins, true)}
            </div>
            {input("title", "data.title", &f.title, "text", true)}
            {textarea("description", "data.description", &f.description, 3)}
            {input("licence", "data.licence", &f.licence, "text", false)}
            {textarea("usage_restrictions", "data.restrictions", &f.usage_restrictions, 2)}
            {input("keywords", "res.keywords.input", &f.keywords, "text", false)}
            <p class="oc-res-form__hint">{t("data.new.hint")}</p>
            <div class="oc-res-form__foot">
                <a class="oc-app-btn" href=f.cancel_href.clone()>{t("app.cancel")}</a>
                <span class="oc-app__spacer"></span>
                <button type="submit" class="oc-app-primary" data-oc="app-save">{icon("check")}<span>{t("data.create")}</span></button>
            </div>
        </form>
    }
}

/// A aplicação Dados.
pub fn app(vm: &DatasetsVm) -> AnyView {
    let toolbar = view! {
        <h2 class="oc-app__title">{t("nav.data")}</h2>
        <span class="oc-app__spacer"></span>
        {(!vm.workspace_filter.is_empty()).then(|| view! {
            <form class="oc-res-filter" method="get" role="search">
                <label><span class="oc-sr">{t("res.filter.workspace")}</span>
                    <select name="workspace">
                        {vm.workspace_filter.iter().map(|o| view! { <option value=o.value.clone() selected=o.selected>{o.label.clone()}</option> }).collect_view()}
                    </select>
                </label>
                <button type="submit" class="oc-app-btn">{t("res.filter.apply")}</button>
            </form>
        })}
        {vm.new_href.clone().map(|h| view! { <a class="oc-app-primary" href=h aria-label=t("data.new")>{icon("plus")}<span>{t("data.new")}</span></a> })}
    }
    .into_any();
    let list_view = list(
        &vm.list,
        "data.list",
        empty("data", "data.empty", Some("data.empty.body")).into_any(),
    );
    let detail_view = match (&vm.form, &vm.dataset) {
        (Some(f), _) => form(f).into_any(),
        (None, Some(d)) => detail(d, vm.list_href.clone()).into_any(),
        (None, None) => detail_or(
            vm.dataset_error,
            empty("data", "data.none_open", None).into_any(),
        ),
    };
    frame(
        "datasets",
        t("nav.data").to_owned(),
        toolbar,
        None,
        two_pane(vm.pane, list_view, detail_view),
        None,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::testing::assert_contracts;
    use crate::ui::view_models::{DatasetFileVm, DatasetVersionStatus};

    #[test]
    fn a_versao_mostra_caminhos_logicos_e_nunca_armazenamento() {
        let v = DatasetVersionVm {
            label: "v2".into(),
            status: DatasetVersionStatus::Published,
            published: Some("12 set 2026".into()),
            notes: None,
            provenance: Some("Derivado de v1, filtrado.".into()),
            derived_from: Some("v1".into()),
            withdrawn_reason: None,
            totals: Some("2 ficheiros · 1,1 GB".into()),
            files: vec![DatasetFileVm {
                path: "medicoes/2026-09.csv".into(),
                size: Some("820 MB".into()),
                href: None,
            }],
            add_file_action: None,
            publish_action: None,
            open: true,
            href: "/datasets/d?v=2".into(),
        };
        let html = view! { {version(&v)} }.to_html();
        assert_contracts(&html);
        assert!(html.contains("medicoes/2026-09.csv") && html.contains(t("data.preview.none")));
        for leak in ["s3://", "garage", "bucket", "object_key"] {
            assert!(!html.to_lowercase().contains(leak));
        }
        assert!(!html.contains(t("data.version.publish")));
    }
}
