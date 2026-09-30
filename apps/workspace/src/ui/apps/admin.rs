//! D006 · Administração (`/admin`, `members.manage`). DESIGN_LOCKED.
//!
//! A superfície administrativa governada da Instância: Membros (o roster, o
//! membro, criar), Papéis (o catálogo de sistema, só leitura) e Instância
//! (informação e aplicações). Não é uma consola de infraestrutura, nem um
//! editor de base de dados, nem um atalho à política.
//!
//! «Membros» vive aqui porque é aqui que o registo o põe (`administration`,
//! palavras-chave «membros»); o directório partilhado `/people` não tem
//! aplicação própria (APP_REGISTRY_CHANGE_REQUIRED, adiado).

use leptos::prelude::*;

use super::org::{
    action, avatar, credential_once, notice, refusal, role_tag, status_tag, unit_role_tag,
};
use super::res::{detail_or, select, two_pane};
use super::{empty, error, frame, load_state, more, nav, primary};
use crate::i18n::{t, tf};
use crate::ui::components::icon;
use crate::ui::view_models::{
    AdminSection, AdminVm, OrgAccessVm, OrgInstanceVm, OrgMemberRowVm, OrgMemberVm,
    OrgMembersListVm, OrgNewMemberVm, OrgRoleDefVm, OrgSecurityVm,
};

fn row(r: &OrgMemberRowVm) -> impl IntoView {
    view! {
        <tr class="oc-res-row" data-part="res-item" data-open=r.active.then_some("")>
            <td class="oc-res-c-title">
                <a class="oc-res-name oc-org-person" href=r.href.clone() aria-current=r.active.then_some("true") data-part="res-open">
                    {avatar(&r.avatar)}
                    <span class="oc-org-person__text">
                        <span class="oc-res-name__title">{r.name.clone()}{r.is_self.then(|| view! { <span class="oc-org-self">{t("org.self")}</span> })}</span>
                        <span class="oc-res-name__meta">
                            {status_tag(r.status)}
                            {r.email.clone().map(|e| view! { <span class="oc-org-person__email">{e}</span> })}
                        </span>
                    </span>
                </a>
            </td>
            <td class="oc-res-c" data-prio="1">{r.position.map_or_else(|| "—".to_owned(), |p| t(p.key()).to_owned())}</td>
            <td class="oc-res-c" data-prio="2">{if r.units.is_empty() { "—".to_owned() } else { r.units.join(" · ") }}</td>
            <td class="oc-res-c" data-prio="3">{r.joined.clone().unwrap_or_else(|| "—".into())}</td>
            <td class="oc-res-c" data-prio="4">{r.last_seen.clone().unwrap_or_else(|| "—".into())}</td>
        </tr>
    }
}

/// O roster. A primeira coluna (nome, estado, endereço) nunca cede; as outras
/// saem por prioridade: Última actividade, Registo, Unidades, Posição.
pub fn roster(l: &OrgMembersListVm) -> AnyView {
    if let Some(s) = load_state(l.load, 8) {
        return s;
    }
    if l.rows.is_empty() {
        return empty(
            "user",
            "admin.members.empty",
            Some("admin.members.empty.body"),
        )
        .into_any();
    }
    view! {
        <table class="oc-res-table oc-org-table" aria-label=t("admin.members.list") data-oc="res-list">
            <thead>
                <tr>
                    <th scope="col" class="oc-res-c-title">{t("org.col.member")}</th>
                    <th scope="col" class="oc-res-c" data-prio="1">{t("org.col.position")}</th>
                    <th scope="col" class="oc-res-c" data-prio="2">{t("org.col.units")}</th>
                    <th scope="col" class="oc-res-c" data-prio="3">{t("org.col.joined")}</th>
                    <th scope="col" class="oc-res-c" data-prio="4">{t("org.col.last_seen")}</th>
                </tr>
            </thead>
            <tbody>{l.rows.iter().map(row).collect_view()}</tbody>
        </table>
        {more(&l.page)}
    }
    .into_any()
}

