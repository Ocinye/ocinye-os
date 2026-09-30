//! D008-B · Ocinye Browser (`/browser`). DESIGN_LOCKED.
//!
//! **Cromado do Browser = interface de confiança do Ocinye. Conteúdo web
//! externo = NÃO confiável.** Tudo o que é Ocinye (abas, barra, endereço,
//! origem, avisos, permissões, transferências, Nye) fica **acima** da faixa
//! `oc-brw-edge` («Conteúdo externo · origem») ou ao lado, num painel do
//! cromado. Nada do Ocinye se desenha dentro de `oc-brw-view`.
//!
//! Abas ≠ janelas: uma janela do Gestor de Janelas (D002) contém as abas; a aba
//! tem id interno (o URL não é identidade). O Browser Manager é do cliente
//! (`static/oc-browser.js`, ADR-0612) e, no Desktop/Dedicado, fala com a casca
//! pela ponte tipada `browser.*` (ADR-0703). Sem ponte, o estado é
//! `NoRuntime` — nunca um sucesso simulado.
//!
//! Web: `<iframe sandbox>` sem `allow-same-origin` nem `allow-top-navigation`,
//! `allow=""`, `referrerpolicy="no-referrer"`; só o endereço pedido; sem recuar/
//! avançar/título; recurso honesto «Abrir num separador do navegador». Nunca um
//! proxy, nunca injecção de JavaScript, nunca API de DOM, nunca ponte para o
//! ocsh. Sem pesquisa (nenhum fornecedor configurado), sem histórico, marcadores,
//! privado, gestor de palavras-passe, extensões ou automação.

use leptos::prelude::*;

use crate::i18n::{t, tf};
use crate::ui::components::icon;
use crate::ui::view_models::{
    BrowserDownloadDest, BrowserDownloadState, BrowserDownloadVm, BrowserExtractionKind,
    BrowserExtractionVm, BrowserNoticeVm, BrowserNyeVm, BrowserPageStateVm, BrowserRuntime,
    BrowserSecurity, BrowserSideVm, BrowserTabState, BrowserTabVm, BrowserVm,
};

/// O único `sandbox` que o Browser Web usa. Um teste falha se mudar.
pub const WEB_SANDBOX: &str =
    "allow-scripts allow-forms allow-popups allow-popups-to-escape-sandbox";

fn rt_key(r: BrowserRuntime) -> &'static str {
    match r {
        BrowserRuntime::Web => "brw.runtime.web",
        BrowserRuntime::Desktop => "brw.runtime.desktop",
        BrowserRuntime::Dedicated => "brw.runtime.dedicated",
    }
}

fn tab(x: &BrowserTabVm, i: usize, active: bool) -> impl IntoView {
    let st = match x.state {
        BrowserTabState::New => "new",
        BrowserTabState::Ready => "ready",
        BrowserTabState::Loading => "loading",
        BrowserTabState::Failed => "failed",
    };
    view! {
        <div class="oc-brw-tab" role="presentation" data-state=st>
            <button type="button" role="tab" class="oc-brw-tab__sel" id=format!("oc-brw-tab-{i}") data-tab=x.id.clone() aria-selected=active.to_string() aria-controls="oc-brw-view" tabindex=if active { "0" } else { "-1" } data-oc="brw-activate">
                {icon(if x.state == BrowserTabState::New { "plus" } else { "ob-globe" })}
                <span class="oc-brw-tab__t">{x.title.clone()}</span>
                {match x.state {
                    BrowserTabState::Loading => Some(view! { <span class="oc-brw-tab__st">{t("brw.tab.loading")}</span> }),
                    BrowserTabState::Failed => Some(view! { <span class="oc-brw-tab__st">{t("brw.tab.failed")}</span> }),
                    _ => None,
                }}
            </button>
            <button type="button" class="oc-brw-tab__x" data-oc="brw-close" data-tab=x.id.clone() aria-label=tf("brw.tab.close", &[("title", x.title.as_str())])>{icon("close")}</button>
        </div>
    }
}

fn external(url: &str, primary: bool) -> impl IntoView {
    view! {
        <a class=if primary { "oc-app-primary" } else { "oc-app-btn" } href=url.to_owned() target="_blank" rel="noopener noreferrer" data-part="brw-external">
            {icon("ob-ext")}<span>{t("brw.open_external")}</span>
        </a>
    }
}

