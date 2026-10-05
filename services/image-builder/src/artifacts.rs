//! B15: the published artifacts, all from the same assembled disks.
//!
//! QCOW2 and RAW from the virt disk; the ISO carries the metal root
//! filesystem as a squashfs (curtin `fsimage` source) next to the OIE live
//! root, with a UEFI-only El Torito image and a GPT-appended ESP so the same
//! file boots from a virtual CD, BMC virtual media or a USB stick.

use std::fs;
use std::path::{Path, PathBuf};

use ocinye_image_contracts::build::ImageBuildError;
use ocinye_image_contracts::manifest::{Arch, ImageFormat, OcinyeImageVersion, Sha256Hex};
use ocinye_image_contracts::oie::OiePayload;

use crate::cmd::{self, p};
use crate::offline::Mounted;

fn art(format: ImageFormat) -> impl Fn() -> ImageBuildError {
    move || ImageBuildError::ArtifactAssemblyFailed { format }
}

/// The ISO volume label (≤ 32, upper case): protects the medium in the OIE.
pub const MEDIA_LABEL: &str = "OCINYE_OS";

pub fn virtual_size(step: &str, disk: &Path) -> Result<u64, ImageBuildError> {
    let out = cmd::output(
        step,
        "qemu-img",
        &["info", "--output", "json", p(disk)],
        art(ImageFormat::Qcow2),
    )?;
    let v: serde_json::Value =
        serde_json::from_slice(&out).map_err(|_| art(ImageFormat::Qcow2)())?;
    v["virtual-size"]
        .as_u64()
        .ok_or_else(art(ImageFormat::Qcow2))
}

/// QCOW2: the virt disk, flattened and compressed.
pub fn qcow2(virt: &Path, out: &Path) -> Result<u64, ImageBuildError> {
    cmd::run(
        "B15",
        "qemu-img",
        &["convert", "-c", "-O", "qcow2", p(virt), p(out)],
        art(ImageFormat::Qcow2),
    )?;
    virtual_size("B15", out)
}

/// RAW: the uncompressed `.raw` (digest recorded) and its `.raw.zst` transport.
pub fn raw(
    virt: &Path,
    raw: &Path,
    zst: &Path,
    level: u32,
) -> Result<(Sha256Hex, u64), ImageBuildError> {
    cmd::run(
        "B15",
        "qemu-img",
        &["convert", "-O", "raw", p(virt), p(raw)],
        art(ImageFormat::Raw),
    )?;
    let (h, n) = cmd::sha256_file(raw).map_err(|_| art(ImageFormat::Raw)())?;
    let lvl = format!("-{level}");
    cmd::run(
        "B15",
        "zstd",
        &["-q", "-f", "-T0", &lvl, "--long=27", p(raw), "-o", p(zst)],
        art(ImageFormat::Raw),
    )?;
    Ok((Sha256Hex(h), n))
}

/// A mounted root filesystem → squashfs (xattrs and device nodes kept: the
/// Docker image store needs both). `extra` adds files by pseudo-definition.
pub fn squashfs(root: &Path, out: &Path, extra: &[(&str, &Path)]) -> Result<(), ImageBuildError> {
    let _ = fs::remove_file(out);
    let mut args: Vec<String> = vec![
        p(root).into(),
        p(out).into(),
        "-noappend".into(),
        "-comp".into(),
        "zstd".into(),
        "-xattrs".into(),
        "-quiet".into(),
        "-e".into(),
        "boot/efi".into(),
    ];
    for (dest, src) in extra {
        let mut dirs = vec![];
        let mut cur = Path::new(dest).parent();
        while let Some(d) = cur.filter(|d| !d.as_os_str().is_empty()) {
            dirs.push(d.to_path_buf());
            cur = d.parent();
        }
        for d in dirs.iter().rev() {
            if !root.join(d).exists() {
                args.extend(["-p".into(), format!("{} d 755 0 0", d.display())]);
            }
        }
        args.extend([
            "-p".into(),
            format!("{dest} f 444 0 0 cat {}", src.display()),
        ]);
    }
    let refs: Vec<&str> = args.iter().map(String::as_str).collect();
    cmd::run("B15", "mksquashfs", &refs, art(ImageFormat::Iso))
}

struct EfiFiles {
    shim: PathBuf,
    grub: PathBuf,
    mm: Option<PathBuf>,
    boot_name: &'static str,
    grub_name: &'static str,
    mm_name: &'static str,
}

