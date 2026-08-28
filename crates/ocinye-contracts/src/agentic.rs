//! Agentic Control Plane contracts.
//!
//! # The distinction this file exists to hold
//!
//! > **Agents understand, plan and orchestrate. The Ocinye Core authorises,
//! > executes, persists and verifies.**
//!
//! Everything here is a *proposal* or a *description*. Nothing here executes,
//! and nothing here grants. A [`CapabilityRequest`] is what an agent asks for;
//! whether it happens is decided by the Core, against the acting person's own
//! authority.
//!
//! # Capability, not `SystemCapability`
//!
//! [`SystemCapability`](crate::SystemCapability) answers *can this installation
//! do X* — is mail configured, is there an AI node. A [`CapabilityId`] here
//! answers *what may an agent ask the Core to do* — create a folder, draft a
//! reply. Two different questions; the names were separated deliberately.
//!
//! # Model output is never system state
//!
//! A model can emit the words "the project was created". That is text. Only a
//! [`CapabilityResult`] returned by the Core means anything happened.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{Classification, Permission, Scope};

// ── Identity ────────────────────────────────────────────────────────────

/// The stable identifier of a capability.
///
/// Dotted, domain-first, and **stable**: it appears in audit rows, in agent
/// definitions and in approvals. Renaming one is a breaking change to
/// everything that referenced it.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CapabilityId(String);

impl CapabilityId {
    /// Build an identifier from a static string.
    ///
    /// `const`-friendly construction is deliberately not offered: identifiers
    /// come from the registry, and inventing one at a call site is how a
    /// capability ends up executing without a descriptor.
    #[must_use]
    pub fn new(value: &'static str) -> Self {
        Self(value.to_owned())
    }

    /// Parse an identifier a model or client proposed.
    ///
    /// Shape is checked here; **existence is not**. A well-formed identifier
    /// for a capability that does not exist is refused by the registry, which
    /// is the only thing that knows what exists (briefing §161).
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        let value = value.trim();

        let plausible = !value.is_empty()
            && value.len() <= 64
            && value.contains('.')
            && value
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '.' || c == '_');

        plausible.then(|| Self(value.to_owned()))
    }

    /// The identifier as text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The domain half, before the first dot.
    #[must_use]
    pub fn domain(&self) -> &str {
        self.0.split('.').next().unwrap_or(&self.0)
    }
}

impl std::fmt::Display for CapabilityId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// A reference to something in the institution.
///
/// # Why agents never pass names
///
/// «the BESS project» is a phrase. Two projects can share it, none may match
/// it, and a model can invent one. A `ResourceRef` names a row, and the Core
/// checks that the acting person may reach that row before anything happens
/// (briefing §41, §160).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResourceRef {
    /// What kind of thing this is.
    pub kind: ResourceKind,
    /// Which one.
    pub id: Uuid,
    /// A human label, for showing in a plan. **Never used to resolve.**
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

/// The kinds of thing an agent can refer to.
///
/// A closed set. A model that names something outside it has named nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResourceKind {
    /// An exploratory idea.
    Idea,
    /// A formal project.
    Project,
    /// A research workspace.
    Workspace,
    /// A scientific unit.
    Unit,
    /// A person.
    Person,
    /// A bibliographic source.
    Source,
    /// A note.
    Note,
    /// A document.
    Document,
    /// A dataset.
    Dataset,
    /// A task.
    Task,
    /// Um compromisso do calendário.
    CalendarEvent,
    /// Um lembrete.
    Reminder,
    /// A mail message.
    MailMessage,
    /// A mail draft.
    MailDraft,
    /// A mailbox.
    Mailbox,
    /// Uma conversa das Mensagens.
    Conversation,
    /// Uma mensagem dentro de uma conversa.
    Message,
    /// An agent definition.
    Agent,
    /// A compute node.
    ComputeNode,
    /// A compute job.
    ComputeJob,
    /// Uma hipótese científica.
    Hypothesis,
    /// Uma metodologia, com identidade própria.
    Methodology,
    /// Uma **versão** de metodologia.
    ///
    /// Um recurso, e não um campo. É o que torna a proveniência honesta: um
    /// resultado produzido com a versão 2 continua a dizer «versão 2» depois
    /// de a versão 5 existir.
    MethodologyVersion,
    /// Uma **versão** de dataset. Pela mesma razão.
    DatasetVersion,
    /// Um estudo: experimento físico, simulação ou análise.
    Study,
    /// Uma execução concreta de um estudo.
    StudyExecution,
    /// Um resultado científico.
    Result,
}

impl ResourceKind {
    /// Stable representation.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Idea => "idea",
            Self::Project => "project",
            Self::Workspace => "workspace",
            Self::Unit => "unit",
            Self::Person => "person",
            Self::Source => "source",
            Self::Note => "note",
            Self::Document => "document",
            Self::Dataset => "dataset",
            Self::Task => "task",
            Self::CalendarEvent => "calendar_event",
            Self::Reminder => "reminder",
            Self::MailMessage => "mail_message",
            Self::MailDraft => "mail_draft",
            Self::Mailbox => "mailbox",
            Self::Conversation => "conversation",
            Self::Message => "message",
            Self::Agent => "agent",
            Self::ComputeNode => "compute_node",
            Self::ComputeJob => "compute_job",
            Self::Hypothesis => "hypothesis",
            Self::Methodology => "methodology",
            Self::MethodologyVersion => "methodology_version",
            Self::DatasetVersion => "dataset_version",
            Self::Study => "study",
            Self::StudyExecution => "study_execution",
            Self::Result => "result",
        }
    }

    /// Parse from the stable representation.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        Self::all().into_iter().find(|kind| kind.as_str() == value)
    }

    /// Every kind.
    #[must_use]
    pub const fn all() -> [Self; 25] {
        [
            Self::Idea,
            Self::Project,
            Self::Workspace,
            Self::Unit,
            Self::Person,
            Self::Source,
            Self::Note,
            Self::Document,
            Self::Dataset,
            Self::Task,
            Self::CalendarEvent,
            Self::Reminder,
            Self::MailMessage,
            Self::MailDraft,
            Self::Mailbox,
            Self::Agent,
            Self::ComputeNode,
            Self::ComputeJob,
            Self::Hypothesis,
            Self::Methodology,
            Self::MethodologyVersion,
            Self::DatasetVersion,
            Self::Study,
            Self::StudyExecution,
            Self::Result,
        ]
    }

    /// What a member calls it.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Idea => "Ideia",
            Self::Project => "Projecto",
            Self::Workspace => "Research Workspace",
            Self::Unit => "Unidade",
            Self::Person => "Pessoa",
            Self::Source => "Referência",
            Self::Note => "Nota",
            Self::Document => "Documento",
            Self::Dataset => "Dataset",
            Self::Task => "Tarefa",
            Self::CalendarEvent => "Compromisso",
            Self::Reminder => "Lembrete",
            Self::MailMessage => "Mensagem",
            Self::MailDraft => "Rascunho",
            Self::Mailbox => "Caixa de correio",
            Self::Conversation => "Conversa",
            Self::Message => "Mensagem",
            Self::Agent => "Agente",
            Self::ComputeNode => "Nó de computação",
            Self::ComputeJob => "Job",
            Self::Hypothesis => "Hipótese",
            Self::Methodology => "Metodologia",
            Self::MethodologyVersion => "Versão da metodologia",
            Self::DatasetVersion => "Versão do dataset",
            Self::Study => "Estudo",
            Self::StudyExecution => "Execução",
            Self::Result => "Resultado",
        }
    }
}