fn access(a: &OrgAccessVm) -> impl IntoView {
    view! {
        <section class="oc-res-sec" data-part="member-roles">
            <h3 class="oc-res-sec__title">{t("org.roles")}</h3>
            {if a.roles.is_empty() {
                view! { <p class="oc-res-sec__empty">{t("org.roles.none")}</p> }.into_any()
            } else {
                view! {
                    <ul class="oc-org-list">
                        {a.roles.iter().map(|r| view! {
                            <li class="oc-org-list__row">{role_tag(r.role)}<span class="oc-app__spacer"></span>{r.revoke.as_ref().map(action)}</li>
                        }).collect_view()}
                    </ul>
                }.into_any()
            }}
            {a.grant.as_ref().map(|g| view! {
                <form class="oc-res-assign" method="get" action=g.action.clone() data-part="role-grant">
                    <input type="hidden" name="confirm" value="grant_role" />
                    {select("role", "org.roles.grant", &g.options, true)}
                    <button type="submit" class="oc-app-btn">{icon("plus")}<span>{t("org.act.grant_role.label")}</span></button>
                </form>
            })}
            <p class="oc-res-form__hint">{t("org.roles.not_all")}</p>
        </section>
        {(!a.grants.is_empty()).then(|| view! {
            <section class="oc-res-sec" data-part="member-grants">
                <h3 class="oc-res-sec__title">{t("org.grants")}</h3>
                <ul class="oc-org-list">
                    {a.grants.iter().map(|g| view! {
                        <li class="oc-org-list__row oc-org-grant">
                            <span class="oc-org-grant__main">
                                <code class="oc-org-perm">{g.permission.clone()}</code>
                                <span class="oc-org-grant__meta">
                                    <span>{g.scope.clone()}</span>
                                    {g.granted_by.clone().map(|b| view! { <span>{tf("org.grants.by", &[("name", b.as_str())])}</span> })}
                                    <span>{g.expires.clone().map_or_else(|| t("org.grants.no_expiry").to_owned(), |x| tf("org.grants.expires", &[("at", x.as_str())]))}</span>
                                </span>
                                <span class="oc-org-grant__reason">{g.reason.clone()}</span>
                            </span>
                            {g.revoke.as_ref().map(action)}
                        </li>
                    }).collect_view()}
                </ul>
            </section>
        })}
        {(!a.permissions.is_empty()).then(|| view! {
            <details class="oc-org-perms" data-part="member-permissions">
                <summary class="oc-app-btn">{icon("list")}<span>{tf("org.perms.show", &[("n", a.permissions.len().to_string().as_str())])}</span></summary>
                <table class="oc-org-perms__table">
                    <caption class="oc-res-form__hint">{t("org.perms.caption")}</caption>
                    <thead><tr><th scope="col">{t("org.perms.permission")}</th><th scope="col">{t("org.perms.source")}</th></tr></thead>
                    <tbody>
                        {a.permissions.iter().map(|p| view! { <tr><td><code class="oc-org-perm">{p.permission.clone()}</code></td><td>{t(p.source.key())}</td></tr> }).collect_view()}
                    </tbody>
                </table>
            </details>
        })}
    }
}

