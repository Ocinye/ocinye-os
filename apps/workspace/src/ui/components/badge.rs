//! Badges e pills.
//!
//! **O estado nunca é comunicado só por cor**: todos os badges levam ponto e
//! texto (`design/README.md` §7.3). Quem não distingue as cores lê a etiqueta.

use leptos::prelude::*;

/// Os sete tons do design.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    /// Activa, Active, Concluída, PUBLIC, OK, Aberto, Project Candidate.
    Ok,
    /// Review, Revisão, Workspace.
    Gold,
    /// Concept, Completed, INTERNAL, Institucional.
    Navy,
    /// Exploration, Em curso, Unidade.
    Blue,
    /// Discovery, Draft, Archived, Suspensa, Desactivado, Pessoal, Baixa.
    Gray,
    /// On Hold, Pausado, CONFIDENTIAL, Restrito, AVISO.
    Warn,
    /// RESTRICTED, Negado, NEGADO, Alta prioridade.
    Err,
}

impl Tone {
    const fn class(self) -> &'static str {
        match self {
            Self::Ok => "oc-badge oc-badge--ok",
            Self::Gold => "oc-badge oc-badge--gold",
            Self::Navy => "oc-badge oc-badge--navy",
            Self::Blue => "oc-badge oc-badge--blue",
            Self::Gray => "oc-badge oc-badge--gray",
            Self::Warn => "oc-badge oc-badge--warn",
            Self::Err => "oc-badge oc-badge--err",
        }
    }

    /// O tom de um rótulo conhecido do domínio.
    ///
    /// Um rótulo desconhecido devolve `Gray` em vez de nada: um estado que não
    /// sabemos classificar continua a ser mostrado, apenas sem sugerir um
    /// significado que não temos.
    #[must_use]
    pub fn of(label: &str) -> Self {
        match label.trim().to_ascii_uppercase().as_str() {
            // Classificação
            "PUBLIC" => Self::Ok,
            "INTERNAL" => Self::Navy,
            "CONFIDENTIAL" => Self::Warn,
            "RESTRICTED" => Self::Err,

            // Estados de Idea
            "DISCOVERY" => Self::Gray,
            "EXPLORATION" => Self::Blue,
            "CONCEPT" => Self::Navy,
            "REVIEW" | "REVISÃO" | "PROJECT_CANDIDATE" => Self::Gold,
            "PROJECT CANDIDATE" => Self::Ok,

            // Estados de Project
            "DRAFT" | "ARCHIVED" | "ARQUIVADA" | "ARQUIVADO" => Self::Gray,
            "ACTIVE" | "ACTIVA" | "ACTIVO" | "OK" | "CONCLUÍDA" | "CONCLUÍDO" | "ABERTO" => {
                Self::Ok
            }
            "ON HOLD" | "ON_HOLD" | "PAUSADO" | "AVISO" | "RESTRITO" => Self::Warn,
            "COMPLETED" => Self::Navy,

            // Ciclo científico
            "ABERTA" | "REGISTADO" | "PLANEADO" | "RASCUNHO" => Self::Gray,
            "A CORRER" | "EM REVISÃO" => Self::Blue,
            "SUSTENTADA" | "VALIDADO" | "EM VIGOR" => Self::Ok,
            "REFUTADA" | "INVALIDADO" => Self::Err,
            "INCONCLUSIVA" | "ABANDONADO" => Self::Warn,
            "RETIRADA" | "SUBSTITUÍDA" | "SUBSTITUÍDO" => Self::Gray,

            // Operacional
            "EM CURSO" | "UNIDADE" => Self::Blue,
            "SUSPENSA" | "DESACTIVADO" | "PESSOAL" | "BAIXA" => Self::Gray,
            "WORKSPACE" => Self::Gold,
            "INSTITUCIONAL" => Self::Navy,
            "NEGADO" | "ALTA" => Self::Err,

            _ => Self::Gray,
        }
    }
}

/// Um badge: ponto mais texto, sempre.
pub fn badge(label: impl Into<String>, tone: Tone) -> impl IntoView {
    let label = label.into();
    view! {
        <span class=tone.class()>
            <i></i>
            {label}
        </span>
    }
}

/// Um badge de classificação.
///
/// As quatro classificações são visíveis em Dataset, Document, Idea, Project e
/// no contexto de IA (`design/README.md` §7.3). O `title` diz o que a sigla
/// significa, para quem a vê pela primeira vez.
pub fn classification_badge(classification: &str) -> impl IntoView {
    let label = classification.trim().to_ascii_uppercase();
    let explanation = match label.as_str() {
        "PUBLIC" => "Publicável fora da instituição",
        "INTERNAL" => "Legível por qualquer membro activo",
        "CONFIDENTIAL" => "Requer pertença à unidade ou ao workspace",
        "RESTRICTED" => "Requer pertença explícita ao workspace",
        _ => "Classificação",
    };
    let tone = Tone::of(&label);
    let title = format!("{label} — {explanation}");

    view! {
        <span class=tone.class() title=title>
            <i></i>
            {label}
        </span>
    }
}

/// Uma pill neutra de tipo ou código.
pub fn pill(label: impl Into<String>) -> impl IntoView {
    let label = label.into();
    view! { <span class="oc-pill">{label}</span> }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn as_quatro_classificacoes_tem_tons_distintos() {
        let tones = [
            Tone::of("PUBLIC"),
            Tone::of("INTERNAL"),
            Tone::of("CONFIDENTIAL"),
            Tone::of("RESTRICTED"),
        ];
        for (i, a) in tones.iter().enumerate() {
            for b in tones.iter().skip(i + 1) {
                assert_ne!(a, b, "duas classificações partilham o mesmo tom");
            }
        }
    }

    #[test]
    fn restricted_usa_o_tom_de_maior_alerta() {
        assert_eq!(Tone::of("RESTRICTED"), Tone::Err);
        assert_eq!(Tone::of("CONFIDENTIAL"), Tone::Warn);
    }

    #[test]
    fn os_estados_de_idea_seguem_o_design() {
        assert_eq!(Tone::of("Discovery"), Tone::Gray);
        assert_eq!(Tone::of("Exploration"), Tone::Blue);
        assert_eq!(Tone::of("Concept"), Tone::Navy);
        assert_eq!(Tone::of("Review"), Tone::Gold);
        assert_eq!(Tone::of("Project Candidate"), Tone::Ok);
    }

    #[test]
    fn um_rotulo_desconhecido_e_mostrado_sem_sugerir_significado() {
        assert_eq!(Tone::of("qualquer coisa nova"), Tone::Gray);
    }

    #[test]
    fn a_classificacao_e_sempre_maiuscula() {
        let html = classification_badge("restricted").to_html();
        assert!(html.contains("RESTRICTED"));
        assert!(!html.contains(">restricted<"));
    }
}
