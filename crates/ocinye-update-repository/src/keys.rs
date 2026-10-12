//! Role keys and the signed root.
//!
//! Four TUF roles, one Ed25519 key each in development: `root`, `targets`,
//! `snapshot`, `timestamp`. A key is a PKCS#8 file, `0600`, in a directory
//! `0700` outside the repository and outside any image (ADR-0026, ADR-0032).
//! The root lists the public keys and thresholds; it is the only thing a
//! machine trusts, and it is the only thing an image carries.

use crate::{err, Error, Result};
use aws_lc_rs::rand::SystemRandom;
use aws_lc_rs::signature::Ed25519KeyPair;
use std::collections::HashMap;
use std::num::NonZeroU64;
use std::path::{Path, PathBuf};
use tough::editor::signed::SignedRole;
use tough::key_source::{KeySource, LocalKeySource};
use tough::schema::{KeyHolder, RoleKeys, RoleType, Root, Signed};

/// The four roles.
pub const ROLES: [RoleType; 4] = [
    RoleType::Root,
    RoleType::Targets,
    RoleType::Snapshot,
    RoleType::Timestamp,
];

fn role_file(role: RoleType) -> &'static str {
    match role {
        RoleType::Root => "root.pk8",
        RoleType::Targets => "targets.pk8",
        RoleType::Snapshot => "snapshot.pk8",
        RoleType::Timestamp => "timestamp.pk8",
        _ => "unused.pk8",
    }
}

/// A directory of role keys.
#[derive(Debug, Clone)]
pub struct KeySet {
    dir: PathBuf,
}

impl KeySet {
    /// Generate a new set in an empty or absent directory. Refuses to
    /// overwrite a key: a lost key is a ceremony, not a re-run.
    pub fn generate(dir: &Path) -> Result<Self> {
        use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt};
        std::fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(dir)?;
        std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))?;
        for role in ROLES {
            if dir.join(role_file(role)).exists() {
                return err(format!("a {role} key already exists in {}", dir.display()));
            }
        }
        let rng = SystemRandom::new();
        for role in ROLES {
            let doc = Ed25519KeyPair::generate_pkcs8(&rng)
                .map_err(|_| Error("key generation failed".into()))?;
            let mut f = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(dir.join(role_file(role)))?;
            std::io::Write::write_all(&mut f, doc.as_ref())?;
            f.sync_all()?;
        }
        Ok(Self {
            dir: dir.to_owned(),
        })
    }

    /// Open an existing set; every key must be there and private.
    pub fn open(dir: &Path) -> Result<Self> {
        use std::os::unix::fs::PermissionsExt;
        for role in ROLES {
            let p = dir.join(role_file(role));
            let meta = std::fs::metadata(&p)
                .map_err(|_| Error(format!("missing {role} key in {}", dir.display())))?;
            if meta.permissions().mode() & 0o077 != 0 {
                return err(format!("{} is readable by others", p.display()));
            }
        }
        Ok(Self {
            dir: dir.to_owned(),
        })
    }

    /// One role's key source.
    #[must_use]
    pub fn source(&self, role: RoleType) -> Box<dyn KeySource> {
        Box::new(LocalKeySource {
            path: self.dir.join(role_file(role)),
        })
    }

    /// Every role's key source.
    #[must_use]
    pub fn sources(&self) -> Vec<Box<dyn KeySource>> {
        ROLES.iter().map(|r| self.source(*r)).collect()
    }
}

async fn public(source: &dyn KeySource) -> Result<tough::schema::key::Key> {
    Ok(source
        .as_sign()
        .await
        .map_err(|e| Error(format!("key: {e}")))?
        .tuf_key())
}

/// A root of version `version` naming `keys` for every role, threshold 1,
/// expiring at `expires`, signed by the root key. `consistent_snapshot` is on:
/// targets and metadata are named by content or version, so a mirror never
/// serves a half-updated file under a stable name.
pub async fn create_root(keys: &KeySet, version: u64, expires: jiff::Timestamp) -> Result<Vec<u8>> {
    let mut root = Root {
        spec_version: "1.0.0".into(),
        consistent_snapshot: true,
        version: NonZeroU64::new(version).ok_or_else(|| Error("version starts at 1".into()))?,
        expires,
        keys: HashMap::new(),
        roles: HashMap::new(),
        _extra: HashMap::new(),
    };
    for role in ROLES {
        let key = public(keys.source(role).as_ref()).await?;
        let id = key.key_id()?;
        root.keys.insert(id.clone(), key);
        root.roles.insert(
            role,
            RoleKeys {
                keyids: vec![id],
                threshold: NonZeroU64::MIN,
                _extra: HashMap::new(),
            },
        );
    }
    sign_root(root, &[keys.source(RoleType::Root)]).await
}

/// The next root, for a rotation to `new_keys`: version + 1, the root role
/// listing the old and the new root key with threshold 1, every other role on
/// the new keys, signed by both root keys. A machine that trusts the old root
/// verifies the new one with the old key, and from then on trusts the new key;
/// a further rotation with only `new_keys` retires the old one.
pub async fn rotate_root(
    current: &[u8],
    old_keys: &KeySet,
    new_keys: &KeySet,
    expires: jiff::Timestamp,
) -> Result<Vec<u8>> {
    let signed: Signed<Root> =
        serde_json::from_slice(current).map_err(|e| Error(format!("root: {e}")))?;
    let mut root = signed.signed;
    let next = root.version.get() + 1;
    root.version = NonZeroU64::new(next).ok_or_else(|| Error("root version".into()))?;
    root.expires = expires;
    let old_root_key = public(old_keys.source(RoleType::Root).as_ref()).await?;
    let old_root_id = old_root_key.key_id()?;
    root.keys.clear();
    root.roles.clear();
    for role in ROLES {
        let key = public(new_keys.source(role).as_ref()).await?;
        let id = key.key_id()?;
        root.keys.insert(id.clone(), key);
        let mut ids = vec![id];
        if role == RoleType::Root && ids[0] != old_root_id {
            root.keys.insert(old_root_id.clone(), old_root_key.clone());
            ids.push(old_root_id.clone());
        }
        root.roles.insert(
            role,
            RoleKeys {
                keyids: ids,
                threshold: NonZeroU64::MIN,
                _extra: HashMap::new(),
            },
        );
    }
    sign_root(
        root,
        &[
            old_keys.source(RoleType::Root),
            new_keys.source(RoleType::Root),
        ],
    )
    .await
}

async fn sign_root(root: Root, sources: &[Box<dyn KeySource>]) -> Result<Vec<u8>> {
    let holder = KeyHolder::Root(root.clone());
    let signed = SignedRole::new(root, &holder, sources, &SystemRandom::new()).await?;
    Ok(signed.buffer().clone())
}

/// The version of a signed root.
pub fn root_version(bytes: &[u8]) -> Result<u64> {
    let signed: Signed<Root> =
        serde_json::from_slice(bytes).map_err(|e| Error(format!("root: {e}")))?;
    Ok(signed.signed.version.get())
}
