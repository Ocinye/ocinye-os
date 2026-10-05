//! `ocinye-image-builder` — builds Ocinye OS images (D013, ADR-0026/0029).
//! PROVISIONAL_PENDING_D011_CERTIFICATION. DEVELOPMENT builds only in phase A:
//! the stable channel fails closed on every gate (`StableGate`).
//!
//! ```text
//! ocinye-image-builder build --bundle DIR --arch amd64|arm64 --out DIR --work DIR
//!     --cache DIR --image-dir infra/image --bin-dir DIR --builder-commit SHA
//!     --signing-dir DIR [--formats qcow2,raw,iso] [--memory MIB] [--cpus N] [--zstd-level N]
//! ocinye-image-builder verify --out DIR --signing-dir DIR
//! ocinye-image-builder stable-gate                 prints the gate record and refuses
//! ```
//!
//! Runs as root inside a disposable Linux builder VM (`scripts/image-build.sh`
//! creates it): it attaches disks with qemu-nbd and boots build VMs with
//! QEMU. It never runs on the operator's machine and never writes to a
//! removable disk.

#![forbid(unsafe_code)]

mod artifacts;
mod cmd;
mod content;
mod inputs;
mod meta;
mod offline;
mod vm;

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

use ocinye_image_contracts::build::{
    BuildEnvironment, BuildInput, BuildInputKind, BuildProvenance, ImageBuildError,
    StableGateRecord, StepRecord,
};
use ocinye_image_contracts::manifest::{
    Arch, BootMode, BuildKind, BuilderRef, FileRef, ImageArtifact, ImageCompatibility, ImageFormat,
    ImageProfile, OcinyeImageManifest, OcinyeImageVersion, ProfileFacts, ReleaseRef, RuntimePins,
    SbomReference, Sha256Hex, SourceRef, SBOM_FORMAT,
};

struct Args {
    bundle: PathBuf,
    arch: Arch,
    out: PathBuf,
    work: PathBuf,
    cache: PathBuf,
    image_dir: PathBuf,
    bin_dir: PathBuf,
    builder_commit: String,
    signing_dir: PathBuf,
    formats: Vec<ImageFormat>,
    memory: u32,
    cpus: u32,
    zstd_level: u32,
    revision: u32,
}

fn usage() -> ! {
    eprintln!("uso: ocinye-image-builder build --bundle DIR --arch amd64|arm64 --out DIR --work DIR --cache DIR --image-dir DIR --bin-dir DIR --builder-commit SHA --signing-dir DIR [--formats qcow2,raw,iso] [--memory MIB] [--cpus N] [--zstd-level N] [--revision N] | verify --out DIR --signing-dir DIR --arch A | stable-gate");
    std::process::exit(2);
}

fn parse(args: &[String]) -> Args {
    let flag = |n: &str| args.windows(2).find(|w| w[0] == n).map(|w| w[1].clone());
    let need = |n: &str| flag(n).unwrap_or_else(|| usage());
    let arch = match need("--arch").as_str() {
        "amd64" => Arch::Amd64,
        "arm64" => Arch::Arm64,
        _ => usage(),
    };
    let formats = flag("--formats")
        .unwrap_or_else(|| "qcow2,raw,iso".into())
        .split(',')
        .map(|f| match f {
            "qcow2" => ImageFormat::Qcow2,
            "raw" => ImageFormat::Raw,
            "iso" => ImageFormat::Iso,
            _ => usage(),
        })
        .collect();
    Args {
        bundle: need("--bundle").into(),
        arch,
        out: need("--out").into(),
        work: need("--work").into(),
        cache: need("--cache").into(),
        image_dir: need("--image-dir").into(),
        bin_dir: need("--bin-dir").into(),
        builder_commit: need("--builder-commit"),
        signing_dir: need("--signing-dir").into(),
        formats,
        memory: flag("--memory")
            .and_then(|m| m.parse().ok())
            .unwrap_or(2048),
        cpus: flag("--cpus").and_then(|m| m.parse().ok()).unwrap_or(2),
        zstd_level: flag("--zstd-level")
            .and_then(|m| m.parse().ok())
            .unwrap_or(19),
        revision: flag("--revision").and_then(|m| m.parse().ok()).unwrap_or(1),
    }
}

