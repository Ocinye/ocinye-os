//! D009 · A iconografia canónica: **um** mapa `ApplicationId → símbolo`.
//!
//! Antes do D009 havia duas fontes: `ui::components::app_icon(href)` (a que se
//! desenha, por rota) e `experience::icon::Icon` (um vocabulário tipado com ids
//! `oc-*` que não existem no sprite). Passa a haver uma: este módulo. O
//! lançador, a barra de aplicações, o título da janela e as fichas lêem daqui;
//! `app_icon(href)` delega para [`for_route`].
//!
//! # Norma do conjunto (ICON-04/05)
//!
//! `viewBox="0 0 16 16"`, traço `currentColor` 1.4 (1.6 nas variantes `app-*`
//! antigas), terminações redondas, sem preenchimento de cor, sem texto, sem
//! raster, sem recurso remoto. Excepção documentada: `nye` traz o dourado da
//! marca (D003) — a Nye é uma só e reconhece-se pela cor.

use ocinye_contracts::ApplicationId as A;

use crate::ui::view_models::Distribution;

/// O mapa canónico, pela ordem de `ApplicationId::ALL`.
pub const APP_ICONS: [(A, &str); 28] = [
    (A::Notes, "notes"),
    (A::Calendar, "calendar"),
    (A::Work, "work"),
    (A::Home, "home"),
    (A::Mail, "mail"),
    (A::Messages, "messages"),
    (A::Files, "files-app"),
    (A::Knowledge, "knowledge"),
    (A::Bibliography, "bibliography"),
    (A::Units, "org-tree"),
    (A::Ideas, "idea"),
    (A::Projects, "project"),
    (A::Datasets, "data"),
    (A::Prompt, "nye"),
    (A::Ai, "ai-fabric"),
    (A::Agents, "agents"),
    (A::Compute, "compute"),
    (A::Resources, "resources"),
    (A::Activity, "activity-feed"),
    (A::Administration, "administration"),
    (A::Audit, "audit-record"),
    (A::Settings, "gear"),
    (A::Help, "help"),
    (A::Terminal, "terminal"),
    (A::Browser, "browser-window"),
    (A::Monitor, "gauge"),
    (A::Results, "results"),
    (A::Trash, "trash"),
];

/// Os quatro ícones de Distribuição. Não são aplicações: nunca vão para a
/// barra de aplicações nem para o lançador (§86).
pub const DIST_ICONS: [(Distribution, &str); 4] = [
    (Distribution::Research, "dist-research"),
    (Distribution::Business, "dist-business"),
    (Distribution::Personal, "dist-personal"),
    (Distribution::Education, "dist-education"),
];

/// Migração (ICON-09): rota → (símbolo antigo, símbolo novo). Os símbolos
/// antigos **ficam** no sprite: ainda são usados por widgets, listas e ecrãs
/// anteriores (`units` na grelha de Indicadores, `files` em linhas de ficheiro,
/// `activity`, `admin`, `shield`, `settings`, `workspace`, `agent`, `ai`,
/// `browser`). Nenhum `<use href>` fica partido.
pub const MIGRATION: &[(&str, &str, &str)] = &[
    ("/files", "files", "files-app"),
    ("/units", "units", "org-tree"),
    ("/ai", "ai", "ai-fabric"),
    ("/ai/agents", "agent", "agents"),
    ("/resources", "workspace", "resources"),
    ("/activity", "activity", "activity-feed"),
    ("/admin", "admin", "administration"),
    ("/audit", "shield", "audit-record"),
    ("/settings", "settings", "gear"),
    ("/browser", "browser", "browser-window"),
];

/// O símbolo de uma aplicação.
#[must_use]
pub fn app_icon_id(id: A) -> &'static str {
    APP_ICONS
        .iter()
        .find(|(a, _)| *a == id)
        .map_or("apps", |(_, s)| s)
}

