//! A disposição do Desktop de um membro (Claude Design D001, FG-017).
//!
//! Uma preferência de apresentação do próprio membro, resolvida pela sessão,
//! como as aplicações fixadas: não altera autorização, e cada pessoa só lê e
//! escreve a sua. Sem linha, o membro segue a predefinição da distribuição, que
//! o Workspace desenha; repor apaga a linha.
//!
//! A escrita é optimista: quem grava diz a versão que leu, e uma versão que já
//! não é a actual é recusada com [`CoreError::Conflict`] — outra sessão gravou
//! entretanto, e sobrepor seria perder essa gravação em silêncio.

use ocinye_contracts::desktop::DesktopLayout;
use ocinye_domain::Principal;
use sqlx::PgPool;

use crate::error::{CoreError, CoreResult};

/// A disposição gravada: a versão e a escolha do membro.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredDesktop {
    /// A versão actual (concorrência optimista).
    pub version: i32,
    /// A disposição.
    pub layout: DesktopLayout,
}

/// A disposição do membro, ou `None` quando nunca personalizou.
///
/// # Errors
///
/// Devolve erro quando a consulta falha ou a linha não é uma disposição.
pub async fn get_desktop(
    pool: &PgPool,
    principal: &Principal,
) -> CoreResult<Option<StoredDesktop>> {
    let linha: Option<(i32, serde_json::Value)> =
        sqlx::query_as("SELECT version, layout FROM member_desktop_layouts WHERE person_id = $1")
            .bind(principal.person_id)
            .fetch_optional(pool)
            .await?;
    linha
        .map(|(version, layout)| {
            serde_json::from_value(layout)
                .map(|layout| StoredDesktop { version, layout })
                .map_err(|e| CoreError::Internal(format!("disposição do Desktop ilegível: {e}")))
        })
        .transpose()
}

/// Grava a disposição, se `expected_version` for a actual.
///
/// `expected_version` é `0` quando o membro nunca personalizou (segue a
/// predefinição). Devolve a nova versão.
///
/// # Errors
///
/// [`CoreError::Validation`] quando a disposição viola o registo;
/// [`CoreError::Conflict`] quando outra sessão gravou entretanto.
pub async fn put_desktop(
    pool: &PgPool,
    principal: &Principal,
    expected_version: i32,
    layout: &DesktopLayout,
    admin: bool,
) -> CoreResult<i32> {
    layout.validate(admin).map_err(CoreError::Validation)?;
    let json = serde_json::to_value(layout)
        .map_err(|e| CoreError::Internal(format!("disposição do Desktop: {e}")))?;

    let gravada: Option<(i32,)> = if expected_version == 0 {
        sqlx::query_as(
            "INSERT INTO member_desktop_layouts (person_id, version, layout)
             VALUES ($1, 1, $2)
             ON CONFLICT (person_id) DO NOTHING
             RETURNING version",
        )
        .bind(principal.person_id)
        .bind(&json)
        .fetch_optional(pool)
        .await?
    } else {
        sqlx::query_as(
            "UPDATE member_desktop_layouts
             SET layout = $3, version = version + 1, updated_at = now()
             WHERE person_id = $1 AND version = $2
             RETURNING version",
        )
        .bind(principal.person_id)
        .bind(expected_version)
        .bind(&json)
        .fetch_optional(pool)
        .await?
    };

    gravada.map(|(v,)| v).ok_or_else(|| {
        CoreError::Conflict(
            "O Desktop foi alterado noutra sessão. Recarregue para ver a versão actual.".to_owned(),
        )
    })
}

/// Repõe a predefinição: o membro deixa de ter disposição própria.
///
/// Muda só a disposição; nenhum dado de widget é tocado.
///
/// # Errors
///
/// Devolve erro quando a escrita falha.
pub async fn reset_desktop(pool: &PgPool, principal: &Principal) -> CoreResult<()> {
    sqlx::query("DELETE FROM member_desktop_layouts WHERE person_id = $1")
        .bind(principal.person_id)
        .execute(pool)
        .await?;
    Ok(())
}