// ── Risk ────────────────────────────────────────────────────────────────

/// How much a capability can cost if it is wrong.
///
/// # Why this is a property of the capability and not of the request
///
/// Sending mail is externally visible whoever asks and whatever they write.
/// Letting the caller — or a model — declare the risk of its own request is how
/// a destructive action arrives labelled harmless (briefing §49).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RiskLevel {
    /// Reads. Changes nothing.
    ReadOnly,
    /// A small, reversible change. Creating a folder.
    LowImpact,
    /// A material institutional change. Archiving a project.
    MaterialMutation,
    /// Something leaves the institution, or reaches someone outside it.
    ExternalEffect,
    /// Privileged, security-sensitive, or irreversible.
    Privileged,
}

impl RiskLevel {
    /// Stable representation.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ReadOnly => "read_only",
            Self::LowImpact => "low_impact",
            Self::MaterialMutation => "material_mutation",
            Self::ExternalEffect => "external_effect",
            Self::Privileged => "privileged",
        }
    }

    /// Parse from the stable representation.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        Self::all().into_iter().find(|risk| risk.as_str() == value)
    }

    /// Every level, ascending.
    #[must_use]
    pub const fn all() -> [Self; 5] {
        [
            Self::ReadOnly,
            Self::LowImpact,
            Self::MaterialMutation,
            Self::ExternalEffect,
            Self::Privileged,
        ]
    }

    /// What a member reads in a plan.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::ReadOnly => "Consulta",
            Self::LowImpact => "Alteração menor",
            Self::MaterialMutation => "Alteração institucional",
            Self::ExternalEffect => "Efeito externo",
            Self::Privileged => "Privilegiada",
        }
    }

    /// Whether this level always needs a person to say yes.
    ///
    /// The rule, and not a default a capability may soften: anything that
    /// leaves the institution or touches privilege is confirmed, every time
    /// (briefing §50).
    #[must_use]
    pub const fn always_requires_approval(self) -> bool {
        matches!(self, Self::ExternalEffect | Self::Privileged)
    }

    /// Whether this level changes institutional state at all.
    #[must_use]
    pub const fn mutates(self) -> bool {
        !matches!(self, Self::ReadOnly)
    }
}

/// When a person has to confirm.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalRequirement {
    /// Never. Reads and trivially reversible writes.
    Never,
    /// When the acting person has not already confirmed this exact plan.
    Once,
    /// Every time, regardless of what was confirmed before.
    Always,
}

impl ApprovalRequirement {
    /// Stable representation.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Never => "never",
            Self::Once => "once",
            Self::Always => "always",
        }
    }

    /// Parse from the stable representation.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "never" => Self::Never,
            "once" => Self::Once,
            "always" => Self::Always,
            _ => return None,
        })
    }
}

// ── Autonomy ────────────────────────────────────────────────────────────

/// How far an agent may go without being asked again.
///
/// Ordered. An agent's level and a capability's ceiling are compared, and the
/// **lower** wins (briefing §69, §71).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AutonomyLevel {
    /// No AI at all.
    Manual,
    /// Explains and suggests. Changes nothing.
    Assist,
    /// Produces a draft the member edits. Changes nothing on its own.
    Compose,
    /// Executes one authorised capability.
    Act,
    /// Executes an approved multi-step plan.
    Workflow,
    /// Starts work nobody asked for.
    ///
    /// **Not reachable in this installation.** Kept in the type so that the
    /// ceiling comparisons are total and so that enabling it later is a
    /// deliberate, reviewable change rather than a new concept
    /// (briefing §70).
    Autonomous,
}

impl AutonomyLevel {
    /// Stable representation.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Manual => "manual",
            Self::Assist => "assist",
            Self::Compose => "compose",
            Self::Act => "act",
            Self::Workflow => "workflow",
            Self::Autonomous => "autonomous",
        }
    }

    /// Parse from the stable representation.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        Self::all()
            .into_iter()
            .find(|level| level.as_str() == value)
    }

    /// Every level, ascending.
    #[must_use]
    pub const fn all() -> [Self; 6] {
        [
            Self::Manual,
            Self::Assist,
            Self::Compose,
            Self::Act,
            Self::Workflow,
            Self::Autonomous,
        ]
    }

    /// What a member reads.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Manual => "Manual",
            Self::Assist => "Explica e sugere",
            Self::Compose => "Prepara rascunhos",
            Self::Act => "Executa uma acção autorizada",
            Self::Workflow => "Executa um plano aprovado",
            Self::Autonomous => "Autónomo",
        }
    }

    /// The highest level this installation permits.
    ///
    /// `Autonomous` is deliberately unreachable: an agent that starts work
    /// nobody asked for needs a policy, an owner and a way to stop it, and none
    /// of those is built (briefing §70, §145).
    #[must_use]
    pub const fn ceiling() -> Self {
        Self::Workflow
    }

    /// Whether this level may execute anything at all.
    #[must_use]
    pub const fn may_execute(self) -> bool {
        matches!(self, Self::Act | Self::Workflow | Self::Autonomous)
    }
}

