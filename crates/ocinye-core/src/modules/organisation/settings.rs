//! A configuração e a marca de uma Instância (Parte 14, ADR-0017).
//!
//! # O que distingue duas Instâncias do mesmo release
//!
//! O nome e o perfil já vivem na organização; as aplicações activas, em
//! `instance_applications`. Aqui ficam a língua por omissão, o fuso horário, as
//! aplicações fixadas por omissão e o logótipo. Tudo configuração: o mesmo
//! binário serve duas instituições diferentes sem um ramo de código.
//!
//! # O que a marca não pode fazer
//!
//! Mudar semântica de segurança ou esconder a plataforma. O logótipo é uma
//! imagem, validada pelos bytes e servida pelo Core a partir do armazenamento
//! governado — nunca um URL remoto nem CSS de terceiros —, e a marca pública
//! diz o nome da Instância e mais nada que um visitante não devesse saber.

use chrono::{DateTime, Utc};
use ocinye_contracts::application::MANIFESTS;
use ocinye_contracts::temporal::TimeZoneName;
use ocinye_contracts::Locale;
use ocinye_domain::policy::{authorize, Action, ResourceContext, ResourceKind};
use ocinye_domain::Principal;
use ocinye_observability::CorrelationIds;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::audit::{self, action, AuditEntry};
use crate::error::{CoreError, CoreResult};
use crate::storage::{sha256_hex, ObjectStore};

/// O maior logótipo aceite. Uma marca é um ícone, não uma fotografia.
pub const MAX_LOGO_BYTES: usize = 512 * 1024;

/// A configuração de uma Instância, como a administração a vê.
#[derive(Debug, Clone, Serialize)]
pub struct InstanceSettings {
    /// `pt`, `en` ou `fr`: a língua de quem ainda não escolheu.
    pub default_locale: String,
    /// Zona IANA da Instância.
    pub timezone: String,
    /// Aplicações fixadas por omissão; `None` é o conjunto do produto.
    pub default_pins: Option<Vec<String>>,
    /// Se há logótipo.
    pub has_logo: bool,
    /// Quando mudou pela última vez; `None` enquanto nunca mudou.
    pub updated_at: Option<DateTime<Utc>>,
}

/// O que a administração submete. Cada campo ausente fica como está.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct SettingsChange {
    /// Nova língua por omissão.
    pub default_locale: Option<String>,
    /// Novo fuso.
    pub timezone: Option<String>,
    /// Novas aplicações por omissão.
    pub default_pins: Option<Vec<String>>,
    /// Volta ao conjunto do produto; prevalece sobre `default_pins`.
    #[serde(default)]
    pub reset_default_pins: bool,
}

/// A marca pública: o que a página de entrada precisa antes de haver sessão.
#[derive(Debug, Clone, Serialize)]
pub struct InstanceBranding {
    /// O nome da Instância.
    pub name: String,
    /// A língua por omissão.
    pub default_locale: String,
    /// Se `GET /instance/logo` tem uma imagem.
    pub has_logo: bool,
    /// O produto — que continua identificável como a plataforma.
    pub product: &'static str,
    /// O perfil da Instância (`research`, `business`, `education`,
    /// `personal`). O início de sessão mostra-o à porta (D3): diz que
    /// aplicações a organização usa, e não é segredo de ninguém.
    pub profile: &'static str,
}

#[derive(sqlx::FromRow)]
struct Linha {
    default_locale: String,
    timezone: String,
    default_pins: Option<Vec<String>>,
    logo_object_id: Option<Uuid>,
    updated_at: DateTime<Utc>,
}

async fn linha(pool: &PgPool, organisation_id: Uuid) -> CoreResult<Option<Linha>> {
    Ok(sqlx::query_as::<_, Linha>(
        "SELECT default_locale, timezone, default_pins, logo_object_id, updated_at
           FROM instance_settings WHERE organisation_id = $1",
    )
    .bind(organisation_id)
    .fetch_optional(pool)
    .await?)
}

fn require(principal: &Principal) -> CoreResult<()> {
    let ctx = ResourceContext::organisation(ResourceKind::Platform, principal.organisation_id);
    authorize(principal, Action::Administer, &ctx)
        .map(|_| ())
        .map_err(|(denial, decision)| CoreError::from_denial(denial, &decision))
}

/// A configuração efectiva, com os valores do produto onde a Instância não
/// decidiu nada. Sem autorização: quem chama decide o que expõe.
///
/// # Errors
///
/// Quando a consulta falha.
pub async fn effective(pool: &PgPool, organisation_id: Uuid) -> CoreResult<InstanceSettings> {
    Ok(match linha(pool, organisation_id).await? {
        Some(l) => InstanceSettings {
            default_locale: l.default_locale,
            timezone: l.timezone,
            default_pins: l.default_pins,
            has_logo: l.logo_object_id.is_some(),
            updated_at: Some(l.updated_at),
        },
        None => InstanceSettings {
            default_locale: "pt".to_owned(),
            timezone: "UTC".to_owned(),
            default_pins: None,
            has_logo: false,
            updated_at: None,
        },
    })
}

