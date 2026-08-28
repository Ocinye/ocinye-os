//! What may leave the institution by email.
//!
//! # Sending is an export
//!
//! A message with an attachment sent to an outside address moves institutional
//! material past the boundary the classification exists to hold. Once it has
//! gone there is no revoking it: no ACL reaches inside somebody else's mailbox
//! (briefing §35).
//!
//! # The rule
//!
//! | Classification | Internal recipients | Any external recipient |
//! |---|---|---|
//! | `PUBLIC` | allowed | allowed |
//! | `INTERNAL` | allowed | allowed, **with confirmation** |
//! | `CONFIDENTIAL` | allowed | allowed, **with confirmation**, audited |
//! | `RESTRICTED` | allowed | **refused** |
//!
//! `RESTRICTED` is the only outright refusal, and it is deliberate: the whole
//! point of that classification is that reaching it requires explicit,
//! attributable authorisation, and an email recipient list is neither.
//!
//! # Confirmation is not a rubber stamp
//!
//! It exists so that sending outside is a decision rather than a reflex. Asking
//! on every message would train people to click through, so `PUBLIC` never
//! asks and internal-only sending never asks (briefing §73).

use ocinye_contracts::{Classification, MailAddress};

/// What the policy decided about one send.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SendDecision {
    /// Send it.
    Allowed,
    /// Send it once the author confirms they meant to.
    NeedsConfirmation {
        /// What the author is being asked to confirm, in their own language.
        reason: String,
        /// The highest classification travelling with the message.
        classification: Classification,
        /// How many recipients are outside the institution.
        external_count: usize,
    },
    /// Do not send it.
    Refused {
        /// Why, in language a member can act on.
        reason: String,
    },
}

impl SendDecision {
    /// Whether the message may go out as things stand.
    #[must_use]
    pub const fn is_allowed(&self) -> bool {
        matches!(self, Self::Allowed)
    }

    /// Whether the policy refused outright.
    #[must_use]
    pub const fn is_refused(&self) -> bool {
        matches!(self, Self::Refused { .. })
    }

    /// A stable label for the audit trail.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Allowed => "allowed",
            Self::NeedsConfirmation { .. } => "needs_confirmation",
            Self::Refused { .. } => "refused",
        }
    }
}

/// The send policy.
pub struct SendPolicy;

impl SendPolicy {
    /// Decide whether a message may be sent.
    ///
    /// `attachment_classifications` carries the classification of every Ocinye
    /// artefact attached. A file uploaded from the composer has none: it is not
    /// institutional material under classification, and the author is
    /// responsible for it as they would be for any file they attach anywhere.
    ///
    /// `confirmed` is what the author answered to a previous
    /// [`SendDecision::NeedsConfirmation`]. It can never turn a refusal into a
    /// send: confirmation is consent to an allowed act, not authority to
    /// perform a forbidden one.
    #[must_use]
    pub fn evaluate(
        recipients: &[MailAddress],
        attachment_classifications: &[Classification],
        confirmed: bool,
    ) -> SendDecision {
        let external: Vec<&MailAddress> = recipients.iter().filter(|r| r.is_external()).collect();

        // The highest classification in the message governs. A single
        // `RESTRICTED` attachment among ten `PUBLIC` ones makes the message
        // `RESTRICTED`.
        let highest = attachment_classifications
            .iter()
            .copied()
            .max_by_key(|c| match c {
                Classification::Public => 0_u8,
                Classification::Internal => 1,
                Classification::Confidential => 2,
                Classification::Restricted => 3,
            });

        let Some(highest) = highest else {
            // Nothing classified is travelling. A message with no institutional
            // attachments is ordinary correspondence.
            return SendDecision::Allowed;
        };

        if external.is_empty() {
            // Everything stays inside. The classification already governs who
            // could read the artefact in the first place.
            return SendDecision::Allowed;
        }

        let external_count = external.len();
        let domains = external_domains(&external);

        match highest {
            Classification::Public => SendDecision::Allowed,

            Classification::Restricted => SendDecision::Refused {
                reason: format!(
                    "Este email contém material RESTRICTED e tem {external_count} \
                     destinatário(s) externo(s) ({domains}). Material RESTRICTED não sai \
                     da instituição por correio electrónico."
                ),
            },

            Classification::Internal | Classification::Confidential if confirmed => {
                SendDecision::Allowed
            }

            Classification::Confidential => SendDecision::NeedsConfirmation {
                reason: format!(
                    "Este email contém material CONFIDENTIAL e tem {external_count} \
                     destinatário(s) externo(s) ({domains}). Confirme que pretende \
                     enviá-lo para fora da instituição."
                ),
                classification: highest,
                external_count,
            },

            Classification::Internal => SendDecision::NeedsConfirmation {
                reason: format!(
                    "Este email contém material INTERNAL e tem {external_count} \
                     destinatário(s) externo(s) ({domains})."
                ),
                classification: highest,
                external_count,
            },
        }
    }

    /// Whether institutional material at this classification may be used as AI
    /// context.
    ///
    /// # Why this is not the same as being able to read it
    ///
    /// `human_read = true` does not imply `ai_processing_allowed = true`
    /// (briefing §37). A person reading a `RESTRICTED` document is bound by
    /// their obligations to the institution; a model is a system whose
    /// retention and routing the institution does not fully control.
    ///
    /// With no external providers enabled the difference is smaller than it
    /// will be. Encoding it now costs nothing and prevents the equivalence
    /// being assumed later, when it matters.
    #[must_use]
    pub const fn may_use_as_ai_context(classification: Classification) -> bool {
        matches!(
            classification,
            Classification::Public | Classification::Internal
        )
    }
}

