//! The 16 phases, as the executor runs them (D011_INSTALL_STEP_MATRIX).
//!
//! P05–P14 are `install/ocinye phase …` — the same functions `ocinye install`
//! runs — called with a fixed argv built from the sealed plan. P01–P04, P08,
//! P15 and P16 are the executor's own: transport verification, prerequisites,
//! the firewall rules Ocinye owns, verification and cleanup.
//!
//! Every phase either completes, or fails with a stable code and a redacted
//! detail. None of them reads anything the operator typed after confirmation:
//! the input is the plan whose seal was checked before the first mutation.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use ocinye_installer_contracts::canonical::sha256_hex;
use ocinye_installer_contracts::ident::InstallationId;
use ocinye_installer_contracts::manifest::{self, ReleaseManifest};
use ocinye_installer_contracts::openpgp;
use ocinye_installer_contracts::paths;
use ocinye_installer_contracts::plan::{InstallationPlan, PhaseId, SystemChange, TlsPlan};
use ocinye_installer_contracts::preflight::{self as pf, CheckStatus};
use ocinye_installer_contracts::protocol::{Event, OneTimeSecret, Operation};
use ocinye_installer_contracts::secret::{redact_line, SecretText};
use ocinye_installer_contracts::verification::{ItemStatus, VerificationReport};

use crate::exec::{self, program};
use crate::preflight as server_preflight;

/// A phase failure.
#[derive(Debug)]
pub struct Failure {
    /// Stable code (D011_FAILURE_MATRIX).
    pub code: String,
    /// May be retried.
    pub retryable: bool,
    /// Redacted detail.
    pub detail: Option<String>,
}

impl Failure {
    fn new(code: &str, retryable: bool, detail: Option<String>) -> Self {
        Self {
            code: code.to_owned(),
            retryable,
            detail,
        }
    }

    fn from_output(code: &str, retryable: bool, o: &exec::Output) -> Self {
        Self::new(code, retryable, Some(redacted_tail(&o.stderr, o.timed_out)))
    }
}

/// The last lines of a child's stderr, redacted and bounded.
#[must_use]
pub fn redacted_tail(stderr: &str, timed_out: bool) -> String {
    let lines: Vec<String> = stderr
        .lines()
        .filter(|l| !l.trim().is_empty())
        .rev()
        .take(4)
        .map(redact_line)
        .collect();
    let mut s: String = lines.into_iter().rev().collect::<Vec<_>>().join(" | ");
    if timed_out {
        s = format!("TIMEOUT {s}");
    }
    s.chars().take(400).collect()
}

/// What the executor needs to run a phase.
pub struct Ctx<'a> {
    /// The installation.
    pub id: &'a InstallationId,
    /// The sealed plan.
    pub plan: &'a InstallationPlan,
    /// The release manifest (checked against the plan).
    pub manifest: &'a ReleaseManifest,
    /// `/var/lib/ocinye-installer/<id>`.
    pub dir: &'a Path,
    /// The session upload directory (`/tmp/ocinye-bootstrap-…`).
    pub upload: &'a Path,
    /// Emit an event.
    pub emit: &'a mut dyn FnMut(Event),
}

/// What a phase changed that the journal must remember.
#[derive(Debug, Default)]
pub struct Effects {
    /// Paths created by the Installer.
    pub created_paths: Vec<String>,
    /// Packages installed by the Installer.
    pub installed_packages: Vec<String>,
    /// ufw rules added.
    pub firewall_rules: Vec<String>,
    /// The configuration was written (P06).
    pub config_written: bool,
    /// The self-signed certificate's SHA-256 (P07).
    pub self_signed_cert_sha256: Option<String>,
    /// The one-time credential (P11) — memory only.
    pub secret: Option<OneTimeSecret>,
}

/// `/srv/ocinye/.staging/<id>/bundle`.
#[must_use]
pub fn staging(id: &InstallationId) -> PathBuf {
    PathBuf::from(paths::ROOT)
        .join(".staging")
        .join(id.as_str())
        .join("bundle")
}

fn install_script(id: &InstallationId) -> PathBuf {
    staging(id).join("install").join("ocinye")
}

fn ocinye(id: &InstallationId, args: &[&str], limit: u64) -> exec::Output {
    exec::run_path(&install_script(id), args, exec::secs(limit))
}

fn sha256_file(path: &Path) -> std::io::Result<String> {
    use sha2::{Digest, Sha256};
    use std::io::Read;
    let mut f = fs::File::open(path)?;
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 1 << 20];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(hex::encode(h.finalize()))
}

/// The executable's own SHA-256.
///
/// # Errors
///
/// I/O errors.
pub fn self_sha256() -> std::io::Result<String> {
    sha256_file(&std::env::current_exe()?)
}

