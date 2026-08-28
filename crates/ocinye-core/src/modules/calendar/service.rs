//! As operações do calendário.

use chrono::{DateTime, Utc};
use ocinye_contracts::calendar::{EventScope, ReminderState, TemporalItemKind};
use ocinye_contracts::temporal::Occurrence;
use ocinye_contracts::Classification;
use ocinye_domain::policy::{authorize, Action, ResourceContext, ResourceKind, VisibilityFilter};
use ocinye_domain::Principal;
use ocinye_observability::CorrelationIds;
use sqlx::PgExecutor;
use uuid::Uuid;

use super::model::{CalendarEvent, Reminder, TemporalItem};
use super::repository::{self as repo, TimeRange};
use crate::audit::{self, action, AuditEntry};
use crate::error::{CoreError, CoreResult};
use crate::outbox::{self, event};
use crate::Tx;

/// O que é preciso para marcar um evento.
///
/// # Não há aqui um `owner_id`, e é deliberado
///
/// O dono de um evento pessoal é derivado do actor, nunca pedido a quem chama.
/// Aceitar o identificador do dono obrigaria a validá-lo bem em todos os
/// caminhos, para sempre; não o aceitar remove a pergunta.
///
/// O dia em que existir delegação legítima — alguém a marcar a agenda de outra
/// pessoa — ela nasce como operação própria, com a sua política. Não se faz
/// deixando qualquer pessoa preencher este campo.
#[derive(Debug, Clone)]
pub struct NewEvent {
    /// A quem pertence.
    pub scope: EventScope,
    /// Unidade, quando o âmbito a exige.
    pub unit_id: Option<Uuid>,
    /// Workspace, quando o âmbito o exige.
    pub workspace_id: Option<Uuid>,
    /// Título.
    pub title: String,
    /// Descrição.
    pub description: Option<String>,
    /// Onde.
    pub location: Option<String>,
    /// Quando.
    pub occurrence: Occurrence,
    /// Classificação pedida. O Core limita-a ao que o âmbito permite.
    pub classification: Option<Classification>,
    /// Quem participa, por referência institucional.
    ///
    /// Pessoas da instituição, e não nomes escritos à mão: uma pessoa guardada
    /// como texto deixa de ser a pessoa e passa a ser uma etiqueta que ninguém
    /// pode autorizar nem notificar. O universo é o que a tabela suporta —
    /// `people` — e o Core recusa quem não existe ou não pertence à mesma
    /// organização.
    pub participants: Vec<Uuid>,
}

/// O que é preciso para pedir um lembrete.
#[derive(Debug, Clone)]
pub struct NewReminder {
    /// Evento a que se refere.
    pub event_id: Option<Uuid>,
    /// Tarefa a que se refere.
    pub task_id: Option<Uuid>,
    /// O que dizer, quando não há recurso.
    pub note: Option<String>,
    /// Quando dispara.
    pub trigger_at: DateTime<Utc>,
}

/// O contexto de autorização de um evento, quando ele é institucional.
///
/// # `Personal` autoriza-se por titularidade, e não por contentor
///
/// > **A agenda pessoal usa uma regra própria de autorização por titularidade,
/// > em vez da política de autorização baseada em contentores institucionais.**
///
/// Não é uma excepção sem autorização: **há** autorização, e a autoridade é
/// `owner == principal.person_id` em vez de pertença a unidade ou workspace.
/// Devolver `None` aqui significa «este âmbito não tem contentor a consultar», e
/// quem chama aplica a regra de titularidade — não «passa sem verificação».
///
/// A distinção importa porque `Personal` e `Institution` são os dois âmbitos sem
/// contentor, e são **opostos**: no primeiro a fronteira é o dono, no segundo é a
/// política institucional. Tratá-los como parecidos por não terem contentor
/// seria abrir a agenda de toda a gente.
async fn context(
    tx: &mut Tx<'_>,
    principal: &Principal,
    request: &NewEvent,
) -> CoreResult<Option<ResourceContext>> {
    Ok(match request.scope {
        EventScope::Personal => None,
        EventScope::Unit => {
            let unit_id = request.unit_id.ok_or_else(|| {
                CoreError::Validation("Um evento de unidade tem de dizer qual.".to_owned())
            })?;
            Some(ResourceContext::unit(
                ResourceKind::CalendarEvent,
                principal.organisation_id,
                unit_id,
            ))
        }
        EventScope::ResearchWorkspace => {
            let workspace_id = request.workspace_id.ok_or_else(|| {
                CoreError::Validation("Um evento de workspace tem de dizer qual.".to_owned())
            })?;
            // Ler o workspace **é** autorizá-lo: um identificador que o actor
            // não alcança não chega a produzir contexto nenhum, e o evento
            // morre aqui em vez de nascer num sítio onde ele não entra.
            let workspace =
                crate::modules::research::get_workspace(&mut **tx, principal, workspace_id).await?;
            Some(crate::modules::research::workspace_context(
                &workspace,
                ResourceKind::CalendarEvent,
            ))
        }
        EventScope::Institution => Some(ResourceContext::organisation(
            ResourceKind::CalendarEvent,
            principal.organisation_id,
        )),
    })
}

