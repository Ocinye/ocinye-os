//! `ocinye-release-manifest`: writes `MANIFEST.json` for a release bundle.
//!
//! Called by `scripts/release-bundle.sh` once every file of the bundle is in
//! place. It hashes every artifact, reads the migrations and the third-party
//! images from the release's own tree, records the image ids it is given, and
//! writes the manifest in its **canonical** form — the same bytes the
//! Installer hashes (`manifest_sha256`). It refuses to write a manifest the
//! Installer would refuse to read.
//!
//! ```text
//! ocinye-release-manifest --bundle DIR --tree DIR --commit SHA40
//!     --created-at RFC3339 --source-date-epoch N --arch amd64|arm64
//!     --build release|proof --image SERVICE=sha256:… (×5)
//! ```
//!
//! No signature: `RELEASE_SIGNING = NOT_IMPLEMENTED`. The sums prove a bundle
//! arrived as it left; they do not prove who made it.

#![forbid(unsafe_code)]

use std::collections::BTreeMap;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use ocinye_installer_contracts::canonical::sha256_hex;
use ocinye_installer_contracts::manifest::{
    Arch, Artifact, BootstrapRef, BuildKind, Compatibility, Image, Migrations, Prerequisites,
    ReleaseInfo, ReleaseManifest, Target, ThirdPartyImage, DOCKER_PACKAGES, SERVICES,
};
use sha2::{Digest, Sha256};

/// Docker's published apt repository key (docs.docker.com/engine/install/ubuntu).
const DOCKER_REPO_KEY_FINGERPRINT: &str = "9DC858229FC7DD38854AE2D88D81803C0EBFCD88";

/// The production Compose file, relative to the tree.
const COMPOSE: &str = "infra/compose/docker-compose.production.yml";

fn fail(msg: &str) -> ! {
    eprintln!("\n  RECUSADO — {msg}\n");
    std::process::exit(1);
}

fn sha256_file(path: &Path) -> (String, u64) {
    let mut f = fs::File::open(path).unwrap_or_else(|_| fail(&format!("falta {}", path.display())));
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 1 << 20];
    let mut n = 0u64;
    loop {
        let read = f.read(&mut buf).unwrap_or_else(|_| fail("leitura"));
        if read == 0 {
            break;
        }
        n += read as u64;
        h.update(&buf[..read]);
    }
    (hex::encode(h.finalize()), n)
}

fn files(dir: &Path, base: &Path, out: &mut Vec<String>) {
    let mut entries: Vec<_> = fs::read_dir(dir)
        .unwrap_or_else(|_| fail("pasta do pacote"))
        .filter_map(Result::ok)
        .collect();
    entries.sort_by_key(std::fs::DirEntry::file_name);
    for e in entries {
        let p = e.path();
        if p.is_dir() {
            files(&p, base, out);
        } else if let Ok(rel) = p.strip_prefix(base) {
            out.push(rel.to_string_lossy().replace('\\', "/"));
        }
    }
}

/// `migrations/*.sql` → count, latest, SHA-256 of the sorted `name sha256` lines.
fn migrations(tree: &Path) -> Migrations {
    let mut lines = Vec::new();
    let mut latest = 0u32;
    let mut entries: Vec<_> = fs::read_dir(tree.join("migrations"))
        .unwrap_or_else(|_| fail("falta migrations/"))
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "sql"))
        .collect();
    entries.sort();
    for p in &entries {
        let name = p
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let (sum, _) = sha256_file(p);
        lines.push(format!("{name} {sum}\n"));
        if let Some(n) = name.get(..4).and_then(|v| v.parse::<u32>().ok()) {
            latest = latest.max(n);
        }
    }
    Migrations {
        count: u32::try_from(entries.len()).unwrap_or(u32::MAX),
        latest: format!("{latest:04}"),
        files_sha256: sha256_hex(lines.concat().as_bytes()),
    }
}

