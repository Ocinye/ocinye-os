//! The Update Engine against a real signed repository served over HTTP, on an
//! in-memory host: N → N+1, refusals, rollback, offline, resume, and a power
//! cut at every step of the way.

mod common;

use common::*;
use ocinye_update_contracts::manifest::MigrationPolicy;
use ocinye_update_contracts::{Phase, UpdateOutcome, UpdateReason};
use ocinye_update_engine::native::pointer;
use ocinye_update_engine::{Engine, Fault, Stop};
use std::sync::Arc;
use std::time::Duration;

const FAST: (u32, Duration) = (2, Duration::from_millis(10));

fn engine(b: &Bench) -> Engine<FakeHost> {
    Engine::open_with(b.paths.clone(), b.host.clone(), facts(), None, FAST)
        .unwrap_or_else(|s| panic!("open: {s:?}"))
}

fn engine_crashing_at(b: &Bench, at: Fault) -> Engine<FakeHost> {
    Engine::open_with(
        b.paths.clone(),
        b.host.clone(),
        facts(),
        Some(Box::new(move |f| f == at)),
        FAST,
    )
    .unwrap()
}

fn run_through(e: &Engine<FakeHost>) -> Result<(), Stop> {
    e.check()?;
    e.download("member-1", "settings")?;
    e.install("member-1", "settings")?;
    Ok(())
}

fn current(b: &Bench) -> Option<String> {
    pointer(&b.paths.current())
}

#[test]
fn n_para_n_mais_1_sem_instancia_e_sem_reinstalar() {
    let b = Bench::new(false);
    let r = b.publish_n1();
    let e = engine(&b);
    assert_eq!(e.status().current.unwrap().version, b.n);
    let s = e.check().unwrap();
    assert_eq!(s.phase, Phase::UpdateAvailable);
    assert_eq!(s.available.as_ref().unwrap().version, r.manifest.version);
    let s = e.download("member-1", "settings").unwrap();
    assert_eq!(s.phase, Phase::AwaitingInstall);
    let s = e.install("member-1", "settings").unwrap();
    assert_eq!(s.phase, Phase::Completed);
    assert_eq!(s.current.unwrap().version, r.manifest.version);
    assert_eq!(s.previous.unwrap().version, b.n);
    assert_eq!(current(&b).unwrap(), r.manifest.version.to_string());
    assert_eq!(pointer(&b.paths.previous()).unwrap(), b.n.to_string());
    // The machine's carried release (local activation) followed.
    assert!(b.paths.carried_release.join(N1_ID).is_dir());
    assert!(!b.paths.carried_release.join(N_ID).exists());
    // Units of the new release, services restarted in order, the engine last.
    let unit = std::fs::read_to_string(b.paths.units.join("ocinye-system.service")).unwrap();
    assert!(unit.contains(&r.manifest.version.to_string()));
    let calls = b.host.calls();
    let i = calls
        .iter()
        .position(|c| c == "restart ocinye-session.service")
        .unwrap();
    let j = calls
        .iter()
        .position(|c| c == "restart ocinye-system.service")
        .unwrap();
    assert!(i < j);
    assert_eq!(calls.last().unwrap(), "restart_engine");
    // The new engine is in place; the old one is kept for a rollback.
    let new_engine =
        std::fs::read_to_string(b.paths.current().join("bin/ocinye-update-engine")).unwrap();
    assert_eq!(new_engine, "engine N+1");
    let old_engine =
        std::fs::read_to_string(b.paths.previous().join("bin/ocinye-update-engine")).unwrap();
    assert_eq!(old_engine, "engine N");
    let h = e.history();
    assert_eq!(h.entries.len(), 1);
    assert_eq!(h.entries[0].outcome, UpdateOutcome::Committed);
    assert_eq!(h.entries[0].via, "settings");
    // Retention: downloads and partials gone after the commit.
    assert!(!b.paths.state.join("verified").exists());
}

