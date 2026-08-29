//! O que o calendário deixa ver, e a quem.
//!
//! # Porque esta suite existe
//!
//! Um calendário é uma lista de onde as pessoas estão e do que estão a fazer. É
//! das superfícies mais indiscretas que uma instituição tem: o título de uma
//! reunião diz com quem se fala, a hora diz quando, e a existência diz que há
//! trabalho a acontecer. Cada teste aqui é uma maneira de o descobrir sem
//! autorização, e a asserção é que não resulta.
//!
//! Salta quando `OCINYE_TEST_DATABASE_URL` não está definida; **falha** quando
//! está e a base não responde.

use chrono::{Duration, NaiveDate, TimeZone, Utc};
use ocinye_contracts::calendar::EventScope;
use ocinye_contracts::temporal::{resolve_local, LocalTimeProblem, Occurrence, TimeZoneName};
use ocinye_contracts::{Classification, TechnicalRole, UnitRole};
use ocinye_core::modules::calendar::{self, NewEvent, TimeRange};
use ocinye_core::modules::{identity, organisation, research};
use ocinye_core::realtime::Realtime;
use ocinye_core::CoreError;
use ocinye_domain::Principal;
use ocinye_observability::CorrelationIds;
use sqlx::PgPool;
use uuid::Uuid;

// ── Fixture ─────────────────────────────────────────────────────────────

async fn pool() -> Option<PgPool> {
    let url = std::env::var("OCINYE_TEST_DATABASE_URL").ok()?;
    let pool = PgPool::connect(&url)
        .await
        .expect("OCINYE_TEST_DATABASE_URL is set but the database is unreachable");
    sqlx::migrate!("../../migrations")
        .run(&pool)
        .await
        .expect("migrations must apply to the test database");
    Some(pool)
}

macro_rules! base {
    () => {
        match pool().await {
            Some(pool) => pool,
            None => {
                eprintln!("skipping: OCINYE_TEST_DATABASE_URL is not set");
                return;
            }
        }
    };
}

fn ids() -> CorrelationIds {
    CorrelationIds::generate()
}

async fn organizacao(pool: &PgPool) -> Uuid {
    sqlx::query_scalar("INSERT INTO organisations (slug, name) VALUES ($1, $2) RETURNING id")
        .bind(format!("cal-{}", Uuid::new_v4().simple()))
        .bind("Instituição do calendário")
        .fetch_one(pool)
        .await
        .expect("organização")
}

async fn pessoa(pool: &PgPool, organisation_id: Uuid, roles: &[TechnicalRole]) -> Principal {
    let handle = format!("c{}", Uuid::new_v4().simple());
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

    for role in roles {
        sqlx::query("INSERT INTO person_roles (person_id, role) VALUES ($1, $2)")
            .bind(person_id)
            .bind(role.as_str())
            .execute(pool)
            .await
            .expect("papel");
    }

    refrescar(pool, person_id).await
}

/// Relê o principal. A pertença muda o que uma pessoa pode fazer, e um principal
/// em cache continuaria a dizer que não pode.
async fn refrescar(pool: &PgPool, person_id: Uuid) -> Principal {
    let record = identity::person_by_id(pool, person_id)
        .await
        .expect("consulta")
        .expect("pessoa");
    identity::principal_for_person(pool, &record)
        .await
        .expect("principal")
}

async fn unidade(pool: &PgPool, admin: &Principal) -> Uuid {
    let mut tx = pool.begin().await.expect("tx");
    let unidade = organisation::create_unit(
        &mut tx,
        admin,
        &ids(),
        organisation::NewUnit {
            code: format!("C{}", &Uuid::new_v4().simple().to_string()[..5]),
            name: "Unidade".to_owned(),
            description: None,
            research_areas: Vec::new(),
        },
    )
    .await
    .expect("unidade");
    tx.commit().await.expect("commit");
    unidade.id
}

async fn membro_gestor(pool: &PgPool, admin: &Principal, unit_id: Uuid, person_id: Uuid) {
    let mut tx = pool.begin().await.expect("tx");
    organisation::add_unit_member(
        &mut tx,
        admin,
        &ids(),
        unit_id,
        person_id,
        UnitRole::Manager,
    )
    .await
    .expect("membro");
    tx.commit().await.expect("commit");
}

async fn workspace(pool: &PgPool, actor: &Principal, unit_id: Uuid) -> Uuid {
    let mut principal = actor.clone();
    let mut tx = pool.begin().await.expect("tx");
    let (_, workspace) = research::create_idea(
        &mut tx,
        &mut principal,
        &ids(),
        research::NewIdea {
            unit_id,
            title: "Ideia".to_owned(),
            summary: None,
            research_question: None,
            hypothesis: None,
            motivation: None,
            keywords: Vec::new(),
            classification: None,
        },
    )
    .await
    .expect("ideia");
    tx.commit().await.expect("commit");
    workspace.id
}

/// Um evento com hora, daqui a uma hora.
fn daqui_a_uma_hora() -> Occurrence {
    let inicio = Utc::now() + Duration::hours(1);
    Occurrence::Timed {
        starts_at: inicio,
        ends_at: inicio + Duration::hours(1),
        timezone: TimeZoneName::parse("Europe/Lisbon").expect("zona"),
    }
}

fn hoje() -> TimeRange {
    TimeRange {
        start: Utc::now() - Duration::days(1),
        end: Utc::now() + Duration::days(1),
    }
}

async fn marcar(pool: &PgPool, actor: &Principal, pedido: NewEvent) -> Uuid {
    let mut tx = pool.begin().await.expect("tx");
    let evento = calendar::create_event(&mut tx, actor, &ids(), pedido)
        .await
        .expect("evento");
    tx.commit().await.expect("commit");
    evento.id
}

async fn ve(pool: &PgPool, actor: &Principal, event_id: Uuid) -> bool {
    let na_agenda = calendar::agenda(pool, actor, hoje(), 200)
        .await
        .expect("agenda")
        .iter()
        .any(|item| item.id == event_id);

    // A superfície endereçada tem de concordar com a agregada. Se divergissem,
    // esconder na agenda e revelar por identificador seria a mesma fuga com dois
    // nomes.
    let por_identificador = calendar::get_event(pool, actor, event_id).await.is_ok();

    assert_eq!(
        na_agenda, por_identificador,
        "a agenda e a leitura por identificador discordam sobre o evento {event_id}"
    );
    na_agenda
}

// ── 1 · Titularidade ────────────────────────────────────────────────────

/// A agenda pessoal de alguém é dessa pessoa.
#[tokio::test]
async fn um_evento_pessoal_e_invisivel_para_outro_membro() {
    let pool = base!();
    let org = organizacao(&pool).await;
    let alice = pessoa(&pool, org, &[TechnicalRole::ResearchMember]).await;
    let bruno = pessoa(&pool, org, &[TechnicalRole::ResearchMember]).await;

    let evento = marcar(
        &pool,
        &alice,
        NewEvent {
            scope: EventScope::Personal,
            unit_id: None,
            workspace_id: None,
            title: "Consulta médica".to_owned(),
            description: None,
            location: None,
            occurrence: daqui_a_uma_hora(),
            classification: None,
            participants: Vec::new(),
        },
    )
    .await;

    assert!(
        ve(&pool, &alice, evento).await,
        "a dona não vê o seu evento"
    );
    assert!(
        !ve(&pool, &bruno, evento).await,
        "outro membro vê a agenda pessoal da Alice"
    );
}

/// O dono é derivado do actor, e não pedido a quem chama.
#[tokio::test]
async fn o_dono_de_um_evento_pessoal_e_sempre_quem_o_marca() {
    let pool = base!();
    let org = organizacao(&pool).await;
    let alice = pessoa(&pool, org, &[TechnicalRole::ResearchMember]).await;

    let evento = marcar(
        &pool,
        &alice,
        NewEvent {
            scope: EventScope::Personal,
            unit_id: None,
            workspace_id: None,
            title: "Pessoal".to_owned(),
            description: None,
            location: None,
            occurrence: daqui_a_uma_hora(),
            classification: None,
            participants: Vec::new(),
        },
    )
    .await;

    let dono: Option<Uuid> =
        sqlx::query_scalar("SELECT owner_id FROM calendar_events WHERE id = $1")
            .bind(evento)
            .fetch_one(&pool)
            .await
            .expect("dono");

    assert_eq!(
        dono,
        Some(alice.person_id),
        "o dono não é quem marcou — e `NewEvent` não tem campo para o escolher"
    );
}

