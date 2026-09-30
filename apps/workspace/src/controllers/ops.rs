//! D007 · Conclusão das aplicações: o que o Core devolve, traduzido para os
//! ViewModels do Design — Mensagens, IA, Agentes, Computação, Meus Recursos,
//! Actividade, Auditoria, Definições e Ajuda.
//!
//! Aqui não se decide nada. Três regras atravessam o módulo:
//!
//! - **Os vocabulários mapeiam-se um a um.** Um valor que o Core ainda não tinha
//!   quando isto se escreveu não vira o vizinho mais parecido: um motivo de IA
//!   desconhecido lê-se «desconhecido», um estado de nó desconhecido não tem
//!   linha.
//! - **Uma acção só aparece quando o Core a oferece** (o papel do actor na
//!   conversa, o âmbito que o Core diz que ele pode criar). O Core decide outra
//!   vez no POST.
//! - **Nada que o Design não deva ver chega a um VM**: nem segredo nem endereço
//!   de fornecedor, nem instruções de um agente que não é do membro, nem
//!   metadata de auditoria fora da lista branca.

use chrono::{DateTime, Utc};
use serde_json::Value;

use crate::controllers::desktop::{bytes, instant, text, Clock};
use crate::controllers::org as og;
use crate::controllers::research as rs;
use crate::i18n::t;
use crate::ui::view_models::{
    ActivityItemVm, AgentFormVm, AgentScopeVm, AgentStatus, AgentVm, AiCap, AiCapVm, AiModelVm,
    AiProviderVm, AiReason, AppNavVm, AppNyeVm, AuditDetailVm, AuditOutcome, AuditRowVm, CapLineVm,
    EntPartVm, HelpShortcutVm, HelpTopicVm, MsgConvRowVm, MsgKind, MsgMemberVm, MsgPresence,
    MsgReactionVm, MsgVm, NodeStatus, NodeVm, OrgActionKind, OrgActionVm, OrgAvatarVm,
    OrgConfirmVm, OrgReason, OwnSessionVm, PinVm, ResClassification, ResItemVm, ResLinkVm,
    ResOptionVm, ResTone, ResourcesVm, StorageStateVm,
};

fn items(v: &Value) -> Vec<Value> {
    v.get("items")
        .and_then(Value::as_array)
        .cloned()
        .or_else(|| v.as_array().cloned())
        .unwrap_or_default()
}

/// A lista de um JSON (`items` ou a própria lista).
#[must_use]
pub fn list_of(v: &Value) -> Vec<Value> {
    items(v)
}

/// Um excerto de texto, no limite de `n` caracteres.
#[must_use]
pub fn excerpt(s: &str, n: usize) -> String {
    let s = s.split_whitespace().collect::<Vec<_>>().join(" ");
    if s.chars().count() <= n {
        s
    } else {
        format!("{}…", s.chars().take(n).collect::<String>())
    }
}

// ═════════════════════════════════════════════════════════════════════════
// Mensagens
// ═════════════════════════════════════════════════════════════════════════

/// A presença que o tempo real resolve. `None` = tempo real em baixo (o campo
/// não vem), que não é «offline».
#[must_use]
pub fn presence(v: Option<&str>) -> Option<MsgPresence> {
    Some(match v? {
        "disponivel" => MsgPresence::Online,
        // Ocupado e «não incomodar» são uma pessoa presente que não quer ser
        // interrompida: o Design tem três estados, e o mais próximo que não
        // mente é «ausente».
        "ausente" | "ocupado" | "nao_incomodar" => MsgPresence::Away,
        "offline" => MsgPresence::Offline,
        _ => return None,
    })
}

/// O tipo da conversa (`direct` | `group`).
#[must_use]
pub fn msg_kind(v: &str) -> Option<MsgKind> {
    match v {
        "direct" => Some(MsgKind::Direct),
        "group" => Some(MsgKind::Group),
        _ => None,
    }
}

/// O papel numa conversa, traduzido.
#[must_use]
pub fn msg_role(v: &str) -> Option<&'static str> {
    match v {
        "owner" => Some("prod.msg.role.owner"),
        "administrator" => Some("prod.msg.role.administrator"),
        "member" => Some("prod.msg.role.member"),
        _ => None,
    }
}

/// Quem governa um grupo (retira outros): dono ou administrador.
#[must_use]
pub fn governs(role: &str) -> bool {
    matches!(role, "owner" | "administrator")
}

/// As conversas do membro.
#[must_use]
pub(crate) fn conv_rows(list: &[Value], open: Option<&str>, clock: &Clock) -> Vec<MsgConvRowVm> {
    list.iter()
        .filter_map(|c| {
            let id = text(c, "id");
            let kind = msg_kind(text(c, "kind"))?;
            let title = text(c, "title").to_owned();
            let n = |k: &str| {
                c.get(k)
                    .and_then(Value::as_u64)
                    .map_or(0, |n| u32::try_from(n).unwrap_or(u32::MAX))
            };
            Some(MsgConvRowVm {
                avatar: og::avatar(&title),
                kind,
                unread: n("unread"),
                mentions: n("unread_mentions"),
                last: c
                    .get("last_body")
                    .and_then(Value::as_str)
                    .filter(|b| !b.trim().is_empty())
                    .map(|b| excerpt(b, 90)),
                last_at: instant(c, "last_at").map(|at| clock.when(at)),
                presence: (kind == MsgKind::Direct)
                    .then(|| {
                        presence(
                            c.get("other")
                                .and_then(|o| o.get("presence"))
                                .and_then(Value::as_str),
                        )
                    })
                    .flatten(),
                href: format!("/messages/{id}"),
                active: open == Some(id),
                title,
            })
        })
        .collect()
}