#[test]
fn com_instancia_o_instalador_do_release_move_a_instancia() {
    let b = Bench::new(true);
    let r = b.publish_n1();
    let e = engine(&b);
    run_through(&e).unwrap();
    let env = std::fs::read_to_string(&b.paths.release_env).unwrap();
    assert!(env.contains(N1_ID), "{env}");
    let calls = b.host.calls();
    assert!(calls.iter().any(|c| c.starts_with("load_image")));
    assert!(calls.iter().any(|c| c == "instance_upgrade"));
    assert_eq!(e.status().current.unwrap().version, r.manifest.version);
}

#[test]
fn o_instalador_falha_e_a_instancia_fica_no_release_que_corria() {
    let b = Bench::new(true);
    b.publish_n1();
    b.host.state.lock().unwrap().instance_upgrade_fails = true;
    let e = engine(&b);
    e.check().unwrap();
    e.download("member-1", "cli").unwrap();
    assert_eq!(
        e.install("member-1", "cli").unwrap_err(),
        Stop::Reason(UpdateReason::InstanceUpgradeFailed)
    );
    let s = e.status();
    assert_eq!(s.phase, Phase::InstallFailed);
    assert_eq!(s.current.unwrap().version, b.n);
    assert_eq!(current(&b).unwrap(), b.n.to_string());
    assert!(std::fs::read_to_string(&b.paths.release_env)
        .unwrap()
        .contains(N_ID));
}

#[test]
fn um_release_que_falha_a_saude_e_revertido_sozinho() {
    let b = Bench::new(true);
    let r = b.publish(N1_ID, 1, 2, "engine bad", false, MigrationPolicy::Expand);
    b.host.state.lock().unwrap().unhealthy_version = Some(r.manifest.version.to_string());
    let e = engine(&b);
    e.check().unwrap();
    e.download("member-1", "settings").unwrap();
    assert_eq!(
        e.install("member-1", "settings").unwrap_err(),
        Stop::Reason(UpdateReason::HealthGateFailed)
    );
    let s = e.status();
    assert_eq!(s.phase, Phase::RolledBack);
    assert_eq!(s.last_outcome, Some(UpdateOutcome::RolledBack));
    assert_eq!(s.current.unwrap().version, b.n);
    assert_eq!(current(&b).unwrap(), b.n.to_string());
    // The Instance went back with its own checkpoint.
    assert!(b.host.calls().iter().any(|c| c == "instance_rollback"));
    assert!(std::fs::read_to_string(&b.paths.release_env)
        .unwrap()
        .contains(N_ID));
    // The failed release is not a rollback target and its files are gone.
    assert_ne!(
        pointer(&b.paths.previous()),
        Some(r.manifest.version.to_string())
    );
    assert!(!b
        .paths
        .native_release(&r.manifest.version.to_string())
        .exists());
    assert_eq!(e.history().entries[0].outcome, UpdateOutcome::RolledBack);
}

#[test]
fn uma_assinatura_errada_e_recusada_antes_de_tudo() {
    let b = Bench::new(false);
    b.publish_n1();
    // The machine trusts another root.
    let other = tempfile::tempdir().unwrap();
    let keys = ocinye_update_repository::keys::KeySet::generate(&other.path().join("k")).unwrap();
    let expires = jiff::Timestamp::now()
        .checked_add(jiff::SignedDuration::from_hours(240))
        .unwrap();
    let root = rt()
        .block_on(ocinye_update_repository::keys::create_root(
            &keys, 1, expires,
        ))
        .unwrap();
    std::fs::write(b.paths.trust.join("development/root.json"), root).unwrap();
    let e = engine(&b);
    assert_eq!(
        e.check().unwrap_err(),
        Stop::Reason(UpdateReason::MetadataUntrusted)
    );
    let s = e.status();
    assert_eq!(s.phase, Phase::Idle);
    assert!(s.available.is_none());
    assert_eq!(s.reason, Some(UpdateReason::MetadataUntrusted));
    assert_eq!(current(&b).unwrap(), b.n.to_string());
}

