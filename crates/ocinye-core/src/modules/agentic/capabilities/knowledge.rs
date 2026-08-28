//! Knowledge capabilities.

use async_trait::async_trait;
use ocinye_contracts::agentic::{
    ApprovalRequirement, AutonomyLevel, CapabilityDescriptor, CapabilityId, CapabilityResult,
    ExecutionStatus, OperationId, ResourceKind as AgenticKind, ResourceRef, Reversibility,
    RiskLevel,
};
use ocinye_contracts::{Classification, PageRequest, Permission, Scope};

use crate::error::{CoreError, CoreResult};
use crate::modules::agentic::executor::ExecutionContext;
use crate::modules::agentic::registry::CapabilityHandler;
use crate::modules::{knowledge, search};

/// Search the institutional index.
///
/// # The capability that works with no AI at all
///
/// Search is deterministic. It is here so that the command surface can answer
/// `Search` intent without a model, which is what keeps it useful in this
/// installation's actual state — zero AI nodes (briefing §32, §66).
pub struct Search;

#[async_trait]
impl CapabilityHandler for Search {
    fn descriptor(&self) -> CapabilityDescriptor {
        CapabilityDescriptor {
            id: CapabilityId::new("knowledge.search"),
            operation: OperationId::new("search::search"),
            domain: "knowledge".to_owned(),
            summary: "Pesquisar no acervo institucional.".to_owned(),
            // A floor, not the boundary.
            //
            // Search is open to any member: the Core applies authorisation
            // *inside* the query, so somebody with access to nothing gets zero
            // results rather than a refusal (`CLAUDE.md` §28). The permission
            // here says «you are a member of this institution», which is the
            // only precondition — `BibliographyView` would be wrong, because no
            // role holds it institutionally and search would become unreachable
            // through this plane for everybody.
            permission: Permission::OrganisationView,
            scope: Scope::Institution,
            risk: RiskLevel::ReadOnly,
            approval: ApprovalRequirement::Never,
            max_autonomy: AutonomyLevel::Workflow,
            reversibility: Reversibility::NothingToUndo,
            supports_dry_run: false,
            classification_ceiling: None,
            input_schema: serde_json::json!({
                "type": "object",
                "required": ["query"],
                "properties": {
                    "query": {"type": "string", "description": "O que procurar."}
                }
            }),
        }
    }

    async fn execute(&self, ctx: &ExecutionContext<'_>) -> CoreResult<CapabilityResult> {
        let query = ctx.text("query")?;

        let (hits, total) = search::search(
            ctx.pool,
            ctx.principal,
            &query,
            None,
            None,
            PageRequest {
                page: 1,
                page_size: 10,
            },
        )
        .await?;

        let resources: Vec<ResourceRef> = hits
            .iter()
            .filter_map(|hit| {
                // A hit whose type this plane has no reference kind for is left
                // out of the references rather than guessed at: a `ResourceRef`
                // of the wrong kind would resolve to the wrong thing later.
                AgenticKind::parse(&hit.entity_type).map(|kind| ResourceRef {
                    kind,
                    id: hit.entity_id,
                    label: Some(hit.title.clone()),
                })
            })
            .collect();

        Ok(CapabilityResult {
            capability: self.descriptor().id,
            status: ExecutionStatus::Succeeded,
            detail: match total {
                0 => "Nenhum resultado.".to_owned(),
                1 => "1 resultado.".to_owned(),
                other => format!("{other} resultados."),
            },
            resources,
            reversibility: Reversibility::NothingToUndo,
            output: Some(serde_json::json!({
                "total": total,
                "items": hits.iter().map(|hit| serde_json::json!({
                    "entity_type": hit.entity_type,
                    "entity_id": hit.entity_id,
                    "title": hit.title,
                    "classification": hit.classification,
                    "excerpt": hit.excerpt,
                    "workspace_id": hit.workspace_id,
                })).collect::<Vec<_>>(),
            })),
        })
    }
}