/// Verify a bundle directory against the manifest: the manifest's own bytes
/// must hash to the planned `manifest_sha256`, and every artifact must match.
///
/// # Errors
///
/// A [`Failure`] naming the first mismatch.
pub fn verify_bundle(
    bundle: &Path,
    manifest_sha256: &str,
    progress: &mut dyn FnMut(Operation),
) -> Result<ReleaseManifest, Failure> {
    let bytes = fs::read(bundle.join("MANIFEST.json"))
        .map_err(|_| Failure::new("TRANSFER_INCOMPLETE", true, Some("MANIFEST.json".into())))?;
    let manifest = ReleaseManifest::parse(&bytes)
        .map_err(|e| Failure::new("INVALID_MANIFEST", false, Some(e.field)))?;
    if manifest.sha256() != manifest_sha256 || sha256_hex(&bytes) != manifest_sha256 {
        return Err(Failure::new(
            "TRANSFER_CHECKSUM_MISMATCH",
            true,
            Some("MANIFEST.json".into()),
        ));
    }
    let total = u32::try_from(manifest.artifacts.len()).unwrap_or(u32::MAX);
    for (i, a) in manifest.artifacts.iter().enumerate() {
        progress(Operation::VerifyArtifact {
            path: a.path.clone(),
            done: u32::try_from(i).unwrap_or(u32::MAX),
            total,
        });
        let path = bundle.join(&a.path);
        let meta = fs::metadata(&path)
            .map_err(|_| Failure::new("TRANSFER_INCOMPLETE", true, Some(a.path.clone())))?;
        let sum = sha256_file(&path)
            .map_err(|_| Failure::new("TRANSFER_INCOMPLETE", true, Some(a.path.clone())))?;
        if meta.len() != a.bytes || sum != a.sha256 {
            return Err(Failure::new(
                "TRANSFER_CHECKSUM_MISMATCH",
                true,
                Some(a.path.clone()),
            ));
        }
    }
    // SHA256SUMS too: install/ocinye checks it again in its own terms.
    let sums = fs::read_to_string(bundle.join("SHA256SUMS"))
        .map_err(|_| Failure::new("TRANSFER_INCOMPLETE", true, Some("SHA256SUMS".into())))?;
    for line in sums.lines().filter(|l| !l.trim().is_empty()) {
        let Some((path, sum)) = manifest::parse_sums_line(line) else {
            return Err(Failure::new(
                "INVALID_MANIFEST",
                false,
                Some("SHA256SUMS".into()),
            ));
        };
        let got = sha256_file(&bundle.join(&path))
            .map_err(|_| Failure::new("TRANSFER_INCOMPLETE", true, Some(path.clone())))?;
        if got != sum {
            return Err(Failure::new("TRANSFER_CHECKSUM_MISMATCH", true, Some(path)));
        }
    }
    Ok(manifest)
}

fn move_tree(from: &Path, to: &Path) -> std::io::Result<()> {
    if let Some(parent) = to.parent() {
        fs::create_dir_all(parent)?;
    }
    if to.exists() {
        fs::remove_dir_all(to)?;
    }
    match fs::rename(from, to) {
        Ok(()) => Ok(()),
        Err(_) => {
            // Different filesystems: copy, then remove.
            copy_tree(from, to)?;
            fs::remove_dir_all(from)
        }
    }
}

fn copy_tree(from: &Path, to: &Path) -> std::io::Result<()> {
    fs::create_dir_all(to)?;
    for e in fs::read_dir(from)? {
        let e = e?;
        let target = to.join(e.file_name());
        let kind = e.file_type()?;
        if kind.is_dir() {
            copy_tree(&e.path(), &target)?;
        } else if kind.is_file() {
            fs::copy(e.path(), &target)?;
        } else {
            // A link copied by root would carry whatever it points at.
            return Err(std::io::Error::other("not a regular file"));
        }
    }
    Ok(())
}

/// Files in a staged bundle that are run, rather than read.
const BUNDLE_EXECUTABLES: [&str; 2] = ["install/ocinye", "ocinye-bootstrap"];

/// Makes a staged tree root's: owned by root, nothing writable by anyone else,
/// the bundle's two programs executable, and nothing but directories and
/// regular files. The upload arrives owned by the SSH user; verifying it
/// before this would check bytes that account could still change.
pub(crate) fn seal_tree(root: &Path) -> std::io::Result<()> {
    fn walk(root: &Path, dir: &Path) -> std::io::Result<()> {
        for e in fs::read_dir(dir)? {
            let path = e?.path();
            let meta = fs::symlink_metadata(&path)?;
            std::os::unix::fs::lchown(&path, Some(0), Some(0))?;
            if meta.is_dir() {
                fs::set_permissions(&path, fs::Permissions::from_mode(0o755))?;
                walk(root, &path)?;
            } else if meta.is_file() {
                let rel = path.strip_prefix(root).unwrap_or(&path);
                let exec = BUNDLE_EXECUTABLES.iter().any(|x| rel == Path::new(x));
                fs::set_permissions(
                    &path,
                    fs::Permissions::from_mode(if exec { 0o755 } else { 0o644 }),
                )?;
            } else {
                return Err(std::io::Error::other("not a regular file"));
            }
        }
        Ok(())
    }
    std::os::unix::fs::lchown(root, Some(0), Some(0))?;
    fs::set_permissions(root, fs::Permissions::from_mode(0o700))?;
    walk(root, root)
}

