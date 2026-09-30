//! D007.1 · As três aplicações que o registo ganhou: Monitor de Actividade,
//! Resultados e Lixo. O que o Core devolve, traduzido para os ViewModels do
//! Design; nada aqui decide.
//!
//! - **Monitor:** só os planos que os nós reportam (memória e disco em uso);
//!   CPU, rede e GPU dizem-se não reportados, nunca zero. A frescura vem do
//!   estado efectivo do nó que o Core calcula pelo último sinal — nenhum limiar
//!   inventado aqui. Sem inventário de serviços, sem «Parar».
//! - **Resultados:** estados e desfechos exactos do Core; cada ligação
//!   (ambiente, substituto, linhagem) relê-se com a autoridade de quem vê.
//! - **Lixo:** ficheiros e notas pessoais do próprio; sem prazo prometido; a
//!   eliminação definitiva não se oferece.

use serde_json::Value;

use crate::controllers::desktop::{bytes, instant, text, Clock};
use crate::controllers::research as rs;
use crate::i18n::{t, tf};
use crate::ui::view_models::{
    Freshness, MetricPlane, MonitorSampleVm, MonitorSummaryVm, NodeStatus, ResItemVm, ResultStatus,
    ResultValidationVm, TrashItemVm, TrashKind, ValidationKind, ValidationOutcome,
};

// ═════════════════════════════════════════════════════════════════════════
// Monitor
// ═════════════════════════════════════════════════════════════════════════

/// Os planos que o agente do nó reporta hoje (`memory_used_bytes`,
/// `storage_used_bytes` no heartbeat).
pub const REPORTED: [MetricPlane; 2] = [MetricPlane::Memory, MetricPlane::Storage];

/// Os que não têm utilização reportada: nomeiam-se, nunca aparecem a zero.
pub const UNREPORTED: [MetricPlane; 3] = [MetricPlane::Cpu, MetricPlane::Network, MetricPlane::Gpu];

/// O plano pedido, se é um dos reportados; senão o primeiro reportado.
#[must_use]
pub fn plane_of(v: Option<&str>) -> MetricPlane {
    REPORTED
        .into_iter()
        .find(|p| Some(p.id()) == v)
        .unwrap_or(MetricPlane::Memory)
}

/// O resumo de `GET /system/operations` (contagens; nenhum nome de membro).
#[must_use]
pub fn summary(ops: &Value) -> MonitorSummaryVm {
    let arr = |k: &str| {
        ops.get(k)
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default()
    };
    let n = |x: usize| u32::try_from(x).unwrap_or(u32::MAX);
    let nodes = arr("nodes");
    let providers = arr("ai_providers");
    let apps = arr("applications");
    let st = ops.get("storage").cloned().unwrap_or(Value::Null);
    let c = |k: &str| {
        st.get(k)
            .and_then(Value::as_u64)
            .map_or(0, |v| u32::try_from(v).unwrap_or(u32::MAX))
    };
    MonitorSummaryVm {
        nodes_online: n(nodes
            .iter()
            .filter(|x| text(x, "status") == "online")
            .count()),
        nodes_total: n(nodes.len()),
        providers_healthy: n(providers
            .iter()
            .filter(|p| {
                p.get("enabled").and_then(Value::as_bool) == Some(true)
                    && text(p, "health") == "healthy"
            })
            .count()),
        providers_total: n(providers.len()),
        apps_active: n(apps
            .iter()
            .filter(|a| a.get("active").and_then(Value::as_bool) == Some(true))
            .count()),
        apps_total: n(apps.len()),
        storage_warning: c("warning"),
        storage_critical: c("critical"),
        storage_over: c("over_quota"),
        personal_used: bytes(
            st.get("personal_used_bytes")
                .and_then(Value::as_i64)
                .and_then(|b| u64::try_from(b).ok())
                .unwrap_or(0),
        ),
    }
}

