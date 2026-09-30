//! D006 · As peças partilhadas de Organização e Administração. DESIGN_LOCKED.
//!
//! Estendem o sistema de aplicação D004 e as peças D005 ([`super::res`]) sem os
//! duplicar. Aqui ficam só as que Unidades e Administração partilham e nenhuma
//! aplicação anterior tinha: o estado da conta, o papel técnico (sem
//! hierarquia visual), o avatar por iniciais, as recusas das invariantes do
//! Core, a confirmação partilhada das acções privilegiadas e a credencial
//! temporária mostrada uma vez.
//!
//! A vista não decide nada: uma acção só aparece quando o VM a traz, e
//! confirmar não autoriza — o Core volta a decidir no POST.

use leptos::prelude::*;

use crate::i18n::{t, tf};
use crate::ui::components::icon;
use crate::ui::view_models::{
    OrgAccountStatus, OrgActionKind, OrgActionVm, OrgAvatarVm, OrgConfirmVm, OrgCredentialOnceVm,
    OrgNotice, OrgReason, OrgRefusal, OrgTechRole, OrgUnitRole, OrgUnitStatus,
};

use super::error;

/// O estado da conta: texto + ícone + tom (a cor nunca vai sozinha).
pub fn status_tag(s: OrgAccountStatus) -> impl IntoView {
    let (tone, ic) = match s {
        OrgAccountStatus::Invited => ("neutral", "clock"),
        OrgAccountStatus::Active => ("done", "check"),
        OrgAccountStatus::Suspended => ("attention", "lock"),
        OrgAccountStatus::Disabled => ("closed", "minus"),
    };
    view! { <span class="oc-res-state oc-org-status" data-tone=tone data-status=s.as_str()>{icon(ic)}{t(s.key())}</span> }
}

/// O estado de uma unidade.
pub fn unit_status_tag(s: OrgUnitStatus) -> impl IntoView {
    let (tone, key) = match s {
        OrgUnitStatus::Active => ("progress", "units.state.active"),
        OrgUnitStatus::Archived => ("closed", "units.state.archived"),
    };
    view! { <span class="oc-res-state" data-tone=tone>{t(key)}</span> }
}

/// Um papel técnico: rótulo e identificador estável. Todos com o mesmo tom:
/// o Core não os ordena, e a vista não sugere hierarquia.
pub fn role_tag(r: OrgTechRole) -> impl IntoView {
    view! { <span class="oc-org-role">{t(r.key())}<code class="oc-org-role__id">{r.as_str()}</code></span> }
}

/// O papel numa unidade.
pub fn unit_role_tag(r: OrgUnitRole) -> impl IntoView {
    view! { <span class="oc-org-urole" data-role=r.as_str()>{t(r.key())}</span> }
}

/// O avatar: imagem servida pelo Core, ou iniciais. Decorativo (o nome vai ao lado).
pub fn avatar(a: &OrgAvatarVm) -> impl IntoView {
    match a.image_href.clone() {
        Some(src) => view! { <span class="oc-org-av" aria-hidden="true"><img src=src alt="" width="32" height="32" loading="lazy" decoding="async" /></span> }.into_any(),
        None => view! { <span class="oc-org-av" aria-hidden="true">{a.initials.clone()}</span> }.into_any(),
    }
}

/// A recusa de uma invariante do Core, com o que fazer a seguir.
pub fn refusal(r: OrgRefusal) -> impl IntoView {
    view! {
        <p class="oc-app-note oc-org-refusal" data-tone="warn" data-refusal=r.key() role="alert">
            {icon("shield")}<span><strong>{t(&format!("{}.title", r.key()))}</strong>" "{t(&format!("{}.body", r.key()))}</span>
        </p>
    }
}

/// O que acabou de acontecer: uma linha, anunciada, que não se sobrepõe a nada.
pub fn notice(n: OrgNotice) -> impl IntoView {
    view! { <p class="oc-app-note oc-org-done" role="status" data-notice=n.key()>{icon("check")}<span>{t(n.key())}</span></p> }
}

/// Uma acção disponível: abre a confirmação partilhada. Nunca executa.
pub fn action(a: &OrgActionVm) -> impl IntoView {
    let class = if a.kind.reduces_access() {
        "oc-app-btn oc-app-btn--danger"
    } else {
        "oc-app-btn"
    };
    let ic = match a.kind {
        OrgActionKind::Suspend | OrgActionKind::RevokeSession => "lock",
        OrgActionKind::Disable
        | OrgActionKind::RemoveUnitMember
        | OrgActionKind::RemoveParticipant
        | OrgActionKind::LeaveConversation
        | OrgActionKind::RevokeRole
        | OrgActionKind::RevokeGrant => "minus",
        OrgActionKind::Reactivate => "refresh",
        OrgActionKind::ResetPassword | OrgActionKind::Provision | OrgActionKind::Reissue => "key",
        OrgActionKind::DeleteInvite => "trash",
        OrgActionKind::ArchiveUnit => "archive",
        OrgActionKind::GrantRole | OrgActionKind::ChangeUnitRole => "edit",
    };
    view! {
        <a class=class href=a.href.clone() data-part="org-action" data-kind=a.kind.key()>
            {icon(ic)}<span>{t(&format!("{}.label", a.kind.key()))}</span>
        </a>
    }
}

