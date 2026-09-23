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

/// Um papel técnico como uma pessoa o lê: a **chave** do rótulo canónico e, quando
/// ajuda, a chave de uma descrição curta do que o papel é. A descrição fica vazia
/// (`""`) quando o rótulo se explica sozinho — um «Administrador da plataforma» não
/// precisa de glosa. As chaves resolvem-se pela via i18n no idioma corrente, pelo
/// que o rótulo é português, inglês ou francês conforme quem lê (§84).
struct Rotulo {
    label_key: &'static str,
    description_key: &'static str,
}

/// A chave da tradução canónica de um papel. É a **única** no Workspace. O `match`
/// mapeia o conjunto fechado do contrato para chaves do catálogo; a tradução
/// própria acontece em [`label`] e [`label_com_descricao`], já com o idioma.
const fn rotulo(role: TechnicalRole) -> Rotulo {
    match role {
        TechnicalRole::ResearchMember => Rotulo {
            label_key: "roles.researcher",
            description_key: "roles.scope.common",
        },
        TechnicalRole::ResearchLead => Rotulo {
            label_key: "roles.research_lead",
            description_key: "roles.lead_desc",
        },
        TechnicalRole::Collaborator => Rotulo {
            label_key: "roles.collaborator",
            description_key: "roles.scope.narrow",
        },
        TechnicalRole::ExternalCollaborator => Rotulo {
            label_key: "roles.external",
            description_key: "roles.scope.assigned_only",
        },
        TechnicalRole::UnitManager => Rotulo {
            label_key: "roles.unit_manager",
            description_key: "",
        },
        TechnicalRole::Auditor => Rotulo {
            label_key: "roles.auditor",
            description_key: "roles.scope.evidence",
        },
        TechnicalRole::OrganisationAdmin => Rotulo {
            label_key: "roles.org_admin",
            description_key: "",
        },
        TechnicalRole::PlatformAdmin => Rotulo {
            label_key: "roles.platform_admin",
            description_key: "",
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

/// O rótulo canónico de um papel, no idioma corrente.
#[must_use]
pub fn label(role: TechnicalRole) -> &'static str {
    crate::i18n::t(rotulo(role).label_key)
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
    let Rotulo {
        label_key,
        description_key,
    } = rotulo(role);
    let label = crate::i18n::t(label_key);
    if description_key.is_empty() {
        label.to_owned()
    } else {
        format!("{label} — {}", crate::i18n::t(description_key))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn research_lead_le_se_lider_de_investigacao() {
        assert_eq!(
            label(TechnicalRole::ResearchLead),
            crate::i18n::t("roles.research_lead")
        );
        assert_eq!(
            label_do_codigo("research_lead"),
            crate::i18n::t("roles.research_lead")
        );
    }

    #[tokio::test]
    async fn o_rotulo_segue_o_idioma_corrente() {
        use crate::i18n::{with_locale, Locale};
        // O mesmo papel, três línguas: o rótulo é apresentação, não contrato.
        let fr = with_locale(Locale::Fr, async {
            label(TechnicalRole::ResearchLead).to_owned()
        })
        .await;
        let en = with_locale(Locale::En, async {
            label(TechnicalRole::ResearchLead).to_owned()
        })
        .await;
        assert_eq!(fr, crate::i18n::t_in(Locale::Fr, "roles.research_lead"));
        assert_eq!(en, crate::i18n::t_in(Locale::En, "roles.research_lead"));
        assert_ne!(fr, en, "o rótulo não mudou com o idioma");
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
        // Com descrição: «Rótulo — descrição», ambos do catálogo, no idioma corrente.
        assert_eq!(
            label_com_descricao(TechnicalRole::ResearchMember),
            format!(
                "{} — {}",
                crate::i18n::t("roles.researcher"),
                crate::i18n::t("roles.scope.common")
            )
        );
        // Sem descrição: só o rótulo, sem o travessão pendurado.
        assert_eq!(
            label_com_descricao(TechnicalRole::PlatformAdmin),
            crate::i18n::t("roles.platform_admin")
        );
    }
}
