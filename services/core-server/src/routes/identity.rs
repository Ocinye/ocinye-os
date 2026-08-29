//! Identity routes: the acting member, people, invitations and roles.

use axum::extract::{DefaultBodyLimit, Multipart, Path, Query, State};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use ocinye_contracts::{AvatarChoice, InstitutionalPosition, Page, PageRequest, TechnicalRole};
use ocinye_core::modules::identity;
use ocinye_core::CoreError;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::ApiError;
use crate::extract::{CurrentPrincipal, Ids};
use crate::state::AppState;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/me", get(me))
        // O avatar do próprio membro. Nenhuma destas rotas recebe uma pessoa:
        // o `me` no caminho não é um parâmetro, é uma afirmação.
        .route("/me/avatar", axum::routing::delete(use_initials))
        .route("/me/avatar/preset", post(choose_preset))
        .route(
            "/me/avatar/photo",
            post(set_photograph).layer(DefaultBodyLimit::max(AVATAR_BODY_LIMIT_BYTES)),
        )
        .route("/me/avatar/{version}", get(read_own_avatar))
        .route("/people", get(list_people))
        .route("/people/{person_id}", get(get_person))
        .route("/invitations", post(create_invitation))
        .route("/invitations/accept", post(accept_invitation))
        .route(
            "/people/{person_id}/roles",
            post(grant_role).delete(revoke_role),
        )
}

/// Largest body accepted for a profile photograph.
///
/// Folga sobre o limite da própria imagem para o envelope multipart, e nada
/// mais: a diferença entre este número e `MAX_AVATAR_BYTES` é espaço para
/// cabeçalhos, não para uma fotografia maior.
const AVATAR_BODY_LIMIT_BYTES: usize = ocinye_core::avatar::MAX_AVATAR_BYTES + 64 * 1024;

/// The acting member, as the Workspace needs it to render context.
#[derive(Serialize)]
struct Me {
    person_id: Uuid,
    display_name: String,
    /// Sign-in name. `None` for rows predating ADR-0103.
    /// Institutional email.
    email: String,
    /// Account status, as the organisation records it.
    status: String,
    /// Como o membro escolheu ser representado.
    ///
    /// Nunca a chave do objecto, o bucket, o endpoint nem um URL assinado: só
    /// o suficiente para a interface montar o endereço da própria leitura.
    avatar: AvatarChoice,
    organisation_id: Uuid,
    /// Technical roles. What the caller *may do*.
    roles: Vec<&'static str>,
    /// Units the caller belongs to, with their role.
    units: Vec<Membership>,
    /// Research workspaces the caller belongs to, with their role.
    workspaces: Vec<Membership>,
    /// Institution-scope permissions the caller holds.
    ///
    /// The Workspace renders navigation and the create menu from this list, so
    /// it never has to guess from a role name — and so a permission-aware
    /// interface stays correct when the role table changes (briefing §65, §67).
    ///
    /// This is a *hint for rendering*, never an authorisation. Every endpoint
    /// re-decides for itself; hiding a control the Core would refuse anyway is
    /// courtesy, not security (`CLAUDE.md` §4).
    capabilities: Vec<&'static str>,
}

#[derive(Serialize)]
struct Membership {
    id: Uuid,
    role: &'static str,
}

/// The caller, as the Core knows them.
///
/// # Why this reads the person row
///
/// The principal carries what *authorisation* needs — identity, organisation,
/// roles, memberships — and nothing more. Username, email and account status
/// are not authorisation, and so were never there. The account screen asked for
/// them anyway and rendered three dashes: a screen promising facts its endpoint
/// did not carry.
///
/// The read is of the caller's own record and cannot be aimed elsewhere; see
/// `identity::get_own_person`.
async fn me(
    State(state): State<AppState>,
    Ids(ids): Ids,
    CurrentPrincipal(principal): CurrentPrincipal,
) -> Result<Json<Me>, ApiError> {
    let mut roles: Vec<&'static str> = principal.roles.iter().map(|r| r.as_str()).collect();
    roles.sort_unstable();

    let capabilities = capabilities_held_anywhere(&state.pool, &principal, &ids).await?;

    let person = identity::get_own_person(&state.pool, &principal).await?;
    let avatar = identity::own_avatar(&state.pool, &principal).await?;

    Ok(Json(Me {
        person_id: principal.person_id,
        display_name: principal.display_name.clone(),
        email: person.email,
        status: person.status,
        avatar,
        organisation_id: principal.organisation_id,
        roles,
        capabilities,
        units: principal
            .unit_roles
            .iter()
            .map(|(id, role)| Membership {
                id: *id,
                role: role.as_str(),
            })
            .collect(),
        workspaces: principal
            .workspace_roles
            .iter()
            .map(|(id, role)| Membership {
                id: *id,
                role: role.as_str(),
            })
            .collect(),
    }))
}

