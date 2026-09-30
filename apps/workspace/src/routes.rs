//! As rotas do Workspace.
//!
//! Todos os ecrãs seguem a mesma forma: resolver a sessão, chamar o Ocinye Core
//! com o token do membro, renderizar. Quando o Core recusa, o Workspace mostra
//! o que o Core disse, em vez de inventar a sua própria versão.
//!
//! O mapa de navegação é o de `design/README.md` §4.

use std::time::{Duration, Instant};

use axum::extract::{DefaultBodyLimit, Form, Multipart, Path, Query, State};
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::routing::{get, post, put};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::Value;
use tower_http::services::ServeDir;
use uuid::Uuid;

use crate::api::{self, ApiFailure};
use crate::controllers::{self, Caller, Shell, ShellContext};
use crate::experience;
use crate::experience::navigation::Screen;
use crate::session::{self, Session};
use crate::ui;
use crate::ui::view_models::{
    DocumentVm, ErrorKind, ErrorVm, FirstAccessVm, IdentityFailVm, LoginVm, MfaChallengeVm,
    MfaCodesVm, MfaSetupVm, RecoverVm, SessionEndReason, SessionEndVm, Surface, Theme,
};
use crate::window_manager;
use crate::WorkspaceState;
use ocinye_contracts::ApplicationId;

/// Todos os caminhos que o Workspace serve.
///
/// Existe para que um teste possa afirmar que nenhuma ligação renderizada
/// aponta para fora desta lista. Uma ligação morta num ambiente institucional
/// não é um detalhe estético: é uma promessa que a interface não cumpre.
#[cfg_attr(
    not(test),
    allow(dead_code, reason = "lido pelo teste de ligações mortas")
)]
pub const ROUTES: &[&str] = &[
    "/",
    "/my-work",
    "/my-work/new",
    "/my-work/{task_id}",
    "/my-work/{task_id}/transitions",
    "/my-work/{task_id}/assignee",
    "/ideas/{idea_id}/transitions",
    "/ideas/{idea_id}/promotion",
    "/projects/{project_id}/transitions",
    "/datasets/{dataset_id}/versions",
    "/datasets/{dataset_id}/versions/{version_id}/files",
    "/datasets/{dataset_id}/versions/{version_id}/publish",
    "/knowledge/sources/new",
    "/knowledge/{section}",
    "/knowledge/{section}/{item_id}",
    "/resources",
    "/notes",
    "/notes/new",
    "/notes/partilhadas",
    "/notes/lixo",
    "/notes/{note_id}/apagar",
    "/notes/{note_id}/restaurar",
    "/notes/{note_id}/eliminar",
    "/notes/{note_id}",
    "/notes/{note_id}/gravar",
    "/notes/{note_id}/mover",
    "/notes/{note_id}/partilhar",
    "/notes/{note_id}/revogar/{person_id}",
    "/notes/{note_id}/revisoes/{revision}",
    "/notes/{note_id}/revisoes/{revision}/restaurar",
    "/notes/folders",
    "/notes/folders/{folder_id}/apagar",
    "/me/files",
    "/me/files/{version_id}/preview",
    "/messages",
    "/messages/{conversation}",
    "/messages/start",
    "/messages/assist",
    "/messages/people",
    "/messages/{conversation}/typing",
    "/messages/{conversation}/send",
    "/messages/{conversation}/react",
    "/messages/{conversation}/read",
    "/messages/{conversation}/members",
    "/messages/{conversation}/leave",
    "/messages/{conversation}/remove",
    "/mail",
    "/mail/{mailbox_id}",
    "/mail/{mailbox_id}/sync",
    "/mail/message/{message_id}",
    "/mail/message/{message_id}/flags",
    "/mail/compose",
    "/mail/drafts",
    "/mail/drafts/{draft_id}",
    "/mail/drafts/{draft_id}/attachments",
    "/mail/drafts/{draft_id}/attachments/{attachment_id}",
    "/mail/people",
    "/mail/assist",
    "/mail/send",
    "/mail/settings",
    "/mail/connect",
    "/mail/{mailbox_id}/connect",
    "/mail/{mailbox_id}/disconnect",
    "/units",
    "/units/code-suggestion",
    "/units/{unit_id}",
    "/units/{unit_id}/edit",
    "/units/{unit_id}/members",
    "/workspaces/{workspace_id}/members",
    "/workspaces/{workspace_id}/members/remove",
    "/units/{unit_id}/members/role",
    "/units/{unit_id}/members/remove",
    "/ideas",
    "/units/new",
    "/projects/new",
    "/bibliography/new",
    "/datasets/new",
    "/datasets/{dataset_id}",
    "/tasks/new",
    "/tasks/{task_id}",
    "/calendar",
    "/calendar/events/new",
    "/calendar/events/{event_id}",
    "/calendar/events/{event_id}/edit",
    "/calendar/events/{event_id}/cancel",
    "/notifications",
    "/notifications/recent",
    "/notifications/read-all",
    "/notifications/{notification_id}/read",
    "/help",
    "/terminal",
    "/terminal/exec",
    "/settings",
    "/settings/security",
    "/settings/language",
    "/settings/apps",
    "/settings/mfa",
    "/settings/mfa/regenerate",
    "/settings/password",
    "/settings/avatar/preset",
    "/settings/avatar/photo",
    "/settings/avatar/initials",
    "/settings/sessions/{session_id}/revoke",
    "/avatar/me/{version}",
    "/ideas/new",
    "/ideas/{idea_id}",
    "/projects",
    "/projects/{project_id}",
    "/workspaces/{workspace_id}",
    "/workspaces/{workspace_id}/science",
    "/workspaces/{workspace_id}/science/hypotheses/new",
    "/workspaces/{workspace_id}/science/methodologies/new",
    "/workspaces/{workspace_id}/science/studies/new",
    "/methodologies/{methodology_id}",
    "/methodologies/{methodology_id}/versions/new",
    "/studies/{study_id}",
    "/studies/{study_id}/executions/new",
    "/executions/{execution_id}",
    "/executions/{execution_id}/results/new",
    "/results/{result_id}",
    "/results/{result_id}/validate",
    "/knowledge",
    "/files",
    "/files/upload",
    "/files/uploads",
    "/files/personal-upload",
    "/files/upload-preflight",
    "/apps/pins",
    "/me/desktop",
    "/me/desktop/restore",
    "/files/uploads/{session_id}",
    "/files/uploads/{session_id}/parts/{part_number}",
    "/files/uploads/{session_id}/complete",
    "/files/folder",
    "/files/{file_id}",
    "/files/{file_id}/version",
    "/files/{file_id}/download",
    "/files/{file_id}/preview",
    "/file-versions/{version_id}/preview",
    "/file-versions/{version_id}/download",
    "/me/files/{version_id}/download",
    "/me/files/{version_id}/text",
    "/me/files/{version_id}/inline",
    "/me/files/{version_id}/thumbnail",
    "/me/files/{version_id}/raw",
    "/me/files/rename",
    "/me/files/move",
    "/me/files/batch/move",
    "/me/files/batch/delete",
    "/me/files/favourite",
    "/me/files/delete",
    "/me/files/restore",
    "/me/files/purge",
    "/files/trash/empty",
    "/me/folders",
    "/me/folders/rename",
    "/me/folders/delete",
    "/bibliography",
    "/bibliography/tools",
    "/datasets",
    "/ai",
    "/ai/agents",
    "/ai/agents/new",
    "/ai/agents/{agent_id}",
    "/ai/prompt",
    "/compute",
    "/activity",
    "/admin",
    "/admin/instance",
    "/admin/monitor",
    "/admin/members/new",
    "/admin/members/{person_id}",
    "/admin/members/{person_id}/position",
    "/admin/members/{person_id}/delete",
    "/admin/members/{person_id}/provision",
    "/admin/members/{person_id}/units",
    "/admin/members/{person_id}/units/{unit_id}/role",
    "/admin/members/{person_id}/units/{unit_id}/remove",
    "/admin/members/{person_id}/workspaces",
    "/admin/members/{person_id}/workspaces/{workspace_id}/role",
    "/admin/members/{person_id}/workspaces/{workspace_id}/remove",
    "/admin/members/{person_id}/reset-password",
    "/admin/members/{person_id}/status",
    "/admin/members/{person_id}/roles",
    "/admin/members/{person_id}/roles/{role}/revoke",
    "/admin/members/{person_id}/grants",
    "/admin/members/{person_id}/grants/{grant_id}/revoke",
    "/admin/members/{person_id}/sessions/{session_id}/revoke",
    "/wm",
    "/wm/{window_id}",
    "/wm/{window_id}/close",
    "/wm/{window_id}/state",
    "/audit",
    "/search",
    "/ask",
    "/ask/plans/{plan_id}/execute",
    "/ask/plans/{plan_id}/reject",
    "/boot",
    "/login",
    "/login/language",
    "/password/recover",
    "/first-access",
    "/mfa",
    "/mfa/confirm",
    "/mfa/acknowledge",
    "/mfa/challenge",
    "/mfa/recovery",
    "/logout",
    "/health",
];

/// O router do Workspace.
pub fn router(state: WorkspaceState) -> Router {
    Router::new()
        // Pessoal
        .route("/", get(home))
        .route("/my-work", get(research::work_page))
        .route(
            "/my-work/new",
            get(research::task_new_page).post(research::task_create),
        )
        .route("/my-work/{task_id}", get(research::task_page))
        .route(
            "/my-work/{task_id}/transitions",
            post(research::task_transition),
        )
        .route("/my-work/{task_id}/assignee", post(research::task_assign))
        .route("/resources", get(meus_recursos))
        // Correio
        // ── Mensagens ───────────────────────────────────────────────────
        .route("/messages", get(messaging))
        .route("/messages/{conversation}", get(messaging_conversation))
        .route("/messages/start", post(messaging_start))
        .route("/messages/assist", post(messaging_assist))
        .route("/messages/people", get(messaging_people))
        .route("/messages/{conversation}/typing", get(messaging_typing))
        .route("/messages/{conversation}/send", post(messaging_send))
        .route("/messages/{conversation}/react", post(messaging_react))
        .route("/messages/{conversation}/read", post(messaging_read))
        .route(
            "/messages/{conversation}/members",
            post(messaging_add_member),
        )
        .route("/messages/{conversation}/leave", post(messaging_leave))
        .route("/messages/{conversation}/remove", post(messaging_remove))
        .route("/mail", get(productivity::mail_page))
        .route("/mail/compose", get(productivity::mail_compose_page))
        .route("/mail/compose/save", post(productivity::mail_compose_save))
        .route("/mail/compose/send", post(productivity::mail_compose_send))
        .route("/mail/people", get(mail_people))
        .route("/mail/assist", post(assist))
        .route("/mail/send", post(send_mail))
        .route("/mail/drafts", post(draft_create))
        .route(
            "/mail/drafts/{draft_id}",
            put(draft_update).delete(draft_discard),
        )
        .route(
            "/mail/drafts/{draft_id}/attachments",
            post(draft_attach).layer(DefaultBodyLimit::max(27 * 1024 * 1024)),
        )
        .route(
            "/mail/drafts/{draft_id}/attachments/{attachment_id}",
            axum::routing::delete(draft_attachment_remove),
        )
        .route(
            "/mail/settings",
            get(mail_settings).post(save_mail_settings),
        )
        // Declaradas antes de `/mail/{mailbox_id}`: são caminhos literais sob
        // um identificador, e a rota genérica apanhá-las-ia primeiro.
        .route("/mail/connect", post(mail_connect_own))
        .route("/mail/{mailbox_id}/connect", post(mail_connect))
        .route("/mail/{mailbox_id}/disconnect", post(mail_disconnect))
        .route(
            "/mail/message/{message_id}",
            get(productivity::mail_message_page),
        )
        .route(
            "/mail/message/{message_id}/op",
            post(productivity::mail_message_op),
        )
        .route("/mail/message/{message_id}/flags", post(mail_flags))
        // Declarada depois das anteriores: `/mail/compose` tem de bater na
        // rota literal, não em `{mailbox_id}`.
        .route("/mail/{mailbox_id}", get(mail_mailbox))
        .route("/mail/{mailbox_id}/sync", post(mail_sync))
        // Investigação
        .route("/units", get(units))
        .route("/units/code-suggestion", get(unit_code_suggestion))
        .route("/units/{unit_id}", get(unit_detail))
        .route(
            "/units/{unit_id}/edit",
            get(edit_unit_form).post(update_unit),
        )
        // Gerir quem pertence a uma unidade. Três operações, três caminhos: uma
        // pertença é autoridade, e cada alteração dela é um acto próprio.
        .route(
            "/workspaces/{workspace_id}/members",
            post(workspace_member_add),
        )
        .route(
            "/workspaces/{workspace_id}/members/remove",
            post(workspace_member_remove),
        )
        .route("/units/{unit_id}/members", post(unit_member_add))
        .route("/units/{unit_id}/members/role", post(unit_member_role))
        .route("/units/{unit_id}/members/remove", post(unit_member_remove))
        .route("/ideas", get(research::ideas_page))
        .route("/calendar", get(productivity::calendar_page))
        .route(
            "/calendar/events/new",
            get(productivity::new_event_form).post(productivity::create_calendar_event),
        )
        .route(
            "/calendar/events/{event_id}",
            get(productivity::event_detail_page),
        )
        .route(
            "/calendar/events/{event_id}/edit",
            get(productivity::edit_event_form).post(productivity::update_calendar_event),
        )
        .route(
            "/calendar/events/{event_id}/cancel",
            post(productivity::cancel_calendar_event),
        )
        .route("/notifications", get(notifications_page))
        .route("/notifications/recent", get(notifications_recent))
        .route("/notifications/read-all", post(notifications_read_all))
        .route(
            "/notifications/{notification_id}/read",
            post(mark_notification_read),
        )
        // Notas pessoais. A criação e a lista partilham o caminho: `GET /notes`
        // mostra as notas, `POST /notes` cria uma e leva o membro ao editor.
        .route(
            "/notes",
            get(productivity::notes_list).post(create_personal_note),
        )
        // A criação a partir do «+ Criar» global tem o seu próprio caminho: o
        // formulário do menu vive na barra de topo de todas as páginas, e um
        // `action="/notes"` colidiria, no DOM, com o formulário de criação da
        // própria lista de Notas. Mesmo efeito, caminho distinto.
        .route("/notes/new", post(create_personal_note))
        // As notas que outra pessoa partilhou com o membro — a vista de leitura.
        .route("/notes/partilhadas", get(productivity::shared_notes_page))
        // O Lixo: as notas apagadas, de onde se restauram ou se eliminam de vez.
        .route("/notes/lixo", get(productivity::notes_trash_page))
        .route("/notes/{note_id}", get(productivity::note_editor))
        // Apagar (leva ao Lixo), restaurar e eliminar definitivamente — do dono.
        .route("/notes/{note_id}/apagar", post(delete_note_route))
        .route("/notes/{note_id}/restaurar", post(restore_note_route))
        .route("/notes/{note_id}/eliminar", post(purge_note_route))
        // Gravar (D004): o formulário do editor, com a revisão em que abriu.
        .route(
            "/notes/{note_id}/gravar",
            post(productivity::save_personal_note),
        )
        // Mover uma nota para uma pasta: fetch em JSON, do editor.
        .route("/notes/{note_id}/mover", post(move_personal_note))
        // Partilha: conceder acesso a uma pessoa e revogá-lo. Formulários, do
        // dono; o Core recusa a quem não é dono da nota.
        .route("/notes/{note_id}/partilhar", post(share_note_route))
        .route(
            "/notes/{note_id}/revogar/{person_id}",
            post(revoke_note_share_route),
        )
        // Histórico: pré-visualizar uma revisão e restaurá-la como revisão nova.
        .route(
            "/notes/{note_id}/revisoes/{revision}",
            get(note_revision_preview),
        )
        .route(
            "/notes/{note_id}/revisoes/{revision}/restaurar",
            post(restore_note_revision_route),
        )
        // Pastas: criar (formulário) e apagar (formulário).
        .route("/notes/folders", post(create_note_folder))
        .route(
            "/notes/folders/{folder_id}/apagar",
            post(delete_note_folder),
        )
        // As imagens de uma nota: carregar (fetch, em JSON) e servir inline, na
        // origem do Workspace. A CSP continua `img-src 'self'`, e a página nunca
        // aprende onde os bytes estão guardados.
        .route(
            "/me/files",
            post(upload_personal_note_file).layer(DefaultBodyLimit::max(FILE_BODY_LIMIT_BYTES)),
        )
        .route(
            "/me/files/{version_id}/preview",
            get(preview_personal_note_file),
        )
        .route("/units/new", get(new_unit_form).post(create_unit))
        .route("/projects/new", get(research::project_new_entry))
        .route("/bibliography/new", get(research::bibliography_new_entry))
        .route(
            "/datasets/new",
            get(research::dataset_new_page).post(research::dataset_create),
        )
        .route("/datasets/{dataset_id}", get(research::dataset_page))
        .route(
            "/datasets/{dataset_id}/versions",
            post(research::dataset_version_create),
        )
        .route(
            "/datasets/{dataset_id}/versions/{version_id}/files",
            post(research::dataset_file_add).layer(DefaultBodyLimit::max(FILE_BODY_LIMIT_BYTES)),
        )
        .route(
            "/datasets/{dataset_id}/versions/{version_id}/publish",
            post(research::dataset_version_publish),
        )
        .route("/tasks/new", get(|| async { Redirect::to("/my-work/new") }))
        .route("/tasks/{task_id}", get(research::task_legacy))
        .route("/help", get(help))
        .route("/terminal", get(terminal))
        .route("/terminal/exec", post(terminal_exec))
        .route("/settings", get(settings_account))
        .route("/settings/security", get(settings_security))
        .route(
            "/settings/language",
            get(settings_language).post(set_language),
        )
        .route("/settings/apps", get(settings_apps).post(save_apps))
        .route("/settings/mfa", get(settings_mfa))
        .route("/settings/mfa/regenerate", post(settings_mfa_regenerate))
        .route("/settings/password", post(change_password))
        .route("/settings/avatar/preset", post(choose_avatar_preset))
        .route(
            "/settings/avatar/photo",
            post(upload_avatar).layer(DefaultBodyLimit::max(AVATAR_BODY_LIMIT_BYTES)),
        )
        .route("/settings/avatar/initials", post(use_initials_avatar))
        // A fotografia do próprio membro. `me` não é um parâmetro: é a sessão
        // que diz de quem é, e a versão no caminho é só o endereço daquele
        // conteúdo.
        .route("/avatar/me/{version}", get(own_avatar))
        .route(
            "/settings/sessions/{session_id}/revoke",
            post(revoke_session),
        )
        .route(
            "/ideas/new",
            get(research::idea_new_page).post(research::idea_create),
        )
        .route("/ideas/{idea_id}", get(research::idea_page))
        .route(
            "/ideas/{idea_id}/transitions",
            post(research::idea_transition),
        )
        .route("/ideas/{idea_id}/promotion", post(research::idea_promote))
        .route("/projects", get(research::projects_page))
        .route("/projects/{project_id}", get(research::project_page))
        .route(
            "/projects/{project_id}/transitions",
            post(research::project_transition),
        )
        .route("/workspaces/{workspace_id}", get(research::workspace_entry))
        // A cadeia científica do ambiente, e um resultado com a sua
        // proveniência. `/results/{id}` é raiz e não está debaixo do
        // ambiente: um resultado é citável, e um caminho que exigisse saber
        // em que ambiente ele vive obrigaria quem tem o link a descobri-lo.
        .route("/workspaces/{workspace_id}/science", get(scientific_chain))
        // Cada criação abre a partir do sítio onde a pergunta nasce, e leva o
        // contexto consigo em vez de o pedir. É o que faz a proveniência
        // acontecer sozinha: quem regista um resultado dentro de uma execução
        // não declara depois que aquela execução o produziu.
        .route(
            "/workspaces/{workspace_id}/science/hypotheses/new",
            get(new_hypothesis).post(create_hypothesis),
        )
        .route(
            "/workspaces/{workspace_id}/science/methodologies/new",
            get(new_methodology).post(create_methodology),
        )
        .route(
            "/workspaces/{workspace_id}/science/studies/new",
            get(new_study).post(create_study),
        )
        .route("/methodologies/{methodology_id}", get(methodology_detail))
        .route(
            "/methodologies/{methodology_id}/versions/new",
            get(new_version).post(publish_version),
        )
        .route("/studies/{study_id}", get(study_detail))
        .route(
            "/studies/{study_id}/executions/new",
            get(new_execution).post(record_execution),
        )
        .route("/executions/{execution_id}", get(execution_detail))
        .route(
            "/executions/{execution_id}/results/new",
            get(new_result).post(create_result),
        )
        .route("/results/{result_id}", get(result_detail))
        .route(
            "/results/{result_id}/validate",
            get(validate_result_form).post(record_validation),
        )
        // Conhecimento
        .route("/knowledge", get(research::knowledge_root))
        .route(
            "/knowledge/sources/new",
            get(research::source_new_page).post(research::source_create),
        )
        .route("/knowledge/{section}", get(research::knowledge_section))
        .route(
            "/knowledge/{section}/{item_id}",
            get(research::knowledge_item),
        )
        .route("/bibliography", get(research::bibliography_entry))
        .route(
            "/bibliography/tools",
            get(bibliography_tools).post(review_bibliography),
        )
        .route("/datasets", get(research::datasets_page))
        .route("/files", get(productivity::files_page))
        .route("/files/selection", post(productivity::files_selection))
        .route("/files/{file_id}/rename", post(productivity::files_rename))
        .route("/files/uploads", post(upload_begin))
        .route("/files/personal-upload", post(upload_begin_personal))
        .route("/files/upload-preflight", post(upload_preflight))
        .route("/apps/pins", put(apps_set_pins))
        .route("/me/desktop", put(desktop_save))
        .route("/me/desktop/restore", post(desktop_restore))
        .route("/admin/monitor", get(admin_monitor))
        .route(
            "/files/uploads/{session_id}",
            get(upload_status).delete(upload_cancel),
        )
        .route(
            "/files/uploads/{session_id}/parts/{part_number}",
            put(upload_send_part).layer(DefaultBodyLimit::max(PARTE_MAXIMA_BYTES)),
        )
        .route(
            "/files/uploads/{session_id}/complete",
            post(upload_complete),
        )
        .route(
            "/files/upload",
            post(files_upload).layer(DefaultBodyLimit::max(FILE_BODY_LIMIT_BYTES)),
        )
        .route("/files/folder", post(productivity::files_new_folder))
        .route("/files/{file_id}", get(file_detail))
        .route(
            "/files/{file_id}/version",
            post(file_new_version).layer(DefaultBodyLimit::max(FILE_BODY_LIMIT_BYTES)),
        )
        .route("/files/{file_id}/download", get(file_download))
        .route("/files/{file_id}/preview", get(file_preview))
        .route(
            "/file-versions/{version_id}/preview",
            get(file_version_preview),
        )
        .route(
            "/file-versions/{version_id}/download",
            get(version_download),
        )
        .route(
            "/me/files/{version_id}/download",
            get(version_download_personal),
        )
        .route("/me/files/{version_id}/text", get(me_file_text))
        .route("/me/files/{version_id}/inline", get(me_file_inline))
        .route("/me/files/{version_id}/thumbnail", get(me_file_thumbnail))
        .route("/me/files/{version_id}/raw", get(me_file_raw))
        .route("/me/files/rename", post(me_file_rename))
        .route("/me/files/move", post(me_file_move))
        .route("/me/files/batch/move", post(me_files_batch_move))
        .route("/me/files/batch/delete", post(me_files_batch_delete))
        .route("/me/files/favourite", post(me_file_favourite))
        .route("/me/files/delete", post(me_file_delete))
        .route("/me/files/restore", post(me_file_restore))
        .route("/me/files/purge", post(me_file_purge))
        .route("/files/trash/empty", post(me_files_purge_all))
        .route("/me/folders", post(me_folder_new))
        .route("/me/folders/rename", post(me_folder_rename))
        .route("/me/folders/delete", post(me_folder_delete))
        // Inteligência
        .route("/ai", get(ai_hub))
        .route("/ai/agents", get(agents))
        .route("/ai/agents/new", get(new_agent).post(create_agent))
        .route("/ai/agents/{agent_id}", get(agent_detail))
        .route("/ai/prompt", get(prompt).post(submit_prompt))
        .route("/compute", get(compute))
        // Institucional
        .route("/activity", get(activity))
        .route("/admin", get(admin))
        .route("/admin/instance", get(admin_instance).post(save_instance))
        .route("/admin/members/new", get(new_member).post(create_member))
        .route("/admin/members/{person_id}", get(member_detail))
        .route(
            "/admin/members/{person_id}/position",
            post(member_set_position),
        )
        .route("/admin/members/{person_id}/delete", post(member_delete))
        .route(
            "/admin/members/{person_id}/provision",
            post(provision_member),
        )
        .route("/admin/members/{person_id}/units", post(member_unit_assign))
        .route(
            "/admin/members/{person_id}/units/{unit_id}/role",
            post(member_unit_role),
        )
        .route(
            "/admin/members/{person_id}/units/{unit_id}/remove",
            post(member_unit_remove),
        )
        .route(
            "/admin/members/{person_id}/workspaces",
            post(member_workspace_assign),
        )
        .route(
            "/admin/members/{person_id}/workspaces/{workspace_id}/role",
            post(member_workspace_role),
        )
        .route(
            "/admin/members/{person_id}/workspaces/{workspace_id}/remove",
            post(member_workspace_remove),
        )
        .route(
            "/admin/members/{person_id}/reset-password",
            post(member_reset_password),
        )
        .route("/admin/members/{person_id}/status", post(member_set_status))
        .route("/admin/members/{person_id}/roles", post(member_role_grant))
        .route(
            "/admin/members/{person_id}/roles/{role}/revoke",
            post(member_role_revoke),
        )
        .route(
            "/admin/members/{person_id}/grants",
            post(member_grant_create),
        )
        .route(
            "/admin/members/{person_id}/grants/{grant_id}/revoke",
            post(member_grant_revoke),
        )
        .route(
            "/admin/members/{person_id}/sessions/{session_id}/revoke",
            post(member_session_revoke),
        )
        // O Gestor de Janelas (D002).
        .route("/wm", get(wm_list).post(wm_open))
        .route("/wm/{window_id}", post(wm_op))
        .route("/wm/{window_id}/close", post(wm_close))
        .route("/wm/{window_id}/state", post(wm_report))
        .route("/audit", get(audit))
        .route("/search", get(search))
        // A Universal Command Surface.
        .route("/ask", get(ask))
        .route("/ask/plans/{plan_id}/execute", post(execute_plan))
        .route("/ask/plans/{plan_id}/reject", post(reject_plan))
        // Autenticação
        .route("/boot", get(boot_screen))
        .route("/login", get(login).post(login_submit))
        .route("/login/language", post(login_language))
        .route("/password/recover", get(password_recover))
        .route("/first-access", get(first_access).post(first_access_submit))
        .route("/mfa", get(mfa_page))
        .route("/mfa/confirm", post(mfa_confirm))
        .route("/mfa/acknowledge", post(mfa_acknowledge))
        .route("/mfa/challenge", post(mfa_challenge))
        .route("/mfa/recovery", post(mfa_recovery))
        .route("/logout", post(logout))
        .route("/health", get(health))
        .nest_service("/static", ServeDir::new(state.config.static_dir.clone()))
        .fallback(not_found)
        // As páginas de erro do Design (D001.1) desenham-se à saída, dentro do
        // idioma do pedido, sobre o estado que a rota escolheu.
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            error_pages,
        ))
        // O portão de arranque corre **antes** de qualquer página ser
        // construída. Uma pessoa que abra o Ocinye OS vê o arranque, e não o
        // Workspace a ser escondido depois.
        .layer(axum::middleware::from_fn(boot_gate))
        // O idioma do pedido entra no escopo antes de qualquer página se
        // construir: assim `t(...)` lê a língua certa em toda a renderização, sem
        // ser fiado por centenas de assinaturas (i18n §5).
        .layer(axum::middleware::from_fn(locale_layer))
        // A rota real do pedido, para a janela que a página abre (D002).
        .layer(axum::middleware::from_fn(request_layer))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            same_origin_only,
        ))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            security_headers,
        ))
        .with_state(state)
}

/// A resposta de todas as páginas enquanto a interface não existe.
///
/// A UI foi apagada por inteiro (ramo `chore/ui-wipe`) à espera do código do
/// Claude Design. As rotas, a sessão e as acções continuam; o que uma pessoa
/// veria responde `503` com um código estável, sem HTML.
fn interface_pending() -> Response {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
        "interface_pending",
    )
        .into_response()
}

/// Resolve o idioma do pedido e corre o resto dentro do seu escopo.
///
/// A língua ambiente vem **só** de uma escolha explícita — o cookie `oc_locale`,
/// a cópia da preferência escrita na primeira entrada e na mudança. Sem escolha,
/// o canónico: o português. O `Accept-Language` do browser **não** decide a
/// língua da página, de propósito — deixar que decidisse mudava a língua de quem
/// nunca a escolheu, só por abrir o Ocinye de outro browser ou país (i18n §15,
/// §62). O que ele declara serve, na fatia seguinte, para pré-seleccionar a
/// opção no ecrã de primeira entrada — não para impor.
///
/// Nenhuma via consulta o Core: a preferência já foi espelhada no cookie no
/// momento certo, para que uma navegação não pague uma ida ao Core só para saber
/// a língua (i18n §67).
async fn locale_layer(request: axum::extract::Request, next: axum::middleware::Next) -> Response {
    let locale = locale_do_pedido(request.headers());
    crate::i18n::with_locale(locale, next.run(request)).await
}

/// Põe o caminho e a pergunta do pedido ao alcance do Gestor de Janelas.
async fn request_layer(request: axum::extract::Request, next: axum::middleware::Next) -> Response {
    let path_and_query = request.uri().path_and_query().map_or_else(
        || request.uri().path().to_owned(),
        |p| p.as_str().to_owned(),
    );
    controllers::windows::with_request(path_and_query, next.run(request)).await
}

/// A língua que este pedido deve falar: a escolhida, ou o canónico.
fn locale_do_pedido(headers: &HeaderMap) -> ocinye_contracts::Locale {
    let cookie = headers.get(header::COOKIE).and_then(|v| v.to_str().ok());
    session::locale_from_cookies(cookie)
        .as_deref()
        .and_then(ocinye_contracts::Locale::normalize)
        .unwrap_or(ocinye_contracts::locale::CANONICAL)
}

/// O portão de arranque.
///
/// # O que faz
///
/// Um pedido de documento que chegue a este Workspace sem ter visto o arranque
/// nesta janela é encaminhado para `/boot`, com o destino original preservado.
///
/// # Porque é que isto é um portão e não um estado dentro das páginas
///
/// Porque a alternativa é cada página decidir por si se já houve arranque — e
/// uma página nova que se esqueça disso passa a ser a porta de trás. Um portão
/// vale para tudo o que passa, incluindo o que ainda não foi escrito.
///
/// # O que não é encaminhado
///
/// O próprio arranque, os estáticos, a sonda de saúde, e tudo o que não é um
/// documento. Um pedido de folha de estilos ou de imagem não é uma pessoa a
/// abrir o Ocinye OS.
///
/// As submissões de formulário também não: encaminhar um `POST` perderia o que
/// alguém escreveu, e o arranque já terá acontecido antes de haver formulário
/// para submeter.
/// Não recebe estado de propósito: o portão decide pelo pedido, e um portão que
/// consultasse o Core a cada navegação seria um monitor contínuo. A observação
/// contínua é da topbar; isto é o ciclo de entrada.
async fn boot_gate(request: axum::extract::Request, next: axum::middleware::Next) -> Response {
    let caminho = request.uri().path().to_owned();
    let metodo = request.method().clone();

    let dispensado = metodo != axum::http::Method::GET
        || caminho == "/boot"
        || caminho == "/health"
        || caminho.starts_with("/static/")
        || caminho.starts_with("/avatar/");

    if dispensado {
        return next.run(request).await;
    }

    // Um pedido que não pede HTML não é uma pessoa a abrir o sistema.
    let quer_html = request
        .headers()
        .get(axum::http::header::ACCEPT)
        .and_then(|v| v.to_str().ok())
        .is_none_or(|v| v.contains("text/html") || v.contains("*/*"));
    if !quer_html {
        return next.run(request).await;
    }

    let cookies = request
        .headers()
        .get(axum::http::header::COOKIE)
        .and_then(|v| v.to_str().ok());

    if crate::boot::has_marker(cookies) {
        return next.run(request).await;
    }

    let destino = match request.uri().query() {
        Some(consulta) => format!("{caminho}?{consulta}"),
        None => caminho,
    };
    let destino =
        crate::boot::safe_return_target(&destino, ROUTES).unwrap_or_else(|| "/".to_owned());

    Redirect::to(&format!("/boot?return_to={}", urlencoding_minimo(&destino))).into_response()
}

/// Codifica um destino para caber numa cadeia de consulta.
///
/// Mínimo de propósito: o destino já passou pela validação contra o catálogo, e
/// o que aqui falta é apenas não partir a própria consulta.
fn urlencoding_minimo(valor: &str) -> String {
    valor
        .chars()
        .map(|c| match c {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '/' | '-' | '_' | '.' | '~' => c.to_string(),
            outro => outro
                .to_string()
                .bytes()
                .map(|b| format!("%{b:02X}"))
                .collect(),
        })
        .collect()
}

