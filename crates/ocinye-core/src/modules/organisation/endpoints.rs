//! Pontos de acesso da Instância (ADR-0020).
//!
//! Um ponto de acesso é um nome de anfitrião configurado que leva a esta
//! Instância — genérico (a Distribuição resolve-se depois da entrada) ou fixo
//! numa Distribuição activada. **O anfitrião escolhe o destino; nunca concede
//! autoridade**, pertença, papel nem contexto.
//!
//! # Invariantes
//!
//! - um nome serve uma só Instância no servidor (índice único);
//! - exactamente um canónico por Instância, sempre activo;
//! - o canónico e o último activo não se desactivam (`endpoint_last_or_canonical`);
//! - um ponto fixo só numa Distribuição activada (chave estrangeira + Core).
//!
//! # O que o Core não faz
//!
//! Não gere DNS, não emite certificados, não escreve configuração do proxy
//! (isso é da instalação, D011). Só **observa** a resolução do nome e a ligação
//! segura, a pedido de quem administra (S29, S30).

use std::time::Duration;

use chrono::{DateTime, Utc};
use ocinye_contracts::access_endpoint::Hostname;
use ocinye_contracts::{Distribution, ErrorCode, Permission};
use ocinye_domain::policy::permissions::can;
use ocinye_domain::policy::{ResourceContext, ResourceKind};
use ocinye_domain::Principal;
use ocinye_observability::CorrelationIds;
use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;

use crate::audit::{self, action, AuditEntry};
use crate::error::{refusal, CoreError, CoreResult};

fn require(principal: &Principal, permission: Permission) -> CoreResult<()> {
    let ctx = ResourceContext::organisation(ResourceKind::Organisation, principal.organisation_id);
    if can(principal, permission, &ctx, None).allowed {
        Ok(())
    } else {
        Err(CoreError::PermissionDenied(
            "Não possui acesso à configuração da instância.".to_owned(),
        ))
    }
}

/// Um ponto de acesso, como a Administração o lê.
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct Endpoint {
    /// Identidade estável (a navegação entre pontos usa este id).
    pub id: Uuid,
    /// O nome normalizado.
    pub hostname: String,
    /// A Distribuição fixa, ou `None` (genérico).
    pub binding_distribution: Option<String>,
    /// `active` · `disabled` · `unverified`.
    pub state: String,
    /// O canónico da Instância.
    pub canonical: bool,
    /// `not_observed` · `resolves_here` · `resolves_elsewhere`.
    pub dns_observation: String,
    /// `not_observed` · `pending` · `valid` · `invalid`.
    pub tls_observation: String,
    /// Muda a cada alteração: uma sessão aberta antes de mudar o destino
    /// termina (S31, S40).
    pub updated_at: DateTime<Utc>,
}

const COLUMNS: &str = "id, hostname, binding_distribution, state, canonical, dns_observation, \
                       tls_observation, updated_at";

fn invalid(message: &str) -> CoreError {
    CoreError::Invariant {
        code: ErrorCode::ValidationError,
        reason: refusal::ENDPOINT_INVALID,
        message: message.to_owned(),
    }
}

/// Os pontos de acesso da Instância (S27).
///
/// # Errors
///
/// [`CoreError::PermissionDenied`] without `organisation.view`; database errors.
pub async fn list(pool: &PgPool, principal: &Principal) -> CoreResult<Vec<Endpoint>> {
    require(principal, Permission::OrganisationView)?;
    Ok(sqlx::query_as(&format!(
        "SELECT {COLUMNS} FROM access_endpoints WHERE organisation_id = $1
          ORDER BY canonical DESC, hostname"
    ))
    .bind(principal.organisation_id)
    .fetch_all(pool)
    .await?)
}