/// A confirmação partilhada das acções privilegiadas.
///
/// Desenhada pela rota **depois** da casca (como `wm::dirty_close` e
/// `nye::confirm_dialog`), acima da janela. O alvo e a acção vão no título;
/// a consequência é a do Core, não uma estimativa. O foco inicial vai para
/// «Cancelar», nunca para o botão que retira acesso. Escape = cancelar.
pub fn confirm(c: &OrgConfirmVm) -> impl IntoView {
    let k = c.kind.key();
    let danger = c.kind.reduces_access();
    let ok_class = if danger {
        "oc-btn-line oc-btn-line--danger"
    } else {
        "oc-btn-gold"
    };
    let (reason_required, min) = match c.reason {
        OrgReason::Required(n) => (true, Some(n)),
        _ => (false, None),
    };
    let has_reason = c.reason != OrgReason::None;
    view! {
        <div class="oc-overlay oc-overlay--center oc-org-confirm" data-open="" data-oc="org-confirm" role="alertdialog" aria-modal="true" aria-labelledby="oc-org-confirm-t" aria-describedby="oc-org-confirm-d">
            <form class="oc-dialog oc-org-confirm__box" method="post" action=c.action.clone() data-kind=k data-danger=danger.then_some("")>
                {c.hidden.iter().map(|(n, v)| view! { <input type="hidden" name=*n value=v.clone() /> }).collect_view()}
                <span class="oc-dialog__icon" aria-hidden="true">{icon(if danger { "shield" } else { "key" })}</span>
                <h2 class="oc-dialog__title" id="oc-org-confirm-t">{tf(&format!("{k}.title"), &[("name", c.target.as_str())])}</h2>
                <dl class="oc-org-confirm__facts">
                    <div class="oc-app-kv"><dt>{t("org.confirm.target")}</dt><dd>{c.target.clone()}</dd></div>
                    {c.context.clone().map(|x| view! { <div class="oc-app-kv"><dt>{t("org.confirm.context")}</dt><dd>{x}</dd></div> })}
                    {c.change.clone().map(|(from, to)| view! {
                        <div class="oc-app-kv"><dt>{t("org.confirm.current")}</dt><dd>{from}</dd></div>
                        <div class="oc-app-kv"><dt>{t("org.confirm.proposed")}</dt><dd><strong>{to}</strong></dd></div>
                    })}
                </dl>
                <p class="oc-dialog__body" id="oc-org-confirm-d">{t(&format!("{k}.body"))}</p>
                {c.refusal.map(refusal)}
                {c.error.map(error)}
                {has_reason.then(|| view! {
                    <label class="oc-app-field oc-org-confirm__reason">
                        <span>{t(if reason_required { "org.confirm.reason" } else { "org.confirm.reason_optional" })}</span>
                        <textarea name="reason" rows="2" required=reason_required.then_some("") minlength=min.map(|n| n.to_string()) data-part="org-reason"></textarea>
                        {min.map(|n| view! { <span class="oc-org-confirm__hint">{tf("org.confirm.reason_min", &[("n", n.to_string().as_str())])}</span> })}
                    </label>
                })}
                <p class="oc-dialog__note">{t("org.confirm.core")}</p>
                <div class="oc-dialog__actions">
                    <a class="oc-btn-plain" href=c.cancel_href.clone() data-oc="org-confirm-cancel" autofocus="">{t("app.cancel")}</a>
                    <span class="oc-dialog__spacer"></span>
                    <button type="submit" class=ok_class data-oc="org-confirm-ok">{t(&format!("{k}.do"))}</button>
                </div>
            </form>
        </div>
    }
}

