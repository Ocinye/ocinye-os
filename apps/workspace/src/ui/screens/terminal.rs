//! O Ocinye Terminal — a casca da aplicação (D14.1) com a execução no Core.
//!
//! # O que este ecrã é
//!
//! A marcação do D14 (`docs/ui/D14_TERMINAL.md`; o pacote chegou como «D13» e o Design renumerou-o): separadores, viewport,
//! prompt, barra de estado e preferências. O comportamento vive em
//! `/static/terminal.js`; cada linha vai ao Core por `POST /terminal/exec`, e o
//! que volta é desenhado com nós de texto.
//!
//! # O que este ecrã não é
//!
//! Uma shell do anfitrião (ADR-0312). E não mostra o que ainda não funciona:
//! divisão de painéis, inspector, elevação e streaming esperam os seus
//! contratos (G-12, G-14) e o Gestor de Janelas (D5), e por isso não têm aqui
//! botões.

use leptos::prelude::*;

use crate::i18n::{t, tf};
use crate::terminal::OCSH_VERSION;

/// O que o ecrã precisa de saber de quem o abre.
#[derive(Debug, Clone)]
pub struct TerminalView {
    /// `membro` no prompt `membro@instância`.
    pub who: String,
    /// `instância` no prompt.
    pub instance: String,
}

fn icon(id: &'static str) -> impl IntoView {
    crate::ui::ods::icone(id, "")
}

/// Uma linha das preferências.
fn pref(
    label: &'static str,
    desc: Option<&'static str>,
    control: impl IntoView + 'static,
) -> impl IntoView {
    view! {
        <div class="ods-term-prefs__row">
            <div class="ods-term-prefs__label">
                {t(label)}
                {desc.map(|d| view! { <div class="ods-term-prefs__desc">{t(d)}</div> })}
            </div>
            {control}
        </div>
    }
}

/// Um grupo de opções exclusivas, gravado localmente pelo `terminal.js`.
fn escolha(
    pref_id: &'static str,
    label: &'static str,
    opts: &'static [(&'static str, &'static str)],
    default: &'static str,
) -> impl IntoView {
    view! {
        <fieldset class="ods-seg" role="radiogroup">
            <legend class="ods-sr-only">{t(label)}</legend>
            {opts
                .iter()
                .map(|(value, key)| {
                    view! {
                        <label class="ods-seg__opt">
                            <input
                                class="ods-radio"
                                type="radio"
                                name=format!("term-{pref_id}")
                                value=*value
                                checked=*value == default
                                data-oc="term-pref"
                                data-pref=pref_id
                            />
                            <span>{t(key)}</span>
                        </label>
                    }
                })
                .collect_view()}
        </fieldset>
    }
}

fn interruptor(pref_id: &'static str, label: &'static str) -> impl IntoView {
    view! {
        <input
            class="ods-switch"
            type="checkbox"
            role="switch"
            aria-label=t(label)
            data-oc="term-pref"
            data-pref=pref_id
        />
    }
}

