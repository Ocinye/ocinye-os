//! A instalação de uma Instância nova, vista pelo Core (D011, ADR-0022).
//!
//! O Ocinye OS Installer fala com o servidor por um executor temporário
//! (`ocinye-bootstrap`), e esse executor só alcança o Core por subcomandos do
//! próprio binário, como o `bootstrap-admin`. Este módulo é o que esses
//! subcomandos chamam:
//!
//! - **`endpoint-seed`** — um ponto de acesso fixo numa Distribuição, criado na
//!   instalação, quando ainda não existe sessão de ninguém e por isso não pode
//!   existir um `Principal` que o crie pela Administração;
//! - **`verify-schema`**, **`verify-instance`**, **`verify-endpoints`**,
//!   **`verify-admin-bootstrap`** — quatro perguntas, só de leitura, que a
//!   verificação do produto faz ao Core.
//!
//! # Porque o `endpoint-seed` não é uma porta dos fundos
//!
//! Só serve a Instância que **esta** instalação criou (o `bootstrap-admin`
//! regista o `installation_id` na auditoria), e só enquanto nenhuma pessoa
//! abriu uma sessão. Depois disso recusa sempre: os pontos de acesso passam a
//! ser da Administração (D010), com autoridade e auditoria de quem administra.
//! As regras do ponto são as mesmas da D010 — o mesmo [`Hostname`], o canónico
//! nunca fixo (A001-M016), um nome serve um só destino, só Distribuições
//! activadas.

use ocinye_contracts::access_endpoint::Hostname;
use ocinye_contracts::Distribution;
use ocinye_observability::CorrelationIds;
use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;

use super::repository as repo;
use crate::audit::{self, action, AuditEntry};
use crate::error::CoreResult;

/// O actor de tudo o que a instalação escreve.
pub const INSTALLER_SUBJECT: &str = "system:installer";

/// A forma de um identificador de instalação: `inst-` e 16 hexadecimais
/// minúsculos. É gerado pelo Installer, e só serve para ligar a Instância à
/// instalação que a criou — não é um segredo nem dá autoridade.
#[must_use]
pub fn is_installation_id(value: &str) -> bool {
    value.strip_prefix("inst-").is_some_and(|hex| {
        hex.len() == 16
            && hex
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    })
}

/// Antes de o `bootstrap-admin` resolver a Instância: já existia uma?
///
/// # Errors
///
/// Database errors.
pub async fn instance_exists(pool: &PgPool) -> CoreResult<bool> {
    Ok(repo::recorded_instance(pool).await?.is_some())
}

/// Regista que a Instância nasceu desta instalação.
///
/// Só o `bootstrap-admin` o chama, e só quando foi ele a criar a Instância.
///
/// # Errors
///
/// Database errors.
pub async fn record_installation(
    pool: &PgPool,
    organisation_id: Uuid,
    installation_id: &str,
    ids: &CorrelationIds,
) -> CoreResult<()> {
    let mut tx = pool.begin().await?;
    audit::record(
        &mut tx,
        None,
        ids,
        AuditEntry::new(action::INSTANCE_INSTALLED, "organisation")
            .resource(organisation_id)
            .system_actor(INSTALLER_SUBJECT, organisation_id)
            .detail("installation_id", installation_id),
    )
    .await?;
    tx.commit().await?;
    Ok(())
}

/// As sete recusas do `endpoint-seed`, com os códigos do contrato do Installer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SeedRefusal {
    /// A Instância não foi criada por esta instalação (ou não existe).
    NotANewInstance,
    /// Alguém já abriu uma sessão: os pontos passaram a ser da Administração.
    SessionsExist,
    /// O nome não é um nome de anfitrião.
    HostInvalid,
    /// O nome é o do ponto canónico, que é genérico e nunca se fixa.
    HostIsCanonical,
    /// O nome já serve outro destino.
    HostTaken,
    /// A Distribuição existe mas não está activada nesta Instância.
    DistributionNotEnabled,
    /// A Distribuição não é uma das quatro.
    DistributionUnknown,
}

impl SeedRefusal {
    /// O código estável, tal como o Installer o lê.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::NotANewInstance => "NOT_A_NEW_INSTANCE",
            Self::SessionsExist => "SESSIONS_EXIST",
            Self::HostInvalid => "HOST_INVALID",
            Self::HostIsCanonical => "HOST_IS_CANONICAL",
            Self::HostTaken => "HOST_TAKEN",
            Self::DistributionNotEnabled => "DISTRIBUTION_NOT_ENABLED",
            Self::DistributionUnknown => "DISTRIBUTION_UNKNOWN",
        }
    }
}

