//! Texto para elementos de texto cru do HTML.
//!
//! O Leptos 0.8, no servidor, escreve os filhos de um `<textarea>` tal como
//! chegam: `</textarea><script>…` no corpo de uma nota ou de um rascunho
//! sairia do campo e correria como código. Dentro de um `<textarea>` o browser
//! descodifica as referências de carácter, por isso escapar `&`, `<` e `>`
//! mostra ao membro exactamente o texto que ele escreveu e nunca fecha o
//! elemento (D004, `docs/ui/CODE_FEEDBACK.md`).

/// O texto de um `<textarea>`, seguro e com o mesmo significado.
#[must_use]
pub fn rcdata(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            other => out.push(other),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::rcdata;

    #[test]
    fn nada_fecha_o_campo() {
        let hostil = "</textarea><script>alert(1)</script>&amp;";
        let seguro = rcdata(hostil);
        assert!(!seguro.contains('<') && !seguro.contains('>'));
        assert_eq!(
            seguro,
            "&lt;/textarea&gt;&lt;script&gt;alert(1)&lt;/script&gt;&amp;amp;"
        );
        assert_eq!(rcdata("# Torre 2\n- item"), "# Torre 2\n- item");
    }
}
