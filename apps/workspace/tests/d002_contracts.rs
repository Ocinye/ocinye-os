//! Guardas das correcções D002.1, estruturais e não de píxeis: o que as
//! produz no browser (medido na integração, `docs/ui/design-integration.json`)
//! fica preso ao código do Design, para que uma revisão seguinte não o desfaça
//! sem ninguém ver. Uma revisão que mude uma destas decisões de propósito
//! actualiza este ficheiro na mesma integração.
//!
//! A colocação do diálogo de fechar (fora de `.oc-desk` e de `.oc-wm`) prova-se
//! na página real, em `d002_journeys.rs`.

use leptos::prelude::*;
use ocinye_workspace::ui::shell;
use ocinye_workspace::ui::view_models::{
    Health, ShellVm, WindowContent, WindowGeometry, WindowState, WindowVm, WmVm,
};

const WM_CSS: &str = include_str!("../static/oc-wm.css");
const WM_JS: &str = include_str!("../static/oc-wm.js");

/// O fim do elemento que abre em `start` (etiquetas `<div>` equilibradas).
fn end_of_div(html: &str, start: usize) -> usize {
    let mut depth = 0usize;
    let mut i = start;
    loop {
        let open = html[i..].find("<div").map(|p| p + i);
        let close = html[i..].find("</div>").map(|p| p + i);
        match (open, close) {
            (Some(o), Some(c)) if o < c => {
                depth += 1;
                i = o + 4;
            }
            (_, Some(c)) => {
                depth -= 1;
                i = c + 6;
                if depth == 0 {
                    return i;
                }
            }
            _ => panic!("<div> sem fim"),
        }
    }
}

/// O início da etiqueta `<div …>` que contém `marker`.
fn div_with(html: &str, marker: &str) -> usize {
    let at = html
        .find(marker)
        .unwrap_or_else(|| panic!("{marker} em falta"));
    html[..at].rfind("<div").expect("<div")
}

/// Uma página de aplicação com duas janelas, pela API pública do Design.
fn page_with_windows() -> String {
    let win = |id: &str, active: bool, z: u16| WindowVm {
        id: id.into(),
        app_id: "files",
        app_href: "/files",
        href: "/files".into(),
        title: "Ficheiros".into(),
        subtitle: None,
        state: WindowState::Normal,
        active,
        z,
        geometry: WindowGeometry::default(),
        dirty: false,
        content: WindowContent::Pending,
    };
    let vm = ShellVm {
        display_name: "Ana".into(),
        core: Some(Health::Operational),
        wm: Some(WmVm {
            windows: vec![win("w1", false, 1), win("w2", true, 2)],
            switcher_hint: Some("Alt + W".into()),
            multi_window_apps: vec!["files"],
        }),
        ..Default::default()
    };
    shell::app_pending(&vm, "Ficheiros".into(), "/files").to_html()
}

/// A. O alternador desenha-se fora do `.oc-desk` (`isolation: isolate`):
/// dentro dele, o véu nunca cobriria a barra de cima.
#[test]
fn o_alternador_esta_fora_do_desktop_isolado() {
    let html = page_with_windows();
    let desk = div_with(&html, r#"class="oc-desk""#);
    let desk_end = end_of_div(&html, desk);
    let sw = html.find(r#"id="oc-switcher""#).expect("alternador");
    assert!(sw > desk_end, "o alternador voltou para dentro do .oc-desk");
    assert_eq!(html.matches(r#"id="oc-switcher""#).count(), 1);
    // E a camada das janelas continua dentro do Desktop.
    let layer = html.find(r#"data-oc="wm-layer""#).expect("camada");
    assert!(layer > desk && layer < desk_end);
}

/// C. O foco do diálogo de fechar só anda por controlos focáveis: um `[href]`
/// genérico apanha o `<use href>` dos ícones e parte o laço.
#[test]
fn os_focaveis_do_dialogo_nao_apanham_o_use_dos_icones() {
    // Num seletor, `[href]` sem elemento vem depois de aspas, vírgula, espaço
    // ou parêntese; `a[href]` (ou o comentário «[href]») não.
    for (i, _) in WM_JS.match_indices("[href]") {
        let before = WM_JS[..i].chars().next_back().unwrap_or(' ');
        assert!(
            !matches!(before, '\'' | '"' | ',' | ' ' | '('),
            "um seletor [href] sem elemento em oc-wm.js (antes: {before:?})"
        );
    }
    assert!(WM_JS.contains("const FOCUSABLE = 'a[href], button:not([disabled])"));
}

/// D. Com os painéis fechados, o `summary` é flex: sem linha de texto, a
/// pastilha e o relógio ficam onde a D001.2.1 os pôs (a pastilha descia 0,3px
/// e o relógio 1px com o `summary` em bloco).
#[test]
fn o_summary_dos_paineis_e_flex_como_os_controlos_d001() {
    let rule = WM_CSS
        .lines()
        .find(|l| l.trim_start().starts_with(".oc-panel-menu > summary {"))
        .expect("a regra do summary dos painéis desapareceu");
    assert!(
        rule.contains("display: flex") && rule.contains("align-items: center"),
        "{rule}"
    );
}
