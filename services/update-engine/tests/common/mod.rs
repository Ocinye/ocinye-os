//! A bench for the engine: a real signed repository on disk, served over
//! HTTP by a small server that can drop a transfer or serve a corrupt one,
//! and an in-memory host whose health, Instance and images a test controls.

#![allow(dead_code)]

use ocinye_update_contracts::manifest::{
    Arch, Artifact, ArtifactRole, Build, ImageRef, MigrationPlan, MigrationPolicy, Notes,
    SpaceRequirement, UpdateManifest, BASE_OS, PRODUCT, SCHEMA,
};
use ocinye_update_contracts::{
    Channel, ReleaseNumber, ReleaseSigning, ReleaseVersion, UpdateReason,
};
use ocinye_update_engine::host::{AptOp, Host};
use ocinye_update_engine::native::NativeInfo;
use ocinye_update_engine::paths::Paths;
use ocinye_update_engine::ImageFacts;
use ocinye_update_repository::keys::{create_root, KeySet};
use ocinye_update_repository::publish::{init_channel, publish_release, Expiry};
use ocinye_update_repository::sha256_file;
use std::collections::BTreeSet;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

pub const CH: Channel = Channel::Development;

pub fn facts() -> ImageFacts {
    ImageFacts {
        default_channel: CH,
        signing: ReleaseSigning::DevOnly,
        arch: Arch::Amd64,
        system_protocol: 1,
    }
}

pub fn rt() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
}

// ── the HTTP server ─────────────────────────────────────────────────────

#[derive(Default)]
pub struct ServerControl {
    /// Drop the connection after this many body bytes, once, for a path
    /// ending with the given suffix.
    pub drop_once: Mutex<Option<(String, u64)>>,
    /// Answer 503 to everything while set.
    pub down: AtomicBool,
    /// Requests seen, with their Range header if any.
    pub seen: Mutex<Vec<(String, Option<String>)>>,
    pub served_bytes: AtomicU64,
}

pub struct Server {
    pub base: String,
    pub control: Arc<ServerControl>,
}

/// Serve `root` on 127.0.0.1.
pub fn serve(root: PathBuf) -> Server {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let control = Arc::new(ServerControl::default());
    let c = Arc::clone(&control);
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let root = root.clone();
            let c = Arc::clone(&c);
            std::thread::spawn(move || answer(&root, &c, stream));
        }
    });
    Server {
        base: format!("http://127.0.0.1:{port}/"),
        control,
    }
}

fn answer(root: &Path, c: &ServerControl, mut stream: std::net::TcpStream) {
    let mut reader = BufReader::new(stream.try_clone().unwrap());
    let mut line = String::new();
    if reader.read_line(&mut line).is_err() {
        return;
    }
    let path = line.split_whitespace().nth(1).unwrap_or("/").to_owned();
    let mut range = None;
    loop {
        let mut h = String::new();
        if reader.read_line(&mut h).is_err() || h == "\r\n" || h.is_empty() {
            break;
        }
        if let Some(v) = h
            .strip_prefix("range: ")
            .or_else(|| h.strip_prefix("Range: "))
        {
            range = Some(v.trim().to_owned());
        }
    }
    c.seen.lock().unwrap().push((path.clone(), range.clone()));
    if c.down.load(Ordering::SeqCst) {
        let _ = stream.write_all(
            b"HTTP/1.1 503 Service Unavailable\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
        );
        return;
    }
    let rel = percent_decode(path.trim_start_matches('/'));
    let file = root.join(&rel);
    let Ok(bytes) = std::fs::read(&file) else {
        let _ = stream
            .write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
        return;
    };
    let start = range
        .as_deref()
        .and_then(|r| r.strip_prefix("bytes="))
        .and_then(|r| r.trim_end_matches('-').parse::<usize>().ok())
        .unwrap_or(0)
        .min(bytes.len());
    let body = &bytes[start..];
    let head = if start > 0 {
        format!(
            "HTTP/1.1 206 Partial Content\r\nContent-Length: {}\r\nContent-Range: bytes {}-{}/{}\r\nConnection: close\r\n\r\n",
            body.len(), start, bytes.len().saturating_sub(1), bytes.len()
        )
    } else {
        format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        )
    };
    let _ = stream.write_all(head.as_bytes());
    let mut limit = body.len();
    {
        let mut d = c.drop_once.lock().unwrap();
        if let Some((suffix, after)) = d.clone() {
            if rel.ends_with(&suffix) {
                limit = (after as usize).min(body.len());
                *d = None;
            }
        }
    }
    let _ = stream.write_all(&body[..limit]);
    c.served_bytes.fetch_add(limit as u64, Ordering::SeqCst);
    let _ = stream.flush();
}

fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let Ok(v) =
                u8::from_str_radix(std::str::from_utf8(&b[i + 1..i + 3]).unwrap_or(""), 16)
            {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

// ── the host ────────────────────────────────────────────────────────────

#[derive(Debug, Default)]
pub struct FakeState {
    pub calls: Vec<String>,
    pub images: BTreeSet<String>,
    /// Unhealthy whenever `current` points at this version.
    pub unhealthy_version: Option<String>,
    pub instance_upgrade_fails: bool,
    pub free: Option<u64>,
    pub boot_id: String,
    pub system_unhealthy: bool,
}

#[derive(Clone)]
pub struct FakeHost {
    pub state: Arc<Mutex<FakeState>>,
    pub paths: Paths,
}

impl FakeHost {
    pub fn new(paths: Paths) -> Self {
        let state = FakeState {
            boot_id: "boot-1".into(),
            ..FakeState::default()
        };
        Self {
            state: Arc::new(Mutex::new(state)),
            paths,
        }
    }
    pub fn calls(&self) -> Vec<String> {
        self.state.lock().unwrap().calls.clone()
    }
    fn log(&self, s: impl Into<String>) {
        self.state.lock().unwrap().calls.push(s.into());
    }
    fn current_version(&self) -> Option<String> {
        ocinye_update_engine::native::pointer(&self.paths.current())
    }
}

impl Host for FakeHost {
    fn now(&self) -> String {
        "2026-10-12T12:00:00Z".into()
    }
    fn boot_id(&self) -> String {
        self.state.lock().unwrap().boot_id.clone()
    }
    fn free_bytes(&self, _: &Path) -> u64 {
        self.state.lock().unwrap().free.unwrap_or(1 << 40)
    }
    fn system_healthy(&self) -> Result<(), UpdateReason> {
        let s = self.state.lock().unwrap();
        if s.system_unhealthy {
            return Err(UpdateReason::SystemUnhealthy);
        }
        let bad = s.unhealthy_version.clone();
        drop(s);
        if bad.is_some() && bad == self.current_version() {
            return Err(UpdateReason::SystemUnhealthy);
        }
        Ok(())
    }
    fn instance_healthy(&self) -> Result<(), UpdateReason> {
        Ok(())
    }
    fn load_image(&self, archive: &Path) -> Result<(), UpdateReason> {
        let id =
            std::fs::read_to_string(archive).map_err(|_| UpdateReason::InstanceUpgradeFailed)?;
        self.log(format!("load_image {}", id.trim()));
        self.state
            .lock()
            .unwrap()
            .images
            .insert(id.trim().to_owned());
        Ok(())
    }
    fn image_present(&self, image_id: &str) -> bool {
        self.state.lock().unwrap().images.contains(image_id)
    }
    fn instance_upgrade(&self, bundle: &Path) -> Result<(), UpdateReason> {
        self.log("instance_upgrade");
        if self.state.lock().unwrap().instance_upgrade_fails {
            return Err(UpdateReason::InstanceUpgradeFailed);
        }
        let id = std::fs::read_to_string(bundle.join("RELEASE")).unwrap();
        let prev = std::fs::read_to_string(&self.paths.release_env).unwrap_or_default();
        std::fs::write(self.paths.release_env.with_extension("previous"), prev).unwrap();
        std::fs::write(
            &self.paths.release_env,
            format!("OCINYE_RELEASE_SHA={}\n", id.trim()),
        )
        .unwrap();
        Ok(())
    }
    fn instance_rollback(&self) -> Result<(), UpdateReason> {
        self.log("instance_rollback");
        let prev = std::fs::read_to_string(self.paths.release_env.with_extension("previous"))
            .unwrap_or_default();
        std::fs::write(&self.paths.release_env, prev).unwrap();
        Ok(())
    }
    fn reload_units(&self) -> Result<(), UpdateReason> {
        self.log("reload_units");
        Ok(())
    }
    fn restart_unit(&self, unit: &str) -> Result<(), UpdateReason> {
        self.log(format!("restart {unit}"));
        Ok(())
    }
    fn unit_active(&self, unit: &str) -> bool {
        unit != "ocinye-display.service"
    }
    fn restart_engine_later(&self) {
        self.log("restart_engine");
    }
    fn apt(&self, op: AptOp) -> Result<String, UpdateReason> {
        self.log(format!("apt {op:?}"));
        Ok(String::new())
    }
}

// ── releases ────────────────────────────────────────────────────────────

/// A native layer tar for `version`, its engine binary saying `engine`.
pub fn native_tar(path: &Path, version: &ReleaseVersion, number: u64, engine: &str) {
    let info = NativeInfo {
        schema: 1,
        version: version.clone(),
        release_number: ReleaseNumber(number),
        channel: CH,
        arch: "amd64".into(),
    };
    let mut b = tar::Builder::new(Vec::new());
    let mut add = |name: &str, data: &[u8], mode: u32| {
        let mut h = tar::Header::new_gnu();
        h.set_size(data.len() as u64);
        h.set_mode(mode);
        h.set_cksum();
        b.append_data(&mut h, name, data).unwrap();
    };
    add("release.json", &serde_json::to_vec(&info).unwrap(), 0o644);
    for bin in ["ocinye-system-service", "ocinye-session-service"] {
        add(
            &format!("bin/{bin}"),
            format!("{bin} {version}").as_bytes(),
            0o755,
        );
    }
    add("bin/ocinye-update-engine", engine.as_bytes(), 0o755);
    add(
        "units/ocinye-system.service",
        format!("[Unit]\nDescription=system {version}\n").as_bytes(),
        0o644,
    );
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, b.into_inner().unwrap()).unwrap();
}

