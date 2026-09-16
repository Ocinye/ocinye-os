//! Validation of human-facing institutional identifiers.
//!
//! These codes appear in citations, in file names and in conversation. They are
//! deliberately constrained so they stay unambiguous, and validated here rather
//! than at each call site.

use crate::error::{DomainError, DomainResult};

/// Longest permitted unit code.
pub(crate) const UNIT_CODE_MAX: usize = 16;
/// Longest permitted compute node identifier.
pub(crate) const NODE_IDENTIFIER_MAX: usize = 24;
/// Longest permitted project code.
pub(crate) const PROJECT_CODE_MAX: usize = 32;

fn validate_code(raw: &str, max: usize, label: &str) -> DomainResult<String> {
    let code = raw.trim().to_ascii_uppercase();

    if code.len() < 2 || code.len() > max {
        return Err(DomainError::Validation(format!(
            "A {label} must be between 2 and {max} characters."
        )));
    }
    if !code.starts_with(|c: char| c.is_ascii_alphabetic()) {
        return Err(DomainError::Validation(format!(
            "A {label} must start with a letter."
        )));
    }
    if !code.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
        return Err(DomainError::Validation(format!(
            "A {label} may contain only letters, digits and hyphens."
        )));
    }
    if code.ends_with('-') || code.contains("--") {
        return Err(DomainError::Validation(format!(
            "A {label} must not end with a hyphen or contain consecutive hyphens."
        )));
    }
    Ok(code)
}

/// Validate and normalise a unit code, for example `AI` or `ENERGY-SYS`.
///
/// # Errors
///
/// Returns [`DomainError::Validation`] when the code is malformed.
pub fn validate_unit_code(raw: &str) -> DomainResult<String> {
    validate_code(raw, UNIT_CODE_MAX, "unit code")
}

/// Validate and normalise a project code.
///
/// # Errors
///
/// Returns [`DomainError::Validation`] when the code is malformed.
pub fn validate_project_code(raw: &str) -> DomainResult<String> {
    validate_code(raw, PROJECT_CODE_MAX, "project code")
}

/// Validate and normalise a compute node identifier, for example `CAM-01`.
///
/// The identifier is supplied at registration. No node identifier is ever
/// hardcoded anywhere in the system (ADR-0500).
///
/// # Errors
///
/// Returns [`DomainError::Validation`] when the identifier is malformed.
pub fn validate_node_identifier(raw: &str) -> DomainResult<String> {
    validate_code(raw, NODE_IDENTIFIER_MAX, "node identifier")
}

/// As palavras que uma abreviatura de unidade ignora: preposições, artigos, e o
/// próprio «Unidade». Uma abreviatura nunca se forma a partir destas.
const UNIT_NAME_STOPWORDS: &[&str] = &[
    "DE", "DA", "DO", "DAS", "DOS", "E", "A", "O", "AS", "OS", "EM", "PARA", "COM", "POR", "NO",
    "NA", "NOS", "NAS", "UNIDADE",
];

/// Dobra um carácter para a sua base ASCII (acentos do português), ou devolve-o.
fn fold_ascii(c: char) -> char {
    match c {
        'Á' | 'À' | 'Â' | 'Ã' | 'Ä' | 'á' | 'à' | 'â' | 'ã' | 'ä' => 'A',
        'É' | 'Ê' | 'È' | 'Ë' | 'é' | 'ê' | 'è' | 'ë' => 'E',
        'Í' | 'Î' | 'Ì' | 'Ï' | 'í' | 'î' | 'ì' | 'ï' => 'I',
        'Ó' | 'Ô' | 'Ò' | 'Õ' | 'Ö' | 'ó' | 'ô' | 'ò' | 'õ' | 'ö' => 'O',
        'Ú' | 'Û' | 'Ù' | 'Ü' | 'ú' | 'û' | 'ù' | 'ü' => 'U',
        'Ç' | 'ç' => 'C',
        'Ñ' | 'ñ' => 'N',
        outro => outro,
    }
}

