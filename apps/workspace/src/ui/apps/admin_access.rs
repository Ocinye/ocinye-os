//! D010 · Administração › Distribuições, Acesso a Distribuições, Pontos de
//! acesso, Predefinições (S26–S33, S37, S38).
//!
//! Code (D010): o pacote trouxe estas secções como marcação em
//! `reference/d010/d010.js` (`cur === 'dist' | 'access' | 'ep' | 'prov'`), só
//! com classes de produção. Isto é a transcrição. Três diferenças, todas por
//! causa do servidor e não do desenho:
//!
//! - os arranjos que a referência escreve com `style=` passam a classes
//!   `oc-adm-*` (bloco Code em `oc-access.css`), porque a CSP descarta esses
//!   atributos;
//! - a referência escolhe tabela ou lista pelo `mob` do JavaScript; aqui as
//!   duas saem, e o CSS mostra a que cabe na largura;
//! - os botões `type="button"` passam a acções reais — `POST` ou a ligação
//!   para a confirmação governada.

use leptos::prelude::*;

use crate::experience::iconography::dist_icon_id;
use crate::i18n::{t, tf};
use crate::ui::components::icon;
use crate::ui::view_models::{
    AdmAccessRowVm, AdmAddEndpointVm, AdmConfirmVm, AdmDistRowVm, AdmEndpointRowVm, AdmFact,
    AdminD010Vm, Distribution,
};

/// O nome de uma Distribuição na língua da página.
#[must_use]
pub fn dist_name(d: Distribution) -> &'static str {
    t(match d {
        Distribution::Research => "dist.research",
        Distribution::Business => "dist.business",
        Distribution::Personal => "dist.personal",
        Distribution::Education => "dist.education",
    })
}

fn tag(tone: &'static str, icn: &'static str, text: &str) -> AnyView {
    view! { <span class="oc-res-state" data-tone=tone>{icon(icn)}{text.to_owned()}</span> }
        .into_any()
}

fn dname(d: Distribution) -> AnyView {
    view! {
        <span class="oc-adm-dname">
            <span class="oc-dist" data-distribution=d.as_str() aria-hidden="true">{icon(dist_icon_id(d))}</span>
            <strong>{dist_name(d)}</strong>
        </span>
    }
    .into_any()
}

fn head(title: &'static str, lead: &'static str) -> AnyView {
    view! {
        <h2 class="oc-adm-title">{t(title)}</h2>
        <p class="oc-adm-lead">{t(lead)}</p>
    }
    .into_any()
}

fn note(tone: &'static str, icn: &'static str, text: AnyView) -> AnyView {
    view! { <p class="oc-app-note" data-tone=tone>{icon(icn)}<span>{text}</span></p> }.into_any()
}

fn btn_class(danger: bool) -> &'static str {
    if danger {
        "oc-app-btn oc-app-btn--danger"
    } else {
        "oc-app-btn"
    }
}

/// Uma acção imediata (activar, dar acesso): um `POST` sem confirmação.
fn post_btn(action: String, icn: &'static str, key: &'static str) -> AnyView {
    view! {
        <form method="post" action=action class="oc-adm-inline">
            <button type="submit" class="oc-app-btn" aria-label=t(key)>{icon(icn)}<span>{t(key)}</span></button>
        </form>
    }
    .into_any()
}

/// Uma acção governada: abre a confirmação.
fn confirm_btn(href: String, icn: &'static str, key: &'static str, danger: bool) -> AnyView {
    view! { <a class=btn_class(danger) href=href aria-label=t(key)>{icon(icn)}<span>{t(key)}</span></a> }.into_any()
}

/// A acção que uma invariante conhecida recusa: fica à vista, desligada, e diz porquê.
fn refused_btn(icn: &'static str, key: &'static str, why: &'static str) -> AnyView {
    view! {
        <button type="button" class="oc-app-btn oc-app-btn--danger" aria-disabled="true" aria-describedby=why aria-label=t(key)>{icon(icn)}<span>{t(key)}</span></button>
    }
    .into_any()
}