/// Associa participantes a um evento, recusando quem não pode sê-lo.
///
/// Devolve quantos ficaram, para o evento de domínio o dizer.
///
/// # Errors
///
/// Recusa quando algum identificador não corresponde a uma pessoa activa desta
/// organização, ou quando a lista excede o tecto — e recusa a operação inteira,
/// porque uma actividade com metade dos participantes não é a actividade que
/// alguém pediu.
async fn adicionar_participantes(
    tx: &mut Tx<'_>,
    principal: &Principal,
    event_id: Uuid,
    pedidos: &[Uuid],
) -> CoreResult<usize> {
    if pedidos.is_empty() {
        return Ok(0);
    }

    // Repetições não são erro de quem marca: são um clique a mais. A chave
    // primária da tabela já as recusaria; deduplicar aqui evita transformar
    // uma conveniência numa mensagem de erro.
    let mut unicos: Vec<Uuid> = pedidos.to_vec();
    unicos.sort_unstable();
    unicos.dedup();

    const MAX_PARTICIPANTES: usize = 200;
    if unicos.len() > MAX_PARTICIPANTES {
        return Err(CoreError::Validation(format!(
            "Uma actividade não pode ter mais do que {MAX_PARTICIPANTES} participantes."
        )));
    }

    for person_id in &unicos {
        let pertence =
            repo::person_in_organisation(&mut **tx, *person_id, principal.organisation_id).await?;
        if !pertence {
            return Err(CoreError::Validation(
                "Um dos participantes não é uma pessoa desta instituição.".to_owned(),
            ));
        }
        repo::add_participant(&mut **tx, event_id, *person_id).await?;
    }

    Ok(unicos.len())
}

/// Marca um evento.
///
/// # Errors
///
/// Recusa quando o actor não pode criar no âmbito pedido, quando o âmbito não
/// traz o contentor que exige, quando o título é vazio, ou quando a escrita
/// falha.
pub async fn create_event(
    tx: &mut Tx<'_>,
    principal: &Principal,
    ids: &CorrelationIds,
    request: NewEvent,
) -> CoreResult<CalendarEvent> {
    let title = request.title.trim();
    if title.is_empty() {
        return Err(CoreError::Validation(
            "Um evento precisa de um título.".to_owned(),
        ));
    }

    if request.scope.needs_container()
        && request.unit_id.is_none()
        && request.workspace_id.is_none()
    {
        return Err(CoreError::Validation(
            "Este âmbito precisa de dizer a que unidade ou workspace pertence.".to_owned(),
        ));
    }

    // Duas regras de autorização, e não uma com uma excepção: contentor quando
    // há contentor, titularidade quando o âmbito é pessoal.
    if let Some(ctx) = context(tx, principal, &request).await? {
        authorize(principal, Action::Create, &ctx)
            .map_err(|(denial, decision)| CoreError::from_denial(denial, &decision))?;
    } else if !principal.is_active {
        return Err(CoreError::PermissionDenied(
            "A sua conta não está activa.".to_owned(),
        ));
    }

    let classification = match request.scope {
        // Um evento pessoal é do próprio e de mais ninguém. Marcá-lo como
        // `INTERNAL` fá-lo-ia legível por qualquer membro activo, que é
        // exactamente o contrário do que «pessoal» quer dizer — a cláusula de
        // visibilidade protege-o, mas uma classificação que promete outra coisa
        // é uma armadilha para a próxima consulta que alguém escrever.
        EventScope::Personal => Classification::Restricted,
        _ => request.classification.unwrap_or(Classification::Internal),
    };

    let owner_id = (request.scope == EventScope::Personal).then_some(principal.person_id);

    let evento = repo::insert_event(
        &mut **tx,
        principal.organisation_id,
        request.scope.as_str(),
        owner_id,
        request.unit_id,
        request.workspace_id,
        title,
        request.description.as_deref(),
        request.location.as_deref(),
        &request.occurrence,
        classification,
        principal.person_id,
    )
    .await?;

    // Os participantes, depois do evento existir e antes de qualquer efeito ser
    // anunciado.
    //
    // # Porque cada um é verificado, e não aceite
    //
    // Um identificador que vem do cliente nomeia alguém; não estabelece que essa
    // pessoa possa ser associada. A verificação é a mesma que o resto do sistema
    // usa — a pessoa existe, e existe **nesta** organização — e recusar aqui é a
    // diferença entre uma actividade com participantes e uma actividade com
    // referências que ninguém consegue resolver.
    let participantes =
        adicionar_participantes(tx, principal, evento.id, &request.participants).await?;

    outbox::emit(
        tx,
        event::CALENDAR_EVENT_CREATED,
        "calendar_event",
        evento.id,
        &ids.correlation_id,
        serde_json::json!({
            "scope": request.scope.as_str(),
            "participants": participantes,
        }),
    )
    .await?;

    audit::record(
        tx,
        Some(principal),
        ids,
        AuditEntry::new(action::CREATE, "calendar_event").resource(evento.id),
    )
    .await?;

    Ok(evento)
}

