//! Verificações comuns aos testes de vista.

/// Falha se o HTML violar as regras de CSP ou de interacção.
pub(crate) fn assert_contracts(html: &str) {
    assert!(!html.contains(&["sty", "le="].concat()), "style inline");
    assert!(!html.contains("<script>"), "script inline");
    for evento in [" onclick=", " onsubmit=", " oninput=", " onchange="] {
        assert!(!html.contains(evento), "handler inline {evento}");
    }
    // Cada <button> submete, tem data-oc ou aria-disabled.
    for (i, _) in html.match_indices("<button") {
        let fim = i + html[i..].find('>').expect("fim de <button>");
        let etiqueta = &html[i..fim];
        assert!(
            etiqueta.contains(r#"type="submit""#)
                || etiqueta.contains("data-oc=")
                || etiqueta.contains(r#"aria-disabled="true""#),
            "botão sem comportamento: {etiqueta}"
        );
    }
}
