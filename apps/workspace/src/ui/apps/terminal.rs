//! D008-A · Ocinye Terminal (`/terminal`). DESIGN_LOCKED.
//!
//! **ocsh is the native governed command shell of Ocinye OS. ocsh does not
//! expose unrestricted host shell access. It operates Ocinye capabilities
//! through the Core.** (ADR-0312)
//!
//! O ecrã é uma sessão: cromado (contexto, estado do Core, Procurar, Limpar
//! ecrã, Ajuda), o registo da sessão (`role="log"`) e a linha de comandos. O
//! parse que decide, a autorização e a execução são do Core
//! (`POST /api/v1/commands/exec`, via `POST /terminal/exec`); aqui não há parse,
//! nem autoridade, nem execução, nem processo. `static/oc-terminal.js` envia a
//! linha e desenha os blocos com nós de texto.
//!
//! Sem separadores nem painéis divididos (SingleInstance; DEFERRED). Sem
//! «sessão de administração», sem inspector, sem «escreva REVOGAR»: a
//! confirmação de alto impacto é a confirmação partilhada D006 sobre um plano
//! congelado do Core (TERMINAL-11, contrato futuro). Um comando desconhecido é
//! 127 e nunca vai para a Nye; só `nye ask …` / `? …` a chamam.

use leptos::prelude::*;

use super::frame;
use crate::i18n::{t, tf};
use crate::ui::components::icon;
use crate::ui::view_models::{
    risk_key, TermBlockVm, TermEntryVm, TermGroup, TermPlanVm, TermRegistryEntryVm, TermTone,
    TerminalVm,
};

fn status(exit: Option<u8>, ms: Option<u64>, cap: Option<&str>) -> AnyView {
    let Some(code) = exit else {
        return view! { <span class="oc-term-st" data-tone="info">{icon("clock")}<span>{t("term.confirm.waiting")}</span></span> }.into_any();
    };
    let tone = TermTone::of_exit(code);
    view! {
        <span class="oc-term-st" data-tone=tone.id()>{icon(tone.icon())}<span>{t(exit_key(code))}</span><span class="oc-term-st__code">{tf("term.exit", &[("code", code.to_string().as_str())])}</span></span>
        {ms.map(|m| view! { <span>{tf("term.ms", &[("ms", m.to_string().as_str())])}</span> })}
        {cap.map(|c| view! { <code class="oc-term-st__code">{c.to_owned()}</code> })}
    }
    .into_any()
}

const fn exit_key(code: u8) -> &'static str {
    match code {
        0 => "term.exit.0",
        2 => "term.exit.2",
        69 => "term.exit.69",
        77 => "term.exit.77",
        126 => "term.exit.126",
        127 => "term.exit.127",
        130 => "term.exit.130",
        _ => "term.exit.1",
    }
}

