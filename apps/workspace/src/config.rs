//! Workspace configuration.

use std::env;
use std::time::Duration;

use anyhow::{bail, Context, Result};

/// Configuration read from the environment.
#[derive(Debug, Clone)]
pub struct WorkspaceConfig {
    /// Address to bind.
    pub bind_address: String,
    /// A topologia por onde o Workspace alcança o Core.
    ///
    /// Governa quando é que plaintext é aceitável em produção. Ver
    /// [`CoreTransport`].
    pub core_transport: CoreTransport,
    /// Public base URL of the Workspace.
    pub public_url: String,
    /// Base URL of the Ocinye Core.
    pub core_url: String,
    /// How long a session lives.
    pub session_ttl: Duration,
    /// Whether the session cookie carries the `Secure` attribute.
    pub cookie_secure: bool,
    /// Log level.
    pub log_level: String,
    /// Log format.
    pub log_format: String,
    /// Whether this is a production deployment.
    pub is_production: bool,
    /// Onde estão os ficheiros estáticos.
    ///
    /// # Porque isto é configuração e não uma constante
    ///
    /// Era `"apps/workspace/static"`, relativo ao directório de trabalho: o
    /// servidor só funcionava se lançado da raiz do repositório, e falhava em
    /// silêncio noutro sítio — o HTML chegava e o JS não, o que faz uma
    /// interface parecer partida sem dizer porquê.
    pub static_dir: String,
}

fn var(key: &str) -> Option<String> {
    env::var(key)
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

/// Como o Workspace alcança o Core.
///
/// # Porque isto é topologia, e não um interruptor
///
/// > **TLS atravessa fronteiras de confiança; transporte local dentro do mesmo
/// > anfitrião de confiança pode continuar em claro.**
///
/// Uma variável do género `OCINYE_ALLOW_INSECURE_CORE=true` transformaria uma
/// política numa opção — e uma política que se desliga não é uma política. O
/// que se declara aqui é a **topologia do transporte**, e ela é depois
/// confrontada com o endereço: declarar não chega, e o endereço sozinho também
/// não.
///
/// É esta a precisão do ADR-0103, e não o seu relaxamento. O objectivo dele —
/// nenhuma credencial em claro numa rede que alguém possa observar — fica
/// intacto; o que muda é deixar de confundir «rede» com «socket dentro do
/// mesmo anfitrião».
///
/// # O que acontece quando o Core sair deste servidor
///
/// A excepção deixa de se aplicar sozinha. Um Core noutra máquina não é
/// alcançável por loopback nem pela rede de serviço do Compose local, e a
/// configuração que hoje é válida passa a ser recusada no arranque — que é
/// exactamente o comportamento desejado.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoreTransport {
    /// O transporte sai deste anfitrião, ou pode sair. Exige TLS.
    Roteavel,
    /// O mesmo processo de sistema operativo, por loopback.
    Loopback,
    /// A rede de serviço do Compose, neste anfitrião, sem porta publicada.
    ContentorLocal,
}

impl CoreTransport {
    /// Lê a declaração. Ausente significa a resposta conservadora.
    fn from_env(var: impl Fn(&str) -> Option<String>) -> Result<Self> {
        match var("OCINYE_CORE_TRANSPORT").as_deref() {
            None | Some("") | Some("routable") => Ok(Self::Roteavel),
            Some("loopback") => Ok(Self::Loopback),
            Some("local-container") => Ok(Self::ContentorLocal),
            Some(outro) => bail!(
                "OCINYE_CORE_TRANSPORT desconhecido: «{outro}». \
                 Os valores são `routable`, `loopback` e `local-container`."
            ),
        }
    }
}

/// O anfitrião de um endereço `http(s)://anfitriao[:porto][/…]`.
fn anfitriao_de(url: &str) -> &str {
    let sem_esquema = url.split("://").nth(1).unwrap_or(url);
    let autoridade = sem_esquema.split('/').next().unwrap_or(sem_esquema);
    // Um endereço IPv6 vem entre parênteses rectos; o porto vem depois deles.
    if let Some(fim) = autoridade.find(']') {
        return &autoridade[..=fim];
    }
    autoridade.split(':').next().unwrap_or(autoridade)
}

