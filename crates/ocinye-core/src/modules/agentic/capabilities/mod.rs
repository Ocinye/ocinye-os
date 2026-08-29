//! The capabilities the Ocinye OS publishes to its agents.
//!
//! # The shape of every one of them
//!
//! A handler is thin. It types its input, calls the **domain service that owns
//! the invariant**, and turns the result into something the interface can show.
//! It never writes SQL, never re-implements a rule, and never authorises: by
//! the time it runs, the executor has already decided that this person may do
//! this here.
//!
//! That thinness is the point. A capability is a published door onto an
//! existing service, not a second way in with its own rules — a handler that
//! reached past its service would be a rule that applies only when an agent
//! asks (`CLAUDE.md` §3).
//!
//! # Why this set, and why it is small
//!
//! Twenty-five capabilities across five domains, added one audited domain at
//! a time. Mail carries seven, because it is the module that exercises every
//! invariant at once — search, read, compose, transform, classification,
//! external effect and human approval. Research and Knowledge carry fifteen
//! between them, because the scientific core is where the institution's memory
//! lives and where an agent is most useful.
//!
//! The registry grows **deliberately, as each domain is audited**. Turning
//! every endpoint into a tool automatically would produce a hundred untested
//! doors and no fronteira worth the name (briefing §207). The Core exposes far
//! more than this through its HTTP surface; what is here is the subset that is
//! a coherent institutional operation, safe to be *proposed* by a model, and
//! worth the test coverage each one carries.

use std::sync::Arc;

use super::registry::CapabilityHandler;

mod calendar;
mod collaboration;
mod compute;
mod data;
mod files;
mod knowledge;
mod mail;
mod messaging;
mod organisation;
mod research;
mod science;
mod self_service;

/// Every handler, in the order they are registered.
pub fn all() -> Vec<Arc<dyn CapabilityHandler>> {
    vec![
        // Knowledge: the read that works with zero AI nodes, then the artefacts
        // themselves, then the edges between them.
        // Mensagens. Cada uma chama a mesma operação que o composer chama.
        Arc::new(messaging::SendMessage),
        Arc::new(messaging::OpenDirect),
        Arc::new(messaging::CreateGroup),
        Arc::new(messaging::AddMember),
        Arc::new(knowledge::Search),
        Arc::new(knowledge::ReadNote),
        Arc::new(knowledge::ReadSource),
        Arc::new(knowledge::ReadDocument),
        Arc::new(files::ReadFileContent),
        Arc::new(knowledge::ListLinks),
        Arc::new(knowledge::CreateNote),
        Arc::new(knowledge::ReviseNote),
        Arc::new(knowledge::CreateSource),
        Arc::new(knowledge::CreateLink),
        // O ciclo científico. `science::record_validation` não está aqui, e é
        // deliberado: é `non_delegable`, atrás de uma fronteira de autoridade.
        Arc::new(science::StateHypothesis),
        Arc::new(science::CreateMethodology),
        Arc::new(science::PublishMethodologyVersion),
        Arc::new(science::DesignStudy),
        Arc::new(science::RecordExecution),
        Arc::new(science::RecordResult),
        Arc::new(science::ReadLineage),
        // A primeira capacidade que atravessa o Capability Runtime. Não muda
        // nada, é determinística, e é útil onde um modelo costuma tropeçar:
        // uma bibliografia que ele redigiu é texto plausível até alguém a ler.
        Arc::new(knowledge::ReviewBibliography),
        // Research: the workspace that gives everything its context, the two
        // lifecycles inside it, and the promotion that joins them.
        Arc::new(research::WorkspaceOverview),
        Arc::new(research::ReadIdea),
        Arc::new(research::ReadProject),
        Arc::new(research::CreateIdea),
        Arc::new(research::ReviseIdea),
        Arc::new(research::TransitionIdea),
        Arc::new(research::TransitionProject),
        Arc::new(research::PromoteIdea),
        // Organisation: o mapa institucional. «Cria uma unidade de Materiais
        // e Economia Circular» é quase o exemplo perfeito do Ocinye OS, e até
        // ao ADR-0307 o plano agentic não lhe chegava — não por decisão, mas
        // porque ninguém tinha decidido.
        //
        // Acrescentar alguém a uma unidade não está aqui: mediu-se, e a
        // filiação expande o acesso efectivo.
        Arc::new(organisation::CreateUnit),
        // Data: os metadados de um dataset. Os ficheiros ficam de fora, e a
        // separação é a razão pela qual esta é endereçável e a outra não.
        Arc::new(data::CreateDataset),
        // Calendário: o tempo institucional. Cada uma chama exactamente a
        // operação que a interface chama — o agente é outra forma de entrar no
        // mesmo sítio, e não um caminho paralelo.
        //
        // Cancelar pede confirmação e as outras não: marcar a mais é um
        // incómodo, cancelar sem querer faz alguém não aparecer.
        Arc::new(calendar::CreateEvent),
        Arc::new(calendar::UpdateEvent),
        Arc::new(calendar::CancelEvent),
        Arc::new(calendar::CreateReminder),
        // Identidade própria: o que um membro faz sobre si mesmo. As operações
        // que mudam a autoridade de **outra** pessoa não estão aqui, e não por
        // esquecimento — ver `self_service`.
        Arc::new(self_service::RevokeOwnSession),
        Arc::new(self_service::ChooseAvatarPreset),
        // Collaboration: the everyday mutation, and the question a project
        // member actually asks.
        Arc::new(collaboration::CreateTask),
        Arc::new(collaboration::ListTasks),
        Arc::new(collaboration::TransitionTask),
        Arc::new(collaboration::AssignTask),
        // Mail: the retrofit. Search and read are the two halves of «encontra
        // o último email do Carlos»; draft, reply and transform are the
        // composing half; evaluate answers the question a send raises, before
        // the send; and send is the one external effect.
        Arc::new(mail::SearchMail),
        Arc::new(mail::ReadMessage),
        Arc::new(mail::DraftMessage),
        Arc::new(mail::DraftReply),
        Arc::new(mail::TransformDraft),
        Arc::new(mail::EvaluateSend),
        Arc::new(mail::SendDraft),
        // Compute: reports real state, which is currently zero nodes.
        Arc::new(compute::ListNodes),
    ]
}
