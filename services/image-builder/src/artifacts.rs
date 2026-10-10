//! B15: the published artifacts, all from the same assembled disks.
//!
//! QCOW2 and RAW from the virt disk; the ISO carries the metal root
//! filesystem as a squashfs (curtin `fsimage` source) next to the OIE live
//! root, with a UEFI-only El Torito image and a GPT-appended ESP so the same
//! file boots from a virtual CD, BMC virtual media or a USB stick.

use std::fs;
use std::path::{Path, PathBuf};

use ocinye_image_contracts::bootmode::{
    MediaMode, MenuAction, MenuEntry, BOOT_MENU, LANG_PARAM, SELFTEST_PARAM, SELFTEST_POLICY_FAIL,
};
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
    // `-e` takes every argument after it as an exclude: it goes last.
    args.extend(["-e".into(), "boot/efi".into()]);
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

/// What the verified Phase A install entry passes to the kernel before its
/// consoles. Every entry starts with it; the install entry adds only its mode
/// and language.
pub const BASE_KERNEL_ARGS: &str = "boot=casper noprompt fsck.mode=skip";

/// ASCII for the firmware console and a plain serial terminal.
fn fold(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            'á' | 'à' | 'â' | 'ã' => 'a',
            'é' | 'ê' | 'è' => 'e',
            'É' => 'E',
            'í' => 'i',
            'ó' | 'ô' | 'õ' => 'o',
            'ú' => 'u',
            'ç' => 'c',
            c if c.is_ascii() => c,
            _ => '?',
        })
        .collect()
}

/// The kernel line of one mode.
pub fn kernel_args(mode: MediaMode, lang: &str, serial: &str, extra: &[&str]) -> String {
    let mut parts: Vec<String> = vec![
        BASE_KERNEL_ARGS.to_owned(),
        mode.kernel_arg(),
        format!("{LANG_PARAM}={lang}"),
    ];
    parts.extend(
        mode.contract()
            .extra_kernel_params
            .iter()
            .map(|p| (*p).to_owned()),
    );
    parts.extend(extra.iter().map(|p| (*p).to_owned()));
    parts.push(format!("console=tty0 console={serial},115200n8 ---"));
    parts.join(" ")
}

fn menu_entries(
    out: &mut String,
    entries: &[MenuEntry],
    lang: (usize, &str),
    serial: &str,
    indent: &str,
    dev: bool,
) {
    for e in entries {
        let label = fold(e.label[lang.0]);
        let key = e.hotkey;
        match e.action {
            MenuAction::Boot(mode) => {
                let args = kernel_args(mode, lang.1, serial, &[]);
                out.push_str(&format!(
                    "{indent}menuentry \"{label}\" --hotkey={key} --id=ocinye-{id} {{\n{indent}  linux /casper/vmlinuz {args}\n{indent}  initrd /casper/initrd\n{indent}}}\n",
                    id = mode.contract().param_value
                ));
            }
            MenuAction::AdvancedSubmenu => {
                out.push_str(&format!("{indent}submenu \"{label}\" --hotkey={key} {{\n"));
                let inner = format!("{indent}  ");
                menu_entries(out, BOOT_MENU.advanced, lang, serial, &inner, dev);
                if dev {
                    // Development builds only: a Live session whose storage
                    // verifier is told to fail. It can only restrict.
                    let args = kernel_args(
                        MediaMode::Live,
                        lang.1,
                        serial,
                        &[&format!("{SELFTEST_PARAM}={SELFTEST_POLICY_FAIL}")],
                    );
                    out.push_str(&format!(
                        "{inner}menuentry \"Self-test: restricted session (development)\" --hotkey=s --id=ocinye-selftest-restricted {{\n{inner}  linux /casper/vmlinuz {args}\n{inner}  initrd /casper/initrd\n{inner}}}\n"
                    ));
                }
                out.push_str(&format!("{indent}}}\n"));
            }
            MenuAction::LanguageSubmenu => {
                out.push_str(&format!("{indent}submenu \"{label}\" --hotkey={key} {{\n"));
                for (code, name) in [("pt", "Portugues"), ("en", "English"), ("fr", "Francais")] {
                    out.push_str(&format!(
                        "{indent}  menuentry \"{name}\" {{\n{indent}    set ocinye_lang={code}\n{indent}    export ocinye_lang\n{indent}    configfile /boot/grub/grub.cfg\n{indent}  }}\n"
                    ));
                }
                out.push_str(&format!("{indent}}}\n"));
            }
            MenuAction::Restart => out.push_str(&format!(
                "{indent}menuentry \"{label}\" --hotkey={key} {{\n{indent}  reboot\n{indent}}}\n"
            )),
            MenuAction::PowerOff => out.push_str(&format!(
                "{indent}menuentry \"{label}\" --hotkey={key} {{\n{indent}  halt\n{indent}}}\n"
            )),
            MenuAction::FirmwareSetup => out.push_str(&format!(
                "{indent}menuentry \"{label}\" --hotkey={key} {{\n{indent}  fwsetup\n{indent}}}\n"
            )),
        }
    }
}

