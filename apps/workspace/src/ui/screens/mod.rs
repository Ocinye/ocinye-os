//! Os ecrãs do Ocinye Workspace.
//!
//! Um módulo por ecrã ou por família de ecrãs, na ordem do mapa de navegação
//! em `design/README.md` §4.
//!
//! Nenhum ecrã constrói uma tabela, um badge ou um botão próprio: tudo vem de
//! [`crate::ui::components`].

pub mod activity;
pub mod administration;
pub mod ai;
pub mod ask;
pub mod boot;
pub mod calendar;
pub mod compute;
pub mod first_access;
pub mod help;
pub mod home;
pub mod knowledge;
pub mod lists;
pub mod login;
pub mod mail;
pub mod my_work;
pub mod notice;
pub mod prompt;
pub mod search;
pub mod settings;
pub mod workspaces;
