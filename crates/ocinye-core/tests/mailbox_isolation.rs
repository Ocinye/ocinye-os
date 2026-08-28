//! Uma caixa é de uma pessoa, e conhecer o seu identificador não a abre.
//!
//! # Porque esta suite existe separada
//!
//! Porque a pergunta é a mesma para **todas** as operações de correio, e uma
//! propriedade que vale para todas mas está testada numa é uma propriedade que
//! a próxima operação vai perder sem que nada o diga.
//!
//! O correio tem hoje onze operações. Cada uma recebe um identificador de
//! caixa vindo de fora — de um formulário, de um caminho de URL, de um plano
//! agentic — e cada uma tem de recusar quando esse identificador nomeia a
//! caixa de outra pessoa.
//!
//! > **Um identificador nomeia âmbito; nunca o concede** (`CLAUDE.md` §34.2).
//!
//! # O que é que estes testes fazem que os outros não fazem
//!
//! Os testes em `mail.rs` provam cada operação em separado, com quem tem
//! direito a ela. Estes fazem o contrário: percorrem as operações **com a
//! pessoa errada**, e exigem recusa em todas.
//!
//! # Porque a recusa é «não encontrado» e não «sem acesso»
//!
//! Porque dizer «existe, mas não é sua» já é informação: confirma que aquela
//! caixa existe, e a quem pergunta pelos identificadores todos confirma quais
//! existem ([ADR-0100]). O Core responde o mesmo que responderia a um
//! identificador inventado.
//!
//! # Onde cada recusa está ancorada
//!
//! Verificado por reversão em 2026-08-27, uma linha de cada vez. O predicado
//! de posse — `kind = 'personal' AND owner_id = $1` — aparece cinco vezes em
//! `repository.rs`, e apagá-lo em cada uma diz quantas destas recusas
//! dependiam dela:
//!
//! | predicado | recusas que caem |
//! |---|---|
//! | a listagem de caixas | 1 |
//! | `accessible_mailbox` | 5 |
//! | `accessible_message` | 2 |
//! | o `UPDATE` de `set_flag` | 0 |
//! | `accessible_draft` | 0 |
//!
//! As oito recusas estão todas presas a alguma coisa, e nenhuma passa por
//! acaso.
//!
//! As duas linhas com zero **não são lacunas**. O `UPDATE` de `set_flag` é uma
//! segunda fechadura na mesma porta: a autorização já recusou acima, e este
//! predicado existe para o caso de ela deixar de o fazer. E `accessible_draft`
//! serve rascunhos, que hoje só existem no caminho agentic e são cobertos em
//! `agentic.rs`.
//!
//! [ADR-0100]: ../../../docs/adrs/0100-authorization-model.md

use ocinye_contracts::{MailAddress, MailFolder};
use ocinye_core::error::CoreError;
use ocinye_core::modules::mail::provider::{
    CredentialProbe, FetchedMessage, MailProvider, MessagePage, OutgoingMessage, ProviderError,
    ProviderHealth, ProviderResult,
};
use ocinye_core::modules::mail::{service as mail, ProviderRegistry};
use ocinye_core::password::sealed::SealingKey;
use ocinye_observability::CorrelationIds;
use sqlx::PgPool;
use std::sync::Arc;
use uuid::Uuid;

const DOMAIN: &str = "ocinye.com";

/// Salta quando não há base de dados; **falha** quando há e algo corre mal.
async fn pool() -> Option<PgPool> {
    let url = std::env::var("OCINYE_TEST_DATABASE_URL").ok()?;
    let pool = PgPool::connect(&url).await.expect("base de dados");
    sqlx::migrate!("../../migrations")
        .run(&pool)
        .await
        .expect("migrations");
    Some(pool)
}

async fn organisation(pool: &PgPool) -> Uuid {
    sqlx::query_scalar("INSERT INTO organisations (slug, name) VALUES ($1, $2) RETURNING id")
        .bind(format!("iso-{}", Uuid::new_v4().simple()))
        .bind("Instituição do isolamento")
        .fetch_one(pool)
        .await
        .expect("organização")
}

