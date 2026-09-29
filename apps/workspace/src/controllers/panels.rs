//! Os painéis da barra de cima (D002 · FG-004, FG-005, FG-006).
//!
//! Só factos que o Core já dá. Nenhum estado é inventado:
//!
//! - **Estado do sistema.** O Core é a única capacidade obrigatória, e é ela
//!   que decide o estado geral; computação, cópias de segurança e IA são
//!   opcionais (a falta de IA nunca torna o Ocinye OS indisponível). O Core
//!   ainda não separa o obrigatório do opcional no `/ready`
//!   (CORE_STATUS_CONTRACT_FOLLOWUP): `degraded` conta como operacional, como
//!   à porta (FG-024). As cópias de segurança ainda não têm registo no Core
//!   (FG-016): «Sem registo», que não afirma êxito nem falha. Uma pergunta que
//!   falhou também fica sem registo — não se afirma o que não se sabe.
//! - **Notificações.** As do membro, pelo `/notifications` que a casca já
//!   pede; cada uma leva ao recurso, que o Core reautoriza ao abrir.
//! - **Relógio.** O mês de hoje no fuso do membro e até três eventos de hoje
//!   do `/calendar/agenda`; sem agenda, o estado vazio do Design.

use chrono::{Datelike, NaiveDate};
use serde_json::Value;

use crate::api::ApiFailure;
use crate::controllers::desktop::{instant, load_of, text, Clock};
use crate::i18n::tf;
use crate::ui::view_models::{
    Capability, CapabilityVm, ClockPanelVm, Health, Load, NotificationItem, NotificationsPanelVm,
    StatusPanelVm, WidgetItem,
};

/// Quantas notificações o painel mostra (HANDOFF: até 6).
const NOTIFICATIONS: usize = 6;
/// Quantos eventos de hoje o painel do relógio mostra (HANDOFF: até 3).
pub const AGENDA: usize = 3;

/// O painel do estado. `core` é o estado do Core já mapeado pela porta.
#[must_use]
pub(crate) fn status(
    core: Health,
    compute: &Result<Value, ApiFailure>,
    ai: Option<Health>,
    storage: &Result<Value, ApiFailure>,
    is_admin: bool,
) -> StatusPanelVm {
    let mut capabilities = vec![CapabilityVm {
        kind: Capability::Core,
        required: true,
        state: Some(core),
        detail: None,
    }];
    if let Some(c) = compute_capability(compute) {
        capabilities.push(c);
    }
    capabilities.push(CapabilityVm {
        kind: Capability::Backup,
        required: false,
        state: None,
        detail: None,
    });
    capabilities.push(CapabilityVm {
        kind: Capability::Ai,
        required: false,
        state: ai,
        detail: None,
    });
    let storage = storage.as_ref().ok().and_then(|v| {
        let s = v.get("storage")?;
        let used = s.get("used_bytes").and_then(Value::as_u64)?;
        let limit = s.get("limit_bytes").and_then(Value::as_u64)?;
        (limit > 0).then_some((used, limit))
    });
    StatusPanelVm {
        // Só as obrigatórias contam para o estado geral.
        overall: core,
        capabilities,
        storage,
        detail_href: is_admin.then_some("/admin/monitor"),
    }
}

/// A computação: sem nós registados não há capacidade a mostrar (a mesma regra
/// do widget D001); uma pergunta recusada também não se mostra.
fn compute_capability(compute: &Result<Value, ApiFailure>) -> Option<CapabilityVm> {
    let v = match compute {
        Ok(v) => v,
        Err(ApiFailure::Forbidden | ApiFailure::Denied) => return None,
        Err(_) => {
            return Some(CapabilityVm {
                kind: Capability::Compute,
                required: false,
                state: None,
                detail: None,
            })
        }
    };
    let n = |k: &str| v.get(k).and_then(Value::as_u64).unwrap_or(0);
    let (up, total) = (n("online_nodes"), n("registered_nodes"));
    if total == 0 {
        return None;
    }
    let state = if up >= total {
        Health::Operational
    } else if up == 0 {
        Health::Unavailable
    } else {
        Health::Degraded
    };
    Some(CapabilityVm {
        kind: Capability::Compute,
        required: false,
        state: Some(state),
        detail: Some(tf(
            "health.nodes.partial",
            &[("up", &up.to_string()), ("total", &total.to_string())],
        )),
    })
}

