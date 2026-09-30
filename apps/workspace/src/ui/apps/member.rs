//! D007 · Definições (`/settings`) e Ajuda (`/help`). DESIGN_LOCKED.
//!
//! Definições editam só a camada do **membro**: avatar, idioma (pt/en/fr),
//! palavra-passe, segundo factor (os ecrãs D001), sessões próprias e aplicações
//! fixadas. O fuso é da Instância e aparece herdado, só leitura. Nada aqui muda
//! a Instância ou a distribuição. Cada secção grava com o seu botão.
//!
//! Ajuda é conteúdo de primeira parte, versionado com o código: um tópico por
//! aplicação a partir do registo, e os atalhos que o runtime declara. Não é o
//! Conhecimento, nem um segundo assistente — «Perguntar à Nye» só entrega a
//! pergunta à Nye canónica.

use leptos::prelude::*;

use super::org::avatar;
use super::{frame, nav, nye, search};
use crate::i18n::{t, tf};
use crate::ui::components::icon;
use crate::ui::view_models::{HelpSection, HelpVm, SettingsSection, SettingsVm};

fn sec_head(key: &'static str) -> impl IntoView {
    view! { <header class="oc-res-head"><div class="oc-res-head__text"><h2 class="oc-res-head__title" id="oc-res-title">{t(key)}</h2></div></header> }
}

fn save_foot(label: &'static str) -> impl IntoView {
    view! { <div class="oc-res-form__foot"><span class="oc-app__spacer"></span><button type="submit" class="oc-app-primary" data-oc="app-save">{icon("check")}<span>{t(label)}</span></button></div> }
}