#[test]
fn um_artefacto_alterado_no_espelho_nao_passa_da_verificacao() {
    let b = Bench::new(false);
    let r = b.publish_n1();
    let e = engine(&b);
    e.check().unwrap();
    // A mirror changes the native layer after publication.
    let native = r.manifest.artifact("native/native-amd64.tar").unwrap();
    let path = b.repo.join("development/targets").join(format!(
        "{}.releases/{}/native/native-amd64.tar",
        native.sha256, r.manifest.version
    ));
    let mut bytes = std::fs::read(&path).unwrap();
    bytes[100] ^= 0xff;
    std::fs::write(&path, bytes).unwrap();
    assert_eq!(
        e.download("member-1", "cli").unwrap_err(),
        Stop::Reason(UpdateReason::ArtifactMismatch)
    );
    let s = e.status();
    assert_eq!(s.phase, Phase::DownloadFailed);
    assert!(!b
        .paths
        .native_release(&r.manifest.version.to_string())
        .exists());
    assert_eq!(current(&b).unwrap(), b.n.to_string());
}

#[test]
fn metadados_antigos_sao_recusados_depois_de_vistos_os_novos() {
    let b = Bench::new(false);
    b.publish_n1();
    let old = b.tmp.path().join("old-repo");
    copy_tree(&b.repo, &old);
    b.publish(N1_ID, 2, 3, "engine N+2", false, MigrationPolicy::Expand);
    let e = engine(&b);
    e.check().unwrap();
    // The mirror now serves the old repository.
    std::fs::remove_dir_all(&b.repo).unwrap();
    copy_tree(&old, &b.repo);
    assert_eq!(
        e.check().unwrap_err(),
        Stop::Reason(UpdateReason::MetadataRollback)
    );
}

#[test]
fn um_espelho_congelado_e_recusado() {
    let b = Bench::new(false);
    let staged = b.tmp.path().join("frozen");
    stage(&staged, N1_ID, 1, 2, 1, "e", false, MigrationPolicy::Expand);
    let expired = ocinye_update_repository::publish::Expiry {
        targets_days: 30,
        snapshot_days: 30,
        timestamp_days: -1,
    };
    rt().block_on(ocinye_update_repository::publish::publish_release(
        &b.repo, CH, &b.keys, &staged, expired,
    ))
    .unwrap();
    let e = engine(&b);
    assert_eq!(
        e.check().unwrap_err(),
        Stop::Reason(UpdateReason::MetadataExpired)
    );
}

#[test]
fn a_transferencia_cai_e_retoma_do_ponto_onde_parou() {
    let b = Bench::new(false);
    b.publish_n1();
    *b.server.control.drop_once.lock().unwrap() = Some(("native-amd64.tar".into(), 512));
    let e = engine(&b);
    e.check().unwrap();
    let s = e.download("member-1", "cli").unwrap();
    assert_eq!(s.phase, Phase::AwaitingInstall);
    let seen = b.server.control.seen.lock().unwrap().clone();
    assert!(
        seen.iter()
            .any(|(p, r)| p.ends_with("native-amd64.tar") && r.as_deref() == Some("bytes=512-")),
        "no resumed request: {seen:?}"
    );
}

#[test]
fn um_fragmento_corrompido_e_deitado_fora_e_volta_a_descarregar() {
    let b = Bench::new(false);
    let r = b.publish_n1();
    let e = engine(&b);
    e.check().unwrap();
    let native = r.manifest.artifact("native/native-amd64.tar").unwrap();
    std::fs::create_dir_all(b.paths.partial()).unwrap();
    std::fs::write(b.paths.partial().join(&native.sha256), vec![0u8; 700]).unwrap();
    // The resumed file is wrong as a whole: refused, partial discarded…
    assert_eq!(
        e.download("member-1", "cli").unwrap_err(),
        Stop::Reason(UpdateReason::ArtifactMismatch)
    );
    assert!(!b.paths.partial().join(&native.sha256).exists());
    // …and the next attempt downloads it whole.
    assert_eq!(
        e.download("member-1", "cli").unwrap().phase,
        Phase::AwaitingInstall
    );
}

#[test]
fn o_repositorio_em_baixo_deixa_tudo_como_estava() {
    let b = Bench::new(false);
    b.publish_n1();
    b.server
        .control
        .down
        .store(true, std::sync::atomic::Ordering::SeqCst);
    let e = engine(&b);
    assert_eq!(
        e.check().unwrap_err(),
        Stop::Reason(UpdateReason::RepositoryUnreachable)
    );
    assert_eq!(e.status().phase, Phase::Idle);
    assert_eq!(current(&b).unwrap(), b.n.to_string());
}