/// `table(label, cols, rows)` da referência: a tabela (com as colunas que saem
/// por `data-prio` a partir da terceira) e a lista do ecrã estreito.
/// `rows(list)` desenha as linhas; `list` diz se é a lista estreita (os ids
/// que as linhas levam não se repetem entre as duas).
fn table(label: &str, cols: &[String], rows: impl Fn(bool) -> Vec<Vec<AnyView>>) -> AnyView {
    let prio = |i: usize| (i > 1).then(|| (i - 1).to_string());
    let class = |i: usize| if i == 0 { "oc-res-c-title" } else { "oc-res-c" };
    let header = cols
        .iter()
        .enumerate()
        .map(|(i, c)| view! { <th scope="col" class=class(i) data-prio=prio(i)>{c.clone()}</th> })
        .collect_view();
    let body = rows(false)
        .into_iter()
        .map(|r| {
            let cells = r
                .into_iter()
                .enumerate()
                .map(|(i, c)| view! { <td class=class(i) data-prio=prio(i)>{c}</td> })
                .collect_view();
            view! { <tr class="oc-res-row">{cells}</tr> }
        })
        .collect_view();
    let list = rows(true)
        .into_iter()
        .map(|r| {
            let cells = r
                .into_iter()
                .enumerate()
                .map(|(i, c)| {
                    if i == 0 {
                        view! { <div>{c}</div> }.into_any()
                    } else {
                        let col = cols.get(i).filter(|c| !c.is_empty()).cloned();
                        view! {
                            <div class="oc-adm-item__kv">
                                {col.map(|c| view! { <span class="oc-adm-item__k">{c}</span> })}
                                <span>{c}</span>
                            </div>
                        }
                        .into_any()
                    }
                })
                .collect_view();
            view! { <li class="oc-adm-item">{cells}</li> }
        })
        .collect_view();
    view! {
        <div class="oc-adm-tablewrap">
            <table class="oc-res-table" aria-label=label.to_owned()>
                <thead><tr>{header}</tr></thead>
                <tbody>{body}</tbody>
            </table>
        </div>
        <ul class="oc-adm-list" aria-label=label.to_owned() data-part="adm-list">{list}</ul>
    }
    .into_any()
}

/// S26 · As quatro Distribuições e o estado de cada uma.
fn distributions(rows: &[AdmDistRowVm], manage: bool) -> AnyView {
    let cols = [
        t("auth.distribution").to_owned(),
        t("adm.dist.col.state").to_owned(),
        t("adm.dist.col.members").to_owned(),
        t("adm.dist.col.endpoints").to_owned(),
        String::new(),
    ];
    let rows = rows.to_vec();
    let body = table(t("adm.sec.distributions"), &cols, move |in_list| {
        let why = if in_list {
            "adm-dist-last-m"
        } else {
            "adm-dist-last"
        };
        rows.iter()
            .map(|r| {
                let d = r.distribution;
                let on = r.state == "enabled";
                let action = if !manage {
                    ().into_any()
                } else if on && r.last {
                    view! {
                        {refused_btn("minus", "adm.dist.disable", why)}
                        <p class="oc-adm-sub" id=why data-part="dist-last">{icon("lock")}" "{t("adm.dist.last")}</p>
                    }
                    .into_any()
                } else if on {
                    confirm_btn(
                        format!("/admin/distributions?confirm=disable&d={}", d.as_str()),
                        "minus",
                        "adm.dist.disable",
                        true,
                    )
                } else {
                    post_btn(
                        format!("/admin/distributions/{}/enable", d.as_str()),
                        "plus",
                        "adm.dist.enable",
                    )
                };
                vec![
                    dname(d),
                    if on {
                        tag("done", "check", t("adm.dist.state.enabled"))
                    } else {
                        tag("closed", "minus", t("adm.dist.state.available"))
                    },
                    if on { r.members.to_string() } else { "—".to_owned() }.into_any(),
                    if on && !r.endpoints.is_empty() {
                        r.endpoints.join(", ")
                    } else {
                        "—".to_owned()
                    }
                    .into_any(),
                    action,
                ]
            })
            .collect()
    });
    view! {
        {head("adm.sec.distributions", "adm.dist.lead")}
        {body}
        {note("info", "shield", t("adm.dist.note").into_any())}
    }
    .into_any()
}

