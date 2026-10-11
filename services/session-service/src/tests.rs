//! The session rules, with the Core and the clock replaced.

use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicU32, Ordering};

use ocinye_system_contracts::capability::FreshAuthClass;
use ocinye_system_contracts::request::Secret;
use ocinye_system_contracts::session::{
    CoreLink, EndReason, SessionReason, SessionReply, SessionRequest, SessionState,
    SignedSessionPolicy,
};

use crate::engine::{
    backoff_secs, Clock, Config, Core, CoreRefusal, Engine, Refresh, GRANT_TTL_SECS,
};
use crate::policy::tests::{key, policy, sign};
use crate::verifier::Cost;

const FAST: Cost = Cost {
    memory_kib: 64,
    passes: 1,
    lanes: 1,
};
const PASSWORD: &str = "correct horse battery";

#[derive(Clone)]
struct FakeCore {
    /// What `refresh` answers.
    refresh: Rc<RefCell<Refresh>>,
    /// What `sign_in` hands out on the right password.
    issued: Rc<RefCell<SignedSessionPolicy>>,
    reachable: Rc<Cell<bool>>,
    refreshes: Rc<Cell<u32>>,
    signed_out: Rc<RefCell<Vec<String>>>,
}

impl FakeCore {
    fn new() -> Self {
        Self {
            refresh: Rc::new(RefCell::new(Refresh::Current(sign(policy(1))))),
            issued: Rc::new(RefCell::new(sign(policy(1)))),
            reachable: Rc::new(Cell::new(true)),
            refreshes: Rc::new(Cell::new(0)),
            signed_out: Rc::new(RefCell::new(Vec::new())),
        }
    }
}

impl Core for FakeCore {
    fn sign_in(
        &self,
        username: &str,
        password: &str,
        _: Option<&str>,
    ) -> Result<SignedSessionPolicy, CoreRefusal> {
        if !self.reachable.get() {
            return Err(CoreRefusal::Unreachable);
        }
        if username == "pessoa" && password == PASSWORD {
            Ok(self.issued.borrow().clone())
        } else {
            Err(CoreRefusal::InvalidCredentials)
        }
    }
    fn refresh(&self, _: &str) -> Refresh {
        self.refreshes.set(self.refreshes.get() + 1);
        if self.reachable.get() {
            self.refresh.borrow().clone()
        } else {
            Refresh::Unreachable
        }
    }
    fn sign_out(&self, session_id: &str) {
        self.signed_out.borrow_mut().push(session_id.to_owned());
    }
}

#[derive(Clone)]
struct FakeClock(Rc<Cell<u64>>);

impl Clock for FakeClock {
    fn monotonic_secs(&self) -> u64 {
        self.0.get()
    }
}

struct Bench {
    core: FakeCore,
    clock: FakeClock,
    path: PathBuf,
    engine: Engine<FakeCore, FakeClock>,
}

fn config(path: &std::path::Path, boot: &str) -> Config {
    Config {
        path: path.to_path_buf(),
        boot_id: boot.to_owned(),
        machine_id: "machine-a".to_owned(),
        instance_key: key(),
        cost: FAST,
    }
}

fn bench() -> Bench {
    static N: AtomicU32 = AtomicU32::new(0);
    let dir = std::env::temp_dir().join(format!(
        "ocinye-session-{}-{}",
        std::process::id(),
        N.fetch_add(1, Ordering::SeqCst)
    ));
    let _ = std::fs::remove_dir_all(&dir);
    let path = dir.join("session.json");
    let core = FakeCore::new();
    let clock = FakeClock(Rc::new(Cell::new(1000)));
    let engine = Engine::new(core.clone(), clock.clone(), config(&path, "boot-1"));
    Bench {
        core,
        clock,
        path,
        engine,
    }
}

impl Bench {
    fn ask(&mut self, r: SessionRequest) -> SessionReply {
        self.engine.handle(r, false)
    }
    fn sign_in(&mut self) -> SessionReply {
        self.ask(SessionRequest::SignIn {
            username: "pessoa".into(),
            password: Secret::new(PASSWORD.into()),
            second_factor: None,
        })
    }
    fn lock(&mut self) {
        self.ask(SessionRequest::Lock {});
    }
    fn unlock(&mut self, password: &str) -> SessionReply {
        self.ask(SessionRequest::Unlock {
            password: Secret::new(password.into()),
        })
    }
    fn advance(&self, secs: u64) {
        self.clock.0.set(self.clock.0.get() + secs);
    }
    fn restart(&mut self, boot: &str) {
        self.engine = Engine::new(
            self.core.clone(),
            self.clock.clone(),
            config(&self.path, boot),
        );
    }
}

