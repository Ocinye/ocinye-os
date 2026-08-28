//! Ocinye Mensagens — a superfície HTTP.
//!
//! # Transporte, e não domínio
//!
//! Nenhum handler decide autorização. Cada um resolve o principal, chama a
//! operação do Core e devolve o resultado. É a mesma operação que a capability
//! agentic chama — e é isso que faz a paridade ser estrutural em vez de
//! prometida (ADR-0307).

use axum::extract::{Path, Query, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use ocinye_core::modules::messaging::{self, Outgoing};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::ApiError;
use crate::extract::{CurrentPrincipal, Ids};
use crate::state::AppState;

/// As rotas das Mensagens.
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/messaging/conversations", get(list).post(start))
        .route("/messaging/conversations/{id}", get(one))
        .route(
            "/messaging/conversations/{id}/messages",
            get(history).post(send),
        )
        .route("/messaging/conversations/{id}/read", post(read))
        .route("/messaging/conversations/{id}/members", post(add_member))
        .route(
            "/messaging/conversations/{id}/members/{who}",
            axum::routing::delete(remove_member),
        )
        .route(
            "/messaging/conversations/{id}/messages/{message}/reactions",
            post(react),
        )
        .route("/messaging/assist", post(assist))
        .route("/messaging/presence", get(presence))
        .route("/messaging/typing", get(typing))
}

// ── Vistas ──────────────────────────────────────────────────────────────

/// Uma pessoa, tal como as Mensagens precisam de a mostrar.
#[derive(Serialize)]
struct PersonView {
    id: Uuid,
    name: String,
    /// A presença resolvida. Ausente quando o tempo real não está a funcionar —
    /// e ausente é diferente de «offline», que é uma afirmação.
    #[serde(skip_serializing_if = "Option::is_none")]
    presence: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    presence_label: Option<&'static str>,
}

#[derive(Serialize)]
struct ConversationView {
    id: Uuid,
    kind: String,
    /// O nome do grupo, ou o da pessoa do outro lado.
    title: String,
    role: String,
    unread: i64,
    unread_mentions: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    last_body: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    last_at: Option<chrono::DateTime<chrono::Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    last_read_at: Option<chrono::DateTime<chrono::Utc>>,
    /// A outra pessoa, numa conversa directa.
    #[serde(skip_serializing_if = "Option::is_none")]
    other: Option<PersonView>,
}

#[derive(Serialize)]
struct MessageView {
    id: Uuid,
    author_id: Uuid,
    author_name: String,
    /// Texto, e sempre texto. Quem o mostra escapa-o.
    body: String,
    created_at: chrono::DateTime<chrono::Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    edited_at: Option<chrono::DateTime<chrono::Utc>>,
    /// A mensagem a que responde, já resolvida para se poder citar.
    #[serde(skip_serializing_if = "Option::is_none")]
    reply_to: Option<Box<ReplyView>>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    mentions: Vec<Uuid>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    reactions: Vec<ReactionView>,
}

#[derive(Serialize)]
struct ReplyView {
    id: Uuid,
    author_name: String,
    /// Um excerto, e não a mensagem inteira: a citação é contexto.
    excerpt: String,
}

#[derive(Serialize)]
struct ReactionView {
    emoji: String,
    count: i64,
    /// Se quem está a olhar já reagiu com este.
    mine: bool,
}

/// Nomes de pessoas, lidos de uma vez.
///
/// Uma consulta por autor seria o `N+1` que transforma uma página de cinquenta
/// mensagens em cinquenta e uma idas à base.
async fn nomes(
    pool: &sqlx::PgPool,
    ids: &[Uuid],
) -> Result<std::collections::HashMap<Uuid, String>, sqlx::Error> {
    if ids.is_empty() {
        return Ok(std::collections::HashMap::new());
    }
    let linhas: Vec<(Uuid, String)> = sqlx::query_as(
        "SELECT id, COALESCE(display_name, full_name) FROM people WHERE id = ANY($1)",
    )
    .bind(ids)
    .fetch_all(pool)
    .await?;
    Ok(linhas.into_iter().collect())
}

// ── Conversas ───────────────────────────────────────────────────────────

