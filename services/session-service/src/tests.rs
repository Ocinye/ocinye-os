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
    backoff_secs, Clock, Config, Core, CoreRefusal, Engine, InstanceKey, Refresh, GRANT_TTL_SECS,
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
    /// The recovery codes the next sign-in comes with.
    codes: Rc<RefCell<Vec<String>>>,
}

impl FakeCore {
    fn new() -> Self {
        Self {
            refresh: Rc::new(RefCell::new(Refresh::Current(sign(policy(1))))),
            issued: Rc::new(RefCell::new(sign(policy(1)))),
            reachable: Rc::new(Cell::new(true)),
            refreshes: Rc::new(Cell::new(0)),
            signed_out: Rc::new(RefCell::new(Vec::new())),
            codes: Rc::new(RefCell::new(Vec::new())),
        }
    }
}

impl Core for FakeCore {
    fn sign_in(
        &self,
        username: &str,
        password: &str,
        _: Option<&str>,
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
    fn begin_enrollment(&self, _: &str, password: &str) -> Result<String, CoreRefusal> {
        if password == PASSWORD {
            Ok("JBSWY3DPEHPK3PXP".to_owned())
        } else {
            Err(CoreRefusal::InvalidCredentials)
        }
    }
    fn take_recovery_codes(&self) -> Vec<String> {
        std::mem::take(&mut *self.codes.borrow_mut())
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
        instance_key: InstanceKey::Fixed(key()),
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
            new_password: None,
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
        new_password: None,
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

#[test]
fn o_verificador_local_e_da_palavra_passe_escolhida_no_primeiro_acesso() {
    let mut b = bench();
    let chosen = "a escolha de quem entra";
    let reply = b.ask(SessionRequest::SignIn {
        username: "pessoa".into(),
        password: Secret::new(PASSWORD.into()),
        second_factor: None,
        new_password: Some(Secret::new(chosen.into())),
    });
    assert!(matches!(state(&reply), SessionState::Active { .. }));
    b.lock();
    // The temporary credential no longer opens anything, here either.
    assert_eq!(
        reason(&b.unlock(PASSWORD)),
        SessionReason::InvalidCredentials
    );
    assert!(matches!(
        state(&b.unlock(chosen)),
        SessionState::Active { .. }
    ));
}

#[test]
fn quem_entra_a_inscrever_o_segundo_factor_recebe_os_codigos_com_a_sessao() {
    let mut b = bench();
    *b.core.codes.borrow_mut() = vec!["aaaa-bbbb".to_owned()];
    match b.sign_in() {
        SessionReply::Enrolled {
            state,
            recovery_codes,
        } => {
            assert!(matches!(state, SessionState::Active { .. }));
            assert_eq!(recovery_codes.len(), 1);
            assert_eq!(recovery_codes[0].expose(), "aaaa-bbbb");
        }
        other => panic!("expected the enrolment's codes, got {other:?}"),
    }
    // Once: the session that follows is an ordinary one.
    b.ask(SessionRequest::SignOut {});
    assert!(matches!(b.sign_in(), SessionReply::State { .. }));
}

#[test]
fn a_chave_do_autenticador_so_se_pede_sem_sessao_aberta() {
    let mut b = bench();
    let begin = || SessionRequest::BeginSecondFactorEnrollment {
        username: "pessoa".into(),
        password: Secret::new(PASSWORD.into()),
    };
    match b.ask(begin()) {
        SessionReply::SecondFactorSecret { secret_base32 } => {
            assert_eq!(secret_base32.expose(), "JBSWY3DPEHPK3PXP")
        }
        other => panic!("expected the key, got {other:?}"),
    }
    b.sign_in();
    assert_eq!(reason(&b.ask(begin())), SessionReason::AlreadySignedIn);
}

#[test]
fn a_chave_da_instancia_le_se_do_ficheiro_de_cada_vez() {
    // The service starts before the seat is linked: no key, no session. Once
    // the link writes the key, sign-in works with no restart in between.
    let mut b = bench();
    let file = b.path.with_file_name("instance-session.pub");
    let mut config = config(&b.path, "boot-1");
    config.instance_key = InstanceKey::File(file.clone());
    b.engine = Engine::new(b.core.clone(), b.clock.clone(), config);
    assert_eq!(reason(&b.sign_in()), SessionReason::PolicyInvalid);
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    std::fs::write(&file, "not a key\n").unwrap();
    assert_eq!(reason(&b.sign_in()), SessionReason::PolicyInvalid);
    std::fs::write(&file, format!("{}\n", hex::encode(key()))).unwrap();
    assert!(matches!(state(&b.sign_in()), SessionState::Active { .. }));
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

mod core_link {
    use std::io::{BufRead, BufReader, Read, Write};
    use std::net::TcpListener;
    use std::sync::{Arc, Mutex};

    use super::*;
    use crate::core_link::HttpCore;

    /// What the fake Core is told to answer, and what it was asked.
    #[derive(Default)]
    struct Script {
        /// The state `/auth/login` reports for the right password.
        login_state: &'static str,
        /// Whether `/auth/machine-session` still knows the session.
        revoked: bool,
        /// What `/auth/mfa` says the account's second factor needs.
        mfa_mode: &'static str,
        asked: Vec<String>,
    }

    /// A Core that speaks just enough HTTP for the link, on a loopback port.
    fn fake_core(script: Arc<Mutex<Script>>) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { return };
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                let path = line.split(' ').nth(1).unwrap_or("").to_owned();
                let (mut length, mut cookie) = (0, String::new());
                loop {
                    let mut header = String::new();
                    reader.read_line(&mut header).unwrap();
                    let header = header.trim().to_ascii_lowercase();
                    if header.is_empty() {
                        break;
                    }
                    if let Some(v) = header.strip_prefix("content-length:") {
                        length = v.trim().parse().unwrap();
                    }
                    if let Some(v) = header.strip_prefix("cookie:") {
                        v.trim().clone_into(&mut cookie);
                    }
                }
                let mut body = vec![0; length];
                reader.read_exact(&mut body).unwrap();
                let body: serde_json::Value = serde_json::from_slice(&body).unwrap_or_default();
                let mut script = script.lock().unwrap();
                script.asked.push(path.clone());
                let (status, answer) = match path.as_str() {
                    "/auth/login" if body["password"] == PASSWORD => (
                        200,
                        serde_json::json!({ "session_token": "gate", "state": script.login_state })
                            .to_string(),
                    ),
                    "/auth/password"
                        if cookie.ends_with("=gate")
                            && body["password"] == body["confirmation"] =>
                    {
                        if body["password"].as_str().is_some_and(|p| p.len() >= 15) {
                            (
                                200,
                                serde_json::json!({ "session_token": "assured", "state": "active" })
                                    .to_string(),
                            )
                        } else {
                            (422, "{}".to_owned())
                        }
                    }
                    "/auth/mfa/challenge"
                        if cookie.ends_with("=gate") && body["code"] == "123456" =>
                    {
                        (
                            200,
                            serde_json::json!({ "session_token": "assured", "state": "active" })
                                .to_string(),
                        )
                    }
                    "/auth/machine-session"
                        if !script.revoked && body["machine_id"] == "machine-a" =>
                    {
                        (200, serde_json::to_string(&sign(policy(1))).unwrap())
                    }
                    "/auth/mfa" if cookie.ends_with("=gate") => (
                        200,
                        serde_json::json!({ "mfa_mode": script.mfa_mode }).to_string(),
                    ),
                    "/auth/mfa/enroll?reveal=true" if cookie.ends_with("=gate") => (
                        200,
                        serde_json::json!({ "otpauth_uri": "otpauth://totp/x", "secret_base32": "JBSWY3DPEHPK3PXP" })
                            .to_string(),
                    ),
                    "/auth/mfa/enroll/confirm" if cookie.ends_with("=gate") && body["code"] == "123456" => (
                        200,
                        serde_json::json!({ "recovery_codes": ["aaaa-bbbb", "cccc-dddd"] }).to_string(),
                    ),
                    "/auth/mfa/acknowledge" if cookie.ends_with("=gate") => (
                        200,
                        serde_json::json!({ "session_token": "assured", "state": "active" }).to_string(),
                    ),
                    "/auth/logout" => (204, String::new()),
                    _ => (401, "{}".to_owned()),
                };
                let _ = write!(
                    stream,
                    "HTTP/1.1 {status} X\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{answer}",
                    answer.len()
                );
            }
        });
        base
    }