/// Privilégio técnico não abre a agenda pessoal de ninguém.
///
/// # Porque isto é um teste e não uma nota
///
/// Porque é a regra que a próxima pessoa a escrever uma consulta vai
/// inconscientemente quebrar. Um administrador de plataforma alcança quase tudo,
/// e «quase» é a palavra que este teste protege. Se um dia existir acesso
/// deliberado à agenda de outra pessoa, ele nasce como concessão explícita — não
/// por acidente de privilégio.
#[tokio::test]
async fn privilegio_tecnico_nao_abre_a_agenda_pessoal() {
    let pool = base!();
    let org = organizacao(&pool).await;
    let alice = pessoa(&pool, org, &[TechnicalRole::ResearchMember]).await;
    let admin = pessoa(&pool, org, &[TechnicalRole::PlatformAdmin]).await;
    let org_admin = pessoa(&pool, org, &[TechnicalRole::OrganisationAdmin]).await;
    let auditor = pessoa(&pool, org, &[TechnicalRole::Auditor]).await;

    let evento = marcar(
        &pool,
        &alice,
        NewEvent {
            scope: EventScope::Personal,
            unit_id: None,
            workspace_id: None,
            title: "Entrevista".to_owned(),
            description: None,
            location: None,
            occurrence: daqui_a_uma_hora(),
            classification: None,
            participants: Vec::new(),
        },
    )
    .await;

    // Controlo positivo **sobre o mesmo evento**, e não sobre outro.
    //
    // Sem isto, este teste passava num mundo onde ninguém vê eventos pessoais —
    // incluindo a dona. Descobri-o por reversão: troquei a titularidade pela
    // classificação, o evento pessoal deixou de ser visível para toda a gente, e
    // esta asserção continuou verde. «Não vê» só significa alguma coisa quando
    // há alguém que vê.
    assert!(
        ve(&pool, &alice, evento).await,
        "a dona não vê o seu próprio evento: o teste não está a medir titularidade"
    );

    for (quem, papel) in [
        (&admin, "PlatformAdmin"),
        (&org_admin, "OrganisationAdmin"),
        (&auditor, "Auditor"),
    ] {
        assert!(
            !ve(&pool, quem, evento).await,
            "{papel} alcança a agenda pessoal da Alice"
        );
    }

    // Controlo positivo: o mesmo administrador **vê** um evento institucional.
    // Sem isto, o teste passaria se o administrador não visse coisa nenhuma —
    // por a fixture estar partida, e não por a regra funcionar.
    let institucional = marcar(
        &pool,
        &admin,
        NewEvent {
            scope: EventScope::Institution,
            unit_id: None,
            workspace_id: None,
            title: "Assembleia".to_owned(),
            description: None,
            location: None,
            occurrence: daqui_a_uma_hora(),
            classification: Some(Classification::Internal),
            participants: Vec::new(),
        },
    )
    .await;
    assert!(
        ve(&pool, &admin, institucional).await,
        "o administrador não vê sequer um evento institucional: a fixture está partida"
    );
}

// ── 2 · Contentor ───────────────────────────────────────────────────────

/// Um evento de workspace exige o artefacto **e** o contentor.
///
/// # Porque o evento aqui é `INTERNAL`, e não `RESTRICTED`
///
/// Porque com `RESTRICTED` este teste passava sem provar nada. A cláusula do
/// artefacto já negava o evento ao forasteiro sozinha, e a metade do contentor
/// nunca chegava a ser exercitada — descobri-o ao remover
/// `contained_in_visible_workspace` e ver a suite continuar verde.
///
/// Um evento `INTERNAL` é legível por qualquer membro activo. Se ele estiver
/// dentro de um workspace `RESTRICTED` que o actor não alcança, só a contenção o
/// pode esconder. É essa metade que este teste mede, e é a fuga F-01 aplicada ao
/// calendário: um evento legível dentro de um workspace inalcançável revelaria
/// que há trabalho onde o actor não entra.
#[tokio::test]
async fn um_evento_de_workspace_inalcancavel_nao_aparece() {
    let pool = base!();
    let org = organizacao(&pool).await;
    let admin = pessoa(&pool, org, &[TechnicalRole::PlatformAdmin]).await;
    let unidade = unidade(&pool, &admin).await;

    let dentro = pessoa(&pool, org, &[TechnicalRole::ResearchMember]).await;
    membro_gestor(&pool, &admin, unidade, dentro.person_id).await;
    let dentro = refrescar(&pool, dentro.person_id).await;
    let workspace = workspace(&pool, &dentro, unidade).await;
    let dentro = refrescar(&pool, dentro.person_id).await;

    // O contentor fecha-se. Sem isto o workspace seria `INTERNAL` e visível a
    // qualquer membro activo — e a contenção não teria nada que esconder.
    let mut tx = pool.begin().await.expect("tx");
    research::reclassify_workspace(
        &mut tx,
        &dentro,
        &ids(),
        workspace,
        Classification::Restricted,
        "fecho para o teste de contenção",
    )
    .await
    .expect("reclassificar");
    tx.commit().await.expect("commit");
    let dentro = refrescar(&pool, dentro.person_id).await;

    let forasteiro = pessoa(&pool, org, &[TechnicalRole::ResearchMember]).await;

    // O evento é `INTERNAL`: a sua própria classificação **não** o esconde.
    let evento = marcar(
        &pool,
        &dentro,
        NewEvent {
            scope: EventScope::ResearchWorkspace,
            unit_id: None,
            workspace_id: Some(workspace),
            title: "Revisão de protocolo".to_owned(),
            description: None,
            location: None,
            occurrence: daqui_a_uma_hora(),
            classification: Some(Classification::Internal),
            participants: Vec::new(),
        },
    )
    .await;

    let classificacao: String =
        sqlx::query_scalar("SELECT classification FROM calendar_events WHERE id = $1")
            .bind(evento)
            .fetch_one(&pool)
            .await
            .expect("classificação");
    assert_eq!(
        classificacao, "INTERNAL",
        "o evento não ficou INTERNAL: o teste voltaria a medir a classificação em vez da contenção"
    );

    assert!(ve(&pool, &dentro, evento).await, "quem está dentro não vê");
    assert!(
        !ve(&pool, &forasteiro, evento).await,
        "um evento INTERNAL de um workspace inalcançável aparece a quem não entra nele: \
         a contenção não está a ser aplicada"
    );
}

/// Dentro de um contentor alcançável, a classificação continua a decidir.
#[tokio::test]
async fn a_classificacao_decide_dentro_de_um_contentor_alcancavel() {
    let pool = base!();
    let org = organizacao(&pool).await;
    let admin = pessoa(&pool, org, &[TechnicalRole::PlatformAdmin]).await;
    let unidade = unidade(&pool, &admin).await;

    let gestor = pessoa(&pool, org, &[TechnicalRole::ResearchMember]).await;
    membro_gestor(&pool, &admin, unidade, gestor.person_id).await;
    let gestor = refrescar(&pool, gestor.person_id).await;

    // Membro da instituição, mas não da unidade.
    let de_fora = pessoa(&pool, org, &[TechnicalRole::ResearchMember]).await;

    let interno = marcar(
        &pool,
        &gestor,
        NewEvent {
            scope: EventScope::Unit,
            unit_id: Some(unidade),
            workspace_id: None,
            title: "Seminário aberto".to_owned(),
            description: None,
            location: None,
            occurrence: daqui_a_uma_hora(),
            classification: Some(Classification::Internal),
            participants: Vec::new(),
        },
    )
    .await;

    let restrito = marcar(
        &pool,
        &gestor,
        NewEvent {
            scope: EventScope::Unit,
            unit_id: Some(unidade),
            workspace_id: None,
            title: "Reunião de direcção".to_owned(),
            description: None,
            location: None,
            occurrence: daqui_a_uma_hora(),
            classification: Some(Classification::Restricted),
            participants: Vec::new(),
        },
    )
    .await;

    // `INTERNAL` é legível por qualquer membro activo — é a regra de ADR-0100, e
    // vale aqui como vale para datasets. Não é uma fuga; é a política.
    assert!(
        ve(&pool, &de_fora, interno).await,
        "um evento INTERNAL de unidade devia ser legível por um membro activo"
    );
    assert!(
        !ve(&pool, &de_fora, restrito).await,
        "um evento RESTRICTED de unidade aparece a quem não gere a unidade"
    );
}

// ── 3 · Organização ─────────────────────────────────────────────────────

/// Outra organização não existe.
#[tokio::test]
async fn outra_organizacao_nunca_aparece_nem_muta() {
    let pool = base!();
    let org_a = organizacao(&pool).await;
    let org_b = organizacao(&pool).await;

    let dentro = pessoa(&pool, org_a, &[TechnicalRole::PlatformAdmin]).await;
    let estranho = pessoa(&pool, org_b, &[TechnicalRole::PlatformAdmin]).await;

    let evento = marcar(
        &pool,
        &dentro,
        NewEvent {
            scope: EventScope::Institution,
            unit_id: None,
            workspace_id: None,
            title: "Assembleia".to_owned(),
            description: None,
            location: None,
            occurrence: daqui_a_uma_hora(),
            classification: Some(Classification::Internal),
            participants: Vec::new(),
        },
    )
    .await;

    assert!(ve(&pool, &dentro, evento).await, "controlo positivo falhou");
    assert!(
        !ve(&pool, &estranho, evento).await,
        "um administrador de outra organização lê o calendário desta"
    );

    let mut tx = pool.begin().await.expect("tx");
    let cancelamento = calendar::cancel_event(&mut tx, &estranho, &ids(), evento).await;
    assert!(
        cancelamento.is_err(),
        "um administrador de outra organização cancelou um evento desta"
    );
}

/// Conhecer o identificador não é ser autorizado.
#[tokio::test]
async fn o_identificador_nao_concede_autoridade() {
    let pool = base!();
    let org = organizacao(&pool).await;
    let alice = pessoa(&pool, org, &[TechnicalRole::ResearchMember]).await;
    let bruno = pessoa(&pool, org, &[TechnicalRole::ResearchMember]).await;

    let evento = marcar(
        &pool,
        &alice,
        NewEvent {
            scope: EventScope::Personal,
            unit_id: None,
            workspace_id: None,
            title: "Pessoal".to_owned(),
            description: None,
            location: None,
            occurrence: daqui_a_uma_hora(),
            classification: None,
            participants: Vec::new(),
        },
    )
    .await;

    // Controlo positivo primeiro: a dona alcança-o pelo identificador. Sem isto,
    // um estado em que ninguém lê nada satisfaria as asserções abaixo.
    assert!(
        calendar::get_event(&pool, &alice, evento).await.is_ok(),
        "a dona não lê o seu próprio evento: o teste não está a medir autoridade"
    );

    // O identificador exacto, escrito à mão.
    let leitura = calendar::get_event(&pool, &bruno, evento).await;
    assert!(
        matches!(leitura, Err(CoreError::NotFound(_))),
        "o identificador exacto revelou um evento alheio: {leitura:?}"
    );

    let mut tx = pool.begin().await.expect("tx");
    let cancelamento = calendar::cancel_event(&mut tx, &bruno, &ids(), evento).await;
    assert!(
        cancelamento.is_err(),
        "o identificador exacto permitiu cancelar um evento alheio"
    );

    // Um identificador inventado dá a mesma resposta que um alheio. Se dessem
    // respostas diferentes, a diferença seria um oráculo de existência.
    let inventado = calendar::get_event(&pool, &bruno, Uuid::new_v4()).await;
    assert_eq!(
        format!("{leitura:?}"),
        format!("{inventado:?}"),
        "«existe mas não é seu» e «não existe» dão respostas distinguíveis"
    );
}