async fn person(pool: &PgPool, organisation_id: Uuid) -> ocinye_domain::Principal {
    let handle = format!("p{}", Uuid::new_v4().simple());

    let person_id: Uuid = sqlx::query_scalar(
        "INSERT INTO people (organisation_id, full_name, email, status)
              VALUES ($1, $2, $3, 'active') RETURNING id",
    )
    .bind(organisation_id)
    .bind(&handle)
    .bind(format!("{handle}@{DOMAIN}"))
    .fetch_one(pool)
    .await
    .expect("pessoa");

    sqlx::query("INSERT INTO person_roles (person_id, role) VALUES ($1, 'research_member')")
        .bind(person_id)
        .execute(pool)
        .await
        .expect("papel");

    let record = ocinye_core::modules::identity::person_by_id(pool, person_id)
        .await
        .expect("consulta")
        .expect("pessoa");
    ocinye_core::modules::identity::principal_for_person(pool, &record)
        .await
        .expect("principal")
}

async fn personal_mailbox(pool: &PgPool, organisation_id: Uuid, owner_id: Uuid) -> (Uuid, String) {
    let address = format!("mb{}@{DOMAIN}", Uuid::new_v4().simple());
    let id = sqlx::query_scalar(
        "INSERT INTO mailboxes (organisation_id, address, kind, owner_id)
              VALUES ($1, $2, 'personal', $3) RETURNING id",
    )
    .bind(organisation_id)
    .bind(&address)
    .bind(owner_id)
    .fetch_one(pool)
    .await
    .expect("caixa");
    (id, address)
}

async fn message_in(pool: &PgPool, mailbox_id: Uuid) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO mail_messages
                (mailbox_id, provider_id, folder, from_address, subject, sent_at)
              VALUES ($1, $2, 'inbox', 'externo@exemplo.com', 'Assunto privado', now())
         RETURNING id",
    )
    .bind(mailbox_id)
    .bind(Uuid::new_v4().to_string())
    .fetch_one(pool)
    .await
    .expect("mensagem")
}

fn chave() -> SealingKey {
    SealingKey::from_base64(&SealingKey::generate()).expect("chave")
}

fn registry() -> Arc<ProviderRegistry> {
    Arc::new(ProviderRegistry::new(
        Arc::new(FornecedorQueRegista),
        ocinye_core::config::MailConfig {
            institutional_domains: vec![DOMAIN.to_owned()],
            imap_host: String::new(),
            imap_port: 993,
            imap_security: ocinye_core::config::MailSecurity::ImplicitTls,
            smtp_host: String::new(),
            smtp_port: 465,
            smtp_security: ocinye_core::config::MailSecurity::ImplicitTls,
            username: String::new(),
            password: String::new(),
            max_message_bytes: 25 * 1024 * 1024,
            sealing_key: Some(chave()),
        },
        Some(chave()),
    ))
}

/// Recusado, e da maneira que não revela nada.
///
/// `NotFound` e não `PermissionDenied`: dizer «existe, mas não é sua» confirma
/// a existência daquela caixa.
#[track_caller]
fn recusado<T: std::fmt::Debug>(resultado: Result<T, CoreError>, operacao: &str) {
    match resultado {
        Err(CoreError::NotFound(_)) => {}
        Err(CoreError::PermissionDenied(_)) => {
            panic!(
                "«{operacao}» recusou dizendo que não tem acesso. Isso confirma que a \
                 caixa existe, e a existência já é informação (ADR-0100)."
            )
        }
        Err(outro) => panic!("«{operacao}» recusou pela razão errada: {outro:?}"),
        Ok(valor) => panic!("«{operacao}» **não recusou**: {valor:?}"),
    }
}

/// Duas pessoas, e a caixa da segunda.
struct Cenario {
    pool: PgPool,
    intruso: ocinye_domain::Principal,
    dono: ocinye_domain::Principal,
    caixa_do_dono: Uuid,
    endereco_do_dono: String,
    mensagem_do_dono: Uuid,
}