/// As mensagens de uma página, da mais antiga para a mais recente (o Core
/// devolve-as da mais recente para a mais antiga).
#[must_use]
pub(crate) fn messages(conv: &str, page: &[Value], me: &str, clock: &Clock) -> Vec<MsgVm> {
    let mut out = Vec::with_capacity(page.len());
    let mut last_day: Option<String> = None;
    for m in page.iter().rev() {
        let id = text(m, "id");
        let at = instant(m, "created_at");
        let day = at.map(|a| rs::day(a, clock));
        let sep = if day.is_some() && day != last_day {
            last_day.clone_from(&day);
            day
        } else {
            None
        };
        let author = text(m, "author_name").to_owned();
        let body = text(m, "body");
        let mentions_me = m
            .get("mentions")
            .and_then(Value::as_array)
            .is_some_and(|a| a.iter().any(|p| p.as_str() == Some(me)));
        let reactions: Vec<MsgReactionVm> = m
            .get("reactions")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .map(|r| MsgReactionVm {
                        emoji: text(r, "emoji").to_owned(),
                        count: r
                            .get("count")
                            .and_then(Value::as_u64)
                            .map_or(0, |n| u32::try_from(n).unwrap_or(u32::MAX)),
                        mine: r.get("mine").and_then(Value::as_bool) == Some(true),
                    })
                    .collect()
            })
            .unwrap_or_default();
        // Uma mensagem retirada chega com o corpo vazio (o Core nunca aceita
        // um envio vazio): não se responde nem se reage a ela.
        let withdrawn = body.is_empty();
        out.push(MsgVm {
            anchor: format!("m-{id}"),
            avatar: og::avatar(&author),
            mine: text(m, "author_id") == me,
            body: (!withdrawn).then(|| body.to_owned()),
            at: at.map(|a| clock.hhmm(a)).unwrap_or_default(),
            edited: m.get("edited_at").is_some_and(|e| !e.is_null()),
            reply: m.get("reply_to").filter(|r| r.is_object()).map(|r| {
                let x = text(r, "excerpt");
                (
                    text(r, "author_name").to_owned(),
                    if x.is_empty() {
                        t("msg.withdrawn").to_owned()
                    } else {
                        x.to_owned()
                    },
                )
            }),
            mentions_me,
            react_action: (!withdrawn).then(|| format!("/messages/{conv}/messages/{id}/react")),
            reply_href: (!withdrawn)
                .then(|| format!("/messages/{conv}?reply={id}#oc-messages-doc-compose")),
            reactions,
            author,
            day: sep,
        });
    }
    out
}

/// A resposta que o compositor prepara: só a uma mensagem desta página, e não
/// retirada. (autor, excerto, id)
#[must_use]
pub fn reply_to(page: &[Value], id: &str) -> Option<(String, String, String)> {
    let m = page.iter().find(|m| text(m, "id") == id)?;
    let body = text(m, "body");
    (!body.is_empty()).then(|| {
        (
            text(m, "author_name").to_owned(),
            excerpt(body, 120),
            id.to_owned(),
        )
    })
}

/// «Mensagens anteriores»: a página veio cheia (50), por isso pode haver mais;
/// o cursor é o instante da mais antiga.
#[must_use]
pub fn older_href(conv: &str, page: &[Value]) -> Option<String> {
    const PAGE: usize = 50;
    if page.len() < PAGE {
        return None;
    }
    let oldest = page.last().map(|m| text(m, "created_at"))?;
    (!oldest.is_empty()).then(|| format!("/messages/{conv}?before={}", rs::encode(oldest)))
}

/// Os participantes de um grupo, com «retirar» só para quem governa e nunca
/// sobre si próprio. (participantes, sair)
#[must_use]
pub fn members(conv: &Value, me: &str) -> (Vec<MsgMemberVm>, Option<OrgActionVm>) {
    let id = text(conv, "id");
    if msg_kind(text(conv, "kind")) != Some(MsgKind::Group) {
        return (Vec::new(), None);
    }
    let governa = governs(text(conv, "role"));
    let ps = conv
        .get("participants")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let list = ps
        .iter()
        .map(|p| {
            let pid = text(p, "id");
            MsgMemberVm {
                name: text(p, "name").to_owned(),
                role: msg_role(text(p, "role")).map_or_else(String::new, |k| t(k).to_owned()),
                remove: (governa && pid != me && !pid.is_empty()).then(|| OrgActionVm {
                    kind: OrgActionKind::RemoveParticipant,
                    href: format!(
                        "/messages/{id}?confirm={}&person={}",
                        og::confirm_code(OrgActionKind::RemoveParticipant),
                        rs::encode(pid)
                    ),
                }),
            }
        })
        .collect();
    let leave = OrgActionVm {
        kind: OrgActionKind::LeaveConversation,
        href: format!(
            "/messages/{id}?confirm={}",
            og::confirm_code(OrgActionKind::LeaveConversation)
        ),
    };
    (list, Some(leave))
}

/// A confirmação de sair ou retirar, só quando a conversa a oferece agora.
#[must_use]
pub fn msg_confirm(
    conv: &Value,
    me: &str,
    kind: OrgActionKind,
    person: Option<&str>,
) -> Option<OrgConfirmVm> {
    let id = text(conv, "id");
    let title = text(conv, "title").to_owned();
    let (list, leave) = members(conv, me);
    let base = format!("/messages/{id}");
    let (target, action, hidden, from) = match kind {
        OrgActionKind::LeaveConversation => {
            leave?;
            (
                title.clone(),
                format!("{base}/leave"),
                vec![],
                format!("confirm={}", og::confirm_code(kind)),
            )
        }
        OrgActionKind::RemoveParticipant => {
            let pid = person?;
            let ps = conv.get("participants").and_then(Value::as_array)?;
            let p = ps.iter().find(|p| text(p, "id") == pid)?;
            let offered = list.iter().any(|m| {
                m.name == text(p, "name")
                    && m.remove
                        .as_ref()
                        .is_some_and(|a| a.href.ends_with(&format!("person={}", rs::encode(pid))))
            });
            if !offered {
                return None;
            }
            (
                text(p, "name").to_owned(),
                format!("{base}/remove"),
                vec![("who", pid.to_owned())],
                format!(
                    "confirm={}&person={}",
                    og::confirm_code(kind),
                    rs::encode(pid)
                ),
            )
        }
        _ => return None,
    };
    Some(OrgConfirmVm {
        kind,
        target,
        context: (kind == OrgActionKind::RemoveParticipant).then_some(title),
        change: None,
        action,
        hidden,
        reason: OrgReason::None,
        cancel_href: format!("{base}?from={}", rs::encode(&from)),
        refusal: None,
        error: None,
    })
}

// ═════════════════════════════════════════════════════════════════════════
// IA
// ═════════════════════════════════════════════════════════════════════════

