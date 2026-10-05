//! First boot (F1–F6) and the claim operations that run as root.
//!
//! The rules are `ocinye_image_contracts::claim`; this module loads the
//! state under the lock, applies one rule, and carries out its effects in an
//! order that never leaves a half-state: the key is written before the state
//! that says it is there, and removed before the state that says it is gone.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;
use std::time::Duration;

use ocinye_image_contracts::claim::{self, ClaimEvent, ClaimId, ClaimState, Outcome};
use ocinye_image_contracts::firstboot::{
    BootstrapId, FirstBootError, FirstBootState, IdentityPart, ImageFacts, KeyFingerprint,
    MachineFingerprint, MachineIdentity, PublicKeyLine,
};
use ocinye_image_contracts::manifest::ImageContentManifest;
use serde_json::json;

use crate::store::{self, Root};
use crate::system::System;

/// How long F1 waits for the kernel RNG.
pub const RNG_WAIT: Duration = Duration::from_secs(120);

const HOST_KEYS: [(&str, &str, &str); 2] = [
    ("ed25519", "ssh_host_ed25519_key", "ssh-ed25519"),
    ("ecdsa", "ssh_host_ecdsa_key", "ecdsa-sha2-nistp256"),
];

fn rand16(sys: &dyn System) -> std::io::Result<[u8; 16]> {
    let mut b = [0u8; 16];
    sys.random(&mut b)?;
    Ok(b)
}

/// F1: machine id (systemd's), host keys, bootstrap id. Complete or nothing:
/// `identity.json` is written last, so its presence means the rest is there.
pub fn identity(root: &Root, sys: &dyn System) -> Result<MachineIdentity, FirstBootError> {
    let fail = |part| FirstBootError::IdentityGenerationFailed { part };
    let machine_id = fs::read_to_string(root.sys_path("/etc/machine-id"))
        .unwrap_or_default()
        .trim()
        .to_owned();
    if !ocinye_image_contracts::is_lower_hex(&machine_id, 32) {
        return Err(fail(IdentityPart::MachineId));
    }
    if let Ok(Some(id)) = store::read_json::<MachineIdentity>(&root.identity()) {
        if id.machine_id == machine_id && host_keys(root).as_deref() == Ok(id.host_keys.as_slice())
        {
            return Ok(id);
        }
    }
    if !sys.wait_rng(RNG_WAIT) {
        return Err(fail(IdentityPart::SshHostKeys));
    }
    let ssh = root.sys_path("/etc/ssh");
    // Never reuse a partial identity: every host key is generated afresh.
    if let Ok(entries) = fs::read_dir(&ssh) {
        for e in entries.flatten() {
            if e.file_name().to_string_lossy().starts_with("ssh_host_") {
                let _ = fs::remove_file(e.path());
            }
        }
    }
    for (kind, file, _) in HOST_KEYS {
        let tmp = ssh.join(format!(".ocinye-{file}"));
        let _ = fs::remove_file(&tmp);
        let _ = fs::remove_file(tmp.with_extension("pub"));
        sys.ssh_keygen(kind, &tmp)
            .map_err(|_| fail(IdentityPart::SshHostKeys))?;
        fs::rename(tmp.with_extension("pub"), ssh.join(format!("{file}.pub")))
            .map_err(|_| fail(IdentityPart::SshHostKeys))?;
        fs::rename(&tmp, ssh.join(file)).map_err(|_| fail(IdentityPart::SshHostKeys))?;
    }
    let host_keys = host_keys(root).map_err(|()| fail(IdentityPart::SshHostKeys))?;
    let mut b = [0u8; 8];
    sys.random(&mut b)
        .map_err(|_| fail(IdentityPart::BootstrapId))?;
    let id = MachineIdentity {
        machine_id,
        host_keys,
        bootstrap_id: BootstrapId::from_bytes(b),
        created_at: claim::rfc3339(sys.now()),
    };
    store::write_json(&root.identity(), &id, 0o600).map_err(|_| fail(IdentityPart::BootstrapId))?;
    Ok(id)
}

