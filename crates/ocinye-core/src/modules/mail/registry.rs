//! Qual credencial abre a sessão de correio.
//!
//! # Duas credenciais, e a razão de serem duas
//!
//! O Ocinye autentica-se no servidor de correio de duas maneiras, consoante
//! quem age (ADR-0409):
//!
//! - **A da instituição**, lida do ambiente ao arranque. É a que o `worker` usa
//!   para indexar e a que sustenta o trabalho agentic. Não pertence a ninguém,
//!   e por isso não pode representar ninguém.
//! - **A de cada membro**, guardada cifrada quando ele liga a sua caixa. É a
//!   que abre a sessão quando é ele a ler ou a enviar, e é ela que faz com que
//!   o servidor de correio veja o autor verdadeiro de cada acção.
//!
//! Escolher errado não dá erro: dá uma acção correcta atribuída à pessoa
//! errada. É por isso que a escolha vive aqui, num sítio só, e não repetida em
//! cada handler.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use chrono::{DateTime, Utc};
use sqlx::PgPool;
use tokio::sync::Mutex;
use uuid::Uuid;

use ocinye_contracts::MailReachability;

use super::imap_smtp::{ImapSmtpConfig, ImapSmtpProvider};
use super::provider::{
    CredentialProbe, MailProvider, ProviderError, ProviderHealth, ProviderResult,
};
use super::repository as repo;
use crate::config::MailConfig;
use crate::password::sealed::{self, SealingKey};
use crate::password::Secret;
use crate::CoreResult;

/// Uma sessão aberta com a credencial de um membro.
struct Sessao {
    /// O instante da credencial que a abriu.
    ///
    /// Muda quando a pessoa volta a ligar a caixa com outra senha, e é isso que
    /// invalida esta entrada — sem ele, trocar a senha deixaria o adaptador a
    /// autenticar-se com a antiga até ao próximo reinício.
    credencial_de: DateTime<Utc>,
    adaptador: Arc<dyn MailProvider>,
}

/// Escolhe o adaptador de correio para cada acção.
pub struct ProviderRegistry {
    /// O adaptador da instituição. Nunca ausente: sem correio configurado é o
    /// `UnconfiguredProvider`, que recusa com uma razão que se lê.
    institucional: Arc<dyn MailProvider>,
    /// Hosts, portos e segurança. A senha desta configuração é a da
    /// instituição, e não entra em nenhuma sessão de membro.
    transporte: MailConfig,
    /// A chave que abre as credenciais guardadas, ausente quando esta
    /// instalação não a configurou.
    chave: Option<SealingKey>,
    /// Uma sessão por caixa ligada.
    ///
    /// Construir um adaptador lê e analisa a loja de raízes TLS inteira. Fazê-lo
    /// a cada listagem de pasta seria pagar isso por clique.
    sessoes: Mutex<HashMap<Uuid, Sessao>>,
    /// A última observação do serviço institucional, e quando foi feita.
    ///
    /// A sonda custa uma ligação TLS e um `LOGIN`, e o estado das capacidades é
    /// pedido em quase todas as páginas do Workspace. Sem isto, abrir a Home
    /// abriria uma sessão IMAP.
    ///
    /// Guardar não é assumir: passados [`SAUDE_VALIDA`], a resposta volta a ser
    /// observada. Meio minuto é curto o suficiente para quem está a arranjar a
    /// configuração ver o efeito, e longo o suficiente para uma navegação
    /// normal não tocar no servidor.
    saude: Mutex<HashMap<Option<Uuid>, (Instant, ProviderHealth)>>,
    /// Como se constrói o adaptador de uma caixa ligada.
    ///
    /// # Porque isto é uma costura e não um detalhe
    ///
    /// O trabalho deste registo é **escolher o adaptador**. Construir um tipo
    /// concreto lá dentro é o que tornava essa escolha impossível de exercitar:
    /// uma caixa com credencial produzia sempre um cliente IMAP verdadeiro,
    /// pelo que qualquer teste que abrisse uma mensagem media a rede — ou,
    /// offline, media o erro de ligação.
    ///
    /// Em produção é sempre o `ImapSmtpProvider`, e há um teste que o afirma.
    /// O que isto acrescenta é a possibilidade de descrever, sem rede, a
    /// instalação em que uma pessoa tem a caixa ligada e a ler.
    construtor: Construtor,
    /// A última observação do **transporte**, e quando foi feita.
    ///
    /// Separada da saúde dos adaptadores porque é outra pergunta e tem outro
    /// custo: um aperto de mão TLS, sem `LOGIN`, sem credencial.
    transporte_saudavel: Mutex<Option<(Instant, MailReachability)>>,
}

