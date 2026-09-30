//! D006 · Unidades (`/units`, `units.view`). DESIGN_LOCKED.
//!
//! Unidades são estrutura institucional: não são projectos, nem equipas, nem
//! um espaço de trabalho genérico. O modelo é plano (sem unidade-mãe). O
//! código é identidade institucional e não muda. Arquivar é história, nunca
//! apagar. Gerir membros exige `may_manage_members` do Core, e o Core volta a
//! decidir em cada operação — incluindo a de não deixar a unidade sem gestor.

use leptos::prelude::*;

use super::org::{action, avatar, notice, refusal, unit_role_tag, unit_status_tag};
use super::res::{detail_or, input, keywords, list, prose, select, textarea, two_pane};
use super::{empty, error, frame, load_state, nav, nye, primary};
use crate::i18n::{t, tf};
use crate::ui::components::icon;
use crate::ui::view_models::{
    UnitAddMemberVm, UnitFormVm, UnitMemberVm, UnitVm, UnitsSection, UnitsVm,
};

fn member_row(m: &UnitMemberVm) -> impl IntoView {
    let name = view! {
        {avatar(&m.avatar)}
        <span class="oc-org-person__text">
            <span class="oc-res-name__title">{m.name.clone()}</span>
            <span class="oc-res-name__meta">{unit_role_tag(m.role)}</span>
        </span>
    };
    view! {
        <li class="oc-org-list__row oc-org-umember" data-part="unit-member">
            {match m.href.clone() {
                Some(h) => view! { <a class="oc-org-person" href=h>{name}</a> }.into_any(),
                None => view! { <span class="oc-org-person" data-static="">{name}</span> }.into_any(),
            }}
            <span class="oc-org-umember__acts">
                {m.change_role.as_ref().map(action)}
                {m.remove.as_ref().map(action)}
            </span>
        </li>
    }
}

fn add_member(a: &UnitAddMemberVm) -> impl IntoView {
    if a.unavailable {
        return view! {
            <div class="oc-org-add" data-part="unit-add" data-unavailable="">
                <p class="oc-res-sub">{t("units.add")}</p>
                <p class="oc-app-note">{icon("link")}<span>{t("units.add.unavailable")}</span></p>
            </div>
        }
        .into_any();
    }
    view! {
        <div class="oc-org-add" data-part="unit-add">
            <p class="oc-res-sub" id="oc-units-add-t">{t("units.add")}</p>
            <form class="oc-org-add__search" method="get" action=a.search_action.clone() role="search" aria-labelledby="oc-units-add-t">
                <label class="oc-app-field">
                    <span>{t("units.add.search")}</span>
                    <input type="search" name="candidate_q" value=a.query.clone() autocomplete="off" data-part="unit-add-q" />
                </label>
                <button type="submit" class="oc-app-btn">{icon("search")}<span>{t("units.add.find")}</span></button>
            </form>
            {a.candidates.as_ref().map(|c| if c.is_empty() {
                view! { <p class="oc-res-sec__empty" role="status">{t("units.add.none")}</p> }.into_any()
            } else {
                view! {
                    <form class="oc-org-add__pick" method="post" action=a.add_action.clone()>
                        <fieldset class="oc-org-cands">
                            <legend class="oc-res-form__hint">{tf("units.add.results", &[("n", c.len().to_string().as_str())])}</legend>
                            {c.iter().map(|o| view! {
                                <label class="oc-org-cand"><input type="radio" name="person_id" value=o.value.clone() required="" checked=o.selected /><span>{o.label.clone()}</span></label>
                            }).collect_view()}
                        </fieldset>
                        <div class="oc-res-assign">
                            {select("role", "units.add.role", &a.roles, true)}
                            <button type="submit" class="oc-app-btn">{icon("plus")}<span>{t("units.add.do")}</span></button>
                        </div>
                    </form>
                }.into_any()
            })}
            <p class="oc-res-form__hint">{t("units.add.hint")}</p>
        </div>
    }
    .into_any()
}

