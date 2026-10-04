//! D010 · Pontos de acesso no Workspace (ADR-0020).
//!
//! Antes de qualquer rota, o anfitrião do pedido resolve-se num ponto de acesso
//! configurado desta Instância — pelo Core, por configuração tipada, nunca pelo
//! texto do nome. **O anfitrião escolhe o destino; nunca concede autoridade.**
//!
//! - desconhecido → S13, falha fechada: sem identidade da Instância, sem
//!   redireccionamento, sem Instância por omissão;
//! - desactivado → S40 (com sessão, que termina) ou S13 (sem);
//! - por verificar → S14, servido fora da Instância;
//! - o Core não responde → S36.
//!
//! # De onde vem o anfitrião
//!
//! Do `Host`. `X-Forwarded-Host` só é lido quando o par TCP está numa lista
//! CIDR configurada (`OCINYE_TRUSTED_PROXIES`), e só com um valor; um cabeçalho
//! de origem não confiável é ignorado, nunca combinado. `Forwarded` não é lido.

use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use axum::extract::{ConnectInfo, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{Html, IntoResponse, Response};
use ocinye_contracts::access_endpoint::Hostname;
use ocinye_contracts::Distribution;
use serde::Deserialize;
use uuid::Uuid;

use crate::WorkspaceState;

/// Um bloco CIDR de proxies de confiança.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cidr {
    network: IpAddr,
    prefix: u8,
}

impl Cidr {
    /// `10.0.0.0/8`, `::1/128`, ou um endereço sozinho (= /32 ou /128).
    ///
    /// # Errors
    ///
    /// A message for anything else.
    pub fn parse(raw: &str) -> Result<Self, String> {
        let raw = raw.trim();
        let (addr, prefix) = match raw.split_once('/') {
            Some((a, p)) => (a, Some(p)),
            None => (raw, None),
        };
        let network: IpAddr = addr
            .parse()
            .map_err(|_| format!("endereço inválido em OCINYE_TRUSTED_PROXIES: «{raw}»"))?;
        let max = if network.is_ipv4() { 32 } else { 128 };
        let prefix =
            match prefix {
                Some(p) => p.parse::<u8>().ok().filter(|p| *p <= max).ok_or_else(|| {
                    format!("prefixo inválido em OCINYE_TRUSTED_PROXIES: «{raw}»")
                })?,
                None => max,
            };
        Ok(Self { network, prefix })
    }

    /// Se o endereço está neste bloco.
    #[must_use]
    pub fn contains(&self, ip: IpAddr) -> bool {
        match (self.network, ip) {
            (IpAddr::V4(n), IpAddr::V4(a)) => {
                let mask = if self.prefix == 0 {
                    0
                } else {
                    u32::MAX << (32 - u32::from(self.prefix))
                };
                (u32::from(n) & mask) == (u32::from(a) & mask)
            }
            (IpAddr::V6(n), IpAddr::V6(a)) => {
                let mask = if self.prefix == 0 {
                    0
                } else {
                    u128::MAX << (128 - u32::from(self.prefix))
                };
                (u128::from(n) & mask) == (u128::from(a) & mask)
            }
            _ => false,
        }
    }
}

/// O anfitrião deste pedido, normalizado (sem porta), ou `None` quando não é
/// um nome de anfitrião válido.
#[must_use]
pub fn request_host(headers: &HeaderMap, peer: Option<IpAddr>, trusted: &[Cidr]) -> Option<String> {
    let from_proxy = peer.is_some_and(|ip| trusted.iter().any(|c| c.contains(ip)));
    let raw = if from_proxy {
        // Um só valor: uma lista (`a, b`) é recusada, não combinada.
        match headers
            .get_all("x-forwarded-host")
            .iter()
            .collect::<Vec<_>>()
            .as_slice()
        {
            [one] => one.to_str().ok().filter(|v| !v.contains(','))?,
            [] => headers.get(header::HOST)?.to_str().ok()?,
            _ => return None,
        }
    } else {
        headers.get(header::HOST)?.to_str().ok()?
    };
    let raw = raw.trim();
    let host = match raw.rsplit_once(':') {
        Some((h, port)) if !h.contains(':') && port.chars().all(|c| c.is_ascii_digit()) => h,
        _ => raw,
    };
    Hostname::parse(host).ok().map(|h| h.as_str().to_owned())
}