/// O ponto, depois de semeado.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Seeded {
    /// `created` ou `unchanged`.
    pub result: &'static str,
    /// A identidade do ponto.
    pub endpoint_id: Uuid,
    /// O nome normalizado.
    pub host: String,
    /// A Distribuição em que está fixo.
    pub distribution: &'static str,
    /// Sempre `active`: a instalação verifica-o de fora depois.
    pub state: &'static str,
}

/// Semeia um ponto fixo numa Distribuição, numa Instância nova.
///
/// `canonical_url` é o `OCINYE_WORKSPACE_PUBLIC_URL` do Core: o canónico
/// semeia-se primeiro a partir dele (é a mesma semente do arranque do Core), e
/// só depois o ponto fixo — de outro modo o fixo chegava primeiro a uma tabela
/// vazia, e o canónico nunca nascia.
///
/// # Errors
///
/// `Ok(Err(refusal))` for each of the seven refusals; `Err` for database
/// errors and an unusable canonical URL.
pub async fn seed_bound_endpoint(
    pool: &PgPool,
    installation_id: &str,
    raw_host: &str,
    raw_distribution: &str,
    canonical_url: Option<&str>,
    ids: &CorrelationIds,
) -> CoreResult<Result<Seeded, SeedRefusal>> {
    let Ok(distribution) = raw_distribution.parse::<Distribution>() else {
        return Ok(Err(SeedRefusal::DistributionUnknown));
    };
    let Ok(host) = Hostname::parse(raw_host) else {
        return Ok(Err(SeedRefusal::HostInvalid));
    };
    let Some(organisation) = repo::recorded_instance(pool).await? else {
        return Ok(Err(SeedRefusal::NotANewInstance));
    };
    if !is_installation_id(installation_id)
        || !installed_by(pool, organisation.id, installation_id).await?
    {
        return Ok(Err(SeedRefusal::NotANewInstance));
    }
    if any_session(pool, organisation.id).await? {
        return Ok(Err(SeedRefusal::SessionsExist));
    }
    // O canónico primeiro (idempotente): é a mesma semente do arranque.
    if let Some(url) = canonical_url {
        super::endpoints::seed(pool, organisation.id, url, ids).await?;
    }

    let mut tx = pool.begin().await?;
    // A mesma tranca da semente: duas instalações em paralelo não semeiam o
    // mesmo nome duas vezes.
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended('ocinye.endpoint_seed', 0))")
        .execute(&mut *tx)
        .await?;
    let existing: Option<(Uuid, Uuid, bool, Option<String>, String)> = sqlx::query_as(
        "SELECT id, organisation_id, canonical, binding_distribution, state
           FROM access_endpoints WHERE hostname = $1 FOR UPDATE",
    )
    .bind(host.as_str())
    .fetch_optional(&mut *tx)
    .await?;
    if let Some((id, owner, canonical, binding, state)) = existing {
        if owner == organisation.id && canonical {
            return Ok(Err(SeedRefusal::HostIsCanonical));
        }
        if owner == organisation.id
            && binding.as_deref() == Some(distribution.as_str())
            && state == "active"
        {
            tx.commit().await?;
            return Ok(Ok(Seeded {
                result: "unchanged",
                endpoint_id: id,
                host: host.as_str().to_owned(),
                distribution: distribution.as_str(),
                state: "active",
            }));
        }
        return Ok(Err(SeedRefusal::HostTaken));
    }
    // FOR SHARE, como na Administração (A001-L012).
    let enabled: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM instance_distributions
                         WHERE organisation_id = $1 AND distribution = $2
                           AND state = 'enabled' FOR SHARE)",
    )
    .bind(organisation.id)
    .bind(distribution.as_str())
    .fetch_one(&mut *tx)
    .await?;
    if !enabled {
        return Ok(Err(SeedRefusal::DistributionNotEnabled));
    }
    let id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO access_endpoints
                (id, organisation_id, hostname, binding_distribution, state, canonical)
         VALUES ($1, $2, $3, $4, 'active', false)",
    )
    .bind(id)
    .bind(organisation.id)
    .bind(host.as_str())
    .bind(distribution.as_str())
    .execute(&mut *tx)
    .await?;
    audit::record(
        &mut tx,
        None,
        ids,
        AuditEntry::new(action::ACCESS_ENDPOINT_SEEDED, "access_endpoint")
            .resource(id)
            .system_actor(INSTALLER_SUBJECT, organisation.id)
            .detail("installation_id", installation_id)
            .detail("hostname", host.as_str())
            .detail("binding", distribution.as_str()),
    )
    .await?;
    tx.commit().await?;
    Ok(Ok(Seeded {
        result: "created",
        endpoint_id: id,
        host: host.as_str().to_owned(),
        distribution: distribution.as_str(),
        state: "active",
    }))
}

