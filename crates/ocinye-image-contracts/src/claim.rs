//! Claim state, claim protocol v1 and the claim transition rules
//! (D013_CLAIM_MODEL.md, D013_STATE_MACHINE.md §3).
//!
//! The rules are pure functions over the persisted state, the volatile
//! pairing-code record and the clock. `ocinye-firstboot` applies them under
//! one `flock`, writing `state.json` atomically (temp → fsync → rename →
//! fsync dir): there is no persisted half-state, and two claimants can never
//! both win.

use serde::{Deserialize, Serialize};

use crate::firstboot::{BootstrapId, ImageFacts, KeyFingerprint, MachineFingerprint};

/// 128-bit random, 32 lowercase hex, single use.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ClaimId(pub String);

impl ClaimId {
    /// From 16 random bytes.
    #[must_use]
    pub fn from_bytes(b: [u8; 16]) -> Self {
        Self(hex::encode(b))
    }

    /// 32 lowercase hex.
    #[must_use]
    pub fn is_valid(&self) -> bool {
        crate::is_lower_hex(&self.0, 32)
    }

    /// What logs and screens show: the first 8 characters.
    #[must_use]
    pub fn truncated(&self) -> String {
        format!("{}…", self.0.get(..8).unwrap_or(""))
    }
}

/// Crockford base32 alphabet (no I, L, O, U).
pub const CROCKFORD: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";

/// Pairing code: 25 Crockford base32 symbols in 5 groups of 5 (125 bits).
/// Never Debug-printed or Displayed in clear; it lives only in tmpfs.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PairingCode(String);

impl std::fmt::Debug for PairingCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("PairingCode(<redacted>)")
    }
}

impl PairingCode {
    /// From at least 16 random bytes: 125 bits taken, 5 bits per symbol.
    #[must_use]
    pub fn from_random(bytes: [u8; 16]) -> Self {
        let n = u128::from_be_bytes(bytes);
        let mut out = String::with_capacity(29);
        for i in 0..25 {
            if i > 0 && i % 5 == 0 {
                out.push('-');
            }
            let sym = ((n >> (5 * (24 - i))) & 0x1f) as usize;
            out.push(CROCKFORD[sym] as char);
        }
        Self(out)
    }

    /// Normalise what an operator typed: uppercase, `O`→`0`, `I`/`L`→`1`,
    /// spaces and separators removed, regrouped. `None` if it cannot be a code.
    #[must_use]
    pub fn normalise(typed: &str) -> Option<Self> {
        let mut syms = String::with_capacity(25);
        for c in typed.chars() {
            let c = c.to_ascii_uppercase();
            let c = match c {
                'O' => '0',
                'I' | 'L' => '1',
                ' ' | '-' => continue,
                c => c,
            };
            if !c.is_ascii() || !CROCKFORD.contains(&(c as u8)) {
                return None;
            }
            syms.push(c);
        }
        if syms.len() != 25 {
            return None;
        }
        let groups: Vec<&str> = (0..5).map(|g| &syms[g * 5..g * 5 + 5]).collect();
        Some(Self(groups.join("-")))
    }

    /// For the one interactive display and the tmpfs file only.
    #[must_use]
    pub fn expose(&self) -> &str {
        &self.0
    }

    /// Constant-time comparison.
    #[must_use]
    pub fn matches(&self, other: &Self) -> bool {
        let (a, b) = (self.0.as_bytes(), other.0.as_bytes());
        a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
    }
}

/// How a claim was proven.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ClaimMethod {
    /// Code shown on the machine's console.
    PairingCode,
    /// A key placed by the platform (cloud-init) or by the OIE USB step.
    ProvisionedKey,
}

/// Persisted state (`state.json`, canonical JSON). No half-states.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "state",
    rename_all = "SCREAMING_SNAKE_CASE",
    deny_unknown_fields
)]
pub enum ClaimState {
    /// Nobody owns the machine.
    Unclaimed {
        /// ≥ 1; increases on every rollback.
        generation: u32,
        /// RFC 3339; brute-force lockout.
        locked_until: Option<String>,
    },
    /// Enrolled, awaiting confirmation by the enrolled key.
    ClaimInProgress {
        /// Single use.
        claim_id: ClaimId,
        /// The enrolled key.
        owner_key: KeyFingerprint,
        /// RFC 3339; ten minutes after enrolment.
        deadline: String,
    },
    /// Owned.
    Claimed {
        /// The claim.
        claim_id: ClaimId,
        /// The owner key.
        owner_key: KeyFingerprint,
        /// How.
        method: ClaimMethod,
        /// RFC 3339.
        claimed_at: String,
    },
}