async fn one(tx: &mut crate::Tx<'_>, organisation_id: Uuid, id: Uuid) -> CoreResult<Endpoint> {
    sqlx::query_as(&format!(
        "SELECT {COLUMNS} FROM access_endpoints WHERE id = $1 AND organisation_id = $2 FOR UPDATE"
    ))
    .bind(id)
    .bind(organisation_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or_else(|| CoreError::NotFound("Ponto de acesso não encontrado.".to_owned()))
}

async fn require_enabled(
    tx: &mut crate::Tx<'_>,
    organisation_id: Uuid,
    binding: Option<Distribution>,
) -> CoreResult<()> {
    if let Some(d) = binding {
        // FOR SHARE: uma desactivação em simultâneo espera por esta escrita, e
        // não deixa um ponto fixo numa Distribuição desactivada (A001-L012).
        let enabled: bool = sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM instance_distributions
                             WHERE organisation_id = $1 AND distribution = $2
                               AND state = 'enabled' FOR SHARE)",
        )
        .bind(organisation_id)
        .bind(d.as_str())
        .fetch_one(&mut **tx)
        .await?;
        if !enabled {
            return Err(CoreError::Invariant {
                code: ErrorCode::Conflict,
                reason: refusal::DISTRIBUTION_NOT_ENABLED,
                message: "Um ponto fixo só se liga a uma Distribuição activada.".to_owned(),
            });
        }
    }
    Ok(())
}

async fn changed(
    tx: &mut crate::Tx<'_>,
    principal: Option<&Principal>,
    ids: &CorrelationIds,
    endpoint: &Endpoint,
    event: &str,
) -> CoreResult<()> {
    audit::record(
        tx,
        principal,
        ids,
        AuditEntry::new(action::ACCESS_ENDPOINT_CHANGED, "access_endpoint")
            .resource(endpoint.id)
            .detail("event", event)
            .detail("hostname", endpoint.hostname.as_str())
            .detail(
                "binding",
                endpoint
                    .binding_distribution
                    .as_deref()
                    .unwrap_or("generic"),
            )
            .detail("state", endpoint.state.as_str()),
    )
    .await
}

/// Acrescenta um ponto de acesso (S28). Nasce **por verificar**: só serve a
/// Instância depois de activado.
///
/// # Errors
///
/// [`CoreError::PermissionDenied`] without `organisation.manage`;
/// [`CoreError::Invariant`] `endpoint_invalid`, `endpoint_conflict`,
/// `distribution_not_enabled`; database errors.
pub async fn create(
    pool: &PgPool,
    principal: &Principal,
    raw_hostname: &str,
    binding: Option<Distribution>,
    ids: &CorrelationIds,
) -> CoreResult<Endpoint> {
    require(principal, Permission::OrganisationManage)?;
    let host = Hostname::parse(raw_hostname).map_err(|error| invalid(&format!("{error}.")))?;
    let mut tx = pool.begin().await?;
    require_enabled(&mut tx, principal.organisation_id, binding).await?;
    let taken: bool =
        sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM access_endpoints WHERE hostname = $1)")
            .bind(host.as_str())
            .fetch_one(&mut *tx)
            .await?;
    if taken {
        return Err(CoreError::Invariant {
            code: ErrorCode::Conflict,
            reason: refusal::ENDPOINT_CONFLICT,
            message: "Este endereço já é um ponto de acesso.".to_owned(),
        });
    }
    let endpoint: Endpoint = sqlx::query_as(&format!(
        "INSERT INTO access_endpoints
                (id, organisation_id, hostname, binding_distribution, state, created_by_id, updated_by_id)
         VALUES ($1, $2, $3, $4, 'unverified', $5, $5)
         RETURNING {COLUMNS}"
    ))
    .bind(Uuid::new_v4())
    .bind(principal.organisation_id)
    .bind(host.as_str())
    .bind(binding.map(Distribution::as_str))
    .bind(principal.person_id)
    .fetch_one(&mut *tx)
    .await?;
    changed(&mut tx, Some(principal), ids, &endpoint, "created").await?;
    tx.commit().await?;
    Ok(endpoint)
}

/// Põe um ponto a servir a Instância. As observações ficam à vista de quem
/// activa (S29/S30): o Core não finge que verificou o que não observou.
///
/// # Errors
///
/// [`CoreError::PermissionDenied`] without `organisation.manage`; database errors.
pub async fn activate(
    pool: &PgPool,
    principal: &Principal,
    id: Uuid,
    ids: &CorrelationIds,
) -> CoreResult<Endpoint> {
    require(principal, Permission::OrganisationManage)?;
    let mut tx = pool.begin().await?;
    let current = one(&mut tx, principal.organisation_id, id).await?;
    if current.state == "active" {
        tx.commit().await?;
        return Ok(current);
    }
    let endpoint: Endpoint = sqlx::query_as(&format!(
        "UPDATE access_endpoints SET state = 'active', updated_at = now(), updated_by_id = $2
          WHERE id = $1 RETURNING {COLUMNS}"
    ))
    .bind(id)
    .bind(principal.person_id)
    .fetch_one(&mut *tx)
    .await?;
    changed(&mut tx, Some(principal), ids, &endpoint, "activated").await?;
    tx.commit().await?;
    Ok(endpoint)
}