async fn cenario(pool: PgPool) -> Cenario {
    let org = organisation(&pool).await;
    let intruso = person(&pool, org).await;
    let dono = person(&pool, org).await;
    let (caixa_do_dono, endereco_do_dono) = personal_mailbox(&pool, org, dono.person_id).await;
    let mensagem_do_dono = message_in(&pool, caixa_do_dono).await;

    // O intruso tem a sua própria caixa, e usa-a normalmente. Sem isto, os
    // testes abaixo passariam num mundo onde ninguém alcança caixa nenhuma.
    personal_mailbox(&pool, org, intruso.person_id).await;

    Cenario {
        pool,
        intruso,
        dono,
        caixa_do_dono,
        endereco_do_dono,
        mensagem_do_dono,
    }
}

/// O controlo positivo: o dono alcança a sua caixa.
///
/// Sem ele, tudo o que se segue passaria num sistema onde o correio está
/// simplesmente partido — e um teste que passa por tudo estar partido é o
/// pior tipo de verde.
#[tokio::test]
async fn o_dono_alcanca_a_sua_propria_caixa() {
    let Some(pool) = pool().await else { return };
    let c = cenario(pool).await;

    let caixa = mail::mailbox(&c.pool, &c.dono, c.caixa_do_dono)
        .await
        .expect("o dono não alcançou a sua própria caixa");
    assert_eq!(caixa.address, c.endereco_do_dono);

    mail::read_message(
        &c.pool,
        &registry(),
        &c.dono,
        c.mensagem_do_dono,
        false,
        &CorrelationIds::generate(),
    )
    .await
    .expect("o dono não leu a sua própria mensagem");
}

/// Conhecer o identificador da caixa não a abre.
#[tokio::test]
async fn conhecer_o_identificador_nao_abre_a_caixa() {
    let Some(pool) = pool().await else { return };
    let c = cenario(pool).await;

    recusado(
        mail::mailbox(&c.pool, &c.intruso, c.caixa_do_dono).await,
        "resolver a caixa",
    );
}

/// Conhecer o identificador da mensagem não a lê.
#[tokio::test]
async fn conhecer_o_identificador_nao_le_a_mensagem() {
    let Some(pool) = pool().await else { return };
    let c = cenario(pool).await;

    recusado(
        mail::read_message(
            &c.pool,
            &registry(),
            &c.intruso,
            c.mensagem_do_dono,
            false,
            &CorrelationIds::generate(),
        )
        .await,
        "ler a mensagem",
    );
}

/// A listagem de caixas devolve apenas as próprias.
///
/// Não basta recusar por identificador: uma listagem que devolvesse a caixa de
/// toda a gente entregaria os identificadores todos a quem os pedisse.
#[tokio::test]
async fn a_listagem_nao_entrega_as_caixas_dos_outros() {
    let Some(pool) = pool().await else { return };
    let c = cenario(pool).await;

    let caixas = mail::mailboxes(&c.pool, &c.intruso)
        .await
        .expect("listar as próprias caixas");

    assert!(
        !caixas.iter().any(|caixa| caixa.id == c.caixa_do_dono),
        "a caixa de outra pessoa apareceu na listagem"
    );
    assert!(
        !caixas.is_empty(),
        "o intruso não viu caixa nenhuma — o teste passaria com o correio partido"
    );
}

/// Ligar uma credencial à caixa de outra pessoa é recusado.
#[tokio::test]
async fn nao_se_liga_credencial_na_caixa_de_outra_pessoa() {
    let Some(pool) = pool().await else { return };
    let c = cenario(pool).await;
    let k = chave();

    recusado(
        mail::connect_mailbox(
            &c.pool,
            &c.intruso,
            c.caixa_do_dono,
            &mail::MailboxConnection {
                chave: Some(&k),
                sonda: &SondaQueAceita,
                senha: "a-senha-do-intruso",
            },
            &CorrelationIds::generate(),
        )
        .await,
        "ligar a caixa",
    );
}

/// Desligar a caixa de outra pessoa é recusado.
///
/// Não é uma leitura, e por isso não seria apanhado por nenhum teste de
/// visibilidade: é uma operação **destrutiva** — apaga a credencial — e um
/// identificador conhecido bastaria para deixar alguém sem correio.
#[tokio::test]
async fn nao_se_desliga_a_caixa_de_outra_pessoa() {
    let Some(pool) = pool().await else { return };
    let c = cenario(pool).await;

    recusado(
        mail::disconnect_mailbox(
            &c.pool,
            &c.intruso,
            c.caixa_do_dono,
            &CorrelationIds::generate(),
        )
        .await,
        "desligar a caixa",
    );
}