/// Um evento, se este actor o puder ler.
///
/// # Errors
///
/// Devolve `NotFound` quando o evento não existe **ou** quando o actor não o
/// alcança — a mesma resposta para os dois, que é a única que não diz se ele
/// existe.
pub async fn get_event<'e>(
    executor: impl PgExecutor<'e>,
    principal: &Principal,
    event_id: Uuid,
) -> CoreResult<CalendarEvent> {
    let filter = VisibilityFilter::for_principal(principal);
    repo::find_event(
        executor,
        principal.organisation_id,
        &filter,
        principal.person_id,
        event_id,
    )
    .await?
    .ok_or_else(|| CoreError::NotFound("Esse evento não existe.".to_owned()))
}

/// Que este actor pode alterar este evento, **no estado em que ele está agora**.
///
/// # Porquê reautorizar tão perto da escrita
///
/// Porque entre ler e escrever pode ter mudado o que importa: a classificação do
/// evento, a pertença do actor, a própria conta. Autorizar a partir de dados
/// lidos antes é autorizar um passado.
///
/// Alcançar não é poder alterar. Um evento pessoal é do dono — e nem sequer um
/// administrador entra aqui, pela mesma razão pela qual não o lê. Os outros
/// passam pela política do contentor.
fn assert_may_change(principal: &Principal, evento: &CalendarEvent) -> CoreResult<()> {
    match evento.scope() {
        EventScope::Personal => {
            if evento.owner_id == Some(principal.person_id) {
                Ok(())
            } else {
                Err(CoreError::PermissionDenied(
                    "Esse evento não é seu.".to_owned(),
                ))
            }
        }
        EventScope::Unit => {
            let ctx = ResourceContext::unit(
                ResourceKind::CalendarEvent,
                principal.organisation_id,
                evento.unit_id.unwrap_or_default(),
            )
            .with_classification(evento.classification());
            authorize(principal, Action::Update, &ctx)
                .map(|_| ())
                .map_err(|(denial, decision)| CoreError::from_denial(denial, &decision))
        }
        EventScope::ResearchWorkspace | EventScope::Institution => {
            let ctx = ResourceContext::organisation(
                ResourceKind::CalendarEvent,
                principal.organisation_id,
            )
            .with_classification(evento.classification());
            authorize(principal, Action::Update, &ctx)
                .map(|_| ())
                .map_err(|(denial, decision)| CoreError::from_denial(denial, &decision))
        }
    }
}