/// Read one note, with its body.
///
/// # The body travels, and that is the point
///
/// «Resume estas três notas» is only answerable if the notes reach the model.
/// What protects the institution is not withholding the body from a member who
/// may read it — it is the two ceilings above: the member's own read policy,
/// applied by the resolver, and the AI-processing ceiling, applied by the
/// Context Engine before anything is sent for inference.
pub struct ReadNote;

#[async_trait]
impl CapabilityHandler for ReadNote {
    fn descriptor(&self) -> CapabilityDescriptor {
        CapabilityDescriptor {
            id: CapabilityId::new("knowledge.note.read"),
            operation: OperationId::new("knowledge::get_note"),
            domain: "knowledge".to_owned(),
            summary: "Ler uma Nota.".to_owned(),
            permission: Permission::NotesView,
            scope: Scope::ResearchWorkspace,
            risk: RiskLevel::ReadOnly,
            approval: ApprovalRequirement::Never,
            max_autonomy: AutonomyLevel::Workflow,
            reversibility: Reversibility::NothingToUndo,
            supports_dry_run: false,
            classification_ceiling: None,
            input_schema: serde_json::json!({"type": "object", "properties": {}}),
        }
    }

    async fn execute(&self, ctx: &ExecutionContext<'_>) -> CoreResult<CapabilityResult> {
        let note_id = ctx.one(AgenticKind::Note)?.reference.id;
        let (note, workspace) = knowledge::get_note(ctx.pool, ctx.principal, note_id).await?;

        Ok(CapabilityResult {
            capability: self.descriptor().id,
            status: ExecutionStatus::Succeeded,
            detail: format!("Nota «{}».", note.title),
            resources: vec![ResourceRef {
                kind: AgenticKind::Note,
                id: note.id,
                label: Some(note.title.clone()),
            }],
            reversibility: Reversibility::NothingToUndo,
            output: Some(serde_json::json!({
                "id": note.id,
                "title": note.title,
                "body": note.body,
                "tags": note.tags,
                "revision": note.revision,
                "classification": note.classification().as_str(),
                "workspace_id": workspace.id,
                "workspace_code": workspace.code,
            })),
        })
    }
}

/// Read one bibliographic source.
///
/// # Metadata, and full text only where a basis was recorded
///
/// Ocinye does not hold full articles because it holds a reference to them. The
/// legal basis lives on the source as `content_right`, and this capability
/// reports it rather than working around it (ADR-0402, `CLAUDE.md` §43).
pub struct ReadSource;

#[async_trait]
impl CapabilityHandler for ReadSource {
    fn descriptor(&self) -> CapabilityDescriptor {
        CapabilityDescriptor {
            id: CapabilityId::new("knowledge.source.read"),
            operation: OperationId::new("knowledge::get_source"),
            domain: "knowledge".to_owned(),
            summary: "Ler uma entrada bibliográfica.".to_owned(),
            permission: Permission::BibliographyView,
            scope: Scope::ResearchWorkspace,
            risk: RiskLevel::ReadOnly,
            approval: ApprovalRequirement::Never,
            max_autonomy: AutonomyLevel::Workflow,
            reversibility: Reversibility::NothingToUndo,
            supports_dry_run: false,
            classification_ceiling: None,
            input_schema: serde_json::json!({"type": "object", "properties": {}}),
        }
    }

    async fn execute(&self, ctx: &ExecutionContext<'_>) -> CoreResult<CapabilityResult> {
        let source_id = ctx.one(AgenticKind::Source)?.reference.id;
        let (source, workspace) = knowledge::get_source(ctx.pool, ctx.principal, source_id).await?;

        Ok(CapabilityResult {
            capability: self.descriptor().id,
            status: ExecutionStatus::Succeeded,
            detail: format!("Fonte «{}».", source.title),
            resources: vec![ResourceRef {
                kind: AgenticKind::Source,
                id: source.id,
                label: Some(source.title.clone()),
            }],
            reversibility: Reversibility::NothingToUndo,
            output: Some(serde_json::json!({
                "id": source.id,
                "title": source.title,
                "source_type": source.source_type,
                "authors": source.authors,
                "year": source.year,
                "container_title": source.container_title,
                "publisher": source.publisher,
                "doi": source.doi,
                "isbn": source.isbn,
                "url": source.url,
                "abstract": source.abstract_text,
                "keywords": source.keywords,
                "licence": source.licence,
                "citation_key": source.citation_key,
                "classification": source.classification().as_str(),
                "content_right": source.content_right().as_str(),
                // Whether the institution may hold the full text at all, and
                // whether it actually does. Two different questions, and an
                // answer that conflates them invites somebody to ask for bytes
                // that must not exist.
                "full_content_permitted": source.content_right().allows_full_content(),
                "full_text_document_id": source.full_text_document_id,
                "workspace_id": workspace.id,
            })),
        })
    }
}