async fn installed_by(
    pool: &PgPool,
    organisation_id: Uuid,
    installation_id: &str,
) -> CoreResult<bool> {
    Ok(sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM audit_events
                         WHERE action = $1 AND resource_type = 'organisation'
                           AND resource_id = $2 AND organisation_id = $2
                           AND actor_subject = $3
                           AND metadata ->> 'installation_id' = $4)",
    )
    .bind(action::INSTANCE_INSTALLED)
    .bind(organisation_id)
    .bind(INSTALLER_SUBJECT)
    .bind(installation_id)
    .fetch_one(pool)
    .await?)
}

/// Uma sessão de qualquer pessoa da Instância, alguma vez — activa, expirada
/// ou revogada. As sessões não se apagam (só em cascata com a pessoa), e um
/// início de sessão fica também na auditoria: as duas fontes contam.
async fn any_session(pool: &PgPool, organisation_id: Uuid) -> CoreResult<bool> {
    Ok(sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM sessions s JOIN people p ON p.id = s.person_id
                         WHERE p.organisation_id = $1)
             OR EXISTS (SELECT 1 FROM audit_events
                         WHERE organisation_id = $1 AND action = $2)",
    )
    .bind(organisation_id)
    .bind(action::SIGN_IN)
    .fetch_one(pool)
    .await?)
}

// ── Verificações só de leitura ────────────────────────────────────────────

/// `verify-schema`: o que a base aplicou, comparado com o que este binário traz.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SchemaState {
    /// A última migração aplicada com sucesso, com quatro dígitos (`0064`).
    pub latest: String,
    /// Quantas foram aplicadas com sucesso.
    pub count: u32,
    /// Quantas este binário traz e a base ainda não tem.
    pub pending: u32,
}

/// Lê o estado do esquema **sem migrar**: verificar nunca escreve.
///
/// # Errors
///
/// Database errors (including a database with no migration table).
pub async fn verify_schema(pool: &PgPool) -> CoreResult<SchemaState> {
    let applied: Vec<i64> =
        sqlx::query_scalar("SELECT version FROM _sqlx_migrations WHERE success ORDER BY version")
            .fetch_all(pool)
            .await?;
    let known: Vec<i64> = crate::db::embedded_migration_versions();
    let pending = known.iter().filter(|v| !applied.contains(v)).count();
    let latest = applied.last().copied().unwrap_or(0);
    Ok(SchemaState {
        latest: format!("{latest:04}"),
        count: u32::try_from(applied.len()).unwrap_or(u32::MAX),
        pending: u32::try_from(pending).unwrap_or(u32::MAX),
    })
}

/// `verify-instance`: a Instância e as Distribuições activadas, tal como a
/// base as tem — e não tal como o plano as pediu.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct InstanceState {
    /// O nome que as pessoas lêem.
    pub name: String,
    /// A identidade técnica.
    pub slug: String,
    /// Activadas: primeiro a de nascimento, depois por ordem de activação.
    pub distributions: Vec<String>,
    /// O registo de aplicações, tal como esta Instância o vê (V07).
    pub applications: ApplicationsState,
}

/// O registo de aplicações numa Instância: quantas existem, quantas estão
/// activas, e se alguma essencial ficou inactiva (o que não pode acontecer).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct ApplicationsState {
    /// Registadas no código deste release.
    pub registered: u32,
    /// Activas nesta Instância.
    pub active: u32,
    /// Essenciais inactivas (sempre 0 numa Instância sã).
    pub essential_inactive: u32,
}

