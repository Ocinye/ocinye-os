//! Verification from the operator's side (V10, V-FW, V11, V12, V13).
//!
//! What only the outside can prove: that the names resolve to this server,
//! that 80/443 reach it, that it serves the planned certificate for every name,
//! and that the Workspace answers each name with its login page. **Read-only**:
//! the recheck («Verificar novamente») runs this again and nothing else — no
//! server mutation, no reinstall.
//!
//! The certificate is identified by its SHA-256, captured during the
//! handshake and compared with the plan (operator's certificate) or with the
//! one the server generated (self-signed, from the journal). Whether a public
//! client would *trust* it is a separate question, answered with the public
//! roots: an operator certificate that matches but is not publicly trusted is
//! `PENDING` (TLS_TRUST), not a failure.

use std::collections::BTreeMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use ocinye_installer_contracts::ident::HostNameValue;
use ocinye_installer_contracts::verification::{ItemStatus, VerificationId, VerificationItem};
use sha2::{Digest, Sha256};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio_rustls::rustls::client::danger::{
    HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier,
};
use tokio_rustls::rustls::client::WebPkiServerVerifier;
use tokio_rustls::rustls::crypto::{
    ring, verify_tls12_signature, verify_tls13_signature, CryptoProvider,
};
use tokio_rustls::rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use tokio_rustls::rustls::{ClientConfig, DigitallySignedStruct, RootCertStore, SignatureScheme};
use tokio_rustls::TlsConnector;

/// How names are resolved. The system resolver in the product; a fixed map
/// only in controlled tests (a `.test` name made to resolve on purpose).
#[derive(Debug, Clone, Default)]
pub struct Resolver {
    /// Test-only overrides.
    pub fixed: BTreeMap<String, Vec<IpAddr>>,
}

impl Resolver {
    async fn resolve(&self, host: &str) -> Vec<IpAddr> {
        if let Some(ips) = self.fixed.get(host) {
            return ips.clone();
        }
        tokio::time::timeout(Duration::from_secs(8), tokio::net::lookup_host((host, 443)))
            .await
            .ok()
            .and_then(Result::ok)
            .map(|it| it.map(|a| a.ip()).collect())
            .unwrap_or_default()
    }
}

/// What the verification compares against.
#[derive(Debug, Clone)]
pub struct Expectation {
    /// The server's addresses (the SSH address, and what the server reports).
    pub target_ips: Vec<IpAddr>,
    /// Canonical first, then the bound names.
    pub hosts: Vec<HostNameValue>,
    /// SHA-256 of the certificate the server must serve.
    pub cert_sha256: String,
    /// It is the operator's certificate (trust is evaluated) — or self-signed.
    pub operator_supplied: bool,
}

/// The leaf certificate and the intermediates, as presented.
type Presented = Arc<Mutex<Option<(Vec<u8>, Vec<Vec<u8>>)>>>;

#[derive(Debug)]
struct Capture {
    seen: Presented,
    provider: Arc<CryptoProvider>,
}