/// Cabeçalhos aplicados a todas as respostas.
///
/// A política de conteúdo permite o stylesheet e o script próprios, as fontes
/// do Google e mais nada: sem origens de terceiros, sem inline, sem frames.
///
/// # O transporte, e porque só em produção
///
/// `Strict-Transport-Security` diz ao browser para nunca mais falar com esta
/// origem em claro. É a defesa contra o primeiro pedido — aquele que acontece
/// antes de qualquer redireccionamento para HTTPS, e onde um intermediário
/// ainda tem uma palavra a dizer.
///
/// Sai apenas quando a instalação é de produção, e aí a configuração já exigiu
/// que `OCINYE_WORKSPACE_PUBLIC_URL` seja `https` e que o cookie de sessão seja
/// `Secure`. Enviá-lo em desenvolvimento, onde o Workspace corre em claro,
/// trancaria o `localhost` do browser de quem desenvolve durante um ano.
async fn security_headers(
    State(state): State<WorkspaceState>,
    request: axum::extract::Request,
    next: axum::middleware::Next,
) -> Response {
    let producao = state.config.is_production;
    let mut response = next.run(request).await;

    if producao {
        response
            .headers_mut()
            .entry("strict-transport-security")
            .or_insert(HeaderValue::from_static(
                "max-age=31536000; includeSubDomains",
            ));
    }

    const HEADERS: &[(&str, &str)] = &[
        ("x-content-type-options", "nosniff"),
        ("x-frame-options", "DENY"),
        ("referrer-policy", "same-origin"),
        ("cross-origin-opener-policy", "same-origin"),
        (
            "content-security-policy",
            "default-src 'none'; \
             script-src 'self'; \
             style-src 'self'; \
             font-src 'self'; \
             img-src 'self' data:; \
             connect-src 'self'; \
             frame-src 'self'; \
             form-action 'self'; base-uri 'none'; frame-ancestors 'none'",
        ),
        (
            "permissions-policy",
            "geolocation=(), microphone=(), camera=()",
        ),
        // As páginas são por membro; uma cache partilhada nunca as pode reter.
        ("cache-control", "no-store"),
    ];

    for (name, value) in HEADERS {
        if let (Ok(name), Ok(value)) = (
            header::HeaderName::from_bytes(name.as_bytes()),
            HeaderValue::from_str(value),
        ) {
            response.headers_mut().entry(name).or_insert(value);
        }
    }
    response
}

/// Recusa escritas que não venham desta origem.
///
/// # Porque `SameSite` não chega
///
/// A sessão do Workspace é um cookie `SameSite=Lax`, e isso bloqueia um `POST`
/// vindo de **outro site**. Mas «site» não é «origem»: `SameSite` compara o
/// domínio registável, por isso uma página em `ocinye.com` — que o `CLAUDE.md`
/// §5 reserva para o futuro website público — é *same-site* com
/// `workspace.ocinye.com`, e o browser envia o cookie com ela.
///
/// O mesmo vale para qualquer XSS num subdomínio irmão: passaria a ser um CSRF
/// contra o Workspace. Um subdomínio não é uma fronteira de confiança
/// (`CLAUDE.md` §16).
///
/// # A regra
///
/// Em métodos que alteram estado, o `Origin` tem de existir e tem de ser esta
/// origem. Os browsers enviam-no em todos os `POST`, incluindo os do próprio
/// sítio, por isso exigi-lo não parte nada — e a sua ausência num pedido do
/// browser é ela própria anómala. `GET` e `HEAD` não alteram estado e não são
/// verificados (nenhuma rota do Workspace muda estado por `GET`, o que este
/// desenho pressupõe e o teste abaixo prende).
async fn same_origin_only(
    State(state): State<WorkspaceState>,
    request: axum::extract::Request,
    next: axum::middleware::Next,
) -> Response {
    if !changes_state(request.method()) {
        return next.run(request).await;
    }

    let origin = request
        .headers()
        .get(header::ORIGIN)
        .and_then(|value| value.to_str().ok());

    let host = request
        .headers()
        .get(header::HOST)
        .and_then(|value| value.to_str().ok());

    if origin_is_ours(origin, host, &state.config.public_url) {
        return next.run(request).await;
    }

    // Sem detalhe: quem sondar a fronteira não recebe um mapa dela.
    tracing::warn!(
        path = %request.uri().path(),
        "refused a state-changing request from another origin"
    );
    (StatusCode::FORBIDDEN, interface_pending()).into_response()
}

/// Métodos que podem alterar estado.
fn changes_state(method: &axum::http::Method) -> bool {
    !matches!(
        *method,
        axum::http::Method::GET | axum::http::Method::HEAD | axum::http::Method::OPTIONS
    )
}

/// Se um `Origin` é o próprio Workspace.
///
/// A origem pública configurada é a resposta em qualquer deployment real.
///
/// # A tolerância local, e o seu limite
///
/// Fora de produção, aceita-se também um `Origin` cujo host coincida com o
/// `Host` do pedido — o mesmo processo responde em `localhost` e em
/// `127.0.0.1`, e o `Host` é preenchido pelo browser com o alvo real, não pelo
/// atacante. Essa tolerância vale **apenas quando a origem configurada não é
/// `https`**, isto é, em desenvolvimento.
///
/// Sem esse limite, comparar só o host aceitaria `http://` num Workspace
/// servido em `https://`, o que é uma despromoção de esquema: alguém na rede
/// que sirva uma página em claro no mesmo nome de host voltaria a poder
/// escrever. A configuração de produção já exige `https`
/// ([`WorkspaceConfig::validate`](crate::config::WorkspaceConfig)), por isso
/// esta linha e essa dizem a mesma coisa.
fn origin_is_ours(origin: Option<&str>, host: Option<&str>, public_url: &str) -> bool {
    let Some(origin) = origin.map(str::trim).filter(|value| !value.is_empty()) else {
        return false;
    };
    // `null` é o que um browser envia a partir de um `iframe` sandboxed ou de
    // um documento `data:`. Nunca é esta origem.
    if origin.eq_ignore_ascii_case("null") {
        return false;
    }

    if origin.trim_end_matches('/') == public_url.trim_end_matches('/') {
        return true;
    }

    if public_url.starts_with("https://") {
        return false;
    }

    let Some(("http", origin_host)) = origin.split_once("://") else {
        return false;
    };

    matches!(host, Some(host) if origin_host.trim_end_matches('/').eq_ignore_ascii_case(host))
}

async fn health() -> &'static str {
    "ok"
}

// ── Sessão ───────────────────────────────────────────────────────────────

/// O membro activo.
struct Member {
    session: Session,
    /// O identificador opaco da sessão no registo do Workspace: a chave da
    /// mesa de janelas. Nunca sai deste processo senão no cookie.
    session_id: String,
    correlation_id: String,
}

/// Para onde vai quem não tem sessão (D12).
///
/// Um browser que traz um cookie de sessão que o Workspace já não conhece teve
/// uma sessão que acabou: diz-se «a sessão expirou». Sem cookie, é uma entrada
/// como outra qualquer.
fn destino_de_entrada(headers: &HeaderMap) -> &'static str {
    let cookie = headers.get(header::COOKIE).and_then(|v| v.to_str().ok());
    if session::session_id_from_cookies(cookie).is_some() {
        "/login?reason=expired"
    } else {
        "/login"
    }
}

fn current_member(state: &WorkspaceState, headers: &HeaderMap) -> Option<Member> {
    let cookie = headers
        .get(header::COOKIE)
        .and_then(|value| value.to_str().ok());
    let id = session::session_id_from_cookies(cookie)?;
    let session = state.sessions.get(&id)?;
    Some(Member {
        session,
        session_id: id,
        correlation_id: Uuid::new_v4().to_string(),
    })
}

/// Obtém um valor do Core, devolvendo `Null` quando um painel isolado falha.
///
/// Um ecrã com vários painéis deve continuar a renderizar quando um deles não
/// carrega; o painel mostra o seu próprio estado vazio.
/// Obtém um valor do Core, distinguindo recusa de ausência.
///
/// [`optional`] engole tudo em `Null`, o que é certo para um painel isolado
/// dentro de um ecrã e **errado** para o conteúdo principal: um 403 ou 404
/// renderizado como lista vazia diz «não existe nenhum» a quem apenas não pode
/// ver (briefing §57).
async fn required(
    state: &WorkspaceState,
    member: &Member,
    path: &str,
) -> Result<Value, ApiFailure> {
    api::get::<Value>(
        state,
        &member.session.access_token,
        &member.correlation_id,
        path,
    )
    .await
}

async fn optional(state: &WorkspaceState, member: &Member, path: &str) -> Value {
    api::get::<Value>(
        state,
        &member.session.access_token,
        &member.correlation_id,
        path,
    )
    .await
    .unwrap_or(Value::Null)
}

/// Uma resposta de erro que a página do Design (D001.1) vai desenhar.
///
/// As rotas devolvem o estado e esta marca; [`error_pages`] troca o corpo pela
/// página certa — dentro da casca se o pedido traz um membro, à porta se não —
/// sem mexer no estado HTTP. Assim as dezenas de acções que já devolviam um
/// 404/403/502 ganham a página sem cada uma ter de saber desenhá-la.
#[derive(Clone, Debug)]
struct ErrorPage {
    kind: ErrorKind,
    reference: Option<String>,
}

fn error_marker(estado: StatusCode, kind: ErrorKind, reference: Option<String>) -> Response {
    let mut resposta = (estado, kind.as_str().to_owned()).into_response();
    resposta
        .extensions_mut()
        .insert(ErrorPage { kind, reference });
    resposta
}

/// Troca o corpo das respostas marcadas por [`error_marker`] pela página de
/// erro do Design, mantendo o estado. Só para quem pede um documento.
async fn error_pages(
    State(state): State<WorkspaceState>,
    request: axum::extract::Request,
    next: axum::middleware::Next,
) -> Response {
    let headers = request.headers().clone();
    let get = request.method() == axum::http::Method::GET;
    let caminho = request
        .uri()
        .path_and_query()
        .map_or_else(|| "/".to_owned(), ToString::to_string);
    let resposta = next.run(request).await;
    let Some(pagina) = resposta.extensions().get::<ErrorPage>().cloned() else {
        return resposta;
    };
    let quer_html = headers
        .get(header::ACCEPT)
        .and_then(|v| v.to_str().ok())
        .is_none_or(|v| v.contains("text/html") || v.contains("*/*"));
    if !quer_html {
        return resposta;
    }
    let estado = resposta.status();
    let vm = ErrorVm {
        kind: pagina.kind,
        reference: pagina.reference,
        // «Tentar de novo» só repete uma leitura; nunca reenvia um formulário.
        retry_href: (get && pagina.kind == ErrorKind::Upstream)
            .then(|| crate::boot::safe_return_target(&caminho, ROUTES))
            .flatten(),
    };
    (estado, error_document(&state, &headers, &vm).await).into_response()
}

fn error_title(kind: ErrorKind) -> &'static str {
    crate::i18n::t(match kind {
        ErrorKind::NotFound => "error.404.title",
        ErrorKind::Forbidden => "error.403.title",
        ErrorKind::Upstream => "error.502.title",
    })
}

/// A página de erro: na casca, se o pedido traz um membro cuja identidade o
/// Core confirma; à porta em qualquer outro caso.
async fn error_document(state: &WorkspaceState, headers: &HeaderMap, vm: &ErrorVm) -> Response {
    let membro = current_member(state, headers)
        .filter(|m| !m.session.must_change_password && !m.session.mfa_required);
    if let Some(member) = membro {
        let quem = caller(&member);
        if let Shell::Ready(ctx) = controllers::shell(state, &quem, "", String::new()).await {
            return html(
                error_title(vm.kind),
                Surface::Shell,
                ui::screens::error::in_shell(&ctx.vm, vm),
            );
        }
    }
    html(
        error_title(vm.kind),
        Surface::Auth,
        ui::screens::error::at_door(&controllers::door(state).await, vm),
    )
}

/// Traduz uma recusa do Core em algo sobre que o membro possa agir.
fn failure_response(failure: &ApiFailure) -> Response {
    // Recusa e inexistência continuam a ter o mesmo aspecto (ADR-0100), e o
    // detalhe de uma avaria vai para o log, nunca para a resposta: 404, 403 e
    // 502 levam a página do Design (D001.1); os outros, o código estável.
    let (estado, codigo) = match failure {
        ApiFailure::Unauthorised => return Redirect::to("/login").into_response(),
        ApiFailure::Denied => {
            return error_marker(StatusCode::NOT_FOUND, ErrorKind::NotFound, None)
        }
        ApiFailure::Forbidden => {
            return error_marker(StatusCode::FORBIDDEN, ErrorKind::Forbidden, None)
        }
        ApiFailure::Unavailable(_) => (StatusCode::SERVICE_UNAVAILABLE, "unavailable"),
        ApiFailure::ApplicationInactive => {
            (StatusCode::SERVICE_UNAVAILABLE, "application_inactive")
        }
        ApiFailure::Rejected(_) => (StatusCode::UNPROCESSABLE_ENTITY, "rejected"),
        ApiFailure::Conflict(_) | ApiFailure::Refused { .. } => (StatusCode::CONFLICT, "conflict"),
        ApiFailure::Failed(message) => {
            return error_marker(
                StatusCode::BAD_GATEWAY,
                ErrorKind::Upstream,
                Some(controllers::reference(message)),
            );
        }
    };
    (estado, codigo).into_response()
}

/// Caminho que o Workspace não serve: 404, com a página do Design.
async fn not_found() -> Response {
    error_marker(StatusCode::NOT_FOUND, ErrorKind::NotFound, None)
}

// ── Arranque ─────────────────────────────────────────────────────────────

/// O que o arranque recebe de quem o pede.
#[derive(Deserialize)]
struct BootQuery {
    /// Para onde seguir quando o Core deixar.
    #[serde(default)]
    return_to: Option<String>,
}

/// `GET /boot` — o arranque institucional, como o `/ready` o disse.
///
/// Quem já tem sessão válida numa Instância pronta não está a entrar: segue
/// directamente para onde ia (a ligação profunda que o portão guardou), em vez
/// de passar pelo «Continuar para o início de sessão» do ecrã de arranque.
async fn boot_screen(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Query(query): Query<BootQuery>,
) -> Response {
    let destino = query
        .return_to
        .as_deref()
        .and_then(|d| crate::boot::safe_return_target(d, ROUTES))
        .unwrap_or_else(|| "/".to_owned());
    let (vm, segue) = controllers::boot(&state).await;

    let mut resposta = if segue && current_member(&state, &headers).is_some() {
        Redirect::to(&destino).into_response()
    } else {
        let titulo = if segue {
            "auth.boot.ready_title"
        } else {
            "auth.boot.blocked_title"
        };
        html(
            crate::i18n::t(titulo),
            Surface::Auth,
            ui::screens::auth::boot::boot(&vm),
        )
    };
    // Uma prontidão em cache é uma resposta sobre um sistema que já não existe.
    resposta.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("no-store, no-cache, must-revalidate"),
    );
    // O marcador só se grava quando houve por onde seguir: gravá-lo num arranque
    // bloqueado faria a tentativa seguinte saltar um problema que continua lá.
    if segue {
        if let Ok(valor) =
            HeaderValue::from_str(&crate::boot::marker_cookie(state.config.cookie_secure))
        {
            resposta.headers_mut().append(header::SET_COOKIE, valor);
        }
    }
    resposta
}

/// Macro de guarda: sem sessão, vai para o login.
macro_rules! member_or_login {
    ($state:expr, $headers:expr) => {
        match current_member(&$state, &$headers) {
            // Quem deve ao Core uma palavra-passe definitiva não passa daqui.
            // O Core recusaria na mesma; isto poupa-lhe um erro em vez de um
            // ecrã (briefing §22).
            Some(member) if member.session.must_change_password => {
                return Redirect::to("/first-access").into_response()
            }
            // E quem deve um segundo factor está num fluxo de autenticação, não
            // numa sessão normal: nenhuma superfície do Workspace — nem a faixa
            // privilegiada — se mostra antes de o MFA estar satisfeito (ADR-0107).
            Some(member) if member.session.mfa_required => {
                return Redirect::to("/mfa").into_response()
            }
            Some(member) => member,
            None => return Redirect::to(destino_de_entrada(&$headers)).into_response(),
        }
    };
}

// ── Interface (Claude Design D001) ─────────────────────────────────────────

/// Um documento do Design: a superfície decide o CSS e o JS, e o tema.
fn html(title: &str, surface: Surface, body: impl leptos::IntoView + 'static) -> Response {
    let theme = match surface {
        Surface::Auth => Theme::Dark,
        Surface::Shell => Theme::Light,
    };
    let doc = DocumentVm {
        title: title.to_owned(),
        surface,
        theme,
    };
    Html(ui::document::render(&doc, body)).into_response()
}

fn caller(member: &Member) -> Caller<'_> {
    Caller {
        session: &member.session,
        correlation_id: &member.correlation_id,
    }
}

/// A identidade da sessão ficou por estabelecer: o Core não deu resposta
/// autoritária ao `/me`. Falha fechado (D001.1): nenhuma casca autenticada,
/// só «Tentar de novo», «Terminar sessão» e a referência, com 503.
async fn identity_indeterminate(
    state: &WorkspaceState,
    reference: String,
    retry_href: &str,
) -> Response {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        html(
            crate::i18n::t("auth.identity.title"),
            Surface::Auth,
            ui::screens::auth::identity::identity_unavailable(&IdentityFailVm {
                door: controllers::door(state).await,
                reference: Some(reference),
                retry_href: retry_href.to_owned(),
            }),
        ),
    )
        .into_response()
}

/// O Core já não reconhece o token desta sessão: a sessão acabou.
fn session_ended(state: &WorkspaceState, headers: &HeaderMap) -> Response {
    if let Some(id) =
        session::session_id_from_cookies(headers.get(header::COOKIE).and_then(|v| v.to_str().ok()))
    {
        state.sessions.remove(&id);
    }
    (
        [(
            header::SET_COOKIE,
            session::clear_cookie_header(state.config.cookie_secure),
        )],
        Redirect::to("/login?reason=expired"),
    )
        .into_response()
}

/// Se a aplicação deste ecrã se oferece a este membro. Um ecrã fora do registo
/// de aplicações (Home, pesquisa, notificações) é de toda a gente.
fn screen_open(ctx: &ShellContext, screen: Screen) -> bool {
    experience::apps::APPLICATIONS
        .iter()
        .find(|a| a.screen == screen)
        .is_none_or(|a| a.visible_to(&ctx.viewer, ctx.core))
}

/// Uma rota de aplicação cujo ecrã o Design ainda não entregou (D002+).
///
/// A janela `app_pending` na casca, para que nenhuma ligação fique morta. Quem
/// não pode ver a aplicação recebe o mesmo que para uma rota inexistente: a
/// grelha não anuncia o que o membro não abre, e a rota escrita à mão também não.
async fn app_page(state: &WorkspaceState, headers: &HeaderMap, screen: Screen) -> Response {
    let title = screen.label().to_owned();
    pending_page(state, headers, Some(screen), title, screen.path()).await
}

async fn pending_page(
    state: &WorkspaceState,
    headers: &HeaderMap,
    gate: Option<Screen>,
    title: String,
    href: &'static str,
) -> Response {
    let member = member_or_login!(state, headers);
    let quem = caller(&member);
    match controllers::shell(state, &quem, href, title.clone()).await {
        Shell::Ready(mut ctx) => {
            if gate.is_some_and(|screen| !screen_open(&ctx, screen)) {
                let vm = ErrorVm {
                    kind: ErrorKind::NotFound,
                    reference: None,
                    retry_href: None,
                };
                return (
                    StatusCode::NOT_FOUND,
                    html(
                        error_title(vm.kind),
                        Surface::Shell,
                        ui::screens::error::in_shell(&ctx.vm, &vm),
                    ),
                )
                    .into_response();
            }
            // D002: uma rota de aplicação abre (ou foca) a sua janela. Só
            // depois do portão acima — uma aplicação escondida nunca ganha
            // janela — e só para ecrãs que são aplicações do registo.
            let app = gate
                .and_then(controllers::windows::application_of)
                .and_then(|a| a.id().parse::<ApplicationId>().ok());
            let q = controllers::windows::page_query(href);
            let mut status = StatusCode::OK;
            if let Some(app) = app {
                if q.frame {
                    return window_frame(title);
                }
                let opened = controllers::windows::open(
                    &state.sessions,
                    &member.session_id,
                    app,
                    &q.href,
                    q.new_window,
                );
                if matches!(opened, Some(Err(window_manager::WmError::TooMany))) {
                    status = StatusCode::CONFLICT;
                } else if q.new_window {
                    // «Nova janela» é uma acção, não um endereço: depois de a
                    // abrir, o endereço passa a ser o da janela, para que
                    // recarregar não abra outra (POST/redirect/GET).
                    return Redirect::to(&q.href).into_response();
                }
                ctx.vm.wm = controllers::windows::view(&state.sessions, &member.session_id, &ctx);
            }
            let dialog = q
                .close
                .as_deref()
                .and_then(|id| {
                    controllers::windows::dirty_close(&state.sessions, &member.session_id, id)
                })
                .map(|d| dirty_dialog(&d));
            let engine = ctx.vm.wm.is_some();
            let page_title = title.clone();
            let body = ui::shell::app_pending(&ctx.vm, title, href);
            (status, shell_page(&page_title, engine, body, dialog)).into_response()
        }
        Shell::SignIn => session_ended(state, headers),
        Shell::Indeterminate(reference) => identity_indeterminate(state, reference, href).await,
    }
}

/// Uma página da casca. Com janelas, carrega também o motor (`wm-engine.js`,
/// de Code) depois dos scripts do Design; sem janelas, é exactamente o
/// documento D001. `dialog` é a confirmação de fechar uma janela com trabalho
/// por guardar (FG-026).
fn shell_page(
    title: &str,
    engine: bool,
    body: impl leptos::IntoView + 'static,
    dialog: Option<leptos::prelude::AnyView>,
) -> Response {
    use leptos::prelude::*;
    // Um diálogo bloqueante por resposta, depois da casca: o de fechar com
    // trabalho por guardar ou a confirmação da Nye (HANDOFF D003 §4).
    let body = view! { {body}{dialog} };
    let doc = DocumentVm {
        title: title.to_owned(),
        surface: Surface::Shell,
        theme: Theme::Light,
    };
    let mut page = ui::document::render(&doc, body);
    if engine {
        // `document.rs` é do Design e não tem lugar para um script de Code
        // (D002_CONTRACT_GAP em CODE_FEEDBACK): entra no fim do `<head>`, com
        // `defer`, depois do `oc-wm.js` de que depende.
        if let Some(at) = page.find("</head>") {
            // D004: o motor de envio e de mover de Ficheiros, também de Code; só
            // age quando a aplicação Ficheiros está na página.
            page.insert_str(
                at,
                r#"<script src="/static/wm-engine.js" defer></script><script src="/static/files-engine.js" defer></script>"#,
            );
        }
    }
    Html(page).into_response()
}

/// O diálogo de fechar com trabalho por guardar, como diálogo da página.
fn dirty_dialog(d: &ui::view_models::DirtyCloseVm) -> leptos::prelude::AnyView {
    use leptos::prelude::*;
    ui::wm::dirty_close(d).into_any()
}

/// `GET {rota}?frame=1` (WM-4): só o corpo da janela, com o título num
/// `<template>`. As aplicações ainda sem ecrã do Design dão o estado
/// `app_pending`.
fn window_frame(title: String) -> Response {
    use leptos::prelude::*;
    let body = view! {
        <template data-part="win-title">{title}</template>
        {ui::components::pending("oc-app-pending", "shell.app.pending")}
    };
    ([(header::CACHE_CONTROL, "no-store")], Html(body.to_html())).into_response()
}

// ── Gestor de Janelas (D002 · FG-010, FG-026, FG-027) ─────────────────────
//
// O motor decide; o JavaScript pede. Cada operação valida a janela (tem de ser
// desta sessão), a operação e os números, e responde com o estado inteiro
// (JS, `Accept: application/json`) ou volta à rota da janela activa (sem JS,
// 303). Nenhuma operação autoriza nada: abrir passa pelo mesmo portão da rota
// da aplicação, e o conteúdo de cada janela continua a ser pedido ao Core.

/// A resposta de uma operação: o estado (JS) ou a rota activa (sem JS).
fn wm_reply(headers: &HeaderMap, desk: &window_manager::Desk) -> Response {
    if aceita_json(headers) {
        return axum::Json(controllers::windows::state_json(desk)).into_response();
    }
    Redirect::to(desk.current_href()).into_response()
}

fn wm_refused(status: StatusCode, reason: &'static str) -> Response {
    (status, axum::Json(serde_json::json!({ "reason": reason }))).into_response()
}

fn wm_error(
    headers: &HeaderMap,
    error: &window_manager::WmError,
    desk: &window_manager::Desk,
    id: &str,
) -> Response {
    use window_manager::WmError;
    match error {
        // Um identificador que não é desta sessão é igual a um que não existe.
        WmError::NoSuchWindow => wm_refused(StatusCode::NOT_FOUND, "no_such_window"),
        WmError::TooMany => wm_refused(StatusCode::CONFLICT, "too_many_windows"),
        WmError::CannotSave => wm_refused(StatusCode::CONFLICT, "cannot_save"),
        // Trabalho por guardar: a decisão é pedida onde a janela está.
        WmError::Dirty => {
            let confirm = with_close_param(desk.current_href(), id);
            if aceita_json(headers) {
                (
                    StatusCode::CONFLICT,
                    axum::Json(serde_json::json!({ "reason": "dirty", "confirm": confirm })),
                )
                    .into_response()
            } else {
                Redirect::to(&confirm).into_response()
            }
        }
    }
}

/// A rota com `close={id}` acrescentado (o diálogo desenha-se nela).
fn with_close_param(href: &str, id: &str) -> String {
    let sep = if href.contains('?') { '&' } else { '?' };
    format!("{href}{sep}close={id}")
}

/// `GET /wm` (WM-1): as janelas desta sessão.
async fn wm_list(State(state): State<WorkspaceState>, headers: HeaderMap) -> Response {
    let Some(member) = membro_ou_recusa(&state, &headers) else {
        return nao_autenticado();
    };
    match state
        .sessions
        .with_desk(&member.session_id, |d| controllers::windows::state_json(d))
    {
        Some(json) => axum::Json(json).into_response(),
        None => nao_autenticado(),
    }
}

#[derive(Deserialize)]
struct WmOpenForm {
    app_id: String,
    #[serde(default)]
    href: Option<String>,
    #[serde(default)]
    window: Option<String>,
}

/// `POST /wm` (WM-3): abrir, ou focar a janela que já existe (uma janela só,
/// ou o mesmo recurso). A aplicação tem de ser visível a este membro — o mesmo
/// portão da sua rota: uma escondida responde como inexistente.
async fn wm_open(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Form(form): Form<WmOpenForm>,
) -> Response {
    let member = member_or_login!(state, headers);
    let Ok(app) = form.app_id.parse::<ApplicationId>() else {
        return wm_refused(StatusCode::NOT_FOUND, "no_such_application");
    };
    let href = form.href.unwrap_or_else(|| app.manifest().route.to_owned());
    if !window_manager::valid_href(app, &href) {
        return wm_refused(StatusCode::UNPROCESSABLE_ENTITY, "invalid_href");
    }
    let quem = caller(&member);
    let ctx = match controllers::shell(&state, &quem, app.manifest().route, String::new()).await {
        Shell::Ready(ctx) => ctx,
        Shell::SignIn => return session_ended(&state, &headers),
        Shell::Indeterminate(reference) => {
            return identity_indeterminate(&state, reference, "/").await
        }
    };
    let visible = controllers::windows::application_of_id(app)
        .is_some_and(|a| a.visible_to(&ctx.viewer, ctx.core));
    if !visible {
        return wm_refused(StatusCode::NOT_FOUND, "no_such_application");
    }
    let new_window = form.window.as_deref() == Some("new");
    let result =
        controllers::windows::open(&state.sessions, &member.session_id, app, &href, new_window);
    match result {
        None => nao_autenticado(),
        Some(Err(error)) => wm_refused(
            StatusCode::CONFLICT,
            if error == window_manager::WmError::TooMany {
                "too_many_windows"
            } else {
                "refused"
            },
        ),
        Some(Ok(opened)) => {
            let existing = matches!(opened, window_manager::Opened::Existing(_));
            if aceita_json(&headers) {
                axum::Json(serde_json::json!({
                    "id": opened.id(),
                    "existing": existing,
                    "href": href,
                }))
                .into_response()
            } else {
                Redirect::to(&href).into_response()
            }
        }
    }
}

#[derive(Deserialize)]
struct WmOpForm {
    op: String,
    #[serde(default)]
    zone: Option<String>,
    #[serde(default)]
    x: Option<i32>,
    #[serde(default)]
    y: Option<i32>,
    #[serde(default)]
    w: Option<u32>,
    #[serde(default)]
    h: Option<u32>,
    #[serde(default)]
    area_w: Option<u32>,
    #[serde(default)]
    area_h: Option<u32>,
}

/// `POST /wm/{id}` (WM-2): focar, minimizar, maximizar, restaurar, fechar,
/// encaixar, mover e redimensionar. Números que não são números, zonas e
/// operações desconhecidas são recusados antes de tocar no estado.
async fn wm_op(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Form(form): Form<WmOpForm>,
) -> Response {
    use window_manager::{Area, Geometry, Zone};
    let Some(member) = membro_ou_recusa(&state, &headers) else {
        return if aceita_json(&headers) {
            nao_autenticado()
        } else {
            Redirect::to(destino_de_entrada(&headers)).into_response()
        };
    };
    let area = match (form.area_w, form.area_h) {
        (Some(w), Some(h)) => match Area::new(w, h) {
            Some(a) => Some(a),
            None => return wm_refused(StatusCode::UNPROCESSABLE_ENTITY, "invalid_area"),
        },
        (None, None) => None,
        _ => return wm_refused(StatusCode::UNPROCESSABLE_ENTITY, "invalid_area"),
    };
    let zone = form.zone.as_deref().map(Zone::parse);
    let result = state.sessions.with_desk(&member.session_id, |d| {
        let outcome = match (form.op.as_str(), zone, form.x, form.y, form.w, form.h) {
            ("focus", None, ..) => d.focus(&id).map(|()| None),
            ("minimize", None, ..) => d.minimize(&id).map(|()| None),
            ("maximize", None, ..) => d.maximize(&id).map(|()| None),
            ("restore", None, ..) => d.restore(&id).map(|()| None),
            ("close", None, ..) => d.close(&id, None).map(|_| None),
            ("snap", Some(Some(z)), ..) => d.snap(&id, z).map(|()| None),
            ("move", None, Some(x), Some(y), None, None) => {
                d.move_to(&id, x, y, area).map(|()| None)
            }
            ("resize", None, Some(x), Some(y), Some(w), Some(h)) => {
                d.resize(&id, Geometry { x, y, w, h }, area).map(|()| None)
            }
            _ => Ok(Some(())),
        };
        (outcome, d.clone())
    });
    let Some((outcome, desk)) = result else {
        return nao_autenticado();
    };
    match outcome {
        Ok(None) => wm_reply(&headers, &desk),
        Ok(Some(())) => wm_refused(StatusCode::UNPROCESSABLE_ENTITY, "invalid_operation"),
        Err(error) => wm_error(&headers, &error, &desk, &id),
    }
}

#[derive(Deserialize)]
struct WmCloseForm {
    decision: String,
}

/// `POST /wm/{id}/close` (FG-026): a decisão do diálogo de fechar, executada
/// exactamente como foi confirmada.
async fn wm_close(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Form(form): Form<WmCloseForm>,
) -> Response {
    let Some(member) = membro_ou_recusa(&state, &headers) else {
        return nao_autenticado();
    };
    let Some(decision) = window_manager::Decision::parse(&form.decision) else {
        return wm_refused(StatusCode::UNPROCESSABLE_ENTITY, "invalid_decision");
    };
    let result = state.sessions.with_desk(&member.session_id, |d| {
        (d.close(&id, Some(decision)), d.clone())
    });
    let Some((outcome, desk)) = result else {
        return nao_autenticado();
    };
    match outcome {
        Ok(_) => wm_reply(&headers, &desk),
        Err(error) => wm_error(&headers, &error, &desk, &id),
    }
}

#[derive(Deserialize)]
struct WmReportForm {
    dirty: bool,
    #[serde(default)]
    can_save: bool,
}

/// `POST /wm/{id}/state`: a aplicação diz se tem trabalho por guardar e se o
/// pode guardar agora. É a única entrada do sinal «por guardar» (FG-026); o
/// motor não o adivinha. Depois de «Guardar», a notícia de que já não há
/// nada por guardar fecha a janela.
async fn wm_report(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Form(form): Form<WmReportForm>,
) -> Response {
    let Some(member) = membro_ou_recusa(&state, &headers) else {
        return nao_autenticado();
    };
    let result = state.sessions.with_desk(&member.session_id, |d| {
        (d.report(&id, form.dirty, form.can_save), d.clone())
    });
    let Some((outcome, desk)) = result else {
        return nao_autenticado();
    };
    match outcome {
        Ok(_) => axum::Json(controllers::windows::state_json(&desk)).into_response(),
        Err(error) => wm_error(&headers, &error, &desk, &id),
    }
}

// ── Desktop (D001 · FG-017) ────────────────────────────────────────────────

