//! As operações do ciclo científico.
//!
//! # A autorização de um artefacto científico
//!
//! É a mesma de qualquer artefacto de investigação, e por uma razão que não é
//! conveniência: uma hipótese, um estudo e um resultado vivem num Research
//! Workspace, e é o ambiente que governa quem lá entra. A classificação
//! própria do artefacto **sobe** por cima da do ambiente quando é mais
//! estrita — um resultado `CONFIDENTIAL` num ambiente `INTERNAL` continua
//! confidencial.
//!
//! Reutiliza-se `research::readable_artefact_workspace`, que já é o sítio onde
//! essa regra vive. Escrever uma segunda versão dela aqui seria escrever uma
//! segunda política de autorização — e duas políticas acabam sempre por
//! discordar.

use ocinye_contracts::{Classification, Permission};
use ocinye_domain::policy::{authorize, can, Action, ResourceKind};
use ocinye_domain::Principal;
use ocinye_observability::CorrelationIds;
use sqlx::PgPool;
use uuid::Uuid;

use super::model::{
    Hypothesis, Methodology, MethodologyVersion, Result as ScientificResult, ResultValidation,
    Study, StudyExecution,
};
use super::repository as repo;
use crate::audit::{self, action, AuditEntry};
use crate::error::{CoreError, CoreResult};
use crate::modules::research::{
    artefact_context, get_workspace, readable_artefact_workspace, ResearchWorkspace,
};
use crate::Tx;

/// A ausência e a recusa dizem o mesmo.
///
/// Distingui-las diria a quem pergunta que o recurso existe — e a existência
/// de uma hipótese já revela o que se está a investigar (ADR-0100).
fn ausente(que: &str) -> CoreError {
    CoreError::NotFound(format!("{que} não encontrado."))
}

// ── Hipóteses ───────────────────────────────────────────────────────────

/// Load one hypothesis, with the workspace that governs it.
///
/// # Errors
///
/// Returns [`CoreError::NotFound`] when absent or not readable.
pub async fn get_hypothesis(
    pool: &PgPool,
    principal: &Principal,
    id: Uuid,
) -> CoreResult<(Hypothesis, ResearchWorkspace)> {
    let hypothesis = repo::find_hypothesis(pool, id, principal.organisation_id)
        .await?
        .ok_or_else(|| ausente("Hipótese"))?;
    let workspace_id = hypothesis.workspace_id.ok_or_else(|| ausente("Hipótese"))?;
    let workspace = readable_artefact_workspace(
        pool,
        principal,
        workspace_id,
        ResourceKind::Hypothesis,
        hypothesis.classification(),
    )
    .await?;
    Ok((hypothesis, workspace))
}

/// State a hypothesis inside a research environment.
///
/// # Errors
///
/// Returns an error when the environment is not reachable, when the caller may
/// not create in it, or when the statement is empty.
pub async fn create_hypothesis(
    tx: &mut Tx<'_>,
    principal: &Principal,
    ids: &CorrelationIds,
    workspace_id: Uuid,
    statement: &str,
    rationale: Option<&str>,
    classification: Classification,
) -> CoreResult<Hypothesis> {
    if statement.trim().is_empty() {
        return Err(CoreError::Validation(
            "Uma hipótese precisa de uma afirmação.".to_owned(),
        ));
    }

    let workspace = get_workspace(&mut **tx, principal, workspace_id).await?;
    let ctx = artefact_context(&workspace, ResourceKind::Hypothesis, classification);
    authorize(principal, Action::Create, &ctx)
        .map_err(|(denial, decision)| CoreError::from_denial(denial, &decision))?;

    let hypothesis = repo::insert_hypothesis(
        &mut **tx,
        principal.organisation_id,
        workspace.unit_id,
        Some(workspace.id),
        // O ambiente não carrega projecto: a ligação a um projecto é uma
        // relação, e as relações vivem na proveniência.
        None,
        statement.trim(),
        rationale.map(str::trim).filter(|r| !r.is_empty()),
        classification.as_str(),
        principal.person_id,
    )
    .await?;

    audit::record(
        tx,
        Some(principal),
        ids,
        AuditEntry::new(action::CREATE, "hypothesis")
            .resource(hypothesis.id)
            .context(&ctx),
    )
    .await?;

    Ok(hypothesis)
}

