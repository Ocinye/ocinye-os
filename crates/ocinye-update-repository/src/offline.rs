//! One release in one file, for a machine without network (`.ocu`).
//!
//! The bundle is a tar of a channel's signed metadata and of the targets of
//! one release, laid out as the repository lays them out. The machine checks
//! it exactly as it checks the online repository — the same TUF client,
//! against the root it already trusts. Where the file came from proves
//! nothing.

use crate::archive::{extract, Extracted, Limits};
use crate::verify::{channel_dir, consistent_path, load_dir, target_info};
use crate::{err, target_name, Error, Result};
use ocinye_update_contracts::{Channel, ReleaseVersion, MANIFEST_TARGET};
use std::path::Path;

/// The marker file at the top of a bundle.
pub const MARKER: &str = "OCINYE-OFFLINE-UPDATE";

/// Pack `version` of `channel` into `out`.
pub async fn pack(
    repo: &Path,
    channel: Channel,
    version: &ReleaseVersion,
    out: &Path,
) -> Result<()> {
    let first = crate::publish::first_root(repo, channel)?;
    let loaded = load_dir(repo, channel, &first, false).await?;
    let manifest =
        crate::verify::read_manifest(&loaded, &target_name(version, MANIFEST_TARGET)).await?;
    let dir = channel_dir(repo, channel);
    let file = std::fs::File::create(out)?;
    let mut b = tar::Builder::new(std::io::BufWriter::new(file));
    b.mode(tar::HeaderMode::Deterministic);
    let marker = serde_json::json!({
        "schema": 1,
        "channel": channel.as_str(),
        "version": version.to_string(),
    })
    .to_string();
    let mut h = tar::Header::new_gnu();
    h.set_size(marker.len() as u64);
    h.set_mode(0o644);
    h.set_cksum();
    b.append_data(&mut h, MARKER, marker.as_bytes())?;
    for e in std::fs::read_dir(dir.join("metadata"))? {
        let e = e?;
        let name = e.file_name().to_string_lossy().into_owned();
        if e.file_type()?.is_file() && name.ends_with(".json") {
            b.append_path_with_name(e.path(), format!("metadata/{name}"))?;
        }
    }
    let mut names = vec![MANIFEST_TARGET.to_owned()];
    names.extend(manifest.artifacts.iter().map(|a| a.target.clone()));
    for t in names {
        let full = target_name(version, &t);
        let (_, sha) =
            target_info(&loaded, &full).ok_or_else(|| Error(format!("{full} not signed")))?;
        let rel = consistent_path(&full, &sha);
        b.append_path_with_name(dir.join("targets").join(&rel), format!("targets/{rel}"))?;
    }
    b.into_inner()?
        .into_inner()
        .map_err(|e| Error(e.to_string()))?
        .sync_all()?;
    Ok(())
}

/// What a bundle says it holds, before anything in it is trusted.
#[derive(Debug, Clone)]
pub struct Unpacked {
    /// The channel it claims.
    pub channel: Channel,
    /// The release it claims.
    pub version: ReleaseVersion,
    /// What was extracted.
    pub extracted: Extracted,
}

/// Extract a bundle into `dest` (which must not exist). Nothing is trusted
/// yet: the caller loads `dest/metadata` and `dest/targets` with its own root.
pub fn unpack(bundle: &Path, dest: &Path) -> Result<Unpacked> {
    let file = std::fs::File::open(bundle)?;
    let extracted = extract(std::io::BufReader::new(file), dest, Limits::OFFLINE)?;
    let marker: serde_json::Value = serde_json::from_slice(&std::fs::read(dest.join(MARKER))?)
        .map_err(|_| Error("the bundle marker is not JSON".into()))?;
    if marker.get("schema").and_then(serde_json::Value::as_u64) != Some(1) {
        return err("unknown offline bundle schema");
    }
    let channel = marker
        .get("channel")
        .and_then(serde_json::Value::as_str)
        .and_then(Channel::parse)
        .ok_or_else(|| Error("the bundle names no channel".into()))?;
    let version = marker
        .get("version")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| Error("the bundle names no release".into()))
        .and_then(|v| ReleaseVersion::parse(v).map_err(|e| Error(e.to_string())))?;
    for f in &extracted.files {
        if f != MARKER && !f.starts_with("metadata/") && !f.starts_with("targets/") {
            return err(format!("{f} has no place in an offline bundle"));
        }
    }
    Ok(Unpacked {
        channel,
        version,
        extracted,
    })
}