/// Sincronizar a caixa de outra pessoa é recusado.
#[tokio::test]
async fn nao_se_sincroniza_a_caixa_de_outra_pessoa() {
    let Some(pool) = pool().await else { return };
    let c = cenario(pool).await;

    recusado(
        mail::sync(
            &c.pool,
            &registry(),
            &c.intruso,
            c.caixa_do_dono,
            MailFolder::Inbox,
            &CorrelationIds::generate(),
        )
        .await,
        "sincronizar a caixa",
    );
}

/// Marcar como lida a mensagem de outra pessoa é recusado.
#[tokio::test]
async fn nao_se_marca_a_mensagem_de_outra_pessoa() {
    let Some(pool) = pool().await else { return };
    let c = cenario(pool).await;

    recusado(
        mail::set_flag(
            &c.pool,
            &registry(),
            &c.intruso,
            c.mensagem_do_dono,
            Some(true),
            None,
        )
        .await,
        "marcar a mensagem",
    );
}

/// Enviar a partir da caixa de outra pessoa é recusado.
///
/// É a fuga com a consequência mais visível de todas: a mensagem sairia com o
/// endereço do dono, e quem a recebesse não teria como saber que não foi ele.
#[tokio::test]
async fn nao_se_envia_a_partir_da_caixa_de_outra_pessoa() {
    let Some(pool) = pool().await else { return };
    let c = cenario(pool).await;

    let destinatario = MailAddress::new("externo@exemplo.com", None, &[DOMAIN.to_owned()]);

    let resultado = mail::send(
        &c.pool,
        &registry(),
        &c.intruso,
        c.caixa_do_dono,
        OutgoingMessage {
            from: mail::sender_identity(&c.endereco_do_dono, None),
            to: vec![mail::sender_identity("externo@exemplo.com", None)],
            cc: Vec::new(),
            bcc: Vec::new(),
            subject: "Em nome de outra pessoa".to_owned(),
            body: "…".to_owned(),
            in_reply_to: None,
            references: Vec::new(),
            attachments: Vec::new(),
        },
        &[destinatario],
        &[],
        true,
        &CorrelationIds::generate(),
    )
    .await;

    recusado(resultado, "enviar a partir da caixa");
}

// ── Duplos ──────────────────────────────────────────────────────────────

/// Um fornecedor que regista o que lhe pedem, e nunca devolve conteúdo.
///
/// Se alguma destas operações lhe chegar, é porque a autorização já foi
/// atravessada — e o teste falha na asserção, não aqui.
struct FornecedorQueRegista;

#[async_trait::async_trait]
impl MailProvider for FornecedorQueRegista {
    fn adapter_name(&self) -> &'static str {
        "isolamento-de-teste"
    }

    async fn health(&self) -> ProviderHealth {
        ProviderHealth {
            endpoints: Vec::new(),
            can_read: true,
            can_send: true,
            detail: "Fornecedor de teste.".to_owned(),
            rejected_credential: false,
        }
    }

    async fn list_messages(
        &self,
        _mailbox_address: &str,
        _folder: MailFolder,
        _cursor: Option<&str>,
        _limit: u32,
    ) -> ProviderResult<MessagePage> {
        Ok(MessagePage {
            messages: Vec::new(),
            next_cursor: None,
        })
    }

    /// Devolve uma mensagem, e é deliberado.
    ///
    /// A primeira escrita devolvia `NotFound`, e o controlo positivo apanhou-o:
    /// as oito recusas passariam **pela razão errada** — o fornecedor a
    /// recusar, e não a autorização. Um duplo que recusa tudo faz uma suite de
    /// autorização verde sem nunca ter observado autorização nenhuma.
    async fn fetch_message(
        &self,
        _mailbox_address: &str,
        _folder: MailFolder,
        _provider_id: &str,
    ) -> ProviderResult<FetchedMessage> {
        Ok(FetchedMessage {
            header: ocinye_core::modules::mail::provider::MessageHeader {
                provider_id: "isolamento".to_owned(),
                message_id: None,
                thread_key: None,
                folder: MailFolder::Inbox,
                from: mail::sender_identity("externo@exemplo.com", None),
                to: Vec::new(),
                cc: Vec::new(),
                subject: Some("Assunto privado".to_owned()),
                snippet: None,
                sent_at: chrono::Utc::now(),
                is_read: false,
                is_starred: false,
                has_attachments: false,
                size_bytes: None,
            },
            text_body: Some("Corpo.".to_owned()),
            html_body: None,
            attachments: Vec::new(),
            bcc: Vec::new(),
        })
    }

    async fn fetch_attachment(
        &self,
        _mailbox_address: &str,
        _folder: MailFolder,
        _provider_id: &str,
        _part_id: &str,
    ) -> ProviderResult<Vec<u8>> {
        Err(ProviderError::NotFound)
    }

    async fn send_message(
        &self,
        _mailbox_address: &str,
        _message: &OutgoingMessage,
    ) -> ProviderResult<Option<String>> {
        Ok(None)
    }

    async fn move_message(
        &self,
        _mailbox_address: &str,
        _folder: MailFolder,
        _provider_id: &str,
        _destination: MailFolder,
    ) -> ProviderResult<()> {
        Ok(())
    }

    async fn set_read(
        &self,
        _mailbox_address: &str,
        _folder: MailFolder,
        _provider_id: &str,
        _read: bool,
    ) -> ProviderResult<()> {
        Ok(())
    }

    async fn set_starred(
        &self,
        _mailbox_address: &str,
        _folder: MailFolder,
        _provider_id: &str,
        _starred: bool,
    ) -> ProviderResult<()> {
        Ok(())
    }
}

