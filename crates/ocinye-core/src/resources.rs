//! Resolver uma referência tipada sob a política de quem a apresenta.
//!
//! # Porque isto é do Core e não do plano agentic
//!
//! Viveu em `modules::agentic::resolver`, e ficou lá por ser o plano agentic o
//! primeiro a precisar. Mas a pergunta que este módulo responde não é agentic:
//!
//! > este par `(tipo, identificador)` nomeia um recurso que existe, e que esta
//! > pessoa pode ler?
//!
//! É a mesma pergunta que qualquer operação tem de fazer quando recebe um
//! identificador vindo de fora — e a proveniência científica recebe dois de
//! cada vez. Deixá-lo no plano agentic obrigava o domínio a chamar o seu
//! próprio cliente, que é a dependência ao contrário.
//!
//! O plano agentic continua a ser um consumidor. Passou a não ser o dono.
//!
//! Turning a name an agent used into a resource it is allowed to touch.
//!
//! # The gap this closes
//!
//! A [`ResourceRef`] arrives in a plan the same way everything else in a plan
//! arrives: a model wrote it. It carries a kind, an identifier and a label, and
//! **none of the three is evidence of anything**. The label is the model's own
//! words. The identifier may name a note in another unit, a document that was
//! archived last week, or nothing at all.
//!
//! Resolution is where that stops being a claim and becomes a fact — or fails.
//! Every reference is looked up in the Core, filtered through the acting
//! person's own read policy, and returned with the context the *resource* has
//! rather than the context the request had.
//!
//! # Why the resource's own context matters
//!
//! Without this, every agentic step is authorised against the institution: the
//! organisation, no unit, no workspace, `INTERNAL`. That is the right context
//! for «may this person create ideas at all» and the wrong one for «may this
//! person read *this* note» — it names no unit to be outside of and no
//! classification to be above.
//!
//! A resolved resource carries its real unit, its real workspace and its real
//! classification, so the executor can ask the question again where it counts
//! (ADR-0306).
//!
//! # Absence and refusal are the same answer
//!
//! A reference the person may not see returns [`CoreError::NotFound`], exactly
//! as one that does not exist. Distinguishing them would let anybody enumerate
//! the institution by identifier and read the difference between «no» and «not
//! yours» — the same rule the HTTP surface already follows (`CLAUDE.md` §60).

use ocinye_contracts::agentic::{ResourceKind as AgenticKind, ResourceRef};
use ocinye_contracts::Classification;
use ocinye_domain::{Principal, ResourceContext, ResourceKind};
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::{CoreError, CoreResult};
use crate::modules::{calendar, collaboration, data, knowledge, research, science};

/// A reference that survived resolution.
///
/// # What it guarantees
///
/// The thing exists, the acting person may read it, and the context below is
/// the institution's, not the model's. A handler that holds one of these may
/// use it without checking again; a handler that holds a bare [`ResourceRef`]
/// may not.
#[derive(Debug, Clone)]
pub struct ResolvedResource {
    /// The reference, with the label replaced by the Core's own title.
    pub reference: ResourceRef,
    /// The resource's own authorization context.
    pub context: ResourceContext,
    /// The title, as the Core holds it.
    pub title: String,
    /// The classification, as the Core holds it.
    pub classification: Classification,
    /// The research workspace it belongs to, when it belongs to one.
    pub workspace_id: Option<Uuid>,
}

impl ResolvedResource {
    /// The reference to hand back to an interface.
    #[must_use]
    pub fn as_ref(&self) -> ResourceRef {
        self.reference.clone()
    }
}

/// Everything resolution needs to know about one resource, before it is shaped.
struct Located {
    kind: ResourceKind,
    title: String,
    classification: Classification,
    unit_id: Uuid,
    /// The research workspace it lives in, when it lives in one.
    ///
    /// `None` for a resource whose scope *is* a unit — a unit itself. Building
    /// a workspace context for it would name a workspace that does not exist,
    /// and every membership comparison against it would then be against
    /// nothing.
    workspace_id: Option<Uuid>,
}