/// Como se constrói o adaptador de uma caixa a partir da sua credencial.
type Construtor =
    Box<dyn Fn(ImapSmtpConfig) -> ProviderResult<Arc<dyn MailProvider>> + Send + Sync>;

/// O construtor de produção: um cliente IMAP e SMTP verdadeiro.
fn construtor_de_producao(config: ImapSmtpConfig) -> ProviderResult<Arc<dyn MailProvider>> {
    ImapSmtpProvider::new(config).map(|adaptador| Arc::new(adaptador) as Arc<dyn MailProvider>)
}

/// Quanto tempo uma observação do serviço de correio continua a valer.
const SAUDE_VALIDA: Duration = Duration::from_secs(30);

impl ProviderRegistry {
    /// Constrói o registo à volta do adaptador da instituição.
    #[must_use]
    pub fn new(
        institucional: Arc<dyn MailProvider>,
        transporte: MailConfig,
        chave: Option<SealingKey>,
    ) -> Self {
        Self {
            institucional,
            transporte,
            chave,
            construtor: Box::new(construtor_de_producao),
            sessoes: Mutex::new(HashMap::new()),
            saude: Mutex::new(HashMap::new()),
            transporte_saudavel: Mutex::new(None),
        }
    }

    /// O que o serviço institucional consegue fazer, agora.
    ///
    /// Observado, e guardado por [`SAUDE_VALIDA`].
    pub async fn institutional_health(&self) -> ProviderHealth {
        self.saude_de(None, || Arc::clone(&self.institucional))
            .await
    }

    /// O que a caixa **desta pessoa** consegue fazer, agora.
    ///
    /// # Porque não basta o estado institucional
    ///
    /// São dois factos, e confundi-los apaga um deles:
    ///
    /// - **a infraestrutura** — esta instalação consegue falar com o serviço;
    /// - **a caixa de quem está a olhar** — esta pessoa ligou a sua.
    ///
    /// Uma instalação sem conta de serviço tem o adaptador institucional
    /// ausente por decisão (ADR-0409). Responder com ele diria «indisponível»
    /// a quem tem a caixa a funcionar — e diria «disponível» a quem ainda não
    /// a ligou, se houvesse conta de serviço. Nenhuma das duas é verdade sobre
    /// a pessoa que perguntou.
    ///
    /// # Errors
    ///
    /// Devolve erro quando a credencial guardada não abre com a chave desta
    /// instalação.
    pub async fn health_for(&self, pool: &PgPool, mailbox_id: Uuid) -> CoreResult<ProviderHealth> {
        let adaptador = self.for_mailbox(pool, mailbox_id).await?;
        Ok(self
            .saude_de(Some(mailbox_id), || Arc::clone(&adaptador))
            .await)
    }

    /// Uma observação guardada, por quem a fez.
    async fn saude_de(
        &self,
        chave: Option<Uuid>,
        adaptador: impl FnOnce() -> Arc<dyn MailProvider>,
    ) -> ProviderHealth {
        let mut guardadas = self.saude.lock().await;
        if let Some((quando, saude)) = guardadas.get(&chave) {
            if quando.elapsed() < SAUDE_VALIDA {
                return saude.clone();
            }
        }

        let saude = adaptador().health().await;
        guardadas.insert(chave, (Instant::now(), saude.clone()));
        saude
    }