// ── Capability descriptor ───────────────────────────────────────────────

/// A operação determinística do Core que uma capability executa.
///
/// # Porque isto existe
///
/// Sem ela, a pergunta «esta capability e aquele formulário terminam no mesmo
/// sítio?» não tinha resposta tipada: a ligação vivia dentro do handler, em
/// código, e duas implementações da mesma regra podiam divergir durante meses
/// sem nada acusar.
///
/// # Não chega ao modelo
///
/// É metadata interna do Ocinye OS. O contexto de inferência recebe
/// identificadores de capability e mais nada — nem operação, nem permissão, nem
/// risco. Risco, aprovação e disponibilidade são factos do Core, e não
/// informação de planeamento (ADR-0307).
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct OperationId(String);

impl OperationId {
    /// Nomeia uma operação do Core.
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// O nome estável.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for OperationId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// A fronteira de confiança que uma delegação atravessaria.
///
/// # Três, e não «operações perigosas»
///
/// Risco alto não fecha nada por si: enviar um email externo é de alto impacto e
/// continua endereçável. O que fecha é a **natureza** da fronteira.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TrustBoundary {
    /// A execução segura exige um segredo que não atravessa o plano agentic.
    ///
    /// Palavras-passe, credenciais temporárias, convites. O agente pode abrir o
    /// ecrã e explicar o que se segue; não recebe o segredo.
    SecretBoundary,
    /// O efeito principal muda quem pode exercer autoridade depois da operação.
    ///
    /// A diferença face a uma operação de alto impacto está no **depois**: um
    /// email enviado é um efeito, um papel concedido muda o que a pessoa passa a
    /// poder fazer a partir dali.
    ///
    /// Conteúdo recuperado não confiável não consegue autorizar nada — o Core
    /// impede — mas consegue induzir propostas até alguém confirmar uma por
    /// cansaço. Não publicar a capability elimina o vector inteiro.
    AuthorityBoundary,
    /// A execução segura depende de bytes que a pessoa escolhe.
    ///
    /// Um ficheiro atravessa o sistema por um caminho determinístico: selector,
    /// multipart, validação, normalização, armazenamento. Bytes, caminhos locais
    /// e URLs arbitrários não são entradas agentic.
    UserMediatedBinaryBoundary,
    /// O efeito é uma afirmação institucional cujo peso vem de quem a faz.
    ///
    /// As outras três fronteiras fecham-se por causa do que a operação
    /// **alcança**: um segredo, a autoridade de alguém, bytes que só a pessoa
    /// tem. Esta fecha-se por causa de quem a **assina**.
    ///
    /// Validar um resultado científico não muda o acesso de ninguém, não revela
    /// nada e não é difícil de desfazer — uma validação errada corrige-se com
    /// outra, e o domínio guarda as duas. O que não se desfaz é a atribuição:
    /// o registo diz que alguém afirmou aquilo, e é essa pessoa que lhe dá
    /// valor. Um agente a produzi-la deixaria a instituição com uma afirmação
    /// sem ninguém por trás.
    ///
    /// Não é risco. É autoria — e por isso não se resolve com aprovação: uma
    /// confirmação humana continuaria a deixar a afirmação escrita como se
    /// tivesse sido feita, e não assumida.
    InstitutionalClaimBoundary,
}

impl TrustBoundary {
    /// O rótulo estável, para a matriz e para os registos.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SecretBoundary => "SECRET_BOUNDARY",
            Self::AuthorityBoundary => "AUTHORITY_BOUNDARY",
            Self::UserMediatedBinaryBoundary => "USER_MEDIATED_BINARY_BOUNDARY",
            Self::InstitutionalClaimBoundary => "INSTITUTIONAL_CLAIM_BOUNDARY",
        }
    }

    /// Todas as fronteiras, para quem tem de as percorrer sem as esquecer.
    ///
    /// # Porque isto existe
    ///
    /// Porque o gerador da matriz iterava um array escrito ao lado, e quando a
    /// `INSTITUTIONAL_CLAIM_BOUNDARY` nasceu a secção de fronteiras passou a
    /// somar catorze de quinze — sem nada falhar. Uma lista mantida à parte do
    /// tipo que enumera é uma lista que fica para trás.
    #[must_use]
    pub const fn all() -> [Self; 4] {
        [
            Self::SecretBoundary,
            Self::AuthorityBoundary,
            Self::UserMediatedBinaryBoundary,
            Self::InstitutionalClaimBoundary,
        ]
    }
}

/// O que o plano agentic pode fazer com uma operação do Core.
///
/// # Três estados, e não quatro
///
/// Não existe «endereçável, capability por implementar». Se a operação existe e
/// é delegável, a capability faz parte da mesma passagem — caso contrário a
/// classificação passaria a ser uma lista de intenções, que é exactamente o
/// estado de que o ADR-0307 nos tirou.
///
/// E [`AgenticExposure::NotImplemented`] descreve o **Core**, não o plano
/// agentic: usa-se quando a operação ainda não existe, e nunca para adiar uma
/// capability sobre uma operação que já funciona.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "disposition", rename_all = "snake_case")]
pub enum AgenticExposure {
    /// Descobrível e invocável pelo plano agentic.
    Addressable {
        /// A capability que a executa.
        capability: CapabilityId,
    },
    /// Existe pela interface determinística, e não pode ser delegada.
    ///
    /// > **Non-delegability is determined by the nature of the trust boundary
    /// > crossed, not by risk level alone.**
    ///
    /// A fronteira é tipada porque a razão em texto livre obriga quem lê a
    /// inferir a classe, e inferir é onde as classificações se confundem umas
    /// com as outras.
    NonDelegable {
        /// Que fronteira de confiança a delegação atravessaria.
        boundary: TrustBoundary,
        /// Porquê, em concreto — não «segurança».
        reason: &'static str,
    },
    /// A operação ainda não existe no Core.
    NotImplemented {
        /// O que falta para existir.
        reason: &'static str,
    },
}