/// Para onde leva uma notificação: o recurso dela, se o Workspace tiver a rota;
/// senão, a lista.
fn notification_href(n: &Value) -> String {
    let id = text(n, "resource_id");
    let valid = !id.is_empty() && id.bytes().all(|b| b.is_ascii_hexdigit() || b == b'-');
    match (n.get("resource_type").and_then(Value::as_str), valid) {
        (Some("calendar_event"), true) => format!("/calendar/events/{id}"),
        (Some("task"), true) => format!("/tasks/{id}"),
        (Some("conversation"), true) => format!("/messages/{id}"),
        _ => "/notifications".to_owned(),
    }
}

/// O painel das notificações, a partir do `/notifications` da casca.
#[must_use]
pub(crate) fn notifications(
    list: &Result<Value, ApiFailure>,
    clock: &Clock,
) -> NotificationsPanelVm {
    let items = match list {
        Ok(v) => {
            let all = v
                .get("notifications")
                .and_then(Value::as_array)
                .map_or(&[][..], Vec::as_slice);
            let shown: Vec<NotificationItem> = all
                .iter()
                .take(NOTIFICATIONS)
                .map(|n| NotificationItem {
                    id: text(n, "id").to_owned(),
                    title: text(n, "title").to_owned(),
                    body: text(n, "body").to_owned(),
                    when: instant(n, "created_at")
                        .map_or(crate::ui::view_models::Ago::Now, |at| clock.ago(at)),
                    read: n.get("read").and_then(Value::as_bool).unwrap_or(false),
                    href: notification_href(n),
                })
                .collect();
            if shown.is_empty() {
                Load::Empty
            } else {
                Load::Ready(shown)
            }
        }
        Err(f) => load_of("notifications", f),
    };
    NotificationsPanelVm { items }
}