/// O símbolo de uma aplicação pela sua rota (o que `AppTile::href` traz).
/// Uma rota que nenhum manifesto declara recebe a grelha do lançador.
#[must_use]
pub fn for_route(href: &str) -> &'static str {
    A::ALL
        .into_iter()
        .find(|a| a.manifest().route == href)
        .map_or("apps", app_icon_id)
}

/// O símbolo de uma Distribuição.
#[must_use]
pub fn dist_icon_id(d: Distribution) -> &'static str {
    DIST_ICONS
        .iter()
        .find(|(x, _)| *x == d)
        .map_or("apps", |(_, s)| s)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SPRITE: &str = include_str!("../../static/icons.svg");

    fn symbol(id: &str) -> Option<&'static str> {
        let open = format!("<symbol id=\"{id}\"");
        let start = SPRITE.find(&open)?;
        let end = SPRITE[start..].find("</symbol>")? + start;
        Some(&SPRITE[start..end])
    }

    #[test]
    fn cada_aplicacao_tem_exactamente_um_simbolo_que_existe() {
        for app in A::ALL {
            let n = APP_ICONS.iter().filter(|(a, _)| *a == app).count();
            assert_eq!(n, 1, "{app}: {n} entradas");
            assert!(
                symbol(app_icon_id(app)).is_some(),
                "{app}: símbolo {} em falta",
                app_icon_id(app)
            );
            assert_eq!(
                for_route(app.manifest().route),
                app_icon_id(app),
                "{app}: a rota dá outro ícone"
            );
        }
    }

    /// Duas aplicações diferentes não partilham o mesmo símbolo (ICON-08).
    #[test]
    fn nao_ha_colisao_semantica() {
        let mut vistos = std::collections::BTreeMap::new();
        for (app, s) in APP_ICONS {
            if let Some(outra) = vistos.insert(s, app) {
                panic!("{app} e {outra} usam o mesmo símbolo {s}");
            }
        }
        for (_, s) in DIST_ICONS {
            assert!(
                !APP_ICONS.iter().any(|(_, a)| *a == s),
                "ícone de Distribuição usado por uma aplicação"
            );
        }
    }

    #[test]
    fn quatro_icones_de_distribuicao_que_existem() {
        assert_eq!(DIST_ICONS.len(), 4);
        for (d, s) in DIST_ICONS {
            assert!(symbol(s).is_some(), "{d:?}: {s} em falta");
        }
    }

    /// ICON-05/10: traço em `currentColor`, sem cor fixa, sem recurso externo,
    /// sem script. A Nye é a excepção documentada (dourado da marca).
    #[test]
    fn os_simbolos_canonicos_sao_seguros_e_seguem_o_tema() {
        // Code (D009): o sprite declara `xmlns="http://www.w3.org/2000/svg"`
        // (e o espaço de nomes dos metadados); isso não é um recurso remoto. O
        // que não pode existir é uma referência que se carregue ou execute.
        for proibido in [
            "<script",
            "<image",
            "<foreignObject",
            "href=\"http",
            "href=\"//",
            "src=",
            "url(",
            "javascript:",
            " on",
        ] {
            let sem_metadados = SPRITE.split("</metadata>").last().unwrap_or(SPRITE);
            assert!(
                !sem_metadados.contains(proibido),
                "o sprite contém {proibido}"
            );
        }
        for s in APP_ICONS
            .iter()
            .map(|(_, s)| *s)
            .chain(DIST_ICONS.iter().map(|(_, s)| *s))
        {
            let body = symbol(s).unwrap();
            assert!(body.contains("viewBox=\"0 0 16 16\""), "{s}: viewBox");
            if s != "nye" {
                assert!(
                    !body.contains("fill=\"#") && !body.contains("stroke=\"#"),
                    "{s}: cor fixa"
                );
            }
        }
    }

    #[test]
    fn a_migracao_nao_parte_referencias() {
        for (route, old, new) in MIGRATION {
            assert!(
                symbol(old).is_some(),
                "{old} saiu do sprite mas ainda é referido"
            );
            assert_eq!(for_route(route), *new, "{route}");
        }
    }
}