fn security(s: &OrgSecurityVm) -> impl IntoView {
    let yes_no = |b: bool| t(if b { "org.yes" } else { "org.no" });
    view! {
        <section class="oc-res-sec" data-part="member-security">
            <h3 class="oc-res-sec__title">{t("org.security")}</h3>
            <dl class="oc-res-meta">
                <div class="oc-app-kv"><dt>{t("org.sec.password")}</dt><dd>{if s.has_permanent_password { s.password_changed.clone().map_or_else(|| t("org.sec.password.set").to_owned(), |d| tf("org.sec.password.since", &[("at", d.as_str())])) } else { t("org.sec.password.none").to_owned() }}</dd></div>
                {s.temporary.as_ref().map(|tc| view! {
                    <div class="oc-app-kv"><dt>{t("org.sec.temporary")}</dt><dd data-expired=tc.expired.then_some("")>{tf(if tc.expired { "org.sec.temporary.expired" } else { "org.sec.temporary.valid" }, &[("at", tc.expires.as_str())])}</dd></div>
                })}
                <div class="oc-app-kv"><dt>{t("org.sec.mfa")}</dt><dd>{if s.mfa_enrolled { t("org.sec.mfa.on").to_owned() } else if s.mfa_required { t("org.sec.mfa.missing").to_owned() } else { t("org.sec.mfa.off").to_owned() }}</dd></div>
                <div class="oc-app-kv"><dt>{t("org.sec.mfa_required")}</dt><dd>{yes_no(s.mfa_required)}</dd></div>
                <div class="oc-app-kv"><dt>{t("org.sec.last_sign_in")}</dt><dd>{s.last_sign_in.clone().unwrap_or_else(|| t("org.sec.never").to_owned())}</dd></div>
                <div class="oc-app-kv"><dt>{t("org.sec.failures")}</dt><dd>{s.recent_failures.to_string()}</dd></div>
            </dl>
            <h4 class="oc-res-sub">{t("org.sessions")}</h4>
            {if s.sessions.is_empty() {
                view! { <p class="oc-res-sec__empty">{t("org.sessions.none")}</p> }.into_any()
            } else {
                view! {
                    <ul class="oc-org-list">
                        {s.sessions.iter().map(|x| view! {
                            <li class="oc-org-list__row oc-org-session">
                                <span class="oc-org-session__main">
                                    <span class="oc-org-session__agent">{x.agent.clone().unwrap_or_else(|| t("org.sessions.unknown_agent").to_owned())}{x.restricted.then(|| view! { <span class="oc-org-self">{t("org.sessions.restricted")}</span> })}</span>
                                    <span class="oc-org-grant__meta">
                                        {x.ip_prefix.clone().map(|p| view! { <code>{p}</code> })}
                                        <span>{tf("org.sessions.seen", &[("at", x.last_seen.as_str())])}</span>
                                        <span>{tf("org.sessions.expires", &[("at", x.expires.as_str())])}</span>
                                    </span>
                                </span>
                                {x.revoke.as_ref().map(action)}
                            </li>
                        }).collect_view()}
                    </ul>
                }.into_any()
            }}
        </section>
    }
}