/// # Errors
///
/// Database errors; `Ok(None)` when there is no Instance.
pub async fn verify_instance(pool: &PgPool) -> CoreResult<Option<InstanceState>> {
    let Some(organisation) = repo::recorded_instance(pool).await? else {
        return Ok(None);
    };
    let distributions: Vec<String> = sqlx::query_scalar(
        "SELECT d.distribution FROM instance_distributions d
           JOIN organisations o ON o.id = d.organisation_id
          WHERE d.organisation_id = $1 AND d.state = 'enabled'
          ORDER BY (d.distribution = o.profile) DESC, d.enabled_at, d.distribution",
    )
    .bind(organisation.id)
    .fetch_all(pool)
    .await?;
    let apps = super::application_states(pool, organisation.id).await?;
    let count = |f: &dyn Fn(&super::ApplicationState) -> bool| {
        u32::try_from(apps.applications.iter().filter(|a| f(a)).count()).unwrap_or(u32::MAX)
    };
    Ok(Some(InstanceState {
        name: organisation.name,
        slug: organisation.slug,
        distributions,
        applications: ApplicationsState {
            registered: count(&|_| true),
            active: count(&|a| a.active),
            essential_inactive: count(&|a| {
                a.class == ocinye_contracts::ApplicationClass::Essential && !a.active
            }),
        },
    }))
}

/// `verify-endpoints`: um ponto, tal como a base o tem.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, sqlx::FromRow)]
pub struct EndpointState {
    /// O nome normalizado.
    pub host: String,
    /// O canónico (genérico).
    pub canonical: bool,
    /// A Distribuição em que está fixo, ou nenhuma.
    pub distribution: Option<String>,
    /// `active` · `disabled` · `unverified`.
    pub state: String,
}

/// # Errors
///
/// Database errors; an empty list when there is no Instance.
pub async fn verify_endpoints(pool: &PgPool) -> CoreResult<Vec<EndpointState>> {
    let Some(organisation) = repo::recorded_instance(pool).await? else {
        return Ok(Vec::new());
    };
    Ok(sqlx::query_as(
        "SELECT hostname AS host, canonical, binding_distribution AS distribution, state
           FROM access_endpoints WHERE organisation_id = $1
          ORDER BY canonical DESC, hostname",
    )
    .bind(organisation.id)
    .fetch_all(pool)
    .await?)
}

/// `verify-admin-bootstrap`: se o primeiro acesso ainda está por fazer. Nunca
/// a credencial — nem sequer a sua existência para além de um booleano.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct AdminBootstrapState {
    /// Existe uma identidade privilegiada na Instância.
    pub privileged_identity_exists: bool,
    /// A credencial temporária que o bootstrap emitiu continua por usar.
    pub temporary_credential_pending: bool,
}

/// # Errors
///
/// Database errors.
pub async fn verify_admin_bootstrap(pool: &PgPool) -> CoreResult<AdminBootstrapState> {
    let Some(organisation) = repo::recorded_instance(pool).await? else {
        return Ok(AdminBootstrapState {
            privileged_identity_exists: false,
            temporary_credential_pending: false,
        });
    };
    let (exists, pending): (bool, bool) = sqlx::query_as(
        "SELECT
            EXISTS (SELECT 1 FROM people
                     WHERE organisation_id = $1 AND identity_kind = 'privileged'),
            EXISTS (SELECT 1 FROM credentials c JOIN people p ON p.id = c.person_id
                     WHERE p.organisation_id = $1 AND p.identity_kind = 'privileged'
                       AND c.kind = 'temporary' AND c.state = 'active'
                       AND c.expires_at > now())",
    )
    .bind(organisation.id)
    .fetch_one(pool)
    .await?;
    Ok(AdminBootstrapState {
        privileged_identity_exists: exists,
        temporary_credential_pending: pending,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn um_identificador_de_instalacao_tem_uma_forma_so() {
        assert!(is_installation_id("inst-0123456789abcdef"));
        for mau in [
            "inst-0123456789ABCDEF",
            "inst-0123456789abcde",
            "inst-0123456789abcdef0",
            "0123456789abcdef",
            "inst-0123456789abcdeg",
            "inst-01234567'; drop",
            "",
        ] {
            assert!(!is_installation_id(mau), "{mau:?} passou");
        }
    }

    #[test]
    fn os_codigos_de_recusa_sao_os_do_contrato() {
        let todas = [
            SeedRefusal::NotANewInstance,
            SeedRefusal::SessionsExist,
            SeedRefusal::HostInvalid,
            SeedRefusal::HostIsCanonical,
            SeedRefusal::HostTaken,
            SeedRefusal::DistributionNotEnabled,
            SeedRefusal::DistributionUnknown,
        ];
        let codigos: std::collections::BTreeSet<_> = todas.iter().map(|r| r.code()).collect();
        assert_eq!(codigos.len(), 7);
        for c in codigos {
            assert!(
                c.bytes().all(|b| b.is_ascii_uppercase() || b == b'_'),
                "{c}"
            );
        }
    }
}
