//! As consultas do calendário.
//!
//! # Duas coisas vivem aqui, e só aqui
//!
//! **A sobreposição temporal** e **quem vê o quê**. Cinco superfícies vão
//! perguntar pela agenda — Centro Temporal, Hoje, Semana, Mês e Agenda — e se
//! cada uma escrevesse a sua condição, mais cedo ou mais tarde duas delas
//! discordariam sobre o que existe. O bug não apareceria como erro: apareceria
//! como um evento que está numa vista e não está na outra.

use chrono::{DateTime, Utc};
use ocinye_domain::policy::VisibilityFilter;
use sqlx::PgExecutor;
use uuid::Uuid;

use super::model::{CalendarEvent, Notification, Reminder, TaskDeadline};
use crate::error::CoreResult;
use crate::visibility::{contained_in_visible_workspace, to_sql, VisibilityColumns};

/// O alias da tabela de eventos, um só, para que o predicado e a consulta não
/// possam discordar sobre a que tabela se referem.
const EVENTS: &str = "e";

const EVENT_COLUMNS: &str = "id, organisation_id, scope, owner_id, unit_id, workspace_id, title, \
                             description, location, all_day, starts_at, ends_at, timezone, \
                             starts_on, ends_before, state, classification, created_by_id, \
                             updated_by_id, created_at, updated_at";

const REMINDER_COLUMNS: &str = "id, organisation_id, owner_id, event_id, task_id, note, \
                                trigger_at, state, attempts, created_at, updated_at";

/// Um intervalo pedido, meio-aberto.
///
/// `[start, end)` como tudo o resto no calendário: o fim de um período é o
/// princípio do seguinte, sem lacuna nem sobreposição. Uma semana que acabasse
/// no domingo inclusive faria o evento da meia-noite de domingo aparecer em duas
/// semanas.
#[derive(Debug, Clone, Copy)]
pub struct TimeRange {
    /// Início, inclusive.
    pub start: DateTime<Utc>,
    /// Fim, exclusivo.
    pub end: DateTime<Utc>,
}

/// Que um item intersecta o intervalo pedido.
///
/// # A fórmula, escrita uma vez
///
/// Dois intervalos meio-abertos intersectam-se quando cada um começa antes de o
/// outro acabar:
///
/// ```text
/// começa_antes_do_fim  AND  acaba_depois_do_início
/// ```
///
/// É o mesmo para as duas formas de evento — o que muda são as colunas, não a
/// pergunta. Escrevê-la aqui é o que impede Hoje e Semana de discordarem sobre
/// um evento que começa à meia-noite.
///
/// O dia inteiro compara-se em **datas civis**, e não convertendo a data a um
/// instante: converter faria um prazo de 24 de Agosto cair a 23 para quem está a
/// leste, que é precisamente o que o modelo evita ao guardar a data.
fn intersects() -> String {
    let alias = EVENTS;
    format!(
        "(({alias}.all_day = FALSE AND {alias}.starts_at < $2 AND {alias}.ends_at > $1) \
         OR ({alias}.all_day = TRUE AND {alias}.starts_on < ($2 AT TIME ZONE 'UTC')::date \
             AND {alias}.ends_before > ($1 AT TIME ZONE 'UTC')::date))"
    )
}