/// The hypotheses of a research environment.
///
/// # Errors
///
/// Returns an error when the environment is not reachable.
pub async fn list_hypotheses(
    pool: &PgPool,
    principal: &Principal,
    workspace_id: Uuid,
) -> CoreResult<Vec<Hypothesis>> {
    let workspace = get_workspace(pool, principal, workspace_id).await?;
    repo::list_hypotheses(pool, workspace.id).await
}

// ── Metodologias ────────────────────────────────────────────────────────

/// The methodologies of a research environment.
///
/// # Errors
///
/// Returns an error when the environment is not reachable.
pub async fn list_methodologies(
    pool: &PgPool,
    principal: &Principal,
    workspace_id: Uuid,
) -> CoreResult<Vec<Methodology>> {
    let workspace = get_workspace(pool, principal, workspace_id).await?;
    repo::list_methodologies(pool, workspace.id).await
}

/// Load one methodology, with the workspace that governs it.
///
/// # Errors
///
/// Returns [`CoreError::NotFound`] when absent or not readable.
pub async fn get_methodology(
    pool: &PgPool,
    principal: &Principal,
    id: Uuid,
) -> CoreResult<(Methodology, ResearchWorkspace)> {
    let methodology = repo::find_methodology(pool, id, principal.organisation_id)
        .await?
        .ok_or_else(|| ausente("Metodologia"))?;
    let workspace_id = methodology
        .workspace_id
        .ok_or_else(|| ausente("Metodologia"))?;
    let workspace = readable_artefact_workspace(
        pool,
        principal,
        workspace_id,
        ResourceKind::Methodology,
        methodology.classification(),
    )
    .await?;
    Ok((methodology, workspace))
}

/// Load one methodology version, with what governs it.
///
/// A version is governed by its methodology: it has no classification of its
/// own, because a version that could be less strict than the method it
/// describes would be a way around the method's own classification.
///
/// # Errors
///
/// Returns [`CoreError::NotFound`] when absent or not readable.
pub async fn get_methodology_version(
    pool: &PgPool,
    principal: &Principal,
    id: Uuid,
) -> CoreResult<(MethodologyVersion, Methodology, ResearchWorkspace)> {
    let (version, methodology) = repo::find_version(pool, id, principal.organisation_id)
        .await?
        .ok_or_else(|| ausente("Versão da metodologia"))?;
    let workspace_id = methodology
        .workspace_id
        .ok_or_else(|| ausente("Versão da metodologia"))?;
    let workspace = readable_artefact_workspace(
        pool,
        principal,
        workspace_id,
        ResourceKind::MethodologyVersion,
        methodology.classification(),
    )
    .await?;
    Ok((version, methodology, workspace))
}

/// Create a methodology.
///
/// # Errors
///
/// Returns an error when the environment is not reachable or the title is
/// empty.
pub async fn create_methodology(
    tx: &mut Tx<'_>,
    principal: &Principal,
    ids: &CorrelationIds,
    workspace_id: Uuid,
    title: &str,
    purpose: Option<&str>,
    classification: Classification,
) -> CoreResult<Methodology> {
    if title.trim().is_empty() {
        return Err(CoreError::Validation(
            "Uma metodologia precisa de um título.".to_owned(),
        ));
    }

    let workspace = get_workspace(&mut **tx, principal, workspace_id).await?;
    let ctx = artefact_context(&workspace, ResourceKind::Methodology, classification);
    authorize(principal, Action::Create, &ctx)
        .map_err(|(denial, decision)| CoreError::from_denial(denial, &decision))?;

    let methodology = repo::insert_methodology(
        &mut **tx,
        principal.organisation_id,
        workspace.unit_id,
        Some(workspace.id),
        // O ambiente não carrega projecto: a ligação a um projecto é uma
        // relação, e as relações vivem na proveniência.
        None,
        title.trim(),
        purpose.map(str::trim).filter(|p| !p.is_empty()),
        classification.as_str(),
        principal.person_id,
    )
    .await?;

    audit::record(
        tx,
        Some(principal),
        ids,
        AuditEntry::new(action::CREATE, "methodology")
            .resource(methodology.id)
            .context(&ctx),
    )
    .await?;

    Ok(methodology)
}

