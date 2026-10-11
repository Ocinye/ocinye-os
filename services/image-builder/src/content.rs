//! B13: what the image contains, measured from the image itself — the
//! release files hashed on the mounted disk, the preloaded images as Docker
//! recorded them, the packages from the build's own inventory — and written
//! into it (`IMAGE_CONTENT.json`, `/etc/ocinye/image.json`).

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use ocinye_image_contracts::build::{
    ContentClass, ImageBuildError, InventoryPackage, PackageInventory, PackageOrigin,
};
use ocinye_image_contracts::firstboot::ImageFacts;
use ocinye_image_contracts::manifest::{
    Arch, ContentFile, ImageContentManifest, ImageFormat, ImageProfile, OciDigest, OciImageRef,
    OciRole, OcinyeImageVersion, Sha256Hex,
};
use serde::Deserialize;

use crate::cmd;
use crate::inputs::Bundle;
use crate::offline::Mounted;

fn mf(field: &str) -> ImageBuildError {
    ImageBuildError::ManifestFailed {
        field: field.into(),
    }
}

/// Installed package names of a root (its dpkg status).
pub fn installed_packages(root: &Path) -> BTreeSet<String> {
    let status = fs::read_to_string(root.join("var/lib/dpkg/status")).unwrap_or_default();
    let mut out = BTreeSet::new();
    for para in status.split("\n\n") {
        let field = |k: &str| para.lines().find_map(|l| l.strip_prefix(k)).map(str::trim);
        if field("Status:").is_some_and(|s| s.ends_with(" installed")) {
            if let Some(n) = field("Package:") {
                out.insert(n.to_owned());
            }
        }
    }
    out
}

/// Why a package not in the Ubuntu base is in the image.
fn class_of(name: &str, base: &BTreeSet<String>) -> (ContentClass, Option<String>) {
    if base.contains(name) {
        return (ContentClass::BaseOs, None);
    }
    let (c, r) = match name {
        n if ocinye_image_contracts::build::DOCKER_PACKAGES.contains(&n) => (
            ContentClass::OcinyeRuntimeRequired,
            "Docker runtime installed by D011 [P, PD-04], held",
        ),
        "ufw" => (
            ContentClass::OcinyeRuntimeRequired,
            "first-boot firewall (F6)",
        ),
        "linux-firmware" | "amd64-microcode" | "intel-microcode" => (
            ContentClass::InstallationRequired,
            "metal profile: hardware support",
        ),
        "efibootmgr" | "shim-signed" | "mokutil" => {
            (ContentClass::InstallationRequired, "UEFI boot")
        }
        "initramfs-tools" | "initramfs-tools-core" | "initramfs-tools-bin" => {
            (ContentClass::InstallationRequired, "initrd for the kernel")
        }
        n if n.starts_with("grub-") || n.starts_with("shim") => {
            (ContentClass::InstallationRequired, "UEFI boot")
        }
        n if n.starts_with("linux-") => {
            (ContentClass::InstallationRequired, "kernel for the profile")
        }
        "casper" | "curtin" | "dosfstools" | "gdisk" | "parted" | "rsync" | "eject" => (
            ContentClass::InstallationRequired,
            "installation environment",
        ),
        _ => (
            ContentClass::OcinyeRuntimeRequired,
            "dependency pulled by a package installed for Ocinye OS",
        ),
    };
    (c, Some(r.to_owned()))
}

/// `packages.tsv` written by `provision.sh finalize`:
/// name, version, arch, archive SHA-256 (or `-`), source URL and suite, held.
pub fn inventory(
    tsv: &str,
    base: &BTreeSet<String>,
    image: &OcinyeImageVersion,
    profile: ImageProfile,
    snapshot: &str,
) -> Result<PackageInventory, ImageBuildError> {
    let mut packages = vec![];
    for line in tsv.lines().filter(|l| !l.is_empty()) {
        let f: Vec<&str> = line.split('\t').collect();
        let [name, version, arch, sha, src, held] = f.as_slice() else {
            return Err(mf("packages.tsv"));
        };
        let sha = Sha256Hex((*sha).to_owned());
        if !sha.is_valid() {
            // A package the archive does not list (local or obsolete): the
            // inventory cannot vouch for it, so the build fails closed.
            return Err(mf(&format!("package_inventory:{name}")));
        }
        let origin = if src.contains("download.docker.com") {
            PackageOrigin::DockerCe
        } else if src.contains("-security") {
            PackageOrigin::UbuntuSecurity
        } else {
            PackageOrigin::UbuntuArchive
        };
        let (class, reason) = class_of(name, base);
        packages.push(InventoryPackage {
            name: (*name).into(),
            version: (*version).into(),
            architecture: (*arch).into(),
            origin,
            sha256: sha,
            class,
            reason,
            held: *held == "1",
        });
    }
    packages.sort_by(|a, b| (&a.name, &a.architecture).cmp(&(&b.name, &b.architecture)));
    packages.dedup_by(|a, b| a.name == b.name && a.architecture == b.architecture);
    let inv = PackageInventory {
        schema: 1,
        image: image.clone(),
        profile,
        apt_snapshot: snapshot.into(),
        packages,
    };
    if let Some(v) = inv.violations().into_iter().next() {
        return Err(mf(&format!(
            "package_inventory:{}",
            serde_json::to_string(&v).unwrap_or_default()
        )));
    }
    Ok(inv)
}