/// The host keys on disk, Ed25519 first; `Err` unless exactly the two expected.
pub fn host_keys(root: &Root) -> Result<Vec<MachineFingerprint>, ()> {
    HOST_KEYS
        .iter()
        .map(|(_, file, alg)| {
            let line = fs::read_to_string(root.sys_path(&format!("/etc/ssh/{file}.pub")))
                .map_err(|_| ())?;
            let k = PublicKeyLine::parse(line.trim()).ok_or(())?;
            if k.algorithm != *alg || !root.sys_path(&format!("/etc/ssh/{file}")).exists() {
                return Err(());
            }
            Ok(MachineFingerprint {
                algorithm: k.algorithm.clone(),
                fingerprint: k.fingerprint(),
            })
        })
        .collect()
}

fn sha256_file(p: &Path) -> std::io::Result<(String, u64)> {
    use sha2::Digest as _;
    use std::io::Read as _;
    let mut f = fs::File::open(p)?;
    let mut h = sha2::Sha256::new();
    let mut buf = vec![0u8; 1 << 16];
    let mut n = 0u64;
    loop {
        let r = f.read(&mut buf)?;
        if r == 0 {
            break;
        }
        n += r as u64;
        h.update(&buf[..r]);
    }
    Ok((hex::encode(h.finalize()), n))
}

fn walk(dir: &Path, base: &Path, out: &mut BTreeSet<String>) -> std::io::Result<()> {
    for e in fs::read_dir(dir)? {
        let e = e?;
        let t = e.file_type()?;
        let p = e.path();
        if t.is_dir() {
            walk(&p, base, out)?;
        } else {
            out.insert(
                p.strip_prefix(base)
                    .map_err(std::io::Error::other)?
                    .to_string_lossy()
                    .into_owned(),
            );
        }
    }
    Ok(())
}

/// F3: the release payload and the preloaded images against the embedded
/// `ImageContentManifest`, read-only, Docker not running.
pub fn integrity(root: &Root) -> Result<(ImageContentManifest, ImageFacts), FirstBootError> {
    let invalid = |f: &str| FirstBootError::ImageContentManifestInvalid { field: f.into() };
    let tampered = |p: &str| FirstBootError::ReleasePayloadTampered { path: p.into() };
    let bytes = fs::read(root.sys_path(ocinye_image_contracts::paths::IMAGE_CONTENT))
        .map_err(|_| invalid("file"))?;
    let m = ImageContentManifest::parse(&bytes).map_err(|e| invalid(&e.field))?;
    let facts: ImageFacts =
        store::read_json(&root.sys_path(ocinye_image_contracts::paths::IMAGE_FACTS))
            .ok()
            .flatten()
            .ok_or_else(|| invalid("image.json"))?;
    if facts.content_sha256 != m.sha256()
        || facts.release_id != m.release_id
        || facts.image != m.image
    {
        return Err(invalid("image.json"));
    }
    let base = root
        .sys_path(ocinye_image_contracts::paths::RELEASE_ROOT)
        .join(&m.release_id);
    let mut present = BTreeSet::new();
    walk(&base, &base, &mut present).map_err(|_| tampered(&m.release_id))?;
    let expected: BTreeSet<String> = m.release_files.iter().map(|f| f.path.clone()).collect();
    if let Some(extra) = present.difference(&expected).next() {
        return Err(tampered(extra));
    }
    for f in &m.release_files {
        match sha256_file(&base.join(&f.path)) {
            Ok((h, n)) if h == f.sha256.0 && n == f.bytes => {}
            _ => return Err(tampered(&f.path)),
        }
    }
    // Preloaded images. Docker keeps them in one of two stores: the classic
    // overlay2 image store (a JSON name index and an image database), or the
    // containerd content store that Docker 29 uses on a fresh install. In both
    // the image config is content-addressed: it must be present and hash to
    // the image id. The overlay2 index also lets the compose name be checked;
    // the containerd name index is a bolt database first boot does not read
    // (layers are not re-hashed here: D013_SECURITY_MATRIX, F3 scope).
    let overlay = root.sys_path("/var/lib/docker/image/overlay2");
    let content =
        root.sys_path("/var/lib/containerd/io.containerd.content.v1.content/blobs/sha256");
    let repos: Option<serde_json::Value> = store::read_json(&overlay.join("repositories.json"))
        .ok()
        .flatten();
    for img in &m.oci_images {
        let id = &img.image_id.0;
        let hex = id.strip_prefix("sha256:").unwrap_or_default();
        let hashes = |p: &Path| sha256_file(p).ok().is_some_and(|(h, _)| h == hex);
        let in_overlay = repos.as_ref().is_some_and(|r| {
            r["Repositories"][&img.name]
                .as_object()
                .is_some_and(|tags| tags.values().any(|v| v.as_str() == Some(id.as_str())))
        }) && hashes(&overlay.join("imagedb/content/sha256").join(hex));
        let in_containerd = hashes(&content.join(hex));
        if !(in_overlay || in_containerd) {
            return Err(tampered(&img.name));
        }
    }
    Ok((m, facts))
}