/// O que se pode mudar num evento já marcado.
///
/// # O que **não** está aqui, e é o ponto
///
/// `owner_id`, `scope`, `unit_id`, `workspace_id`, `organisation_id`,
/// `classification`. Nenhum campo de autoridade estrutural entra por aqui, e não
/// por serem validados mal — por não existirem no pedido.
///
/// A alternativa seria aceitá-los e validar em todos os caminhos, para sempre. E
/// é onde nascem os três ataques óbvios: mudar o dono de um evento pessoal para
/// outra pessoa, mover um evento para um workspace inalcançável, ou promover um
/// evento de unidade a institucional. Nenhum deles tem defesa a escrever aqui,
/// porque nenhum deles tem forma de ser expresso.
///
/// Se um dia mover um evento entre âmbitos fizer falta, nasce como operação
/// própria — com a sua política, autorizando **origem e destino**.
#[derive(Debug, Clone, Default)]
pub struct EventEdit {
    /// Novo título.
    pub title: Option<String>,
    /// Nova descrição. `Some(None)` apaga.
    pub description: Option<Option<String>>,
    /// Nova localização. `Some(None)` apaga.
    pub location: Option<Option<String>>,
    /// Nova ocorrência.
    pub occurrence: Option<Occurrence>,
}

/// Altera um evento.
///
/// # Errors
///
/// Recusa quando o actor não alcança o evento, não o pode alterar, ou quando o
/// título fica vazio.
pub async fn update_event(
    tx: &mut Tx<'_>,
    principal: &Principal,
    ids: &CorrelationIds,
    event_id: Uuid,
    edit: EventEdit,
) -> CoreResult<CalendarEvent> {
    // Relido dentro da transacção, e autorizado a seguir: o estado que decide é
    // o que está lá agora, não o que estava quando a página foi desenhada.
    let evento = get_event(&mut **tx, principal, event_id).await?;
    assert_may_change(principal, &evento)?;

    if let Some(title) = edit.title.as_deref() {
        if title.trim().is_empty() {
            return Err(CoreError::Validation(
                "Um evento precisa de um título.".to_owned(),
            ));
        }
    }

    let alterado = repo::update_event(
        &mut **tx,
        event_id,
        edit.title.as_deref().map(str::trim),
        edit.description.as_ref().map(Option::as_deref),
        edit.location.as_ref().map(Option::as_deref),
        edit.occurrence.as_ref(),
        principal.person_id,
    )
    .await?;

    audit::record(
        tx,
        Some(principal),
        ids,
        AuditEntry::new(action::UPDATE, "calendar_event").resource(event_id),
    )
    .await?;

    Ok(alterado)
}

/// Cancela um evento.
///
/// # Errors
///
/// Recusa quando o actor não alcança o evento ou não pode alterá-lo.
pub async fn cancel_event(
    tx: &mut Tx<'_>,
    principal: &Principal,
    ids: &CorrelationIds,
    event_id: Uuid,
) -> CoreResult<CalendarEvent> {
    let evento = get_event(&mut **tx, principal, event_id).await?;
    assert_may_change(principal, &evento)?;

    // Idempotente: cancelar o que já está cancelado devolve o mesmo evento, sem
    // segunda escrita e sem erro. Quem carrega duas vezes no botão não merece
    // uma falha, e um cliente que repete um pedido também não.
    //
    // E é transição de estado, não apagamento: quem esperava a reunião precisa
    // de saber que não vai acontecer, e um evento que desaparece não diz nada a
    // ninguém.
    let cancelado = repo::cancel_event(&mut **tx, event_id, principal.person_id)
        .await?
        .unwrap_or(evento);

    audit::record(
        tx,
        Some(principal),
        ids,
        AuditEntry::new(action::UPDATE, "calendar_event").resource(cancelado.id),
    )
    .await?;

    Ok(cancelado)
}

/// Pede um lembrete.
///
/// # Errors
///
/// Recusa quando o recurso referido não é alcançável pelo actor, ou quando o
/// lembrete não diz nada.
pub async fn create_reminder(
    tx: &mut Tx<'_>,
    principal: &Principal,
    ids: &CorrelationIds,
    request: NewReminder,
) -> CoreResult<Reminder> {
    if request.event_id.is_some() && request.task_id.is_some() {
        return Err(CoreError::Validation(
            "Um lembrete refere um recurso, não dois.".to_owned(),
        ));
    }

    let note = request
        .note
        .as_deref()
        .map(str::trim)
        .filter(|n| !n.is_empty());
    if request.event_id.is_none() && request.task_id.is_none() && note.is_none() {
        return Err(CoreError::Validation(
            "Um lembrete sem recurso tem de dizer o que é.".to_owned(),
        ));
    }

    // Um lembrete sobre um recurso é uma forma de o ler mais tarde. Se o actor
    // não o alcança agora, não pode agendar que lho mostrem.
    if let Some(event_id) = request.event_id {
        get_event(&mut **tx, principal, event_id).await?;
    }

    let lembrete = repo::insert_reminder(
        &mut **tx,
        principal.organisation_id,
        principal.person_id,
        request.event_id,
        request.task_id,
        note,
        request.trigger_at,
    )
    .await?;

    audit::record(
        tx,
        Some(principal),
        ids,
        AuditEntry::new(action::CREATE, "reminder").resource(lembrete.id),
    )
    .await?;

    Ok(lembrete)
}

