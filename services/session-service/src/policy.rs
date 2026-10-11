//! The session policy the Core signs, and its verification.
//!
//! The Instance's public key is learned once, when the machine is claimed. A
//! policy is accepted only when the signature over its canonical JSON verifies
//! with that key **and** it names this machine: a valid policy for another
//! machine is not a policy for this one.

use ed25519_dalek::{Signature, VerifyingKey};
use ocinye_installer_contracts::canonical::to_canonical;
use ocinye_system_contracts::session::{SessionPolicy, SignedSessionPolicy};

/// The bytes a policy's signature covers.
#[must_use]
pub fn signed_bytes(policy: &SessionPolicy) -> Option<Vec<u8>> {
    to_canonical(policy).ok().map(String::into_bytes)
}

/// Verify a signed policy for this machine.
#[must_use]
pub fn verify<'a>(
    signed: &'a SignedSessionPolicy,
    instance_key: &[u8; 32],
    machine_id: &str,
) -> Option<&'a SessionPolicy> {
    let key = VerifyingKey::from_bytes(instance_key).ok()?;
    let raw: [u8; 64] = hex::decode(&signed.signature).ok()?.try_into().ok()?;
    let signature = Signature::from_bytes(&raw);
    let message = signed_bytes(&signed.policy)?;
    key.verify_strict(&message, &signature).ok()?;
    (signed.policy.machine_id == machine_id && signed.policy.max_unlock_attempts > 0)
        .then_some(&signed.policy)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};

    pub(crate) const SEED: [u8; 32] = [7; 32];

    pub(crate) fn key() -> [u8; 32] {
        SigningKey::from_bytes(&SEED).verifying_key().to_bytes()
    }

    pub(crate) fn policy(epoch: u64) -> SessionPolicy {
        SessionPolicy {
            member_id: "m-1".into(),
            member_name: "Pessoa de Prova".into(),
            session_id: "s-1".into(),
            machine_id: "machine-a".into(),
            revocation_epoch: epoch,
            offline_unlock: true,
            max_offline_secs: Some(3600),
            max_unlock_attempts: 5,
            idle_lock_secs: 300,
            issued_at: 1_800_000_000,
        }
    }

    pub(crate) fn sign(policy: SessionPolicy) -> SignedSessionPolicy {
        let sig = SigningKey::from_bytes(&SEED).sign(&signed_bytes(&policy).unwrap());
        SignedSessionPolicy {
            policy,
            signature: hex::encode(sig.to_bytes()),
        }
    }

    #[test]
    fn uma_politica_assinada_para_esta_maquina_e_aceite() {
        assert!(verify(&sign(policy(1)), &key(), "machine-a").is_some());
    }

    #[test]
    fn outra_maquina_outra_chave_ou_um_campo_alterado_sao_recusados() {
        let good = sign(policy(1));
        assert!(verify(&good, &key(), "machine-b").is_none());
        assert!(verify(&good, &[9; 32], "machine-a").is_none());
        let mut tampered = good.clone();
        tampered.policy.offline_unlock = false;
        assert!(verify(&tampered, &key(), "machine-a").is_none());
        let mut widened = good.clone();
        widened.policy.max_offline_secs = None;
        assert!(verify(&widened, &key(), "machine-a").is_none());
        let mut garbage = good;
        garbage.signature = "zz".into();
        assert!(verify(&garbage, &key(), "machine-a").is_none());
    }
}