#[derive(Serialize)]
struct PersonView {
    id: Uuid,
    full_name: String,
    display_name: Option<String>,
    email: String,
    /// Institutional position. Shown for attribution; grants nothing.
    institutional_position: Option<String>,
    orcid: Option<String>,
    status: String,
}

impl From<identity::Person> for PersonView {
    fn from(person: identity::Person) -> Self {
        Self {
            id: person.id,
            full_name: person.full_name,
            display_name: person.display_name,
            email: person.email,
            institutional_position: person.institutional_position,
            orcid: person.orcid,
            status: person.status,
        }
    }
}

async fn list_people(
    State(state): State<AppState>,
    CurrentPrincipal(principal): CurrentPrincipal,
    Query(page): Query<PageRequest>,
) -> Result<Json<Page<PersonView>>, ApiError> {
    let (people, total) = identity::list_people(&state.pool, &principal, page).await?;
    Ok(Json(Page::new(
        people.into_iter().map(PersonView::from).collect(),
        page,
        total,
    )))
}

async fn get_person(
    State(state): State<AppState>,
    CurrentPrincipal(principal): CurrentPrincipal,
    Path(person_id): Path<Uuid>,
) -> Result<Json<PersonView>, ApiError> {
    let person = identity::get_person(&state.pool, &principal, person_id).await?;
    Ok(Json(PersonView::from(person)))
}

#[derive(Deserialize)]
struct InviteRequest {
    email: String,
    full_name: String,
    #[serde(default)]
    institutional_position: Option<String>,
}

/// The invitation, with its one-time token.
///
/// The token is returned here and nowhere else: only its digest is stored, so
/// it cannot be recovered from the database or a backup.
#[derive(Serialize)]
struct InvitationIssued {
    invitation_id: Uuid,
    expires_at: chrono::DateTime<chrono::Utc>,
    token: String,
}

async fn create_invitation(
    State(state): State<AppState>,
    CurrentPrincipal(principal): CurrentPrincipal,
    Ids(ids): Ids,
    Json(request): Json<InviteRequest>,
) -> Result<Json<InvitationIssued>, ApiError> {
    let position =
        match request.institutional_position.as_deref() {
            Some(raw) => Some(InstitutionalPosition::parse(raw).ok_or_else(|| {
                CoreError::Validation("Unknown institutional position.".to_owned())
            })?),
            None => None,
        };

    let mut tx = state.pool.begin().await.map_err(CoreError::from)?;
    let issued = identity::create_invitation(
        &mut tx,
        &principal,
        &ids,
        identity::NewInvitation {
            email: request.email,
            full_name: request.full_name,
            institutional_position: position,
        },
    )
    .await?;
    tx.commit().await.map_err(CoreError::from)?;

    Ok(Json(InvitationIssued {
        invitation_id: issued.invitation.id,
        expires_at: issued.invitation.expires_at,
        token: issued.token,
    }))
}

#[derive(Deserialize)]
struct AcceptRequest {
    token: String,
}

/// Accept an invitation.
///
/// Unauthenticated by design: the token is the proof. The person created cannot
/// act yet — access begins at first verified sign-in.
async fn accept_invitation(
    State(state): State<AppState>,
    Ids(ids): Ids,
    Json(request): Json<AcceptRequest>,
) -> Result<Json<PersonView>, ApiError> {
    let mut tx = state.pool.begin().await.map_err(CoreError::from)?;
    let person = identity::accept_invitation(&mut tx, &ids, &request.token).await?;
    tx.commit().await.map_err(CoreError::from)?;
    Ok(Json(PersonView::from(person)))
}

#[derive(Deserialize)]
struct RoleRequest {
    role: String,
    #[serde(default)]
    reason: Option<String>,
}

fn parse_role(raw: &str) -> Result<TechnicalRole, CoreError> {
    TechnicalRole::parse(raw)
        .ok_or_else(|| CoreError::Validation("Unknown technical role.".to_owned()))
}

