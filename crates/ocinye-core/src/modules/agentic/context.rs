//! The Ocinye Context Engine.
//!
//! # One rule
//!
//! > **Only the minimum authorised context required for the task reaches the
//! > model.**
//!
//! Both halves matter. *Authorised*, because a model that receives material the
//! member could not read has leaked it whatever the answer says. *Minimum*,
//! because context is finite and a model given the whole institution answers
//! worse than one given the five relevant things (briefing §25, §27, §139).
//!
//! # Two ceilings, not one
//!
//! Retrieval is filtered by the member's own read policy — that is the
//! authorisation. It is then filtered *again* by what may be sent for
//! inference, which is lower, because reading is not processing
//! ([`may_process_with_ai`]). A member entitled to `CONFIDENTIAL` material
//! still does not get it summarised by a model on somebody else's hardware
//! (briefing §114, §115).
//!
//! # Two ways material arrives
//!
//! **Retrieval** finds things: the member asked a question, and search answers
//! it within their own policy. **Selection** is the member pointing: they had
//! three notes open and said «resume isto».
//!
//! Selection is not a shortcut around anything. A pointed-at resource goes
//! through the same resolver as any other reference — it is looked up, checked
//! against the member's read policy, and then checked again against the
//! AI-processing ceiling. What selection changes is relevance, not authority:
//! the member said *these*, so these come first and retrieval fills the rest.
//!
//! # Provenance
//!
//! Every retrieved item keeps its identity and classification, so an answer can
//! say what informed it. An answer whose sources cannot be named is not usable
//! as institutional evidence (briefing §28).

use ocinye_contracts::{Classification, Permission, RagScope};
use ocinye_domain::{may_process_with_ai, Principal};
use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;

use super::resolver::ResolvedResource;
use crate::error::CoreResult;
use crate::modules::search;

/// How many artefacts one envelope may carry.
///
/// Small on purpose. The point of retrieval is to find the few things that
/// matter, and a model handed forty documents attends to none of them.
const MAX_SOURCES: u32 = 8;

/// How a piece of material came to be in the envelope.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Provenance {
    /// The member pointed at it.
    Selected,
    /// Search found it.
    Retrieved,
}

/// One thing that informed an answer.
#[derive(Debug, Clone, Serialize)]
pub struct ContextSource {
    /// Whether the member chose this or search found it.
    pub provenance: Provenance,
    /// What kind of artefact.
    pub entity_type: String,
    /// Which one.
    pub entity_id: Uuid,
    /// Its title.
    pub title: String,
    /// How it is classified. Carried so an answer can be attributed.
    pub classification: String,
    /// The excerpt that matched.
    pub excerpt: String,
}

/// Everything an agent is given about the situation.
///
/// # What is not here
///
/// Credentials. Session tokens. Other people's material. The member's password
/// hash. Anything the acting person could not themselves read. The envelope is
/// built from what the Core already authorised, and there is no field through
/// which anything else could arrive.
#[derive(Debug, Clone, Serialize)]
pub struct ContextEnvelope {
    /// Who is acting.
    pub actor_name: String,
    /// Their organisation.
    pub organisation_id: Uuid,
    /// The module they are in, when the request came from one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub module: Option<String>,
    /// The research workspace they are in, when in one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workspace_id: Option<Uuid>,
    /// The resource they are looking at, when looking at one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource_id: Option<Uuid>,
    /// What was retrieved, already filtered twice.
    pub sources: Vec<ContextSource>,
    /// The highest classification anything in `sources` carries.
    pub peak_classification: Classification,
    /// How many results the member could read but which may not be processed.
    ///
    /// Shown rather than hidden: «I found things I am not allowed to send to a
    /// model» is different from «I found nothing», and a member deciding
    /// whether the answer is complete needs to know which (briefing §188).
    pub withheld_from_inference: usize,
    /// The capabilities the agent may propose here.
    ///
    /// # Identificadores, e mais nada
    ///
    /// > **The inference context receives capability identifiers, never the
    /// > internal capability registry representation.**
    ///
    /// Não vai daqui a permissão, nem o risco, nem a aprovação exigida, nem o
    /// esquema de entrada, nem a `OperationId`. Essas coisas são **factos do
    /// Core**, e não informação de planeamento: um modelo que as conhecesse
    /// poderia argumentar sobre elas, e o que se quer é que não tenha nada sobre
    /// que argumentar.
    ///
    /// Hoje isto é verdade por construção. A guarda existe para o dia em que
    /// alguém quiser mandar mais «para o modelo planear melhor» — que é
    /// exactamente como uma fronteira destas se perde (ADR-0307).
    pub available_capabilities: Vec<String>,
}