#[test]
fn sem_espaco_nao_se_comeca() {
    let b = Bench::new(false);
    b.publish_n1();
    b.host.state.lock().unwrap().free = Some(10);
    let e = engine(&b);
    e.check().unwrap();
    assert_eq!(
        e.download("member-1", "cli").unwrap_err(),
        Stop::Reason(UpdateReason::InsufficientSpace)
    );
    assert_eq!(current(&b).unwrap(), b.n.to_string());
}

#[test]
fn uma_contracao_nao_e_oferecida_a_quem_ainda_depende_do_que_ela_retira() {
    let b = Bench::new(false);
    let staged = b.tmp.path().join("contract");
    let r = stage(
        &staged,
        N1_ID,
        1,
        3,
        2,
        "e",
        false,
        MigrationPolicy::Contract { requires_source: 2 },
    );
    rt().block_on(ocinye_update_repository::publish::publish_release(
        &b.repo,
        CH,
        &b.keys,
        &staged,
        ocinye_update_repository::publish::Expiry::for_channel(CH),
    ))
    .unwrap();
    let e = engine(&b);
    // The machine is on number 1; the release needs 2 first.
    assert!(matches!(
        e.check().unwrap_err(),
        Stop::Reason(UpdateReason::SourceTooOld | UpdateReason::MigrationIncompatible)
    ));
    assert!(e.status().available.is_none());
    let _ = r;
}

#[test]
fn um_pacote_offline_segue_o_mesmo_caminho() {
    let b = Bench::new(false);
    let r = b.publish_n1();
    let ocu = b.paths.inbox().join("ocinye-os-update.ocu");
    std::fs::create_dir_all(b.paths.inbox()).unwrap();
    rt().block_on(ocinye_update_repository::offline::pack(
        &b.repo,
        CH,
        &r.manifest.version,
        &ocu,
    ))
    .unwrap();
    // No network at all.
    b.server
        .control
        .down
        .store(true, std::sync::atomic::Ordering::SeqCst);
    let e = engine(&b);
    assert_eq!(e.status().offline_bundles.len(), 1);
    assert_eq!(
        e.import("ocinye-os-update.ocu").unwrap().phase,
        Phase::UpdateAvailable
    );
    e.download("member-1", "cli").unwrap();
    assert_eq!(
        e.install("member-1", "cli").unwrap().phase,
        Phase::Completed
    );
    assert_eq!(e.status().current.unwrap().version, r.manifest.version);
}

#[test]
fn um_pacote_offline_de_outra_raiz_e_recusado() {
    let b = Bench::new(false);
    // Another publisher, another root, the same release.
    let other = Bench::new(false);
    let r = other.publish_n1();
    std::fs::create_dir_all(b.paths.inbox()).unwrap();
    let ocu = b.paths.inbox().join("forged.ocu");
    rt().block_on(ocinye_update_repository::offline::pack(
        &other.repo,
        CH,
        &r.manifest.version,
        &ocu,
    ))
    .unwrap();
    let e = engine(&b);
    assert_eq!(
        e.import("forged.ocu").unwrap_err(),
        Stop::Reason(UpdateReason::MetadataUntrusted)
    );
    assert!(e.status().available.is_none());
}

#[test]
fn o_instalador_do_pacote_descarregado_e_executavel() {
    use std::os::unix::fs::PermissionsExt;
    let b = Bench::new(true);
    let r = b.publish_n1();
    let e = engine(&b);
    e.check().unwrap();
    e.download("member-1", "settings").unwrap();
    let installer = b
        .paths
        .verified(&r.manifest.version.to_string())
        .join("bundle/install/ocinye");
    let mode = std::fs::metadata(&installer).unwrap().permissions().mode();
    assert_eq!(mode & 0o777, 0o755, "{}", installer.display());
}

#[test]
fn reverter_a_mao_volta_ao_release_anterior() {
    let b = Bench::new(true);
    let r = b.publish_n1();
    let e = engine(&b);
    run_through(&e).unwrap();
    let s = e.rollback("member-1", "settings").unwrap();
    assert_eq!(s.phase, Phase::RolledBack);
    assert_eq!(s.current.unwrap().version, b.n);
    assert_eq!(s.previous.unwrap().version, r.manifest.version);
    assert_eq!(current(&b).unwrap(), b.n.to_string());
    assert!(std::fs::read_to_string(&b.paths.release_env)
        .unwrap()
        .contains(N_ID));
}