pub struct Release {
    pub manifest: UpdateManifest,
    pub staged: PathBuf,
}

/// Stage a release with a real native tar and a fake D011 bundle.
#[allow(clippy::too_many_arguments)] // a test fixture: every argument is a release property
pub fn stage(
    dir: &Path,
    id: &str,
    rev: u32,
    number: u64,
    minimum: u64,
    engine: &str,
    reboot: bool,
    policy: MigrationPolicy,
) -> Release {
    let version = ReleaseVersion::new(id, rev).unwrap();
    let image_id = format!(
        "sha256:{}",
        format!("{id}{rev}")
            .repeat(8)
            .chars()
            .take(64)
            .collect::<String>()
            .replace(|c: char| !c.is_ascii_hexdigit(), "a")
    );
    let files: Vec<(&str, ArtifactRole, Vec<u8>)> = vec![
        (
            "bundle/MANIFEST.json",
            ArtifactRole::ReleaseBundle,
            format!("{{\"id\":\"{id}\"}}").into_bytes(),
        ),
        (
            "bundle/RELEASE",
            ArtifactRole::ReleaseBundle,
            format!("{id}\n").into_bytes(),
        ),
        (
            "bundle/SHA256SUMS",
            ArtifactRole::ReleaseBundle,
            b"sums\n".to_vec(),
        ),
        (
            "bundle/install/ocinye",
            ArtifactRole::ReleaseBundle,
            b"#!/bin/sh\n".to_vec(),
        ),
        (
            "bundle/images/ocinye-core-server.tar",
            ArtifactRole::ReleaseBundle,
            image_id.clone().into_bytes(),
        ),
    ];
    let mut artifacts = Vec::new();
    for (t, role, bytes) in &files {
        let p = dir.join(t);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(&p, bytes).unwrap();
        let (sha256, n) = sha256_file(&p).unwrap();
        artifacts.push(Artifact {
            target: (*t).into(),
            role: *role,
            sha256,
            bytes: n,
        });
    }
    let native = dir.join("native/native-amd64.tar");
    native_tar(&native, &version, number, engine);
    let (sha256, n) = sha256_file(&native).unwrap();
    artifacts.push(Artifact {
        target: "native/native-amd64.tar".into(),
        role: ArtifactRole::Native,
        sha256,
        bytes: n,
    });
    let total = artifacts.iter().map(|a| a.bytes).sum();
    let manifest = UpdateManifest {
        schema: SCHEMA,
        product: PRODUCT.into(),
        version,
        release_number: ReleaseNumber(number),
        channel: CH,
        created_at: "2026-10-12T10:00:00Z".into(),
        arch: Arch::Amd64,
        commit: format!("{id}{}", "0".repeat(28)),
        build: Build::Proof,
        base_os: BASE_OS.into(),
        minimum_source: ReleaseNumber(minimum),
        system_protocol: 1,
        release_manifest_sha256: artifacts[0].sha256.clone(),
        artifacts,
        images: vec![ImageRef {
            name: "core-server".into(),
            image_id,
        }],
        migrations: MigrationPlan {
            schema_version: 64,
            policy,
        },
        space: SpaceRequirement {
            download_bytes: total,
            install_bytes: total * 2,
        },
        reboot_required: reboot,
        security: false,
        notes: Notes {
            pt: "Teste.".into(),
            en: "Test.".into(),
            fr: "Essai.".into(),
        },
        synthetic: None,
    };
    std::fs::write(
        dir.join("manifest.json"),
        serde_json::to_vec(&manifest).unwrap(),
    )
    .unwrap();
    Release {
        manifest,
        staged: dir.to_owned(),
    }
}