/// Uma capacidade de inferência (`AiCapability`).
#[must_use]
pub fn ai_cap(v: &str) -> Option<AiCap> {
    match v {
        "GENERAL" => Some(AiCap::General),
        "CODING" => Some(AiCap::Coding),
        "REASONING" => Some(AiCap::Reasoning),
        "EMBEDDING" => Some(AiCap::Embedding),
        _ => None,
    }
}

/// O motivo tipado (`AiReasonCode`). Os que o Design não desenha — e qualquer
/// um que esta versão não conheça — lêem-se «desconhecido», nunca o vizinho.
#[must_use]
pub fn ai_reason(v: &str) -> AiReason {
    match v {
        "AI_NO_PROVIDER_AVAILABLE" => AiReason::NoProvider,
        "AI_NO_COMPATIBLE_MODEL" => AiReason::NoCompatibleModel,
        "AI_CAPACITY_UNAVAILABLE" => AiReason::CapacityUnavailable,
        "AI_MODEL_HARDWARE_NOT_SATISFIED" => AiReason::HardwareNotSatisfied,
        "AI_PROVIDER_UNHEALTHY" => AiReason::ProviderUnhealthy,
        "AI_MODEL_LOADING" => AiReason::ModelLoading,
        "AI_DISABLED_BY_POLICY" => AiReason::DisabledByPolicy,
        "AI_POLICY_BLOCKED" => AiReason::PolicyBlocked,
        _ => AiReason::Unknown,
    }
}