/// Read one document's metadata.
///
/// # Metadata only, deliberately
///
/// The bytes live in object storage, behind a separate authorisation and a
/// separate availability question. This capability answers «what is this
/// document» without depending on storage being reachable, so it keeps working
/// in the state this installation is actually in (briefing §39, §41).
pub struct ReadDocument;

#[async_trait]
impl CapabilityHandler for ReadDocument {
    fn descriptor(&self) -> CapabilityDescriptor {
        CapabilityDescriptor {
            id: CapabilityId::new("knowledge.document.read"),
            operation: OperationId::new("knowledge::get_document"),
            domain: "knowledge".to_owned(),
            summary: "Ler os metadados de um Documento.".to_owned(),
            permission: Permission::DocumentsView,
            scope: Scope::ResearchWorkspace,
            risk: RiskLevel::ReadOnly,
            approval: ApprovalRequirement::Never,
            max_autonomy: AutonomyLevel::Workflow,
            reversibility: Reversibility::NothingToUndo,
            supports_dry_run: false,
            classification_ceiling: None,
            input_schema: serde_json::json!({"type": "object", "properties": {}}),
        }
    }

    async fn execute(&self, ctx: &ExecutionContext<'_>) -> CoreResult<CapabilityResult> {
        let document_id = ctx.one(AgenticKind::Document)?.reference.id;
        let (document, workspace) =
            knowledge::get_document(ctx.pool, ctx.principal, document_id).await?;

        Ok(CapabilityResult {
            capability: self.descriptor().id,
            status: ExecutionStatus::Succeeded,
            detail: format!("Documento «{}».", document.title),
            resources: vec![ResourceRef {
                kind: AgenticKind::Document,
                id: document.id,
                label: Some(document.title.clone()),
            }],
            reversibility: Reversibility::NothingToUndo,
            output: Some(serde_json::json!({
                "id": document.id,
                "title": document.title,
                "description": document.description,
                "kind": document.kind,
                "document_date": document.document_date,
                "original_filename": document.original_filename,
                "content_type": document.content_type,
                "size_bytes": document.size_bytes,
                "checksum_sha256": document.checksum_sha256,
                "classification": document.classification().as_str(),
                "workspace_id": workspace.id,
                // Said plainly rather than implied by absence: the content is
                // not here, and obtaining it is a separate, authorised act.
                "content_included": false,
            })),
        })
    }
}

/// List the typed relations of a research workspace.
///
/// # The institutional-memory capability
///
/// Everything else here reads one artefact. This one reads the edges — what
/// cites what, what derived from what, which note supports which project. It is
/// the closest the Ocinye OS currently comes to answering «how does this work
/// connect to the rest of the institution» (`CLAUDE.md` §13, §14).
pub struct ListLinks;

#[async_trait]
impl CapabilityHandler for ListLinks {
    fn descriptor(&self) -> CapabilityDescriptor {
        CapabilityDescriptor {
            id: CapabilityId::new("knowledge.links.list"),
            operation: OperationId::new("knowledge::list_links"),
            domain: "knowledge".to_owned(),
            summary: "Listar as relações de um Research Workspace.".to_owned(),
            permission: Permission::OrganisationView,
            scope: Scope::ResearchWorkspace,
            risk: RiskLevel::ReadOnly,
            approval: ApprovalRequirement::Never,
            max_autonomy: AutonomyLevel::Workflow,
            reversibility: Reversibility::NothingToUndo,
            supports_dry_run: false,
            classification_ceiling: None,
            input_schema: serde_json::json!({"type": "object", "properties": {}}),
        }
    }

