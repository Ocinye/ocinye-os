//! A entrega de um lembrete.
//!
//! # Quatro factos, quatro camadas
//!
//! ```text
//! Reminder            a intenção: alguém quis ser lembrado
//!    ↓  chegou a hora
//! Worker              o mecanismo: reclama a linha, atomicamente
//!    ↓
//! ReminderDelivery    o facto por canal: foi entregue, e por onde
//!    ↓
//! Notification        o efeito visível: há algo para a pessoa ver
//! ```
//!
//! São quatro coisas diferentes e nenhuma substitui outra. `Delivered` no
//! lembrete **não** quer dizer que a pessoa leu: quer dizer que o sistema
//! cumpriu. Se um dia isso se confundir, o sino passa a mentir.

use chrono::{DateTime, Utc};
use sqlx::{PgExecutor, PgPool};
use uuid::Uuid;

use super::model::Reminder;
use crate::error::CoreResult;
use crate::Tx;

/// Quantas vezes se tenta antes de desistir.
///
/// Um lembrete que falha sempre deixa de ser tentado, como o `outbox` já faz.
/// Sem isto, um erro permanente fá-lo-ia voltar em cada passagem, para sempre.
const MAX_ATTEMPTS: i32 = 5;

/// O canal in-app. O único que existe hoje.
pub const IN_APP: &str = "in_app";

/// Reclama os lembretes que já passaram da hora.
///
/// # Porque `FOR UPDATE SKIP LOCKED`
///
/// Porque dois workers a correr ao mesmo tempo têm de poder trabalhar sem se
/// atrapalharem **e** sem entregarem o mesmo lembrete duas vezes. O `SKIP
/// LOCKED` faz o segundo worker saltar a linha que o primeiro tem trancada, em
/// vez de esperar por ela — trabalham em conjuntos disjuntos.
///
/// A tranca dura o que a transacção durar. Quem chama tem de fazer a entrega e
/// o `commit` dentro dela, senão a linha solta-se antes de o trabalho estar
/// feito.
///
/// O `SKIP LOCKED` sozinho não chega, e é por isso que `reminder_deliveries`
/// tem chave `(reminder_id, channel)`: se um dia esta consulta mudar e a
/// exclusão falhar, a base recusa a segunda entrega em vez de a criar.
///
/// # Errors
///
/// Devolve erro quando a consulta falha.
pub async fn claim_due(
    tx: &mut Tx<'_>,
    now: DateTime<Utc>,
    batch: i64,
) -> CoreResult<Vec<Reminder>> {
    let lembretes = sqlx::query_as::<_, Reminder>(
        "SELECT id, organisation_id, owner_id, event_id, task_id, note, trigger_at, state,
                attempts, created_at, updated_at
           FROM reminders
          WHERE state IN ('scheduled', 'snoozed')
            AND trigger_at <= $1
            AND attempts < $3
          ORDER BY trigger_at
          LIMIT $2
            FOR UPDATE SKIP LOCKED",
    )
    .bind(now)
    .bind(batch)
    .bind(MAX_ATTEMPTS)
    .fetch_all(&mut **tx)
    .await?;
    Ok(lembretes)
}

/// Entrega um lembrete pelo canal in-app.
///
/// Cria a notificação, regista a entrega e move o lembrete. As três coisas na
/// mesma transacção: uma notificação sem entrega registada voltaria a ser
/// criada na passagem seguinte, e um lembrete marcado como entregue sem
/// notificação seria um lembrete que ninguém recebeu.
///
/// # Errors
///
/// Devolve erro quando alguma das três escritas falha — incluindo quando a
/// chave `(reminder_id, channel)` já existe, que é a base a recusar uma segunda
/// entrega do mesmo canal.
pub async fn deliver_in_app(tx: &mut Tx<'_>, reminder: &Reminder) -> CoreResult<Uuid> {
    let titulo = reminder
        .note
        .as_deref()
        .filter(|nota| !nota.trim().is_empty())
        .unwrap_or("Lembrete");

    // O tipo e o identificador do recurso, e **não** o seu conteúdo. Quando a
    // pessoa abrir isto, o Core reautoriza o recurso nesse momento: uma
    // notificação não é uma cópia autorizada de nada.
    let (resource_type, resource_id) = match (reminder.event_id, reminder.task_id) {
        (Some(id), _) => (Some("calendar_event"), Some(id)),
        (_, Some(id)) => (Some("task"), Some(id)),
        _ => (None, None),
    };

    let notification_id: Uuid = sqlx::query_scalar(
        "INSERT INTO notifications
             (organisation_id, recipient_id, kind, title, resource_type, resource_id)
         VALUES ($1, $2, 'reminder', $3, $4, $5)
         RETURNING id",
    )
    .bind(reminder.organisation_id)
    .bind(reminder.owner_id)
    .bind(titulo)
    .bind(resource_type)
    .bind(resource_id)
    .fetch_one(&mut **tx)
    .await?;

    sqlx::query(
        "INSERT INTO reminder_deliveries (reminder_id, channel, notification_id)
         VALUES ($1, $2, $3)",
    )
    .bind(reminder.id)
    .bind(IN_APP)
    .bind(notification_id)
    .execute(&mut **tx)
    .await?;

    sqlx::query(
        "UPDATE reminders SET state = 'delivered', attempts = attempts + 1, updated_at = now()
          WHERE id = $1",
    )
    .bind(reminder.id)
    .execute(&mut **tx)
    .await?;

    Ok(notification_id)
}