/// `GET /messaging/conversations`
async fn list(
    State(state): State<AppState>,
    Ids(ids): Ids,
    CurrentPrincipal(principal): CurrentPrincipal,
) -> Result<Json<Vec<ConversationView>>, ApiError> {
    let listadas = messaging::conversations(&state.pool, &principal)
        .await
        .map_err(|error| ApiError::new(error, &ids))?;

    let outros: Vec<Uuid> = listadas.iter().filter_map(|l| l.other_id).collect();
    let mapa = nomes(&state.pool, &outros)
        .await
        .map_err(|error| ApiError::new(error.into(), &ids))?;

    let mut vistas = Vec::with_capacity(listadas.len());
    for listada in listadas {
        let other = match listada.other_id {
            Some(id) => {
                let sinais = state.realtime.sinais(id).await;
                let estado = ocinye_core::realtime::presence::resolver(sinais);
                Some(PersonView {
                    id,
                    name: mapa.get(&id).cloned().unwrap_or_default(),
                    presence: state.realtime.saudavel().then(|| estado.as_str()),
                    presence_label: state.realtime.saudavel().then(|| estado.label()),
                })
            }
            None => None,
        };

        let title = match (&listada.conversation.name, &other) {
            (Some(nome), _) => nome.clone(),
            (None, Some(pessoa)) => pessoa.name.clone(),
            // Uma directa cuja outra pessoa saiu da instituição. A conversa fica
            // legível — o que lá está foi dito —, e diz o que aconteceu.
            (None, None) => "Conversa".to_owned(),
        };

        vistas.push(ConversationView {
            id: listada.conversation.id,
            kind: listada.conversation.kind.clone(),
            title,
            role: listada.conversation.role.clone(),
            unread: listada.unread,
            unread_mentions: listada.unread_mentions,
            last_body: listada.last_body.clone(),
            last_at: listada.last_at,
            last_read_at: listada.conversation.last_read_at,
            other,
        });
    }

    Ok(Json(vistas))
}

#[derive(Deserialize)]
struct StartBody {
    /// Com quem, para uma directa.
    #[serde(default)]
    with: Option<Uuid>,
    /// O nome, para um grupo.
    #[serde(default)]
    name: Option<String>,
    /// Quem pertence, para um grupo.
    #[serde(default)]
    members: Vec<Uuid>,
}

/// `POST /messaging/conversations`
async fn start(
    State(state): State<AppState>,
    Ids(ids): Ids,
    CurrentPrincipal(principal): CurrentPrincipal,
    Json(body): Json<StartBody>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let id = match (body.with, body.name.as_deref()) {
        (Some(outra), None) => messaging::open_direct(&state.pool, &principal, outra, &ids).await,
        (None, Some(nome)) => {
            messaging::create_group(&state.pool, &principal, nome, &body.members, &ids).await
        }
        _ => Err(ocinye_core::CoreError::Validation(
            "Uma conversa é directa — com uma pessoa — ou é um grupo, com um nome.".to_owned(),
        )),
    }
    .map_err(|error| ApiError::new(error, &ids))?;

    Ok(Json(serde_json::json!({ "id": id })))
}

/// `GET /messaging/conversations/{id}`
async fn one(
    State(state): State<AppState>,
    Ids(ids): Ids,
    CurrentPrincipal(principal): CurrentPrincipal,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let listadas = messaging::conversations(&state.pool, &principal)
        .await
        .map_err(|error| ApiError::new(error, &ids))?;
    let listada = listadas
        .into_iter()
        .find(|l| l.conversation.id == id)
        .ok_or_else(|| {
            ApiError::new(
                ocinye_core::CoreError::NotFound("Conversa não encontrada.".to_owned()),
                &ids,
            )
        })?;

    let membros = ocinye_core::modules::messaging::repository::participants(&state.pool, id)
        .await
        .map_err(|error| ApiError::new(error, &ids))?;
    let mapa = nomes(&state.pool, &membros)
        .await
        .map_err(|error| ApiError::new(error.into(), &ids))?;

    let mut pessoas = Vec::with_capacity(membros.len());
    for pessoa in &membros {
        let sinais = state.realtime.sinais(*pessoa).await;
        let estado = ocinye_core::realtime::presence::resolver(sinais);
        pessoas.push(serde_json::json!({
            "id": pessoa,
            "name": mapa.get(pessoa).cloned().unwrap_or_default(),
            "presence": state.realtime.saudavel().then(|| estado.as_str()),
            "presence_label": state.realtime.saudavel().then(|| estado.label()),
        }));
    }

    let titulo = listada.conversation.name.clone().unwrap_or_else(|| {
        listada
            .other_id
            .and_then(|o| mapa.get(&o).cloned())
            .unwrap_or_else(|| "Conversa".to_owned())
    });

    Ok(Json(serde_json::json!({
        "id": id,
        "kind": listada.conversation.kind,
        "title": titulo,
        "role": listada.conversation.role,
        "participants": pessoas,
        "last_read_at": listada.conversation.last_read_at,
    })))
}