/// Publish the next version of a methodology.
///
/// # Porque uma versão nova e nunca uma alteração
///
/// Porque a versão anterior pode já estar referenciada por proveniência. Se o
/// seu conteúdo pudesse mudar, a linhagem passaria a descrever outra coisa —
/// sem que ninguém alterasse uma aresta, e sem que nada o dissesse. Corrigir
/// um método é publicar; a versão corrigida é outra.
///
/// # Errors
///
/// Returns an error when the methodology is not reachable, when the caller may
/// not write to it, or when the summary is empty.
#[allow(clippy::too_many_arguments)]
pub async fn publish_methodology_version(
    tx: &mut Tx<'_>,
    pool: &PgPool,
    principal: &Principal,
    ids: &CorrelationIds,
    methodology_id: Uuid,
    label: &str,
    summary: &str,
    document_id: Option<Uuid>,
) -> CoreResult<MethodologyVersion> {
    if summary.trim().is_empty() {
        return Err(CoreError::Validation(
            "Uma versão precisa de dizer o que muda.".to_owned(),
        ));
    }

    let (methodology, workspace) = get_methodology(pool, principal, methodology_id).await?;
    let ctx = artefact_context(
        &workspace,
        ResourceKind::MethodologyVersion,
        methodology.classification(),
    );
    authorize(principal, Action::Create, &ctx)
        .map_err(|(denial, decision)| CoreError::from_denial(denial, &decision))?;

    // A que estava em vigor, antes de a nova existir.
    //
    // Lida dentro do `tx`, porque duas publicações concorrentes têm de ver a
    // mesma coisa: ler pelo `pool` deixaria as duas encontrarem a mesma
    // anterior e substituí-la duas vezes, e a metodologia ficaria com duas
    // versões a dizerem que estão em vigor.
    let anterior = repo::find_version_in_force(&mut **tx, methodology.id).await?;

    let version = repo::insert_version(
        &mut **tx,
        methodology.id,
        label.trim(),
        summary.trim(),
        document_id,
        principal.person_id,
    )
    .await?;

    // ── Substituir é o que publicar significa ───────────────────────────
    //
    // Sem isto, `superseded_by_id` ficava sempre nulo, a relação `Supersedes`
    // da matriz nunca era usada, e o ecrã mostrava todas as versões como
    // publicadas — o que é falso: só uma está em vigor. Uma pessoa a escolher
    // «a versão publicada» via duas e não tinha como saber qual.
    //
    // A versão anterior **fica**. Não se apaga o que a proveniência já cita:
    // um resultado produzido com ela continua a dizer que foi com ela.
    if let Some(anterior) = anterior {
        repo::supersede_version(&mut **tx, anterior.id, version.id).await?;
        crate::modules::knowledge::record_operation_provenance(
            tx,
            principal.organisation_id,
            Some(workspace.id),
            ocinye_contracts::agentic::ResourceKind::MethodologyVersion,
            version.id,
            ocinye_contracts::provenance::ProvenanceRelation::Supersedes,
            ocinye_contracts::agentic::ResourceKind::MethodologyVersion,
            anterior.id,
            principal.person_id,
        )
        .await?;
    }

    audit::record(
        tx,
        Some(principal),
        ids,
        AuditEntry::new(action::CREATE, "methodology_version")
            .resource(version.id)
            .context(&ctx)
            .detail("label", version.label.clone()),
    )
    .await?;

    Ok(version)
}

/// The versions of a methodology, newest first.
///
/// # Errors
///
/// Returns an error when the methodology is not reachable.
pub async fn list_methodology_versions(
    pool: &PgPool,
    principal: &Principal,
    methodology_id: Uuid,
) -> CoreResult<Vec<MethodologyVersion>> {
    let (methodology, _) = get_methodology(pool, principal, methodology_id).await?;
    repo::list_versions(pool, methodology.id).await
}

// ── Estudos ─────────────────────────────────────────────────────────────

/// Load one study, with the workspace that governs it.
///
/// # Errors
///
/// Returns [`CoreError::NotFound`] when absent or not readable.
pub async fn get_study(
    pool: &PgPool,
    principal: &Principal,
    id: Uuid,
) -> CoreResult<(Study, ResearchWorkspace)> {
    let study = repo::find_study(pool, id, principal.organisation_id)
        .await?
        .ok_or_else(|| ausente("Estudo"))?;
    let workspace_id = study.workspace_id.ok_or_else(|| ausente("Estudo"))?;
    let workspace = readable_artefact_workspace(
        pool,
        principal,
        workspace_id,
        ResourceKind::Study,
        study.classification(),
    )
    .await?;
    Ok((study, workspace))
}