/// O ponto de acesso deste pedido, como o Core o resolveu.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Endpoint {
    /// Identidade estável.
    pub endpoint_id: Uuid,
    /// A Distribuição fixa, se o ponto é fixo.
    pub binding: Option<String>,
    /// `active` · `disabled` · `unverified`.
    pub state: String,
    /// Se a Distribuição fixa ainda está activada.
    pub binding_enabled: bool,
    /// Muda quando o ponto muda.
    pub revision: String,
    /// O nome da Instância, mostrado antes da entrada.
    pub instance_name: String,
    /// O anfitrião (preenchido pelo Workspace).
    #[serde(default)]
    pub host: String,
}

impl Endpoint {
    /// A Distribuição a que o ponto está preso, se é fixo.
    #[must_use]
    pub fn bound(&self) -> Option<Distribution> {
        self.binding.as_deref().and_then(|d| d.parse().ok())
    }
}

/// O que um anfitrião dá.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Resolution {
    /// Serve a Instância.
    Serve(Endpoint),
    /// Nome que não é um ponto desta Instância (S13).
    Unknown,
    /// Existe e está desactivado (S40 / S13).
    Disabled(Endpoint),
    /// Existe e ainda não serve (S14).
    Unverified,
    /// O Core não respondeu (S36).
    CoreDown,
}

/// Cache curta das resoluções (o mesmo anfitrião em cada pedido de uma página).
#[derive(Clone, Default)]
pub struct HostCache(Arc<Mutex<HashMap<String, (Instant, Resolution)>>>);

const TTL: Duration = Duration::from_secs(5);

impl HostCache {
    fn get(&self, host: &str) -> Option<Resolution> {
        let map = self.0.lock().ok()?;
        map.get(host)
            .filter(|(at, _)| at.elapsed() < TTL)
            .map(|(_, r)| r.clone())
    }

    /// Testes sem Core: a resolução de um anfitrião, dada à mão e sem prazo.
    #[cfg(test)]
    pub(crate) fn seed(&self, host: &str, r: Resolution) {
        if let Ok(mut map) = self.0.lock() {
            map.insert(
                host.to_owned(),
                (Instant::now() + Duration::from_secs(3600), r),
            );
        }
    }

    fn put(&self, host: &str, r: &Resolution) {
        if let Ok(mut map) = self.0.lock() {
            // Cheio: saem os expirados. Se continuar cheio, o novo não entra —
            // e nunca se esvazia tudo, que deixava uma enxurrada de `Host`
            // inventados expulsar os nomes verdadeiros (A001-L008).
            if map.len() >= 256 {
                map.retain(|_, (at, _)| at.elapsed() < TTL);
            }
            if map.len() >= 256 && !map.contains_key(host) {
                return;
            }
            map.insert(host.to_owned(), (Instant::now(), r.clone()));
        }
    }

    /// Esquece tudo (depois de uma mudança feita por este Workspace).
    pub fn clear(&self) {
        if let Ok(mut map) = self.0.lock() {
            map.clear();
        }
    }
}

/// Resolve um anfitrião pelo Core (público, `GET /api/v1/access/resolve`).
pub async fn resolve(state: &WorkspaceState, host: &str) -> Resolution {
    if let Some(hit) = state.hosts.get(host) {
        return hit;
    }
    let url = format!("{}/api/v1/access/resolve", state.config.core_url);
    let answer = state
        .http
        .get(url)
        .query(&[("host", host)])
        .timeout(Duration::from_secs(3))
        .send()
        .await;
    let resolution = match answer {
        Ok(r) if r.status() == StatusCode::NOT_FOUND => Resolution::Unknown,
        Ok(r) if r.status().is_success() => match r.json::<Endpoint>().await {
            Ok(mut e) => {
                e.host = host.to_owned();
                match e.state.as_str() {
                    "active" => Resolution::Serve(e),
                    "disabled" => Resolution::Disabled(e),
                    _ => Resolution::Unverified,
                }
            }
            Err(_) => Resolution::CoreDown,
        },
        _ => Resolution::CoreDown,
    };
    if resolution != Resolution::CoreDown {
        state.hosts.put(host, &resolution);
    }
    resolution
}