/// A abreviatura institucional de um nome de unidade — o **radical** do código,
/// com o prefixo `U`, sem o número.
///
/// `Computação e Sistemas` → `UCS`; `Robótica` → `UROB`; `Unidade de Energias
/// Renováveis` → `UER`. Determinística: o mesmo nome dá sempre o mesmo radical.
/// O número (`-001`, `-002`) é alocado por quem chama, contra o que já existe,
/// para esta função ficar pura e testável.
///
/// Nunca forma o radical a partir de *stopwords* (`de`, `da`, `do`, `e`), nem do
/// próprio «Unidade»; um nome que se reduz a nada cai em `UNI`. O resultado é um
/// radical válido (começa por letra, só `[A-Z]`), curto o suficiente para o
/// sufixo `-NNN` manter o código dentro de 16 caracteres.
///
/// **Os códigos das unidades iniciais não saem daqui** — são dados de *seed*,
/// escolhidos pela instituição (ex.: `UAI` para «Inteligência Artificial», que
/// esta função abreviaria como `UIA`). O gerador serve as unidades futuras.
#[must_use]
pub fn unit_code_stem(name: &str) -> String {
    let normalizado: String = name
        .chars()
        .map(fold_ascii)
        .collect::<String>()
        .to_ascii_uppercase();

    let tokens: Vec<&str> = normalizado
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|t| t.chars().any(|c| c.is_ascii_alphabetic()))
        .filter(|t| !UNIT_NAME_STOPWORDS.contains(t))
        .collect();

    if tokens.is_empty() {
        return "UNI".to_owned();
    }

    let abreviatura: String = if tokens.len() == 1 {
        // Uma palavra: as três primeiras letras (Robótica → ROB).
        tokens[0]
            .chars()
            .filter(char::is_ascii_alphabetic)
            .take(3)
            .collect()
    } else {
        // Várias: a inicial de cada, até quatro (Computação Sistemas → CS).
        tokens
            .iter()
            .filter_map(|t| t.chars().find(|c| c.is_ascii_alphabetic()))
            .take(4)
            .collect()
    };

    let abreviatura: String = abreviatura
        .chars()
        .filter(char::is_ascii_alphabetic)
        .take(8)
        .collect();
    if abreviatura.is_empty() {
        return "UNI".to_owned();
    }
    format!("U{abreviatura}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_and_normalises_realistic_codes() {
        assert_eq!(validate_unit_code(" ai ").unwrap(), "AI");
        assert_eq!(validate_unit_code("energy-sys").unwrap(), "ENERGY-SYS");
        assert_eq!(validate_node_identifier("cam-01").unwrap(), "CAM-01");
    }

    #[test]
    fn rejects_malformed_codes() {
        for bad in [
            "",
            "A",
            "1AI",
            "AI_X",
            "AI--X",
            "AI-",
            "  ",
            &"A".repeat(64),
        ] {
            assert!(validate_unit_code(bad).is_err(), "should reject {bad:?}");
        }
    }

    #[test]
    fn node_identifiers_are_not_privileged_by_name() {
        // CAM-01 is just a value: nothing in validation treats it specially.
        assert_eq!(validate_node_identifier("CAM-01").unwrap(), "CAM-01");
        assert_eq!(validate_node_identifier("HPC-99").unwrap(), "HPC-99");
    }

    #[test]
    fn unit_code_stem_reduz_nomes_reais_a_um_radical_deterministico() {
        // Vários tokens significativos → inicial de cada.
        assert_eq!(unit_code_stem("Computação e Sistemas"), "UCS");
        assert_eq!(unit_code_stem("Dados e Conhecimento"), "UDC");
        assert_eq!(unit_code_stem("Investigação e Desenvolvimento"), "UID");
        assert_eq!(unit_code_stem("Inteligência Artificial"), "UIA");
        // «Unidade de ...» é descartado; sobra o essencial.
        assert_eq!(unit_code_stem("Unidade de Energias Renováveis"), "UER");
        // Um só token significativo → três primeiras letras.
        assert_eq!(unit_code_stem("Unidade de Robótica"), "UROB");
        assert_eq!(unit_code_stem("Unidade de Biotecnologia"), "UBIO");
    }

    #[test]
    fn unit_code_stem_e_estavel_perante_acentos_espacos_e_caixa() {
        // O mesmo nome, escrito de formas diferentes, dá o mesmo radical.
        assert_eq!(unit_code_stem("  robótica  "), "UROB");
        assert_eq!(unit_code_stem("ROBOTICA"), "UROB");
        assert_eq!(unit_code_stem("Robotica"), "UROB");
    }

    #[test]
    fn unit_code_stem_nunca_se_forma_de_stopwords() {
        // Um nome só de palavras vazias não gera abreviatura absurda: cai em UNI.
        assert_eq!(unit_code_stem("de da do e"), "UNI");
        assert_eq!(unit_code_stem("Unidade"), "UNI");
        assert_eq!(unit_code_stem("   "), "UNI");
    }

    #[test]
    fn um_radical_gerado_com_sufixo_e_um_codigo_valido() {
        // O radical compõe-se com `-NNN` num código que a validação aceita, e
        // que cabe no limite de 16 caracteres.
        for nome in [
            "Computação e Sistemas",
            "Investigação e Desenvolvimento",
            "Unidade de Energias Renováveis",
            "Unidade de Biotecnologia Molecular Aplicada Avançada",
        ] {
            let stem = unit_code_stem(nome);
            let codigo = format!("{stem}-001");
            assert!(codigo.len() <= UNIT_CODE_MAX, "código {codigo:?} excede 16");
            assert_eq!(
                validate_unit_code(&codigo).unwrap(),
                codigo,
                "código gerado {codigo:?} devia ser válido"
            );
        }
    }
}
