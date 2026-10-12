//! The native layer: the binaries, units and interface of a release.
//!
//! ```text
//! /usr/lib/ocinye/releases/<version>/release.json
//!                                    bin/ocinye-…
//!                                    units/ocinye-….service
//! /usr/lib/ocinye/current  -> releases/<version>
//! /usr/lib/ocinye/previous -> releases/<version>
//! ```
//!
//! A release directory is never changed once complete: it is extracted under
//! `<version>.staging` and renamed. The pointers are symbolic links replaced
//! by `rename(2)`, so at every instant each points at a whole release.

use crate::paths::Paths;
use crate::store::write_atomic;
use ocinye_update_contracts::{Channel, ReleaseNumber, ReleaseVersion, UpdateReason};
use ocinye_update_repository::archive::{extract, Limits};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// `release.json` at the top of a native release.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeInfo {
    /// Always 1.
    pub schema: u32,
    /// Identity.
    pub version: ReleaseVersion,
    /// Number.
    pub release_number: ReleaseNumber,
    /// Channel.
    pub channel: Channel,
    /// `amd64` or `arm64`.
    pub arch: String,
}

/// The binaries every native release carries.
pub const REQUIRED_BINARIES: [&str; 3] = [
    "ocinye-system-service",
    "ocinye-session-service",
    "ocinye-update-engine",
];

/// Read a release's `release.json`.
#[must_use]
pub fn info(dir: &Path) -> Option<NativeInfo> {
    let bytes = std::fs::read(dir.join("release.json")).ok()?;
    let i: NativeInfo = serde_json::from_slice(&bytes).ok()?;
    (i.schema == 1).then_some(i)
}

/// The version a pointer names, if it names a whole release.
#[must_use]
pub fn pointer(path: &Path) -> Option<String> {
    let target = std::fs::read_link(path).ok()?;
    let name = target.strip_prefix("releases").ok()?.to_str()?.to_owned();
    ReleaseVersion::parse(&name).ok().map(|_| name)
}

/// Extract `tar` as release `version`, unless it is already there whole.
pub fn stage(paths: &Paths, version: &ReleaseVersion, tar: &Path) -> Result<(), UpdateReason> {
    let dest = paths.native_release(&version.to_string());
    if info(&dest).is_some_and(|i| &i.version == version) {
        return Ok(());
    }
    if dest.exists() {
        // An incomplete directory under the final name is never used.
        std::fs::remove_dir_all(&dest).map_err(|_| UpdateReason::ArchiveUnsafe)?;
    }
    let staging = dest.with_extension("staging");
    if staging.exists() {
        std::fs::remove_dir_all(&staging).map_err(|_| UpdateReason::ArchiveUnsafe)?;
    }
    let file = std::fs::File::open(tar).map_err(|_| UpdateReason::ArtifactMismatch)?;
    extract(std::io::BufReader::new(file), &staging, Limits::NATIVE)
        .map_err(|_| UpdateReason::ArchiveUnsafe)?;
    let ok = info(&staging).is_some_and(|i| &i.version == version)
        && REQUIRED_BINARIES
            .iter()
            .all(|b| staging.join("bin").join(b).is_file());
    if !ok {
        let _ = std::fs::remove_dir_all(&staging);
        return Err(UpdateReason::ArchiveUnsafe);
    }
    std::fs::rename(&staging, &dest).map_err(|_| UpdateReason::ArchiveUnsafe)?;
    sync_dir(&paths.native_root.join("releases"));
    Ok(())
}

fn sync_dir(dir: &Path) {
    if let Ok(f) = std::fs::File::open(dir) {
        let _ = f.sync_all();
    }
}

/// Point `pointer` at `releases/<version>`, atomically.
pub fn set_pointer(paths: &Paths, pointer_path: &Path, version: &str) -> std::io::Result<()> {
    let tmp = paths.native_root.join(format!(
        ".{}.next",
        pointer_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("ptr")
    ));
    let _ = std::fs::remove_file(&tmp);
    std::os::unix::fs::symlink(PathBuf::from("releases").join(version), &tmp)?;
    std::fs::rename(&tmp, pointer_path)?;
    sync_dir(&paths.native_root);
    Ok(())
}

/// Remove a pointer.
pub fn clear_pointer(paths: &Paths, pointer_path: &Path) {
    let _ = std::fs::remove_file(pointer_path);
    sync_dir(&paths.native_root);
}

/// Put the release's units in place, each file atomically. Only
/// `ocinye-*` unit files are taken.
pub fn install_units(paths: &Paths, version: &str) -> Result<Vec<String>, UpdateReason> {
    let dir = paths.native_release(version).join("units");
    let mut installed = Vec::new();
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return Ok(installed);
    };
    for e in entries.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        let valid = name.starts_with("ocinye-")
            && [".service", ".path", ".timer", ".socket"]
                .iter()
                .any(|s| name.ends_with(s))
            && name
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'@' | b'_'));
        if !valid || !e.file_type().is_ok_and(|t| t.is_file()) {
            continue;
        }
        let bytes = std::fs::read(e.path()).map_err(|_| UpdateReason::ArchiveUnsafe)?;
        write_atomic(&paths.units.join(&name), &bytes)
            .map_err(|_| UpdateReason::HealthGateFailed)?;
        installed.push(name);
    }
    installed.sort();
    Ok(installed)
}

/// Remove every native release except `keep`; never the ones the pointers
/// name.
pub fn collect(paths: &Paths, keep: &[String]) {
    let mut keep: Vec<String> = keep.to_vec();
    keep.extend(pointer(&paths.current()));
    keep.extend(pointer(&paths.previous()));
    let Ok(entries) = std::fs::read_dir(paths.native_root.join("releases")) else {
        return;
    };
    for e in entries.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        if !keep.contains(&name) {
            let _ = std::fs::remove_dir_all(e.path());
        }
    }
}