/// A credencial temporária, mostrada uma vez.
///
/// O campo começa escondido (`type="password"`); «Mostrar» e «Copiar» são do
/// `oc-apps.js` e funcionam sem guardar nada. Sem JS, o texto lê-se pela
/// selecção. Diz o que o Core fez e o que não fez: nada foi enviado.
pub fn credential_once(c: &OrgCredentialOnceVm) -> impl IntoView {
    let k = c.origin.key();
    view! {
        <section class="oc-org-cred" data-part="org-credential" aria-labelledby="oc-org-cred-t">
            <header class="oc-org-cred__head">
                <span class="oc-org-cred__ic" aria-hidden="true">{icon("key")}</span>
                <div>
                    <h2 class="oc-res-head__title" id="oc-org-cred-t">{t(&format!("{k}.issued"))}</h2>
                    <p class="oc-org-cred__who">{c.name.clone()}</p>
                </div>
            </header>
            <p class="oc-app-note" data-tone="warn">{icon("eye")}<span><strong>{t("org.cred.once")}</strong>" "{t("org.cred.once.body")}</span></p>
            <dl class="oc-res-meta">
                <div class="oc-app-kv"><dt>{t("org.col.email")}</dt><dd><code>{c.email.clone()}</code></dd></div>
                <div class="oc-app-kv"><dt>{t("org.cred.expires")}</dt><dd>{c.expires.clone()}</dd></div>
            </dl>
            <div class="oc-org-cred__field">
                <label class="oc-app-field">
                    <span>{t("org.cred.secret")}</span>
                    <input type="password" readonly="" value=c.secret.clone() autocomplete="off" spellcheck="false" data-part="org-secret" id="oc-org-secret" />
                </label>
                <button type="button" class="oc-app-btn" data-oc="org-secret-show" aria-controls="oc-org-secret" aria-pressed="false" data-show=t("org.cred.show") data-hide=t("org.cred.hide")>{icon("eye")}<span>{t("org.cred.show")}</span></button>
                <button type="button" class="oc-app-btn" data-oc="org-secret-copy" aria-controls="oc-org-secret" data-done=t("org.cred.copied")>{icon("copy")}<span>{t("org.cred.copy")}</span></button>
            </div>
            <ul class="oc-org-cred__facts">
                <li>{t("org.cred.fact.not_sent")}</li>
                <li>{t("org.cred.fact.restricted")}</li>
                <li>{t("org.cred.fact.unrecoverable")}</li>
            </ul>
            <div class="oc-res-form__foot">
                <span class="oc-app__spacer"></span>
                <a class="oc-app-primary" href=c.done_href.clone() data-oc="org-cred-done">{icon("check")}<span>{t("org.cred.done")}</span></a>
            </div>
        </section>
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::testing::assert_contracts;

    fn c(kind: OrgActionKind, reason: OrgReason) -> OrgConfirmVm {
        OrgConfirmVm {
            kind,
            target: "<b>Marta</b>".into(),
            context: None,
            change: None,
            action: "/admin/members/p/status".into(),
            hidden: vec![("status", "suspended".into())],
            reason,
            cancel_href: "/admin/members/p".into(),
            refusal: None,
            error: None,
        }
    }

    #[test]
    fn confirmar_nomeia_o_alvo_e_o_foco_vai_para_cancelar() {
        let html = confirm(&c(OrgActionKind::Suspend, OrgReason::Required(4))).to_html();
        assert_contracts(&html);
        assert!(html.contains("&lt;b&gt;Marta") && !html.contains("<b>Marta"));
        assert!(html.contains(r#"data-oc="org-confirm-cancel" autofocus"#));
        assert!(!html.contains(r#"data-oc="org-confirm-ok" autofocus"#));
        assert!(html.contains(r#"name="reason""#) && html.contains(r#"minlength="4""#));
        assert!(html.contains("oc-btn-line--danger"));
    }

    #[test]
    fn sem_motivo_no_dominio_nao_ha_campo_de_motivo() {
        let html = confirm(&c(OrgActionKind::RevokeSession, OrgReason::None)).to_html();
        assert!(!html.contains(r#"name="reason""#));
    }

    #[test]
    fn a_credencial_comeca_escondida_e_diz_que_nada_foi_enviado() {
        let html = credential_once(&OrgCredentialOnceVm {
            origin: OrgActionKind::ResetPassword,
            name: "Marta".into(),
            email: "marta@x".into(),
            secret: "s3cr3t-Value".into(),
            expires: "amanhã".into(),
            done_href: "/admin/members/p".into(),
        })
        .to_html();
        assert_contracts(&html);
        assert!(html.contains(r#"type="password""#) && html.contains(t("org.cred.fact.not_sent")));
    }

    #[test]
    fn o_estado_leva_sempre_texto() {
        for s in [
            OrgAccountStatus::Invited,
            OrgAccountStatus::Active,
            OrgAccountStatus::Suspended,
            OrgAccountStatus::Disabled,
        ] {
            let html = status_tag(s).to_html();
            assert!(html.contains(t(s.key())) && !html.contains(s.key()));
        }
    }

    #[test]
    fn os_papeis_nao_tem_tom_de_hierarquia() {
        let a = role_tag(OrgTechRole::PlatformAdmin).to_html();
        let b = role_tag(OrgTechRole::Collaborator).to_html();
        assert!(!a.contains("data-tone") && !b.contains("data-tone"));
    }
}