/// The kinds a study may be.
///
/// Fechado, e verificado aqui além do `CHECK` da base: um valor desconhecido
/// que chegasse do cliente seria rejeitado pela base com um erro de
/// integridade, e um erro de integridade não é uma mensagem que se mostre a
/// quem escreveu um formulário.
const STUDY_KINDS: [&str; 3] = ["physical_experiment", "simulation", "analysis"];

/// Os estados por que uma execução passa.
///
/// Escrito aqui e não só no `CHECK` da migration porque a base recusa com o
/// texto de um constraint — «violates check constraint
/// `ck_study_executions_status`» — e isso chega ao caller como avaria, não como
/// recusa. Um estado desconhecido é um erro de quem chama, e tem de o saber
/// numa frase que diga quais são os estados.
const EXECUTION_STATUSES: [&str; 5] = ["recorded", "running", "succeeded", "failed", "aborted"];

/// Design a study inside a research environment.
///
/// # Errors
///
/// Returns an error when the environment or the hypothesis is not reachable,
/// or when the kind is not one the domain knows.
#[allow(clippy::too_many_arguments)]
pub async fn create_study(
    tx: &mut Tx<'_>,
    pool: &PgPool,
    principal: &Principal,
    ids: &CorrelationIds,
    workspace_id: Uuid,
    hypothesis_id: Option<Uuid>,
    methodology_version_id: Option<Uuid>,
    title: &str,
    kind: &str,
    objective: Option<&str>,
    classification: Classification,
) -> CoreResult<Study> {
    if title.trim().is_empty() {
        return Err(CoreError::Validation(
            "Um estudo precisa de um título.".to_owned(),
        ));
    }
    if !STUDY_KINDS.contains(&kind) {
        return Err(CoreError::Validation(format!(
            "«{kind}» não é um género de estudo. Os géneros são: {}.",
            STUDY_KINDS.join(", ")
        )));
    }

    let workspace = get_workspace(&mut **tx, principal, workspace_id).await?;
    let ctx = artefact_context(&workspace, ResourceKind::Study, classification);
    authorize(principal, Action::Create, &ctx)
        .map_err(|(denial, decision)| CoreError::from_denial(denial, &decision))?;

    // A hipótese que se testa tem de ser alcançável por quem desenha o estudo.
    // Sem isto, um identificador conhecido ligava um estudo a uma hipótese de
    // outra unidade — e a listagem dessa unidade passaria a mostrar um estudo
    // que ninguém lá dentro criou.
    if let Some(hypothesis_id) = hypothesis_id {
        get_hypothesis(pool, principal, hypothesis_id).await?;
    }

    // A versão, e não a metodologia.
    //
    // É resolvida com a política de quem cria — um identificador nomeia
    // âmbito, não o concede — e é a **versão** que fica na aresta: um estudo
    // que seguiu a versão 2 continua a dizer «versão 2» depois de a 5 existir.
    if let Some(methodology_version_id) = methodology_version_id {
        get_methodology_version(pool, principal, methodology_version_id).await?;
    }

    let study = repo::insert_study(
        &mut **tx,
        principal.organisation_id,
        workspace.unit_id,
        Some(workspace.id),
        // O ambiente não carrega projecto: a ligação a um projecto é uma
        // relação, e as relações vivem na proveniência.
        None,
        hypothesis_id,
        title.trim(),
        kind,
        objective.map(str::trim).filter(|o| !o.is_empty()),
        classification.as_str(),
        principal.person_id,
    )
    .await?;

    // ── E a proveniência que esta operação observou ─────────────────────
    //
    // `hypothesis_id` era uma coluna e mais nada. Uma coluna responde «que
    // hipótese?» a quem já tem o estudo à frente; não põe o estudo na
    // linhagem da hipótese, e uma travessia a montante a partir do resultado
    // parava na execução. A cadeia científica não é navegável por colunas.
    //
    // `origin = operation`: não foi ninguém que afirmou que este estudo testa
    // aquela hipótese — foi este acto que o estabeleceu.
    if let Some(hypothesis_id) = hypothesis_id {
        crate::modules::knowledge::record_operation_provenance(
            tx,
            principal.organisation_id,
            Some(workspace.id),
            ocinye_contracts::agentic::ResourceKind::Study,
            study.id,
            ocinye_contracts::provenance::ProvenanceRelation::Tests,
            ocinye_contracts::agentic::ResourceKind::Hypothesis,
            hypothesis_id,
            principal.person_id,
        )
        .await?;
    }

    if let Some(methodology_version_id) = methodology_version_id {
        crate::modules::knowledge::record_operation_provenance(
            tx,
            principal.organisation_id,
            Some(workspace.id),
            ocinye_contracts::agentic::ResourceKind::Study,
            study.id,
            ocinye_contracts::provenance::ProvenanceRelation::Follows,
            ocinye_contracts::agentic::ResourceKind::MethodologyVersion,
            methodology_version_id,
            principal.person_id,
        )
        .await?;
    }

    audit::record(
        tx,
        Some(principal),
        ids,
        AuditEntry::new(action::CREATE, "study")
            .resource(study.id)
            .context(&ctx)
            .detail("kind", kind),
    )
    .await?;

    Ok(study)
}