/// A configuração, para a administração.
///
/// # Errors
///
/// [`CoreError::PermissionDenied`] sem administração da plataforma.
pub async fn get_settings(pool: &PgPool, principal: &Principal) -> CoreResult<InstanceSettings> {
    require(principal)?;
    effective(pool, principal.organisation_id).await
}

fn validar_pins(ids: &[String]) -> CoreResult<()> {
    let mut vistos = std::collections::BTreeSet::new();
    for id in ids {
        let fixavel = MANIFESTS
            .iter()
            .any(|m| m.id.as_str() == id.as_str() && m.can_pin);
        if !fixavel {
            return Err(CoreError::Validation(format!(
                "«{id}» não é uma aplicação que se possa fixar."
            )));
        }
        if !vistos.insert(id.as_str()) {
            return Err(CoreError::Validation(format!("«{id}» está repetida.")));
        }
    }
    Ok(())
}

/// Muda a configuração. Cada campo validado contra o que o produto conhece: a
/// língua contra os três locales, o fuso contra a base IANA, as aplicações
/// contra os manifestos.
///
/// # Errors
///
/// [`CoreError::PermissionDenied`] sem administração da plataforma;
/// [`CoreError::Validation`] para um valor que o produto não conhece.
pub async fn set_settings(
    pool: &PgPool,
    principal: &Principal,
    change: SettingsChange,
    ids: &CorrelationIds,
) -> CoreResult<InstanceSettings> {
    require(principal)?;
    let actual = effective(pool, principal.organisation_id).await?;

    let locale = match change.default_locale.as_deref() {
        Some(valor) => Locale::normalize(valor)
            .map(|l| l.as_str().to_owned())
            .ok_or_else(|| {
                CoreError::Validation("Língua desconhecida: pt, en ou fr.".to_owned())
            })?,
        None => actual.default_locale,
    };
    let timezone = match change.timezone.as_deref() {
        Some(valor) => {
            TimeZoneName::parse(valor).map_err(CoreError::Validation)?;
            valor.to_owned()
        }
        None => actual.timezone,
    };
    let pins = if change.reset_default_pins {
        None
    } else if let Some(lista) = change.default_pins {
        validar_pins(&lista)?;
        Some(lista)
    } else {
        actual.default_pins
    };

    let mut tx = pool.begin().await?;
    sqlx::query(
        "INSERT INTO instance_settings
             (organisation_id, default_locale, timezone, default_pins, updated_by_id)
         VALUES ($1, $2, $3, $4, $5)
         ON CONFLICT (organisation_id) DO UPDATE
             SET default_locale = EXCLUDED.default_locale, timezone = EXCLUDED.timezone,
                 default_pins = EXCLUDED.default_pins, updated_by_id = EXCLUDED.updated_by_id,
                 updated_at = now()",
    )
    .bind(principal.organisation_id)
    .bind(&locale)
    .bind(&timezone)
    .bind(pins.as_deref())
    .bind(principal.person_id)
    .execute(&mut *tx)
    .await?;
    audit::record(
        &mut tx,
        Some(principal),
        ids,
        AuditEntry::new(action::ADMIN_OPERATION, "instance_settings")
            .resource(principal.organisation_id)
            .detail("default_locale", locale.as_str())
            .detail("timezone", timezone.as_str()),
    )
    .await?;
    tx.commit().await?;
    effective(pool, principal.organisation_id).await
}

/// O tipo de uma imagem pelos seus bytes, e só para os três que se servem
/// inline em segurança. Um SVG é um documento com script: não entra.
fn tipo_de_imagem(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(&[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]) {
        Some("image/png")
    } else if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        Some("image/jpeg")
    } else if bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        Some("image/webp")
    } else {
        None
    }
}

