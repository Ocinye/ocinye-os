//! Detached signatures, trusted keys and revocations (ADR-0026,
//! D013_SIGNING_MODEL.md).
//!
//! minisign (Ed25519, prehashed) over `SHA256SUMS`. The trusted comment is the
//! publication statement and has a closed grammar; [`verify_release`] is the
//! one decision the Installer (E-03/E-06) and the image E2E make:
//! cryptography first, then key, validity, channel, revocation and the
//! statement against the files actually held. A development key can never
//! make an image look `stable`.

use serde::{Deserialize, Serialize};

use crate::build::Channel;
use crate::manifest::{Arch, Sha256Hex};

/// Key roles.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KeyRole {
    /// Offline; signs `trusted-keys.json` and `revocations.json`.
    Root,
    /// Hardware token; signs `candidate` and `stable`.
    Release,
    /// Signs `development` only.
    Development,
}

impl KeyRole {
    /// Channels this role may sign, whatever a key entry claims.
    #[must_use]
    pub fn may_sign(self, channel: Channel) -> bool {
        match self {
            Self::Release => matches!(channel, Channel::Candidate | Channel::Stable),
            Self::Development => channel == Channel::Development,
            Self::Root => false,
        }
    }
}

/// minisign key id: 16 uppercase hex, as `minisign` prints it.
#[must_use]
pub fn is_key_id(s: &str) -> bool {
    s.len() == 16
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'A'..=b'F').contains(&b))
}

/// `ocinye-os <id>-r<N>[-dev] <arch> channel=<c> sha256sums=<h> ts=<epoch>`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SignedStatement {
    /// `<id>-r<N>[-dev]` (without the product prefix).
    pub image: String,
    /// Architecture.
    pub arch: Arch,
    /// Channel.
    pub channel: Channel,
    /// SHA-256 of `SHA256SUMS`.
    pub sha256sums: Sha256Hex,
    /// Unix seconds.
    pub ts: i64,
}

fn channel_str(c: Channel) -> &'static str {
    match c {
        Channel::Development => "development",
        Channel::Candidate => "candidate",
        Channel::Stable => "stable",
    }
}

fn valid_image_tag(s: &str) -> bool {
    let core = s.strip_suffix("-dev").unwrap_or(s);
    let Some((id, rev)) = core.split_once("-r") else {
        return false;
    };
    crate::is_lower_hex(id, 12)
        && !rev.is_empty()
        && !rev.starts_with('0')
        && rev.bytes().all(|b| b.is_ascii_digit())
        && rev.parse::<u32>().is_ok()
}

impl SignedStatement {
    /// The exact trusted comment.
    #[must_use]
    pub fn render(&self) -> String {
        format!(
            "{} {} {} channel={} sha256sums={} ts={}",
            crate::PRODUCT,
            self.image,
            self.arch.as_str(),
            channel_str(self.channel),
            self.sha256sums.0,
            self.ts
        )
    }

    /// Strict parse: exactly six space-separated fields in this order, no
    /// extras, no duplicates, canonical spelling only (re-rendering gives
    /// back the input byte for byte).
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        let f: Vec<&str> = s.split(' ').collect();
        let [product, image, arch, channel, sums, ts] = f.as_slice() else {
            return None;
        };
        if *product != crate::PRODUCT || !valid_image_tag(image) {
            return None;
        }
        let arch = match *arch {
            "amd64" => Arch::Amd64,
            "arm64" => Arch::Arm64,
            _ => return None,
        };
        let channel = match channel.strip_prefix("channel=")? {
            "development" => Channel::Development,
            "candidate" => Channel::Candidate,
            "stable" => Channel::Stable,
            _ => return None,
        };
        let sha256sums = Sha256Hex(sums.strip_prefix("sha256sums=")?.to_owned());
        let ts_s = ts.strip_prefix("ts=")?;
        if !sha256sums.is_valid() || ts_s.is_empty() || !ts_s.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        let st = Self {
            image: (*image).to_owned(),
            arch,
            channel,
            sha256sums,
            ts: ts_s.parse().ok()?,
        };
        (st.render() == s).then_some(st)
    }
}