tokio::task_local! {
    static CURRENT: Endpoint;
}

/// O ponto de acesso do pedido em curso (posto pelo [`layer`]).
#[must_use]
pub fn current() -> Option<Endpoint> {
    CURRENT.try_with(Clone::clone).ok()
}

/// Corre `f` dentro do escopo de um ponto (testes de vistas).
pub async fn with_endpoint<F: std::future::Future>(endpoint: Endpoint, f: F) -> F::Output {
    CURRENT.scope(endpoint, f).await
}

fn is_static(path: &str) -> bool {
    path.starts_with("/static/") || path == "/health" || path == "/favicon.ico"
}

/// O middleware: resolve o anfitrião antes de qualquer rota.
pub async fn layer(
    State(state): State<WorkspaceState>,
    request: axum::extract::Request,
    next: axum::middleware::Next,
) -> Response {
    let path = request.uri().path().to_owned();
    if is_static(&path) {
        return next.run(request).await;
    }
    let peer = request
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map(|c| c.0.ip());
    let host = request_host(request.headers(), peer, &state.config.trusted_proxies);
    let shown = host.clone().unwrap_or_default();
    let resolution = match &host {
        Some(h) => resolve(&state, h).await,
        None => Resolution::Unknown,
    };
    let session_id = crate::session::session_id_from_cookies(
        request
            .headers()
            .get(header::COOKIE)
            .and_then(|v| v.to_str().ok()),
    );
    match resolution {
        Resolution::Serve(endpoint) => CURRENT.scope(endpoint, next.run(request)).await,
        // Mudar a língua da página de recusa só escreve um cookie deste
        // anfitrião: não toca em nada da Instância, e o botão funciona.
        _ if path == "/login/language" => next.run(request).await,
        Resolution::Unknown => stop_page(StatusCode::NOT_FOUND, Stop::Unknown(shown)),
        Resolution::Disabled(endpoint) => {
            // S40 só a quem tinha uma sessão viva neste ponto: termina-a e
            // apaga o cookie. Depois disso (ou sem sessão) é um nome que não
            // serve a Instância (S13).
            let live = session_id
                .as_deref()
                .and_then(|sid| state.sessions.get(sid));
            if let Some(sid) = &session_id {
                state.sessions.remove(sid);
            }
            if let Some(session) = &live {
                crate::routes::end_core_session(&state, &session.access_token);
            }
            if live.is_some() {
                let mut response =
                    stop_page(StatusCode::GONE, Stop::EndpointDisabled(endpoint.host));
                if let Ok(v) = header::HeaderValue::from_str(&crate::session::clear_cookie_header(
                    state.config.cookie_secure,
                )) {
                    response.headers_mut().append(header::SET_COOKIE, v);
                }
                response
            } else {
                stop_page(StatusCode::NOT_FOUND, Stop::Unknown(shown))
            }
        }
        Resolution::Unverified => {
            stop_page(StatusCode::SERVICE_UNAVAILABLE, Stop::Misconfigured(shown))
        }
        Resolution::CoreDown => stop_page(StatusCode::SERVICE_UNAVAILABLE, Stop::CoreDown),
    }
}

/// As páginas servidas fora da Instância.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Stop {
    /// S13 · anfitrião desconhecido.
    Unknown(String),
    /// S14 · ponto por verificar / ligação segura inválida.
    Misconfigured(String),
    /// S36 · o Core não responde neste endereço.
    CoreDown,
    /// S40 · o ponto foi desactivado a meio da sessão.
    EndpointDisabled(String),
}

fn stop_page(status: StatusCode, stop: Stop) -> Response {
    let body = crate::ui::screens::auth::access::stop_document(&stop);
    let mut response = (status, Html(body)).into_response();
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        header::HeaderValue::from_static("no-store"),
    );
    response
}