/// Muda o destino (S31): genérico ↔ fixo. As sessões abertas nesse anfitrião
/// terminam no pedido seguinte (a revisão muda).
///
/// # Errors
///
/// [`CoreError::PermissionDenied`] without `organisation.manage`;
/// [`CoreError::Invariant`] `distribution_not_enabled`; database errors.
pub async fn bind(
    pool: &PgPool,
    principal: &Principal,
    id: Uuid,
    binding: Option<Distribution>,
    ids: &CorrelationIds,
) -> CoreResult<Endpoint> {
    require(principal, Permission::OrganisationManage)?;
    let mut tx = pool.begin().await?;
    let current = one(&mut tx, principal.organisation_id, id).await?;
    // O canónico é o ponto genérico da Instância (ADR-0020 §10): fixá-lo numa
    // Distribuição — e depois desactivá-la — deixava quem administra sem
    // entrada nenhuma pelo Workspace (A001-M016).
    if current.canonical && binding.is_some() {
        return Err(CoreError::Invariant {
            code: ErrorCode::Conflict,
            reason: refusal::ENDPOINT_CANONICAL_GENERIC,
            message: "O ponto de acesso canónico é genérico e não se fixa numa Distribuição."
                .to_owned(),
        });
    }
    // O mesmo destino outra vez não muda nada: nem auditoria, nem revisão (a
    // revisão nova terminaria as sessões do ponto) (A001-L011).
    if current.binding_distribution.as_deref() == binding.map(Distribution::as_str) {
        tx.commit().await?;
        return Ok(current);
    }
    require_enabled(&mut tx, principal.organisation_id, binding).await?;
    let endpoint: Endpoint = sqlx::query_as(&format!(
        "UPDATE access_endpoints
            SET binding_distribution = $2, updated_at = now(), updated_by_id = $3
          WHERE id = $1 RETURNING {COLUMNS}"
    ))
    .bind(id)
    .bind(binding.map(Distribution::as_str))
    .bind(principal.person_id)
    .fetch_one(&mut *tx)
    .await?;
    changed(&mut tx, Some(principal), ids, &endpoint, "bound").await?;
    tx.commit().await?;
    Ok(endpoint)
}

/// Desactiva um ponto (S32). Recusa o canónico e o último activo — ficar sem
/// nenhum é ficar sem acesso à Instância. As sessões nele terminam (S40).
///
/// # Errors
///
/// [`CoreError::PermissionDenied`] without `organisation.manage`;
/// [`CoreError::Invariant`] `endpoint_last_or_canonical`; database errors.
pub async fn disable(
    pool: &PgPool,
    principal: &Principal,
    id: Uuid,
    ids: &CorrelationIds,
) -> CoreResult<Endpoint> {
    require(principal, Permission::OrganisationManage)?;
    let mut tx = pool.begin().await?;
    sqlx::query("SELECT 1 FROM access_endpoints WHERE organisation_id = $1 FOR UPDATE")
        .bind(principal.organisation_id)
        .execute(&mut *tx)
        .await?;
    let current = one(&mut tx, principal.organisation_id, id).await?;
    if current.state == "disabled" {
        tx.commit().await?;
        return Ok(current);
    }
    let other_active: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM access_endpoints
                         WHERE organisation_id = $1 AND id <> $2 AND state = 'active')",
    )
    .bind(principal.organisation_id)
    .bind(id)
    .fetch_one(&mut *tx)
    .await?;
    if current.canonical || (current.state == "active" && !other_active) {
        return Err(CoreError::Invariant {
            code: ErrorCode::Conflict,
            reason: refusal::ENDPOINT_LAST_OR_CANONICAL,
            message: "O ponto de acesso canónico, ou o último activo, não se desactiva.".to_owned(),
        });
    }
    let endpoint: Endpoint = sqlx::query_as(&format!(
        "UPDATE access_endpoints SET state = 'disabled', updated_at = now(), updated_by_id = $2
          WHERE id = $1 RETURNING {COLUMNS}"
    ))
    .bind(id)
    .bind(principal.person_id)
    .fetch_one(&mut *tx)
    .await?;
    changed(&mut tx, Some(principal), ids, &endpoint, "disabled").await?;
    tx.commit().await?;
    Ok(endpoint)
}