fn efi_files(root: &Path, arch: Arch) -> Result<EfiFiles, ImageBuildError> {
    let first = |c: &[&str]| c.iter().map(|x| root.join(x)).find(|x| x.exists());
    let (shim, grub, mm, boot_name, grub_name, mm_name) = match arch {
        Arch::Amd64 => (
            first(&[
                "usr/lib/shim/shimx64.efi.signed.latest",
                "usr/lib/shim/shimx64.efi.signed",
            ]),
            first(&["usr/lib/grub/x86_64-efi-signed/gcdx64.efi.signed"]),
            first(&["usr/lib/shim/mmx64.efi"]),
            "BOOTX64.EFI",
            "grubx64.efi",
            "mmx64.efi",
        ),
        Arch::Arm64 => (
            first(&[
                "usr/lib/shim/shimaa64.efi.signed.latest",
                "usr/lib/shim/shimaa64.efi.signed",
            ]),
            first(&["usr/lib/grub/arm64-efi-signed/gcdaa64.efi.signed"]),
            first(&["usr/lib/shim/mmaa64.efi"]),
            "BOOTAA64.EFI",
            "grubaa64.efi",
            "mmaa64.efi",
        ),
    };
    Ok(EfiFiles {
        shim: shim.ok_or_else(art(ImageFormat::Iso))?,
        grub: grub.ok_or_else(art(ImageFormat::Iso))?,
        mm,
        boot_name,
        grub_name,
        mm_name,
    })
}

/// The newest kernel in `boot` that has its initrd (the `vmlinuz` and
/// `initrd.img` links are not always both there, and they are absolute or
/// relative depending on the package that wrote them).
pub fn kernel_pair(boot: &Path) -> Option<(PathBuf, PathBuf)> {
    let mut versions: Vec<String> = fs::read_dir(boot)
        .ok()?
        .flatten()
        .filter_map(|e| {
            e.file_name()
                .to_str()?
                .strip_prefix("vmlinuz-")
                .map(str::to_owned)
        })
        .filter(|v| boot.join(format!("initrd.img-{v}")).is_file())
        .collect();
    // Numeric order: 6.8.0-146 is newer than 6.8.0-99.
    versions.sort_by_key(|v| {
        v.split(|c: char| !c.is_ascii_digit())
            .filter_map(|n| n.parse::<u64>().ok())
            .collect::<Vec<_>>()
    });
    let v = versions.pop()?;
    Some((
        boot.join(format!("vmlinuz-{v}")),
        boot.join(format!("initrd.img-{v}")),
    ))
}

/// GRUB menu: the installer, and the firmware settings. Serial and screen.
pub fn grub_cfg(image: &OcinyeImageVersion, arch: Arch) -> String {
    let serial = match arch {
        Arch::Amd64 => "ttyS0",
        Arch::Arm64 => "ttyAMA0",
    };
    let dev = if image.build_kind == ocinye_image_contracts::manifest::BuildKind::Development {
        " · DESENVOLVIMENTO"
    } else {
        ""
    };
    format!(
        "set timeout=10\nset default=0\ninsmod all_video\nserial --unit=0 --speed=115200\nterminal_input console serial\nterminal_output console serial\n\
menuentry \"Instalar Ocinye OS · {name}{dev}\" {{\n  linux /casper/vmlinuz boot=casper noprompt fsck.mode=skip console=tty0 console={serial},115200n8 ---\n  initrd /casper/initrd\n}}\n\
menuentry \"Firmware (UEFI)\" {{\n  fwsetup\n}}\n",
        name = image.name()
    )
}