/// `PUT /me/desktop` — grava a disposição no Core. O Core valida e decide; o
/// Workspace só traduz o conflito para o `409` que o Desktop espera.
async fn desktop_save(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    axum::Json(pedido): axum::Json<Value>,
) -> Response {
    let Some(member) = membro_ou_recusa(&state, &headers) else {
        return nao_autenticado();
    };
    let resultado = api::put(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        "/api/v1/me/desktop",
        &pedido,
    )
    .await;
    match resultado {
        Err(ApiFailure::Conflict(mensagem)) => (
            StatusCode::CONFLICT,
            axum::Json(serde_json::json!({ "message": mensagem })),
        )
            .into_response(),
        outro => encaminhar(outro),
    }
}

/// `POST /me/desktop/restore` — repõe a predefinição. Com JS (`Accept: JSON`)
/// responde a disposição; sem JS, volta ao Desktop.
async fn desktop_restore(State(state): State<WorkspaceState>, headers: HeaderMap) -> Response {
    let Some(member) = membro_ou_recusa(&state, &headers) else {
        return nao_autenticado();
    };
    let resultado = api::post(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        "/api/v1/me/desktop/restore",
        &serde_json::json!({}),
    )
    .await;
    if aceita_json(&headers) {
        return encaminhar(resultado);
    }
    match resultado {
        Ok(_) => Redirect::to("/").into_response(),
        Err(falha) => failure_response(&falha),
    }
}

// ── Pessoal ──────────────────────────────────────────────────────────────

/// `GET /` — o Desktop (D001): a disposição do membro e os 14 widgets.
async fn home(State(state): State<WorkspaceState>, headers: HeaderMap) -> Response {
    let member = member_or_login!(state, headers);
    let quem = caller(&member);
    let crumb = Screen::Home.label().to_owned();
    match controllers::shell(&state, &quem, "/", crumb).await {
        Shell::Ready(mut ctx) => {
            // D002: o Desktop fica por baixo das janelas abertas.
            ctx.vm.wm = controllers::windows::view(&state.sessions, &member.session_id, &ctx);
            let q = controllers::windows::page_query("/");
            let dialog = q
                .close
                .as_deref()
                .and_then(|id| {
                    controllers::windows::dirty_close(&state.sessions, &member.session_id, id)
                })
                .map(|d| dirty_dialog(&d));
            let engine = ctx.vm.wm.is_some();
            let vm = controllers::desktop::desktop(*ctx, &quem, &state).await;
            shell_page(
                crate::i18n::t("desk.title"),
                engine,
                ui::screens::home::home(&vm),
                dialog,
            )
        }
        Shell::SignIn => session_ended(&state, &headers),
        Shell::Indeterminate(reference) => identity_indeterminate(&state, reference, "/").await,
    }
}

/// Sem ecrã entregue pelo Design: a janela `app_pending` na casca (D001).
async fn meus_recursos(State(state): State<WorkspaceState>, headers: HeaderMap) -> Response {
    app_page(&state, &headers, Screen::Resources).await
}

// ── Correio ──────────────────────────────────────────────────────────────
//
// Uma regra atravessa todos estes manipuladores: **apenas `send_mail` fala com
// o serviço de correio**. `assist` devolve texto e volta a desenhar o composer;
// não tem forma de enviar, e não é por convenção — é por não chamar a rota que
// envia (briefing §15).

#[derive(Deserialize)]
struct SyncForm {
    #[serde(default)]
    folder: Option<String>,
}

/// Actualiza uma pasta e volta para ela, com o resultado à vista.
///
/// Redirecciona em vez de renderizar: sem isso, recarregar a página voltaria a
/// submeter o pedido de actualização.
async fn mail_sync(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(mailbox_id): Path<Uuid>,
    Form(form): Form<SyncForm>,
) -> Response {
    let member = member_or_login!(state, headers);
    let folder = form.folder.unwrap_or_else(|| "inbox".to_owned());

    let body = serde_json::json!({ "folder": folder });
    let path = format!("/api/v1/mail/mailboxes/{mailbox_id}/sync");

    let outcome = match api::post(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        &path,
        &body,
    )
    .await
    {
        Ok(result) => {
            let indexed = result.get("indexed").and_then(Value::as_u64).unwrap_or(0);
            format!("{indexed} mensagem(ns) actualizada(s).")
        }
        Err(ApiFailure::Unauthorised) => return Redirect::to("/login").into_response(),
        // A falha volta com a caixa, não num ecrã de erro: a lista continua
        // utilizável, apenas desactualizada, e o membro precisa de saber isso.
        Err(failure) => failure.to_string(),
    };

    Redirect::to(&format!(
        "/mail/{mailbox_id}?folder={folder}&sync={}",
        urlencoding_minimal(&outcome)
    ))
    .into_response()
}

/// `GET /mail/{id}`: a caixa, na aplicação Correio (D004).
async fn mail_mailbox(Path(mailbox_id): Path<Uuid>) -> Response {
    Redirect::to(&format!("/mail?box={mailbox_id}")).into_response()
}

#[derive(Deserialize)]
struct FlagForm {
    field: String,
    value: String,
}

async fn mail_flags(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(message_id): Path<Uuid>,
    Form(form): Form<FlagForm>,
) -> Response {
    let member = member_or_login!(state, headers);

    let value = form.value == "true";
    let body = match form.field.as_str() {
        "starred" => serde_json::json!({ "starred": value }),
        "read" => serde_json::json!({ "read": value }),
        // Um campo desconhecido não é um pedido a interpretar generosamente.
        _ => return failure_response(&ApiFailure::Denied),
    };

    // Marcar como não lida tem de sobreviver à re-abertura imediata: o destino
    // leva `?unread=1`, para a mensagem voltar a mostrar-se **sem** ser marcada
    // como lida outra vez. Qualquer outra acção (marcar como lida, assinalar)
    // abre normalmente.
    let marcou_nao_lida = form.field == "read" && !value;
    let destino = if marcou_nao_lida {
        format!("/mail/message/{message_id}?unread=1")
    } else {
        format!("/mail/message/{message_id}")
    };

    let path = format!("/api/v1/mail/messages/{message_id}/flags");
    match api::post(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        &path,
        &body,
    )
    .await
    {
        Ok(_) | Err(ApiFailure::Denied) => Redirect::to(&destino).into_response(),
        Err(failure) => failure_response(&failure),
    }
}

/// O formulário do composer, tal como chega das duas rotas que o submetem.
#[derive(Deserialize)]
struct ComposeForm {
    #[serde(default)]
    draft_id: Option<String>,
    #[serde(default)]
    mailbox_id: String,
    #[serde(default)]
    to: String,
    #[serde(default)]
    cc: String,
    #[serde(default)]
    bcc: String,
    #[serde(default)]
    subject: String,
    #[serde(default)]
    body: String,
    #[serde(default)]
    html_body: String,
    #[serde(default)]
    confirmed: Option<String>,
}

/// Página retirada no apagamento da UI; espera o Claude Design.
///
/// A assistência de escrita devolvia o texto dentro do compositor. Sem
/// compositor, não há onde o pôr, e pedi-lo ao Core seria gastar para nada.
async fn assist() -> Response {
    interface_pending()
}

/// Envia. A única rota do Workspace que o faz.
async fn send_mail(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Form(form): Form<ComposeForm>,
) -> Response {
    let member = member_or_login!(state, headers);

    let split = |raw: &str| -> Vec<String> {
        raw.split([',', ';'])
            .map(str::trim)
            .filter(|address| !address.is_empty())
            .map(str::to_owned)
            .collect()
    };

    let html_body = Some(form.html_body.clone()).filter(|html| !html.is_empty());
    let draft_id = form.draft_id.clone().filter(|id| !id.is_empty());
    let body = serde_json::json!({
        "mailbox_id": form.mailbox_id,
        "to": split(&form.to),
        "cc": split(&form.cc),
        "bcc": split(&form.bcc),
        "subject": form.subject,
        "body": form.body,
        "html_body": html_body,
        "draft_id": draft_id,
        "confirmed": form.confirmed.is_some(),
    });

    match api::post(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        "/api/v1/mail/send",
        &body,
    )
    .await
    {
        Ok(_) => {
            // Enviada: o rascunho que a suportava deixa de fazer sentido. É um
            // best-effort — se a limpeza falhar, o rascunho fica na pasta, o que
            // é preferível a falhar um envio que já aconteceu.
            if let Some(draft_id) = form.draft_id.as_deref().filter(|id| !id.is_empty()) {
                let _ = api::delete(
                    &state,
                    &member.session.access_token,
                    &member.correlation_id,
                    &format!("/api/v1/mail/drafts/{draft_id}"),
                )
                .await;
            }
            Redirect::to("/mail").into_response()
        }
        Err(ApiFailure::Unauthorised) => Redirect::to("/login").into_response(),
        Err(failure) => failure_response(&failure),
    }
}

// ── Rascunhos (autosave, fecho seguro) ─────────────────────────────────────
//
// Estes endpoints são chamados pelo JS do compositor (mesmo origem) e devolvem
// JSON, não HTML: um autosave não navega. A autoridade é do Core — aqui só se
// reencaminha com o token do membro. Sem sessão, respondem 401 em JSON, para o
// `fetch` distinguir «não guardado» de «guardado».

/// Parte as strings de destinatários em arrays, como o envio faz, e devolve o
/// corpo pronto para o Core.
fn corpo_de_rascunho(recebido: &Value) -> Value {
    let split = |chave: &str| -> Vec<String> {
        recebido
            .get(chave)
            .and_then(Value::as_str)
            .unwrap_or("")
            .split([',', ';'])
            .map(str::trim)
            .filter(|address| !address.is_empty())
            .map(str::to_owned)
            .collect()
    };
    serde_json::json!({
        // Uma caixa vazia viaja como `null`, nunca como `""`: o Core desserializa
        // `mailbox_id` para `Option<Uuid>`, e `""` não é um UUID — dava um `422`
        // opaco que o compositor mostrava como «Erro ao guardar rascunho»,
        // levando o anexo consigo. `null` chega como ausência, e o Core devolve a
        // razão legível.
        "mailbox_id": recebido
            .get("mailbox_id")
            .and_then(Value::as_str)
            .filter(|caixa| !caixa.is_empty()),
        "to": split("to"),
        "cc": split("cc"),
        "bcc": split("bcc"),
        "subject": recebido.get("subject").and_then(Value::as_str).unwrap_or(""),
        "body": recebido.get("body").and_then(Value::as_str).unwrap_or(""),
        "html_body": recebido.get("html_body").and_then(Value::as_str),
        // Como o `mailbox_id`: `in_reply_to` também é `Option<Uuid>` no Core, e
        // uma mensagem nova traz `""`. Vazio viaja como `null`, nunca como `""`.
        "in_reply_to": recebido
            .get("in_reply_to")
            .and_then(Value::as_str)
            .filter(|id| !id.is_empty()),
    })
}

/// Traduz o resultado de uma chamada ao Core numa resposta JSON para o `fetch`.
fn resposta_de_rascunho(resultado: std::result::Result<Value, ApiFailure>) -> Response {
    match resultado {
        Ok(value) => (StatusCode::OK, Json(value)).into_response(),
        Err(ApiFailure::Unauthorised) => (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({ "error": "sessão expirada" })),
        )
            .into_response(),
        Err(failure) => (
            StatusCode::BAD_GATEWAY,
            Json(serde_json::json!({ "error": failure.to_string() })),
        )
            .into_response(),
    }
}

/// `POST /mail/drafts` — cria um rascunho (primeira alteração com conteúdo).
async fn draft_create(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Response {
    let Some(member) = current_member(&state, &headers) else {
        return (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({ "error": "sem sessão" })),
        )
            .into_response();
    };
    let resultado = api::post(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        "/api/v1/mail/drafts",
        &corpo_de_rascunho(&body),
    )
    .await;
    resposta_de_rascunho(resultado)
}

/// `PUT /mail/drafts/{draft_id}` — autosave de um rascunho existente.
async fn draft_update(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(draft_id): Path<String>,
    Json(body): Json<Value>,
) -> Response {
    let Some(member) = current_member(&state, &headers) else {
        return (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({ "error": "sem sessão" })),
        )
            .into_response();
    };
    let resultado = api::put(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        &format!("/api/v1/mail/drafts/{draft_id}"),
        &corpo_de_rascunho(&body),
    )
    .await;
    resposta_de_rascunho(resultado)
}

/// `DELETE /mail/drafts/{draft_id}` — descarta um rascunho.
async fn draft_discard(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(draft_id): Path<String>,
) -> Response {
    let Some(member) = current_member(&state, &headers) else {
        return (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({ "error": "sem sessão" })),
        )
            .into_response();
    };
    let resultado = api::delete(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        &format!("/api/v1/mail/drafts/{draft_id}"),
    )
    .await;
    resposta_de_rascunho(resultado)
}

/// `POST /mail/drafts/{draft_id}/attachments` — anexa um ficheiro ao rascunho.
async fn draft_attach(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(draft_id): Path<String>,
    multipart: Multipart,
) -> Response {
    let Some(member) = current_member(&state, &headers) else {
        return (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({ "error": "sem sessão" })),
        )
            .into_response();
    };
    let (ficheiro, _campos) = ler_carregamento(multipart).await;
    let Some((nome, tipo, dados)) = ficheiro else {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "ficheiro em falta" })),
        )
            .into_response();
    };
    let resultado = api::upload(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        &format!("/api/v1/mail/drafts/{draft_id}/attachments"),
        nome,
        tipo,
        dados,
    )
    .await;
    resposta_de_rascunho(resultado)
}

/// `DELETE /mail/drafts/{draft_id}/attachments/{attachment_id}` — retira um anexo.
async fn draft_attachment_remove(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path((draft_id, attachment_id)): Path<(String, String)>,
) -> Response {
    let Some(member) = current_member(&state, &headers) else {
        return (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({ "error": "sem sessão" })),
        )
            .into_response();
    };
    let resultado = api::delete(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        &format!("/api/v1/mail/drafts/{draft_id}/attachments/{attachment_id}"),
    )
    .await;
    resposta_de_rascunho(resultado)
}

/// Sem ecrã entregue pelo Design: a janela `app_pending` na casca (D001).
async fn mail_settings(State(state): State<WorkspaceState>, headers: HeaderMap) -> Response {
    app_page(&state, &headers, Screen::Mail).await
}

// ── Mensagens ────────────────────────────────────────────────────────────

/// Sem ecrã entregue pelo Design: a janela `app_pending` na casca (D001).
async fn messaging(State(state): State<WorkspaceState>, headers: HeaderMap) -> Response {
    app_page(&state, &headers, Screen::Messaging).await
}

/// Sem ecrã entregue pelo Design: a janela `app_pending` na casca (D001).
async fn messaging_conversation(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
) -> Response {
    app_page(&state, &headers, Screen::Messaging).await
}

#[derive(Deserialize)]
struct StartForm {
    #[serde(default)]
    with: Option<Uuid>,
    #[serde(default)]
    name: Option<String>,
    /// Identificadores separados por vírgula, como o formulário os envia.
    #[serde(default)]
    members: String,
}

/// Começa uma conversa — directa ou de grupo — e abre-a.
async fn messaging_start(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Form(form): Form<StartForm>,
) -> Response {
    let member = member_or_login!(state, headers);

    let membros: Vec<Uuid> = form
        .members
        .split(',')
        .filter_map(|parte| Uuid::parse_str(parte.trim()).ok())
        .collect();

    let corpo = serde_json::json!({
        "with": form.with,
        "name": form.name.as_deref().map(str::trim).filter(|n| !n.is_empty()),
        "members": membros,
    });

    match api::post(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        "/api/v1/messaging/conversations",
        &corpo,
    )
    .await
    {
        Ok(valor) => {
            let id = valor.get("id").and_then(Value::as_str).unwrap_or_default();
            Redirect::to(&format!("/messages/{id}")).into_response()
        }
        Err(failure) => failure_response(&failure),
    }
}

#[derive(Deserialize)]
struct ProcuraDePessoas {
    #[serde(default)]
    q: String,
}

/// Procura pessoas da instituição para começar uma conversa.
///
/// # Porque filtra no servidor
///
/// Porque uma instituição não cabe num `select`, e carregá-la inteira para
/// filtrar no browser seria mandar a lista de toda a gente para cada pessoa que
/// abre as Mensagens. O universo continua a ser o que o Core autoriza — este
/// caminho não alarga nada.
async fn messaging_people(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Query(procura): Query<ProcuraDePessoas>,
) -> Response {
    let member = member_or_login!(state, headers);

    let termo = procura.q.trim().to_lowercase();
    if termo.chars().count() < 2 {
        // Duas letras é o mínimo. Com uma, a resposta seria metade da
        // instituição, e a lista deixaria de ajudar a escolher.
        return axum::Json(serde_json::json!({ "people": [] })).into_response();
    }

    let pagina = optional(&state, &member, "/api/v1/people?page_size=200").await;
    let eu = eu_id(&state, &member).await;

    let pessoas: Vec<Value> = pagina
        .get("items")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter(|p| {
            // Nunca a própria: uma conversa consigo mesmo não existe, e o Core
            // recusa-a na mesma.
            p.get("id").and_then(Value::as_str) != Some(&eu.to_string())
                && p.get("status").and_then(Value::as_str) != Some("deactivated")
        })
        .filter(|p| {
            let campo = |nome: &str| {
                p.get(nome)
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_lowercase()
            };
            campo("full_name").contains(&termo)
                || campo("display_name").contains(&termo)
                || campo("email").contains(&termo)
        })
        .take(20)
        .map(|p| {
            serde_json::json!({
                "id": p.get("id"),
                "name": p
                    .get("display_name")
                    .and_then(Value::as_str)
                    .filter(|n| !n.is_empty())
                    .or_else(|| p.get("full_name").and_then(Value::as_str))
                    .unwrap_or_default(),
                "email": p.get("email"),
            })
        })
        .collect();

    axum::Json(serde_json::json!({ "people": pessoas })).into_response()
}

/// Quem se pode pôr num «Para».
///
/// # Porque não reutiliza a rota das Mensagens
///
/// Porque as duas respondem a perguntas diferentes. Mensagens procura **com
/// quem conversar**, e por isso exclui a própria pessoa: uma conversa consigo
/// mesmo não existe. Escrever a si próprio existe, e é normal — um lembrete,
/// um teste de configuração, uma cópia de arquivo.
///
/// Partilhar a rota faria uma das duas mentir. Aqui procura-se por **nome ou
/// endereço institucional**, que desde o [ADR-0106] é a identidade humana;
/// nome de utilizador não existe.
///
/// [ADR-0106]: https://github.com/Ocinye/ocinye-os/blob/main/docs/adrs/0106-email-as-the-single-credential.md
async fn mail_people(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Query(procura): Query<ProcuraDePessoas>,
) -> Response {
    let member = member_or_login!(state, headers);

    let termo = procura.q.trim().to_lowercase();
    if termo.chars().count() < 2 {
        // Com uma letra a resposta seria metade da instituição, e uma lista
        // dessas não ajuda a escolher.
        return axum::Json(serde_json::json!({ "people": [] })).into_response();
    }

    let pagina = optional(&state, &member, "/api/v1/people?page_size=200").await;

    let pessoas: Vec<Value> = pagina
        .get("items")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter(|p| p.get("status").and_then(Value::as_str) != Some("deactivated"))
        .filter(|p| {
            let campo = |nome: &str| {
                p.get(nome)
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_lowercase()
            };
            campo("full_name").contains(&termo)
                || campo("display_name").contains(&termo)
                || campo("email").contains(&termo)
        })
        // Sem endereço não há para onde escrever, e oferecê-lo seria oferecer
        // um destinatário que não recebe.
        .filter(|p| {
            p.get("email")
                .and_then(Value::as_str)
                .is_some_and(|e| !e.is_empty())
        })
        .take(20)
        .map(|p| {
            serde_json::json!({
                "name": p
                    .get("display_name")
                    .and_then(Value::as_str)
                    .filter(|n| !n.is_empty())
                    .or_else(|| p.get("full_name").and_then(Value::as_str))
                    .unwrap_or_default(),
                "email": p.get("email"),
            })
        })
        .collect();

    axum::Json(serde_json::json!({ "people": pessoas })).into_response()
}

#[derive(Deserialize)]
struct AssistForm {
    action: String,
    draft: String,
}

/// Pede ao Ocinye para trabalhar um rascunho.
///
/// # Porque devolve JSON e não uma página
///
/// Porque o que volta é uma **proposta**, e o rascunho da pessoa tem de ficar
/// exactamente onde estava. Recarregar a página para mostrar uma sugestão
/// perderia o que ela estava a escrever.
async fn messaging_assist(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Form(form): Form<AssistForm>,
) -> Response {
    let member = member_or_login!(state, headers);

    match api::post(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        "/api/v1/messaging/assist",
        &serde_json::json!({ "action": form.action, "draft": form.draft }),
    )
    .await
    {
        Ok(valor) => axum::Json(valor).into_response(),
        // Uma assistência que falha não perde o que estava escrito, e diz
        // porquê em vez de ficar calada.
        Err(falha) => axum::Json(serde_json::json!({
            "text": null,
            "reason": falha.to_string(),
        }))
        .into_response(),
    }
}

/// Quem está a escrever nesta conversa.
async fn messaging_typing(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(conversation): Path<String>,
) -> Response {
    let member = member_or_login!(state, headers);

    let resposta = optional(
        &state,
        &member,
        &format!("/api/v1/messaging/typing?conversation={conversation}"),
    )
    .await;

    axum::Json(resposta).into_response()
}

#[derive(Deserialize)]
struct SendForm {
    #[serde(default)]
    body: String,
    #[serde(default)]
    reply_to: Option<Uuid>,
    /// Identificadores separados por vírgula.
    #[serde(default)]
    mentions: String,
    /// A chave que torna o envio idempotente.
    ///
    /// Vem do formulário porque é o cliente que sabe que **este** é o mesmo
    /// envio que já tentou. Um duplo-clique traz a mesma, e o Core devolve a
    /// mensagem que a primeira escreveu.
    #[serde(default)]
    idempotency_key: String,
}

/// Envia uma mensagem.
async fn messaging_send(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(conversation): Path<String>,
    Form(form): Form<SendForm>,
) -> Response {
    let member = member_or_login!(state, headers);

    let mencoes: Vec<Uuid> = form
        .mentions
        .split(',')
        .filter_map(|parte| Uuid::parse_str(parte.trim()).ok())
        .collect();

    // O autor não vai daqui. Vai do principal, no Core.
    let corpo = serde_json::json!({
        "body": form.body,
        "reply_to": form.reply_to,
        "mentions": mencoes,
        "idempotency_key": (!form.idempotency_key.trim().is_empty())
            .then(|| form.idempotency_key.trim()),
    });

    match api::post(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        &format!("/api/v1/messaging/conversations/{conversation}/messages"),
        &corpo,
    )
    .await
    {
        Ok(_) => Redirect::to(&format!("/messages/{conversation}")).into_response(),
        Err(failure) => failure_response(&failure),
    }
}

#[derive(Deserialize)]
struct ReactForm {
    message: Uuid,
    emoji: String,
}

/// Põe ou tira uma reacção.
async fn messaging_react(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(conversation): Path<String>,
    Form(form): Form<ReactForm>,
) -> Response {
    let member = member_or_login!(state, headers);
    let caminho = format!(
        "/api/v1/messaging/conversations/{conversation}/messages/{}/reactions",
        form.message
    );

    match api::post(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        &caminho,
        &serde_json::json!({ "emoji": form.emoji }),
    )
    .await
    {
        Ok(_) => Redirect::to(&format!("/messages/{conversation}")).into_response(),
        Err(failure) => failure_response(&failure),
    }
}

#[derive(Deserialize)]
struct ReadForm {
    until: String,
}

/// Marca a conversa como lida até um instante.
async fn messaging_read(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(conversation): Path<String>,
    Form(form): Form<ReadForm>,
) -> Response {
    let member = member_or_login!(state, headers);

    match api::post(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        &format!("/api/v1/messaging/conversations/{conversation}/read"),
        &serde_json::json!({ "until": form.until }),
    )
    .await
    {
        Ok(_) => Redirect::to(&format!("/messages/{conversation}")).into_response(),
        Err(failure) => failure_response(&failure),
    }
}

#[derive(Deserialize)]
struct MemberForm {
    who: Uuid,
}

/// Acrescenta alguém ao grupo.
async fn messaging_add_member(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(conversation): Path<String>,
    Form(form): Form<MemberForm>,
) -> Response {
    let member = member_or_login!(state, headers);

    match api::post(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        &format!("/api/v1/messaging/conversations/{conversation}/members"),
        &serde_json::json!({ "who": form.who }),
    )
    .await
    {
        Ok(_) => Redirect::to(&format!("/messages/{conversation}")).into_response(),
        Err(failure) => failure_response(&failure),
    }
}

/// Retira alguém do grupo.
async fn messaging_remove(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(conversation): Path<String>,
    Form(form): Form<MemberForm>,
) -> Response {
    let member = member_or_login!(state, headers);

    match api::delete(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        &format!(
            "/api/v1/messaging/conversations/{conversation}/members/{}",
            form.who
        ),
    )
    .await
    {
        Ok(_) => Redirect::to(&format!("/messages/{conversation}")).into_response(),
        Err(failure) => failure_response(&failure),
    }
}

/// Quem está a agir, tal como o Core o identifica.
async fn eu_id(state: &WorkspaceState, member: &Member) -> Uuid {
    quem_sou(&optional(state, member, "/api/v1/me").await)
}

/// Lê o identificador da pessoa da resposta de `/api/v1/me`.
///
/// # O campo chama-se `person_id`
///
/// E isto esteve a ler `id`. O `unwrap_or_default()` devolvia o UUID nulo, que
/// não é ninguém — e como ninguém é o autor de nada, **nenhuma mensagem era
/// própria**. Sem erro, sem aviso: a conversa inteira aparecia alinhada como se
/// fosse de outra pessoa.
///
/// Uma chave errada num JSON não dá erro. É por isso que existe o guarda em
/// `quem_sou_le_o_campo_que_o_core_escreve`, que compara com a forma real.
fn quem_sou(me: &Value) -> Uuid {
    me.get("person_id")
        .and_then(Value::as_str)
        .and_then(|s| Uuid::parse_str(s).ok())
        .unwrap_or_default()
}

/// Sai do grupo.
async fn messaging_leave(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(conversation): Path<String>,
) -> Response {
    let member = member_or_login!(state, headers);

    match api::delete(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        &format!(
            "/api/v1/messaging/conversations/{conversation}/members/{}",
            eu_id(&state, &member).await
        ),
    )
    .await
    {
        // Depois de sair, a conversa deixa de existir para quem saiu.
        Ok(_) => Redirect::to("/messages").into_response(),
        Err(failure) => failure_response(&failure),
    }
}

#[derive(Deserialize)]
struct MailSettingsForm {
    #[serde(default)]
    signature: String,
    // Uma checkbox: presente quando marcada, ausente quando não. `Some` = usar
    // a assinatura oficial.
    #[serde(default)]
    official_signature: Option<String>,
    #[serde(default)]
    remote_content_policy: String,
}

async fn save_mail_settings(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Form(form): Form<MailSettingsForm>,
) -> Response {
    let member = member_or_login!(state, headers);

    let body = serde_json::json!({
        "signature": form.signature,
        "official_signature": form.official_signature.is_some(),
        "remote_content_policy": form.remote_content_policy,
    });

    match api::post(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        "/api/v1/mail/preferences",
        &body,
    )
    .await
    {
        Ok(_) => Redirect::to("/mail/settings").into_response(),
        Err(failure) => failure_response(&failure),
    }
}

/// O que o formulário de ligação envia.
#[derive(serde::Deserialize)]
struct LigacaoDeCaixa {
    /// A senha **da caixa**, no serviço de correio.
    ///
    /// Não é a credencial do Ocinye: essa é o endereço institucional
    /// (ADR-0106), com a palavra-passe do Ocinye OS. Esta é outra, e nenhuma
    /// serve para obter a outra.
    ///
    /// Havia aqui um `username` ao lado, com o argumento de que nem todos os
    /// serviços usam o endereço como conta. É verdade em geral e não é verdade
    /// aqui — e o custo de o manter era deixar o browser escolher a conta com
    /// que o Ocinye se autentica. O Core resolve-a a partir da caixa que já
    /// autorizou (ADR-0409). Se um dia houver um serviço que peça outra coisa,
    /// isso é uma decisão de instalação e não um campo no ecrã de quem liga.
    password: String,
}

/// Liga uma caixa com a credencial de quem a está a ligar.
///
/// # Porque a senha não volta a passar por aqui
///
/// Atravessa este processo uma vez, a caminho do Core, e nunca regressa: o
/// formulário abre sempre vazio, e não há endpoint que a devolva. O que a
/// Experience sabe de uma caixa ligada é que está ligada.
async fn mail_connect(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(mailbox_id): Path<String>,
    Form(form): Form<LigacaoDeCaixa>,
) -> Response {
    let member = member_or_login!(state, headers);

    // Só a senha. O endereço não viaja: o Core resolve-o a partir da caixa que
    // já autorizou para esta pessoa, e um endereço vindo do formulário deixaria
    // o browser escolher a conta com que a sessão de correio abre.
    let body = serde_json::json!({
        "password": form.password,
    });

    match api::post(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        &format!("/api/v1/mail/mailboxes/{mailbox_id}/connect"),
        &body,
    )
    .await
    {
        Ok(_) => Redirect::to("/mail/settings").into_response(),
        Err(failure) => failure_response(&failure),
    }
}

/// Liga a caixa pessoal do próprio membro, criando-a se ainda não existe.
///
/// É o caminho do estado vazio: sem caixa nenhuma, o Core cria a caixa ao
/// endereço institucional do membro e liga-a. Só a senha viaja — o endereço vem
/// da identidade de quem liga, no Core.
async fn mail_connect_own(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Form(form): Form<LigacaoDeCaixa>,
) -> Response {
    let member = member_or_login!(state, headers);

    let body = serde_json::json!({
        "password": form.password,
    });

    match api::post(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        "/api/v1/mail/mailbox/connect",
        &body,
    )
    .await
    {
        Ok(_) => Redirect::to("/mail/settings").into_response(),
        Err(failure) => failure_response(&failure),
    }
}

/// Desliga uma caixa e esquece a credencial.
async fn mail_disconnect(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(mailbox_id): Path<String>,
) -> Response {
    let member = member_or_login!(state, headers);

    match api::post(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        &format!("/api/v1/mail/mailboxes/{mailbox_id}/disconnect"),
        &serde_json::json!({}),
    )
    .await
    {
        Ok(_) => Redirect::to("/mail/settings").into_response(),
        Err(failure) => failure_response(&failure),
    }
}

// ── Listas ───────────────────────────────────────────────────────────────

/// Sem ecrã entregue pelo Design: a janela `app_pending` na casca (D001).
async fn units(State(state): State<WorkspaceState>, headers: HeaderMap) -> Response {
    app_page(&state, &headers, Screen::Units).await
}

/// Sem ecrã entregue pelo Design: a janela `app_pending` na casca (D001).
async fn agents(State(state): State<WorkspaceState>, headers: HeaderMap) -> Response {
    app_page(&state, &headers, Screen::Agents).await
}
/// Sem ecrã entregue pelo Design: a janela `app_pending` na casca (D001).
async fn admin(State(state): State<WorkspaceState>, headers: HeaderMap) -> Response {
    app_page(&state, &headers, Screen::Admin).await
}
/// Sem ecrã entregue pelo Design: a janela `app_pending` na casca (D001).
async fn audit(State(state): State<WorkspaceState>, headers: HeaderMap) -> Response {
    app_page(&state, &headers, Screen::Audit).await
}

// ── Administração de membros ─────────────────────────────────────────────

/// Sem ecrã entregue pelo Design: a janela `app_pending` na casca (D001).
async fn new_member(State(state): State<WorkspaceState>, headers: HeaderMap) -> Response {
    app_page(&state, &headers, Screen::Admin).await
}

/// Acção retirada no apagamento da UI; espera o Claude Design.
///
/// Produz um segredo que se mostra uma única vez; sem ecrã, perdia-se.
async fn create_member() -> Response {
    interface_pending()
}

/// Sem ecrã entregue pelo Design: a janela `app_pending` na casca (D001).
async fn member_detail(State(state): State<WorkspaceState>, headers: HeaderMap) -> Response {
    app_page(&state, &headers, Screen::Admin).await
}

/// Corpo do formulário de atribuição de unidade a um membro.
#[derive(serde::Deserialize)]
struct AtribuirUnidadeForm {
    unit_id: String,
    role: String,
}

/// Corpo do formulário de alteração de papel numa unidade.
#[derive(serde::Deserialize)]
struct PapelUnidadeForm {
    role: String,
}

/// `POST /admin/members/{person_id}/units` — o administrador atribui uma
/// unidade a este membro. A operação bate no Core, que reautoriza o **actor**
/// (não o membro aqui aberto) sobre a gestão de membros dessa unidade.
async fn member_unit_assign(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(person_id): Path<String>,
    axum::extract::Form(form): axum::extract::Form<AtribuirUnidadeForm>,
) -> Response {
    let member = member_or_login!(state, headers);
    let destino = format!("/admin/members/{person_id}");
    let body = serde_json::json!({ "person_id": person_id, "role": form.role });
    let path = format!("/api/v1/units/{}/members", form.unit_id);
    match api::post(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        &path,
        &body,
    )
    .await
    {
        Ok(_) => Redirect::to(&destino).into_response(),
        Err(failure) => failure_response(&failure),
    }
}