fn state(
    tone: &'static str,
    ic: &'static str,
    title: String,
    body: Option<String>,
    acts: Option<AnyView>,
    small: Option<String>,
) -> AnyView {
    view! {
        <div class="oc-brw-state" data-tone=tone role=if tone == "err" { "alert" } else { "status" }>
            <span class="oc-brw-state__ic" aria-hidden="true">{icon(ic)}</span>
            <p class="oc-brw-state__t">{title}</p>
            {body.map(|b| view! { <p>{b}</p> })}
            {acts.map(|a| view! { <div class="oc-brw-state__acts">{a}</div> })}
            {small.map(|s| view! { <p class="oc-brw-state__small">{s}</p> })}
        </div>
    }
    .into_any()
}

fn page(vm: &BrowserVm) -> AnyView {
    let retry = || {
        view! { <button type="button" class="oc-app-btn" data-oc="brw-reload">{icon("ob-reload")}<span>{t("brw.fail.retry")}</span></button> }.into_any()
    };
    match &vm.page {
        BrowserPageStateVm::NewTab => view! {
            <div class="oc-brw-new"><p class="oc-brw-new__t">{t("brw.new.title")}</p><p>{t("brw.new.body")}</p><p>{t("brw.new.deferred")}</p></div>
        }.into_any(),
        // Web: o servidor desenha o iframe com o endereço pedido; o Desktop pinta o webview por baixo.
        BrowserPageStateVm::External { .. } => match vm.runtime {
            BrowserRuntime::Web => view! {
                <iframe title=tf("brw.edge", &[("origin", "")]) src=vm.navigation.url.clone() sandbox=WEB_SANDBOX referrerpolicy="no-referrer" allow=""></iframe>
            }.into_any(),
            _ => view! { <div class="oc-brw-native" data-part="brw-native-slot" aria-hidden="true"></div> }.into_any(),
        },
        BrowserPageStateVm::WebMayBeBlocked { url } => state("warn", "ob-warn", t("brw.web.blocked.title").into(), Some(t("brw.web.blocked.body").into()), Some(external(url, true).into_any()), Some(t("brw.open_external.note").into())),
        BrowserPageStateVm::Invalid { text } => state("warn", "ob-search", t("brw.invalid.title").into(), Some(tf("brw.invalid.body", &[("text", text.as_str())])), None, None),
        BrowserPageStateVm::BlockedScheme { scheme } => state("err", "ob-warn", tf("brw.scheme.title", &[("scheme", scheme.as_str())]), Some(t("brw.scheme.body").into()), None, None),
        BrowserPageStateVm::Failed { host } => state("err", "ob-warn", tf("brw.fail.title", &[("host", host.as_str())]), Some(t("brw.fail.dns").into()), Some(retry()), None),
        BrowserPageStateVm::Certificate { host } => state("err", "ob-lock", tf("brw.cert.title", &[("host", host.as_str())]), Some(t("brw.cert.body").into()), None, None),
        BrowserPageStateVm::Crashed => state("err", "ob-warn", t("brw.crash.title").into(), Some(t("brw.crash.body").into()), Some(view! { <button type="button" class="oc-app-btn" data-oc="brw-reload">{icon("ob-reload")}<span>{t("brw.reload")}</span></button> }.into_any()), None),
        BrowserPageStateVm::Loading { host } => state("", "ob-globe", format!("{} · {}", t("brw.tab.loading"), host), None, None, None),
        BrowserPageStateVm::NoRuntime => state("warn", "ob-pc", tf("brw.fail.title", &[("host", vm.navigation.host.as_deref().unwrap_or(""))]), Some(t("brw.web.note").into()), None, None),
    }
}