/// P01 · the executor in its place, verified.
pub fn p01(ctx: &mut Ctx<'_>) -> Result<Effects, Failure> {
    (ctx.emit)(Event::StepProgress {
        phase: PhaseId::P01,
        operation: Operation::VerifyBootstrap,
    });
    let own = self_sha256().map_err(|_| Failure::new("BOOTSTRAP_HASH_MISMATCH", false, None))?;
    if own != ctx.manifest.bootstrap.sha256 {
        return Err(Failure::new("BOOTSTRAP_HASH_MISMATCH", false, None));
    }
    Ok(Effects::default())
}

/// P02 · the uploaded release, verified on the server, then staged.
pub fn p02(ctx: &mut Ctx<'_>) -> Result<Effects, Failure> {
    let uploaded = ctx.upload.join("bundle");
    let staged = staging(ctx.id);
    let mut fx = Effects::default();
    if !Path::new(paths::ROOT).exists() {
        fx.created_paths.push(paths::ROOT.to_owned());
    }
    // Into root's staging first, sealed, and only then verified: what is
    // checked is what will run.
    if uploaded.join("MANIFEST.json").exists() {
        move_tree(&uploaded, &staged)
            .map_err(|e| Failure::new("DISK_FULL", true, Some(e.kind().to_string())))?;
    }
    if let Some(parent) = staged.parent() {
        let _ = fs::set_permissions(parent, fs::Permissions::from_mode(0o700));
    }
    seal_tree(&staged)
        .map_err(|e| Failure::new("TRANSFER_INCOMPLETE", true, Some(e.to_string())))?;
    let emit = &mut *ctx.emit;
    verify_bundle(&staged, &ctx.plan.release.manifest_sha256, &mut |op| {
        emit(Event::StepProgress {
            phase: PhaseId::P02,
            operation: op,
        });
    })?;
    // Operator TLS material: into the root-only state directory.
    let tls_in = ctx.upload.join("tls");
    if tls_in.exists() {
        let tls = ctx.dir.join("tls");
        move_tree(&tls_in, &tls).map_err(|_| Failure::new("TLS_INSTALL_FAILED", true, None))?;
        seal_tree(&tls).map_err(|_| Failure::new("TLS_INSTALL_FAILED", true, None))?;
        let _ = fs::set_permissions(tls.join("instance.key"), fs::Permissions::from_mode(0o600));
    }
    Ok(fx)
}

/// P03 · the blocking preflight items are what they were at review.
pub fn p03(ctx: &mut Ctx<'_>) -> Result<Effects, Failure> {
    (ctx.emit)(Event::StepProgress {
        phase: PhaseId::P03,
        operation: Operation::RecheckPreflight,
    });
    let facts = crate::facts::read();
    let report = server_preflight::run(
        &facts,
        ctx.plan.release.arch,
        chrono::Utc::now().timestamp(),
        &crate::hardware::Root::system(),
        &crate::state::StateRoot::system(),
        &mut |_| {},
    );
    for (id, before) in &ctx.plan.preflight {
        let now = report.item(*id).map(|i| i.status);
        // The executor's own state (journal, staging) may change PF-JOURNAL /
        // PF-SRVDIR / PF-EXIST: those are explained by the journal.
        if matches!(
            id,
            pf::PreflightCheckId::PfJournal
                | pf::PreflightCheckId::PfSrvdir
                | pf::PreflightCheckId::PfExist
        ) {
            continue;
        }
        if now == Some(CheckStatus::Blocked) && *before != CheckStatus::Blocked {
            return Err(Failure::new(
                "PREFLIGHT_CHANGED",
                true,
                Some(format!("{id:?}")),
            ));
        }
    }
    Ok(Effects::default())
}

fn dpkg_installed(package: &str) -> bool {
    let o = exec::run(
        program::DPKG_QUERY,
        &["-W", "-f=${Status}", package],
        exec::secs(10),
    );
    o.ok() && o.stdout.contains("install ok installed")
}

fn apt(args: &[&str], limit: u64) -> exec::Output {
    let mut all = vec!["-o", "DPkg::Lock::Timeout=300", "-y"];
    all.extend_from_slice(args);
    exec::run(program::APT_GET, &all, exec::secs(limit))
}