impl AgenticExposure {
    /// A capability que executa esta operação, se houver.
    #[must_use]
    pub const fn capability(&self) -> Option<&CapabilityId> {
        match self {
            Self::Addressable { capability } => Some(capability),
            _ => None,
        }
    }

    /// A fronteira atravessada, quando a operação não é delegável.
    #[must_use]
    pub const fn boundary(&self) -> Option<TrustBoundary> {
        match self {
            Self::NonDelegable { boundary, .. } => Some(*boundary),
            _ => None,
        }
    }

    /// A razão declarada, quando a disposição exige uma.
    #[must_use]
    pub const fn reason(&self) -> Option<&'static str> {
        match self {
            Self::NonDelegable { reason, .. } | Self::NotImplemented { reason } => Some(reason),
            Self::Addressable { .. } => None,
        }
    }

    /// Um rótulo estável, para a matriz e para os registos.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Addressable { .. } => "addressable",
            Self::NonDelegable { .. } => "non_delegable",
            Self::NotImplemented { .. } => "not_implemented",
        }
    }
}

/// Everything the Core publishes about one capability.
///
/// The registry hands these out; a model never sees a handler, a SQL statement
/// or a table name.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityDescriptor {
    /// Stable identifier.
    pub id: CapabilityId,
    /// A operação do Core que esta capability executa.
    ///
    /// Metadata interna: nunca chega ao modelo. Serve à paridade, à integridade
    /// do registry, aos testes e à observabilidade (ADR-0307).
    pub operation: OperationId,
    /// The domain that owns it.
    pub domain: String,
    /// What it does, in language a member could read.
    pub summary: String,
    /// The permission the acting person must hold.
    ///
    /// Necessary, never sufficient: classification, scope and domain
    /// invariants are checked as well.
    pub permission: Permission,
    /// Where it can apply.
    pub scope: Scope,
    /// What it costs if wrong.
    pub risk: RiskLevel,
    /// When a person must confirm.
    pub approval: ApprovalRequirement,
    /// The highest autonomy at which it may run.
    pub max_autonomy: AutonomyLevel,
    /// Whether the effects can be undone, and how.
    pub reversibility: Reversibility,
    /// Whether it can be asked to describe its effect without causing it.
    pub supports_dry_run: bool,
    /// The highest classification it may touch.
    ///
    /// `None` means it does not touch classified material at all.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub classification_ceiling: Option<Classification>,
    /// The shape of its input, as JSON Schema.
    ///
    /// Published so a model can be constrained, and so a proposed input can be
    /// validated **before** anything runs (briefing §174).
    pub input_schema: serde_json::Value,
}

impl CapabilityDescriptor {
    /// Whether this capability changes institutional state.
    #[must_use]
    pub const fn mutates(&self) -> bool {
        self.risk.mutates()
    }

    /// Whether a plan containing it needs confirmation.
    ///
    /// The risk level can force approval that the descriptor did not ask for.
    /// It cannot go the other way: a capability may be *more* cautious than its
    /// risk level, never less.
    #[must_use]
    pub const fn requires_approval(&self) -> bool {
        self.risk.always_requires_approval()
            || matches!(
                self.approval,
                ApprovalRequirement::Once | ApprovalRequirement::Always
            )
    }
}

/// Whether the effect of a capability can be undone.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Reversibility {
    /// Nothing to undo: it did not change anything.
    NothingToUndo,
    /// Undoable through the interface.
    Reversible,
    /// Undoable, but only by someone with more authority.
    ReversibleByAdministrator,
    /// Cannot be undone. A sent message; a deleted row.
    Irreversible,
}

impl Reversibility {
    /// Stable representation.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NothingToUndo => "nothing_to_undo",
            Self::Reversible => "reversible",
            Self::ReversibleByAdministrator => "reversible_by_administrator",
            Self::Irreversible => "irreversible",
        }
    }

    /// Whether an Undo affordance may be offered for this.
    ///
    /// Offering Undo where none exists is worse than not offering it: somebody
    /// acts on the belief that they can take it back (briefing §137).
    #[must_use]
    pub const fn may_offer_undo(self) -> bool {
        matches!(self, Self::Reversible)
    }
}

// ── Requests and results ────────────────────────────────────────────────

/// One capability, with the input an agent proposes for it.
///
/// **A proposal.** Constructing one performs nothing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityRequest {
    /// Which capability.
    pub capability: CapabilityId,
    /// Its input. Validated against the descriptor's schema before use.
    pub input: serde_json::Value,
    /// What it acts on, when it acts on something that already exists.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub resources: Vec<ResourceRef>,
    /// Describe the effect instead of causing it.
    #[serde(default)]
    pub dry_run: bool,
}

/// What actually happened.
///
/// # The only evidence of execution
///
/// A model saying "done" is text. This is the Core's answer, and it is the only
/// thing an agent may report from (briefing §5, §55).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityResult {
    /// Which capability ran.
    pub capability: CapabilityId,
    /// How it ended.
    pub status: ExecutionStatus,
    /// What it produced or touched.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub resources: Vec<ResourceRef>,
    /// A factual sentence, in the member's language.
    pub detail: String,
    /// Whether the effect can be undone.
    pub reversibility: Reversibility,
    /// Structured output, for capabilities that return data.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<serde_json::Value>,
}

/// How one execution ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionStatus {
    /// It ran and did what it said.
    Succeeded,
    /// It described what it would do, and did nothing.
    DryRun,
    /// Refused: the acting person may not do this.
    PermissionDenied,
    /// Refused: the installation cannot do this right now.
    CapabilityUnavailable,
    /// Refused: the input did not match the schema or the domain rules.
    ValidationFailed,
    /// Refused: what it names does not exist, or is not reachable.
    ResourceNotFound,
    /// Refused: a person has to confirm first.
    ApprovalRequired,
    /// It ran and failed.
    Failed,
    /// It never ran, because an earlier step in the plan failed.
    ///
    /// Distinct from `Failed` on purpose: «did not happen» and «went wrong» are
    /// different things to report (briefing §56).
    NotAttempted,
}