/// What a client may see of the state (no claim id of another party).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "state",
    rename_all = "SCREAMING_SNAKE_CASE",
    deny_unknown_fields
)]
pub enum ClaimStateView {
    /// Claimable.
    Unclaimed {
        /// Attempts left on the current code.
        attempts_left: u8,
        /// RFC 3339.
        code_expires_at: String,
        /// RFC 3339 when locked.
        locked_until: Option<String>,
    },
    /// Someone enrolled.
    ClaimInProgress {
        /// RFC 3339.
        deadline: String,
    },
    /// Owned.
    Claimed,
}

/// The challenge the server presents (`Welcome`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClaimChallenge {
    /// [`crate::CLAIM_PROTOCOL`].
    pub protocol: u16,
    /// Image facts.
    pub image: ImageFacts,
    /// State.
    pub state: ClaimStateView,
    /// Host keys, Ed25519 first.
    pub host_keys: Vec<MachineFingerprint>,
    /// Bootstrap id.
    pub bootstrap_id: BootstrapId,
}

/// Client → server (closed).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "cmd", rename_all = "snake_case", deny_unknown_fields)]
pub enum ClaimCommand {
    /// First message.
    Hello {
        /// Protocol version the client speaks.
        protocol: u16,
    },
    /// The key to enrol is the key the SSH session authenticated with (passed
    /// by sshd to the forced command), never a key named in the message.
    Enroll {
        /// The code from the console.
        code: PairingCode,
    },
    /// Give an enrolment back.
    Abort {
        /// The claim.
        claim_id: ClaimId,
    },
}

/// Server → client (closed).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "PascalCase")]
pub enum ClaimEvent {
    /// Answer to Hello.
    Welcome {
        /// The challenge.
        challenge: ClaimChallenge,
    },
    /// Enrolment succeeded.
    Enrolled {
        /// Hand to `confirm`.
        claim_id: ClaimId,
        /// The enrolled key.
        owner_key: KeyFingerprint,
        /// RFC 3339.
        confirm_deadline: String,
    },
    /// Confirmation succeeded.
    Confirmed {
        /// The claim.
        claim_id: ClaimId,
        /// RFC 3339.
        claimed_at: String,
        /// The owner key.
        owner_key: KeyFingerprint,
    },
    /// Abort accepted.
    Aborted,
    /// A typed refusal.
    Refused {
        /// Why.
        reason: ClaimRefusal,
    },
}

/// Typed refusals — no generic error.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "code", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ClaimRefusal {
    /// Wrong code.
    InvalidCode {
        /// Attempts left on this code.
        remaining: u8,
    },
    /// The code is past its fifteen minutes, used, or exhausted.
    CodeExpired,
    /// Brute-force lockout.
    Locked {
        /// RFC 3339.
        until: String,
    },
    /// The machine is owned.
    AlreadyClaimed,
    /// Someone else enrolled.
    ClaimInProgress {
        /// RFC 3339.
        deadline: String,
    },
    /// Replay or wrong claim.
    ClaimIdMismatch,
    /// Provisioned claim with a key the platform did not provision.
    NotProvisionedKey,
    /// The release payload or the embedded manifest failed integrity.
    IntegrityFailed,
    /// First boot did not produce an identity.
    IdentityMissing,
    /// Unknown protocol.
    ProtocolUnsupported {
        /// The one this server speaks.
        supported: u16,
    },
    /// The confirmation came from an account other than `ocinye`.
    WrongAccount,
}

/// Lifetime of a pairing code.
pub const CODE_LIFETIME_S: i64 = 15 * 60;
/// Window to confirm an enrolment.
pub const CONFIRM_WINDOW_S: i64 = 10 * 60;
/// Attempts per code.
pub const ATTEMPTS_PER_CODE: u8 = 5;
/// Exhausted codes in a row before lockout.
pub const EXHAUSTED_BEFORE_LOCK: u8 = 3;
/// First lockout; doubles up to [`LOCK_MAX_S`].
pub const LOCK_BASE_S: i64 = 15 * 60;
/// Longest lockout.
pub const LOCK_MAX_S: i64 = 4 * 60 * 60;

