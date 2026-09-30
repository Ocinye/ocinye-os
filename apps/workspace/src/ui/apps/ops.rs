//! D007 · Peças partilhadas das nove aplicações de conclusão. DESIGN_LOCKED.
//!
//! O medidor (só números que o Core/runtime reporta), o motivo honesto de
//! indisponibilidade da inferência, os estados com texto, e a linha de
//! evento com alvo redigido. Tudo sobre [`super`] e [`super::res`].

use leptos::prelude::*;

use crate::i18n::{t, tf};
use crate::ui::components::icon;
use crate::ui::view_models::{
    AgentStatus, AiCap, AiReason, AuditOutcome, CapLineVm, MsgPresence, NodeStatus, ResStateVm,
    ResTone, StorageStateVm,
};

use super::res::state_tag;

const fn st(key: &'static str, tone: ResTone) -> ResStateVm {
    ResStateVm { key, tone }
}

/// Estado de um agente.
#[must_use]
pub const fn agent_state(s: AgentStatus) -> ResStateVm {
    match s {
        AgentStatus::Ready => st("agents.state.ready", ResTone::Done),
        AgentStatus::Configured => st("agents.state.configured", ResTone::Attention),
        AgentStatus::Disabled => st("agents.state.disabled", ResTone::Closed),
        AgentStatus::Archived => st("agents.state.archived", ResTone::Closed),
    }
}

/// Estado de um nó.
#[must_use]
pub const fn node_state(s: NodeStatus) -> ResStateVm {
    match s {
        NodeStatus::Pending => st("compute.state.pending", ResTone::Neutral),
        NodeStatus::Online => st("compute.state.online", ResTone::Done),
        NodeStatus::Offline => st("compute.state.offline", ResTone::Attention),
        NodeStatus::Draining => st("compute.state.draining", ResTone::Attention),
        NodeStatus::Retired => st("compute.state.retired", ResTone::Closed),
    }
}

/// Estado do armazenamento pessoal.
#[must_use]
pub const fn storage_state(s: StorageStateVm) -> ResStateVm {
    match s {
        StorageStateVm::Normal => st("resources.state.normal", ResTone::Done),
        StorageStateVm::Warning => st("resources.state.warning", ResTone::Attention),
        StorageStateVm::Critical => st("resources.state.critical", ResTone::Attention),
        StorageStateVm::OverQuota => st("resources.state.over", ResTone::Closed),
    }
}

/// Resultado de auditoria (texto + ícone).
pub fn outcome_tag(o: AuditOutcome) -> impl IntoView {
    let (k, tone, ic) = match o {
        AuditOutcome::Success => ("audit.outcome.success", "done", "check"),
        AuditOutcome::Denied => ("audit.outcome.denied", "attention", "lock"),
        AuditOutcome::Failure => ("audit.outcome.failure", "closed", "warning"),
    };
    view! { <span class="oc-res-state oc-ops-outcome" data-tone=tone>{icon(ic)}{t(k)}</span> }
}

/// A presença, quando o tempo real a resolve. Ausente ≠ offline.
pub fn presence(p: Option<MsgPresence>) -> AnyView {
    let Some(p) = p else { return ().into_any() };
    let (k, v) = match p {
        MsgPresence::Online => ("msg.presence.online", "online"),
        MsgPresence::Away => ("msg.presence.away", "away"),
        MsgPresence::Offline => ("msg.presence.offline", "offline"),
    };
    view! { <span class="oc-ops-presence" data-presence=v><span class="oc-ops-presence__dot" aria-hidden="true"></span>{t(k)}</span> }.into_any()
}

/// A chave de uma capacidade de inferência.
#[must_use]
pub const fn cap_key(c: AiCap) -> &'static str {
    match c {
        AiCap::General => "ai.cap.general",
        AiCap::Coding => "ai.cap.coding",
        AiCap::Reasoning => "ai.cap.reasoning",
        AiCap::Embedding => "ai.cap.embedding",
    }
}

/// O motivo, em linguagem precisa. Nunca «offline» genérico.
#[must_use]
pub const fn reason_key(r: AiReason) -> &'static str {
    match r {
        AiReason::NoProvider => "ai.reason.no_provider",
        AiReason::NoCompatibleModel => "ai.reason.no_model",
        AiReason::CapacityUnavailable => "ai.reason.capacity",
        AiReason::HardwareNotSatisfied => "ai.reason.hardware",
        AiReason::ProviderUnhealthy => "ai.reason.unhealthy",
        AiReason::ModelLoading => "ai.reason.loading",
        AiReason::DisabledByPolicy => "ai.reason.disabled",
        AiReason::PolicyBlocked => "ai.reason.policy",
        AiReason::Unknown => "ai.reason.unknown",
    }
}

/// Um medidor: barra só quando há físico e uso reportados; os números vão sempre.
pub fn meter(l: &CapLineVm) -> impl IntoView {
    let dash = || "—".to_owned();
    view! {
        <div class="oc-ops-meter" data-part="ops-meter">
            <div class="oc-ops-meter__head">
                <span class="oc-ops-meter__label">{t(l.label_key)}</span>
                <span class="oc-ops-meter__val">{l.consumed.clone().map_or_else(|| t("ops.not_reported").to_owned(), |c| tf("ops.used_of", &[("used", c.as_str()), ("total", l.physical.clone().unwrap_or_else(dash).as_str())]))}</span>
            </div>
            {l.consumed_pct.map(|p| view! {
                <span class="oc-ops-meter__bar" role="img" aria-label=tf("ops.pct", &[("n", p.to_string().as_str())])><span class="oc-ops-meter__fill" data-pct=p.to_string()></span></span>
            })}
            <dl class="oc-ops-meter__facts">
                <div><dt>{t("ops.physical")}</dt><dd>{l.physical.clone().unwrap_or_else(dash)}</dd></div>
                <div><dt>{t("ops.reserved")}</dt><dd>{l.reserved.clone()}</dd></div>
                <div><dt>{t("ops.allocatable")}</dt><dd>{l.allocatable.clone().unwrap_or_else(dash)}</dd></div>
            </dl>
        </div>
    }
}

/// Um bloco de estado da plataforma (IA, Computação): calmo, sem alarme.
pub fn plane(
    ok: bool,
    title_key: &'static str,
    facts: Vec<(String, String)>,
    message: Option<String>,
) -> impl IntoView {
    view! {
        <section class="oc-ops-plane" data-ok=ok.then_some("") data-part="ops-plane">
            <p class="oc-ops-plane__title">{icon(if ok { "check" } else { "minus" })}<span>{t(title_key)}</span></p>
            <dl class="oc-ops-plane__facts">{facts.into_iter().map(|(k, v)| view! { <div><dt>{k}</dt><dd>{v}</dd></div> }).collect_view()}</dl>
            {message.map(|m| view! { <p class="oc-res-form__hint">{m}</p> })}
        </section>
    }
}

/// Um estado com texto (reexportado para as aplicações D007).
pub fn tag(s: ResStateVm) -> impl IntoView {
    state_tag(s)
}