    /// O que a sonda observou, na forma que o plano de plataforma consome.
    ///
    /// **Configuração é intenção; isto é observação.** Uma instalação com as
    /// quatro variáveis preenchidas e o serviço em baixo não está disponível,
    /// e dizer que está apresenta uma Entrada vazia a quem espera correio.
    /// O que a sonda observou do **transporte**, sem credencial nenhuma.
    ///
    /// # Porque o transporte se mede sem conta
    ///
    /// Porque «o serviço está lá» e «esta conta entra» são perguntas
    /// diferentes, e só a segunda precisa de uma senha. TCP abre, o TLS aperta
    /// a mão, o certificado valida para este nome — e nada disto exige uma
    /// conta de serviço que a instalação decidiu não ter (ADR-0409).
    ///
    /// A versão anterior devolvia um estado próprio para «sem conta de
    /// serviço» e mapeava-o em `Degraded`. Isso fazia a ausência de indexação
    /// autónoma — que já tem casa em `MailSync` — degradar também o `Mail`, e
    /// o correio de quem tinha a caixa ligada e saudável passava a parecer
    /// defeituoso.
    pub async fn reachability(&self) -> MailReachability {
        if !self.transporte.transport_configured() {
            return MailReachability::NotConfigured;
        }

        let mut guardadas = self.transporte_saudavel.lock().await;
        if let Some((quando, observado)) = guardadas.as_ref() {
            if quando.elapsed() < SAUDE_VALIDA {
                return *observado;
            }
        }

        let (leitura, envio) = tokio::join!(
            super::imap_smtp::transporte_responde(
                &self.transporte.imap_host,
                self.transporte.imap_port
            ),
            super::imap_smtp::transporte_responde(
                &self.transporte.smtp_host,
                self.transporte.smtp_port
            ),
        );

        let observado = MailReachability::observed(true, leitura, envio);
        *guardadas = Some((Instant::now(), observado));
        observado
    }

    /// O mesmo registo, com outro construtor de adaptadores.
    ///
    /// Existe para os testes poderem descrever uma caixa ligada **e a
    /// responder** sem rede. Em produção ninguém chama isto, e há um teste que
    /// verifica que o construtor por omissão é o verdadeiro.
    #[must_use]
    pub fn com_construtor(mut self, construtor: Construtor) -> Self {
        self.construtor = construtor;
        self
    }

    /// O adaptador da instituição.
    ///
    /// É o que o `worker` e o trabalho agentic usam: indexar não é um acto de
    /// ninguém em particular, e não há membro a quem atribuí-lo.
    #[must_use]
    pub fn institutional(&self) -> &Arc<dyn MailProvider> {
        &self.institucional
    }

    /// O adaptador com que uma acção sobre esta caixa deve ser feita.
    ///
    /// Devolve o do membro quando a caixa está ligada e a chave existe, e o da
    /// instituição em qualquer outro caso — que é o comportamento que o Ocinye
    /// tinha antes de as caixas se poderem ligar, e continua a ser correcto.
    ///
    /// # Errors
    ///
    /// Devolve erro quando a leitura da credencial falha, ou quando a
    /// credencial guardada não abre com a chave desta instalação.
    pub async fn for_mailbox(
        &self,
        pool: &PgPool,
        mailbox_id: Uuid,
    ) -> CoreResult<Arc<dyn MailProvider>> {
        let Some(chave) = self.chave.as_ref() else {
            return Ok(Arc::clone(&self.institucional));
        };
        let Some(credencial) = repo::credential_of(pool, mailbox_id).await? else {
            // Larga a sessão desta caixa.
            //
            // Não é isto que faz uma caixa desligada voltar à credencial da
            // instituição — isso decide-se acima, na ausência da credencial, e
            // uma reversão que apague esta linha continua com a suite verde.
            // O que esta linha faz é não deixar um adaptador autenticado vivo
            // em memória depois de a pessoa ter mandado esquecer a senha.
            self.sessoes.lock().await.remove(&mailbox_id);
            return Ok(Arc::clone(&self.institucional));
        };

        let mut sessoes = self.sessoes.lock().await;
        if let Some(sessao) = sessoes.get(&mailbox_id) {
            if sessao.credencial_de == credencial.updated_at {
                return Ok(Arc::clone(&sessao.adaptador));
            }
        }

        let senha = sealed::open(chave, &credencial.sealed)?;
        let adaptador = (self.construtor)(ImapSmtpConfig {
            imap_host: self.transporte.imap_host.clone(),
            imap_port: self.transporte.imap_port,
            imap_security: self.transporte.imap_security,
            smtp_host: self.transporte.smtp_host.clone(),
            smtp_port: self.transporte.smtp_port,
            smtp_security: self.transporte.smtp_security,
            username: credencial.username.clone(),
            password: Secret::new(senha),
        })
        .map_err(super::service::from_provider)?;

        sessoes.insert(
            mailbox_id,
            Sessao {
                credencial_de: credencial.updated_at,
                adaptador: Arc::clone(&adaptador),
            },
        );

        Ok(adaptador)
    }
}