    async fn execute(&self, ctx: &ExecutionContext<'_>) -> CoreResult<CapabilityResult> {
        let workspace_id = ctx.one(AgenticKind::Workspace)?.reference.id;
        let links = knowledge::list_links(ctx.pool, ctx.principal, workspace_id).await?;

        Ok(CapabilityResult {
            capability: self.descriptor().id,
            status: ExecutionStatus::Succeeded,
            detail: match links.len() {
                0 => "Nenhuma relação registada.".to_owned(),
                1 => "1 relação.".to_owned(),
                other => format!("{other} relações."),
            },
            // Deliberately no `ResourceRef` per endpoint. A link says two things
            // are related; it does not say the reader may reach both, and
            // returning references would suggest they may (briefing §135).
            resources: Vec::new(),
            reversibility: Reversibility::NothingToUndo,
            output: Some(serde_json::json!({ "links": links })),
        })
    }
}

/// Create a note in a research workspace.
///
/// # Reversible, and therefore safe to propose
///
/// A note is additive: it takes nothing away and changes no other artefact. The
/// worst outcome is a note somebody deletes. That is what puts this at low
/// impact while a state transition sits two levels above it.
///
/// # The text is the member's, not the model's
///
/// The Core records the acting person as author. A model preparing prose does
/// not make the model an author, and the provenance of the *operation* — which
/// agent, which capability, which plan — is recorded separately in audit
/// (briefing §87, §88).
pub struct CreateNote;

#[async_trait]
impl CapabilityHandler for CreateNote {
    fn descriptor(&self) -> CapabilityDescriptor {
        CapabilityDescriptor {
            id: CapabilityId::new("knowledge.note.create"),
            operation: OperationId::new("knowledge::create_note"),
            domain: "knowledge".to_owned(),
            summary: "Criar uma Nota num Research Workspace.".to_owned(),
            permission: Permission::NotesCreate,
            scope: Scope::ResearchWorkspace,
            risk: RiskLevel::LowImpact,
            approval: ApprovalRequirement::Never,
            max_autonomy: AutonomyLevel::Workflow,
            reversibility: Reversibility::Reversible,
            supports_dry_run: true,
            classification_ceiling: None,
            input_schema: serde_json::json!({
                "type": "object",
                "required": ["title", "body"],
                "properties": {
                    "title": {"type": "string"},
                    "body": {"type": "string"},
                    "tags": {"type": "array"},
                    "classification": {"type": "string"}
                }
            }),
        }
    }

    async fn execute(&self, ctx: &ExecutionContext<'_>) -> CoreResult<CapabilityResult> {
        let workspace_id = ctx.one(AgenticKind::Workspace)?.reference.id;
        let title = ctx.text("title")?;
        let body = ctx.text("body")?;
        let tags: Vec<String> = ctx.optional("tags")?.unwrap_or_default();

        // A classification a model proposed is a request. `create_note` caps it
        // against the workspace and refuses what it cannot grant.
        let classification = ctx
            .optional::<String>("classification")?
            .and_then(|raw| Classification::parse(&raw));

        if ctx.dry_run {
            return Ok(CapabilityResult {
                capability: self.descriptor().id,
                status: ExecutionStatus::DryRun,
                detail: format!("Seria criada a Nota «{title}»."),
                resources: Vec::new(),
                reversibility: Reversibility::NothingToUndo,
                output: None,
            });
        }

        let mut tx = ctx.pool.begin().await?;
        let note = knowledge::create_note(
            &mut tx,
            ctx.principal,
            ctx.ids,
            workspace_id,
            knowledge::NewNote {
                title: title.clone(),
                body,
                tags,
                classification,
            },
        )
        .await?;
        tx.commit().await?;

        Ok(CapabilityResult {
            capability: self.descriptor().id,
            status: ExecutionStatus::Succeeded,
            detail: format!("Nota «{title}» criada."),
            resources: vec![ResourceRef {
                kind: AgenticKind::Note,
                id: note.id,
                label: Some(title),
            }],
            reversibility: Reversibility::Reversible,
            output: Some(serde_json::json!({ "note_id": note.id })),
        })
    }
}

