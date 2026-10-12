//! Adding a release to a channel.
//!
//! ```text
//! <repo>/<channel>/metadata/   N.root.json, root.json, N.targets.json,
//!                              N.snapshot.json, timestamp.json
//! <repo>/<channel>/targets/    <sha256>.releases/<version>/…
//! ```
//!
//! Publication is atomic for a client: targets are content-addressed and never
//! replaced; versioned metadata files are new names; `timestamp.json` — the
//! only file a client reads by a fixed name — is renamed into place last. A
//! client that reads before the rename sees the previous release set, intact;
//! after it, the new one.

use crate::keys::{root_version, KeySet};
use crate::verify::{channel_dir, load_dir, releases};
use crate::{err, target_name, Error, Result};
use ocinye_update_contracts::{Channel, UpdateManifest};
use std::num::NonZeroU64;
use std::path::{Path, PathBuf};
use tough::editor::signed::PathExists;
use tough::editor::RepositoryEditor;
use tough::TargetName;

/// How long each role's metadata stays valid.
#[derive(Debug, Clone, Copy)]
pub struct Expiry {
    /// `targets`, days.
    pub targets_days: i64,
    /// `snapshot`, days.
    pub snapshot_days: i64,
    /// `timestamp`, days. A machine refuses metadata older than this: it is
    /// what stops a frozen mirror. An offline bundle must be imported within
    /// it.
    pub timestamp_days: i64,
}

impl Expiry {
    /// The policy of a channel.
    #[must_use]
    pub const fn for_channel(channel: Channel) -> Self {
        match channel {
            Channel::Development => Self {
                targets_days: 90,
                snapshot_days: 60,
                timestamp_days: 30,
            },
            Channel::Candidate => Self {
                targets_days: 90,
                snapshot_days: 30,
                timestamp_days: 14,
            },
            Channel::Stable => Self {
                targets_days: 90,
                snapshot_days: 30,
                timestamp_days: 7,
            },
        }
    }
}

fn days(n: i64) -> Result<jiff::Timestamp> {
    jiff::Timestamp::now()
        .checked_add(jiff::SignedDuration::from_hours(n * 24))
        .map_err(|e| Error(e.to_string()))
}

fn metadata_dir(repo: &Path, channel: Channel) -> PathBuf {
    channel_dir(repo, channel).join("metadata")
}

/// Create a channel with its first root.
pub fn init_channel(repo: &Path, channel: Channel, root: &[u8]) -> Result<()> {
    let m = metadata_dir(repo, channel);
    if m.join("1.root.json").exists() {
        return err(format!("channel {} already exists", channel.as_str()));
    }
    if root_version(root)? != 1 {
        return err("a channel starts at root version 1");
    }
    std::fs::create_dir_all(&m)?;
    std::fs::create_dir_all(channel_dir(repo, channel).join("targets"))?;
    write_atomic(&m.join("1.root.json"), root)?;
    write_atomic(&m.join("root.json"), root)?;
    Ok(())
}

/// Add a rotated root (`N.root.json`, then `root.json`).
pub fn add_root(repo: &Path, channel: Channel, root: &[u8]) -> Result<()> {
    let m = metadata_dir(repo, channel);
    let v = root_version(root)?;
    let latest = latest_root(&m)?.0;
    if v != latest + 1 {
        return err(format!("the next root is version {}, not {v}", latest + 1));
    }
    write_atomic(&m.join(format!("{v}.root.json")), root)?;
    write_atomic(&m.join("root.json"), root)?;
    Ok(())
}

/// The first root: what a machine is installed with.
pub fn first_root(repo: &Path, channel: Channel) -> Result<Vec<u8>> {
    Ok(std::fs::read(
        metadata_dir(repo, channel).join("1.root.json"),
    )?)
}

fn latest_root(m: &Path) -> Result<(u64, PathBuf)> {
    let mut best = None;
    for e in std::fs::read_dir(m)? {
        let name = e?.file_name().to_string_lossy().into_owned();
        if let Some(n) = name
            .strip_suffix(".root.json")
            .and_then(|v| v.parse::<u64>().ok())
        {
            if best.as_ref().is_none_or(|(b, _)| n > *b) {
                best = Some((n, m.join(&name)));
            }
        }
    }
    best.ok_or_else(|| Error(format!("{} has no root", m.display())))
}

fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    let tmp = path.with_extension("tmp-publish");
    {
        let mut f = std::fs::File::create(&tmp)?;
        std::io::Write::write_all(&mut f, bytes)?;
        f.sync_all()?;
    }
    std::fs::rename(&tmp, path)?;
    if let Some(dir) = path.parent() {
        std::fs::File::open(dir)?.sync_all()?;
    }
    Ok(())
}