fn member_detail(m: &OrgMemberVm, back: String) -> impl IntoView {
    let actions = !m.account_actions.is_empty();
    view! {
        <article class="oc-res-doc" data-part="member">
            <header class="oc-res-head">
                <a class="oc-app__icon oc-res-back" href=back aria-label=t("res.back")>{icon("chev-l")}</a>
                {avatar(&m.avatar)}
                <div class="oc-res-head__text">
                    <h2 class="oc-res-head__title" id="oc-res-title">{m.name.clone()}</h2>
                    <p class="oc-res-head__tags">
                        {status_tag(m.status)}
                        {m.is_self.then(|| view! { <span class="oc-org-self">{t("org.self")}</span> })}
                    </p>
                </div>
            </header>
            {m.notice.map(notice)}
            {m.refusal.map(refusal)}
            {actions.then(|| view! { <div class="oc-res-actions" data-part="member-actions">{m.account_actions.iter().map(action).collect_view()}</div> })}
            {m.is_self.then(|| view! { <p class="oc-app-note">{icon("shield")}<span>{t("org.self.note")}</span></p> })}
            {(m.status == crate::ui::view_models::OrgAccountStatus::Disabled).then(|| view! { <p class="oc-app-note">{icon("minus")}<span>{t("org.disabled.note")}</span></p> })}
            <dl class="oc-res-meta">
                {m.email.clone().map(|e| view! { <div class="oc-app-kv"><dt>{t("org.col.email")}</dt><dd><code>{e}</code></dd></div> })}
                <div class="oc-app-kv"><dt>{t("org.col.position")}</dt><dd>{m.position.map_or_else(|| t("org.pos.none").to_owned(), |p| t(p.key()).to_owned())}</dd></div>
                {m.joined.clone().map(|j| view! { <div class="oc-app-kv"><dt>{t("org.col.joined")}</dt><dd>{j}</dd></div> })}
                {m.last_seen.clone().map(|j| view! { <div class="oc-app-kv"><dt>{t("org.col.last_seen")}</dt><dd>{j}</dd></div> })}
            </dl>
            {m.position_form.as_ref().map(|f| view! {
                <form class="oc-res-assign" method="post" action=f.action.clone() data-part="member-position">
                    {select("position", "org.pos.change", &f.options, false)}
                    <button type="submit" class="oc-app-btn">{t("org.pos.save")}</button>
                </form>
                <p class="oc-res-form__hint">{t("org.pos.hint")}</p>
            })}
            <section class="oc-res-sec" data-part="member-units">
                <h3 class="oc-res-sec__title">{t("org.units")}</h3>
                {if m.units.is_empty() {
                    view! { <p class="oc-res-sec__empty">{t("org.units.none")}</p> }.into_any()
                } else {
                    view! {
                        <ul class="oc-res-links">
                            {m.units.iter().map(|u| {
                                let inner = view! {
                                    <span class="oc-res-link__ic" aria-hidden="true">{icon("units")}</span>
                                    <span class="oc-res-link__main">
                                        <span class="oc-res-link__title">{u.name.clone()}</span>
                                        <span class="oc-res-link__meta"><span class="oc-res-name__code">{u.code.clone()}</span>{u.role.map(unit_role_tag)}</span>
                                    </span>
                                };
                                match u.href.clone() {
                                    Some(h) => view! { <li><a class="oc-res-link" href=h data-part="res-link">{inner}</a></li> }.into_any(),
                                    None => view! { <li><span class="oc-res-link" data-static="">{inner}</span></li> }.into_any(),
                                }
                            }).collect_view()}
                        </ul>
                    }.into_any()
                }}
            </section>
            {(!m.workspaces.is_empty()).then(|| view! {
                <section class="oc-res-sec" data-part="member-workspaces">
                    <h3 class="oc-res-sec__title">{t("org.workspaces")}</h3>
                    <ul class="oc-res-links">
                        {m.workspaces.iter().map(|w| {
                            let inner = view! {
                                <span class="oc-res-link__ic" aria-hidden="true">{icon(if w.title.is_some() { "project" } else { "lock" })}</span>
                                <span class="oc-res-link__main">
                                    <span class="oc-res-link__title">{w.title.clone().unwrap_or_else(|| t("org.workspaces.hidden").to_owned())}</span>
                                    <span class="oc-res-link__meta"><span>{t(w.role.key())}</span></span>
                                </span>
                            };
                            match w.href.clone() {
                                Some(h) => view! { <li><a class="oc-res-link" href=h data-part="res-link">{inner}</a></li> }.into_any(),
                                None => view! { <li><span class="oc-res-link" data-static="">{inner}</span></li> }.into_any(),
                            }
                        }).collect_view()}
                    </ul>
                </section>
            })}
            {m.access.as_ref().map(access)}
            {m.security.as_ref().map(security)}
        </article>
    }
}

fn new_member(f: &OrgNewMemberVm, back: String) -> impl IntoView {
    view! {
        <form class="oc-res-form" id="oc-admin-doc-new" method="post" action=f.action.clone() data-oc="app-doc" data-state="clean" aria-labelledby="oc-res-title">
            <header class="oc-res-head">
                <a class="oc-app__icon oc-res-back" href=back.clone() aria-label=t("res.back")>{icon("chev-l")}</a>
                <div class="oc-res-head__text"><h2 class="oc-res-head__title" id="oc-res-title">{t("admin.new")}</h2></div>
            </header>
            {f.refusal.map(refusal)}
            {f.error.map(error)}
            <p class="oc-app-note">{icon("key")}<span>{t("admin.new.how")}</span></p>
            <label class="oc-app-field"><span>{t("admin.new.name")}</span><input type="text" name="full_name" value=f.full_name.clone() required="" autocomplete="off" /></label>
            <label class="oc-app-field"><span>{t("org.col.email")}</span><input type="email" name="email" value=f.email.clone() required="" autocomplete="off" spellcheck="false" /></label>
            <div class="oc-app-row2">
                {select("role", "admin.new.role", &f.roles, true)}
                {select("position", "org.col.position", &f.positions, false)}
            </div>
            {select("unit_id", "admin.new.unit", &f.units, false)}
            <p class="oc-res-form__hint">{t("admin.new.hint")}</p>
            <div class="oc-res-form__foot">
                <a class="oc-app-btn" href=back>{t("app.cancel")}</a>
                <span class="oc-app__spacer"></span>
                <button type="submit" class="oc-app-primary" data-oc="app-save">{icon("check")}<span>{t("admin.new.do")}</span></button>
            </div>
        </form>
    }
}