fn detail(u: &UnitVm, back: String) -> impl IntoView {
    view! {
        <article class="oc-res-doc" data-part="unit">
            <header class="oc-res-head">
                <a class="oc-app__icon oc-res-back" href=back aria-label=t("res.back")>{icon("chev-l")}</a>
                <div class="oc-res-head__text">
                    <p class="oc-res-head__code">{u.code.clone()}</p>
                    <h2 class="oc-res-head__title" id="oc-res-title">{u.name.clone()}</h2>
                    <p class="oc-res-head__tags">{unit_status_tag(u.status)}</p>
                </div>
            </header>
            {u.notice.map(notice)}
            {u.refusal.map(refusal)}
            <div class="oc-res-actions">
                {u.edit_href.clone().map(|h| view! { <a class="oc-app-btn" href=h>{icon("edit")}<span>{t("units.edit")}</span></a> })}
                {u.archive.as_ref().map(action)}
                {u.nye.as_ref().map(nye)}
            </div>
            {keywords(&u.areas)}
            {prose("units.description", u.description.as_ref())}
            <section class="oc-res-sec" data-part="unit-members">
                <h3 class="oc-res-sec__title">{t("units.members")}</h3>
                {match load_state(u.members_load, 4) {
                    Some(s) => s,
                    None if u.members.is_empty() => view! { <p class="oc-res-sec__empty">{t("units.members.none")}</p> }.into_any(),
                    None => view! { <ul class="oc-org-list">{u.members.iter().map(member_row).collect_view()}</ul> }.into_any(),
                }}
                {u.add.as_ref().map(add_member)}
                {(!u.may_manage).then(|| view! { <p class="oc-res-form__hint">{t("units.members.readonly")}</p> })}
                <p class="oc-res-form__hint">{t("units.members.scope")}</p>
            </section>
        </article>
    }
}

fn form(f: &UnitFormVm, back: String) -> impl IntoView {
    let id = if f.is_new {
        "oc-units-doc-new".to_owned()
    } else {
        "oc-units-doc-edit".to_owned()
    };
    view! {
        <form class="oc-res-form" id=id method="post" action=f.action.clone() data-oc="app-doc" data-state="clean" aria-labelledby="oc-res-title">
            <header class="oc-res-head">
                <a class="oc-app__icon oc-res-back" href=back.clone() aria-label=t("res.back")>{icon("chev-l")}</a>
                <div class="oc-res-head__text"><h2 class="oc-res-head__title" id="oc-res-title">{t(if f.is_new { "units.new" } else { "units.edit" })}</h2></div>
            </header>
            {f.error.map(error)}
            {f.code.clone().map(|c| view! {
                <div class="oc-app-field"><span>{t("units.code")}</span><p class="oc-org-readonly"><code>{c}</code><span>{t("units.code.immutable")}</span></p></div>
            })}
            {input("name", "units.name", &f.name, "text", true)}
            {f.is_new.then(|| view! {
                <p class="oc-org-suggest" data-part="unit-code-suggestion" aria-live="polite">
                    <span>{t("units.code.suggested")}</span>
                    <code>{f.code_suggestion.clone().unwrap_or_else(|| "—".into())}</code>
                    <span class="oc-res-form__hint">{t("units.code.indicative")}</span>
                </p>
            })}
            {textarea("description", "units.description", &f.description, 4)}
            {input("research_areas", "units.areas.input", &f.areas, "text", false)}
            {f.is_new.then(|| view! { <p class="oc-res-form__hint">{t("units.new.hint")}</p> })}
            <div class="oc-res-form__foot">
                <a class="oc-app-btn" href=back>{t("app.cancel")}</a>
                <span class="oc-app__spacer"></span>
                <button type="submit" class="oc-app-primary" data-oc="app-save">{icon("check")}<span>{t(if f.is_new { "units.create" } else { "units.save" })}</span></button>
            </div>
        </form>
    }
}

