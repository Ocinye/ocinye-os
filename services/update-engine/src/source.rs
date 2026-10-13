//! Where releases come from: the configured repository or an imported offline
//! bundle. Both are read by the same TUF client against the root this machine
//! trusts, with the same metadata cache — so an offline bundle cannot replay
//! metadata older than what the machine saw online, and the reverse.

use crate::paths::Paths;
use futures::StreamExt;
use ocinye_update_contracts::config::RepositoryEndpoint;
use ocinye_update_contracts::{
    Channel, ReleaseVersion, UpdateManifest, UpdateReason, MANIFEST_TARGET,
};
use ocinye_update_repository::transport::HttpTransport;
use ocinye_update_repository::verify::{
    consistent_path, dir_url, read_manifest, releases, target_info,
};
use ocinye_update_repository::{sha256_file, target_name};

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Duration;
use tough::{Repository, RepositoryLoader};
use url::Url;

/// Where the release comes from.
#[derive(Debug, Clone)]
pub enum Origin {
    /// The configured repository.
    Repository(RepositoryEndpoint),
    /// An unpacked offline bundle.
    Offline(PathBuf),
}

/// Where one artifact goes: its partial file, then its verified place.
pub struct Destination<'a> {
    /// Partial downloads, named by SHA-256.
    pub partial_dir: &'a Path,
    /// The final, verified file.
    pub verified: &'a Path,
}

/// An opened, verified source.
pub struct Source {
    repo: Repository,
    origin: Origin,
    transport: HttpTransport,
    targets_url: Url,
}

/// Map a TUF failure to the reason a person can act on.
fn reason_of(e: &tough::error::Error) -> UpdateReason {
    use tough::error::Error as E;
    match e {
        E::ExpiredMetadata { .. } => UpdateReason::MetadataExpired,
        E::OlderMetadata { .. }
        | E::OlderSnapshotInTimestamp { .. }
        | E::SnapshotRoleRollback { .. } => UpdateReason::MetadataRollback,
        E::Transport { .. } => UpdateReason::RepositoryUnreachable,
        _ => UpdateReason::MetadataUntrusted,
    }
}

impl Source {
    /// Open and verify the metadata of `channel` from `origin`.
    pub fn open(
        rt: &tokio::runtime::Runtime,
        paths: &Paths,
        channel: Channel,
        origin: Origin,
    ) -> Result<Self, UpdateReason> {
        let root = std::fs::read(paths.trust.join(channel.as_str()).join("root.json"))
            .map_err(|_| UpdateReason::MetadataUntrusted)?;
        let (metadata_url, targets_url) = match &origin {
            Origin::Repository(r) => (
                Url::parse(&r.metadata_url).map_err(|_| UpdateReason::RepositoryNotConfigured)?,
                Url::parse(&r.targets_url).map_err(|_| UpdateReason::RepositoryNotConfigured)?,
            ),
            Origin::Offline(dir) => (
                dir_url(&dir.join("metadata")).map_err(|_| UpdateReason::BundleInvalid)?,
                dir_url(&dir.join("targets")).map_err(|_| UpdateReason::BundleInvalid)?,
            ),
        };
        let datastore = paths.tuf(channel);
        std::fs::create_dir_all(&datastore).map_err(|_| UpdateReason::JournalInvalid)?;
        let transport = HttpTransport::new().map_err(|_| UpdateReason::RepositoryUnreachable)?;
        let loader = RepositoryLoader::new(&root, metadata_url, targets_url.clone())
            .transport(transport.clone())
            .datastore(&datastore);
        let repo = rt.block_on(loader.load()).map_err(|e| reason_of(&e))?;
        Ok(Self {
            repo,
            origin,
            transport,
            targets_url,
        })
    }

    /// Every release on the channel, newest first.
    pub fn releases(
        &self,
        rt: &tokio::runtime::Runtime,
    ) -> Result<Vec<UpdateManifest>, UpdateReason> {
        rt.block_on(releases(&self.repo))
            .map(|v| v.into_iter().map(|r| r.manifest).collect())
            .map_err(|_| UpdateReason::ManifestMismatch)
    }

    /// One release's manifest.
    pub fn manifest(
        &self,
        rt: &tokio::runtime::Runtime,
        version: &ReleaseVersion,
    ) -> Result<UpdateManifest, UpdateReason> {
        rt.block_on(read_manifest(
            &self.repo,
            &target_name(version, MANIFEST_TARGET),
        ))
        .map_err(|_| UpdateReason::ManifestMismatch)
    }