// ── 4 · Tempo ───────────────────────────────────────────────────────────

/// Uma zona que não existe é recusada onde entra.
#[tokio::test]
async fn uma_zona_invalida_e_recusada() {
    assert!(TimeZoneName::parse("Europe/Paris").is_ok());
    for invalida in ["Europa/Paris", "GMT+1", "", "Mars/Olympus"] {
        assert!(
            TimeZoneName::parse(invalida).is_err(),
            "«{invalida}» foi aceite como zona horária"
        );
    }
}

/// As horas das transições produzem erro tipado, e não uma escolha em silêncio.
#[tokio::test]
async fn hora_inexistente_e_ambigua_dao_erro_tipado() {
    let paris = TimeZoneName::parse("Europe/Paris").expect("zona");

    // 2026-03-29: o relógio salta das 02:00 para as 03:00 em Paris.
    let inexistente = NaiveDate::from_ymd_opt(2026, 3, 29)
        .expect("data")
        .and_hms_opt(2, 30, 0)
        .expect("hora");
    assert_eq!(
        resolve_local(inexistente, paris),
        Err(LocalTimeProblem::DoesNotExist),
        "uma hora que não existe foi convertida à mesma"
    );

    // 2026-10-25: as 02:30 acontecem duas vezes.
    let ambigua = NaiveDate::from_ymd_opt(2026, 10, 25)
        .expect("data")
        .and_hms_opt(2, 30, 0)
        .expect("hora");
    assert_eq!(
        resolve_local(ambigua, paris),
        Err(LocalTimeProblem::Ambiguous),
        "uma hora ambígua foi resolvida sem dizer qual"
    );

    // Controlo positivo: uma hora normal converte-se.
    let normal = NaiveDate::from_ymd_opt(2026, 6, 15)
        .expect("data")
        .and_hms_opt(14, 0, 0)
        .expect("hora");
    let instante = resolve_local(normal, paris).expect("14:00 de Junho existe em Paris");
    assert_eq!(
        instante,
        Utc.with_ymd_and_hms(2026, 6, 15, 12, 0, 0).unwrap(),
        "14:00 em Paris em Junho são 12:00 UTC"
    );
}

// ── 5 · Coerência entre superfícies ─────────────────────────────────────

/// O universo autorizado não muda com o intervalo pedido.
///
/// # A propriedade
///
/// > **A apresentação de um item temporal pode diferir entre o Centro Temporal,
/// > Hoje, Semana, Mês e Agenda; a sua autorização não.**
///
/// As cinco superfícies chamam `agenda()` com intervalos diferentes. Este teste
/// pergunta a mesma coisa com os cinco intervalos e exige a mesma resposta sobre
/// **pertença ao universo visível** — não sobre o que cada vista mostra.
#[tokio::test]
async fn as_superficies_concordam_sobre_o_universo_visivel() {
    let pool = base!();
    let org = organizacao(&pool).await;
    let admin = pessoa(&pool, org, &[TechnicalRole::PlatformAdmin]).await;
    let unidade = unidade(&pool, &admin).await;

    let dentro = pessoa(&pool, org, &[TechnicalRole::ResearchMember]).await;
    membro_gestor(&pool, &admin, unidade, dentro.person_id).await;
    let dentro = refrescar(&pool, dentro.person_id).await;
    let forasteiro = pessoa(&pool, org, &[TechnicalRole::ResearchMember]).await;

    let restrito = marcar(
        &pool,
        &dentro,
        NewEvent {
            scope: EventScope::Unit,
            unit_id: Some(unidade),
            workspace_id: None,
            title: "Direcção".to_owned(),
            description: None,
            location: None,
            occurrence: daqui_a_uma_hora(),
            classification: Some(Classification::Restricted),
            participants: Vec::new(),
        },
    )
    .await;

    let agora = Utc::now();
    let intervalos = [
        ("centro temporal", Duration::hours(6)),
        ("hoje", Duration::days(1)),
        ("semana", Duration::days(7)),
        ("mês", Duration::days(31)),
        ("agenda", Duration::days(90)),
    ];

    for (nome, largura) in intervalos {
        let range = TimeRange {
            start: agora - largura,
            end: agora + largura,
        };

        let visto_por_dentro = calendar::agenda(&pool, &dentro, range, 500)
            .await
            .expect("agenda")
            .iter()
            .any(|item| item.id == restrito);
        let visto_de_fora = calendar::agenda(&pool, &forasteiro, range, 500)
            .await
            .expect("agenda")
            .iter()
            .any(|item| item.id == restrito);

        assert!(
            visto_por_dentro,
            "«{nome}» esconde o evento a quem o pode ver"
        );
        assert!(
            !visto_de_fora,
            "«{nome}» revela um evento RESTRICTED a quem não gere a unidade"
        );
    }
}

/// A contagem e a lista respondem sobre o mesmo conjunto.
#[tokio::test]
async fn a_contagem_concorda_com_a_agenda() {
    let pool = base!();
    let org = organizacao(&pool).await;
    let admin = pessoa(&pool, org, &[TechnicalRole::PlatformAdmin]).await;
    let unidade = unidade(&pool, &admin).await;

    let gestor = pessoa(&pool, org, &[TechnicalRole::ResearchMember]).await;
    membro_gestor(&pool, &admin, unidade, gestor.person_id).await;
    let gestor = refrescar(&pool, gestor.person_id).await;
    let forasteiro = pessoa(&pool, org, &[TechnicalRole::ResearchMember]).await;

    for indice in 0..5 {
        marcar(
            &pool,
            &gestor,
            NewEvent {
                scope: EventScope::Unit,
                unit_id: Some(unidade),
                workspace_id: None,
                title: format!("Reunião {indice}"),
                description: None,
                location: None,
                occurrence: daqui_a_uma_hora(),
                classification: Some(Classification::Restricted),
                participants: Vec::new(),
            },
        )
        .await;
    }

    for (quem, nome) in [(&gestor, "gestor"), (&forasteiro, "forasteiro")] {
        let lista = calendar::agenda(&pool, quem, hoje(), 500)
            .await
            .expect("agenda")
            .len();
        let contagem = calendar::agenda_count(&pool, quem, hoje())
            .await
            .expect("contagem");
        assert_eq!(
            i64::try_from(lista).expect("cabe"),
            contagem,
            "para «{nome}», a lista e a contagem discordam"
        );
    }
}

/// Uma consulta que falha não é uma agenda vazia.
///
/// # Porque isto tem de ser afirmado
///
/// Porque a maneira mais fácil de esconder uma avaria é devolver `Ok(vec![])`. A
/// pessoa vê «não tem nada marcado», acredita, e falta à reunião. Um erro tem de
/// chegar como erro até acima.
#[tokio::test]
async fn uma_consulta_falhada_nao_e_uma_agenda_vazia() {
    let pool = base!();
    let org = organizacao(&pool).await;
    let alguem = pessoa(&pool, org, &[TechnicalRole::ResearchMember]).await;

    // Um intervalo válido devolve `Ok` — controlo positivo.
    assert!(calendar::agenda(&pool, &alguem, hoje(), 10).await.is_ok());

    // Uma base fechada devolve `Err`, e não uma lista vazia.
    let fechada =
        PgPool::connect_lazy("postgres://ninguem@127.0.0.1:1/nao-existe").expect("pool preguiçosa");
    let resultado = calendar::agenda(&fechada, &alguem, hoje(), 10).await;
    assert!(
        resultado.is_err(),
        "uma base inalcançável devolveu uma agenda em vez de um erro"
    );
}

// ── 6 · Mutações ────────────────────────────────────────────────────────

/// A alteração não sabe mudar autoridade.
///
/// # Porque este teste é estrutural
///
/// Porque a defesa não é uma validação: é a ausência do campo. `EventEdit` não
/// tem `owner_id`, `scope`, `unit_id`, `workspace_id` nem `classification`, e o
/// `UPDATE` não menciona essas colunas. Este teste confirma que continua assim
/// depois de alguém mexer — altera tudo o que se pode alterar, e verifica que
/// nada de estrutural se moveu.
#[tokio::test]
async fn alterar_um_evento_nao_lhe_muda_a_autoridade() {
    let pool = base!();
    let org = organizacao(&pool).await;
    let alice = pessoa(&pool, org, &[TechnicalRole::ResearchMember]).await;
    let bruno = pessoa(&pool, org, &[TechnicalRole::ResearchMember]).await;

    let evento = marcar(
        &pool,
        &alice,
        NewEvent {
            scope: EventScope::Personal,
            unit_id: None,
            workspace_id: None,
            title: "Antes".to_owned(),
            description: None,
            location: None,
            occurrence: daqui_a_uma_hora(),
            classification: None,
            participants: Vec::new(),
        },
    )
    .await;

    let antes: (String, Option<Uuid>, Option<Uuid>, Option<Uuid>, String) = sqlx::query_as(
        "SELECT scope, owner_id, unit_id, workspace_id, classification
           FROM calendar_events WHERE id = $1",
    )
    .bind(evento)
    .fetch_one(&pool)
    .await
    .expect("antes");

    let mut tx = pool.begin().await.expect("tx");
    calendar::update_event(
        &mut tx,
        &alice,
        &ids(),
        evento,
        calendar::EventEdit {
            title: Some("Depois".to_owned()),
            description: Some(Some("nova".to_owned())),
            location: Some(Some("sala 2".to_owned())),
            occurrence: Some(Occurrence::AllDay {
                starts_on: NaiveDate::from_ymd_opt(2026, 9, 1).expect("data"),
                ends_before: NaiveDate::from_ymd_opt(2026, 9, 2).expect("data"),
            }),
        },
    )
    .await
    .expect("alterar");
    tx.commit().await.expect("commit");

    let depois: (String, Option<Uuid>, Option<Uuid>, Option<Uuid>, String) = sqlx::query_as(
        "SELECT scope, owner_id, unit_id, workspace_id, classification
           FROM calendar_events WHERE id = $1",
    )
    .bind(evento)
    .fetch_one(&pool)
    .await
    .expect("depois");

    assert_eq!(
        antes, depois,
        "uma alteração mexeu em âmbito, dono, contentor ou classificação"
    );

    // Controlo positivo: o que **é** alterável mudou mesmo. Sem isto, a
    // igualdade acima passaria se a operação não tivesse feito nada.
    let titulo: String = sqlx::query_scalar("SELECT title FROM calendar_events WHERE id = $1")
        .bind(evento)
        .fetch_one(&pool)
        .await
        .expect("título");
    assert_eq!(titulo, "Depois", "a alteração não alterou nada");

    // E continua a ser da Alice, não do Bruno.
    assert!(!ve(&pool, &bruno, evento).await);
}

