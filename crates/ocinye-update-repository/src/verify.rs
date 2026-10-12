//! Reading a repository as a client does.

use crate::{err, target_name, Error, Result};
use ocinye_update_contracts::{UpdateManifest, MANIFEST_TARGET};
use std::path::Path;
use tough::{ExpirationEnforcement, IntoVec, Repository, RepositoryLoader, TargetName, Transport};
use url::Url;

/// A release found in a repository, with its manifest verified against the
/// signed targets.
#[derive(Debug, Clone)]
pub struct Release {
    /// The manifest.
    pub manifest: UpdateManifest,
}

/// Where a channel lives in a repository directory.
#[must_use]
pub fn channel_dir(repo: &Path, channel: ocinye_update_contracts::Channel) -> std::path::PathBuf {
    repo.join(channel.as_str())
}

/// A `file://` URL of a directory, with the trailing slash TUF needs.
pub fn dir_url(dir: &Path) -> Result<Url> {
    let abs = std::fs::canonicalize(dir)?;
    Url::from_directory_path(&abs)
        .map_err(|()| Error(format!("{} is not a directory", abs.display())))
}

/// Load a repository: verify the root chain from `root`, then timestamp,
/// snapshot and targets, with expiry enforced unless `allow_expired` (only the
/// publisher, re-signing an expired channel, sets it).
pub async fn load(
    root: &[u8],
    metadata_url: Url,
    targets_url: Url,
    datastore: Option<&Path>,
    transport: impl Transport + 'static,
    allow_expired: bool,
) -> Result<Repository> {
    let mut loader = RepositoryLoader::new(&root, metadata_url, targets_url).transport(transport);
    if let Some(d) = datastore {
        loader = loader.datastore(d);
    }
    if allow_expired {
        loader = loader.expiration_enforcement(ExpirationEnforcement::Unsafe);
    }
    Ok(loader.load().await?)
}

/// Load a repository from a directory of the publisher.
pub async fn load_dir(
    repo: &Path,
    channel: ocinye_update_contracts::Channel,
    root: &[u8],
    allow_expired: bool,
) -> Result<Repository> {
    let dir = channel_dir(repo, channel);
    load(
        root,
        dir_url(&dir.join("metadata"))?,
        dir_url(&dir.join("targets"))?,
        None,
        tough::FilesystemTransport,
        allow_expired,
    )
    .await
}

/// Length and lower-case hex SHA-256 of a target, from the signed metadata.
#[must_use]
pub fn target_info(repo: &Repository, name: &str) -> Option<(u64, String)> {
    let name = TargetName::new(name).ok()?;
    let t = repo.targets().signed.targets.get(&name)?;
    Some((t.length, hex::encode(&t.hashes.sha256)))
}

/// The path of a target under the targets base, with consistent snapshots.
#[must_use]
pub fn consistent_path(name: &str, sha256: &str) -> String {
    format!("{sha256}.{name}")
}

/// Every release of the repository, newest number first. A release whose
/// manifest fails validation, or whose artifacts are not targets with the
/// same length and hash, is an error: a signed repository never offers half
/// a release.
pub async fn releases(repo: &Repository) -> Result<Vec<Release>> {
    let mut out = Vec::new();
    let names: Vec<String> = repo
        .targets()
        .signed
        .targets
        .keys()
        .map(|n| n.raw().to_owned())
        .filter(|n| n.starts_with("releases/") && n.ends_with(&format!("/{MANIFEST_TARGET}")))
        .collect();
    for name in names {
        let manifest = read_manifest(repo, &name).await?;
        let expected = target_name(&manifest.version, MANIFEST_TARGET);
        if expected != name {
            return err(format!("{name} names another release"));
        }
        out.push(Release { manifest });
    }
    out.sort_by_key(|r| std::cmp::Reverse(r.manifest.release_number));
    Ok(out)
}

/// Read and check one release's manifest.
pub async fn read_manifest(repo: &Repository, name: &str) -> Result<UpdateManifest> {
    let target = TargetName::new(name)?;
    let stream = repo
        .read_target(&target)
        .await?
        .ok_or_else(|| Error(format!("{name} is not a target")))?;
    let bytes = stream
        .into_vec()
        .await
        .map_err(|e| Error(format!("tuf: {e}")))?;
    let manifest = UpdateManifest::parse(&bytes).map_err(|e| Error(e.to_string()))?;
    for a in &manifest.artifacts {
        let full = target_name(&manifest.version, &a.target);
        match target_info(repo, &full) {
            Some((len, sha)) if len == a.bytes && sha == a.sha256 => {}
            Some(_) => {
                return err(format!(
                    "{full}: the manifest and the signed targets disagree"
                ))
            }
            None => return err(format!("{full} is not a signed target")),
        }
    }
    Ok(manifest)
}
