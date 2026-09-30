//! A Nye ligada ao Core (D003 · NYE-01 a NYE-14, ADR-0619).
//!
//! A Nye não decide nada. Este módulo traduz o que o Core já decidiu para os
//! view models do Design:
//!
//! - **Disponibilidade** (NYE-01): pesquisar depende só do Core; perguntar e
//!   agir dependem de inferência (o Planner precisa dela) e da permissão
//!   `ai.use`; voz e anexos não existem nesta Instância. Nunca «IA ligada».
//! - **Pesquisa** (NYE-04): os resultados vêm do `POST /agentic/invoke` com
//!   `intent = search`, que é determinístico e filtrado pelo Core para quem
//!   pergunta. Só entram tipos com rota canónica no Workspace; um tipo sem
//!   rota não aparece (não se inventa ligação).
//! - **Propostas** (NYE-06 a NYE-08): um plano do Core vira uma
//!   `NyeProposalVm`. O risco é o do Core (`RiskLevel`), a necessidade de
//!   confirmação é a do Core (`requires_approval`), o estado é o do Core
//!   (`PlanState`). O `digest` identifica o que foi mostrado; não autoriza.
//! - **Conversas** (NYE-02, NYE-03): as do membro, pelo Core. Uma resposta
//!   degradada (sem inferência) não é apresentada como resposta: é o estado
//!   de falha tipado, sem o texto de sistema do Core.
//!
//! Nenhum nome de modelo ou fornecedor sai daqui.

use ocinye_contracts::access::Permission;
use serde_json::Value;

/// Se este membro pode usar a assistência (`ai.use`).
#[must_use]
pub fn may_use_ai(viewer: &Viewer) -> bool {
    viewer.can(Permission::AiUse)
}

use crate::controllers::desktop::{instant, text, Clock};
use crate::experience::navigation::Viewer;
use crate::i18n::t;
use crate::ui::view_models::{
    NyeAuth, NyeAvail, NyeAvailability, NyeBlock, NyeComposerState, NyeComposerVm, NyeConvItemVm,
    NyeConversationVm, NyeExecState, NyeExecutionVm, NyeField, NyeGrounding, NyeHitGroupVm,
    NyeHitVm, NyeIntent, NyeKind, NyeLine, NyeLink, NyeMessageVm, NyeMsgState, NyeProposalVm,
    NyeReason, NyeRisk, NyeRole,
};

/// Quantos resultados de um tipo a superfície mostra.
const HITS_PER_GROUP: usize = 6;

/// NYE-01 · O que a Nye pode fazer agora, com o que a casca já perguntou.
///
/// `may_use_ai`: o membro tem `ai.use` (`Viewer::can`). `core_ok`: o Core
/// respondeu à prontidão. `ai`: o `GET /ai/status`, ou `None` quando não houve
/// resposta (o que não é «sem inferência»).
#[must_use]
pub fn availability(may_use_ai: bool, core_ok: bool, ai: Option<&Value>) -> NyeAvailability {
    if !core_ok {
        let down = NyeAvail::Unavailable(NyeReason::CoreUnavailable);
        return NyeAvailability {
            search: down,
            ask: down,
            act: down,
            voice_input: NyeAvail::Unavailable(NyeReason::VoiceUnavailable),
            voice_output: NyeAvail::Unavailable(NyeReason::VoiceUnavailable),
            attachments: NyeAvail::Unavailable(NyeReason::CapabilityUnavailable),
            link: NyeLink::CoreUnavailable,
        };
    }
    let inference = if !may_use_ai {
        NyeAvail::Unavailable(NyeReason::PermissionDenied)
    } else {
        match ai {
            None => NyeAvail::Unavailable(NyeReason::InferenceUnavailable),
            Some(status) => {
                let general = status
                    .get("capabilities")
                    .and_then(Value::as_array)
                    .is_some_and(|caps| {
                        caps.iter().any(|c| {
                            c.get("capability").and_then(Value::as_str) == Some("GENERAL")
                                && c.get("available").and_then(Value::as_bool) == Some(true)
                        })
                    });
                let providers = status.get("providers").and_then(Value::as_u64).unwrap_or(0);
                if status.get("available").and_then(Value::as_bool) == Some(true) && general {
                    NyeAvail::Available
                } else if providers == 0 {
                    NyeAvail::Unavailable(NyeReason::NoInference)
                } else {
                    NyeAvail::Unavailable(NyeReason::NoCompatibleModel)
                }
            }
        }
    };
    NyeAvailability {
        search: NyeAvail::Available,
        ask: inference,
        // Hoje o Planner é inferência: sem ela não há propostas. Os comandos
        // determinísticos (abrir, criar) continuam na pesquisa.
        act: inference,
        // Sem STT/TTS nesta Instância (NYE-11, NYE-12): nunca um microfone
        // que pareça funcionar.
        voice_input: NyeAvail::Unavailable(NyeReason::VoiceUnavailable),
        voice_output: NyeAvail::Unavailable(NyeReason::VoiceUnavailable),
        // NYE-10: anexar por referência ainda não é contrato do Core.
        attachments: NyeAvail::Unavailable(NyeReason::CapabilityUnavailable),
        link: NyeLink::Connected,
    }
}

