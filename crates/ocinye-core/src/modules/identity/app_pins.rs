//! As aplicações que um membro fixou na barra lateral.
//!
//! Uma preferência de apresentação do próprio membro, resolvida pela sessão: não
//! altera autorização, e cada pessoa só lê e escreve as suas. O Core guarda uma
//! lista **ordenada** de identificadores técnicos de aplicação — opacos para ele,
//! definidos pelo registo do Workspace — e devolve-a tal como a recebeu.
//!
//! A ausência de linha e uma lista vazia são distintas: sem linha, o membro
//! nunca escolheu (o Workspace aplica o conjunto por omissão); vazia é a escolha
//! de não fixar nenhuma. Por isso [`list_app_pins`] devolve `Option`.

use ocinye_domain::Principal;
use sqlx::PgPool;

use crate::error::{CoreError, CoreResult};

/// Quantas aplicações um membro pode fixar. Um tecto generoso — o registo tem
/// duas dezenas —, o suficiente para travar uma escrita abusiva sem limitar o
/// uso real.
const MAX_PINS: usize = 40;

/// O comprimento máximo de um identificador de aplicação. Os do registo têm uma
/// dúzia de caracteres; isto trava um valor absurdo sem conhecer o catálogo.
const MAX_ID_LEN: usize = 64;

/// As aplicações fixadas por um membro, pela ordem da barra.
///
/// `None` quando o membro nunca escolheu — nesse caso o Workspace aplica o
/// conjunto por omissão. `Some(vec![])` quando escolheu não fixar nenhuma.
///
/// # Errors
///
/// Devolve erro quando a consulta falha.
pub async fn list_app_pins(
    pool: &PgPool,
    principal: &Principal,
) -> CoreResult<Option<Vec<String>>> {
    let linha: Option<(Vec<String>,)> =
        sqlx::query_as("SELECT pinned_app_ids FROM member_app_pins WHERE person_id = $1")
            .bind(principal.person_id)
            .fetch_optional(pool)
            .await?;
    Ok(linha.map(|(ids,)| ids))
}

/// Fixa exactamente esta lista, por esta ordem — substituindo a anterior.
///
/// A escrita é a escolha inteira: fixar, desafixar e reordenar são todos «passa a
/// ser esta a lista». Os identificadores são validados na forma (não vazios, sem
/// espaços, dentro do comprimento, sem repetições, até ao tecto) mas **não** no
/// significado: se são aplicações reais e fixáveis é o Workspace, dono do
/// registo, que decide antes de chamar. O Core guarda tokens.
///
/// # Errors
///
/// [`CoreError::Validation`] quando a lista excede o tecto, tem um identificador
/// mal formado, ou repete um.
pub async fn set_app_pins(pool: &PgPool, principal: &Principal, ids: &[String]) -> CoreResult<()> {
    if ids.len() > MAX_PINS {
        return Err(CoreError::Validation(format!(
            "Não é possível fixar mais de {MAX_PINS} aplicações."
        )));
    }
    let mut vistos = std::collections::BTreeSet::new();
    for id in ids {
        if id.is_empty()
            || id.len() > MAX_ID_LEN
            || !id
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        {
            return Err(CoreError::Validation(
                "Identificador de aplicação inválido.".to_owned(),
            ));
        }
        if !vistos.insert(id.as_str()) {
            return Err(CoreError::Validation(
                "Uma aplicação não pode ser fixada duas vezes.".to_owned(),
            ));
        }
    }

    sqlx::query(
        "INSERT INTO member_app_pins (person_id, pinned_app_ids, updated_at)
         VALUES ($1, $2, now())
         ON CONFLICT (person_id)
         DO UPDATE SET pinned_app_ids = EXCLUDED.pinned_app_ids, updated_at = now()",
    )
    .bind(principal.person_id)
    .bind(ids)
    .execute(pool)
    .await?;
    Ok(())
}