/// As capacidades do estado, com o encaminhamento quando a política é
/// legível (só a autoridade de plataforma a lê).
#[must_use]
pub fn ai_caps(status: &Value, policy: Option<&Value>, providers: &[Value]) -> Vec<AiCapVm> {
    let label_of = |id: &str| {
        providers
            .iter()
            .find(|p| text(p, "id") == id)
            .map(|p| text(p, "label").to_owned())
    };
    status
        .get("capabilities")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|c| {
                    let cap = ai_cap(text(c, "capability"))?;
                    let available = c.get("available").and_then(Value::as_bool) == Some(true);
                    let pref = policy.and_then(|p| {
                        p.get("preferences")
                            .and_then(Value::as_array)
                            .and_then(|l| {
                                l.iter()
                                    .find(|x| text(x, "capability") == text(c, "capability"))
                                    .cloned()
                            })
                    });
                    Some(AiCapVm {
                        cap,
                        available,
                        model: rs::opt(c, "configured_model"),
                        reason: (!available).then(|| {
                            c.get("reason")
                                .and_then(Value::as_str)
                                .map_or(AiReason::Unknown, ai_reason)
                        }),
                        preferred: pref
                            .as_ref()
                            .and_then(|p| p.get("preferred_provider_id").and_then(Value::as_str))
                            .and_then(label_of),
                        // Sem política legível não se sabe; com política e sem
                        // linha, o Core recorre (a omissão é permitir).
                        fallback: policy.map(|_| {
                            pref.as_ref()
                                .and_then(|p| p.get("allow_fallback").and_then(Value::as_bool))
                                .unwrap_or(true)
                        }),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

/// O estado declarado de um modelo, traduzido, com o tom.
#[must_use]
pub fn model_status(v: &str) -> (String, ResTone) {
    match v {
        "available" => (t("prod.ai.model.available").to_owned(), ResTone::Done),
        "unavailable" => (
            t("prod.ai.model.unavailable").to_owned(),
            ResTone::Attention,
        ),
        "disabled" => (t("prod.ai.model.disabled").to_owned(), ResTone::Closed),
        _ => (t("ai.reason.unknown").to_owned(), ResTone::Neutral),
    }
}

/// Os modelos registados. O fornecedor de um modelo de fornecedor diz-se pelo
/// rótulo quando o actor o pode ler, senão pelo tipo — nunca pelo identificador.
#[must_use]
pub fn ai_models(models: &[Value], providers: &[Value]) -> Vec<AiModelVm> {
    models
        .iter()
        .map(|m| {
            let kind = text(m, "provider_kind");
            let local = kind != "external";
            let provider = match kind {
                "ocinye_node" => text(m, "provider_name").to_owned(),
                _ => providers
                    .iter()
                    .find(|p| text(p, "id") == text(m, "provider_name"))
                    .map_or_else(
                        || {
                            t(if local {
                                "prod.ai.provider.local"
                            } else {
                                "prod.ai.provider.external"
                            })
                            .to_owned()
                        },
                        |p| text(p, "label").to_owned(),
                    ),
            };
            let (status, tone) = model_status(text(m, "status"));
            AiModelVm {
                name: text(m, "model_name").to_owned(),
                version: text(m, "version").to_owned(),
                provider,
                local,
                status,
                tone,
                enabled: m.get("enabled").and_then(Value::as_bool) == Some(true),
                caps: m
                    .get("capabilities")
                    .and_then(Value::as_array)
                    .map(|a| {
                        a.iter()
                            .filter_map(Value::as_str)
                            .filter_map(ai_cap)
                            .collect()
                    })
                    .unwrap_or_default(),
                max_class: rs::classification(text(m, "max_classification")),
            }
        })
        .collect()
}

/// Os fornecedores, sem credencial nem endereço: só o que a lista branca diz.
#[must_use]
pub(crate) fn ai_providers(list: &[Value], clock: &Clock) -> Vec<AiProviderVm> {
    list.iter()
        .map(|p| AiProviderVm {
            label: text(p, "label").to_owned(),
            kind: text(p, "kind").to_owned(),
            local: text(p, "residency") == "local",
            enabled: p.get("enabled").and_then(Value::as_bool) == Some(true),
            healthy: match text(p, "health") {
                "healthy" => Some(true),
                "unreachable" | "refused" => Some(false),
                _ => None,
            },
            checked: instant(p, "last_checked_at").map(|at| clock.when(at)),
            has_credential: p.get("secret_id").is_some_and(|s| !s.is_null()),
            // Activar/desactivar fica para quando houver o contrato do segredo
            // só-de-escrita e a confirmação desenhada (AI-05, DEFERRED).
            toggle: None,
        })
        .collect()
}

/// O tecto externo da política: `None` = sem IA externa.
#[must_use]
pub fn external_max(policy: &Value) -> Option<ResClassification> {
    policy
        .get("external_max_classification")
        .and_then(Value::as_str)
        .map(rs::classification)
}

// ═════════════════════════════════════════════════════════════════════════
// Agentes
// ═════════════════════════════════════════════════════════════════════════

/// O estado derivado de um agente (`AgentState`).
#[must_use]
pub fn agent_status(v: &str) -> Option<AgentStatus> {
    match v {
        "ready" => Some(AgentStatus::Ready),
        "configured" => Some(AgentStatus::Configured),
        "disabled" => Some(AgentStatus::Disabled),
        "archived" => Some(AgentStatus::Archived),
        _ => None,
    }
}

/// O âmbito (`AgentScope`).
#[must_use]
pub fn agent_scope(v: &str) -> Option<AgentScopeVm> {
    match v {
        "personal" => Some(AgentScopeVm::Personal),
        "workspace" => Some(AgentScopeVm::Workspace),
        "unit" => Some(AgentScopeVm::Unit),
        "institutional" => Some(AgentScopeVm::Institutional),
        _ => None,
    }
}

const fn scope_label(s: AgentScopeVm) -> &'static str {
    match s {
        AgentScopeVm::Personal => "agents.scope.personal",
        AgentScopeVm::Workspace => "agents.scope.workspace",
        AgentScopeVm::Unit => "agents.scope.unit",
        AgentScopeVm::Institutional => "agents.scope.institutional",
    }
}

/// As colunas da lista de agentes.
pub const AGENT_COLUMNS: [&str; 3] = ["agents.col.cap", "agents.col.scope", "agents.col.created"];

/// A lista de agentes.
#[must_use]
pub(crate) fn agent_items(list: &[Value], open: Option<&str>, clock: &Clock) -> Vec<ResItemVm> {
    list.iter()
        .filter_map(|a| {
            let id = text(a, "id");
            let status = agent_status(text(a, "state"))?;
            Some(ResItemVm {
                title: text(a, "name").to_owned(),
                code: None,
                state: Some(crate::ui::apps::ops::agent_state(status)),
                cells: vec![
                    ai_cap(text(a, "capability"))
                        .map(|c| t(crate::ui::apps::ops::cap_key(c)).to_owned()),
                    agent_scope(text(a, "scope")).map(|s| t(scope_label(s)).to_owned()),
                    instant(a, "created_at").map(|at| rs::day(at, clock)),
                ],
                overdue: false,
                priority: None,
                href: format!("/ai/agents/{id}"),
                active: open == Some(id),
            })
        })
        .collect()
}

/// Um agente aberto. As instruções só vêm do Core para quem o criou; aqui não
/// se volta a decidir, mas também não se inventam.
#[must_use]
pub(crate) fn agent(a: &Value, target: Option<ResLinkVm>, clock: &Clock) -> Option<AgentVm> {
    let id = text(a, "id");
    let status = agent_status(text(a, "state"))?;
    let b = |k: &str| a.get(k).and_then(Value::as_bool) == Some(true);
    Some(AgentVm {
        name: text(a, "name").to_owned(),
        purpose: rs::opt(a, "purpose"),
        status,
        cap: ai_cap(text(a, "capability"))?,
        scope: agent_scope(text(a, "scope"))?,
        scope_target: target,
        max_class: rs::classification(text(a, "max_classification")),
        sources: (
            b("uses_bibliography"),
            b("uses_documents"),
            b("uses_datasets"),
        ),
        instructions: rs::opt(a, "instructions"),
        created: format!(
            "{} · {}",
            text(a, "created_by_name"),
            instant(a, "created_at")
                .map(|at| rs::day(at, clock))
                .unwrap_or_default()
        ),
        // Abrir na Nye só quando alguma capacidade o serve agora: a execução é
        // da Nye e do Core, e um agente sem inferência não se executa.
        nye: (status == AgentStatus::Ready).then(|| AppNyeVm {
            href: format!("/ai/prompt?ref=agent:{id}"),
            label_key: "agents.nye",
        }),
    })
}

/// O formulário de criação, só com os âmbitos que o Core diz que o actor pode
/// criar (`GET /ai/agents/capabilities`). `None` = nenhum.
#[must_use]
pub fn agent_form(caps: &Value) -> Option<AgentFormVm> {
    let scopes: Vec<Value> = caps
        .get("scopes")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let offered: Vec<(AgentScopeVm, &Value)> = scopes
        .iter()
        .filter_map(|s| agent_scope(text(s, "scope")).map(|k| (k, s)))
        .collect();
    if offered.is_empty() {
        return None;
    }
    let opt = |value: &str, label: &str| ResOptionVm {
        value: value.to_owned(),
        label: label.to_owned(),
        selected: false,
    };
    let mut targets = vec![opt("", t("prod.agents.target.none"))];
    for (k, s) in &offered {
        for x in s
            .get("targets")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            targets.push(opt(
                text(x, "id"),
                &format!("{} · {}", t(scope_label(*k)), text(x, "name")),
            ));
        }
    }
    let mut scopes: Vec<ResOptionVm> = offered
        .iter()
        .map(|(k, s)| opt(text(s, "scope"), t(scope_label(*k))))
        .collect();
    if let Some(first) = scopes.first_mut() {
        first.selected = true;
    }
    Some(AgentFormVm {
        action: "/ai/agents/new".to_owned(),
        scopes,
        targets,
        caps: [
            AiCap::General,
            AiCap::Coding,
            AiCap::Reasoning,
            AiCap::Embedding,
        ]
        .into_iter()
        .map(|c| ResOptionVm {
            value: match c {
                AiCap::General => "GENERAL",
                AiCap::Coding => "CODING",
                AiCap::Reasoning => "REASONING",
                AiCap::Embedding => "EMBEDDING",
            }
            .to_owned(),
            label: t(crate::ui::apps::ops::cap_key(c)).to_owned(),
            selected: c == AiCap::General,
        })
        .collect(),
        classes: [
            ("PUBLIC", ResClassification::Public),
            ("INTERNAL", ResClassification::Internal),
            ("CONFIDENTIAL", ResClassification::Confidential),
            ("RESTRICTED", ResClassification::Restricted),
        ]
        .into_iter()
        .map(|(v, c)| ResOptionVm {
            value: v.to_owned(),
            label: t(c.key()).to_owned(),
            selected: c == ResClassification::Internal,
        })
        .collect(),
        error: None,
    })
}

/// O âmbito submetido pertence aos oferecidos (o Core decide na mesma; isto só
/// evita mandar ao Core o que o formulário nunca ofereceu).
#[must_use]
pub fn scope_offered(caps: &Value, scope: &str, target: Option<&str>) -> bool {
    caps.get("scopes")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|s| text(s, "scope") == scope)
        .any(|s| {
            let ts = s.get("targets").and_then(Value::as_array);
            match (ts.is_some_and(|a| !a.is_empty()), target) {
                (false, None) => true,
                (true, Some(id)) => ts.into_iter().flatten().any(|x| text(x, "id") == id),
                _ => false,
            }
        })
}

// ═════════════════════════════════════════════════════════════════════════
// Computação
// ═════════════════════════════════════════════════════════════════════════

/// O estado de um nó (`ComputeNodeStatus`).
#[must_use]
pub fn node_status(v: &str) -> Option<NodeStatus> {
    match v {
        "pending_enrollment" => Some(NodeStatus::Pending),
        "online" => Some(NodeStatus::Online),
        "offline" => Some(NodeStatus::Offline),
        "draining" => Some(NodeStatus::Draining),
        "retired" => Some(NodeStatus::Retired),
        _ => None,
    }
}

fn label_or_code(key: Option<&'static str>, code: &str) -> String {
    key.map_or_else(|| code.to_owned(), |k| t(k).to_owned())
}

/// O tipo de nó, traduzido.
#[must_use]
pub fn node_kind(v: &str) -> String {
    label_or_code(
        match v {
            "gpu" => Some("prod.compute.kind.gpu"),
            "cpu" => Some("prod.compute.kind.cpu"),
            "hpc" => Some("prod.compute.kind.hpc"),
            "storage" => Some("prod.compute.kind.storage"),
            _ => None,
        },
        v,
    )
}

/// O controlo institucional, traduzido.
#[must_use]
pub fn node_control(v: &str) -> String {
    label_or_code(
        match v {
            "OCINYE" => Some("prod.compute.control.ocinye"),
            "EXTERNAL" => Some("prod.compute.control.external"),
            _ => None,
        },
        v,
    )
}

/// A residência física, traduzida.
#[must_use]
pub fn node_residency(v: &str) -> String {
    label_or_code(
        match v {
            "UNDECLARED" => Some("prod.compute.residency.undeclared"),
            "THIRD_PARTY_CLOUD" => Some("prod.compute.residency.cloud"),
            "OCINYE_CAMAMA" => Some("prod.compute.residency.camama"),
            "OCINYE_COLOCATION" => Some("prod.compute.residency.colocation"),
            _ => None,
        },
        v,
    )
}

fn gpus(n: &Value) -> Option<String> {
    let c = n.get("capacity")?;
    let count = c.get("gpus").and_then(Value::as_i64).unwrap_or(0);
    (count > 0).then(|| {
        let mem = c
            .get("gpu_memory_bytes")
            .and_then(Value::as_i64)
            .and_then(|m| u64::try_from(m).ok())
            .filter(|m| *m > 0)
            .map(bytes);
        mem.map_or_else(|| count.to_string(), |m| format!("{count} · {m}"))
    })
}

/// Uma linha de capacidade: números só quando reportados; a fracção só com o
/// físico e o uso reportados.
#[must_use]
pub fn cap_line(label_key: &'static str, l: &Value, cores: bool) -> CapLineVm {
    let fmt = |k: &str| {
        l.get(k).and_then(Value::as_i64).map(|n| {
            if cores {
                n.to_string()
            } else {
                bytes(u64::try_from(n).unwrap_or(0))
            }
        })
    };
    let physical = l.get("physical").and_then(Value::as_i64);
    let consumed = l.get("consumed").and_then(Value::as_i64);
    CapLineVm {
        label_key,
        physical: fmt("physical"),
        reserved: fmt("reserved").unwrap_or_else(|| "0".to_owned()),
        allocatable: fmt("allocatable"),
        consumed: fmt("consumed"),
        consumed_pct: match (physical, consumed) {
            (Some(p), Some(c)) if p > 0 => u8::try_from((c.max(0) * 100 / p).min(100)).ok(),
            _ => None,
        },
    }
}

/// As colunas da lista de nós.
pub const NODE_COLUMNS: [&str; 3] = ["compute.col.kind", "compute.col.gpus", "compute.col.seen"];

/// A lista de nós.
#[must_use]
pub(crate) fn node_items(list: &[Value], open: Option<&str>, clock: &Clock) -> Vec<ResItemVm> {
    list.iter()
        .filter_map(|n| {
            let id = text(n, "id");
            let status = node_status(text(n, "status"))?;
            Some(ResItemVm {
                title: text(n, "display_name").to_owned(),
                code: Some(text(n, "identifier").to_owned()),
                state: Some(crate::ui::apps::ops::node_state(status)),
                cells: vec![
                    Some(node_kind(text(n, "kind"))),
                    gpus(n),
                    instant(n, "last_seen_at").map(|at| clock.relative(at)),
                ],
                overdue: false,
                priority: None,
                href: format!("/compute/nodes/{id}"),
                active: open == Some(id),
            })
        })
        .collect()
}

/// Um nó aberto.
#[must_use]
pub(crate) fn node(n: &Value, clock: &Clock) -> Option<NodeVm> {
    let status = node_status(text(n, "status"))?;
    let c = n.get("capacity").cloned().unwrap_or(Value::Null);
    let line = |k: &str| c.get(k).cloned().unwrap_or(Value::Null);
    Some(NodeVm {
        name: text(n, "display_name").to_owned(),
        identifier: text(n, "identifier").to_owned(),
        status,
        kind: node_kind(text(n, "kind")),
        location: rs::opt(n, "location_label"),
        control: node_control(text(n, "institutional_control")),
        residency: node_residency(text(n, "physical_residency")),
        lines: vec![
            cap_line("compute.cpu", &line("cpu_cores"), true),
            cap_line("compute.memory", &line("memory_bytes"), false),
            cap_line("compute.storage", &line("storage_bytes"), false),
        ],
        gpus: gpus(n),
        agent_version: rs::opt(n, "agent_version"),
        last_seen: instant(n, "last_seen_at").map(|at| clock.relative(at)),
    })
}

// ═════════════════════════════════════════════════════════════════════════
// Meus Recursos
// ═════════════════════════════════════════════════════════════════════════

/// O estado do armazenamento pessoal.
#[must_use]
pub fn storage_state(v: &str) -> StorageStateVm {
    match v {
        "warning" => StorageStateVm::Warning,
        "critical" => StorageStateVm::Critical,
        "over_quota" => StorageStateVm::OverQuota,
        _ => StorageStateVm::Normal,
    }
}

fn pct(part: i64, whole: i64) -> Option<u8> {
    (whole > 0).then(|| u8::try_from((part.max(0) * 100 / whole).min(100)).unwrap_or(100))
}

/// O quadro de consumo do próprio membro (`GET /resources/me`).
#[must_use]
pub(crate) fn resources(v: &Value, clock: &Clock) -> ResourcesVm {
    let s = v.get("storage").cloned().unwrap_or(Value::Null);
    let e = v.get("storage_entitlement").cloned().unwrap_or(Value::Null);
    let n = |x: &Value, k: &str| x.get(k).and_then(Value::as_i64).unwrap_or(0);
    let b = |x: i64| bytes(u64::try_from(x).unwrap_or(0));
    let used = n(&s, "used_bytes");
    let reserved = n(&s, "reserved_bytes");
    let limit = n(&s, "limit_bytes");
    ResourcesVm {
        load: crate::ui::view_models::AppLoad::Ready,
        used: b(used),
        reserved: b(reserved),
        limit: (limit > 0).then(|| b(limit)),
        available: b(n(&s, "available_bytes")),
        used_pct: pct(used, limit),
        reserved_pct: (reserved > 0).then(|| pct(reserved, limit)).flatten(),
        state: storage_state(text(&s, "state")),
        entitlement: b(n(&e, "quantity")),
        parts: e
            .get("parts")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .map(|p| EntPartVm {
                        source: t(match text(p, "source") {
                            "override" => "resources.src.override",
                            "temporary" => "resources.src.temporary",
                            _ => "resources.src.profile",
                        })
                        .to_owned(),
                        quantity: b(n(p, "quantity")),
                        expires: instant(p, "expires_at").map(|at| rs::day(at, clock)),
                        note: text(p, "note").to_owned(),
                    })
                    .collect()
            })
            .unwrap_or_default(),
        files_href: Some("/files".to_owned()),
    }
}