async fn grant_role(
    State(state): State<AppState>,
    CurrentPrincipal(principal): CurrentPrincipal,
    Ids(ids): Ids,
    Path(person_id): Path<Uuid>,
    Json(request): Json<RoleRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let role = parse_role(&request.role)?;
    let reason = request.reason.unwrap_or_default();

    let mut tx = state.pool.begin().await.map_err(CoreError::from)?;
    identity::grant_role(&mut tx, &principal, &ids, person_id, role, &reason).await?;
    tx.commit().await.map_err(CoreError::from)?;

    Ok(Json(serde_json::json!({ "granted": role.as_str() })))
}

async fn revoke_role(
    State(state): State<AppState>,
    CurrentPrincipal(principal): CurrentPrincipal,
    Ids(ids): Ids,
    Path(person_id): Path<Uuid>,
    Json(request): Json<RoleRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let role = parse_role(&request.role)?;

    let mut tx = state.pool.begin().await.map_err(CoreError::from)?;
    identity::revoke_role(&mut tx, &principal, &ids, person_id, role).await?;
    tx.commit().await.map_err(CoreError::from)?;

    Ok(Json(serde_json::json!({ "revoked": role.as_str() })))
}

// --- Avatar ----------------------------------------------------------------

/// O identificador de um avatar do produto.
#[derive(Deserialize)]
struct PresetChoice {
    preset: String,
}

/// Volta às iniciais.
async fn use_initials(
    State(state): State<AppState>,
    CurrentPrincipal(principal): CurrentPrincipal,
) -> Result<Json<AvatarChoice>, ApiError> {
    // O armazenamento pode não estar configurado, e voltar às iniciais tem de
    // funcionar à mesma: não conseguir limpar o bucket não é razão para prender
    // alguém à fotografia que lá está.
    Ok(Json(
        identity::use_initials(&state.pool, &principal, state.store().ok()).await?,
    ))
}

/// Escolhe um avatar do catálogo do produto.
async fn choose_preset(
    State(state): State<AppState>,
    CurrentPrincipal(principal): CurrentPrincipal,
    Json(choice): Json<PresetChoice>,
) -> Result<Json<AvatarChoice>, ApiError> {
    Ok(Json(
        identity::choose_preset(&state.pool, &principal, state.store().ok(), &choice.preset)
            .await?,
    ))
}

/// Carrega a fotografia do próprio membro.
async fn set_photograph(
    State(state): State<AppState>,
    CurrentPrincipal(principal): CurrentPrincipal,
    multipart: Multipart,
) -> Result<Json<AvatarChoice>, ApiError> {
    let upload = super::knowledge::read_upload_public(multipart).await?;
    let store = state.store()?;

    // O `content_type` que o cliente declarou não é consultado: a normalização
    // decide o formato pelos bytes. Declarar `image/jpeg` sobre um executável
    // não o torna um, e perguntar ao cliente o que ele enviou é perguntar a
    // quem tem interesse na resposta.
    Ok(Json(
        identity::set_photograph(
            &state.pool,
            &principal,
            store,
            &state.config.organisation_slug,
            &upload.data,
        )
        .await?,
    ))
}

/// Lê a fotografia do próprio membro.
///
/// # Porque não redirecciona para o URL assinado
///
/// Um URL assinado dura cinco minutos e muda a cada assinatura. Num `<img>` de
/// uma barra lateral isso significa três coisas más ao mesmo tempo: a cache do
/// browser nunca acerta, o endereço do bucket aparece em todas as páginas, e
/// uma janela aberta há mais de cinco minutos mostra a imagem partida.
///
/// A fotografia tem alguns kilobytes. Passá-la por aqui custa menos do que
/// qualquer uma dessas três coisas, e deixa a shell a falar apenas com o Ocinye
/// OS.
///
/// # A versão não é autoridade
///
/// O caminho traz um identificador de conteúdo, e o identificador não abre
/// nada: a sessão é que diz de quem é o avatar, e a consulta confirma que a
/// versão pedida é a **desse** principal. Uma versão de outra pessoa devolve o
/// mesmo que uma versão inexistente.
async fn read_own_avatar(
    State(state): State<AppState>,
    CurrentPrincipal(principal): CurrentPrincipal,
    Path(version): Path<String>,
) -> Result<Response, ApiError> {
    let key = identity::own_photograph_key(&state.pool, &principal, &version).await?;
    let bytes = state.store()?.get(&key).await?;

    Ok((
        [
            (
                axum::http::header::CONTENT_TYPE,
                ocinye_core::avatar::AVATAR_CONTENT_TYPE,
            ),
            // Uma fotografia nova recebe uma versão nova, e portanto um
            // endereço novo. Este nunca muda de conteúdo, e pode ser guardado
            // para sempre — `private` porque é de uma pessoa, e uma cache
            // partilhada não deve servi-lo a outra.
            (
                axum::http::header::CACHE_CONTROL,
                "private, max-age=31536000, immutable",
            ),
        ],
        bytes,
    )
        .into_response())
}