/// O ecrã.
pub fn terminal(v: &TerminalView) -> impl IntoView {
    let personal = t("terminal.context.personal");
    let who = format!("{}@{}", v.who, v.instance);
    let who_attr = who.clone();
    let powered = tf(
        "terminal.welcome.powered",
        &[
            ("version", OCSH_VERSION),
            ("instance", &v.instance),
            ("context", personal),
        ],
    );
    view! {
        <section
            class="ods-term"
            data-oc="terminal"
            data-theme="dark"
            data-density="normal"
            data-cursor="block"
            data-who=who_attr
            data-personal=personal
            data-t-tab=t("terminal.tab.default")
            data-t-tab-close=t("terminal.tab.close")
            data-t-running=t("terminal.running.meta")
            data-t-search=t("terminal.mode.search")
            data-t-rs-title=t("terminal.rs.title")
            data-t-rs-none=t("terminal.rs.none")
            data-t-rs-hint=t("terminal.rs.hint")
            data-t-ac-hint=t("terminal.ac.hint")
            data-t-offline=t("terminal.offline.placeholder")
            data-t-network=t("ocsh.err.network")
            data-t-output=t("terminal.output.aria")
            data-t-line=t("terminal.line.aria")
        >
            <div class="ods-term__tabs">
                <div class="ods-term__tablist" role="tablist" data-oc="term-tabs"></div>
                <button class="ods-term__tool" type="button" data-oc="term-tab-new" aria-label=t("terminal.tab.new") title=t("terminal.tab.new")>
                    {icon("plus")}
                </button>
                <span class="ods-term__spacer"></span>
                <button class="ods-term__tool" type="button" data-oc="term-prefs" aria-label=t("terminal.prefs.title") title=t("terminal.prefs.title")>
                    {icon("settings")}
                </button>
            </div>

            <div class="ods-term__alert" data-oc="term-offline" role="alert" hidden>
                {icon("offline")}
                <strong>{t("terminal.offline.title")}</strong>
                <span>{t("terminal.reconnecting")}</span>
            </div>

            <div class="ods-term__body">
                <div class="ods-term__panes" data-split="row" data-oc="term-panes"></div>
            </div>

            <div class="ods-term__status" data-oc="term-status">
                <span class="ods-term__conn" data-state="ok" data-oc="term-conn">
                    <i></i>
                    <span data-part="conn-label">{t("terminal.status.connected")}</span>
                </span>
                <span data-part="status-context">{personal}</span>
                <span class="ods-term__spacer"></span>
                <span>{format!("ocsh {OCSH_VERSION}")}</span>
            </div>

            // O molde de uma sessão: clonado por cada separador.
            <template data-oc="term-session-template">
                <div class="ods-term-pane" data-oc="term-pane">
                    <div class="ods-term-pane__view" data-ocs="" role="log" aria-live="polite" aria-label=t("terminal.output.aria")>
                        <div class="ods-term-entry" data-part="welcome">
                            <div class="ods-term-out" data-kind="lines">
                                <div class="ods-term-out__line ods-term-out__title">{t("terminal.welcome.title")}</div>
                                <div class="ods-term-out__line"><span class="ods-term-seg" data-tone="dim">{powered}</span></div>
                                <div class="ods-term-out__line"><span class="ods-term-seg" data-tone="fg">{t("terminal.welcome.start")}</span></div>
                                {["help", "whoami", "context list", "nodes list"]
                                    .into_iter()
                                    .map(|c| view! {
                                        <div class="ods-term-out__line" data-indent="2">
                                            <button type="button" class="ods-term-seg" data-tone="key" data-go="run" data-oc="term-go" data-line=c>{c}</button>
                                        </div>
                                    })
                                    .collect_view()}
                            </div>
                        </div>
                        <div class="ods-term-input" data-oc="term-input" data-mode="normal">
                            <span class="ods-term-prompt" data-part="prompt">
                                <span class="ods-term-prompt__who">{who}</span>
                                <span class="ods-term-prompt__ctx">{personal}</span>
                                <span class="ods-term-prompt__cwd">"~"</span>
                                <span class="ods-term-prompt__caret">"›"</span>
                            </span>
                            <div class="ods-term-input__field">
                                <div class="ods-term-input__mirror" aria-hidden="true">
                                    <span class="ods-term-input__typed"></span>
                                    <span class="ods-term-input__ghost"></span>
                                    <span class="ods-term-input__cursor"></span>
                                </div>
                                <textarea rows="1" spellcheck="false" autocomplete="off" autocapitalize="off" aria-label=t("terminal.line.aria") data-oc="term-line"></textarea>
                            </div>
                        </div>
                    </div>
                </div>
            </template>

            <div class="ods-term-prefs__scrim" data-oc="term-prefs-close" hidden></div>
            <aside class="ods-term-prefs" data-oc="term-prefs-sheet" aria-label=t("terminal.prefs.title") hidden>
                <div class="ods-term-prefs__head">
                    <div>
                        <div class="ods-term-prefs__title">{t("terminal.prefs.title")}</div>
                        <div class="ods-term-prefs__sub">{t("terminal.prefs.sub")}</div>
                    </div>
                    <span class="ods-term__spacer"></span>
                    <button class="ods-term__tool" type="button" data-oc="term-prefs-close" aria-label=t("terminal.cancel")>
                        {icon("close")}
                    </button>
                </div>
                <div class="ods-term-prefs__body">
                    <section class="ods-term-prefs__sec">
                        <h3>{t("terminal.prefs.appearance")}</h3>
                        {pref("terminal.prefs.theme", Some("terminal.prefs.theme.desc"),
                            escolha("theme", "terminal.prefs.theme", &[("dark", "terminal.prefs.theme.dark"), ("light", "terminal.prefs.theme.light")], "dark"))}
                        {pref("terminal.prefs.font_size", None, view! {
                            <input type="range" min="11" max="18" step="1" value="13"
                                aria-label=t("terminal.prefs.font_size") data-oc="term-pref" data-pref="fs" />
                        })}
                        {pref("terminal.prefs.cursor", None,
                            escolha("cursor", "terminal.prefs.cursor", &[("block", "terminal.prefs.cursor.block"), ("bar", "terminal.prefs.cursor.bar"), ("under", "terminal.prefs.cursor.under")], "block"))}
                        {pref("terminal.prefs.density", None,
                            escolha("density", "terminal.prefs.density", &[("compact", "terminal.prefs.density.compact"), ("normal", "terminal.prefs.density.normal"), ("comfy", "terminal.prefs.density.comfy")], "normal"))}
                    </section>
                    <section class="ods-term-prefs__sec">
                        <h3>{t("terminal.prefs.output")}</h3>
                        {pref("terminal.prefs.show_duration", None, interruptor("dur", "terminal.prefs.show_duration"))}
                        {pref("terminal.prefs.show_exit", Some("terminal.prefs.show_exit.desc"), interruptor("exit", "terminal.prefs.show_exit"))}
                    </section>
                    <section class="ods-term-prefs__sec">
                        <h3>{t("terminal.prefs.history")}</h3>
                        {pref("terminal.prefs.history.secrets", Some("terminal.prefs.history.secrets.desc"), view! {
                            <span class="ods-term-prefs__locked">{icon("lock")}"Core"</span>
                        })}
                    </section>
                    <section class="ods-term-prefs__sec">
                        <h3>{t("terminal.prefs.security")}</h3>
                        {pref("terminal.prefs.confirmations", Some("terminal.prefs.confirmations.desc"), view! {
                            <span class="ods-term-prefs__locked">{icon("lock")}"Core"</span>
                        })}
                    </section>
                </div>
            </aside>
        </section>
        <script src="/static/terminal.js" defer></script>
    }
}