/// The studies of a research environment.
///
/// # Errors
///
/// Returns an error when the environment is not reachable.
pub async fn list_studies(
    pool: &PgPool,
    principal: &Principal,
    workspace_id: Uuid,
) -> CoreResult<Vec<Study>> {
    let workspace = get_workspace(pool, principal, workspace_id).await?;
    repo::list_studies(pool, workspace.id).await
}

// ── Execuções ───────────────────────────────────────────────────────────

/// Load one execution, with the study and workspace that govern it.
///
/// # Errors
///
/// Returns [`CoreError::NotFound`] when absent or not readable.
pub async fn get_execution(
    pool: &PgPool,
    principal: &Principal,
    id: Uuid,
) -> CoreResult<(StudyExecution, Study, ResearchWorkspace)> {
    let (execution, study) = repo::find_execution(pool, id, principal.organisation_id)
        .await?
        .ok_or_else(|| ausente("Execução"))?;
    let workspace_id = study.workspace_id.ok_or_else(|| ausente("Execução"))?;
    let workspace = readable_artefact_workspace(
        pool,
        principal,
        workspace_id,
        ResourceKind::StudyExecution,
        study.classification(),
    )
    .await?;
    Ok((execution, study, workspace))
}

/// What a run of a study recorded about itself.
#[derive(Debug, Default, Clone)]
pub struct ExecutionRecord<'a> {
    /// Where it stands.
    pub status: &'a str,
    /// The compute node it ran on, when it ran on one the Ocinye knows.
    pub compute_node_id: Option<Uuid>,
    /// A free description of where it ran, when there is no node.
    pub environment: Option<&'a str>,
    /// The software that ran it.
    pub software_name: Option<&'a str>,
    /// Its version.
    pub software_version: Option<&'a str>,
    /// Its commit.
    pub software_commit: Option<&'a str>,
    /// Anything else worth recording.
    pub notes: Option<&'a str>,
    /// A versão exacta da metodologia que esta corrida seguiu.
    ///
    /// A versão, e nunca a metodologia: é o que torna a proveniência estável
    /// no tempo. Entra como aresta na mesma transacção, com
    /// `origin = operation`, porque foi este acto que a estabeleceu.
    pub methodology_version_id: Option<Uuid>,
    /// As versões de dataset que entraram nesta corrida.
    ///
    /// Versões, e não datasets. Um dataset cresce; uma corrida consumiu o que
    /// existia naquele dia, e é isso que a linhagem tem de dizer.
    pub dataset_version_ids: &'a [Uuid],
}