    fn link(state: &'static str) -> (HttpCore, Arc<Mutex<Script>>, PathBuf) {
        let script = Arc::new(Mutex::new(Script {
            login_state: state,
            ..Script::default()
        }));
        let path = bench().path.with_file_name("core-link.json");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let core =
            HttpCore::new(&fake_core(Arc::clone(&script)), "machine-a", path.clone()).unwrap();
        (core, script, path)
    }

    #[test]
    fn entrar_usa_o_login_do_core_e_pede_a_politica_assinada() {
        let (core, script, path) = link("active");
        assert_eq!(
            core.sign_in("pessoa", "errada", None, None),
            Err(CoreRefusal::InvalidCredentials)
        );
        let signed = core.sign_in("pessoa", PASSWORD, None, None).unwrap();
        assert_eq!(signed, sign(policy(1)));
        // The token is kept where only the service reads it, and survives it.
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        let again = HttpCore::new(&core_base(&script), "machine-a", path).unwrap();
        assert_eq!(again.refresh("s-1"), Refresh::Unreachable);
        assert_eq!(core.refresh("s-1"), Refresh::Current(sign(policy(1))));
    }

    /// A base nothing listens on: the reloaded link has the token but no Core.
    fn core_base(_: &Arc<Mutex<Script>>) -> String {
        let port = TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        format!("http://127.0.0.1:{port}")
    }