/// Resolve one reference, or refuse it.
///
/// # Errors
///
/// Returns [`CoreError::NotFound`] when the resource does not exist, when the
/// acting person may not read it, or when the reference names a kind this plane
/// cannot address. The three are deliberately indistinguishable.
pub async fn resolve(
    pool: &PgPool,
    principal: &Principal,
    reference: &ResourceRef,
) -> CoreResult<ResolvedResource> {
    // Every branch below goes through the domain service that owns the read,
    // which is what applies the acting person's policy. This module never
    // queries and never decides; it shapes what those services return into the
    // context the executor needs (`CLAUDE.md` §17).
    let located = locate(pool, principal, reference).await.map_err(mask)?;

    let context = match located.workspace_id {
        Some(workspace_id) => ResourceContext::workspace(
            located.kind,
            principal.organisation_id,
            located.unit_id,
            workspace_id,
            located.classification,
        ),
        None => ResourceContext::unit(located.kind, principal.organisation_id, located.unit_id)
            .with_classification(located.classification),
    };

    Ok(ResolvedResource {
        reference: ResourceRef {
            kind: reference.kind,
            id: reference.id,
            // The Core's title, never the model's label. A plan that shows
            // «Relatório de Segurança» for a note actually called something
            // else is a plan the member confirmed under a wrong description.
            label: Some(located.title.clone()),
        },
        context,
        title: located.title,
        classification: located.classification,
        workspace_id: located.workspace_id,
    })
}

/// Resolve several, in order.
///
/// # Errors
///
/// Fails on the first reference that does not resolve. A plan step that names
/// four resources and may reach three of them is not a step that runs on three.
pub async fn resolve_all(
    pool: &PgPool,
    principal: &Principal,
    references: &[ResourceRef],
) -> CoreResult<Vec<ResolvedResource>> {
    let mut resolved = Vec::with_capacity(references.len());
    for reference in references {
        resolved.push(resolve(pool, principal, reference).await?);
    }
    Ok(resolved)
}

/// The one refusal this module produces.
fn not_found() -> CoreError {
    CoreError::NotFound("Este recurso não existe, ou não lhe está acessível.".to_owned())
}

/// Collapse every refusal into absence.
///
/// A domain service distinguishes «no such note» from «not yours», and it is
/// right to: an HTTP route knows which workspace the caller asked about. Here
/// the identifier came from a model, and preserving the distinction would turn
/// the agentic plane into an oracle for enumerating the institution by
/// identifier (`CLAUDE.md` §60).
fn mask(error: CoreError) -> CoreError {
    match error {
        CoreError::NotFound(_) | CoreError::PermissionDenied(_) | CoreError::Domain(_) => {
            not_found()
        }
        other => other,
    }
}

