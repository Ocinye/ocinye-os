//! Operator-supplied TLS material, checked locally before any plan (I11,
//! D011_TLS_MATRIX).
//!
//! Checks: the certificate parses; the private key matches it; it has not
//! expired (warning under 30 days); its names cover **every** endpoint; the
//! optional chain parses and links to it. Invalid material blocks I11 and
//! nothing reaches the server.
//!
//! The private key is read here only to compare its public half with the
//! certificate's; it is never printed, logged, put in the plan, the journal or
//! the receipt. It travels once, as a file, to a 0600 path on the server.

use std::path::Path;

use aws_lc_rs::signature::{EcdsaKeyPair, Ed25519KeyPair, KeyPair, RsaKeyPair};
use ocinye_installer_contracts::ident::HostNameValue;
use ocinye_installer_contracts::plan::TlsPlan;
use rustls_pki_types::pem::PemObject;
use rustls_pki_types::{CertificateDer, PrivateKeyDer};
use serde::Serialize;
use sha2::{Digest, Sha256};
use x509_parser::prelude::*;

/// One check, for the I11 list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TlsCheck {
    /// The certificate parses.
    Parse,
    /// The key matches the certificate.
    KeyMatch,
    /// Not expired.
    NotExpired,
    /// Covers every endpoint.
    CoversNames,
    /// The chain parses and links to the certificate.
    Chain,
}

/// The result of the I11 checks.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TlsValidation {
    /// Each check and whether it passed.
    pub checks: Vec<(TlsCheck, bool)>,
    /// `notAfter`, RFC 3339.
    pub not_after: Option<String>,
    /// Expires within 30 days (warning).
    pub expiring_soon: bool,
    /// Names the certificate covers.
    pub names: Vec<String>,
    /// Endpoints it does not cover.
    pub uncovered: Vec<String>,
    /// The plan entry, when every check passed.
    #[serde(skip)]
    pub plan: Option<TlsPlan>,
}

impl TlsValidation {
    /// Every check passed.
    #[must_use]
    pub fn valid(&self) -> bool {
        self.plan.is_some()
    }
}

fn covers(names: &[String], host: &str) -> bool {
    names.iter().any(|n| {
        n == host
            || n.strip_prefix("*.").is_some_and(|suffix| {
                host.split_once('.')
                    .is_some_and(|(label, rest)| !label.is_empty() && rest == suffix)
            })
    })
}

fn key_public(der: &PrivateKeyDer<'_>) -> Option<Vec<u8>> {
    let bytes = der.secret_der();
    if let Ok(k) = Ed25519KeyPair::from_pkcs8(bytes) {
        return Some(k.public_key().as_ref().to_vec());
    }
    for alg in [
        &aws_lc_rs::signature::ECDSA_P256_SHA256_ASN1_SIGNING,
        &aws_lc_rs::signature::ECDSA_P384_SHA384_ASN1_SIGNING,
    ] {
        if let Ok(k) = EcdsaKeyPair::from_pkcs8(alg, bytes) {
            return Some(k.public_key().as_ref().to_vec());
        }
    }
    if let Ok(k) = RsaKeyPair::from_pkcs8(bytes).or_else(|_| RsaKeyPair::from_der(bytes)) {
        return Some(k.public_key().as_ref().to_vec());
    }
    None
}