/// O painel do relógio: o mês de hoje no fuso do membro, e a agenda de hoje.
#[must_use]
pub(crate) fn clock(clock: &Clock, agenda: Load<Vec<WidgetItem>>) -> ClockPanelVm {
    let today = clock.now.with_timezone(&clock.zone.zone()).date_naive();
    let first = today.with_day(1).unwrap_or(today);
    let next = if today.month() == 12 {
        NaiveDate::from_ymd_opt(today.year() + 1, 1, 1)
    } else {
        NaiveDate::from_ymd_opt(today.year(), today.month() + 1, 1)
    };
    let days = next
        .and_then(|n| u8::try_from((n - first).num_days()).ok())
        .unwrap_or(30);
    ClockPanelVm {
        today: (
            today.year(),
            u8::try_from(today.month()).unwrap_or(1),
            u8::try_from(today.day()).unwrap_or(1),
        ),
        first_weekday: u8::try_from(first.weekday().num_days_from_monday()).unwrap_or(0),
        days_in_month: days,
        agenda,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};
    use serde_json::json;

    fn relogio() -> Clock {
        Clock {
            now: Utc.with_ymd_and_hms(2026, 9, 29, 10, 0, 0).unwrap(),
            zone: ocinye_contracts::temporal::TimeZoneName::parse("Europe/Lisbon").unwrap(),
            core_ok: true,
            is_admin: false,
        }
    }

    #[test]
    fn so_o_core_decide_e_a_ia_nunca_torna_o_ocinye_indisponivel() {
        let p = status(
            Health::Operational,
            &Ok(json!({"online_nodes": 3, "registered_nodes": 4})),
            Some(Health::Unavailable),
            &Ok(json!({"storage": {"used_bytes": 42, "limit_bytes": 100}})),
            false,
        );
        assert_eq!(p.overall, Health::Operational);
        assert!(p.capabilities[0].required && p.capabilities[0].kind == Capability::Core);
        assert!(p.capabilities[1..].iter().all(|c| !c.required));
        let compute = p
            .capabilities
            .iter()
            .find(|c| c.kind == Capability::Compute)
            .unwrap();
        assert_eq!(compute.state, Some(Health::Degraded));
        assert!(compute.detail.as_deref().unwrap().contains("3/4"));
        // Sem registo de cópias: não se afirma êxito nem falha.
        let backup = p
            .capabilities
            .iter()
            .find(|c| c.kind == Capability::Backup)
            .unwrap();
        assert_eq!(backup.state, None);
        assert_eq!(p.storage, Some((42, 100)));
        assert_eq!(p.detail_href, None);
    }

    #[test]
    fn sem_nos_nao_ha_computacao_e_uma_falha_nao_e_indisponivel() {
        let zero = status(
            Health::Unavailable,
            &Ok(json!({"online_nodes": 0, "registered_nodes": 0})),
            None,
            &Err(ApiFailure::Unavailable(None)),
            true,
        );
        assert!(zero
            .capabilities
            .iter()
            .all(|c| c.kind != Capability::Compute));
        assert_eq!(zero.overall, Health::Unavailable);
        assert_eq!(zero.storage, None);
        assert_eq!(zero.detail_href, Some("/admin/monitor"));
        let falhou = status(
            Health::Operational,
            &Err(ApiFailure::Unavailable(None)),
            None,
            &Ok(json!({})),
            false,
        );
        let c = falhou
            .capabilities
            .iter()
            .find(|c| c.kind == Capability::Compute)
            .unwrap();
        assert_eq!(c.state, None);
    }

    #[test]
    fn as_notificacoes_levam_ao_recurso_e_nunca_a_outro_sitio() {
        let v = json!({"notifications": [
            {"id": "a", "title": "Reunião", "resource_type": "calendar_event",
             "resource_id": "8f14e45f-ceea-467a-9575-4a5b2f5c1d10", "read": false,
             "created_at": "2026-09-29T09:55:00Z"},
            {"id": "b", "title": "x", "resource_type": "conversation",
             "resource_id": "../../admin", "read": true},
            {"id": "c", "title": "y", "resource_type": "desconhecido", "resource_id": "1"},
        ], "unread": 1});
        let p = notifications(&Ok(v), &relogio());
        let Load::Ready(items) = p.items else {
            panic!()
        };
        assert_eq!(
            items[0].href,
            "/calendar/events/8f14e45f-ceea-467a-9575-4a5b2f5c1d10"
        );
        assert_eq!(items[0].when, crate::ui::view_models::Ago::Minutes(5));
        assert!(!items[0].read);
        assert_eq!(items[1].href, "/notifications");
        assert_eq!(items[2].href, "/notifications");
        let vazio = notifications(&Ok(json!({"notifications": []})), &relogio());
        assert_eq!(vazio.items, Load::Empty);
    }

    #[test]
    fn o_mes_do_relogio_e_o_do_fuso_do_membro() {
        let p = clock(&relogio(), Load::Empty);
        assert_eq!(p.today, (2026, 9, 29));
        // 1 de Setembro de 2026 é uma terça-feira.
        assert_eq!(p.first_weekday, 1);
        assert_eq!(p.days_in_month, 30);
        let mut dez = relogio();
        dez.now = Utc.with_ymd_and_hms(2026, 12, 31, 23, 30, 0).unwrap();
        let p = clock(&dez, Load::Empty);
        assert_eq!((p.today, p.days_in_month), ((2026, 12, 31), 31));
    }
}