/// The deb822 source for Docker's repository on Ubuntu 24.04.
#[must_use]
pub fn docker_sources(arch: &str) -> String {
    format!(
        "Types: deb\nURIs: https://download.docker.com/linux/ubuntu\nSuites: noble\nComponents: stable\nArchitectures: {arch}\nSigned-By: /etc/apt/keyrings/docker.asc\n"
    )
}

/// P04 · prerequisites, only the planned ones.
pub fn p04(ctx: &mut Ctx<'_>) -> Result<Effects, Failure> {
    let mut fx = Effects::default();
    for change in &ctx.plan.system_changes {
        if !change.is_allowed(ctx.manifest) {
            return Err(Failure::new("PLAN_CHANGE_NOT_ALLOWED", false, None));
        }
        match change {
            SystemChange::EnsurePackages { packages } => {
                let missing: Vec<&str> = packages
                    .iter()
                    .map(String::as_str)
                    .filter(|p| !dpkg_installed(p))
                    .collect();
                if missing.is_empty() {
                    continue;
                }
                (ctx.emit)(Event::StepProgress {
                    phase: PhaseId::P04,
                    operation: Operation::AptInstall {
                        packages: missing.iter().map(|s| (*s).to_owned()).collect(),
                    },
                });
                let up = apt(&["update"], 600);
                if !up.ok() {
                    return Err(Failure::from_output("APT_UPDATE_FAILED", true, &up));
                }
                let mut args = vec!["--no-install-recommends", "install"];
                args.extend(missing.iter().copied());
                let o = apt(&args, 1200);
                if !o.ok() {
                    return Err(Failure::from_output(
                        "PREREQUISITE_INSTALL_FAILED",
                        true,
                        &o,
                    ));
                }
                fx.installed_packages
                    .extend(missing.iter().map(|s| (*s).to_owned()));
            }
            SystemChange::InstallDockerFromOfficialRepo {
                arch,
                packages,
                repo_key_fingerprint,
                ..
            } => {
                (ctx.emit)(Event::StepProgress {
                    phase: PhaseId::P04,
                    operation: Operation::VerifyDockerKey,
                });
                let key = exec::run(
                    program::CURL,
                    &[
                        "-fsSL",
                        "--proto",
                        "=https",
                        "--tlsv1.2",
                        "--max-time",
                        "60",
                        "https://download.docker.com/linux/ubuntu/gpg",
                    ],
                    exec::secs(90),
                );
                if !key.ok() {
                    return Err(Failure::from_output("REPO_KEY_DOWNLOAD_FAILED", true, &key));
                }
                // HTTPS is not trust: the fingerprint is.
                match openpgp::primary_fingerprint(&key.stdout) {
                    Ok(fp) if fp == *repo_key_fingerprint => {}
                    Ok(_) => return Err(Failure::new("REPO_KEY_MISMATCH", false, None)),
                    Err(e) => {
                        return Err(Failure::new(
                            "REPO_KEY_MISMATCH",
                            false,
                            Some(e.to_string()),
                        ))
                    }
                }
                let keyrings = Path::new("/etc/apt/keyrings");
                if !keyrings.exists() {
                    fs::create_dir_all(keyrings)
                        .map_err(|_| Failure::new("FILESYSTEM_ERROR", true, None))?;
                    let _ = fs::set_permissions(keyrings, fs::Permissions::from_mode(0o755));
                }
                // The bytes that were checked are the bytes that are trusted.
                crate::state::write_atomic(
                    &keyrings.join("docker.asc"),
                    key.stdout.as_bytes(),
                    0o644,
                )
                .map_err(|_| Failure::new("FILESYSTEM_ERROR", true, None))?;
                crate::state::write_atomic(
                    Path::new("/etc/apt/sources.list.d/docker.sources"),
                    docker_sources(arch.dpkg()).as_bytes(),
                    0o644,
                )
                .map_err(|_| Failure::new("FILESYSTEM_ERROR", true, None))?;
                fx.created_paths.push("/etc/apt/keyrings/docker.asc".into());
                fx.created_paths
                    .push("/etc/apt/sources.list.d/docker.sources".into());
                let up = apt(&["update"], 600);
                if !up.ok() {
                    return Err(Failure::from_output("APT_UPDATE_FAILED", true, &up));
                }
                let before: Vec<String> = packages
                    .iter()
                    .filter(|p| !dpkg_installed(p))
                    .cloned()
                    .collect();
                (ctx.emit)(Event::StepProgress {
                    phase: PhaseId::P04,
                    operation: Operation::AptInstall {
                        packages: packages.clone(),
                    },
                });
                let mut args = vec!["--no-install-recommends", "install"];
                args.extend(packages.iter().map(String::as_str));
                let o = apt(&args, 1800);
                if !o.ok() {
                    return Err(Failure::from_output(
                        "PREREQUISITE_INSTALL_FAILED",
                        true,
                        &o,
                    ));
                }
                fx.installed_packages.extend(before);
                (ctx.emit)(Event::StepProgress {
                    phase: PhaseId::P04,
                    operation: Operation::EnableDocker,
                });
                let en = exec::run(
                    program::SYSTEMCTL,
                    &["enable", "--now", "docker"],
                    exec::secs(120),
                );
                if !en.ok() {
                    return Err(Failure::from_output(
                        "PREREQUISITE_INSTALL_FAILED",
                        true,
                        &en,
                    ));
                }
                let obs = server_preflight::observe_docker();
                if pf::classify_runtime(true, &obs) != pf::ContainerRuntimeState::DetectedSupported
                {
                    return Err(Failure::new(
                        "PREREQUISITE_INSTALL_FAILED",
                        true,
                        Some("docker verification".into()),
                    ));
                }
            }
            SystemChange::FirewallAllow { .. } => {}
        }
    }
    Ok(fx)
}