fn files_under(dir: &Path) -> Result<Vec<(String, PathBuf)>> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_owned()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d)? {
            let e = e?;
            let t = e.file_type()?;
            let p = e.path();
            if t.is_dir() {
                stack.push(p);
            } else if t.is_file() {
                let rel = p
                    .strip_prefix(dir)
                    .map_err(|_| Error("path".into()))?
                    .to_str()
                    .ok_or_else(|| Error("a staged path is not UTF-8".into()))?
                    .to_owned();
                out.push((rel, p));
            } else {
                return err(format!("{} is not a regular file", p.display()));
            }
        }
    }
    out.sort();
    Ok(out)
}

/// What a publication produced.
#[derive(Debug, Clone)]
pub struct Published {
    /// The release.
    pub manifest: UpdateManifest,
    /// Version of `targets.json` now.
    pub targets_version: u64,
    /// Version of `timestamp.json` now.
    pub timestamp_version: u64,
}

/// Publish the release staged in `staged` (see [`crate::stage`]) on `channel`.
pub async fn publish_release(
    repo: &Path,
    channel: Channel,
    keys: &KeySet,
    staged: &Path,
    expiry: Expiry,
) -> Result<Published> {
    let manifest = UpdateManifest::parse(&std::fs::read(staged.join("manifest.json"))?)
        .map_err(|e| Error(e.to_string()))?;
    if manifest.channel != channel {
        return err("the staged release is for another channel");
    }
    let m = metadata_dir(repo, channel);
    let targets_dir = channel_dir(repo, channel).join("targets");
    let root_path = latest_root(&m)?.1;
    let first = first_root(repo, channel)?;
    let existing = if m.join("timestamp.json").exists() {
        Some(load_dir(repo, channel, &first, true).await?)
    } else {
        None
    };
    let (mut editor, next) = match existing {
        Some(repo_loaded) => {
            for r in releases(&repo_loaded).await? {
                if r.manifest.release_number >= manifest.release_number {
                    return err(format!(
                        "release number {} is not above {} already on {}",
                        manifest.release_number.0,
                        r.manifest.release_number.0,
                        channel.as_str()
                    ));
                }
                if r.manifest.version == manifest.version {
                    return err(format!("{} is already published", manifest.version));
                }
            }
            let next = (
                repo_loaded.targets().signed.version.get() + 1,
                repo_loaded.snapshot().signed.version.get() + 1,
                repo_loaded.timestamp().signed.version.get() + 1,
            );
            (
                RepositoryEditor::from_repo(&root_path, repo_loaded).await?,
                next,
            )
        }
        None => (RepositoryEditor::new(&root_path).await?, (1, 1, 1)),
    };
    let nz = |v: u64| NonZeroU64::new(v).ok_or_else(|| Error("version".into()));
    editor
        .targets_version(nz(next.0)?)?
        .targets_expires(days(expiry.targets_days)?)?
        .snapshot_version(nz(next.1)?)
        .snapshot_expires(days(expiry.snapshot_days)?)
        .timestamp_version(nz(next.2)?)
        .timestamp_expires(days(expiry.timestamp_days)?);
    let files = files_under(staged)?;
    let mut names = Vec::new();
    for (rel, path) in &files {
        let name = TargetName::new(target_name(&manifest.version, rel))?;
        let (_, target) = RepositoryEditor::build_target(path).await?;
        editor.add_target(name.clone(), target)?;
        names.push((name, path.clone()));
    }
    let signed = editor.sign(&keys.sources()).await?;
    // Targets first: content-addressed, never replaced, invisible until the
    // timestamp names a snapshot that names them.
    for (name, path) in &names {
        signed
            .copy_target(path, &targets_dir, PathExists::Skip, Some(name))
            .await?;
    }
    let tmp = m.join(format!(".publish-{}", std::process::id()));
    if tmp.exists() {
        std::fs::remove_dir_all(&tmp)?;
    }
    std::fs::create_dir_all(&tmp)?;
    signed.write(&tmp).await?;
    for e in std::fs::read_dir(&tmp)? {
        let e = e?;
        let name = e.file_name().to_string_lossy().into_owned();
        if name == "timestamp.json" {
            continue;
        }
        let dest = m.join(&name);
        if dest.exists() {
            if std::fs::read(&dest)? != std::fs::read(e.path())? {
                std::fs::remove_dir_all(&tmp)?;
                return err(format!("{name} already exists with other content"));
            }
            continue;
        }
        std::fs::rename(e.path(), &dest)?;
    }
    std::fs::File::open(&m)?.sync_all()?;
    std::fs::rename(tmp.join("timestamp.json"), m.join("timestamp.json"))?;
    std::fs::File::open(&m)?.sync_all()?;
    std::fs::remove_dir_all(&tmp)?;
    Ok(Published {
        manifest,
        targets_version: next.0,
        timestamp_version: next.2,
    })
}