/// Cancelar é uma transição, e repetir não dói.
#[tokio::test]
async fn cancelar_e_idempotente_e_nao_apaga() {
    let pool = base!();
    let org = organizacao(&pool).await;
    let alice = pessoa(&pool, org, &[TechnicalRole::ResearchMember]).await;

    let evento = marcar(
        &pool,
        &alice,
        NewEvent {
            scope: EventScope::Personal,
            unit_id: None,
            workspace_id: None,
            title: "A cancelar".to_owned(),
            description: None,
            location: None,
            occurrence: daqui_a_uma_hora(),
            classification: None,
            participants: Vec::new(),
        },
    )
    .await;

    for tentativa in 1..=3 {
        let mut tx = pool.begin().await.expect("tx");
        let resultado = calendar::cancel_event(&mut tx, &alice, &ids(), evento).await;
        tx.commit().await.expect("commit");
        assert!(
            resultado.is_ok(),
            "cancelar à {tentativa}.ª vez devolveu erro: {resultado:?}"
        );
    }

    let (estado, existe): (String, i64) =
        sqlx::query_as("SELECT state, COUNT(*) OVER () FROM calendar_events WHERE id = $1")
            .bind(evento)
            .fetch_one(&pool)
            .await
            .expect("estado");

    assert_eq!(estado, "cancelled");
    assert_eq!(existe, 1, "cancelar apagou a linha em vez de a transitar");

    // O evento cancelado continua visível para quem o esperava. Desaparecer não
    // avisaria ninguém.
    assert!(ve(&pool, &alice, evento).await);
}

/// Perder a pertença tira o acesso.
///
/// # O que este teste prova, e o que não prova
///
/// Prova que a **política** está certa: revogada a pertença, o mesmo actor deixa
/// de poder alterar o evento — e que o **estado do evento** é relido dentro da
/// transacção, imediatamente antes da escrita.
///
/// Não prova que o Core resista a um `Principal` obsoleto, porque não resiste, e
/// nenhum módulo deste repositório resiste: a autorização corre contra o
/// `Principal` que lhe entregam. A frescura é contrato de quem chama, e o
/// extractor HTTP cumpre-o — carrega o principal a cada pedido
/// (`services/core-server/src/extract.rs`).
///
/// Descobri isto ao escrever este teste com um principal em cache: a alteração
/// passou. Deixo-o escrito assim, com o `refrescar` explícito, porque esconder a
/// diferença faria parecer que há uma defesa onde só há uma convenção.
#[tokio::test]
async fn a_autorizacao_segue_a_pertenca_actual() {
    let pool = base!();
    let org = organizacao(&pool).await;
    let admin = pessoa(&pool, org, &[TechnicalRole::PlatformAdmin]).await;
    let unidade = unidade(&pool, &admin).await;

    let gestor = pessoa(&pool, org, &[TechnicalRole::ResearchMember]).await;
    membro_gestor(&pool, &admin, unidade, gestor.person_id).await;
    let gestor = refrescar(&pool, gestor.person_id).await;

    let evento = marcar(
        &pool,
        &gestor,
        NewEvent {
            scope: EventScope::Unit,
            unit_id: Some(unidade),
            workspace_id: None,
            title: "Reunião".to_owned(),
            description: None,
            location: None,
            occurrence: daqui_a_uma_hora(),
            classification: Some(Classification::Restricted),
            participants: Vec::new(),
        },
    )
    .await;

    // Controlo positivo: com a pertença, altera.
    let mut tx = pool.begin().await.expect("tx");
    assert!(
        calendar::update_event(
            &mut tx,
            &gestor,
            &ids(),
            evento,
            calendar::EventEdit {
                title: Some("Ainda posso".to_owned()),
                ..calendar::EventEdit::default()
            }
        )
        .await
        .is_ok(),
        "o gestor não conseguiu alterar o evento da sua unidade"
    );
    tx.commit().await.expect("commit");

    // A pertença é revogada. O `principal` em mão continua a dizer que ele é
    // gestor — é exactamente o estado obsoleto que a reautorização apanha.
    sqlx::query(
        "UPDATE unit_memberships SET revoked_at = now() WHERE unit_id = $1 AND person_id = $2",
    )
    .bind(unidade)
    .bind(gestor.person_id)
    .execute(&pool)
    .await
    .expect("revogar");

    // Recarregado, como o extractor HTTP faz a cada pedido.
    let agora_sem_pertenca = refrescar(&pool, gestor.person_id).await;
    let mut tx = pool.begin().await.expect("tx");
    let resultado = calendar::update_event(
        &mut tx,
        &agora_sem_pertenca,
        &ids(),
        evento,
        calendar::EventEdit {
            title: Some("Já não devia".to_owned()),
            ..calendar::EventEdit::default()
        },
    )
    .await;

    assert!(
        resultado.is_err(),
        "com a pertença revogada e o principal recarregado, a alteração passou"
    );

    // E deixa de o ver, o que fecha o par: sem pertença não lê nem escreve.
    assert!(!ve(&pool, &agora_sem_pertenca, evento).await);
}

/// O estado do evento que decide é o que está na base, não o que foi lido antes.
///
/// Alguém fecha a classificação do evento enquanto outra pessoa o tem aberto. A
/// alteração seguinte tem de ser decidida contra o estado novo.
#[tokio::test]
async fn a_alteracao_le_o_estado_do_evento_dentro_da_transaccao() {
    let pool = base!();
    let org = organizacao(&pool).await;
    let admin = pessoa(&pool, org, &[TechnicalRole::PlatformAdmin]).await;
    let unidade = unidade(&pool, &admin).await;

    let gestor = pessoa(&pool, org, &[TechnicalRole::ResearchMember]).await;
    membro_gestor(&pool, &admin, unidade, gestor.person_id).await;
    let gestor = refrescar(&pool, gestor.person_id).await;

    let de_fora = pessoa(&pool, org, &[TechnicalRole::ResearchMember]).await;

    let evento = marcar(
        &pool,
        &gestor,
        NewEvent {
            scope: EventScope::Unit,
            unit_id: Some(unidade),
            workspace_id: None,
            title: "Aberto".to_owned(),
            description: None,
            location: None,
            occurrence: daqui_a_uma_hora(),
            classification: Some(Classification::Internal),
            participants: Vec::new(),
        },
    )
    .await;

    // Controlo positivo: com o evento `INTERNAL`, quem está fora vê-o.
    assert!(ve(&pool, &de_fora, evento).await);

    // Alguém fecha-o.
    sqlx::query("UPDATE calendar_events SET classification = 'RESTRICTED' WHERE id = $1")
        .bind(evento)
        .execute(&pool)
        .await
        .expect("fechar");

    // A leitura seguinte já respeita o estado novo, sem ninguém recarregar nada.
    assert!(
        !ve(&pool, &de_fora, evento).await,
        "o evento fechado continua visível: a consulta está a decidir sobre um estado antigo"
    );
}

// ── 7 · Lembretes e entrega ─────────────────────────────────────────────

/// O dono de um lembrete é quem o pede.
#[tokio::test]
async fn o_dono_de_um_lembrete_e_sempre_quem_o_pede() {
    let pool = base!();
    let org = organizacao(&pool).await;
    let alice = pessoa(&pool, org, &[TechnicalRole::ResearchMember]).await;

    let mut tx = pool.begin().await.expect("tx");
    let lembrete = calendar::create_reminder(
        &mut tx,
        &alice,
        &ids(),
        calendar::NewReminder {
            event_id: None,
            task_id: None,
            note: Some("rever o relatório".to_owned()),
            trigger_at: Utc::now() + Duration::hours(2),
        },
    )
    .await
    .expect("lembrete");
    tx.commit().await.expect("commit");

    assert_eq!(
        lembrete.owner_id, alice.person_id,
        "o dono não é quem pediu — e `NewReminder` não tem campo para o escolher"
    );
}

/// Um lembrete sobre um recurso inalcançável não se agenda.
#[tokio::test]
async fn nao_se_agenda_um_lembrete_sobre_o_que_nao_se_alcanca() {
    let pool = base!();
    let org = organizacao(&pool).await;
    let alice = pessoa(&pool, org, &[TechnicalRole::ResearchMember]).await;
    let bruno = pessoa(&pool, org, &[TechnicalRole::ResearchMember]).await;

    let evento = marcar(
        &pool,
        &alice,
        NewEvent {
            scope: EventScope::Personal,
            unit_id: None,
            workspace_id: None,
            title: "Pessoal".to_owned(),
            description: None,
            location: None,
            occurrence: daqui_a_uma_hora(),
            classification: None,
            participants: Vec::new(),
        },
    )
    .await;

    let mut tx = pool.begin().await.expect("tx");
    let resultado = calendar::create_reminder(
        &mut tx,
        &bruno,
        &ids(),
        calendar::NewReminder {
            event_id: Some(evento),
            task_id: None,
            note: None,
            trigger_at: Utc::now(),
        },
    )
    .await;

    assert!(
        resultado.is_err(),
        "agendou-se um lembrete sobre um evento que o actor não alcança — \
         o que revelaria a sua existência à hora marcada"
    );
}