/// S37 · Quem pode abrir cada Distribuição activada. Não são papéis.
fn access(dists: &[Distribution], rows: &[AdmAccessRowVm]) -> AnyView {
    let mut cols = vec![t("org.col.member").to_owned()];
    cols.extend(dists.iter().map(|d| dist_name(*d).to_owned()));
    let rows = rows.to_vec();
    let body = table(t("adm.sec.dist_access"), &cols, move |_| {
        rows.iter()
            .map(|r| {
                let mut cells = vec![r.name.clone().into_any()];
                cells.extend(r.cells.iter().map(|c| {
                    let state = if c.has {
                        tag("done", "check", t("adm.access.has"))
                    } else {
                        tag("closed", "minus", t("adm.access.none"))
                    };
                    let action = if c.has && c.protected {
                        refused_btn("minus", "adm.access.revoke", "adm-access-last")
                    } else if c.has {
                        confirm_btn(
                            format!(
                                "/admin/distribution-access?confirm=revoke&person={}&d={}",
                                r.person_id,
                                c.distribution.as_str()
                            ),
                            "minus",
                            "adm.access.revoke",
                            true,
                        )
                    } else {
                        post_btn(
                            format!(
                                "/admin/distribution-access/{}/{}/grant",
                                r.person_id,
                                c.distribution.as_str()
                            ),
                            "plus",
                            "adm.access.grant",
                        )
                    };
                    view! { {state}<span class="oc-adm-cell-action">{action}</span> }.into_any()
                }));
                cells
            })
            .collect()
    });
    view! {
        {head("adm.sec.dist_access", "adm.access.lead")}
        {body}
        <p class="oc-app-note" data-tone="info" id="adm-access-last">{icon("lock")}<span>{t("adm.access.last")}</span></p>
    }
    .into_any()
}

fn tls_tag(s: &str) -> AnyView {
    match s {
        "valid" => tag("done", "shield", t("adm.ep.tls.active")),
        "invalid" => tag("err", "warning", t("adm.ep.tls.invalid")),
        _ => tag("attention", "clock", t("adm.ep.tls.pending")),
    }
}

fn state_tag(s: &str) -> AnyView {
    match s {
        "active" => tag("done", "check", t("adm.ep.state.active")),
        "disabled" => tag("closed", "minus", t("adm.ep.state.disabled")),
        _ => tag("attention", "clock", t("adm.ep.state.unverified")),
    }
}

/// «Genérico» ou «Entra directamente em {distribuição}».
#[must_use]
pub fn binding_text(b: Option<Distribution>) -> String {
    match b {
        Some(d) => tf("adm.ep.binding.dist", &[("distribution", dist_name(d))]),
        None => t("adm.ep.binding.generic").to_owned(),
    }
}

fn host_code(h: &str) -> AnyView {
    view! { <code class="oc-adm-host">{h.to_owned()}</code> }.into_any()
}