fn phase_out(o: &exec::Output, code: &str, retryable: bool) -> Result<(), Failure> {
    if o.ok() {
        Ok(())
    } else {
        Err(Failure::from_output(code, retryable, o))
    }
}

/// P05 · the release tree.
pub fn p05(ctx: &mut Ctx<'_>) -> Result<Effects, Failure> {
    (ctx.emit)(Event::StepProgress {
        phase: PhaseId::P05,
        operation: Operation::ExtractRelease,
    });
    phase_out(
        &ocinye(ctx.id, &["phase", "release"], 600),
        "FILESYSTEM_ERROR",
        true,
    )?;
    let mut fx = Effects::default();
    fx.created_paths.push(paths::ROOT.to_owned());
    Ok(fx)
}

fn distributions_args(plan: &InstallationPlan) -> Vec<String> {
    plan.configuration
        .distributions
        .as_slice()
        .iter()
        .flat_map(|d| ["--distribution".to_owned(), d.as_str().to_owned()])
        .collect()
}

/// P06 · configuration and secrets (point of no return; never regenerated).
pub fn p06(ctx: &mut Ctx<'_>, already_written: bool) -> Result<Effects, Failure> {
    let mut fx = Effects::default();
    if already_written {
        fx.config_written = true;
        return Ok(fx);
    }
    (ctx.emit)(Event::StepProgress {
        phase: PhaseId::P06,
        operation: Operation::WriteConfiguration,
    });
    let c = &ctx.plan.configuration;
    let url = format!("https://{}", c.endpoints.canonical.as_str());
    let mut args: Vec<String> = vec![
        "phase".into(),
        "config".into(),
        "--domain".into(),
        c.endpoints.canonical.as_str().into(),
        "--public-url".into(),
        url,
        "--instance-name".into(),
        c.instance_name.as_str().into(),
    ];
    args.extend(distributions_args(ctx.plan));
    let refs: Vec<&str> = args.iter().map(String::as_str).collect();
    phase_out(&ocinye(ctx.id, &refs, 120), "CONFIG_WRITE_FAILED", false)?;
    fx.config_written = true;
    fx.created_paths.push(paths::CONFIG.to_owned());
    Ok(fx)
}

/// P07 · proxy and TLS.
pub fn p07(ctx: &mut Ctx<'_>) -> Result<Effects, Failure> {
    (ctx.emit)(Event::StepProgress {
        phase: PhaseId::P07,
        operation: Operation::WriteProxy,
    });
    let c = &ctx.plan.configuration;
    let tls_dir = ctx.dir.join("tls");
    let cert = tls_dir.join("instance.pem");
    let key = tls_dir.join("instance.key");
    let mut args: Vec<String> = vec![
        "phase".into(),
        "proxy".into(),
        "--domain".into(),
        c.endpoints.canonical.as_str().into(),
    ];
    for b in &c.endpoints.bound {
        args.push("--host".into());
        args.push(b.host.as_str().into());
    }
    match &c.tls {
        TlsPlan::OperatorSupplied { .. } => {
            args.extend([
                "--tls".into(),
                "provided".into(),
                "--cert".into(),
                cert.to_string_lossy().into_owned(),
                "--key".into(),
                key.to_string_lossy().into_owned(),
            ]);
        }
        TlsPlan::SelfSignedTest => args.extend(["--tls".into(), "self-signed".into()]),
    }
    (ctx.emit)(Event::StepProgress {
        phase: PhaseId::P07,
        operation: Operation::InstallTls,
    });
    let refs: Vec<&str> = args.iter().map(String::as_str).collect();
    let o = ocinye(ctx.id, &refs, 600);
    phase_out(&o, "TLS_INSTALL_FAILED", true)?;
    let served = o
        .stdout
        .lines()
        .find_map(|l| l.strip_prefix("cert_sha256\t"))
        .map(str::to_owned)
        .ok_or_else(|| Failure::new("TLS_INSTALL_FAILED", true, Some("cert_sha256".into())))?;
    let mut fx = Effects::default();
    (ctx.emit)(Event::TlsInstalled {
        cert_sha256: served.clone(),
    });
    match &c.tls {
        TlsPlan::OperatorSupplied { cert_sha256, .. } => {
            if served != *cert_sha256 {
                return Err(Failure::new(
                    "TLS_INSTALL_FAILED",
                    false,
                    Some("certificate differs from plan".into()),
                ));
            }
            // The key now lives only in /etc/ocinye/tls (0600).
            let _ = fs::remove_file(&key);
        }
        TlsPlan::SelfSignedTest => fx.self_signed_cert_sha256 = Some(served),
    }
    Ok(fx)
}