/// Add a bibliographic entry.
///
/// # Metadata only, always
///
/// This capability records a reference. It never attaches full content, because
/// doing so requires a recorded legal basis and a deliberate act by a person
/// (ADR-0402). An agent that could quietly attach an article would be a
/// copyright decision made by a model.
pub struct CreateSource;

#[async_trait]
impl CapabilityHandler for CreateSource {
    fn descriptor(&self) -> CapabilityDescriptor {
        CapabilityDescriptor {
            id: CapabilityId::new("knowledge.source.create"),
            operation: OperationId::new("knowledge::create_source"),
            domain: "knowledge".to_owned(),
            summary: "Adicionar uma entrada bibliográfica.".to_owned(),
            permission: Permission::BibliographyCreate,
            scope: Scope::ResearchWorkspace,
            risk: RiskLevel::LowImpact,
            approval: ApprovalRequirement::Never,
            max_autonomy: AutonomyLevel::Workflow,
            reversibility: Reversibility::Reversible,
            supports_dry_run: true,
            classification_ceiling: None,
            input_schema: serde_json::json!({
                "type": "object",
                "required": ["title"],
                "properties": {
                    "title": {"type": "string"},
                    "source_type": {"type": "string"},
                    "authors": {"type": "array"},
                    "year": {"type": "integer"},
                    "container_title": {"type": "string"},
                    "publisher": {"type": "string"},
                    "doi": {"type": "string"},
                    "url": {"type": "string"},
                    "abstract": {"type": "string"},
                    "keywords": {"type": "array"}
                }
            }),
        }
    }

    async fn execute(&self, ctx: &ExecutionContext<'_>) -> CoreResult<CapabilityResult> {
        let workspace_id = ctx.one(AgenticKind::Workspace)?.reference.id;
        let title = ctx.text("title")?;

        if ctx.dry_run {
            return Ok(CapabilityResult {
                capability: self.descriptor().id,
                status: ExecutionStatus::DryRun,
                detail: format!("Seria acrescentada a fonte «{title}»."),
                resources: Vec::new(),
                reversibility: Reversibility::NothingToUndo,
                output: None,
            });
        }

        let mut tx = ctx.pool.begin().await?;
        let source = knowledge::create_source(
            &mut tx,
            ctx.principal,
            ctx.ids,
            workspace_id,
            knowledge::NewSource {
                source_type: ctx
                    .optional::<String>("source_type")?
                    .and_then(|raw| knowledge::SourceType::parse(&raw)),
                title: title.clone(),
                authors: ctx.optional("authors")?.unwrap_or_default(),
                year: ctx.optional("year")?,
                container_title: ctx.optional("container_title")?,
                publisher: ctx.optional("publisher")?,
                doi: ctx.optional("doi")?,
                isbn: ctx.optional("isbn")?,
                url: ctx.optional("url")?,
                abstract_text: ctx.optional("abstract")?,
                keywords: ctx.optional("keywords")?.unwrap_or_default(),
                licence: None,
                // Never proposed by a model. The default holds metadata only,
                // and raising it is a decision with a legal basis behind it.
                content_right: None,
                origin: Some("agentic".to_owned()),
                citation_key: None,
                classification: None,
                // Reserved for imports that carry their own record, such as
                // BibTeX. Nothing a model wrote belongs here.
                raw_metadata: None,
            },
        )
        .await?;
        tx.commit().await?;

        Ok(CapabilityResult {
            capability: self.descriptor().id,
            status: ExecutionStatus::Succeeded,
            detail: format!("Fonte «{title}» acrescentada, apenas com metadados."),
            resources: vec![ResourceRef {
                kind: AgenticKind::Source,
                id: source.id,
                label: Some(title),
            }],
            reversibility: Reversibility::Reversible,
            output: Some(serde_json::json!({
                "source_id": source.id,
                "content_right": source.content_right().as_str(),
            })),
        })
    }
}