/// A razão tipada de um `AgenticOutcome::Unavailable` ou de uma resposta
/// degradada (`AiReasonCode`).
#[must_use]
pub fn reason_of(code: &str) -> NyeReason {
    match code {
        "AI_PERMISSION_DENIED" | "AI_MODEL_NOT_ENTITLED" => NyeReason::PermissionDenied,
        "AI_POLICY_BLOCKED" | "AI_DISABLED_BY_POLICY" => NyeReason::PolicyBlocked,
        "AI_NO_PROVIDER_AVAILABLE" | "AI_CAPACITY_UNAVAILABLE" => NyeReason::NoInference,
        "AI_NO_COMPATIBLE_MODEL" | "AI_MODEL_HARDWARE_NOT_SATISFIED" => {
            NyeReason::NoCompatibleModel
        }
        "AI_PROVIDER_UNHEALTHY" => NyeReason::ProviderUnavailable,
        _ => NyeReason::InferenceUnavailable,
    }
}

/// O tipo do pedido, do valor do formulário (`None` = automático).
#[must_use]
pub fn intent_of(value: Option<&str>) -> Option<NyeIntent> {
    match value {
        Some("search") => Some(NyeIntent::Search),
        Some("ask") => Some(NyeIntent::Ask),
        Some("act") => Some(NyeIntent::Act),
        _ => None,
    }
}

/// «Lido como»: o que o Core leria numa frase sem modo escolhido
/// (`Intent::detect`, determinístico).
#[must_use]
pub fn detected(query: &str) -> NyeIntent {
    use ocinye_contracts::agentic::Intent;
    match Intent::detect(query) {
        Intent::Search => NyeIntent::Search,
        Intent::Ask => NyeIntent::Ask,
        _ => NyeIntent::Act,
    }
}

/// A rota canónica de um resultado, e o tipo e a aplicação que o mostram.
/// `None` para tipos sem ecrã no Workspace: não se inventa uma ligação.
fn route_of(entity: &str, id: &str) -> Option<(NyeKind, &'static str, String)> {
    let valid = !id.is_empty() && id.bytes().all(|b| b.is_ascii_hexdigit() || b == b'-');
    if !valid {
        return None;
    }
    let (kind, app, prefix) = match entity {
        "note" => (NyeKind::Note, "nav.notes", "/notes/"),
        "file" => (NyeKind::File, "nav.files", "/files/"),
        "project" => (NyeKind::Project, "nav.projects", "/projects/"),
        "idea" => (NyeKind::Other, "nav.ideas", "/ideas/"),
        "dataset" => (NyeKind::Dataset, "nav.datasets", "/datasets/"),
        "unit" => (NyeKind::Other, "nav.units", "/units/"),
        _ => return None,
    };
    Some((kind, app, format!("{prefix}{id}")))
}