/// Keys the platform or the OIE placed on `ocinye` before first boot.
fn read_provisioned(root: &Root) -> Vec<KeyFingerprint> {
    let mut v: Vec<_> = fs::read_to_string(root.ocinye_keys())
        .unwrap_or_default()
        .lines()
        .filter_map(|l| PublicKeyLine::parse(l.trim()))
        .map(|k| k.fingerprint())
        .collect();
    v.sort();
    v.dedup();
    v
}

/// Apply the claim access that matches the state: the `claimable` marker
/// and the account. Run under the lock.
fn sync_access(root: &Root, sys: &dyn System, state: &ClaimState, now: i64) -> std::io::Result<()> {
    let open = match state {
        ClaimState::Unclaimed { locked_until, .. } => locked_until
            .as_deref()
            .and_then(claim::unix)
            .is_none_or(|u| now >= u),
        _ => false,
    };
    if open {
        store::write_atomic(&root.claimable(), b"", 0o644)?;
    } else {
        store::remove(&root.claimable())?;
    }
    sys.claim_account(!matches!(state, ClaimState::Claimed { .. }))
}

/// The runtime enabled at `Claimed` and only then (§ Docker held and disabled).
fn on_claimed(sys: &dyn System) {
    for unit in [
        "containerd.service",
        "docker.socket",
        "docker.service",
        "unattended-upgrades.service",
    ] {
        let _ = sys.systemctl(&["enable", "--now", unit]);
    }
}

/// F1–F6 at boot. Later boots keep the identity, re-check integrity and
/// issue a new code (codes never survive a reboot).
pub fn init(root: &Root, sys: &dyn System) -> FirstBootState {
    let state = init_inner(root, sys);
    let _ = store::write_json(&root.firstboot(), &state, 0o600);
    match &state {
        FirstBootState::Failed { error } => {
            store::journal(root, "firstboot_failed", Some(json!(error)))
        }
        _ => store::journal(root, "firstboot_ready", None),
    }
    state
}

fn init_inner(root: &Root, sys: &dyn System) -> FirstBootState {
    if root.prepare().is_err() {
        return FirstBootState::Failed {
            error: FirstBootError::ClaimServiceFailed,
        };
    }
    // Nothing is claimable while first boot runs.
    let _ = store::remove(&root.claimable());
    let _ = store::remove(&root.code());
    let id = match identity(root, sys) {
        Ok(id) => id,
        Err(error) => return FirstBootState::Failed { error },
    };
    if let Err(error) = integrity(root) {
        return FirstBootState::Failed { error };
    }
    let Ok(_lock) = store::lock(root) else {
        return FirstBootState::Failed {
            error: FirstBootError::ClaimServiceFailed,
        };
    };
    let now = sys.now();
    let fresh = |root: &Root| -> std::io::Result<()> {
        let offered = root.offered();
        let _ = fs::remove_dir_all(&offered);
        fs::create_dir_all(&offered)?;
        use std::os::unix::fs::PermissionsExt as _;
        fs::set_permissions(&offered, fs::Permissions::from_mode(0o700))?;
        sys.chown_offered(&offered)
    };
    let result = (|| -> std::io::Result<()> {
        fresh(root)?;
        let mut state: ClaimState = match store::read_json(&root.claim_state())? {
            Some(s) => s,
            None => {
                // The first boot of this machine: record what the platform
                // provisioned and close the firewall before anything listens.
                store::write_json(&root.provisioned(), &read_provisioned(root), 0o600)?;
                sys.ufw(&["default", "deny", "incoming"])?;
                sys.ufw(&["default", "allow", "outgoing"])?;
                sys.ufw(&["limit", "22/tcp", "comment", "ocinye-firstboot"])?;
                sys.ufw(&["--force", "enable"])?;
                let s = ClaimState::Unclaimed {
                    generation: 1,
                    locked_until: None,
                };
                store::write_json(&root.claim_state(), &s, 0o600)?;
                store::journal(
                    root,
                    "unclaimed",
                    Some(json!({"generation": 1, "bootstrap_id": id.bootstrap_id})),
                );
                s
            }
        };
        if let ClaimState::ClaimInProgress { deadline, .. } = &state {
            match claim::unix(deadline) {
                Some(d) if d > now => sys.arm_timeout(d - now)?,
                _ => {
                    let o = claim::roll_back(&state, None, now, rand16(sys)?, 0);
                    state = apply(root, sys, &state, o)?.unwrap_or(state);
                }
            }
        }
        if matches!(state, ClaimState::Unclaimed { .. }) {
            // While unclaimed, `ocinye` holds only what the platform provisioned
            // (a key enrolled just before a crash does not survive the reboot).
            let provisioned: Vec<KeyFingerprint> =
                store::read_json(&root.provisioned())?.unwrap_or_default();
            let text = fs::read_to_string(root.ocinye_keys()).unwrap_or_default();
            let kept: String = text
                .lines()
                .filter(|l| {
                    PublicKeyLine::parse(l.trim())
                        .is_some_and(|k| provisioned.contains(&k.fingerprint()))
                })
                .map(|l| format!("{l}\n"))
                .collect();
            if kept != text {
                store::write_atomic(&root.ocinye_keys(), kept.as_bytes(), 0o600)?;
                sys.chown_ocinye(&root.ocinye_keys())?;
                store::journal(root, "unprovisioned_keys_removed", None);
            }
            let rec = claim::new_code(rand16(sys)?, 1, now, None);
            store::write_json(&root.code(), &rec, 0o600)?;
            store::journal(
                root,
                "code_issued",
                Some(json!({"code_generation": rec.code_generation})),
            );
        }
        if matches!(state, ClaimState::Claimed { .. }) {
            on_claimed(sys);
        }
        sync_access(root, sys, &state, now)
    })();
    match result {
        Ok(()) => FirstBootState::Ready,
        Err(_) => FirstBootState::Failed {
            error: FirstBootError::ClaimServiceFailed,
        },
    }
}