/// A agenda de um intervalo, com tudo o que o actor pode ver.
///
/// # Uma pergunta, cinco superfícies
///
/// Centro Temporal, Hoje, Semana, Mês e Agenda chamam **esta** função. Podem
/// projectar o resultado de maneiras diferentes; não podem discordar sobre o
/// universo de recursos que o actor alcança.
///
/// # Errors
///
/// Devolve erro quando a consulta falha — e um erro é um erro, nunca uma agenda
/// vazia.
pub async fn agenda<'e>(
    executor: impl PgExecutor<'e> + Copy,
    principal: &Principal,
    range: TimeRange,
    limit: i64,
) -> CoreResult<Vec<TemporalItem>> {
    let filter = VisibilityFilter::for_principal(principal);
    let eventos = repo::events_in_range(
        executor,
        principal.organisation_id,
        &filter,
        principal.person_id,
        range,
        limit,
    )
    .await?;

    // Os prazos de tarefas entram por projecção, e não por cópia. A
    // visibilidade é a **da tarefa** — é a tarefa que decide quem vê o seu
    // prazo —, e por isso a consulta é outra, com o mesmo filtro de origem.
    let prazos =
        repo::task_deadlines_in_range(executor, principal.organisation_id, &filter, range, limit)
            .await?;

    let mut itens: Vec<TemporalItem> = eventos
        .into_iter()
        .map(|evento| TemporalItem {
            kind: TemporalItemKind::Event,
            id: evento.id,
            title: evento.title.clone(),
            occurrence: evento.occurrence(),
            state: evento.state.clone(),
            classification: evento.classification(),
            workspace_id: evento.workspace_id,
            unit_id: evento.unit_id,
        })
        .collect();

    itens.extend(prazos.into_iter().map(|prazo| TemporalItem {
        kind: TemporalItemKind::TaskDue,
        id: prazo.id,
        title: prazo.title,
        // Um prazo é uma data civil, e ocupa o dia. Meio-aberto, como tudo o
        // resto: vence *nesse* dia, não à meia-noite seguinte.
        occurrence: Occurrence::AllDay {
            starts_on: prazo.due_on,
            ends_before: prazo.due_on.succ_opt().unwrap_or(prazo.due_on),
        },
        state: prazo.state,
        classification:
            Classification::parse(&prazo.classification).unwrap_or(Classification::Internal),
        workspace_id: Some(prazo.workspace_id),
        unit_id: Some(prazo.unit_id),
    }));

    let referencia = ocinye_contracts::temporal::TimeZoneName::utc();
    itens.sort_by_key(|item| item.occurrence.ordering_instant(referencia));

    Ok(itens)
}

/// Adia um lembrete.
///
/// # Errors
///
/// Recusa quando o lembrete não é do actor, ou quando já não está pendente.
pub async fn snooze_reminder(
    tx: &mut Tx<'_>,
    principal: &Principal,
    ids: &CorrelationIds,
    reminder_id: Uuid,
    until: DateTime<Utc>,
) -> CoreResult<Reminder> {
    transicionar(
        tx,
        principal,
        ids,
        reminder_id,
        ReminderState::Snoozed,
        Some(until),
    )
    .await
}

/// A pessoa diz que já viu.
///
/// # Errors
///
/// Recusa quando o lembrete não é do actor.
pub async fn dismiss_reminder(
    tx: &mut Tx<'_>,
    principal: &Principal,
    ids: &CorrelationIds,
    reminder_id: Uuid,
) -> CoreResult<Reminder> {
    transicionar(
        tx,
        principal,
        ids,
        reminder_id,
        ReminderState::Dismissed,
        None,
    )
    .await
}

/// Deixa de fazer sentido.
///
/// # Errors
///
/// Recusa quando o lembrete não é do actor.
pub async fn cancel_reminder(
    tx: &mut Tx<'_>,
    principal: &Principal,
    ids: &CorrelationIds,
    reminder_id: Uuid,
) -> CoreResult<Reminder> {
    transicionar(
        tx,
        principal,
        ids,
        reminder_id,
        ReminderState::Cancelled,
        None,
    )
    .await
}