fn block(b: &TermBlockVm) -> AnyView {
    match b {
        TermBlockVm::Note { tone, title, body, suggestions } => view! {
            <div class="oc-term-note" data-tone=tone.id()>
                {icon(tone.icon())}
                <p class="oc-term-note__t">{title.clone()}</p>
                {body.clone().map(|b| view! { <p class="oc-term-note__b">{b}</p> })}
                {(!suggestions.is_empty()).then(|| view! {
                    <ul class="oc-term-note__sugg">{suggestions.iter().map(|s| view! { <li><button type="button" class="oc-term-sugg" data-oc="term-suggest">{s.clone()}</button></li> }).collect_view()}</ul>
                })}
            </div>
        }.into_any(),
        TermBlockVm::Table { columns, rows, pipeline, empty } => view! {
            {pipeline.clone().map(|p| view! { <p class="oc-term-pipe">{p}</p> })}
            {match empty {
                Some(e) => view! { <p class="oc-term-pipe">{e.clone()}</p> }.into_any(),
                None => view! {
                    <div class="oc-term-x"><table class="oc-term-table">
                        <thead><tr>{columns.iter().map(|c| view! { <th scope="col">{c.clone()}</th> }).collect_view()}</tr></thead>
                        <tbody>{rows.iter().map(|r| view! { <tr>{r.iter().map(|c| view! { <td>{visible(c)}</td> }).collect_view()}</tr> }).collect_view()}</tbody>
                    </table></div>
                }.into_any(),
            }}
        }.into_any(),
        TermBlockVm::Facts { rows } => view! {
            <dl class="oc-term-facts">{rows.iter().map(|(k, v)| view! { <dt>{k.clone()}</dt><dd>{visible(v)}</dd> }).collect_view()}</dl>
        }.into_any(),
        TermBlockVm::Help { groups, footer } => view! {
            <dl class="oc-term-help">{groups.iter().map(|(g, items)| view! {
                <dt class="oc-term-help__group">{g.clone()}</dt>
                {items.iter().map(|(u, d)| view! { <dt>{u.clone()}</dt><dd>{d.clone()}</dd> }).collect_view()}
            }).collect_view()}</dl>
            {footer.clone().map(|f| view! { <p class="oc-term-pipe">{f}</p> })}
        }.into_any(),
        TermBlockVm::Links { items, note } => view! {
            <ul class="oc-res-links">{items.iter().map(|l| view! {
                <li><a class="oc-res-link" href=l.href.clone() data-part="res-link">
                    <span class="oc-res-link__ic" aria-hidden="true">{icon(l.icon)}</span>
                    <span class="oc-res-link__main"><span class="oc-res-link__title">{visible(&l.title)}</span><span class="oc-res-link__meta"><span>{tf("term.link.open", &[("app", l.app_label.as_str())])}</span></span></span>
                </a></li>
            }).collect_view()}</ul>
            <p class="oc-term-pipe">{note.clone()}</p>
        }.into_any(),
        TermBlockVm::Nye { paragraphs, sources } => view! {
            <p class="oc-term-pipe">{t("term.nye.asked")}</p>
            <section class="oc-term-card" data-kind="nye">
                <p class="oc-term-card__h">{icon("nye")}<span>{t("term.nye.label")}</span></p>
                {paragraphs.iter().map(|p| view! { <p>{visible(p)}</p> }).collect_view()}
                {(!sources.is_empty()).then(|| view! { <p class="oc-term-card__note">{t("term.nye.sources")}{": "}<code>{sources.join(", ")}</code></p> })}
                <p class="oc-term-card__note">{t("term.nye.note")}</p>
            </section>
        }.into_any(),
        TermBlockVm::Receipt(r) => view! {
            <section class="oc-term-card" data-kind="receipt" role="status">
                <p class="oc-term-card__h">{icon("check")}<span>{t("term.receipt.title")}</span></p>
                <dl class="oc-term-facts">
                    <dt>{t("term.receipt.id")}</dt><dd>{r.id.clone()}</dd>
                    <dt>{t("term.receipt.at")}</dt><dd>{r.at.clone()}</dd>
                    <dt>{t("term.confirm.k.capability")}</dt><dd>{r.capability.clone()}</dd>
                </dl>
                {r.audit_href.clone().map(|h| view! { <a class="oc-app-btn" href=h>{icon("audit")}<span>{t("term.receipt.audit")}</span></a> })}
            </section>
        }.into_any(),
    }
}

/// Caracteres de controlo (C0, C1, ESC, bidi) ficam visíveis; nunca interpretados.
/// Igual a `visible()` em `oc-terminal.js`; um teste compara os dois.
pub use crate::ui::components::visible;

fn entry(e: &TermEntryVm) -> impl IntoView {
    view! {
        <li class="oc-term-entry" data-part="term-entry" data-exit=e.exit.map(|x| x.to_string()).unwrap_or_default()>
            <p class="oc-term-echo">
                <span class="oc-term-echo__ctx">{e.context_label.clone()}</span>
                <span class="oc-term-echo__line">{visible(&e.echo)}</span>
                <span class="oc-term-echo__meta">{status(e.exit, e.ms, e.capability.as_deref())}</span>
            </p>
            <div class="oc-term-out">{e.blocks.iter().map(block).collect_view()}</div>
        </li>
    }
}

fn registry(r: &[TermRegistryEntryVm]) -> impl IntoView {
    view! { <template data-part="term-registry">{r.iter().map(|x| view! { <i data-cmd=x.completion.clone() data-group=x.group.key()>{t(x.help_key)}</i> }).collect_view()}</template> }
}

/// A ajuda imediata a partir do registo desta pessoa (a mesma que `help` devolve do Core).
#[must_use]
pub fn help_groups(r: &[TermRegistryEntryVm]) -> Vec<(String, Vec<(String, String)>)> {
    let order = [
        TermGroup::Shell,
        TermGroup::Workspace,
        TermGroup::Work,
        TermGroup::Ai,
        TermGroup::System,
        TermGroup::Admin,
    ];
    order
        .iter()
        .filter_map(|g| {
            let items: Vec<_> = r
                .iter()
                .filter(|x| x.group == *g)
                .map(|x| (x.usage.clone(), t(x.help_key).to_owned()))
                .collect();
            (!items.is_empty()).then(|| (t(g.key()).to_owned(), items))
        })
        .collect()
}