/// Record a run of a study.
///
/// # Errors
///
/// Returns an error when the study is not reachable or the caller may not
/// write to it.
pub async fn record_execution(
    tx: &mut Tx<'_>,
    pool: &PgPool,
    principal: &Principal,
    ids: &CorrelationIds,
    study_id: Uuid,
    record: &ExecutionRecord<'_>,
) -> CoreResult<StudyExecution> {
    let (study, workspace) = get_study(pool, principal, study_id).await?;
    let ctx = artefact_context(
        &workspace,
        ResourceKind::StudyExecution,
        study.classification(),
    );
    authorize(principal, Action::Create, &ctx)
        .map_err(|(denial, decision)| CoreError::from_denial(denial, &decision))?;

    let status = if record.status.is_empty() {
        "recorded"
    } else {
        record.status
    };
    if !EXECUTION_STATUSES.contains(&status) {
        return Err(CoreError::Validation(format!(
            "«{status}» não é um estado de execução. Os estados são: {}.",
            EXECUTION_STATUSES.join(", ")
        )));
    }

    let execution = repo::insert_execution(
        &mut **tx,
        principal.organisation_id,
        study.id,
        status,
        record.compute_node_id,
        record.environment,
        record.software_name,
        record.software_version,
        record.software_commit,
        record.notes,
        principal.person_id,
    )
    .await?;

    // ── A proveniência que esta corrida estabeleceu ─────────────────────
    //
    // Cada recurso é resolvido com a política de quem regista, antes de
    // qualquer aresta: um identificador nomeia âmbito e não o concede, e uma
    // aresta para um recurso inalcançável seria uma fuga com outro nome.
    if let Some(methodology_version_id) = record.methodology_version_id {
        get_methodology_version(pool, principal, methodology_version_id).await?;
        crate::modules::knowledge::record_operation_provenance(
            tx,
            principal.organisation_id,
            Some(workspace.id),
            ocinye_contracts::agentic::ResourceKind::StudyExecution,
            execution.id,
            ocinye_contracts::provenance::ProvenanceRelation::Follows,
            ocinye_contracts::agentic::ResourceKind::MethodologyVersion,
            methodology_version_id,
            principal.person_id,
        )
        .await?;
    }

    for dataset_version_id in record.dataset_version_ids {
        crate::resources::resolve(
            pool,
            principal,
            &ocinye_contracts::agentic::ResourceRef {
                kind: ocinye_contracts::agentic::ResourceKind::DatasetVersion,
                id: *dataset_version_id,
                label: None,
            },
        )
        .await?;

        // A direcção segue a matriz: os dados entram **na** execução.
        crate::modules::knowledge::record_operation_provenance(
            tx,
            principal.organisation_id,
            Some(workspace.id),
            ocinye_contracts::agentic::ResourceKind::DatasetVersion,
            *dataset_version_id,
            ocinye_contracts::provenance::ProvenanceRelation::InputTo,
            ocinye_contracts::agentic::ResourceKind::StudyExecution,
            execution.id,
            principal.person_id,
        )
        .await?;
    }

    audit::record(
        tx,
        Some(principal),
        ids,
        AuditEntry::new(action::CREATE, "study_execution")
            .resource(execution.id)
            .context(&ctx)
            .detail("sequence", i64::from(execution.sequence)),
    )
    .await?;

    Ok(execution)
}

/// The runs of a study, newest first.
///
/// # Errors
///
/// Returns an error when the study is not reachable.
pub async fn list_executions(
    pool: &PgPool,
    principal: &Principal,
    study_id: Uuid,
) -> CoreResult<Vec<StudyExecution>> {
    let (study, _) = get_study(pool, principal, study_id).await?;
    repo::list_executions(pool, study.id).await
}

// ── Resultados ──────────────────────────────────────────────────────────

/// Load one result, with the workspace that governs it.
///
/// # Errors
///
/// Returns [`CoreError::NotFound`] when absent or not readable.
pub async fn get_result(
    pool: &PgPool,
    principal: &Principal,
    id: Uuid,
) -> CoreResult<(ScientificResult, ResearchWorkspace)> {
    let result = repo::find_result(pool, id, principal.organisation_id)
        .await?
        .ok_or_else(|| ausente("Resultado"))?;
    let workspace_id = result.workspace_id.ok_or_else(|| ausente("Resultado"))?;
    let workspace = readable_artefact_workspace(
        pool,
        principal,
        workspace_id,
        ResourceKind::Result,
        result.classification(),
    )
    .await?;
    Ok((result, workspace))
}

