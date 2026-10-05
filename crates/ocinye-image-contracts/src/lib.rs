//! Typed contracts of the Ocinye OS images (D013, ADR-0026 to ADR-0029).
//!
//! D013 Code Phase A — `PROVISIONAL_PENDING_D011_CERTIFICATION` wherever a type
//! leans on a D011 contract (marked `[P]` with its dependency id, see
//! `docs/install/images-d011-dependencies.md`). Canonical JSON and secret
//! handling are the D011 ones (`ocinye_installer_contracts`, PD-02).
//!
//! No `HashMap<String, Value>` anywhere: manifests, signatures, claim and
//! machine identity are closed types with `deny_unknown_fields`, and the rules
//! that decide anything — claim transitions, disk eligibility, the signed
//! statement grammar, manifest validation — are pure functions here, so the
//! programs that run on a server only carry them out.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

pub mod build;
pub mod claim;
pub mod firstboot;
pub mod manifest;
pub mod oie;
pub mod receipt;
pub mod signing;

/// Version of the claim protocol spoken by `ocinye-firstboot claim-serve`.
pub const CLAIM_PROTOCOL: u16 = 1;
/// Schema of `OcinyeImageManifest`, `ImageContentManifest` and their siblings.
pub const IMAGE_MANIFEST_SCHEMA: u32 = 1;
/// Product name in every manifest.
pub const PRODUCT: &str = "ocinye-os";

/// Fixed image paths. The operator never provides a path on the server.
pub mod paths {
    /// Embedded release payload (read-only).
    pub const RELEASE_ROOT: &str = "/usr/lib/ocinye/release";
    /// Embedded content manifest.
    pub const IMAGE_CONTENT: &str = "/usr/lib/ocinye/image/IMAGE_CONTENT.json";
    /// Non-secret image facts.
    pub const IMAGE_FACTS: &str = "/etc/ocinye/image.json";
    /// First-boot state, identity and journal (root 0700).
    pub const FIRSTBOOT_STATE: &str = "/var/lib/ocinye-firstboot";
    /// Volatile pairing code and offered keys (tmpfs).
    pub const FIRSTBOOT_RUN: &str = "/run/ocinye-firstboot";
    /// OIE installation journal on the installed disk.
    pub const OIE_JOURNAL: &str = "/var/log/ocinye/oie-install.json";
    /// The firstboot program (forced command target).
    pub const FIRSTBOOT_BIN: &str = "/usr/lib/ocinye/ocinye-firstboot";
}

/// Lowercase hex of exactly `len` characters.
#[must_use]
pub fn is_lower_hex(s: &str, len: usize) -> bool {
    s.len() == len
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