/// Find the resource behind a reference, through the service that owns it.
///
/// # Os recursos científicos entram aqui, e não num resolvedor próprio
///
/// A proveniência recebe dois pares `(tipo, identificador)` de cada vez e tem
/// de os provar a ambos. Um segundo resolvedor para a ciência seria uma
/// segunda política de leitura — e duas políticas acabam por discordar no dia
/// em que uma delas for corrigida.
async fn locate(
    pool: &PgPool,
    principal: &Principal,
    reference: &ResourceRef,
) -> CoreResult<Located> {
    match reference.kind {
        AgenticKind::Idea => {
            let (idea, workspace) = research::get_idea(pool, principal, reference.id).await?;
            Ok(Located {
                kind: ResourceKind::Idea,
                title: idea.title,
                classification: workspace.classification(),
                unit_id: workspace.unit_id,
                workspace_id: Some(workspace.id),
            })
        }
        AgenticKind::Project => {
            let (project, workspace) = research::get_project(pool, principal, reference.id).await?;
            Ok(Located {
                kind: ResourceKind::Project,
                title: project.title,
                classification: workspace.classification(),
                unit_id: workspace.unit_id,
                workspace_id: Some(workspace.id),
            })
        }
        AgenticKind::Workspace => {
            let workspace = research::get_workspace(pool, principal, reference.id).await?;
            Ok(Located {
                kind: ResourceKind::ResearchWorkspace,
                title: workspace.title.clone(),
                classification: workspace.classification(),
                unit_id: workspace.unit_id,
                workspace_id: Some(workspace.id),
            })
        }
        AgenticKind::Source => {
            let (source, workspace) = knowledge::get_source(pool, principal, reference.id).await?;
            Ok(Located {
                kind: ResourceKind::Source,
                title: source.title.clone(),
                // The artefact's own classification may sit above its
                // workspace's. The stricter of the two governs, because a
                // `CONFIDENTIAL` source inside an `INTERNAL` workspace is still
                // confidential.
                classification: stricter(source.classification(), workspace.classification()),
                unit_id: source.unit_id,
                workspace_id: Some(source.workspace_id),
            })
        }
        AgenticKind::Note => {
            let (note, workspace) = knowledge::get_note(pool, principal, reference.id).await?;
            Ok(Located {
                kind: ResourceKind::Note,
                title: note.title.clone(),
                classification: stricter(note.classification(), workspace.classification()),
                unit_id: note.unit_id,
                workspace_id: Some(note.workspace_id),
            })
        }
        AgenticKind::Document => {
            let (document, workspace) =
                knowledge::get_document(pool, principal, reference.id).await?;
            Ok(Located {
                kind: ResourceKind::Document,
                title: document.title.clone(),
                classification: stricter(document.classification(), workspace.classification()),
                unit_id: document.unit_id,
                workspace_id: Some(document.workspace_id),
            })
        }
        // Um compromisso resolve-se pela operação que já o autoriza: se o actor
        // não o alcança, não há `Located` nenhum — e um identificador que o
        // modelo escreveu morre aqui, antes de chegar a qualquer capability
        // (ADR-0306).
        AgenticKind::CalendarEvent => {
            let evento = calendar::get_event(pool, principal, reference.id).await?;
            Ok(Located {
                kind: ResourceKind::CalendarEvent,
                title: evento.title.clone(),
                classification: evento.classification(),
                // Sem unidade — um evento pessoal ou institucional não tem —
                // usa-se a organização, que é o âmbito em que ele existe.
                unit_id: evento.unit_id.unwrap_or(principal.organisation_id),
                workspace_id: evento.workspace_id,
            })
        }
        // Um lembrete é de quem é. Não há contentor a consultar, e a consulta
        // filtra pelo dono.
        AgenticKind::Reminder => {
            let lembretes = calendar::pending_reminders(pool, principal, 200).await?;
            let lembrete = lembretes
                .into_iter()
                .find(|r| r.id == reference.id)
                .ok_or_else(|| CoreError::NotFound("Esse lembrete não existe.".to_owned()))?;
            Ok(Located {
                kind: ResourceKind::Reminder,
                title: lembrete
                    .note
                    .clone()
                    .unwrap_or_else(|| "Lembrete".to_owned()),
                classification: ocinye_contracts::Classification::Restricted,
                unit_id: principal.organisation_id,
                workspace_id: None,
            })
        }
        AgenticKind::Task => {
            let (task, workspace) = collaboration::get_task(pool, principal, reference.id).await?;
            Ok(Located {
                kind: ResourceKind::Task,
                title: task.title.clone(),
                classification: stricter(task.classification(), workspace.classification()),
                unit_id: task.unit_id,
                workspace_id: Some(task.workspace_id),
            })
        }
        // A unit is the scope an Idea is born into, so it has to be
        // addressable: a capability that names its unit through `input` is
        // authorised against the organisation instead, where a permission that
        // comes from membership does not exist — which made
        // `research.idea.create` unreachable by exactly the people who would
        // use it (ADR-0306).
        //
        // A unit carries no classification of its own; `INTERNAL` is what the
        // shape of the institution is, and the unit read above is the gate.
        AgenticKind::Unit => {
            let unit =
                crate::modules::organisation::get_unit(pool, principal, reference.id).await?;
            Ok(Located {
                kind: ResourceKind::Unit,
                title: unit.name.clone(),
                classification: Classification::Internal,
                unit_id: unit.id,
                workspace_id: None,
            })
        }

        // Kinds this plane does not address. A reference to one resolves to
        // nothing rather than to a guess — including the mail kinds, whose
        // ── Os recursos científicos ─────────────────────────────────
        //
        // Cada um passa pelo serviço que detém a sua leitura, e é esse que
        // aplica a política de quem pergunta. Este módulo não consulta a base
        // e não decide: dá forma ao que os serviços devolvem.
        AgenticKind::Hypothesis => {
            let (hypothesis, workspace) =
                science::get_hypothesis(pool, principal, reference.id).await?;
            Ok(Located {
                kind: ResourceKind::Hypothesis,
                title: hypothesis.statement.clone(),
                classification: stricter(hypothesis.classification(), workspace.classification()),
                unit_id: hypothesis.unit_id,
                workspace_id: hypothesis.workspace_id,
            })
        }
        AgenticKind::Methodology => {
            let (methodology, workspace) =
                science::get_methodology(pool, principal, reference.id).await?;
            Ok(Located {
                kind: ResourceKind::Methodology,
                title: methodology.title.clone(),
                classification: stricter(methodology.classification(), workspace.classification()),
                unit_id: methodology.unit_id,
                workspace_id: methodology.workspace_id,
            })
        }
        AgenticKind::MethodologyVersion => {
            let (version, methodology, workspace) =
                science::get_methodology_version(pool, principal, reference.id).await?;
            Ok(Located {
                kind: ResourceKind::MethodologyVersion,
                // O título diz de quem é a versão: «Método X — v2» lê-se; «v2»
                // sozinho não diz de quê.
                title: format!("{} — {}", methodology.title, version.label),
                classification: stricter(methodology.classification(), workspace.classification()),
                unit_id: methodology.unit_id,
                workspace_id: methodology.workspace_id,
            })
        }
        AgenticKind::Study => {
            let (study, workspace) = science::get_study(pool, principal, reference.id).await?;
            Ok(Located {
                kind: ResourceKind::Study,
                title: study.title.clone(),
                classification: stricter(study.classification(), workspace.classification()),
                unit_id: study.unit_id,
                workspace_id: study.workspace_id,
            })
        }
        AgenticKind::StudyExecution => {
            let (execution, study, workspace) =
                science::get_execution(pool, principal, reference.id).await?;
            Ok(Located {
                kind: ResourceKind::StudyExecution,
                title: format!("{} — execução {}", study.title, execution.sequence),
                classification: stricter(study.classification(), workspace.classification()),
                unit_id: study.unit_id,
                workspace_id: study.workspace_id,
            })
        }
        AgenticKind::Result => {
            let (result, workspace) = science::get_result(pool, principal, reference.id).await?;
            Ok(Located {
                kind: ResourceKind::Result,
                title: result.title.clone(),
                classification: stricter(result.classification(), workspace.classification()),
                unit_id: result.unit_id,
                workspace_id: result.workspace_id,
            })
        }
        AgenticKind::DatasetVersion => {
            let (version, dataset, workspace) =
                data::get_dataset_version(pool, principal, reference.id).await?;
            Ok(Located {
                kind: ResourceKind::DatasetVersion,
                title: format!("{} — {}", dataset.title, version.label),
                classification: stricter(dataset.classification(), workspace.classification()),
                unit_id: dataset.unit_id,
                workspace_id: Some(dataset.workspace_id),
            })
        }

        // capabilities carry their own identifiers and never route through here.
        AgenticKind::Person
        | AgenticKind::Dataset
        | AgenticKind::MailMessage
        | AgenticKind::MailDraft
        | AgenticKind::Mailbox
        | AgenticKind::Agent
        | AgenticKind::ComputeNode
        | AgenticKind::ComputeJob
        // Uma conversa e uma mensagem não se desreferenciam por identificador
        // escrito por um modelo. As capacidades das Mensagens resolvem os seus
        // alvos a partir de quem se nomeia, e a participação decide o resto.
        | AgenticKind::Conversation
        | AgenticKind::Message => Err(not_found()),
    }
}

/// The stricter of two classifications.
fn stricter(left: Classification, right: Classification) -> Classification {
    if left.level() >= right.level() {
        left
    } else {
        right
    }
}

// Não há aqui um `may_act_on`, e não falta.
//
// A distinção que ele guardava — resolver prova que a pessoa pode **ver** a
// coisa, autorizar prova que pode fazer-lhe **aquilo** — continua a ser feita,
// e mais completa: o executor chama `may_invoke(principal, agent, descriptor,
// context, resource_id)` por cada recurso resolvido, e essa passa também pela
// fronteira do agente e pelo descritor da capacidade. Um predicado mais fino a
// responder à mesma pergunta seria um segundo sítio a decidir autorização.