/// `POST /admin/members/{person_id}/units/{unit_id}/role` — altera o papel do
/// membro na unidade. É um `upsert`: o Core aceita o mesmo membro com o papel
/// novo.
async fn member_unit_role(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path((person_id, unit_id)): Path<(String, String)>,
    axum::extract::Form(form): axum::extract::Form<PapelUnidadeForm>,
) -> Response {
    let member = member_or_login!(state, headers);
    let destino = format!("/admin/members/{person_id}");
    let body = serde_json::json!({ "person_id": person_id, "role": form.role });
    let path = format!("/api/v1/units/{unit_id}/members");
    match api::post(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        &path,
        &body,
    )
    .await
    {
        Ok(_) => Redirect::to(&destino).into_response(),
        Err(failure) => failure_response(&failure),
    }
}

/// `POST /admin/members/{person_id}/units/{unit_id}/remove` — remove a pertença
/// do membro à unidade. A linha fica: que alguém pertenceu é memória
/// institucional.
async fn member_unit_remove(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path((person_id, unit_id)): Path<(String, String)>,
) -> Response {
    let member = member_or_login!(state, headers);
    let destino = format!("/admin/members/{person_id}");
    let path = format!("/api/v1/units/{unit_id}/members/{person_id}");
    match api::post(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        &path,
        &serde_json::json!({}),
    )
    .await
    {
        Ok(_) => Redirect::to(&destino).into_response(),
        Err(failure) => failure_response(&failure),
    }
}

/// Corpo do formulário de atribuição de research workspace a um membro.
#[derive(serde::Deserialize)]
struct AtribuirWorkspaceForm {
    workspace_id: String,
    role: String,
}

/// `POST /admin/members/{person_id}/workspaces` — atribui um research workspace
/// ao membro. Reautorizado no Core sobre o **actor**; administrar a pertença
/// não concede leitura do conteúdo do workspace.
async fn member_workspace_assign(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(person_id): Path<String>,
    axum::extract::Form(form): axum::extract::Form<AtribuirWorkspaceForm>,
) -> Response {
    let member = member_or_login!(state, headers);
    let destino = format!("/admin/members/{person_id}");
    let body = serde_json::json!({ "person_id": person_id, "role": form.role });
    let path = format!("/api/v1/workspaces/{}/members", form.workspace_id);
    match api::post(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        &path,
        &body,
    )
    .await
    {
        Ok(_) => Redirect::to(&destino).into_response(),
        Err(failure) => failure_response(&failure),
    }
}

/// `POST /admin/members/{person_id}/workspaces/{workspace_id}/role` — altera o
/// papel do membro no workspace (upsert).
async fn member_workspace_role(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path((person_id, workspace_id)): Path<(String, String)>,
    axum::extract::Form(form): axum::extract::Form<PapelUnidadeForm>,
) -> Response {
    let member = member_or_login!(state, headers);
    let destino = format!("/admin/members/{person_id}");
    let body = serde_json::json!({ "person_id": person_id, "role": form.role });
    let path = format!("/api/v1/workspaces/{workspace_id}/members");
    match api::post(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        &path,
        &body,
    )
    .await
    {
        Ok(_) => Redirect::to(&destino).into_response(),
        Err(failure) => failure_response(&failure),
    }
}

/// `POST /admin/members/{person_id}/workspaces/{workspace_id}/remove` — remove a
/// pertença do membro ao workspace.
async fn member_workspace_remove(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path((person_id, workspace_id)): Path<(String, String)>,
) -> Response {
    let member = member_or_login!(state, headers);
    let destino = format!("/admin/members/{person_id}");
    let path = format!("/api/v1/workspaces/{workspace_id}/members/{person_id}");
    match api::post(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        &path,
        &serde_json::json!({}),
    )
    .await
    {
        Ok(_) => Redirect::to(&destino).into_response(),
        Err(failure) => failure_response(&failure),
    }
}

/// Corpo do formulário de alteração de estado da conta.
#[derive(serde::Deserialize)]
struct EstadoContaForm {
    status: String,
    reason: String,
}

/// `POST /admin/members/{person_id}/status` — suspende, desactiva ou reactiva.
///
/// A autoridade é reautorizada no Core, que também recusa auto-bloqueio e
/// deixar a instituição sem administrador capaz de entrar. A recusa volta ao
/// detalhe, com a razão à vista.
async fn member_set_status(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(person_id): Path<String>,
    axum::extract::Form(form): axum::extract::Form<EstadoContaForm>,
) -> Response {
    let member = member_or_login!(state, headers);
    let destino = format!("/admin/members/{person_id}");
    let path = format!("/api/v1/administration/members/{person_id}/status");
    let body = serde_json::json!({ "status": form.status, "reason": form.reason });
    match api::post(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        &path,
        &body,
    )
    .await
    {
        Ok(_) => Redirect::to(&destino).into_response(),
        Err(failure) => failure_response(&failure),
    }
}

/// Corpo do formulário de posição institucional.
#[derive(serde::Deserialize)]
struct PosicaoForm {
    /// Código da posição, ou vazio para limpar.
    #[serde(default)]
    position: String,
}

/// `POST /admin/members/{person_id}/position` — define, muda ou limpa a posição
/// institucional.
///
/// A posição é registo, não acesso (ADR-0100): esta operação não toca em papéis
/// nem permissões. A autoridade é reautorizada no Core; a recusa volta ao
/// detalhe, com a razão à vista.
async fn member_set_position(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(person_id): Path<String>,
    axum::extract::Form(form): axum::extract::Form<PosicaoForm>,
) -> Response {
    let member = member_or_login!(state, headers);
    let destino = format!("/admin/members/{person_id}");
    let path = format!("/api/v1/administration/members/{person_id}/position");
    let body = serde_json::json!({ "position": form.position });
    match api::post(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        &path,
        &body,
    )
    .await
    {
        Ok(_) => Redirect::to(&destino).into_response(),
        Err(failure) => failure_response(&failure),
    }
}

/// `POST /admin/members/{person_id}/delete` — apaga um convite por aceitar.
///
/// Um formulário HTML não fala `DELETE`; o Core, sim — e o verbo certo viaja
/// daqui para lá. Só um convite que ninguém aceitou e que nunca foi usado se
/// apaga; para uma conta já usada o Core recusa, e a razão («desactive, que
/// preserva a autoria») volta ao detalhe. No sucesso, o membro deixou de
/// existir: reencaminha-se para a lista, e não para um detalhe que seria 404.
async fn member_delete(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(person_id): Path<String>,
) -> Response {
    let member = member_or_login!(state, headers);
    let path = format!("/api/v1/administration/members/{person_id}");
    match api::delete(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        &path,
    )
    .await
    {
        Ok(_) => Redirect::to("/admin").into_response(),
        Err(failure) => failure_response(&failure),
    }
}

/// Acção retirada no apagamento da UI; espera o Claude Design.
///
/// Produz um segredo que se mostra uma única vez; sem ecrã, perdia-se.
async fn member_reset_password() -> Response {
    interface_pending()
}

/// Corpo do formulário de concessão de papel técnico.
#[derive(serde::Deserialize)]
struct PapelTecnicoForm {
    role: String,
    reason: String,
}

/// `POST /admin/members/{person_id}/roles` — concede um papel técnico.
async fn member_role_grant(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(person_id): Path<String>,
    axum::extract::Form(form): axum::extract::Form<PapelTecnicoForm>,
) -> Response {
    let member = member_or_login!(state, headers);
    let destino = format!("/admin/members/{person_id}");
    let path = format!("/api/v1/people/{person_id}/roles");
    let body = serde_json::json!({ "role": form.role, "reason": form.reason });
    match api::post(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        &path,
        &body,
    )
    .await
    {
        Ok(_) => Redirect::to(&destino).into_response(),
        Err(failure) => failure_response(&failure),
    }
}

/// `POST /admin/members/{person_id}/roles/{role}/revoke` — revoga um papel.
///
/// Um formulário HTML não fala `DELETE`; o Core, sim. O verbo certo viaja daqui
/// para o Core, com o papel no corpo — retirar o último Platform Admin é
/// recusado lá, e a razão volta ao detalhe.
async fn member_role_revoke(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path((person_id, role)): Path<(String, String)>,
) -> Response {
    let member = member_or_login!(state, headers);
    let destino = format!("/admin/members/{person_id}");
    let path = format!("/api/v1/people/{person_id}/roles");
    let body = serde_json::json!({ "role": role });
    match api::delete_with_body(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        &path,
        &body,
    )
    .await
    {
        Ok(_) => Redirect::to(&destino).into_response(),
        Err(failure) => failure_response(&failure),
    }
}

/// Corpo do formulário de concessão de grant explícito.
#[derive(serde::Deserialize)]
struct GrantForm {
    permission: String,
    scope: String,
    reason: String,
}

/// `POST /admin/members/{person_id}/grants` — concede um grant explícito.
async fn member_grant_create(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(person_id): Path<String>,
    axum::extract::Form(form): axum::extract::Form<GrantForm>,
) -> Response {
    let member = member_or_login!(state, headers);
    let destino = format!("/admin/members/{person_id}");
    let body = serde_json::json!({
        "subject_id": person_id,
        "permission": form.permission,
        "scope": form.scope,
        "reason": form.reason,
    });
    match api::post(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        "/api/v1/administration/grants",
        &body,
    )
    .await
    {
        Ok(_) => Redirect::to(&destino).into_response(),
        Err(failure) => failure_response(&failure),
    }
}

/// Corpo do formulário de revogação de grant.
#[derive(serde::Deserialize)]
struct RevogarGrantForm {
    reason: String,
}

/// `POST /admin/members/{person_id}/grants/{grant_id}/revoke` — revoga um grant.
async fn member_grant_revoke(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path((person_id, grant_id)): Path<(String, String)>,
    axum::extract::Form(form): axum::extract::Form<RevogarGrantForm>,
) -> Response {
    let member = member_or_login!(state, headers);
    let destino = format!("/admin/members/{person_id}");
    let path = format!("/api/v1/administration/grants/{grant_id}");
    let body = serde_json::json!({ "reason": form.reason });
    match api::delete_with_body(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        &path,
        &body,
    )
    .await
    {
        Ok(_) => Redirect::to(&destino).into_response(),
        Err(failure) => failure_response(&failure),
    }
}

/// `POST /admin/members/{person_id}/sessions/{session_id}/revoke` — revoga **uma**
/// sessão de um membro. O Core reautoriza o actor e valida que a sessão pertence
/// ao membro (anti-IDOR); a recusa volta ao detalhe.
async fn member_session_revoke(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path((person_id, session_id)): Path<(String, String)>,
) -> Response {
    let member = member_or_login!(state, headers);
    let destino = format!("/admin/members/{person_id}");
    let path = format!("/api/v1/administration/members/{person_id}/sessions/{session_id}/revoke");
    match api::post(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        &path,
        &serde_json::json!({}),
    )
    .await
    {
        Ok(_) => Redirect::to(&destino).into_response(),
        Err(failure) => failure_response(&failure),
    }
}

/// Acção retirada no apagamento da UI; espera o Claude Design.
///
/// Produz um segredo que se mostra uma única vez; sem ecrã, perdia-se.
async fn provision_member() -> Response {
    interface_pending()
}

/// Sem ecrã entregue pelo Design: a janela `app_pending` na casca (D001).
async fn bibliography_tools(State(state): State<WorkspaceState>, headers: HeaderMap) -> Response {
    app_page(&state, &headers, Screen::Bibliography).await
}

/// Acção retirada no apagamento da UI; espera o Claude Design.
///
/// A revisão devolvia o seu resultado num ecrã.
async fn review_bibliography() -> Response {
    interface_pending()
}

// ── Investigação ─────────────────────────────────────────────────────────

/// Sem ecrã entregue pelo Design: a janela `app_pending` na casca (D001).
async fn unit_detail(State(state): State<WorkspaceState>, headers: HeaderMap) -> Response {
    app_page(&state, &headers, Screen::Units).await
}

// ── Ciência ──────────────────────────────────────────────────────────────

/// Sem ecrã entregue pelo Design: a janela `app_pending` na casca (D001).
async fn scientific_chain(State(state): State<WorkspaceState>, headers: HeaderMap) -> Response {
    app_page(&state, &headers, Screen::Projects).await
}

/// Sem ecrã entregue pelo Design: a janela `app_pending` na casca (D001).
async fn result_detail(State(state): State<WorkspaceState>, headers: HeaderMap) -> Response {
    app_page(&state, &headers, Screen::Projects).await
}

// ── Construir a cadeia ───────────────────────────────────────────────────

// ── Hipótese ─────────────────────────────────────────────────────────────

/// Sem ecrã entregue pelo Design: a janela `app_pending` na casca (D001).
async fn new_hypothesis(State(state): State<WorkspaceState>, headers: HeaderMap) -> Response {
    app_page(&state, &headers, Screen::Projects).await
}

#[derive(Deserialize)]
struct NovaHipoteseForm {
    #[serde(default)]
    statement: String,
    #[serde(default)]
    rationale: String,
    #[serde(default)]
    classification: String,
}

async fn create_hypothesis(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(workspace_id): Path<Uuid>,
    Form(form): Form<NovaHipoteseForm>,
) -> Response {
    let member = member_or_login!(state, headers);
    let corpo = serde_json::json!({
        "statement": form.statement,
        "rationale": blank_to_none(form.rationale),
        "classification": blank_to_none(form.classification),
    });

    match api::post(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        &format!("/api/v1/workspaces/{workspace_id}/hypotheses"),
        &corpo,
    )
    .await
    {
        Ok(_) => Redirect::to(&format!("/workspaces/{workspace_id}/science")).into_response(),
        Err(failure) => failure_response(&failure),
    }
}

// ── Metodologia e versões ────────────────────────────────────────────────

/// Sem ecrã entregue pelo Design: a janela `app_pending` na casca (D001).
async fn new_methodology(State(state): State<WorkspaceState>, headers: HeaderMap) -> Response {
    app_page(&state, &headers, Screen::Projects).await
}

#[derive(Deserialize)]
struct NovaMetodologiaForm {
    #[serde(default)]
    title: String,
    #[serde(default)]
    purpose: String,
    #[serde(default)]
    classification: String,
}

async fn create_methodology(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(workspace_id): Path<Uuid>,
    Form(form): Form<NovaMetodologiaForm>,
) -> Response {
    let member = member_or_login!(state, headers);
    let corpo = serde_json::json!({
        "title": form.title,
        "purpose": blank_to_none(form.purpose),
        "classification": blank_to_none(form.classification),
    });

    match api::post(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        &format!("/api/v1/workspaces/{workspace_id}/methodologies"),
        &corpo,
    )
    .await
    {
        Ok(criada) => {
            // Para a metodologia, e não de volta à lista: o passo seguinte é
            // publicar uma versão, e é lá que ele está.
            match criada.get("id").and_then(Value::as_str) {
                Some(id) => Redirect::to(&format!("/methodologies/{id}")).into_response(),
                None => {
                    Redirect::to(&format!("/workspaces/{workspace_id}/science")).into_response()
                }
            }
        }
        Err(failure) => failure_response(&failure),
    }
}

/// Sem ecrã entregue pelo Design: a janela `app_pending` na casca (D001).
async fn methodology_detail(State(state): State<WorkspaceState>, headers: HeaderMap) -> Response {
    app_page(&state, &headers, Screen::Projects).await
}

/// Sem ecrã entregue pelo Design: a janela `app_pending` na casca (D001).
async fn new_version(State(state): State<WorkspaceState>, headers: HeaderMap) -> Response {
    app_page(&state, &headers, Screen::Projects).await
}

#[derive(Deserialize)]
struct NovaVersaoForm {
    #[serde(default)]
    label: String,
    #[serde(default)]
    summary: String,
}

async fn publish_version(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(methodology_id): Path<Uuid>,
    Form(form): Form<NovaVersaoForm>,
) -> Response {
    let member = member_or_login!(state, headers);
    let corpo = serde_json::json!({"label": form.label, "summary": form.summary});

    match api::post(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        &format!("/api/v1/methodologies/{methodology_id}/versions"),
        &corpo,
    )
    .await
    {
        Ok(_) => Redirect::to(&format!("/methodologies/{methodology_id}")).into_response(),
        Err(failure) => failure_response(&failure),
    }
}

// ── Estudo e execuções ───────────────────────────────────────────────────

/// Sem ecrã entregue pelo Design: a janela `app_pending` na casca (D001).
async fn new_study(State(state): State<WorkspaceState>, headers: HeaderMap) -> Response {
    app_page(&state, &headers, Screen::Projects).await
}

#[derive(Deserialize)]
struct NovoEstudoForm {
    #[serde(default)]
    title: String,
    #[serde(default)]
    kind: String,
    #[serde(default)]
    objective: String,
    #[serde(default)]
    hypothesis_id: String,
    #[serde(default)]
    methodology_version_id: String,
    #[serde(default)]
    classification: String,
}

async fn create_study(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(workspace_id): Path<Uuid>,
    Form(form): Form<NovoEstudoForm>,
) -> Response {
    let member = member_or_login!(state, headers);
    let corpo = serde_json::json!({
        "title": form.title,
        "kind": form.kind,
        "objective": blank_to_none(form.objective),
        "hypothesis_id": blank_to_none(form.hypothesis_id),
        "methodology_version_id": blank_to_none(form.methodology_version_id),
        "classification": blank_to_none(form.classification),
    });

    match api::post(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        &format!("/api/v1/workspaces/{workspace_id}/studies"),
        &corpo,
    )
    .await
    {
        Ok(criado) => match criado.get("id").and_then(Value::as_str) {
            Some(id) => Redirect::to(&format!("/studies/{id}")).into_response(),
            None => Redirect::to(&format!("/workspaces/{workspace_id}/science")).into_response(),
        },
        Err(failure) => failure_response(&failure),
    }
}

/// Sem ecrã entregue pelo Design: a janela `app_pending` na casca (D001).
async fn study_detail(State(state): State<WorkspaceState>, headers: HeaderMap) -> Response {
    app_page(&state, &headers, Screen::Projects).await
}

/// Sem ecrã entregue pelo Design: a janela `app_pending` na casca (D001).
async fn new_execution(State(state): State<WorkspaceState>, headers: HeaderMap) -> Response {
    app_page(&state, &headers, Screen::Projects).await
}

#[derive(Deserialize)]
struct NovaExecucaoForm {
    #[serde(default)]
    status: String,
    #[serde(default)]
    environment: String,
    #[serde(default)]
    software_name: String,
    #[serde(default)]
    software_version: String,
    #[serde(default)]
    notes: String,
    #[serde(default)]
    methodology_version_id: String,
    #[serde(default)]
    dataset_version_id: String,
}

async fn record_execution(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(study_id): Path<Uuid>,
    Form(form): Form<NovaExecucaoForm>,
) -> Response {
    let member = member_or_login!(state, headers);

    let datasets: Vec<String> = blank_to_none(form.dataset_version_id).into_iter().collect();

    let corpo = serde_json::json!({
        "status": blank_to_none(form.status),
        "environment": blank_to_none(form.environment),
        "software_name": blank_to_none(form.software_name),
        "software_version": blank_to_none(form.software_version),
        "notes": blank_to_none(form.notes),
        "methodology_version_id": blank_to_none(form.methodology_version_id),
        "dataset_version_ids": datasets,
    });

    match api::post(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        &format!("/api/v1/studies/{study_id}/executions"),
        &corpo,
    )
    .await
    {
        Ok(criada) => match criada.get("id").and_then(Value::as_str) {
            Some(id) => Redirect::to(&format!("/executions/{id}")).into_response(),
            None => Redirect::to(&format!("/studies/{study_id}")).into_response(),
        },
        Err(failure) => failure_response(&failure),
    }
}

// ── Execução e resultado ─────────────────────────────────────────────────

/// Sem ecrã entregue pelo Design: a janela `app_pending` na casca (D001).
async fn execution_detail(State(state): State<WorkspaceState>, headers: HeaderMap) -> Response {
    app_page(&state, &headers, Screen::Projects).await
}

/// Uma execução, o estudo a que pertence, e o que ela produziu.
///
/// Os resultados vêm do ambiente e são filtrados por esta execução: não há
/// listagem por execução no Core, e inventar uma rota só para o ecrã seria pôr
/// no Core uma pergunta que só a interface faz.
async fn cadeia_da_execucao(
    state: &WorkspaceState,
    member: &Member,
    execution_id: Uuid,
) -> Result<(Value, Value, Value), api::ApiFailure> {
    let execution = api::get::<Value>(
        state,
        &member.session.access_token,
        &member.correlation_id,
        &format!("/api/v1/executions/{execution_id}"),
    )
    .await?;

    let study_id = text_de(&execution, "study_id");
    let study = api::get::<Value>(
        state,
        &member.session.access_token,
        &member.correlation_id,
        &format!("/api/v1/studies/{study_id}"),
    )
    .await?;

    let workspace_id = text_de(&study, "workspace_id");
    let todos = optional(
        state,
        member,
        &format!("/api/v1/workspaces/{workspace_id}/results"),
    )
    .await;

    let esperado = execution_id.to_string();
    let meus: Vec<Value> = todos
        .as_array()
        .map(|linhas| {
            linhas
                .iter()
                .filter(|r| r.get("execution_id").and_then(Value::as_str) == Some(&esperado))
                .cloned()
                .collect()
        })
        .unwrap_or_default();

    Ok((execution, study, Value::Array(meus)))
}

fn text_de(valor: &Value, chave: &str) -> String {
    valor
        .get(chave)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned()
}

/// Sem ecrã entregue pelo Design: a janela `app_pending` na casca (D001).
async fn new_result(State(state): State<WorkspaceState>, headers: HeaderMap) -> Response {
    app_page(&state, &headers, Screen::Projects).await
}

#[derive(Deserialize)]
struct NovoResultadoForm {
    #[serde(default)]
    title: String,
    #[serde(default)]
    summary: String,
    #[serde(default)]
    classification: String,
}

async fn create_result(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(execution_id): Path<Uuid>,
    Form(form): Form<NovoResultadoForm>,
) -> Response {
    let member = member_or_login!(state, headers);

    // O ambiente vem do estudo, e a origem vem do caminho.
    //
    // A pessoa nunca escolhe de onde o resultado veio: abriu este formulário
    // dentro de uma execução, e é essa execução que a operação do Core liga —
    // na mesma transacção, com `origin = operation`.
    let (_, study, _) = match cadeia_da_execucao(&state, &member, execution_id).await {
        Ok(tudo) => tudo,
        Err(failure) => return failure_response(&failure),
    };
    let workspace_id = text_de(&study, "workspace_id");

    let corpo = serde_json::json!({
        "title": form.title,
        "summary": form.summary,
        "execution_id": execution_id,
        "classification": blank_to_none(form.classification),
    });

    match api::post(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        &format!("/api/v1/workspaces/{workspace_id}/results"),
        &corpo,
    )
    .await
    {
        Ok(criado) => match criado.get("id").and_then(Value::as_str) {
            Some(id) => Redirect::to(&format!("/results/{id}")).into_response(),
            None => Redirect::to(&format!("/executions/{execution_id}")).into_response(),
        },
        Err(failure) => failure_response(&failure),
    }
}

/// Campos de uma validação.
#[derive(Deserialize)]
struct ValidationForm {
    #[serde(default)]
    kind: String,
    #[serde(default)]
    outcome: String,
    #[serde(default)]
    execution_id: String,
    #[serde(default)]
    note: String,
}

/// Sem ecrã entregue pelo Design: a janela `app_pending` na casca (D001).
async fn validate_result_form(State(state): State<WorkspaceState>, headers: HeaderMap) -> Response {
    app_page(&state, &headers, Screen::Projects).await
}

/// Regista a afirmação, em nome de quem a faz.
async fn record_validation(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(result_id): Path<Uuid>,
    Form(form): Form<ValidationForm>,
) -> Response {
    let member = member_or_login!(state, headers);

    // Uma cadeia vazia no `<select>` é «nenhuma», e não um identificador
    // inválido: submeter `""` como UUID faria o Core recusar por má forma em
    // vez de aceitar a ausência, que é o que a pessoa escolheu.
    let execution_id = (!form.execution_id.is_empty()).then_some(form.execution_id.as_str());
    let note = (!form.note.trim().is_empty()).then(|| form.note.trim());

    let body = serde_json::json!({
        "kind": form.kind,
        "outcome": form.outcome,
        "execution_id": execution_id,
        "note": note,
    });

    match api::post(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        &format!("/api/v1/results/{result_id}/validations"),
        &body,
    )
    .await
    {
        Ok(_) => Redirect::to(&format!("/results/{result_id}")).into_response(),
        Err(failure) => failure_response(&failure),
    }
}

// ── Conhecimento ─────────────────────────────────────────────────────────

// ── Inteligência ─────────────────────────────────────────────────────────

/// Sem ecrã entregue pelo Design: a janela `app_pending` na casca (D001).
async fn ai_hub(State(state): State<WorkspaceState>, headers: HeaderMap) -> Response {
    app_page(&state, &headers, Screen::Ai).await
}

/// Sem ecrã entregue pelo Design: a janela `app_pending` na casca (D001).
async fn new_agent(State(state): State<WorkspaceState>, headers: HeaderMap) -> Response {
    app_page(&state, &headers, Screen::Agents).await
}

/// Sem ecrã entregue pelo Design: a janela `app_pending` na casca (D001).
async fn agent_detail(State(state): State<WorkspaceState>, headers: HeaderMap) -> Response {
    app_page(&state, &headers, Screen::Agents).await
}

/// Campos do construtor de agentes.
#[derive(Deserialize)]
struct NewAgentForm {
    #[serde(default)]
    name: String,
    #[serde(default)]
    purpose: String,
    #[serde(default)]
    instructions: String,
    #[serde(default)]
    capability: String,
    #[serde(default)]
    scope: String,
    #[serde(default)]
    max_classification: String,
    // Uma checkbox não marcada não é submetida: a ausência do campo é `false`.
    #[serde(default)]
    uses_bibliography: Option<String>,
    #[serde(default)]
    uses_documents: Option<String>,
    #[serde(default)]
    uses_datasets: Option<String>,
}

/// `POST /ai/agents/new`
///
/// Antes desta auditoria este caminho não existia: o formulário submetia e o
/// Axum devolvia 405. Um agente é uma definição e guarda-se sem nó de IA; o que
/// falta é onde correr, e o estado do agente di-lo.
async fn create_agent(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Form(form): Form<NewAgentForm>,
) -> Response {
    let member = member_or_login!(state, headers);

    let body = serde_json::json!({
        "name": form.name,
        "purpose": form.purpose,
        "instructions": form.instructions,
        "capability": if form.capability.is_empty() { "general".to_owned() } else { form.capability },
        "scope": if form.scope.is_empty() { "personal".to_owned() } else { form.scope },
        "max_classification": if form.max_classification.is_empty() {
            "INTERNAL".to_owned()
        } else {
            form.max_classification
        },
        "uses_bibliography": form.uses_bibliography.is_some(),
        "uses_documents": form.uses_documents.is_some(),
        "uses_datasets": form.uses_datasets.is_some(),
    });

    match api::post(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        "/api/v1/ai/agents",
        &body,
    )
    .await
    {
        Ok(_) => Redirect::to("/ai/agents").into_response(),
        Err(ApiFailure::Unauthorised) => Redirect::to("/login").into_response(),
        Err(failure) => failure_response(&failure),
    }
}

#[derive(Deserialize, Default)]
struct NyeAppQuery {
    /// A conversa aberta.
    #[serde(default)]
    c: Option<Uuid>,
    /// Filtro da lista de conversas.
    #[serde(default)]
    cq: Option<String>,
    /// `sources` | `activity`.
    #[serde(default)]
    panel: Option<String>,
    /// Modo de voz.
    #[serde(default)]
    voice: Option<String>,
    /// D004 · A referência tipada de uma aplicação (`note:<id>`, `event:<id>`,
    /// `file:<id>`, `message:<id>`). Só contexto: relê-se com a sessão do
    /// membro, e o que ele não pode ver não aparece.
    #[serde(default)]
    r#ref: Option<String>,
}