/// A delegated key (root-signed `trusted-keys.json`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TrustedKey {
    /// 16 uppercase hex.
    pub key_id: String,
    /// minisign public key, base64 (the second line of a `.pub`).
    pub public_key: String,
    /// Role.
    pub role: KeyRole,
    /// Channels allowed for this key (intersected with the role's).
    pub channels: Vec<Channel>,
    /// RFC 3339.
    pub not_before: String,
    /// RFC 3339.
    pub not_after: String,
}

/// `trusted-keys.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TrustedKeys {
    /// 1.
    pub schema: u32,
    /// RFC 3339.
    pub issued_at: String,
    /// Keys.
    pub keys: Vec<TrustedKey>,
}

/// Root-signed revocations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Revocations {
    /// 1.
    pub schema: u32,
    /// RFC 3339.
    pub issued_at: String,
    /// Revoked keys.
    pub keys: Vec<RevokedKey>,
    /// Revoked artifacts.
    pub artifacts: Vec<RevokedArtifact>,
}

/// A revoked key.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RevokedKey {
    /// Key id.
    pub key_id: String,
    /// RFC 3339: signatures after this instant are revoked; `None` = all.
    pub compromised_at: Option<String>,
    /// Why.
    pub reason: String,
}

/// A revoked artifact.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RevokedArtifact {
    /// Digest of the published file.
    pub sha256: Sha256Hex,
    /// File name.
    pub image: String,
    /// Why.
    pub reason: String,
    /// Replacement name.
    pub replaced_by: Option<String>,
}

/// Verification outcome (Installer E-03 / E-06).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "result", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SignatureVerification {
    /// All checks passed.
    Valid {
        /// Channel.
        channel: Channel,
        /// Role of the signing key.
        role: KeyRole,
        /// The statement.
        statement: SignedStatement,
    },
    /// Malformed signature file or failed Ed25519 check.
    InvalidSignature,
    /// No trusted key has that id.
    UnknownKey,
    /// The key is revoked.
    KeyRevoked,
    /// Outside the key's validity.
    KeyExpired,
    /// The key or its role may not sign that channel.
    ChannelNotAllowedForKey,
    /// The trusted comment is malformed or differs from what is held.
    StatementMismatch {
        /// Field.
        field: String,
    },
    /// A published artifact is revoked.
    ArtifactRevoked {
        /// Why.
        reason: String,
    },
    /// A file differs from `SHA256SUMS`.
    DigestMismatch {
        /// File.
        file: String,
    },
}

/// Key id of a minisign public key or signature line (bytes 2..10, shown as
/// minisign does: little-endian u64 in uppercase hex).
#[must_use]
pub fn key_id_of_b64(b64_line: &str) -> Option<String> {
    use base64::Engine as _;
    let bin = base64::engine::general_purpose::STANDARD
        .decode(b64_line.trim())
        .ok()?;
    let id: [u8; 8] = bin.get(2..10)?.try_into().ok()?;
    Some(format!("{:016X}", u64::from_le_bytes(id)))
}

/// What is expected of the release the Installer holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Expected<'a> {
    /// `<id>-r<N>[-dev]` from the verified image manifest.
    pub image: &'a str,
    /// Architecture.
    pub arch: Arch,
    /// SHA-256 of every published file, by name (from `SHA256SUMS` after
    /// re-hashing the files held).
    pub artifacts: &'a [(String, Sha256Hex)],
}