/// GRUB menu, generated from the boot-mode contract
/// (`ocinye_image_contracts::bootmode::BOOT_MENU`). Serial and screen.
///
/// It waits: `timeout=-1` means no countdown, and `default` only says which
/// entry is highlighted first. Nothing boots until the operator presses Enter
/// or a hotkey.
pub fn grub_cfg(image: &OcinyeImageVersion, arch: Arch) -> String {
    let serial = match arch {
        Arch::Amd64 => "ttyS0",
        Arch::Arm64 => "ttyAMA0",
    };
    let dev = image.build_kind == ocinye_image_contracts::manifest::BuildKind::Development;
    // The contract has no timeout to copy and no default to boot; if that ever
    // changed, this refuses to produce a menu rather than quietly follow.
    assert!(
        BOOT_MENU.timeout_seconds.is_none() && !BOOT_MENU.automatic_default_boot,
        "the boot menu must wait for the operator"
    );
    let mut out = format!(
        "set timeout=-1\nset default={focus}\ninsmod all_video\nserial --unit=0 --speed=115200\nterminal_input console serial\nterminal_output console serial\n\
if [ -z \"$ocinye_lang\" ]; then set ocinye_lang=pt; fi\nexport ocinye_lang\n",
        focus = BOOT_MENU.initial_focus
    );
    let marker = if dev { " - DESENVOLVIMENTO" } else { "" };
    for (i, (code, cond)) in [("en", "if"), ("fr", "elif"), ("pt", "else")]
        .into_iter()
        .enumerate()
    {
        let idx = match code {
            "en" => 1,
            "fr" => 2,
            _ => 0,
        };
        if i < 2 {
            out.push_str(&format!("{cond} [ \"$ocinye_lang\" = \"{code}\" ]; then\n"));
        } else {
            out.push_str("else\n");
        }
        menu_entries(&mut out, BOOT_MENU.entries, (idx, code), serial, "  ", dev);
    }
    out.push_str("fi\n");
    // Says which image this is; selecting it does nothing.
    out.push_str(&format!(
        "menuentry \"-- {name}{marker} --\" {{\n  true\n}}\n",
        name = image.name()
    ));
    out
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
    // The OIE checks the medium against these facts: they must be inside.
    let listing = cmd::output(
        "B15",
        "unsquashfs",
        &[
            "-l",
            p(&tree.join("casper/filesystem.squashfs")),
            "usr/lib/ocinye/oie/payload.json",
        ],
        art(ImageFormat::Iso),
    )?;
    if !String::from_utf8_lossy(&listing).contains("usr/lib/ocinye/oie/payload.json") {
        return Err(f());
    }
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
    // xorriso writes inside the build VM, then one sequential copy out
    // (random writes onto the shared output folder are very slow).
    let local = work.join("image.iso");
    let _ = fs::remove_file(&local);
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
            p(&local),
            p(&tree),
        ],
        art(ImageFormat::Iso),
    )?;
    // The tree is a copy of what the ISO now holds: give the space back.
    let _ = fs::remove_dir_all(&tree);
    fs::copy(&local, out).map_err(|_| f())?;
    let _ = fs::remove_file(&local);
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

    fn linux_lines(g: &str) -> Vec<&str> {
        g.lines()
            .map(str::trim)
            .filter_map(|l| l.strip_prefix("linux /casper/vmlinuz "))
            .collect()
    }

    fn mode_of(line: &str) -> Vec<&str> {
        line.split(' ')
            .filter_map(|a| a.strip_prefix("ocinye.mode="))
            .collect()
    }

    #[test]
    fn o_menu_espera_e_nenhum_modo_arranca_por_tempo() {
        for arch in [Arch::Amd64, Arch::Arm64] {
            let g = grub_cfg(&version(), arch);
            // No countdown anywhere, and exactly one timeout setting.
            let timeouts: Vec<&str> = g.lines().filter(|l| l.contains("timeout")).collect();
            assert_eq!(timeouts, vec!["set timeout=-1"], "{g}");
            // The first entry of every language block, the one highlighted
            // first, is the Live session, never the installer.
            let first_ids: Vec<&str> = g
                .split("then\n")
                .chain(g.split("else\n").skip(1))
                .skip(1)
                .filter_map(|block| {
                    block
                        .lines()
                        .find(|l| l.trim_start().starts_with("menuentry "))
                })
                .collect();
            assert!(first_ids.len() >= 3, "{g}");
            for l in first_ids {
                assert!(l.contains("--id=ocinye-live"), "{l}");
            }
            assert!(g.starts_with("set timeout=-1\nset default=0\n"));
        }
    }

    #[test]
    fn cada_entrada_de_arranque_tem_exactamente_um_modo() {
        let g = grub_cfg(&version(), Arch::Amd64);
        let lines = linux_lines(&g);
        // 3 languages x (live, install, hardware-check, recovery, self-test).
        assert_eq!(lines.len(), 15, "{g}");
        for l in &lines {
            let modes = mode_of(l);
            assert_eq!(modes.len(), 1, "{l}");
            assert!(
                ["live", "install", "hardware-check", "recovery"].contains(&modes[0]),
                "{l}"
            );
            // And the contract reads the line the same way.
            let resolved = ocinye_image_contracts::bootmode::ResolvedMode::from_cmdline(l);
            assert!(resolved.is_explicit(), "{l}");
            assert_eq!(resolved.mode().contract().param_value, modes[0], "{l}");
        }
    }

    #[test]
    fn a_linha_de_instalacao_e_a_verificada_mais_o_modo_e_a_lingua() {
        for (arch, serial) in [(Arch::Amd64, "ttyS0"), (Arch::Arm64, "ttyAMA0")] {
            let g = grub_cfg(&version(), arch);
            let install: Vec<&str> = linux_lines(&g)
                .into_iter()
                .filter(|l| mode_of(l) == ["install"])
                .collect();
            assert_eq!(install.len(), 3);
            for (l, lang) in install.iter().zip(["en", "fr", "pt"]) {
                // Phase A booted: boot=casper noprompt fsck.mode=skip
                // console=tty0 console=<serial>,115200n8 ---
                assert_eq!(
                    *l,
                    format!("boot=casper noprompt fsck.mode=skip ocinye.mode=install ocinye.lang={lang} console=tty0 console={serial},115200n8 ---")
                );
            }
        }
    }

    #[test]
    fn os_modos_nao_destrutivos_levam_todos_os_parametros_de_seguranca() {
        let g = grub_cfg(&version(), Arch::Amd64);
        for l in linux_lines(&g) {
            let install = mode_of(l) == ["install"];
            for p in ocinye_image_contracts::bootmode::SAFE_KERNEL_PARAMS {
                assert_eq!(l.split(' ').any(|a| a == *p), !install, "{p} in {l}");
            }
            // The self-test parameter only ever rides on a Live line.
            if l.contains("ocinye.selftest=") {
                assert_eq!(mode_of(l), ["live"], "{l}");
            }
        }
    }

    #[test]
    fn o_menu_tem_as_sete_escolhas_e_a_recuperacao_nao_instala() {
        let g = grub_cfg(&version(), Arch::Amd64);
        for needle in [
            "\"Experimentar o Ocinye OS\" --hotkey=t",
            "\"Try Ocinye OS\" --hotkey=t",
            "\"Essayer Ocinye OS\" --hotkey=t",
            "\"Install Ocinye OS\" --hotkey=i",
            "\"Hardware Check\" --hotkey=h",
            "submenu \"Advanced / Recovery\" --hotkey=a",
            "submenu \"Language\" --hotkey=l",
            "\"Restart\" --hotkey=r",
            "\"Power Off\" --hotkey=p",
            "reboot",
            "halt",
            "fwsetup",
        ] {
            assert!(g.contains(needle), "{needle}\n{g}");
        }
        // Labels are ASCII: the firmware console and a serial line render them.
        assert!(g.is_ascii(), "{g}");
        // Nothing under Advanced / Recovery is the installer.
        for block in g.split("submenu \"").skip(1) {
            let body = block.split("\n  }\n").next().unwrap_or_default();
            if block.starts_with("Adv") || block.starts_with("Ava") {
                assert!(!body.contains("ocinye.mode=install"), "{body}");
            }
        }
    }

    #[test]
    fn o_auto_teste_so_existe_em_construcoes_de_desenvolvimento() {
        let mut v = version();
        assert!(grub_cfg(&v, Arch::Amd64).contains("ocinye.selftest=policy-fail"));
        v.build_kind = ocinye_image_contracts::manifest::BuildKind::Release;
        let g = grub_cfg(&v, Arch::Amd64);
        assert!(!g.contains("ocinye.selftest"), "{g}");
        assert!(!g.contains("DESENVOLVIMENTO"));
        assert_eq!(linux_lines(&g).len(), 12);
    }

    fn guard_file(rel: &str) -> String {
        let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../infra/image/oie-rootfs")
            .join(rel);
        fs::read_to_string(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
    }

    /// Run the real initramfs script with `/proc/cmdline` and `/run` pointed
    /// at a scratch directory; returns whether it armed.
    fn guard_script_arms(case: usize, cmdline: &str) -> bool {
        let d = std::env::temp_dir().join(format!("ocinye-guard-{}-{case}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        fs::write(d.join("cmdline"), format!("{cmdline}\n")).unwrap();
        let script = guard_file("usr/share/initramfs-tools/scripts/init-top/ocinye-blockguard")
            .replace("/proc/cmdline", &d.join("cmdline").display().to_string())
            .replace("/run/", &format!("{}/run/", d.display()));
        fs::write(d.join("guard.sh"), script).unwrap();
        let st = std::process::Command::new("sh")
            .arg(d.join("guard.sh"))
            .status()
            .unwrap();
        assert!(st.success(), "{cmdline}");
        let marker = d.join("run/ocinye/guard");
        let armed = fs::read_to_string(&marker).is_ok_and(|s| s.trim() == "armed");
        if armed {
            // Armed means RAID assembly and LVM activation are masked too.
            for mask in ["64-md-raid-assembly.rules", "69-lvm.rules"] {
                let m = d.join("run/udev/rules.d").join(mask);
                assert!(fs::metadata(&m).is_ok_and(|x| x.len() == 0), "{mask}");
            }
        }
        let _ = fs::remove_dir_all(&d);
        armed
    }

    #[test]
    fn a_guarda_do_initramfs_arma_como_o_contrato_manda() {
        use ocinye_image_contracts::bootmode::guard_must_arm;
        let cases = [
            // The four menu lines, as generated.
            kernel_args(MediaMode::Live, "pt", "ttyS0", &[]),
            kernel_args(MediaMode::Install, "pt", "ttyS0", &[]),
            kernel_args(MediaMode::HardwareCheck, "en", "ttyS0", &[]),
            kernel_args(MediaMode::Recovery, "fr", "ttyS0", &[]),
            // The frozen Phase A line: no mode at all.
            "boot=casper noprompt fsck.mode=skip console=tty0 console=ttyS0,115200n8 ---".into(),
            "boot=casper ocinye.mode=".into(),
            "boot=casper ocinye.mode=INSTALL".into(),
            "boot=casper ocinye.mode=installer".into(),
            "boot=casper ocinye.mode=install ocinye.mode=live".into(),
            "boot=casper ocinye.mode=live ocinye.mode=install".into(),
            "boot=casper ocinye.mode=install ocinye.mode=install".into(),
            "boot=casper xocinye.mode=install".into(),
            "boot=casper --- ocinye.mode=install".into(),
            "boot=casper -- ocinye.mode=install".into(),
            "boot=casper ocinye.mode=install ocinye.selftest=policy-fail".into(),
            // Not the medium: an installed system never arms, whatever it says.
            "root=PARTUUID=1234 ro console=tty0".into(),
            "root=PARTUUID=1234 ro ocinye.mode=live".into(),
            "--- boot=casper".into(),
        ];
        let mut armed_count = 0;
        for (i, c) in cases.iter().enumerate() {
            let expected = guard_must_arm(c);
            assert_eq!(guard_script_arms(i, c), expected, "{c}");
            armed_count += usize::from(expected);
        }
        // Both outcomes were exercised: a script that always or never armed
        // would not pass by agreeing with itself.
        assert!(
            armed_count >= 9 && armed_count <= cases.len() - 4,
            "{armed_count}"
        );
        // Exactly the explicit installer on the medium is left unarmed.
        assert!(!guard_must_arm(&cases[1]));
        assert!(guard_must_arm(&cases[4]));
    }

    #[test]
    fn a_guarda_fala_do_mesmo_suporte_e_da_mesma_marca() {
        use ocinye_image_contracts::bootmode::{GUARD_ARMED, GUARD_MARKER};
        let functions = guard_file("usr/lib/ocinye/guard/casper-guard-functions");
        assert!(
            functions.contains(&format!("= \"{MEDIA_LABEL}\"")),
            "{functions}"
        );
        assert!(functions.contains("= \"iso9660\""));
        assert!(functions.contains(&format!("[ -e {GUARD_MARKER} ]")));
        let rule = guard_file("usr/lib/udev/rules.d/01-ocinye-blockguard.rules");
        assert!(rule.contains(&format!("TEST!=\"{GUARD_MARKER}\"")));
        assert!(rule.contains("blockdev --setro $devnode"));
        // The rule never makes anything writable, and nothing in the medium's
        // fragments does.
        let init = guard_file("usr/share/initramfs-tools/scripts/init-top/ocinye-blockguard");
        assert!(init.contains(&format!("echo {GUARD_ARMED} >{GUARD_MARKER}")));
        let sweep =
            guard_file("usr/share/initramfs-tools/scripts/casper-premount/05ocinye_blockguard");
        let hook = guard_file("usr/share/initramfs-tools/hooks/ocinye-blockguard");
        // casper's swap step is restrained by name: the read-only flag does
        // not stop swapon.
        assert!(hook.contains("casper-bottom/13swap") && hook.contains("&& exit 0"));
        for text in [&functions, &rule, &init, &sweep, &hook] {
            assert!(
                !text.contains("--setrw"),
                "a guard file makes a device writable"
            );
        }
    }
}
