//! A001 · A passagem de uma base anterior à D010 (0001–0059) para o esquema
//! corrente, numa base só desta prova: nada se perde, nada se duplica, e nada
//! se activa sozinho (ADR-0019).
//!
//! As migrações aplicam-se pelo mesmo `Migrator` do repositório, em duas
//! metades — até à 0059, os dados de uma Instância real, depois o resto.

use std::borrow::Cow;

use sqlx::migrate::Migrator;
use sqlx::PgPool;
use uuid::Uuid;

static MIGRATOR: Migrator = sqlx::migrate!("../../migrations");

/// A última migração anterior à D010.
const ANTES_DA_D010: i64 = 59;

async fn base_propria() -> Option<(PgPool, String, PgPool)> {
    let Ok(url) = std::env::var("OCINYE_TEST_DATABASE_URL") else {
        assert!(
            std::env::var("CI").is_err(),
            "OCINYE_TEST_DATABASE_URL em falta em CI: a passagem para a D010 ficaria por provar"
        );
        return None;
    };
    let admin = PgPool::connect(&url).await.expect("base de teste");
    let nome = format!("ocinye_d010_up_{}", Uuid::new_v4().simple());
    sqlx::query(&format!("CREATE DATABASE {nome}"))
        .execute(&admin)
        .await
        .expect("criar a base");
    let (base, consulta) = url
        .split_once('?')
        .map_or((url.as_str(), ""), |(b, q)| (b, q));
    let raiz = base.rsplit_once('/').map_or(base, |(raiz, _)| raiz);
    let alvo = if consulta.is_empty() {
        format!("{raiz}/{nome}")
    } else {
        format!("{raiz}/{nome}?{consulta}")
    };
    let pool = PgPool::connect(&alvo).await.expect("ligar");
    Some((admin, nome, pool))
}

#[tokio::test]
async fn uma_instancia_anterior_passa_a_d010_sem_perder_nem_duplicar() {
    let Some((admin, nome, pool)) = base_propria().await else {
        return;
    };

    // 1 · Até à 0059: o esquema de uma Instância instalada antes da D010.
    let ate = Migrator {
        migrations: Cow::Owned(
            MIGRATOR
                .iter()
                .filter(|m| m.version <= ANTES_DA_D010)
                .cloned()
                .collect(),
        ),
        ..Migrator::DEFAULT
    };
    ate.run(&pool).await.expect("migrações até à 0059");

    // 2 · Os dados dessa Instância: a organização que governa (Business),
    // outra organização da base, membros com e sem personalização.
    let gov = Uuid::new_v4();
    let outra = Uuid::new_v4();
    let a = Uuid::new_v4();
    let b = Uuid::new_v4();
    let c = Uuid::new_v4();
    for (id, slug, perfil) in [(gov, "gov", "business"), (outra, "outra", "personal")] {
        sqlx::query("INSERT INTO organisations (id, slug, name, profile) VALUES ($1, $2, $2, $3)")
            .bind(id)
            .bind(slug)
            .bind(perfil)
            .execute(&pool)
            .await
            .expect("organização");
    }
    sqlx::query("INSERT INTO instance_identity (organisation_id) VALUES ($1)")
        .bind(gov)
        .execute(&pool)
        .await
        .expect("instância");
    for (id, org) in [(a, gov), (b, gov), (c, outra)] {
        sqlx::query(
            "INSERT INTO people (id, organisation_id, full_name, email, status)
             VALUES ($1, $2, $3, $3 || '@exemplo.test', 'active')",
        )
        .bind(id)
        .bind(org)
        .bind(id.simple().to_string())
        .execute(&pool)
        .await
        .expect("pessoa");
    }
    let disposicao = serde_json::json!({
        "wallpaper": "module", "fit": "fill", "dim": 30,
        "widgets": [{"id": "tasks", "kind": "tasks", "w": 1, "h": 2, "minimized": false}]
    });
    sqlx::query(
        "INSERT INTO member_desktop_layouts (person_id, version, layout) VALUES ($1, 3, $2)",
    )
    .bind(a)
    .bind(&disposicao)
    .execute(&pool)
    .await
    .expect("disposição");
    for (id, pins) in [(a, vec!["files", "notes"]), (b, vec![])] {
        sqlx::query("INSERT INTO member_app_pins (person_id, pinned_app_ids) VALUES ($1, $2)")
            .bind(id)
            .bind(pins)
            .execute(&pool)
            .await
            .expect("fixações");
    }

    // 3 · O resto das migrações, como uma actualização as aplicaria.
    MIGRATOR
        .run(&pool)
        .await
        .expect("migrações D010 e seguintes");

    // Activada: só a Distribuição que cada organização tinha.
    let activadas: Vec<(Uuid, String)> = sqlx::query_as(
        "SELECT organisation_id, distribution FROM instance_distributions
          WHERE state = 'enabled' ORDER BY organisation_id, distribution",
    )
    .fetch_all(&pool)
    .await
    .expect("activadas");
    let mut esperadas = vec![(gov, "business".to_owned()), (outra, "personal".to_owned())];
    esperadas.sort();
    assert_eq!(activadas, esperadas, "activou-se a mais, ou a menos");

    // Acesso: cada membro à da sua organização, uma só linha.
    let acesso: Vec<(Uuid, String)> = sqlx::query_as(
        "SELECT person_id, distribution FROM member_distribution_access ORDER BY person_id",
    )
    .fetch_all(&pool)
    .await
    .expect("acesso");
    let mut esperado = vec![
        (a, "business".to_owned()),
        (b, "business".to_owned()),
        (c, "personal".to_owned()),
    ];
    esperado.sort();
    assert_eq!(acesso, esperado);

    // A disposição (com o fundo lá dentro) passou inteira para a Distribuição
    // que a Instância tinha, com a mesma versão; nenhuma cópia noutra.
    let linhas: Vec<(Uuid, String, i32, serde_json::Value)> = sqlx::query_as(
        "SELECT person_id, distribution, version, layout FROM member_desktop_layouts",
    )
    .fetch_all(&pool)
    .await
    .expect("disposições");
    assert_eq!(linhas, vec![(a, "business".to_owned(), 3, disposicao)]);

    // As fixações também — incluindo a lista vazia, que é uma escolha.
    let fixacoes: Vec<(Uuid, String, Vec<String>)> = sqlx::query_as(
        "SELECT person_id, distribution, pinned_app_ids FROM member_app_pins ORDER BY person_id",
    )
    .fetch_all(&pool)
    .await
    .expect("fixações");
    let mut esperadas = vec![
        (
            a,
            "business".to_owned(),
            vec!["files".to_owned(), "notes".to_owned()],
        ),
        (b, "business".to_owned(), Vec::new()),
    ];
    esperadas.sort();
    assert_eq!(fixacoes, esperadas);

    // Nada do que a D010 acrescenta nasce sozinho numa Instância existente.
    let pontos: i64 = sqlx::query_scalar("SELECT count(*) FROM access_endpoints")
        .fetch_one(&pool)
        .await
        .expect("pontos");
    assert_eq!(pontos, 0);

    pool.close().await;
    let _ = sqlx::query(&format!("DROP DATABASE IF EXISTS {nome} WITH (FORCE)"))
        .execute(&admin)
        .await;
}