/// TERMINAL-11 · o diálogo partilhado D006 sobre o plano congelado. Confirmar envia só `plan`.
#[must_use]
pub fn confirm(p: &TermPlanVm) -> impl IntoView {
    let kv = |k: &'static str, v: AnyView| view! { <div class="oc-app-kv"><dt>{t(k)}</dt><dd>{v}</dd></div> };
    view! {
        <div class="oc-overlay oc-overlay--center oc-org-confirm" data-open="" data-oc="org-confirm" role="alertdialog" aria-modal="true" aria-labelledby="oc-term-confirm-t" aria-describedby="oc-term-confirm-d">
            <form class="oc-dialog oc-org-confirm__box" method="post" action="/terminal/plans/approve" data-kind="ocsh.plan" data-danger="">
                <input type="hidden" name="plan" value=p.id.clone()/>
                <span class="oc-dialog__icon" aria-hidden="true">{icon("shield")}</span>
                <h2 class="oc-dialog__title" id="oc-term-confirm-t" data-part="term-confirm-title">{tf("term.confirm.title", &[("action", p.action_label.as_str())])}</h2>
                <dl class="oc-org-confirm__facts">
                    {kv("term.confirm.k.command", view! { <code data-part="term-confirm-line">{visible(&p.echo)}</code> }.into_any())}
                    {kv("term.confirm.k.target", view! { <span data-part="term-confirm-target">{p.target_label.clone()}</span> }.into_any())}
                    {kv("term.confirm.k.capability", view! { <code data-part="term-confirm-cap">{p.capability.clone()}</code> }.into_any())}
                    {kv("term.confirm.k.risk", view! { <span data-part="term-confirm-risk">{t(risk_key(p.risk))}</span> }.into_any())}
                    {kv("term.confirm.k.context", view! { <span data-part="term-confirm-ctx">{p.context_label.clone()}</span> }.into_any())}
                    {kv("term.confirm.k.plan", view! { <code data-part="term-confirm-plan">{p.id_short.clone()}</code> }.into_any())}
                    {kv("term.confirm.k.valid", view! { <span data-part="term-confirm-valid">{p.valid_until.clone()}</span> }.into_any())}
                </dl>
                <p class="oc-dialog__body" id="oc-term-confirm-d">{t("term.confirm.body")}</p>
                <p class="oc-dialog__note">{t("term.confirm.core")}</p>
                <div class="oc-dialog__actions">
                    <a class="oc-btn-plain" href="/terminal" data-oc="org-confirm-cancel" autofocus="">{t("app.cancel")}</a>
                    <span class="oc-dialog__spacer"></span>
                    <button type="submit" class="oc-btn-line oc-btn-line--danger" data-oc="org-confirm-ok">{t("term.confirm.do")}</button>
                </div>
            </form>
        </div>
    }
}

