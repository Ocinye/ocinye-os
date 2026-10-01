//! O contexto activo de uma sessão, dentro da Distribuição activa (ADR-0625).
//!
//! # O que é
//!
//! O âmbito em que o membro diz estar a trabalhar — a Organização, uma
//! Unidade de que é membro, um Projecto de que é membro, ou o seu Espaço
//! pessoal. Escolhe-se no Desktop (o chip de contexto), nunca na entrada, e é
//! reposto quando a Distribuição muda (nunca transportado).
//!
//! # O que não é
//!
//! Autorização. Um contexto não concede nada: cada operação continua a ser
//! autorizada pelo Core, com ou sem contexto. E não é uma «unidade activa»
//! global que decida por omissão onde as coisas nascem — nenhuma operação lê
//! hoje o contexto da sessão (CLAUDE.md §34.3, emendado pela ADR-0625).
//!
//! Só existem os contextos que o domínio tem: Equipa, Turma e Departamento não
//! existem, e não se fabricam.

use ocinye_domain::Principal;
use ocinye_observability::CorrelationIds;
use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;

use crate::audit::{self, action, AuditEntry};
use crate::error::{refusal, CoreError, CoreResult};

/// Um contexto que o membro pode usar.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Context {
    /// `organisation` · `unit` · `project` · `personal`.
    pub kind: &'static str,
    /// A unidade ou o projecto; `None` para a Organização e o Espaço pessoal.
    pub id: Option<Uuid>,
    /// Como se mostra (o código e o nome, ou o nome da Organização).
    pub name: String,
}

/// Os contextos do membro e o activo desta sessão.
#[derive(Debug, Clone, Serialize)]
pub struct MyContexts {
    /// Os que pode usar, pela ordem do chip.
    pub contexts: Vec<Context>,
    /// O activo, se a sessão escolheu um e ele ainda é seu.
    pub active: Option<Context>,
    /// A sessão tinha um contexto que deixou de ser do membro (S21): foi
    /// reposto, e o Workspace di-lo.
    pub revoked: Option<String>,
}

/// Os contextos que o domínio dá a este membro (S19). Sem nenhum para lá da
/// Organização e do Espaço pessoal, a lista diz só esses (S20 mostra-o).
///
/// # Errors
///
/// Database errors.
pub async fn available(pool: &PgPool, principal: &Principal) -> CoreResult<Vec<Context>> {
    let org: String = sqlx::query_scalar("SELECT name FROM organisations WHERE id = $1")
        .bind(principal.organisation_id)
        .fetch_one(pool)
        .await?;
    let units: Vec<(Uuid, String, String)> = sqlx::query_as(
        "SELECT u.id, u.code, u.name
           FROM unit_memberships m JOIN units u ON u.id = m.unit_id
          WHERE m.person_id = $1 AND m.revoked_at IS NULL
            AND u.organisation_id = $2 AND u.status = 'active'
          ORDER BY u.code",
    )
    .bind(principal.person_id)
    .bind(principal.organisation_id)
    .fetch_all(pool)
    .await?;
    let projects: Vec<(Uuid, String, String)> = sqlx::query_as(
        "SELECT w.id, w.code, w.title
           FROM workspace_memberships m JOIN research_workspaces w ON w.id = m.workspace_id
          WHERE m.person_id = $1 AND m.revoked_at IS NULL
            AND w.organisation_id = $2 AND w.kind = 'project' AND w.archived_at IS NULL
          ORDER BY w.code",
    )
    .bind(principal.person_id)
    .bind(principal.organisation_id)
    .fetch_all(pool)
    .await?;
    let mut out = vec![Context {
        kind: "organisation",
        id: None,
        name: org,
    }];
    out.extend(units.into_iter().map(|(id, code, name)| Context {
        kind: "unit",
        id: Some(id),
        name: format!("{code} · {name}"),
    }));
    out.extend(projects.into_iter().map(|(id, code, title)| Context {
        kind: "project",
        id: Some(id),
        name: format!("{code} · {title}"),
    }));
    out.push(Context {
        kind: "personal",
        id: None,
        name: String::new(),
    });
    Ok(out)
}