/// Que eventos este actor pode ler.
///
/// # As quatro cláusulas, e porque não são uma
///
/// - **`personal`** — alcança-se por ser de quem é, e por mais nada. Nenhum
///   papel técnico entra nesta cláusula: um administrador de plataforma não lê
///   a agenda pessoal de ninguém, e é deliberado. O dia em que existir um
///   acesso explícito e deliberado, ele entra aqui — não por acidente de
///   privilégio.
/// - **`unit`** — a classificação do evento tem de passar. Um evento `INTERNAL`
///   de uma unidade é legível por qualquer membro activo, que é a mesma regra
///   que já governa datasets e referências (ADR-0100).
/// - **`research_workspace`** — **artefacto ∩ contentor**. O evento tem de ser
///   legível *e* o workspace que o contém também: um evento legível dentro de um
///   workspace inalcançável revelaria que há trabalho onde o actor não entra.
/// - **`institution`** — a classificação decide, como em qualquer recurso
///   institucional.
fn visible(filter: &VisibilityFilter, actor: Uuid) -> String {
    let alias = EVENTS;
    if filter.deny_all {
        return "(FALSE)".to_owned();
    }

    let artefacto = to_sql(
        filter,
        VisibilityColumns::aliased("e.unit_id", "e.workspace_id", "e.classification"),
    );
    let contido = contained_in_visible_workspace(filter, alias);

    format!(
        "(({alias}.scope = 'personal' AND {alias}.owner_id = '{actor}') \
          OR ({alias}.scope = 'unit' AND {artefacto}) \
          OR ({alias}.scope = 'research_workspace' AND {artefacto} AND {contido}) \
          OR ({alias}.scope = 'institution' AND {artefacto}))"
    )
}

/// Os eventos que este actor pode ver e que caem no intervalo.
///
/// # Errors
///
/// Devolve erro quando a consulta falha.
pub async fn events_in_range<'e>(
    executor: impl PgExecutor<'e>,
    organisation_id: Uuid,
    filter: &VisibilityFilter,
    actor: Uuid,
    range: TimeRange,
    limit: i64,
) -> CoreResult<Vec<CalendarEvent>> {
    let predicado = visible(filter, actor);
    let intersecta = intersects();
    let eventos = sqlx::query_as::<_, CalendarEvent>(&format!(
        "SELECT {EVENT_COLUMNS} FROM calendar_events e
          WHERE e.organisation_id = $3
            AND {intersecta}
            AND {predicado}
          ORDER BY COALESCE(e.starts_at, (e.starts_on::timestamptz)), e.title
          LIMIT $4"
    ))
    .bind(range.start)
    .bind(range.end)
    .bind(organisation_id)
    .bind(limit)
    .fetch_all(executor)
    .await?;
    Ok(eventos)
}

/// Quantos eventos o actor pode ver no intervalo.
///
/// Usa **exactamente** o mesmo predicado da listagem. Já aconteceu neste
/// repositório uma contagem e uma lista discordarem por terem condições escritas
/// à parte, e o resultado é uma paginação que promete páginas que não existem.
///
/// # Errors
///
/// Devolve erro quando a consulta falha.
pub async fn count_events_in_range<'e>(
    executor: impl PgExecutor<'e>,
    organisation_id: Uuid,
    filter: &VisibilityFilter,
    actor: Uuid,
    range: TimeRange,
) -> CoreResult<i64> {
    let predicado = visible(filter, actor);
    let intersecta = intersects();
    let total = sqlx::query_scalar::<_, i64>(&format!(
        "SELECT COUNT(*) FROM calendar_events e
          WHERE e.organisation_id = $3 AND {intersecta} AND {predicado}"
    ))
    .bind(range.start)
    .bind(range.end)
    .bind(organisation_id)
    .fetch_one(executor)
    .await?;
    Ok(total)
}

/// Um evento, se este actor o puder ler.
///
/// # Porque a autorização está na consulta
///
/// Porque um identificador não concede autoridade. Ler primeiro e decidir depois
/// funciona até alguém esquecer o «depois»; assim, um evento que o actor não
/// alcança e um evento que não existe dão a mesma resposta — que é a única que
/// não revela nada.
///
/// # Errors
///
/// Devolve erro quando a consulta falha.
pub async fn find_event<'e>(
    executor: impl PgExecutor<'e>,
    organisation_id: Uuid,
    filter: &VisibilityFilter,
    actor: Uuid,
    event_id: Uuid,
) -> CoreResult<Option<CalendarEvent>> {
    let predicado = visible(filter, actor);
    let evento = sqlx::query_as::<_, CalendarEvent>(&format!(
        "SELECT {EVENT_COLUMNS} FROM calendar_events e
          WHERE e.organisation_id = $1 AND e.id = $2 AND {predicado}"
    ))
    .bind(organisation_id)
    .bind(event_id)
    .fetch_optional(executor)
    .await?;
    Ok(evento)
}