fn state(r: &SessionReply) -> &SessionState {
    match r {
        SessionReply::State { state } => state,
        other => panic!("expected a state, got {other:?}"),
    }
}

fn reason(r: &SessionReply) -> SessionReason {
    match r {
        SessionReply::Refused { reason } => *reason,
        other => panic!("expected a refusal, got {other:?}"),
    }
}

#[test]
fn entrar_precisa_do_core_e_da_palavra_passe_certa() {
    let mut b = bench();
    b.core.reachable.set(false);
    assert_eq!(reason(&b.sign_in()), SessionReason::CoreUnreachable);
    b.core.reachable.set(true);
    let wrong = b.ask(SessionRequest::SignIn {
        username: "pessoa".into(),
        password: Secret::new("nope".into()),
        second_factor: None,
    });
    assert_eq!(reason(&wrong), SessionReason::InvalidCredentials);
    assert!(matches!(
        state(&b.sign_in()),
        SessionState::Active {
            core: CoreLink::Online,
            ..
        }
    ));
    assert_eq!(reason(&b.sign_in()), SessionReason::AlreadySignedIn);
}

#[test]
fn uma_politica_para_outra_maquina_nao_abre_sessao_e_o_core_e_avisado() {
    let mut b = bench();
    let mut other = policy(1);
    other.machine_id = "machine-b".into();
    *b.core.issued.borrow_mut() = sign(other);
    assert_eq!(reason(&b.sign_in()), SessionReason::PolicyInvalid);
    assert_eq!(b.core.signed_out.borrow().as_slice(), ["s-1"]);
    assert!(matches!(b.engine.state(), SessionState::SignedOut { .. }));
    assert!(!b.path.exists());
}

#[test]
fn desbloquear_funciona_com_o_core_em_baixo_quando_a_politica_permite() {
    let mut b = bench();
    b.sign_in();
    b.lock();
    b.core.reachable.set(false);
    let r = b.unlock(PASSWORD);
    assert!(matches!(
        state(&r),
        SessionState::Active {
            core: CoreLink::Unreachable,
            ..
        }
    ));
}

#[test]
fn sem_permissao_de_desbloqueio_offline_a_sessao_fica_bloqueada_e_nao_termina() {
    let mut b = bench();
    let mut strict = policy(1);
    strict.offline_unlock = false;
    *b.core.issued.borrow_mut() = sign(strict);
    b.sign_in();
    b.lock();
    b.core.reachable.set(false);
    assert_eq!(
        reason(&b.unlock(PASSWORD)),
        SessionReason::OfflineUnlockNotPermitted
    );
    assert!(matches!(b.engine.state(), SessionState::Locked { .. }));
    // The Core comes back: the same session unlocks.
    b.core.reachable.set(true);
    *b.core.refresh.borrow_mut() = Refresh::Current(b.core.issued.borrow().clone());
    assert!(matches!(
        state(&b.unlock(PASSWORD)),
        SessionState::Active {
            core: CoreLink::Online,
            ..
        }
    ));
}

#[test]
fn o_limite_de_tempo_offline_e_da_politica_e_quando_passa_a_sessao_termina() {
    let mut b = bench();
    b.sign_in();
    b.lock();
    b.core.reachable.set(false);
    b.advance(3000);
    assert!(matches!(
        state(&b.unlock(PASSWORD)),
        SessionState::Active { .. }
    ));
    b.lock();
    b.advance(700);
    assert_eq!(
        reason(&b.unlock(PASSWORD)),
        SessionReason::OfflinePolicyExpired
    );
    assert_eq!(
        b.engine.state(),
        SessionState::SignedOut {
            ended: Some(EndReason::OfflinePolicyExhausted)
        }
    );
    assert!(!b.path.exists());
}

#[test]
fn o_core_a_confirmar_repoe_o_tempo_offline() {
    let mut b = bench();
    b.sign_in();
    b.lock();
    b.core.reachable.set(false);
    b.advance(3000);
    b.unlock(PASSWORD);
    b.lock();
    b.core.reachable.set(true);
    b.advance(500);
    assert!(matches!(
        state(&b.unlock(PASSWORD)),
        SessionState::Active {
            core: CoreLink::Online,
            ..
        }
    ));
    b.lock();
    b.core.reachable.set(false);
    b.advance(3000);
    assert!(matches!(
        state(&b.unlock(PASSWORD)),
        SessionState::Active { .. }
    ));
}