/// Relate two research objects.
///
/// # Both endpoints, or neither
///
/// «Relaciona esta nota ao Projecto X» asserts something about the note *and*
/// about the project. Being able to write in one workspace is not authority to
/// name a resource in another, and a relation whose far end the member cannot
/// reach would be a side channel: read the near end, learn the far one exists
/// (briefing §134, §135).
///
/// The enforcement is structural rather than written here. Both endpoints
/// travel as `resources` on the request, and the executor resolves every one of
/// them before this handler runs — so a far end the member cannot read has
/// already stopped the step.
pub struct CreateLink;

#[async_trait]
impl CapabilityHandler for CreateLink {
    fn descriptor(&self) -> CapabilityDescriptor {
        CapabilityDescriptor {
            id: CapabilityId::new("knowledge.link.create"),
            operation: OperationId::new("knowledge::link_objects"),
            domain: "knowledge".to_owned(),
            summary: "Relacionar dois objectos de investigação.".to_owned(),
            permission: Permission::LinksCreate,
            scope: Scope::ResearchWorkspace,
            risk: RiskLevel::LowImpact,
            approval: ApprovalRequirement::Never,
            max_autonomy: AutonomyLevel::Workflow,
            reversibility: Reversibility::Reversible,
            supports_dry_run: true,
            classification_ceiling: None,
            input_schema: serde_json::json!({
                "type": "object",
                "required": ["relation"],
                "properties": {
                    "relation": {
                        "type": "string",
                        "description":
                            "cites, supports, refutes, derived_from, uses, produces, relates_to",
                    },
                    "note": {"type": "string", "description": "Porquê, em texto livre."}
                }
            }),
        }
    }

    async fn execute(&self, ctx: &ExecutionContext<'_>) -> CoreResult<CapabilityResult> {
        let relation = ctx.text("relation")?;
        let note: Option<String> = ctx.optional("note")?;

        // The endpoints come from `resources`, not from the input, and that is
        // deliberate: `resources` is the field the executor resolves. Reading
        // identifiers out of the input would route around the gate.
        let [from, to] = match ctx.resources {
            [from, to] => [from, to],
            _ => {
                return Err(CoreError::Validation(
                    "Uma relação precisa exactamente de dois recursos: a origem e o destino."
                        .to_owned(),
                ))
            }
        };

        // Both ends must sit in the same research workspace. A relation that
        // spans two workspaces would have no single authorization context, and
        // no honest answer to «who may see this edge».
        let (Some(workspace_id), Some(target_workspace)) = (from.workspace_id, to.workspace_id)
        else {
            return Err(CoreError::Validation(
                "Ambos os recursos têm de pertencer a um Research Workspace.".to_owned(),
            ));
        };
        if workspace_id != target_workspace {
            return Err(CoreError::Validation(
                "Os dois recursos pertencem a Research Workspaces diferentes.".to_owned(),
            ));
        }

        if ctx.dry_run {
            return Ok(CapabilityResult {
                capability: self.descriptor().id,
                status: ExecutionStatus::DryRun,
                detail: format!("«{}» passaria a {relation} «{}».", from.title, to.title),
                resources: Vec::new(),
                reversibility: Reversibility::NothingToUndo,
                output: None,
            });
        }

        let mut tx = ctx.pool.begin().await?;
        let link = knowledge::link_objects(
            &mut tx,
            ctx.principal,
            ctx.ids,
            workspace_id,
            from.reference.kind.as_str(),
            from.reference.id,
            &relation,
            to.reference.kind.as_str(),
            to.reference.id,
            note.as_deref(),
        )
        .await?;
        tx.commit().await?;

        Ok(CapabilityResult {
            capability: self.descriptor().id,
            status: ExecutionStatus::Succeeded,
            detail: format!("«{}» {relation} «{}».", from.title, to.title),
            resources: vec![from.as_ref(), to.as_ref()],
            reversibility: Reversibility::Reversible,
            output: Some(serde_json::json!({ "link_id": link.id })),
        })
    }
}