// ═════════════════════════════════════════════════════════════════════════
// Actividade
// ═════════════════════════════════════════════════════════════════════════

/// O que a releitura do alvo, com a autoridade de agora, disse.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Target {
    /// O Core devolveu-o: o título de agora.
    Visible(String),
    /// O evento não tem alvo com ecrã, e o ambiente é legível.
    Context,
    /// Já não é legível (ou nunca foi): nada dele se mostra.
    Redacted,
}

/// A leitura autorizada de cada tipo de alvo (a mesma da Nye e das ligações).
#[must_use]
pub fn target_path(subject_type: &str, id: &str) -> Option<(String, &'static str)> {
    Some(match subject_type {
        "idea" => (format!("/api/v1/ideas/{id}"), "title"),
        "project" => (format!("/api/v1/projects/{id}"), "title"),
        "task" => (format!("/api/v1/tasks/{id}"), "title"),
        "dataset" => (format!("/api/v1/datasets/{id}"), "title"),
        "source" => (format!("/api/v1/sources/{id}"), "title"),
        "document" => (format!("/api/v1/documents/{id}"), "title"),
        _ => return None,
    })
}

/// O título que a leitura autorizada devolveu (a ideia vem dentro de `idea`,
/// como na Nye; os outros no topo). Vazio = nada legível.
#[must_use]
pub fn target_title<'a>(subject_type: &str, v: &'a Value, field: &str) -> &'a str {
    match subject_type {
        "idea" => v.get("idea").map_or("", |i| text(i, field)),
        _ => text(v, field),
    }
}