#[test]
fn uma_sessao_revogada_termina_ao_ressincronizar_mesmo_com_a_palavra_passe_certa() {
    let mut b = bench();
    b.sign_in();
    b.lock();
    *b.core.refresh.borrow_mut() = Refresh::Revoked;
    assert_eq!(reason(&b.unlock(PASSWORD)), SessionReason::SessionRevoked);
    assert_eq!(
        b.engine.state(),
        SessionState::SignedOut {
            ended: Some(EndReason::Revoked)
        }
    );
}

#[test]
fn uma_epoca_de_revogacao_mais_alta_termina_a_sessao() {
    let mut b = bench();
    b.sign_in();
    b.lock();
    *b.core.refresh.borrow_mut() = Refresh::Current(sign(policy(2)));
    assert_eq!(reason(&b.unlock(PASSWORD)), SessionReason::SessionRevoked);
}

#[test]
fn uma_palavra_passe_errada_nunca_faz_perguntar_ao_core() {
    let mut b = bench();
    b.sign_in();
    b.lock();
    let before = b.core.refreshes.get();
    assert_eq!(
        reason(&b.unlock("wrong")),
        SessionReason::InvalidCredentials
    );
    assert_eq!(b.core.refreshes.get(), before);
    assert!(matches!(b.engine.state(), SessionState::Locked { .. }));
}

#[test]
fn as_tentativas_falhadas_atrasam_e_ao_limite_o_verificador_e_destruido() {
    let mut b = bench();
    b.sign_in();
    b.lock();
    assert_eq!(reason(&b.unlock("1")), SessionReason::InvalidCredentials);
    assert_eq!(reason(&b.unlock("2")), SessionReason::InvalidCredentials);
    // The third failure starts the backoff.
    assert_eq!(reason(&b.unlock("3")), SessionReason::InvalidCredentials);
    assert_eq!(
        reason(&b.unlock(PASSWORD)),
        SessionReason::Throttled {
            retry_after_secs: backoff_secs(3)
        }
    );
    b.advance(backoff_secs(3));
    assert_eq!(reason(&b.unlock("4")), SessionReason::InvalidCredentials);
    b.advance(backoff_secs(4));
    // The fifth is the policy's limit: the session ends, the right password no longer helps.
    assert_eq!(reason(&b.unlock("5")), SessionReason::InvalidCredentials);
    assert_eq!(
        b.engine.state(),
        SessionState::SignedOut {
            ended: Some(EndReason::TooManyAttempts)
        }
    );
    assert_eq!(reason(&b.unlock(PASSWORD)), SessionReason::NotSignedIn);
    assert!(!b.path.exists());
}

#[test]
fn uma_tentativa_certa_repoe_a_contagem() {
    let mut b = bench();
    b.sign_in();
    for _ in 0..3 {
        b.lock();
        b.unlock("x");
        b.unlock("y");
        assert!(matches!(
            state(&b.unlock(PASSWORD)),
            SessionState::Active { .. }
        ));
    }
}

#[test]
fn o_servico_a_reiniciar_volta_bloqueado_e_a_maquina_a_reiniciar_termina_a_sessao() {
    let mut b = bench();
    b.sign_in();
    assert!(b.path.exists());
    // Same boot: the service restarted with a session open. It comes back locked.
    b.restart("boot-1");
    assert!(matches!(b.engine.state(), SessionState::Locked { .. }));
    assert!(matches!(
        state(&b.unlock(PASSWORD)),
        SessionState::Active { .. }
    ));
    // Another boot: the session did not survive.
    b.restart("boot-2");
    assert_eq!(
        b.engine.state(),
        SessionState::SignedOut {
            ended: Some(EndReason::Restarted)
        }
    );
    assert!(!b.path.exists());
}

#[test]
fn o_atraso_sobrevive_a_um_reinicio_do_servico() {
    let mut b = bench();
    b.sign_in();
    b.lock();
    for p in ["1", "2", "3"] {
        b.unlock(p);
    }
    b.restart("boot-1");
    assert!(matches!(
        reason(&b.unlock(PASSWORD)),
        SessionReason::Throttled { .. }
    ));
}

#[test]
fn o_registo_guardado_nao_contem_a_palavra_passe_e_so_o_dono_o_le() {
    use std::os::unix::fs::PermissionsExt;
    let mut b = bench();
    b.sign_in();
    let text = std::fs::read_to_string(&b.path).unwrap();
    assert!(!text.contains(PASSWORD));
    assert!(text.contains("$argon2id$"));
    assert_eq!(
        std::fs::metadata(&b.path).unwrap().permissions().mode() & 0o777,
        0o600
    );
}