/// `GET /ai/prompt` (D003 · FG-D3-41): a aplicação Nye, numa janela gerida de
/// uma só instância. As conversas vêm do Core e são do membro; uma conversa
/// que não é dele não abre (o Core responde como se não existisse).
async fn prompt(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Query(query): Query<NyeAppQuery>,
) -> Response {
    use leptos::prelude::*;
    use ui::view_models::{NyeLang, NyePanel, NyeVoiceState, NyeVoiceVm, WindowContent};

    let member = member_or_login!(state, headers);
    let quem = caller(&member);
    let title = Screen::Prompt.label().to_owned();
    let href = Screen::Prompt.path();
    let mut ctx = match controllers::shell(&state, &quem, href, title.clone()).await {
        Shell::Ready(ctx) => ctx,
        Shell::SignIn => return session_ended(&state, &headers),
        Shell::Indeterminate(reference) => {
            return identity_indeterminate(&state, reference, href).await
        }
    };
    if !screen_open(&ctx, Screen::Prompt) {
        let vm = ErrorVm {
            kind: ErrorKind::NotFound,
            reference: None,
            retry_href: None,
        };
        return (
            StatusCode::NOT_FOUND,
            html(
                error_title(vm.kind),
                Surface::Shell,
                ui::screens::error::in_shell(&ctx.vm, &vm),
            ),
        )
            .into_response();
    }
    let page = controllers::windows::page_query(href);
    // A conversa é estado da aplicação, não da janela: a janela da Nye é uma
    // só, e mostra a rota da aplicação.
    let _ = controllers::windows::open(
        &state.sessions,
        &member.session_id,
        ApplicationId::Prompt,
        href,
        false,
    );
    ctx.vm.wm = controllers::windows::view(&state.sessions, &member.session_id, &ctx);
    if let Some(wm) = ctx.vm.wm.as_mut() {
        for w in wm.windows.iter_mut().filter(|w| w.app_id == "prompt") {
            w.content = WindowContent::Ready;
        }
    }
    let clock = controllers::desktop::Clock {
        now: chrono::Utc::now(),
        zone: ctx.zone,
        core_ok: ctx.core.operational(),
        is_admin: ctx.is_admin,
    };
    let availability = match ctx.vm.nye.as_ref() {
        Some(n) => n.availability,
        None => return failure_response(&ApiFailure::Unavailable(None)),
    };
    let current_id = query.c.map(|c| c.to_string());
    let (list, current) = tokio::join!(quem.get(&state, "/api/v1/ai/conversations"), async {
        match query.c {
            Some(id) => Some(
                quem.get(&state, &format!("/api/v1/ai/conversations/{id}"))
                    .await,
            ),
            None => None,
        }
    });
    let conversations = list.ok().map(|l| {
        controllers::nye::conversations(
            &l,
            current_id.as_deref(),
            query.cq.as_deref().unwrap_or_default(),
            &clock,
        )
    });
    // Uma conversa que não abre (de outra pessoa, ou que já não existe) é uma
    // conversa nova: não se diz porquê.
    let current = current
        .and_then(Result::ok)
        .map(|v| controllers::nye::conversation(&v, &clock));
    let open_id = current.as_ref().and(current_id.as_deref());
    let app = ui::view_models::NyeAppVm {
        availability,
        context: None,
        conversations,
        conv_query: query.cq.clone().unwrap_or_default(),
        current,
        composer: {
            let mut c = controllers::nye::composer(open_id, availability.ask);
            if let Some(about) =
                productivity::nye_reference(&state, &member, query.r#ref.as_deref()).await
            {
                c.text = about;
            }
            c
        },
        voice: (query.voice.as_deref() == Some("1")).then(|| NyeVoiceVm {
            state: match availability.voice_input {
                ui::view_models::NyeAvail::Available => NyeVoiceState::Idle,
                ui::view_models::NyeAvail::Unavailable(r) => NyeVoiceState::Unavailable(r),
            },
            lang: match crate::i18n::current() {
                ocinye_contracts::Locale::En => NyeLang::En,
                ocinye_contracts::Locale::Fr => NyeLang::Fr,
                _ => NyeLang::Pt,
            },
            lang_detected: false,
            transcript: None,
            replay: false,
        }),
        panel: match query.panel.as_deref() {
            Some("sources") => NyePanel::Sources,
            Some("activity") => NyePanel::Activity,
            _ => NyePanel::None,
        },
        panel_sources: Vec::new(),
        panel_steps: Vec::new(),
        suggestions: Vec::new(),
    };
    if page.frame {
        let body = view! {
            <template data-part="win-title">{title.clone()}</template>
            {ui::nye::app(&app)}
        };
        return ([(header::CACHE_CONTROL, "no-store")], Html(body.to_html())).into_response();
    }
    let engine = ctx.vm.wm.is_some();
    let body =
        ui::shell::shell_with_window(&ctx.vm, ().into_any(), Some(ui::nye::app(&app).into_any()));
    shell_page(&title, engine, body, None)
}

/// O pedido escrito na Nye: o campo `q` do compositor do Design.
#[derive(Deserialize)]
struct PromptForm {
    #[serde(default)]
    q: String,
}

#[derive(Deserialize, Default)]
struct PromptQuery {
    #[serde(default)]
    c: Option<Uuid>,
}

/// `POST /ai/prompt` (D003 · NYE-02, NYE-03): o pedido vai ao Core, que o
/// guarda na conversa do membro (a aberta, se for dele) e responde — com uma
/// resposta de modelo ou com a conclusão degradada tipada. Volta à conversa.
async fn submit_prompt(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Query(query): Query<PromptQuery>,
    Form(form): Form<PromptForm>,
) -> Response {
    let member = member_or_login!(state, headers);
    let mut back = query
        .c
        .map_or_else(|| "/ai/prompt".to_owned(), |c| format!("/ai/prompt?c={c}"));
    // Um pedido vazio não é um turno: nada foi pedido, e nada vai ao Core.
    if !form.q.trim().is_empty() {
        let mut body = serde_json::json!({ "prompt": form.q });
        if let Some(c) = query.c {
            body["conversation_id"] = Value::String(c.to_string());
        }
        match api::post(
            &state,
            &member.session.access_token,
            &member.correlation_id,
            "/api/v1/ai/prompt",
            &body,
        )
        .await
        {
            Ok(reply) => {
                if let Some(c) = reply.get("conversation_id").and_then(Value::as_str) {
                    back = format!("/ai/prompt?c={c}");
                }
            }
            Err(ApiFailure::Unauthorised) => return session_ended(&state, &headers),
            Err(failure) => return failure_response(&failure),
        }
    }
    Redirect::to(&back).into_response()
}

/// Sem ecrã entregue pelo Design: a janela `app_pending` na casca (D001).
async fn search(State(state): State<WorkspaceState>, headers: HeaderMap) -> Response {
    app_page(&state, &headers, Screen::Search).await
}

// ── A Universal Command Surface ──────────────────────────────────────────

#[derive(Deserialize, Default)]
struct AskQuery {
    #[serde(default)]
    q: Option<String>,
    #[serde(default)]
    intent: Option<String>,
    /// Mostrar este plano (depois de executar ou recusar).
    #[serde(default)]
    plan: Option<Uuid>,
    /// Pedir a confirmação forte deste plano.
    #[serde(default)]
    confirm: Option<Uuid>,
}

/// Um pedido à Nye: `POST /agentic/invoke` em nome do membro.
async fn nye_invoke(
    state: &WorkspaceState,
    member: &Member,
    text: &str,
    intent: Option<&str>,
) -> Result<Value, ApiFailure> {
    let mut body = serde_json::json!({ "utterance": text.trim() });
    if let Some(i) = intent {
        body["intent"] = Value::String(i.to_owned());
    }
    api::post(
        state,
        &member.session.access_token,
        &member.correlation_id,
        "/api/v1/agentic/invoke",
        &body,
    )
    .await
}

/// A pesquisa determinística do Core (`GET /search`), com a autorização dele:
/// os mesmos campos que as fontes do `invoke`.
async fn nye_search(
    state: &WorkspaceState,
    member: &Member,
    text: &str,
) -> Result<Vec<Value>, ApiFailure> {
    let page = api::get::<Value>(
        state,
        &member.session.access_token,
        &member.correlation_id,
        &format!(
            "/api/v1/search?q={}&page_size=36",
            urlencoding_minimal(text.trim())
        ),
    )
    .await?;
    Ok(page
        .get("items")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default())
}

/// O que o Core diz de um plano do membro, reavaliado agora.
async fn plan_detail(
    state: &WorkspaceState,
    member: &Member,
    plan_id: Uuid,
) -> Result<Value, ApiFailure> {
    api::get::<Value>(
        state,
        &member.session.access_token,
        &member.correlation_id,
        &format!("/api/v1/agentic/plans/{plan_id}"),
    )
    .await
}

/// `GET /ask` (D003 · NYE-04, NYE-06): a superfície universal da Nye aberta
/// por cima do Desktop.
///
/// Pesquisar vai ao Core (`POST /agentic/invoke`, determinístico, filtrado
/// para quem pergunta); perguntar e agir também, e o Core responde com um
/// plano ou com a razão tipada por que não pode. Sem inferência, a pesquisa
/// continua: o mesmo pedido é respondido como pesquisa.
async fn ask(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Query(query): Query<AskQuery>,
) -> Response {
    let member = member_or_login!(state, headers);
    let quem = caller(&member);
    let crumb = Screen::Home.label().to_owned();
    let mut ctx = match controllers::shell(&state, &quem, "/", crumb).await {
        Shell::Ready(ctx) => ctx,
        Shell::SignIn => return session_ended(&state, &headers),
        Shell::Indeterminate(reference) => {
            return identity_indeterminate(&state, reference, "/ask").await
        }
    };
    ctx.vm.wm = controllers::windows::view(&state.sessions, &member.session_id, &ctx);
    let clock = controllers::desktop::Clock {
        now: chrono::Utc::now(),
        zone: ctx.zone,
        core_ok: ctx.core.operational(),
        is_admin: ctx.is_admin,
    };
    let text = query.q.clone().unwrap_or_default();
    let intent = controllers::nye::intent_of(query.intent.as_deref());
    let base = ctx.vm.nye.as_ref().map(|n| n.availability);
    let Some(mut availability) = base else {
        return failure_response(&ApiFailure::Unavailable(None));
    };
    let mut hits = Vec::new();
    let mut answer = None;
    let mut dialog = None;

    if !text.trim().is_empty() {
        match nye_invoke(
            &state,
            &member,
            &text,
            intent.map(ui::view_models::NyeIntent::as_str),
        )
        .await
        {
            Ok(outcome) => match outcome.get("kind").and_then(Value::as_str) {
                Some("results") => {
                    hits = controllers::nye::hits(
                        outcome
                            .get("sources")
                            .and_then(Value::as_array)
                            .map_or(&[][..], Vec::as_slice),
                    );
                }
                Some("planned" | "executed") => {
                    let requires = outcome
                        .get("requires_approval")
                        .and_then(Value::as_bool)
                        .unwrap_or(false);
                    if let Some(plan) = outcome.get("plan") {
                        answer = Some(controllers::nye::proposals_message(vec![
                            controllers::nye::proposal(plan, requires, &clock),
                        ]));
                    }
                }
                Some("unavailable") => {
                    let reason = controllers::nye::reason_of(
                        outcome
                            .get("reason_code")
                            .and_then(Value::as_str)
                            .unwrap_or_default(),
                    );
                    let off = ui::view_models::NyeAvail::Unavailable(reason);
                    availability.ask = off;
                    availability.act = off;
                    // A pesquisa continua a funcionar sem inferência — e sem
                    // `ai.use`: o `invoke` recusa tudo a quem não pode usar a
                    // assistência, pesquisa incluída, por isso a pesquisa sai
                    // da rota determinística do Core, com a mesma autorização.
                    match nye_search(&state, &member, &text).await {
                        Ok(items) => hits = controllers::nye::hits(&items),
                        Err(ApiFailure::Unauthorised) => {
                            return session_ended(&state, &headers);
                        }
                        Err(failure) => {
                            tracing::warn!(correlation_id = %member.correlation_id, %failure, "nye search failed");
                            availability.search = ui::view_models::NyeAvail::Unavailable(
                                ui::view_models::NyeReason::CoreUnavailable,
                            );
                        }
                    }
                }
                _ => {}
            },
            Err(ApiFailure::Unauthorised) => return session_ended(&state, &headers),
            Err(failure) => {
                tracing::warn!(correlation_id = %member.correlation_id, %failure, "nye invoke failed");
                let down = ui::view_models::NyeAvail::Unavailable(
                    ui::view_models::NyeReason::CoreUnavailable,
                );
                availability.search = down;
            }
        }
    }

    // Um plano pedido pelo endereço: sempre o que o Core diz agora, e só se
    // for do membro (um plano de outra pessoa lê-se como inexistente).
    if let Some(plan_id) = query.confirm.or(query.plan) {
        if let Ok(plan) = plan_detail(&state, &member, plan_id).await {
            let requires = plan
                .get("requires_approval")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            let p = controllers::nye::proposal(&plan, requires, &clock);
            let open = plan.get("open").and_then(Value::as_bool) == Some(true);
            if query.confirm.is_some() && open && requires && p.risk.high_impact() {
                dialog = Some(nye_confirm_dialog(&p));
            }
            answer = Some(controllers::nye::proposals_message(vec![p]));
        }
    }

    let mut surface = controllers::nye::surface(availability, &text, intent, true);
    surface.hits = hits;
    surface.answer = answer;
    ctx.vm.nye = Some(surface);
    let engine = ctx.vm.wm.is_some();
    let vm = controllers::desktop::desktop(*ctx, &quem, &state).await;
    shell_page(
        crate::i18n::t("nye.surface.label"),
        engine,
        ui::screens::home::home(&vm),
        dialog,
    )
}

fn nye_confirm_dialog(p: &ui::view_models::NyeProposalVm) -> leptos::prelude::AnyView {
    use leptos::prelude::*;
    ui::nye::confirm_dialog(p).into_any()
}

#[derive(Deserialize)]
struct ExecuteForm {
    #[serde(default)]
    digest: Option<String>,
}

/// Confirma e executa um plano (D003 · NYE-07, NYE-08).
///
/// A confirmação vale para o que o membro viu: o formulário traz o `digest`
/// que foi mostrado, e um plano que já não é esse (ou um pedido sem ele) é
/// recusado antes de se pedir ao Core que aprove. A aprovação continua ligada
/// ao `digest` do próprio Core; esta verificação só impede que um formulário
/// antigo confirme outra coisa.
async fn execute_plan(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(plan_id): Path<Uuid>,
    Form(form): Form<ExecuteForm>,
) -> Response {
    let member = member_or_login!(state, headers);
    let back = format!("/ask?plan={plan_id}");

    let plan = match plan_detail(&state, &member, plan_id).await {
        Ok(plan) => plan,
        Err(ApiFailure::Unauthorised) => return session_ended(&state, &headers),
        Err(failure) => return failure_response(&failure),
    };
    let current = plan
        .get("digest")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let shown = form.digest.as_deref().unwrap_or_default();
    if current.is_empty() || shown != current {
        tracing::warn!(correlation_id = %member.correlation_id, %plan_id, "confirmation does not match the plan");
        return (StatusCode::CONFLICT, Redirect::to(&back)).into_response();
    }

    // Confirmar e executar são dois pedidos ao Core, nesta ordem: a
    // confirmação liga-se ao digest do plano, e a execução verifica-a. Um só
    // pedido tornaria impossível distinguir «confirmado» de «executado».
    let approve = format!("/api/v1/agentic/plans/{plan_id}/approve");
    if let Err(failure) = api::post(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        &approve,
        &serde_json::json!({}),
    )
    .await
    {
        return failure_response(&failure);
    }

    let execute = format!("/api/v1/agentic/plans/{plan_id}/execute");
    match api::post(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        &execute,
        &serde_json::json!({}),
    )
    .await
    {
        Ok(_) | Err(ApiFailure::Denied) => Redirect::to(&back).into_response(),
        Err(failure) => failure_response(&failure),
    }
}

/// Recusa um plano.
async fn reject_plan(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(plan_id): Path<Uuid>,
) -> Response {
    let member = member_or_login!(state, headers);

    let path = format!("/api/v1/agentic/plans/{plan_id}/reject");
    match api::post(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        &path,
        &serde_json::json!({}),
    )
    .await
    {
        Ok(_) | Err(ApiFailure::Denied) => {
            Redirect::to(&format!("/ask?plan={plan_id}")).into_response()
        }
        Err(failure) => failure_response(&failure),
    }
}

/// Escapa um termo para o colocar numa query string.
///
/// Mínimo de propósito: só os caracteres que quebrariam a query. Uma dependência
/// inteira para isto seria desproporcionada (`CLAUDE.md` §54).
fn urlencoding_minimal(value: &str) -> String {
    value
        .bytes()
        .map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                char::from(byte).to_string()
            }
            other => format!("%{other:02X}"),
        })
        .collect()
}

/// Sem ecrã entregue pelo Design: a janela `app_pending` na casca (D001).
async fn compute(State(state): State<WorkspaceState>, headers: HeaderMap) -> Response {
    app_page(&state, &headers, Screen::Compute).await
}

// ── Institucional ────────────────────────────────────────────────────────

/// Sem ecrã entregue pelo Design: a janela `app_pending` na casca (D001).
async fn activity(State(state): State<WorkspaceState>, headers: HeaderMap) -> Response {
    app_page(&state, &headers, Screen::Activity).await
}

// ── Criar ideia ──────────────────────────────────────────────────────────

/// Sem ecrã entregue pelo Design: a janela `app_pending` na casca (D001).
async fn settings_account(State(state): State<WorkspaceState>, headers: HeaderMap) -> Response {
    app_page(&state, &headers, Screen::Settings).await
}

/// Sem ecrã entregue pelo Design: a janela `app_pending` na casca (D001).
async fn settings_language(State(state): State<WorkspaceState>, headers: HeaderMap) -> Response {
    app_page(&state, &headers, Screen::Settings).await
}

/// A escolha de idioma submetida.
#[derive(Deserialize)]
struct LanguageForm {
    locale: String,
}

/// Grava o idioma escolhido.
///
/// O idioma valida-se contra a lista fechada (`pt`/`en`/`fr`): um valor fora dos
/// três não escreve cookie nenhum — entrada não confiável não vira preferência
/// (i18n §68). O cookie é a cópia por browser da preferência; a redirecção volta
/// à mesma página, que a renderiza já na língua nova porque o middleware relê o
/// cookie acabado de escrever.
async fn set_language(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Form(form): Form<LanguageForm>,
) -> Response {
    let _member = member_or_login!(state, headers);
    let Ok(locale) = form.locale.parse::<ocinye_contracts::Locale>() else {
        return Redirect::to("/settings/language").into_response();
    };
    let cookie = session::locale_cookie_header(locale.as_str(), state.config.cookie_secure);
    let mut resposta = Redirect::to("/settings/language?ok=1").into_response();
    if let Ok(valor) = HeaderValue::from_str(&cookie) {
        resposta.headers_mut().insert(header::SET_COOKIE, valor);
    }
    resposta
}

/// Sem ecrã entregue pelo Design: a janela `app_pending` na casca (D001).
async fn settings_apps(State(state): State<WorkspaceState>, headers: HeaderMap) -> Response {
    app_page(&state, &headers, Screen::Settings).await
}

/// Sem ecrã entregue pelo Design: a janela `app_pending` na casca (D001).
async fn admin_instance(State(state): State<WorkspaceState>, headers: HeaderMap) -> Response {
    app_page(&state, &headers, Screen::Admin).await
}

/// Grava o perfil e o estado de cada aplicação opcional que mudou.
///
/// O corpo traz `profile` e um `app:<id>` por aplicação opcional, com
/// `profile`, `active` ou `inactive`. Só se envia ao Core o que mudou; o Core
/// valida os identificadores, recusa as essenciais e reautoriza tudo.
async fn save_instance(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    corpo: axum::body::Bytes,
) -> Response {
    let Some(member) = membro_ou_recusa(&state, &headers) else {
        return Redirect::to("/login").into_response();
    };
    let actual = match required(&state, &member, "/api/v1/instance/applications").await {
        Ok(payload) => payload,
        Err(failure) => return failure_response(&failure),
    };

    let mut perfil: Option<String> = None;
    let mut pedidos: Vec<(String, Option<bool>)> = Vec::new();
    for (chave, valor) in url::form_urlencoded::parse(&corpo) {
        if chave == "profile" {
            perfil = Some(valor.into_owned());
        } else if let Some(id) = chave.strip_prefix("app:") {
            // O id entra no caminho de um pedido ao Core: só um id que o registo
            // conhece, nunca o texto do formulário tal como veio (um `../` levaria
            // o pedido a outra rota).
            if experience::apps::by_id(id).is_none() {
                continue;
            }
            let pedido = match valor.as_ref() {
                "active" => Some(true),
                "inactive" => Some(false),
                _ => None,
            };
            pedidos.push((id.to_owned(), pedido));
        }
    }

    // O perfil primeiro: as aplicações «como o perfil» leem o novo.
    if let Some(perfil) =
        perfil.filter(|p| actual.get("profile").and_then(Value::as_str) != Some(p))
    {
        if let Err(failure) = api::put(
            &state,
            &member.session.access_token,
            &member.correlation_id,
            "/api/v1/instance/profile",
            &serde_json::json!({ "profile": perfil }),
        )
        .await
        {
            return failure_response(&failure);
        }
    }

    let estado_de = |id: &str| -> Option<Option<bool>> {
        actual
            .get("applications")
            .and_then(Value::as_array)?
            .iter()
            .find(|a| a.get("id").and_then(Value::as_str) == Some(id))
            .map(|a| {
                if a.get("explicit").and_then(Value::as_bool) == Some(true) {
                    a.get("active").and_then(Value::as_bool)
                } else {
                    None
                }
            })
    };
    for (id, pedido) in pedidos {
        if estado_de(&id) == Some(pedido) {
            continue;
        }
        if let Err(failure) = api::put(
            &state,
            &member.session.access_token,
            &member.correlation_id,
            &format!("/api/v1/instance/applications/{id}"),
            &serde_json::json!({ "active": pedido }),
        )
        .await
        {
            return failure_response(&failure);
        }
    }

    Redirect::to("/admin/instance?ok=1").into_response()
}

/// Grava o conjunto de aplicações fixadas, ou repõe as predefinições.
///
/// O corpo traz uma `pinned` por caixa marcada (`x-www-form-urlencoded` com
/// chaves repetidas, que o `Form` não desserializa para `Vec`; lê-se o corpo em
/// bruto). Guardar **preserva a ordem** que o membro tinha para as que ficam, e
/// junta as novas ao fim — a ordem fina muda-se arrastando na barra. «Repor»
/// (`action=reset`) devolve o conjunto por omissão. Só se guardam ids de
/// aplicações reais e fixáveis; o resto cai em silêncio.
async fn save_apps(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    corpo: axum::body::Bytes,
) -> Response {
    let Some(member) = membro_ou_recusa(&state, &headers) else {
        return Redirect::to("/login").into_response();
    };

    let mut marcadas: Vec<String> = Vec::new();
    let mut repor = false;
    for (chave, valor) in url::form_urlencoded::parse(&corpo) {
        match chave.as_ref() {
            "pinned" if experience::apps::is_pinnable(&valor) => marcadas.push(valor.into_owned()),
            "action" if valor == "reset" => repor = true,
            _ => {}
        }
    }

    let nova = if repor {
        experience::apps::default_pins()
    } else {
        // Preserva a ordem actual para as que ficam, e junta as novas ao fim.
        let atuais: Vec<String> = api::get::<Value>(
            &state,
            &member.session.access_token,
            &member.correlation_id,
            "/api/v1/me/apps/pins",
        )
        .await
        .ok()
        .and_then(|v| {
            v.get("pinned").and_then(Value::as_array).map(|a| {
                a.iter()
                    .filter_map(Value::as_str)
                    .map(str::to_owned)
                    .collect()
            })
        })
        .unwrap_or_default();

        let marcadas_set: std::collections::BTreeSet<&str> =
            marcadas.iter().map(String::as_str).collect();
        let mut ordenada: Vec<String> = atuais
            .iter()
            .filter(|id| marcadas_set.contains(id.as_str()))
            .cloned()
            .collect();
        for id in &marcadas {
            if !ordenada.contains(id) {
                ordenada.push(id.clone());
            }
        }
        ordenada
    };

    let _ = api::put(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        "/api/v1/me/apps/pins",
        &serde_json::json!({ "pinned": nova }),
    )
    .await;

    Redirect::to("/settings/apps?ok=1").into_response()
}

/// Sem ecrã entregue pelo Design: a janela `app_pending` na casca (D001).
async fn help(State(state): State<WorkspaceState>, headers: HeaderMap) -> Response {
    app_page(&state, &headers, Screen::Help).await
}

/// Sem ecrã entregue pelo Design: a janela `app_pending` na casca (D001).
async fn terminal(State(state): State<WorkspaceState>, headers: HeaderMap) -> Response {
    app_page(&state, &headers, Screen::Terminal).await
}

/// `POST /terminal/exec` — leva uma linha ao Core e devolve-a localizada.
///
/// Não faz parse que conte nem decide nada: o Core faz os dois (ADR-0312 §2).
/// O que volta é JSON para o `terminal.js`, que o desenha com nós de texto.
async fn terminal_exec(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Json(body): Json<ocinye_contracts::ocsh::wire::ExecRequest>,
) -> Response {
    let Some(member) = current_member(&state, &headers) else {
        return (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({ "error": "sem sessão" })),
        )
            .into_response();
    };
    let pedido = serde_json::json!({ "line": body.line, "context": body.context });
    let resposta = api::post(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        "/api/v1/commands/exec",
        &pedido,
    )
    .await;
    match resposta {
        Ok(valor) => {
            match serde_json::from_value::<ocinye_contracts::ocsh::wire::ExecResponse>(valor) {
                Ok(r) => Json(crate::terminal::localize(&r)).into_response(),
                Err(_) => {
                    Json(crate::terminal::transport_failure("ocsh.err.failed", 1)).into_response()
                }
            }
        }
        Err(ApiFailure::Unauthorised) => (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({ "error": "sem sessão" })),
        )
            .into_response(),
        Err(ApiFailure::ApplicationInactive | ApiFailure::Unavailable(_)) => Json(
            crate::terminal::transport_failure("ocsh.err.unavailable", 69),
        )
        .into_response(),
        Err(ApiFailure::Forbidden | ApiFailure::Denied) => {
            Json(crate::terminal::transport_failure("ocsh.denied", 77)).into_response()
        }
        Err(_) => Json(crate::terminal::transport_failure("ocsh.err.network", 1)).into_response(),
    }
}

/// Largest body the Workspace accepts for a profile photograph.
///
/// O mesmo limite do Core, mais o envelope multipart. Recusar aqui poupa uma
/// travessia; recusar só aqui não bastaria, porque o Core não pode confiar em
/// quem o chama.
const AVATAR_BODY_LIMIT_BYTES: usize = 8 * 1024 * 1024 + 64 * 1024;

/// Escolhe um avatar do catálogo Ocinye.
async fn choose_avatar_preset(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Form(form): Form<AvatarPresetForm>,
) -> Response {
    let member = member_or_login!(state, headers);
    avatar_outcome(
        api::post(
            &state,
            &member.session.access_token,
            &member.correlation_id,
            "/api/v1/me/avatar/preset",
            &serde_json::json!({ "preset": form.preset }),
        )
        .await,
    )
}

/// O identificador escolhido na grelha de avatares.
#[derive(Deserialize)]
struct AvatarPresetForm {
    preset: String,
}

/// Volta às iniciais.
async fn use_initials_avatar(State(state): State<WorkspaceState>, headers: HeaderMap) -> Response {
    let member = member_or_login!(state, headers);

    // A operação é `DELETE` no Core e `POST` aqui: um formulário HTML só sabe
    // enviar `GET` e `POST`, e usar `GET` para uma operação que muda estado é
    // exactamente o que a guarda de mesma origem existe para impedir.
    let resultado = match state
        .http
        .delete(format!("{}/api/v1/me/avatar", state.config.core_url))
        .bearer_auth(&member.session.access_token)
        .header(
            ocinye_observability::CORRELATION_ID_HEADER,
            &member.correlation_id,
        )
        .send()
        .await
    {
        Ok(response) if response.status().is_success() => Ok(serde_json::Value::Null),
        Ok(response) if response.status().as_u16() == 401 => Err(ApiFailure::Unauthorised),
        Ok(response) => Err(ApiFailure::Failed(format!(
            "the Core returned status {}",
            response.status()
        ))),
        Err(error) => Err(ApiFailure::Failed(format!(
            "the Core is unreachable: {error}"
        ))),
    };

    avatar_outcome(resultado)
}

/// Carrega a fotografia do próprio membro.
async fn upload_avatar(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    mut multipart: Multipart,
) -> Response {
    let member = member_or_login!(state, headers);

    let mut ficheiro: Option<(String, String, Vec<u8>)> = None;
    while let Ok(Some(field)) = multipart.next_field().await {
        if field.name() == Some("file") {
            let nome = field.file_name().unwrap_or("fotografia").to_owned();
            let tipo = field
                .content_type()
                .unwrap_or("application/octet-stream")
                .to_owned();
            match field.bytes().await {
                Ok(bytes) => ficheiro = Some((nome, tipo, bytes.to_vec())),
                Err(_) => return avatar_error("A fotografia não pôde ser lida."),
            }
        }
    }

    let Some((nome, tipo, dados)) = ficheiro else {
        return avatar_error("Escolha uma fotografia antes de confirmar.");
    };
    if dados.is_empty() {
        return avatar_error("Escolha uma fotografia antes de confirmar.");
    }

    // O tipo declarado viaja porque o multipart o exige, e não porque alguém
    // acredite nele: é o Core que decide o formato pelos bytes.
    avatar_outcome(
        api::upload(
            &state,
            &member.session.access_token,
            &member.correlation_id,
            "/api/v1/me/avatar/photo",
            nome,
            tipo,
            dados,
        )
        .await,
    )
}

/// Traduz o resultado de uma mudança de avatar em navegação.
///
/// Sucesso e falha voltam ambos a Definições, e é lá que a mensagem aparece. O
/// estado persistido é o que a página seguinte lê do Core: não há aqui nenhum
/// optimismo local a mostrar uma escolha que o Core não confirmou.
fn avatar_outcome(resultado: Result<serde_json::Value, ApiFailure>) -> Response {
    match resultado {
        Ok(_) => Redirect::to("/settings?avatar=ok").into_response(),
        Err(ApiFailure::Unauthorised) => Redirect::to("/login").into_response(),

        // O armazenamento não está de pé. A mensagem que vinha do Core dizia
        // «The object could not be stored.» — em inglês, e a descrever o que
        // falhou em vez do que se passa. Quem carregou uma fotografia conclui
        // que a fotografia tem alguma coisa de errado, e volta a tentar com
        // outra.
        Err(ApiFailure::Unavailable(_)) => avatar_error(
            "O armazenamento institucional não está a responder. \
             A fotografia não foi guardada — não é um problema com a imagem, \
             e os avatares Ocinye e as iniciais continuam disponíveis.",
        ),

        // O Core recusa um identificador que não esteja no seu catálogo, e a
        // recusa vinha em inglês: «That is not an Ocinye avatar.» Como a grelha
        // só oferece o que está no catálogo, esta recusa só acontece quando os
        // dois lados discordam — tipicamente porque um deles ainda não foi
        // reiniciado. Dizê-lo é mais útil do que repetir a frase do Core.
        Err(ApiFailure::Failed(message)) if message.contains("Ocinye avatar") => avatar_error(
            "Este avatar não faz parte do catálogo que o Core conhece. \
             Se acabou de haver uma actualização, o serviço pode ainda estar a arrancar.",
        ),

        Err(ApiFailure::Failed(message)) => avatar_error(&message),
        Err(_) => avatar_error("A imagem de perfil não pôde ser alterada."),
    }
}

/// Volta a Definições com uma razão que o membro possa ler.
fn avatar_error(message: &str) -> Response {
    Redirect::to(&format!("/settings?avatar_erro={}", urlencoding(message))).into_response()
}

/// Codifica um texto para caber num parâmetro de query.
fn urlencoding(value: &str) -> String {
    value
        .bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
                (b as char).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect()
}

/// A fotografia do próprio membro.
///
/// # Porque passa por aqui
///
/// O Core podia devolver um URL assinado e o browser ir buscá-lo directamente.
/// Não vai: um URL assinado dura cinco minutos, muda a cada render e traz o
/// endereço do bucket consigo. Numa imagem que aparece em todas as páginas, isso
/// significa uma cache que nunca acerta e o armazenamento institucional escrito
/// no HTML.
///
/// A fotografia tem alguns kilobytes, e passá-la por aqui deixa a shell a falar
/// só com o Ocinye OS.
async fn own_avatar(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(version): Path<String>,
) -> Response {
    let Some(member) = current_member(&state, &headers) else {
        // Sem sessão não há avatar, e não há como saber de quem seria. `404` e
        // não um reencaminhamento: quem pediu isto foi um `<img>`, e devolver-lhe
        // a página de login seria devolver HTML a quem esperava uma imagem.
        return StatusCode::NOT_FOUND.into_response();
    };

    match api::bytes(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        &format!("/api/v1/me/avatar/{version}"),
    )
    .await
    {
        Ok((content_type, bytes)) => (
            [
                (header::CONTENT_TYPE, content_type),
                // O endereço só muda quando a fotografia muda, e por isso este
                // conteúdo nunca muda. `private` porque é de uma pessoa: uma
                // cache partilhada não deve servi-lo a outra.
                (
                    header::CACHE_CONTROL,
                    "private, max-age=31536000, immutable".to_owned(),
                ),
            ],
            bytes,
        )
            .into_response(),
        // Uma fotografia que não chega não é uma falha da página: o componente
        // cai nas iniciais sozinho.
        Err(_) => StatusCode::NOT_FOUND.into_response(),
    }
}

/// Sem ecrã entregue pelo Design: a janela `app_pending` na casca (D001).
async fn settings_security(State(state): State<WorkspaceState>, headers: HeaderMap) -> Response {
    app_page(&state, &headers, Screen::Settings).await
}

/// Sem ecrã entregue pelo Design: a janela `app_pending` na casca (D001).
async fn settings_mfa(State(state): State<WorkspaceState>, headers: HeaderMap) -> Response {
    app_page(&state, &headers, Screen::Settings).await
}

/// Acção retirada no apagamento da UI; espera o Claude Design.
///
/// Produz um segredo que se mostra uma única vez; sem ecrã, perdia-se.
async fn settings_mfa_regenerate() -> Response {
    interface_pending()
}

#[derive(Deserialize)]
struct ChangePasswordForm {
    current: String,
    password: String,
    confirmation: String,
}

/// A mudança de palavra-passe, e a rotação de sessão que a acompanha.
///
/// O Core revoga todas as sessões e emite uma nova. A sessão do Workspace é
/// substituída aqui pela mesma razão: manter a antiga deixaria o membro com um
/// identificador que o Core já não reconhece, e o próximo pedido cairia no
/// início de sessão sem explicação nenhuma.
async fn change_password(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Form(form): Form<ChangePasswordForm>,
) -> Response {
    let member = member_or_login!(state, headers);

    let body = serde_json::json!({
        "current": form.current,
        "password": form.password,
        "confirmation": form.confirmation,
    });

    match api::post(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        "/api/v1/auth/password/change",
        &body,
    )
    .await
    .and_then(CoreSession::from_payload)
    {
        Ok(session) => {
            if let Some(id) = session::session_id_from_cookies(
                headers
                    .get(header::COOKIE)
                    .and_then(|value| value.to_str().ok()),
            ) {
                state.sessions.remove(&id);
            }
            let session_id = state.sessions.create(Session {
                access_token: session.token,
                display_name: session.display_name,
                email: member.session.email.clone(),
                must_change_password: session.must_change_password,
                mfa_required: session.mfa_required,
                expires_at: Instant::now() + state.config.session_ttl,
            });
            // Uma identidade que exige MFA reautentica o segundo factor depois de
            // mudar a palavra-passe — a sessão nova é um portão, não uma sessão
            // pronta (ADR-0107).
            let destino = if session.mfa_required {
                "/mfa"
            } else {
                "/settings/security"
            };
            (
                StatusCode::SEE_OTHER,
                [
                    (header::LOCATION, destino.to_owned()),
                    (
                        header::SET_COOKIE,
                        session::cookie_header(
                            &session_id,
                            state.config.cookie_secure,
                            state.config.session_ttl,
                        ),
                    ),
                ],
            )
                .into_response()
        }
        Err(ApiFailure::Unauthorised) => Redirect::to("/login").into_response(),
        Err(failure) => failure_response(&failure),
    }
}

/// Terminar uma sessão própria.
///
/// O identificador vem do formulário, e é o Core que resolve a posse. Se a
/// sessão terminada for a actual, o Workspace larga a sua também: dizer
/// «terminada» e continuar autenticado seria mentir sobre o efeito.
async fn revoke_session(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(session_id): Path<Uuid>,
) -> Response {
    let member = member_or_login!(state, headers);

    match api::post(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        &format!("/api/v1/auth/sessions/{session_id}/revoke"),
        &serde_json::json!({}),
    )
    .await
    {
        Ok(_) | Err(ApiFailure::Failed(_)) => {
            // O Core devolve `204`, que o cliente HTTP lê como corpo vazio.
            // Se a sessão terminada era a que sustenta este pedido, a nossa
            // deixa de valer — e é o próximo pedido que o descobriria.
            Redirect::to("/settings/security").into_response()
        }
        Err(ApiFailure::Unauthorised) => {
            if let Some(id) = session::session_id_from_cookies(
                headers
                    .get(header::COOKIE)
                    .and_then(|value| value.to_str().ok()),
            ) {
                state.sessions.remove(&id);
            }
            Redirect::to("/login").into_response()
        }
        Err(failure) => failure_response(&failure),
    }
}

/// Sem ecrã entregue pelo Design: a janela `app_pending` na casca (D001).
async fn new_unit_form(State(state): State<WorkspaceState>, headers: HeaderMap) -> Response {
    app_page(&state, &headers, Screen::Units).await
}

#[derive(Deserialize)]
struct NewUnitForm {
    name: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    research_areas: String,
}