/// NYE-04 · Os resultados do Core, agrupados pelo tipo, pela ordem do Core.
#[must_use]
pub fn hits(sources: &[Value]) -> Vec<NyeHitGroupVm> {
    let mut groups: Vec<(String, NyeHitGroupVm)> = Vec::new();
    for s in sources {
        let entity = text(s, "entity_type");
        let Some((kind, app, href)) = route_of(entity, text(s, "entity_id")) else {
            continue;
        };
        let excerpt = text(s, "excerpt").trim();
        let meta = (!excerpt.is_empty()).then(|| {
            let short: String = excerpt.chars().take(90).collect();
            if short.len() < excerpt.len() {
                format!("{short}…")
            } else {
                short
            }
        });
        let hit = NyeHitVm {
            kind,
            title: text(s, "title").to_owned(),
            context: String::new(),
            meta,
            app: Some(t(app).to_owned()),
            href,
        };
        match groups.iter_mut().find(|(e, _)| e == entity) {
            Some((_, g)) if g.hits.len() < HITS_PER_GROUP => g.hits.push(hit),
            Some(_) => {}
            None => groups.push((
                entity.to_owned(),
                NyeHitGroupVm {
                    kind,
                    hits: vec![hit],
                },
            )),
        }
    }
    groups.into_iter().map(|(_, g)| g).collect()
}

/// O risco do Core, na apresentação do Design. `Navigation` e `Destructive`
/// não vêm daqui: o primeiro é abrir por ligação (sem plano), o segundo não
/// tem capacidade no Core.
#[must_use]
pub fn risk_of(core: &str) -> NyeRisk {
    match core {
        "read_only" => NyeRisk::ReadOnly,
        "low_impact" => NyeRisk::ReversibleWrite,
        "material_mutation" => NyeRisk::InstitutionalChange,
        "external_effect" => NyeRisk::ExternalCommunication,
        // Desconhecido: a apresentação mais cautelosa que o Core conhece.
        _ => NyeRisk::Privileged,
    }
}

fn state_of(plan_state: &str) -> NyeExecState {
    match plan_state {
        "proposed" => NyeExecState::Proposed,
        "awaiting_approval" => NyeExecState::AwaitingConfirmation,
        "approved" => NyeExecState::Authorized,
        "executing" => NyeExecState::Running,
        "completed" => NyeExecState::Completed,
        "partially_completed" => NyeExecState::Partial,
        "failed" => NyeExecState::Failed,
        "rejected" => NyeExecState::Rejected,
        "expired" => NyeExecState::Expired,
        "cancelled" => NyeExecState::Cancelled,
        _ => NyeExecState::Blocked,
    }
}

