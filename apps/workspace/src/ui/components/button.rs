//! Botões.
//!
//! Quatro variantes, todas de `design/README.md` §7.1. Um botão que navega
//! renderiza-se como `<a>`: tem de funcionar com clique do meio, com o teclado
//! e sem JavaScript.

use leptos::prelude::*;

use crate::ui::icon::{icon, Icon};

/// As variantes do design.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Variant {
    /// Acção institucional. Navy sobre branco.
    Primary,
    /// CTA de destaque. Sunrise Gold — nunca em áreas grandes.
    Gold,
    /// Acção secundária. Branco com borda.
    Secondary,
    /// Sobre fundo navy, dentro de cartões escuros.
    OnNavy,
}

impl Variant {
    const fn class(self) -> &'static str {
        match self {
            Self::Primary => "oc-btn oc-btn--primary",
            Self::Gold => "oc-btn oc-btn--gold",
            Self::Secondary => "oc-btn oc-btn--secondary",
            Self::OnNavy => "oc-btn oc-btn--on-navy",
        }
    }
}

/// Um botão do design.
pub struct Button {
    label: String,
    variant: Variant,
    /// Destino. Quando presente, renderiza um `<a>` em vez de um `<button>`.
    href: Option<String>,
    icon: Option<Icon>,
    /// Ponto dourado à esquerda do rótulo, como nas acções de IA.
    dot: bool,
    /// Por que razão a acção não está disponível, quando não está.
    ///
    /// A razão viaja com o botão porque **são várias**: o ecrã de destino pode
    /// não existir, ou a pessoa pode não ter a autorização que ele exige. Dizer
    /// «ainda não disponível» a quem apenas não tem acesso seria falso.
    unavailable: Option<String>,
}

impl Button {
    /// Um botão com rótulo e variante.
    #[must_use]
    pub fn new(label: impl Into<String>, variant: Variant) -> Self {
        Self {
            label: label.into(),
            variant,
            href: None,
            icon: None,
            dot: false,
            unavailable: None,
        }
    }

    /// Torna-o uma ligação.
    #[must_use]
    pub fn href(mut self, href: impl Into<String>) -> Self {
        self.href = Some(href.into());
        self
    }

    /// Acrescenta o ponto dourado de destaque.
    #[must_use]
    pub const fn with_dot(mut self) -> Self {
        self.dot = true;
        self
    }

    /// Marca a acção como ainda não disponível: o ecrã de destino não existe.
    ///
    /// Renderizá-la como ligação levaria a um 404; escondê-la contrariaria o
    /// design. Fica visível e declarada, tal como as tabs sem ecrã.
    #[must_use]
    pub fn not_yet_available(mut self) -> Self {
        self.unavailable = Some("Ainda não disponível".to_owned());
        self
    }

    /// Marca a acção como indisponível **por uma razão dada**.
    ///
    /// Usa-se quando a acção existe e é a pessoa que não lhe chega. Esconder o
    /// botão deixaria a interface a mudar de forma consoante quem olha, e quem
    /// não o vê não fica a saber que existe nem porque não o tem.
    #[must_use]
    pub fn unavailable_because(mut self, reason: impl Into<String>) -> Self {
        self.unavailable = Some(reason.into());
        self
    }
}

/// Renderiza um botão.
pub fn button(spec: Button) -> impl IntoView {
    let Button {
        label,
        variant,
        href,
        icon: kind,
        dot,
        unavailable,
    } = spec;
    let class = variant.class();

    // The inner view is built per branch rather than cloned: a Leptos view is
    // not Clone, and duplicating the construction is cheaper than boxing it.
    let inner = |label: String| {
        view! {
            {dot.then(|| view! { <span class="oc-btn__dot"></span> })}
            {kind.map(|k| icon(k, 13))}
            {label}
        }
    };

    if let Some(reason) = unavailable {
        let content = inner(label.clone());
        return view! {
            <span
                class=format!("{class} oc-unavailable")
                aria-disabled="true"
                title=reason
            >
                {content}
            </span>
        }
        .into_any();
    }

    // Sem destino, é um `<button type="submit">`.
    //
    // Durante algum tempo foi `type="button"`, e isso é o único valor de `type`
    // que garante que nada acontece: dentro de um formulário, `button` é
    // precisamente o botão que **não** submete. «Criar Unidade», «Criar Ideia»,
    // «Criar Referência», «Criar Dataset», «Promover a Projecto» e «Mudar
    // palavra-passe» estavam todos assim — desenhados, alcançáveis pelo
    // teclado, e mudos.
    //
    // `submit` é também o que o HTML faz sozinho com um `<button>` sem `type`
    // dentro de um formulário; era `button` que era o desvio. Um botão deste
    // componente que não seja um link nem tenha `data-oc` existe para submeter
    // alguma coisa — se não existir formulário à volta dele, é o formulário que
    // falta, e o teste estrutural passou a exigi-lo.
    href.map_or_else(
        || {
            let content = inner(label.clone());
            view! { <button type="submit" class=class>{content}</button> }.into_any()
        },
        |href| {
            let content = inner(label.clone());
            view! { <a class=class href=href>{content}</a> }.into_any()
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn um_botao_com_destino_e_uma_ligacao() {
        let html = button(Button::new("Ver", Variant::Secondary).href("/units")).to_html();
        assert!(html.contains("<a"));
        assert!(html.contains("href=\"/units\""));
    }

    /// Um botão sem destino submete.
    ///
    /// Este teste afirmava o contrário, e é por isso que o defeito sobreviveu:
    /// verificava que o componente emitia `type="button"` — precisamente o
    /// valor que, dentro de um formulário, garante que nada acontece. Passava,
    /// e seis acções de criação não funcionavam.
    ///
    /// Um teste que fixa o comportamento errado é pior do que não haver teste:
    /// dá-lhe a aparência de intenção.
    #[test]
    fn um_botao_sem_destino_submete() {
        let html = button(Button::new("Filtrar", Variant::Secondary)).to_html();
        assert!(html.contains("<button"));
        assert!(
            html.contains("type=\"submit\""),
            "um botão sem destino tem de submeter o formulário que o contém: {html}"
        );
    }
}