/// Phase A: nothing is satisfied, so `stable` is refused before any work.
fn stable_gate_today() -> StableGateRecord {
    StableGateRecord {
        d011_certified: false,
        d011_phase_b_sync: false,
        d013_final_integration: false,
        immutable_oci_identities: false,
        production_signing: false,
        legal_review: false,
        redis_decision: false,
        certification: false,
    }
}

struct Steps(Vec<StepRecord>);

impl Steps {
    fn time<T>(
        &mut self,
        id: &str,
        f: impl FnOnce() -> Result<T, ImageBuildError>,
    ) -> Result<T, ImageBuildError> {
        let t = Instant::now();
        cmd::log(id, "start");
        let r = f();
        let status = if r.is_ok() { "completed" } else { "failed" };
        self.0.push(StepRecord {
            id: id.into(),
            status: status.into(),
            duration_s: t.elapsed().as_secs(),
        });
        cmd::log(id, status);
        r
    }
}

fn asm(step: &str) -> ImageBuildError {
    ImageBuildError::ImageAssemblyFailed { step: step.into() }
}

#[allow(clippy::too_many_lines)]
fn build(a: &Args) -> Result<(), ImageBuildError> {
    let mut steps = Steps(vec![]);
    let started = chrono::Utc::now();
    let base_cfg: inputs::BaseConfig = serde_json::from_slice(
        &fs::read(a.image_dir.join("base.json")).map_err(|_| asm("base.json"))?,
    )
    .map_err(|_| asm("base.json"))?;

    // B01–B03.
    let bundle = steps.time("B02", || inputs::bundle(&a.bundle, a.arch))?;
    steps.time("B01", || {
        inputs::source(&bundle.manifest.release.commit, true)
    })?;
    if !ocinye_image_contracts::is_lower_hex(&a.builder_commit, 40) {
        return Err(ImageBuildError::SourceNotCertified);
    }
    let base = steps.time("B03", || {
        inputs::base(&base_cfg, &a.image_dir, &a.cache, a.arch)
    })?;
    let image = OcinyeImageVersion {
        release_id: bundle.manifest.release.id.clone(),
        revision: a.revision,
        build_kind: BuildKind::Development,
    };
    let stem = image.artifact_stem(a.arch);
    fs::create_dir_all(&a.out).map_err(|_| asm("out"))?;
    fs::create_dir_all(&a.work).map_err(|_| asm("work"))?;

    // The Ubuntu base's own packages (BaseOs class in the inventory).
    let base_packages: BTreeSet<String> = {
        let m = offline::Mounted::attach("B03", &base.image, &a.work.join("mnt-base"), true)?;
        content::installed_packages(&m.mnt)
    };

    // B04: the build VM and the inputs it receives.
    let stage = a.work.join("stage");
    let _ = fs::remove_dir_all(&stage);
    steps.time("B04", || {
        fs::create_dir_all(stage.join("release")).map_err(|_| asm("stage"))?;
        for e in fs::read_dir(&a.bundle).map_err(|_| asm("stage"))?.flatten() {
            let name = e.file_name();
            if name == "images" {
                continue;
            }
            cmd::run(
                "B04",
                "cp",
                &["-a", cmd::p(&e.path()), cmd::p(&stage.join("release"))],
                || asm("stage"),
            )?;
        }
        cmd::run(
            "B04",
            "cp",
            &["-a", cmd::p(&a.bundle.join("images")), cmd::p(&stage)],
            || asm("stage"),
        )?;
        let third: String = bundle
            .manifest
            .third_party_images
            .iter()
            .map(|t| format!("{}\n", t.reference))
            .collect();
        vm::stage_file(&stage, "third-party.txt", third.as_bytes(), 0o644)?;
        let env = format!(
            "ARCH={}\nAPT_SNAPSHOT={}\nRELEASE_ID={}\nDOCKER_KEY_FPR={}\nBUILD_USER={}\n",
            a.arch.as_str(),
            base_cfg.apt_snapshot,
            bundle.manifest.release.id,
            bundle.manifest.prerequisites.docker_repo_key_fingerprint,
            vm::BUILD_USER
        );
        vm::stage_file(&stage, "build.env", env.as_bytes(), 0o644)?;
        vm::stage_file(
            &stage,
            "provision.sh",
            &fs::read(a.image_dir.join("build/provision.sh")).map_err(|_| asm("provision"))?,
            0o755,
        )?;
        for b in ["ocinye-firstboot", "ocinye-oie"] {
            vm::stage_file(
                &stage,
                &format!("bin/{b}"),
                &fs::read(a.bin_dir.join(b)).map_err(|_| asm(b))?,
                0o755,
            )?;
        }
        cmd::run(
            "B04",
            "tar",
            &[
                "-C",
                cmd::p(&a.image_dir.join("rootfs")),
                "--owner=0",
                "--group=0",
                "-cf",
                cmd::p(&stage.join("rootfs.tar")),
                ".",
            ],
            || asm("rootfs"),
        )
    })?;
    let bvm = vm::BuildVm::prepare(a.arch, &a.work.join("vm"), 2222, a.memory, a.cpus)?;

    // B05–B10, pass 1: the common layer.
    let common = a.work.join("common.qcow2");
    bvm.overlay(&base.image, &common, "24G")?;
    let (oci_json, runtime_txt) = steps.time("B05-B09 common", || {
        let r = bvm.boot(&common, "common")?;
        r.upload("B05", &stage)?;
        r.ssh("B06", "sudo /root/ocinye-build/provision.sh common >&2")?;
        let oci = r.ssh("B08", "sudo cat /root/ocinye-build/out/oci.json")?;
        let rt = r.ssh("B07", "sudo cat /root/ocinye-build/out/dpkg-runtime.txt")?;
        r.poweroff(false)?;
        Ok((
            String::from_utf8_lossy(&oci).into_owned(),
            String::from_utf8_lossy(&rt).into_owned(),
        ))
    })?;
    let oci_images = content::oci_images(&oci_json, &bundle)?;
    let pin = |p: &str| {
        runtime_txt
            .lines()
            .find_map(|l| l.strip_prefix(&format!("{p} ")))
            .unwrap_or_default()
            .to_owned()
    };
    let runtime = RuntimePins {
        docker_ce: pin("docker-ce"),
        docker_ce_cli: pin("docker-ce-cli"),
        containerd_io: pin("containerd.io"),
        docker_compose_plugin: pin("docker-compose-plugin"),
        repo_key_fingerprint: bundle
            .manifest
            .prerequisites
            .docker_repo_key_fingerprint
            .clone(),
    };

    // Pass 2: one overlay per profile, then finalize inside, then B11–B13 offline.
    let want_iso = a.formats.contains(&ImageFormat::Iso);
    let mut profiles: Vec<&str> = vec![];
    if a.formats
        .iter()
        .any(|f| matches!(f, ImageFormat::Qcow2 | ImageFormat::Raw))
    {
        profiles.push("virt");
    }
    if want_iso {
        profiles.extend(["metal", "oie"]);
    }
    let mut facts = vec![];
    let mut inventories = vec![];
    let mut content_paths = std::collections::BTreeMap::new();
    let mut inspections = vec![];
    for prof in &profiles {
        let disk = a.work.join(format!("{prof}.qcow2"));
        bvm.overlay(&common, &disk, "24G")?;
        let tsv = steps.time(&format!("B06-B10 {prof}"), || {
            let r = bvm.boot(&disk, prof)?;
            r.ssh("B06", &format!("sudo /root/ocinye-build/provision.sh {prof} >&2"))?;
            // finalize removes the build user's SSH access: read the inventory
            // and schedule the power-off in the same session.
            let tsv = r.ssh("B10", &format!("sudo /root/ocinye-build/provision.sh finalize {prof} >&2 && sudo cat /root/ocinye-build/out/packages.tsv && sudo systemd-run --on-active=3 systemctl poweroff >&2"))?;
            r.poweroff(true)?;
            Ok(String::from_utf8_lossy(&tsv).into_owned())
        })?;
        let m = offline::Mounted::attach("B11", &disk, &a.work.join(format!("mnt-{prof}")), false)?;
        let cleaned = steps.time(&format!("B11 {prof}"), || offline::sanitize(&m))?;
        cmd::log("B11", &format!("{prof}: {}", cleaned.join(", ")));
        if *prof == "oie" {
            let ins = offline::inspect(&m, prof, &bundle.manifest.release.id);
            write_json(
                &a.out.join(format!("provenance/inspection-{prof}.json")),
                &ins,
            )?;
            if let Some(c) = ins.first_failure() {
                return Err(ImageBuildError::InspectionFailed {
                    check: format!("{prof}:{c}"),
                });
            }
            inspections.push(prof.to_string());
            continue;
        }
        let profile = if *prof == "metal" {
            ImageProfile::Metal
        } else {
            ImageProfile::Virt
        };
        let format = if *prof == "metal" {
            ImageFormat::Iso
        } else {
            ImageFormat::Qcow2
        };
        let inv = content::inventory(
            &tsv,
            &base_packages,
            &image,
            profile,
            &base_cfg.apt_snapshot,
        )?;
        let inv_path = a.out.join(format!("inventory/packages-{prof}.json"));
        let inv_sha = meta::write_canonical(&inv_path, &inv)?;
        let files = content::release_files(&m, &bundle.manifest.release.id)?;
        let cm = content::content_manifest(
            &image,
            profile,
            a.arch,
            &bundle,
            files,
            oci_images.clone(),
            inv_sha.clone(),
        );
        steps.time(&format!("B13 {prof}"), || {
            content::embed(&m, &cm, format, &base.identity.serial)
        })?;
        let cpath = a.work.join(format!("IMAGE_CONTENT-{prof}.json"));
        fs::write(&cpath, cm.to_canonical()).map_err(|_| asm("content"))?;
        content_paths.insert(prof.to_string(), cpath);
        let ins = steps.time(&format!("B12 {prof}"), || {
            Ok(offline::inspect(&m, prof, &bundle.manifest.release.id))
        })?;
        write_json(
            &a.out.join(format!("provenance/inspection-{prof}.json")),
            &ins,
        )?;
        if let Some(c) = ins.first_failure() {
            return Err(ImageBuildError::InspectionFailed {
                check: format!("{prof}:{c}"),
            });
        }
        inspections.push(prof.to_string());
        let used = cmd::output(
            "B12",
            "df",
            &["--output=used", "-B1", cmd::p(&m.mnt)],
            || asm("df"),
        )?;
        let installed_bytes = String::from_utf8_lossy(&used)
            .lines()
            .nth(1)
            .and_then(|l| l.trim().parse().ok())
            .unwrap_or(0);
        facts.push(ProfileFacts {
            name: profile,
            content_sha256: Sha256Hex(cm.sha256()),
            package_inventory_sha256: inv_sha.clone(),
            installed_bytes,
        });
        inventories.push(FileRef {
            file: format!("inventory/packages-{prof}.json"),
            sha256: inv_sha,
        });
        if *prof == "virt" {
            let sb = a.out.join(format!("sbom/{stem}.spdx.json"));
            fs::create_dir_all(sb.parent().ok_or_else(|| asm("sbom"))?).map_err(|_| asm("sbom"))?;
            let sbom_sha = steps.time("B16 sbom", || meta::sbom(&m.mnt, &sb, &stem))?;
            fs::write(a.work.join("sbom.sha256"), &sbom_sha.0).map_err(|_| asm("sbom"))?;
        }
    }

    // B15: artifacts.
    let mut artifacts = vec![];
    let boot = vec![match a.arch {
        Arch::Amd64 => BootMode::UefiX86_64,
        Arch::Arm64 => BootMode::UefiAarch64,
    }];
    let virt = a.work.join("virt.qcow2");
    if a.formats.contains(&ImageFormat::Qcow2) {
        let out = a.out.join(format!("{stem}.qcow2"));
        let vbytes = steps.time("B15 qcow2", || artifacts::qcow2(&virt, &out))?;
        let (h, n) = cmd::sha256_file(&out).map_err(|_| asm("qcow2"))?;
        artifacts.push(ImageArtifact {
            format: ImageFormat::Qcow2,
            profile: ImageProfile::Virt,
            file: format!("{stem}.qcow2"),
            sha256: Sha256Hex(h),
            bytes: n,
            uncompressed_sha256: None,
            uncompressed_bytes: None,
            virtual_bytes: Some(vbytes),
            boot: boot.clone(),
        });
    }
    if a.formats.contains(&ImageFormat::Raw) {
        let raw = a.work.join(format!("{stem}.raw"));
        let zst = a.out.join(format!("{stem}.raw.zst"));
        let (uh, un) = steps.time("B15 raw", || {
            artifacts::raw(&virt, &raw, &zst, a.zstd_level)
        })?;
        let (h, n) = cmd::sha256_file(&zst).map_err(|_| asm("raw"))?;
        let _ = fs::remove_file(&raw);
        artifacts.push(ImageArtifact {
            format: ImageFormat::Raw,
            profile: ImageProfile::Virt,
            file: format!("{stem}.raw.zst"),
            sha256: Sha256Hex(h),
            bytes: n,
            uncompressed_sha256: Some(uh),
            uncompressed_bytes: Some(un),
            virtual_bytes: Some(un),
            boot: boot.clone(),
        });
    }
    if want_iso {
        let out = a.out.join(format!("{stem}.iso"));
        let oie = offline::Mounted::attach(
            "B15",
            &a.work.join("oie.qcow2"),
            &a.work.join("mnt-oie-ro"),
            true,
        )?;
        let metal = offline::Mounted::attach(
            "B15",
            &a.work.join("metal.qcow2"),
            &a.work.join("mnt-metal-ro"),
            true,
        )?;
        let cpath = content_paths
            .get("metal")
            .ok_or_else(|| asm("metal content"))?
            .clone();
        steps.time("B15 iso", || {
            artifacts::iso(&a.work, &out, a.arch, &image, &oie, &metal, &cpath)
        })?;
        let (h, n) = cmd::sha256_file(&out).map_err(|_| asm("iso"))?;
        artifacts.push(ImageArtifact {
            format: ImageFormat::Iso,
            profile: ImageProfile::Metal,
            file: format!("{stem}.iso"),
            sha256: Sha256Hex(h),
            bytes: n,
            uncompressed_sha256: None,
            uncompressed_bytes: None,
            virtual_bytes: None,
            boot: boot.clone(),
        });
    }

    // B16: metadata.
    let oci_sha = meta::write_canonical(&a.out.join("inventory/oci-images.json"), &oci_images)?;
    let mut inputs_list: Vec<BuildInput> = base.inputs.clone();
    inputs_list.push(BuildInput {
        kind: BuildInputKind::AptSnapshot,
        identity: base_cfg.apt_snapshot.clone(),
        digest: Sha256Hex(sha_text(&base_cfg.apt_snapshot)),
        trust: "apt-inrelease".into(),
    });
    inputs_list.push(BuildInput {
        kind: BuildInputKind::ReleaseBundle,
        identity: bundle.manifest.release.id.clone(),
        digest: bundle.manifest_sha256.clone(),
        trust: "manifest-sha256".into(),
    });
    inputs_list.push(BuildInput {
        kind: BuildInputKind::DockerRepo,
        identity: format!("docker-ce {}", runtime.docker_ce),
        digest: Sha256Hex(sha_text(&runtime_txt)),
        trust: format!("openpgp:{}", runtime.repo_key_fingerprint),
    });
    for o in &oci_images {
        inputs_list.push(BuildInput {
            kind: BuildInputKind::OciImage,
            identity: o.reference.clone(),
            digest: Sha256Hex(o.image_id.0.trim_start_matches("sha256:").into()),
            trust: "oci-digest".into(),
        });
    }
    let mut outputs: Vec<FileRef> = artifacts
        .iter()
        .map(|x| FileRef {
            file: x.file.clone(),
            sha256: x.sha256.clone(),
        })
        .collect();
    outputs.extend(inventories.iter().cloned());
    outputs.push(FileRef {
        file: "inventory/oci-images.json".into(),
        sha256: oci_sha,
    });
    let mut rnd = [0u8; 6];
    let _ = fs::File::open("/dev/urandom")
        .and_then(|mut f| std::io::Read::read_exact(&mut f, &mut rnd));
    let provenance = BuildProvenance {
        schema: 1,
        image: image.clone(),
        source_commit: bundle.manifest.release.commit.clone(),
        source_tree_clean: true,
        source_date_epoch: bundle.manifest.release.source_date_epoch,
        inputs: inputs_list,
        builder_version: env!("CARGO_PKG_VERSION").into(),
        builder_commit: a.builder_commit.clone(),
        environment: BuildEnvironment::Local,
        execution_id: format!("local-{}", hex::encode(rnd)),
        steps: steps.0.clone(),
        outputs,
    };
    let prov_sha =
        meta::write_canonical(&a.out.join("provenance/build-provenance.json"), &provenance)?;
    let sbom_sha = Sha256Hex(fs::read_to_string(a.work.join("sbom.sha256")).unwrap_or_default());
    let manifest = OcinyeImageManifest {
        schema: ocinye_image_contracts::IMAGE_MANIFEST_SCHEMA,
        product: ocinye_image_contracts::PRODUCT.into(),
        image: image.clone(),
        source: SourceRef {
            commit: bundle.manifest.release.commit.clone(),
            tree_clean: true,
            source_date_epoch: bundle.manifest.release.source_date_epoch,
        },
        release: ReleaseRef {
            id: bundle.manifest.release.id.clone(),
            manifest_sha256: bundle.manifest_sha256.clone(),
            bootstrap_sha256: bundle.bootstrap_sha256.clone(),
        },
        ubuntu_base: base.identity.clone(),
        apt_snapshot: base_cfg.apt_snapshot.clone(),
        architecture: a.arch,
        profiles: facts,
        artifacts,
        oci_images: oci_images.clone(),
        runtime,
        sbom: SbomReference {
            format: SBOM_FORMAT.into(),
            file: format!("sbom/{stem}.spdx.json"),
            sha256: sbom_sha,
        },
        package_inventory: inventories,
        provenance: FileRef {
            file: "provenance/build-provenance.json".into(),
            sha256: prov_sha,
        },
        builder: BuilderRef {
            name: "ocinye-image-builder".into(),
            version: env!("CARGO_PKG_VERSION").into(),
            commit: a.builder_commit.clone(),
            environment: BuildEnvironment::Local,
        },
        compatibility: ImageCompatibility {
            bootstrap_protocol: ocinye_installer_contracts::BOOTSTRAP_PROTOCOL,
            claim_protocol: ocinye_image_contracts::CLAIM_PROTOCOL,
            installer_min: "0.1.0".into(),
            d011_receipt_schema_min: 2,
        },
        created_at: chrono::DateTime::from_timestamp(bundle.manifest.release.source_date_epoch, 0)
            .map(|t| t.to_rfc3339_opts(chrono::SecondsFormat::Secs, true))
            .unwrap_or_default(),
    };
    manifest
        .validate()
        .map_err(|e| ImageBuildError::ManifestFailed { field: e.field })?;
    fs::write(a.out.join("IMAGE_MANIFEST.json"), manifest.to_canonical())
        .map_err(|_| asm("IMAGE_MANIFEST.json"))?;
    fs::copy(a.bundle.join("MANIFEST.json"), a.out.join("MANIFEST.json"))
        .map_err(|_| asm("MANIFEST.json"))?;
    let record = serde_json::json!({ "schema": 1, "started_at": started.to_rfc3339_opts(chrono::SecondsFormat::Secs, true), "steps": steps.0, "inspected": inspections });
    write_json(&a.out.join("provenance/build-record.json"), &record)?;
    let sums = meta::sha256sums(&a.out)?;
    fs::write(
        a.out.join("SHA256SUMS"),
        ocinye_image_contracts::signing::render_sha256sums(&sums),
    )
    .map_err(|_| asm("SHA256SUMS"))?;
    let (sk, pk) = meta::dev_key(&a.signing_dir)?;
    let now = chrono::Utc::now().timestamp();
    meta::sign_dev(&a.out, &sk, &image, a.arch, now)?;
    // B17.
    let v = meta::verify(&a.out, &pk, &image, a.arch, now)?;
    cmd::log(
        "B17",
        &format!(
            "IMAGE_VERIFIED {}",
            serde_json::to_string(&v).unwrap_or_default()
        ),
    );
    Ok(())
}