/// Guarda o logótipo da Instância.
///
/// # Errors
///
/// [`CoreError::PermissionDenied`] sem administração da plataforma;
/// [`CoreError::Validation`] para uma imagem demasiado grande ou que não seja
/// PNG, JPEG ou WebP pelos bytes; os erros do armazenamento.
pub async fn set_logo(
    pool: &PgPool,
    store: &ObjectStore,
    principal: &Principal,
    bytes: Vec<u8>,
    ids: &CorrelationIds,
) -> CoreResult<InstanceSettings> {
    require(principal)?;
    if bytes.len() > MAX_LOGO_BYTES {
        return Err(CoreError::Validation(format!(
            "O logótipo tem {} bytes; o máximo é {MAX_LOGO_BYTES}.",
            bytes.len()
        )));
    }
    let tipo = tipo_de_imagem(&bytes).ok_or_else(|| {
        CoreError::Validation("O logótipo tem de ser PNG, JPEG ou WebP.".to_owned())
    })?;
    let object_id = Uuid::new_v4();
    let chave = format!("instance/logo/{object_id}");
    let soma = sha256_hex(&bytes);
    let tamanho = i64::try_from(bytes.len()).unwrap_or(i64::MAX);
    store.put(&chave, tipo, &soma, bytes).await?;

    let mut tx = pool.begin().await?;
    let gravado = sqlx::query(
        "INSERT INTO storage_objects
             (id, backend_id, organisation_id, object_key, original_filename, content_type,
              size_bytes, checksum_sha256, classification, status, created_by_id)
         SELECT $1, b.id, $2, $3, 'logo', $4, $5, $6, 'PUBLIC', 'stored', $7
           FROM storage_backends b WHERE b.is_default AND b.is_active",
    )
    .bind(object_id)
    .bind(principal.organisation_id)
    .bind(&chave)
    .bind(tipo)
    .bind(tamanho)
    .bind(&soma)
    .bind(principal.person_id)
    .execute(&mut *tx)
    .await?
    .rows_affected();
    if gravado == 0 {
        store.delete(&chave).await;
        return Err(CoreError::StorageUnavailable(
            "Não há armazenamento por omissão activo.".to_owned(),
        ));
    }
    sqlx::query(
        "INSERT INTO instance_settings (organisation_id, logo_object_id, updated_by_id)
         VALUES ($1, $2, $3)
         ON CONFLICT (organisation_id) DO UPDATE
             SET logo_object_id = EXCLUDED.logo_object_id,
                 updated_by_id = EXCLUDED.updated_by_id, updated_at = now()",
    )
    .bind(principal.organisation_id)
    .bind(object_id)
    .bind(principal.person_id)
    .execute(&mut *tx)
    .await?;
    audit::record(
        &mut tx,
        Some(principal),
        ids,
        AuditEntry::new(action::ADMIN_OPERATION, "instance_settings")
            .resource(principal.organisation_id)
            .detail("event", "logo_set")
            .detail("content_type", tipo),
    )
    .await?;
    tx.commit().await?;
    effective(pool, principal.organisation_id).await
}

/// O logótipo, para servir: o tipo e os bytes. `None` quando não há.
///
/// # Errors
///
/// Os erros da consulta e do armazenamento.
pub async fn logo(
    pool: &PgPool,
    store: &ObjectStore,
    organisation_id: Uuid,
) -> CoreResult<Option<(String, Vec<u8>)>> {
    let objecto: Option<(String, String)> = sqlx::query_as(
        "SELECT o.object_key, o.content_type
           FROM instance_settings s JOIN storage_objects o ON o.id = s.logo_object_id
          WHERE s.organisation_id = $1 AND o.status = 'stored'",
    )
    .bind(organisation_id)
    .fetch_optional(pool)
    .await?;
    match objecto {
        Some((chave, tipo)) => Ok(Some((tipo, store.get(&chave).await?))),
        None => Ok(None),
    }
}

/// A marca pública da Instância.
///
/// # Errors
///
/// Quando a consulta falha.
pub async fn branding(pool: &PgPool, organisation_id: Uuid) -> CoreResult<InstanceBranding> {
    let definicoes = effective(pool, organisation_id).await?;
    Ok(InstanceBranding {
        name: super::instance_name(pool, organisation_id).await?,
        default_locale: definicoes.default_locale,
        has_logo: definicoes.has_logo,
        product: "Ocinye OS",
        profile: super::applications::profile_of(pool, organisation_id)
            .await?
            .as_str(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn so_tres_imagens_e_pelos_bytes() {
        assert_eq!(
            tipo_de_imagem(&[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a, 0]),
            Some("image/png")
        );
        assert_eq!(
            tipo_de_imagem(&[0xff, 0xd8, 0xff, 0xe0]),
            Some("image/jpeg")
        );
        assert_eq!(tipo_de_imagem(b"RIFF\0\0\0\0WEBPVP8 "), Some("image/webp"));
        assert_eq!(tipo_de_imagem(b"<svg onload='x'>"), None);
        assert_eq!(tipo_de_imagem(b"GIF89a"), None);
    }

    #[test]
    fn so_se_fixam_aplicacoes_fixaveis_e_sem_repetir() {
        assert!(validar_pins(&["files".to_owned(), "notes".to_owned()]).is_ok());
        assert!(validar_pins(&["nao-existe".to_owned()]).is_err());
        assert!(validar_pins(&["files".to_owned(), "files".to_owned()]).is_err());
    }
}