async fn addresses(host: &str) -> Option<Vec<std::net::IpAddr>> {
    let lookup = tokio::time::timeout(Duration::from_secs(3), tokio::net::lookup_host((host, 443)));
    match lookup.await {
        Ok(Ok(found)) => {
            let mut ips: Vec<_> = found.map(|a| a.ip()).collect();
            ips.sort();
            ips.dedup();
            Some(ips)
        }
        _ => None,
    }
}

/// Observa a resolução do nome e a ligação segura de um ponto (S29, S30).
/// Não muda o estado do ponto: só regista o que viu.
///
/// - DNS: o nome resolve para os mesmos endereços que o canónico → `resolves_here`;
///   para outros → `resolves_elsewhere`; não resolve → `not_observed`.
/// - TLS: um pedido `https://` ao nome com verificação normal de certificado:
///   aceite → `valid`; recusado por certificado → `invalid`; sem ligação → `pending`.
///
/// # Errors
///
/// [`CoreError::PermissionDenied`] without `organisation.manage`; database errors.
pub async fn observe(pool: &PgPool, principal: &Principal, id: Uuid) -> CoreResult<Endpoint> {
    require(principal, Permission::OrganisationManage)?;
    let endpoints = list(pool, principal).await?;
    let target = endpoints
        .iter()
        .find(|e| e.id == id)
        .ok_or_else(|| CoreError::NotFound("Ponto de acesso não encontrado.".to_owned()))?;
    let canonical = endpoints.iter().find(|e| e.canonical);
    let mine = addresses(&target.hostname).await;
    let reference = match canonical {
        Some(c) if c.id != target.id => addresses(&c.hostname).await,
        _ => mine.clone(),
    };
    let dns = match (&mine, &reference) {
        (Some(a), Some(b)) if !a.is_empty() && a.iter().any(|ip| b.contains(ip)) => "resolves_here",
        (Some(a), _) if !a.is_empty() => "resolves_elsewhere",
        _ => "not_observed",
    };
    let tls = if mine.as_ref().is_some_and(|a| !a.is_empty()) {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|e| CoreError::Internal(e.to_string()))?;
        match client
            .head(format!("https://{}/", target.hostname))
            .send()
            .await
        {
            Ok(_) => "valid",
            Err(error) if error.is_connect() && format!("{error:?}").contains("certificate") => {
                "invalid"
            }
            Err(error) if format!("{error:?}").to_lowercase().contains("certificate") => "invalid",
            Err(_) => "pending",
        }
    } else {
        "not_observed"
    };
    Ok(sqlx::query_as(&format!(
        "UPDATE access_endpoints SET dns_observation = $2, tls_observation = $3
          WHERE id = $1 RETURNING {COLUMNS}"
    ))
    .bind(id)
    .bind(dns)
    .bind(tls)
    .fetch_one(pool)
    .await?)
}

/// O que o Workspace precisa de saber de um anfitrião, antes de qualquer
/// sessão (pública: devolve só o destino e a identidade que a página de
/// entrada mostra, nunca membros nem as Distribuições acessíveis).
#[derive(Debug, Clone, Serialize)]
pub struct Resolved {
    /// O ponto.
    pub endpoint_id: Uuid,
    /// A Distribuição fixa, se o ponto é fixo.
    pub binding: Option<String>,
    /// `active` · `disabled` · `unverified`.
    pub state: String,
    /// Se a Distribuição fixa ainda está activada (S12).
    pub binding_enabled: bool,
    /// Revisão: muda quando o ponto muda (sessões abertas antes terminam).
    pub revision: String,
    /// O nome da Instância (mostrado antes da entrada, S07/S08).
    pub instance_name: String,
}