// ── Mensagens ───────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct HistoryQuery {
    /// O instante antes do qual continuar a ler.
    #[serde(default)]
    before: Option<chrono::DateTime<chrono::Utc>>,
}

/// `GET /messaging/conversations/{id}/messages`
async fn history(
    State(state): State<AppState>,
    Ids(ids): Ids,
    CurrentPrincipal(principal): CurrentPrincipal,
    Path(id): Path<Uuid>,
    Query(query): Query<HistoryQuery>,
) -> Result<Json<Vec<MessageView>>, ApiError> {
    let mensagens = messaging::history(&state.pool, &principal, id, query.before)
        .await
        .map_err(|error| ApiError::new(error, &ids))?;

    montar(&state, &principal, &mensagens, &ids).await
}

/// Monta as vistas de uma página de mensagens.
///
/// Tudo em consultas agregadas: autores, respostas, menções e reacções. Uma
/// consulta por mensagem seria o `N+1` que se sente em cada abertura.
async fn montar(
    state: &AppState,
    principal: &ocinye_domain::Principal,
    mensagens: &[ocinye_core::modules::messaging::repository::Message],
    ids: &ocinye_observability::CorrelationIds,
) -> Result<Json<Vec<MessageView>>, ApiError> {
    use ocinye_core::modules::messaging::repository as repo;

    let ids_das_mensagens: Vec<Uuid> = mensagens.iter().map(|m| m.id).collect();
    let alvos: Vec<Uuid> = mensagens.iter().filter_map(|m| m.reply_to_id).collect();

    let mut pessoas: Vec<Uuid> = mensagens.iter().map(|m| m.author_id).collect();

    let citadas = if alvos.is_empty() {
        Vec::new()
    } else {
        let mut encontradas = Vec::new();
        for alvo in &alvos {
            if let Some(m) = mensagens.iter().find(|m| m.id == *alvo) {
                encontradas.push(m.clone());
            } else if let Some(m) =
                repo::message_in(&state.pool, mensagens[0].conversation_id, *alvo)
                    .await
                    .map_err(|error| ApiError::new(error, ids))?
            {
                encontradas.push(m);
            }
        }
        encontradas
    };
    pessoas.extend(citadas.iter().map(|m| m.author_id));

    let mapa = nomes(&state.pool, &pessoas)
        .await
        .map_err(|error| ApiError::new(error.into(), ids))?;

    let mencoes = repo::mentions_of(&state.pool, &ids_das_mensagens)
        .await
        .map_err(|error| ApiError::new(error, ids))?;
    let reaccoes = repo::reactions_of(&state.pool, &ids_das_mensagens)
        .await
        .map_err(|error| ApiError::new(error, ids))?;

    let vistas = mensagens
        .iter()
        .map(|m| MessageView {
            id: m.id,
            author_id: m.author_id,
            author_name: mapa.get(&m.author_id).cloned().unwrap_or_default(),
            body: if m.deleted_at.is_some() {
                String::new()
            } else {
                m.body.clone()
            },
            created_at: m.created_at,
            edited_at: m.edited_at,
            reply_to: m.reply_to_id.and_then(|alvo| {
                citadas.iter().find(|c| c.id == alvo).map(|c| {
                    Box::new(ReplyView {
                        id: c.id,
                        author_name: mapa.get(&c.author_id).cloned().unwrap_or_default(),
                        excerpt: excerto(&c.body),
                    })
                })
            }),
            mentions: mencoes
                .iter()
                .filter(|(msg, _)| *msg == m.id)
                .map(|(_, pessoa)| *pessoa)
                .collect(),
            reactions: reaccoes
                .iter()
                .filter(|(msg, _, _, _)| *msg == m.id)
                .map(|(_, emoji, quantas, quem)| ReactionView {
                    emoji: emoji.clone(),
                    count: *quantas,
                    mine: quem.contains(&principal.person_id),
                })
                .collect(),
        })
        .collect();

    Ok(Json(vistas))
}