/// Dois workers em corrida entregam o lembrete uma só vez.
///
/// # Porque não chega correr o worker duas vezes
///
/// Porque correr duas vezes em sequência não exercita corrida nenhuma: a
/// primeira passagem termina antes de a segunda começar, e qualquer verificação
/// ingénua passa. A posse atómica existe para o caso em que as duas passagens se
/// sobrepõem no tempo, e é esse que este teste constrói: duas transacções
/// abertas ao mesmo tempo, ambas a tentar reclamar o mesmo lembrete.
///
/// # Porque o lembrete é antiquíssimo, e porque o lote é de um
///
/// `claim_due` é global de propósito: um worker reclama por toda a instituição.
/// Numa base partilhada por testes concorrentes, um lote de dez varre também as
/// linhas de quem está a correr ao lado — e uma delas pode estar a ser
/// actualizada por outra transacção em voo. O `SKIP LOCKED` salta o que está
/// **trancado**; uma linha concorrentemente **actualizada** faz o executor
/// seguir a cadeia de versões e esperar pela transacção que a alterou.
///
/// Esse é um bloqueio que não acaba enquanto o vizinho não terminar, e o
/// vizinho está à espera desta transacção. Encontrei-o em produção da pior
/// maneira: um binário deste teste ficou vinte horas pendurado a segurar uma
/// transacção aberta na base de testes, com a outra ligação atrás dele.
///
/// Um lembrete com dez anos de atraso é, por construção, o mais antigo da
/// tabela — os outros testes marcam para minutos atrás. Com `ORDER BY
/// trigger_at` e lote de um, as duas transacções olham exactamente para a mesma
/// linha, e para nenhuma outra. É a corrida no seu estado puro, e é a única
/// linha que este teste toca.
#[tokio::test]
async fn dois_workers_em_corrida_entregam_uma_so_vez() {
    let pool = base!();
    let org = organizacao(&pool).await;
    let alice = pessoa(&pool, org, &[TechnicalRole::ResearchMember]).await;

    let mut tx = pool.begin().await.expect("tx");
    let lembrete = calendar::create_reminder(
        &mut tx,
        &alice,
        &ids(),
        calendar::NewReminder {
            event_id: None,
            task_id: None,
            note: Some("agora".to_owned()),
            trigger_at: Utc::now() - Duration::days(3650),
        },
    )
    .await
    .expect("lembrete");
    tx.commit().await.expect("commit");

    let agora = Utc::now();

    // As duas transacções abrem **antes** de qualquer uma reclamar. É isso que
    // faz disto uma corrida e não uma sequência.
    let mut primeiro = pool.begin().await.expect("tx do primeiro worker");
    let mut segundo = pool.begin().await.expect("tx do segundo worker");

    // Se a exclusão se perder, o segundo worker fica à espera do primeiro — e o
    // primeiro está à espera deste teste, que está à espera do segundo. Sem
    // limite, isso não é uma falha: é um processo pendurado para sempre, verde
    // no relatório porque nunca chega ao fim. Com limite, é uma falha que se lê.
    sqlx::query("SET LOCAL lock_timeout = '5s'")
        .execute(&mut *segundo)
        .await
        .expect("limite de espera");

    let reclamados_pelo_primeiro = calendar::delivery::claim_due(&mut primeiro, agora, 1)
        .await
        .expect("primeira reclamação");
    let reclamados_pelo_segundo = calendar::delivery::claim_due(&mut segundo, agora, 1)
        .await
        .expect(
            "segunda reclamação: se isto excedeu o tempo de espera, a exclusão deixou de excluir",
        );

    let nosso = |lista: &[ocinye_core::modules::calendar::Reminder]| {
        lista.iter().any(|r| r.id == lembrete.id)
    };

    assert!(
        nosso(&reclamados_pelo_primeiro),
        "o primeiro worker não reclamou um lembrete que já passou da hora"
    );
    assert!(
        !nosso(&reclamados_pelo_segundo),
        "os dois workers reclamaram o mesmo lembrete: o `SKIP LOCKED` não está a excluir"
    );

    calendar::delivery::deliver_in_app(&mut primeiro, &lembrete)
        .await
        .expect("entrega");
    primeiro.commit().await.expect("commit do primeiro");
    segundo.rollback().await.expect("rollback do segundo");

    let entregas: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM reminder_deliveries WHERE reminder_id = $1")
            .bind(lembrete.id)
            .fetch_one(&pool)
            .await
            .expect("entregas");
    let notificacoes: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM notifications WHERE recipient_id = $1 AND kind = 'reminder'",
    )
    .bind(alice.person_id)
    .fetch_one(&pool)
    .await
    .expect("notificações");

    assert_eq!(entregas, 1, "o lembrete foi entregue mais do que uma vez");
    assert_eq!(notificacoes, 1, "criaram-se notificações a mais");
}

/// A segunda entrega do mesmo canal é recusada pela base, e não pela sorte.
#[tokio::test]
async fn a_base_recusa_uma_segunda_entrega_do_mesmo_canal() {
    let pool = base!();
    let org = organizacao(&pool).await;
    let alice = pessoa(&pool, org, &[TechnicalRole::ResearchMember]).await;

    let mut tx = pool.begin().await.expect("tx");
    let lembrete = calendar::create_reminder(
        &mut tx,
        &alice,
        &ids(),
        calendar::NewReminder {
            event_id: None,
            task_id: None,
            note: Some("duas vezes".to_owned()),
            // No futuro, e é deliberado: este teste chama `deliver_in_app`
            // directamente e nunca varre. Um lembrete **vencido** fica ao
            // alcance do `claim_due` de qualquer outro teste — que não tem
            // âmbito de organização nem de dono e varre a base inteira — e os
            // testes deste ficheiro correm em concorrência sobre a mesma base.
            // Foi assim que a contagem de tentativas chegou a 2 onde se
            // esperava 1.
            trigger_at: Utc::now() + Duration::hours(6),
        },
    )
    .await
    .expect("lembrete");
    calendar::delivery::deliver_in_app(&mut tx, &lembrete)
        .await
        .expect("primeira entrega");
    tx.commit().await.expect("commit");

    // Mesmo que a posse falhasse, a base tem de recusar. É a segunda linha de
    // defesa, e a que sobrevive a uma consulta mal alterada no futuro.
    let mut tx = pool.begin().await.expect("tx");
    let segunda = calendar::delivery::deliver_in_app(&mut tx, &lembrete).await;
    assert!(
        segunda.is_err(),
        "a base aceitou uma segunda entrega in-app do mesmo lembrete"
    );
}

/// Uma entrega falhada não diz que foi entregue.
#[tokio::test]
async fn uma_entrega_falhada_nao_marca_o_lembrete_como_entregue() {
    let pool = base!();
    let org = organizacao(&pool).await;
    let alice = pessoa(&pool, org, &[TechnicalRole::ResearchMember]).await;

    let mut tx = pool.begin().await.expect("tx");
    let lembrete = calendar::create_reminder(
        &mut tx,
        &alice,
        &ids(),
        calendar::NewReminder {
            event_id: None,
            task_id: None,
            note: Some("vai falhar".to_owned()),
            // No futuro, e é deliberado: este teste chama `deliver_in_app`
            // directamente e nunca varre. Um lembrete **vencido** fica ao
            // alcance do `claim_due` de qualquer outro teste — que não tem
            // âmbito de organização nem de dono e varre a base inteira — e os
            // testes deste ficheiro correm em concorrência sobre a mesma base.
            // Foi assim que a contagem de tentativas chegou a 2 onde se
            // esperava 1.
            trigger_at: Utc::now() + Duration::hours(6),
        },
    )
    .await
    .expect("lembrete");
    tx.commit().await.expect("commit");

    // Uma entrega que rebenta a meio: a transacção é desfeita.
    let mut tx = pool.begin().await.expect("tx");
    calendar::delivery::deliver_in_app(&mut tx, &lembrete)
        .await
        .expect("entrega");
    tx.rollback().await.expect("desfazer");

    let estado: String = sqlx::query_scalar("SELECT state FROM reminders WHERE id = $1")
        .bind(lembrete.id)
        .fetch_one(&pool)
        .await
        .expect("estado");
    let notificacoes: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM notifications WHERE recipient_id = $1")
            .bind(alice.person_id)
            .fetch_one(&pool)
            .await
            .expect("notificações");

    assert_eq!(
        estado, "scheduled",
        "um lembrete cuja entrega foi desfeita ficou marcado como entregue"
    );
    assert_eq!(notificacoes, 0, "ficou uma notificação órfã");

    // O contador de tentativas sobrevive à reversão, na sua própria transacção.
    calendar::delivery::record_failure(&pool, lembrete.id)
        .await
        .expect("registar falha");
    let tentativas: i32 = sqlx::query_scalar("SELECT attempts FROM reminders WHERE id = $1")
        .bind(lembrete.id)
        .fetch_one(&pool)
        .await
        .expect("tentativas");
    assert_eq!(tentativas, 1, "a tentativa falhada não ficou contada");
}