fn notice(n: &BrowserNoticeVm) -> AnyView {
    let row = |tone: &'static str, ic: &'static str, text: AnyView, acts: Option<AnyView>| {
        view! {
        <div class="oc-brw-notice" data-tone=tone role="status">{icon(ic)}<p class="oc-brw-notice__txt">{text}</p>{acts.map(|a| view! { <div class="oc-brw-notice__acts">{a}</div> })}</div>
    }.into_any()
    };
    let code = |s: &str| view! { <code>{s.to_owned()}</code> }.into_any();
    match n {
        BrowserNoticeVm::WebLimits { url } => row("info", "status", view! { {t("brw.web.note")} }.into_any(), Some(external(url, false).into_any())),
        BrowserNoticeVm::WebOpened { origin } => row("info", "ob-ext", view! { {tf("brw.web.opened", &[("origin", origin.as_str())])} }.into_any(), None),
        BrowserNoticeVm::WebPermissions => row("info", "ob-mic", view! { {t("brw.perm.web")} }.into_any(), None),
        BrowserNoticeVm::WebDownloads => row("info", "ob-dl", view! { {t("brw.dl.web")} }.into_any(), None),
        BrowserNoticeVm::PopupTab { origin } => row("info", "ob-plus", view! { {tf("brw.popup.tab", &[("origin", origin.as_str())])} }.into_any(), None),
        BrowserNoticeVm::PopupBlocked { origin } => row("warn", "ob-warn", view! { {tf("brw.popup.blocked", &[("origin", origin.as_str())])} }.into_any(), None),
        BrowserNoticeVm::FullscreenDenied { origin } => row("info", "ob-full", view! { {tf("brw.fullscreen.denied", &[("origin", origin.as_str())])} }.into_any(), None),
        BrowserNoticeVm::Idn { host } => row("warn", "ob-warn", view! { {t("brw.idn")}" "{code(host)} }.into_any(), None),
        // A origem exacta, nunca o título da página (D008 §115).
        BrowserNoticeVm::Permission(p) => view! {
            <div class="oc-brw-notice oc-brw-perm" data-tone="warn" role="alertdialog" aria-labelledby="oc-brw-perm-t" aria-describedby="oc-brw-perm-d">
                {icon(p.kind.icon())}
                <p class="oc-brw-notice__txt">
                    <strong id="oc-brw-perm-t">{tf("brw.perm.ask", &[("origin", p.origin.as_str()), ("what", t(p.kind.key()))])}</strong><br/>
                    <span id="oc-brw-perm-d">{t("brw.perm.body")}" "{t("brw.perm.scope")}</span>
                </p>
                <div class="oc-brw-notice__acts">
                    <button type="button" class="oc-app-btn" data-oc="brw-perm" data-decision="deny" data-request=p.request_id.clone()>{t("brw.perm.deny")}</button>
                    <button type="button" class="oc-app-btn" data-oc="brw-perm" data-decision="once" data-request=p.request_id.clone()>{t("brw.perm.once")}</button>
                </div>
            </div>
        }.into_any(),
    }
}

fn side_head(k: &'static str, ic: &'static str, part: &'static str) -> impl IntoView {
    view! {
        <h3 class="oc-brw-side__h" id="oc-brw-side-t">{icon(ic)}<span>{t(k)}</span><span class="oc-app__spacer"></span>
            <button type="button" class="oc-brw-tool" data-oc="brw-side" data-side=part aria-label=t("brw.nye.close")>{icon("close")}</button></h3>
    }
}

fn extraction(x: &BrowserExtractionVm) -> impl IntoView {
    view! {
        <section class="oc-brw-side__box" aria-labelledby="oc-brw-nye-what">
            <p id="oc-brw-nye-what"><strong>{t("brw.nye.what")}</strong></p>
            <dl>
                <dt>{t("brw.nye.k.url")}</dt><dd><code>{x.url.clone()}</code></dd>
                <dt>{t("brw.nye.k.title")}</dt><dd>{x.title.clone()}</dd>
                <dt>{t("brw.nye.k.kind")}</dt><dd>{t(match x.kind { BrowserExtractionKind::Selection => "brw.nye.kind.selection", BrowserExtractionKind::Main => "brw.nye.kind.main" })}</dd>
                <dt>{t("brw.nye.k.size")}</dt><dd>{tf("brw.nye.size", &[("n", x.chars.to_string().as_str()), ("max", x.max_chars.to_string().as_str())])}</dd>
                <dt>{t("brw.nye.k.at")}</dt><dd>{x.read_at.clone()}</dd>
            </dl>
            <p class="oc-brw-side__quote">{crate::ui::components::visible(&x.excerpt)}</p>
            <p>{t("brw.nye.never")}</p>
            <p><strong>{t("brw.nye.untrusted")}</strong></p>
            <p>{tf("brw.nye.egress", &[("policy", t(x.egress_key))])}</p>
        </section>
    }
}