/// Record a result, and the provenance the operation already knows.
///
/// # Porque a proveniência entra na mesma transacção
///
/// Porque a operação **conhece** a relação: dizer «este resultado veio desta
/// execução» não é uma inferência a confirmar depois — é o que acabou de ser
/// pedido. Escrever o resultado e adiar a aresta produziria, no dia em que a
/// segunda escrita falhasse, um resultado sem origem que aparenta ter uma.
///
/// E um resultado sem origem é pior do que nenhum resultado: aparece na
/// listagem, abre-se, e a secção de proveniência está vazia — o que se lê como
/// «não foi registada» e não como «perdeu-se».
///
/// A relação obrigatória é uma só: `Result --produced_by--> StudyExecution`,
/// quando há execução. Tudo o resto — que dados entraram, que metodologia se
/// seguiu — é declarado, porque a operação não o sabe.
///
/// # Errors
///
/// Returns an error when the environment or the execution is not reachable,
/// or when the title or summary are empty. Nada é escrito nesse caso.
#[allow(clippy::too_many_arguments)]
pub async fn create_result(
    tx: &mut Tx<'_>,
    pool: &PgPool,
    principal: &Principal,
    ids: &CorrelationIds,
    workspace_id: Uuid,
    execution_id: Option<Uuid>,
    title: &str,
    summary: &str,
    classification: Classification,
) -> CoreResult<ScientificResult> {
    if title.trim().is_empty() || summary.trim().is_empty() {
        return Err(CoreError::Validation(
            "Um resultado precisa de um título e do que diz.".to_owned(),
        ));
    }

    let workspace = get_workspace(&mut **tx, principal, workspace_id).await?;
    let ctx = artefact_context(&workspace, ResourceKind::Result, classification);
    authorize(principal, Action::Create, &ctx)
        .map_err(|(denial, decision)| CoreError::from_denial(denial, &decision))?;

    // A execução tem de ser alcançável por quem regista o resultado. Sem isto,
    // um identificador conhecido atribuiria um resultado a uma execução de
    // outra unidade — e a linhagem dessa unidade passaria a mostrar um
    // resultado que ninguém lá dentro registou.
    if let Some(execution_id) = execution_id {
        get_execution(pool, principal, execution_id).await?;
    }

    let result = repo::insert_result(
        &mut **tx,
        principal.organisation_id,
        workspace.unit_id,
        Some(workspace.id),
        None,
        execution_id,
        title.trim(),
        summary.trim(),
        classification.as_str(),
        principal.person_id,
    )
    .await?;

    // ── E a origem, no mesmo `tx` ───────────────────────────────────────
    //
    // `origin = operation`: não foi ninguém que afirmou a relação, foi a
    // operação que a produziu. É a distinção que separa o que o sistema
    // observou do que uma pessoa declarou — e a rota de declaração manual não
    // pode escrever este valor.
    if let Some(execution_id) = execution_id {
        crate::modules::knowledge::record_operation_provenance(
            tx,
            principal.organisation_id,
            Some(workspace.id),
            ocinye_contracts::agentic::ResourceKind::Result,
            result.id,
            ocinye_contracts::provenance::ProvenanceRelation::ProducedBy,
            ocinye_contracts::agentic::ResourceKind::StudyExecution,
            execution_id,
            principal.person_id,
        )
        .await?;
    }

    audit::record(
        tx,
        Some(principal),
        ids,
        AuditEntry::new(action::CREATE, "result")
            .resource(result.id)
            .context(&ctx),
    )
    .await?;

    Ok(result)
}