/// S27 · Os pontos de acesso; S29/S30 · o detalhe de um.
fn endpoints(
    rows: &[AdmEndpointRowVm],
    open: Option<&AdmEndpointRowVm>,
    enabled: &[Distribution],
    manage: bool,
) -> AnyView {
    let cols = [
        t("adm.ep.col.host").to_owned(),
        t("adm.ep.col.binding").to_owned(),
        t("adm.dist.col.state").to_owned(),
        t("adm.ep.col.tls").to_owned(),
    ];
    let list = rows.to_vec();
    let body = table(t("adm.sec.endpoints"), &cols, move |_| {
        list.iter()
            .map(|e| {
                vec![
                    view! {
                        <a class="oc-adm-hostlink" href=format!("/admin/endpoints?open={}", e.id)>{host_code(&e.host)}</a>
                        {e.canonical.then(|| view! { " "<span class="oc-res-state" data-tone="progress">{t("adm.ep.canonical")}</span> })}
                    }
                    .into_any(),
                    binding_text(e.binding).into_any(),
                    state_tag(&e.state),
                    tls_tag(&e.tls),
                ]
            })
            .collect()
    });
    let detail = open.map(|e| {
        // S29: a resolução do nome é observada, nunca configurada. A referência
        // desenha «Ainda não observado»; o que a observação viu depois diz-se
        // com as duas frases Code (`adm.ep.dns.here`/`.elsewhere`).
        let dns = match e.dns.as_str() {
            "resolves_here" => tag("done", "check", t("adm.ep.dns.here")),
            "resolves_elsewhere" => tag("err", "warning", t("adm.ep.dns.elsewhere")),
            _ => tag("attention", "clock", t("adm.ep.dns.unknown")),
        };
        let external = (e.dns != "resolves_here").then(|| {
            note(
                "warn",
                "warning",
                view! { <strong>{t("adm.ep.external")}</strong>" "{tf("adm.ep.dns.body", &[("host", e.host.as_str())])} }
                    .into_any(),
            )
        });
        let current = e.binding.map_or("generic", Distribution::as_str);
        let options = std::iter::once(("generic", binding_text(None)))
            .chain(enabled.iter().map(|d| (d.as_str(), binding_text(Some(*d)))))
            .map(|(v, l)| view! { <option value=v selected={v == current}>{l}</option> })
            .collect_view();
        view! {
            <section class="oc-adm-detail" data-part="ep-detail" aria-labelledby="ep-detail-t">
                <h3 class="oc-adm-subtitle" id="ep-detail-t">{host_code(&e.host)}" · "{t("adm.ep.dns.title")}</h3>
                {external}
                <dl class="oc-res-meta">
                    <div class="oc-app-kv"><dt>{t("adm.ep.dns.observed")}</dt><dd>{dns}</dd></div>
                    <div class="oc-app-kv"><dt>{t("adm.ep.col.tls")}</dt><dd>{tls_tag(&e.tls)}</dd></div>
                    <div class="oc-app-kv"><dt>{t("adm.dist.col.state")}</dt><dd>{state_tag(&e.state)}</dd></div>
                </dl>
                {note("info", "shield", t("adm.ep.tls.note").into_any())}
                {manage.then(|| view! {
                    <div class="oc-adm-actions">
                        {post_btn(format!("/admin/endpoints/{}/observe", e.id), "refresh", "adm.ep.observe")}
                        {(e.state != "active").then(|| post_btn(format!("/admin/endpoints/{}/activate", e.id), "check", "adm.ep.activate"))}
                        {(e.state != "disabled").then(|| confirm_btn(format!("/admin/endpoints?confirm=disable&id={}", e.id), "minus", "adm.dist.disable", true))}
                    </div>
                    <form class="oc-adm-actions" method="get" action="/admin/endpoints">
                        <input type="hidden" name="confirm" value="bind" />
                        <input type="hidden" name="id" value=e.id.clone() />
                        <label class="oc-app-field"><span>{t("adm.ep.field.binding")}</span><select name="to">{options}</select></label>
                        <button type="submit" class="oc-app-btn" aria-label=t("adm.ep.bind.go")>{icon("link")}<span>{t("adm.ep.bind.go")}</span></button>
                    </form>
                })}
            </section>
        }
    });
    view! {
        {head("adm.sec.endpoints", "adm.ep.lead")}
        {manage.then(|| view! {
            <div class="oc-adm-actions">{confirm_btn("/admin/endpoints?add=1".to_owned(), "plus", "adm.ep.add", false)}</div>
        })}
        {body}
        {detail}
    }
    .into_any()
}

/// S33 · O ponto de partida efectivo por Distribuição; a camada da Instância
/// está adiada (FG-014) e a página di-lo.
fn defaults(dists: &[Distribution]) -> AnyView {
    let cols = [
        t("auth.distribution").to_owned(),
        t("adm.prov.col.effective").to_owned(),
        t("adm.prov.col.instance").to_owned(),
    ];
    let dists = dists.to_vec();
    let body = table(t("adm.sec.defaults"), &cols, move |_| {
        dists
            .iter()
            .map(|d| {
                vec![
                    dname(*d),
                    tf(
                        "desk.prov.distribution",
                        &[
                            ("distribution", dist_name(*d)),
                            (
                                "version",
                                crate::experience::distribution::DISTRIBUTION_DEFAULTS_VERSION
                                    .to_string()
                                    .as_str(),
                            ),
                        ],
                    )
                    .into_any(),
                    tag("closed", "minus", t("adm.prov.not_available")),
                ]
            })
            .collect()
    });
    view! { {head("adm.sec.defaults", "adm.prov.lead")}{body} }.into_any()
}

/// O corpo de uma secção D010 da Administração.
pub fn section(vm: &AdminD010Vm) -> impl IntoView {
    let body = match vm {
        AdminD010Vm::Distributions { rows, manage } => distributions(rows, *manage),
        AdminD010Vm::Access { dists, rows } => access(dists, rows),
        AdminD010Vm::Endpoints {
            rows,
            open,
            enabled,
            manage,
        } => endpoints(rows, open.as_ref(), enabled, *manage),
        AdminD010Vm::Defaults { dists } => defaults(dists),
    };
    view! { <div class="oc-adm-d010" data-part="adm-d010">{body}</div> }
}

fn fact(f: &AdmFact) -> AnyView {
    match f {
        AdmFact::Text(s) => s.clone().into_any(),
        AdmFact::Strong(s) => view! { <strong>{s.clone()}</strong> }.into_any(),
        AdmFact::Code(s) => host_code(s),
    }
}