/// Um excerto para citar.
fn excerto(corpo: &str) -> String {
    const LIMITE: usize = 120;
    let limpo = corpo.trim();
    if limpo.chars().count() <= LIMITE {
        return limpo.to_owned();
    }
    let cortado: String = limpo.chars().take(LIMITE).collect();
    format!("{cortado}…")
}

#[derive(Deserialize)]
struct SendBody {
    body: String,
    #[serde(default)]
    reply_to: Option<Uuid>,
    #[serde(default)]
    mentions: Vec<Uuid>,
    #[serde(default)]
    idempotency_key: Option<String>,
}

/// `POST /messaging/conversations/{id}/messages`
async fn send(
    State(state): State<AppState>,
    Ids(ids): Ids,
    CurrentPrincipal(principal): CurrentPrincipal,
    Path(id): Path<Uuid>,
    Json(body): Json<SendBody>,
) -> Result<Json<serde_json::Value>, ApiError> {
    // O autor **não** vem do pedido. Vem do principal, e não há campo que o
    // permita dizer: um cliente que escolhesse o remetente escreveria como
    // qualquer pessoa da instituição.
    let message_id = messaging::send(
        &state.pool,
        &principal,
        &state.realtime,
        id,
        &Outgoing {
            body: &body.body,
            reply_to: body.reply_to,
            mentions: &body.mentions,
            idempotency_key: body.idempotency_key.as_deref(),
        },
        &ids,
    )
    .await
    .map_err(|error| ApiError::new(error, &ids))?;

    Ok(Json(serde_json::json!({ "id": message_id })))
}

#[derive(Deserialize)]
struct ReadBody {
    /// Até onde. Move-se para a frente e nunca para trás.
    until: chrono::DateTime<chrono::Utc>,
}

/// `POST /messaging/conversations/{id}/read`
async fn read(
    State(state): State<AppState>,
    Ids(ids): Ids,
    CurrentPrincipal(principal): CurrentPrincipal,
    Path(id): Path<Uuid>,
    Json(body): Json<ReadBody>,
) -> Result<Json<serde_json::Value>, ApiError> {
    messaging::mark_read(&state.pool, &principal, &state.realtime, id, body.until)
        .await
        .map_err(|error| ApiError::new(error, &ids))?;
    Ok(Json(serde_json::json!({ "read_until": body.until })))
}

#[derive(Deserialize)]
struct MemberBody {
    who: Uuid,
}

/// `POST /messaging/conversations/{id}/members`
async fn add_member(
    State(state): State<AppState>,
    Ids(ids): Ids,
    CurrentPrincipal(principal): CurrentPrincipal,
    Path(id): Path<Uuid>,
    Json(body): Json<MemberBody>,
) -> Result<Json<serde_json::Value>, ApiError> {
    messaging::add_member(&state.pool, &principal, &state.realtime, id, body.who, &ids)
        .await
        .map_err(|error| ApiError::new(error, &ids))?;
    Ok(Json(serde_json::json!({ "added": body.who })))
}

/// `DELETE /messaging/conversations/{id}/members/{who}`
async fn remove_member(
    State(state): State<AppState>,
    Ids(ids): Ids,
    CurrentPrincipal(principal): CurrentPrincipal,
    Path((id, who)): Path<(Uuid, Uuid)>,
) -> Result<Json<serde_json::Value>, ApiError> {
    messaging::remove_member(&state.pool, &principal, &state.realtime, id, who, &ids)
        .await
        .map_err(|error| ApiError::new(error, &ids))?;
    Ok(Json(serde_json::json!({ "removed": who })))
}

#[derive(Deserialize)]
struct ReactionBody {
    emoji: String,
}