/// A notificação não é uma cópia autorizada do recurso.
#[tokio::test]
async fn a_notificacao_nao_carrega_o_conteudo_do_recurso() {
    let pool = base!();
    let org = organizacao(&pool).await;
    let admin = pessoa(&pool, org, &[TechnicalRole::PlatformAdmin]).await;
    let unidade = unidade(&pool, &admin).await;

    let gestor = pessoa(&pool, org, &[TechnicalRole::ResearchMember]).await;
    membro_gestor(&pool, &admin, unidade, gestor.person_id).await;
    let gestor = refrescar(&pool, gestor.person_id).await;

    let evento = marcar(
        &pool,
        &gestor,
        NewEvent {
            scope: EventScope::Unit,
            unit_id: Some(unidade),
            workspace_id: None,
            title: "Segredo industrial da unidade".to_owned(),
            description: Some("detalhes confidenciais".to_owned()),
            location: None,
            occurrence: daqui_a_uma_hora(),
            classification: Some(Classification::Restricted),
            participants: Vec::new(),
        },
    )
    .await;

    let mut tx = pool.begin().await.expect("tx");
    let lembrete = calendar::create_reminder(
        &mut tx,
        &gestor,
        &ids(),
        calendar::NewReminder {
            event_id: Some(evento),
            task_id: None,
            note: None,
            // No futuro, e é deliberado: este teste chama `deliver_in_app`
            // directamente e nunca varre. Um lembrete **vencido** fica ao
            // alcance do `claim_due` de qualquer outro teste — que não tem
            // âmbito de organização nem de dono e varre a base inteira — e os
            // testes deste ficheiro correm em concorrência sobre a mesma base.
            // Foi assim que a contagem de tentativas chegou a 2 onde se
            // esperava 1.
            trigger_at: Utc::now() + Duration::hours(6),
        },
    )
    .await
    .expect("lembrete");
    calendar::delivery::deliver_in_app(&mut tx, &lembrete)
        .await
        .expect("entrega");
    tx.commit().await.expect("commit");

    let (titulo, corpo, tipo, recurso): (String, Option<String>, Option<String>, Option<Uuid>) =
        sqlx::query_as(
            "SELECT title, body, resource_type, resource_id FROM notifications
              WHERE recipient_id = $1",
        )
        .bind(gestor.person_id)
        .fetch_one(&pool)
        .await
        .expect("notificação");

    assert!(
        !titulo.contains("Segredo industrial"),
        "a notificação copiou o título do evento: passaria a ser uma cópia que \
         ninguém reautoriza"
    );
    assert_eq!(corpo, None, "a notificação copiou a descrição do evento");
    assert_eq!(tipo.as_deref(), Some("calendar_event"));
    assert_eq!(
        recurso,
        Some(evento),
        "a notificação não aponta para o recurso: não há para onde reautorizar"
    );

    // A perda de acesso não é contornável pela notificação: quem abrir o recurso
    // volta a passar pelo Core.
    sqlx::query("UPDATE unit_memberships SET revoked_at = now() WHERE person_id = $1")
        .bind(gestor.person_id)
        .execute(&pool)
        .await
        .expect("revogar");
    let depois = refrescar(&pool, gestor.person_id).await;
    assert!(
        calendar::get_event(&pool, &depois, evento).await.is_err(),
        "o recurso continua legível depois de a pertença ser revogada"
    );
}

// ── 8 · Projecção de prazos ─────────────────────────────────────────────

/// Um prazo de tarefa aparece na agenda sem virar evento.
///
/// # A propriedade que isto protege
///
/// Copiar a tarefa para `calendar_events` daria a vista unificada de graça e
/// duas datas para o mesmo prazo. Uma delas ficaria desactualizada, e não há
/// forma de saber qual (ADR-0410).
#[tokio::test]
async fn um_prazo_de_tarefa_aparece_sem_criar_evento() {
    let pool = base!();
    let org = organizacao(&pool).await;
    let admin = pessoa(&pool, org, &[TechnicalRole::PlatformAdmin]).await;
    let (unidade, _) = (unidade(&pool, &admin).await, ());

    let membro = pessoa(&pool, org, &[TechnicalRole::ResearchMember]).await;
    membro_gestor(&pool, &admin, unidade, membro.person_id).await;
    let membro = refrescar(&pool, membro.person_id).await;
    let workspace = workspace(&pool, &membro, unidade).await;
    let membro = refrescar(&pool, membro.person_id).await;

    let eventos_antes: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM calendar_events WHERE organisation_id = $1")
            .bind(org)
            .fetch_one(&pool)
            .await
            .expect("contagem");

    let amanha = (Utc::now() + Duration::days(1)).date_naive();
    let mut tx = pool.begin().await.expect("tx");
    let tarefa = ocinye_core::modules::collaboration::create_task(
        &mut tx,
        &membro,
        &ids(),
        ocinye_core::modules::collaboration::NewTask {
            workspace_id: workspace,
            title: "Entregar o relatório".to_owned(),
            description: None,
            priority: ocinye_core::modules::collaboration::TaskPriority::default(),
            assignee_id: None,
            due_on: Some(amanha),
        },
    )
    .await
    .expect("tarefa");
    tx.commit().await.expect("commit");

    let intervalo = TimeRange {
        start: Utc::now() - Duration::days(1),
        end: Utc::now() + Duration::days(7),
    };
    let agenda = calendar::agenda(&pool, &membro, intervalo, 200)
        .await
        .expect("agenda");

    let prazo = agenda
        .iter()
        .find(|item| item.id == tarefa.id)
        .expect("o prazo da tarefa não aparece na agenda");

    assert_eq!(
        prazo.kind,
        ocinye_contracts::calendar::TemporalItemKind::TaskDue,
        "o prazo apareceu como evento: a interface ofereceria «cancelar» sobre ele"
    );

    let eventos_depois: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM calendar_events WHERE organisation_id = $1")
            .bind(org)
            .fetch_one(&pool)
            .await
            .expect("contagem");
    assert_eq!(
        eventos_antes, eventos_depois,
        "criar uma tarefa com prazo criou um evento de calendário"
    );

    // Mudar o prazo muda a projecção. Não há segunda cópia a actualizar.
    let daqui_a_um_mes = (Utc::now() + Duration::days(30)).date_naive();
    sqlx::query("UPDATE tasks SET due_on = $2 WHERE id = $1")
        .bind(tarefa.id)
        .bind(daqui_a_um_mes)
        .execute(&pool)
        .await
        .expect("mudar o prazo");

    let agenda = calendar::agenda(&pool, &membro, intervalo, 200)
        .await
        .expect("agenda");
    assert!(
        !agenda.iter().any(|item| item.id == tarefa.id),
        "o prazo mudou para fora do intervalo e continua a aparecer nele"
    );

    // Tirar o prazo tira a projecção.
    sqlx::query("UPDATE tasks SET due_on = NULL WHERE id = $1")
        .bind(tarefa.id)
        .execute(&pool)
        .await
        .expect("tirar o prazo");

    let largo = TimeRange {
        start: Utc::now() - Duration::days(1),
        end: Utc::now() + Duration::days(365),
    };
    let agenda = calendar::agenda(&pool, &membro, largo, 200)
        .await
        .expect("agenda");
    assert!(
        !agenda.iter().any(|item| item.id == tarefa.id),
        "uma tarefa sem prazo continua a aparecer na agenda"
    );
}

/// O prazo de uma tarefa inalcançável não aparece na agenda de ninguém.
///
/// A visibilidade de um prazo é a **da tarefa** — artefacto ∩ contentor —, e não
/// a dos eventos. Se o calendário aplicasse a sua própria, um prazo revelaria
/// que há trabalho onde o actor não entra.
#[tokio::test]
async fn um_prazo_de_uma_tarefa_inalcancavel_nao_aparece() {
    let pool = base!();
    let org = organizacao(&pool).await;
    let admin = pessoa(&pool, org, &[TechnicalRole::PlatformAdmin]).await;
    let unidade = unidade(&pool, &admin).await;

    let dentro = pessoa(&pool, org, &[TechnicalRole::ResearchMember]).await;
    membro_gestor(&pool, &admin, unidade, dentro.person_id).await;
    let dentro = refrescar(&pool, dentro.person_id).await;
    let workspace = workspace(&pool, &dentro, unidade).await;
    let dentro = refrescar(&pool, dentro.person_id).await;

    // O contentor fecha-se: só a contenção pode esconder um prazo INTERNAL.
    let mut tx = pool.begin().await.expect("tx");
    research::reclassify_workspace(
        &mut tx,
        &dentro,
        &ids(),
        workspace,
        Classification::Restricted,
        "fecho para o teste de projecção",
    )
    .await
    .expect("reclassificar");
    tx.commit().await.expect("commit");
    let dentro = refrescar(&pool, dentro.person_id).await;

    let amanha = (Utc::now() + Duration::days(1)).date_naive();
    let mut tx = pool.begin().await.expect("tx");
    let tarefa = ocinye_core::modules::collaboration::create_task(
        &mut tx,
        &dentro,
        &ids(),
        ocinye_core::modules::collaboration::NewTask {
            workspace_id: workspace,
            title: "Segredo com prazo".to_owned(),
            description: None,
            priority: ocinye_core::modules::collaboration::TaskPriority::default(),
            assignee_id: None,
            due_on: Some(amanha),
        },
    )
    .await
    .expect("tarefa");
    tx.commit().await.expect("commit");

    // A tarefa nasce com a classificação do workspace — aqui, `RESTRICTED`. Com
    // ela, a cláusula do artefacto negava o prazo ao forasteiro sozinha, e a
    // metade da contenção nunca era exercitada: descobri-o ao removê-la e ver o
    // teste continuar verde.
    //
    // `INTERNAL` é legível por qualquer membro activo. Dentro de um workspace
    // `RESTRICTED`, só a contenção o pode esconder — e é isso que se mede.
    sqlx::query("UPDATE tasks SET classification = 'INTERNAL' WHERE id = $1")
        .bind(tarefa.id)
        .execute(&pool)
        .await
        .expect("abrir a classificação da tarefa");

    let classificacao: String =
        sqlx::query_scalar("SELECT classification FROM tasks WHERE id = $1")
            .bind(tarefa.id)
            .fetch_one(&pool)
            .await
            .expect("classificação");
    assert_eq!(
        classificacao, "INTERNAL",
        "a tarefa não ficou INTERNAL: o teste voltaria a medir a classificação"
    );

    let intervalo = TimeRange {
        start: Utc::now() - Duration::days(1),
        end: Utc::now() + Duration::days(7),
    };

    let forasteiro = pessoa(&pool, org, &[TechnicalRole::ResearchMember]).await;

    let visto_por_dentro = calendar::agenda(&pool, &dentro, intervalo, 200)
        .await
        .expect("agenda")
        .iter()
        .any(|item| item.id == tarefa.id);
    let visto_de_fora = calendar::agenda(&pool, &forasteiro, intervalo, 200)
        .await
        .expect("agenda")
        .iter()
        .any(|item| item.id == tarefa.id);

    assert!(
        visto_por_dentro,
        "quem está dentro não vê o seu próprio prazo"
    );
    assert!(
        !visto_de_fora,
        "o prazo de uma tarefa num workspace inalcançável aparece a quem não entra nele"
    );
}