/// The ISO. `oie` and `metal` are the mounted, finalized disks.
#[allow(clippy::too_many_arguments)]
pub fn iso(
    work: &Path,
    out: &Path,
    arch: Arch,
    image: &OcinyeImageVersion,
    oie: &Mounted,
    metal: &Mounted,
    content_json: &Path,
) -> Result<OiePayload, ImageBuildError> {
    let f = art(ImageFormat::Iso);
    let tree = work.join("iso");
    let _ = fs::remove_dir_all(&tree);
    for d in [".disk", "boot/grub", "casper", "payload", "ocinye"] {
        fs::create_dir_all(tree.join(d)).map_err(|_| f())?;
    }
    // The installed system: the metal root filesystem (curtin fsimage).
    let rootfs = tree.join("payload/rootfs-metal.squashfs");
    squashfs(&metal.mnt, &rootfs, &[])?;
    let (rh, rn) = cmd::sha256_file(&rootfs).map_err(|_| f())?;
    fs::copy(content_json, tree.join("ocinye/IMAGE_CONTENT.json")).map_err(|_| f())?;
    let content_sha = cmd::sha256_file(content_json).map_err(|_| f())?.0;
    // What the OIE checks the medium against, inside its own root.
    let facts = OiePayload {
        image: image.clone(),
        rootfs: "payload/rootfs-metal.squashfs".into(),
        rootfs_sha256: Sha256Hex(rh),
        rootfs_bytes: rn,
        content_sha256: Sha256Hex(content_sha),
        media_label: MEDIA_LABEL.into(),
    };
    let facts_path = work.join("payload.json");
    fs::write(
        &facts_path,
        ocinye_installer_contracts::canonical::to_canonical(&facts).map_err(|_| f())?,
    )
    .map_err(|_| f())?;
    squashfs(
        &oie.mnt,
        &tree.join("casper/filesystem.squashfs"),
        &[("usr/lib/ocinye/oie/payload.json", &facts_path)],
    )?;
    let (kernel, initrd) = kernel_pair(&oie.mnt.join("boot")).ok_or_else(&f)?;
    fs::copy(&kernel, tree.join("casper/vmlinuz")).map_err(|_| f())?;
    fs::copy(&initrd, tree.join("casper/initrd")).map_err(|_| f())?;
    fs::write(
        tree.join(".disk/info"),
        format!("Ocinye OS {} {}\n", image.name(), arch.as_str()),
    )
    .map_err(|_| f())?;
    fs::write(tree.join("boot/grub/grub.cfg"), grub_cfg(image, arch)).map_err(|_| f())?;
    // The ESP: Ubuntu-signed shim and the CD GRUB, which finds /.disk/info
    // and reads /boot/grub/grub.cfg from the ISO filesystem.
    let e = efi_files(&oie.mnt, arch)?;
    let efi = work.join("efi.img");
    let _ = fs::remove_file(&efi);
    cmd::run(
        "B15",
        "mkfs.vfat",
        &["-C", "-n", "OCINYE_ESP", p(&efi), "8192"],
        art(ImageFormat::Iso),
    )?;
    cmd::run(
        "B15",
        "mmd",
        &["-i", p(&efi), "::/EFI", "::/EFI/BOOT"],
        art(ImageFormat::Iso),
    )?;
    cmd::run(
        "B15",
        "mcopy",
        &[
            "-i",
            p(&efi),
            p(&e.shim),
            &format!("::/EFI/BOOT/{}", e.boot_name),
        ],
        art(ImageFormat::Iso),
    )?;
    cmd::run(
        "B15",
        "mcopy",
        &[
            "-i",
            p(&efi),
            p(&e.grub),
            &format!("::/EFI/BOOT/{}", e.grub_name),
        ],
        art(ImageFormat::Iso),
    )?;
    if let Some(mm) = &e.mm {
        cmd::run(
            "B15",
            "mcopy",
            &["-i", p(&efi), p(mm), &format!("::/EFI/BOOT/{}", e.mm_name)],
            art(ImageFormat::Iso),
        )?;
    }
    let _ = fs::remove_file(out);
    cmd::run(
        "B15",
        "xorriso",
        &[
            "-as",
            "mkisofs",
            "-r",
            "-V",
            MEDIA_LABEL,
            "-J",
            "-joliet-long",
            "-l",
            "-iso-level",
            "3",
            "-partition_offset",
            "16",
            "-append_partition",
            "2",
            "0xef",
            p(&efi),
            "-appended_part_as_gpt",
            "-c",
            "/boot.catalog",
            "-e",
            "--interval:appended_partition_2:all::",
            "-no-emul-boot",
            "-o",
            p(out),
            p(&tree),
        ],
        art(ImageFormat::Iso),
    )?;
    // The tree is a copy of what the ISO now holds: give the space back.
    let _ = fs::remove_dir_all(&tree);
    Ok(facts)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ocinye_image_contracts::manifest::fixtures::version;

    #[test]
    fn o_nucleo_mais_recente_com_initrd() {
        let d = std::env::temp_dir().join(format!("ocinye-boot-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        for f in [
            "vmlinuz-6.8.0-100-generic",
            "initrd.img-6.8.0-100-generic",
            "vmlinuz-6.8.0-146-generic",
            "initrd.img-6.8.0-146-generic",
            "vmlinuz-6.8.0-150-generic",
        ] {
            fs::write(d.join(f), "").unwrap();
        }
        let (k, i) = kernel_pair(&d).unwrap();
        assert!(
            k.ends_with("vmlinuz-6.8.0-146-generic") && i.ends_with("initrd.img-6.8.0-146-generic")
        );
    }

    #[test]
    fn menu_tem_consola_serie_e_marca_desenvolvimento() {
        let g = grub_cfg(&version(), Arch::Amd64);
        assert!(g.contains("console=ttyS0,115200n8") && g.contains("boot=casper"));
        assert!(g.contains("DESENVOLVIMENTO"));
        assert!(grub_cfg(&version(), Arch::Arm64).contains("console=ttyAMA0,115200n8"));
    }
}