// ── the bench ───────────────────────────────────────────────────────────

pub struct Bench {
    pub tmp: tempfile::TempDir,
    pub paths: Paths,
    pub repo: PathBuf,
    pub keys: KeySet,
    pub server: Server,
    pub host: FakeHost,
    pub n: ReleaseVersion,
}

pub const N_ID: &str = "aaaaaaaaaaaa";
pub const N1_ID: &str = "bbbbbbbbbbbb";

fn in_a_year() -> jiff::Timestamp {
    jiff::Timestamp::now()
        .checked_add(jiff::SignedDuration::from_hours(24 * 365))
        .unwrap()
}

impl Bench {
    /// A machine on release N (number 1), with or without an Instance, and a
    /// channel that has N.
    pub fn new(instance: bool) -> Self {
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::under(&tmp.path().join("root"));
        let keys = KeySet::generate(&tmp.path().join("keys")).unwrap();
        let rt = rt();
        let root = rt.block_on(create_root(&keys, 1, in_a_year())).unwrap();
        let repo = tmp.path().join("repo");
        init_channel(&repo, CH, &root).unwrap();
        std::fs::create_dir_all(paths.trust.join(CH.as_str())).unwrap();
        std::fs::write(paths.trust.join(CH.as_str()).join("root.json"), &root).unwrap();
        // The machine: native release N current, first start done.
        let n = ReleaseVersion::new(N_ID, 1).unwrap();
        let tar = tmp.path().join("n-native.tar");
        native_tar(&tar, &n, 1, "engine N");
        ocinye_update_engine::native::stage(&paths, &n, &tar).unwrap();
        ocinye_update_engine::native::set_pointer(&paths, &paths.current(), &n.to_string())
            .unwrap();
        std::fs::create_dir_all(paths.firstboot_state.parent().unwrap()).unwrap();
        std::fs::write(&paths.firstboot_state, "{}").unwrap();
        std::fs::create_dir_all(paths.carried_release.join(N_ID)).unwrap();
        std::fs::create_dir_all(paths.units.clone()).unwrap();
        if instance {
            std::fs::create_dir_all(paths.release_env.parent().unwrap()).unwrap();
            std::fs::write(&paths.release_env, format!("OCINYE_RELEASE_SHA={N_ID}\n")).unwrap();
        }
        let server = serve(repo.clone());
        let host = FakeHost::new(paths.clone());
        let bench = Self {
            tmp,
            paths,
            repo,
            keys,
            server,
            host,
            n,
        };
        bench.configure_repository();
        bench
    }

    pub fn configure_repository(&self) {
        let config = ocinye_update_contracts::UpdateConfig {
            schema: 1,
            channel: CH,
            repository: Some(ocinye_update_contracts::RepositoryEndpoint {
                metadata_url: format!("{}development/metadata/", self.server.base),
                targets_url: format!("{}development/targets/", self.server.base),
            }),
            auto_check: false,
            auto_download: false,
        };
        std::fs::create_dir_all(self.paths.config.parent().unwrap()).unwrap();
        std::fs::write(&self.paths.config, serde_json::to_vec(&config).unwrap()).unwrap();
    }

    pub fn publish(
        &self,
        id: &str,
        rev: u32,
        number: u64,
        engine: &str,
        reboot: bool,
        policy: MigrationPolicy,
    ) -> Release {
        let staged = self.tmp.path().join(format!("stage-{id}-{rev}"));
        let r = stage(&staged, id, rev, number, 1, engine, reboot, policy);
        rt().block_on(publish_release(
            &self.repo,
            CH,
            &self.keys,
            &staged,
            Expiry::for_channel(CH),
        ))
        .unwrap();
        r
    }

    pub fn publish_n1(&self) -> Release {
        self.publish(N1_ID, 1, 2, "engine N+1", false, MigrationPolicy::Expand)
    }
}