fn remove_key_line(root: &Root, sys: &dyn System, fp: &KeyFingerprint) -> std::io::Result<()> {
    let text = fs::read_to_string(root.ocinye_keys()).unwrap_or_default();
    let kept: String = text
        .lines()
        .filter(|l| PublicKeyLine::parse(l.trim()).is_none_or(|k| &k.fingerprint() != fp))
        .map(|l| format!("{l}\n"))
        .collect();
    // The rename replaces the inode: re-own it, sshd reads it as `ocinye`.
    store::write_atomic(&root.ocinye_keys(), kept.as_bytes(), 0o600)?;
    sys.chown_ocinye(&root.ocinye_keys())
}

/// Carry out an outcome under the lock; returns the new state if it changed.
///
/// Order: a key enrolled is written to `authorized_keys` *before* the state
/// that names it; a rolled-back key is removed *before* the state forgets it.
pub fn apply(
    root: &Root,
    sys: &dyn System,
    before: &ClaimState,
    o: Outcome,
) -> std::io::Result<Option<ClaimState>> {
    if let (Some(ClaimState::Unclaimed { .. }), ClaimState::ClaimInProgress { owner_key, .. }) =
        (&o.state, before)
    {
        remove_key_line(root, sys, owner_key)?;
    }
    if let Some(code) = &o.code {
        match code {
            Some(rec) => store::write_json(&root.code(), rec, 0o600)?,
            None => store::remove(&root.code())?,
        }
    }
    if let Some(s) = &o.state {
        store::write_json(&root.claim_state(), s, 0o600)?;
    }
    let now = sys.now();
    let after = o.state.clone().unwrap_or_else(|| before.clone());
    sync_access(root, sys, &after, now)?;
    let detail = match (&o.event, &o.code) {
        (ClaimEvent::Refused { reason }, _) => Some(json!({"reason": reason})),
        (_, Some(Some(rec))) => Some(json!({"code_generation": rec.code_generation})),
        (ClaimEvent::Enrolled { owner_key, .. } | ClaimEvent::Confirmed { owner_key, .. }, _) => {
            Some(json!({"owner_key": owner_key}))
        }
        _ => None,
    };
    store::journal(root, o.journal, detail);
    if matches!(o.state, Some(ClaimState::Claimed { .. })) {
        on_claimed(sys);
    }
    Ok(o.state)
}