/// Se este endereço é loopback.
fn e_loopback(anfitriao: &str) -> bool {
    matches!(anfitriao, "localhost" | "127.0.0.1" | "[::1]" | "::1")
        || anfitriao.starts_with("127.")
}

/// Se este endereço tem a forma de um **nome de serviço** do Compose.
///
/// Uma etiqueta só, sem pontos: é o que o DNS interno do Docker resolve, e é
/// tudo o que ele resolve. Um `10.0.50.20` não é um nome de serviço — é uma
/// máquina algures numa rede privada, que pode não ser esta. Um
/// `core.internal.empresa` também não: tem pontos, e um resolvedor qualquer
/// pode devolvê-lo a partir de outro sítio.
///
/// Isto **não** concede confiança por si. É a metade que confirma que o
/// endereço é coerente com a topologia declarada; sem a declaração, um nome de
/// uma etiqueta continua a ser recusado.
fn e_nome_de_servico(anfitriao: &str) -> bool {
    !anfitriao.is_empty()
        && !anfitriao.contains('.')
        && !anfitriao.contains(':')
        && !anfitriao.starts_with('[')
        && anfitriao != "localhost"
        && anfitriao
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

impl WorkspaceConfig {
    /// Read configuration from the environment.
    ///
    /// # Errors
    ///
    /// Returns an error when a required value is missing, or when a production
    /// deployment is configured in a way that would weaken session security.
    pub fn from_env() -> Result<Self> {
        let is_production =
            var("OCINYE_ENVIRONMENT").is_some_and(|value| value.eq_ignore_ascii_case("production"));

        let public_url = var("OCINYE_WORKSPACE_PUBLIC_URL")
            .context("OCINYE_WORKSPACE_PUBLIC_URL is required")?
            .trim_end_matches('/')
            .to_owned();

        let config = Self {
            bind_address: var("OCINYE_WORKSPACE_BIND_ADDRESS")
                .unwrap_or_else(|| "0.0.0.0:8090".to_owned()),
            core_url: var("OCINYE_CORE_URL")
                .context("OCINYE_CORE_URL is required")?
                .trim_end_matches('/')
                .to_owned(),
            session_ttl: Duration::from_secs(
                var("OCINYE_WORKSPACE_SESSION_TTL_SECONDS")
                    .and_then(|value| value.parse().ok())
                    .unwrap_or(8 * 3600),
            ),
            // Secure cookies are required in production and default on
            // elsewhere; only an explicit opt-out for plain-HTTP local work
            // turns them off.
            cookie_secure: is_production
                || !var("OCINYE_WORKSPACE_COOKIE_INSECURE")
                    .is_some_and(|value| value == "1" || value.eq_ignore_ascii_case("true")),
            log_level: var("OCINYE_LOG_LEVEL").unwrap_or_else(|| "info".to_owned()),
            log_format: var("OCINYE_LOG_FORMAT").unwrap_or_else(|| {
                if is_production {
                    "json".into()
                } else {
                    "pretty".into()
                }
            }),
            public_url,
            is_production,
            static_dir: var("OCINYE_WORKSPACE_STATIC_DIR")
                .unwrap_or_else(|| "apps/workspace/static".to_owned()),
            core_transport: CoreTransport::from_env(var)?,
        };

        config.validate()?;
        Ok(config)
    }

    fn validate(&self) -> Result<()> {
        if !self.is_production {
            return Ok(());
        }

        if !self.public_url.starts_with("https://") {
            bail!("OCINYE_WORKSPACE_PUBLIC_URL must use https in production");
        }
        if !self.cookie_secure {
            bail!("session cookies must be Secure in production");
        }
        // A ligação ao Core transporta credenciais no arranque de sessão: sob o
        // ADR-0103 este é o troço por onde uma palavra-passe passa.
        //
        // TLS atravessa fronteiras de confiança. Dentro do mesmo anfitrião, e
        // só aí, o transporte pode continuar em claro — e a excepção depende da
        // **topologia declarada**, confrontada com o endereço. Nem a declaração
        // sozinha, nem o endereço sozinho.
        if self.core_url.starts_with("https://") {
            return Ok(());
        }

        let anfitriao = anfitriao_de(&self.core_url);
        match self.core_transport {
            CoreTransport::Loopback if e_loopback(anfitriao) => Ok(()),
            CoreTransport::ContentorLocal if e_nome_de_servico(anfitriao) => Ok(()),

            // Declarada local, endereço que não é. Um `10.0.50.20` é uma máquina
            // algures numa rede privada — que pode não ser esta. Um
            // `core.internal.empresa` tem pontos, e um resolvedor qualquer pode
            // devolvê-lo a partir de outro sítio.
            CoreTransport::Loopback | CoreTransport::ContentorLocal => bail!(
                "OCINYE_CORE_URL é «{}», e OCINYE_CORE_TRANSPORT declara transporte \
                 local. O anfitrião «{anfitriao}» não é coerente com essa topologia: \
                 loopback exige 127.0.0.1 ou localhost, e a rede de serviço exige um \
                 nome de uma etiqueta. Um endereço privado ou um nome com pontos \
                 podem estar noutra máquina, e por isso exigem TLS.",
                self.core_url
            ),

            // Sem declaração de topologia local, plaintext é recusado — mesmo
            // para `http://core:8080`. Não é o nome que concede confiança.
            CoreTransport::Roteavel => bail!(
                "OCINYE_CORE_URL usa http em produção e OCINYE_CORE_TRANSPORT não \
                 declara transporte local a este anfitrião. Credenciais não \
                 atravessam em claro uma fronteira de rede roteável: use https, ou \
                 declare `loopback` ou `local-container` se o Core corre neste \
                 anfitrião sem porta publicada."
            ),
        }
    }

    // Não há aqui um `redirect_uri`, e a rota que ele nomeava também não existe.
    //
    // Devolvia `{public_url}/auth/callback`, e não há `/auth/callback` no
    // catálogo do Workspace: a entrada acontece pelo formulário, contra o Core.
    // Uma configuração que descreve um caminho inexistente é pior do que
    // nenhuma — parece que o fluxo existe.
}

#[cfg(test)]
mod fronteira_de_transporte {
    use super::{CoreTransport, WorkspaceConfig};
    use std::time::Duration;

    /// Uma configuração de produção com o transporte e o endereço em causa.
    ///
    /// Tudo o resto é válido de propósito: o que se mede é uma decisão, e uma
    /// configuração com dois defeitos não diz qual dos dois a guarda apanhou.
    fn producao(core_url: &str, core_transport: CoreTransport) -> WorkspaceConfig {
        WorkspaceConfig {
            bind_address: "0.0.0.0:8090".to_owned(),
            core_transport,
            public_url: "https://os.ocinye.com".to_owned(),
            core_url: core_url.to_owned(),
            session_ttl: Duration::from_secs(3600),
            cookie_secure: true,
            log_level: "info".to_owned(),
            log_format: "json".to_owned(),
            is_production: true,
            static_dir: "/srv/ocinye/static".to_owned(),
        }
    }

    /// TLS é sempre aceite, seja qual for a topologia declarada.
    #[test]
    fn tls_atravessa_qualquer_fronteira() {
        for transporte in [
            CoreTransport::Roteavel,
            CoreTransport::Loopback,
            CoreTransport::ContentorLocal,
        ] {
            assert!(
                producao("https://api.ocinye.com", transporte)
                    .validate()
                    .is_ok(),
                "https recusado com {transporte:?}"
            );
        }
    }

    /// A topologia desta primeira instalação: um anfitrião, sem porta publicada.
    #[test]
    fn o_contentor_local_do_mesmo_anfitriao_pode_falar_em_claro() {
        assert!(
            producao("http://core:8080", CoreTransport::ContentorLocal)
                .validate()
                .is_ok(),
            "o transporte local declarado foi recusado"
        );
        assert!(
            producao("http://127.0.0.1:8080", CoreTransport::Loopback)
                .validate()
                .is_ok(),
            "o loopback declarado foi recusado"
        );
    }

    /// **Não é o nome `core` que concede confiança.**
    ///
    /// Este é o teste que separa uma política de um interruptor: o mesmo
    /// endereço que passa com a topologia declarada é recusado sem ela.
    #[test]
    fn sem_a_topologia_declarada_o_mesmo_endereco_e_recusado() {
        let erro = producao("http://core:8080", CoreTransport::Roteavel)
            .validate()
            .expect_err("plaintext sem topologia local devia ser recusado");
        assert!(
            erro.to_string()
                .contains("não \n             declara transporte local")
                || erro.to_string().contains("declara transporte local"),
            "a recusa não explica o que falta: {erro}"
        );
    }

    /// Um nome público continua a exigir TLS, mesmo com a topologia declarada.
    ///
    /// Declarar `local-container` e apontar para `api.ocinye.com` seria dizer
    /// que a Internet é o mesmo anfitrião.
    #[test]
    fn um_nome_publico_e_recusado_mesmo_com_topologia_local() {
        for endereco in ["http://api.ocinye.com", "http://example.org"] {
            for transporte in [CoreTransport::ContentorLocal, CoreTransport::Loopback] {
                assert!(
                    producao(endereco, transporte).validate().is_err(),
                    "{endereco} foi aceite com {transporte:?}"
                );
            }
        }
    }

    /// Um endereço privado não prova que a máquina é esta.
    ///
    /// `10.0.50.20` é uma máquina algures numa rede privada. Pode ser este
    /// servidor, pode ser outro do outro lado de um túnel — e a diferença entre
    /// os dois é toda a diferença.
    #[test]
    fn um_endereco_privado_nao_prova_o_mesmo_anfitriao() {
        for endereco in [
            "http://10.0.50.20:8080",
            "http://172.16.4.9:8080",
            "http://192.168.1.20:8080",
        ] {
            assert!(
                producao(endereco, CoreTransport::ContentorLocal)
                    .validate()
                    .is_err(),
                "{endereco} foi aceite como transporte local"
            );
        }
    }

    /// Um nome com pontos pode ser resolvido a partir de outro sítio.
    #[test]
    fn um_nome_com_pontos_nao_e_um_servico_local() {
        for endereco in [
            "http://core.internal.empresa:8080",
            "http://core.some-network:8080",
            "http://core.internal:8080",
        ] {
            assert!(
                producao(endereco, CoreTransport::ContentorLocal)
                    .validate()
                    .is_err(),
                "{endereco} foi aceite como serviço da rede local"
            );
        }
    }

    /// Fora de produção nada disto se aplica: quem desenvolve corre em claro.
    #[test]
    fn fora_de_producao_a_guarda_nao_se_aplica() {
        let mut config = producao("http://localhost:8080", CoreTransport::Roteavel);
        config.is_production = false;
        config.cookie_secure = false;
        config.public_url = "http://localhost:8090".to_owned();
        assert!(config.validate().is_ok());
    }

    /// Um valor desconhecido é recusado, e não interpretado como o conservador.
    #[test]
    fn uma_topologia_desconhecida_e_recusada() {
        let erro = CoreTransport::from_env(|chave| {
            (chave == "OCINYE_CORE_TRANSPORT").then(|| "same-host-ish".to_owned())
        })
        .expect_err("um valor inventado devia ser recusado");
        assert!(
            erro.to_string().contains("same-host-ish"),
            "a recusa não diz qual foi o valor: {erro}"
        );
    }
}