/// A origem que um pedido escrito tem de trazer neste ponto: o esquema e a
/// porta da instalação, o anfitrião **deste** ponto (ADR-0020 §7) — não
/// qualquer ponto da Instância.
#[must_use]
pub fn expected_origin(public_url: &str, endpoint_host: &str) -> String {
    let (scheme, rest) = public_url
        .split_once("://")
        .unwrap_or(("https", public_url));
    let authority = rest.split('/').next().unwrap_or_default();
    // A porta da instalação (sem ela em produção, `https` na 443); um endereço
    // IPv6 entre parênteses rectos não tem porta depois de um `:` solto.
    let port = match authority.rsplit_once(':') {
        Some((h, p)) if !h.contains('[') && p.chars().all(|c| c.is_ascii_digit()) => {
            format!(":{p}")
        }
        Some((h, p)) if h.ends_with(']') && p.chars().all(|c| c.is_ascii_digit()) => {
            format!(":{p}")
        }
        _ => String::new(),
    };
    format!("{scheme}://{endpoint_host}{port}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn h(pairs: &[(&str, &str)]) -> HeaderMap {
        let mut m = HeaderMap::new();
        for (k, v) in pairs {
            m.append(
                header::HeaderName::from_bytes(k.as_bytes()).unwrap(),
                v.parse().unwrap(),
            );
        }
        m
    }

    #[test]
    fn o_anfitriao_vem_do_host_e_so_de_um_proxy_de_confianca_vem_o_encaminhado() {
        let proxy = [Cidr::parse("10.0.0.0/8").unwrap()];
        let headers = h(&[
            ("host", "os.empresa.com:443"),
            ("x-forwarded-host", "research.empresa.com"),
        ]);
        // De fora: o X-Forwarded-Host é ignorado.
        let outsider: IpAddr = "203.0.113.9".parse().unwrap();
        assert_eq!(
            request_host(&headers, Some(outsider), &proxy).as_deref(),
            Some("os.empresa.com")
        );
        // Sem lista: ignorado.
        let inside: IpAddr = "10.1.2.3".parse().unwrap();
        assert_eq!(
            request_host(&headers, Some(inside), &[]).as_deref(),
            Some("os.empresa.com")
        );
        // Do proxy de confiança: usado.
        assert_eq!(
            request_host(&headers, Some(inside), &proxy).as_deref(),
            Some("research.empresa.com")
        );
        // Dois valores, ou uma lista: recusado (nunca combinado).
        let two = h(&[
            ("host", "os.empresa.com"),
            ("x-forwarded-host", "a.empresa.com"),
            ("x-forwarded-host", "b.empresa.com"),
        ]);
        assert_eq!(request_host(&two, Some(inside), &proxy), None);
        let list = h(&[
            ("host", "os.empresa.com"),
            ("x-forwarded-host", "a.empresa.com, b.empresa.com"),
        ]);
        assert_eq!(request_host(&list, Some(inside), &proxy), None);
        // Um Host que não é um nome não resolve nada.
        assert_eq!(
            request_host(&h(&[("host", "*.empresa.com")]), None, &proxy),
            None
        );
        assert_eq!(
            request_host(&h(&[("host", "localhost:8080")]), None, &proxy),
            None
        );
        assert_eq!(request_host(&HeaderMap::new(), None, &proxy), None);
    }

    #[test]
    fn cidr() {
        let c = Cidr::parse("192.168.0.0/16").unwrap();
        assert!(c.contains("192.168.4.5".parse().unwrap()));
        assert!(!c.contains("192.169.0.1".parse().unwrap()));
        assert!(!c.contains("::1".parse().unwrap()));
        assert!(Cidr::parse("::1").unwrap().contains("::1".parse().unwrap()));
        assert!(Cidr::parse("10.0.0.0/33").is_err());
        assert!(Cidr::parse("nope").is_err());
    }

    #[test]
    fn a_origem_esperada_e_a_deste_ponto() {
        assert_eq!(
            expected_origin("https://os.empresa.com", "www.empresa.com"),
            "https://www.empresa.com"
        );
        assert_eq!(
            expected_origin("http://127.0.0.1:18081", "a.ocinye.test"),
            "http://a.ocinye.test:18081"
        );
    }
}
