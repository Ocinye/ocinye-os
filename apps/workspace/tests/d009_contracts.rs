//! Guardas estáticas da D009: a iconografia canónica, as predefinições por
//! Distribuição e o que elas nunca podem fazer. Não precisam de base de dados.

use std::path::Path;

use ocinye_contracts::{ApplicationId, InstanceProfile};
use ocinye_workspace::experience::{apps, distribution, iconography};
use ocinye_workspace::ui::view_models::{Distribution, Wallpaper};

fn read(p: &str) -> String {
    std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join(p))
        .unwrap_or_else(|e| panic!("{p}: {e}"))
}

/// Os ids de símbolo do sprite.
fn simbolos() -> Vec<String> {
    read("static/icons.svg")
        .split("<symbol id=\"")
        .skip(1)
        .filter_map(|x| x.split('"').next())
        .map(str::to_owned)
        .collect()
}

/// ICON-01 · Uma aplicação registada, um ícone, e o ícone existe. O mapa, o
/// registo e o lançador têm o mesmo tamanho.
#[test]
fn cada_aplicacao_registada_tem_um_icone_que_existe() {
    let sprite = simbolos();
    assert_eq!(ApplicationId::ALL.len(), 28);
    assert_eq!(iconography::APP_ICONS.len(), ApplicationId::ALL.len());
    assert_eq!(apps::APPLICATIONS.len(), ApplicationId::ALL.len());
    for a in ApplicationId::ALL {
        let n = iconography::APP_ICONS
            .iter()
            .filter(|(x, _)| *x == a)
            .count();
        assert_eq!(n, 1, "{a:?} tem {n} ícones no mapa");
        let s = iconography::app_icon_id(a);
        assert!(sprite.iter().any(|x| x == s), "{a:?} → {s}, que não existe");
        // Nenhuma superfície por rota se afasta do mapa.
        assert_eq!(
            ocinye_workspace::ui::components::app_icon(a.manifest().route),
            s,
            "{a:?}: a rota dá outro ícone"
        );
    }
}

/// ICON-08 · Dois ecrãs diferentes não partilham o ícone canónico.
#[test]
fn nao_ha_icones_repetidos_entre_aplicacoes() {
    let mut vistos = std::collections::BTreeMap::new();
    for (a, s) in iconography::APP_ICONS {
        if let Some(outro) = vistos.insert(s, a) {
            panic!("{a:?} e {outro:?} partilham {s}");
        }
    }
    for (d, s) in iconography::DIST_ICONS {
        assert!(
            !iconography::APP_ICONS.iter().any(|(_, x)| *x == s),
            "{d:?}: o ícone da Distribuição é também de uma aplicação"
        );
    }
}

/// ICON-02 · Quatro ícones de Distribuição, que existem, e que não são
/// aplicações.
#[test]
fn quatro_icones_de_distribuicao() {
    let sprite = simbolos();
    assert_eq!(iconography::DIST_ICONS.len(), 4);
    for d in [
        Distribution::Research,
        Distribution::Business,
        Distribution::Personal,
        Distribution::Education,
    ] {
        let s = iconography::dist_icon_id(d);
        assert!(sprite.iter().any(|x| x == s), "{d:?} → {s}");
        assert!(s.parse::<ApplicationId>().is_err(), "{s} é uma aplicação");
    }
}

/// Nenhum `<use>` partido: cada `icon("…")` escrito no código de interface
/// nomeia um símbolo que existe.
#[test]
fn nenhum_icone_literal_aponta_para_o_vazio() {
    let sprite = simbolos();
    let mut partidos = Vec::new();
    let mut lidos = 0;
    let mut pilha = vec![Path::new(env!("CARGO_MANIFEST_DIR")).join("src/ui")];
    while let Some(dir) = pilha.pop() {
        for e in std::fs::read_dir(&dir).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                pilha.push(p);
                continue;
            }
            if p.extension().and_then(|x| x.to_str()) != Some("rs") {
                continue;
            }
            lidos += 1;
            let src = std::fs::read_to_string(&p).unwrap();
            // Só a chamada `icon("…")` a sério, não `app_icon("/rota")`.
            let mut desde = 0;
            while let Some(rel) = src[desde..].find("icon(\"") {
                let at = desde + rel;
                desde = at + 6;
                let antes = src[..at].chars().next_back();
                if antes.is_some_and(|c| c.is_alphanumeric() || c == '_') {
                    continue;
                }
                let id = src[at + 6..].split('"').next().unwrap_or_default();
                if !id.is_empty() && !sprite.iter().any(|x| x == id) {
                    partidos.push(format!("{}: {id}", p.display()));
                }
            }
        }
    }
    assert!(lidos > 20, "controlo positivo: {lidos} ficheiros lidos");
    assert!(partidos.is_empty(), "ícones inexistentes: {partidos:?}");
}

