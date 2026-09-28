//! A interface do Ocinye Workspace (Claude Design).
//!
//! DESIGN_LOCKED · `docs/ui/DESIGN_LOCK.md`. Cada vista é uma função pura
//! `vista(&ViewModel) -> impl IntoView`: não chama o Core, não lê a sessão e não
//! decide autorização. O servidor preenche o ViewModel (`view_models`) e
//! entrega a árvore a [`document::render`].
//!
//! Regras que todo o módulo cumpre (e que os testes verificam):
//! - sem `style=""`, sem `<script>` inline e sem `on*=""` (CSP `'self'`);
//! - todo o texto sai de `crate::i18n::{t, tf, tp}`;
//! - cada `<button>` submete, tem `data-oc` ou `aria-disabled="true"`;
//! - comportamento só em `static/*.js`, ligado por `data-oc` e `data-part`.

pub mod components;
pub mod document;
pub mod screens;
pub mod shell;
pub mod view_models;

#[cfg(test)]
pub(crate) mod testing;