/// The volatile record of the current pairing code (tmpfs, root 0600).
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CodeRecord {
    /// The code.
    pub code: PairingCode,
    /// Increases with every new code (the journal records only this).
    pub code_generation: u32,
    /// Unix seconds.
    pub expires_at: i64,
    /// Attempts left.
    pub attempts_left: u8,
    /// Codes exhausted in a row (any source).
    pub exhausted_in_a_row: u8,
    /// Consecutive lockouts (for doubling).
    pub lock_level: u8,
}

impl std::fmt::Debug for CodeRecord {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CodeRecord")
            .field("code_generation", &self.code_generation)
            .field("expires_at", &self.expires_at)
            .field("attempts_left", &self.attempts_left)
            .finish_non_exhaustive()
    }
}

/// Unix seconds → RFC 3339.
#[must_use]
pub fn rfc3339(unix: i64) -> String {
    chrono::DateTime::from_timestamp(unix, 0)
        .map(|t| t.to_rfc3339_opts(chrono::SecondsFormat::Secs, true))
        .unwrap_or_default()
}

/// RFC 3339 → unix seconds.
#[must_use]
pub fn unix(rfc: &str) -> Option<i64> {
    chrono::DateTime::parse_from_rfc3339(rfc)
        .ok()
        .map(|t| t.timestamp())
}

/// The effect of one rule application. `state`/`code` are `Some` when they
/// must be written (both under the same lock).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Outcome {
    /// What the client receives.
    pub event: ClaimEvent,
    /// New persisted state, if changed.
    pub state: Option<ClaimState>,
    /// New code record, if changed (`Some(None)` removes the code).
    pub code: Option<Option<CodeRecord>>,
    /// Journal event name (never carries a secret).
    pub journal: &'static str,
}

fn refused(reason: ClaimRefusal, journal: &'static str) -> Outcome {
    Outcome {
        event: ClaimEvent::Refused { reason },
        state: None,
        code: None,
        journal,
    }
}

/// A fresh code record.
#[must_use]
pub fn new_code(
    random: [u8; 16],
    generation: u32,
    now: i64,
    carried: Option<&CodeRecord>,
) -> CodeRecord {
    CodeRecord {
        code: PairingCode::from_random(random),
        code_generation: generation,
        expires_at: now + CODE_LIFETIME_S,
        attempts_left: ATTEMPTS_PER_CODE,
        exhausted_in_a_row: carried.map_or(0, |c| c.exhausted_in_a_row),
        lock_level: carried.map_or(0, |c| c.lock_level),
    }
}