fn ufw_allows() -> Vec<u16> {
    server_preflight::parse_ufw_status(&exec::run(program::UFW, &["status"], exec::secs(15)).stdout)
        .1
}

/// P08 · the firewall rules Ocinye owns (ufw only, only the planned ones).
pub fn p08(ctx: &mut Ctx<'_>) -> Result<Effects, Failure> {
    let mut fx = Effects::default();
    for change in &ctx.plan.system_changes {
        let SystemChange::FirewallAllow { port, comment, .. } = change else {
            continue;
        };
        if !change.is_allowed(ctx.manifest) {
            return Err(Failure::new("PLAN_CHANGE_NOT_ALLOWED", false, None));
        }
        if ufw_allows().contains(port) {
            continue; // already allowed: no duplicate rule
        }
        (ctx.emit)(Event::StepProgress {
            phase: PhaseId::P08,
            operation: Operation::FirewallAllow { port: *port },
        });
        let rule = format!("{port}/tcp");
        let o = exec::run(
            program::UFW,
            &["allow", &rule, "comment", comment],
            exec::secs(60),
        );
        if !o.ok() {
            return Err(Failure::from_output("FIREWALL_APPLY_FAILED", true, &o));
        }
        if !ufw_allows().contains(port) {
            return Err(Failure::new("FIREWALL_APPLY_FAILED", true, Some(rule)));
        }
        fx.firewall_rules.push(rule);
    }
    Ok(fx)
}

/// P09 · images.
pub fn p09(ctx: &mut Ctx<'_>) -> Result<Effects, Failure> {
    for svc in manifest::SERVICES {
        (ctx.emit)(Event::StepProgress {
            phase: PhaseId::P09,
            operation: Operation::LoadImage {
                service: svc.to_owned(),
            },
        });
    }
    phase_out(
        &ocinye(ctx.id, &["phase", "images"], 1800),
        "IMAGE_LOAD_FAILED",
        true,
    )?;
    Ok(Effects::default())
}

/// P10 · database and storage.
pub fn p10(ctx: &mut Ctx<'_>) -> Result<Effects, Failure> {
    (ctx.emit)(Event::StepProgress {
        phase: PhaseId::P10,
        operation: Operation::StartPersistence,
    });
    let o = ocinye(ctx.id, &["phase", "data"], 1800);
    let code = if o.stderr.contains("armazenamento") {
        "OBJECT_STORE_INIT_FAILED"
    } else if o.stderr.contains("pull") {
        "IMAGE_PULL_FAILED"
    } else {
        "DATABASE_UNHEALTHY"
    };
    phase_out(&o, code, true)?;
    Ok(Effects::default())
}

/// Parse the one credential line of `phase instance`.
#[must_use]
pub fn parse_credential(stdout: &str) -> Option<OneTimeSecret> {
    let line = stdout.lines().find(|l| l.starts_with("credential\t"))?;
    let mut parts = line.splitn(4, '\t');
    let _ = parts.next();
    let user = parts.next()?.trim().to_owned();
    let expires_at = parts.next()?.trim().to_owned();
    let value = parts.next()?.trim().to_owned();
    (!user.is_empty() && !value.is_empty()).then(|| OneTimeSecret {
        user,
        expires_at,
        value: SecretText::new(value),
    })
}