/// A aplicação Unidades.
pub fn app(vm: &UnitsVm) -> AnyView {
    let toolbar = view! {
        <h2 class="oc-app__title">{t("nav.units")}</h2>
        <span class="oc-app__spacer"></span>
        {vm.new_href.clone().map(|h| primary(h, "plus", "units.new"))}
    }
    .into_any();
    let empty_view = if vm.section == UnitsSection::Archived {
        empty("archive", "units.empty.archived", None).into_any()
    } else {
        empty("units", "units.empty", Some("units.empty.body")).into_any()
    };
    let list_view = list(&vm.list, "units.list", empty_view);
    let detail_view = match (&vm.form, &vm.unit) {
        (Some(f), _) => form(f, vm.list_href.clone()).into_any(),
        (None, Some(u)) => detail(u, vm.list_href.clone()).into_any(),
        (None, None) => detail_or(
            vm.unit_error,
            empty("units", "units.none_open", None).into_any(),
        ),
    };
    frame(
        "units",
        t("nav.units").to_owned(),
        toolbar,
        Some(nav(&vm.nav).into_any()),
        two_pane(vm.pane, list_view, detail_view),
        None,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::testing::assert_contracts;
    use crate::ui::view_models::{
        AppError, AppLoad, AppPageVm, OrgActionKind, OrgActionVm, OrgAvatarVm, OrgRefusal,
        OrgUnitRole, OrgUnitStatus, ResListVm, ResPane,
    };

    fn vm() -> UnitsVm {
        UnitsVm {
            section: UnitsSection::Active,
            nav: vec![],
            list: ResListVm {
                columns: vec![],
                items: vec![],
                load: AppLoad::Ready,
                page: AppPageVm::default(),
            },
            pane: ResPane::List,
            unit: None,
            unit_error: None,
            form: None,
            new_href: None,
            list_href: "/units".into(),
        }
    }

    fn unit(may: bool) -> UnitVm {
        UnitVm {
            code: "UENR-001".into(),
            name: "<b>Energia</b>".into(),
            status: OrgUnitStatus::Active,
            description: None,
            areas: vec![],
            members: vec![UnitMemberVm {
                name: "Ana".into(),
                avatar: OrgAvatarVm {
                    initials: "AS".into(),
                    image_href: None,
                },
                role: OrgUnitRole::Manager,
                href: None,
                change_role: may.then(|| OrgActionVm {
                    kind: OrgActionKind::ChangeUnitRole,
                    href: "?confirm=unit_role".into(),
                }),
                remove: may.then(|| OrgActionVm {
                    kind: OrgActionKind::RemoveUnitMember,
                    href: "?confirm=unit_remove".into(),
                }),
            }],
            members_load: AppLoad::Ready,
            may_manage: may,
            edit_href: None,
            archive: None,
            add: None,
            nye: None,
            refusal: None,
            notice: None,
        }
    }

    #[test]
    fn sem_unidades_e_sem_criar_nao_oferece_criar() {
        let html = app(&vm()).to_html();
        assert_contracts(&html);
        assert!(!html.contains("oc-app-primary") && html.contains(t("units.empty")));
    }

    #[test]
    fn so_leitura_nao_tem_accoes_de_membro_e_um_membro_sem_admin_nao_e_elo() {
        let mut v = vm();
        v.pane = ResPane::Detail;
        v.unit = Some(unit(false));
        let html = app(&v).to_html();
        assert_contracts(&html);
        assert!(html.contains("&lt;b&gt;Energia") && !html.contains(r#"data-part="org-action""#));
        assert!(html.contains(r#"data-static"#) && html.contains(t("units.members.readonly")));
    }

    #[test]
    fn a_ultima_gestora_mostra_a_recusa_do_core() {
        let mut v = vm();
        v.pane = ResPane::Detail;
        let mut u = unit(true);
        u.refusal = Some(OrgRefusal::LastUnitManager);
        v.unit = Some(u);
        let html = app(&v).to_html();
        assert!(html.contains(r#"data-refusal="org.refusal.last_manager""#));
    }

    #[test]
    fn editar_mostra_o_codigo_so_de_leitura() {
        let mut v = vm();
        v.pane = ResPane::Detail;
        v.form = Some(UnitFormVm {
            action: "/units/u/edit".into(),
            is_new: false,
            code: Some("UENR-001".into()),
            code_suggestion: None,
            name: "Energia".into(),
            description: String::new(),
            areas: String::new(),
            error: None,
        });
        let html = app(&v).to_html();
        assert!(!html.contains(r#"name="code""#) && html.contains("UENR-001"));
    }

    #[test]
    fn unidade_revogada_nao_mostra_conteudo() {
        let mut v = vm();
        v.pane = ResPane::Detail;
        v.unit_error = Some(AppError::NotFound);
        assert!(app(&v)
            .to_html()
            .contains(r#"data-error="app.err.not_found""#));
    }
}
