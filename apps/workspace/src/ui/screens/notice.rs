//! Ecrãs de excepção: caminho inexistente e falha inesperada.
//!
//! # Porque existem
//!
//! Sem eles, um caminho desconhecido devolvia o 404 vazio do Axum — uma página
//! em branco com o estilo do framework e não do Ocinye OS (briefing §75). Uma
//! falha do Core devolvia um erro cru pela mesma razão (§76).
//!
//! # O que não fazem
//!
//! Não mostram detalhe técnico. Nem stack trace, nem SQL, nem enum interno.
//! Levam o identificador de correlação, que é o que permite investigar sem
//! expor nada (briefing §47).

use leptos::prelude::*;

use crate::ui::components::{button, Button, Variant};
use crate::ui::icon::{icon, Icon};

/// Caminho inexistente.
///
/// Deliberadamente sem o caminho pedido no corpo: ecoá-lo devolveria texto do
/// utilizador para dentro da página, e não acrescenta nada que ele não saiba.
pub fn not_found() -> impl IntoView {
    view! {
        <div class="oc-notice">
            <span class="oc-notice__tile">{icon(Icon::EmptyState, 26)}</span>
            <h1>"Página não encontrada"</h1>
            <p>
                "Este endereço não corresponde a nenhum ecrã do Ocinye Workspace. Pode ter sido
                 movido, ou o endereço pode estar incompleto."
            </p>
            <div class="oc-row oc-gap-5">
                {button(Button::new("Ir para a Home", Variant::Primary).href("/"))}
                {button(Button::new("O Meu Trabalho", Variant::Secondary).href("/my-work"))}
            </div>
        </div>
    }
}

/// Falha inesperada, com a referência para investigação.
pub fn failure(correlation_id: &str) -> impl IntoView {
    let reference = correlation_id.to_owned();

    view! {
        <div class="oc-notice">
            <span class="oc-notice__tile">{icon(Icon::Shield, 26)}</span>
            <h1>"Ocorreu um erro inesperado"</h1>
            <p>
                "A operação não foi concluída. Nada foi alterado. Se o problema persistir,
                 indique a referência abaixo a quem opera o Ocinye OS."
            </p>
            <p class="oc-mono oc-notice__reference">"Referência: "{reference}</p>
            <div class="oc-row oc-gap-5">
                {button(Button::new("Ir para a Home", Variant::Primary).href("/"))}
            </div>
        </div>
    }
}

/// Recusa de acesso.
///
/// Usada onde a existência do recurso não é segredo. Onde for, o Core devolve
/// `not_found` e é [`not_found`] que aparece (ADR-0100).
pub fn access_denied() -> impl IntoView {
    view! {
        <div class="oc-notice">
            <span class="oc-notice__tile">{icon(Icon::Shield, 26)}</span>
            <h1>"Não possui acesso a este recurso"</h1>
            <p>
                "O seu acesso é definido pelas unidades e Research Workspaces de que faz parte.
                 Se precisa deste recurso para o seu trabalho, peça acesso a quem administra a
                 sua unidade."
            </p>
            <div class="oc-row oc-gap-5">
                {button(Button::new("O Meu Trabalho", Variant::Primary).href("/my-work"))}
            </div>
        </div>
    }
}

/// Uma dependência da operação não está de pé.
///
/// # Porque não é o ecrã de erro
///
/// O ecrã de erro pede que se avise alguém e dá uma referência para o log: é
/// para o que ninguém esperava. Isto é outra coisa — a capacidade existe, o
/// produto sabe fazê-la, e a instalação é que não tem uma peça a responder.
///
/// A Ajuda separa os dois estados para o membro, e a interface tem de os
/// separar também: um pede que se volte mais tarde, o outro que se reporte.
///
/// # A razão vem do Core, quando ele a der
///
/// O Core sabe **qual** é a peça que falta, e escreve-o: «o correio
/// institucional ainda não foi configurado nesta instalação». O Workspace
/// deitava essa frase fora e mostrava só o parágrafo genérico, que acaba em
/// «quem administra o sistema saberá o que falta» — e quem estava a ler era,
/// muitas vezes, precisamente quem administra o sistema.
pub fn unavailable(razao: Option<String>) -> impl IntoView {
    // A frase do Core substitui a genérica, e não se acumula com ela: duas
    // explicações da mesma coisa lêem-se como se fossem duas coisas.
    let explicacao = razao.unwrap_or_else(|| {
        "Um serviço de que esta operação depende não está a responder nesta \
         instalação — quem administra o sistema saberá qual."
            .to_owned()
    });
    view! {
        <div class="oc-notice">
            <span class="oc-notice__tile">{icon(Icon::SystemStatus, 26)}</span>
            <h1>"Esta operação não está disponível agora"</h1>
            <p>{explicacao}</p>
            <p class="oc-notice__aside">
                "A capacidade existe no Ocinye OS. Não é um problema com o que fez nem
                 com o seu acesso."
            </p>
            <div class="oc-row oc-gap-5">
                {button(Button::new("O Meu Trabalho", Variant::Primary).href("/my-work"))}
            </div>
        </div>
    }
}