#[async_trait::async_trait]
impl CredentialProbe for ProviderRegistry {
    /// Abre uma sessão com a credencial oferecida, e mais nada.
    ///
    /// Lista uma mensagem da Entrada porque é a operação mais barata que
    /// obriga a um `LOGIN` a sério: um `test_connection` de SMTP diria que o
    /// servidor responde, e não que esta senha entra.
    async fn verify(&self, endereco: &str, username: &str, senha: &str) -> ProviderResult<()> {
        if self.transporte.imap_host.is_empty() {
            return Err(ProviderError::NotConfigured);
        }

        let adaptador = ImapSmtpProvider::new(ImapSmtpConfig {
            imap_host: self.transporte.imap_host.clone(),
            imap_port: self.transporte.imap_port,
            imap_security: self.transporte.imap_security,
            smtp_host: self.transporte.smtp_host.clone(),
            smtp_port: self.transporte.smtp_port,
            smtp_security: self.transporte.smtp_security,
            username: username.to_owned(),
            password: Secret::new(senha.to_owned()),
        })?;

        adaptador
            .list_messages(endereco, ocinye_contracts::MailFolder::Inbox, None, 1)
            .await
            .map(|_| ())
    }
}

#[cfg(test)]
mod o_construtor_por_omissao {
    use super::*;

    /// Uma instalação nova constrói clientes IMAP verdadeiros.
    ///
    /// # O defeito que isto guarda
    ///
    /// A costura existe para os testes. Uma costura para testes que ficasse
    /// aberta em produção seria pior do que não a ter: bastaria alguém
    /// esquecer-se de a fechar para o correio institucional passar a falar com
    /// um duplo.
    ///
    /// Aqui não se pode comparar ponteiros de função, e por isso mede-se o
    /// efeito: o construtor por omissão, dado um anfitrião impossível, tem de
    /// se comportar como o cliente verdadeiro — recusa a construir, e não
    /// devolve alegremente um adaptador que responde a tudo.
    #[test]
    fn o_construtor_por_omissao_e_o_cliente_verdadeiro() {
        let config = ImapSmtpConfig {
            imap_host: String::new(),
            imap_port: 993,
            imap_security: crate::config::MailSecurity::ImplicitTls,
            smtp_host: String::new(),
            smtp_port: 465,
            smtp_security: crate::config::MailSecurity::ImplicitTls,
            username: "quem@ocinye.com".to_owned(),
            password: Secret::new(String::new()),
        };

        // O construtor que o `new()` **instala**, e não a função solta.
        //
        // A primeira escrita chamava `construtor_de_producao` directamente, e
        // por isso não media nada: uma reversão que instalasse outro
        // construtor no registo deixava este teste verde. O que se afirma é
        // sobre o registo; é ao registo que se pergunta.
        let registo = ProviderRegistry::new(
            Arc::new(super::super::provider::UnconfiguredProvider),
            crate::config::MailConfig {
                institutional_domains: vec!["ocinye.com".to_owned()],
                imap_host: "exemplo".to_owned(),
                imap_port: 993,
                imap_security: crate::config::MailSecurity::ImplicitTls,
                smtp_host: "exemplo".to_owned(),
                smtp_port: 465,
                smtp_security: crate::config::MailSecurity::ImplicitTls,
                username: String::new(),
                password: String::new(),
                max_message_bytes: 1024,
                sealing_key: None,
            },
            None,
        );

        let adaptador = (registo.construtor)(config).expect("construir");
        assert_eq!(
            adaptador.adapter_name(),
            ImapSmtpProvider::new(ImapSmtpConfig {
                imap_host: "exemplo".to_owned(),
                imap_port: 993,
                imap_security: crate::config::MailSecurity::ImplicitTls,
                smtp_host: "exemplo".to_owned(),
                smtp_port: 465,
                smtp_security: crate::config::MailSecurity::ImplicitTls,
                username: "quem@ocinye.com".to_owned(),
                password: Secret::new(String::new()),
            })
            .expect("referência")
            .adapter_name(),
            "o construtor por omissão não produz o cliente IMAP verdadeiro — uma \
             caixa ligada passaria a falar com outra coisa"
        );
    }
}