/// A aplicação Definições.
pub fn settings_app(vm: &SettingsVm) -> AnyView {
    let toolbar = view! { <h2 class="oc-app__title">{t("nav.settings")}</h2> }.into_any();
    let head = view! {
        {vm.saved.then(|| view! { <p class="oc-app-note oc-org-done" role="status">{icon("check")}<span>{t("settings.saved")}</span></p> })}
        {vm.error.map(super::error)}
    };
    let body = match vm.section {
        SettingsSection::Account => view! {
            {sec_head("settings.account")}{head}
            <dl class="oc-res-meta">
                <div class="oc-app-kv"><dt>{t("admin.new.name")}</dt><dd>{vm.name.clone()}</dd></div>
                <div class="oc-app-kv"><dt>{t("org.col.email")}</dt><dd><code>{vm.email.clone()}</code></dd></div>
            </dl>
            <p class="oc-res-form__hint">{t("settings.account.admin")}</p>
            <section class="oc-res-sec"><h3 class="oc-res-sec__title">{t("settings.avatar")}</h3>
                <div class="oc-ops-avatar">{avatar(&vm.avatar)}
                    <form method="post" action="/settings/avatar/initials"><button type="submit" class="oc-app-btn">{t("settings.avatar.initials")}</button></form>
                </div>
                <form class="oc-ops-presets" method="post" action="/settings/avatar/preset">
                    <fieldset class="oc-org-cands"><legend class="oc-res-form__hint">{t("settings.avatar.presets")}</legend>
                        {vm.presets.iter().map(|o| view! { <label class="oc-org-cand"><input type="radio" name="preset" value=o.value.clone() checked=o.selected required="" /><span>{o.label.clone()}</span></label> }).collect_view()}
                    </fieldset>
                    {save_foot("settings.avatar.save")}
                </form>
                <form class="oc-res-assign" method="post" action="/settings/avatar/photo" enctype="multipart/form-data">
                    <label class="oc-app-field"><span>{t("settings.avatar.photo")}</span><input type="file" name="photo" accept="image/png,image/jpeg,image/webp" required="" /></label>
                    <button type="submit" class="oc-app-btn">{icon("upload")}<span>{t("settings.avatar.upload")}</span></button>
                </form>
            </section>
        }.into_any(),
        SettingsSection::Language => view! {
            {sec_head("settings.language")}{head}
            <form class="oc-res-form oc-ops-inline" method="post" action="/settings/language">
                <fieldset class="oc-org-cands"><legend class="oc-res-form__hint">{t("settings.language.pick")}</legend>
                    {vm.locales.iter().map(|o| view! { <label class="oc-org-cand"><input type="radio" name="locale" value=o.value.clone() checked=o.selected required="" /><span lang=o.value.clone()>{o.label.clone()}</span></label> }).collect_view()}
                </fieldset>
                {save_foot("settings.language.save")}
            </form>
            <section class="oc-res-sec"><h3 class="oc-res-sec__title">{t("settings.timezone")}</h3>
                <p class="oc-org-readonly"><code>{vm.timezone.clone()}</code><span>{t("settings.timezone.inherited")}</span></p>
            </section>
        }.into_any(),
        SettingsSection::Security => view! {
            {sec_head("settings.security")}{head}
            <form class="oc-res-form oc-ops-inline" id="oc-settings-doc-password" method="post" action="/settings/password" data-oc="app-doc" data-state="clean">
                <h3 class="oc-res-sec__title">{t("settings.password")}</h3>
                {vm.password_changed.clone().map(|d| view! { <p class="oc-res-form__hint">{tf("org.sec.password.since", &[("at", d.as_str())])}</p> })}
                <label class="oc-app-field"><span>{t("settings.password.current")}</span><input type="password" name="current_password" autocomplete="current-password" required="" /></label>
                <label class="oc-app-field"><span>{t("settings.password.new")}</span><input type="password" name="new_password" autocomplete="new-password" minlength="15" required="" /></label>
                <p class="oc-res-form__hint">{t("settings.password.rule")}</p>
                {save_foot("settings.password.save")}
            </form>
            <section class="oc-res-sec"><h3 class="oc-res-sec__title">{t("org.sec.mfa")}</h3>
                <p class="oc-res-form__hint">{t(if vm.mfa.0 { "org.sec.mfa.on" } else if vm.mfa.1 { "org.sec.mfa.missing" } else { "org.sec.mfa.off" })}</p>
                <a class="oc-app-btn" href="/settings/mfa">{icon("shield")}<span>{t(if vm.mfa.0 { "settings.mfa.manage" } else { "settings.mfa.setup" })}</span></a>
            </section>
            <section class="oc-res-sec"><h3 class="oc-res-sec__title">{t("org.sessions")}</h3>
                <ul class="oc-org-list">{vm.sessions.iter().map(|s| view! {
                    <li class="oc-org-list__row oc-org-session"><span class="oc-org-session__main"><span class="oc-org-session__agent">{s.agent.clone()}{s.current.then(|| view! { <span class="oc-org-self">{t("settings.session.current")}</span> })}</span><span class="oc-org-grant__meta"><span>{tf("org.sessions.seen", &[("at", s.seen.as_str())])}</span></span></span>
                    {s.revoke_action.clone().map(|a| view! { <form method="post" action=a><button type="submit" class="oc-app-btn oc-app-btn--danger">{icon("lock")}<span>{t("org.act.revoke_session.label")}</span></button></form> })}</li>
                }).collect_view()}</ul>
            </section>
        }.into_any(),
        SettingsSection::Apps => view! {
            {sec_head("settings.apps")}{head}
            <form class="oc-res-form oc-ops-inline" id="oc-settings-doc-apps" method="post" action="/settings/apps" data-oc="app-doc" data-state="clean">
                <p class="oc-res-form__hint">{tf("settings.apps.source", &[("source", vm.pins_source.as_str())])}</p>
                <fieldset class="oc-ops-pins"><legend class="oc-sr">{t("settings.apps")}</legend>
                    {vm.pins.iter().map(|p| view! { <label class="oc-app-check oc-ops-pin"><input type="checkbox" name="pinned" value=p.id.clone() checked=p.pinned />{icon(p.icon)}<span>{p.label.clone()}</span></label> }).collect_view()}
                </fieldset>
                <p class="oc-app-note">{icon("shield")}<span>{t("settings.apps.not_access")}</span></p>
                <div class="oc-res-form__foot"><button type="submit" class="oc-app-btn" name="action" value="reset">{t("settings.apps.reset")}</button><span class="oc-app__spacer"></span><button type="submit" class="oc-app-primary" data-oc="app-save">{icon("check")}<span>{t("settings.apps.save")}</span></button></div>
            </form>
        }.into_any(),
    };
    let main =
        view! { <article class="oc-res-doc oc-org-wide" data-part="settings">{body}</article> }
            .into_any();
    frame(
        "settings",
        t("nav.settings").to_owned(),
        toolbar,
        Some(nav(&vm.nav).into_any()),
        main,
        None,
    )
}

