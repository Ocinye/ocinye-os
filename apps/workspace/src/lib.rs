//! Ocinye Workspace — the principal human interface of the Ocinye OS.
//!
//! Not a website with a private area: the working environment through which
//! members of the institution act on the Ocinye Core.
//!
//! # Backend-for-Frontend, deliberately
//!
//! This server runs the OIDC flow and **holds the tokens itself**. The browser
//! receives an opaque session cookie and nothing else. A token in a browser is a
//! token exposed to every script on the page; keeping it here removes that class
//! of problem entirely (ADR-0600).
//!
//! # The browser is never authority
//!
//! Views hide what a member cannot use, because showing it would be unkind.
//! They never *decide* it. Every operation is authorised by the Core, which
//! would refuse it regardless of what this server rendered (briefing §17).
//!
//! # Rendering
//!
//! Leptos in server-side rendering, implementing the design dossier in
//! [`design/`](../../../design/README.md). A single, bounded progressive
//! enhancement layer (`static/app.js`) provides the command palette, the
//! collapsible sidebar and the create menu — DOM behaviour only, never data and
//! never an authorization decision (ADR-0602).
//!
//! Hydration remains the declared destination; the components are already
//! Leptos, so adopting it is a build-chain change rather than a rewrite.

#![forbid(unsafe_code)]

//! # Porque isto é uma biblioteca além de um binário
//!
//! O percurso que interessa provar começa num browser e acaba no PostgreSQL:
//!
//! ```text
//! browser → rota do Workspace → HTTP → Core → PostgreSQL
//! ```
//!
//! Um teste que reconstruísse o router seria livre de divergir dele, e provaria
//! um frontend isolado em vez de provar que uma pessoa consegue usar o sistema.
//! Expor o router como biblioteca deixa o harness montar o Workspace verdadeiro.

pub mod api;
pub mod boot;
pub mod config;
pub mod routes;
pub mod session;
pub mod ui;

use crate::config::WorkspaceConfig;
use crate::session::SessionStore;

/// Shared state of the Workspace server.
#[derive(Clone)]
pub struct WorkspaceState {
    /// Configuration.
    pub config: std::sync::Arc<WorkspaceConfig>,
    /// Server-side sessions. Tokens live here, never in the browser.
    pub sessions: SessionStore,
    /// HTTP client used for the Core and the identity provider.
    pub http: reqwest::Client,
}
