//! The denial matrix and the order of the gates, over the whole catalogue.

use std::sync::Mutex;

use ocinye_system_contracts::capability::{
    AuditClass, AuthorizationClass, CapabilityId, Family, FreshAuthClass, CATALOGUE,
};
use ocinye_system_contracts::reason::Reason;
use ocinye_system_contracts::request::{Outcome, Payload, Request};

use crate::audit::{AuditRecord, AuditSink};
use crate::authority::{CoreAnswer, CoreAuthority, Peer, Session, SessionView};
use crate::service::{Platform, Service};

/// Counts what reached the system: the property under test is mostly that
/// nothing did.
#[derive(Default)]
struct Recorder(Mutex<Vec<CapabilityId>>);

impl Platform for Recorder {
    fn perform(&self, request: &Request) -> Result<Payload, Reason> {
        self.0.lock().unwrap().push(request.capability());
        Ok(Payload::Done)
    }
}

struct Sess {
    session: Session,
    /// A grant that is valid once for one exact binding.
    grant: Mutex<Option<(String, String)>>,
}

impl Sess {
    fn new(session: Session) -> Self {
        Self {
            session,
            grant: Mutex::new(None),
        }
    }
}

impl SessionView for Sess {
    fn session(&self) -> Session {
        self.session.clone()
    }
    fn consume_grant(
        &self,
        grant: &str,
        _actor: &str,
        binding: &str,
        _class: FreshAuthClass,
    ) -> bool {
        let mut held = self.grant.lock().unwrap();
        if held
            .as_ref()
            .is_some_and(|(g, b)| g == grant && b == binding)
        {
            *held = None;
            true
        } else {
            false
        }
    }
}

enum Core {
    Permits,
    Denies,
    Down,
}

impl CoreAuthority for Core {
    fn permits(&self, _member: &str, _family: Family) -> CoreAnswer {
        match self {
            Self::Permits => CoreAnswer::Permits,
            Self::Denies => CoreAnswer::Denies,
            Self::Down => CoreAnswer::Unreachable,
        }
    }
}

#[derive(Default)]
struct Trail(Mutex<Vec<AuditRecord>>);

impl AuditSink for Trail {
    fn record(&self, record: &AuditRecord) {
        self.0.lock().unwrap().push(record.clone());
    }
}