/// A aplicação Ajuda.
pub fn help_app(vm: &HelpVm) -> AnyView {
    let toolbar = view! {
        <h2 class="oc-app__title">{t("nav.help")}</h2><span class="oc-app__spacer"></span>
        {vm.query.as_ref().map(|q| search("/help".into(), q, "help.search"))}
    }
    .into_any();
    let body = match vm.section {
        HelpSection::Apps => {
            if vm.topics.is_empty() {
                super::empty("search", "help.search.empty", None).into_any()
            } else {
                view! {
                    <ul class="oc-ops-topics">{vm.topics.iter().map(|x| view! {
                        <li class="oc-ops-topic" id=x.anchor.clone()>
                            <span class="oc-res-link__ic" aria-hidden="true">{icon(x.icon)}</span>
                            <div class="oc-ops-topic__main"><h3 class="oc-ops-topic__title">{x.title.clone()}<span class="oc-ops-sub">{x.category.clone()}</span></h3><p class="oc-ops-topic__body">{x.body.clone()}</p></div>
                            {x.open_href.clone().map(|h| view! { <a class="oc-app-btn" href=h>{t("help.open")}</a> })}
                        </li>
                    }).collect_view()}</ul>
                }.into_any()
            }
        }
        HelpSection::Shortcuts => view! {
            <table class="oc-ops-table" aria-label=t("help.shortcuts")>
                <thead><tr><th scope="col">{t("help.keys")}</th><th scope="col">{t("help.what")}</th></tr></thead>
                <tbody>{vm.shortcuts.iter().map(|s| view! { <tr><th scope="row"><kbd class="oc-kbd">{s.keys.clone()}</kbd></th><td>{s.what.clone()}</td></tr> }).collect_view()}</tbody>
            </table>
        }.into_any(),
    };
    let key = if vm.section == HelpSection::Apps {
        "help.apps"
    } else {
        "help.shortcuts"
    };
    let main = view! {
        <article class="oc-res-doc oc-org-wide" data-part="help">
            {sec_head(key)}
            <p class="oc-res-form__hint">{t("help.source")}</p>
            {body}
            {vm.nye.as_ref().map(|n| view! { <p class="oc-app-note">{icon("nye")}<span>{t("help.nye")}</span>{nye(n)}</p> })}
        </article>
    }
    .into_any();
    frame(
        "help",
        t("nav.help").to_owned(),
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
    use crate::ui::view_models::{HelpTopicVm, OrgAvatarVm, ResOptionVm};

    #[test]
    fn definicoes_nao_editam_o_fuso_da_instancia() {
        let vm = SettingsVm {
            section: SettingsSection::Language,
            nav: vec![],
            saved: false,
            error: None,
            name: "M".into(),
            email: "m@x".into(),
            avatar: OrgAvatarVm {
                initials: "M".into(),
                image_href: None,
            },
            presets: vec![],
            locales: vec![ResOptionVm {
                value: "pt".into(),
                label: "Português".into(),
                selected: true,
            }],
            timezone: "Africa/Luanda".into(),
            mfa: (false, false),
            password_changed: None,
            sessions: vec![],
            pins: vec![],
            pins_source: String::new(),
        };
        let html = settings_app(&vm).to_html();
        assert_contracts(&html);
        assert!(html.contains("Africa/Luanda") && !html.contains(r#"name="timezone""#));
    }

    #[test]
    fn ajuda_escapa_a_pesquisa_e_nao_tem_conversa() {
        let vm = HelpVm {
            section: HelpSection::Apps,
            nav: vec![],
            query: Some("<script>".into()),
            topics: vec![HelpTopicVm {
                title: "Notas".into(),
                icon: "notes",
                category: "Produtividade".into(),
                body: "x".into(),
                open_href: None,
                anchor: "notes".into(),
            }],
            shortcuts: vec![],
            nye: None,
        };
        let html = help_app(&vm).to_html();
        assert!(!html.contains("<script>") && !html.contains("<textarea"));
    }
}