/// `image:` lines of the production Compose that are not release images.
fn third_party(tree: &Path) -> Vec<ThirdPartyImage> {
    let text = fs::read_to_string(tree.join(COMPOSE))
        .unwrap_or_else(|_| fail("falta o Compose de produção"));
    let mut seen = Vec::new();
    for line in text.lines() {
        let Some(image) = line.trim().strip_prefix("image:") else {
            continue;
        };
        let image = image.trim().trim_matches('"').to_owned();
        if image.starts_with("ocinye/") || seen.contains(&image) {
            continue;
        }
        seen.push(image);
    }
    seen.into_iter()
        .map(|reference| ThirdPartyImage {
            reference,
            from_compose: COMPOSE.to_owned(),
        })
        .collect()
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut flags: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut it = args.iter();
    while let Some(flag) = it.next() {
        let value = it
            .next()
            .unwrap_or_else(|| fail(&format!("{flag} precisa de um valor")));
        flags.entry(flag.clone()).or_default().push(value.clone());
    }
    let one = |k: &str| -> String {
        match flags.get(k).map(Vec::as_slice) {
            Some([v]) => v.clone(),
            _ => fail(&format!("{k} é obrigatório, uma vez")),
        }
    };
    for k in flags.keys() {
        if !matches!(
            k.as_str(),
            "--bundle"
                | "--tree"
                | "--commit"
                | "--created-at"
                | "--source-date-epoch"
                | "--arch"
                | "--build"
                | "--image"
        ) {
            fail(&format!("opção desconhecida: {k}"));
        }
    }
    let bundle = PathBuf::from(one("--bundle"));
    let tree = PathBuf::from(one("--tree"));
    let commit = one("--commit");
    let arch = match one("--arch").as_str() {
        "amd64" => Arch::Amd64,
        "arm64" => Arch::Arm64,
        other => fail(&format!("arquitectura desconhecida: {other}")),
    };
    let build = match one("--build").as_str() {
        "release" => BuildKind::Release,
        "proof" => BuildKind::Proof,
        other => fail(&format!("build desconhecido: {other}")),
    };
    let image_ids: BTreeMap<String, String> = flags
        .get("--image")
        .cloned()
        .unwrap_or_default()
        .iter()
        .map(|v| {
            let (svc, id) = v
                .split_once('=')
                .unwrap_or_else(|| fail("--image é SERVIÇO=sha256:…"));
            (svc.to_owned(), id.to_owned())
        })
        .collect();

    let mut paths = Vec::new();
    files(&bundle, &bundle, &mut paths);
    let artifacts: Vec<Artifact> = paths
        .iter()
        .filter(|p| {
            !matches!(
                p.as_str(),
                "MANIFEST.json" | "SHA256SUMS" | "RELEASE" | "BUILD"
            )
        })
        .map(|p| {
            let (sha256, bytes) = sha256_file(&bundle.join(p));
            Artifact {
                path: p.clone(),
                sha256,
                bytes,
            }
        })
        .collect();
    let id = commit.get(..12).unwrap_or_default().to_owned();
    let images = SERVICES
        .iter()
        .map(|svc| Image {
            name: format!("ocinye/ocinye-{svc}"),
            tag: id.clone(),
            archive: format!("images/ocinye-{svc}.tar"),
            image_id: image_ids
                .get(*svc)
                .cloned()
                .unwrap_or_else(|| fail(&format!("falta --image {svc}=…"))),
        })
        .collect();
    let bootstrap_sha = artifacts
        .iter()
        .find(|a| a.path == "ocinye-bootstrap")
        .map(|a| a.sha256.clone())
        .unwrap_or_else(|| fail("o pacote não traz ocinye-bootstrap"));
    let manifest = ReleaseManifest {
        schema: ocinye_installer_contracts::manifest::SCHEMA,
        product: "ocinye-os".into(),
        release: ReleaseInfo {
            id,
            commit,
            build,
            created_at: one("--created-at"),
            source_date_epoch: one("--source-date-epoch")
                .parse()
                .unwrap_or_else(|_| fail("--source-date-epoch")),
        },
        target: Target {
            os: "linux".into(),
            arch,
        },
        artifacts,
        images,
        third_party_images: third_party(&tree),
        migrations: migrations(&tree),
        compatibility: Compatibility {
            readiness_contract: ocinye_contracts::readiness::CONTRACT_VERSION,
            bootstrap_protocol: ocinye_installer_contracts::BOOTSTRAP_PROTOCOL,
            installer_min: env!("CARGO_PKG_VERSION").into(),
            docker_engine_min: "24.0".into(),
            compose: "v2".into(),
            supported_prerequisite_targets: vec![
                ocinye_installer_contracts::SUPPORTED_TARGET.into()
            ],
        },
        prerequisites: Prerequisites {
            docker_repo_key_fingerprint: DOCKER_REPO_KEY_FINGERPRINT.into(),
            docker_packages: DOCKER_PACKAGES.iter().map(|s| (*s).to_owned()).collect(),
        },
        bootstrap: BootstrapRef {
            path: "ocinye-bootstrap".into(),
            sha256: bootstrap_sha,
            is_static: true,
        },
    };
    if let Err(e) = manifest.validate() {
        fail(&format!(
            "o manifesto seria recusado pelo Installer: {}",
            e.field
        ));
    }
    let bytes = manifest.canonical_bytes();
    fs::write(bundle.join("MANIFEST.json"), bytes.as_bytes()).unwrap_or_else(|_| fail("escrita"));
    println!("{}", manifest.sha256());
}