/// A transição, com a autoridade onde tem de estar.
///
/// O dono entra na condição da escrita, e não numa leitura anterior: um lembrete
/// alheio e um lembrete inexistente dão a mesma resposta, que é a única que não
/// diz se existe.
async fn transicionar(
    tx: &mut Tx<'_>,
    principal: &Principal,
    ids: &CorrelationIds,
    reminder_id: Uuid,
    state: ReminderState,
    trigger_at: Option<DateTime<Utc>>,
) -> CoreResult<Reminder> {
    let lembrete = repo::transition_reminder(
        &mut **tx,
        reminder_id,
        principal.person_id,
        state.as_str(),
        trigger_at,
    )
    .await?
    .ok_or_else(|| CoreError::NotFound("Esse lembrete não existe.".to_owned()))?;

    audit::record(
        tx,
        Some(principal),
        ids,
        AuditEntry::new(action::UPDATE, "reminder").resource(lembrete.id),
    )
    .await?;

    Ok(lembrete)
}

/// Marca uma notificação como lida.
///
/// # Errors
///
/// Devolve erro quando a escrita falha. Uma notificação de outra pessoa não é
/// erro: simplesmente não é alterada, porque o destinatário entra na condição.
pub async fn mark_notification_read<'e>(
    executor: impl PgExecutor<'e>,
    principal: &Principal,
    notification_id: Uuid,
) -> CoreResult<()> {
    super::delivery::mark_read(executor, principal.person_id, notification_id).await
}

/// Quantos itens a agenda tem no intervalo.
///
/// Chama o mesmo predicado da listagem, e é por isso que existe como função e
/// não como consulta escrita à parte: uma contagem que discorde da lista promete
/// páginas que não existem.
///
/// # Errors
///
/// Devolve erro quando a consulta falha.
pub async fn agenda_count<'e>(
    executor: impl PgExecutor<'e>,
    principal: &Principal,
    range: TimeRange,
) -> CoreResult<i64> {
    let filter = VisibilityFilter::for_principal(principal);
    repo::count_events_in_range(
        executor,
        principal.organisation_id,
        &filter,
        principal.person_id,
        range,
    )
    .await
}

/// Os lembretes que esta pessoa ainda espera.
///
/// Sem filtro de visibilidade porque não é preciso: um lembrete é de quem é, e a
/// consulta pergunta pelo dono. Não há aqui um universo institucional a
/// restringir — há uma pessoa a ver os seus.
///
/// # Errors
///
/// Devolve erro quando a consulta falha.
pub async fn pending_reminders<'e>(
    executor: impl PgExecutor<'e>,
    principal: &Principal,
    limit: i64,
) -> CoreResult<Vec<Reminder>> {
    repo::pending_reminders(executor, principal.person_id, limit).await
}

/// As notificações desta pessoa.
///
/// # Errors
///
/// Devolve erro quando a consulta falha.
pub async fn notifications<'e>(
    executor: impl PgExecutor<'e>,
    principal: &Principal,
    limit: i64,
) -> CoreResult<Vec<super::model::Notification>> {
    repo::notifications_for(executor, principal.person_id, limit).await
}

/// Quantas notificações esta pessoa tem por ler.
///
/// É o que o sino da barra superior mostra. Zero é zero — o ponto dourado não se
/// pinta por decoração.
///
/// # Errors
///
/// Devolve erro quando a consulta falha.
pub async fn unread_notifications<'e>(
    executor: impl PgExecutor<'e>,
    principal: &Principal,
) -> CoreResult<i64> {
    repo::unread_count(executor, principal.person_id).await
}

/// Quem participa numa actividade.
///
/// # Porque existe no serviço e não no repositório
///
/// Porque o repositório é a forma como o módulo fala com a base, e não a forma
/// como o resto do sistema fala com o módulo. Expor o repositório para uma
/// leitura seria abrir a fronteira inteira por causa de uma consulta.
///
/// # Errors
///
/// Devolve erro quando a leitura falha.
pub async fn participants_of(
    executor: impl sqlx::PgExecutor<'_>,
    event_id: Uuid,
) -> CoreResult<Vec<(Uuid, String)>> {
    repo::participants_of(executor, event_id).await
}