/// ICON-10 · O sprite não carrega nem executa nada.
#[test]
fn o_sprite_nao_carrega_nem_executa_nada() {
    let svg = read("static/icons.svg");
    let corpo = svg.split("</metadata>").last().unwrap_or(&svg);
    for proibido in [
        "<script",
        "<foreignObject",
        "<image",
        "href=\"http",
        "href=\"//",
        "javascript:",
        "url(",
        " onload",
        " onclick",
        " onerror",
    ] {
        assert!(!corpo.contains(proibido), "o sprite contém {proibido}");
    }
}

/// DIST-01 · Quatro Distribuições, as do enum fechado; uma desconhecida cai
/// na predefinição mínima do sistema — nunca em Research.
#[test]
fn uma_distribuicao_desconhecida_e_a_do_sistema_e_nunca_research() {
    assert_eq!(distribution::DEFAULTS.len(), 4);
    for s in ["", "Research", "RESEARCH", "science", "../research"] {
        assert_eq!(distribution::parse(s), None, "«{s}»");
        assert_eq!(
            apps::default_pins_for(Some(s)),
            apps::default_pins(),
            "«{s}»"
        );
    }
    assert!(
        distribution::widgets(None).is_empty(),
        "um Desktop vazio é vazio"
    );
    assert_eq!(distribution::look(None).0, Wallpaper::Ocinye);
    assert_ne!(
        apps::default_pins_for(Some("desconhecida")),
        apps::default_pins_for(Some("research"))
    );
    assert_eq!(
        distribution::restore_target(false, None),
        distribution::Provenance::System
    );
}

/// As predefinições aprovadas, por Distribuição (docs/ui/distribution-defaults.md).
#[test]
fn as_predefinicoes_sao_as_aprovadas() {
    let casos: [(Distribution, &[&str], &[&str], Wallpaper); 4] = [
        (
            Distribution::Research,
            &[
                "work",
                "projects",
                "ideas",
                "datasets",
                "results",
                "knowledge",
                "files",
                "notes",
            ],
            &["kpis", "projects"],
            Wallpaper::Field,
        ),
        (
            Distribution::Business,
            &[
                "work", "calendar", "mail", "messages", "projects", "files", "notes",
            ],
            &["tasks", "calendar", "projects", "files"],
            Wallpaper::Module,
        ),
        (
            Distribution::Personal,
            &["files", "notes", "calendar", "work", "resources", "trash"],
            &["notes", "files", "calendar", "storage"],
            Wallpaper::Calm,
        ),
        (
            Distribution::Education,
            &[
                "work",
                "units",
                "projects",
                "knowledge",
                "bibliography",
                "calendar",
                "files",
                "notes",
            ],
            &["calendar", "tasks", "projects", "notes"],
            Wallpaper::Lattice,
        ),
    ];
    for (d, pins, widgets, fundo) in casos {
        assert_eq!(distribution::default_pins(Some(d)), pins, "{d:?}");
        let ws: Vec<String> = distribution::widgets(Some(d))
            .into_iter()
            .map(|w| w.id)
            .collect();
        assert_eq!(ws, widgets, "{d:?}");
        assert_eq!(distribution::look(Some(d)).0, fundo, "{d:?}");
        assert!(
            !ws.iter().any(|w| w == "notice"),
            "{d:?}: avisos por omissão"
        );
    }
}

/// Nunca fixados por omissão (não quer dizer proibidos): as aplicações de
/// autoridade, o Terminal, o Browser e a Nye.
#[test]
fn nunca_fixados_por_omissao() {
    for d in &distribution::DEFAULTS {
        for a in [
            ApplicationId::Administration,
            ApplicationId::Audit,
            ApplicationId::Monitor,
            ApplicationId::Terminal,
            ApplicationId::Browser,
            ApplicationId::Prompt,
        ] {
            assert!(!d.pins.contains(&a), "{:?} fixa {a:?}", d.distribution);
        }
    }
}

/// A activação por Distribuição é a de antes da D009 (ADR-0014). A D009 não a
/// muda: fixações por omissão não são aplicações activas.
#[test]
fn a_activacao_nao_mudou() {
    let antes: [(InstanceProfile, &str); 4] = [
        (InstanceProfile::Research, "notes,calendar,work,home,mail,messages,files,knowledge,bibliography,units,ideas,projects,datasets,prompt,ai,agents,compute,resources,activity,administration,audit,settings,help,terminal,browser,monitor,results,trash"),
        (InstanceProfile::Business, "notes,calendar,work,home,mail,messages,files,units,projects,prompt,resources,activity,administration,audit,settings,help,terminal,browser,monitor,trash"),
        (InstanceProfile::Education, "notes,calendar,work,home,mail,messages,files,knowledge,bibliography,units,projects,prompt,resources,activity,administration,audit,settings,help,terminal,browser,monitor,trash"),
        (InstanceProfile::Personal, "notes,calendar,work,home,mail,files,prompt,resources,administration,settings,help,terminal,browser,monitor,trash"),
    ];
    for (p, lista) in antes {
        let agora: Vec<&str> = ApplicationId::ALL
            .into_iter()
            .filter(|a| p.activates(*a))
            .map(ApplicationId::as_str)
            .collect();
        assert_eq!(agora.join(","), lista, "{p:?}");
        // Cada fixação por omissão é de uma aplicação activa nessa Distribuição.
        let d = distribution::parse(p.as_str()).expect("perfil ↔ Distribuição");
        for a in distribution::defaults(d).pins {
            assert!(p.activates(*a), "{p:?} fixa {a:?}, inactiva");
        }
    }
}