/// Verify `SHA256SUMS` + its `.minisig` against the trusted keys and the
/// revocations at `now` (unix seconds).
#[must_use]
pub fn verify_release(
    sums: &[u8],
    minisig: &str,
    trusted: &TrustedKeys,
    revoked: &Revocations,
    expected: &Expected<'_>,
    now: i64,
) -> SignatureVerification {
    use sha2::Digest as _;
    let Ok(sig) = minisign_verify::Signature::decode(minisig) else {
        return SignatureVerification::InvalidSignature;
    };
    let Some(sig_key_id) = minisig.lines().nth(1).and_then(key_id_of_b64) else {
        return SignatureVerification::InvalidSignature;
    };
    let Some(key) = trusted.keys.iter().find(|k| k.key_id == sig_key_id) else {
        return SignatureVerification::UnknownKey;
    };
    // The public key must carry the id it is listed under.
    if key_id_of_b64(&key.public_key).as_deref() != Some(key.key_id.as_str()) {
        return SignatureVerification::UnknownKey;
    }
    let Ok(pk) = minisign_verify::PublicKey::from_base64(&key.public_key) else {
        return SignatureVerification::UnknownKey;
    };
    // Prehashed (BLAKE2b) signatures only: legacy is refused.
    if pk.verify(sums, &sig, false).is_err() {
        return SignatureVerification::InvalidSignature;
    }
    let Some(st) = SignedStatement::parse(sig.trusted_comment()) else {
        return SignatureVerification::StatementMismatch {
            field: "trusted_comment".into(),
        };
    };
    let rfc = |s: &str| crate::claim::unix(s);
    if let Some(r) = revoked.keys.iter().find(|r| r.key_id == key.key_id) {
        match r.compromised_at.as_deref().map(rfc) {
            None => return SignatureVerification::KeyRevoked,
            Some(Some(at)) if st.ts >= at => return SignatureVerification::KeyRevoked,
            Some(None) => return SignatureVerification::KeyRevoked,
            Some(Some(_)) => {}
        }
    }
    match (rfc(&key.not_before), rfc(&key.not_after)) {
        (Some(nb), Some(na)) if nb <= now && now <= na && nb <= st.ts && st.ts <= na => {}
        _ => return SignatureVerification::KeyExpired,
    }
    if !key.role.may_sign(st.channel) || !key.channels.contains(&st.channel) {
        return SignatureVerification::ChannelNotAllowedForKey;
    }
    let digest = hex::encode(sha2::Sha256::digest(sums));
    if st.sha256sums.0 != digest {
        return SignatureVerification::StatementMismatch {
            field: "sha256sums".into(),
        };
    }
    if st.image != expected.image {
        return SignatureVerification::StatementMismatch {
            field: "image".into(),
        };
    }
    if st.arch != expected.arch {
        return SignatureVerification::StatementMismatch {
            field: "arch".into(),
        };
    }
    let dev_name = st.image.ends_with("-dev");
    if dev_name != (st.channel == Channel::Development) {
        return SignatureVerification::StatementMismatch {
            field: "channel".into(),
        };
    }
    let listed = match parse_sha256sums(sums) {
        Some(l) => l,
        None => {
            return SignatureVerification::StatementMismatch {
                field: "sha256sums".into(),
            }
        }
    };
    for (file, sha) in expected.artifacts {
        if listed
            .iter()
            .find(|(f, _)| f == file)
            .is_none_or(|(_, s)| s != sha)
        {
            return SignatureVerification::DigestMismatch { file: file.clone() };
        }
        if let Some(r) = revoked.artifacts.iter().find(|r| &r.sha256 == sha) {
            return SignatureVerification::ArtifactRevoked {
                reason: r.reason.clone(),
            };
        }
    }
    SignatureVerification::Valid {
        channel: st.channel,
        role: key.role,
        statement: st,
    }
}

/// `SHA256SUMS`: sorted, LF, `<64 hex>  <name>` lines, safe names, no
/// duplicates. `None` if anything else is in it.
#[must_use]
pub fn parse_sha256sums(bytes: &[u8]) -> Option<Vec<(String, Sha256Hex)>> {
    let text = std::str::from_utf8(bytes).ok()?;
    if text.is_empty() || !text.ends_with('\n') || text.contains('\r') {
        return None;
    }
    let mut out: Vec<(String, Sha256Hex)> = Vec::new();
    for line in text.lines() {
        let (h, name) = line.split_once("  ")?;
        let h = Sha256Hex(h.to_owned());
        if !h.is_valid() || !crate::manifest::is_safe_file_name(name) {
            return None;
        }
        if out.last().is_some_and(|(prev, _)| prev.as_str() >= name) {
            return None;
        }
        out.push((name.to_owned(), h));
    }
    Some(out)
}