/// One line of `docker image ls --digests --no-trunc --format '{{json .}}'`.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct DockerLs {
    repository: String,
    tag: String,
    #[serde(default)]
    digest: String,
    #[serde(rename = "ID")]
    id: String,
}

/// The preloaded images: release images must have the ids the release
/// manifest says; third-party ones are named by their pinned digest.
pub fn oci_images(oci_json: &str, bundle: &Bundle) -> Result<Vec<OciImageRef>, ImageBuildError> {
    let rows: Vec<DockerLs> = oci_json
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(serde_json::from_str)
        .collect::<Result<_, _>>()
        .map_err(|_| mf("oci.json"))?;
    let id_of = |repo: &str, tag: &str| {
        rows.iter()
            .find(|r| r.repository == repo && r.tag == tag)
            .map(|r| r.id.clone())
    };
    let mut out = vec![];
    for img in &bundle.manifest.images {
        let id = id_of(&img.name, &img.tag).ok_or_else(|| ImageBuildError::OciDigestMismatch {
            name: img.name.clone(),
        })?;
        if id != img.image_id {
            return Err(ImageBuildError::OciDigestMismatch {
                name: img.name.clone(),
            });
        }
        out.push(OciImageRef {
            name: img.name.clone(),
            role: OciRole::Release,
            reference: format!("{}:{}", img.name, img.tag),
            digest: OciDigest(img.image_id.clone()),
            image_id: OciDigest(id),
        });
    }
    for t in &bundle.manifest.third_party_images {
        // By tag when the reference has one, else by the pinned digest (a
        // digest-only reference never has a tag in the store).
        let id = match &t.tag {
            Some(tag) => id_of(&t.repository, tag),
            None => rows
                .iter()
                .find(|r| r.repository == t.repository && r.digest == t.digest)
                .map(|r| r.id.clone()),
        }
        .ok_or_else(|| ImageBuildError::OciDigestMismatch {
            name: t.repository.clone(),
        })?;
        out.push(OciImageRef {
            name: t.repository.clone(),
            role: OciRole::ThirdParty,
            reference: t.reference.clone(),
            digest: OciDigest(t.digest.clone()),
            image_id: OciDigest(id),
        });
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}

/// Hash every file of the release payload as it is on the image.
pub fn release_files(m: &Mounted, release_id: &str) -> Result<Vec<ContentFile>, ImageBuildError> {
    fn walk(dir: &Path, base: &Path, out: &mut Vec<ContentFile>) -> std::io::Result<()> {
        for e in fs::read_dir(dir)? {
            let e = e?;
            let pth = e.path();
            if e.file_type()?.is_dir() {
                walk(&pth, base, out)?;
            } else {
                let (h, n) = cmd::sha256_file(&pth)?;
                let rel = pth
                    .strip_prefix(base)
                    .map_err(std::io::Error::other)?
                    .to_string_lossy()
                    .into_owned();
                out.push(ContentFile {
                    path: rel,
                    sha256: Sha256Hex(h),
                    bytes: n,
                });
            }
        }
        Ok(())
    }
    let base = m.path(&format!(
        "{}/{release_id}",
        ocinye_image_contracts::paths::RELEASE_ROOT
    ));
    let mut out = vec![];
    walk(&base, &base, &mut out).map_err(|_| mf("release_files"))?;
    out.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(out)
}

/// Write the embedded manifest and the facts into the mounted image.
pub fn embed(
    m: &Mounted,
    content: &ImageContentManifest,
    format: ImageFormat,
    ubuntu_serial: &str,
) -> Result<(), ImageBuildError> {
    content.validate().map_err(|e| mf(&e.field))?;
    let p = m.path(ocinye_image_contracts::paths::IMAGE_CONTENT);
    fs::create_dir_all(p.parent().ok_or_else(|| mf("path"))?)
        .map_err(|_| mf("IMAGE_CONTENT.json"))?;
    fs::write(&p, content.to_canonical()).map_err(|_| mf("IMAGE_CONTENT.json"))?;
    let facts = ImageFacts {
        image: content.image.clone(),
        profile: content.profile,
        release_id: content.release_id.clone(),
        content_sha256: content.sha256(),
        ubuntu_serial: ubuntu_serial.into(),
        source_format: format,
    };
    let fp = m.path(ocinye_image_contracts::paths::IMAGE_FACTS);
    fs::create_dir_all(fp.parent().ok_or_else(|| mf("path"))?).map_err(|_| mf("image.json"))?;
    let text = ocinye_installer_contracts::canonical::to_canonical(&facts)
        .map_err(|_| mf("image.json"))?;
    fs::write(&fp, text).map_err(|_| mf("image.json"))
}

/// Assemble the content manifest of one profile.
#[allow(clippy::too_many_arguments)]
pub fn content_manifest(
    image: &OcinyeImageVersion,
    profile: ImageProfile,
    arch: Arch,
    bundle: &Bundle,
    release_files: Vec<ContentFile>,
    oci_images: Vec<OciImageRef>,
    inventory_sha256: Sha256Hex,
) -> ImageContentManifest {
    ImageContentManifest {
        schema: ocinye_image_contracts::IMAGE_MANIFEST_SCHEMA,
        image: image.clone(),
        profile,
        architecture: arch,
        release_id: bundle.manifest.release.id.clone(),
        release_manifest_sha256: bundle.manifest_sha256.clone(),
        release_files,
        oci_images,
        package_inventory_sha256: inventory_sha256,
        firstboot_version: env!("CARGO_PKG_VERSION").into(),
        claim_protocol: ocinye_image_contracts::CLAIM_PROTOCOL,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ocinye_image_contracts::manifest::fixtures::{h, version};

    #[test]
    fn inventario_classifica_e_falha_fechado() {
        let base: BTreeSet<String> = ["systemd".to_owned(), "openssh-server".to_owned()].into();
        let tsv = format!(
            "systemd\t255.4\tamd64\t{a}\thttp://snapshot.ubuntu.com/ubuntu/x noble-updates/main\t0\n\
             docker-ce\t5:29.1\tamd64\t{b}\thttps://download.docker.com/linux/ubuntu noble/stable\t1\n\
             ufw\t0.36\tall\t{c}\thttp://snapshot.ubuntu.com/ubuntu/x noble/main\t0\n",
            a = h('a').0,
            b = h('b').0,
            c = h('c').0
        );
        let inv = inventory(
            &tsv,
            &base,
            &version(),
            ImageProfile::Virt,
            "20261001T000000Z",
        )
        .unwrap();
        assert_eq!(
            inv.packages
                .iter()
                .map(|p| p.name.as_str())
                .collect::<Vec<_>>(),
            ["docker-ce", "systemd", "ufw"]
        );
        let d = &inv.packages[0];
        assert_eq!(
            (d.origin, d.class, d.held),
            (
                PackageOrigin::DockerCe,
                ContentClass::OcinyeRuntimeRequired,
                true
            )
        );
        assert_eq!(inv.packages[1].class, ContentClass::BaseOs);
        // Docker not held: refused.
        let unheld = tsv.replace("noble/stable\t1", "noble/stable\t0");
        assert!(inventory(&unheld, &base, &version(), ImageProfile::Virt, "x").is_err());
        // A package without an archive digest: refused.
        let nodigest = format!("{tsv}mystery\t1\tamd64\t-\t-\t0\n");
        assert_eq!(
            inventory(&nodigest, &base, &version(), ImageProfile::Virt, "x").unwrap_err(),
            mf("package_inventory:mystery")
        );
        // A build tool left behind: refused.
        let tool = format!(
            "{tsv}xorriso\t1\tamd64\t{}\thttp://x noble/main\t0\n",
            h('d').0
        );
        assert!(inventory(&tool, &base, &version(), ImageProfile::Virt, "x").is_err());
    }

    #[test]
    fn estado_do_dpkg() {
        let d = std::env::temp_dir().join(format!("ocinye-dpkg-{}", std::process::id()));
        fs::create_dir_all(d.join("var/lib/dpkg")).unwrap();
        fs::write(d.join("var/lib/dpkg/status"), "Package: a\nStatus: install ok installed\n\nPackage: b\nStatus: deinstall ok config-files\n\nPackage: c\nStatus: hold ok installed\n").unwrap();
        assert_eq!(
            installed_packages(&d),
            ["a".to_owned(), "c".to_owned()].into()
        );
    }
}