    #[test]
    fn o_segundo_factor_e_pedido_e_depois_satisfeito() {
        let (core, script, _) = link("mfa_required");
        assert_eq!(
            core.sign_in("pessoa", PASSWORD, None, None),
            Err(CoreRefusal::SecondFactorRequired)
        );
        // The gate session is not left open at the Core.
        assert_eq!(script.lock().unwrap().asked.last().unwrap(), "/auth/logout");
        assert_eq!(
            core.sign_in("pessoa", PASSWORD, Some("000000"), None),
            Err(CoreRefusal::InvalidCredentials)
        );
        assert!(core
            .sign_in("pessoa", PASSWORD, Some("123456"), None)
            .is_ok());
    }

    #[test]
    fn uma_credencial_temporaria_so_abre_sessao_depois_de_substituida() {
        let (core, script, _) = link("password_change_required");
        assert_eq!(
            core.sign_in("pessoa", PASSWORD, None, None),
            Err(CoreRefusal::PasswordChangeRequired)
        );
        // The gate session is not left open at the Core.
        assert_eq!(script.lock().unwrap().asked.last().unwrap(), "/auth/logout");
        // The Core judges the choice, and a refusal is said as such.
        assert_eq!(
            core.sign_in("pessoa", PASSWORD, None, Some("curta")),
            Err(CoreRefusal::PasswordNotAccepted)
        );
        assert_eq!(script.lock().unwrap().asked.last().unwrap(), "/auth/logout");
        assert!(core
            .sign_in("pessoa", PASSWORD, None, Some("uma escolha bem comprida"))
            .is_ok());
        let asked = script.lock().unwrap().asked.clone();
        let tail: Vec<&str> = asked
            .iter()
            .rev()
            .take(3)
            .rev()
            .map(String::as_str)
            .collect();
        assert_eq!(
            tail,
            ["/auth/login", "/auth/password", "/auth/machine-session"]
        );
    }