fn roles(r: &[OrgRoleDefVm]) -> impl IntoView {
    view! {
        <article class="oc-res-doc oc-org-wide" data-part="admin-roles">
            <header class="oc-res-head"><div class="oc-res-head__text"><h2 class="oc-res-head__title" id="oc-res-title">{t("admin.roles")}</h2></div></header>
            <p class="oc-app-note">{icon("shield")}<span>{t("admin.roles.system")}</span></p>
            <ul class="oc-org-roles">
                {r.iter().map(|d| view! {
                    <li class="oc-org-roles__item">
                        <h3 class="oc-org-roles__name">{role_tag(d.role)}</h3>
                        {if d.permissions.is_empty() {
                            view! { <p class="oc-res-sec__empty">{t("admin.roles.no_perms")}</p> }.into_any()
                        } else {
                            view! { <ul class="oc-org-perm-list" aria-label=tf("admin.roles.perms_of", &[("role", t(d.role.key()))])>{d.permissions.iter().map(|p| view! { <li><code class="oc-org-perm">{p.clone()}</code></li> }).collect_view()}</ul> }.into_any()
                        }}
                    </li>
                }).collect_view()}
            </ul>
            <section class="oc-res-sec">
                <h3 class="oc-res-sec__title">{t("admin.roles.context")}</h3>
                <dl class="oc-res-meta">
                    <div class="oc-app-kv"><dt>{t("nav.units")}</dt><dd>{t("org.unit_role.manager")}" · "{t("org.unit_role.member")}</dd></div>
                    <div class="oc-app-kv"><dt>{t("org.workspaces")}</dt><dd>{t("org.ws_role.lead")}" · "{t("org.ws_role.member")}" · "{t("org.ws_role.viewer")}</dd></div>
                </dl>
                <p class="oc-res-form__hint">{t("admin.roles.position")}</p>
            </section>
        </article>
    }
}