/// P11 · Instance, migrations, first administrator. One shot.
pub fn p11(ctx: &mut Ctx<'_>) -> Result<Effects, Failure> {
    (ctx.emit)(Event::StepProgress {
        phase: PhaseId::P11,
        operation: Operation::CreateInstance,
    });
    let c = &ctx.plan.configuration;
    let mut args: Vec<String> = vec![
        "phase".into(),
        "instance".into(),
        "--instance-name".into(),
        c.instance_name.as_str().into(),
    ];
    args.extend(distributions_args(ctx.plan));
    args.extend([
        "--name".into(),
        c.admin.person.name.as_str().into(),
        "--email".into(),
        c.admin.person.email.as_str().into(),
        "--admin-name".into(),
        c.admin.privileged.name.as_str().into(),
        "--admin-email".into(),
        c.admin.privileged.email.as_str().into(),
        "--installation-id".into(),
        ctx.id.as_str().into(),
    ]);
    let refs: Vec<&str> = args.iter().map(String::as_str).collect();
    let mut o = ocinye(ctx.id, &refs, 1800);
    let secret = parse_credential(&o.stdout);
    // The child's stdout held the credential: wipe it from this process.
    {
        use zeroize::Zeroize as _;
        o.stdout.zeroize();
    }
    if !o.ok() {
        let code = if o.stderr.contains("Recusado") || o.stderr.contains("Conflict") {
            "ADMIN_BOOTSTRAP_REFUSED"
        } else if o.stderr.to_lowercase().contains("migration") {
            "MIGRATION_FAILED"
        } else {
            "DATABASE_FAILURE"
        };
        return Err(Failure::from_output(code, false, &o));
    }
    let Some(secret) = secret else {
        return Err(Failure::new(
            "ADMIN_BOOTSTRAP_REFUSED",
            false,
            Some("no credential".into()),
        ));
    };
    (ctx.emit)(Event::CredentialIssued {
        user: secret.user.clone(),
        expires_at: secret.expires_at.clone(),
    });
    Ok(Effects {
        secret: Some(secret),
        ..Effects::default()
    })
}

/// P12 · bound access endpoints through the Core's `endpoint-seed`.
pub fn p12(ctx: &mut Ctx<'_>) -> Result<Effects, Failure> {
    use ocinye_installer_contracts::core_output::EndpointSeedOutput;
    for b in &ctx.plan.configuration.endpoints.bound {
        (ctx.emit)(Event::StepProgress {
            phase: PhaseId::P12,
            operation: Operation::SeedEndpoint {
                host: b.host.as_str().to_owned(),
            },
        });
        let o = ocinye(
            ctx.id,
            &[
                "phase",
                "endpoint",
                "--installation-id",
                ctx.id.as_str(),
                "--host",
                b.host.as_str(),
                "--bind",
                b.distribution.as_str(),
            ],
            600,
        );
        let line = o
            .stdout
            .lines()
            .rev()
            .find(|l| l.starts_with('{'))
            .unwrap_or("");
        match serde_json::from_str::<EndpointSeedOutput>(line) {
            Ok(EndpointSeedOutput::Seeded { .. }) => {}
            Ok(EndpointSeedOutput::Refused { refused }) => {
                return Err(Failure::new(
                    "ENDPOINT_SEED_FAILED",
                    false,
                    Some(format!("{refused:?} {}", b.host)),
                ));
            }
            Err(_) => return Err(Failure::from_output("ENDPOINT_SEED_FAILED", true, &o)),
        }
    }
    Ok(Effects::default())
}

/// P13 · services.
pub fn p13(ctx: &mut Ctx<'_>) -> Result<Effects, Failure> {
    for svc in ["core", "workspace", "proxy"] {
        (ctx.emit)(Event::StepProgress {
            phase: PhaseId::P13,
            operation: Operation::WaitHealthy {
                service: svc.to_owned(),
            },
        });
    }
    let o = ocinye(ctx.id, &["phase", "services"], 1200);
    let svc = ["core", "workspace", "proxy"]
        .into_iter()
        .find(|s| o.stderr.contains(&format!("serviço {s} ")))
        .unwrap_or("unknown");
    if !o.ok() {
        return Err(Failure::from_output(
            &format!("SERVICE_UNHEALTHY:{svc}"),
            true,
            &o,
        ));
    }
    Ok(Effects::default())
}

/// P14 · start on boot.
pub fn p14(ctx: &mut Ctx<'_>) -> Result<Effects, Failure> {
    (ctx.emit)(Event::StepProgress {
        phase: PhaseId::P14,
        operation: Operation::EnableSystemd,
    });
    phase_out(
        &ocinye(ctx.id, &["phase", "systemd"], 120),
        "SYSTEMD_FAILED",
        true,
    )?;
    Ok(Effects::default())
}

/// P15 · server verification (read-only).
pub fn p15(ctx: &mut Ctx<'_>) -> Result<Effects, Failure> {
    (ctx.emit)(Event::VerificationStarted);
    let emit = &mut *ctx.emit;
    let items = crate::verify::run(ctx.plan, ctx.manifest, &mut |i| {
        emit(Event::VerificationItem { item: i.clone() });
    });
    let report = VerificationReport { items };
    (ctx.emit)(Event::VerificationCompleted {
        report: report.clone(),
    });
    let failed = report
        .items
        .iter()
        .find(|i| {
            i.status != ItemStatus::Pass
                && ocinye_installer_contracts::verification::VerificationId::SERVER_MANDATORY
                    .contains(&i.id)
        })
        .map(|i| format!("{:?}:{}", i.id, i.evidence));
    if let Some(f) = failed {
        return Err(Failure::new("VERIFICATION_FAILED", true, Some(f)));
    }
    Ok(Effects::default())
}