/// Revise an existing Note.
///
/// # Why a revision is low risk here, and would not be everywhere
///
/// `update_note` snapshots the previous version into `note_revisions` before
/// writing, so what the note said is not lost — it is history. That is what
/// makes this `Reversible` rather than a claim: there is something to go back
/// to, and the domain put it there without this capability asking.
///
/// The classification is deliberately absent from the schema. Reclassifying is
/// a governance act with its own permission and its own audit shape, and a
/// capability that could do it while renaming would hide it inside an edit
/// (briefing §12).
pub struct ReviseNote;

#[async_trait]
impl CapabilityHandler for ReviseNote {
    fn descriptor(&self) -> CapabilityDescriptor {
        CapabilityDescriptor {
            id: CapabilityId::new("knowledge.note.revise"),
            operation: OperationId::new("knowledge::update_note"),
            domain: "knowledge".to_owned(),
            summary: "Rever uma Nota. A versão anterior fica no histórico.".to_owned(),
            permission: Permission::NotesEdit,
            scope: Scope::ResearchWorkspace,
            risk: RiskLevel::LowImpact,
            approval: ApprovalRequirement::Never,
            max_autonomy: AutonomyLevel::Workflow,
            reversibility: Reversibility::Reversible,
            supports_dry_run: true,
            classification_ceiling: None,
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "title": {"type": "string"},
                    "body": {"type": "string"},
                    "tags": {"type": "array"}
                }
            }),
        }
    }

    async fn execute(&self, ctx: &ExecutionContext<'_>) -> CoreResult<CapabilityResult> {
        let note_id = ctx.one(AgenticKind::Note)?.reference.id;

        let title: Option<String> = ctx.optional("title")?;
        let body: Option<String> = ctx.optional("body")?;
        let tags: Option<Vec<String>> = ctx.optional("tags")?;

        if title.is_none() && body.is_none() && tags.is_none() {
            return Err(CoreError::Validation(
                "Indique o que pretende alterar na Nota.".to_owned(),
            ));
        }

        if ctx.dry_run {
            let (note, _) = knowledge::get_note(ctx.pool, ctx.principal, note_id).await?;
            return Ok(CapabilityResult {
                capability: self.descriptor().id,
                status: ExecutionStatus::DryRun,
                detail: format!("«{}» seria revista.", note.title),
                resources: Vec::new(),
                reversibility: Reversibility::NothingToUndo,
                output: None,
            });
        }

        let mut tx = ctx.pool.begin().await?;
        let note = knowledge::update_note(
            &mut tx,
            ctx.principal,
            ctx.ids,
            note_id,
            title.as_deref(),
            body.as_deref(),
            tags,
        )
        .await?;
        tx.commit().await?;

        Ok(CapabilityResult {
            capability: self.descriptor().id,
            status: ExecutionStatus::Succeeded,
            detail: format!("Nota «{}» revista.", note.title),
            resources: vec![ResourceRef {
                kind: AgenticKind::Note,
                id: note.id,
                label: Some(note.title.clone()),
            }],
            reversibility: Reversibility::Reversible,
            output: Some(serde_json::json!({
                "note_id": note.id,
                "revision": note.revision,
            })),
        })
    }
}
/// Rever uma bibliografia BibTeX.
///
/// # Porque um agente pode pedir isto
///
/// Porque não muda nada. Não guarda, não envia, não altera autoridade, e a
/// mesma entrada dá sempre a mesma saída. É a forma de capacidade que menos
/// exige de quem a autoriza: o pior resultado é uma leitura que não serviu.
///
/// E é útil onde um agente costuma tropeçar. Um modelo que redija uma
/// bibliografia produz texto plausível; passá-lo por aqui diz o que é
/// estruturalmente legível e o que não é, sem que ninguém tenha de acreditar
/// no modelo.
///
/// # O agente não alcança o Runtime
///
/// Este handler chama a mesma Core Operation que a interface humana chama. Quem
/// decide o que se executa é o Core; o que o plano agentic traz é o pedido.
pub struct ReviewBibliography;

