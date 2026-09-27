//! O Terminal (ocsh): executar uma linha de comandos (ADR-0312).
//!
//! # Porque isto não é o `POST /capability/run` que as rotas agentic recusam
//!
//! Esta rota recebe **uma linha**, e não um identificador de capability com um
//! JSON. A linha só pode nomear comandos do registo fechado do ocsh; cada
//! comando está ligado, no código, a uma capability e a uma tradução fixa dos
//! seus argumentos; e o executor agentic aplica autoridade fresca, política,
//! esquema, risco e auditoria como para qualquer outra entrada. O que exige
//! confirmação não corre aqui — vira um plano.

use axum::extract::State;
use axum::routing::post;
use axum::{Json, Router};
use ocinye_contracts::ocsh::wire::{ExecRequest, ExecResponse};
use ocinye_core::modules::terminal;

use crate::error::ApiError;
use crate::extract::{CurrentPrincipal, Ids};
use crate::state::AppState;

/// Rotas do Terminal.
pub fn routes() -> Router<AppState> {
    Router::new().route("/commands/exec", post(exec))
}

/// `POST /commands/exec` — ler, autorizar e executar uma linha do ocsh.
///
/// Sucesso HTTP para qualquer resultado da linha, incluindo recusas e erros de
/// uso: o código de saída do ocsh diz o que aconteceu. Um erro HTTP é só o Core
/// não ter conseguido concluir.
async fn exec(
    State(state): State<AppState>,
    Ids(ids): Ids,
    CurrentPrincipal(principal): CurrentPrincipal,
    Json(request): Json<ExecRequest>,
) -> Result<Json<ExecResponse>, ApiError> {
    let deps = terminal::Deps {
        pool: &state.pool,
        capabilities: &state.capabilities,
        realtime: &state.realtime,
        ids: &ids,
    };
    Ok(Json(terminal::execute(&deps, &principal, &request).await?))
}