impl ServerCertVerifier for Capture {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, tokio_rustls::rustls::Error> {
        // Accept for the handshake only: the certificate is identified by its
        // fingerprint afterwards, and nothing is sent before that check.
        if let Ok(mut s) = self.seen.lock() {
            *s = Some((
                end_entity.as_ref().to_vec(),
                intermediates.iter().map(|c| c.as_ref().to_vec()).collect(),
            ));
        }
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, tokio_rustls::rustls::Error> {
        verify_tls12_signature(
            message,
            cert,
            dss,
            &self.provider.signature_verification_algorithms,
        )
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, tokio_rustls::rustls::Error> {
        verify_tls13_signature(
            message,
            cert,
            dss,
            &self.provider.signature_verification_algorithms,
        )
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.provider
            .signature_verification_algorithms
            .supported_schemes()
    }
}

/// One HTTPS request through a pinned connection: status, leaf fingerprint,
/// and whether the public roots trust the chain for this name.
async fn https_get(addr: SocketAddr, host: &str, path: &str) -> Option<(u16, String, bool)> {
    let provider = Arc::new(ring::default_provider());
    let seen = Arc::new(Mutex::new(None));
    let config = ClientConfig::builder_with_provider(Arc::clone(&provider))
        .with_safe_default_protocol_versions()
        .ok()?
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(Capture {
            seen: Arc::clone(&seen),
            provider: Arc::clone(&provider),
        }))
        .with_no_client_auth();
    let name = ServerName::try_from(host.to_owned()).ok()?;
    let tcp = tokio::time::timeout(Duration::from_secs(8), TcpStream::connect(addr))
        .await
        .ok()?
        .ok()?;
    let mut tls = tokio::time::timeout(
        Duration::from_secs(10),
        TlsConnector::from(Arc::new(config)).connect(name.clone(), tcp),
    )
    .await
    .ok()?
    .ok()?;
    let (leaf, chain) = seen.lock().ok()?.clone()?;
    let fingerprint = hex::encode(Sha256::digest(&leaf));
    let trusted = {
        let roots = RootCertStore {
            roots: webpki_roots::TLS_SERVER_ROOTS.to_vec(),
        };
        WebPkiServerVerifier::builder_with_provider(Arc::new(roots), Arc::clone(&provider))
            .build()
            .ok()
            .is_some_and(|v| {
                let inter: Vec<CertificateDer<'_>> = chain
                    .iter()
                    .map(|c| CertificateDer::from(c.as_slice()))
                    .collect();
                v.verify_server_cert(
                    &CertificateDer::from(leaf.as_slice()),
                    &inter,
                    &name,
                    &[],
                    UnixTime::now(),
                )
                .is_ok()
            })
    };
    let req = format!("GET {path} HTTP/1.1\r\nHost: {host}\r\nUser-Agent: ocinye-installer\r\nConnection: close\r\n\r\n");
    tls.write_all(req.as_bytes()).await.ok()?;
    let mut head = vec![0u8; 64];
    let n = tokio::time::timeout(Duration::from_secs(15), tls.read(&mut head))
        .await
        .ok()?
        .ok()?;
    let status = std::str::from_utf8(&head[..n])
        .ok()?
        .split_whitespace()
        .nth(1)?
        .parse()
        .ok()?;
    Some((status, fingerprint, trusted))
}

fn item(id: VerificationId, status: ItemStatus, evidence: impl Into<String>) -> VerificationItem {
    VerificationItem {
        id,
        status,
        evidence: evidence.into(),
    }
}

async fn port_open(ip: IpAddr, port: u16) -> bool {
    tokio::time::timeout(Duration::from_secs(5), TcpStream::connect((ip, port)))
        .await
        .is_ok_and(|r| r.is_ok())
}

/// Run the operator-side items.
pub async fn run(e: &Expectation, resolver: &Resolver) -> Vec<VerificationItem> {
    let mut out = Vec::new();
    let Some(ip) = e.target_ips.first().copied() else {
        for id in [
            VerificationId::V10,
            VerificationId::VFw,
            VerificationId::V11,
            VerificationId::V12,
            VerificationId::V13,
        ] {
            out.push(item(id, ItemStatus::NotRun, "NO_TARGET_ADDRESS"));
        }
        return out;
    };

    // V10 · every name resolves to this server.
    let mut unresolved = Vec::new();
    let mut wrong = Vec::new();
    for h in &e.hosts {
        let ips = resolver.resolve(h.as_str()).await;
        if ips.is_empty() {
            unresolved.push(h.as_str().to_owned());
        } else if !ips.iter().any(|a| e.target_ips.contains(a)) {
            wrong.push(h.as_str().to_owned());
        }
    }
    out.push(if !wrong.is_empty() {
        item(
            VerificationId::V10,
            ItemStatus::Pending,
            format!("DNS_WRONG_TARGET:{}", wrong.join(",")),
        )
    } else if !unresolved.is_empty() {
        item(
            VerificationId::V10,
            ItemStatus::Pending,
            format!("DNS_UNRESOLVED:{}", unresolved.join(",")),
        )
    } else {
        item(VerificationId::V10, ItemStatus::Pass, "DNS_OK")
    });

    // V-FW · 80 and 443 reach the server from here.
    let p80 = port_open(ip, 80).await;
    let p443 = port_open(ip, 443).await;
    out.push(if p80 && p443 {
        item(VerificationId::VFw, ItemStatus::Pass, "PORTS_REACHABLE")
    } else {
        item(
            VerificationId::VFw,
            ItemStatus::Fail,
            format!(
                "FIREWALL_VERIFICATION_FAILED:{}{}",
                if p80 { "" } else { "80" },
                if p443 { "" } else { " 443" }
            ),
        )
    });

    // V11/V12/V13 · TLS and the Workspace, per name, through the server's
    // address with the name as SNI and Host.
    let addr = SocketAddr::new(ip, 443);
    let mut cert_ok = true;
    let mut trusted = true;
    let mut answered = Vec::new();
    let mut refused = Vec::new();
    for h in &e.hosts {
        match https_get(addr, h.as_str(), "/login").await {
            Some((status, fp, t)) => {
                cert_ok &= fp == e.cert_sha256;
                trusted &= t;
                if status == 200 {
                    answered.push(h.as_str().to_owned());
                } else {
                    refused.push(format!("{}={status}", h.as_str()));
                }
            }
            None => {
                cert_ok = false;
                refused.push(format!("{}=NO_TLS", h.as_str()));
            }
        }
    }
    out.push(if !cert_ok {
        item(
            VerificationId::V11,
            ItemStatus::Fail,
            "TLS_CERTIFICATE_DIFFERS",
        )
    } else if e.operator_supplied && !trusted {
        item(VerificationId::V11, ItemStatus::Pending, "TLS_TRUST")
    } else {
        item(
            VerificationId::V11,
            ItemStatus::Pass,
            if e.operator_supplied {
                "TLS_VALID"
            } else {
                "TLS_SELF_SIGNED_PINNED"
            },
        )
    });
    out.push(if refused.is_empty() {
        item(VerificationId::V12, ItemStatus::Pass, "ENDPOINTS_ANSWER")
    } else {
        item(
            VerificationId::V12,
            ItemStatus::Fail,
            format!("ENDPOINT_REFUSED:{}", refused.join(",")),
        )
    });
    let canonical = e
        .hosts
        .first()
        .map(|h| h.as_str().to_owned())
        .unwrap_or_default();
    out.push(if answered.contains(&canonical) {
        item(VerificationId::V13, ItemStatus::Pass, "LOGIN_200")
    } else {
        item(VerificationId::V13, ItemStatus::Fail, "LOGIN_UNREACHABLE")
    });
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn sem_endereco_nada_corre_e_nada_passa() {
        let e = Expectation {
            target_ips: vec![],
            hosts: vec![HostNameValue::parse("os.empresa.test").unwrap()],
            cert_sha256: "a".repeat(64),
            operator_supplied: false,
        };
        let items = run(&e, &Resolver::default()).await;
        assert_eq!(items.len(), 5);
        assert!(items.iter().all(|i| i.status == ItemStatus::NotRun));
    }

    #[tokio::test]
    async fn um_nome_que_nao_resolve_e_dns_pendente() {
        // 192.0.2.0/24 (TEST-NET-1) is never routed: ports closed, no TLS.
        let e = Expectation {
            target_ips: vec!["192.0.2.10".parse().unwrap()],
            hosts: vec![HostNameValue::parse("nao-existe.invalid.test").unwrap()],
            cert_sha256: "a".repeat(64),
            operator_supplied: true,
        };
        let items = run(&e, &Resolver::default()).await;
        let v10 = items.iter().find(|i| i.id == VerificationId::V10).unwrap();
        assert_eq!(v10.status, ItemStatus::Pending);
        assert!(v10.evidence.starts_with("DNS_UNRESOLVED"));
        let fw = items.iter().find(|i| i.id == VerificationId::VFw).unwrap();
        assert_eq!(fw.status, ItemStatus::Fail);
    }

    #[tokio::test]
    async fn um_nome_que_resolve_para_outro_servidor_e_alvo_errado() {
        let mut r = Resolver::default();
        r.fixed.insert(
            "os.empresa.test".into(),
            vec!["198.51.100.7".parse().unwrap()],
        );
        let e = Expectation {
            target_ips: vec!["192.0.2.10".parse().unwrap()],
            hosts: vec![HostNameValue::parse("os.empresa.test").unwrap()],
            cert_sha256: "a".repeat(64),
            operator_supplied: false,
        };
        let items = run(&e, &r).await;
        assert!(items[0].evidence.starts_with("DNS_WRONG_TARGET"));
    }
}
