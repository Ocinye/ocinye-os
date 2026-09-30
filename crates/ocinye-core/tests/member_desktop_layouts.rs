//! A disposição do Desktop — a preferência por membro (Claude Design D001, FG-017).
//!
//! Sem disposição, `None` (o Workspace desenha a predefinição da distribuição);
//! a primeira gravação traz a versão `0`; cada gravação seguinte traz a versão
//! que leu, e uma versão obsoleta é recusada com conflito — duas sessões não se
//! sobrepõem em silêncio. A forma é validada contra o registo antes de gravar,
//! repor apaga a disposição própria, e a de um membro não é a de outro.
//!
//! Salta quando `OCINYE_TEST_DATABASE_URL` não está definida; falha se estiver
//! mas a base não responder.

use ocinye_contracts::desktop::{DesktopLayout, PlacedWidget};
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
            "OCINYE_TEST_DATABASE_URL em falta em CI: a disposição do Desktop ficaria por verificar"
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

fn layout(widgets: &[(&str, u8, u8)]) -> DesktopLayout {
    DesktopLayout {
        wallpaper: "org".to_owned(),
        fit: "fill".to_owned(),
        dim: 20,
        widgets: widgets
            .iter()
            .map(|&(k, w, h)| PlacedWidget {
                id: k.to_owned(),
                kind: k.to_owned(),
                w,
                h,
                minimized: false,
            })
            .collect(),
    }
}

/// Sem disposição, `None`; a primeira gravação é a versão 1 e lê-se tal como foi.
#[tokio::test]
async fn sem_disposicao_segue_a_predefinicao_e_a_primeira_gravacao_e_a_versao_um() {
    let Some(pool) = pool().await else { return };
    let quem = member(&pool).await;

    assert_eq!(
        identity::get_desktop(&pool, &quem).await.expect("ler"),
        None
    );

    let minha = layout(&[("notice", 2, 1), ("tasks", 1, 2)]);
    let v = identity::put_desktop(&pool, &quem, 0, &minha, false)
        .await
        .expect("gravar");
    assert_eq!(v, 1);
    let lida = identity::get_desktop(&pool, &quem)
        .await
        .expect("ler")
        .expect("gravada");
    assert_eq!((lida.version, lida.layout), (1, minha));
}

/// Uma versão obsoleta é um conflito, e não sobrepõe a gravação da outra sessão.
#[tokio::test]
async fn duas_sessoes_nao_se_sobrepoem_em_silencio() {
    let Some(pool) = pool().await else { return };
    let quem = member(&pool).await;

    let a = layout(&[("notice", 2, 1), ("tasks", 1, 2)]);
    let b = layout(&[("notice", 2, 1), ("notes", 1, 1)]);
    identity::put_desktop(&pool, &quem, 0, &a, false)
        .await
        .expect("v1");
    // As duas janelas leram a versão 1; a primeira grava, a segunda chega tarde.
    assert_eq!(
        identity::put_desktop(&pool, &quem, 1, &a, false)
            .await
            .expect("v2"),
        2
    );
    let tarde = identity::put_desktop(&pool, &quem, 1, &b, false).await;
    assert!(matches!(tarde, Err(CoreError::Conflict(_))), "{tarde:?}");
    // E a primeira gravação também não pode ser repetida com a versão 0.
    let de_novo = identity::put_desktop(&pool, &quem, 0, &b, false).await;
    assert!(
        matches!(de_novo, Err(CoreError::Conflict(_))),
        "{de_novo:?}"
    );
    let lida = identity::get_desktop(&pool, &quem)
        .await
        .expect("ler")
        .expect("gravada");
    assert_eq!(
        (lida.version, lida.layout),
        (2, a),
        "a gravação tardia não pode ter ganho"
    );
}

/// A forma é validada antes de gravar: nada de inválido chega à base.
#[tokio::test]
async fn uma_disposicao_invalida_e_recusada_sem_gravar() {
    let Some(pool) = pool().await else { return };
    let quem = member(&pool).await;

    for errada in [
        layout(&[("notice", 2, 1), ("nye", 1, 1)]), // tipo desconhecido
        layout(&[("notice", 2, 1), ("kpis", 1, 1)]), // tamanho não permitido
        layout(&[("notice", 2, 1), ("notice", 2, 1)]), // repetido
    ] {
        let r = identity::put_desktop(&pool, &quem, 0, &errada, false).await;
        assert!(matches!(r, Err(CoreError::Validation(_))), "{r:?}");
    }
    assert_eq!(
        identity::get_desktop(&pool, &quem).await.expect("ler"),
        None
    );
}

/// D009: nenhum tipo é obrigatório — um Desktop sem avisos, ou vazio, é válido.
#[tokio::test]
async fn sem_obrigatorios_um_desktop_vazio_grava() {
    let Some(pool) = pool().await else { return };
    let quem = member(&pool).await;
    let v = identity::put_desktop(&pool, &quem, 0, &layout(&[("tasks", 1, 2)]), false)
        .await
        .expect("sem avisos");
    identity::put_desktop(&pool, &quem, v, &layout(&[]), false)
        .await
        .expect("vazio");
}

/// Repor apaga a disposição própria; a gravação seguinte volta a ser a primeira.
#[tokio::test]
async fn repor_volta_a_predefinicao() {
    let Some(pool) = pool().await else { return };
    let quem = member(&pool).await;

    identity::put_desktop(&pool, &quem, 0, &layout(&[("notice", 2, 1)]), false)
        .await
        .expect("gravar");
    identity::reset_desktop(&pool, &quem).await.expect("repor");
    assert_eq!(
        identity::get_desktop(&pool, &quem).await.expect("ler"),
        None
    );
    assert_eq!(
        identity::put_desktop(&pool, &quem, 0, &layout(&[("notice", 2, 1)]), false)
            .await
            .expect("gravar de novo"),
        1
    );
}

/// A disposição é do próprio: a de um membro não é a de outro.
#[tokio::test]
async fn a_disposicao_de_um_membro_nao_e_a_de_outro() {
    let Some(pool) = pool().await else { return };
    let a = member(&pool).await;
    let b = member(&pool).await;

    identity::put_desktop(&pool, &a, 0, &layout(&[("notice", 2, 1)]), false)
        .await
        .expect("gravar de A");
    assert_eq!(identity::get_desktop(&pool, &b).await.expect("ler B"), None);
    identity::reset_desktop(&pool, &b).await.expect("B repõe");
    assert!(
        identity::get_desktop(&pool, &a)
            .await
            .expect("ler A")
            .is_some(),
        "repor de B apagou A"
    );
}