/// Uma leitura por nó, no plano pedido, a partir de `GET /compute/nodes`. O
/// nó é a fonte da métrica, com a ligação à sua ficha na Computação.
#[must_use]
pub(crate) fn samples(nodes: &[Value], plane: MetricPlane, clock: &Clock) -> Vec<MonitorSampleVm> {
    let line = match plane {
        MetricPlane::Storage => "storage_bytes",
        _ => "memory_bytes",
    };
    nodes
        .iter()
        .filter_map(|n| {
            let status = crate::controllers::ops::node_status(text(n, "status"))?;
            let l = n
                .get("capacity")
                .and_then(|c| c.get(line))
                .cloned()
                .unwrap_or(Value::Null);
            let physical = l.get("physical").and_then(Value::as_i64);
            let consumed = l.get("consumed").and_then(Value::as_i64);
            let b = |x: i64| bytes(u64::try_from(x).unwrap_or(0));
            // A frescura é a do Core: um nó que deixou de dar sinal lê-se
            // `offline` (o limiar é `node_offline_after`), e o último valor
            // mostra-se como antigo, nunca como actual.
            let freshness = match (consumed, status) {
                (None, _) => Freshness::Unreported,
                (Some(_), NodeStatus::Online) => Freshness::Live,
                (Some(_), _) => Freshness::Stale,
            };
            Some(MonitorSampleVm {
                source: text(n, "identifier").to_owned(),
                name: text(n, "display_name").to_owned(),
                status,
                used: consumed.map(b),
                total: physical.map(b),
                pct: match (physical, consumed) {
                    (Some(p), Some(c)) if p > 0 => u8::try_from((c.max(0) * 100 / p).min(100)).ok(),
                    _ => None,
                },
                seen: instant(n, "last_seen_at").map(|at| clock.relative(at)),
                freshness,
                compute_href: Some(format!("/compute/nodes/{}", text(n, "id"))),
            })
        })
        .collect()
}

// ═════════════════════════════════════════════════════════════════════════
// Resultados
// ═════════════════════════════════════════════════════════════════════════

/// O estado de um resultado (`ck_results_status`).
#[must_use]
pub fn result_status(v: &str) -> Option<ResultStatus> {
    match v {
        "draft" => Some(ResultStatus::Draft),
        "under_review" => Some(ResultStatus::UnderReview),
        "validated" => Some(ResultStatus::Validated),
        "superseded" => Some(ResultStatus::Superseded),
        "invalidated" => Some(ResultStatus::Invalidated),
        _ => None,
    }
}

/// Os estados, pela ordem do ciclo, para a navegação.
pub const STATES: [&str; 5] = [
    "draft",
    "under_review",
    "validated",
    "superseded",
    "invalidated",
];

/// As colunas da lista de resultados.
pub const RESULT_COLUMNS: [&str; 2] = ["results.col.workspace", "results.col.updated"];

/// Uma linha de resultado. `workspace` = o título do ambiente, já lido.
#[must_use]
pub(crate) fn result_item(
    r: &Value,
    workspace: Option<&str>,
    open: Option<&str>,
    clock: &Clock,
) -> Option<ResItemVm> {
    let id = text(r, "id");
    let status = result_status(text(r, "status"))?;
    Some(ResItemVm {
        title: text(r, "title").to_owned(),
        code: None,
        state: Some(crate::ui::apps::results::result_state(status)),
        cells: vec![
            workspace.map(str::to_owned),
            instant(r, "created_at").map(|at| rs::day(at, clock)),
        ],
        overdue: false,
        priority: None,
        href: format!("/results/{id}"),
        active: open == Some(id),
    })
}

/// Uma validação, com o desfecho real.
#[must_use]
pub(crate) fn validation(v: &Value, clock: &Clock) -> Option<ResultValidationVm> {
    Some(ResultValidationVm {
        kind: match text(v, "kind") {
            "validation" => ValidationKind::Validation,
            "reproduction" => ValidationKind::Reproduction,
            _ => return None,
        },
        outcome: match text(v, "outcome") {
            "confirmed" => ValidationOutcome::Confirmed,
            "contradicted" => ValidationOutcome::Contradicted,
            "inconclusive" => ValidationOutcome::Inconclusive,
            _ => return None,
        },
        // O Core não devolve quem registou a validação (RES-09).
        by: None,
        at: instant(v, "created_at")
            .map(|at| rs::day(at, clock))
            .unwrap_or_default(),
        note: rs::opt(v, "note"),
        execution: None,
    })
}

// ═════════════════════════════════════════════════════════════════════════
// Lixo
// ═════════════════════════════════════════════════════════════════════════

/// A secção do Lixo.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrashSection {
    /// Tudo.
    All,
    /// Ficheiros.
    Files,
    /// Notas.
    Notes,
}

/// A secção pelo valor de `?nav=`.
#[must_use]
pub fn section_of(v: Option<&str>) -> TrashSection {
    match v {
        Some("files") => TrashSection::Files,
        Some("notes") => TrashSection::Notes,
        _ => TrashSection::All,
    }
}

/// O valor de `?nav=` de uma secção.
#[must_use]
pub const fn section_id(s: TrashSection) -> Option<&'static str> {
    match s {
        TrashSection::All => None,
        TrashSection::Files => Some("files"),
        TrashSection::Notes => Some("notes"),
    }
}

