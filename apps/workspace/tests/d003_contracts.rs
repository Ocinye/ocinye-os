//! Guardas das correcções D003.1, semânticas e não de píxeis: o que as produz
//! no browser (medido na integração com teclado real, `docs/ui/design-integration.json`)
//! fica preso ao código do Design, para que uma revisão seguinte não o desfaça
//! sem ninguém ver. Uma revisão que mude uma destas decisões de propósito
//! actualiza este ficheiro na mesma integração.

const NYE_JS: &str = include_str!("../static/oc-nye.js");
const NYE_CSS: &str = include_str!("../static/oc-nye.css");

/// O corpo da função `name` no `oc-nye.js` (até à função seguinte ao mesmo nível).
fn function_body(name: &str) -> &'static str {
    let start = NYE_JS
        .find(&format!("function {name}("))
        .unwrap_or_else(|| panic!("function {name} em falta"));
    let rest = &NYE_JS[start + 1..];
    let end = rest
        .find("\n  function ")
        .or_else(|| rest.find("\n  $$("))
        .map_or(NYE_JS.len(), |e| start + 1 + e);
    &NYE_JS[start..end]
}

/// A. Aberta pelo servidor com um pedido (`/ask?q=…`), o filtro corre pelo
/// mesmo caminho que ao escrever: o evento `input`, e não um segundo filtro.
#[test]
fn o_pedido_inicial_filtra_pelo_mesmo_caminho_que_escrever() {
    let surface = function_body("surface");
    assert!(
        surface.contains("dispatchEvent(new Event('input'"),
        "o pedido inicial deixou de disparar o filtro do input"
    );
    assert!(
        surface.contains("if (q && q.value.trim()) refilter()"),
        "o filtro inicial deixou de depender do texto vindo do servidor"
    );
}

/// B e C. Aberta, a superfície é modal: o resto da casca fica `inert`, e
/// deixa de o estar ao fechar.
#[test]
fn aberta_o_resto_fica_inert_e_fechada_volta() {
    let trap = function_body("trap");
    assert!(
        trap.contains("setAttribute('inert', '')"),
        "a casca por trás não fica inert"
    );
    assert!(
        trap.contains("removeAttribute('inert')"),
        "o inert não sai ao fechar"
    );
    // Um diálogo bloqueante (fechar com alterações, confirmação) nunca fica inert.
    assert!(trap.contains(r#"[data-oc="dirty-close"], [data-oc="nye-confirm"]"#));
}

/// D. O foco só vai a controlos reais: nenhum seletor `[href]` sem elemento
/// (apanharia o `<use href>` dos ícones), e nada dentro de um SVG.
#[test]
fn o_foco_nunca_cai_num_href_de_svg() {
    let sel = NYE_JS
        .split("const FOCUSABLE_UI = '")
        .nth(1)
        .and_then(|r| r.split('\'').next())
        .expect("FOCUSABLE_UI em falta");
    for part in sel.split(',') {
        let part = part.trim();
        assert!(
            !part.starts_with("[href"),
            "seletor genérico {part} apanha o <use href>"
        );
    }
    assert!(sel.contains("a[href]") && sel.contains("button:not([disabled])"));
    assert!(NYE_JS.contains("!(el instanceof SVGElement)"));
    // E o Tab dá a volta dentro da superfície, nos dois sentidos.
    let trap = function_body("trap");
    assert!(
        trap.contains("e.shiftKey && i === 0")
            && trap.contains("!e.shiftKey && i === l.length - 1")
    );
}

/// E. Ecrã estreito: 44 px de área activa nos controlos da Nye.
#[test]
fn alvos_de_toque_de_44px_no_ecra_estreito() {
    let block = NYE_CSS
        .split("D003.1 · alvos de toque")
        .nth(1)
        .and_then(|r| r.split("@media (max-width: 640px) {").nth(1))
        .expect("bloco de alvos de toque em falta");
    let block = &block[..block.find("\n}\n").expect("fim do bloco")];
    assert!(block.contains("--oc-nye-touch: 44px"));
    assert!(block
        .contains("width: max(100%, var(--oc-nye-touch)); height: max(100%, var(--oc-nye-touch))"));
    for sel in [
        ".oc-nye-mode::after",
        ".oc-nye-icon-btn::after",
        ".oc-nye-send::after",
    ] {
        assert!(block.contains(sel), "{sel} sem área de 44px");
    }
    assert!(block.contains("a.oc-nyeu__continue { min-height: var(--oc-nye-touch)"));
}

/// F. Esc em dois tempos: num resultado volta ao campo (e a superfície fica
/// aberta); só no campo fecha. Corre na captura, antes do Esc da casca.
#[test]
fn esc_num_resultado_volta_ao_campo() {
    let trap = function_body("trap");
    let esc = trap
        .split("if (e.key === 'Escape') {")
        .nth(1)
        .expect("Esc em falta");
    let esc = &esc[..esc.find("return;").expect("return")];
    assert!(
        esc.contains("a !== q")
            && esc.contains("stopImmediatePropagation()")
            && esc.contains("q.focus()")
    );
    assert!(
        trap.contains("}, true);"),
        "o Esc deixou de correr na captura"
    );
    // Fechada sem navegar, o foco volta a quem abriu, ou ao campo da barra.
    assert!(trap.contains(
        "const back = opener && document.contains(opener) && visible(opener) ? opener : invoker();"
    ));
}