impl ExecutionStatus {
    /// Stable representation.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Succeeded => "succeeded",
            Self::DryRun => "dry_run",
            Self::PermissionDenied => "permission_denied",
            Self::CapabilityUnavailable => "capability_unavailable",
            Self::ValidationFailed => "validation_failed",
            Self::ResourceNotFound => "resource_not_found",
            Self::ApprovalRequired => "approval_required",
            Self::Failed => "failed",
            Self::NotAttempted => "not_attempted",
        }
    }

    /// Whether anything changed.
    #[must_use]
    pub const fn changed_something(self) -> bool {
        matches!(self, Self::Succeeded)
    }

    /// What a member reads.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Succeeded => "Concluída",
            Self::DryRun => "Simulação",
            Self::PermissionDenied => "Sem acesso",
            Self::CapabilityUnavailable => "Indisponível",
            Self::ValidationFailed => "Pedido inválido",
            Self::ResourceNotFound => "Não encontrado",
            Self::ApprovalRequired => "Aguarda confirmação",
            Self::Failed => "Falhou",
            Self::NotAttempted => "Não executada",
        }
    }
}

// ── Plans ───────────────────────────────────────────────────────────────

/// One step of a plan.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionStep {
    /// Its position, from 1.
    pub ordinal: u16,
    /// What it will do, in the member's language.
    pub summary: String,
    /// What it asks the Core to do.
    pub request: CapabilityRequest,
    /// The risk the registry assigned. **Not proposed by the model.**
    pub risk: RiskLevel,
    /// How it ended, once it has.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<CapabilityResult>,
}

/// A plan an agent proposes.
///
/// # This is not chain-of-thought
///
/// It is the operational plan: which capabilities, in which order, on which
/// resources. The model's reasoning is not stored, here or anywhere
/// (briefing §48).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionPlan {
    /// Identifier.
    pub id: Uuid,
    /// What the member asked for, as the agent understood it.
    pub intent: String,
    /// The steps, in order.
    pub steps: Vec<ActionStep>,
    /// Where the plan stands.
    pub state: PlanState,
    /// A digest of the material content of the plan.
    ///
    /// An approval is bound to this. Change what the plan does and the digest
    /// changes, which invalidates the approval — so «yes, send that» cannot
    /// become authority to send something else (briefing §100, §101).
    pub digest: String,
}

impl ActionPlan {
    /// The highest risk anywhere in the plan.
    ///
    /// A plan is as dangerous as its most dangerous step.
    #[must_use]
    pub fn peak_risk(&self) -> RiskLevel {
        self.steps
            .iter()
            .map(|step| step.risk)
            .max()
            .unwrap_or(RiskLevel::ReadOnly)
    }

    /// Whether any step changes institutional state.
    #[must_use]
    pub fn mutates(&self) -> bool {
        self.steps.iter().any(|step| step.risk.mutates())
    }

    /// The autonomy a plan needs to run without being asked again.
    #[must_use]
    pub fn required_autonomy(&self) -> AutonomyLevel {
        if !self.mutates() {
            AutonomyLevel::Assist
        } else if self.steps.len() > 1 {
            AutonomyLevel::Workflow
        } else {
            AutonomyLevel::Act
        }
    }
}

/// Where a plan stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlanState {
    /// Built, not yet acted on.
    Proposed,
    /// Waiting for a person.
    AwaitingApproval,
    /// Confirmed, not yet run.
    Approved,
    /// Running.
    Executing,
    /// Every step succeeded.
    Completed,
    /// Some steps succeeded and some did not.
    PartiallyCompleted,
    /// Nothing succeeded.
    Failed,
    /// A person said no.
    Rejected,
    /// The approval window closed.
    Expired,
    /// Cancelled before finishing.
    Cancelled,
}

impl PlanState {
    /// Stable representation.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Proposed => "proposed",
            Self::AwaitingApproval => "awaiting_approval",
            Self::Approved => "approved",
            Self::Executing => "executing",
            Self::Completed => "completed",
            Self::PartiallyCompleted => "partially_completed",
            Self::Failed => "failed",
            Self::Rejected => "rejected",
            Self::Expired => "expired",
            Self::Cancelled => "cancelled",
        }
    }

    /// Parse from the stable representation.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        Self::all()
            .into_iter()
            .find(|state| state.as_str() == value)
    }

    /// Every state.
    #[must_use]
    pub const fn all() -> [Self; 10] {
        [
            Self::Proposed,
            Self::AwaitingApproval,
            Self::Approved,
            Self::Executing,
            Self::Completed,
            Self::PartiallyCompleted,
            Self::Failed,
            Self::Rejected,
            Self::Expired,
            Self::Cancelled,
        ]
    }

    /// Whether the plan is finished, whatever the outcome.
    #[must_use]
    pub const fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Completed
                | Self::PartiallyCompleted
                | Self::Failed
                | Self::Rejected
                | Self::Expired
                | Self::Cancelled
        )
    }

    /// What a member reads.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Proposed => "Proposto",
            Self::AwaitingApproval => "Aguarda confirmação",
            Self::Approved => "Confirmado",
            Self::Executing => "Em execução",
            Self::Completed => "Concluído",
            Self::PartiallyCompleted => "Parcialmente concluído",
            Self::Failed => "Falhou",
            Self::Rejected => "Recusado",
            Self::Expired => "Expirado",
            Self::Cancelled => "Cancelado",
        }
    }
}

// ── Intent ──────────────────────────────────────────────────────────────

/// What the member is trying to do with the command surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Intent {
    /// Find something. Deterministic; needs no model.
    Search,
    /// Ask a question about the institution.
    Ask,
    /// Have something done.
    Act,
}