// ── 9 · A passagem do worker ────────────────────────────────────────────

/// O worker entrega o que já passou da hora, e não entrega o que ainda não.
///
/// # Porque isto é a funcionalidade, e não a função
///
/// As funções de entrega existiam e ninguém as chamava. Um lembrete que nunca
/// dispara não é um lembrete — este teste chama a passagem que o worker chama,
/// e mede o que ela deixa atrás.
/// Corre passagens do worker até este lembrete concreto ser entregue.
///
/// Uma passagem reclama um lote de cinquenta. Isolado, o lembrete deste teste é
/// o único vencido e uma passagem chega; com a suite inteira a correr em
/// paralelo, pode haver mais de cinquenta vencidos ao mesmo tempo e o dele fica
/// para a passagem seguinte.
///
/// O teste assumia que uma passagem bastava, e falhou uma vez no sweep completo
/// por causa disso. Em produção o worker corre repetidamente — é isto que ele
/// faz, e não uma tentativa única — portanto esperar por passagens sucessivas é
/// mais fiel ao produto, e não menos.
///
/// O limite existe para que um lembrete que **nunca** seja entregue continue a
/// falhar o teste em vez de o prender para sempre.
/// # Porque um lembrete vencido é partilhado, e não deste teste
///
/// `claim_due` não tem âmbito nenhum: nem organização, nem dono. Varre a base
/// inteira, e está certo — é o que um worker faz. Mas os testes correm em
/// concorrência **entre binários**: `apps/workspace/tests/browser.rs` também
/// chama `deliver_due`, e o `cargo test --workspace` corre-o ao mesmo tempo
/// que este ficheiro, contra a mesma base.
///
/// Logo, **qualquer lembrete com `trigger_at` no passado está ao alcance de
/// qualquer varrimento**. Um varrimento cuja entrega falhe chama
/// `record_failure`, que incrementa `attempts` e deixa o estado em
/// `scheduled` — indistinguível, para quem lê o teste, de uma tentativa
/// própria. Foi assim que `uma_entrega_falhada_nao_marca_o_lembrete_como_entregue`
/// contou 2 tentativas onde esperava 1, numa corrida completa de `verify.sh` em
/// 2026-08-29.
///
/// A regra que daí resulta: **um teste que não varre não cria lembretes
/// vencidos.** Os três que chamavam `deliver_in_app` directamente passaram a
/// usar uma hora futura, e ficaram fora do alcance de qualquer varrimento.
///
/// Continua exposto, deliberadamente, o `dois_workers_em_corrida_entregam_uma_so_vez`:
/// precisa de um lembrete vencido para exercer o `SKIP LOCKED`, e defende-se
/// com um `trigger_at` de dez anos atrás, que o faz ordenar primeiro. Se
/// aparecer uma intermitência ali, é aqui que está o mapa.
async fn entregar_ate_chegar(pool: &sqlx::PgPool, lembrete: uuid::Uuid) -> bool {
    for _ in 0..20 {
        calendar::delivery::deliver_due(pool)
            .await
            .expect("passagem");
        let entregue: i64 =
            sqlx::query_scalar("SELECT count(*) FROM reminder_deliveries WHERE reminder_id = $1")
                .bind(lembrete)
                .fetch_one(pool)
                .await
                .expect("estado da entrega");
        if entregue > 0 {
            return true;
        }
    }
    false
}

#[tokio::test]
async fn a_passagem_do_worker_entrega_o_que_esta_vencido() {
    let pool = base!();
    let org = organizacao(&pool).await;
    let alice = pessoa(&pool, org, &[TechnicalRole::ResearchMember]).await;

    let mut tx = pool.begin().await.expect("tx");
    let vencido = calendar::create_reminder(
        &mut tx,
        &alice,
        &ids(),
        calendar::NewReminder {
            event_id: None,
            task_id: None,
            note: Some("já passou".to_owned()),
            trigger_at: Utc::now() - Duration::minutes(5),
        },
    )
    .await
    .expect("vencido");
    let futuro = calendar::create_reminder(
        &mut tx,
        &alice,
        &ids(),
        calendar::NewReminder {
            event_id: None,
            task_id: None,
            note: Some("ainda falta".to_owned()),
            trigger_at: Utc::now() + Duration::hours(3),
        },
    )
    .await
    .expect("futuro");
    tx.commit().await.expect("commit");

    // Passagens até este lembrete concreto ser entregue. Uma passagem reclama
    // cinquenta, e com a suite inteira em paralelo pode haver mais do que isso
    // vencidos ao mesmo tempo.
    assert!(
        entregar_ate_chegar(&pool, vencido.id).await,
        "vinte passagens do worker e o lembrete vencido continua por entregar"
    );

    async fn estado(pool: &PgPool, id: Uuid) -> String {
        sqlx::query_scalar::<_, String>("SELECT state FROM reminders WHERE id = $1")
            .bind(id)
            .fetch_one(pool)
            .await
            .expect("estado")
    }
    assert_eq!(estado(&pool, vencido.id).await, "delivered");
    assert_eq!(
        estado(&pool, futuro.id).await,
        "scheduled",
        "a passagem entregou um lembrete que ainda não venceu"
    );

    // A notificação existe, e é da dona.
    let notificacoes = calendar::notifications(&pool, &alice, 10)
        .await
        .expect("notificações");
    assert_eq!(notificacoes.len(), 1);
    assert_eq!(
        calendar::unread_notifications(&pool, &alice)
            .await
            .expect("por ler"),
        1
    );

    // Uma segunda passagem não entrega outra vez. É a idempotência que impede a
    // pessoa de ser avisada duas vezes da mesma coisa.
    let outra_vez = calendar::delivery::deliver_due(&pool)
        .await
        .expect("segunda passagem");
    let notificacoes = calendar::notifications(&pool, &alice, 10)
        .await
        .expect("notificações");
    assert_eq!(
        notificacoes.len(),
        1,
        "a segunda passagem entregou o mesmo lembrete outra vez ({outra_vez} entregues)"
    );
}

/// Um lembrete adiado volta a vencer, e é entregue então.
#[tokio::test]
async fn um_lembrete_adiado_e_entregue_quando_volta_a_vencer() {
    let pool = base!();
    let org = organizacao(&pool).await;
    let alice = pessoa(&pool, org, &[TechnicalRole::ResearchMember]).await;

    let mut tx = pool.begin().await.expect("tx");
    let lembrete = calendar::create_reminder(
        &mut tx,
        &alice,
        &ids(),
        calendar::NewReminder {
            event_id: None,
            task_id: None,
            note: Some("adiável".to_owned()),
            trigger_at: Utc::now() - Duration::minutes(1),
        },
    )
    .await
    .expect("lembrete");

    // Adiado para o futuro: a passagem não lhe toca.
    calendar::snooze_reminder(
        &mut tx,
        &alice,
        &ids(),
        lembrete.id,
        Utc::now() + Duration::hours(2),
    )
    .await
    .expect("adiar");
    tx.commit().await.expect("commit");

    calendar::delivery::deliver_due(&pool)
        .await
        .expect("passagem");
    assert_eq!(
        calendar::unread_notifications(&pool, &alice)
            .await
            .expect("contar"),
        0,
        "um lembrete adiado para o futuro foi entregue à mesma"
    );

    // A hora chega.
    sqlx::query("UPDATE reminders SET trigger_at = now() - interval '1 minute' WHERE id = $1")
        .bind(lembrete.id)
        .execute(&pool)
        .await
        .expect("vencer");

    assert!(
        entregar_ate_chegar(&pool, lembrete.id).await,
        "vinte passagens do worker e o lembrete continua por entregar"
    );
    assert_eq!(
        calendar::unread_notifications(&pool, &alice)
            .await
            .expect("contar"),
        1,
        "um lembrete adiado que voltou a vencer não foi entregue"
    );
}

/// Um lembrete dispensado nunca é entregue.
#[tokio::test]
async fn um_lembrete_dispensado_nao_volta() {
    let pool = base!();
    let org = organizacao(&pool).await;
    let alice = pessoa(&pool, org, &[TechnicalRole::ResearchMember]).await;

    let mut tx = pool.begin().await.expect("tx");
    let lembrete = calendar::create_reminder(
        &mut tx,
        &alice,
        &ids(),
        calendar::NewReminder {
            event_id: None,
            task_id: None,
            note: Some("já vi".to_owned()),
            trigger_at: Utc::now() - Duration::minutes(1),
        },
    )
    .await
    .expect("lembrete");
    calendar::dismiss_reminder(&mut tx, &alice, &ids(), lembrete.id)
        .await
        .expect("dispensar");
    tx.commit().await.expect("commit");

    calendar::delivery::deliver_due(&pool)
        .await
        .expect("passagem");
    assert_eq!(
        calendar::unread_notifications(&pool, &alice)
            .await
            .expect("contar"),
        0,
        "um lembrete dispensado foi entregue"
    );
}

// ── 10 · A autoridade na execução ───────────────────────────────────────

