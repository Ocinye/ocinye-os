//! The trust model of the update repository, end to end on disk: what a
//! machine accepts, and every way a mirror or a file could lie to it.

use ocinye_update_contracts::manifest::{
    Arch, Artifact, ArtifactRole, Build, ImageRef, MigrationPlan, MigrationPolicy, Notes,
    SpaceRequirement, UpdateManifest, BASE_OS, PRODUCT, SCHEMA,
};
use ocinye_update_contracts::{Channel, ReleaseNumber, ReleaseVersion, MANIFEST_TARGET};
use ocinye_update_repository::keys::{create_root, rotate_root, KeySet};
use ocinye_update_repository::publish::{init_channel, publish_release, rotate_keys, Expiry};
use ocinye_update_repository::verify::{dir_url, load, load_dir, read_manifest, releases};
use ocinye_update_repository::{offline, sha256_file, target_name};
use std::path::Path;

const CH: Channel = Channel::Development;

fn in_a_year() -> jiff::Timestamp {
    jiff::Timestamp::now()
        .checked_add(jiff::SignedDuration::from_hours(24 * 365))
        .unwrap()
}

/// A staged release with made-up bytes but every rule of a real one.
fn fake_release(dir: &Path, id: &str, revision: u32, number: u64) -> UpdateManifest {
    let version = ReleaseVersion::new(id, revision).unwrap();
    let files: Vec<(&str, ArtifactRole, Vec<u8>)> = vec![
        (
            "bundle/MANIFEST.json",
            ArtifactRole::ReleaseBundle,
            format!("{{\"release\":\"{id}\",\"n\":{number}}}").into_bytes(),
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
            "native/native-amd64.tar",
            ArtifactRole::Native,
            vec![number as u8; 4096],
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
    let total = artifacts.iter().map(|a| a.bytes).sum();
    let m = UpdateManifest {
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
        minimum_source: ReleaseNumber(number - 1),
        system_protocol: 1,
        release_manifest_sha256: artifacts[0].sha256.clone(),
        artifacts,
        images: vec![ImageRef {
            name: "core-server".into(),
            image_id: format!("sha256:{}", "e".repeat(64)),
        }],
        migrations: MigrationPlan {
            schema_version: 64,
            policy: MigrationPolicy::None,
        },
        space: SpaceRequirement {
            download_bytes: total,
            install_bytes: total * 2,
        },
        reboot_required: false,
        security: false,
        notes: Notes {
            pt: "Teste.".into(),
            en: "Test.".into(),
            fr: "Essai.".into(),
        },
        synthetic: None,
    };
    std::fs::write(dir.join("manifest.json"), serde_json::to_vec(&m).unwrap()).unwrap();
    m
}

struct Repo {
    _tmp: tempfile::TempDir,
    root: std::path::PathBuf,
    keys: KeySet,
    first_root: Vec<u8>,
}

async fn repo() -> Repo {
    let tmp = tempfile::tempdir().unwrap();
    let keys = KeySet::generate(&tmp.path().join("keys")).unwrap();
    let first_root = create_root(&keys, 1, in_a_year()).await.unwrap();
    let root = tmp.path().join("repo");
    init_channel(&root, CH, &first_root).unwrap();
    Repo {
        root,
        keys,
        first_root,
        _tmp: tmp,
    }
}

async fn publish(r: &Repo, id: &str, rev: u32, n: u64) -> UpdateManifest {
    let staged = r.root.parent().unwrap().join(format!("stage-{id}-{rev}"));
    let m = fake_release(&staged, id, rev, n);
    publish_release(&r.root, CH, &r.keys, &staged, Expiry::for_channel(CH))
        .await
        .unwrap();
    m
}

fn copy_tree(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for e in std::fs::read_dir(from).unwrap() {
        let e = e.unwrap();
        let dest = to.join(e.file_name());
        if e.file_type().unwrap().is_dir() {
            copy_tree(&e.path(), &dest);
        } else {
            std::fs::copy(e.path(), dest).unwrap();
        }
    }
}

#[tokio::test]
async fn um_cliente_ve_os_releases_publicados_mais_novo_primeiro() {
    let r = repo().await;
    publish(&r, "aaaaaaaaaaaa", 1, 1).await;
    publish(&r, "bbbbbbbbbbbb", 1, 2).await;
    let loaded = load_dir(&r.root, CH, &r.first_root, false).await.unwrap();
    let rs = releases(&loaded).await.unwrap();
    assert_eq!(rs.len(), 2);
    assert_eq!(rs[0].manifest.release_number, ReleaseNumber(2));
    assert_eq!(rs[1].manifest.version.release_id(), "aaaaaaaaaaaa");
}

#[tokio::test]
async fn o_numero_nunca_recua_nem_se_republica_o_mesmo() {
    let r = repo().await;
    publish(&r, "aaaaaaaaaaaa", 1, 5).await;
    let staged = r.root.parent().unwrap().join("older");
    fake_release(&staged, "bbbbbbbbbbbb", 1, 4);
    assert!(
        publish_release(&r.root, CH, &r.keys, &staged, Expiry::for_channel(CH))
            .await
            .is_err()
    );
    let again = r.root.parent().unwrap().join("again");
    fake_release(&again, "aaaaaaaaaaaa", 1, 6);
    assert!(
        publish_release(&r.root, CH, &r.keys, &again, Expiry::for_channel(CH))
            .await
            .is_err()
    );
}

#[tokio::test]
async fn um_alvo_alterado_no_espelho_e_recusado() {
    let r = repo().await;
    let m = publish(&r, "aaaaaaaaaaaa", 1, 1).await;
    // Find the manifest's file under targets/ and change one byte.
    let targets = r.root.join("development/targets");
    let mut changed = false;
    for e in walk(&targets) {
        if e.to_string_lossy().ends_with(MANIFEST_TARGET) {
            let mut b = std::fs::read(&e).unwrap();
            let last = b.len() - 2;
            b[last] ^= 1;
            std::fs::write(&e, b).unwrap();
            changed = true;
        }
    }
    assert!(changed);
    let loaded = load_dir(&r.root, CH, &r.first_root, false).await.unwrap();
    let e = read_manifest(&loaded, &target_name(&m.version, MANIFEST_TARGET))
        .await
        .unwrap_err()
        .to_string();
    assert!(
        e.to_lowercase().contains("hash") || e.contains("sha256") || e.contains("digest"),
        "refused for another reason: {e}"
    );
}

#[tokio::test]
async fn metadados_alterados_ou_outra_raiz_sao_recusados() {
    let r = repo().await;
    publish(&r, "aaaaaaaaaaaa", 1, 1).await;
    // Another root: a mirror that signs with its own keys.
    let other = tempfile::tempdir().unwrap();
    let other_keys = KeySet::generate(&other.path().join("k")).unwrap();
    let other_root = create_root(&other_keys, 1, in_a_year()).await.unwrap();
    let e = load_dir(&r.root, CH, &other_root, false)
        .await
        .unwrap_err()
        .to_string();
    assert!(
        e.contains("Signature threshold") || e.contains("signature"),
        "refused for another reason: {e}"
    );
    // A byte changed in the signed targets.
    let meta = r.root.join("development/metadata");
    let targets = std::fs::read_dir(&meta)
        .unwrap()
        .map(|e| e.unwrap().path())
        .find(|p| p.to_string_lossy().ends_with(".targets.json"))
        .unwrap();
    let text = std::fs::read_to_string(&targets).unwrap();
    std::fs::write(
        &targets,
        text.replacen("\"length\":", "\"length\": ", 1)
            .replacen("4096", "4097", 1),
    )
    .unwrap();
    assert!(load_dir(&r.root, CH, &r.first_root, false).await.is_err());
}

#[tokio::test]
async fn metadados_expirados_sao_recusados_um_espelho_congelado() {
    let r = repo().await;
    let staged = r.root.parent().unwrap().join("s");
    fake_release(&staged, "aaaaaaaaaaaa", 1, 1);
    let expired = Expiry {
        targets_days: 30,
        snapshot_days: 30,
        timestamp_days: -1,
    };
    publish_release(&r.root, CH, &r.keys, &staged, expired)
        .await
        .unwrap();
    let e = load_dir(&r.root, CH, &r.first_root, false)
        .await
        .unwrap_err();
    assert!(e.to_string().to_lowercase().contains("expir"), "{e}");
    assert!(load_dir(&r.root, CH, &r.first_root, true).await.is_ok());
}

#[tokio::test]
async fn metadados_antigos_depois_de_ver_novos_sao_recusados() {
    let r = repo().await;
    publish(&r, "aaaaaaaaaaaa", 1, 1).await;
    let old_copy = r.root.parent().unwrap().join("old-mirror");
    copy_tree(&r.root, &old_copy);
    publish(&r, "bbbbbbbbbbbb", 1, 2).await;
    let datastore = tempfile::tempdir().unwrap();
    let ch = r.root.join("development");
    // The machine sees the new state…
    load(
        &r.first_root,
        dir_url(&ch.join("metadata")).unwrap(),
        dir_url(&ch.join("targets")).unwrap(),
        Some(datastore.path()),
        tough::FilesystemTransport,
        false,
    )
    .await
    .unwrap();
    // …then a mirror serves the old one: refused.
    let old = old_copy.join("development");
    let replay = load(
        &r.first_root,
        dir_url(&old.join("metadata")).unwrap(),
        dir_url(&old.join("targets")).unwrap(),
        Some(datastore.path()),
        tough::FilesystemTransport,
        false,
    )
    .await;
    let e = replay
        .expect_err("a replayed repository was accepted")
        .to_string();
    assert!(e.contains("version"), "refused for another reason: {e}");
}

#[tokio::test]
async fn a_raiz_roda_e_a_maquina_segue_a_cadeia() {
    let r = repo().await;
    publish(&r, "aaaaaaaaaaaa", 1, 1).await;
    let new_dir = r.root.parent().unwrap().join("keys-2");
    let new_keys = KeySet::generate(&new_dir).unwrap();
    let current = std::fs::read(r.root.join("development/metadata/1.root.json")).unwrap();
    let rotated = rotate_root(&current, &r.keys, &new_keys, in_a_year())
        .await
        .unwrap();
    rotate_keys(&r.root, CH, &new_keys, &rotated, Expiry::for_channel(CH))
        .await
        .unwrap();
    // Right after the rotation, before any new release, the channel loads.
    load_dir(&r.root, CH, &r.first_root, false).await.unwrap();
    let staged = r.root.parent().unwrap().join("after-rotation");
    fake_release(&staged, "bbbbbbbbbbbb", 1, 2);
    if let Err(e) = publish_release(&r.root, CH, &new_keys, &staged, Expiry::for_channel(CH)).await
    {
        panic!("publish after rotation: {e}");
    }
    // A machine installed with root 1 accepts the release signed by keys 2.
    let loaded = load_dir(&r.root, CH, &r.first_root, false).await.unwrap();
    assert_eq!(
        releases(&loaded).await.unwrap()[0].manifest.release_number,
        ReleaseNumber(2)
    );
    // The old targets key alone can no longer publish.
    let staged = r.root.parent().unwrap().join("old-keys");
    fake_release(&staged, "cccccccccccc", 1, 3);
    assert!(
        publish_release(&r.root, CH, &r.keys, &staged, Expiry::for_channel(CH))
            .await
            .is_err()
            || load_dir(&r.root, CH, &r.first_root, false).await.is_err()
    );
}

#[tokio::test]
async fn um_pacote_offline_passa_pela_mesma_verificacao() {
    let r = repo().await;
    let m = publish(&r, "aaaaaaaaaaaa", 1, 1).await;
    let ocu = r.root.parent().unwrap().join("release.ocu");
    offline::pack(&r.root, CH, &m.version, &ocu).await.unwrap();
    let dest = r.root.parent().unwrap().join("unpacked");
    let u = offline::unpack(&ocu, &dest).unwrap();
    assert_eq!(u.version, m.version);
    let loaded = load(
        &r.first_root,
        dir_url(&dest.join("metadata")).unwrap(),
        dir_url(&dest.join("targets")).unwrap(),
        None,
        tough::FilesystemTransport,
        false,
    )
    .await
    .unwrap();
    let got = read_manifest(&loaded, &target_name(&m.version, MANIFEST_TARGET))
        .await
        .unwrap();
    assert_eq!(got, m);
    // The same bundle under a root this machine does not trust: refused.
    let other = tempfile::tempdir().unwrap();
    let other_root = create_root(
        &KeySet::generate(&other.path().join("k")).unwrap(),
        1,
        in_a_year(),
    )
    .await
    .unwrap();
    assert!(load(
        &other_root,
        dir_url(&dest.join("metadata")).unwrap(),
        dir_url(&dest.join("targets")).unwrap(),
        None,
        tough::FilesystemTransport,
        false
    )
    .await
    .is_err());
}

fn walk(dir: &Path) -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    for e in std::fs::read_dir(dir).unwrap() {
        let p = e.unwrap().path();
        if p.is_dir() {
            out.extend(walk(&p));
        } else {
            out.push(p);
        }
    }
    out
}