/// O ecrã do Terminal.
#[must_use]
pub fn terminal(vm: &TerminalVm) -> AnyView {
    let ctx = vm.context.label.clone();
    let off = !vm.core_online;
    let toolbar = view! {
        <h2 class="oc-app__title">{t("terminal.app")}</h2>
        <span class="oc-term-ctx" role="status" aria-label=tf("term.context.aria", &[("context", ctx.as_str())])>
            <span class="oc-term-ctx__k" aria-hidden="true">{t("term.context")}</span>
            <span class="oc-term-ctx__v" data-part="term-ctx" aria-hidden="true">{ctx.clone()}</span>
        </span>
        <span class="oc-term-sess" data-state=if off { "offline" } else { "online" }>{t(if off { "terminal.status.disconnected" } else { "terminal.status.connected" })}</span>
        <span class="oc-app__spacer"></span>
        <button type="button" class="oc-app-btn" data-oc="term-find" aria-pressed="false" aria-controls="oc-term-find" aria-label=t("term.search")>{icon("search")}<span>{t("term.search")}</span></button>
        <button type="button" class="oc-app-btn" data-oc="term-clear" aria-label=t("term.clear")>{icon("minus")}<span>{t("term.clear")}</span></button>
        <button type="button" class="oc-app-btn" data-oc="term-help" aria-label=t("term.help")>{icon("help")}<span>{t("term.help")}</span></button>
    }
    .into_any();
    let welcome_hint = t("term.welcome.hint");
    let main = view! {
        <div class="oc-term" data-oc="term"
            data-context=vm.context.id.clone().unwrap_or_default()
            data-context-label=ctx.clone()
            data-core=if off { "offline" } else { "online" }
            data-t-nye=t("term.nye.label") data-t-nye-note=t("term.nye.note")
            data-t-exit=t("term.exit") data-t-ms=t("term.ms")
            data-t-cleared=t("term.clear.note") data-t-history-empty=t("term.history.empty") data-t-history-note=t("term.history.note")
            data-t-offline=t("term.prompt.offline") data-t-network=t("ocsh.err.network") data-t-no-session=t("ocsh.denied") data-t-running=t("terminal.running")
            data-exit0=t("term.exit.0") data-exit1=t("term.exit.1") data-exit2=t("term.exit.2") data-exit69=t("term.exit.69") data-exit77=t("term.exit.77") data-exit126=t("term.exit.126") data-exit127=t("term.exit.127") data-exit130=t("term.exit.130")>
            <div class="oc-term-find" id="oc-term-find" data-part="term-find" role="search" hidden="" data-of=t("term.search.n") data-none=t("term.search.none")>
                <label class="oc-app-field"><span class="oc-sr">{t("term.search.field")}</span><input type="search" placeholder=t("term.search.field") autocomplete="off"/></label>
                <span class="oc-term-find__n" data-part="term-find-n"></span>
                <button type="button" class="oc-app-btn" data-oc="term-find-prev" aria-label=t("term.search.prev")>{icon("chev-u")}</button>
                <button type="button" class="oc-app-btn" data-oc="term-find-next" aria-label=t("term.search.next")>{icon("chev-d")}</button>
                <button type="button" class="oc-app-btn" data-oc="term-find" aria-label=t("term.search.close")>{icon("close")}</button>
                <p class="oc-term-find__note">{t("term.search.local")}</p>
            </div>
            <div class="oc-term-scroll" data-part="term-scroll" tabindex="0" role="log" aria-label=t("term.output")>
                <ol class="oc-term-log" data-part="term-log">
                    <li class="oc-term-welcome" data-part="term-welcome">
                        <p class="oc-term-welcome__name">"Ocinye Terminal"<span class="oc-term-welcome__ver">{format!("ocsh {}", vm.version)}</span></p>
                        <p>{t("term.statement")}" "{t("term.statement.2")}</p>
                        <p>{welcome_hint}</p>
                    </li>
                    {vm.scrollback.iter().map(entry).collect_view()}
                </ol>
            </div>
            <div class="oc-term-foot">
                <section class="oc-term-paste" data-part="term-paste" hidden="" data-title=t("terminal.paste.title")>
                    <p class="oc-term-paste__t" data-part="term-paste-n"></p>
                    <p>{t("term.paste.body")}</p>
                    <ol></ol>
                    <div class="oc-term-paste__acts">
                        <button type="button" class="oc-app-btn" data-oc="term-paste-next">{icon("arrow-r")}<span>{t("term.paste.next")}</span></button>
                        <button type="button" class="oc-app-btn" data-oc="term-paste-discard">{icon("close")}<span>{t("term.paste.discard")}</span></button>
                    </div>
                </section>
                <ul class="oc-term-complete" id="oc-term-complete" data-part="term-complete" role="listbox" aria-label=t("term.complete.aria") hidden="">
                    <li class="oc-term-complete__note" role="presentation">{t("term.complete.note")}</li>
                </ul>
                <form class="oc-term-prompt" data-part="term-prompt" method="post" action="/terminal/exec" data-offline=off.then_some("")>
                    <span class="oc-term-prompt__ctx" data-part="term-ctx" aria-hidden="true">{ctx.clone()}</span>
                    <input id="oc-term-in" data-part="term-input" name="line" type="text" autocomplete="off" autocapitalize="off" spellcheck="false" enterkeyhint="send" maxlength="4096"
                        role="combobox" aria-autocomplete="list" aria-controls="oc-term-complete" aria-expanded="false"
                        aria-label=t("term.prompt.label")
                        placeholder=t(if off { "term.prompt.offline" } else { "term.prompt.placeholder" })
                        disabled=off/>
                    <button type="submit" class="oc-app-primary oc-term-run" aria-label=t("term.run") disabled=off>{icon("arrow-r")}<span>{t("term.run")}</span></button>
                </form>
                <p class="oc-term-keys"><span>{t("term.keys")}</span></p>
            </div>
            {registry(&vm.registry)}
            <template data-part="term-confirm" data-title=t("term.confirm.title")></template>
            <p class="oc-sr" role="status" aria-live="polite" data-part="term-live"></p>
        </div>
    }
    .into_any();
    frame(
        "terminal",
        t("terminal.app").to_owned(),
        toolbar,
        None,
        main,
        None,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::view_models::{TermContextVm, TermGroup};

    fn vm() -> TerminalVm {
        TerminalVm {
            context: TermContextVm {
                id: None,
                label: "pessoal".into(),
            },
            core_online: true,
            version: "1.0",
            registry: vec![TermRegistryEntryVm {
                usage: "whoami".into(),
                completion: "whoami".into(),
                help_key: "ocsh.cmd.whoami",
                group: TermGroup::Workspace,
            }],
            scrollback: vec![],
        }
    }
    fn html(v: &TerminalVm) -> String {
        terminal(v).to_html()
    }

    /// D008 §232: nada no ecrã sugere uma shell do anfitrião nem uma execução genérica.
    #[test]
    fn sem_shell_do_anfitriao_nem_exec_generico() {
        let h = html(&vm());
        for proibido in [
            "/bin/sh",
            "bash -c",
            "exec(",
            "data-oc=\"term-exec\"",
            "Runtime.exec",
            "host-shell",
            "sudo",
        ] {
            assert!(!h.contains(proibido), "{proibido}");
        }
        assert!(h.contains("action=\"/terminal/exec\""), "só o BFF tipado");
    }

    #[test]
    fn a_frase_canonica_esta_no_ecra() {
        assert!(html(&vm()).contains(t("term.statement.2")));
    }

    #[test]
    fn sem_core_o_prompt_nao_envia() {
        let mut v = vm();
        v.core_online = false;
        let h = html(&v);
        assert!(h.contains("data-offline"));
        assert!(h.matches("disabled").count() >= 2);
    }

    #[test]
    fn controlo_visivel_nunca_interpretado() {
        assert_eq!(visible("\u{1b}[2Jx"), "␛[2Jx");
        assert_eq!(visible("\u{202e}exe"), "⟨U+202E⟩exe");
        assert_eq!(visible("a\u{7}b"), "a␇b");
    }

    /// O eco é a linha redigida; um segredo nunca entra na sessão desenhada.
    #[test]
    fn o_eco_e_redigido() {
        let mut v = vm();
        v.scrollback.push(TermEntryVm {
            echo: ocinye_contracts::ocsh::redact("nye ask x --token abc.def"),
            context_label: "pessoal".into(),
            exit: Some(2),
            ms: Some(1),
            capability: None,
            blocks: vec![],
        });
        assert!(!html(&v).contains("abc.def"));
    }

    /// Um comando desconhecido desenha-se como o Core o devolve (127), sem bloco da Nye.
    #[test]
    fn desconhecido_nao_vira_nye() {
        let mut v = vm();
        v.scrollback.push(TermEntryVm {
            echo: "foobar".into(),
            context_label: "pessoal".into(),
            exit: Some(127),
            ms: Some(1),
            capability: None,
            blocks: vec![TermBlockVm::Note {
                tone: TermTone::Warn,
                title: "x".into(),
                body: None,
                suggestions: vec!["help".into()],
            }],
        });
        let h = html(&v);
        assert!(!h.contains("data-kind=\"nye\""));
        assert!(h.contains("data-exit=\"127\""));
    }

    #[test]
    fn confirmacao_so_envia_o_plano() {
        let p = TermPlanVm {
            id: "7f3c".into(),
            id_short: "7f3c".into(),
            action_label: "a".into(),
            echo: "projects archive X".into(),
            capability: "research.project.archive".into(),
            risk: ocinye_contracts::agentic::RiskLevel::MaterialMutation,
            context_label: "c".into(),
            target_label: "X".into(),
            valid_until: "10:57".into(),
        };
        let h = confirm(&p).to_html();
        assert!(h.contains("name=\"plan\""));
        assert!(
            !h.contains("name=\"line\""),
            "a linha não é relida depois da confirmação"
        );
        assert!(
            !h.contains("REVOGAR") && !h.contains("type=\"text\""),
            "sem palavra escrita"
        );
    }
}
