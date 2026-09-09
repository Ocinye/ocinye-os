//! Catálogo humano dos papéis técnicos — rótulo e descrição, em português.
//!
//! # Porquê aqui, e não nos contratos
//!
//! O [`TechnicalRole`] é um contrato: um conjunto fechado, com uma representação
//! estável em inglês (`as_str`). O **rótulo** que uma pessoa lê é outra coisa —
//! é português europeu, é da Experience, e pode mudar sem que o contrato mude.
//! Por isso vive aqui, e não nos contratos: uma normalização de texto humano não
//! é uma alteração de esquema, de API nem de autorização.
//!
//! # Uma lista, não duas
//!
//! O seletor de criação e o crachá de acesso liam, cada um, a sua própria lista
//! de strings. Duas listas do mesmo facto divergem — foi assim que «Research
//! Lead» sobreviveu num sítio depois de se ter decidido «Líder de investigação».
//! Agora ambos lêem daqui. Um papel que este build não conhece mostra-se pelo
//! seu identificador: inventar uma tradução seria pior do que expor o código.
//!
//! Nada aqui concede acesso. O rótulo é texto; a autoridade é do Core (ADR-0100).

use ocinye_contracts::TechnicalRole;

/// Um papel técnico como uma pessoa o lê: o rótulo canónico e, quando ajuda, uma
/// descrição curta do que o papel é. A descrição fica vazia quando o rótulo se
/// explica sozinho — um «Administrador da plataforma» não precisa de glosa.
struct Rotulo {
    label: &'static str,
    description: &'static str,
}

/// A tradução canónica de um papel. É a **única** no Workspace.
const fn rotulo(role: TechnicalRole) -> Rotulo {
    match role {
        TechnicalRole::ResearchMember => Rotulo {
            label: "Investigador",
            description: "acesso científico comum",
        },
        TechnicalRole::ResearchLead => Rotulo {
            label: "Líder de investigação",
            description: "lidera ideias e projectos",
        },
        TechnicalRole::Collaborator => Rotulo {
            label: "Colaborador",
            description: "âmbito estreito",
        },
        TechnicalRole::ExternalCollaborator => Rotulo {
            label: "Colaborador externo",
            description: "só o que for atribuído",
        },
        TechnicalRole::UnitManager => Rotulo {
            label: "Gestor de unidade",
            description: "",
        },
        TechnicalRole::Auditor => Rotulo {
            label: "Auditor",
            description: "evidência, sem conteúdo",
        },
        TechnicalRole::OrganisationAdmin => Rotulo {
            label: "Administrador da organização",
            description: "",
        },
        TechnicalRole::PlatformAdmin => Rotulo {
            label: "Administrador da plataforma",
            description: "",
        },
    }
}

/// Os papéis oferecidos ao criar ou atribuir a um membro, ordenados do mais
/// estreito para o mais amplo — a primeira opção é a que se escolhe por omissão,
/// a última a que exige pensar. Deriva do conjunto fechado do contrato: se um
/// papel novo aparecer no `TechnicalRole`, adiciona-se aqui de propósito, e não
/// por acaso.
pub const OFERECIDOS: [TechnicalRole; 8] = [
    TechnicalRole::ResearchMember,
    TechnicalRole::ResearchLead,
    TechnicalRole::Collaborator,
    TechnicalRole::ExternalCollaborator,
    TechnicalRole::UnitManager,
    TechnicalRole::Auditor,
    TechnicalRole::OrganisationAdmin,
    TechnicalRole::PlatformAdmin,
];

/// O rótulo canónico de um papel.
#[must_use]
pub fn label(role: TechnicalRole) -> &'static str {
    rotulo(role).label
}

/// O rótulo canónico a partir do código estável do papel. Um código que este
/// build não conhece devolve-se tal como veio — honesto, e nunca uma invenção.
#[must_use]
pub fn label_do_codigo(code: &str) -> String {
    TechnicalRole::parse(code).map_or_else(|| code.to_owned(), |role| label(role).to_owned())
}

/// O rótulo com a descrição, para um seletor: «Rótulo — descrição» quando há
/// descrição, só o rótulo quando não há.
#[must_use]
pub fn label_com_descricao(role: TechnicalRole) -> String {
    let Rotulo { label, description } = rotulo(role);
    if description.is_empty() {
        label.to_owned()
    } else {
        format!("{label} — {description}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn research_lead_le_se_lider_de_investigacao() {
        assert_eq!(label(TechnicalRole::ResearchLead), "Líder de investigação");
        assert_eq!(label_do_codigo("research_lead"), "Líder de investigação");
    }

    #[test]
    fn o_codigo_desconhecido_devolve_se_tal_como_veio() {
        assert_eq!(
            label_do_codigo("papel_que_nao_existe"),
            "papel_que_nao_existe"
        );
    }

    #[test]
    fn nenhum_rotulo_e_o_codigo_ingles() {
        // Nenhum papel oferecido se mostra pelo identificador de contrato: cada um
        // tem tradução, e «Research Lead» não sobrevive em lado nenhum.
        for role in OFERECIDOS {
            let label = label(role);
            assert_ne!(label, role.as_str(), "{} sem tradução", role.as_str());
            assert!(
                !label.contains("Research Lead"),
                "«Research Lead» sobreviveu em {}",
                role.as_str()
            );
        }
    }

    #[test]
    fn o_seletor_junta_rotulo_e_descricao() {
        assert_eq!(
            label_com_descricao(TechnicalRole::ResearchMember),
            "Investigador — acesso científico comum"
        );
        assert_eq!(
            label_com_descricao(TechnicalRole::PlatformAdmin),
            "Administrador da plataforma"
        );
    }
}
