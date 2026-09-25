//! As aplicações fixadas na barra lateral — a preferência por membro (§11 do
//! briefing do Gestor de Aplicações).
//!
//! Sem escolha, a leitura devolve `None` (o Workspace aplica o conjunto por
//! omissão); uma lista vazia é uma escolha, distinta de nunca ter escolhido;
//! escrever substitui pela lista inteira, pela ordem dada; e a validação de
//! forma recusa listas grandes de mais, ids mal formados e repetidos.
//!
//! Salta quando `OCINYE_TEST_DATABASE_URL` não está definida; falha se estiver
//! mas a base não responder.

use ocinye_contracts::TechnicalRole;
use ocinye_core::modules::identity;
use ocinye_core::CoreError;
use ocinye_domain::Principal;
use sqlx::PgPool;
use uuid::Uuid;

async fn pool() -> Option<PgPool> {
    let Ok(url) = std::env::var("OCINYE_TEST_DATABASE_URL") else {
        assert!(
            std::env::var("CI").is_err(),
            "OCINYE_TEST_DATABASE_URL em falta em CI: a fixação de aplicações ficaria por verificar"
        );
        return None;
    };
    let pool = PgPool::connect(&url)
        .await
        .expect("OCINYE_TEST_DATABASE_URL definida mas a base não responde");
    sqlx::migrate!("../../migrations")
        .run(&pool)
        .await
        .expect("migrations");
    Some(pool)
}

async fn member(pool: &PgPool) -> Principal {
    let slug = format!("s{}", Uuid::new_v4().simple());
    let organisation_id: Uuid =
        sqlx::query_scalar("INSERT INTO organisations (slug, name) VALUES ($1, $1) RETURNING id")
            .bind(&slug)
            .fetch_one(pool)
            .await
            .expect("organização");
    let handle = format!("p{}", Uuid::new_v4().simple());
    let person_id: Uuid = sqlx::query_scalar(
        "INSERT INTO people (organisation_id, full_name, email, status)
         VALUES ($1, $2, $3, 'active') RETURNING id",
    )
    .bind(organisation_id)
    .bind(&handle)
    .bind(format!("{handle}@ocinye.com"))
    .fetch_one(pool)
    .await
    .expect("pessoa");
    sqlx::query("INSERT INTO person_roles (person_id, role) VALUES ($1, $2)")
        .bind(person_id)
        .bind(TechnicalRole::ResearchMember.as_str())
        .execute(pool)
        .await
        .expect("papel");
    let pessoa = identity::person_by_id(pool, person_id)
        .await
        .expect("consulta")
        .expect("pessoa");
    identity::principal_for_person(pool, &pessoa)
        .await
        .expect("principal")
}

/// Sem escolha, `None`; depois de escrever, a lista tal como veio, pela ordem.
#[tokio::test]
async fn a_ausencia_e_distinta_da_lista_vazia_e_a_ordem_preserva_se() {
    let Some(pool) = pool().await else { return };
    let quem = member(&pool).await;

    // Nunca escolheu.
    assert_eq!(
        identity::list_app_pins(&pool, &quem).await.expect("ler"),
        None,
        "sem linha, a leitura tem de ser None para o Workspace aplicar o padrão"
    );

    // Fixa três, por uma ordem deliberada.
    let escolha = vec![
        "files".to_owned(),
        "notes".to_owned(),
        "projects".to_owned(),
    ];
    identity::set_app_pins(&pool, &quem, &escolha)
        .await
        .expect("fixar");
    assert_eq!(
        identity::list_app_pins(&pool, &quem).await.expect("ler"),
        Some(escolha.clone()),
        "a lista guardada não é a que foi escrita, ou perdeu a ordem"
    );

    // Reordena: substitui pela lista inteira.
    let reordenada = vec![
        "projects".to_owned(),
        "files".to_owned(),
        "notes".to_owned(),
    ];
    identity::set_app_pins(&pool, &quem, &reordenada)
        .await
        .expect("reordenar");
    assert_eq!(
        identity::list_app_pins(&pool, &quem).await.expect("ler"),
        Some(reordenada),
        "reordenar tem de substituir pela lista inteira"
    );

    // Tira tudo: uma lista vazia é uma escolha — Some(vec![]), não None.
    identity::set_app_pins(&pool, &quem, &[])
        .await
        .expect("esvaziar");
    assert_eq!(
        identity::list_app_pins(&pool, &quem).await.expect("ler"),
        Some(Vec::new()),
        "esvaziar é uma escolha explícita, distinta de nunca ter escolhido"
    );
}

/// A validação de forma recusa o que não pode guardar.
#[tokio::test]
async fn a_validacao_recusa_ids_maus_e_repetidos() {
    let Some(pool) = pool().await else { return };
    let quem = member(&pool).await;

    // Repetido.
    let r = identity::set_app_pins(&pool, &quem, &["files".to_owned(), "files".to_owned()]).await;
    assert!(
        matches!(r, Err(CoreError::Validation(_))),
        "um id repetido tem de ser recusado"
    );

    // Mal formado (espaço).
    let r = identity::set_app_pins(&pool, &quem, &["a b".to_owned()]).await;
    assert!(
        matches!(r, Err(CoreError::Validation(_))),
        "um id com espaço tem de ser recusado"
    );

    // Grande de mais.
    let muitos: Vec<String> = (0..100).map(|i| format!("app{i}")).collect();
    let r = identity::set_app_pins(&pool, &quem, &muitos).await;
    assert!(
        matches!(r, Err(CoreError::Validation(_))),
        "uma lista enorme tem de ser recusada"
    );

    // E nenhuma dessas escritas deixou estado: continua sem escolha.
    assert_eq!(
        identity::list_app_pins(&pool, &quem).await.expect("ler"),
        None,
        "uma escrita recusada não pode ter guardado nada"
    );
}

/// A fixação é do próprio: a lista de um membro não é a de outro (IDOR fechado
/// pela resolução do dono na sessão).
#[tokio::test]
async fn a_fixacao_de_um_membro_nao_e_a_de_outro() {
    let Some(pool) = pool().await else { return };
    let a = member(&pool).await;
    let b = member(&pool).await;

    identity::set_app_pins(&pool, &a, &["mail".to_owned()])
        .await
        .expect("fixar de A");

    assert_eq!(
        identity::list_app_pins(&pool, &b).await.expect("ler B"),
        None,
        "a fixação de A não pode aparecer a B"
    );
    assert_eq!(
        identity::list_app_pins(&pool, &a).await.expect("ler A"),
        Some(vec!["mail".to_owned()]),
        "a fixação de A tem de continuar a ser a de A"
    );
}