/// Every permission the caller holds **somewhere**.
///
/// # Porque não só no contexto institucional
///
/// Porque havia direitos que nunca podiam aparecer aqui. `DocumentsView`,
/// `BibliographyView` e `DatasetsView` só existem como concessão contextual —
/// pertença a unidade ou a ambiente —, e a lista era calculada contra o
/// contexto da organização, onde `unit_id` e `workspace_id` são `None`. Nenhuma
/// concessão contextual se aplica lá, pelo que nenhuma das três entrava na
/// lista, para ninguém.
///
/// O efeito era visível: as quatro entradas de CONHECIMENTO apareciam esbatidas
/// na navegação **para toda a gente**, incluindo para quem pertencia a um
/// ambiente cheio de ficheiros. A navegação dizia «não tem autorização» a quem
/// tinha, e o mesmo mecanismo escondia o botão de carregar dentro do ecrã.
///
/// Isto avalia a mesma política contra os contextos que a pessoa **tem**: a
/// organização, cada unidade e cada ambiente de que é membro. Não é uma segunda
/// fonte de verdade — é a mesma função `can`, perguntada nos sítios certos.
///
/// Continua a ser um **indício de renderização, nunca uma autorização**: cada
/// operação volta a decidir por si, no recurso concreto. «Tem este direito
/// nalgum sítio» não é «tem este direito aqui», e esconder um controlo que o
/// Core recusaria é cortesia — mostrar um que ele aceitaria é o mínimo.
///
/// # Errors
///
/// Devolve erro quando a consulta das unidades dos ambientes falha.
async fn capabilities_held_anywhere(
    pool: &sqlx::PgPool,
    principal: &ocinye_domain::Principal,
    ids: &ocinye_observability::CorrelationIds,
) -> Result<Vec<&'static str>, ApiError> {
    use ocinye_domain::{ResourceContext, ResourceKind};

    let mut contexts = vec![ResourceContext::organisation(
        ResourceKind::Organisation,
        principal.organisation_id,
    )];

    for unit_id in principal.unit_ids() {
        contexts.push(ResourceContext::unit(
            ResourceKind::Unit,
            principal.organisation_id,
            unit_id,
        ));
    }

    // A unidade de cada ambiente vem da base e não de um palpite: um `unit_id`
    // inventado aqui concederia direitos de unidade a quem só é membro do
    // ambiente, que é precisamente o erro que esta função existe para não
    // cometer noutra direcção.
    let workspace_ids: Vec<uuid::Uuid> = principal.workspace_roles.keys().copied().collect();
    if !workspace_ids.is_empty() {
        let pares: Vec<(uuid::Uuid, uuid::Uuid)> = sqlx::query_as(
            "SELECT id, unit_id FROM research_workspaces
              WHERE id = ANY($1) AND organisation_id = $2",
        )
        .bind(&workspace_ids)
        .bind(principal.organisation_id)
        .fetch_all(pool)
        .await
        .map_err(|error| ApiError::new(CoreError::from(error), ids))?;

        for (workspace_id, unit_id) in pares {
            contexts.push(ResourceContext::workspace(
                ResourceKind::ResearchWorkspace,
                principal.organisation_id,
                unit_id,
                workspace_id,
                // A classificação não participa na posse de uma permissão: ela
                // governa as acções sobre um recurso, e é reavaliada lá.
                ocinye_contracts::Classification::Internal,
            ));
        }
    }

    Ok(ocinye_contracts::Permission::all()
        .into_iter()
        .filter(|permission| {
            contexts
                .iter()
                .any(|ctx| ocinye_domain::can(principal, *permission, ctx, None).allowed)
        })
        .map(ocinye_contracts::Permission::as_str)
        .collect())
}