#[test]
fn a_entrada_de_recuperacao_do_arranque_volta_ao_anterior_uma_vez_por_arranque() {
    let b = Bench::new(true);
    let r = b.publish_n1();
    let e = engine(&b);
    // Nothing asked: nothing done.
    assert!(e
        .recovery_request("BOOT_IMAGE=/vmlinuz root=UUID=x ro")
        .is_none());
    // Asked with nothing to return to: refused, nothing changed.
    let before = current(&b);
    let refused = e
        .recovery_request("root=UUID=x ro ocinye.update=rollback")
        .unwrap();
    assert!(matches!(
        refused,
        Err(Stop::Reason(UpdateReason::RollbackUnsafe))
    ));
    assert_eq!(current(&b), before);

    run_through(&e).unwrap();
    assert_eq!(current(&b).unwrap(), r.manifest.version.to_string());
    // A new boot, from the recovery entry.
    b.host.state.lock().unwrap().boot_id = "boot-2".into();
    // A parameter after `--` is for init, not for this.
    assert!(e.recovery_request("ro -- ocinye.update=rollback").is_none());
    let s = e
        .recovery_request("root=UUID=x ro ocinye.update=rollback")
        .unwrap()
        .unwrap();
    assert_eq!(s.phase, Phase::RolledBack);
    assert_eq!(current(&b).unwrap(), b.n.to_string());
    assert_eq!(e.history().entries[0].via, "recovery");
    // The audit says who: the boot menu, not a member.
    let audit = std::fs::read_to_string(b.paths.audit()).unwrap();
    let last: serde_json::Value = serde_json::from_str(audit.lines().last().unwrap()).unwrap();
    assert_eq!(last["event"], "rollback");
    assert_eq!(last["actor"], "boot-menu");
    assert_eq!(last["to"], b.n.to_string());
    // The engine restarts itself after a rollback: the same boot does not
    // roll back again.
    let again = engine(&b);
    assert!(again
        .recovery_request("root=UUID=x ro ocinye.update=rollback")
        .is_none());
    assert_eq!(current(&b).unwrap(), b.n.to_string());
}

#[test]
fn cancelar_antes_de_instalar_nao_deixa_nada() {
    let b = Bench::new(false);
    let r = b.publish_n1();
    let e = engine(&b);
    e.check().unwrap();
    e.download("member-1", "cli").unwrap();
    assert!(b
        .paths
        .native_release(&r.manifest.version.to_string())
        .exists());
    let s = e.cancel().unwrap();
    assert_eq!(s.phase, Phase::UpdateAvailable);
    assert!(!b
        .paths
        .native_release(&r.manifest.version.to_string())
        .exists());
    assert!(!b.paths.state.join("verified").exists());
    assert_eq!(current(&b).unwrap(), b.n.to_string());
}

#[test]
fn um_release_que_pede_reinicio_so_se_confirma_depois_de_um_arranque_saudavel() {
    let b = Bench::new(false);
    let r = b.publish(N1_ID, 1, 2, "engine N+1", true, MigrationPolicy::Expand);
    let e = engine(&b);
    run_through(&e).unwrap();
    let s = e.status();
    assert_eq!(s.phase, Phase::AwaitingReboot);
    assert!(s.restart_required);
    drop(e);
    // The engine restarts without a reboot: still waiting.
    let e = engine(&b);
    assert_eq!(e.status().phase, Phase::AwaitingReboot);
    drop(e);
    // The machine restarts: a new boot validates and commits.
    b.host.state.lock().unwrap().boot_id = "boot-2".into();
    let e = engine(&b);
    let s = e.status();
    assert_eq!(s.phase, Phase::Completed);
    assert_eq!(s.current.unwrap().version, r.manifest.version);
    assert!(e.history().entries[0].rebooted);
}

