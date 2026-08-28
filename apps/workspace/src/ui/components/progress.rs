//! Progresso: barra e donut.

use leptos::prelude::*;

/// Uma barra de progresso com percentagem ao lado.
///
/// `role="progressbar"` com os valores reais: quem usa leitor de ecrã ouve a
/// percentagem, não uma barra sem significado.
pub fn progress_bar(pct: u8) -> impl IntoView {
    let pct = pct.min(100);
    view! {
        <div
            class="oc-progress"
            data-pct=pct.to_string()
            role="progressbar"
            aria-valuenow=pct.to_string()
            aria-valuemin="0"
            aria-valuemax="100"
        >
            <div class="oc-progress__track">
                <div class="oc-progress__fill"></div>
            </div>
            <span class="oc-progress__pct">{format!("{pct}%")}</span>
        </div>
    }
}

/// Um donut de progresso, para o cabeçalho de um projecto.
pub fn donut(pct: u8) -> impl IntoView {
    let pct = pct.min(100);
    view! {
        <div
            class="oc-donut"
            data-pct=pct.to_string()
            role="progressbar"
            aria-valuenow=pct.to_string()
            aria-valuemin="0"
            aria-valuemax="100"
        >
            <div class="oc-donut__core">{format!("{pct}%")}</div>
        </div>
    }
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
    fn a_largura_da_barra_e_declaravel_para_qualquer_valor() {
        // A percentagem é o único valor contínuo da interface, e o CSS enumera
        // os 101 possíveis. Um que faltasse deixaria a barra a zero em silêncio.
        let css = include_str!("../../../static/ocinye.css");
        for pct in [0_u8, 1, 37, 99, 100] {
            assert!(
                css.contains(&format!(r#"[data-pct="{pct}"]"#)),
                "falta a regra de {pct}%"
            );
        }
    }

    #[test]
    fn o_progresso_e_anunciado_a_um_leitor_de_ecra() {
        let html = progress_bar(65).to_html();
        assert!(html.contains("role=\"progressbar\""));
        assert!(html.contains("aria-valuenow=\"65\""));
    }
}