/// Errors of the root-side commands, as the process reports them.
#[derive(Debug, PartialEq, Eq)]
pub enum CommandError {
    /// A typed refusal (printed as a `Refused` event).
    Refused(claim::ClaimRefusal),
    /// Not initialised or not readable.
    NotReady,
}

fn refused_or(o: &Outcome) -> Result<ClaimEvent, CommandError> {
    match &o.event {
        ClaimEvent::Refused { reason } => Err(CommandError::Refused(reason.clone())),
        e => Ok(e.clone()),
    }
}

fn ready(root: &Root) -> Result<ClaimState, CommandError> {
    match store::read_json::<FirstBootState>(&root.firstboot()) {
        Ok(Some(FirstBootState::Ready)) => {}
        _ => {
            return Err(CommandError::Refused(claim::ClaimRefusal::IntegrityFailed));
        }
    }
    store::read_json(&root.claim_state())
        .ok()
        .flatten()
        .ok_or(CommandError::NotReady)
}

/// `confirm --claim <id>` (as root through `sudo -n` from `ocinye`).
pub fn confirm(
    root: &Root,
    sys: &dyn System,
    claim_id: &ClaimId,
    sudo_user: &str,
) -> Result<ClaimEvent, CommandError> {
    let _l = store::lock(root).map_err(|_| CommandError::NotReady)?;
    let st = ready(root)?;
    let o = claim::confirm(&st, claim_id, sudo_user, sys.now());
    apply(root, sys, &st, o.clone()).map_err(|_| CommandError::NotReady)?;
    if o.state.is_some() {
        let _ = sys.systemctl(&["stop", "ocinye-claim-timeout.timer"]);
    }
    refused_or(&o)
}

/// `claim --provisioned --key <fp>`.
pub fn claim_provisioned(
    root: &Root,
    sys: &dyn System,
    key: &KeyFingerprint,
    sudo_user: &str,
) -> Result<ClaimEvent, CommandError> {
    let _l = store::lock(root).map_err(|_| CommandError::NotReady)?;
    let st = ready(root)?;
    let provisioned: Vec<KeyFingerprint> = store::read_json(&root.provisioned())
        .ok()
        .flatten()
        .unwrap_or_default();
    let o = claim::claim_provisioned(
        &st,
        key,
        &provisioned,
        sudo_user,
        sys.now(),
        rand16(sys).map_err(|_| CommandError::NotReady)?,
    );
    apply(root, sys, &st, o.clone()).map_err(|_| CommandError::NotReady)?;
    refused_or(&o)
}

fn code_generation(root: &Root) -> u32 {
    store::read_json::<claim::CodeRecord>(&root.code())
        .ok()
        .flatten()
        .map_or(0, |c| c.code_generation)
}

/// The timer: roll back if the deadline passed.
pub fn claim_timeout(root: &Root, sys: &dyn System) -> Result<ClaimEvent, CommandError> {
    let _l = store::lock(root).map_err(|_| CommandError::NotReady)?;
    let st = ready(root)?;
    let o = claim::roll_back(
        &st,
        None,
        sys.now(),
        rand16(sys).map_err(|_| CommandError::NotReady)?,
        code_generation(root),
    );
    apply(root, sys, &st, o.clone()).map_err(|_| CommandError::NotReady)?;
    refused_or(&o)
}

/// Console `U`: physical presence lifts a lockout and issues a new code.
pub fn unlock(root: &Root, sys: &dyn System) -> Result<(), CommandError> {
    let _l = store::lock(root).map_err(|_| CommandError::NotReady)?;
    let st = ready(root)?;
    let ClaimState::Unclaimed {
        generation,
        locked_until: Some(_),
    } = st
    else {
        return Ok(());
    };
    let now = sys.now();
    let next = ClaimState::Unclaimed {
        generation,
        locked_until: None,
    };
    let rec = claim::new_code(
        rand16(sys).map_err(|_| CommandError::NotReady)?,
        code_generation(root) + 1,
        now,
        None,
    );
    let o = Outcome {
        event: ClaimEvent::Aborted,
        state: Some(next),
        code: Some(Some(rec)),
        journal: "unlocked_locally",
    };
    apply(root, sys, &st, o).map_err(|_| CommandError::NotReady)?;
    Ok(())
}