fn request(id: CapabilityId) -> Request {
    let json = match id {
        CapabilityId::NetworkConfigure => r#"{"capability":"network.configure","interface":"enp3s0","addressing":{"mode":"DHCP"}}"#.to_owned(),
        CapabilityId::WifiConnect => r#"{"capability":"wifi.connect","ssid":"Lab","passphrase":"correct horse"}"#.to_owned(),
        CapabilityId::WifiForget => r#"{"capability":"wifi.forget","ssid":"Lab"}"#.to_owned(),
        CapabilityId::WifiSetRadio => r#"{"capability":"wifi.set_radio","enabled":true}"#.to_owned(),
        CapabilityId::BluetoothSetPower => r#"{"capability":"bluetooth.set_power","powered":true}"#.to_owned(),
        CapabilityId::BluetoothPair | CapabilityId::BluetoothConnect | CapabilityId::BluetoothDisconnect | CapabilityId::BluetoothRemove => {
            format!(r#"{{"capability":"{}","address":"AA:BB:CC:DD:EE:FF"}}"#, id.as_str())
        }
        CapabilityId::AudioSetVolume => r#"{"capability":"audio.set_volume","target":"OUTPUT","percent":40}"#.to_owned(),
        CapabilityId::AudioSetMute => r#"{"capability":"audio.set_mute","target":"INPUT","muted":true}"#.to_owned(),
        CapabilityId::AudioSetOutput | CapabilityId::AudioSetInput => format!(r#"{{"capability":"{}","id":51}}"#, id.as_str()),
        CapabilityId::DisplaySetBrightness => r#"{"capability":"display.set_brightness","percent":40}"#.to_owned(),
        CapabilityId::DisplayConfigure => r#"{"capability":"display.configure","output":"eDP-1","width":1920,"height":1200,"scale_percent":125}"#.to_owned(),
        CapabilityId::LocaleConfigure => r#"{"capability":"locale.configure","language":"pt"}"#.to_owned(),
        CapabilityId::TimeConfigure => r#"{"capability":"time.configure","timezone":"Africa/Luanda","automatic":true}"#.to_owned(),
        CapabilityId::KeyboardConfigure => r#"{"capability":"keyboard.configure","layout":"pt","variant":null}"#.to_owned(),
        CapabilityId::AccessibilityConfigure => r#"{"capability":"accessibility.configure","text_scale_percent":115,"high_contrast":false,"reduce_motion":true}"#.to_owned(),
        CapabilityId::PrivacySetMicrophone | CapabilityId::PrivacySetCamera => format!(r#"{{"capability":"{}","allowed":false}}"#, id.as_str()),
        CapabilityId::RemoteAccessConfigure => r#"{"capability":"remote_access.configure","ssh_enabled":false}"#.to_owned(),
        CapabilityId::ActivationStart => r#"{"capability":"activation.start","domain":"ocinye.empresa.example","instance_name":"Instituto","distributions":["research"],"person_name":"Pessoa","person_email":"pessoa@empresa.example","admin_name":"Admin","admin_email":"admin@empresa.example"}"#.to_owned(),
        other => format!(r#"{{"capability":"{}"}}"#, other.as_str()),
    };
    serde_json::from_str(&json).unwrap()
}

fn active() -> Session {
    Session::Active {
        member: "m-1".to_owned(),
    }
}

struct Bench {
    platform: Recorder,
    session: Sess,
    core: Core,
    trail: Trail,
}

impl Bench {
    fn new(session: Session, core: Core) -> Self {
        Self {
            platform: Recorder::default(),
            session: Sess::new(session),
            core,
            trail: Trail::default(),
        }
    }
    fn call(&self, peer: Peer, grant: Option<&str>, r: &Request) -> Outcome {
        Service {
            platform: &self.platform,
            session: &self.session,
            core: &self.core,
            audit: &self.trail,
        }
        .call(peer, grant, r)
    }
    fn reached(&self) -> usize {
        self.platform.0.lock().unwrap().len()
    }
}

fn refused(o: &Outcome) -> &Reason {
    match o {
        Outcome::Refused { reason } => reason,
        Outcome::Ok { .. } => panic!("expected a refusal"),
    }
}

#[test]
fn um_par_desconhecido_nao_alcanca_nada_em_capacidade_nenhuma() {
    for s in CATALOGUE {
        let b = Bench::new(active(), Core::Permits);
        let o = b.call(Peer::Other, None, &request(s.id));
        assert!(
            matches!(refused(&o), Reason::NotAuthorized { .. }),
            "{}",
            s.id.as_str()
        );
        assert_eq!(b.reached(), 0, "{}", s.id.as_str());
    }
}

#[test]
fn a_presenca_so_alcanca_o_que_e_da_presenca() {
    for s in CATALOGUE {
        let b = Bench::new(Session::SignedOut, Core::Permits);
        let o = b.call(Peer::SystemInterface, None, &request(s.id));
        match (s.authorization, s.fresh_auth) {
            (AuthorizationClass::Presence, FreshAuthClass::None) => {
                assert!(matches!(o, Outcome::Ok { .. }), "{}", s.id.as_str())
            }
            (AuthorizationClass::Presence, class) => {
                assert_eq!(
                    refused(&o),
                    &Reason::FreshAuthRequired { class },
                    "{}",
                    s.id.as_str()
                );
                assert_eq!(b.reached(), 0);
            }
            (needs, _) => {
                assert_eq!(
                    refused(&o),
                    &Reason::NotAuthorized { needs },
                    "{}",
                    s.id.as_str()
                );
                assert_eq!(b.reached(), 0);
            }
        }
    }
}

#[test]
fn um_membro_nao_alcanca_o_que_e_do_operador_sem_o_core_o_permitir() {
    for s in CATALOGUE
        .iter()
        .filter(|s| s.authorization == AuthorizationClass::Operator)
    {
        let b = Bench::new(active(), Core::Denies);
        assert_eq!(
            refused(&b.call(Peer::SystemInterface, None, &request(s.id))),
            &Reason::NotAuthorized {
                needs: AuthorizationClass::Operator
            }
        );
        assert_eq!(b.reached(), 0, "{}", s.id.as_str());

        let b = Bench::new(active(), Core::Down);
        assert_eq!(
            refused(&b.call(Peer::SystemInterface, None, &request(s.id))),
            &Reason::AuthorityUnavailable
        );
        assert_eq!(b.reached(), 0, "{}", s.id.as_str());
    }
}

#[test]
fn com_o_core_em_baixo_tudo_o_que_nao_e_do_operador_continua_a_funcionar() {
    for s in CATALOGUE.iter().filter(|s| {
        s.authorization != AuthorizationClass::Operator && s.fresh_auth == FreshAuthClass::None
    }) {
        let b = Bench::new(active(), Core::Down);
        assert!(
            matches!(
                b.call(Peer::SystemInterface, None, &request(s.id)),
                Outcome::Ok { .. }
            ),
            "{}",
            s.id.as_str()
        );
    }
}

#[test]
fn com_a_sessao_bloqueada_so_passa_o_que_o_catalogo_permite_bloqueado() {
    for s in CATALOGUE {
        let b = Bench::new(
            Session::Locked {
                member: "m-1".to_owned(),
            },
            Core::Permits,
        );
        let o = b.call(Peer::SystemInterface, None, &request(s.id));
        if s.while_locked {
            assert!(
                !matches!(refused_or_ok(&o), Some(Reason::SessionLocked)),
                "{}",
                s.id.as_str()
            );
        } else {
            assert_eq!(refused(&o), &Reason::SessionLocked, "{}", s.id.as_str());
            assert_eq!(b.reached(), 0);
        }
    }
}

fn refused_or_ok(o: &Outcome) -> Option<&Reason> {
    match o {
        Outcome::Refused { reason } => Some(reason),
        Outcome::Ok { .. } => None,
    }
}

#[test]
fn uma_concessao_vale_uma_vez_e_so_para_o_pedido_exacto() {
    let restart = request(CapabilityId::PowerRestart);
    let shutdown = request(CapabilityId::PowerShutdown);
    let b = Bench::new(active(), Core::Permits);
    let bind = |r: &Request| serde_json::to_string(r).unwrap();

    // No grant: asked for one, nothing happens.
    assert_eq!(
        refused(&b.call(Peer::SystemInterface, None, &restart)),
        &Reason::FreshAuthRequired {
            class: FreshAuthClass::Confirm
        }
    );
    // A grant for another request is not a grant for this one.
    *b.session.grant.lock().unwrap() = Some(("g1".to_owned(), bind(&shutdown)));
    assert_eq!(
        refused(&b.call(Peer::SystemInterface, Some("g1"), &restart)),
        &Reason::GrantInvalid
    );
    assert_eq!(b.reached(), 0);
    // The right grant works once.
    *b.session.grant.lock().unwrap() = Some(("g2".to_owned(), bind(&restart)));
    assert!(matches!(
        b.call(Peer::SystemInterface, Some("g2"), &restart),
        Outcome::Ok { .. }
    ));
    assert_eq!(
        refused(&b.call(Peer::SystemInterface, Some("g2"), &restart)),
        &Reason::GrantInvalid
    );
    assert_eq!(b.reached(), 1);
}

#[test]
fn a_autorizacao_vem_antes_da_validacao() {
    // Out of range AND unauthorised: the answer is about authority, so the
    // shape of a valid request is not disclosed to someone who may not make it.
    let bad: Request = serde_json::from_str(
        r#"{"capability":"time.configure","timezone":"../../etc/shadow","automatic":null}"#,
    )
    .unwrap();
    let b = Bench::new(Session::SignedOut, Core::Permits);
    assert!(matches!(
        refused(&b.call(Peer::SystemInterface, None, &bad)),
        Reason::NotAuthorized { .. }
    ));
    // Authorised with a grant, it is then refused as out of range, before the platform.
    let b = Bench::new(active(), Core::Permits);
    *b.session.grant.lock().unwrap() = Some(("g".to_owned(), serde_json::to_string(&bad).unwrap()));
    assert_eq!(
        refused(&b.call(Peer::SystemInterface, Some("g"), &bad)),
        &Reason::ValueOutOfRange
    );
    assert_eq!(b.reached(), 0);
}

#[test]
fn o_operador_de_recuperacao_alcanca_tudo_e_fica_registado() {
    for s in CATALOGUE {
        let b = Bench::new(Session::SignedOut, Core::Down);
        assert!(
            matches!(
                b.call(Peer::RecoveryOperator, None, &request(s.id)),
                Outcome::Ok { .. }
            ),
            "{}",
            s.id.as_str()
        );
        let trail = b.trail.0.lock().unwrap();
        if s.audit == AuditClass::None {
            assert!(trail.is_empty(), "{}", s.id.as_str());
        } else {
            assert_eq!(trail.len(), 1);
            assert_eq!(trail[0].actor, "recovery-operator");
            assert_eq!(trail[0].result, "OK");
        }
    }
}

#[test]
fn uma_recusa_de_seguranca_fica_registada_e_o_segredo_nunca() {
    let b = Bench::new(Session::SignedOut, Core::Permits);
    let connect = request(CapabilityId::WifiConnect);
    assert!(matches!(
        refused(&b.call(Peer::SystemInterface, None, &connect)),
        Reason::NotAuthorized { .. }
    ));
    let trail = b.trail.0.lock().unwrap();
    assert_eq!(trail.len(), 1);
    assert_eq!(trail[0].result, "NOT_AUTHORIZED");
    let line = serde_json::to_string(&trail[0]).unwrap();
    assert!(!line.contains("correct horse"), "{line}");
}

#[test]
fn o_core_so_age_por_um_membro_com_sessao_activa() {
    let r = request(CapabilityId::DisplaySetBrightness);
    let b = Bench::new(Session::SignedOut, Core::Permits);
    assert!(matches!(
        refused(&b.call(Peer::Core, None, &r)),
        Reason::NotAuthorized { .. }
    ));
    let b = Bench::new(active(), Core::Permits);
    assert!(matches!(b.call(Peer::Core, None, &r), Outcome::Ok { .. }));
}
