//! Ocinye Mensagens — comunicação institucional em tempo real.
//!
//! # Onde vive o quê
//!
//! O que é durável está no PostgreSQL: conversas, quem pertence a elas, o que
//! foi dito, a quem se respondeu, quem foi mencionado, quem reagiu e até onde
//! cada pessoa leu.
//!
//! O que é efémero está no plano realtime, com TTL: presença e `typing`
//! ([`crate::realtime`], ADR-0012). Nada disso passa por aqui.
//!
//! # Participação não é papel institucional
//!
//! Quem alcança uma conversa decide-se pela participação, e por mais nada. Um
//! `owner` de grupo é `owner` **daquele grupo**: não herda autoridade nenhuma
//! da instituição, e nenhuma autoridade da instituição lha dá.

pub mod repository;
pub mod service;

pub use service::{
    add_member, assist, build_assist_prompt, conversations, create_group, history, mark_read,
    open_direct, remove_member, send, toggle_reaction, AssistAction, Outgoing, MAX_BODY,
    MAX_PARTICIPANTS, PAGE_SIZE,
};