/// P16 · remove the temporary executor, the staging and the upload.
pub fn p16(ctx: &mut Ctx<'_>) -> Result<Effects, Failure> {
    (ctx.emit)(Event::StepProgress {
        phase: PhaseId::P16,
        operation: Operation::RemoveBootstrap,
    });
    let mut incomplete = false;
    for p in [ctx.dir.join("ocinye-bootstrap"), ctx.dir.join("tls")] {
        let r = if p.is_dir() {
            fs::remove_dir_all(&p)
        } else {
            fs::remove_file(&p)
        };
        incomplete |= r.is_err() && p.exists();
    }
    if let Some(stage) = staging(ctx.id).parent() {
        incomplete |= fs::remove_dir_all(stage).is_err() && stage.exists();
    }
    if is_upload_dir(ctx.upload) {
        incomplete |= fs::remove_dir_all(ctx.upload).is_err() && ctx.upload.exists();
    }
    if incomplete {
        (ctx.emit)(Event::StepWarning {
            phase: PhaseId::P16,
            code: "CLEANUP_INCOMPLETE".into(),
        });
    }
    Ok(Effects::default())
}

/// `/tmp/ocinye-bootstrap-<8..32 lowercase alnum>`, and nothing else.
#[must_use]
pub fn is_upload_dir(p: &Path) -> bool {
    p.to_str()
        .and_then(|s| s.strip_prefix(paths::UPLOAD_PREFIX))
        .is_some_and(|rest| {
            (8..=32).contains(&rest.len())
                && rest
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_credencial_le_se_de_uma_linha_e_so_dessa() {
        let s =
            parse_credential("ruido\ncredential\tadmin@x.test\t2026-10-05 14:21 UTC\tAbc-123\n")
                .unwrap();
        assert_eq!(s.user, "admin@x.test");
        assert_eq!(s.value.expose(), "Abc-123");
        assert!(!format!("{s:?}").contains("Abc-123"));
        assert!(parse_credential("Palavra-passe X\n").is_none());
        assert!(parse_credential("credential\t\t\t\n").is_none());
    }

    #[test]
    fn so_a_pasta_de_envio_da_sessao_se_apaga() {
        assert!(is_upload_dir(Path::new("/tmp/ocinye-bootstrap-a1b2c3d4")));
        for mau in [
            "/tmp/ocinye-bootstrap-",
            "/tmp/ocinye-bootstrap-../../etc",
            "/tmp/ocinye-bootstrap-ABCDEFGH",
            "/tmp",
            "/",
            "/srv/ocinye",
        ] {
            assert!(!is_upload_dir(Path::new(mau)), "{mau}");
        }
    }

    #[test]
    fn a_fonte_do_docker_e_a_oficial_com_a_chave_fixada() {
        let s = docker_sources("arm64");
        assert!(s.contains("URIs: https://download.docker.com/linux/ubuntu\n"));
        assert!(s.contains("Suites: noble\n"));
        assert!(s.contains("Signed-By: /etc/apt/keyrings/docker.asc\n"));
    }

    #[test]
    fn o_detalhe_de_uma_falha_e_redigido_e_curto() {
        let t = redacted_tail(
            "a\nPOSTGRES_PASSWORD=abc\n  Palavra-passe  X9\nfim\n",
            false,
        );
        assert!(!t.contains("abc") && !t.contains("X9"), "{t}");
        assert!(redacted_tail(&"x".repeat(2000), true).starts_with("TIMEOUT"));
        assert!(redacted_tail(&"y".repeat(2000), false).chars().count() <= 400);
    }

    #[test]
    fn um_pacote_alterado_e_recusado_no_servidor() {
        let dir = std::env::temp_dir().join(format!("ocinye-bundle-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("images")).unwrap();
        let m = crate::tests::fixture_manifest(&dir);
        let sha = m.sha256();
        verify_bundle(&dir, &sha, &mut |_| {}).expect("íntegro");
        // Um byte a mais numa imagem.
        let img = dir.join("images/ocinye-core-server.tar");
        let mut bytes = fs::read(&img).unwrap();
        bytes.push(b'!');
        fs::write(&img, bytes).unwrap();
        let err = verify_bundle(&dir, &sha, &mut |_| {}).unwrap_err();
        assert_eq!(err.code, "TRANSFER_CHECKSUM_MISMATCH");
        assert_eq!(err.detail.as_deref(), Some("images/ocinye-core-server.tar"));
        // Outro manifesto que o planeado.
        let err = verify_bundle(&dir, &"0".repeat(64), &mut |_| {}).unwrap_err();
        assert_eq!(err.code, "TRANSFER_CHECKSUM_MISMATCH");
        let _ = fs::remove_dir_all(&dir);
    }
}