/// Record that somebody validated or reproduced a result.
///
/// # Reprodutibilidade é evidência
///
/// O desfecho é obrigatório e não tem valor por omissão que signifique
/// sucesso. Um resultado não fica reproduzido porque alguém registou a
/// intenção de o reproduzir: fica reproduzido quando existe outra execução e
/// alguém escreveu o que ela mostrou — incluindo quando mostrou o contrário.
///
/// # Errors
///
/// Returns an error when the result or the execution is not reachable, or when
/// the kind or outcome are not ones the domain knows.
#[allow(clippy::too_many_arguments)]
pub async fn record_validation(
    tx: &mut Tx<'_>,
    pool: &PgPool,
    principal: &Principal,
    ids: &CorrelationIds,
    result_id: Uuid,
    kind: &str,
    outcome: &str,
    execution_id: Option<Uuid>,
    note: Option<&str>,
) -> CoreResult<ResultValidation> {
    const KINDS: [&str; 2] = ["validation", "reproduction"];
    const OUTCOMES: [&str; 3] = ["confirmed", "contradicted", "inconclusive"];

    if !KINDS.contains(&kind) {
        return Err(CoreError::Validation(
            "Uma verificação é uma validação ou uma reprodução.".to_owned(),
        ));
    }
    if !OUTCOMES.contains(&outcome) {
        return Err(CoreError::Validation(
            "Um desfecho é: confirmou, contradisse, ou foi inconclusivo.".to_owned(),
        ));
    }

    // Uma reprodução precisa da execução que a reproduziu.
    //
    // Reprodutibilidade é evidência, e não um rótulo. Um resultado não fica
    // reproduzido porque alguém escreveu que o reproduziu: fica reproduzido
    // quando existe outra corrida e alguém registou o que ela mostrou. Sem
    // esta regra, `kind = "reproduction"` seria uma palavra que qualquer
    // pessoa podia escrever sobre qualquer coisa.
    //
    // Uma validação é outra coisa e não exige execução: quem valida pode
    // estar a ler o resultado contra o que já se sabe, sem correr nada.
    if kind == "reproduction" && execution_id.is_none() {
        return Err(CoreError::Validation(
            "Uma reprodução precisa da execução que a reproduziu. Sem ela é uma \
             afirmação, e não uma reprodução."
                .to_owned(),
        ));
    }

    let (result, workspace) = get_result(pool, principal, result_id).await?;
    let ctx = artefact_context(&workspace, ResourceKind::Result, result.classification());

    // Duas portas, e as duas fecham.
    //
    // `Action::Update` diz que esta pessoa pode escrever neste ambiente e
    // alcança esta classificação. Não chega: escrever no ambiente é o que
    // qualquer membro faz, e afirmar que um resultado se confirma é outra
    // coisa — é dizer o que a instituição sabe.
    //
    // `ResultsValidate` é a permissão que separa as duas, e vive nos papéis
    // de liderança de ambiente e de gestão de unidade. Sem esta verificação a
    // permissão existiria no catálogo e não governaria nada, e a distinção
    // ficaria a viver só no botão que o Workspace mostra — que é o cliente a
    // decidir, e o cliente nunca decide (`CLAUDE.md` §4).
    if !can(
        principal,
        Permission::ResultsValidate,
        &ctx,
        Some(result.id),
    )
    .allowed
    {
        return Err(CoreError::PermissionDenied(
            "Validar ou dar por reproduzido um resultado exige liderança do ambiente \
             ou gestão da unidade."
                .to_owned(),
        ));
    }

    authorize(principal, Action::Update, &ctx)
        .map_err(|(denial, decision)| CoreError::from_denial(denial, &decision))?;

    if let Some(execution_id) = execution_id {
        get_execution(pool, principal, execution_id).await?;
    }

    let validation = repo::insert_validation(
        &mut **tx,
        principal.organisation_id,
        result.id,
        kind,
        outcome,
        execution_id,
        None,
        note,
        principal.person_id,
    )
    .await?;

    // Um resultado confirmado passa a validado. Contradito ou inconclusivo
    // **não** muda de estado por si: quem contradiz um resultado não decide
    // sozinho que ele está errado, e o estado dele é uma afirmação
    // institucional que se toma com o resto da evidência à frente.
    if kind == "validation" && outcome == "confirmed" && result.status == "draft" {
        repo::set_result_status(&mut **tx, result.id, "validated").await?;
    }

    audit::record(
        tx,
        Some(principal),
        ids,
        AuditEntry::new(action::UPDATE, "result")
            .resource(result.id)
            .context(&ctx)
            .detail("validation", kind.to_owned())
            .detail("outcome", outcome.to_owned()),
    )
    .await?;

    Ok(validation)
}

/// The results of a research environment.
///
/// # Errors
///
/// Returns an error when the environment is not reachable.
pub async fn list_results(
    pool: &PgPool,
    principal: &Principal,
    workspace_id: Uuid,
) -> CoreResult<Vec<ScientificResult>> {
    let workspace = get_workspace(pool, principal, workspace_id).await?;
    repo::list_results(pool, workspace.id).await
}

/// What has been said about a result.
///
/// # Errors
///
/// Returns an error when the result is not reachable.
pub async fn list_validations(
    pool: &PgPool,
    principal: &Principal,
    result_id: Uuid,
) -> CoreResult<Vec<ResultValidation>> {
    let (result, _) = get_result(pool, principal, result_id).await?;
    repo::list_validations(pool, result.id).await
}
