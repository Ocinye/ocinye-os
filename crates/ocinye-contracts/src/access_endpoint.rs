//! D010 · Pontos de acesso (Access Endpoints; Claude Design, integrado pela Code).
//!
//! Um ponto de acesso é uma forma configurada de chegar a uma Instância:
//!   anfitrião → Instância            (genérico)
//!   anfitrião → Instância + Distribuição (fixo)
//! Nunca concede autoridade, membro, papel nem contexto (ADR-0020). Anfitrião desconhecido
//! falha fechado. Nenhuma decisão de segurança lê o texto do anfitrião (sem `contains("business")`).

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::distribution::Distribution;

/// Identidade estável do ponto de acesso. A navegação entre pontos usa este id, nunca um URL
/// vindo do pedido (sem open redirect).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EndpointId(pub Uuid);

/// Um nome de anfitrião normalizado: minúsculas, IDNA em ASCII (punycode), sem porta, sem
/// ponto final, sem curinga, sem esquema nem caminho, rótulos 1..=63, total ≤ 253.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Hostname(String);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
/// Porque um nome de anfitrião foi recusado.
pub enum HostnameError {
    /// Vazio.
    Empty,
    /// Leva esquema (`https://`) ou caminho.
    HasSchemeOrPath,
    /// Leva porta.
    HasPort,
    /// Leva curinga (`*`).
    Wildcard,
    /// Rótulo inválido, ou sem domínio (precisa de um ponto).
    InvalidLabel,
    /// Mais de 253 caracteres.
    TooLong,
}

impl std::fmt::Display for HostnameError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Empty => "nome de anfitrião vazio",
            Self::HasSchemeOrPath => "o nome de anfitrião não leva esquema nem caminho",
            Self::HasPort => "o nome de anfitrião não leva porta",
            Self::Wildcard => "o nome de anfitrião não aceita curingas",
            Self::InvalidLabel => "nome de anfitrião inválido",
            Self::TooLong => "nome de anfitrião demasiado longo",
        })
    }
}

impl std::error::Error for HostnameError {}

impl Hostname {
    /// Normaliza e valida. A conversão IDNA é da implementação (crate `idna`); aqui fica a regra.
    pub fn parse(raw: &str) -> Result<Self, HostnameError> {
        let s = raw.trim().trim_end_matches('.').to_ascii_lowercase();
        if s.is_empty() {
            return Err(HostnameError::Empty);
        }
        if s.contains("://") || s.contains('/') {
            return Err(HostnameError::HasSchemeOrPath);
        }
        if s.contains(':') {
            return Err(HostnameError::HasPort);
        }
        if s.contains('*') {
            return Err(HostnameError::Wildcard);
        }
        if s.len() > 253 {
            return Err(HostnameError::TooLong);
        }
        let ok = s.split('.').all(|l| {
            !l.is_empty()
                && l.len() <= 63
                && !l.starts_with('-')
                && !l.ends_with('-')
                && l.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
        });
        if !ok || !s.contains('.') {
            return Err(HostnameError::InvalidLabel);
        }
        Ok(Self(s))
    }
    #[must_use]
    /// O nome normalizado.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for Hostname {
    type Error = HostnameError;
    fn try_from(v: String) -> Result<Self, Self::Error> {
        Self::parse(&v)
    }
}
impl From<Hostname> for String {
    fn from(h: Hostname) -> Self {
        h.0
    }
}

/// Destino do ponto: genérico (Distribuição resolvida depois da entrada) ou fixo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "distribution", rename_all = "snake_case")]
pub enum EndpointBinding {
    /// Genérico: resolve a Instância; a Distribuição decide-se depois da entrada.
    Generic,
    /// Fixo numa Distribuição activada da Instância.
    Distribution(Distribution),
}

/// Estado operativo. `Unverified` não serve a Instância (serve S14).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EndpointState {
    /// Serve a Instância.
    Active,
    /// Desactivado: não serve entrada nova (S40, S13).
    Disabled,
    /// Por verificar: não serve a Instância (S14).
    Unverified,
}