/// O verbo neutro de um evento redigido: diz que aconteceu, nunca a quê.
#[must_use]
pub fn kind_phrase(kind: &str) -> &'static str {
    match kind {
        "created" => "prod.activity.kind.created",
        "updated" => "prod.activity.kind.updated",
        "state_changed" => "prod.activity.kind.state_changed",
        "commented" => "prod.activity.kind.commented",
        "member_added" => "prod.activity.kind.member_added",
        "attached" => "prod.activity.kind.attached",
        "published" => "prod.activity.kind.published",
        "shared" => "prod.activity.kind.shared",
        "revoked" => "prod.activity.kind.revoked",
        "deleted" => "prod.activity.kind.deleted",
        "restored" => "prod.activity.kind.restored",
        _ => "prod.activity.kind.other",
    }
}

/// Um evento, com o alvo resolvido agora. Um alvo que deixou de ser visível não
/// leva título, resumo (que o cita), classificação, nem ambiente.
#[must_use]
pub(crate) fn activity_item(
    e: &Value,
    target: &Target,
    context: Option<String>,
    clock: &Clock,
    last_day: &mut Option<String>,
) -> ActivityItemVm {
    let at = instant(e, "created_at");
    let day = at.map(|a| rs::day(a, clock));
    let sep = if day.is_some() && day != *last_day {
        last_day.clone_from(&day);
        day
    } else {
        None
    };
    let redacted = *target == Target::Redacted;
    let link = match target {
        Target::Visible(title) => {
            let st = text(e, "subject_type");
            let (kind, href, kind_label) = rs::kind_and_href(st, text(e, "subject_id"));
            Some(ResLinkVm {
                kind,
                kind_label,
                title: title.clone(),
                meta: None,
                relation: None,
                by_operation: false,
                href,
            })
        }
        _ => None,
    };
    ActivityItemVm {
        actor: rs::opt(e, "actor_name"),
        summary: if redacted {
            t(kind_phrase(text(e, "kind"))).to_owned()
        } else {
            text(e, "summary").to_owned()
        },
        target: link,
        redacted,
        context: if redacted { None } else { context },
        class: if redacted {
            ResClassification::Restricted
        } else {
            rs::classification(text(e, "classification"))
        },
        at: at.map(|a| clock.hhmm(a)).unwrap_or_default(),
        day: sep,
    }
}

// ═════════════════════════════════════════════════════════════════════════
// Auditoria
// ═════════════════════════════════════════════════════════════════════════

