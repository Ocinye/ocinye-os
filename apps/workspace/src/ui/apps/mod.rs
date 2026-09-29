//! D004 · Aplicações de produtividade. DESIGN_LOCKED.
//!
//! O sistema de aplicação partilhado (a moldura, a barra, a navegação lateral,
//! o inspector, a pesquisa de âmbito, os estados vazio/a carregar/erro, o estado
//! de gravação, a ligação à Nye) e as quatro aplicações que o usam:
//! [`files`], [`notes`], [`calendar`], [`mail`].
//!
//! Tudo é o corpo de uma janela gerida D002 (`WindowContent::Ready`): a moldura
//! da janela é do gestor; a aplicação começa em `.oc-app`. O layout responde à
//! largura da **janela** (container queries), não do ecrã: a 1440 uma janela
//! normal, a 924 maximizada, a 390 o ecrã inteiro — sem motor próprio.
//!
//! A vista não decide nada: dados já autorizados pelo Core, datas e tamanhos
//! já formatados na língua e no fuso do membro, acções só quando o VM as traz.
//! Nada de armazenamento (bucket, chave de objecto, caminho do anfitrião).

use leptos::prelude::*;

use crate::i18n::{t, tf};
use crate::ui::components::icon;
use crate::ui::view_models::{
    AppError, AppLoad, AppNavVm, AppNyeVm, AppPageVm, AppSaveState, FileKind,
};

pub mod calendar;
pub mod files;
pub mod mail;
pub mod notes;

/// O ícone de um tipo de ficheiro.
#[must_use]
pub const fn kind_icon(k: FileKind) -> &'static str {
    match k {
        FileKind::Folder => "folder",
        FileKind::Image => "image",
        FileKind::Pdf | FileKind::Document => "ot-doc",
        FileKind::Text => "type",
        FileKind::Code => "code",
        FileKind::Data => "data",
        FileKind::Archive => "archive",
        FileKind::Media => "ot-play",
        FileKind::Other => "files",
    }
}

/// A moldura de uma aplicação. `side` e `inspector` são opcionais; abaixo de
/// 720 px de janela a navegação é uma gaveta e o inspector ocupa a aplicação.
pub fn frame(
    app: &'static str,
    label: String,
    toolbar: AnyView,
    side: Option<AnyView>,
    main: AnyView,
    inspector: Option<AnyView>,
) -> AnyView {
    let side_id = format!("oc-{app}-side");
    let has_side = side.is_some();
    let has_insp = inspector.is_some();
    view! {
        <div class="oc-app" data-oc="app" data-app=app data-inspector=has_insp.then_some("")>
            <div class="oc-app__bar" role="toolbar" aria-label=label>
                {has_side.then(|| view! {
                    <button type="button" class="oc-app__icon oc-app__side-btn" data-oc="app-drawer" aria-controls=side_id.clone() aria-expanded="false" aria-label=t("app.nav")>{icon("sidebar")}</button>
                })}
                {toolbar}
            </div>
            <div class="oc-app__body">
                {side.map(|s| view! { <nav class="oc-app__side" id=side_id.clone() data-part="app-side" aria-label=t("app.nav")>{s}</nav> })}
                <div class="oc-app__main" data-part="app-main">{main}</div>
                {inspector.map(|i| view! { <aside class="oc-app__insp" data-part="app-insp" aria-label=t("app.details")>{i}</aside> })}
            </div>
            <p class="oc-sr" role="status" aria-live="polite" data-part="app-live"></p>
        </div>
    }
    .into_any()
}

/// A navegação lateral (secções, pastas). Contagens só quando o Core as dá.
pub fn nav(items: &[AppNavVm]) -> impl IntoView {
    view! {
        <ul class="oc-app-nav">
            {items.iter().map(|n| view! {
                <li>
                    <a class="oc-app-nav__item" href=n.href.clone() aria-current=n.active.then_some("page")>
                        {icon(n.icon)}
                        <span class="oc-app-nav__label">{n.label.clone()}</span>
                        {n.count.map(|c| view! { <span class="oc-app-nav__count">{c.to_string()}</span> })}
                    </a>
                </li>
            }).collect_view()}
        </ul>
    }
}

/// A pesquisa da aplicação. O âmbito fica escrito no marcador («Pesquisar em
/// Notas»): a pesquisa de todo o Ocinye é a da Nye (⌘K), nunca esta.
pub fn search(action: String, query: &str, scope_key: &'static str) -> impl IntoView {
    view! {
        <form class="oc-app-search" method="get" action=action role="search">
            <label class="oc-app-search__field">
                <span class="oc-sr">{t(scope_key)}</span>
                {icon("search")}
                <input type="search" name="q" value=query.to_owned() placeholder=t(scope_key) autocomplete="off" data-part="app-q" />
            </label>
        </form>
    }
}

/// Um botão de acção principal da barra (a criação desta aplicação).
pub fn primary(href: String, ic: &'static str, key: &'static str) -> impl IntoView {
    view! { <a class="oc-app-primary" href=href>{icon(ic)}<span>{t(key)}</span></a> }
}