/// Os contextos e o activo; um activo que deixou de ser do membro é reposto
/// aqui mesmo (S21) — nunca se mostra o que já não é dele.
///
/// # Errors
///
/// Database errors.
pub async fn mine(
    pool: &PgPool,
    principal: &Principal,
    session_id: Uuid,
    stored: Option<&(String, Option<Uuid>)>,
) -> CoreResult<MyContexts> {
    let contexts = available(pool, principal).await?;
    let found = stored.and_then(|(kind, id)| {
        contexts
            .iter()
            .find(|c| c.kind == kind.as_str() && c.id == *id)
            .cloned()
    });
    let revoked = match (stored, &found) {
        (Some((kind, id)), None) => {
            clear(pool, session_id).await?;
            let name: Option<String> = match (kind.as_str(), id) {
                ("unit", Some(id)) => sqlx::query_scalar(
                    "SELECT code || ' · ' || name FROM units WHERE id = $1 AND organisation_id = $2",
                )
                .bind(id)
                .bind(principal.organisation_id)
                .fetch_optional(pool)
                .await?,
                ("project", Some(id)) => sqlx::query_scalar(
                    "SELECT code || ' · ' || title FROM research_workspaces
                      WHERE id = $1 AND organisation_id = $2",
                )
                .bind(id)
                .bind(principal.organisation_id)
                .fetch_optional(pool)
                .await?,
                _ => None,
            };
            Some(name.unwrap_or_default())
        }
        _ => None,
    };
    Ok(MyContexts {
        contexts,
        active: found,
        revoked,
    })
}

async fn clear(pool: &PgPool, session_id: Uuid) -> CoreResult<()> {
    sqlx::query(
        "UPDATE sessions SET active_context_kind = NULL, active_context_id = NULL WHERE id = $1",
    )
    .bind(session_id)
    .execute(pool)
    .await?;
    Ok(())
}

/// Escolhe o contexto activo (S19), ou repõe-o (`kind = None`). Só os que o
/// membro tem, e só dentro de uma Distribuição activa. Auditado.
///
/// # Errors
///
/// [`CoreError::Invariant`] `context_unavailable`; database errors.
pub async fn choose(
    pool: &PgPool,
    principal: &Principal,
    session_id: Uuid,
    active_distribution: Option<ocinye_contracts::Distribution>,
    kind: Option<&str>,
    id: Option<Uuid>,
    ids: &CorrelationIds,
) -> CoreResult<Option<Context>> {
    let unavailable = || CoreError::Invariant {
        code: ocinye_contracts::ErrorCode::PermissionDenied,
        reason: refusal::CONTEXT_UNAVAILABLE,
        message: "Este contexto não está disponível.".to_owned(),
    };
    let Some(kind) = kind else {
        clear(pool, session_id).await?;
        return Ok(None);
    };
    if active_distribution.is_none() {
        return Err(unavailable());
    }
    let chosen = available(pool, principal)
        .await?
        .into_iter()
        .find(|c| c.kind == kind && c.id == id)
        .ok_or_else(unavailable)?;
    let mut tx = pool.begin().await?;
    sqlx::query(
        "UPDATE sessions SET active_context_kind = $2, active_context_id = $3
          WHERE id = $1 AND person_id = $4",
    )
    .bind(session_id)
    .bind(chosen.kind)
    .bind(chosen.id)
    .bind(principal.person_id)
    .execute(&mut *tx)
    .await?;
    let mut entry = AuditEntry::new(action::CONTEXT_CHANGED, "session_context")
        .resource(principal.person_id)
        .detail("kind", chosen.kind);
    if let Some(id) = chosen.id {
        entry = entry.detail("context_id", id.to_string());
    }
    audit::record(&mut tx, Some(principal), ids, entry).await?;
    tx.commit().await?;
    Ok(Some(chosen))
}