fn nye(n: &BrowserNyeVm) -> AnyView {
    let head = side_head("brw.nye", "ob-nye", "brw-nye");
    match n {
        BrowserNyeVm::WebUnavailable => view! { {head}<p class="oc-app-note">{icon("status")}<span>{t("brw.nye.web")}</span></p> }.into_any(),
        BrowserNyeVm::NoAi => view! { {head}<p class="oc-app-note" data-tone="warn">{icon("warning")}<span>{t("brw.nye.no_ai")}</span></p> }.into_any(),
        BrowserNyeVm::Preview { extraction: x, question } => view! {
            {head}{extraction(x)}
            <form class="oc-brw-side__form" method="post" action="/browser/nye" data-part="brw-nye-form">
                <label class="oc-app-field"><span>{t("brw.nye.question")}</span><textarea name="question" rows="2">{question.clone()}</textarea></label>
                <div class="oc-brw-side__acts">
                    <button type="submit" class="oc-app-primary">{icon("ob-nye")}<span>{t("brw.nye.send")}</span></button>
                    <button type="button" class="oc-app-btn" data-oc="brw-side" data-side="brw-nye">{t("app.cancel")}</button>
                </div>
            </form>
        }.into_any(),
        BrowserNyeVm::Answer { paragraphs, mentions_instructions, extraction: x } => view! {
            {head}
            <section class="oc-brw-side__box">
                <p><strong>{t("brw.nye.answer")}</strong></p>
                {paragraphs.iter().map(|p| view! { <p>{crate::ui::components::visible(p)}</p> }).collect_view()}
                {mentions_instructions.then(|| view! { <p class="oc-app-note" data-tone="warn">{icon("shield")}<span>{t("brw.nye.injection")}</span></p> })}
                <p class="oc-term-card__note">{tf("brw.nye.answer.source", &[("url", x.url.as_str()), ("at", x.read_at.as_str())])}</p>
            </section>
            {extraction(x)}
        }.into_any(),
    }
}

fn download(d: &BrowserDownloadVm) -> impl IntoView {
    let (st, key) = match d.state {
        BrowserDownloadState::Pending => ("pending", "brw.dl.st.pending"),
        BrowserDownloadState::Downloading => ("downloading", "brw.dl.st.downloading"),
        BrowserDownloadState::Complete => ("complete", "brw.dl.st.complete"),
        BrowserDownloadState::Failed => ("failed", "brw.dl.st.failed"),
        BrowserDownloadState::Cancelled => ("cancelled", "brw.dl.st.cancelled"),
    };
    let bytes = match (&d.done, &d.total) {
        (Some(a), Some(b)) => Some(tf(
            "brw.dl.bytes",
            &[("done", a.as_str()), ("total", b.as_str())],
        )),
        (Some(a), None) => Some(tf("brw.dl.bytes_unknown", &[("done", a.as_str())])),
        _ => None,
    };
    let dest = d.destination.as_ref().map(|x| match x {
        BrowserDownloadDest::Host => t("brw.dl.dest.host").to_owned(),
        BrowserDownloadDest::Files { folder } => {
            tf("brw.dl.dest.files", &[("folder", folder.as_str())])
        }
    });
    view! {
        <li class="oc-brw-dl__it" data-state=st>
            <p class="oc-brw-dl__name">{d.file_name.clone()}</p>
            <p class="oc-brw-dl__meta"><span>{t(key)}</span>{bytes.clone().map(|b| view! { <span>{b}</span> })}<span>{tf("brw.dl.from", &[("origin", d.origin.as_str())])}</span></p>
            {d.pct.map(|p| view! { <span class="oc-brw-dl__bar" role="img" aria-label=bytes.clone().unwrap_or_default()><span data-pct=p.to_string()></span></span> })}
            {d.suggested.clone().map(|s| view! { <p class="oc-brw-dl__meta">{tf("brw.dl.name_changed", &[("name", crate::ui::components::visible(&s).as_str())])}</p> })}
            {(d.state == BrowserDownloadState::Failed).then(|| view! { <p class="oc-brw-dl__meta">{t("brw.dl.failed.body")}</p> })}
            {dest.map(|x| view! { <p class="oc-brw-dl__meta"><span>{t("brw.dl.dest")}": "{x}</span></p> })}
            <div class="oc-brw-dl__acts">{match d.state {
                BrowserDownloadState::Pending => view! {
                    <button type="button" class="oc-app-btn" data-oc="brw-dl-save" data-download=d.id.clone()>{icon("ob-pc")}<span>{t("brw.dl.save")}</span></button>
                    <button type="button" class="oc-app-btn" data-oc="brw-dl-files" data-download=d.id.clone()>{icon("ob-files")}<span>{t("brw.dl.files")}</span></button>
                    <button type="button" class="oc-app-btn" data-oc="brw-dl-cancel" data-download=d.id.clone()>{t("brw.dl.cancel")}</button>
                }.into_any(),
                BrowserDownloadState::Downloading => view! { <button type="button" class="oc-app-btn" data-oc="brw-dl-cancel" data-download=d.id.clone()>{t("brw.dl.cancel")}</button> }.into_any(),
                BrowserDownloadState::Failed => view! { <button type="button" class="oc-app-btn" data-oc="brw-dl-retry" data-download=d.id.clone()>{icon("refresh")}<span>{t("brw.dl.retry")}</span></button> }.into_any(),
                _ => ().into_any(),
            }}</div>
        </li>
    }
}