/// Um plano do Calendário não corre com autoridade já revogada.
///
/// # Porque isto se testa aqui e não só no ciclo de vida
///
/// Porque a fronteira é transversal e o Calendário **não a implementa**: herda-a.
/// Este teste é a prova de que herdou — se alguém puser um `reload` próprio
/// dentro do Calendário, ou se o Calendário deixar de passar pelo executor, isto
/// continua verde por outra razão e deixa de significar o que diz.
///
/// Por isso o caminho é o real: plano persistido, confirmação, revogação, e
/// execução com o retrato antigo em mão.
#[tokio::test]
async fn um_plano_do_calendario_nao_corre_com_autoridade_revogada() {
    let pool = base!();
    let org = organizacao(&pool).await;
    let admin = pessoa(&pool, org, &[TechnicalRole::PlatformAdmin]).await;
    let unidade = unidade(&pool, &admin).await;

    let actor = pessoa(&pool, org, &[TechnicalRole::ResearchMember]).await;
    membro_gestor(&pool, &admin, unidade, actor.person_id).await;
    let actor = refrescar(&pool, actor.person_id).await;

    // Controlo positivo: com autoridade, a capability marca.
    let amanha = (Utc::now() + Duration::days(1)).date_naive();
    let entrada = serde_json::json!({
        "title": "Pelo agente",
        "starts_at": format!("{amanha}T09:00"),
        "ends_at": format!("{amanha}T10:00"),
        "timezone": "Europe/Lisbon"
    });

    let com_autoridade = ocinye_core::modules::agentic::execute(
        &pool,
        capacidades(),
        &Realtime::ausente(),
        &actor,
        &ocinye_core::modules::agentic::runtime::main_agent_boundary(),
        None,
        &ocinye_contracts::agentic::CapabilityRequest {
            capability: ocinye_contracts::agentic::CapabilityId::new("calendar.event.create"),
            input: entrada.clone(),
            resources: Vec::new(),
            dry_run: false,
        },
        &ocinye_domain::ResourceContext::organisation(
            ocinye_domain::ResourceKind::CalendarEvent,
            org,
        ),
        true,
        &ids(),
    )
    .await
    .expect("resultado");
    assert_eq!(
        com_autoridade.status,
        ocinye_contracts::agentic::ExecutionStatus::Succeeded,
        "a capability não marca sequer com autoridade: {}",
        com_autoridade.detail
    );

    // A conta é suspensa. O retrato em mão continua a dizer que está activa —
    // é exactamente o estado obsoleto que a fronteira apanha.
    let obsoleto = actor.clone();
    assert!(
        obsoleto.is_active,
        "o retrato não dizia activo: nada é medido"
    );

    sqlx::query("UPDATE people SET status = 'suspended' WHERE id = $1")
        .bind(actor.person_id)
        .execute(&pool)
        .await
        .expect("suspender");

    let eventos_antes: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM calendar_events WHERE organisation_id = $1")
            .bind(org)
            .fetch_one(&pool)
            .await
            .expect("contagem");

    let com_autoridade_revogada = ocinye_core::modules::agentic::execute(
        &pool,
        capacidades(),
        &Realtime::ausente(),
        &obsoleto,
        &ocinye_core::modules::agentic::runtime::main_agent_boundary(),
        None,
        &ocinye_contracts::agentic::CapabilityRequest {
            capability: ocinye_contracts::agentic::CapabilityId::new("calendar.event.create"),
            input: entrada,
            resources: Vec::new(),
            dry_run: false,
        },
        &ocinye_domain::ResourceContext::organisation(
            ocinye_domain::ResourceKind::CalendarEvent,
            org,
        ),
        true,
        &ids(),
    )
    .await;

    let recusado = match com_autoridade_revogada {
        Err(_) => true,
        Ok(resultado) => resultado.status != ocinye_contracts::agentic::ExecutionStatus::Succeeded,
    };
    assert!(
        recusado,
        "um plano do Calendário correu com a conta já suspensa"
    );

    let eventos_depois: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM calendar_events WHERE organisation_id = $1")
            .bind(org)
            .fetch_one(&pool)
            .await
            .expect("contagem");
    assert_eq!(
        eventos_antes, eventos_depois,
        "a execução recusada deixou um evento escrito"
    );
}

/// O Calendário não implementa recarregamento próprio de autoridade.
///
/// A fronteira é uma, e é central. Um `reload` dentro do Calendário seria uma
/// segunda convenção — e a próxima pessoa a escrever um módulo não a herdaria.
#[test]
fn o_calendario_nao_tem_fronteira_de_autoridade_propria() {
    for ficheiro in [
        include_str!("../src/modules/calendar/service.rs"),
        include_str!("../src/modules/calendar/repository.rs"),
        include_str!("../src/modules/calendar/delivery.rs"),
        include_str!("../src/modules/agentic/capabilities/calendar.rs"),
    ] {
        for proibido in [
            "principal_for_person",
            "load_principal",
            "authority::resolve",
        ] {
            assert!(
                !ficheiro.contains(proibido),
                "o Calendário contém «{proibido}»: passou a resolver autoridade por \
                 si, em vez de herdar a fronteira central (ADR-0411)"
            );
        }
    }
}

/// O Capability Runtime, com os componentes desta árvore.
fn capacidades() -> &'static ocinye_core::capabilities::Capabilities {
    use std::sync::OnceLock;
    static UM: OnceLock<ocinye_core::capabilities::Capabilities> = OnceLock::new();
    UM.get_or_init(|| {
        ocinye_core::capabilities::Capabilities::load(&format!(
            "{}/../../target/wasm32-wasip1/release",
            env!("CARGO_MANIFEST_DIR")
        ))
        .expect("motor de capacidades")
    })
}

// ── 9 · Participantes ───────────────────────────────────────────────────

/// Quem participa fica associado por referência institucional.
#[tokio::test]
async fn os_participantes_ficam_associados() {
    let pool = base!();
    let org = organizacao(&pool).await;
    let alice = pessoa(&pool, org, &[TechnicalRole::ResearchMember]).await;
    let bruno = pessoa(&pool, org, &[TechnicalRole::ResearchMember]).await;
    let carla = pessoa(&pool, org, &[TechnicalRole::ResearchMember]).await;

    let evento = marcar(
        &pool,
        &alice,
        NewEvent {
            scope: EventScope::Personal,
            unit_id: None,
            workspace_id: None,
            title: "Conselho científico".to_owned(),
            description: None,
            location: None,
            occurrence: daqui_a_uma_hora(),
            classification: None,
            participants: vec![bruno.person_id, carla.person_id],
        },
    )
    .await;

    let quem = ocinye_core::modules::calendar::participants_of(&pool, evento)
        .await
        .expect("participantes");

    assert_eq!(quem.len(), 2, "os participantes não ficaram associados");
    let ids: Vec<_> = quem.iter().map(|(id, _)| *id).collect();
    assert!(ids.contains(&bruno.person_id) && ids.contains(&carla.person_id));
}

/// A mesma pessoa duas vezes é a mesma pessoa.
///
/// Um clique a mais não é um erro a devolver a quem marca: a chave primária da
/// tabela recusaria o duplicado, e a operação inteira falharia por causa de uma
/// conveniência.
#[tokio::test]
async fn a_mesma_pessoa_duas_vezes_conta_uma() {
    let pool = base!();
    let org = organizacao(&pool).await;
    let alice = pessoa(&pool, org, &[TechnicalRole::ResearchMember]).await;
    let bruno = pessoa(&pool, org, &[TechnicalRole::ResearchMember]).await;

    let evento = marcar(
        &pool,
        &alice,
        NewEvent {
            scope: EventScope::Personal,
            unit_id: None,
            workspace_id: None,
            title: "Ponto de situação".to_owned(),
            description: None,
            location: None,
            occurrence: daqui_a_uma_hora(),
            classification: None,
            participants: vec![bruno.person_id, bruno.person_id, bruno.person_id],
        },
    )
    .await;

    let quem = ocinye_core::modules::calendar::participants_of(&pool, evento)
        .await
        .expect("participantes");
    assert_eq!(
        quem.len(),
        1,
        "a mesma pessoa ficou associada mais do que uma vez"
    );
}

/// Alguém de fora da instituição não entra numa actividade dela.
///
/// # Porque o teste existe do lado do Core
///
/// Porque um identificador vindo do cliente **nomeia** alguém e não estabelece
/// que essa pessoa possa ser associada. A Experience mostra o universo
/// autorizado; nada impede um pedido feito à mão com outro identificador.
#[tokio::test]
async fn alguem_de_outra_instituicao_nao_participa() {
    let pool = base!();
    let org = organizacao(&pool).await;
    let outra = organizacao(&pool).await;
    let alice = pessoa(&pool, org, &[TechnicalRole::ResearchMember]).await;
    let estranho = pessoa(&pool, outra, &[TechnicalRole::ResearchMember]).await;

    let mut tx = pool.begin().await.expect("tx");
    let resultado = ocinye_core::modules::calendar::create_event(
        &mut tx,
        &alice,
        &ids(),
        NewEvent {
            scope: EventScope::Personal,
            unit_id: None,
            workspace_id: None,
            title: "Reunião".to_owned(),
            description: None,
            location: None,
            occurrence: daqui_a_uma_hora(),
            classification: None,
            participants: vec![estranho.person_id],
        },
    )
    .await;

    assert!(
        resultado.is_err(),
        "uma pessoa de outra instituição foi associada a uma actividade"
    );
}

/// Um identificador que não é ninguém também não passa.
#[tokio::test]
async fn um_identificador_inventado_nao_participa() {
    let pool = base!();
    let org = organizacao(&pool).await;
    let alice = pessoa(&pool, org, &[TechnicalRole::ResearchMember]).await;

    let mut tx = pool.begin().await.expect("tx");
    let resultado = ocinye_core::modules::calendar::create_event(
        &mut tx,
        &alice,
        &ids(),
        NewEvent {
            scope: EventScope::Personal,
            unit_id: None,
            workspace_id: None,
            title: "Reunião".to_owned(),
            description: None,
            location: None,
            occurrence: daqui_a_uma_hora(),
            classification: None,
            participants: vec![Uuid::new_v4()],
        },
    )
    .await;

    assert!(resultado.is_err(), "um identificador inventado foi aceite");
}