/// O endereço de uma secção, com o item aberto.
#[must_use]
pub fn trash_href(s: TrashSection, open: Option<&str>) -> String {
    let mut q: Vec<String> = Vec::new();
    if let Some(n) = section_id(s) {
        q.push(format!("nav={n}"));
    }
    if let Some(o) = open {
        q.push(format!("open={}", rs::encode(o)));
    }
    if q.is_empty() {
        "/trash".to_owned()
    } else {
        format!("/trash?{}", q.join("&"))
    }
}

/// Um item do Lixo (`file:<id>` ou `note:<id>`), pelo instante em que foi
/// apagado — o que o Core diz, não uma estimativa.
#[must_use]
pub(crate) fn trash_item(kind: TrashKind, v: &Value, clock: &Clock) -> TrashItemVm {
    let id = text(v, "id").to_owned();
    let deleted = instant(v, "deleted_at").or_else(|| instant(v, "updated_at"));
    TrashItemVm {
        kind,
        name: match kind {
            TrashKind::File => text(v, "name").to_owned(),
            TrashKind::Note => {
                rs::opt(v, "title").unwrap_or_else(|| t("notes.untitled").to_owned())
            }
        },
        deleted: deleted
            .map(|at| format!("{} {}", rs::day(at, clock), clock.hhmm(at)))
            .unwrap_or_default(),
        // Só o dono apaga e só o dono vê o seu Lixo: quem apagou é o próprio.
        deleted_by: None,
        origin: Some(
            t(match kind {
                TrashKind::File => "nav.files",
                TrashKind::Note => "nav.notes",
            })
            .to_owned(),
        ),
        size: match kind {
            TrashKind::File => v
                .get("size_bytes")
                .and_then(Value::as_i64)
                .and_then(|b| u64::try_from(b).ok())
                .map(bytes),
            TrashKind::Note => None,
        },
        // Um ficheiro no Lixo continua a ocupar o armazenamento do membro.
        counts_storage: kind == TrashKind::File,
        restore_action: Some(match kind {
            TrashKind::File => "/trash/files/restore".to_owned(),
            TrashKind::Note => "/trash/notes/restore".to_owned(),
        }),
        id,
    }
}

/// O identificador composto de um item (`file:<id>`, `note:<id>`).
#[must_use]
pub fn trash_key(i: &TrashItemVm) -> String {
    format!(
        "{}:{}",
        match i.kind {
            TrashKind::File => "file",
            TrashKind::Note => "note",
        },
        i.id
    )
}

/// A linha de um item na lista.
#[must_use]
pub fn trash_row(i: &TrashItemVm, section: TrashSection, open: Option<&str>) -> ResItemVm {
    let key = trash_key(i);
    let (_, kind_key) = crate::ui::apps::trash::kind_meta(i.kind);
    ResItemVm {
        title: i.name.clone(),
        code: None,
        state: None,
        cells: vec![
            Some(t(kind_key).to_owned()),
            Some(i.deleted.clone()),
            i.origin.clone(),
        ],
        overdue: false,
        priority: None,
        href: trash_href(section, Some(&key)),
        active: open == Some(key.as_str()),
    }
}

/// As colunas da lista do Lixo.
pub const TRASH_COLUMNS: [&str; 3] = ["trash.col.kind", "trash.col.deleted", "trash.col.origin"];

/// A navegação do Lixo, com as contagens que o Core devolveu.
#[must_use]
pub fn trash_nav(
    active: TrashSection,
    files: usize,
    notes: usize,
) -> Vec<crate::ui::view_models::AppNavVm> {
    let n = |x: usize| Some(u32::try_from(x).unwrap_or(u32::MAX));
    [
        (TrashSection::All, "trash.nav.all", "trash", files + notes),
        (TrashSection::Files, "trash.nav.files", "files", files),
        (TrashSection::Notes, "trash.nav.notes", "notes", notes),
    ]
    .into_iter()
    .map(|(s, k, ic, c)| {
        let mut v = rs::nav(k, ic, trash_href(s, None), s == active);
        v.count = n(c);
        v
    })
    .collect()
}

/// O rótulo de uma execução de estudo («Execução n.º 3»).
#[must_use]
pub fn execution_label(v: &Value) -> String {
    tf(
        "prod.results.execution_n",
        &[(
            "n",
            v.get("sequence")
                .and_then(Value::as_i64)
                .map(|n| n.to_string())
                .unwrap_or_default()
                .as_str(),
        )],
    )
}