impl Intent {
    /// Stable representation.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Search => "search",
            Self::Ask => "ask",
            Self::Act => "act",
        }
    }

    /// Parse from the stable representation.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "search" => Self::Search,
            "ask" => Self::Ask,
            "act" => Self::Act,
            _ => return None,
        })
    }

    /// Guess what a member meant, from what they wrote.
    ///
    /// # Why this is deterministic and not a model
    ///
    /// Two reasons, and the second is the important one.
    ///
    /// It has to work with zero AI nodes — routing a request to a model in
    /// order to decide whether to route it to a model is circular, and would
    /// make the command surface useless in this installation's actual state.
    ///
    /// And it has to be **stable**. A member who types the same sentence twice
    /// must get the same behaviour twice. An intent classifier that varies is a
    /// command surface that sometimes acts when it was asked a question.
    ///
    /// # It reads a *sentence*, not a bag of words
    ///
    /// The verb has to be the **first word**. «cria uma tarefa» is an
    /// instruction; «relatório sobre criação de tarefas» is a search that
    /// happens to contain a related noun, and «projectos de energia» is a
    /// search that contains none at all.
    ///
    /// This is the whole reason a nominalisation does not become an action.
    ///
    /// # It only ever guesses downward
    ///
    /// When the reading is not clear, the answer is [`Intent::Search`] — the
    /// one that changes nothing. A phrase read as `Act` that was meant as a
    /// question performs something nobody asked for; the reverse merely shows
    /// results (briefing §31, §189).
    ///
    /// The member can always override: the three modes stay visible.
    #[must_use]
    pub fn detect(utterance: &str) -> Self {
        let text = utterance.trim().to_lowercase();
        if text.is_empty() {
            return Self::Search;
        }

        let first = text.split_whitespace().next().unwrap_or_default();

        // Imperatives, in the second person singular — how one addresses an
        // assistant in European Portuguese. Matched on the **first word**:
        // «cria uma tarefa» is an instruction, «relatório sobre criação de
        // tarefas» is a search that happens to contain the word.
        const IMPERATIVES: &[&str] = &[
            "cria",
            "criar",
            "adiciona",
            "adicionar",
            "prepara",
            "preparar",
            "escreve",
            "escrever",
            "responde",
            "responder",
            "envia",
            "enviar",
            "move",
            "mover",
            "arquiva",
            "arquivar",
            "apaga",
            "apagar",
            "atribui",
            "atribuir",
            "marca",
            "marcar",
            "agenda",
            "agendar",
            "resume",
            "resumir",
            "traduz",
            "traduzir",
            "anexa",
            "anexar",
            "associa",
            "associar",
            "renomeia",
            "renomear",
            "actualiza",
            "actualizar",
            "atualiza",
            "atualizar",
            "abre",
            "abrir",
            "adiciona",
            "remove",
            "remover",
            "partilha",
            "partilhar",
            "promove",
            "promover",
            "cancela",
            "cancelar",
        ];

        // Imperatives that *are* searches. «Encontra o último relatório» is an
        // instruction in form and a search in substance, and routing it to
        // `Act` would make it need a model — which this installation does not
        // have, so a perfectly answerable request would come back
        // unavailable.
        //
        // Checked **before** the general imperatives, and safe by the same
        // rule as everything else here: `Search` changes nothing.
        const READ_IMPERATIVES: &[&str] = &[
            "encontra",
            "encontrar",
            "procura",
            "procurar",
            "mostra",
            "mostrar",
            "lista",
            "listar",
            "pesquisa",
            "pesquisar",
        ];

        if READ_IMPERATIVES.contains(&first) {
            return Self::Search;
        }

        if IMPERATIVES.contains(&first) {
            return Self::Act;
        }

        // Questions. A leading interrogative, or a question mark anywhere.
        const INTERROGATIVES: &[&str] = &[
            "qual", "quais", "quem", "quando", "onde", "porque", "porquê", "como", "quanto",
            "quantos", "quantas", "o", "que",
        ];

        if text.ends_with('?')
            || (INTERROGATIVES.contains(&first) && text.contains(' ') && text.len() > 12)
        {
            return Self::Ask;
        }

        // Everything else is a search. Including «relatório baterias 2026»,
        // which is the commonest thing anybody types into a bar.
        Self::Search
    }

    /// Whether serving this intent needs a model at all.
    ///
    /// Search does not. That is what keeps the command surface useful with zero
    /// AI nodes, which is this installation's actual state (briefing §32, §66).
    #[must_use]
    pub const fn needs_inference(self) -> bool {
        matches!(self, Self::Ask | Self::Act)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_capability_identifier_must_be_well_formed() {
        assert!(CapabilityId::parse("mail.draft_reply").is_some());
        assert!(CapabilityId::parse("research.idea.create").is_some());

        // Sem domínio, com maiúsculas, com espaços, ou vazio: nenhum destes
        // nomeia uma capability deste sistema.
        assert!(CapabilityId::parse("draft").is_none());
        assert!(CapabilityId::parse("Mail.Send").is_none());
        assert!(CapabilityId::parse("mail send").is_none());
        assert!(CapabilityId::parse("").is_none());
        assert!(CapabilityId::parse(&"a.".repeat(40)).is_none());
    }

    #[test]
    fn a_well_formed_identifier_is_not_an_existing_one() {
        // A forma é verificável aqui; a existência não. Um modelo pode
        // inventar `mail.delete_everything` e passar esta validação — é o
        // registry que o recusa, porque é o único que sabe o que existe.
        assert!(CapabilityId::parse("mail.delete_everything").is_some());
    }

    #[test]
    fn the_domain_is_the_first_segment() {
        assert_eq!(CapabilityId::new("mail.send").domain(), "mail");
        assert_eq!(
            CapabilityId::new("research.idea.create").domain(),
            "research"
        );
    }

    #[test]
    fn external_and_privileged_always_need_a_person() {
        assert!(RiskLevel::ExternalEffect.always_requires_approval());
        assert!(RiskLevel::Privileged.always_requires_approval());

        assert!(!RiskLevel::ReadOnly.always_requires_approval());
        assert!(!RiskLevel::LowImpact.always_requires_approval());
        assert!(!RiskLevel::MaterialMutation.always_requires_approval());
    }

    #[test]
    fn only_read_only_leaves_the_institution_unchanged() {
        assert!(!RiskLevel::ReadOnly.mutates());
        for risk in [
            RiskLevel::LowImpact,
            RiskLevel::MaterialMutation,
            RiskLevel::ExternalEffect,
            RiskLevel::Privileged,
        ] {
            assert!(risk.mutates(), "{risk:?} devia contar como alteração");
        }
    }

    #[test]
    fn risk_is_ordered_so_a_plan_can_take_its_maximum() {
        assert!(RiskLevel::ReadOnly < RiskLevel::LowImpact);
        assert!(RiskLevel::LowImpact < RiskLevel::MaterialMutation);
        assert!(RiskLevel::MaterialMutation < RiskLevel::ExternalEffect);
        assert!(RiskLevel::ExternalEffect < RiskLevel::Privileged);
    }

    #[test]
    fn a_capability_may_be_more_cautious_than_its_risk_but_never_less() {
        let reckless = descriptor(RiskLevel::ExternalEffect, ApprovalRequirement::Never);
        assert!(
            reckless.requires_approval(),
            "um efeito externo declarou-se sem confirmação e foi aceite"
        );

        let cautious = descriptor(RiskLevel::ReadOnly, ApprovalRequirement::Always);
        assert!(cautious.requires_approval());

        let ordinary = descriptor(RiskLevel::ReadOnly, ApprovalRequirement::Never);
        assert!(!ordinary.requires_approval());
    }

    #[test]
    fn autonomy_is_ordered_and_capped_below_autonomous() {
        assert!(AutonomyLevel::Assist < AutonomyLevel::Act);
        assert!(AutonomyLevel::Act < AutonomyLevel::Workflow);
        assert!(AutonomyLevel::Workflow < AutonomyLevel::Autonomous);

        // O tecto desta instalação. Um agente autónomo precisa de política,
        // dono e forma de o parar; nada disso está construído.
        assert_eq!(AutonomyLevel::ceiling(), AutonomyLevel::Workflow);
        assert!(AutonomyLevel::Autonomous > AutonomyLevel::ceiling());
    }

    #[test]
    fn only_the_acting_levels_may_execute() {
        assert!(!AutonomyLevel::Manual.may_execute());
        assert!(!AutonomyLevel::Assist.may_execute());
        assert!(!AutonomyLevel::Compose.may_execute());
        assert!(AutonomyLevel::Act.may_execute());
        assert!(AutonomyLevel::Workflow.may_execute());
    }

    #[test]
    fn undo_is_offered_only_where_undo_exists() {
        assert!(Reversibility::Reversible.may_offer_undo());

        // Uma mensagem enviada não volta atrás, e prometer que volta é pior do
        // que não oferecer nada.
        assert!(!Reversibility::Irreversible.may_offer_undo());
        assert!(!Reversibility::NothingToUndo.may_offer_undo());
        assert!(!Reversibility::ReversibleByAdministrator.may_offer_undo());
    }

    #[test]
    fn a_plan_is_as_dangerous_as_its_worst_step() {
        let plan = plan(vec![
            RiskLevel::ReadOnly,
            RiskLevel::ExternalEffect,
            RiskLevel::LowImpact,
        ]);

        assert_eq!(plan.peak_risk(), RiskLevel::ExternalEffect);
        assert!(plan.mutates());
        assert_eq!(plan.required_autonomy(), AutonomyLevel::Workflow);
    }

    #[test]
    fn a_read_only_plan_needs_no_authority_to_act() {
        let plan = plan(vec![RiskLevel::ReadOnly, RiskLevel::ReadOnly]);

        assert_eq!(plan.peak_risk(), RiskLevel::ReadOnly);
        assert!(!plan.mutates());
        assert_eq!(plan.required_autonomy(), AutonomyLevel::Assist);
    }

    #[test]
    fn an_empty_plan_is_harmless() {
        let plan = plan(vec![]);
        assert_eq!(plan.peak_risk(), RiskLevel::ReadOnly);
        assert!(!plan.mutates());
    }

    #[test]
    fn an_imperative_reads_as_an_instruction() {
        for utterance in [
            "Cria uma pasta Relatórios dentro do Projecto BESS",
            "prepara uma resposta ao Carlos",
            "Envia isto ao Fidel",
            "arquiva estes documentos",
            "Resume esta conversa",
        ] {
            assert_eq!(
                Intent::detect(utterance),
                Intent::Act,
                "«{utterance}» não foi lido como instrução"
            );
        }
    }

    #[test]
    fn a_question_reads_as_a_question() {
        for utterance in [
            "qual foi a última decisão neste projecto?",
            "Quem é o responsável pela Unidade de Energia",
            "quantos datasets estão sem licença?",
            "isto tem alguma coisa a ver com o BESS?",
        ] {
            assert_eq!(
                Intent::detect(utterance),
                Intent::Ask,
                "«{utterance}» não foi lido como pergunta"
            );
        }
    }

    #[test]
    fn keywords_read_as_a_search() {
        // O que mais se escreve numa barra.
        for utterance in [
            "relatório baterias 2026",
            "hidrogénio verde Angola",
            "BESS",
            "",
        ] {
            assert_eq!(
                Intent::detect(utterance),
                Intent::Search,
                "«{utterance}» não foi lido como pesquisa"
            );
        }
    }

    #[test]
    fn ambiguity_always_falls_to_the_intent_that_changes_nothing() {
        // A garantia que mais importa nesta função: uma leitura errada para
        // `Act` executa o que ninguém pediu; para `Search` apenas mostra
        // resultados.
        for utterance in [
            "criação de tarefas no Ocinye",
            "relatório sobre envio de correio",
            "documento de arquivo",
            "notas sobre resumo do projecto",
        ] {
            assert_eq!(
                Intent::detect(utterance),
                Intent::Search,
                "«{utterance}» foi lido como instrução por conter um verbo"
            );
        }
    }

    #[test]
    fn the_examples_from_the_briefing_are_read_correctly() {
        // §73 — consultas nominais.
        for utterance in [
            "criação de tarefas no Ocinye",
            "projectos de energia",
            "email do Carlos",
            "relatórios 2026",
            "estrutura de datasets",
        ] {
            assert_eq!(
                Intent::detect(utterance),
                Intent::Search,
                "«{utterance}» devia ser uma pesquisa"
            );
        }

        // §74 — perguntas.
        for utterance in [
            "O que mudou neste projecto?",
            "Qual é o último relatório?",
            "Quem participa nesta Idea?",
            "Há algum email do Carlos?",
        ] {
            assert_eq!(
                Intent::detect(utterance),
                Intent::Ask,
                "«{utterance}» devia ser uma pergunta"
            );
        }

        // §75 — instruções inequívocas.
        for utterance in [
            "Cria uma tarefa.",
            "Abre o Project BESS.",
            "Move este documento para Arquivo.",
            "Prepara uma resposta ao Carlos.",
        ] {
            assert_eq!(
                Intent::detect(utterance),
                Intent::Act,
                "«{utterance}» devia ser uma instrução"
            );
        }
    }

    #[test]
    fn an_instruction_that_is_a_search_routes_to_search() {
        // «Encontra o último relatório» é uma instrução na forma e uma
        // pesquisa na substância. Encaminhá-la para `Act` fá-la exigir um
        // modelo, e esta instalação não tem nenhum — um pedido perfeitamente
        // respondível voltaria indisponível.
        for utterance in [
            "Encontra o último relatório da Unidade de Energia",
            "Mostra os datasets sem licença definida",
            "Procura emails do Carlos sobre o BESS",
            "Lista os jobs de Compute que falharam ontem",
        ] {
            assert_eq!(
                Intent::detect(utterance),
                Intent::Search,
                "«{utterance}» devia ser servida sem modelo"
            );
        }
    }

    #[test]
    fn a_dangerous_instruction_is_still_only_an_intent() {
        // §76. «Envia o email» é `Act`. O que acontece a seguir é decidido
        // pelo registry e pela política de aprovação, não por esta função —
        // que não sabe o que é risco e não devia saber.
        assert_eq!(Intent::detect("Envia o email."), Intent::Act);
    }

    #[test]
    fn a_verb_inside_a_topic_stays_a_search() {
        // §77. Regressões suficientes para que uma alteração à heurística que
        // as quebre seja notada.
        for utterance in [
            "notas sobre envio de correio",
            "política de arquivo de documentos",
            "manual de criação de datasets",
            "histórico de remoção de membros",
            "processo de promoção de ideias a projectos",
            "guia para responder a parceiros",
            "documento sobre cancelamento de jobs",
        ] {
            assert_eq!(
                Intent::detect(utterance),
                Intent::Search,
                "«{utterance}» foi lido como instrução"
            );
        }
    }

    #[test]
    fn detection_is_portuguese_and_does_not_pretend_otherwise() {
        // O Workspace é português-first. Uma frase inglesa não é reconhecida
        // como instrução, e cai para pesquisa — que é o comportamento seguro.
        //
        // **Não é suporte a inglês**, e este teste existe para que não seja
        // declarado como tal sem alguém o implementar (`CLAUDE.md` §52, §69).
        assert_eq!(Intent::detect("create a task for Carlos"), Intent::Search);
        assert_eq!(Intent::detect("send the email"), Intent::Search);
    }

    #[test]
    fn detection_is_stable() {
        // Quem escreve a mesma frase duas vezes tem de obter o mesmo
        // comportamento duas vezes.
        let utterance = "Cria uma tarefa para o Carlos";
        let first = Intent::detect(utterance);
        for _ in 0..20 {
            assert_eq!(Intent::detect(utterance), first);
        }
    }

    #[test]
    fn search_works_without_a_model_and_the_others_do_not() {
        assert!(!Intent::Search.needs_inference());
        assert!(Intent::Ask.needs_inference());
        assert!(Intent::Act.needs_inference());
    }

    #[test]
    fn not_attempted_is_not_the_same_as_failed() {
        // «Não aconteceu» e «correu mal» são coisas diferentes a reportar.
        assert!(!ExecutionStatus::NotAttempted.changed_something());
        assert!(!ExecutionStatus::Failed.changed_something());
        assert_ne!(
            ExecutionStatus::NotAttempted.as_str(),
            ExecutionStatus::Failed.as_str()
        );
        assert!(ExecutionStatus::Succeeded.changed_something());
        assert!(!ExecutionStatus::DryRun.changed_something());
    }

    #[test]
    fn every_stable_representation_round_trips() {
        for risk in RiskLevel::all() {
            assert_eq!(RiskLevel::parse(risk.as_str()), Some(risk));
        }
        for level in AutonomyLevel::all() {
            assert_eq!(AutonomyLevel::parse(level.as_str()), Some(level));
        }
        for state in PlanState::all() {
            assert_eq!(PlanState::parse(state.as_str()), Some(state));
        }
        for kind in ResourceKind::all() {
            assert_eq!(ResourceKind::parse(kind.as_str()), Some(kind));
        }
    }

    // ── fixtures ────────────────────────────────────────────────────────

    fn descriptor(risk: RiskLevel, approval: ApprovalRequirement) -> CapabilityDescriptor {
        CapabilityDescriptor {
            id: CapabilityId::new("test.capability"),
            operation: OperationId::new("test::fixture"),
            domain: "test".to_owned(),
            summary: "Uma capacidade de teste.".to_owned(),
            permission: Permission::AiUse,
            scope: Scope::Institution,
            risk,
            approval,
            max_autonomy: AutonomyLevel::Act,
            reversibility: Reversibility::NothingToUndo,
            supports_dry_run: false,
            classification_ceiling: None,
            input_schema: serde_json::json!({"type": "object"}),
        }
    }

    fn plan(risks: Vec<RiskLevel>) -> ActionPlan {
        ActionPlan {
            id: Uuid::nil(),
            intent: "teste".to_owned(),
            steps: risks
                .into_iter()
                .enumerate()
                .map(|(index, risk)| ActionStep {
                    ordinal: u16::try_from(index + 1).unwrap_or(1),
                    summary: "passo".to_owned(),
                    request: CapabilityRequest {
                        capability: CapabilityId::new("test.capability"),
                        input: serde_json::json!({}),
                        resources: Vec::new(),
                        dry_run: false,
                    },
                    risk,
                    result: None,
                })
                .collect(),
            state: PlanState::Proposed,
            digest: String::new(),
        }
    }
}
