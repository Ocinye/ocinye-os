//! A autorização do plano realtime.
//!
//! # O que estes testes guardam
//!
//! Que conhecer um identificador nunca chega. É a diferença entre um sistema em
//! que uma conversa é privada e um em que ela é privada até alguém adivinhar um
//! UUID — e a diferença não se vê a olho: as duas versões compilam, respondem, e
//! parecem iguais na interface.
//!
//! E que a autoridade é **reestabelecida**. Uma pessoa removida de um grupo
//! deixa de o ouvir no mesmo instante, e não quando o socket dela se fechar.

use ocinye_core::realtime::events::Channel;
use ocinye_core_server::routes::realtime::pode_ouvir;
use sqlx::PgPool;
use uuid::Uuid;

/// Connect and migrate, or skip.
async fn pool() -> Option<PgPool> {
    let url = std::env::var("OCINYE_TEST_DATABASE_URL").ok()?;
    let pool = PgPool::connect(&url)
        .await
        .expect("OCINYE_TEST_DATABASE_URL está definida mas a base não responde");
    ocinye_core::db::migrate(&pool).await.expect("migrations");
    // Antes da primeira escrita, e não depois: falhar depois de escrever
    // não é uma guarda, é um relatório de estragos.
    ocinye_core::fixtures::refuse_canonical_organisation(&pool).await;
    Some(pool)
}

async fn organisation(pool: &PgPool) -> Uuid {
    let slug = format!("rt{}", Uuid::new_v4().simple());
    sqlx::query_scalar("INSERT INTO organisations (slug, name) VALUES ($1, $1) RETURNING id")
        .bind(&slug)
        .fetch_one(pool)
        .await
        .expect("organisation")
}

async fn pessoa(pool: &PgPool, organisation_id: Uuid) -> Uuid {
    let handle = format!("p{}", Uuid::new_v4().simple());
    sqlx::query_scalar(
        "INSERT INTO people (organisation_id, full_name, email, status)
              VALUES ($1, $2, $3, 'active') RETURNING id",
    )
    .bind(organisation_id)
    .bind(&handle)
    .bind(format!("{handle}@ocinye.com"))
    .fetch_one(pool)
    .await
    .expect("person")
}

async fn grupo(pool: &PgPool, organisation_id: Uuid, criador: Uuid) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO conversations (organisation_id, kind, name, created_by_id)
              VALUES ($1, 'group', $2, $3) RETURNING id",
    )
    .bind(organisation_id)
    .bind(format!(
        "Grupo {}",
        &Uuid::new_v4().simple().to_string()[..6]
    ))
    .bind(criador)
    .fetch_one(pool)
    .await
    .expect("conversa")
}

async fn juntar(pool: &PgPool, conversa: Uuid, quem: Uuid) {
    sqlx::query(
        "INSERT INTO conversation_participants (conversation_id, person_id)
              VALUES ($1, $2)",
    )
    .bind(conversa)
    .bind(quem)
    .execute(pool)
    .await
    .expect("participação");
}

#[tokio::test]
async fn conhecer_o_identificador_de_uma_conversa_nao_da_acesso_a_ela() {
    let Some(pool) = pool().await else { return };
    let org = organisation(&pool).await;
    let dentro = pessoa(&pool, org).await;
    let fora = pessoa(&pool, org).await;

    let conversa = grupo(&pool, org, dentro).await;
    juntar(&pool, conversa, dentro).await;

    let canal = Channel::Conversation { id: conversa };

    assert!(
        pode_ouvir(&pool, dentro, canal).await,
        "quem participa devia poder ouvir"
    );
    assert!(
        !pode_ouvir(&pool, fora, canal).await,
        "quem não participa ouviu uma conversa por conhecer o identificador"
    );
}

#[tokio::test]
async fn sair_de_um_grupo_retira_o_canal_no_mesmo_instante() {
    // «Identity may persist. Authority must be re-established.» Sem isto, uma
    // pessoa removida às 10h continuaria a receber a conversa às 18h — não por
    // defeito nenhum, mas porque nada voltou a perguntar.
    let Some(pool) = pool().await else { return };
    let org = organisation(&pool).await;
    let quem = pessoa(&pool, org).await;

    let conversa = grupo(&pool, org, quem).await;
    juntar(&pool, conversa, quem).await;
    let canal = Channel::Conversation { id: conversa };

    assert!(pode_ouvir(&pool, quem, canal).await);

    sqlx::query(
        "UPDATE conversation_participants SET left_at = now()
          WHERE conversation_id = $1 AND person_id = $2",
    )
    .bind(conversa)
    .bind(quem)
    .execute(&pool)
    .await
    .expect("remover");

    assert!(
        !pode_ouvir(&pool, quem, canal).await,
        "quem saiu do grupo continuou a poder ouvi-lo"
    );
}

#[tokio::test]
async fn o_canal_pessoal_e_de_uma_pessoa_so() {
    let Some(pool) = pool().await else { return };
    let org = organisation(&pool).await;
    let ana = pessoa(&pool, org).await;
    let outro = pessoa(&pool, org).await;

    assert!(pode_ouvir(&pool, ana, Channel::Person { id: ana }).await);
    assert!(
        !pode_ouvir(&pool, outro, Channel::Person { id: ana }).await,
        "o canal pessoal de alguém foi audível por outra pessoa"
    );
}

#[tokio::test]
async fn uma_conversa_que_nao_existe_nao_e_audivel() {
    // Não porque não exista — porque ninguém participa nela. A resposta é a
    // mesma para «não existe» e para «não é tua», e tem de ser: distingui-las
    // diria a quem adivinha identificadores qual deles acertou.
    let Some(pool) = pool().await else { return };
    let org = organisation(&pool).await;
    let quem = pessoa(&pool, org).await;

    assert!(!pode_ouvir(&pool, quem, Channel::Conversation { id: Uuid::new_v4() }).await);
}

#[tokio::test]
async fn a_participacao_de_uma_conversa_nao_serve_outra() {
    let Some(pool) = pool().await else { return };
    let org = organisation(&pool).await;
    let quem = pessoa(&pool, org).await;

    let aqui = grupo(&pool, org, quem).await;
    let ali = grupo(&pool, org, quem).await;
    juntar(&pool, aqui, quem).await;

    assert!(pode_ouvir(&pool, quem, Channel::Conversation { id: aqui }).await);
    assert!(
        !pode_ouvir(&pool, quem, Channel::Conversation { id: ali }).await,
        "pertencer a um grupo deu acesso a outro"
    );
}