    #[test]
    fn um_segundo_factor_por_inscrever_inscreve_se_aqui_e_os_codigos_entregam_se_uma_vez() {
        let (core, script, _) = link("mfa_required");
        script.lock().unwrap().mfa_mode = "enrollment";
        assert_eq!(
            core.sign_in("pessoa", PASSWORD, None, None),
            Err(CoreRefusal::SecondFactorEnrollmentRequired)
        );
        assert_eq!(script.lock().unwrap().asked.last().unwrap(), "/auth/logout");
        // The key comes from the Core, and the session that fetched it is closed.
        assert_eq!(
            core.begin_enrollment("pessoa", "errada"),
            Err(CoreRefusal::InvalidCredentials)
        );
        assert_eq!(
            core.begin_enrollment("pessoa", PASSWORD).as_deref(),
            Ok("JBSWY3DPEHPK3PXP")
        );
        assert_eq!(script.lock().unwrap().asked.last().unwrap(), "/auth/logout");
        // A wrong code enrols nothing and yields no codes.
        assert_eq!(
            core.sign_in("pessoa", PASSWORD, Some("000000"), None),
            Err(CoreRefusal::InvalidCredentials)
        );
        assert!(core.take_recovery_codes().is_empty());
        // The right one confirms, acknowledges, and opens the session.
        assert!(core
            .sign_in("pessoa", PASSWORD, Some("123456"), None)
            .is_ok());
        let asked = script.lock().unwrap().asked.clone();
        let tail: Vec<&str> = asked
            .iter()
            .rev()
            .take(5)
            .rev()
            .map(String::as_str)
            .collect();
        assert_eq!(
            tail,
            [
                "/auth/login",
                "/auth/mfa",
                "/auth/mfa/enroll/confirm",
                "/auth/mfa/acknowledge",
                "/auth/machine-session"
            ]
        );
        assert_eq!(core.take_recovery_codes(), ["aaaa-bbbb", "cccc-dddd"]);
        assert!(
            core.take_recovery_codes().is_empty(),
            "os códigos entregaram-se duas vezes"
        );
    }

    #[test]
    fn uma_conta_ja_inscrita_nao_recebe_outra_chave() {
        let (core, script, _) = link("mfa_required");
        script.lock().unwrap().mfa_mode = "challenge";
        assert_eq!(
            core.begin_enrollment("pessoa", PASSWORD),
            Err(CoreRefusal::SecondFactorRequired)
        );
        assert!(!script
            .lock()
            .unwrap()
            .asked
            .iter()
            .any(|p| p.starts_with("/auth/mfa/enroll")));
    }

    #[test]
    fn uma_sessao_revogada_no_core_e_esquecida_e_sair_avisa_o_core() {
        let (core, script, _) = link("active");
        core.sign_in("pessoa", PASSWORD, None, None).unwrap();
        script.lock().unwrap().revoked = true;
        assert_eq!(core.refresh("s-1"), Refresh::Revoked);
        // Forgotten: the Core is not asked again about a session it ended.
        assert_eq!(core.refresh("s-1"), Refresh::Unreachable);

        script.lock().unwrap().revoked = false;
        core.sign_in("pessoa", PASSWORD, None, None).unwrap();
        core.sign_out("s-1");
        assert_eq!(script.lock().unwrap().asked.last().unwrap(), "/auth/logout");
        assert_eq!(core.refresh("s-1"), Refresh::Unreachable);
    }

    #[test]
    fn sem_core_nao_ha_resposta_inventada() {
        let script = Arc::new(Mutex::new(Script::default()));
        let core = HttpCore::new(
            &core_base(&script),
            "machine-a",
            bench().path.with_file_name("x.json"),
        )
        .unwrap();
        assert_eq!(
            core.sign_in("pessoa", PASSWORD, None, None),
            Err(CoreRefusal::Unreachable)
        );
    }
}