fn instance(i: &OrgInstanceVm) -> AnyView {
    if let Some(s) = load_state(i.load, 6) {
        return s;
    }
    let dist_key = format!("admin.dist.{}", i.distribution.as_str());
    view! {
        <article class="oc-res-doc oc-org-wide" data-part="admin-instance">
            <header class="oc-res-head"><div class="oc-res-head__text"><h2 class="oc-res-head__title" id="oc-res-title">{t("admin.instance")}</h2></div></header>
            {i.notice.map(notice)}
            <dl class="oc-res-meta">
                <div class="oc-app-kv"><dt>{t("admin.instance.name")}</dt><dd>{i.name.clone()}</dd></div>
                <div class="oc-app-kv"><dt>{t("admin.instance.distribution")}</dt><dd>{t(&dist_key)}</dd></div>
                <div class="oc-app-kv"><dt>{t("admin.instance.locale")}</dt><dd>{i.default_locale.clone()}</dd></div>
                <div class="oc-app-kv"><dt>{t("admin.instance.timezone")}</dt><dd><code>{i.timezone.clone()}</code></dd></div>
                {i.updated.clone().map(|u| view! { <div class="oc-app-kv"><dt>{t("admin.instance.updated")}</dt><dd>{u}</dd></div> })}
            </dl>
            <p class="oc-res-form__hint">{t("admin.instance.distribution.hint")}</p>
            <section class="oc-res-sec" data-part="admin-apps">
                <h3 class="oc-res-sec__title">{t("admin.apps")}</h3>
                <p class="oc-res-form__hint">{t("admin.apps.hint")}</p>
                <form method="post" action=i.apps_action.clone().unwrap_or_default() class="oc-org-apps">
                    <table class="oc-org-apps__table">
                        <thead><tr><th scope="col">{t("admin.apps.app")}</th><th scope="col">{t("admin.apps.state")}</th><th scope="col">{t("admin.apps.decision")}</th></tr></thead>
                        <tbody>
                            {i.apps.iter().map(|a| {
                                let name = format!("app:{}", a.id);
                                let cur = if !a.explicit { "profile" } else if a.active { "active" } else { "inactive" };
                                let editable = i.apps_action.is_some() && !a.essential;
                                view! {
                                    <tr data-active=a.active.then_some("")>
                                        <th scope="row"><span class="oc-org-app">{icon(a.icon)}<span>{a.label.clone()}</span></span></th>
                                        <td><span class="oc-res-state" data-tone=if a.active { "done" } else { "closed" }>{t(if a.active { "admin.apps.active" } else { "admin.apps.inactive" })}</span></td>
                                        <td>
                                            {if a.essential {
                                                view! { <span class="oc-org-muted">{t("admin.apps.essential")}</span> }.into_any()
                                            } else if editable {
                                                view! {
                                                    <label class="oc-org-app__choice">
                                                        <span class="oc-sr">{tf("admin.apps.decision_for", &[("app", a.label.as_str())])}</span>
                                                        <select name=name>
                                                            <option value="profile" selected={cur == "profile"}>{tf(if a.profile_default { "admin.apps.profile_on" } else { "admin.apps.profile_off" }, &[])}</option>
                                                            <option value="active" selected={cur == "active"}>{t("admin.apps.force_on")}</option>
                                                            <option value="inactive" selected={cur == "inactive"}>{t("admin.apps.force_off")}</option>
                                                        </select>
                                                    </label>
                                                }.into_any()
                                            } else {
                                                view! { <span class="oc-org-muted">{t(match cur { "profile" => "admin.apps.by_profile", "active" => "admin.apps.force_on", _ => "admin.apps.force_off" })}</span> }.into_any()
                                            }}
                                        </td>
                                    </tr>
                                }
                            }).collect_view()}
                        </tbody>
                    </table>
                    {i.apps_action.is_some().then(|| view! {
                        <p class="oc-res-form__hint">{t("admin.apps.keep")}</p>
                        <div class="oc-res-form__foot"><span class="oc-app__spacer"></span><button type="submit" class="oc-app-primary">{icon("check")}<span>{t("admin.apps.save")}</span></button></div>
                    })}
                </form>
            </section>
        </article>
    }
    .into_any()
}