/// Sem assistente nem estado «visto»: os primeiros passos abrem a pedido, e
/// nada os grava no navegador.
#[test]
fn os_primeiros_passos_nao_fingem_memoria() {
    for f in [
        "static/oc-shell.js",
        "static/oc-desk.js",
        "static/oc-base.js",
    ] {
        let js = read(f).to_lowercase();
        for proibido in [
            "onboarding",
            "seen_",
            "first_run",
            "firstrun",
            "getting-started",
        ] {
            assert!(!js.contains(proibido), "{f}: {proibido}");
        }
    }
    let shell = read("src/ui/shell/mod.rs");
    assert!(
        shell.contains("oc-menu__pop--dist"),
        "controlo positivo: o painel existe"
    );
    assert!(!shell.contains("<details class=\"oc-menu\" data-oc=\"menu\" open"));
}

/// Um indicador do Desktop é uma ligação a uma aplicação: leva o ícone
/// canónico dela, como a barra e o lançador.
#[test]
fn os_indicadores_usam_o_icone_da_sua_aplicacao() {
    for (_, _, icone, rota) in ocinye_workspace::ui::screens::home::registry::KPIS {
        assert_eq!(
            *icone,
            ocinye_workspace::ui::components::app_icon(rota),
            "o indicador de {rota}"
        );
    }
}

/// G9-09 · Um widget de várias aplicações não desaparece por faltar uma: os
/// Indicadores de Business (sem Ideias nem Dados) continuam; Tarefas vive de
/// O Meu Trabalho sem Projectos; só sem nenhuma o widget fica escondido.
#[test]
fn um_widget_de_varias_aplicacoes_basta_ver_uma() {
    use ocinye_workspace::ui::view_models::WidgetKind as K;
    let business = |a: ApplicationId| InstanceProfile::Business.activates(a);
    assert!(distribution::widget_shown(K::Kpis, business));
    assert!(distribution::widget_shown(K::Tasks, |a| a == ApplicationId::Work));
    assert!(!distribution::widget_shown(K::Kpis, |_| false));
    assert!(!distribution::widget_shown(K::Projects, |a| a != ApplicationId::Projects));
    assert!(
        distribution::widget_shown(K::Health, |_| false),
        "sem aplicação: regra própria"
    );
    // A ordem dos indicadores é a das suas aplicações.
    let apps = distribution::widget_apps(K::Kpis);
    for ((_, _, _, rota), a) in ocinye_workspace::ui::screens::home::registry::KPIS
        .iter()
        .zip(apps)
    {
        assert_eq!(*rota, a.manifest().route, "{a:?}");
    }
    assert_eq!(
        apps.len(),
        ocinye_workspace::ui::screens::home::registry::KPIS.len()
    );
}

/// A 390 px o distintivo da Distribuição e «Abrir aplicações» recebem o toque
/// em 44 px (medido no browser por *hit-testing*); o distintivo continua com
/// 26 px à vista. Esta guarda impede que a regra desapareça.
#[test]
fn no_movel_o_distintivo_recebe_o_toque_em_44_px() {
    let css = read("static/oc-shell.css");
    let bloco = css
        .split("@media (max-width: 640px)")
        .find(|b| b.contains(".oc-dist::after"))
        .expect("a regra móvel do distintivo");
    let regra = bloco.split('}').take(4).collect::<Vec<_>>().join("}");
    assert!(regra.contains("inset: -9px"), "26 + 2 × 9 = 44: {regra}");
    assert!(regra.contains(".oc-dist-go { min-height: 44px"), "{regra}");
    assert!(
        css.contains(".oc-dist { width: 26px; height: 26px;"),
        "o tamanho à vista não muda"
    );
}

/// Fechar o lançador ou a paleta com Escape devolve o foco a quem os abriu
/// (antes caía no `<body>`); só quando estavam abertos e o foco estava dentro.
#[test]
fn fechar_o_lancador_devolve_o_foco() {
    let js = read("static/oc-shell.js");
    let overlay = js
        .split("function overlay(")
        .nth(1)
        .and_then(|x| x.split("function overlays(").next())
        .expect("overlay()");
    assert!(
        overlay.contains("opener = document.activeElement"),
        "{overlay}"
    );
    assert!(
        overlay.contains("el.contains(document.activeElement)) opener.focus()"),
        "{overlay}"
    );
}