    /// Fetch one artifact into `verified` (its final place), resuming from
    /// `partial_dir/<sha256>`. Only a file whose length and SHA-256 are the
    /// signed ones reaches `verified`.
    pub fn fetch(
        &self,
        rt: &tokio::runtime::Runtime,
        version: &ReleaseVersion,
        target: &str,
        dest: &Destination<'_>,
        cancel: &AtomicBool,
        done: &AtomicU64,
    ) -> Result<(), UpdateReason> {
        let (partial_dir, verified) = (dest.partial_dir, dest.verified);
        let full = target_name(version, target);
        let (len, sha) = target_info(&self.repo, &full).ok_or(UpdateReason::ManifestMismatch)?;
        if verified.exists() {
            if sha256_file(verified)
                .map(|(h, n)| h == sha && n == len)
                .unwrap_or(false)
            {
                done.fetch_add(len, Ordering::Relaxed);
                return Ok(());
            }
            std::fs::remove_file(verified).map_err(|_| UpdateReason::InsufficientSpace)?;
        }
        std::fs::create_dir_all(partial_dir).map_err(|_| UpdateReason::InsufficientSpace)?;
        let partial = partial_dir.join(&sha);
        let rel = consistent_path(&full, &sha);
        match &self.origin {
            Origin::Offline(dir) => {
                copy_bounded(&dir.join("targets").join(&rel), &partial, len, cancel, done)?;
            }
            Origin::Repository(_) => {
                let url = self
                    .targets_url
                    .join(&rel)
                    .map_err(|_| UpdateReason::ManifestMismatch)?;
                let mut attempt = 0u32;
                loop {
                    match rt.block_on(download(
                        self.transport.client(),
                        &url,
                        &partial,
                        len,
                        cancel,
                        done,
                    )) {
                        Ok(()) => break,
                        Err(UpdateReason::Cancelled) => return Err(UpdateReason::Cancelled),
                        Err(UpdateReason::ArtifactMismatch) => {
                            // A corrupt resumed fragment: start this one over.
                            let _ = std::fs::remove_file(&partial);
                            return Err(UpdateReason::ArtifactMismatch);
                        }
                        Err(e) if attempt >= 3 => return Err(e),
                        Err(_) => {
                            attempt += 1;
                            std::thread::sleep(Duration::from_secs(2u64.pow(attempt)));
                        }
                    }
                }
            }
        }
        let (h, n) = sha256_file(&partial).map_err(|_| UpdateReason::ArtifactMismatch)?;
        if h != sha || n != len {
            let _ = std::fs::remove_file(&partial);
            return Err(UpdateReason::ArtifactMismatch);
        }
        if let Some(p) = verified.parent() {
            std::fs::create_dir_all(p).map_err(|_| UpdateReason::InsufficientSpace)?;
        }
        std::fs::rename(&partial, verified).map_err(|_| UpdateReason::InsufficientSpace)?;
        Ok(())
    }
}

fn copy_bounded(
    from: &Path,
    to: &Path,
    len: u64,
    cancel: &AtomicBool,
    done: &AtomicU64,
) -> Result<(), UpdateReason> {
    let mut src = std::fs::File::open(from).map_err(|_| UpdateReason::BundleInvalid)?;
    let mut dst = std::fs::File::create(to).map_err(|_| UpdateReason::InsufficientSpace)?;
    let mut buf = vec![0u8; 1 << 20];
    let mut total = 0u64;
    loop {
        if cancel.load(Ordering::Relaxed) {
            return Err(UpdateReason::Cancelled);
        }
        let n = src
            .read(&mut buf)
            .map_err(|_| UpdateReason::BundleInvalid)?;
        if n == 0 {
            break;
        }
        total += n as u64;
        if total > len {
            return Err(UpdateReason::ArtifactMismatch);
        }
        dst.write_all(&buf[..n])
            .map_err(|_| UpdateReason::InsufficientSpace)?;
        done.fetch_add(n as u64, Ordering::Relaxed);
    }
    dst.sync_all()
        .map_err(|_| UpdateReason::InsufficientSpace)?;
    Ok(())
}

/// Download `url` into `partial`, resuming from its current length. Never
/// writes more than `len` bytes.
async fn download(
    client: &reqwest::Client,
    url: &Url,
    partial: &Path,
    len: u64,
    cancel: &AtomicBool,
    done: &AtomicU64,
) -> Result<(), UpdateReason> {
    let have = std::fs::metadata(partial).map(|m| m.len()).unwrap_or(0);
    if have > len {
        let _ = std::fs::remove_file(partial);
        return Err(UpdateReason::ArtifactMismatch);
    }
    done.fetch_add(have, Ordering::Relaxed);
    if have == len {
        return Ok(());
    }
    let mut req = client.get(url.clone()).timeout(Duration::from_secs(3600));
    if have > 0 {
        req = req.header(reqwest::header::RANGE, format!("bytes={have}-"));
    }
    let resp = req
        .send()
        .await
        .map_err(|_| UpdateReason::RepositoryUnreachable)?;
    let status = resp.status();
    let resuming = status == reqwest::StatusCode::PARTIAL_CONTENT;
    if !(status.is_success()) {
        return Err(UpdateReason::RepositoryUnreachable);
    }
    let mut file = if resuming {
        std::fs::OpenOptions::new().append(true).open(partial)
    } else {
        // The server ignored the range: start over.
        done.fetch_sub(have, Ordering::Relaxed);
        std::fs::File::create(partial)
    }
    .map_err(|_| UpdateReason::InsufficientSpace)?;
    let mut written = if resuming { have } else { 0 };
    let mut stream = resp.bytes_stream();
    while let Some(chunk) = stream.next().await {
        if cancel.load(Ordering::Relaxed) {
            file.sync_all().ok();
            return Err(UpdateReason::Cancelled);
        }
        let chunk = chunk.map_err(|_| UpdateReason::DownloadInterrupted)?;
        written += chunk.len() as u64;
        if written > len {
            return Err(UpdateReason::ArtifactMismatch);
        }
        file.write_all(&chunk)
            .map_err(|_| UpdateReason::InsufficientSpace)?;
        done.fetch_add(chunk.len() as u64, Ordering::Relaxed);
    }
    file.sync_all()
        .map_err(|_| UpdateReason::InsufficientSpace)?;
    if written != len {
        return Err(UpdateReason::DownloadInterrupted);
    }
    Ok(())
}
