//! The installation itself: check the medium, hand curtin a fixed layout on
//! the confirmed stable path, check what was written, prepare first boot.
//!
//! curtin is given a by-path (never `/dev/sdX`), a template this module
//! renders, and no network: the base install is offline by construction.

use std::fs;
use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::process::Command;

use ocinye_image_contracts::firstboot::{KeyFingerprint, PublicKeyLine};
use ocinye_image_contracts::manifest::{ImageContentManifest, Sha256Hex};
use ocinye_image_contracts::oie::{layout, InstallationTargetDisk, OieError, OiePayload};

/// Where the OIE keeps its own facts in the live root.
pub const PAYLOAD_FACTS: &str = "/usr/lib/ocinye/oie/payload.json";
/// Where casper mounts the medium.
pub const MEDIA: &str = "/cdrom";
/// curtin's target mount.
pub const TARGET: &str = "/target";

pub fn sha256_file(p: &Path) -> std::io::Result<(String, u64)> {
    use sha2::Digest as _;
    let mut f = fs::File::open(p)?;
    let mut h = sha2::Sha256::new();
    let mut buf = vec![0u8; 1 << 20];
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

/// A path on the medium, refusing anything that would leave it.
fn on_media(media: &Path, rel: &str) -> Result<PathBuf, OieError> {
    if rel.is_empty() || rel.starts_with('/') || rel.split('/').any(|c| c == ".." || c.is_empty()) {
        return Err(OieError::ImageManifestInvalid {
            field: "rootfs".into(),
        });
    }
    Ok(media.join(rel))
}

/// Step 1: the payload and the content manifest on the medium are the ones
/// this OIE was built with.
pub fn check_media(media: &Path, facts: &OiePayload) -> Result<ImageContentManifest, OieError> {
    let content_path = media.join("ocinye/IMAGE_CONTENT.json");
    let bytes = fs::read(&content_path).map_err(|_| OieError::PayloadDigestMismatch {
        path: "ocinye/IMAGE_CONTENT.json".into(),
    })?;
    let m = ImageContentManifest::parse(&bytes)
        .map_err(|e| OieError::ImageManifestInvalid { field: e.field })?;
    if Sha256Hex(m.sha256()) != facts.content_sha256 || m.image != facts.image {
        return Err(OieError::PayloadDigestMismatch {
            path: "ocinye/IMAGE_CONTENT.json".into(),
        });
    }
    let rootfs = on_media(media, &facts.rootfs)?;
    match sha256_file(&rootfs) {
        Ok((h, n)) if h == facts.rootfs_sha256.0 && n == facts.rootfs_bytes => Ok(m),
        _ => Err(OieError::PayloadDigestMismatch {
            path: facts.rootfs.clone(),
        }),
    }
}

/// The curtin configuration: GPT, ESP 1 GiB, ext4 on the rest, no swap, no
/// network configuration written, nothing fetched.
pub fn curtin_config(disk: &InstallationTargetDisk, rootfs: &Path) -> String {
    let esp_mib = layout::ESP_BYTES >> 20;
    // curtin sizes partitions in bytes (no "rest of the disk"): the root is
    // what is left after the 1 MiB alignment, the ESP and 1 MiB for the
    // backup GPT, in whole MiB.
    let root_mib = (disk.bytes >> 20).saturating_sub(1 + esp_mib + 1);
    format!(
        "\
# Rendered by ocinye-oie for {by_path} (confirmed with its token).
showtrace: true
install:
  unmount: disabled
  save_install_config: /root/curtin-install-cfg.yaml
  save_install_log: /root/curtin-install.log
storage:
  version: 1
  config:
    - {{id: disk0, type: disk, path: {by_path}, ptable: gpt, wipe: superblock-recursive, preserve: false, grub_device: false}}
    - {{id: esp, type: partition, device: disk0, size: {esp_mib}M, flag: boot, number: 1, grub_device: true}}
    - {{id: root, type: partition, device: disk0, size: {root_mib}M, number: 2}}
    - {{id: esp_fs, type: format, volume: esp, fstype: fat32, label: {esp_label}}}
    - {{id: root_fs, type: format, volume: root, fstype: ext4, label: {root_label}}}
    - {{id: root_mnt, type: mount, device: root_fs, path: /}}
    - {{id: esp_mnt, type: mount, device: esp_fs, path: /boot/efi}}
sources:
  - {{type: fsimage, uri: \"file://{rootfs}\"}}
swap:
  size: 0
network:
  config: disabled
apt:
  preserve_sources_list: true
grub:
  update_nvram: true
kernel:
  install: false
",
        by_path = disk.by_path,
        rootfs = rootfs.display(),
        esp_label = layout::ESP_LABEL,
        root_label = layout::ROOT_LABEL,
        root_mib = root_mib,
    )
}

/// Steps 2–3–5: curtin partitions, writes and installs the boot loader.
pub fn run_curtin(config: &Path) -> Result<(), OieError> {
    let out = Command::new("curtin")
        .args(["-v", "--showtrace", "install", "--config"])
        .arg(config)
        .env("TARGET_MOUNT_POINT", TARGET)
        .output()
        .map_err(|_| OieError::DiskWriteFailed {
            device: "curtin".into(),
        })?;
    let _ = fs::write(
        "/run/ocinye-oie/curtin.out",
        [&out.stdout[..], &out.stderr[..]].concat(),
    );
    if out.status.success() {
        return Ok(());
    }
    let text = String::from_utf8_lossy(&out.stderr).to_lowercase();
    if text.contains("grub") || text.contains("efibootmgr") || text.contains("shim") {
        Err(OieError::BootloaderInstallFailed)
    } else {
        Err(OieError::DiskWriteFailed {
            device: "target".into(),
        })
    }
}

/// Step 4: the release payload on the target is the one in the content
/// manifest, and the target carries no machine identity.
pub fn verify_target(target: &Path, m: &ImageContentManifest) -> Result<(), OieError> {
    let base = target
        .join(ocinye_image_contracts::paths::RELEASE_ROOT.trim_start_matches('/'))
        .join(&m.release_id);
    for f in &m.release_files {
        match sha256_file(&base.join(&f.path)) {
            Ok((h, n)) if h == f.sha256.0 && n == f.bytes => {}
            _ => {
                return Err(OieError::PayloadDigestMismatch {
                    path: f.path.clone(),
                })
            }
        }
    }
    let mid = fs::read_to_string(target.join("etc/machine-id")).unwrap_or_default();
    if !mid.trim().is_empty() {
        return Err(OieError::ImageManifestInvalid {
            field: "etc/machine-id".into(),
        });
    }
    let ssh = target.join("etc/ssh");
    if fs::read_dir(&ssh)
        .map(|d| {
            d.flatten()
                .any(|e| e.file_name().to_string_lossy().starts_with("ssh_host_"))
        })
        .unwrap_or(false)
    {
        return Err(OieError::ImageManifestInvalid {
            field: "etc/ssh/ssh_host_*".into(),
        });
    }
    Ok(())
}

/// `ocinye-operator.pub` on a removable volume: one public Ed25519/ECDSA key.
/// A private key, an RSA key or several keys are refused.
pub fn read_operator_key(text: &str) -> Option<PublicKeyLine> {
    if text.contains("PRIVATE KEY") {
        return None;
    }
    let lines: Vec<&str> = text
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .collect();
    let [line] = lines.as_slice() else {
        return None;
    };
    PublicKeyLine::parse(line)
        .filter(|k| k.algorithm == "ssh-ed25519" || k.algorithm.starts_with("ecdsa-"))
}

/// Look for the operator key on mounted removable volumes and on volumes the
/// OIE mounts read-only for the search.
pub fn find_operator_key(roots: &[PathBuf]) -> Option<(PathBuf, PublicKeyLine)> {
    roots.iter().find_map(|r| {
        let p = r.join("ocinye-operator.pub");
        let meta = fs::metadata(&p).ok()?;
        if meta.len() > 16 * 1024 {
            return None;
        }
        read_operator_key(&fs::read_to_string(&p).ok()?).map(|k| (p, k))
    })
}

/// Step 6: the operator key on `ocinye` (owner taken from the target's own
/// passwd), the journal, and nothing else.
pub fn prepare_first_boot(
    target: &Path,
    key: Option<&PublicKeyLine>,
    journal: &str,
) -> Result<Option<KeyFingerprint>, OieError> {
    let fail = || OieError::DiskWriteFailed {
        device: "target".into(),
    };
    let log = target.join("var/log/ocinye");
    fs::create_dir_all(&log).map_err(|_| fail())?;
    fs::write(log.join("oie-install.json"), journal).map_err(|_| fail())?;
    let Some(k) = key else { return Ok(None) };
    let ssh = target.join("home/ocinye/.ssh");
    fs::create_dir_all(&ssh).map_err(|_| fail())?;
    fs::write(ssh.join("authorized_keys"), format!("{}\n", k.to_line())).map_err(|_| fail())?;
    let st = Command::new("chroot")
        .arg(target)
        .args(["/bin/sh", "-c", "chown -R ocinye:ocinye /home/ocinye/.ssh && chmod 700 /home/ocinye/.ssh && chmod 600 /home/ocinye/.ssh/authorized_keys"])
        .status()
        .map_err(|_| fail())?;
    if !st.success() {
        return Err(fail());
    }
    Ok(Some(k.fingerprint()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ocinye_image_contracts::oie::{classify, ProbedDisk};

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("ocinye-oie-test-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    fn target_disk() -> InstallationTargetDisk {
        classify(&[ProbedDisk {
            device: "vda".into(),
            by_path: "/dev/disk/by-path/pci-0000:00:04.0".into(),
            bytes: 40_000_000_000,
            model: String::new(),
            serial: Some("OCY-TARGET-7F3A".into()),
            wwn: None,
            partitions: 0,
            filesystems: vec![],
            boot_media: false,
            mounted: false,
            read_only: false,
        }])
        .remove(0)
    }

    #[test]
    fn o_curtin_recebe_o_caminho_confirmado_e_o_esquema_fixo() {
        let c = curtin_config(
            &target_disk(),
            Path::new("/cdrom/payload/rootfs-metal.squashfs"),
        );
        assert!(c.contains("path: /dev/disk/by-path/pci-0000:00:04.0, ptable: gpt"));
        assert!(!c.contains("/dev/vda"), "never a kernel name");
        assert!(c.contains("size: 1024M, flag: boot"));
        // 40 GB disk = 38146 MiB; minus 1 + 1024 + 1.
        assert!(c.contains("size: 37120M, number: 2"), "{c}");
        assert!(c.contains("fstype: ext4") && c.contains("swap:\n  size: 0"));
        assert!(c.contains("network:\n  config: disabled"));
        assert!(c.contains("type: fsimage, uri: \"file:///cdrom/payload/rootfs-metal.squashfs\""));
    }

    #[test]
    fn suporte_alterado_e_recusado_antes_de_escrever() {
        let media = tmp("media");
        fs::create_dir_all(media.join("payload")).unwrap();
        fs::create_dir_all(media.join("ocinye")).unwrap();
        let m = ocinye_image_contracts::manifest::fixtures::content();
        fs::write(media.join("ocinye/IMAGE_CONTENT.json"), m.to_canonical()).unwrap();
        fs::write(media.join("payload/rootfs.squashfs"), b"rootfs bytes").unwrap();
        let (h, n) = sha256_file(&media.join("payload/rootfs.squashfs")).unwrap();
        let facts = OiePayload {
            image: m.image.clone(),
            rootfs: "payload/rootfs.squashfs".into(),
            rootfs_sha256: Sha256Hex(h),
            rootfs_bytes: n,
            content_sha256: Sha256Hex(m.sha256()),
            media_label: "OCINYE_OS".into(),
        };
        assert!(check_media(&media, &facts).is_ok());
        fs::write(media.join("payload/rootfs.squashfs"), b"rootfs bytez").unwrap();
        assert_eq!(
            check_media(&media, &facts).unwrap_err(),
            OieError::PayloadDigestMismatch {
                path: "payload/rootfs.squashfs".into()
            }
        );
        let mut bad = facts.clone();
        bad.rootfs = "../etc/shadow".into();
        assert!(matches!(
            check_media(&media, &bad),
            Err(OieError::PayloadDigestMismatch { .. } | OieError::ImageManifestInvalid { .. })
        ));
        fs::write(media.join("ocinye/IMAGE_CONTENT.json"), b"{}").unwrap();
        assert!(matches!(
            check_media(&media, &facts),
            Err(OieError::ImageManifestInvalid { .. })
        ));
    }

    #[test]
    fn so_uma_chave_publica_ed25519_ou_ecdsa() {
        use base64::Engine as _;
        let mut blob = 11u32.to_be_bytes().to_vec();
        blob.extend_from_slice(b"ssh-ed25519");
        blob.extend_from_slice(&[7u8; 36]);
        let line = format!(
            "ssh-ed25519 {} op@laptop",
            base64::engine::general_purpose::STANDARD.encode(&blob)
        );
        assert!(read_operator_key(&line).is_some());
        assert!(read_operator_key(&format!("# operator\n{line}\n")).is_some());
        assert!(
            read_operator_key(&format!("{line}\n{line}\n")).is_none(),
            "one key only"
        );
        assert!(read_operator_key("-----BEGIN OPENSSH PRIVATE KEY-----\nabc\n").is_none());
        assert!(read_operator_key("ssh-dss AAAA").is_none());
        assert!(read_operator_key("").is_none());
    }

    #[test]
    fn o_alvo_sem_identidade_e_com_o_release_certo() {
        let t = tmp("target");
        let mut m = ocinye_image_contracts::manifest::fixtures::content();
        let rel = t.join("usr/lib/ocinye/release").join(&m.release_id);
        fs::create_dir_all(&rel).unwrap();
        fs::write(rel.join("MANIFEST.json"), "{}").unwrap();
        let (h, n) = sha256_file(&rel.join("MANIFEST.json")).unwrap();
        m.release_files = vec![ocinye_image_contracts::manifest::ContentFile {
            path: "MANIFEST.json".into(),
            sha256: Sha256Hex(h),
            bytes: n,
        }];
        fs::create_dir_all(t.join("etc/ssh")).unwrap();
        fs::write(t.join("etc/machine-id"), "").unwrap();
        assert!(verify_target(&t, &m).is_ok());
        fs::write(t.join("etc/ssh/ssh_host_ed25519_key"), "x").unwrap();
        assert!(
            verify_target(&t, &m).is_err(),
            "a host key in the payload is a clone hazard"
        );
        fs::remove_file(t.join("etc/ssh/ssh_host_ed25519_key")).unwrap();
        fs::write(
            t.join("etc/machine-id"),
            "0123456789abcdef0123456789abcdef\n",
        )
        .unwrap();
        assert!(verify_target(&t, &m).is_err());
        fs::write(t.join("etc/machine-id"), "").unwrap();
        fs::write(rel.join("MANIFEST.json"), "{ }").unwrap();
        assert!(verify_target(&t, &m).is_err());
    }
}