/// O que o Core observa da ligação segura. O Core não emite certificados (D011).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TlsObservation {
    /// Ainda não observada.
    NotObserved,
    /// Observação em curso, sem resultado.
    Pending,
    /// Certificado válido para este nome.
    Valid,
    /// Certificado inválido ou ausente.
    Invalid,
}

/// O que o Core observa da resolução do nome. O Ocinye não gere DNS.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DnsObservation {
    /// Ainda não observada.
    NotObserved,
    /// Resolve para o mesmo endereço que o ponto canónico.
    ResolvesHere,
    /// Resolve para outro endereço.
    ResolvesElsewhere,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
/// Um ponto de acesso configurado (ADR-0020 §1).
pub struct AccessEndpoint {
    /// Identidade estável (navegação só por id).
    pub id: EndpointId,
    /// Nome normalizado, único no servidor.
    pub hostname: Hostname,
    /// Genérico ou fixo numa Distribuição.
    pub binding: EndpointBinding,
    /// Estado operativo.
    pub state: EndpointState,
    /// Exactamente um por Instância; activo; não se desactiva nem apaga sem outro canónico.
    pub canonical: bool,
    /// Última observação da resolução do nome.
    pub dns: DnsObservation,
    /// Última observação da ligação segura.
    pub tls: TlsObservation,
}

/// Resultado da resolução do anfitrião de um pedido (middleware do Workspace).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostResolution {
    /// Serve a Instância por este ponto.
    Endpoint(AccessEndpoint),
    /// Nenhum ponto configurado para este nome: S13, sem identidade da Instância, sem redirect.
    Unknown,
    /// Existe mas está desactivado: S40 (sessão) ou S13-equivalente na entrada.
    Disabled,
    /// Existe mas não está verificado/operacional: S14, servido fora da Instância.
    NotOperational,
}

/// De onde vem o anfitrião do pedido. Só se aceita `X-Forwarded-Host` de um proxy de confiança
/// configurado (lista CIDR tipada); caso contrário usa-se `Host`. Nunca `Forwarded` arbitrário.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostSource {
    /// O `Host` do pedido.
    Direct,
    /// `X-Forwarded-Host` vindo de um proxy de confiança.
    TrustedProxy,
}

/// Recusas de administração (ADR-0111).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum EndpointRefusal {
    /// O nome já existe nesta Instância (S28).
    Conflict,
    /// O nome não passou a validação.
    Invalid {
        /// Porquê.
        reason: HostnameError,
    },
    /// Desactivar/apagar o canónico ou o único activo (S32).
    LastOrCanonical,
    /// Fixar numa Distribuição não activada.
    DistributionNotEnabled,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn o_nome_normaliza_se_e_recusa_o_que_nao_e_um_anfitriao() {
        assert_eq!(
            Hostname::parse(" OS.Empresa.COM. ").unwrap().as_str(),
            "os.empresa.com"
        );
        assert_eq!(Hostname::parse("127.0.0.1").unwrap().as_str(), "127.0.0.1");
        for (raw, err) in [
            ("", HostnameError::Empty),
            ("https://os.empresa.com", HostnameError::HasSchemeOrPath),
            ("os.empresa.com/x", HostnameError::HasSchemeOrPath),
            ("os.empresa.com:443", HostnameError::HasPort),
            ("*.empresa.com", HostnameError::Wildcard),
            ("localhost", HostnameError::InvalidLabel),
            ("-x.empresa.com", HostnameError::InvalidLabel),
            ("a..b.com", HostnameError::InvalidLabel),
            ("empresa_x.com", HostnameError::InvalidLabel),
            ("é.empresa.com", HostnameError::InvalidLabel),
        ] {
            assert_eq!(Hostname::parse(raw), Err(err), "«{raw}»");
        }
        let longo = format!("{}.com", "a.".repeat(130));
        assert_eq!(Hostname::parse(&longo), Err(HostnameError::TooLong));
    }
}