/// `Enroll{code}` from a session that authenticated with `offered`.
///
/// `fresh_code`/`claim_id_random` are the randomness the caller already drew,
/// so the rule stays pure. A wrong code burns an attempt; an exhausted code is
/// replaced; three exhausted codes in a row lock the machine (doubling).
#[must_use]
pub fn enroll(
    state: &ClaimState,
    code: Option<&CodeRecord>,
    presented: &PairingCode,
    offered: &KeyFingerprint,
    now: i64,
    claim_id_random: [u8; 16],
    fresh_code: [u8; 16],
) -> Outcome {
    let generation = match state {
        ClaimState::Claimed { .. } => {
            return refused(ClaimRefusal::AlreadyClaimed, "enroll_refused_claimed")
        }
        ClaimState::ClaimInProgress { deadline, .. } => {
            return refused(
                ClaimRefusal::ClaimInProgress {
                    deadline: deadline.clone(),
                },
                "enroll_refused_in_progress",
            )
        }
        ClaimState::Unclaimed {
            generation,
            locked_until,
        } => {
            if let Some(until) = locked_until.as_deref().and_then(unix) {
                if now < until {
                    return refused(
                        ClaimRefusal::Locked {
                            until: rfc3339(until),
                        },
                        "enroll_refused_locked",
                    );
                }
            }
            *generation
        }
    };
    let Some(rec) = code else {
        return refused(ClaimRefusal::CodeExpired, "enroll_refused_no_code");
    };
    if now >= rec.expires_at || rec.attempts_left == 0 {
        let next = new_code(fresh_code, rec.code_generation + 1, now, Some(rec));
        return Outcome {
            event: ClaimEvent::Refused {
                reason: ClaimRefusal::CodeExpired,
            },
            state: None,
            code: Some(Some(next)),
            journal: "code_expired",
        };
    }
    if !rec.code.matches(presented) {
        let mut next = rec.clone();
        next.attempts_left -= 1;
        if next.attempts_left > 0 {
            return Outcome {
                event: ClaimEvent::Refused {
                    reason: ClaimRefusal::InvalidCode {
                        remaining: next.attempts_left,
                    },
                },
                state: None,
                code: Some(Some(next)),
                journal: "enroll_invalid_code",
            };
        }
        // Exhausted: a new code; three in a row lock the machine.
        let mut fresh = new_code(fresh_code, rec.code_generation + 1, now, Some(rec));
        fresh.exhausted_in_a_row = rec.exhausted_in_a_row + 1;
        if fresh.exhausted_in_a_row >= EXHAUSTED_BEFORE_LOCK {
            let secs = (LOCK_BASE_S << rec.lock_level.min(8)).min(LOCK_MAX_S);
            fresh.exhausted_in_a_row = 0;
            fresh.lock_level = rec.lock_level.saturating_add(1);
            let until = now + secs;
            return Outcome {
                event: ClaimEvent::Refused {
                    reason: ClaimRefusal::Locked {
                        until: rfc3339(until),
                    },
                },
                state: Some(ClaimState::Unclaimed {
                    generation,
                    locked_until: Some(rfc3339(until)),
                }),
                code: Some(Some(fresh)),
                journal: "claim_locked",
            };
        }
        return Outcome {
            event: ClaimEvent::Refused {
                reason: ClaimRefusal::InvalidCode { remaining: 0 },
            },
            state: None,
            code: Some(Some(fresh)),
            journal: "code_exhausted",
        };
    }
    let claim_id = ClaimId::from_bytes(claim_id_random);
    let deadline = rfc3339(now + CONFIRM_WINDOW_S);
    Outcome {
        event: ClaimEvent::Enrolled {
            claim_id: claim_id.clone(),
            owner_key: offered.clone(),
            confirm_deadline: deadline.clone(),
        },
        state: Some(ClaimState::ClaimInProgress {
            claim_id,
            owner_key: offered.clone(),
            deadline,
        }),
        // The code is consumed: one use only.
        code: Some(None),
        journal: "enrolled",
    }
}

/// `confirm --claim <id>` run by `sudo_user` (must be `ocinye`).
#[must_use]
pub fn confirm(state: &ClaimState, claim_id: &ClaimId, sudo_user: &str, now: i64) -> Outcome {
    if sudo_user != "ocinye" {
        return refused(ClaimRefusal::WrongAccount, "confirm_refused_account");
    }
    match state {
        ClaimState::Claimed { .. } => {
            refused(ClaimRefusal::AlreadyClaimed, "confirm_refused_claimed")
        }
        ClaimState::Unclaimed { .. } => {
            refused(ClaimRefusal::ClaimIdMismatch, "confirm_refused_unclaimed")
        }
        ClaimState::ClaimInProgress {
            claim_id: id,
            owner_key,
            deadline,
        } => {
            if id != claim_id {
                return refused(ClaimRefusal::ClaimIdMismatch, "confirm_refused_mismatch");
            }
            if unix(deadline).is_none_or(|d| now >= d) {
                return refused(ClaimRefusal::CodeExpired, "confirm_refused_deadline");
            }
            let claimed_at = rfc3339(now);
            Outcome {
                event: ClaimEvent::Confirmed {
                    claim_id: id.clone(),
                    claimed_at: claimed_at.clone(),
                    owner_key: owner_key.clone(),
                },
                state: Some(ClaimState::Claimed {
                    claim_id: id.clone(),
                    owner_key: owner_key.clone(),
                    method: ClaimMethod::PairingCode,
                    claimed_at,
                }),
                code: Some(None),
                journal: "claimed_pairing",
            }
        }
    }
}