/// The external domains a recipient list touches, de-duplicated and readable.
fn external_domains(external: &[&MailAddress]) -> String {
    let mut domains: Vec<&str> = external.iter().filter_map(|r| r.domain()).collect();
    domains.sort_unstable();
    domains.dedup();

    if domains.len() > 3 {
        format!("{} e mais {}", domains[..3].join(", "), domains.len() - 3)
    } else {
        domains.join(", ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn internal(address: &str) -> MailAddress {
        MailAddress::new(address, None, &["ocinye.com".to_owned()])
    }

    fn external(address: &str) -> MailAddress {
        let built = MailAddress::new(address, None, &["ocinye.com".to_owned()]);
        assert!(built.is_external(), "{address} deveria ser externo");
        built
    }

    #[test]
    fn a_message_with_no_institutional_attachments_just_sends() {
        let decision = SendPolicy::evaluate(&[external("parceiro@exemplo.com")], &[], false);
        assert!(decision.is_allowed());
    }

    #[test]
    fn classified_material_moves_freely_inside_the_institution() {
        for classification in [
            Classification::Internal,
            Classification::Confidential,
            Classification::Restricted,
        ] {
            let decision =
                SendPolicy::evaluate(&[internal("colega@ocinye.com")], &[classification], false);
            assert!(
                decision.is_allowed(),
                "{classification:?} foi barrado internamente"
            );
        }
    }

    #[test]
    fn restricted_never_leaves_the_institution() {
        let decision = SendPolicy::evaluate(
            &[internal("colega@ocinye.com"), external("fora@exemplo.com")],
            &[Classification::Restricted],
            false,
        );

        assert!(decision.is_refused());
        match decision {
            SendDecision::Refused { reason } => {
                assert!(reason.contains("RESTRICTED"));
                assert!(reason.contains("exemplo.com"));
            }
            other => panic!("esperava recusa, obtive {other:?}"),
        }
    }

    #[test]
    fn confirmation_cannot_turn_a_refusal_into_a_send() {
        // A garantia que mais importa: confirmar é consentir num acto
        // permitido, nunca autoridade para realizar um proibido.
        let decision = SendPolicy::evaluate(
            &[external("fora@exemplo.com")],
            &[Classification::Restricted],
            true,
        );
        assert!(decision.is_refused(), "confirmar contornou a recusa");
    }

    #[test]
    fn confidential_to_the_outside_asks_first_and_then_allows() {
        let recipients = [external("parceiro@exemplo.com")];
        let attachments = [Classification::Confidential];

        let first = SendPolicy::evaluate(&recipients, &attachments, false);
        match &first {
            SendDecision::NeedsConfirmation {
                classification,
                external_count,
                reason,
            } => {
                assert_eq!(*classification, Classification::Confidential);
                assert_eq!(*external_count, 1);
                assert!(reason.contains("CONFIDENTIAL"));
            }
            other => panic!("esperava confirmação, obtive {other:?}"),
        }

        assert!(SendPolicy::evaluate(&recipients, &attachments, true).is_allowed());
    }

    #[test]
    fn the_highest_classification_governs_the_whole_message() {
        // Um único anexo RESTRICTED entre dez PUBLIC torna a mensagem
        // RESTRICTED.
        let decision = SendPolicy::evaluate(
            &[external("fora@exemplo.com")],
            &[
                Classification::Public,
                Classification::Public,
                Classification::Restricted,
                Classification::Internal,
            ],
            true,
        );
        assert!(decision.is_refused());
    }

    #[test]
    fn public_material_never_asks() {
        // Perguntar em cada mensagem treina as pessoas a clicar sem ler.
        let decision = SendPolicy::evaluate(
            &[external("qualquer@exemplo.com")],
            &[Classification::Public],
            false,
        );
        assert!(decision.is_allowed());
    }

    #[test]
    fn a_lookalike_domain_counts_as_external() {
        // `ocinye.com.atacante.net` termina no domínio institucional e não é
        // o domínio institucional.
        let decision = SendPolicy::evaluate(
            &[external("alvo@ocinye.com.atacante.net")],
            &[Classification::Restricted],
            false,
        );
        assert!(decision.is_refused());
    }

    #[test]
    fn many_external_domains_are_summarised_rather_than_listed_forever() {
        let recipients: Vec<MailAddress> = (0..8)
            .map(|i| external(&format!("x@dominio{i}.example")))
            .collect();

        match SendPolicy::evaluate(&recipients, &[Classification::Confidential], false) {
            SendDecision::NeedsConfirmation { reason, .. } => {
                assert!(reason.contains("e mais 5"), "{reason}");
            }
            other => panic!("esperava confirmação, obtive {other:?}"),
        }
    }

    #[test]
    fn reading_something_is_not_permission_to_feed_it_to_a_model() {
        assert!(SendPolicy::may_use_as_ai_context(Classification::Public));
        assert!(SendPolicy::may_use_as_ai_context(Classification::Internal));
        assert!(!SendPolicy::may_use_as_ai_context(
            Classification::Confidential
        ));
        assert!(!SendPolicy::may_use_as_ai_context(
            Classification::Restricted
        ));
    }

    #[test]
    fn decisions_carry_a_stable_label_for_the_audit_trail() {
        assert_eq!(SendDecision::Allowed.as_str(), "allowed");
        assert_eq!(
            SendDecision::Refused {
                reason: String::new()
            }
            .as_str(),
            "refused"
        );
    }
}
