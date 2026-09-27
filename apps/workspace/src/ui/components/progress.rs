//! Progresso: barra e donut.

use leptos::prelude::*;

/// Uma barra de progresso com percentagem ao lado.
///
/// `role="progressbar"` com os valores reais: quem usa leitor de ecrã ouve a
/// percentagem, não uma barra sem significado.
pub fn progress_bar(pct: u8) -> impl IntoView {
    let pct = pct.min(100);
    // A barra do D1: a largura sai de `data-ods-value`, posta pelo `app.js`.
    view! {
        <div
            class="ods-progress"
            data-pct=pct.to_string()
            data-ods-value=pct.to_string()
            role="progressbar"
            aria-valuenow=pct.to_string()
            aria-valuemin="0"
            aria-valuemax="100"
        >
            <div class="ods-progress__bar"></div>
        </div>
        <span class="ods-label">{format!("{pct}%")}</span>
    }
}

/// Um donut de progresso, para o cabeçalho de um projecto.
pub fn donut(pct: u8) -> impl IntoView {
    // O D1 não tem anel de progresso: a mesma barra, com o valor escrito.
    progress_bar(pct)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_percentagem_e_limitada_a_cem() {
        let html = progress_bar(180).to_html();
        // A largura vem de `data-pct`, e não de um `style` inline: a CSP do
        // Workspace descarta esse atributo, e a barra ficaria sempre a zero.
        assert!(html.contains(r#"data-pct="100""#));
        assert!(html.contains(r#"aria-valuenow="100""#));
    }

    #[test]
    fn o_progresso_e_anunciado_a_um_leitor_de_ecra() {
        let html = progress_bar(65).to_html();
        assert!(html.contains("role=\"progressbar\""));
        assert!(html.contains("aria-valuenow=\"65\""));
    }
}