#[test]
fn um_arranque_que_falha_tres_vezes_reverte() {
    let b = Bench::new(false);
    let r = b.publish(N1_ID, 1, 2, "engine N+1", true, MigrationPolicy::Expand);
    let e = engine(&b);
    run_through(&e).unwrap();
    drop(e);
    b.host.state.lock().unwrap().unhealthy_version = Some(r.manifest.version.to_string());
    for boot in 2..=4 {
        b.host.state.lock().unwrap().boot_id = format!("boot-{boot}");
        let e = engine(&b);
        drop(e);
    }
    let e = engine(&b);
    let s = e.status();
    assert_eq!(s.phase, Phase::RolledBack, "{s:?}");
    assert_eq!(current(&b).unwrap(), b.n.to_string());
}

/// The power-cut matrix: the process stops at every step; a new engine
/// reconciles; the machine is on the old release whole or on the new one
/// whole, never between; and an update then completes.
#[test]
fn um_corte_de_energia_em_cada_passo_nunca_deixa_meio_release() {
    let faults = [
        Fault::AfterManifest,
        Fault::MidArtifacts,
        Fault::AfterVerify,
        Fault::AfterStage,
        Fault::AfterEntered,
        Fault::AfterImages,
        Fault::AfterInstance,
        Fault::AfterNativeSwitch,
        Fault::AfterUnits,
        Fault::AfterRestart,
        Fault::BeforeCommit,
        Fault::InCommit,
    ];
    for fault in faults {
        let b = Bench::new(true);
        let r = b.publish_n1();
        let e = engine_crashing_at(&b, fault);
        let stop = run_through(&e).unwrap_err();
        assert_eq!(stop, Stop::Crashed(fault), "{fault:?}");
        drop(e);
        // The machine starts again.
        let e = engine(&b);
        let s = e.status();
        let on_new = current(&b).as_deref() == Some(&*r.manifest.version.to_string());
        let env = std::fs::read_to_string(&b.paths.release_env).unwrap();
        match s.phase {
            Phase::Completed => {
                assert!(on_new, "{fault:?}: committed but not on N+1");
                assert!(env.contains(N1_ID), "{fault:?}");
            }
            Phase::DownloadFailed
            | Phase::AwaitingInstall
            | Phase::RolledBack
            | Phase::UpdateAvailable => {
                assert!(!on_new, "{fault:?}: not committed but on N+1");
                assert_eq!(current(&b).unwrap(), b.n.to_string(), "{fault:?}");
                assert!(env.contains(N_ID), "{fault:?}: Instance left on {env}");
            }
            other => panic!("{fault:?}: left in {other:?}"),
        }
        // A cut during the install is said to be one: the health gate never
        // ran, and the record does not claim it failed.
        if matches!(
            fault,
            Fault::AfterEntered
                | Fault::AfterImages
                | Fault::AfterInstance
                | Fault::AfterNativeSwitch
                | Fault::AfterUnits
                | Fault::AfterRestart
        ) {
            assert_eq!(
                s.reason,
                Some(UpdateReason::InstallInterrupted),
                "{fault:?}: {:?}",
                s.phase
            );
        }
        // From there, an update completes.
        if s.phase != Phase::Completed {
            if s.phase != Phase::AwaitingInstall {
                if s.phase == Phase::RolledBack {
                    e.check().unwrap();
                }
                e.download("member-1", "cli")
                    .unwrap_or_else(|x| panic!("{fault:?}: {x:?}"));
            }
            e.install("member-1", "cli")
                .unwrap_or_else(|x| panic!("{fault:?}: {x:?}"));
        }
        let s = e.status();
        assert_eq!(s.phase, Phase::Completed, "{fault:?}");
        assert_eq!(
            current(&b).unwrap(),
            r.manifest.version.to_string(),
            "{fault:?}"
        );
    }
}

#[test]
fn dois_pedidos_longos_nao_correm_ao_mesmo_tempo() {
    let b = Bench::new(false);
    b.publish_n1();
    let e = Arc::new(engine(&b));
    e.reserve("download").unwrap();
    assert_eq!(e.reserve("install"), Err(UpdateReason::MaintenanceBusy));
    e.release();
    assert!(e.reserve("install").is_ok());
}

fn copy_tree(from: &std::path::Path, to: &std::path::Path) {
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