/// A ligação contextual à Nye: abre a Nye canónica com a referência. Nunca um
/// segundo assistente.
pub fn nye(n: &AppNyeVm) -> impl IntoView {
    view! { <a class="oc-app-nye" href=n.href.clone() data-oc="app-nye">{icon("nye")}<span>{t(n.label_key)}</span></a> }
}

/// O estado de gravação, discreto na barra. Não é um toast.
pub fn save_state(s: &AppSaveState) -> AnyView {
    let (state, ic, text) = match s {
        AppSaveState::Clean => return ().into_any(),
        AppSaveState::Dirty => ("dirty", "edit", t("app.save.dirty").to_owned()),
        AppSaveState::Saving => ("saving", "refresh", t("app.save.saving").to_owned()),
        AppSaveState::Saved(at) => (
            "saved",
            "check",
            tf("app.save.saved", &[("at", at.as_str())]),
        ),
        AppSaveState::Failed(e) => (
            "failed",
            "warning",
            t(&format!("{}.title", e.key())).to_owned(),
        ),
    };
    view! { <span class="oc-app-save" data-state=state role="status">{icon(ic)}<span>{text}</span></span> }.into_any()
}

/// Um erro tipado, em linha, com a mesma linguagem dos erros D001/D002.
/// Nunca revela o que o membro não pode ver.
pub fn error(e: AppError) -> impl IntoView {
    let k = e.key();
    let ic = match e {
        AppError::PermissionDenied | AppError::Revoked => "lock",
        AppError::Reconnecting => "refresh",
        AppError::NotConnected => "link",
        _ => "warning",
    };
    view! {
        <div class="oc-app-state" data-state="error" data-error=k role="alert">
            <span class="oc-app-state__icon" aria-hidden="true">{icon(ic)}</span>
            <p class="oc-app-state__title">{t(&format!("{k}.title"))}</p>
            <p class="oc-app-state__body">{t(&format!("{k}.body"))}</p>
        </div>
    }
}

/// O estado vazio de uma vista (uma frase precisa, nunca «nada aqui»).
pub fn empty(
    ic: &'static str,
    title_key: &'static str,
    body_key: Option<&'static str>,
) -> impl IntoView {
    view! {
        <div class="oc-app-state" data-state="empty" role="status">
            <span class="oc-app-state__icon" aria-hidden="true">{icon(ic)}</span>
            <p class="oc-app-state__title">{t(title_key)}</p>
            {body_key.map(|k| view! { <p class="oc-app-state__body">{t(k)}</p> })}
        </div>
    }
}

/// Esqueleto parcial de uma lista (nunca um spinner da aplicação inteira).
pub fn skeleton(rows: usize) -> impl IntoView {
    view! {
        <div class="oc-app-skel" aria-busy="true">
            <span class="oc-sr">{t("app.loading")}</span>
            {(0..rows).map(|_| view! { <span class="oc-app-skel__row" aria-hidden="true"></span> }).collect_view()}
        </div>
    }
}

/// Carregar ou falhar, antes do conteúdo; `None` = pronto.
pub fn load_state(l: AppLoad, rows: usize) -> Option<AnyView> {
    match l {
        AppLoad::Ready => None,
        AppLoad::Loading => Some(skeleton(rows).into_any()),
        AppLoad::Failed(e) => Some(error(e).into_any()),
    }
}

/// «Mostrar mais» (cursor) e o resumo «N de M».
pub fn more(p: &AppPageVm) -> impl IntoView {
    (p.more_href.is_some() || p.summary.is_some()).then(|| view! {
        <div class="oc-app-more">
            {p.summary.clone().map(|s| view! { <span class="oc-app-more__sum">{s}</span> })}
            {p.more_href.clone().map(|h| view! { <a class="oc-app-btn" href=h data-oc="app-more">{t("app.more")}</a> })}
        </div>
    })
}

/// A moldura de um documento aberto num editor: o formulário que reporta
/// `data-dirty` ao gestor de janelas e que «Guardar» do fecho D002 submete.
#[must_use]
pub fn doc_form_id(app: &str, id: &str) -> String {
    format!("oc-{app}-doc-{id}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::testing::assert_contracts;

    #[test]
    fn a_moldura_tem_barra_navegacao_e_regiao_viva() {
        let html = frame(
            "files",
            "Ficheiros".into(),
            view! { <span>"x"</span> }.into_any(),
            Some(view! { <span>"n"</span> }.into_any()),
            view! { <p>"m"</p> }.into_any(),
            None,
        )
        .to_html();
        assert_contracts(&html);
        assert!(html.contains(r#"role="toolbar""#) && html.contains(r#"data-part="app-live""#));
        assert!(html.contains(r#"aria-controls="oc-files-side""#));
    }

    #[test]
    fn cada_erro_tem_titulo_e_texto_nas_tres_linguas() {
        for e in [
            AppError::Unavailable,
            AppError::PermissionDenied,
            AppError::NotFound,
            AppError::Conflict,
            AppError::SyncFailed,
            AppError::SaveFailed,
            AppError::UploadFailed,
            AppError::TransportFailed,
            AppError::Revoked,
            AppError::Reconnecting,
            AppError::NotConnected,
        ] {
            let html = error(e).to_html();
            assert!(
                !html.contains(&format!("{}.title", e.key())),
                "chave crua {}",
                e.key()
            );
        }
    }
}