/// Split a comma-separated research-areas field into a clean list.
fn parse_research_areas(raw: &str) -> Vec<String> {
    raw.split(',')
        .map(str::trim)
        .filter(|a| !a.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

async fn create_unit(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Form(form): Form<NewUnitForm>,
) -> Response {
    let member = member_or_login!(state, headers);

    // No `code`: the Core generates it from the name. Sending an empty one would
    // be an explicit empty code, which is a different thing.
    let body = serde_json::json!({
        "name": form.name,
        "description": blank_to_none(form.description),
        "research_areas": parse_research_areas(&form.research_areas),
    });

    match api::post(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        "/api/v1/units",
        &body,
    )
    .await
    {
        Ok(_) => Redirect::to("/units").into_response(),
        Err(ApiFailure::Unauthorised) => Redirect::to("/login").into_response(),
        Err(failure) => failure_response(&failure),
    }
}

/// `GET /units/code-suggestion?name=…` — proxy to the Core's code preview.
///
/// The browser reaches only the Workspace origin (`connect-src 'self'`), so the
/// live code preview in the «Nova Unidade» form fetches this, and the Workspace
/// asks the Core with the member's session. Returns the Core's JSON verbatim
/// (`{"code": "UCS-001"}`).
#[derive(Deserialize)]
struct CodeSuggestionQuery {
    name: String,
}

async fn unit_code_suggestion(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Query(query): Query<CodeSuggestionQuery>,
) -> Response {
    let Some(member) = current_member(&state, &headers) else {
        return StatusCode::UNAUTHORIZED.into_response();
    };

    let encoded = urlencoding_minimal(&query.name);
    match api::get::<Value>(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        &format!("/api/v1/units/code-suggestion?name={encoded}"),
    )
    .await
    {
        Ok(value) => Json(value).into_response(),
        Err(ApiFailure::Unauthorised) => StatusCode::UNAUTHORIZED.into_response(),
        Err(_) => StatusCode::BAD_GATEWAY.into_response(),
    }
}

/// Sem ecrã entregue pelo Design: a janela `app_pending` na casca (D001).
async fn edit_unit_form(State(state): State<WorkspaceState>, headers: HeaderMap) -> Response {
    app_page(&state, &headers, Screen::Units).await
}

#[derive(Deserialize)]
struct EditUnitForm {
    name: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    research_areas: String,
    // Round-trips for the read-only display only; never sent to the Core. Lets
    // an error re-render show the code without a second fetch.
}

/// `POST /units/{id}/edit` — apply the edit through the Core's `PUT /units/{id}`.
///
/// The code is not sent: it is immutable, and the Core ignores it anyway.
async fn update_unit(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(unit_id): Path<Uuid>,
    Form(form): Form<EditUnitForm>,
) -> Response {
    let member = member_or_login!(state, headers);

    let body = serde_json::json!({
        "name": form.name.clone(),
        "description": blank_to_none(form.description.clone()),
        "research_areas": parse_research_areas(&form.research_areas),
    });

    match api::put(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        &format!("/api/v1/units/{unit_id}"),
        &body,
    )
    .await
    {
        Ok(_) => Redirect::to(&format!("/units/{unit_id}")).into_response(),
        Err(ApiFailure::Unauthorised) => Redirect::to("/login").into_response(),
        Err(failure) => failure_response(&failure),
    }
}

fn blank_to_none(value: String) -> Option<String> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_owned())
}

// ── Notas pessoais ───────────────────────────────────────────────────────

/// Cria uma nota vazia e leva o membro ao editor dela.
///
/// Uma nota nova nasce com um título neutro e um documento vazio; o membro
/// dá-lhe o título e o conteúdo no editor, e o autosave grava. Assim o editor
/// tem sempre uma nota com identidade e revisão, sem um estado «por gravar pela
/// primeira vez» à parte.
async fn create_personal_note(State(state): State<WorkspaceState>, headers: HeaderMap) -> Response {
    let member = member_or_login!(state, headers);
    let body = serde_json::json!({
        "title": "Nota sem título",
        "document": { "schema_version": 1, "blocks": [{ "type": "paragraph", "content": [] }] },
    });
    match api::post(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        "/api/v1/me/notes",
        &body,
    )
    .await
    {
        Ok(created) => {
            let id = created
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or_default();
            Redirect::to(&format!("/notes/{id}")).into_response()
        }
        Err(failure) => failure_response(&failure),
    }
}

/// Sem ecrã entregue pelo Design: a janela `app_pending` na casca (D001).
async fn note_revision_preview(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
) -> Response {
    app_page(&state, &headers, Screen::Notes).await
}

/// O formulário de restauro de uma revisão.
#[derive(Deserialize)]
struct RestoreForm {
    base_revision: i64,
}

/// Restaura uma revisão e volta ao editor. O Core recusa a quem só tem leitura,
/// e responde `409` se a nota mudou entretanto — aqui volta-se ao editor, que
/// mostra a versão corrente.
async fn restore_note_revision_route(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path((note_id, revision)): Path<(Uuid, i64)>,
    Form(form): Form<RestoreForm>,
) -> Response {
    let member = member_or_login!(state, headers);
    let body = serde_json::json!({ "base_revision": form.base_revision });
    let _ = api::post(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        &format!("/api/v1/me/notes/{note_id}/revisions/{revision}/restore"),
        &body,
    )
    .await;
    Redirect::to(&format!("/notes/{note_id}")).into_response()
}

/// Apaga uma nota (leva-a ao Lixo) e volta à lista. Do dono; o Core recusa a
/// quem não o é.
async fn delete_note_route(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(note_id): Path<Uuid>,
) -> Response {
    let member = member_or_login!(state, headers);
    let _ = api::delete(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        &format!("/api/v1/me/notes/{note_id}"),
    )
    .await;
    Redirect::to("/notes").into_response()
}

/// Restaura uma nota do Lixo e volta à lista.
async fn restore_note_route(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(note_id): Path<Uuid>,
) -> Response {
    let member = member_or_login!(state, headers);
    let _ = api::post(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        &format!("/api/v1/me/notes/{note_id}/restore"),
        &serde_json::json!({}),
    )
    .await;
    Redirect::to("/notes").into_response()
}

/// Elimina definitivamente uma nota do Lixo e fica no Lixo.
async fn purge_note_route(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(note_id): Path<Uuid>,
) -> Response {
    let member = member_or_login!(state, headers);
    let _ = api::delete(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        &format!("/api/v1/me/notes/{note_id}/purge"),
    )
    .await;
    Redirect::to("/notes/lixo").into_response()
}

/// Concede acesso a uma pessoa — formulário do dono.
#[derive(Deserialize)]
struct ShareNoteForm {
    person_id: String,
    role: String,
}

/// Partilha uma nota com uma pessoa e volta ao editor. O Core recusa se quem
/// pede não for o dono, ou se o papel for desconhecido.
async fn share_note_route(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(note_id): Path<Uuid>,
    Form(form): Form<ShareNoteForm>,
) -> Response {
    let member = member_or_login!(state, headers);
    let body = serde_json::json!({ "person_id": form.person_id, "role": form.role });
    let _ = api::post(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        &format!("/api/v1/me/notes/{note_id}/shares"),
        &body,
    )
    .await;
    Redirect::to(&format!("/notes/{note_id}")).into_response()
}

/// Revoga o acesso de uma pessoa e volta ao editor.
async fn revoke_note_share_route(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path((note_id, person_id)): Path<(Uuid, Uuid)>,
) -> Response {
    let member = member_or_login!(state, headers);
    let _ = api::delete(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        &format!("/api/v1/me/notes/{note_id}/shares/{person_id}"),
    )
    .await;
    Redirect::to(&format!("/notes/{note_id}")).into_response()
}

/// Carrega uma imagem para uma nota — fetch, respondido em JSON.
///
/// Falha fechada com `401` em vez de redireccionar, como o autosave: o editor
/// carrega por `fetch`, e uma redirecção para o login viraria uma página que ele
/// leria como sucesso. O Core valida o tipo e guarda o ficheiro do membro; aqui
/// só se faz de canal, e devolve-se a versão de ficheiro que a nota vai citar.
async fn upload_personal_note_file(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    multipart: Multipart,
) -> Response {
    let member = match current_member(&state, &headers) {
        Some(member) if !member.session.must_change_password && !member.session.mfa_required => {
            member
        }
        _ => return StatusCode::UNAUTHORIZED.into_response(),
    };

    let (ficheiro, _campos) = ler_carregamento(multipart).await;
    let Some((nome, tipo, dados)) = ficheiro else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    if dados.is_empty() {
        return StatusCode::BAD_REQUEST.into_response();
    }

    match api::upload(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        "/api/v1/me/files",
        nome,
        tipo,
        dados,
    )
    .await
    {
        Ok(criado) => {
            let file_version_id = criado
                .get("file_version_id")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned();
            axum::Json(serde_json::json!({ "file_version_id": file_version_id })).into_response()
        }
        Err(ApiFailure::Unauthorised) => StatusCode::UNAUTHORIZED.into_response(),
        // A razão do Core — «só imagens PNG, JPEG ou WebP» — chega ao editor para
        // a mostrar, em vez de um erro genérico.
        Err(failure) => (StatusCode::UNPROCESSABLE_ENTITY, failure.to_string()).into_response(),
    }
}

/// Serve a imagem de uma nota, inline, na origem do Workspace.
///
/// Pela mesma razão da pré-visualização de ficheiros: a CSP continua
/// `img-src 'self'`, e a página nunca aprende onde os bytes estão. A autoridade
/// é do Core, que só a serve ao dono.
async fn preview_personal_note_file(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(version_id): Path<Uuid>,
) -> Response {
    let member = member_or_login!(state, headers);

    match api::get_inline(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        &format!("/api/v1/me/files/{version_id}/preview"),
    )
    .await
    {
        Ok((tipo, bytes)) => {
            let Ok(tipo) = HeaderValue::from_str(&tipo) else {
                return StatusCode::BAD_GATEWAY.into_response();
            };
            (
                [
                    (header::CONTENT_TYPE, tipo),
                    (
                        header::CONTENT_DISPOSITION,
                        HeaderValue::from_static("inline"),
                    ),
                    (
                        header::X_CONTENT_TYPE_OPTIONS,
                        HeaderValue::from_static("nosniff"),
                    ),
                    (
                        header::CACHE_CONTROL,
                        HeaderValue::from_static("private, max-age=0, must-revalidate"),
                    ),
                ],
                bytes,
            )
                .into_response()
        }
        Err(failure) => failure_response(&failure),
    }
}

/// Mostra um ficheiro pessoal inline (Quick Look), same-origin.
///
/// Retransmite os bytes que o Core serve — imagem ou PDF — com
/// `Content-Disposition: inline`. O PDF é desenhado dentro de uma `iframe`
/// isolada na página; aqui só se garante que os bytes chegam à origem própria.
async fn me_file_inline(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(version_id): Path<Uuid>,
) -> Response {
    let member = member_or_login!(state, headers);

    match api::get_inline(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        &format!("/api/v1/me/files/{version_id}/inline"),
    )
    .await
    {
        Ok((tipo, bytes)) => {
            let Ok(tipo) = HeaderValue::from_str(&tipo) else {
                return StatusCode::BAD_GATEWAY.into_response();
            };
            // Esta resposta é enquadrada pela própria página (a `iframe` do Quick
            // Look). Os cabeçalhos anti-enquadramento que o middleware carimba em
            // tudo — `X-Frame-Options: DENY` e `frame-ancestors 'none'` — recusam
            // esse enquadramento, e deixariam o PDF em branco. Aqui abrem-se para
            // **a própria origem, e só ela**: pré-definidos no handler, o
            // `or_insert` do middleware mantém-nos. O resto continua fechado
            // (`default-src 'none'`); `object-src 'self'` deixa o visualizador de
            // PDF do browser instanciar-se same-origin.
            (
                [
                    (header::CONTENT_TYPE, tipo),
                    (
                        header::CONTENT_DISPOSITION,
                        HeaderValue::from_static("inline"),
                    ),
                    (
                        header::X_CONTENT_TYPE_OPTIONS,
                        HeaderValue::from_static("nosniff"),
                    ),
                    (
                        header::X_FRAME_OPTIONS,
                        HeaderValue::from_static("SAMEORIGIN"),
                    ),
                    (
                        header::CONTENT_SECURITY_POLICY,
                        HeaderValue::from_static(
                            "default-src 'none'; object-src 'self'; frame-ancestors 'self'",
                        ),
                    ),
                    (
                        header::CACHE_CONTROL,
                        HeaderValue::from_static("private, max-age=0, must-revalidate"),
                    ),
                ],
                bytes,
            )
                .into_response()
        }
        Err(failure) => failure_response(&failure),
    }
}

/// Serve a miniatura de um ficheiro pessoal, same-origin, para a grelha.
///
/// Quando ainda não há miniatura pronta, o Core responde `404` (e põe a versão
/// na fila); aqui retransmite-se esse `404` para a `<img>` da ficha cair no
/// ícone do tipo, sem quebrar nada. Qualquer outra falha também vira `404`: uma
/// miniatura em falta nunca é motivo para uma página de erro.
async fn me_file_thumbnail(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(version_id): Path<Uuid>,
) -> Response {
    let member = member_or_login!(state, headers);

    match api::get_inline(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        &format!("/api/v1/me/files/{version_id}/thumbnail"),
    )
    .await
    {
        Ok((tipo, bytes)) => {
            let Ok(tipo) = HeaderValue::from_str(&tipo) else {
                return StatusCode::BAD_GATEWAY.into_response();
            };
            (
                [
                    (header::CONTENT_TYPE, tipo),
                    (
                        header::CONTENT_DISPOSITION,
                        HeaderValue::from_static("inline"),
                    ),
                    (
                        header::X_CONTENT_TYPE_OPTIONS,
                        HeaderValue::from_static("nosniff"),
                    ),
                    (
                        header::CACHE_CONTROL,
                        HeaderValue::from_static("private, max-age=0, must-revalidate"),
                    ),
                ],
                bytes,
            )
                .into_response()
        }
        Err(_) => StatusCode::NOT_FOUND.into_response(),
    }
}

/// Descarrega um ficheiro pessoal same-origin, com o nome que o Core devolve.
///
/// Substitui a ligação assinada, que apontava para o host interno do
/// armazenamento e o browser não alcançava. Aqui os bytes vêm pela origem do
/// Workspace, e o `Content-Disposition` do Core — que traz o nome higienizado —
/// é retransmitido tal e qual.
async fn me_file_raw(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(version_id): Path<Uuid>,
) -> Response {
    let member = member_or_login!(state, headers);

    match api::get_download(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        &format!("/api/v1/me/files/{version_id}/raw"),
    )
    .await
    {
        Ok((tipo, disposition, bytes)) => {
            let Ok(tipo) = HeaderValue::from_str(&tipo) else {
                return StatusCode::BAD_GATEWAY.into_response();
            };
            let disposition = disposition
                .as_deref()
                .and_then(|d| HeaderValue::from_str(d).ok())
                .unwrap_or_else(|| HeaderValue::from_static("attachment"));
            (
                [
                    (header::CONTENT_TYPE, tipo),
                    (header::CONTENT_DISPOSITION, disposition),
                    (
                        header::X_CONTENT_TYPE_OPTIONS,
                        HeaderValue::from_static("nosniff"),
                    ),
                    (
                        header::CACHE_CONTROL,
                        HeaderValue::from_static("private, max-age=0, must-revalidate"),
                    ),
                ],
                bytes,
            )
                .into_response()
        }
        Err(failure) => failure_response(&failure),
    }
}

/// Serve o texto de uma versão pessoal, same-origin, para o Quick Look.
///
/// O Core reavalia a posse e devolve `text/plain` já validado como UTF-8; aqui
/// só se retransmite, com `nosniff`, para o visualizador o mostrar escapado num
/// `<pre>` — nunca interpretado.
async fn me_file_text(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(version_id): Path<Uuid>,
) -> Response {
    let member = member_or_login!(state, headers);

    match api::get_inline(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        &format!("/api/v1/me/files/{version_id}/text"),
    )
    .await
    {
        Ok((_tipo, bytes)) => (
            [
                (
                    header::CONTENT_TYPE,
                    HeaderValue::from_static("text/plain; charset=utf-8"),
                ),
                (
                    header::X_CONTENT_TYPE_OPTIONS,
                    HeaderValue::from_static("nosniff"),
                ),
                (
                    header::CACHE_CONTROL,
                    HeaderValue::from_static("private, max-age=0, must-revalidate"),
                ),
            ],
            bytes,
        )
            .into_response(),
        Err(failure) => failure_response(&failure),
    }
}

/// O formulário de criação de uma pasta de notas.
#[derive(Deserialize)]
struct NewNoteFolderForm {
    name: String,
}

/// Cria uma pasta e leva o membro à lista recortada por ela.
async fn create_note_folder(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Form(form): Form<NewNoteFolderForm>,
) -> Response {
    let member = member_or_login!(state, headers);
    let body = serde_json::json!({ "name": form.name });
    match api::post(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        "/api/v1/me/folders",
        &body,
    )
    .await
    {
        Ok(folder) => {
            let id = folder.get("id").and_then(Value::as_str).unwrap_or_default();
            Redirect::to(&format!("/notes?folder={}", urlencoding_minimal(id))).into_response()
        }
        Err(failure) => failure_response(&failure),
    }
}

/// Apaga uma pasta e volta à lista. As notas ficam sem pasta, não se perdem.
async fn delete_note_folder(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(folder_id): Path<Uuid>,
) -> Response {
    let member = member_or_login!(state, headers);
    let _ = api::delete(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        &format!("/api/v1/me/folders/{folder_id}"),
    )
    .await;
    Redirect::to("/notes").into_response()
}

/// Move uma nota para uma pasta — fetch em JSON, do editor.
///
/// Como o autosave, falha fechada com `401` em vez de redireccionar, para o
/// editor nunca confundir uma página de login com uma mudança bem-sucedida.
async fn move_personal_note(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(note_id): Path<Uuid>,
    axum::Json(body): axum::Json<Value>,
) -> Response {
    let member = match current_member(&state, &headers) {
        Some(member) if !member.session.must_change_password && !member.session.mfa_required => {
            member
        }
        _ => return StatusCode::UNAUTHORIZED.into_response(),
    };
    match api::post(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        &format!("/api/v1/me/notes/{note_id}/folder"),
        &body,
    )
    .await
    {
        Ok(_) => StatusCode::OK.into_response(),
        Err(ApiFailure::Unauthorised) => StatusCode::UNAUTHORIZED.into_response(),
        Err(_) => StatusCode::BAD_GATEWAY.into_response(),
    }
}

// ── Autenticação ─────────────────────────────────────────────────────────

/// `?reason=` do início de sessão (D12 / D13).
#[derive(Deserialize)]
struct LoginQuery {
    #[serde(default)]
    reason: Option<String>,
}

/// `GET /login` — a porta (D001), ou o fim de sessão com `?reason=`.
///
/// Sob o ADR-0103 o Workspace apresenta o formulário e envia as credenciais ao
/// Core, que é a autoridade de autenticação. Quem já tem sessão válida não tem
/// nada a fazer à porta.
async fn login(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Query(q): Query<LoginQuery>,
) -> Response {
    if q.reason.is_none()
        && current_member(&state, &headers)
            .is_some_and(|m| !m.session.must_change_password && !m.session.mfa_required)
    {
        return Redirect::to("/").into_response();
    }
    let door = controllers::door(&state).await;
    let fim = match q.reason.as_deref() {
        Some("expired") => Some((SessionEndReason::Expired, "auth.end.expired_title")),
        // G-27: o Core ainda não diz o motivo de uma revogação.
        Some("revoked") => Some((SessionEndReason::Revoked, "auth.end.revoked_title")),
        _ => None,
    };
    match fim {
        Some((reason, titulo)) => html(
            crate::i18n::t(titulo),
            Surface::Auth,
            ui::screens::auth::login::session_end(&SessionEndVm { door, reason }),
        ),
        None => html(
            crate::i18n::t("auth.login.title"),
            Surface::Auth,
            ui::screens::auth::login::login(&LoginVm {
                door,
                error: None,
                email: String::new(),
            }),
        ),
    }
}

/// `GET /password/recover` — D10. O envio (G-26) ainda não existe no Core: a
/// vista recebe `available=false` e não submete nada.
async fn password_recover(State(state): State<WorkspaceState>) -> Response {
    html(
        crate::i18n::t("auth.recover.title"),
        Surface::Auth,
        ui::screens::auth::login::recover(&RecoverVm {
            door: controllers::door(&state).await,
            available: false,
            sent: false,
        }),
    )
}

/// A escolha de idioma à porta, antes de haver sessão (G-30).
#[derive(Deserialize)]
struct LanguageAtTheDoor {
    #[serde(default)]
    lang: String,
    #[serde(default)]
    return_to: String,
}

/// `POST /login/language` — grava o cookie de idioma e volta à porta.
///
/// Não toca em conta nenhuma (não há sessão). O destino é uma lista fechada,
/// para não ser um redireccionamento aberto.
async fn login_language(
    State(state): State<WorkspaceState>,
    Form(form): Form<LanguageAtTheDoor>,
) -> Response {
    let destino = match form.return_to.as_str() {
        "/password/recover" => "/password/recover",
        _ => "/login",
    };
    let Some(locale) = ocinye_contracts::Locale::normalize(&form.lang) else {
        return Redirect::to(destino).into_response();
    };
    let cookie = crate::session::locale_cookie_header(locale.as_str(), state.config.cookie_secure);
    ([(header::SET_COOKIE, cookie)], Redirect::to(destino)).into_response()
}

/// Credenciais submetidas pelo formulário.
#[derive(Deserialize)]
struct LoginForm {
    #[serde(default)]
    email: String,
    #[serde(default)]
    password: String,
}

/// Recebe o formulário e pede ao Core que autentique.
async fn login_submit(
    State(state): State<WorkspaceState>,
    Form(form): Form<LoginForm>,
) -> Response {
    let correlation_id = Uuid::new_v4().to_string();

    let outcome = api::post_unauthenticated(
        &state,
        &correlation_id,
        "/api/v1/auth/login",
        &serde_json::json!({
            "email": form.email,
            "password": form.password,
        }),
    )
    .await;

    let session = match outcome.and_then(CoreSession::from_payload) {
        Ok(session) => session,
        Err(failure) => {
            // Uma só mensagem para todas as falhas de credencial: o Workspace não
            // a enriquece, que reintroduziria o oráculo que o Core evita
            // (briefing §35). É a recusa do Core (`SIGN_IN_REFUSED`), na língua
            // da porta — o Core só a escreve em português.
            tracing::info!(detail = %failure, "sign-in refused");
            return (
                StatusCode::UNAUTHORIZED,
                html(
                    crate::i18n::t("auth.login.title"),
                    Surface::Auth,
                    ui::screens::auth::login::login(&LoginVm {
                        door: controllers::door(&state).await,
                        error: Some(crate::i18n::t("auth.refused.sign_in").to_owned()),
                        email: form.email,
                    }),
                ),
            )
                .into_response();
        }
    };

    let ttl = if session.must_change_password || session.mfa_required {
        // Curta, tal como a do Core: existe para completar uma tarefa — mudar a
        // palavra-passe ou satisfazer o segundo factor.
        Duration::from_secs(30 * 60)
    } else {
        state.config.session_ttl
    };

    let session_id = state.sessions.create(Session {
        access_token: session.token,
        display_name: session.display_name,
        email: form.email.clone(),
        must_change_password: session.must_change_password,
        mfa_required: session.mfa_required,
        expires_at: Instant::now() + ttl,
    });

    let destination = if session.must_change_password {
        "/first-access"
    } else if session.mfa_required {
        "/mfa"
    } else {
        "/"
    };

    (
        StatusCode::SEE_OTHER,
        [
            (header::LOCATION, destination.to_owned()),
            (
                header::SET_COOKIE,
                session::cookie_header(&session_id, state.config.cookie_secure, ttl),
            ),
        ],
    )
        .into_response()
}

/// A sessão devolvida pelo Core.
struct CoreSession {
    token: String,
    display_name: String,
    must_change_password: bool,
    /// A sessão exige um segundo factor por satisfazer (ADR-0107). O Core
    /// devolve-o no campo `state`; o Workspace usa-o para encaminhar ao fluxo de
    /// MFA e não mostrar superfície privilegiada entretanto.
    mfa_required: bool,
}

impl CoreSession {
    /// Lê a resposta do Core.
    ///
    /// Uma resposta sem token é tratada como falha e não como sessão anónima:
    /// prosseguir sem token deixaria o membro num Workspace que falha em cada
    /// chamada seguinte, sem dizer porquê.
    fn from_payload(payload: Value) -> Result<Self, ApiFailure> {
        let token = payload
            .get("session_token")
            .and_then(Value::as_str)
            .filter(|token| !token.is_empty())
            .ok_or_else(|| ApiFailure::Failed("O Ocinye Core não devolveu uma sessão.".to_owned()))?
            .to_owned();

        Ok(Self {
            token,
            display_name: payload
                .get("display_name")
                .and_then(Value::as_str)
                .unwrap_or("Membro")
                .to_owned(),
            // Derivado do **estado**, e não do booleano `must_change_password`
            // do Core: esse é `!permits_ordinary_work()`, verdadeiro também para
            // `mfa_required` — e encaminhar uma sessão de MFA para o primeiro
            // acesso mandá-la-ia definir uma palavra-passe que já tem. Cada
            // estado restrito diz o seu, e o encaminhamento distingue-os.
            must_change_password: payload.get("state").and_then(Value::as_str)
                == Some("password_change_required"),
            mfa_required: payload.get("state").and_then(Value::as_str) == Some("mfa_required"),
        })
    }
}

/// `GET /first-access` — definir a palavra-passe definitiva (D001).
async fn first_access(State(state): State<WorkspaceState>, headers: HeaderMap) -> Response {
    let Some(member) = current_member(&state, &headers) else {
        return Redirect::to("/login").into_response();
    };
    // Quem já tem palavra-passe definitiva não tem nada a fazer aqui.
    if !member.session.must_change_password {
        return Redirect::to("/").into_response();
    }
    first_access_view(&state, &member, None).await
}

/// O ecrã de primeiro acesso, com a recusa do Core quando a houve.
async fn first_access_view(
    state: &WorkspaceState,
    member: &Member,
    error: Option<String>,
) -> Response {
    // O mínimo vem do Core (`minimum_password_length`); a vista não o fixa.
    let (door, sessao) = tokio::join!(
        controllers::door(state),
        optional(state, member, "/api/v1/auth/session"),
    );
    let vm = FirstAccessVm {
        door,
        display_name: member.session.display_name.clone(),
        email: member.session.email.clone(),
        min_length: sessao
            .get("minimum_password_length")
            .and_then(Value::as_u64)
            .and_then(|n| u32::try_from(n).ok())
            .unwrap_or(FALLBACK_MIN_PASSWORD_LENGTH),
        error,
    };
    let estado = if vm.error.is_some() {
        StatusCode::UNPROCESSABLE_ENTITY
    } else {
        StatusCode::OK
    };
    (
        estado,
        html(
            crate::i18n::t("auth.first.title"),
            Surface::Auth,
            ui::screens::auth::first_access::first_access(&vm),
        ),
    )
        .into_response()
}

/// O mínimo do ADR-0104, só para quando o Core não respondeu ao pedir a sessão.
const FALLBACK_MIN_PASSWORD_LENGTH: u32 = 15;

/// Nova palavra-passe submetida.
#[derive(Deserialize)]
struct PasswordForm {
    #[serde(default)]
    password: String,
    #[serde(default)]
    confirmation: String,
}

/// Envia a nova palavra-passe ao Core e roda a sessão local.
async fn first_access_submit(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Form(form): Form<PasswordForm>,
) -> Response {
    let Some(member) = current_member(&state, &headers) else {
        return Redirect::to("/login").into_response();
    };

    let outcome = api::post(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        "/api/v1/auth/password",
        &serde_json::json!({
            "password": form.password,
            "confirmation": form.confirmation,
        }),
    )
    .await;

    let session = match outcome.and_then(CoreSession::from_payload) {
        Ok(session) => session,
        // A sessão restrita expirou a meio: recomeçar é o único caminho.
        Err(ApiFailure::Unauthorised) => return Redirect::to("/login").into_response(),
        // A recusa do Core (a política da palavra-passe), ao lado do campo.
        Err(ApiFailure::Rejected(mensagem)) => {
            return first_access_view(&state, &member, Some(mensagem)).await;
        }
        Err(failure) => return failure_response(&failure),
    };

    // O Core revogou a sessão antiga e emitiu outra. A sessão local segue-a:
    // manter a antiga deixaria o Workspace a apontar para um token morto.
    if let Some(id) = session::session_id_from_cookies(
        headers
            .get(header::COOKIE)
            .and_then(|value| value.to_str().ok()),
    ) {
        state.sessions.remove(&id);
    }

    let session_id = state.sessions.create(Session {
        access_token: session.token,
        display_name: session.display_name,
        // O nome de utilizador não muda ao trocar a palavra-passe: vem da
        // sessão anterior, porque este formulário não o pede nem o deveria
        // pedir.
        email: member.session.email.clone(),
        must_change_password: false,
        mfa_required: session.mfa_required,
        expires_at: Instant::now() + state.config.session_ttl,
    });

    // Uma identidade privilegiada que acaba de definir a palavra-passe cai
    // directamente no enrolamento do segundo factor, não numa sessão pronta.
    let destino = if session.mfa_required { "/mfa" } else { "/" };
    (
        StatusCode::SEE_OTHER,
        [
            (header::LOCATION, destino.to_owned()),
            (
                header::SET_COOKIE,
                session::cookie_header(
                    &session_id,
                    state.config.cookie_secure,
                    state.config.session_ttl,
                ),
            ),
        ],
    )
        .into_response()
}

// ── Segundo factor (ADR-0107) ───────────────────────────────────────────────

/// Um código submetido — de autenticador ou de recuperação.
#[derive(Deserialize)]
struct MfaCodeForm {
    code: String,
}

/// `?show_key=1`: a chave manual só vem a pedido (ADR-0107).
#[derive(Deserialize)]
struct MfaQuery {
    #[serde(default)]
    show_key: Option<String>,
}

/// `GET /mfa` — o ecrã certo, decidido pelo Core (`mfa_mode`), nunca por
/// heurística: configurar (D8a) ou o desafio (D8).
async fn mfa_page(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Query(query): Query<MfaQuery>,
) -> Response {
    let Some(member) = current_member(&state, &headers) else {
        return Redirect::to("/login").into_response();
    };
    if !member.session.mfa_required {
        return Redirect::to("/").into_response();
    }
    let estado = optional(&state, &member, "/api/v1/auth/mfa").await;
    match estado
        .get("mfa_mode")
        .and_then(Value::as_str)
        .unwrap_or("challenge")
    {
        "not_required" => Redirect::to("/").into_response(),
        "challenge" => mfa_challenge_view(&state, None, false).await,
        _ => {
            let mostrar = matches!(query.show_key.as_deref(), Some("1" | "true"));
            mfa_setup_view(&state, &member, mostrar, None).await
        }
    }
}

/// D8a: o QR (e, a pedido, a chave manual) do seed por confirmar que o Core
/// devolve — sempre o mesmo até ser confirmado.
async fn mfa_setup_view(
    state: &WorkspaceState,
    member: &Member,
    show_key: bool,
    error: Option<String>,
) -> Response {
    // `reveal=true`, e não `reveal=1`: a query do Core lê um `bool` canónico.
    let caminho = if show_key {
        "/api/v1/auth/mfa/enroll?reveal=true"
    } else {
        "/api/v1/auth/mfa/enroll"
    };
    let corpo = serde_json::json!({});
    let (door, inscricao) = tokio::join!(
        controllers::door(state),
        api::post(
            state,
            &member.session.access_token,
            &member.correlation_id,
            caminho,
            &corpo,
        ),
    );
    let (otpauth_uri, manual_key, error) = match inscricao {
        Ok(payload) => (
            payload
                .get("otpauth_uri")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned(),
            payload
                .get("secret_base32")
                .and_then(Value::as_str)
                .filter(|_| show_key)
                .unwrap_or_default()
                .to_owned(),
            error,
        ),
        Err(ApiFailure::Unauthorised) => return Redirect::to("/login").into_response(),
        Err(failure) => (
            String::new(),
            String::new(),
            error.or(Some(failure.to_string())),
        ),
    };
    html(
        crate::i18n::t("auth.mfa.setup_title"),
        Surface::Auth,
        ui::screens::auth::mfa::setup(&MfaSetupVm {
            door,
            otpauth_uri,
            manual_key,
            error,
        }),
    )
}

/// D8: o desafio do segundo factor, com a recusa do Core quando a houve.
async fn mfa_challenge_view(
    state: &WorkspaceState,
    error: Option<String>,
    recovery_open: bool,
) -> Response {
    let estado = if error.is_some() {
        StatusCode::UNAUTHORIZED
    } else {
        StatusCode::OK
    };
    (
        estado,
        html(
            crate::i18n::t("auth.mfa.challenge_title"),
            Surface::Auth,
            ui::screens::auth::mfa::challenge(&MfaChallengeVm {
                door: controllers::door(state).await,
                error,
                recovery_open,
            }),
        ),
    )
        .into_response()
}