/// Escreve um evento.
///
/// # Errors
///
/// Devolve erro quando a inserção falha — incluindo quando viola uma das
/// restrições de coerência da tabela, que é a última linha de defesa contra um
/// evento a meio caminho entre as duas formas.
#[expect(clippy::too_many_arguments, reason = "é a forma de uma linha")]
pub async fn insert_event<'e>(
    executor: impl PgExecutor<'e>,
    organisation_id: Uuid,
    scope: &str,
    owner_id: Option<Uuid>,
    unit_id: Option<Uuid>,
    workspace_id: Option<Uuid>,
    title: &str,
    description: Option<&str>,
    location: Option<&str>,
    occurrence: &ocinye_contracts::temporal::Occurrence,
    classification: ocinye_contracts::Classification,
    created_by: Uuid,
) -> CoreResult<CalendarEvent> {
    use ocinye_contracts::temporal::Occurrence;

    let (all_day, starts_at, ends_at, timezone, starts_on, ends_before) = match occurrence {
        Occurrence::Timed {
            starts_at,
            ends_at,
            timezone,
        } => (
            false,
            Some(*starts_at),
            Some(*ends_at),
            Some(timezone.as_str().to_owned()),
            None,
            None,
        ),
        Occurrence::AllDay {
            starts_on,
            ends_before,
        } => (true, None, None, None, Some(*starts_on), Some(*ends_before)),
    };

    let evento = sqlx::query_as::<_, CalendarEvent>(&format!(
        "INSERT INTO calendar_events
             (organisation_id, scope, owner_id, unit_id, workspace_id, title, description,
              location, all_day, starts_at, ends_at, timezone, starts_on, ends_before,
              classification, created_by_id, updated_by_id)
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$16)
         RETURNING {EVENT_COLUMNS}"
    ))
    .bind(organisation_id)
    .bind(scope)
    .bind(owner_id)
    .bind(unit_id)
    .bind(workspace_id)
    .bind(title)
    .bind(description)
    .bind(location)
    .bind(all_day)
    .bind(starts_at)
    .bind(ends_at)
    .bind(timezone)
    .bind(starts_on)
    .bind(ends_before)
    .bind(classification.as_str())
    .bind(created_by)
    .fetch_one(executor)
    .await?;
    Ok(evento)
}