/// A confirmação governada da D010 (S31, S32, S37-revoke, S38) — a marcação
/// da D006: factos imutáveis, a nota de auditoria, o foco inicial em Cancelar;
/// uma recusa conhecida tira o botão de confirmar.
pub fn confirm(vm: &AdmConfirmVm) -> impl IntoView {
    let facts = vm
        .facts
        .iter()
        .map(|(k, v)| view! { <div class="oc-app-kv"><dt>{k.clone()}</dt><dd>{fact(v)}</dd></div> })
        .collect_view();
    let hidden = vm
        .hidden
        .iter()
        .map(|(k, v)| view! { <input type="hidden" name=k.clone() value=v.clone() /> })
        .collect_view();
    view! {
        <div class="oc-overlay oc-overlay--center oc-org-confirm" data-open="" data-d010="" data-oc="org-confirm" role="alertdialog" aria-modal="true" aria-labelledby="d10c-t" aria-describedby="d10c-d">
            <form class="oc-dialog oc-org-confirm__box" method="post" action=vm.action.clone() data-danger="">
                <span class="oc-dialog__icon" aria-hidden="true">{icon(vm.icon)}</span>
                <h2 class="oc-dialog__title" id="d10c-t">{vm.title.clone()}</h2>
                <dl class="oc-org-confirm__facts">{facts}</dl>
                <p class="oc-dialog__body" id="d10c-d">{vm.body.clone()}</p>
                {hidden}
                {vm.refusal.clone().map(|r| view! { <p class="oc-app-note oc-org-refusal" data-tone="warn" role="alert">{icon("shield")}<span>{r}</span></p> })}
                <p class="oc-dialog__note">{t("adm.audit.note")}</p>
                <div class="oc-dialog__actions">
                    <a class="oc-btn-plain" href=vm.cancel.clone() data-oc="org-confirm-cancel">{t("app.cancel")}</a>
                    <span class="oc-dialog__spacer"></span>
                    {vm.refusal.is_none().then(|| view! { <button type="submit" class="oc-btn-line oc-btn-line--danger">{t(vm.ok_key)}</button> })}
                </div>
            </form>
        </div>
    }
}

/// S28 · Acrescentar um ponto de acesso: nasce «por verificar».
pub fn add_endpoint(vm: &AdmAddEndpointVm) -> impl IntoView {
    let options = std::iter::once(("generic", binding_text(None)))
        .chain(vm.dists.iter().map(|d| (d.as_str(), binding_text(Some(*d)))))
        .map(|(v, l)| {
            let checked = v == vm.binding;
            view! { <label class="oc-adm-radio"><input type="radio" name="binding" value=v checked=checked />{l}</label> }
        })
        .collect_view();
    let invalid = vm.error.is_some();
    view! {
        <div class="oc-overlay oc-overlay--center" data-open="" data-d010="" data-oc="org-confirm" role="dialog" aria-modal="true" aria-labelledby="d10a-t">
            <form class="oc-dialog oc-adm-form" method="post" action="/admin/endpoints">
                <h2 class="oc-dialog__title" id="d10a-t">{t("adm.ep.add.title")}</h2>
                <label class="oc-app-field">
                    <span>{t("adm.ep.field.host")}</span>
                    <input name="host" value=vm.host.clone() required="" autocomplete="off" spellcheck="false"
                        aria-invalid=invalid.then_some("true")
                        aria-describedby=if invalid { "d10a-e d10a-h" } else { "d10a-h" } />
                </label>
                {vm.error.map(|k| view! { <p id="d10a-e" class="oc-app-note" data-tone="warn" role="alert">{icon("warning")}<span>{t(k)}</span></p> })}
                <p id="d10a-h" class="oc-adm-sub">{t("adm.ep.field.host.hint")}</p>
                <fieldset class="oc-adm-fieldset">
                    <legend>{t("adm.ep.field.binding")}</legend>
                    {options}
                </fieldset>
                <div class="oc-dialog__actions">
                    <a class="oc-btn-plain" href="/admin/endpoints" data-oc="org-confirm-cancel">{t("app.cancel")}</a>
                    <span class="oc-dialog__spacer"></span>
                    <button type="submit" class="oc-btn-gold">{t("adm.ep.save")}</button>
                </div>
            </form>
        </div>
    }
}