/// `POST /messaging/conversations/{id}/messages/{message}/reactions`
async fn react(
    State(state): State<AppState>,
    Ids(ids): Ids,
    CurrentPrincipal(principal): CurrentPrincipal,
    Path((id, message)): Path<(Uuid, Uuid)>,
    Json(body): Json<ReactionBody>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let posta = messaging::toggle_reaction(
        &state.pool,
        &principal,
        &state.realtime,
        id,
        message,
        &body.emoji,
    )
    .await
    .map_err(|error| ApiError::new(error, &ids))?;

    Ok(Json(serde_json::json!({ "reacted": posta })))
}

#[derive(Deserialize)]
struct AssistBody {
    action: String,
    draft: String,
}

/// `POST /messaging/assist`
///
/// Devolve uma proposta. **Nunca envia** — enviar é outra operação, e é a pessoa
/// que a desencadeia.
async fn assist(
    State(state): State<AppState>,
    Ids(ids): Ids,
    CurrentPrincipal(principal): CurrentPrincipal,
    Json(body): Json<AssistBody>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let _ = &state;
    let texto = messaging::assist(&principal, &body.action, &body.draft)
        .await
        .map_err(|error| ApiError::new(error, &ids))?;

    Ok(Json(serde_json::json!({ "text": texto })))
}

// ── Presença e `typing` ─────────────────────────────────────────────────

#[derive(Deserialize)]
struct PeopleQuery {
    /// Os identificadores, separados por vírgula.
    #[serde(default)]
    ids: Option<String>,
}

/// `GET /messaging/presence?ids=…`
///
/// # Porque um pedido para várias pessoas
///
/// Porque a lista de conversas precisa da presença de todas de uma vez, e um
/// pedido por pessoa é o `N+1` do plano realtime.
async fn presence(
    State(state): State<AppState>,
    Ids(ids): Ids,
    CurrentPrincipal(principal): CurrentPrincipal,
    Query(query): Query<PeopleQuery>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let _ = &principal;
    let pedidos: Vec<Uuid> = query
        .ids
        .unwrap_or_default()
        .split(',')
        .filter_map(|parte| Uuid::parse_str(parte.trim()).ok())
        .take(200)
        .collect();

    let mut saida = serde_json::Map::new();
    for pessoa in pedidos {
        let sinais = state.realtime.sinais(pessoa).await;
        let estado = ocinye_core::realtime::presence::resolver(sinais);
        saida.insert(
            pessoa.to_string(),
            serde_json::json!({
                "state": estado.as_str(),
                "label": estado.label(),
            }),
        );
    }

    let _ = &ids;
    Ok(Json(serde_json::json!({
        "realtime": state.realtime.saudavel(),
        "people": saida,
    })))
}

#[derive(Deserialize)]
struct TypingQuery {
    conversation: Uuid,
}

/// `GET /messaging/typing?conversation=…`
async fn typing(
    State(state): State<AppState>,
    Ids(ids): Ids,
    CurrentPrincipal(principal): CurrentPrincipal,
    Query(query): Query<TypingQuery>,
) -> Result<Json<serde_json::Value>, ApiError> {
    use ocinye_core::modules::messaging::repository as repo;

    // Quem está a escrever numa conversa só se diz a quem participa nela. Sem
    // isto, saber que duas pessoas falam uma com a outra ficaria à distância de
    // um identificador.
    if !repo::participates(&state.pool, query.conversation, principal.person_id)
        .await
        .map_err(|error| ApiError::new(error, &ids))?
    {
        return Err(ApiError::new(
            ocinye_core::CoreError::NotFound("Conversa não encontrada.".to_owned()),
            &ids,
        ));
    }

    let quem: Vec<Uuid> = state
        .realtime
        .quem_escreve(query.conversation)
        .await
        .into_iter()
        .filter(|p| *p != principal.person_id)
        .collect();

    let mapa = nomes(&state.pool, &quem)
        .await
        .map_err(|error| ApiError::new(error.into(), &ids))?;
    let etiquetas: Vec<String> = quem.iter().filter_map(|p| mapa.get(p).cloned()).collect();

    Ok(Json(serde_json::json!({
        "people": quem,
        "phrase": ocinye_core::realtime::presence::frase_de_escrita(&etiquetas),
    })))
}