/// O Core percebeu o pedido e recusou-o pelo conteúdo.
///
/// # Porque não é um erro
///
/// Porque nada correu mal. A operação foi entendida e não pode acontecer como
/// foi pedida — e o Core diz porquê, numa frase escrita para quem a lê. Uma
/// página de erro com uma referência de log mandaria a pessoa perguntar a
/// alguém aquilo que a frase já responde.
pub fn rejected(razao: &str) -> impl IntoView {
    let razao = razao.to_owned();
    view! {
        <div class="oc-notice">
            <span class="oc-notice__tile">{icon(Icon::SystemStatus, 26)}</span>
            <h1>"O pedido não foi aceite"</h1>
            <p>{razao}</p>
            <p class="oc-notice__aside">
                "Nada correu mal. O Ocinye OS percebeu o pedido e não o pode registar
                 tal como foi feito."
            </p>
            <div class="oc-row oc-gap-5">
                {button(Button::new("O Meu Trabalho", Variant::Primary).href("/my-work"))}
            </div>
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A razão que o Core deu chega ao membro.
    ///
    /// # O defeito que isto guarda
    ///
    /// O Core respondeu «o correio institucional ainda não foi configurado
    /// nesta instalação do Ocinye OS» — a frase que diz exactamente o que
    /// falta. O Workspace mapeava o `503` para uma variante sem campos, a
    /// frase morria no cliente, e a página dizia «quem administra o sistema
    /// saberá o que falta» a quem administrava o sistema.
    #[test]
    fn a_razao_do_core_aparece_em_vez_da_frase_generica() {
        let razao = "O correio institucional ainda não foi configurado nesta \
                     instalação do Ocinye OS.";
        let html = unavailable(Some(razao.to_owned())).to_html();

        assert!(html.contains(razao), "a razão do Core não chegou ao ecrã");
        assert!(
            !html.contains("saberá qual"),
            "a frase genérica ficou por baixo da razão; são duas explicações da \
             mesma coisa e lêem-se como duas coisas"
        );
        // O que continua verdadeiro seja qual for a razão.
        assert!(html.contains("Não é um problema com o que fez"));
    }

    /// Sem razão, a página continua a dizer alguma coisa.
    #[test]
    fn sem_razao_a_pagina_explica_o_que_pode() {
        let html = unavailable(None).to_html();
        assert!(html.contains("não está a responder nesta instalação"));
        assert!(html.contains("Esta operação não está disponível agora"));
    }

    #[test]
    fn o_404_oferece_uma_saida_e_nao_ecoa_o_caminho() {
        let html = not_found().to_html();
        assert!(html.contains("Página não encontrada"));
        assert!(html.contains(r#"href="/""#));
        // Sem "Oops", sem "Under construction" (briefing §59).
        for banido in ["Oops", "Coming soon", "Under construction", "404"] {
            assert!(!html.contains(banido), "o ecrã usa «{banido}»");
        }
    }

    #[test]
    fn a_falha_leva_a_referencia_e_nenhum_detalhe_tecnico() {
        let html = failure("abc123").to_html();
        assert!(html.contains("Referência: "));
        assert!(html.contains("abc123"));
        assert!(html.contains("Nada foi alterado."));
        for banido in ["panic", "SQL", "unwrap", "Error {", "sqlx"] {
            assert!(!html.contains(banido), "o ecrã expõe «{banido}»");
        }
    }

    #[test]
    fn a_recusa_diz_o_que_fazer_a_seguir() {
        // Um beco sem saída é pior do que uma recusa (briefing §106).
        let html = access_denied().to_html();
        assert!(html.contains("peça acesso"));
        assert!(html.contains(r#"href="/my-work""#));
    }

    #[test]
    fn texto_interpolado_nao_injecta_markup() {
        let html = failure("<script>alert(1)</script>").to_html();
        assert!(!html.contains("<script>alert(1)</script>"));
    }
}
