//! O manifesto de continuidade descreve-se sobre o **esquema real**.
//!
//! O portão estrutural [`toda_a_tabela_do_esquema_tem_uma_decisao`] prova que
//! cada tabela tem uma decisão de continuidade. Não prova que a decisão
//! `Identidades` se consegue **executar**: a enumeração faz `SELECT id FROM
//! {tabela}`, e uma tabela que viaje sem uma coluna `id` parte o `snapshot`, e
//! com ele o `institutional-backup` e o `verify-snapshot`. Foi o que aconteceu
//! quando a `file_favourites` nasceu com chave composta e sem `id`: o backup
//! deixou de conseguir produzir uma cópia verificável, e nenhum teste o via —
//! porque nenhum corria o manifesto de facto.

use sqlx::PgPool;

async fn pool() -> Option<PgPool> {
    let url = std::env::var("OCINYE_TEST_DATABASE_URL").ok()?;
    let pool = PgPool::connect(&url)
        .await
        .expect("OCINYE_TEST_DATABASE_URL definida mas a base não responde");
    sqlx::migrate!("../../migrations")
        .run(&pool)
        .await
        .expect("as migrações têm de aplicar à base de teste");
    Some(pool)
}

/// O manifesto corre sobre o esquema real, e toda a tabela que viaja se enumera.
#[tokio::test]
async fn o_manifesto_descreve_se_sobre_o_esquema_real() {
    let Some(pool) = pool().await else { return };

    let manifesto = ocinye_core::continuity::manifest::descrever(&pool)
        .await
        .expect(
            "o manifesto de continuidade tem de se descrever sobre o esquema real — \
             se falha aqui, o backup e o verify-snapshot falham em produção",
        );

    // A tabela que partiu a enumeração é a prova viva: se viaja, tem de aparecer
    // enumerada, e não com a favorita de um membro a ficar para trás.
    assert!(
        manifesto
            .familias
            .iter()
            .any(|familia| familia.tabela == "file_favourites"),
        "file_favourites não foi enumerada pelo manifesto"
    );
}