/// Re-sign `snapshot` and `timestamp` with fresh expiry and nothing else
/// changed: what keeps a channel alive between releases.
pub async fn refresh(repo: &Path, channel: Channel, keys: &KeySet, expiry: Expiry) -> Result<u64> {
    let m = metadata_dir(repo, channel);
    let first = first_root(repo, channel)?;
    let loaded = load_dir(repo, channel, &first, true).await?;
    let next = (
        loaded.targets().signed.version.get() + 1,
        loaded.snapshot().signed.version.get() + 1,
        loaded.timestamp().signed.version.get() + 1,
    );
    let root_path = latest_root(&m)?.1;
    let mut editor = RepositoryEditor::from_repo(&root_path, loaded).await?;
    let nz = |v: u64| NonZeroU64::new(v).ok_or_else(|| Error("version".into()));
    editor
        .targets_version(nz(next.0)?)?
        .targets_expires(days(expiry.targets_days)?)?
        .snapshot_version(nz(next.1)?)
        .snapshot_expires(days(expiry.snapshot_days)?)
        .timestamp_version(nz(next.2)?)
        .timestamp_expires(days(expiry.timestamp_days)?);
    let signed = editor.sign(&keys.sources()).await?;
    let tmp = m.join(format!(".refresh-{}", std::process::id()));
    std::fs::create_dir_all(&tmp)?;
    signed.write(&tmp).await?;
    for e in std::fs::read_dir(&tmp)? {
        let e = e?;
        let name = e.file_name().to_string_lossy().into_owned();
        if name != "timestamp.json" && !m.join(&name).exists() {
            std::fs::rename(e.path(), m.join(&name))?;
        }
    }
    std::fs::rename(tmp.join("timestamp.json"), m.join("timestamp.json"))?;
    std::fs::File::open(&m)?.sync_all()?;
    std::fs::remove_dir_all(&tmp)?;
    Ok(next.2)
}

/// Rotate a channel's keys: the new root (from
/// [`crate::keys::rotate_root`]) is added, and `targets`, `snapshot` and
/// `timestamp` are signed again by `new_keys`, so that a machine following the
/// root chain finds every role signed by the keys the new root names. The
/// channel is read under its current chain, before the new root exists.
pub async fn rotate_keys(
    repo: &Path,
    channel: Channel,
    new_keys: &KeySet,
    new_root: &[u8],
    expiry: Expiry,
) -> Result<u64> {
    let m = metadata_dir(repo, channel);
    let first = first_root(repo, channel)?;
    let loaded = load_dir(repo, channel, &first, true).await?;
    let latest = latest_root(&m)?.0;
    if root_version(new_root)? != latest + 1 {
        return err(format!("the next root is version {}", latest + 1));
    }
    let next = (
        loaded.targets().signed.version.get() + 1,
        loaded.snapshot().signed.version.get() + 1,
        loaded.timestamp().signed.version.get() + 1,
    );
    let tmp = m.join(format!(".rotate-{}", std::process::id()));
    if tmp.exists() {
        std::fs::remove_dir_all(&tmp)?;
    }
    std::fs::create_dir_all(&tmp)?;
    let root_path = tmp.join("next.root.json");
    std::fs::write(&root_path, new_root)?;
    let mut editor = RepositoryEditor::from_repo(&root_path, loaded).await?;
    let nz = |v: u64| NonZeroU64::new(v).ok_or_else(|| Error("version".into()));
    editor
        .targets_version(nz(next.0)?)?
        .targets_expires(days(expiry.targets_days)?)?
        .snapshot_version(nz(next.1)?)
        .snapshot_expires(days(expiry.snapshot_days)?)
        .timestamp_version(nz(next.2)?)
        .timestamp_expires(days(expiry.timestamp_days)?);
    let signed = editor.sign(&new_keys.sources()).await?;
    let out = tmp.join("out");
    std::fs::create_dir_all(&out)?;
    signed.write(&out).await?;
    for e in std::fs::read_dir(&out)? {
        let e = e?;
        let name = e.file_name().to_string_lossy().into_owned();
        if name == "timestamp.json" || name.ends_with(".root.json") || name == "root.json" {
            continue;
        }
        if !m.join(&name).exists() {
            std::fs::rename(e.path(), m.join(&name))?;
        }
    }
    add_root(repo, channel, new_root)?;
    std::fs::rename(out.join("timestamp.json"), m.join("timestamp.json"))?;
    std::fs::File::open(&m)?.sync_all()?;
    std::fs::remove_dir_all(&tmp)?;
    Ok(latest + 1)
}