/// `claim --provisioned --key <fp>` run by `ocinye`, the key being one the
/// platform provisioned (cloud-init or the OIE USB step).
#[must_use]
pub fn claim_provisioned(
    state: &ClaimState,
    key: &KeyFingerprint,
    provisioned: &[KeyFingerprint],
    sudo_user: &str,
    now: i64,
    claim_id_random: [u8; 16],
) -> Outcome {
    if sudo_user != "ocinye" {
        return refused(ClaimRefusal::WrongAccount, "provisioned_refused_account");
    }
    match state {
        ClaimState::Claimed { .. } => {
            refused(ClaimRefusal::AlreadyClaimed, "provisioned_refused_claimed")
        }
        ClaimState::ClaimInProgress { deadline, .. } => refused(
            ClaimRefusal::ClaimInProgress {
                deadline: deadline.clone(),
            },
            "provisioned_refused_in_progress",
        ),
        ClaimState::Unclaimed { .. } => {
            if !provisioned.contains(key) {
                return refused(ClaimRefusal::NotProvisionedKey, "provisioned_refused_key");
            }
            let claim_id = ClaimId::from_bytes(claim_id_random);
            let claimed_at = rfc3339(now);
            Outcome {
                event: ClaimEvent::Confirmed {
                    claim_id: claim_id.clone(),
                    claimed_at: claimed_at.clone(),
                    owner_key: key.clone(),
                },
                state: Some(ClaimState::Claimed {
                    claim_id,
                    owner_key: key.clone(),
                    method: ClaimMethod::ProvisionedKey,
                    claimed_at,
                }),
                code: Some(None),
                journal: "claimed_provisioned",
            }
        }
    }
}

/// The deadline passed (or `Abort{claim_id}`): roll back to `Unclaimed`
/// with the next generation and a new code. `claim_id` is `None` for the timer.
#[must_use]
pub fn roll_back(
    state: &ClaimState,
    claim_id: Option<&ClaimId>,
    now: i64,
    fresh_code: [u8; 16],
    last_code_generation: u32,
) -> Outcome {
    match state {
        ClaimState::ClaimInProgress {
            claim_id: id,
            deadline,
            ..
        } => {
            let by_timer = claim_id.is_none();
            if let Some(c) = claim_id {
                if c != id {
                    return refused(ClaimRefusal::ClaimIdMismatch, "abort_refused_mismatch");
                }
            } else if unix(deadline).is_some_and(|d| now < d) {
                return refused(
                    ClaimRefusal::ClaimInProgress {
                        deadline: deadline.clone(),
                    },
                    "timeout_not_due",
                );
            }
            Outcome {
                event: ClaimEvent::Aborted,
                state: Some(ClaimState::Unclaimed {
                    generation: generation_after(state),
                    locked_until: None,
                }),
                code: Some(Some(new_code(
                    fresh_code,
                    last_code_generation + 1,
                    now,
                    None,
                ))),
                journal: if by_timer {
                    "claim_failed_recoverable_timeout"
                } else {
                    "claim_failed_recoverable_abort"
                },
            }
        }
        ClaimState::Claimed { .. } => {
            refused(ClaimRefusal::AlreadyClaimed, "abort_refused_claimed")
        }
        ClaimState::Unclaimed { .. } => {
            refused(ClaimRefusal::ClaimIdMismatch, "abort_refused_unclaimed")
        }
    }
}

/// Console «Libertar» (physical presence) while `Claimed` and before D011
/// has passed P06 [P, PD-07]: back to `Unclaimed` with a new generation.
#[must_use]
pub fn local_release(
    state: &ClaimState,
    d011_past_p06: bool,
    now: i64,
    fresh_code: [u8; 16],
    last_code_generation: u32,
) -> Outcome {
    match state {
        ClaimState::Claimed { .. } if !d011_past_p06 => Outcome {
            event: ClaimEvent::Aborted,
            state: Some(ClaimState::Unclaimed {
                generation: generation_after(state),
                locked_until: None,
            }),
            code: Some(Some(new_code(
                fresh_code,
                last_code_generation + 1,
                now,
                None,
            ))),
            journal: "claim_released_locally",
        },
        ClaimState::Claimed { .. } => {
            refused(ClaimRefusal::AlreadyClaimed, "release_refused_provisioned")
        }
        _ => refused(ClaimRefusal::ClaimIdMismatch, "release_refused_not_claimed"),
    }
}

