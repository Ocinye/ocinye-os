//! Guarda de isolamento para harnesses que produzem fixtures.
//!
//! # A propriedade
//!
//! > **Um harness que produz fixtures recusa-se a escrever quando a base de
//! > destino contém a organização canónica da Ocinye — a não ser que se declare
//! > explicitamente um ensaio de restauro ou migração, que não usa o caminho
//! > normal de fixtures.**
//!
//! # Porque isto existe
//!
//! Porque a base de desenvolvimento ficou com **19 560 organizações** de
//! fixtures — `e2e-`, `cal-`, `msg`, `test-`, `iso-` e outras — e ~29 000
//! pessoas que nunca existiram. Entraram por duas portas, e nenhuma delas era
//! um defeito de código:
//!
//! - alguém apontou `OCINYE_TEST_DATABASE_URL` à base institucional;
//! - e o harness de browser adoptava «a organização mais recente» em vez de
//!   criar a sua, pelo que, apontado à base errada, escrevia **dentro da
//!   instituição real**.
//!
//! Nenhuma variável de ambiente protege contra a primeira. Um `_test` no nome
//! da base também não: é convenção, e uma convenção não recusa nada.
//!
//! # Porque o sinal é a organização, e não o nome da base
//!
//! Porque é o sinal que diz exactamente o que interessa: *estou prestes a
//! escrever dentro da instituição?* Nenhuma migration semeia organizações, pelo
//! que uma base de teste tem zero linhas com o slug canónico e a base
//! institucional tem uma. O nome da base é uma pista; a presença da instituição
//! é a coisa.
//!
//! # A saída deliberada
//!
//! `OCINYE_TEST_ALLOW_CANONICAL_ORG` existe para um ensaio de restauro ou de
//! migração sobre uma cópia da instituição — trabalho legítimo que **não** passa
//! pelo caminho normal de fixtures. Tem de ser declarada por quem a quer, e
//! aparece no que o teste imprime, para que nunca seja um silêncio.

/// O slug da organização institucional da Ocinye.
const ORGANIZACAO_CANONICA: &str = "ocinye";

/// A variável que declara um ensaio deliberado sobre uma cópia da instituição.
const ESCAPE_DECLARADO: &str = "OCINYE_TEST_ALLOW_CANONICAL_ORG";

/// Recusa-se a continuar se esta base contém a instituição.
///
/// Chamada pelos harnesses **antes da primeira escrita**. Falhar depois de
/// escrever não é uma guarda: é um relatório de estragos.
///
/// # Panics
///
/// Entra em pânico — falhando o teste — quando a base de destino contém a
/// organização canónica e nenhum ensaio foi declarado.
pub async fn refuse_canonical_organisation(pool: &sqlx::PgPool) {
    if std::env::var(ESCAPE_DECLARADO).is_ok() {
        eprintln!(
            "AVISO: {ESCAPE_DECLARADO} está definida. Este harness vai escrever \
             numa base que contém a organização canónica, por declaração \
             explícita de quem o corre."
        );
        return;
    }

    let canonica: i64 = sqlx::query_scalar("SELECT count(*) FROM organisations WHERE slug = $1")
        .bind(ORGANIZACAO_CANONICA)
        .fetch_one(pool)
        .await
        .unwrap_or(0);

    assert!(
        canonica == 0,
        "recusa antes da primeira escrita: a base de teste contém a organização \
         canónica «{ORGANIZACAO_CANONICA}».\n\
         \n\
         `OCINYE_TEST_DATABASE_URL` está a apontar para a base institucional. \
         Escrever fixtures aqui poluiria a instituição — foi assim que esta \
         árvore ganhou 19 560 organizações de teste e ~29 000 pessoas que nunca \
         existiram.\n\
         \n\
         Aponte `OCINYE_TEST_DATABASE_URL` para uma base de teste. Se isto é um \
         ensaio deliberado de restauro ou migração sobre uma cópia da \
         instituição, declare-o com {ESCAPE_DECLARADO}=1."
    );
}