/// `POST /mfa/confirm` — confirma o enrolamento e mostra os códigos de
/// recuperação (D8b), uma única vez, nesta resposta. Ainda não fecha o portão:
/// a sessão continua a exigir MFA até o acknowledgement.
async fn mfa_confirm(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Form(form): Form<MfaCodeForm>,
) -> Response {
    let Some(member) = current_member(&state, &headers) else {
        return Redirect::to("/login").into_response();
    };
    match api::post(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        "/api/v1/auth/mfa/enroll/confirm",
        &serde_json::json!({ "code": form.code }),
    )
    .await
    {
        Ok(payload) => {
            let codes = payload
                .get("recovery_codes")
                .and_then(Value::as_array)
                .map(|itens| {
                    itens
                        .iter()
                        .filter_map(Value::as_str)
                        .map(str::to_owned)
                        .collect()
                })
                .unwrap_or_default();
            let mut resposta = html(
                crate::i18n::t("auth.mfa.codes_title"),
                Surface::Auth,
                ui::screens::auth::mfa::codes(&MfaCodesVm {
                    door: controllers::door(&state).await,
                    codes,
                }),
            );
            // Os códigos são segredos de uso único: nunca em cache.
            resposta
                .headers_mut()
                .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
            resposta
        }
        Err(ApiFailure::Unauthorised) => Redirect::to("/login").into_response(),
        Err(failure) => mfa_setup_view(&state, &member, false, Some(failure.to_string())).await,
    }
}

/// Troca a sessão-portão pela sessão nova, `active` e assegurada, que o Core
/// devolveu — e leva o membro para dentro.
fn trocar_sessao(
    state: &WorkspaceState,
    headers: &HeaderMap,
    email: String,
    payload: Value,
) -> Response {
    let Ok(session) = CoreSession::from_payload(payload) else {
        return Redirect::to("/login").into_response();
    };
    if let Some(id) = session::session_id_from_cookies(
        headers
            .get(header::COOKIE)
            .and_then(|value| value.to_str().ok()),
    ) {
        state.sessions.remove(&id);
    }
    let session_id = state.sessions.create(Session {
        access_token: session.token,
        display_name: session.display_name,
        email,
        must_change_password: session.must_change_password,
        mfa_required: session.mfa_required,
        expires_at: Instant::now() + state.config.session_ttl,
    });
    (
        StatusCode::SEE_OTHER,
        [
            (header::LOCATION, "/".to_owned()),
            (
                header::SET_COOKIE,
                session::cookie_header(
                    &session_id,
                    state.config.cookie_secure,
                    state.config.session_ttl,
                ),
            ),
        ],
    )
        .into_response()
}

/// `POST /mfa/acknowledge` — a pessoa guardou os códigos; conclui o enrolamento.
async fn mfa_acknowledge(State(state): State<WorkspaceState>, headers: HeaderMap) -> Response {
    let Some(member) = current_member(&state, &headers) else {
        return Redirect::to("/login").into_response();
    };
    match api::post(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        "/api/v1/auth/mfa/acknowledge",
        &serde_json::json!({}),
    )
    .await
    {
        Ok(payload) => trocar_sessao(&state, &headers, member.session.email.clone(), payload),
        Err(ApiFailure::Unauthorised) => Redirect::to("/login").into_response(),
        // Não deve acontecer no fluxo normal; volta ao início do MFA.
        Err(_) => Redirect::to("/mfa").into_response(),
    }
}

/// `POST /mfa/challenge` — o segundo factor de um login posterior.
async fn mfa_challenge(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Form(form): Form<MfaCodeForm>,
) -> Response {
    let Some(member) = current_member(&state, &headers) else {
        return Redirect::to("/login").into_response();
    };
    match api::post(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        "/api/v1/auth/mfa/challenge",
        &serde_json::json!({ "code": form.code }),
    )
    .await
    {
        Ok(payload) => trocar_sessao(&state, &headers, member.session.email.clone(), payload),
        Err(ApiFailure::Unauthorised) => Redirect::to("/login").into_response(),
        Err(failure) => mfa_challenge_view(&state, Some(failure.to_string()), false).await,
    }
}

/// `POST /mfa/recovery` — entrar com um código de recuperação de uso único.
async fn mfa_recovery(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Form(form): Form<MfaCodeForm>,
) -> Response {
    let Some(member) = current_member(&state, &headers) else {
        return Redirect::to("/login").into_response();
    };
    match api::post(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        "/api/v1/auth/mfa/recovery",
        &serde_json::json!({ "code": form.code }),
    )
    .await
    {
        Ok(payload) => trocar_sessao(&state, &headers, member.session.email.clone(), payload),
        Err(ApiFailure::Unauthorised) => Redirect::to("/login").into_response(),
        Err(failure) => mfa_challenge_view(&state, Some(failure.to_string()), true).await,
    }
}

/// Termina a sessão.
///
/// Só por `POST`: uma sessão não deve poder ser encerrada por um `GET` que
/// alguém consiga provocar a partir de outra página.
async fn logout(State(state): State<WorkspaceState>, headers: HeaderMap) -> Response {
    let member = current_member(&state, &headers);
    let cookie = headers
        .get(header::COOKIE)
        .and_then(|value| value.to_str().ok());
    if let Some(id) = session::session_id_from_cookies(cookie) {
        state.sessions.remove(&id);
    }

    // Diz ao Core para revogar a sessão do lado dele. Se falhar, a sessão local
    // desaparece na mesma: o pior caso é uma sessão órfã que expira sozinha.
    if let Some(member) = member.as_ref() {
        let _ = api::post(
            &state,
            &member.session.access_token,
            &member.correlation_id,
            "/api/v1/auth/logout",
            &serde_json::json!({}),
        )
        .await;
    }

    let destination = "/login".to_owned();

    (
        StatusCode::SEE_OTHER,
        [
            (header::LOCATION, destination),
            (
                header::SET_COOKIE,
                session::clear_cookie_header(state.config.cookie_secure),
            ),
        ],
    )
        .into_response()
}

#[cfg(test)]
mod csrf_tests {
    use super::*;

    const PUBLIC: &str = "https://workspace.ocinye.com";

    #[test]
    fn a_sibling_subdomain_is_not_this_origin() {
        // O caso que `SameSite` não cobre: `ocinye.com` é *same-site* com
        // `workspace.ocinye.com`, por isso o cookie viajaria com o pedido.
        for hostile in [
            "https://ocinye.com",
            "https://www.ocinye.com",
            "http://workspace.ocinye.com",
            "https://workspace.ocinye.com.evil.example",
            "https://evil.example",
            "null",
            "",
        ] {
            assert!(
                !origin_is_ours(Some(hostile), Some("workspace.ocinye.com"), PUBLIC),
                "{hostile:?} foi aceite como sendo o Ocinye Workspace"
            );
        }
    }

    #[test]
    fn an_absent_origin_is_refused_on_a_write() {
        // Os browsers enviam `Origin` em todos os `POST`. A sua ausência num
        // pedido que altera estado não é o funcionamento normal de um.
        assert!(!origin_is_ours(None, Some("workspace.ocinye.com"), PUBLIC));
    }

    #[test]
    fn the_configured_public_origin_is_accepted() {
        assert!(origin_is_ours(
            Some(PUBLIC),
            Some("workspace.ocinye.com"),
            PUBLIC
        ));
        assert!(origin_is_ours(
            Some("https://workspace.ocinye.com/"),
            None,
            PUBLIC
        ));
    }

    #[test]
    fn an_origin_matching_the_request_host_is_accepted() {
        // Desenvolvimento local: o mesmo processo responde em `localhost` e em
        // `127.0.0.1`, e o `Host` é preenchido pelo browser com o alvo real.
        assert!(origin_is_ours(
            Some("http://127.0.0.1:8090"),
            Some("127.0.0.1:8090"),
            "http://localhost:8090"
        ));
        assert!(!origin_is_ours(
            Some("http://127.0.0.1:8090"),
            Some("localhost:8090"),
            "http://localhost:8090"
        ));
    }

    #[test]
    fn only_state_changing_methods_are_checked() {
        assert!(!changes_state(&axum::http::Method::GET));
        assert!(!changes_state(&axum::http::Method::HEAD));
        assert!(changes_state(&axum::http::Method::POST));
        assert!(changes_state(&axum::http::Method::DELETE));
        assert!(changes_state(&axum::http::Method::PATCH));
        assert!(changes_state(&axum::http::Method::PUT));
    }
}

#[cfg(test)]
mod router_tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Method, Request};
    use tower::ServiceExt;

    /// A origem pública desta instalação, tal como a guarda de mesma origem a
    /// compara.
    const PUBLIC: &str = "https://workspace.ocinye.com";

    /// O estado que a sonda dá ao `Router`.
    ///
    /// Nenhum pedido chega ao Core: sem cookie de sessão os handlers reencaminham
    /// para o login antes de lá tocarem. O cliente HTTP existe porque o estado o
    /// exige, e fica sem uso.
    fn state() -> WorkspaceState {
        WorkspaceState {
            config: std::sync::Arc::new(crate::config::WorkspaceConfig {
                core_transport: crate::config::CoreTransport::Roteavel,
                bind_address: "127.0.0.1:0".to_owned(),
                public_url: PUBLIC.to_owned(),
                core_url: "http://127.0.0.1:1".to_owned(),
                session_ttl: std::time::Duration::from_secs(3600),
                cookie_secure: false,
                log_level: "error".to_owned(),
                log_format: "pretty".to_owned(),
                is_production: false,
                static_dir: format!("{}/static", env!("CARGO_MANIFEST_DIR")),
            }),
            sessions: session::SessionStore::new(),
            http: reqwest::Client::new(),
        }
    }

    /// Um código que nenhum handler do Workspace devolve.
    ///
    /// É esta a peça que separa as duas perguntas. `404` responde às duas ao
    /// mesmo tempo — «esta rota não existe» e «esta rota existe e o recurso
    /// não» — e por isso não responde a nenhuma. Trocado o fallback do router
    /// por um código impossível, `418` passa a significar exactamente uma
    /// coisa: **o router não reconheceu o caminho**.
    const SENTINELA: StatusCode = StatusCode::IM_A_TEAPOT;

    /// O `Router` real da aplicação, com o fallback trocado pela sentinela.
    ///
    /// Não é uma segunda tabela de rotas nem uma reconstrução: é
    /// `routes::router`, o mesmo que o `main` monta. A única diferença é para
    /// onde vai um caminho que ele não reconhece.
    fn probe_router() -> Router {
        router(state()).fallback(|| async { SENTINELA })
    }

    /// Um UUID válido que não corresponde a nada.
    ///
    /// Serve para concretizar `{id}`. O handler há-de não encontrar o recurso —
    /// e não encontrar é a resposta certa: prova que o handler foi alcançado.
    const NADA: &str = "00000000-0000-0000-0000-000000000000";

    /// Substitui os parâmetros de um padrão por um UUID válido.
    fn concretizar(rota: &str) -> String {
        rota.split('/')
            .map(|segmento| {
                if segmento.starts_with('{') {
                    NADA
                } else {
                    segmento
                }
            })
            .collect::<Vec<_>>()
            .join("/")
    }

    /// Leva um pedido ao `Router` real e devolve o estado da resposta.
    async fn probe(method: Method, path: &str) -> StatusCode {
        let mut request = Request::builder()
            .method(method.clone())
            .uri(path)
            .header(header::HOST, "workspace.ocinye.com");

        // Um `POST` sem `Origin` é recusado pela guarda de mesma origem antes de
        // chegar à rota, e a sonda ficaria a medir a guarda em vez do router.
        if method != Method::GET {
            request = request.header(header::ORIGIN, PUBLIC);
        }

        // E o marcador de arranque, pela mesma razão: sem ele o portão
        // encaminharia para `/boot` e a sonda mediria o portão em vez das rotas
        // que existe para inventariar.
        request = request.header(header::COOKIE, "oc_boot=1");

        probe_router()
            .oneshot(request.body(Body::empty()).expect("pedido"))
            .await
            .expect("resposta")
            .status()
    }

    /// Todas as rotas do contrato existem no `Router` que a aplicação constrói.
    ///
    /// # O que esta prova mudou
    ///
    /// `ROUTES` era uma cópia à mão do router, e nada as ligava. Uma rota
    /// removida do `Router` e esquecida em `ROUTES` deixava a varredura de
    /// ligações mortas verde enquanto a ligação passava a dar 404 — a auditoria
    /// afirmava um servidor que já não existia.
    ///
    /// Agora `ROUTES` deixa de significar «acreditamos que o servidor conhece
    /// isto» e passa a significar «este é o catálogo do contrato de navegação, e
    /// cada entrada é verificada contra o `Router` real».
    #[tokio::test]
    async fn cada_rota_do_contrato_existe_no_router_real() {
        let mut ausentes: Vec<String> = Vec::new();

        for rota in ROUTES {
            let caminho = concretizar(rota);
            // O contrato é de navegação: percorre-se com `GET`. As operações
            // que só aceitam `POST` respondem `405`, que é o router a
            // reconhecer o caminho — e é isso que aqui se mede.
            let estado = probe(Method::GET, &caminho).await;
            if estado == SENTINELA {
                ausentes.push(format!("{rota} (sondado como {caminho})"));
            }
        }

        assert!(
            ausentes.is_empty(),
            "rotas no contrato que o Router real não reconhece:\n  {}",
            ausentes.join("\n  "),
        );
    }

    /// A sentinela distingue rota ausente de recurso ausente.
    ///
    /// # O controlo positivo
    ///
    /// Sem ele, o teste acima poderia estar a medir outra coisa: se tudo
    /// respondesse `418`, ou se nada respondesse, passaria à mesma.
    ///
    /// A primeira asserção é a que faz o trabalho, e não é decorativa — foi
    /// escrita depois de uma reversão *não* ter falhado. Trocada a sentinela por
    /// `404` e retirado o fallback próprio, o teste continuava verde: as sondas
    /// correm sem sessão, e uma rota conhecida reencaminha para o login antes de
    /// chegar ao seu próprio «não encontrado». As duas respostas nunca chegavam
    /// a colidir nesta fixture, e por isso a sonda não estava a provar que sabia
    /// distingui-las.
    ///
    /// Fixar `SENTINELA != NOT_FOUND` prova a propriedade directamente, e não
    /// por acaso do estado da autenticação: o que a sonda observa é o fallback
    /// que ela própria instalou, e nunca um `404` vindo de outro sítio.
    #[tokio::test]
    async fn a_sentinela_separa_rota_ausente_de_recurso_ausente() {
        assert_ne!(
            SENTINELA,
            StatusCode::NOT_FOUND,
            "a sentinela confundiu-se com o `404` que os handlers também devolvem"
        );

        // Um caminho que o router não conhece.
        assert_eq!(
            probe(Method::GET, "/unidades-antigas").await,
            SENTINELA,
            "um caminho inexistente devia cair no fallback da sonda"
        );

        // Uma rota real com um recurso que não existe: o handler é alcançado.
        let estado = probe(Method::GET, &format!("/units/{NADA}")).await;
        assert_ne!(
            estado, SENTINELA,
            "uma rota real caiu no fallback: a sonda está a medir a coisa errada"
        );

        // E o fallback verdadeiro da aplicação continua a ser o ecrã de 404,
        // que é o que um visitante vê. A sentinela existe só dentro da sonda.
        // Com o marcador de arranque: isto mede encaminhamento, e uma pessoa
        // que chega a uma rota inexistente já passou pelo arranque. Sem ele o
        // portão encaminharia para `/boot` e o teste mediria o portão.
        let aplicacao = router(state())
            .oneshot(
                Request::builder()
                    .uri("/unidades-antigas")
                    .header(header::HOST, "workspace.ocinye.com")
                    .header(header::COOKIE, "oc_boot=1")
                    .body(Body::empty())
                    .expect("pedido"),
            )
            .await
            .expect("resposta");
        assert_eq!(aplicacao.status(), StatusCode::NOT_FOUND);
    }

    /// As operações existem no método que os formulários usam.
    ///
    /// Uma acção não fica provada por existir um `GET` com o mesmo caminho.
    /// `Terminar sessão` submete `POST /logout`, e é o `POST` que tem de ser
    /// reconhecido — um `GET /logout` seria outra coisa, e uma que não
    /// queremos que exista.
    #[tokio::test]
    async fn as_operacoes_existem_no_metodo_que_os_formularios_usam() {
        for (metodo, caminho) in [
            (Method::POST, "/logout".to_owned()),
            (Method::POST, "/units/new".to_owned()),
            (Method::POST, "/ideas/new".to_owned()),
            // D005: os formulários de criação e as acções das cinco aplicações.
            (Method::POST, format!("/ideas/{NADA}/transitions")),
            (Method::POST, format!("/ideas/{NADA}/promotion")),
            (Method::POST, format!("/projects/{NADA}/transitions")),
            (Method::POST, "/knowledge/sources/new".to_owned()),
            (Method::POST, "/datasets/new".to_owned()),
            (Method::POST, format!("/datasets/{NADA}/versions")),
            (
                Method::POST,
                format!("/datasets/{NADA}/versions/{NADA}/publish"),
            ),
            (Method::POST, "/my-work/new".to_owned()),
            (Method::POST, format!("/my-work/{NADA}/transitions")),
            (Method::POST, format!("/my-work/{NADA}/assignee")),
            (Method::POST, "/settings/password".to_owned()),
            (Method::POST, format!("/settings/sessions/{NADA}/revoke")),
            (Method::POST, "/login".to_owned()),
        ] {
            let estado = probe(metodo.clone(), &caminho).await;
            assert_ne!(
                estado, SENTINELA,
                "{metodo} {caminho} não é reconhecido pelo Router real"
            );
            assert_ne!(
                estado,
                StatusCode::METHOD_NOT_ALLOWED,
                "{metodo} {caminho} existe como caminho mas não neste método"
            );
        }

        // O inverso importa tanto: encerrar uma sessão não pode ser provocável
        // por um `GET` que alguém consiga fazer o browser emitir.
        assert_eq!(
            probe(Method::GET, "/logout").await,
            StatusCode::METHOD_NOT_ALLOWED,
            "`/logout` passou a aceitar `GET`"
        );
    }
}

// ── Calendário ──────────────────────────────────────────────────────────
//
// O Workspace não decide o que é visível: pede o intervalo ao Core e desenha o
// que ele devolveu. Nenhuma das quatro vistas consulta nada por si — recebem
// todas o mesmo conjunto autorizado (ADR-0410).

/// As notificações recentes, para o painel do sino.
///
/// # Porque uma rota própria e não a página
///
/// Porque o painel abre a pedido, e a página é um histórico. Renderizar a lista
/// em cada navegação seria pedir ao Core tudo isto a cada clique, para o
/// esconder quase sempre.
async fn notifications_recent(State(state): State<WorkspaceState>, headers: HeaderMap) -> Response {
    let member = member_or_login!(state, headers);
    let resposta = optional(&state, &member, "/api/v1/notifications?page_size=12").await;
    axum::Json(resposta).into_response()
}

/// Sem ecrã entregue pelo Design: a janela `app_pending` na casca (D001).
async fn notifications_page(State(state): State<WorkspaceState>, headers: HeaderMap) -> Response {
    let title = crate::i18n::t("shell.notifications").to_owned();
    pending_page(&state, &headers, None, title, "/notifications").await
}

/// O Monitor (destino do Estado do sistema para administradores): sem ecrã
/// entregue pelo Design, a janela `app_pending`, com a visibilidade da
/// Administração.
async fn admin_monitor(State(state): State<WorkspaceState>, headers: HeaderMap) -> Response {
    app_page(&state, &headers, Screen::Admin).await
}

/// `POST /notifications/read-all` (D002 · FG-005): «Marcar todas como lidas»
/// no painel. Só as do membro — o Core põe o destinatário na condição — e
/// volta à página de onde veio, se for deste Workspace.
async fn notifications_read_all(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
) -> Response {
    let member = member_or_login!(state, headers);
    let resultado = api::post(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        "/api/v1/notifications/read-all",
        &serde_json::json!({}),
    )
    .await;
    if let Err(falha) = resultado {
        return failure_response(&falha);
    }
    Redirect::to(&voltar_a(&state, &headers)).into_response()
}

/// A página de onde o formulário veio, só se for deste Workspace; senão, o
/// Desktop. Nunca um endereço de fora (o `Referer` é do browser).
fn voltar_a(state: &WorkspaceState, headers: &HeaderMap) -> String {
    let origem = state.config.public_url.trim_end_matches('/');
    headers
        .get(header::REFERER)
        .and_then(|v| v.to_str().ok())
        .and_then(|r| r.strip_prefix(origem))
        .filter(|p| p.starts_with('/') && !p.starts_with("//"))
        .and_then(|p| crate::boot::safe_return_target(p, ROUTES))
        .unwrap_or_else(|| "/".to_owned())
}

async fn mark_notification_read(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(notification_id): Path<Uuid>,
) -> Response {
    let member = member_or_login!(state, headers);
    let _ = api::post(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        &format!("/api/v1/notifications/{notification_id}/read"),
        &serde_json::json!({}),
    )
    .await;
    Redirect::to("/notifications").into_response()
}

// ── Ficheiros institucionais ─────────────────────────────────────────────
//
// > **Uma pasta é uma estrutura de navegação dentro de um contentor de
// > autoridade.** Arrumar não classifica, e carregar não afirma conhecimento.
//
// Nada aqui decide acesso. Cada handler leva o membro ao Core e mostra o que o
// Core deixou; um identificador escrito à mão na barra de endereço chega ao
// mesmo sítio que qualquer outro, e recebe a mesma resposta.

/// O maior corpo que o Workspace aceita num ficheiro institucional.
///
/// O mesmo limite do Core, mais o envelope multipart.
const FILE_BODY_LIMIT_BYTES: usize = 640 * 1024 * 1024 + 64 * 1024;

/// Lê o ficheiro e os campos de um multipart.
async fn ler_carregamento(
    mut multipart: Multipart,
) -> (
    Option<(String, String, Vec<u8>)>,
    std::collections::HashMap<String, String>,
) {
    let mut ficheiro = None;
    let mut campos = std::collections::HashMap::new();

    while let Ok(Some(field)) = multipart.next_field().await {
        match field.name().map(ToOwned::to_owned) {
            Some(nome) if nome == "file" => {
                let filename = field.file_name().unwrap_or("ficheiro").to_owned();
                let tipo = field
                    .content_type()
                    .unwrap_or("application/octet-stream")
                    .to_owned();
                if let Ok(bytes) = field.bytes().await {
                    ficheiro = Some((filename, tipo, bytes.to_vec()));
                }
            }
            Some(nome) => {
                if let Ok(valor) = field.text().await {
                    campos.insert(nome, valor);
                }
            }
            None => {}
        }
    }

    (ficheiro, campos)
}

/// Para onde voltar depois de uma operação, com uma mensagem.
///
/// O destino vem do formulário, mas nunca se confia nele: só se aceita um
/// caminho desta aplicação. Um `return_to` para outro sítio seria uma redirecção
/// aberta oferecida a quem soubesse construir o formulário.
fn regresso(campos: &std::collections::HashMap<String, String>, sufixo: &str) -> Response {
    let destino = campos
        .get("return_to")
        .filter(|caminho| caminho.starts_with("/files") && !caminho.starts_with("//"))
        .cloned()
        .unwrap_or_else(|| "/files".to_owned());

    let junta = if destino.contains('?') { '&' } else { '?' };
    Redirect::to(&format!("{destino}{junta}{sufixo}")).into_response()
}

/// Carrega um ficheiro institucional.
/// A sessão de quem pede, ou uma recusa que o JavaScript entende.
///
/// # Porque não a macro `member_or_login!`
///
/// Porque essa devolve um redireccionamento para `/login`, e um `fetch` segue
/// redireccionamentos: o JavaScript receberia `200` com a página de entrada e
/// tentaria lê-la como JSON. `401` diz a verdade — e é a única resposta com que
/// o carregamento pode fazer alguma coisa sensata.
fn membro_ou_recusa(state: &WorkspaceState, headers: &HeaderMap) -> Option<Member> {
    current_member(state, headers).filter(|member| !member.session.must_change_password)
}

/// A recusa que o JavaScript entende.
///
/// Uma `Response` inteira no `Err` de um `Result` são mais de cem bytes
/// carregados por cada chamada que corre **bem**. O caminho feliz não deve pagar
/// o tamanho do infeliz.
fn nao_autenticado() -> Response {
    (StatusCode::UNAUTHORIZED, axum::Json(serde_json::json!({}))).into_response()
}

/// O maior corpo que uma parte pode ter, deste lado.
///
/// O mesmo limite do Core, pela mesma razão: um pedido acima disto não
/// atravessa o edge, e aceitá-lo aqui só adiaria a recusa para o salto
/// seguinte — depois de a rede já ter sido gasta.
const PARTE_MAXIMA_BYTES: usize = 40 * 1024 * 1024;

#[derive(Deserialize)]
struct AberturaDeCarregamento {
    workspace_id: Uuid,
    filename: String,
    content_type: String,
    size_bytes: i64,
    #[serde(default)]
    folder_id: Option<Uuid>,
    #[serde(default)]
    classification: Option<String>,
}

/// Abre a sessão. O browser fala com o Workspace; o Workspace fala com o Core.
///
/// # Porque isto passa por aqui e não vai directo ao Core
///
/// Porque o browser conhece um hostname — o da Experience — e a sessão dele é
/// um cookie deste lado. Mandá-lo falar com `api.ocinye.com` obrigaria a abrir
/// CORS com credenciais e a explicar-lhe uma segunda origem para a mesma
/// instituição. A fronteira mantém-se: quem fala com o Core é o servidor.
async fn upload_begin(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    axum::Json(pedido): axum::Json<AberturaDeCarregamento>,
) -> Response {
    let Some(member) = membro_ou_recusa(&state, &headers) else {
        return nao_autenticado();
    };

    let mut corpo = serde_json::json!({
        "filename": pedido.filename,
        "content_type": pedido.content_type,
        "size_bytes": pedido.size_bytes,
    });
    if let Some(folder_id) = pedido.folder_id {
        corpo["folder_id"] = serde_json::json!(folder_id);
    }
    if let Some(classification) = pedido.classification.filter(|c| !c.is_empty()) {
        corpo["classification"] = serde_json::json!(classification);
    }

    encaminhar(
        api::post(
            &state,
            &member.session.access_token,
            &member.correlation_id,
            &format!("/api/v1/workspaces/{}/uploads", pedido.workspace_id),
            &corpo,
        )
        .await,
    )
}

/// Corpo para abrir uma sessão pessoal por partes.
#[derive(Deserialize)]
struct AberturaPessoal {
    filename: String,
    content_type: String,
    size_bytes: i64,
    #[serde(default)]
    folder_id: Option<String>,
}

/// `POST /files/upload-preflight` — «cabe este ficheiro?», respondido pelo Core.
///
/// Encaminha para o preflight do Core, que decide por capacidade (quota, o que
/// está reservado, o tecto do backend). O browser não é autoridade da quota; isto
/// é a cortesia de recusar cedo, com os números.
async fn upload_preflight(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    axum::Json(pedido): axum::Json<serde_json::Value>,
) -> Response {
    let Some(member) = membro_ou_recusa(&state, &headers) else {
        return nao_autenticado();
    };
    let size = pedido.get("size_bytes").cloned().unwrap_or(Value::Null);
    encaminhar(
        api::post(
            &state,
            &member.session.access_token,
            &member.correlation_id,
            "/api/v1/me/files/uploads/preflight",
            &serde_json::json!({ "size_bytes": size }),
        )
        .await,
    )
}

#[derive(Deserialize)]
struct AppPinsForm {
    pinned: Vec<String>,
}

/// `PUT /apps/pins` — substitui a lista de aplicações fixadas na barra lateral.
///
/// Fixar, desafixar e reordenar são todos «passa a ser esta a lista». O Workspace
/// é o dono do registo, por isso valida aqui: cada id tem de ser uma aplicação
/// **real e fixável** (as estruturais, Home e O Meu Trabalho, não se fixam), e os
/// desconhecidos caem em silêncio em vez de recusarem a escrita inteira. O Core
/// guarda a lista limpa; a autorização de cada ecrã continua a ser dele.
async fn apps_set_pins(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    axum::Json(pedido): axum::Json<AppPinsForm>,
) -> Response {
    let Some(member) = membro_ou_recusa(&state, &headers) else {
        return nao_autenticado();
    };
    // Filtra pelo registo e retira repetições, mantendo a ordem do pedido.
    let mut vistos = std::collections::BTreeSet::new();
    let limpos: Vec<String> = pedido
        .pinned
        .into_iter()
        .filter(|id| experience::apps::is_pinnable(id) && vistos.insert(id.clone()))
        .collect();
    encaminhar(
        api::put(
            &state,
            &member.session.access_token,
            &member.correlation_id,
            "/api/v1/me/apps/pins",
            &serde_json::json!({ "pinned": limpos }),
        )
        .await,
    )
}

/// `POST /files/personal-upload` — abre uma sessão por partes para o espaço
/// pessoal.
///
/// O gémeo pessoal de [`upload_begin`]: sem ambiente, porque um ficheiro pessoal
/// é do próprio. As partes, o estado, o fecho e o cancelamento seguem pelas mesmas
/// rotas de sessão — o browser deixa de precisar de saber qual dos dois destinos
/// abriu a sessão assim que a tem.
async fn upload_begin_personal(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    axum::Json(pedido): axum::Json<AberturaPessoal>,
) -> Response {
    let Some(member) = membro_ou_recusa(&state, &headers) else {
        return nao_autenticado();
    };
    encaminhar(
        api::post(
            &state,
            &member.session.access_token,
            &member.correlation_id,
            "/api/v1/me/files/uploads/sessions",
            &serde_json::json!({
                "filename": pedido.filename,
                "content_type": pedido.content_type,
                "size_bytes": pedido.size_bytes,
                // A pasta em que o carregamento começou. O Core valida a pertença e
                // recusa uma vazia/mal formada; aqui só se reencaminha o que veio.
                "folder_id": pedido.folder_id
                    .as_deref()
                    .filter(|f| !f.is_empty()),
            }),
        )
        .await,
    )
}

/// O que o servidor já recebeu.
///
/// É esta rota que torna a retoma real: quem volta pergunta ao servidor o que
/// falta, em vez de confiar numa lista que só existia na página que fechou.
async fn upload_status(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(session_id): Path<Uuid>,
) -> Response {
    let Some(member) = membro_ou_recusa(&state, &headers) else {
        return nao_autenticado();
    };
    encaminhar(
        api::get::<Value>(
            &state,
            &member.session.access_token,
            &member.correlation_id,
            &format!("/api/v1/uploads/{session_id}"),
        )
        .await,
    )
}

#[derive(Deserialize)]
struct SomaDaParte {
    sha256: String,
}

async fn upload_send_part(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path((session_id, part_number)): Path<(Uuid, i32)>,
    Query(query): Query<SomaDaParte>,
    body: axum::body::Bytes,
) -> Response {
    let Some(member) = membro_ou_recusa(&state, &headers) else {
        return nao_autenticado();
    };
    encaminhar(
        api::put_bytes(
            &state,
            &member.session.access_token,
            &member.correlation_id,
            &format!(
                "/api/v1/uploads/{session_id}/parts/{part_number}?sha256={}",
                query.sha256
            ),
            body.to_vec(),
        )
        .await,
    )
}

#[derive(Deserialize)]
struct SomaFinal {
    sha256: String,
}

async fn upload_complete(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(session_id): Path<Uuid>,
    axum::Json(pedido): axum::Json<SomaFinal>,
) -> Response {
    let Some(member) = membro_ou_recusa(&state, &headers) else {
        return nao_autenticado();
    };
    encaminhar(
        api::post(
            &state,
            &member.session.access_token,
            &member.correlation_id,
            &format!("/api/v1/uploads/{session_id}/complete"),
            &serde_json::json!({ "sha256": pedido.sha256 }),
        )
        .await,
    )
}

async fn upload_cancel(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(session_id): Path<Uuid>,
) -> Response {
    let Some(member) = membro_ou_recusa(&state, &headers) else {
        return nao_autenticado();
    };
    encaminhar(
        api::delete(
            &state,
            &member.session.access_token,
            &member.correlation_id,
            &format!("/api/v1/uploads/{session_id}"),
        )
        .await,
    )
}

/// Traduz a resposta do Core em JSON para o browser.
///
/// O código de estado vem do Core. Achatar tudo em `200` com um campo `erro`
/// obrigaria o JavaScript a inventar a sua própria noção de falha, e a recusa
/// institucional deixaria de ser legível pelas ferramentas do browser.
fn encaminhar(resultado: Result<Value, ApiFailure>) -> Response {
    match resultado {
        Ok(valor) => (StatusCode::OK, axum::Json(valor)).into_response(),
        Err(ApiFailure::Unauthorised) => {
            (StatusCode::UNAUTHORIZED, axum::Json(serde_json::json!({}))).into_response()
        }
        Err(ApiFailure::Forbidden) | Err(ApiFailure::Denied) => (
            StatusCode::FORBIDDEN,
            axum::Json(serde_json::json!({ "recusado": true })),
        )
            .into_response(),
        // A recusa pelo conteúdo — 422 — traz a mensagem escrita para o membro,
        // e é por aqui que o código tipado de capacidade (`STORAGE_*`) chega ao
        // browser. Achatá-la num 502 genérico perderia o motivo, e o carregador
        // não teria como dizer «sem espaço» em vez de «falhou».
        Err(ApiFailure::Rejected(mensagem)) => (
            StatusCode::UNPROCESSABLE_ENTITY,
            axum::Json(serde_json::json!({ "message": mensagem })),
        )
            .into_response(),
        Err(falha) => (
            StatusCode::BAD_GATEWAY,
            axum::Json(serde_json::json!({ "erro": format!("{falha:?}") })),
        )
            .into_response(),
    }
}

/// O pedido pede a resposta em JSON (o carregador com barra de progresso), e não
/// uma navegação de página inteira?
///
/// O carregador do browser envia cada ficheiro por `XMLHttpRequest` com
/// `Accept: application/json` para poder ler o progresso e o resultado de cada
/// um. Sem JavaScript, o mesmo formulário submete-se por inteiro e volta com um
/// redireccionamento — e é essa a razão de negociar aqui, e não impor um só
/// comportamento aos dois caminhos.
fn aceita_json(headers: &HeaderMap) -> bool {
    headers
        .get(header::ACCEPT)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.contains("application/json"))
}