/// Render `SHA256SUMS` from `(name, digest)` pairs.
#[must_use]
pub fn render_sha256sums(entries: &[(String, Sha256Hex)]) -> String {
    let mut e: Vec<_> = entries.to_vec();
    e.sort_by(|a, b| a.0.cmp(&b.0));
    e.iter().map(|(n, h)| format!("{}  {n}\n", h.0)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine as _;
    use blake2::Digest as _;
    use ed25519_dalek::Signer as _;

    const NOW: i64 = 1_791_000_000;

    struct TestKey {
        sk: ed25519_dalek::SigningKey,
        id: [u8; 8],
    }

    impl TestKey {
        fn new(seed: u8) -> Self {
            Self {
                sk: ed25519_dalek::SigningKey::from_bytes(&[seed; 32]),
                id: [seed, 0xd1, 0xe5, 0, 0, 0, 0, 0x7e],
            }
        }
        fn key_id(&self) -> String {
            format!("{:016X}", u64::from_le_bytes(self.id))
        }
        fn public_b64(&self) -> String {
            let mut b = vec![b'E', b'd'];
            b.extend_from_slice(&self.id);
            b.extend_from_slice(self.sk.verifying_key().as_bytes());
            base64::engine::general_purpose::STANDARD.encode(b)
        }
        /// A minisign prehashed signature, as `minisign -S -H` writes it.
        fn sign(&self, data: &[u8], trusted: &str) -> String {
            let h = blake2::Blake2b512::digest(data);
            let sig = self.sk.sign(&h).to_bytes();
            let mut l2 = vec![b'E', b'D'];
            l2.extend_from_slice(&self.id);
            l2.extend_from_slice(&sig);
            let mut global = sig.to_vec();
            global.extend_from_slice(trusted.as_bytes());
            let g = self.sk.sign(&global).to_bytes();
            let e = &base64::engine::general_purpose::STANDARD;
            format!(
                "untrusted comment: test\n{}\ntrusted comment: {trusted}\n{}\n",
                e.encode(l2),
                e.encode(g)
            )
        }
        fn trusted(&self, role: KeyRole, channels: Vec<Channel>) -> TrustedKey {
            TrustedKey {
                key_id: self.key_id(),
                public_key: self.public_b64(),
                role,
                channels,
                not_before: "2026-01-01T00:00:00Z".into(),
                not_after: "2027-01-31T00:00:00Z".into(),
            }
        }
    }

    fn sha(b: &[u8]) -> Sha256Hex {
        use sha2::Digest as _;
        Sha256Hex(hex::encode(sha2::Sha256::digest(b)))
    }

    struct Fixture {
        sums: String,
        artifacts: Vec<(String, Sha256Hex)>,
    }

    fn fixture() -> Fixture {
        let artifacts = vec![
            ("IMAGE_MANIFEST.json".to_owned(), sha(b"{}")),
            (
                "ocinye-os-0123456789ab-r1-dev-amd64.iso".to_owned(),
                sha(b"iso bytes"),
            ),
        ];
        Fixture {
            sums: render_sha256sums(&artifacts),
            artifacts,
        }
    }

    fn statement(f: &Fixture, image: &str, channel: Channel) -> String {
        SignedStatement {
            image: image.into(),
            arch: Arch::Amd64,
            channel,
            sha256sums: sha(f.sums.as_bytes()),
            ts: NOW,
        }
        .render()
    }

    fn empty_rev() -> Revocations {
        Revocations {
            schema: 1,
            issued_at: "2026-10-01T00:00:00Z".into(),
            keys: vec![],
            artifacts: vec![],
        }
    }

    fn run(
        f: &Fixture,
        sig: &str,
        keys: Vec<TrustedKey>,
        rev: &Revocations,
        image: &str,
    ) -> SignatureVerification {
        let trusted = TrustedKeys {
            schema: 1,
            issued_at: "2026-10-01T00:00:00Z".into(),
            keys,
        };
        verify_release(
            f.sums.as_bytes(),
            sig,
            &trusted,
            rev,
            &Expected {
                image,
                arch: Arch::Amd64,
                artifacts: &f.artifacts,
            },
            NOW,
        )
    }

    const DEV: &str = "0123456789ab-r1-dev";
    const REL: &str = "0123456789ab-r1";

    #[test]
    fn valida_e_cada_falha_dura() {
        let f = fixture();
        let dev = TestKey::new(7);
        let dk = || vec![dev.trusted(KeyRole::Development, vec![Channel::Development])];
        let good = dev.sign(f.sums.as_bytes(), &statement(&f, DEV, Channel::Development));
        assert!(matches!(
            run(&f, &good, dk(), &empty_rev(), DEV),
            SignatureVerification::Valid {
                channel: Channel::Development,
                ..
            }
        ));

        // Wrong key.
        let other = TestKey::new(9);
        let by_other = other.sign(f.sums.as_bytes(), &statement(&f, DEV, Channel::Development));
        assert_eq!(
            run(&f, &by_other, dk(), &empty_rev(), DEV),
            SignatureVerification::UnknownKey
        );
        // A key listed under another key's id.
        let mut swapped = other.trusted(KeyRole::Development, vec![Channel::Development]);
        swapped.key_id = dev.key_id();
        assert_eq!(
            run(&f, &good, vec![swapped], &empty_rev(), DEV),
            SignatureVerification::UnknownKey
        );

        // Invalid signature: flip a byte of the signature line.
        let mut lines: Vec<String> = good.lines().map(Into::into).collect();
        let mut raw = base64::engine::general_purpose::STANDARD
            .decode(&lines[1])
            .unwrap();
        raw[20] ^= 1;
        lines[1] = base64::engine::general_purpose::STANDARD.encode(raw);
        assert_eq!(
            run(&f, &(lines.join("\n") + "\n"), dk(), &empty_rev(), DEV),
            SignatureVerification::InvalidSignature
        );

        // Modified SHA256SUMS.
        let mut g = fixture();
        g.sums.push_str(&format!("{}  zz-extra\n", sha(b"x").0));
        assert_eq!(
            run(&g, &good, dk(), &empty_rev(), DEV),
            SignatureVerification::InvalidSignature
        );

        // Modified artifact (re-hashed by the holder).
        let mut h = fixture();
        h.artifacts[1].1 = sha(b"other iso bytes");
        assert_eq!(
            run(&h, &good, dk(), &empty_rev(), DEV),
            SignatureVerification::DigestMismatch {
                file: h.artifacts[1].0.clone()
            }
        );
        // Modified IMAGE_MANIFEST.json.
        let mut m = fixture();
        m.artifacts[0].1 = sha(b"{\"x\":1}");
        assert_eq!(
            run(&m, &good, dk(), &empty_rev(), DEV),
            SignatureVerification::DigestMismatch {
                file: "IMAGE_MANIFEST.json".into()
            }
        );

        // Revoked key.
        let mut rev = empty_rev();
        rev.keys.push(RevokedKey {
            key_id: dev.key_id(),
            compromised_at: None,
            reason: "test".into(),
        });
        assert_eq!(
            run(&f, &good, dk(), &rev, DEV),
            SignatureVerification::KeyRevoked
        );
        // Compromised after the signature: still valid.
        rev.keys[0].compromised_at = Some(crate::claim::rfc3339(NOW + 10));
        assert!(matches!(
            run(&f, &good, dk(), &rev, DEV),
            SignatureVerification::Valid { .. }
        ));

        // Revoked artifact.
        let mut rev = empty_rev();
        rev.artifacts.push(RevokedArtifact {
            sha256: f.artifacts[1].1.clone(),
            image: f.artifacts[1].0.clone(),
            reason: "cve".into(),
            replaced_by: None,
        });
        assert_eq!(
            run(&f, &good, dk(), &rev, DEV),
            SignatureVerification::ArtifactRevoked {
                reason: "cve".into()
            }
        );

        // Expired key.
        let mut exp = dev.trusted(KeyRole::Development, vec![Channel::Development]);
        exp.not_after = "2026-02-01T00:00:00Z".into();
        assert_eq!(
            run(&f, &good, vec![exp], &empty_rev(), DEV),
            SignatureVerification::KeyExpired
        );
    }

    #[test]
    fn chave_de_desenvolvimento_nunca_assina_stable() {
        let f = fixture();
        let dev = TestKey::new(7);
        // Even if the trusted-keys entry wrongly allows stable, the role forbids it.
        let lax = vec![dev.trusted(
            KeyRole::Development,
            vec![Channel::Development, Channel::Stable],
        )];
        let sig = dev.sign(f.sums.as_bytes(), &statement(&f, REL, Channel::Stable));
        assert_eq!(
            run(&f, &sig, lax, &empty_rev(), REL),
            SignatureVerification::ChannelNotAllowedForKey
        );
        // A release key signing a -dev name as stable: statement mismatch.
        let rel = TestKey::new(3);
        let rk = vec![rel.trusted(KeyRole::Release, vec![Channel::Candidate, Channel::Stable])];
        let sig = rel.sign(f.sums.as_bytes(), &statement(&f, DEV, Channel::Stable));
        assert_eq!(
            run(&f, &sig, rk, &empty_rev(), DEV),
            SignatureVerification::StatementMismatch {
                field: "channel".into()
            }
        );
    }

    #[test]
    fn comentario_confiavel_com_campos_extra_falha() {
        let f = fixture();
        let dev = TestKey::new(7);
        let dk = vec![dev.trusted(KeyRole::Development, vec![Channel::Development])];
        let extra = format!("{} note=x", statement(&f, DEV, Channel::Development));
        let sig = dev.sign(f.sums.as_bytes(), &extra);
        assert_eq!(
            run(&f, &sig, dk, &empty_rev(), DEV),
            SignatureVerification::StatementMismatch {
                field: "trusted_comment".into()
            }
        );
        let base = statement(&f, DEV, Channel::Development);
        for bad in [
            base.replacen("ocinye-os", "ocinye", 1),
            base.replacen(" amd64", " x86_64", 1),
            base.replacen("ts=", "ts=+", 1),
            base.replacen("-r1-", "-r01-", 1),
            base.replacen(' ', "  ", 1),
            base.replacen("channel=development", "channel=Development", 1),
        ] {
            assert!(SignedStatement::parse(&bad).is_none(), "{bad}");
        }
        assert!(SignedStatement::parse(&base).is_some());
    }

    #[test]
    fn sha256sums_e_estrito() {
        let f = fixture();
        assert_eq!(parse_sha256sums(f.sums.as_bytes()).unwrap(), f.artifacts);
        let h = sha(b"a").0;
        for bad in [
            format!("{h} a\n"),
            format!("{h}  b\n{h}  a\n"),
            format!("{h}  a\n{h}  a\n"),
            format!("{h}  ../a\n"),
            format!("{h}  a\r\n"),
            format!("{h}  a"),
            String::new(),
        ] {
            assert!(parse_sha256sums(bad.as_bytes()).is_none(), "{bad:?}");
        }
    }

    #[test]
    fn as_chaves_sinteticas_do_design_leem_se() {
        let k: TrustedKeys = serde_json::from_str(
            r#"{"schema":1,"issued_at":"2026-10-01T00:00:00Z","keys":[{"key_id":"E0E0000000000001","public_key":"x","role":"release","channels":["candidate","stable"],"not_before":"2026-01-01T00:00:00Z","not_after":"2027-01-31T00:00:00Z"}]}"#,
        )
        .unwrap();
        assert!(is_key_id(&k.keys[0].key_id));
        assert!(!is_key_id("e0e0000000000001"));
    }
}