#[async_trait]
impl CapabilityHandler for ReviewBibliography {
    fn descriptor(&self) -> CapabilityDescriptor {
        CapabilityDescriptor {
            id: CapabilityId::new("knowledge.bibliography.review"),
            operation: OperationId::new("knowledge::review_bibliography"),
            domain: "knowledge".to_owned(),
            summary: "Validar e normalizar referências BibTeX.".to_owned(),
            // A mesma permissão que acrescentar uma referência. Rever é o passo
            // anterior, e não faz sentido onde não se pode acrescentar.
            permission: Permission::BibliographyCreate,
            scope: Scope::ResearchWorkspace,
            // Nada muda: é uma leitura sobre texto que veio no pedido.
            risk: RiskLevel::ReadOnly,
            approval: ApprovalRequirement::Never,
            max_autonomy: AutonomyLevel::Workflow,
            reversibility: Reversibility::NothingToUndo,
            // Uma simulação seria idêntica à execução, porque a execução não
            // deixa rasto. Oferecer as duas seria oferecer a mesma coisa duas
            // vezes com nomes diferentes.
            supports_dry_run: false,
            classification_ceiling: None,
            input_schema: serde_json::json!({
                "type": "object",
                "required": ["bibtex"],
                "properties": {
                    "bibtex": {"type": "string"}
                }
            }),
        }
    }

    async fn execute(&self, ctx: &ExecutionContext<'_>) -> CoreResult<CapabilityResult> {
        let workspace_id = ctx.one(AgenticKind::Workspace)?.reference.id;
        let bibtex = ctx.text("bibtex")?;

        let revisao = knowledge::review_bibliography(
            ctx.pool,
            ctx.capabilities,
            ctx.principal,
            workspace_id,
            &bibtex,
        )
        .await?;

        let detail = if revisao.is_complete() {
            format!(
                "{} referência(s) lidas, todas legíveis.",
                revisao.read_count()
            )
        } else {
            format!(
                "{} referência(s) lidas; {} não foram legíveis.",
                revisao.read_count(),
                revisao.unreadable.len()
            )
        };

        Ok(CapabilityResult {
            capability: self.descriptor().id,
            status: ExecutionStatus::Succeeded,
            detail,
            resources: Vec::new(),
            reversibility: Reversibility::NothingToUndo,
            output: Some(serde_json::to_value(&revisao).unwrap_or(serde_json::Value::Null)),
        })
    }
}

#[cfg(test)]
mod autoridade_declarada {
    use super::*;

    /// A revisão de bibliografia exige o mesmo que acrescentar uma referência.
    ///
    /// # Porque isto é um teste e não o comentário que já lá estava
    ///
    /// O descritor dizia, em prosa, «a mesma permissão que acrescentar uma
    /// referência». Baixei-a para `BibliographyView` e a suite inteira — 250
    /// testes — passou. A frase era verdadeira e não era verificada, que é a
    /// definição de autoridade obsoleta: o que o plano agentic **declara** e o
    /// que o Core **exige** deixam de ser a mesma coisa sem que nada o diga.
    ///
    /// A declaração não é decorativa. `registry().available_to(...)` filtra por
    /// ela: uma permissão declarada a menos mostra a capability a quem o Core
    /// vai recusar; a mais esconde-a de quem podia usá-la. Falha fechada nos
    /// dois sentidos, e nos dois sentidos descreve mal o sistema.
    ///
    /// A permissão é derivada de `CreateSource`, não escrita outra vez aqui: se
    /// a de acrescentar mudar, esta acompanha ou este teste diz porquê.
    #[test]
    fn rever_bibliografia_exige_o_mesmo_que_acrescentar_uma_referencia() {
        let rever = ReviewBibliography.descriptor();
        let acrescentar = CreateSource.descriptor();

        assert_eq!(
            rever.permission,
            acrescentar.permission,
            "`{}` declara `{}` e `{}` declara `{}`: a revisão é o passo anterior \
             a acrescentar, e não faz sentido onde não se pode acrescentar",
            rever.id,
            rever.permission.as_str(),
            acrescentar.id,
            acrescentar.permission.as_str(),
        );
    }
}