/// Altera os campos que um evento deixa alterar.
///
/// # Nenhum campo de autoridade entra aqui
///
/// Nem organização, nem âmbito, nem dono, nem contentor, nem classificação. O
/// `UPDATE` não os menciona — não por serem verificados antes, mas por não
/// haver forma de os exprimir. Uma consulta que não escreve uma coluna não pode
/// ser levada a escrevê-la.
///
/// `COALESCE` mantém o que não foi pedido. A descrição e a localização usam um
/// sinalizador próprio porque para elas «não mexer» e «apagar» são coisas
/// diferentes, e `NULL` sozinho não sabe distinguir as duas.
///
/// # Errors
///
/// Devolve erro quando a actualização falha, incluindo por violar as restrições
/// de coerência temporal da tabela.
pub async fn update_event<'e>(
    executor: impl PgExecutor<'e>,
    event_id: Uuid,
    title: Option<&str>,
    description: Option<Option<&str>>,
    location: Option<Option<&str>>,
    occurrence: Option<&ocinye_contracts::temporal::Occurrence>,
    by: Uuid,
) -> CoreResult<CalendarEvent> {
    use ocinye_contracts::temporal::Occurrence;

    let (muda_quando, all_day, starts_at, ends_at, timezone, starts_on, ends_before) =
        match occurrence {
            None => (false, false, None, None, None, None, None),
            Some(Occurrence::Timed {
                starts_at,
                ends_at,
                timezone,
            }) => (
                true,
                false,
                Some(*starts_at),
                Some(*ends_at),
                Some(timezone.as_str().to_owned()),
                None,
                None,
            ),
            Some(Occurrence::AllDay {
                starts_on,
                ends_before,
            }) => (
                true,
                true,
                None,
                None,
                None,
                Some(*starts_on),
                Some(*ends_before),
            ),
        };

    let evento = sqlx::query_as::<_, CalendarEvent>(&format!(
        "UPDATE calendar_events SET
             title       = COALESCE($2, title),
             description = CASE WHEN $3 THEN $4 ELSE description END,
             location    = CASE WHEN $5 THEN $6 ELSE location END,
             all_day     = CASE WHEN $7 THEN $8  ELSE all_day     END,
             starts_at   = CASE WHEN $7 THEN $9  ELSE starts_at   END,
             ends_at     = CASE WHEN $7 THEN $10 ELSE ends_at     END,
             timezone    = CASE WHEN $7 THEN $11 ELSE timezone    END,
             starts_on   = CASE WHEN $7 THEN $12 ELSE starts_on   END,
             ends_before = CASE WHEN $7 THEN $13 ELSE ends_before END,
             updated_by_id = $14,
             updated_at    = now()
          WHERE id = $1
          RETURNING {EVENT_COLUMNS}"
    ))
    .bind(event_id)
    .bind(title)
    .bind(description.is_some())
    .bind(description.flatten())
    .bind(location.is_some())
    .bind(location.flatten())
    .bind(muda_quando)
    .bind(all_day)
    .bind(starts_at)
    .bind(ends_at)
    .bind(timezone)
    .bind(starts_on)
    .bind(ends_before)
    .bind(by)
    .fetch_one(executor)
    .await?;
    Ok(evento)
}

/// Marca um evento como cancelado.
///
/// Não o apaga: quem o esperava precisa de saber que não vai acontecer, e um
/// evento que desaparece não diz nada a ninguém.
///
/// # Errors
///
/// Devolve erro quando a actualização falha.
pub async fn cancel_event<'e>(
    executor: impl PgExecutor<'e>,
    event_id: Uuid,
    by: Uuid,
) -> CoreResult<Option<CalendarEvent>> {
    let evento = sqlx::query_as::<_, CalendarEvent>(&format!(
        "UPDATE calendar_events
            SET state = 'cancelled', updated_by_id = $2, updated_at = now()
          WHERE id = $1 AND state <> 'cancelled'
          RETURNING {EVENT_COLUMNS}"
    ))
    .bind(event_id)
    .bind(by)
    .fetch_optional(executor)
    .await?;
    Ok(evento)
}

/// Escreve um lembrete.
///
/// # Errors
///
/// Devolve erro quando a inserção falha.
pub async fn insert_reminder<'e>(
    executor: impl PgExecutor<'e>,
    organisation_id: Uuid,
    owner_id: Uuid,
    event_id: Option<Uuid>,
    task_id: Option<Uuid>,
    note: Option<&str>,
    trigger_at: DateTime<Utc>,
) -> CoreResult<Reminder> {
    let lembrete = sqlx::query_as::<_, Reminder>(&format!(
        "INSERT INTO reminders (organisation_id, owner_id, event_id, task_id, note, trigger_at)
         VALUES ($1,$2,$3,$4,$5,$6)
         RETURNING {REMINDER_COLUMNS}"
    ))
    .bind(organisation_id)
    .bind(owner_id)
    .bind(event_id)
    .bind(task_id)
    .bind(note)
    .bind(trigger_at)
    .fetch_one(executor)
    .await?;
    Ok(lembrete)
}