fn sha_text(s: &str) -> String {
    use sha2::Digest as _;
    hex::encode(sha2::Sha256::digest(s.as_bytes()))
}

fn write_json<T: serde::Serialize>(path: &Path, v: &T) -> Result<(), ImageBuildError> {
    meta::write_canonical(path, v).map(|_| ())
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let code = match args.first().map(String::as_str) {
        Some("build") => {
            let a = parse(&args);
            match build(&a) {
                Ok(()) => 0,
                Err(e) => {
                    eprintln!(
                        "BUILD_FAILED {}",
                        serde_json::to_string(&e).unwrap_or_default()
                    );
                    1
                }
            }
        }
        Some("verify") => {
            let flag = |n: &str| {
                args.windows(2)
                    .find(|w| w[0] == n)
                    .map(|w| w[1].clone())
                    .unwrap_or_else(|| usage())
            };
            let out = PathBuf::from(flag("--out"));
            let arch = if flag("--arch") == "arm64" {
                Arch::Arm64
            } else {
                Arch::Amd64
            };
            let m: OcinyeImageManifest = match fs::read(out.join("IMAGE_MANIFEST.json"))
                .ok()
                .and_then(|b| OcinyeImageManifest::parse(&b).ok())
            {
                Some(m) => m,
                None => {
                    eprintln!("IMAGE_MANIFEST.json inválido");
                    std::process::exit(1);
                }
            };
            match meta::verify(
                &out,
                &PathBuf::from(flag("--signing-dir")).join("ocinye-dev.pub"),
                &m.image,
                arch,
                chrono::Utc::now().timestamp(),
            ) {
                Ok(v) => {
                    println!("{}", serde_json::to_string(&v).unwrap_or_default());
                    0
                }
                Err(e) => {
                    eprintln!(
                        "VERIFY_FAILED {}",
                        serde_json::to_string(&e).unwrap_or_default()
                    );
                    1
                }
            }
        }
        Some("stable-gate") => {
            let rec = stable_gate_today();
            println!("{}", serde_json::to_string(&rec).unwrap_or_default());
            match rec.first_unsatisfied() {
                Some(g) => {
                    eprintln!(
                        "STABLE_GATE_NOT_SATISFIED {}",
                        serde_json::to_string(&ImageBuildError::StableGateNotSatisfied { gate: g })
                            .unwrap_or_default()
                    );
                    1
                }
                None => 0,
            }
        }
        _ => usage(),
    };
    std::process::exit(code);
}

#[cfg(test)]
mod tests {
    use super::*;
    use ocinye_image_contracts::build::StableGate;

    #[test]
    fn o_portao_estavel_falha_fechado_na_fase_a() {
        let rec = stable_gate_today();
        assert_eq!(rec.first_unsatisfied(), Some(StableGate::D011Certified));
        assert_eq!(StableGate::ALL.len(), 8);
    }
}