/// Uma sonda que aceita qualquer credencial.
///
/// Deliberado: estes testes medem **autorização**, e uma sonda que recusasse
/// faria a ligação falhar antes de a autorização ser sequer consultada — o
/// teste ficaria verde sem ter observado nada.
struct SondaQueAceita;

#[async_trait::async_trait]
impl CredentialProbe for SondaQueAceita {
    async fn verify(&self, _endereco: &str, _username: &str, _senha: &str) -> ProviderResult<()> {
        Ok(())
    }
}

/// Uma caixa **ligada** lê-se com a credencial do próprio.
///
/// # Porque isto não estava coberto
///
/// Todos os testes acima usam caixas sem credencial, e nessas o registo cai
/// para o adaptador da instituição. O caminho que uma pessoa percorre de
/// verdade é o outro: com credencial guardada, o registo abre-a e constrói o
/// adaptador **dela**.
///
/// São dois caminhos, e só um estava a ser exercitado.
#[tokio::test]
async fn uma_caixa_ligada_le_se_com_a_credencial_do_proprio() {
    let Some(pool) = pool().await else { return };
    let c = cenario(pool).await;
    let k = chave();

    let registo = Arc::new(
        ProviderRegistry::new(
            Arc::new(FornecedorQueRegista),
            ocinye_core::config::MailConfig {
                institutional_domains: vec![DOMAIN.to_owned()],
                imap_host: "irrelevante".to_owned(),
                imap_port: 993,
                imap_security: ocinye_core::config::MailSecurity::ImplicitTls,
                smtp_host: "irrelevante".to_owned(),
                smtp_port: 465,
                smtp_security: ocinye_core::config::MailSecurity::ImplicitTls,
                username: String::new(),
                password: String::new(),
                max_message_bytes: 25 * 1024 * 1024,
                sealing_key: Some(k.clone()),
            },
            Some(k.clone()),
        )
        .com_construtor(Box::new(|_| {
            Ok(Arc::new(FornecedorQueRegista) as Arc<dyn MailProvider>)
        })),
    );

    mail::connect_mailbox(
        &c.pool,
        &c.dono,
        c.caixa_do_dono,
        &mail::MailboxConnection {
            chave: Some(&k),
            sonda: &SondaQueAceita,
            senha: "a-senha-da-caixa",
        },
        &CorrelationIds::generate(),
    )
    .await
    .expect("ligar a própria caixa");

    mail::read_message(
        &c.pool,
        &registo,
        &c.dono,
        c.mensagem_do_dono,
        false,
        &CorrelationIds::generate(),
    )
    .await
    .expect("ler a própria mensagem com a caixa ligada");
}
