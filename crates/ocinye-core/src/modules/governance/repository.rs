//! Audit persistence, read side only.

use chrono::{DateTime, Utc};
use sqlx::PgExecutor;
use uuid::Uuid;

use super::model::AuditRecord;
use crate::error::CoreResult;

/// A leitura da auditoria, com **as duas camadas** de quem agiu.
///
/// # Porque há dois `LEFT JOIN` sobre a mesma tabela
///
/// Porque quem executou e quem responde podem ser identidades diferentes. Uma
/// operação administrativa é executada por uma identidade privilegiada — que é
/// o que a coluna regista, e está certo: foi ela que correu. Mas uma linha que
/// dissesse apenas «Fidel Admin» perde a pessoa por trás, e uma auditoria que
/// não sabe dizer quem responde não é uma auditoria.
///
/// O segundo `JOIN` resolve a ligação em vez de a duplicar na escrita. Gravar o
/// nome do dono em cada linha daria uma segunda cópia a divergir da primeira no
/// dia em que alguém mudar de nome — e a auditoria passaria a ter duas versões
/// do mesmo facto sem forma de escolher entre elas.
///
/// `NULL` quando o actor é uma pessoa comum, que é o caso normal: não há
/// ninguém por trás dela senão ela própria.
const SELECT: &str = "SELECT a.id, a.occurred_at, a.actor_person_id, p.full_name AS actor_name,
                             p.identity_kind AS actor_identity_kind,
                             dono.full_name AS actor_on_behalf_of,
                             a.action, a.resource_type, a.resource_id, a.unit_id, a.workspace_id,
                             a.classification, a.outcome, a.correlation_id, a.metadata
                        FROM audit_events a
                        LEFT JOIN people p ON p.id = a.actor_person_id
                        LEFT JOIN people dono ON dono.id = p.belongs_to_person_id";

/// List audit records matching the filters.
///
/// # Errors
///
/// Returns an error when the query fails.
#[expect(
    clippy::too_many_arguments,
    reason = "um parâmetro por coluna: a alternativa é uma struct que só existe para atravessar esta chamada"
)]
pub async fn list<'e>(
    executor: impl PgExecutor<'e>,
    organisation_id: Uuid,
    resource_type: Option<&str>,
    resource_id: Option<Uuid>,
    actor_person_id: Option<Uuid>,
    since: Option<DateTime<Utc>>,
    limit: i64,
    offset: i64,
) -> CoreResult<Vec<AuditRecord>> {
    let records = sqlx::query_as::<_, AuditRecord>(&format!(
        "{SELECT}
          WHERE a.organisation_id = $1
            AND ($2::text IS NULL OR a.resource_type = $2)
            AND ($3::uuid IS NULL OR a.resource_id = $3)
            AND ($4::uuid IS NULL OR a.actor_person_id = $4)
            AND ($5::timestamptz IS NULL OR a.occurred_at >= $5)
          ORDER BY a.occurred_at DESC
          LIMIT $6 OFFSET $7"
    ))
    .bind(organisation_id)
    .bind(resource_type)
    .bind(resource_id)
    .bind(actor_person_id)
    .bind(since)
    .bind(limit)
    .bind(offset)
    .fetch_all(executor)
    .await?;
    Ok(records)
}

/// Count audit records matching the filters.
///
/// # Errors
///
/// Returns an error when the query fails.
pub async fn count<'e>(
    executor: impl PgExecutor<'e>,
    organisation_id: Uuid,
    resource_type: Option<&str>,
    resource_id: Option<Uuid>,
    actor_person_id: Option<Uuid>,
    since: Option<DateTime<Utc>>,
) -> CoreResult<i64> {
    let total = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM audit_events
          WHERE organisation_id = $1
            AND ($2::text IS NULL OR resource_type = $2)
            AND ($3::uuid IS NULL OR resource_id = $3)
            AND ($4::uuid IS NULL OR actor_person_id = $4)
            AND ($5::timestamptz IS NULL OR occurred_at >= $5)",
    )
    .bind(organisation_id)
    .bind(resource_type)
    .bind(resource_id)
    .bind(actor_person_id)
    .bind(since)
    .fetch_one(executor)
    .await?;
    Ok(total)
}
