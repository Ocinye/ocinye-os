//! As regras visuais da D001.2 que a integração verificou no browser, presas ao
//! CSS do Design para que uma revisão seguinte não as desfaça sem ninguém ver.
//!
//! Não são testes de píxeis: lêem as regras que produzem o comportamento medido
//! (`docs/ui/design-integration.json`). Uma revisão do Design que mude uma
//! destas decisões de propósito actualiza este ficheiro na mesma integração.

const SHELL: &str = include_str!("../static/oc-shell.css");
const DESK: &str = include_str!("../static/oc-desk.css");

/// A declaração de uma regra, pelo selector exacto no início da linha.
fn rule<'a>(css: &'a str, selector: &str) -> &'a str {
    css.lines()
        .find(|l| l.trim_start().starts_with(&format!("{selector} {{")))
        .unwrap_or_else(|| panic!("a regra `{selector}` desapareceu"))
}

#[test]
fn a_pastilha_core_ia_ve_se_sem_passar_o_rato() {
    let normal = rule(SHELL, ".oc-status");
    assert!(
        normal.contains("background: #F3F6F9"),
        "a pastilha voltou a ser transparente no estado normal: {normal}"
    );
    assert!(
        SHELL.contains(".oc-status:focus-visible"),
        "sem foco visível"
    );
}

#[test]
fn sem_a_barra_de_aplicacoes_a_grelha_ganha_a_margem_do_desktop() {
    assert!(
        SHELL.contains(r#".oc-desk[data-dock="hidden"] .oc-desk__main { padding-left: 30px; }"#)
    );
}

#[test]
fn as_colunas_respondem_a_area_de_trabalho_e_nao_a_janela() {
    // 4 colunas só com a área de trabalho ≥ 960px: a 924×540 são 2.
    assert!(DESK.contains("@container oc-desk-main (max-width: 959px)"));
}

/// Os Indicadores são exactamente 4 ou 2 por linha (D001.2.1): com `auto-fit`
/// havia uma faixa de 640–643px de área em que ficavam 3 + 1 e o quarto era
/// cortado. Quatro precisam de 642px dentro de um widget com 2px de contorno.
#[test]
fn os_indicadores_sao_quatro_ou_dois_nunca_tres_mais_um() {
    let kpis = rule(DESK, ".oc-kpis");
    assert!(!kpis.contains("auto-fit"), "{kpis}");
    assert!(kpis.contains("repeat(4, minmax(0, 1fr))"), "{kpis}");
    assert!(DESK.contains("@container oc-desk-main (max-width: 643.98px)"));
}

#[test]
fn o_titulo_de_um_widget_parte_em_linhas_e_nao_e_cortado() {
    let titulo = rule(DESK, ".oc-dw__head h2");
    assert!(!titulo.contains("text-overflow: ellipsis"), "{titulo}");
    assert!(titulo.contains("white-space: normal") && titulo.contains("line-clamp: 2"));
    let sub = rule(DESK, ".oc-dw__sub");
    assert!(!sub.contains("text-overflow: ellipsis"), "{sub}");
}