/// Os lembretes pendentes de uma pessoa.
///
/// Filtra por dono na consulta: um lembrete é de quem é, e conhecer o seu
/// identificador não o torna alcançável.
///
/// # Errors
///
/// Devolve erro quando a consulta falha.
pub async fn pending_reminders<'e>(
    executor: impl PgExecutor<'e>,
    owner_id: Uuid,
    limit: i64,
) -> CoreResult<Vec<Reminder>> {
    let lembretes = sqlx::query_as::<_, Reminder>(&format!(
        "SELECT {REMINDER_COLUMNS} FROM reminders
          WHERE owner_id = $1 AND state IN ('scheduled', 'snoozed')
          ORDER BY trigger_at
          LIMIT $2"
    ))
    .bind(owner_id)
    .bind(limit)
    .fetch_all(executor)
    .await?;
    Ok(lembretes)
}

/// Os prazos de tarefas que caem no intervalo e que este actor pode ver.
///
/// # Porque isto é uma consulta e não uma cópia
///
/// Uma `Task` com `due_on` já é um compromisso temporal. Copiá-la para
/// `calendar_events` daria duas datas para o mesmo prazo, e uma delas ficaria
/// errada sem ninguém saber qual. O calendário **pergunta** por elas; a tarefa
/// continua a pertencer a Collaboration (ADR-0410).
///
/// A visibilidade é a das tarefas — artefacto ∩ contentor —, e não a dos
/// eventos: um prazo é da tarefa, e é a tarefa que decide quem o vê.
///
/// # Errors
///
/// Devolve erro quando a consulta falha.
pub async fn task_deadlines_in_range<'e>(
    executor: impl PgExecutor<'e>,
    organisation_id: Uuid,
    filter: &VisibilityFilter,
    range: TimeRange,
    limit: i64,
) -> CoreResult<Vec<TaskDeadline>> {
    let artefacto = to_sql(
        filter,
        VisibilityColumns::aliased("t.unit_id", "t.workspace_id", "t.classification"),
    );
    let contido = contained_in_visible_workspace(filter, "t");

    let prazos = sqlx::query_as::<_, TaskDeadline>(&format!(
        "SELECT t.id, t.title, t.due_on, t.state, t.classification, t.workspace_id, t.unit_id
           FROM tasks t
          WHERE t.organisation_id = $3
            AND t.due_on IS NOT NULL
            AND t.due_on >= ($1 AT TIME ZONE 'UTC')::date
            AND t.due_on <  ($2 AT TIME ZONE 'UTC')::date
            AND {artefacto} AND {contido}
          ORDER BY t.due_on, t.title
          LIMIT $4"
    ))
    .bind(range.start)
    .bind(range.end)
    .bind(organisation_id)
    .bind(limit)
    .fetch_all(executor)
    .await?;
    Ok(prazos)
}

/// Move um lembrete para outro estado.
///
/// O dono entra na condição: conhecer o identificador de um lembrete alheio não
/// o torna alterável.
///
/// # Errors
///
/// Devolve erro quando a escrita falha.
pub async fn transition_reminder<'e>(
    executor: impl PgExecutor<'e>,
    reminder_id: Uuid,
    owner_id: Uuid,
    state: &str,
    trigger_at: Option<DateTime<Utc>>,
) -> CoreResult<Option<Reminder>> {
    let lembrete = sqlx::query_as::<_, Reminder>(&format!(
        "UPDATE reminders
            SET state = $3,
                trigger_at = COALESCE($4, trigger_at),
                updated_at = now()
          WHERE id = $1 AND owner_id = $2
          RETURNING {REMINDER_COLUMNS}"
    ))
    .bind(reminder_id)
    .bind(owner_id)
    .bind(state)
    .bind(trigger_at)
    .fetch_optional(executor)
    .await?;
    Ok(lembrete)
}