/// A aplicação Administração.
pub fn app(vm: &AdminVm) -> AnyView {
    // Recusada inteira (ligação directa sem autoridade, privilégio perdido):
    // nada protegido chega à vista — nem a navegação.
    if let Some(e) = vm.error {
        let toolbar = view! { <h2 class="oc-app__title">{t("nav.admin")}</h2> }.into_any();
        return frame(
            "admin",
            t("nav.admin").to_owned(),
            toolbar,
            None,
            error(e).into_any(),
            None,
        );
    }
    let toolbar = view! {
        <h2 class="oc-app__title">{t("nav.admin")}</h2>
        <span class="oc-app__spacer"></span>
        {(vm.section == AdminSection::Members).then(|| vm.new_href.clone().map(|h| primary(h, "user-plus", "admin.new"))).flatten()}
    }
    .into_any();
    let main = match vm.section {
        AdminSection::Members => {
            let detail = if let Some(c) = &vm.credential {
                credential_once(c).into_any()
            } else if let Some(f) = &vm.new_member {
                new_member(f, vm.list_href.clone()).into_any()
            } else if let Some(m) = &vm.member {
                member_detail(m, vm.list_href.clone()).into_any()
            } else {
                detail_or(
                    vm.member_error,
                    empty("user", "admin.none_open", None).into_any(),
                )
            };
            two_pane(vm.pane, roster(&vm.members), detail)
        }
        AdminSection::Roles => roles(&vm.roles).into_any(),
        AdminSection::Instance => vm.instance.as_ref().map_or_else(
            || error(crate::ui::view_models::AppError::Unavailable).into_any(),
            instance,
        ),
    };
    frame(
        "admin",
        t("nav.admin").to_owned(),
        toolbar,
        Some(nav(&vm.nav).into_any()),
        main,
        None,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::testing::assert_contracts;
    use crate::ui::view_models::{
        AppError, AppLoad, AppPageVm, OrgAccountStatus, OrgActionKind, OrgActionVm, OrgAvatarVm,
        OrgCredentialOnceVm, ResPane,
    };

    fn vm() -> AdminVm {
        AdminVm {
            section: AdminSection::Members,
            nav: vec![],
            error: None,
            members: OrgMembersListVm {
                rows: vec![OrgMemberRowVm {
                    name: "<i>Rui</i>".into(),
                    email: Some("rui@x".into()),
                    avatar: OrgAvatarVm {
                        initials: "RT".into(),
                        image_href: None,
                    },
                    status: OrgAccountStatus::Suspended,
                    position: None,
                    units: vec!["UENR-001".into()],
                    joined: None,
                    last_seen: None,
                    href: "/admin/members/r".into(),
                    active: false,
                    is_self: false,
                }],
                load: AppLoad::Ready,
                page: AppPageVm::default(),
            },
            pane: ResPane::List,
            member: None,
            member_error: None,
            new_member: None,
            credential: None,
            new_href: None,
            roles: vec![],
            instance: None,
            list_href: "/admin".into(),
        }
    }

    fn member(status: OrgAccountStatus, is_self: bool, actions: Vec<OrgActionVm>) -> OrgMemberVm {
        OrgMemberVm {
            name: "Marta".into(),
            email: None,
            avatar: OrgAvatarVm {
                initials: "MQ".into(),
                image_href: None,
            },
            status,
            position: None,
            is_self,
            joined: None,
            last_seen: None,
            units: vec![],
            workspaces: vec![],
            access: None,
            security: None,
            account_actions: actions,
            position_form: None,
            refusal: None,
            notice: None,
        }
    }

    #[test]
    fn o_roster_escapa_nomes_e_mostra_o_estado_com_texto() {
        let html = app(&vm()).to_html();
        assert_contracts(&html);
        assert!(html.contains("&lt;i&gt;Rui") && html.contains(t("org.status.suspended")));
        assert!(html.contains(r#"data-prio="4""#));
    }

    #[test]
    fn sem_autoridade_nada_protegido_aparece() {
        let mut v = vm();
        v.error = Some(AppError::Revoked);
        let html = app(&v).to_html();
        assert!(html.contains(r#"data-error="app.err.revoked""#));
        assert!(!html.contains("Rui") && !html.contains("oc-app-nav"));
    }

    #[test]
    fn sem_acoes_do_core_nao_ha_botoes_de_conta() {
        let mut v = vm();
        v.pane = ResPane::Detail;
        v.member = Some(member(OrgAccountStatus::Active, true, vec![]));
        let html = app(&v).to_html();
        assert!(
            !html.contains(r#"data-part="member-actions""#) && html.contains(t("org.self.note"))
        );
    }

    #[test]
    fn uma_accao_abre_a_confirmacao_e_nao_executa() {
        let mut v = vm();
        v.pane = ResPane::Detail;
        v.member = Some(member(
            OrgAccountStatus::Active,
            false,
            vec![OrgActionVm {
                kind: OrgActionKind::Suspend,
                href: "/admin/members/m?confirm=suspend".into(),
            }],
        ));
        let html = app(&v).to_html();
        assert!(html.contains(r#"href="/admin/members/m?confirm=suspend""#));
        assert!(!html.contains("<form class=\"oc-dialog"));
    }

    #[test]
    fn o_segredo_so_existe_no_ecra_da_credencial() {
        let mut v = vm();
        v.member = Some(member(OrgAccountStatus::Invited, false, vec![]));
        assert!(!app(&v).to_html().contains("Tmp-9fQ2"));
        v.credential = Some(OrgCredentialOnceVm {
            origin: OrgActionKind::Provision,
            name: "Marta".into(),
            email: "m@x".into(),
            secret: "Tmp-9fQ2".into(),
            expires: "x".into(),
            done_href: "/admin/members/m".into(),
        });
        v.pane = ResPane::Detail;
        let html = app(&v).to_html();
        assert_eq!(html.matches("Tmp-9fQ2").count(), 1);
    }
}