#[test]
fn terminar_a_sessao_apaga_o_verificador_e_avisa_o_core() {
    let mut b = bench();
    b.sign_in();
    assert!(matches!(
        state(&b.ask(SessionRequest::SignOut {})),
        SessionState::SignedOut {
            ended: Some(EndReason::SignedOut)
        }
    ));
    assert!(!b.path.exists());
    assert_eq!(b.core.signed_out.borrow().as_slice(), ["s-1"]);
    assert_eq!(
        reason(&b.ask(SessionRequest::Lock {})),
        SessionReason::NotSignedIn
    );
}

fn grant(r: &SessionReply) -> String {
    match r {
        SessionReply::Granted { grant } => grant.clone(),
        other => panic!("expected a grant, got {other:?}"),
    }
}

fn consume(b: &mut Bench, g: &str, actor: &str, binding: &str, class: FreshAuthClass) -> bool {
    match b.engine.handle(
        SessionRequest::ConsumeGrant {
            grant: g.into(),
            actor: actor.into(),
            binding: binding.into(),
            class,
        },
        true,
    ) {
        SessionReply::Consumed { valid } => valid,
        other => panic!("{other:?}"),
    }
}

#[test]
fn uma_concessao_serve_uma_vez_para_o_pedido_exacto_e_para_quem_a_pediu() {
    let mut b = bench();
    b.sign_in();
    let g = grant(&b.ask(SessionRequest::Confirm {
        binding: "restart".into(),
    }));
    assert!(
        !consume(
            &mut b,
            &g,
            "member:m-1",
            "shutdown",
            FreshAuthClass::Confirm
        ),
        "another request"
    );
    assert!(
        !consume(&mut b, &g, "member:m-1", "restart", FreshAuthClass::Confirm),
        "already presented"
    );
    let g = grant(&b.ask(SessionRequest::Confirm {
        binding: "restart".into(),
    }));
    assert!(
        !consume(&mut b, &g, "member:m-2", "restart", FreshAuthClass::Confirm),
        "another actor"
    );
    let g = grant(&b.ask(SessionRequest::Confirm {
        binding: "restart".into(),
    }));
    assert!(consume(
        &mut b,
        &g,
        "member:m-1",
        "restart",
        FreshAuthClass::Confirm
    ));
    assert!(
        !consume(&mut b, &g, "member:m-1", "restart", FreshAuthClass::Confirm),
        "single use"
    );
}

#[test]
fn uma_confirmacao_nao_vale_por_uma_reautenticacao_e_a_reautenticacao_vale_por_ambas() {
    let mut b = bench();
    b.sign_in();
    let g = grant(&b.ask(SessionRequest::Confirm {
        binding: "x".into(),
    }));
    assert!(!consume(
        &mut b,
        &g,
        "member:m-1",
        "x",
        FreshAuthClass::LocalReauth
    ));
    let wrong = b.ask(SessionRequest::Reauthenticate {
        binding: "x".into(),
        password: Secret::new("nope".into()),
    });
    assert_eq!(reason(&wrong), SessionReason::InvalidCredentials);
    let g = grant(&b.ask(SessionRequest::Reauthenticate {
        binding: "x".into(),
        password: Secret::new(PASSWORD.into()),
    }));
    assert!(consume(
        &mut b,
        &g,
        "member:m-1",
        "x",
        FreshAuthClass::Confirm
    ));
    let g = grant(&b.ask(SessionRequest::Reauthenticate {
        binding: "x".into(),
        password: Secret::new(PASSWORD.into()),
    }));
    assert!(
        !consume(&mut b, &g, "member:m-1", "x", FreshAuthClass::CoreStepUp),
        "never a Core step-up"
    );
}

#[test]
fn uma_concessao_expira_bloquear_apaga_as_e_so_o_servico_do_sistema_as_consome() {
    let mut b = bench();
    b.sign_in();
    let g = grant(&b.ask(SessionRequest::Confirm {
        binding: "x".into(),
    }));
    b.advance(GRANT_TTL_SECS + 1);
    assert!(!consume(
        &mut b,
        &g,
        "member:m-1",
        "x",
        FreshAuthClass::Confirm
    ));
    let g = grant(&b.ask(SessionRequest::Confirm {
        binding: "x".into(),
    }));
    b.lock();
    assert!(!consume(
        &mut b,
        &g,
        "member:m-1",
        "x",
        FreshAuthClass::Confirm
    ));
    let unprivileged = b.ask(SessionRequest::ConsumeGrant {
        grant: g,
        actor: "a".into(),
        binding: "x".into(),
        class: FreshAuthClass::Confirm,
    });
    assert_eq!(reason(&unprivileged), SessionReason::NotAuthorized);
}