/// As chaves de metadata que a Auditoria mostra. **Lista branca**: uma chave
/// que não está aqui conta-se e não se mostra — incluindo as que o Core venha a
/// escrever amanhã. Ficam fora, de propósito: endereços e nomes de pessoas,
/// identificadores de pessoas e de sessões, motivos em texto livre, títulos,
/// nomes, rótulos e códigos (dizem *o quê*, e a auditoria diz *que*), somas de
/// verificação e tudo o que se pareça com um segredo.
pub const AUDIT_ALLOWED_KEYS: &[&str] = &[
    "allow_fallback",
    "application",
    "attachments",
    "capability",
    "content_right",
    "content_type",
    "cpu_cores",
    "credential_kind",
    "default_locale",
    "event",
    "expires_at",
    "external_max_classification",
    "external_recipients",
    "favourite",
    "fields",
    "file_count",
    "from",
    "granted_role",
    "kind",
    "memory_bytes",
    "mfa",
    "origin",
    "outcome",
    "parts",
    "permission",
    "policy",
    "position",
    "previous_position",
    "profile",
    "recipients",
    "recovery_codes_issued",
    "relation",
    "replaced_temporary",
    "requires_approval",
    "residency",
    "revision",
    "revoked_role",
    "risk",
    "role",
    "scope",
    "sequence",
    "session_state",
    "sessions_revoked",
    "size_bytes",
    "state",
    "status",
    "steps",
    "steps_succeeded",
    "storage_bytes",
    "timezone",
    "to",
    "total_parts",
    "validation",
    "version",
];

/// O comprimento máximo de um valor mostrado.
const AUDIT_VALUE_MAX: usize = 80;

/// A metadata de um registo: (chave, valor) só das chaves da lista branca com
/// valor escalar; o resto conta-se. Um valor longo corta-se; um objecto ou
/// lista, mesmo numa chave permitida, não se mostra.
#[must_use]
pub fn audit_metadata(meta: &Value) -> (Vec<(String, String)>, u32) {
    let Some(obj) = meta.as_object() else {
        return (Vec::new(), 0);
    };
    let mut shown = Vec::new();
    let mut omitted = 0u32;
    for (k, v) in obj {
        let value = match v {
            Value::String(s) => Some(excerpt(s, AUDIT_VALUE_MAX)),
            Value::Number(n) => Some(n.to_string()),
            Value::Bool(b) => Some(b.to_string()),
            _ => None,
        };
        match value {
            Some(v) if AUDIT_ALLOWED_KEYS.contains(&k.as_str()) => shown.push((k.clone(), v)),
            _ => omitted += 1,
        }
    }
    shown.sort();
    (shown, omitted)
}

/// O resultado (`success` | `denied` | `failure`).
#[must_use]
pub fn audit_outcome(v: &str) -> Option<AuditOutcome> {
    match v {
        "success" => Some(AuditOutcome::Success),
        "denied" => Some(AuditOutcome::Denied),
        "failure" => Some(AuditOutcome::Failure),
        _ => None,
    }
}

/// Uma linha de auditoria. `base` é a lista com os filtros correntes.
#[must_use]
pub(crate) fn audit_row(
    r: &Value,
    base: &str,
    open: Option<&str>,
    clock: &Clock,
) -> Option<AuditRowVm> {
    let id = text(r, "id");
    let at = instant(r, "occurred_at")?;
    let sep = if base.contains('?') { '&' } else { '?' };
    let actor = text(r, "actor_person_id");
    Some(AuditRowVm {
        at: at
            .with_timezone(&clock.zone.zone())
            .format("%d/%m/%Y %H:%M:%S")
            .to_string(),
        at_utc: at.format("%Y-%m-%dT%H:%M:%SZ").to_string(),
        actor: rs::opt(r, "actor_name"),
        on_behalf: rs::opt(r, "actor_on_behalf_of"),
        action: text(r, "action").to_owned(),
        resource_type: text(r, "resource_type").to_owned(),
        outcome: audit_outcome(text(r, "outcome"))?,
        class: rs::opt(r, "classification").map(|c| rs::classification(&c)),
        href: format!("{base}{sep}open={}", rs::encode(id)),
        active: open == Some(id),
        actor_filter_href: (!actor.is_empty())
            .then(|| format!("/audit?actor={}", rs::encode(actor))),
    })
}

/// O detalhe: a linha, a correlação e a metadata pela lista branca.
#[must_use]
pub fn audit_detail(r: &Value, row: AuditRowVm, target: Option<ResLinkVm>) -> AuditDetailVm {
    let (metadata, omitted) = audit_metadata(r.get("metadata").unwrap_or(&Value::Null));
    AuditDetailVm {
        row,
        correlation: rs::opt(r, "correlation_id").map(|c| excerpt(&c, 64)),
        metadata,
        omitted,
        target,
    }
}

/// Os tipos de recurso que a Auditoria filtra (os que o Core escreve).
pub const AUDIT_TYPES: &[&str] = &[
    "ai_agent",
    "ai_provider",
    "compute_node",
    "dataset",
    "dataset_version",
    "document",
    "file",
    "idea",
    "mail_message",
    "note",
    "person",
    "project",
    "research_workspace",
    "source",
    "task",
    "unit",
    "unit_membership",
    "workspace_membership",
];

/// As opções do filtro de tipo.
#[must_use]
pub fn audit_types(selected: Option<&str>) -> Vec<ResOptionVm> {
    let mut out = vec![ResOptionVm {
        value: String::new(),
        label: t("audit.types.all").to_owned(),
        selected: selected.is_none_or(str::is_empty),
    }];
    out.extend(AUDIT_TYPES.iter().map(|k| ResOptionVm {
        value: (*k).to_owned(),
        label: (*k).to_owned(),
        selected: selected == Some(*k),
    }));
    out
}

// ═════════════════════════════════════════════════════════════════════════
// Definições
// ═════════════════════════════════════════════════════════════════════════

/// O avatar do membro como o Core o guarda.
#[must_use]
pub fn avatar_vm(name: &str, choice: &ocinye_contracts::AvatarChoice) -> OrgAvatarVm {
    let mut a = og::avatar(name);
    a.image_href = match choice {
        ocinye_contracts::AvatarChoice::Initials => None,
        ocinye_contracts::AvatarChoice::Preset { preset } => {
            ocinye_contracts::AvatarChoice::preset_file(preset)
                .map(|f| format!("/static/avatars/{f}"))
        }
        ocinye_contracts::AvatarChoice::Custom { version } => {
            Some(format!("/avatar/me/{}", rs::encode(version)))
        }
    };
    a
}