/// Console «Libertar» while `Claimed` and before D011 passed P06 [P, PD-07].
pub fn release(
    root: &Root,
    sys: &dyn System,
    d011_past_p06: bool,
) -> Result<ClaimEvent, CommandError> {
    let _l = store::lock(root).map_err(|_| CommandError::NotReady)?;
    let st = ready(root)?;
    let o = claim::local_release(
        &st,
        d011_past_p06,
        sys.now(),
        rand16(sys).map_err(|_| CommandError::NotReady)?,
        code_generation(root),
    );
    if let (Some(_), ClaimState::Claimed { owner_key, .. }) = (&o.state, &st) {
        remove_key_line(root, sys, owner_key).map_err(|_| CommandError::NotReady)?;
    }
    apply(root, sys, &st, o.clone()).map_err(|_| CommandError::NotReady)?;
    refused_or(&o)
}

/// Whether D011 has gone past P06 (secrets/Instance exist) [P, PD-07].
pub fn d011_past_p06(root: &Root) -> bool {
    root.sys_path("/etc/ocinye/ocinye.env").exists() || root.sys_path("/srv/ocinye").exists()
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::store::testing;
    use crate::system::fake::Fake;
    use ocinye_image_contracts::manifest::{ContentFile, ImageFormat, ImageProfile, Sha256Hex};

    pub const T0: i64 = 1_791_000_000;

    /// A machine image under a test root: machine id, release payload,
    /// embedded manifest and facts, an empty Docker image store.
    pub fn machine(name: &str) -> Root {
        let r = testing::root(name);
        fs::create_dir_all(r.sys_path("/etc/ssh")).unwrap();
        fs::create_dir_all(r.sys_path("/etc/ocinye")).unwrap();
        fs::write(
            r.sys_path("/etc/machine-id"),
            "0123456789abcdef0123456789abcdef\n",
        )
        .unwrap();
        let mut m = ocinye_image_contracts::manifest::fixtures::content();
        m.oci_images.clear();
        let rel = r
            .sys_path(ocinye_image_contracts::paths::RELEASE_ROOT)
            .join(&m.release_id);
        fs::create_dir_all(rel.join("install")).unwrap();
        let mut files = vec![];
        for (p, body) in [("MANIFEST.json", "{}"), ("install/ocinye", "#!/bin/sh\n")] {
            fs::write(rel.join(p), body).unwrap();
            let (h, n) = sha256_file(&rel.join(p)).unwrap();
            files.push(ContentFile {
                path: p.into(),
                sha256: Sha256Hex(h),
                bytes: n,
            });
        }
        m.release_files = files;
        m.validate().unwrap();
        let p = r.sys_path(ocinye_image_contracts::paths::IMAGE_CONTENT);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(
            &p,
            ocinye_installer_contracts::canonical::to_canonical(&m).unwrap(),
        )
        .unwrap();
        let facts = ImageFacts {
            image: m.image.clone(),
            profile: ImageProfile::Virt,
            release_id: m.release_id.clone(),
            content_sha256: m.sha256(),
            ubuntu_serial: "20261001".into(),
            source_format: ImageFormat::Qcow2,
        };
        store::write_json(
            &r.sys_path(ocinye_image_contracts::paths::IMAGE_FACTS),
            &facts,
            0o644,
        )
        .unwrap();
        let d = r.sys_path("/var/lib/docker/image/overlay2");
        fs::create_dir_all(d.join("imagedb/content/sha256")).unwrap();
        fs::write(d.join("repositories.json"), r#"{"Repositories":{}}"#).unwrap();
        r
    }

    #[test]
    fn primeiro_arranque_gera_identidade_e_fica_por_reclamar() {
        let r = machine("init");
        let sys = Fake::new(T0);
        assert_eq!(init(&r, &sys), FirstBootState::Ready);
        let id: MachineIdentity = store::read_json(&r.identity()).unwrap().unwrap();
        assert_eq!(id.host_keys.len(), 2);
        assert_eq!(id.host_keys[0].algorithm, "ssh-ed25519");
        assert!(id.bootstrap_id.is_valid());
        assert_eq!(
            store::read_json::<ClaimState>(&r.claim_state()).unwrap(),
            Some(ClaimState::Unclaimed {
                generation: 1,
                locked_until: None
            })
        );
        assert!(r.code().exists() && r.claimable().exists());
        assert!(sys.called("ufw limit 22/tcp comment ocinye-firstboot"));
        assert!(
            !sys.called("systemctl enable --now docker"),
            "docker stays disabled until claimed"
        );
        // A second boot keeps the identity and issues a new code.
        let c1 = fs::read(r.code()).unwrap();
        sys.seed.set(77);
        assert_eq!(init(&r, &sys), FirstBootState::Ready);
        let id2: MachineIdentity = store::read_json(&r.identity()).unwrap().unwrap();
        assert_eq!(id2, id);
        assert_ne!(
            fs::read(r.code()).unwrap(),
            c1,
            "codes never survive a reboot"
        );
        let j = fs::read_to_string(r.journal()).unwrap();
        let rec: claim::CodeRecord = store::read_json(&r.code()).unwrap().unwrap();
        assert!(
            !j.contains(rec.code.expose()),
            "the journal never holds a code"
        );
    }

    #[test]
    fn dois_clones_tem_identidades_diferentes() {
        let a = machine("clone-a");
        let b = machine("clone-b");
        let sa = Fake::new(T0);
        let sb = Fake::new(T0);
        sb.seed.set(200);
        fs::write(
            b.sys_path("/etc/machine-id"),
            "fedcba9876543210fedcba9876543210\n",
        )
        .unwrap();
        init(&a, &sa);
        init(&b, &sb);
        let ia: MachineIdentity = store::read_json(&a.identity()).unwrap().unwrap();
        let ib: MachineIdentity = store::read_json(&b.identity()).unwrap().unwrap();
        assert_ne!(ia.machine_id, ib.machine_id);
        assert_ne!(ia.bootstrap_id, ib.bootstrap_id);
        assert_ne!(ia.host_keys, ib.host_keys);
        let ca: claim::CodeRecord = store::read_json(&a.code()).unwrap().unwrap();
        let cb: claim::CodeRecord = store::read_json(&b.code()).unwrap().unwrap();
        assert!(!ca.code.matches(&cb.code));
    }

    #[test]
    fn sem_entropia_nao_ha_identidade_nem_reclamacao() {
        let r = machine("norng");
        let mut sys = Fake::new(T0);
        sys.rng_ready = false;
        assert_eq!(
            init(&r, &sys),
            FirstBootState::Failed {
                error: FirstBootError::IdentityGenerationFailed {
                    part: IdentityPart::SshHostKeys
                }
            }
        );
        assert!(!r.claimable().exists() && !r.code().exists() && !r.identity().exists());
        // An empty machine-id (systemd did not run) is refused as well.
        let r = machine("nomid");
        fs::write(r.sys_path("/etc/machine-id"), "").unwrap();
        assert!(matches!(
            init(&r, &Fake::new(T0)),
            FirstBootState::Failed {
                error: FirstBootError::IdentityGenerationFailed {
                    part: IdentityPart::MachineId
                }
            }
        ));
    }

    #[test]
    fn release_adulterado_desliga_a_reclamacao() {
        let r = machine("tamper");
        let id =
            store::read_json::<ImageFacts>(&r.sys_path(ocinye_image_contracts::paths::IMAGE_FACTS))
                .unwrap()
                .unwrap()
                .release_id;
        let rel = r
            .sys_path(ocinye_image_contracts::paths::RELEASE_ROOT)
            .join(id);
        fs::write(rel.join("install/ocinye"), "#!/bin/sh\nevil\n").unwrap();
        assert_eq!(
            init(&r, &Fake::new(T0)),
            FirstBootState::Failed {
                error: FirstBootError::ReleasePayloadTampered {
                    path: "install/ocinye".into()
                }
            }
        );
        assert!(!r.claimable().exists());
        // An extra file is tampering too.
        let r = machine("extra");
        let id =
            store::read_json::<ImageFacts>(&r.sys_path(ocinye_image_contracts::paths::IMAGE_FACTS))
                .unwrap()
                .unwrap()
                .release_id;
        fs::write(
            r.sys_path(ocinye_image_contracts::paths::RELEASE_ROOT)
                .join(id)
                .join("x"),
            "",
        )
        .unwrap();
        assert!(matches!(
            init(&r, &Fake::new(T0)),
            FirstBootState::Failed {
                error: FirstBootError::ReleasePayloadTampered { .. }
            }
        ));
        // And a manifest that no longer matches the facts.
        let r = machine("facts");
        let p = r.sys_path(ocinye_image_contracts::paths::IMAGE_FACTS);
        let mut f: ImageFacts = store::read_json(&p).unwrap().unwrap();
        f.content_sha256 = "0".repeat(64);
        store::write_json(&p, &f, 0o644).unwrap();
        assert!(matches!(
            init(&r, &Fake::new(T0)),
            FirstBootState::Failed {
                error: FirstBootError::ImageContentManifestInvalid { .. }
            }
        ));
    }

    #[test]
    fn imagens_pre_carregadas_conferem_nos_dois_armazens() {
        use ocinye_image_contracts::manifest::{OciDigest, OciImageRef, OciRole};
        for store_kind in ["overlay2", "containerd"] {
            let r = machine(&format!("oci-{store_kind}"));
            let cfg = br#"{"architecture":"amd64"}"#;
            let p = r.sys.join("cfg");
            fs::write(&p, cfg).unwrap();
            let (hex, _) = sha256_file(&p).unwrap();
            let id = format!("sha256:{hex}");
            let blob = if store_kind == "overlay2" {
                let d = r.sys_path("/var/lib/docker/image/overlay2");
                fs::write(
                    d.join("repositories.json"),
                    format!(r#"{{"Repositories":{{"nginx":{{"nginx:x":"{id}"}}}}}}"#),
                )
                .unwrap();
                d.join("imagedb/content/sha256").join(&hex)
            } else {
                let d =
                    r.sys_path("/var/lib/containerd/io.containerd.content.v1.content/blobs/sha256");
                fs::create_dir_all(&d).unwrap();
                d.join(&hex)
            };
            fs::write(&blob, cfg).unwrap();
            // Put the image in the embedded manifest and the facts.
            let mp = r.sys_path(ocinye_image_contracts::paths::IMAGE_CONTENT);
            let mut m = ImageContentManifest::parse(&fs::read(&mp).unwrap()).unwrap();
            m.oci_images = vec![OciImageRef {
                name: "nginx".into(),
                role: OciRole::ThirdParty,
                reference: format!("nginx:1.30.5-alpine@sha256:{}", "a".repeat(64)),
                digest: OciDigest(format!("sha256:{}", "a".repeat(64))),
                image_id: OciDigest(id.clone()),
            }];
            fs::write(&mp, m.to_canonical()).unwrap();
            let fp = r.sys_path(ocinye_image_contracts::paths::IMAGE_FACTS);
            let mut f: ImageFacts = store::read_json(&fp).unwrap().unwrap();
            f.content_sha256 = m.sha256();
            store::write_json(&fp, &f, 0o644).unwrap();
            assert!(integrity(&r).is_ok(), "{store_kind}");
            // A rewritten config no longer hashes to its id.
            fs::write(&blob, br#"{"architecture":"evil"}"#).unwrap();
            assert_eq!(
                integrity(&r).unwrap_err(),
                FirstBootError::ReleasePayloadTampered {
                    path: "nginx".into()
                },
                "{store_kind}"
            );
            // And a missing one is missing.
            fs::remove_file(&blob).unwrap();
            assert!(integrity(&r).is_err());
        }
    }

    #[test]
    fn chave_provisionada_reclama_e_liga_o_docker() {
        let r = machine("prov");
        let sys = Fake::new(T0);
        // The platform put a key on `ocinye` before first boot.
        let k = fs::read_to_string({
            sys.ssh_keygen("ed25519", &r.sys.join("opkey")).unwrap();
            r.sys.join("opkey.pub")
        })
        .unwrap();
        fs::write(r.ocinye_keys(), &k).unwrap();
        init(&r, &sys);
        let fp = PublicKeyLine::parse(k.trim()).unwrap().fingerprint();
        let other = KeyFingerprint(format!("SHA256:{}", "Z".repeat(43)));
        assert!(matches!(
            claim_provisioned(&r, &sys, &other, "ocinye"),
            Err(CommandError::Refused(_))
        ));
        assert!(matches!(
            claim_provisioned(&r, &sys, &fp, "ocinye"),
            Ok(ClaimEvent::Confirmed { .. })
        ));
        assert!(!r.claimable().exists() && !r.code().exists());
        assert!(sys.called("systemctl enable --now docker.service"));
        assert!(sys.called("claim-account false"));
        // Replay after Claimed.
        assert!(matches!(
            claim_provisioned(&r, &sys, &fp, "ocinye"),
            Err(CommandError::Refused(claim::ClaimRefusal::AlreadyClaimed))
        ));
    }
}