/// Regista que uma tentativa falhou, sem dizer que foi entregue.
///
/// Corre na sua própria transacção, porque a que falhou vai ser desfeita — e o
/// contador tem de sobreviver a essa reversão, senão o lembrete é tentado para
/// sempre.
///
/// # Errors
///
/// Devolve erro quando a escrita falha.
pub async fn record_failure<'e>(
    executor: impl PgExecutor<'e>,
    reminder_id: Uuid,
) -> CoreResult<()> {
    sqlx::query("UPDATE reminders SET attempts = attempts + 1, updated_at = now() WHERE id = $1")
        .bind(reminder_id)
        .execute(executor)
        .await?;
    Ok(())
}

/// Quantos se reclamam de cada vez.
const BATCH: i64 = 50;

/// Entrega o que já passou da hora.
///
/// Devolve quantos foram entregues, para que o registo diga alguma coisa quando
/// não diz nada.
///
/// # A hora é a do Core
///
/// `Utc::now()` deste processo, e nunca um relógio de browser. Quem mexe no
/// relógio do seu computador não deve conseguir antecipar os lembretes de
/// ninguém.
///
/// # Errors
///
/// Devolve erro quando a base não responde. Um lembrete que falha a entrega
/// **não** faz a passagem falhar: fica contado, e a passagem seguinte volta a
/// tentar até ao limite.
pub async fn deliver_due(pool: &PgPool) -> CoreResult<usize> {
    let agora = chrono::Utc::now();

    // A reclamação e a entrega vivem na mesma transacção: a tranca do `SKIP
    // LOCKED` dura o que a transacção durar, e soltá-la antes de a entrega estar
    // escrita abriria a janela para um segundo worker reclamar o mesmo.
    let mut tx = pool.begin().await?;
    let pendentes = claim_due(&mut tx, agora, BATCH).await?;

    if pendentes.is_empty() {
        tx.rollback().await?;
        return Ok(0);
    }

    let mut entregues = 0_usize;
    let mut falhados = Vec::new();

    for lembrete in &pendentes {
        match deliver_in_app(&mut tx, lembrete).await {
            Ok(_) => entregues += 1,
            Err(error) => {
                tracing::warn!(
                    reminder_id = %lembrete.id,
                    error = %error,
                    "reminder delivery failed"
                );
                falhados.push(lembrete.id);
            }
        }
    }

    // Se algum falhou, a transacção inteira é suspeita: uma escrita recusada a
    // meio pode ter deixado a transacção em estado de erro, e continuar nela
    // escreveria por cima disso. Desfaz-se, e contam-se as tentativas fora —
    // senão o contador desaparecia com a reversão e o lembrete seria tentado
    // para sempre.
    if falhados.is_empty() {
        tx.commit().await?;
        Ok(entregues)
    } else {
        tx.rollback().await?;
        for id in falhados {
            if let Err(error) = record_failure(pool, id).await {
                tracing::error!(reminder_id = %id, error = %error, "could not record the attempt");
            }
        }
        Ok(0)
    }
}

/// Marca uma notificação como lida.
///
/// # Errors
///
/// Devolve erro quando a escrita falha.
pub async fn mark_read<'e>(
    executor: impl PgExecutor<'e>,
    recipient_id: Uuid,
    notification_id: Uuid,
) -> CoreResult<()> {
    // O destinatário entra na condição, e não só o identificador: marcar como
    // lida a notificação de outra pessoa não é grave, mas é uma escrita que não
    // lhe pertence.
    sqlx::query(
        "UPDATE notifications SET read_at = now()
          WHERE id = $1 AND recipient_id = $2 AND read_at IS NULL",
    )
    .bind(notification_id)
    .bind(recipient_id)
    .execute(executor)
    .await?;
    Ok(())
}