#[test]
fn sem_sessao_uma_confirmacao_e_da_presenca_e_bloqueado_nao_ha_reautenticacao() {
    let mut b = bench();
    let g = grant(&b.ask(SessionRequest::Confirm {
        binding: "restart".into(),
    }));
    assert!(consume(
        &mut b,
        &g,
        "presence",
        "restart",
        FreshAuthClass::Confirm
    ));
    b.sign_in();
    b.lock();
    let locked = b.ask(SessionRequest::Reauthenticate {
        binding: "x".into(),
        password: Secret::new(PASSWORD.into()),
    });
    assert_eq!(reason(&locked), SessionReason::Locked);
    // Locked, a confirmation is presence's: the member is not at an unlocked seat.
    let g = grant(&b.ask(SessionRequest::Confirm {
        binding: "restart".into(),
    }));
    assert!(!consume(
        &mut b,
        &g,
        "member:m-1",
        "restart",
        FreshAuthClass::Confirm
    ));
}

mod socket {
    use std::os::unix::net::UnixStream;
    use std::sync::Mutex;

    use ocinye_system_contracts::wire::{read_frame, write_frame, ClientFrame, ServerFrame};
    use ocinye_system_contracts::CONTRACT_VERSION;

    use super::*;
    use crate::server::{serve, Peer};

    /// Write what a client would, serve the connection to its end, and return
    /// the client's side to read the answers from.
    fn talk(peer: Peer, hello: &ClientFrame, requests: &[SessionRequest]) -> UnixStream {
        let (mut client, mut service) = UnixStream::pair().unwrap();
        write_frame(&mut client, hello).unwrap();
        for r in requests {
            write_frame(&mut client, r).unwrap();
        }
        client.shutdown(std::net::Shutdown::Write).unwrap();
        let engine = Mutex::new(bench().engine);
        serve(&mut service, peer, &engine);
        client
    }

    fn hello() -> ClientFrame {
        ClientFrame::Hello {
            contract: CONTRACT_VERSION,
        }
    }

    fn consume() -> SessionRequest {
        SessionRequest::ConsumeGrant {
            grant: "g".into(),
            actor: "member:m".into(),
            binding: "b".into(),
            class: FreshAuthClass::Confirm,
        }
    }

    #[test]
    fn sem_aperto_de_mao_nao_ha_conversa() {
        let mut c = talk(
            Peer::SystemInterface,
            &ClientFrame::Subscribe {},
            &[SessionRequest::State {}],
        );
        assert!(matches!(
            read_frame::<_, ServerFrame>(&mut c).unwrap(),
            ServerFrame::Rejected { .. }
        ));
        assert!(read_frame::<_, SessionReply>(&mut c).is_err());
    }

    #[test]
    fn outro_contrato_e_recusado_no_aperto_de_mao() {
        let mut c = talk(
            Peer::Privileged,
            &ClientFrame::Hello {
                contract: CONTRACT_VERSION + 1,
            },
            &[],
        );
        assert!(matches!(
            read_frame::<_, ServerFrame>(&mut c).unwrap(),
            ServerFrame::Rejected { .. }
        ));
    }

    #[test]
    fn um_par_desconhecido_nao_recebe_nem_o_estado() {
        let mut c = talk(Peer::Other, &hello(), &[SessionRequest::State {}]);
        assert!(matches!(
            read_frame::<_, ServerFrame>(&mut c).unwrap(),
            ServerFrame::Welcome { .. }
        ));
        assert_eq!(
            reason(&read_frame(&mut c).unwrap()),
            SessionReason::NotAuthorized
        );
    }

    #[test]
    fn so_o_system_service_consome_provas() {
        let mut c = talk(
            Peer::SystemInterface,
            &hello(),
            &[SessionRequest::State {}, consume()],
        );
        let _: ServerFrame = read_frame(&mut c).unwrap();
        assert!(matches!(
            state(&read_frame(&mut c).unwrap()),
            SessionState::SignedOut { .. }
        ));
        assert_eq!(
            reason(&read_frame(&mut c).unwrap()),
            SessionReason::NotAuthorized
        );

        let mut c = talk(Peer::Privileged, &hello(), &[consume()]);
        let _: ServerFrame = read_frame(&mut c).unwrap();
        assert_eq!(
            read_frame::<_, SessionReply>(&mut c).unwrap(),
            SessionReply::Consumed { valid: false }
        );
    }
}