/// O ecrã do Browser. Sem barra `oc-app__bar`: o cromado do Browser é a barra.
#[must_use]
pub fn browser(vm: &BrowserVm) -> AnyView {
    let web = vm.runtime == BrowserRuntime::Web;
    let nav = &vm.navigation;
    let many = vm.tabs.len() > 8;
    let is_new = matches!(vm.page, BrowserPageStateVm::NewTab);
    let showing = matches!(vm.page, BrowserPageStateVm::External { .. });
    let origin = match &vm.page {
        BrowserPageStateVm::External { origin } => origin.clone(),
        _ => String::new(),
    };
    let sec = nav.security.map(|s| match s {
        BrowserSecurity::Https => "https",
        BrowserSecurity::Http => "http",
        BrowserSecurity::Blocked => "blocked",
    });
    let sec_label = match nav.security {
        Some(BrowserSecurity::Https) => t("brw.sec.https"),
        Some(BrowserSecurity::Http) => t("brw.sec.http"),
        _ => t("brw.address"),
    };
    let host = nav.host.clone().unwrap_or_default();
    let back = |cls: &'static str| {
        (!web).then(|| view! {
        <button type="button" class=cls data-oc="brw-back" aria-label=t("brw.back") disabled=!nav.can_back.unwrap_or(false)>{icon("ob-back")}</button>
        <button type="button" class=cls data-oc="brw-forward" aria-label=t("brw.forward") disabled=!nav.can_forward.unwrap_or(false)>{icon("ob-fwd")}</button>
    })
    };
    let side_part = match &vm.side {
        Some(BrowserSideVm::Downloads(_)) => "brw-downloads",
        _ => "brw-nye",
    };
    view! {
        <div class="oc-app" data-oc="app" data-app="browser">
            <div class="oc-app__body"><div class="oc-app__main" data-part="app-main">
            <div class="oc-brw" data-oc="brw" data-runtime=vm.runtime.as_str() data-t-https=t("brw.sec.https") data-t-http=t("brw.sec.http")>
                <div class="oc-brw-tabs" data-many=many.then_some("")>
                    <div class="oc-brw-tabs__list" role="tablist" aria-label=t("brw.tabs") data-part="brw-tablist">
                        {vm.tabs.iter().enumerate().map(|(i, x)| tab(x, i, i == vm.active)).collect_view()}
                    </div>
                    <button type="button" class="oc-brw-tabs__btn" data-oc="brw-new" aria-label=t("brw.tab.new")>{icon("plus")}</button>
                    {many.then(|| view! { <button type="button" class="oc-brw-tabs__btn" data-oc="brw-switcher" aria-expanded="false" aria-controls="oc-brw-switch">{icon("list")}<span>{tf("brw.tab.all", &[("n", vm.tabs.len().to_string().as_str())])}</span></button> })}
                </div>
                <div class="oc-brw-bar" role="toolbar" aria-label=t("browser.app")>
                    {back("oc-brw-nav")}
                    {(!is_new).then(|| if nav.loading {
                        view! { <button type="button" class="oc-brw-nav" data-oc="brw-stop" aria-label=t("brw.stop")>{icon("close")}</button> }.into_any()
                    } else {
                        view! { <button type="button" class="oc-brw-nav" data-oc="brw-reload" aria-label=t("brw.reload")>{icon("ob-reload")}</button> }.into_any()
                    })}
                    <form class="oc-brw-addr" data-part="brw-address" method="get" action="/browser">
                        {sec.map(|s| view! {
                            <button type="button" class="oc-brw-site" data-sec=s data-oc="brw-side" data-side="brw-siteinfo" aria-expanded="false" aria-label=format!("{}: {}, {}", t("brw.site"), sec_label, host)>
                                {icon(if s == "https" { "ob-lock" } else { "ob-warn" })}
                                <span class="oc-brw-site__sec" data-part="brw-sec">{sec_label}</span>
                                {(!host.is_empty()).then(|| view! { <span class="oc-brw-site__host" data-part="brw-origin">{host.clone()}</span> })}
                            </button>
                        })}
                        {(web && showing && nav.typed.is_none()).then(|| view! { <span class="oc-brw-addr__label">{t("brw.requested")}</span> })}
                        <input type="text" name="url" inputmode="url" autocomplete="off" autocapitalize="off" spellcheck="false" enterkeyhint="go" aria-label=t("brw.address") placeholder=t("brw.address.placeholder") value=nav.typed.clone().unwrap_or_else(|| nav.url.clone())/>
                        <button type="submit" class="oc-brw-addr__go">{t("brw.go")}</button>
                    </form>
                    <span class="oc-brw-rt" data-rt=vm.runtime.as_str() title=t("brw.runtime")>{t(rt_key(vm.runtime))}</span>
                    <button type="button" class="oc-brw-tool" data-oc="brw-side" data-side="brw-nye" aria-expanded="false" aria-disabled=is_new.then_some("true")>{icon("ob-nye")}<span>{t("brw.nye.short")}</span></button>
                    {vm.downloads_supported.then(|| view! { <button type="button" class="oc-brw-tool" data-oc="brw-side" data-side="brw-downloads" aria-expanded="false" aria-label=t("brw.menu.downloads")>{icon("ob-dl")}</button> })}
                    <button type="button" class="oc-brw-tool oc-brw-count" data-oc="brw-switcher" aria-expanded="false" aria-controls="oc-brw-switch" aria-label=tf("brw.mobile.tabs", &[("n", vm.tabs.len().to_string().as_str())])>{vm.tabs.len().to_string()}</button>
                    <button type="button" class="oc-brw-tool" data-oc="brw-menu" aria-haspopup="menu" aria-expanded="false" aria-label=t("brw.menu")>{icon("ob-more")}</button>
                </div>
                {vm.notices.iter().map(notice).collect_view()}
                {showing.then(|| view! {
                    <div class="oc-brw-edge" data-part="brw-edge">{icon("ob-ext")}<span class="oc-brw-edge__o">{tf("brw.edge", &[("origin", origin.as_str())])}</span><span class="oc-brw-edge__n">{t("brw.edge.note")}</span></div>
                })}
                <div class="oc-brw-body" data-part="brw-body" data-side=vm.side.is_some().then_some("")>
                    <div class="oc-brw-view" id="oc-brw-view" role="tabpanel" aria-labelledby=format!("oc-brw-tab-{}", vm.active) data-part="brw-view" data-origin=showing.then(|| host.clone()) data-empty=(!showing).then_some("")>{page(vm)}</div>
                    {vm.side.as_ref().map(|s| view! {
                        <aside class="oc-brw-side" data-part=side_part aria-labelledby="oc-brw-side-t">{match s {
                            BrowserSideVm::Nye(n) => nye(n),
                            BrowserSideVm::Downloads(list) => view! { {side_head("brw.menu.downloads", "ob-dl", "brw-downloads")}<ul class="oc-brw-dl">{list.iter().map(download).collect_view()}</ul><p>{t("brw.dl.no_open")}</p> }.into_any(),
                        }}</aside>
                    })}
                    {sec.map(|s| view! {
                        <aside class="oc-brw-side" data-part="brw-siteinfo" hidden="" aria-labelledby="oc-brw-site-t">
                            <h3 class="oc-brw-side__h" id="oc-brw-site-t">{icon(if s == "https" { "ob-lock" } else { "ob-warn" })}<span>{t("brw.site")}</span></h3>
                            <dl><dt>{t("brw.address")}</dt><dd><code>{nav.url.clone()}</code></dd><dt>{t("brw.runtime")}</dt><dd>{t(rt_key(vm.runtime))}</dd></dl>
                            <p>{t(if s == "http" { "brw.sec.http.detail" } else { "brw.sec.https.detail" })}</p>
                            <p>{t("brw.edge.note")}</p>
                        </aside>
                    })}
                </div>
                <section class="oc-brw-switch" id="oc-brw-switch" data-part="brw-switch" hidden="" aria-labelledby="oc-brw-switch-t">
                    <h3 class="oc-brw-switch__h" id="oc-brw-switch-t">{format!("{} · {}", t("brw.mobile.switcher"), vm.tabs.len())}<span class="oc-app__spacer"></span>
                        <button type="button" class="oc-app-btn" data-oc="brw-new">{icon("plus")}<span>{t("brw.tab.new")}</span></button></h3>
                    <ul>{vm.tabs.iter().enumerate().map(|(i, x)| view! {
                        <li aria-current=(i == vm.active).then_some("true")>
                            <button type="button" class="oc-brw-switch__sel" data-oc="brw-activate" data-tab=x.id.clone()><span class="oc-brw-switch__t">{x.title.clone()}</span><span class="oc-brw-switch__o">{x.origin.clone().unwrap_or_default()}</span></button>
                            <button type="button" class="oc-brw-switch__x" data-oc="brw-close" data-tab=x.id.clone() aria-label=tf("brw.tab.close", &[("title", x.title.as_str())])>{icon("close")}</button>
                        </li>
                    }).collect_view()}</ul>
                    <button type="button" class="oc-app-primary" data-oc="brw-switcher">{icon("arrow-l")}<span>{t("brw.mobile.done")}</span></button>
                </section>
                <div class="oc-brw-foot" role="toolbar" aria-label=t("brw.tabs")>
                    {back("oc-brw-nav")}
                    {(!is_new).then(|| view! { <button type="button" class="oc-brw-nav" data-oc="brw-reload" aria-label=t("brw.reload")>{icon("ob-reload")}</button> })}
                    <button type="button" class="oc-brw-tool" data-oc="brw-side" data-side="brw-nye" aria-label=t("brw.nye") aria-disabled=is_new.then_some("true")>{icon("ob-nye")}</button>
                    <button type="button" class="oc-brw-tool" data-oc="brw-menu" aria-haspopup="menu" aria-expanded="false" aria-label=t("brw.menu")>{icon("ob-more")}</button>
                </div>
                <template data-part="brw-tpl-frame"><iframe title=tf("brw.edge", &[("origin", "")]) sandbox=WEB_SANDBOX referrerpolicy="no-referrer" allow=""></iframe></template>
                <template data-part="brw-tpl-blocked"><div class="oc-brw-state" data-tone="err" role="alert"><span class="oc-brw-state__ic" aria-hidden="true">{icon("ob-warn")}</span><p class="oc-brw-state__t">{tf("brw.scheme.title", &[("scheme", "")])}<span data-slot="scheme"></span></p><p>{t("brw.scheme.body")}</p></div></template>
                <template data-part="brw-tpl-invalid"><div class="oc-brw-state" data-tone="warn" role="status"><span class="oc-brw-state__ic" aria-hidden="true">{icon("ob-search")}</span><p class="oc-brw-state__t">{t("brw.invalid.title")}</p><p><span data-slot="text"></span></p><p>{t("brw.new.body")}</p></div></template>
                <template data-part="brw-tpl-no-runtime"><div class="oc-brw-state" data-tone="warn" role="status"><span class="oc-brw-state__ic" aria-hidden="true">{icon("ob-pc")}</span><p class="oc-brw-state__t">{t("brw.web.blocked.title")}</p><p>{t("brw.web.note")}</p></div></template>
                <p class="oc-sr" role="status" aria-live="polite" data-part="brw-live"></p>
            </div>
            </div></div>
            <p class="oc-sr" role="status" aria-live="polite" data-part="app-live"></p>
        </div>
    }
    .into_any()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::view_models::{BrowserNavigationVm, BrowserPermissionKind, BrowserPermissionVm};

    fn vm(runtime: BrowserRuntime, page: BrowserPageStateVm) -> BrowserVm {
        BrowserVm {
            runtime,
            tabs: vec![BrowserTabVm {
                id: "t0".into(),
                title: "<img src=x onerror=alert(1)>".into(),
                origin: Some("clima-costeiro.example".into()),
                state: BrowserTabState::Ready,
            }],
            active: 0,
            navigation: BrowserNavigationVm {
                can_back: None,
                can_forward: None,
                loading: false,
                url: "https://clima-costeiro.example/".into(),
                host: Some("clima-costeiro.example".into()),
                security: Some(BrowserSecurity::Https),
                typed: None,
            },
            page,
            notices: vec![],
            side: None,
            downloads_supported: false,
        }
    }
    fn ext() -> BrowserPageStateVm {
        BrowserPageStateVm::External {
            origin: "https://clima-costeiro.example".into(),
        }
    }

    /// BROWSER-06 · o iframe da Web nunca partilha a origem nem navega o Ocinye.
    #[test]
    fn iframe_web_isolado() {
        let h = browser(&vm(BrowserRuntime::Web, ext())).to_html();
        assert!(h.contains(WEB_SANDBOX));
        for p in ["allow-same-origin", "allow-top-navigation", "allow-modals"] {
            assert!(!h.contains(p), "{p}");
        }
        assert!(h.contains("referrerpolicy=\"no-referrer\"") && h.contains("allow=\"\""));
    }

    /// A fronteira existe quando há conteúdo externo, e nenhum controlo do Ocinye fica dentro da vista.
    #[test]
    fn fronteira_de_confianca() {
        let h = browser(&vm(BrowserRuntime::Web, ext())).to_html();
        let edge = h.find("class=\"oc-brw-edge\"").expect("faixa");
        let view = h.find("class=\"oc-brw-view\"").expect("vista");
        assert!(edge < view);
        let dentro = &h[view..h[view..].find("</div>").map_or(h.len(), |i| view + i)];
        assert!(
            !dentro.contains("data-oc="),
            "controlo do Ocinye dentro do conteúdo externo"
        );
    }

    /// Web: sem recuar/avançar (histórico do site não observável) — nunca botões mortos activos.
    #[test]
    fn web_sem_recuar_nem_avancar() {
        let h = browser(&vm(BrowserRuntime::Web, ext())).to_html();
        assert!(!h.contains("data-oc=\"brw-back\"") && !h.contains("data-oc=\"brw-forward\""));
    }

    #[test]
    fn titulo_hostil_escapado() {
        assert!(!browser(&vm(BrowserRuntime::Web, ext()))
            .to_html()
            .contains("<img src=x"));
    }

    #[test]
    fn permissao_mostra_a_origem() {
        let mut v = vm(BrowserRuntime::Desktop, ext());
        v.notices
            .push(BrowserNoticeVm::Permission(BrowserPermissionVm {
                request_id: "r1".into(),
                origin: "https://clima-costeiro.example".into(),
                kind: BrowserPermissionKind::Microphone,
            }));
        let h = browser(&v).to_html();
        assert!(h.contains("https://clima-costeiro.example"));
        assert!(
            !h.contains("data-decision=\"always\""),
            "sem persistência que o runtime não garante"
        );
    }

    #[test]
    fn esquema_perigoso_tem_estado() {
        let h = browser(&vm(
            BrowserRuntime::Web,
            BrowserPageStateVm::BlockedScheme {
                scheme: "javascript:".into(),
            },
        ))
        .to_html();
        assert!(h.contains("javascript:"));
        assert!(
            !h.contains("src=\"javascript:"),
            "nunca navega para o esquema"
        );
    }

    /// D008 §140 · nenhuma ponte Browser → ocsh no ecrã.
    #[test]
    fn sem_ponte_para_o_ocsh() {
        let h = browser(&vm(BrowserRuntime::Desktop, ext())).to_html();
        assert!(!h.contains("/terminal") && !h.contains("ocsh"));
    }

    /// Sem proxy, sem executeJavaScript, sem DOM API.
    #[test]
    fn sem_proxy_nem_injeccao() {
        let h = browser(&vm(BrowserRuntime::Web, ext())).to_html();
        for p in ["/proxy", "executeJavaScript", "querySelector", "eval("] {
            assert!(!h.contains(p), "{p}");
        }
    }
}