/// As notificações de uma pessoa, as mais recentes primeiro.
///
/// # Errors
///
/// Devolve erro quando a consulta falha.
pub async fn notifications_for<'e>(
    executor: impl PgExecutor<'e>,
    recipient_id: Uuid,
    limit: i64,
) -> CoreResult<Vec<Notification>> {
    let notificacoes = sqlx::query_as::<_, Notification>(
        "SELECT id, recipient_id, kind, title, body, resource_type, resource_id, read_at, created_at
           FROM notifications
          WHERE recipient_id = $1
          ORDER BY created_at DESC
          LIMIT $2",
    )
    .bind(recipient_id)
    .bind(limit)
    .fetch_all(executor)
    .await?;
    Ok(notificacoes)
}

/// Quantas notificações esta pessoa tem por ler.
///
/// # Errors
///
/// Devolve erro quando a consulta falha.
pub async fn unread_count<'e>(
    executor: impl PgExecutor<'e>,
    recipient_id: Uuid,
) -> CoreResult<i64> {
    let total = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM notifications WHERE recipient_id = $1 AND read_at IS NULL",
    )
    .bind(recipient_id)
    .fetch_one(executor)
    .await?;
    Ok(total)
}

/// Se uma pessoa existe e pertence a esta organização.
///
/// A fronteira da organização é verificada aqui, e não confiada ao chamador: um
/// identificador que vem de fora nomeia alguém, e a única forma de saber se essa
/// pessoa é desta instituição é perguntar à base.
///
/// Uma pessoa desactivada não participa: associá-la seria marcar uma reunião
/// com quem já não trabalha aqui. A coluna é `deactivated_at` — escrevi
/// `disabled_at` de cabeça, e o Postgres recusou a consulta inteira.
pub async fn person_in_organisation<'e>(
    executor: impl sqlx::PgExecutor<'e>,
    person_id: uuid::Uuid,
    organisation_id: uuid::Uuid,
) -> crate::CoreResult<bool> {
    let existe: Option<(bool,)> = sqlx::query_as(
        "SELECT true FROM people WHERE id = $1 AND organisation_id = $2 AND deactivated_at IS NULL",
    )
    .bind(person_id)
    .bind(organisation_id)
    .fetch_optional(executor)
    .await?;
    Ok(existe.is_some())
}

/// Associa uma pessoa a um evento.
///
/// `ON CONFLICT DO NOTHING`: associar duas vezes a mesma pessoa é a mesma
/// actividade com as mesmas pessoas, e não um erro a devolver a quem marca.
pub async fn add_participant<'e>(
    executor: impl sqlx::PgExecutor<'e>,
    event_id: uuid::Uuid,
    person_id: uuid::Uuid,
) -> crate::CoreResult<()> {
    sqlx::query(
        "INSERT INTO calendar_event_participants (event_id, person_id) \
         VALUES ($1, $2) ON CONFLICT DO NOTHING",
    )
    .bind(event_id)
    .bind(person_id)
    .execute(executor)
    .await?;
    Ok(())
}

/// Quem participa num evento.
pub async fn participants_of<'e>(
    executor: impl sqlx::PgExecutor<'e>,
    event_id: uuid::Uuid,
) -> crate::CoreResult<Vec<(uuid::Uuid, String)>> {
    let linhas: Vec<(uuid::Uuid, String)> = sqlx::query_as(
        // `display_name` é anulável; `full_name` não. Uma pessoa sem nome de
        // apresentação continua a ser a pessoa, e devolver `NULL` faria a
        // leitura inteira falhar por causa de um campo opcional.
        "SELECT p.id, COALESCE(p.display_name, p.full_name) \
         FROM calendar_event_participants c \
         JOIN people p ON p.id = c.person_id \
         WHERE c.event_id = $1 ORDER BY 2",
    )
    .bind(event_id)
    .fetch_all(executor)
    .await?;
    Ok(linhas)
}