/// Os valores simples da entrada de um passo, pelo nome do campo.
fn fields_of(input: &Value) -> Vec<NyeField> {
    input
        .as_object()
        .map(|o| {
            o.iter()
                .filter_map(|(k, v)| {
                    let value = match v {
                        Value::String(s) => s.clone(),
                        Value::Number(n) => n.to_string(),
                        Value::Bool(b) => b.to_string(),
                        _ => return None,
                    };
                    let long = value.chars().count() > 80 || value.contains('\n');
                    Some(NyeField {
                        label: k.clone(),
                        value,
                        long,
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

/// NYE-06 a NYE-08 · Um plano do Core como proposta.
///
/// `plan`: o `ActionPlan` de `invoke`, ou o plano de `GET /agentic/plans/{id}`
/// (que traz `risk`, `requires_approval`, `open` e `approval_expires_at`
/// reavaliados pelo Core agora). `requires_approval` vem do Core.
#[must_use]
pub(crate) fn proposal(plan: &Value, requires_approval: bool, clock: &Clock) -> NyeProposalVm {
    let steps = plan
        .get("steps")
        .and_then(Value::as_array)
        .map_or(&[][..], Vec::as_slice);
    let peak = plan.get("risk").and_then(Value::as_str).map_or_else(
        || {
            steps
                .iter()
                .map(|s| risk_rank(text(s, "risk")))
                .max()
                .map_or("read_only", |r| RISKS[r])
        },
        |r| r,
    );
    let state = state_of(text(plan, "state"));
    let capability = steps
        .iter()
        .map(|s| text(s.get("request").unwrap_or(s), "capability").to_owned())
        .collect::<Vec<_>>()
        .join(", ");
    let target = steps
        .iter()
        .map(|s| text(s, "summary").to_owned())
        .collect::<Vec<_>>()
        .join(" · ");
    let fields = steps
        .iter()
        .flat_map(|s| {
            fields_of(
                s.get("request")
                    .and_then(|r| r.get("input"))
                    .unwrap_or(&Value::Null),
            )
        })
        .collect();
    let lines = steps
        .iter()
        .map(|s| {
            let result = s.get("result").filter(|r| !r.is_null());
            NyeLine {
                title: text(s, "summary").to_owned(),
                meta: Some(text(s.get("request").unwrap_or(s), "capability").to_owned()),
                ok: result.map(|r| text(r, "status") == "succeeded"),
                href: None,
            }
        })
        .collect();
    let ran = matches!(
        state,
        NyeExecState::Completed | NyeExecState::Partial | NyeExecState::Failed
    );
    let execution = ran.then(|| NyeExecutionVm {
        summary: None,
        error: match state {
            NyeExecState::Failed => Some(NyeReason::ExecutionFailed),
            NyeExecState::Partial => Some(NyeReason::PartialExecution),
            _ => None,
        },
        error_detail: None,
        // O executor não diz que repetir é seguro: nunca se oferece.
        retry_allowed: false,
        retry_action: None,
        audit_ref: Some(text(plan, "id").chars().take(8).collect()),
        at: None,
        progress: None,
    });
    NyeProposalVm {
        plan_id: text(plan, "id").to_owned(),
        digest: text(plan, "digest").to_owned(),
        title: text(plan, "intent").to_owned(),
        capability,
        target,
        scope: None,
        fields,
        lines,
        consequences: Vec::new(),
        risk: risk_of(peak),
        auth: if requires_approval {
            NyeAuth::ConfirmationRequired
        } else {
            NyeAuth::Authorized
        },
        state,
        // O Core não guarda o pedido original (ADR-0301): não há de onde
        // preencher uma nova proposta. «Alterar» fica por contrato.
        edit_href: None,
        superseded: false,
        cites_external: false,
        expires_at: instant(plan, "approval_expires_at").map(|at| clock.hhmm(at)),
        execution,
    }
}

const RISKS: [&str; 5] = [
    "read_only",
    "low_impact",
    "material_mutation",
    "external_effect",
    "privileged",
];

fn risk_rank(r: &str) -> usize {
    RISKS
        .iter()
        .position(|x| *x == r)
        .unwrap_or(RISKS.len() - 1)
}

/// O texto de uma resposta em blocos seguros: parágrafos, e listas quando
/// todas as linhas de um bloco são itens. Nunca HTML: a vista escreve texto.
#[must_use]
pub fn blocks(content: &str) -> Vec<NyeBlock> {
    content
        .split("\n\n")
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .map(|p| {
            let lines: Vec<&str> = p.lines().map(str::trim).collect();
            let items: Option<Vec<String>> = lines
                .iter()
                .map(|l| {
                    l.strip_prefix("- ")
                        .or_else(|| l.strip_prefix("* "))
                        .map(str::to_owned)
                })
                .collect();
            match items {
                Some(items) if !items.is_empty() => NyeBlock::List(items),
                _ => NyeBlock::Para {
                    text: lines.join(" "),
                    cites: Vec::new(),
                },
            }
        })
        .collect()
}

fn message(id: String, role: NyeRole, at: String, turn: &Value) -> NyeMessageVm {
    let status = text(turn, "status");
    let degraded = role == NyeRole::Nye && !status.is_empty() && status != "COMPLETED";
    let (blocks_, state) = if degraded {
        // A resposta de sistema do Core diz porque não houve resposta; a Nye
        // mostra essa razão tipada, na língua do membro, e não o texto.
        (
            Vec::new(),
            NyeMsgState::Failed(reason_of(text(turn, "reason_code"))),
        )
    } else {
        (blocks(text(turn, "content")), NyeMsgState::Complete)
    };
    NyeMessageVm {
        id,
        role,
        at,
        blocks: blocks_,
        state,
        grounding: if role == NyeRole::Nye && !degraded {
            NyeGrounding::Ungrounded
        } else {
            NyeGrounding::NotApplicable
        },
        processing: None,
        sources: Vec::new(),
        steps: Vec::new(),
        hits: Vec::new(),
        proposals: Vec::new(),
        attachments: Vec::new(),
        stream_src: None,
        stop_action: None,
        retry_action: None,
        egress_action: None,
    }
}

/// NYE-02 · A lista de conversas do membro.
#[must_use]
pub(crate) fn conversations(
    list: &Value,
    current: Option<&str>,
    query: &str,
    clock: &Clock,
) -> Vec<NyeConvItemVm> {
    let needle = query.trim().to_lowercase();
    list.get("items")
        .or_else(|| list.get("conversations"))
        .and_then(Value::as_array)
        .map_or(&[][..], Vec::as_slice)
        .iter()
        .filter(|c| needle.is_empty() || text(c, "title").to_lowercase().contains(&needle))
        .map(|c| {
            let id = text(c, "id");
            NyeConvItemVm {
                title: text(c, "title").to_owned(),
                at: instant(c, "updated_at")
                    .map(|a| clock.hhmm(a))
                    .unwrap_or_default(),
                href: format!("/ai/prompt?c={id}"),
                active: current == Some(id),
            }
        })
        .collect()
}

/// NYE-02 · Uma conversa do membro, com os turnos.
#[must_use]
pub(crate) fn conversation(view: &Value, clock: &Clock) -> NyeConversationVm {
    let messages = view
        .get("turns")
        .and_then(Value::as_array)
        .map_or(&[][..], Vec::as_slice)
        .iter()
        .map(|turn| {
            let role = if text(turn, "role") == "member" {
                NyeRole::Member
            } else {
                NyeRole::Nye
            };
            let seq = turn.get("seq").and_then(Value::as_i64).unwrap_or(0);
            let at = instant(turn, "created_at")
                .map(|a| clock.hhmm(a))
                .unwrap_or_default();
            message(format!("t{seq}"), role, at, turn)
        })
        .collect();
    NyeConversationVm {
        title: text(view, "title").to_owned(),
        messages,
    }
}

/// O composer: pede ao Core pela conversa aberta; sem inferência fica
/// indisponível com a razão (a pesquisa continua na superfície).
#[must_use]
pub fn composer(current: Option<&str>, ask: NyeAvail) -> NyeComposerVm {
    NyeComposerVm {
        action: current.map_or_else(|| "/ai/prompt".to_owned(), |c| format!("/ai/prompt?c={c}")),
        text: String::new(),
        attachments: Vec::new(),
        state: match ask {
            NyeAvail::Available => NyeComposerState::Idle,
            NyeAvail::Unavailable(reason) => NyeComposerState::Unavailable(reason),
        },
        stop_action: None,
        attach_href: None,
    }
}

/// NYE-04 · A superfície universal. Fechada em todas as páginas da casca;
/// aberta pelo `/ask`. `Ctrl K` abre-a em todas as plataformas (`oc-shell.js`
/// aceita Ctrl e ⌘), e só o `runtime.js` pode perguntar pelo sistema.
#[must_use]
pub fn surface(
    availability: NyeAvailability,
    query: &str,
    intent: Option<NyeIntent>,
    open: bool,
) -> crate::ui::view_models::NyeSurfaceVm {
    crate::ui::view_models::NyeSurfaceVm {
        availability,
        intent,
        detected: (intent.is_none() && !query.trim().is_empty()).then(|| detected(query)),
        query: query.to_owned(),
        hits: Vec::new(),
        answer: None,
        context: None,
        continue_href: "/ai/prompt".to_owned(),
        shortcut: Some(crate::experience::shortcuts::NYE.keys.to_owned()),
        open,
    }
}

/// Uma mensagem da Nye que só traz propostas (o resultado de agir).
#[must_use]
pub fn proposals_message(proposals: Vec<NyeProposalVm>) -> NyeMessageVm {
    NyeMessageVm {
        id: "nye-act".to_owned(),
        role: NyeRole::Nye,
        at: String::new(),
        blocks: Vec::new(),
        state: NyeMsgState::Complete,
        grounding: NyeGrounding::NotApplicable,
        processing: None,
        sources: Vec::new(),
        steps: Vec::new(),
        hits: Vec::new(),
        proposals,
        attachments: Vec::new(),
        stream_src: None,
        stop_action: None,
        retry_action: None,
        egress_action: None,
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
            zone: ocinye_contracts::temporal::TimeZoneName::utc(),
            core_ok: true,
            is_admin: false,
        }
    }

    #[test]
    fn sem_inferencia_pesquisar_continua_e_a_razao_e_tipada() {
        let status = json!({"available": false, "providers": 0, "capabilities": []});
        let a = availability(true, true, Some(&status));
        assert_eq!(a.search, NyeAvail::Available);
        assert_eq!(a.ask, NyeAvail::Unavailable(NyeReason::NoInference));
        assert_eq!(a.act, NyeAvail::Unavailable(NyeReason::NoInference));
        assert_eq!(
            a.voice_input,
            NyeAvail::Unavailable(NyeReason::VoiceUnavailable)
        );
        assert_eq!(a.link, NyeLink::Connected);
        // Sem permissão, a razão é a permissão, não o hardware.
        let a = availability(false, true, Some(&status));
        assert_eq!(a.ask, NyeAvail::Unavailable(NyeReason::PermissionDenied));
        // Com um modelo geral disponível, perguntar e agir ficam disponíveis.
        let ok = json!({"available": true, "providers": 1, "capabilities": [{"capability": "GENERAL", "available": true}]});
        assert_eq!(availability(true, true, Some(&ok)).ask, NyeAvail::Available);
        // Sem Core, nada, e a ligação diz porquê.
        let down = availability(true, false, Some(&ok));
        assert_eq!(
            down.search,
            NyeAvail::Unavailable(NyeReason::CoreUnavailable)
        );
        assert_eq!(down.link, NyeLink::CoreUnavailable);
    }

    #[test]
    fn so_entram_resultados_com_rota_canonica() {
        let id = "8f14e45f-ceea-467a-9575-4a5b2f5c1d10";
        let s = vec![
            json!({"entity_type": "note", "entity_id": id, "title": "Ata", "excerpt": "x"}),
            json!({"entity_type": "document", "entity_id": id, "title": "Sem rota"}),
            json!({"entity_type": "note", "entity_id": "../x", "title": "Mau"}),
            json!({"entity_type": "project", "entity_id": id, "title": "Solander"}),
        ];
        let g = hits(&s);
        assert_eq!(g.len(), 2);
        assert_eq!(g[0].hits[0].href, format!("/notes/{id}"));
        assert_eq!(g[1].kind, NyeKind::Project);
        assert!(g.iter().all(|g| g
            .hits
            .iter()
            .all(|h| h.title != "Sem rota" && h.title != "Mau")));
    }

    #[test]
    fn o_risco_e_a_confirmacao_sao_os_do_core() {
        let plan = json!({
            "id": "11111111-2222-3333-4444-555555555555", "intent": "Criar uma nota",
            "state": "awaiting_approval", "digest": "abc",
            "steps": [{"summary": "Criar a Nota", "risk": "low_impact",
                       "request": {"capability": "knowledge.note.create", "input": {"title": "Ata"}}}]
        });
        let p = proposal(&plan, true, &relogio());
        assert_eq!(p.risk, NyeRisk::ReversibleWrite);
        assert_eq!(p.auth, NyeAuth::ConfirmationRequired);
        assert_eq!(p.state, NyeExecState::AwaitingConfirmation);
        assert_eq!(p.digest, "abc");
        assert_eq!(p.fields[0].label, "title");
        assert!(p.execution.is_none() && p.edit_href.is_none());
        // O Core diz que não precisa: a Nye não inventa uma confirmação.
        assert_eq!(proposal(&plan, false, &relogio()).auth, NyeAuth::Authorized);
        // Nenhum risco do Core é «Destrutivo»; um desconhecido é tratado como privilegiado.
        for r in RISKS {
            assert_ne!(risk_of(r), NyeRisk::Destructive);
            assert_ne!(risk_of(r), NyeRisk::Navigation);
        }
        assert_eq!(risk_of("?"), NyeRisk::Privileged);
    }

    #[test]
    fn uma_resposta_degradada_e_um_estado_e_nao_um_texto() {
        let conv = json!({"title": "T", "turns": [
            {"seq": 1, "role": "member", "content": "Olá"},
            {"seq": 2, "role": "system", "content": "Não há inferência disponível.", "status": "DEGRADED", "reason_code": "AI_NO_PROVIDER_AVAILABLE"}
        ]});
        let c = conversation(&conv, &relogio());
        assert_eq!(c.messages.len(), 2);
        assert!(c.messages[1].blocks.is_empty());
        assert_eq!(
            c.messages[1].state,
            NyeMsgState::Failed(NyeReason::NoInference)
        );
        assert_eq!(
            blocks("- a\n- b\n\nfim"),
            vec![
                NyeBlock::List(vec!["a".into(), "b".into()]),
                NyeBlock::Para {
                    text: "fim".into(),
                    cites: vec![]
                }
            ]
        );
    }
}