/// Os avatares do produto, com o escolhido.
#[must_use]
pub fn presets(choice: &ocinye_contracts::AvatarChoice) -> Vec<ResOptionVm> {
    let current = match choice {
        ocinye_contracts::AvatarChoice::Preset { preset } => Some(preset.as_str()),
        _ => None,
    };
    ocinye_contracts::AVATAR_PRESETS
        .iter()
        .map(|(id, _)| ResOptionVm {
            value: (*id).to_owned(),
            label: (*id).to_owned(),
            selected: current == Some(*id),
        })
        .collect()
}

/// As três línguas, pelo seu nome na própria língua.
#[must_use]
pub fn locales(current: ocinye_contracts::Locale) -> Vec<ResOptionVm> {
    [
        (ocinye_contracts::Locale::Pt, "Português"),
        (ocinye_contracts::Locale::En, "English"),
        (ocinye_contracts::Locale::Fr, "Français"),
    ]
    .into_iter()
    .map(|(l, name)| ResOptionVm {
        value: l.as_str().to_owned(),
        label: name.to_owned(),
        selected: l == current,
    })
    .collect()
}

/// As sessões próprias. Terminar só as outras: terminar esta é sair.
#[must_use]
pub(crate) fn own_sessions(list: &[Value], clock: &Clock) -> Vec<OwnSessionVm> {
    list.iter()
        .map(|s| {
            let current = s.get("is_current").and_then(Value::as_bool) == Some(true);
            let id = text(s, "id");
            OwnSessionVm {
                agent: rs::opt(s, "user_agent").map_or_else(
                    || t("prod.settings.session.unknown").to_owned(),
                    |a| excerpt(&a, 60),
                ),
                seen: instant(s, "last_seen_at")
                    .or_else(|| instant(s, "issued_at"))
                    .map(|at| clock.relative(at))
                    .unwrap_or_default(),
                current,
                revoke_action: (!current && !id.is_empty())
                    .then(|| format!("/settings/sessions/{}/revoke", rs::encode(id))),
            }
        })
        .collect()
}

/// As aplicações fixáveis que o membro vê, marcadas as da lista efectiva.
#[must_use]
pub fn pins(
    visible: &[&'static crate::experience::apps::Application],
    pinned: &[String],
) -> Vec<PinVm> {
    visible
        .iter()
        .filter(|a| a.can_pin())
        .map(|a| PinVm {
            id: a.id().to_owned(),
            label: a.label().to_owned(),
            icon: crate::ui::components::app_icon(a.route()),
            pinned: pinned.iter().any(|p| p == a.id()),
        })
        .collect()
}

/// A origem da lista efectiva (`member` | `instance` | `product`), traduzida.
#[must_use]
pub fn pins_source(v: &str) -> String {
    t(match v {
        "member" => "settings.pins.member",
        "instance" => "settings.pins.instance",
        _ => "settings.pins.product",
    })
    .to_owned()
}

/// A navegação das Definições.
#[must_use]
pub fn settings_nav(active: crate::ui::view_models::SettingsSection) -> Vec<AppNavVm> {
    use crate::ui::view_models::SettingsSection as S;
    [
        (S::Account, "settings.nav.account", "user", "/settings"),
        (
            S::Language,
            "settings.nav.language",
            "language",
            "/settings/language",
        ),
        (
            S::Security,
            "settings.nav.security",
            "shield",
            "/settings/security",
        ),
        (S::Apps, "settings.nav.apps", "apps", "/settings/apps"),
    ]
    .into_iter()
    .map(|(s, k, ic, h)| rs::nav(k, ic, h.to_owned(), s == active))
    .collect()
}

// ═════════════════════════════════════════════════════════════════════════
// Ajuda
// ═════════════════════════════════════════════════════════════════════════

/// Um tópico por aplicação do registo, filtrado no servidor pela pesquisa
/// (rótulo, descrição e palavras estáveis). «Abrir» só para as que o membro vê.
#[must_use]
pub fn help_topics(
    all: &[crate::experience::apps::Application],
    visible: &[&'static crate::experience::apps::Application],
    query: &str,
) -> Vec<HelpTopicVm> {
    let q = query.trim().to_lowercase();
    let q: String = q.chars().take(80).collect();
    all.iter()
        .filter(|a| {
            q.is_empty()
                || a.label().to_lowercase().contains(&q)
                || a.description().to_lowercase().contains(&q)
                || a.keywords.iter().any(|k| k.contains(q.as_str()))
        })
        .map(|a| HelpTopicVm {
            title: a.label().to_owned(),
            icon: crate::ui::components::app_icon(a.route()),
            category: t(a.category().label_key()).to_owned(),
            body: a.description().to_owned(),
            open_href: visible
                .iter()
                .any(|v| v.id() == a.id())
                .then(|| a.route().to_owned()),
            anchor: format!("help-{}", a.id()),
        })
        .collect()
}

/// Os atalhos declarados (`experience::shortcuts`), traduzidos.
#[must_use]
pub fn help_shortcuts() -> Vec<HelpShortcutVm> {
    crate::experience::shortcuts::SHORTCUTS
        .iter()
        .map(|s| HelpShortcutVm {
            keys: s.keys.to_owned(),
            what: t(s.what_key).to_owned(),
        })
        .collect()
}

/// A navegação da Ajuda.
#[must_use]
pub fn help_nav(shortcuts: bool) -> Vec<AppNavVm> {
    vec![
        rs::nav("help.nav.apps", "apps", "/help".to_owned(), !shortcuts),
        rs::nav(
            "help.nav.shortcuts",
            "command",
            "/help?nav=shortcuts".to_owned(),
            shortcuts,
        ),
    ]
}

/// A Nye com a pergunta da Ajuda: entrega-a à superfície canónica.
#[must_use]
pub fn help_nye(query: &str) -> AppNyeVm {
    let q = query.trim();
    AppNyeVm {
        href: if q.is_empty() {
            "/ai/prompt".to_owned()
        } else {
            format!("/ask?q={}", rs::encode(q))
        },
        label_key: "ai.open_nye",
    }
}

/// O instante de uma data do filtro «desde» (`AAAA-MM-DD`) à meia-noite do
/// fuso do membro, em RFC 3339.
#[must_use]
pub(crate) fn since_instant(d: &str, clock: &Clock) -> Option<DateTime<Utc>> {
    let day = chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d").ok()?;
    Some(crate::controllers::productivity::calendar::local_midnight(
        day, clock.zone,
    ))
}