type ResolvedRow = (Uuid, Option<String>, String, bool, DateTime<Utc>, String);

/// Resolve um anfitrião **desta** Instância (a organização que este Core serve).
/// Um nome de outra Instância é desconhecido aqui. Nome desconhecido → `None`
/// (falha fechada, S13): nunca a primeira Instância, nunca adivinhar.
///
/// # Errors
///
/// Database errors.
pub async fn resolve(
    pool: &PgPool,
    organisation_id: Uuid,
    raw_hostname: &str,
) -> CoreResult<Option<Resolved>> {
    let Ok(host) = Hostname::parse(raw_hostname) else {
        return Ok(None);
    };
    let row: Option<ResolvedRow> = sqlx::query_as(
        "SELECT e.id, e.binding_distribution, e.state,
                COALESCE(d.state = 'enabled', true), e.updated_at, o.name
           FROM access_endpoints e
           JOIN organisations o ON o.id = e.organisation_id
           LEFT JOIN instance_distributions d
             ON d.organisation_id = e.organisation_id AND d.distribution = e.binding_distribution
          WHERE e.hostname = $1 AND e.organisation_id = $2",
    )
    .bind(host.as_str())
    .bind(organisation_id)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(
        |(endpoint_id, binding, state, binding_enabled, updated_at, instance_name)| Resolved {
            endpoint_id,
            binding,
            state,
            binding_enabled,
            revision: updated_at.timestamp_micros().to_string(),
            instance_name,
        },
    ))
}

/// O anfitrião de um ponto (para navegar entre pontos por id — nunca por um
/// URL vindo do pedido).
///
/// # Errors
///
/// Database errors.
pub async fn hostname_of(
    pool: &PgPool,
    organisation_id: Uuid,
    id: Uuid,
) -> CoreResult<Option<String>> {
    Ok(sqlx::query_scalar(
        "SELECT hostname FROM access_endpoints
          WHERE id = $1 AND organisation_id = $2 AND state = 'active'",
    )
    .bind(id)
    .bind(organisation_id)
    .fetch_optional(pool)
    .await?)
}

/// A semente (M4, ADR-0020 §10): com a tabela vazia, o ponto canónico genérico
/// nasce do anfitrião de `OCINYE_WORKSPACE_PUBLIC_URL`, auditado. Depois disso
/// a variável já não decide nada. Sem fonte, nada se cria — e o Workspace
/// falha fechado (S13) até alguém configurar um ponto.
///
/// # Errors
///
/// [`CoreError::Configuration`] for a public URL whose host is not a valid
/// host name; database errors.
pub async fn seed(
    pool: &PgPool,
    organisation_id: Uuid,
    public_url: &str,
    ids: &CorrelationIds,
) -> CoreResult<bool> {
    let host = public_url
        .split("://")
        .nth(1)
        .unwrap_or(public_url)
        .split(['/', '?', '#'])
        .next()
        .unwrap_or_default();
    let host = host.rsplit_once(':').map_or(host, |(h, port)| {
        if port.chars().all(|c| c.is_ascii_digit()) {
            h
        } else {
            host
        }
    });
    let host = Hostname::parse(host).map_err(|error| {
        CoreError::Configuration(format!(
            "OCINYE_WORKSPACE_PUBLIC_URL não dá um anfitrião válido para o ponto canónico: {error}"
        ))
    })?;
    let mut tx = pool.begin().await?;
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended('ocinye.endpoint_seed', 0))")
        .execute(&mut *tx)
        .await?;
    let any: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM access_endpoints WHERE organisation_id = $1)",
    )
    .bind(organisation_id)
    .fetch_one(&mut *tx)
    .await?;
    if any {
        tx.commit().await?;
        return Ok(false);
    }
    let endpoint: Endpoint = sqlx::query_as(&format!(
        "INSERT INTO access_endpoints (id, organisation_id, hostname, state, canonical)
         VALUES ($1, $2, $3, 'active', true)
         RETURNING {COLUMNS}"
    ))
    .bind(Uuid::new_v4())
    .bind(organisation_id)
    .bind(host.as_str())
    .fetch_one(&mut *tx)
    .await?;
    changed(&mut tx, None, ids, &endpoint, "seeded").await?;
    tx.commit().await?;
    Ok(true)
}