fn generation_after(state: &ClaimState) -> u32 {
    // Generations only grow; a rollback from a claim continues past the
    // generation it started in (recorded in the journal by the caller).
    match state {
        ClaimState::Unclaimed { generation, .. } => generation + 1,
        _ => 2,
    }
}

/// The client-visible view.
#[must_use]
pub fn view(state: &ClaimState, code: Option<&CodeRecord>) -> ClaimStateView {
    match state {
        ClaimState::Unclaimed { locked_until, .. } => ClaimStateView::Unclaimed {
            attempts_left: code.map_or(0, |c| c.attempts_left),
            code_expires_at: code.map_or_else(String::new, |c| rfc3339(c.expires_at)),
            locked_until: locked_until.clone(),
        },
        ClaimState::ClaimInProgress { deadline, .. } => ClaimStateView::ClaimInProgress {
            deadline: deadline.clone(),
        },
        ClaimState::Claimed { .. } => ClaimStateView::Claimed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(c: char) -> KeyFingerprint {
        KeyFingerprint(format!("SHA256:{}", c.to_string().repeat(43)))
    }

    const T0: i64 = 1_791_000_000;

    fn unclaimed() -> ClaimState {
        ClaimState::Unclaimed {
            generation: 1,
            locked_until: None,
        }
    }

    #[test]
    fn o_codigo_tem_125_bits_e_normaliza_o_que_se_escreve() {
        let c = PairingCode::from_random([0xab; 16]);
        assert_eq!(c.expose().len(), 29);
        assert!(c.expose().split('-').all(|g| g.len() == 5));
        assert!(!c.expose().contains(['I', 'L', 'O', 'U']));
        let typed = c.expose().to_lowercase().replace('-', " ");
        assert!(PairingCode::normalise(&typed).unwrap().matches(&c));
        assert!(PairingCode::normalise("short").is_none());
        assert!(PairingCode::normalise(&"U".repeat(25)).is_none());
        assert_eq!(format!("{c:?}"), "PairingCode(<redacted>)");
        // Distinct randomness → distinct codes.
        assert_ne!(
            PairingCode::from_random([1; 16]),
            PairingCode::from_random([2; 16])
        );
    }

    #[test]
    fn inscrever_e_confirmar_reclama_a_maquina() {
        let rec = new_code([7; 16], 1, T0, None);
        let o = enroll(
            &unclaimed(),
            Some(&rec),
            &rec.code,
            &key('a'),
            T0 + 5,
            [9; 16],
            [8; 16],
        );
        let ClaimEvent::Enrolled { claim_id, .. } = o.event.clone() else {
            panic!("{o:?}")
        };
        assert_eq!(o.code, Some(None), "code consumed");
        let st = o.state.unwrap();
        let c = confirm(&st, &claim_id, "ocinye", T0 + 60);
        assert!(matches!(c.event, ClaimEvent::Confirmed { .. }));
        assert!(matches!(
            c.state,
            Some(ClaimState::Claimed {
                method: ClaimMethod::PairingCode,
                ..
            })
        ));
    }

    #[test]
    fn repeticoes_sao_recusadas() {
        let rec = new_code([7; 16], 1, T0, None);
        let o = enroll(
            &unclaimed(),
            Some(&rec),
            &rec.code,
            &key('a'),
            T0,
            [9; 16],
            [8; 16],
        );
        let in_progress = o.state.unwrap();
        // The consumed code, again.
        assert_eq!(
            enroll(
                &in_progress,
                None,
                &rec.code,
                &key('b'),
                T0,
                [1; 16],
                [2; 16]
            )
            .event,
            ClaimEvent::Refused {
                reason: ClaimRefusal::ClaimInProgress {
                    deadline: rfc3339(T0 + CONFIRM_WINDOW_S)
                }
            }
        );
        // A different claim id.
        let wrong = ClaimId::from_bytes([0; 16]);
        assert_eq!(
            confirm(&in_progress, &wrong, "ocinye", T0).event,
            ClaimEvent::Refused {
                reason: ClaimRefusal::ClaimIdMismatch
            }
        );
        // Confirmed, then everything is ALREADY_CLAIMED.
        let ClaimState::ClaimInProgress { claim_id, .. } = &in_progress else {
            panic!()
        };
        let claimed = confirm(&in_progress, claim_id, "ocinye", T0).state.unwrap();
        assert_eq!(
            confirm(&claimed, claim_id, "ocinye", T0).event,
            ClaimEvent::Refused {
                reason: ClaimRefusal::AlreadyClaimed
            }
        );
        assert_eq!(
            enroll(
                &claimed,
                Some(&new_code([3; 16], 2, T0, None)),
                &rec.code,
                &key('a'),
                T0,
                [1; 16],
                [2; 16]
            )
            .event,
            ClaimEvent::Refused {
                reason: ClaimRefusal::AlreadyClaimed
            }
        );
        // Without a code record (consumed or after reboot) the old code is expired.
        assert_eq!(
            enroll(
                &unclaimed(),
                None,
                &rec.code,
                &key('a'),
                T0,
                [1; 16],
                [2; 16]
            )
            .event,
            ClaimEvent::Refused {
                reason: ClaimRefusal::CodeExpired
            }
        );
    }

    #[test]
    fn a_confirmacao_exige_a_conta_ocinye_e_o_prazo() {
        let rec = new_code([7; 16], 1, T0, None);
        let st = enroll(
            &unclaimed(),
            Some(&rec),
            &rec.code,
            &key('a'),
            T0,
            [9; 16],
            [8; 16],
        )
        .state
        .unwrap();
        let ClaimState::ClaimInProgress { claim_id, .. } = &st else {
            panic!()
        };
        assert_eq!(
            confirm(&st, claim_id, "root", T0).event,
            ClaimEvent::Refused {
                reason: ClaimRefusal::WrongAccount
            }
        );
        assert_eq!(
            confirm(&st, claim_id, "ocinye", T0 + CONFIRM_WINDOW_S).event,
            ClaimEvent::Refused {
                reason: ClaimRefusal::CodeExpired
            }
        );
    }

    #[test]
    fn o_prazo_devolve_a_maquina_e_gera_codigo_novo() {
        let rec = new_code([7; 16], 4, T0, None);
        let st = enroll(
            &unclaimed(),
            Some(&rec),
            &rec.code,
            &key('a'),
            T0,
            [9; 16],
            [8; 16],
        )
        .state
        .unwrap();
        assert!(
            matches!(
                roll_back(&st, None, T0 + 5, [5; 16], 4).event,
                ClaimEvent::Refused { .. }
            ),
            "not due"
        );
        let o = roll_back(&st, None, T0 + CONFIRM_WINDOW_S, [5; 16], 4);
        assert_eq!(o.journal, "claim_failed_recoverable_timeout");
        assert!(matches!(
            o.state,
            Some(ClaimState::Unclaimed { generation: 2, .. })
        ));
        let fresh = o.code.unwrap().unwrap();
        assert_eq!(fresh.code_generation, 5);
        assert!(!fresh.code.matches(&rec.code));
    }

    #[test]
    fn codigo_expirado_ou_esgotado_e_substituido_e_tres_esgotados_bloqueiam() {
        let rec = new_code([7; 16], 1, T0, None);
        let o = enroll(
            &unclaimed(),
            Some(&rec),
            &rec.code,
            &key('a'),
            T0 + CODE_LIFETIME_S,
            [9; 16],
            [8; 16],
        );
        assert_eq!(
            o.event,
            ClaimEvent::Refused {
                reason: ClaimRefusal::CodeExpired
            }
        );
        assert_eq!(o.code.unwrap().unwrap().code_generation, 2);

        let wrong = PairingCode::from_random([0; 16]);
        let mut cur = new_code([7; 16], 1, T0, None);
        let mut state = unclaimed();
        let mut locked = false;
        for round in 0..3 {
            for attempt in 0..ATTEMPTS_PER_CODE {
                let o = enroll(
                    &state,
                    Some(&cur),
                    &wrong,
                    &key('x'),
                    T0 + 1,
                    [9; 16],
                    [round * 10 + attempt; 16],
                );
                if let Some(s) = o.state {
                    state = s;
                }
                cur = o.code.unwrap().unwrap();
                if let ClaimEvent::Refused {
                    reason: ClaimRefusal::Locked { .. },
                } = o.event
                {
                    locked = true;
                }
            }
        }
        assert!(locked, "three exhausted codes lock the machine");
        let ClaimState::Unclaimed {
            locked_until: Some(until),
            ..
        } = &state
        else {
            panic!("{state:?}")
        };
        assert_eq!(unix(until), Some(T0 + 1 + LOCK_BASE_S));
        // While locked even the right code is refused.
        let o = enroll(
            &state,
            Some(&cur),
            &cur.code,
            &key('a'),
            T0 + 2,
            [9; 16],
            [8; 16],
        );
        assert!(matches!(
            o.event,
            ClaimEvent::Refused {
                reason: ClaimRefusal::Locked { .. }
            }
        ));
        // After the lock the code shown during it has expired and is replaced;
        // the replacement is accepted.
        let after = T0 + 2 + LOCK_BASE_S;
        let o = enroll(
            &state,
            Some(&cur),
            &cur.code,
            &key('a'),
            after,
            [9; 16],
            [8; 16],
        );
        assert_eq!(
            o.event,
            ClaimEvent::Refused {
                reason: ClaimRefusal::CodeExpired
            }
        );
        let fresh = o.code.unwrap().unwrap();
        let o = enroll(
            &state,
            Some(&fresh),
            &fresh.code,
            &key('a'),
            after + 1,
            [9; 16],
            [7; 16],
        );
        assert!(matches!(o.event, ClaimEvent::Enrolled { .. }), "{o:?}");
    }

    #[test]
    fn reclamacao_por_chave_provisionada() {
        let ok = claim_provisioned(&unclaimed(), &key('p'), &[key('p')], "ocinye", T0, [1; 16]);
        assert!(matches!(
            ok.state,
            Some(ClaimState::Claimed {
                method: ClaimMethod::ProvisionedKey,
                ..
            })
        ));
        assert_eq!(
            claim_provisioned(&unclaimed(), &key('q'), &[key('p')], "ocinye", T0, [1; 16]).event,
            ClaimEvent::Refused {
                reason: ClaimRefusal::NotProvisionedKey
            }
        );
        let claimed = ok.state.unwrap();
        assert_eq!(
            claim_provisioned(&claimed, &key('p'), &[key('p')], "ocinye", T0, [1; 16]).event,
            ClaimEvent::Refused {
                reason: ClaimRefusal::AlreadyClaimed
            }
        );
    }

    #[test]
    fn libertacao_local_so_antes_da_d011() {
        let claimed =
            claim_provisioned(&unclaimed(), &key('p'), &[key('p')], "ocinye", T0, [1; 16])
                .state
                .unwrap();
        assert!(matches!(
            local_release(&claimed, false, T0, [2; 16], 1).state,
            Some(ClaimState::Unclaimed { .. })
        ));
        assert!(local_release(&claimed, true, T0, [2; 16], 1)
            .state
            .is_none());
    }

    #[test]
    fn o_estado_persistido_segue_o_esquema() {
        let s = ClaimState::ClaimInProgress {
            claim_id: ClaimId::from_bytes([0xab; 16]),
            owner_key: key('a'),
            deadline: rfc3339(T0),
        };
        let j = ocinye_installer_contracts::canonical::to_canonical(&s).unwrap();
        assert!(j.contains("\"state\":\"CLAIM_IN_PROGRESS\""), "{j}");
        let back: ClaimState = serde_json::from_str(&j).unwrap();
        assert_eq!(back, s);
        assert!(serde_json::from_str::<ClaimState>(
            &j.replace("\"deadline\"", "\"extra\":1,\"deadline\"")
        )
        .is_err());
        let cmd: ClaimCommand = serde_json::from_str("{\"cmd\":\"hello\",\"protocol\":1}").unwrap();
        assert_eq!(cmd, ClaimCommand::Hello { protocol: 1 });
        assert!(
            serde_json::from_str::<ClaimCommand>("{\"cmd\":\"exec\",\"argv\":[\"sh\"]}").is_err()
        );
        assert!(
            serde_json::from_str::<ClaimCommand>("{\"cmd\":\"hello\",\"protocol\":1,\"x\":1}")
                .is_err()
        );
    }
}