async fn files_upload(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    multipart: Multipart,
) -> Response {
    let quer_json = aceita_json(&headers);
    // Um `fetch`/XHR segue redireccionamentos: mandá-lo para `/login` dar-lhe-ia
    // `200` com a página de entrada, que ele tentaria ler como JSON. `401` diz a
    // verdade. Sem JS, o redireccionamento é o comportamento certo.
    let member = match membro_ou_recusa(&state, &headers) {
        Some(member) => member,
        None if quer_json => return nao_autenticado(),
        None => return Redirect::to("/login").into_response(),
    };
    let (ficheiro, campos) = ler_carregamento(multipart).await;

    let Some((nome, tipo, dados)) = ficheiro else {
        return recusa_de_carregamento(quer_json, &campos, "vazio", StatusCode::BAD_REQUEST);
    };
    if dados.is_empty() {
        return recusa_de_carregamento(quer_json, &campos, "vazio", StatusCode::BAD_REQUEST);
    }
    // O nome viaja para a resposta em JSON, antes de `nome` ser consumido no envio.
    let nome_resposta = nome.clone();

    // Sem ambiente indicado, o destino é o espaço pessoal: «Meus ficheiros»
    // existe para todo o membro activo, sem exigir ambiente nenhum. Com
    // ambiente, é um carregamento institucional, autorizado pelo Core.
    let resultado = match campos.get("workspace_id").filter(|w| !w.is_empty()) {
        None => {
            api::upload_with_fields(
                &state,
                &member.session.access_token,
                &member.correlation_id,
                "/api/v1/me/files/uploads",
                nome,
                tipo,
                dados,
                // A pasta pessoal de destino viaja com o ficheiro. Sem ela, é a
                // raiz; com ela, o Core coloca-o lá — que é o que faltava para um
                // carregamento dentro de um Dossier não cair fora dele.
                vec![(
                    "folder_id",
                    campos.get("folder_id").cloned().unwrap_or_default(),
                )],
            )
            .await
        }
        Some(workspace_id) => {
            api::upload_with_fields(
                &state,
                &member.session.access_token,
                &member.correlation_id,
                &format!("/api/v1/workspaces/{workspace_id}/files"),
                nome,
                tipo,
                dados,
                vec![
                    (
                        "classification",
                        campos.get("classification").cloned().unwrap_or_default(),
                    ),
                    (
                        "folder_id",
                        campos.get("folder_id").cloned().unwrap_or_default(),
                    ),
                ],
            )
            .await
        }
    };

    match resultado {
        Ok(_) if quer_json => (
            StatusCode::OK,
            axum::Json(serde_json::json!({ "ok": true, "filename": nome_resposta })),
        )
            .into_response(),
        Ok(_) => regresso(&campos, "ok=carregado"),
        Err(ApiFailure::Unauthorised) if quer_json => nao_autenticado(),
        Err(ApiFailure::Unauthorised) => Redirect::to("/login").into_response(),
        Err(ApiFailure::Unavailable(_)) => recusa_de_carregamento(
            quer_json,
            &campos,
            "armazenamento",
            StatusCode::INSUFFICIENT_STORAGE,
        ),
        Err(_) => recusa_de_carregamento(quer_json, &campos, "recusado", StatusCode::BAD_GATEWAY),
    }
}

/// A recusa de um carregamento, na língua que o pedido entende.
///
/// Com JavaScript, um estado HTTP verdadeiro e um `erro` legível por ficheiro;
/// sem ele, o redireccionamento de sempre com a mensagem no sufixo. O mesmo
/// motivo em ambos, dito de duas maneiras.
fn recusa_de_carregamento(
    quer_json: bool,
    campos: &std::collections::HashMap<String, String>,
    erro: &str,
    estado: StatusCode,
) -> Response {
    if quer_json {
        (estado, axum::Json(serde_json::json!({ "erro": erro }))).into_response()
    } else {
        regresso(campos, &format!("erro={erro}"))
    }
}

// ── Meus ficheiros: arrumar (pastas, mudar nome, mover) ──────────────────
//
// Formulários que submetem para o BFF, que fala com o Core por JSON. O destino
// de regresso viaja no próprio formulário, e é sempre uma rota de Ficheiros.

/// Um destino de regresso seguro: só rotas de Ficheiros, nunca uma URL externa.
fn regresso_ficheiros(destino: &str, sufixo: &str) -> Response {
    let base = if destino.starts_with("/files") {
        destino
    } else {
        "/files"
    };
    let juncao = if base.contains('?') { '&' } else { '?' };
    Redirect::to(&format!("{base}{juncao}{sufixo}")).into_response()
}

#[derive(Deserialize)]
struct MeFolderNewForm {
    name: String,
}

async fn me_folder_new(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Form(form): Form<MeFolderNewForm>,
) -> Response {
    let member = member_or_login!(state, headers);
    if form.name.trim().is_empty() {
        return regresso_ficheiros("/files", "erro=nome");
    }
    let corpo = serde_json::json!({ "name": form.name.trim() });
    match api::post(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        "/api/v1/me/folders",
        &corpo,
    )
    .await
    {
        Ok(_) => regresso_ficheiros("/files", "ok=pasta"),
        Err(ApiFailure::Unauthorised) => Redirect::to("/login").into_response(),
        Err(_) => regresso_ficheiros("/files", "erro=nome"),
    }
}

#[derive(Deserialize)]
struct MeFolderRenameForm {
    folder_id: Uuid,
    name: String,
}

async fn me_folder_rename(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Form(form): Form<MeFolderRenameForm>,
) -> Response {
    let member = member_or_login!(state, headers);
    let destino = format!("/files?folder={}", form.folder_id);
    if form.name.trim().is_empty() {
        return regresso_ficheiros(&destino, "erro=nome");
    }
    let corpo = serde_json::json!({ "folder_id": form.folder_id, "name": form.name.trim() });
    match api::post(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        "/api/v1/me/folders/rename",
        &corpo,
    )
    .await
    {
        Ok(_) => regresso_ficheiros(&destino, "ok=pasta"),
        Err(ApiFailure::Unauthorised) => Redirect::to("/login").into_response(),
        Err(_) => regresso_ficheiros(&destino, "erro=nome"),
    }
}

#[derive(Deserialize)]
struct MeFolderDeleteForm {
    folder_id: Uuid,
}

async fn me_folder_delete(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Form(form): Form<MeFolderDeleteForm>,
) -> Response {
    let member = member_or_login!(state, headers);
    // Apagar uma pasta pessoal é a rota que já existe (servia as notas):
    // `DELETE /me/folders/{id}`. O serviço desprende os ficheiros para a raiz.
    match api::delete(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        &format!("/api/v1/me/folders/{}", form.folder_id),
    )
    .await
    {
        Ok(_) => regresso_ficheiros("/files", "ok=pasta-apagada"),
        Err(ApiFailure::Unauthorised) => Redirect::to("/login").into_response(),
        Err(_) => regresso_ficheiros("/files", "erro=recusado"),
    }
}

#[derive(Deserialize)]
struct MeFileRenameForm {
    file_id: Uuid,
    name: String,
    #[serde(default)]
    return_to: String,
}

async fn me_file_rename(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Form(form): Form<MeFileRenameForm>,
) -> Response {
    let member = member_or_login!(state, headers);
    let destino = if form.return_to.is_empty() {
        "/files".to_owned()
    } else {
        form.return_to.clone()
    };
    if form.name.trim().is_empty() {
        return regresso_ficheiros(&destino, "erro=nome");
    }
    let corpo = serde_json::json!({ "file_id": form.file_id, "name": form.name.trim() });
    match api::post(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        "/api/v1/me/files/rename",
        &corpo,
    )
    .await
    {
        Ok(_) => regresso_ficheiros(&destino, "ok=renomeado"),
        Err(ApiFailure::Unauthorised) => Redirect::to("/login").into_response(),
        Err(_) => regresso_ficheiros(&destino, "erro=nome"),
    }
}

#[derive(Deserialize)]
struct MeFileMoveForm {
    file_id: Uuid,
    /// A pasta de destino; vazia move para a raiz.
    #[serde(default)]
    folder_id: String,
    #[serde(default)]
    return_to: String,
}

async fn me_file_move(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Form(form): Form<MeFileMoveForm>,
) -> Response {
    let member = member_or_login!(state, headers);
    let destino = if form.return_to.is_empty() {
        "/files".to_owned()
    } else {
        form.return_to.clone()
    };
    // Uma pasta vazia é a raiz: viaja como `null`, nunca como `""`.
    let folder = Uuid::parse_str(form.folder_id.trim()).ok();
    let corpo = serde_json::json!({ "file_id": form.file_id, "folder_id": folder });
    match api::post(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        "/api/v1/me/files/move",
        &corpo,
    )
    .await
    {
        Ok(_) => regresso_ficheiros(&destino, "ok=movido"),
        Err(ApiFailure::Unauthorised) => Redirect::to("/login").into_response(),
        Err(_) => regresso_ficheiros(&destino, "erro=recusado"),
    }
}

/// Uma lista de identificadores separada por vírgulas, como a selecção a envia.
///
/// Cada um é validado ao ser parseado; o que não for um `Uuid` cai fora em
/// silêncio, porque veio do cliente e não é autoridade. O Core reautoriza cada
/// ficheiro à porta, um a um.
fn ids_da_seleccao(bruto: &str) -> Vec<Uuid> {
    bruto
        .split(',')
        .filter_map(|parte| Uuid::parse_str(parte.trim()).ok())
        .collect()
}

#[derive(Deserialize)]
struct MeFilesBatchMoveForm {
    #[serde(default)]
    file_ids: String,
    #[serde(default)]
    folder_id: String,
    #[serde(default)]
    return_to: String,
}

/// Move vários ficheiros pessoais de uma vez para uma pasta (ou para a raiz).
///
/// Reutiliza a operação de mover um: cada ficheiro é reautorizado pela posse no
/// Core. Um que recuse não pára os outros; o regresso diz quantos moveram.
async fn me_files_batch_move(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Form(form): Form<MeFilesBatchMoveForm>,
) -> Response {
    let member = member_or_login!(state, headers);
    let destino = if form.return_to.is_empty() {
        "/files".to_owned()
    } else {
        form.return_to.clone()
    };
    let folder = Uuid::parse_str(form.folder_id.trim()).ok();
    let ids = ids_da_seleccao(&form.file_ids);
    if ids.is_empty() {
        return regresso_ficheiros(&destino, "erro=vazio");
    }

    let mut movidos = 0_usize;
    for file_id in ids {
        let corpo = serde_json::json!({ "file_id": file_id, "folder_id": folder });
        match api::post(
            &state,
            &member.session.access_token,
            &member.correlation_id,
            "/api/v1/me/files/move",
            &corpo,
        )
        .await
        {
            Ok(_) => movidos += 1,
            Err(ApiFailure::Unauthorised) => return Redirect::to("/login").into_response(),
            Err(_) => {}
        }
    }
    regresso_ficheiros(&destino, &format!("ok=movidos&n={movidos}"))
}

#[derive(Deserialize)]
struct MeFilesBatchDeleteForm {
    #[serde(default)]
    file_ids: String,
    #[serde(default)]
    return_to: String,
}

/// Põe vários ficheiros pessoais no Lixo de uma vez.
///
/// Reutiliza o apagar de um (que é reversível, vai para o Lixo); cada ficheiro é
/// reautorizado pela posse no Core.
async fn me_files_batch_delete(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Form(form): Form<MeFilesBatchDeleteForm>,
) -> Response {
    let member = member_or_login!(state, headers);
    let destino = if form.return_to.is_empty() {
        "/files".to_owned()
    } else {
        form.return_to.clone()
    };
    let ids = ids_da_seleccao(&form.file_ids);
    if ids.is_empty() {
        return regresso_ficheiros(&destino, "erro=vazio");
    }

    let mut apagados = 0_usize;
    for file_id in ids {
        let corpo = serde_json::json!({ "file_id": file_id });
        match api::post(
            &state,
            &member.session.access_token,
            &member.correlation_id,
            "/api/v1/me/files/delete",
            &corpo,
        )
        .await
        {
            Ok(_) => apagados += 1,
            Err(ApiFailure::Unauthorised) => return Redirect::to("/login").into_response(),
            Err(_) => {}
        }
    }
    regresso_ficheiros(&destino, &format!("ok=lixo&n={apagados}"))
}

#[derive(Deserialize)]
struct MeFileFavouriteForm {
    file_id: Uuid,
    #[serde(default)]
    return_to: String,
}

/// Alterna a marca de favorito de um ficheiro pessoal e volta à lista.
async fn me_file_favourite(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Form(form): Form<MeFileFavouriteForm>,
) -> Response {
    let member = member_or_login!(state, headers);
    let destino = if form.return_to.is_empty() {
        "/files".to_owned()
    } else {
        form.return_to.clone()
    };
    let corpo = serde_json::json!({ "file_id": form.file_id });
    match api::post(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        "/api/v1/me/files/favourite",
        &corpo,
    )
    .await
    {
        Ok(_) => regresso_ficheiros(&destino, "ok=favorito"),
        Err(ApiFailure::Unauthorised) => Redirect::to("/login").into_response(),
        Err(_) => regresso_ficheiros(&destino, "erro=recusado"),
    }
}

#[derive(Deserialize)]
struct MeFileActionForm {
    file_id: Uuid,
    #[serde(default)]
    return_to: String,
}

/// Uma acção sobre um ficheiro pessoal — apagar (para o Lixo), restaurar ou
/// apagar definitivamente. Todas seguem a mesma forma.
async fn me_file_action(
    state: &WorkspaceState,
    member: &Member,
    caminho: &str,
    file_id: Uuid,
    destino: &str,
    ok: &'static str,
) -> Response {
    let corpo = serde_json::json!({ "file_id": file_id });
    match api::post(
        state,
        &member.session.access_token,
        &member.correlation_id,
        caminho,
        &corpo,
    )
    .await
    {
        Ok(_) => regresso_ficheiros(destino, ok),
        Err(ApiFailure::Unauthorised) => Redirect::to("/login").into_response(),
        Err(_) => regresso_ficheiros(destino, "erro=recusado"),
    }
}

async fn me_file_delete(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Form(form): Form<MeFileActionForm>,
) -> Response {
    let member = member_or_login!(state, headers);
    let destino = if form.return_to.is_empty() {
        "/files".to_owned()
    } else {
        form.return_to
    };
    me_file_action(
        &state,
        &member,
        "/api/v1/me/files/delete",
        form.file_id,
        &destino,
        "ok=lixo",
    )
    .await
}

async fn me_file_restore(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Form(form): Form<MeFileActionForm>,
) -> Response {
    let member = member_or_login!(state, headers);
    me_file_action(
        &state,
        &member,
        "/api/v1/me/files/restore",
        form.file_id,
        "/files?trash=1",
        "ok=restaurado",
    )
    .await
}

async fn me_file_purge(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Form(form): Form<MeFileActionForm>,
) -> Response {
    let member = member_or_login!(state, headers);
    me_file_action(
        &state,
        &member,
        "/api/v1/me/files/purge",
        form.file_id,
        "/files?trash=1",
        "ok=apagado",
    )
    .await
}

/// `POST /files/trash/empty` — esvazia o Lixo pessoal de uma vez.
///
/// Um só pedido ao Core, que apaga tudo o que é do próprio e devolve quantos. A
/// confirmação vive no ecrã (um passo deliberado antes deste botão); a autoridade
/// é reavaliada no Core, ficheiro a ficheiro.
async fn me_files_purge_all(State(state): State<WorkspaceState>, headers: HeaderMap) -> Response {
    let member = member_or_login!(state, headers);
    match api::post(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        "/api/v1/me/files/purge-all",
        &serde_json::json!({}),
    )
    .await
    {
        Ok(_) => regresso_ficheiros("/files?trash=1", "ok=lixo_vazio"),
        Err(ApiFailure::Unauthorised) => Redirect::to("/login").into_response(),
        Err(_) => regresso_ficheiros("/files?trash=1", "erro=recusado"),
    }
}

/// Sem ecrã entregue pelo Design: a janela `app_pending` na casca (D001).
async fn file_detail(State(state): State<WorkspaceState>, headers: HeaderMap) -> Response {
    app_page(&state, &headers, Screen::Files).await
}

/// Carrega uma versão nova de um ficheiro que já existe.
async fn file_new_version(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(file_id): Path<Uuid>,
    multipart: Multipart,
) -> Response {
    let member = member_or_login!(state, headers);
    let (ficheiro, _) = ler_carregamento(multipart).await;

    let destino = format!("/files/{file_id}");
    let Some((nome, tipo, dados)) = ficheiro else {
        return Redirect::to(&format!("{destino}?erro=vazio")).into_response();
    };
    if dados.is_empty() {
        return Redirect::to(&format!("{destino}?erro=vazio")).into_response();
    }

    let resultado = api::upload_with_fields(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        &format!("/api/v1/files/{file_id}/versions"),
        nome,
        tipo,
        dados,
        vec![],
    )
    .await;

    match resultado {
        Ok(_) => Redirect::to(&format!("{destino}?ok=versao")).into_response(),
        Err(ApiFailure::Unauthorised) => Redirect::to("/login").into_response(),
        Err(ApiFailure::Unavailable(_)) => {
            Redirect::to(&format!("{destino}?erro=armazenamento")).into_response()
        }
        Err(_) => Redirect::to(&format!("{destino}?erro=recusado")).into_response(),
    }
}

/// Serve a pré-visualização na origem do Workspace.
///
/// # Porque os bytes passam por aqui
///
/// Porque a alternativa era pôr a URL do armazenamento num `<img>` e alargar a
/// `Content-Security-Policy` desta aplicação — hoje `img-src 'self' data:` — ao
/// host do object storage, que é configurável e pode ser externo.
///
/// > **A Experience não precisa de conhecer nem confiar no endpoint físico onde
/// > os bytes institucionais estão guardados.**
///
/// O tipo é o que o Core declarou, contra a lista fechada dele. Este handler
/// não o adivinha nem o corrige: repeti-lo aqui seria uma segunda opinião sobre
/// uma decisão que já foi tomada no sítio certo.
async fn file_preview(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(file_id): Path<Uuid>,
) -> Response {
    let member = member_or_login!(state, headers);

    match api::get_inline(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        &format!("/api/v1/files/{file_id}/preview"),
    )
    .await
    {
        Ok((tipo, bytes)) => {
            let Ok(tipo) = HeaderValue::from_str(&tipo) else {
                return StatusCode::BAD_GATEWAY.into_response();
            };
            (
                [
                    (header::CONTENT_TYPE, tipo),
                    (
                        header::CONTENT_DISPOSITION,
                        HeaderValue::from_static("inline"),
                    ),
                    (
                        header::X_CONTENT_TYPE_OPTIONS,
                        HeaderValue::from_static("nosniff"),
                    ),
                    (
                        header::CACHE_CONTROL,
                        HeaderValue::from_static("private, max-age=0, must-revalidate"),
                    ),
                ],
                bytes,
            )
                .into_response()
        }
        Err(failure) => failure_response(&failure),
    }
}

/// Serve inline a pré-visualização de uma versão exacta.
///
/// Pela mesma razão da outra: a CSP continua `img-src 'self'`, e a página nunca
/// aprende onde os bytes estão.
async fn file_version_preview(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(version_id): Path<Uuid>,
) -> Response {
    let member = member_or_login!(state, headers);

    match api::get_inline(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        &format!("/api/v1/file-versions/{version_id}/preview"),
    )
    .await
    {
        Ok((tipo, bytes)) => {
            let Ok(tipo) = HeaderValue::from_str(&tipo) else {
                return StatusCode::BAD_GATEWAY.into_response();
            };
            (
                [
                    (header::CONTENT_TYPE, tipo),
                    (
                        header::CONTENT_DISPOSITION,
                        HeaderValue::from_static("inline"),
                    ),
                    (
                        header::X_CONTENT_TYPE_OPTIONS,
                        HeaderValue::from_static("nosniff"),
                    ),
                    (
                        header::CACHE_CONTROL,
                        HeaderValue::from_static("private, max-age=0, must-revalidate"),
                    ),
                ],
                bytes,
            )
                .into_response()
        }
        Err(failure) => failure_response(&failure),
    }
}

/// Descarrega a versão corrente, same-origin.
///
/// Antes redireccionava para uma ligação assinada; essa ligação aponta para o
/// host interno do armazenamento (`object-store:9000`), que o browser não
/// alcança, e por isso a descarga institucional estava partida. Agora os bytes
/// saem pela origem do Workspace, como a pré-visualização e a descarga pessoal
/// já saíam (ADR-0608).
async fn file_download(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(file_id): Path<Uuid>,
) -> Response {
    let member = member_or_login!(state, headers);
    descarga_same_origin(&state, &member, &format!("/api/v1/files/{file_id}/raw")).await
}

/// Descarrega uma versão exacta, same-origin (ADR-0608).
async fn version_download(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(version_id): Path<Uuid>,
) -> Response {
    let member = member_or_login!(state, headers);
    descarga_same_origin(
        &state,
        &member,
        &format!("/api/v1/file-versions/{version_id}/raw"),
    )
    .await
}

/// Descarrega um ficheiro pessoal pela sua versão corrente, same-origin.
///
/// A autoridade é a posse, reavaliada no Core: uma versão que não seja do dono
/// responde «não encontrado». Serve os bytes pela origem do Workspace (ADR-0608),
/// como [`me_file_raw`] — a experiência liga ao `/raw`; esta rota é o caminho
/// equivalente para a versão corrente.
async fn version_download_personal(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(version_id): Path<Uuid>,
) -> Response {
    let member = member_or_login!(state, headers);
    descarga_same_origin(
        &state,
        &member,
        &format!("/api/v1/me/files/{version_id}/raw"),
    )
    .await
}

/// Pede os bytes ao Core e serve-os same-origin, com o `attachment` que o Core
/// já compôs.
///
/// Substitui o redireccionamento para uma ligação assinada: o armazenamento não
/// tem endpoint público, pelo que essa ligação apontava para um host que o
/// browser não alcança (ADR-0608). Os bytes saem pela origem do Workspace, e a
/// localização do armazenamento nunca chega à página — como uma URL assinada num
/// `href` também nunca chegava, e pela mesma razão de privacidade.
async fn descarga_same_origin(state: &WorkspaceState, member: &Member, caminho: &str) -> Response {
    match api::get_download(
        state,
        &member.session.access_token,
        &member.correlation_id,
        caminho,
    )
    .await
    {
        Ok((tipo, disposition, bytes)) => {
            let Ok(tipo) = HeaderValue::from_str(&tipo) else {
                return StatusCode::BAD_GATEWAY.into_response();
            };
            let disposition = disposition
                .as_deref()
                .and_then(|d| HeaderValue::from_str(d).ok())
                .unwrap_or_else(|| HeaderValue::from_static("attachment"));
            (
                [
                    (header::CONTENT_TYPE, tipo),
                    (header::CONTENT_DISPOSITION, disposition),
                    (
                        header::X_CONTENT_TYPE_OPTIONS,
                        HeaderValue::from_static("nosniff"),
                    ),
                    (
                        header::CACHE_CONTROL,
                        HeaderValue::from_static("private, max-age=0, must-revalidate"),
                    ),
                ],
                bytes,
            )
                .into_response()
        }
        Err(failure) => failure_response(&failure),
    }
}

// ── Gestão de pertenças ─────────────────────────────────────────────────
//
// Uma pertença **é** autoridade. Acrescentar alguém a uma unidade concede-lhe
// direitos sobre o que lá está; retirá-lo tira-lhos. Nada aqui decide: cada
// operação leva o membro ao Core, que volta a autorizar contra o contentor
// concreto — e recusa a quem tente por HTTP directo o que a interface não lhe
// ofereceu.

#[derive(Deserialize)]
struct MembroDaUnidade {
    person_id: Uuid,
    #[serde(default)]
    role: String,
}

fn de_volta_ao_ambiente(workspace_id: Uuid, sufixo: &str) -> Response {
    Redirect::to(&format!("/workspaces/{workspace_id}?{sufixo}")).into_response()
}

fn de_volta_a_unidade(unit_id: Uuid, sufixo: &str) -> Response {
    Redirect::to(&format!("/units/{unit_id}?{sufixo}")).into_response()
}

/// Traduz a recusa do Core no motivo que a interface mostra.
fn motivo_da_recusa(failure: &ApiFailure) -> &'static str {
    match failure {
        ApiFailure::Forbidden | ApiFailure::Denied => "autoridade",
        // O Core devolve conflito quando a operação deixaria a unidade sem
        // ninguém que a governe.
        ApiFailure::Failed(mensagem) if mensagem.contains("409") => "ultimo",
        _ => "recusado",
    }
}

/// Acrescentar alguém ao Research Workspace.
///
/// A operação vai ao Core pelo mesmo caminho que o ecrã usou para decidir se
/// mostrava o formulário — e o Core decide outra vez. A ausência do controlo
/// nunca foi a defesa.
async fn workspace_member_add(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(workspace_id): Path<Uuid>,
    Form(form): Form<MembroDaUnidade>,
) -> Response {
    let member = member_or_login!(state, headers);

    let resultado = api::post(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        &format!("/api/v1/workspaces/{workspace_id}/members"),
        &serde_json::json!({
            "person_id": form.person_id,
            "role": if form.role.is_empty() { "member" } else { &form.role },
        }),
    )
    .await;

    match resultado {
        Ok(_) => de_volta_ao_ambiente(workspace_id, "ok=adicionado"),
        Err(ApiFailure::Unauthorised) => Redirect::to("/login").into_response(),
        Err(falha) => {
            de_volta_ao_ambiente(workspace_id, &format!("erro={}", motivo_da_recusa(&falha)))
        }
    }
}

async fn workspace_member_remove(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(workspace_id): Path<Uuid>,
    Form(form): Form<MembroDaUnidade>,
) -> Response {
    let member = member_or_login!(state, headers);
    let person_id = form.person_id;

    let resultado = api::post(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        &format!("/api/v1/workspaces/{workspace_id}/members/{person_id}"),
        &serde_json::json!({}),
    )
    .await;

    match resultado {
        Ok(_) => de_volta_ao_ambiente(workspace_id, "ok=removido"),
        Err(ApiFailure::Unauthorised) => Redirect::to("/login").into_response(),
        Err(falha) => {
            de_volta_ao_ambiente(workspace_id, &format!("erro={}", motivo_da_recusa(&falha)))
        }
    }
}

async fn unit_member_add(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(unit_id): Path<Uuid>,
    Form(form): Form<MembroDaUnidade>,
) -> Response {
    let member = member_or_login!(state, headers);

    let resultado = api::post(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        &format!("/api/v1/units/{unit_id}/members"),
        &serde_json::json!({
            "person_id": form.person_id,
            "role": if form.role.is_empty() { "member" } else { &form.role },
        }),
    )
    .await;

    match resultado {
        Ok(_) => de_volta_a_unidade(unit_id, "ok=adicionado"),
        Err(ApiFailure::Unauthorised) => Redirect::to("/login").into_response(),
        Err(falha) => de_volta_a_unidade(unit_id, &format!("erro={}", motivo_da_recusa(&falha))),
    }
}

/// Alterar o papel é a mesma operação que acrescentar: o Core faz upsert.
///
/// Não há aqui um caminho de escrita paralelo — seria uma segunda autoridade
/// com outro nome.
async fn unit_member_role(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(unit_id): Path<Uuid>,
    Form(form): Form<MembroDaUnidade>,
) -> Response {
    let member = member_or_login!(state, headers);

    let resultado = api::post(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        &format!("/api/v1/units/{unit_id}/members"),
        &serde_json::json!({ "person_id": form.person_id, "role": form.role }),
    )
    .await;

    match resultado {
        Ok(_) => de_volta_a_unidade(unit_id, "ok=papel"),
        Err(ApiFailure::Unauthorised) => Redirect::to("/login").into_response(),
        Err(falha) => de_volta_a_unidade(unit_id, &format!("erro={}", motivo_da_recusa(&falha))),
    }
}

async fn unit_member_remove(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(unit_id): Path<Uuid>,
    Form(form): Form<MembroDaUnidade>,
) -> Response {
    let member = member_or_login!(state, headers);
    let person_id = form.person_id;

    let resultado = api::post(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        &format!("/api/v1/units/{unit_id}/members/{person_id}"),
        &serde_json::json!({}),
    )
    .await;

    match resultado {
        Ok(_) => de_volta_a_unidade(unit_id, "ok=removido"),
        Err(ApiFailure::Unauthorised) => Redirect::to("/login").into_response(),
        Err(falha) => de_volta_a_unidade(unit_id, &format!("erro={}", motivo_da_recusa(&falha))),
    }
}

#[cfg(test)]
mod corpo_de_rascunho_tests {
    use super::corpo_de_rascunho;
    use serde_json::json;

    /// Campos `Option<Uuid>` vazios viajam como `null`, nunca como `""`.
    ///
    /// O Core desserializa `mailbox_id` e `in_reply_to` para `Option<Uuid>`, e
    /// `""` não é um UUID — dava um `422` que o compositor mostrava como «Erro ao
    /// guardar rascunho», levando o anexo consigo. Uma mensagem nova traz os dois
    /// vazios.
    #[test]
    fn campos_uuid_vazios_viajam_como_null() {
        let corpo = corpo_de_rascunho(&json!({
            "mailbox_id": "",
            "in_reply_to": "",
            "to": "",
            "subject": "",
            "body": "",
        }));
        assert!(
            corpo["mailbox_id"].is_null(),
            "mailbox_id vazio devia ser null"
        );
        assert!(
            corpo["in_reply_to"].is_null(),
            "in_reply_to vazio devia ser null"
        );
    }

    /// Um `mailbox_id` real é preservado; um `in_reply_to` real também.
    #[test]
    fn campos_uuid_preenchidos_sao_preservados() {
        let id = "70bc515b-020e-4d7c-8805-36845d1e4537";
        let reply = "11111111-1111-4111-8111-111111111111";
        let corpo = corpo_de_rascunho(&json!({
            "mailbox_id": id,
            "in_reply_to": reply,
            "to": "a@b.com",
        }));
        assert_eq!(corpo["mailbox_id"], id);
        assert_eq!(corpo["in_reply_to"], reply);
        assert_eq!(corpo["to"], json!(["a@b.com"]));
    }
}

#[cfg(test)]
mod locale_tests {
    use super::locale_do_pedido;
    use axum::http::{header, HeaderMap, HeaderValue};
    use ocinye_contracts::Locale;

    fn com(nome: axum::http::HeaderName, valor: &str) -> HeaderMap {
        let mut h = HeaderMap::new();
        h.insert(nome, HeaderValue::from_str(valor).unwrap());
        h
    }

    #[test]
    fn sem_escolha_a_lingua_e_o_canonico() {
        // Sem cookie, é português — o predefinido não é uma adivinha.
        assert_eq!(locale_do_pedido(&HeaderMap::new()), Locale::Pt);
    }

    #[test]
    fn o_accept_language_do_browser_nao_decide_a_lingua() {
        // Um browser em inglês, sem escolha explícita, continua a ver português:
        // a língua não muda só por se abrir o Ocinye de outro browser (i18n §15,
        // §62). É também o que os testes de browser assumem.
        let so_accept = com(header::ACCEPT_LANGUAGE, "en-US,en;q=0.9,fr;q=0.8");
        assert_eq!(locale_do_pedido(&so_accept), Locale::Pt);
    }

    #[test]
    fn o_cookie_escolhido_decide() {
        assert_eq!(
            locale_do_pedido(&com(header::COOKIE, "oc_locale=fr")),
            Locale::Fr
        );
        assert_eq!(
            locale_do_pedido(&com(header::COOKIE, "oc_locale=en")),
            Locale::En
        );
        // Uma variante regional no cookie normaliza; uma língua inválida cai no pt.
        assert_eq!(
            locale_do_pedido(&com(header::COOKIE, "oc_locale=fr-FR")),
            Locale::Fr
        );
        assert_eq!(
            locale_do_pedido(&com(header::COOKIE, "oc_locale=de")),
            Locale::Pt
        );
    }
}

#[cfg(test)]
mod carregamento_tests {
    use super::aceita_json;
    use axum::http::{header, HeaderMap, HeaderValue};

    /// O carregador com barra de progresso pede JSON; a navegação sem JS não.
    ///
    /// É esta distinção que deixa o mesmo `/files/upload` responder com estado
    /// (para o XHR ler o progresso e o resultado) ou com um redireccionamento
    /// (para o formulário voltar à página) — sem impor um comportamento aos dois.
    #[test]
    fn so_pede_json_quem_o_aceita() {
        let mut com = HeaderMap::new();
        com.insert(header::ACCEPT, HeaderValue::from_static("application/json"));
        assert!(aceita_json(&com), "Accept: application/json pede JSON");

        let mut html = HeaderMap::new();
        html.insert(
            header::ACCEPT,
            HeaderValue::from_static("text/html,application/xhtml+xml"),
        );
        assert!(!aceita_json(&html), "um browser a navegar não pede JSON");

        assert!(
            !aceita_json(&HeaderMap::new()),
            "sem Accept, não se assume JSON"
        );
    }
}

// D004 · As aplicações de produtividade. Declarado no fim: usa `member_or_login!`.
mod productivity;
mod research;