/// Validate operator TLS material for these endpoints.
#[must_use]
pub fn validate(
    cert_pem: &Path,
    key_pem: &Path,
    chain_pem: Option<&Path>,
    hosts: &[&HostNameValue],
    now: chrono::DateTime<chrono::Utc>,
) -> TlsValidation {
    let mut checks = Vec::new();
    let certs: Vec<CertificateDer<'static>> = CertificateDer::pem_file_iter(cert_pem)
        .map(|it| it.filter_map(Result::ok).collect())
        .unwrap_or_default();
    let leaf_der = certs.first();
    let parsed = leaf_der.and_then(|d| X509Certificate::from_der(d.as_ref()).ok().map(|(_, c)| c));
    checks.push((TlsCheck::Parse, parsed.is_some()));
    let Some(cert) = parsed else {
        return TlsValidation {
            checks,
            not_after: None,
            expiring_soon: false,
            names: vec![],
            uncovered: hosts.iter().map(|h| h.as_str().to_owned()).collect(),
            plan: None,
        };
    };

    let key = PrivateKeyDer::from_pem_file(key_pem).ok();
    let key_match = key
        .as_ref()
        .and_then(key_public)
        .is_some_and(|pk| pk == cert.public_key().subject_public_key.data.as_ref());
    checks.push((TlsCheck::KeyMatch, key_match));

    let not_after_ts = cert.validity().not_after.timestamp();
    let not_after = chrono::DateTime::from_timestamp(not_after_ts, 0);
    let not_before_ok = cert.validity().not_before.timestamp() <= now.timestamp();
    let not_expired = not_after.is_some_and(|na| na > now) && not_before_ok;
    checks.push((TlsCheck::NotExpired, not_expired));
    let expiring_soon = not_after.is_some_and(|na| na - now < chrono::Duration::days(30));

    let mut names: Vec<String> = cert
        .subject_alternative_name()
        .ok()
        .flatten()
        .map(|san| {
            san.value
                .general_names
                .iter()
                .filter_map(|g| match g {
                    GeneralName::DNSName(n) => Some(n.to_ascii_lowercase()),
                    _ => None,
                })
                .collect()
        })
        .unwrap_or_default();
    names.sort();
    names.dedup();
    let uncovered: Vec<String> = hosts
        .iter()
        .filter(|h| !covers(&names, h.as_str()))
        .map(|h| h.as_str().to_owned())
        .collect();
    checks.push((TlsCheck::CoversNames, uncovered.is_empty()));

    // The chain: the file's certificates and any after the leaf in the
    // certificate file; the first must have issued the leaf.
    let mut chain: Vec<CertificateDer<'static>> = certs.iter().skip(1).cloned().collect();
    if let Some(p) = chain_pem {
        match CertificateDer::pem_file_iter(p) {
            Ok(it) => chain.extend(it.filter_map(Result::ok)),
            Err(_) => chain.clear(),
        }
    }
    let chain_ok = match (chain_pem, chain.first()) {
        (None, None) => true,
        (_, Some(first)) => X509Certificate::from_der(first.as_ref())
            .is_ok_and(|(_, issuer)| issuer.subject() == cert.issuer()),
        (Some(_), None) => false,
    };
    checks.push((TlsCheck::Chain, chain_ok));

    let all = checks.iter().all(|(_, ok)| *ok);
    let plan = all.then(|| TlsPlan::OperatorSupplied {
        cert_sha256: hex::encode(Sha256::digest(leaf_der.map_or(&[][..], AsRef::as_ref))),
        not_after: not_after
            .map(|t| t.to_rfc3339_opts(chrono::SecondsFormat::Secs, true))
            .unwrap_or_default(),
        covers: names
            .iter()
            .filter_map(|n| HostNameValue::parse(n).ok())
            .collect(),
        chain: chain_pem.is_some() || certs.len() > 1,
    });
    TlsValidation {
        checks,
        not_after: not_after.map(|t| t.to_rfc3339_opts(chrono::SecondsFormat::Secs, true)),
        expiring_soon,
        names,
        uncovered,
        plan,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rcgen::{CertificateParams, Issuer, KeyPair};
    use std::fs;

    struct Files(std::path::PathBuf);

    impl Files {
        fn new(name: &str) -> Self {
            let d = std::env::temp_dir().join(format!("ocinye-tls-{name}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&d);
            fs::create_dir_all(&d).unwrap();
            Self(d)
        }
        fn put(&self, name: &str, content: &str) -> std::path::PathBuf {
            let p = self.0.join(name);
            fs::write(&p, content).unwrap();
            p
        }
    }

    impl Drop for Files {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn hosts() -> Vec<HostNameValue> {
        ["os.empresa.test", "research.empresa.test"]
            .iter()
            .map(|h| HostNameValue::parse(h).unwrap())
            .collect()
    }

    fn leaf(names: &[&str], after: (i32, u8, u8)) -> (String, KeyPair) {
        let key = KeyPair::generate().unwrap();
        let mut p =
            CertificateParams::new(names.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>())
                .unwrap();
        p.not_before = rcgen::date_time_ymd(2026, 1, 1);
        p.not_after = rcgen::date_time_ymd(after.0, after.1, after.2);
        (p.self_signed(&key).unwrap().pem(), key)
    }

    fn now() -> chrono::DateTime<chrono::Utc> {
        chrono::DateTime::parse_from_rfc3339("2026-10-04T12:00:00Z")
            .unwrap()
            .into()
    }

    fn check(v: &TlsValidation, c: TlsCheck) -> bool {
        v.checks.iter().find(|(k, _)| *k == c).unwrap().1
    }

    #[test]
    fn um_par_valido_que_cobre_todos_os_pontos_passa() {
        let f = Files::new("ok");
        let (cert, key) = leaf(&["os.empresa.test", "*.empresa.test"], (2027, 9, 30));
        let c = f.put("c.pem", &cert);
        let k = f.put("k.pem", &key.serialize_pem());
        let h = hosts();
        let v = validate(&c, &k, None, &h.iter().collect::<Vec<_>>(), now());
        assert!(v.valid(), "{:?}", v.checks);
        assert!(!v.expiring_soon);
        let Some(TlsPlan::OperatorSupplied {
            cert_sha256,
            not_after,
            ..
        }) = v.plan
        else {
            panic!("sem plano");
        };
        assert_eq!(cert_sha256.len(), 64);
        assert!(not_after.starts_with("2027-09-30"));
        // O plano não leva a chave.
        assert!(!serde_json::to_string(&v.names).unwrap().contains("PRIVATE"));
    }

    #[test]
    fn uma_chave_de_outro_certificado_e_recusada() {
        let f = Files::new("mismatch");
        let (cert, _) = leaf(&["os.empresa.test", "research.empresa.test"], (2027, 9, 30));
        let other = KeyPair::generate().unwrap();
        let c = f.put("c.pem", &cert);
        let k = f.put("k.pem", &other.serialize_pem());
        let h = hosts();
        let v = validate(&c, &k, None, &h.iter().collect::<Vec<_>>(), now());
        assert!(!v.valid());
        assert!(!check(&v, TlsCheck::KeyMatch));
    }

    #[test]
    fn um_certificado_expirado_ou_curto_nos_nomes_e_recusado() {
        let f = Files::new("expired");
        let (cert, key) = leaf(&["os.empresa.test", "research.empresa.test"], (2026, 5, 1));
        let c = f.put("c.pem", &cert);
        let k = f.put("k.pem", &key.serialize_pem());
        let h = hosts();
        let v = validate(&c, &k, None, &h.iter().collect::<Vec<_>>(), now());
        assert!(!check(&v, TlsCheck::NotExpired));
        let (cert, key) = leaf(&["os.empresa.test"], (2027, 9, 30));
        let c = f.put("c2.pem", &cert);
        let k = f.put("k2.pem", &key.serialize_pem());
        let v = validate(&c, &k, None, &h.iter().collect::<Vec<_>>(), now());
        assert!(!check(&v, TlsCheck::CoversNames));
        assert_eq!(v.uncovered, ["research.empresa.test"]);
    }

    #[test]
    fn lixo_nao_e_um_certificado_nem_uma_chave() {
        let f = Files::new("junk");
        let c = f.put("c.pem", "not a certificate");
        let k = f.put("k.pem", "not a key");
        let h = hosts();
        let v = validate(&c, &k, None, &h.iter().collect::<Vec<_>>(), now());
        assert!(!v.valid());
        assert!(!check(&v, TlsCheck::Parse));
    }

    #[test]
    fn a_cadeia_tem_de_ligar_ao_certificado() {
        let f = Files::new("chain");
        let ca_key = KeyPair::generate().unwrap();
        let mut ca = CertificateParams::new(Vec::<String>::new()).unwrap();
        ca.is_ca = rcgen::IsCa::Ca(rcgen::BasicConstraints::Unconstrained);
        ca.distinguished_name
            .push(rcgen::DnType::CommonName, "Autoridade de Teste");
        let ca_cert = ca.self_signed(&ca_key).unwrap();
        let issuer = Issuer::new(ca, ca_key);
        let leaf_key = KeyPair::generate().unwrap();
        let mut p = CertificateParams::new(vec![
            "os.empresa.test".to_owned(),
            "research.empresa.test".to_owned(),
        ])
        .unwrap();
        p.not_before = rcgen::date_time_ymd(2026, 1, 1);
        p.not_after = rcgen::date_time_ymd(2027, 9, 30);
        let leaf_cert = p.signed_by(&leaf_key, &issuer).unwrap();
        let c = f.put("c.pem", &leaf_cert.pem());
        let k = f.put("k.pem", &leaf_key.serialize_pem());
        let good = f.put("chain.pem", &ca_cert.pem());
        let h = hosts();
        let hs: Vec<_> = h.iter().collect();
        assert!(validate(&c, &k, Some(&good), &hs, now()).valid());
        let (other_ca, _) = leaf(&["outra.test"], (2027, 1, 1));
        let bad = f.put("bad.pem", &other_ca);
        let v = validate(&c, &k, Some(&bad), &hs, now());
        assert!(!check(&v, TlsCheck::Chain));
    }

    #[test]
    fn os_nomes_cobrem_se_exactamente_ou_por_um_nivel_de_curinga() {
        let names = vec!["os.empresa.test".to_owned(), "*.empresa.test".to_owned()];
        assert!(covers(&names, "os.empresa.test"));
        assert!(covers(&names, "research.empresa.test"));
        assert!(!covers(&names, "a.b.empresa.test"));
        assert!(!covers(&names, "empresa.test"));
        assert!(!covers(&["x.test".to_owned()], "y.test"));
    }
}