impl ContextEnvelope {
    /// Whether anything was retrieved at all.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.sources.is_empty()
    }
}

/// Build the envelope for one request.
///
/// # Errors
///
/// Returns whatever search returns. A member with access to nothing gets an
/// empty envelope, which is a correct answer and not a refusal.
pub async fn assemble(
    pool: &PgPool,
    principal: &Principal,
    query: &str,
    scope: RagScope,
    workspace_id: Option<Uuid>,
    module: Option<&str>,
    local_inference: bool,
) -> CoreResult<ContextEnvelope> {
    // Scope narrows retrieval. It never widens authorisation: `search::search`
    // applies the member's own read policy inside the query, so a wider scope
    // simply returns more of what they could already reach.
    let scoped_workspace = match scope {
        RagScope::ResearchWorkspace | RagScope::Project => workspace_id,
        RagScope::Institutional | RagScope::Unit => None,
    };

    let (hits, _) = search::search(
        pool,
        principal,
        query,
        None,
        scoped_workspace,
        ocinye_contracts::PageRequest {
            page: 1,
            // Over-fetch a little, because the inference filter below removes
            // some and a short page should still be full where it can be.
            page_size: MAX_SOURCES * 2,
        },
    )
    .await?;

    let mut sources = Vec::new();
    let mut withheld = 0_usize;
    let mut peak = Classification::Public;

    for hit in hits {
        let classification =
            Classification::parse(&hit.classification).unwrap_or(Classification::Restricted);

        // The second ceiling. The member may read this; a model may not
        // necessarily process it.
        if !may_process_with_ai(classification, local_inference) {
            withheld += 1;
            continue;
        }

        if sources.len() >= MAX_SOURCES as usize {
            continue;
        }

        if classification.level() > peak.level() {
            peak = classification;
        }

        sources.push(ContextSource {
            provenance: Provenance::Retrieved,
            entity_type: hit.entity_type,
            entity_id: hit.entity_id,
            title: hit.title,
            classification: hit.classification,
            excerpt: hit.excerpt.unwrap_or_default(),
        });
    }

    Ok(ContextEnvelope {
        actor_name: principal.display_name.clone(),
        organisation_id: principal.organisation_id,
        module: module.map(ToOwned::to_owned),
        workspace_id,
        resource_id: None,
        sources,
        peak_classification: peak,
        withheld_from_inference: withheld,
        available_capabilities: Vec::new(),
    })
}

/// Fold the member's selection into an envelope.
///
/// # Order matters, and it is not cosmetic
///
/// Selected material goes first. Context is finite and attention is finite;
/// what the member pointed at should not be pushed out by the eighth search
/// hit. Retrieval then fills whatever room is left.
///
/// # Errors
///
/// Returns whatever the domain services return when reading the selected
/// resources. A selection the member cannot reach has already failed in
/// [`resolve_all`](super::resolver::resolve_all), before this runs.
pub async fn with_selection(
    pool: &PgPool,
    principal: &Principal,
    envelope: &mut ContextEnvelope,
    selection: &[ResolvedResource],
    local_inference: bool,
) -> CoreResult<()> {
    let mut selected = Vec::with_capacity(selection.len());

    for resource in selection {
        // The second ceiling, applied to selection exactly as to retrieval.
        // Pointing at something is not permission to have it processed: a
        // member may read a `CONFIDENTIAL` note and still not be entitled to
        // have a model on somebody else's hardware summarise it
        // (`CLAUDE.md` §36, briefing §19).
        if !may_process_with_ai(resource.classification, local_inference) {
            envelope.withheld_from_inference += 1;
            continue;
        }

        if resource.classification.level() > envelope.peak_classification.level() {
            envelope.peak_classification = resource.classification;
        }

        selected.push(ContextSource {
            provenance: Provenance::Selected,
            entity_type: resource.reference.kind.as_str().to_owned(),
            entity_id: resource.reference.id,
            title: resource.title.clone(),
            classification: resource.classification.as_str().to_owned(),
            excerpt: excerpt_of(pool, principal, resource).await?,
        });
    }

    envelope.resource_id = selection.first().map(|resource| resource.reference.id);

    // Selection first, then as much retrieval as still fits.
    let room = (MAX_SOURCES as usize).saturating_sub(selected.len());
    let retrieved: Vec<ContextSource> = envelope.sources.drain(..).take(room).collect();
    selected.extend(retrieved);
    envelope.sources = selected;

    Ok(())
}

/// The text of a selected resource, as far as the institution holds it.
///
/// # Why not one query
///
/// Each kind is read through the service that owns it, so each read is
/// authorised by the module whose invariant it is. The repetition is the price
/// of not having a second place where reading rules live (`CLAUDE.md` §17).
async fn excerpt_of(
    pool: &PgPool,
    principal: &Principal,
    resource: &ResolvedResource,
) -> CoreResult<String> {
    use ocinye_contracts::agentic::ResourceKind as Kind;

    let text = match resource.reference.kind {
        Kind::Note => {
            let (note, _) =
                crate::modules::knowledge::get_note(pool, principal, resource.reference.id).await?;
            note.body
        }
        Kind::Source => {
            let (source, _) =
                crate::modules::knowledge::get_source(pool, principal, resource.reference.id)
                    .await?;
            source.abstract_text.unwrap_or_default()
        }
        Kind::Idea => {
            let (idea, _) =
                crate::modules::research::get_idea(pool, principal, resource.reference.id).await?;
            [
                idea.summary.as_deref(),
                idea.research_question.as_deref(),
                idea.hypothesis.as_deref(),
            ]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
            .join("\n")
        }
        Kind::Project => {
            let (project, _) =
                crate::modules::research::get_project(pool, principal, resource.reference.id)
                    .await?;
            [project.summary.as_deref(), project.objectives.as_deref()]
                .into_iter()
                .flatten()
                .collect::<Vec<_>>()
                .join("\n")
        }
        // A document's bytes live in object storage, behind a separate
        // authorisation and a separate availability question. The title and
        // description travel; the content does not (briefing §39, §41).
        // Everything else contributes its title alone.
        _ => String::new(),
    };

    Ok(text.chars().take(MAX_EXCERPT).collect())
}

/// How much of one selected resource may travel.
///
/// A note can be long. Eight of them at full length is not context, it is a
/// corpus, and a model given a corpus attends to none of it.
const MAX_EXCERPT: usize = 2_000;

/// The domains worth offering for a request, from where it came from.
///
/// # Why not always everything
///
/// Sixty descriptors to plan «create a task» is wasted context and a map of the
/// system handed to whatever is on the other end. A request from inside Mail is
/// about mail; one from the command surface could be about anything, and only
/// then is the whole set warranted (briefing §21, §138).
#[must_use]
pub fn domains_for(module: Option<&str>) -> Option<Vec<&'static str>> {
    match module {
        Some("mail") => Some(vec!["mail", "knowledge"]),
        Some("research") => Some(vec!["research", "knowledge", "collaboration"]),
        Some("knowledge") => Some(vec!["knowledge", "research"]),
        Some("data") => Some(vec!["data", "knowledge"]),
        Some("compute") => Some(vec!["compute"]),
        // The command surface, or somewhere unrecognised. Everything, because
        // the member could be asking about anything.
        _ => None,
    }
}

/// Whether this member may use agentic assistance at all.
#[must_use]
pub fn may_use_assistance(principal: &Principal) -> bool {
    let institution = ocinye_domain::ResourceContext::organisation(
        ocinye_domain::ResourceKind::Person,
        principal.organisation_id,
    );
    ocinye_domain::can(principal, Permission::AiUse, &institution, None).allowed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_request_from_a_module_narrows_the_domains_offered() {
        assert_eq!(domains_for(Some("mail")), Some(vec!["mail", "knowledge"]));
        assert_eq!(
            domains_for(Some("compute")),
            Some(vec!["compute"]),
            "um pedido dentro da Computação não precisa das capabilities de correio"
        );
    }

    #[test]
    fn the_command_surface_gets_everything_because_it_could_be_anything() {
        assert_eq!(domains_for(None), None);
        assert_eq!(domains_for(Some("desconhecido")), None);
    }

    #[test]
    fn an_envelope_reports_what_it_withheld_rather_than_hiding_it() {
        // «Encontrei coisas que não posso enviar a um modelo» é diferente de
        // «não encontrei nada», e quem decide se a resposta está completa
        // precisa de saber qual dos dois é.
        let envelope = ContextEnvelope {
            actor_name: "Ana".to_owned(),
            organisation_id: Uuid::nil(),
            module: None,
            workspace_id: None,
            resource_id: None,
            sources: Vec::new(),
            peak_classification: Classification::Public,
            withheld_from_inference: 3,
            available_capabilities: Vec::new(),
        };

        assert!(envelope.is_empty());
        assert_eq!(envelope.withheld_from_inference, 3);
    }

    #[test]
    fn the_inference_ceiling_is_below_the_reading_ceiling() {
        // Sem nó local — o estado desta instalação.
        assert!(may_process_with_ai(Classification::Internal, false));
        assert!(!may_process_with_ai(Classification::Confidential, false));

        // Um nó local sobe o tecto sem o remover.
        assert!(may_process_with_ai(Classification::Confidential, true));
        assert!(!may_process_with_ai(Classification::Restricted, true));
    }
}
