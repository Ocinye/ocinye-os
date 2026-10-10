//! Validated identifiers: every value an operator types, parsed once at the
//! edge into a type that cannot hold anything else.
//!
//! The rule (D011_SECURITY_MATRIX #15): values travel as typed fields and as
//! separate argv elements, never concatenated into a shell string. These
//! newtypes make the second half cheap to believe — a [`SshUser`] cannot
//! contain a space, a quote or a semicolon, because it cannot be constructed
//! with one.

use std::fmt;
use std::net::{Ipv4Addr, Ipv6Addr};
use std::num::NonZeroU16;

use ocinye_contracts::access_endpoint::Hostname;
use serde::{Deserialize, Serialize};

/// Why an operator-provided value was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum InputError {
    /// Not a host name, IPv4 or IPv6 address (shell fragments, paths, spaces…).
    InvalidHost,
    /// Not a valid SSH user name.
    InvalidUser,
    /// Port 0 or not a number.
    InvalidPort,
    /// Empty, too long, or with control characters.
    InvalidText,
    /// Not an e-mail address.
    InvalidEmail,
    /// Not of the expected identifier form.
    InvalidIdentifier,
}

impl fmt::Display for InputError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidHost => "invalid host",
            Self::InvalidUser => "invalid ssh user",
            Self::InvalidPort => "invalid port",
            Self::InvalidText => "invalid text",
            Self::InvalidEmail => "invalid e-mail address",
            Self::InvalidIdentifier => "invalid identifier",
        })
    }
}

impl std::error::Error for InputError {}

/// The address of the target server.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum TargetHost {
    /// A DNS name (same normalisation as access endpoints).
    Name(HostNameValue),
    /// An IPv4 address.
    V4(Ipv4Addr),
    /// An IPv6 address.
    V6(Ipv6Addr),
}

impl TargetHost {
    /// Parse what the operator typed.
    ///
    /// # Errors
    ///
    /// [`InputError::InvalidHost`] for anything that is not exactly a name or
    /// an address — `srv-01.empresa.test; rm` included.
    pub fn parse(raw: &str) -> Result<Self, InputError> {
        let t = raw.trim();
        if t.chars().any(|c| c.is_whitespace() || c.is_control()) {
            return Err(InputError::InvalidHost);
        }
        if let Ok(ip) = t.parse::<Ipv4Addr>() {
            return Ok(Self::V4(ip));
        }
        let unbracketed = t.trim_start_matches('[').trim_end_matches(']');
        if let Ok(ip) = unbracketed.parse::<Ipv6Addr>() {
            return Ok(Self::V6(ip));
        }
        HostNameValue::parse(t).map(Self::Name)
    }

    /// The form used to connect and to display.
    #[must_use]
    pub fn as_display(&self) -> String {
        match self {
            Self::Name(n) => n.as_str().to_owned(),
            Self::V4(ip) => ip.to_string(),
            Self::V6(ip) => ip.to_string(),
        }
    }
}

/// A DNS host name: lowercase LDH labels, at least one dot, ≤ 253 characters.
///
/// The rule is [`Hostname`] — the one D010 access endpoints use — so the
/// Installer never accepts a name the Core would refuse.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct HostNameValue(String);

impl HostNameValue {
    /// # Errors
    ///
    /// [`InputError::InvalidHost`].
    pub fn parse(raw: &str) -> Result<Self, InputError> {
        // Hostname::parse trims; a value with inner whitespace or a trailing
        // shell fragment fails on the label rule.
        if raw.chars().any(|c| c.is_whitespace() || c.is_control()) {
            return Err(InputError::InvalidHost);
        }
        Hostname::parse(raw)
            .map(|h| Self(h.as_str().to_owned()))
            .map_err(|_| InputError::InvalidHost)
    }

    /// The normalised name.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for HostNameValue {
    type Error = InputError;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(&value)
    }
}

impl From<HostNameValue> for String {
    fn from(value: HostNameValue) -> Self {
        value.0
    }
}

impl fmt::Display for HostNameValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// `[a-z_][a-z0-9_-]{0,31}` — the portable POSIX user name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct SshUser(String);

impl SshUser {
    /// # Errors
    ///
    /// [`InputError::InvalidUser`].
    pub fn parse(raw: &str) -> Result<Self, InputError> {
        let bytes = raw.as_bytes();
        let ok = !bytes.is_empty()
            && bytes.len() <= 32
            && (bytes[0].is_ascii_lowercase() || bytes[0] == b'_')
            && bytes[1..]
                .iter()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || *b == b'_' || *b == b'-');
        if ok {
            Ok(Self(raw.to_owned()))
        } else {
            Err(InputError::InvalidUser)
        }
    }

    /// The user name.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for SshUser {
    type Error = InputError;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(&value)
    }
}

impl From<SshUser> for String {
    fn from(value: SshUser) -> Self {
        value.0
    }
}

/// An SSH port.
///
/// # Errors
///
/// [`InputError::InvalidPort`].
pub fn parse_port(raw: &str) -> Result<NonZeroU16, InputError> {
    raw.trim()
        .parse::<NonZeroU16>()
        .map_err(|_| InputError::InvalidPort)
}

fn bounded_text(raw: &str, max: usize) -> Result<String, InputError> {
    let t = raw.trim();
    if t.is_empty() || t.chars().count() > max || t.chars().any(char::is_control) {
        return Err(InputError::InvalidText);
    }
    Ok(t.to_owned())
}

/// The name of the Instance, as people read it (≤ 120 characters).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct InstanceName(String);

impl InstanceName {
    /// # Errors
    ///
    /// [`InputError::InvalidText`].
    pub fn parse(raw: &str) -> Result<Self, InputError> {
        bounded_text(raw, 120).map(Self)
    }

    /// The name.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The slug the Core derives from the name (`bootstrap-admin`'s
    /// `slug_do_nome`): shown read-only on I08, and compared with what the
    /// Core reports at verification.
    #[must_use]
    pub fn derived_slug(&self) -> String {
        let mut slug = String::with_capacity(self.0.len());
        for c in self.0.chars().flat_map(char::to_lowercase) {
            let base = match c {
                'á' | 'à' | 'â' | 'ã' | 'ä' => 'a',
                'é' | 'è' | 'ê' | 'ë' => 'e',
                'í' | 'ì' | 'î' | 'ï' => 'i',
                'ó' | 'ò' | 'ô' | 'õ' | 'ö' => 'o',
                'ú' | 'ù' | 'û' | 'ü' => 'u',
                'ç' => 'c',
                'ñ' => 'n',
                other => other,
            };
            if base.is_ascii_alphanumeric() {
                slug.push(base);
            } else if !slug.ends_with('-') && !slug.is_empty() {
                slug.push('-');
            }
        }
        slug.trim_end_matches('-').chars().take(64).collect()
    }
}

impl TryFrom<String> for InstanceName {
    type Error = InputError;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(&value)
    }
}

impl From<InstanceName> for String {
    fn from(value: InstanceName) -> Self {
        value.0
    }
}

/// A person's full name (≤ 200 characters, no control characters).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct PersonName(String);

impl PersonName {
    /// # Errors
    ///
    /// [`InputError::InvalidText`].
    pub fn parse(raw: &str) -> Result<Self, InputError> {
        bounded_text(raw, 200).map(Self)
    }

    /// The name.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for PersonName {
    type Error = InputError;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(&value)
    }
}

impl From<PersonName> for String {
    fn from(value: PersonName) -> Self {
        value.0
    }
}

/// An e-mail address: one `@`, a non-empty local part without whitespace or
/// control characters, and a host-name domain. Lowercased domain.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct EmailAddress(String);

impl EmailAddress {
    /// # Errors
    ///
    /// [`InputError::InvalidEmail`].
    pub fn parse(raw: &str) -> Result<Self, InputError> {
        let t = raw.trim();
        let (local, domain) = t.split_once('@').ok_or(InputError::InvalidEmail)?;
        let local_ok = !local.is_empty()
            && local.len() <= 64
            && !local
                .chars()
                .any(|c| c.is_whitespace() || c.is_control() || matches!(c, '@' | '"' | '\\'));
        let domain = HostNameValue::parse(domain).map_err(|_| InputError::InvalidEmail)?;
        if !local_ok || t.len() > 254 {
            return Err(InputError::InvalidEmail);
        }
        Ok(Self(format!("{local}@{}", domain.as_str())))
    }

    /// The address.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Case-insensitive equality, as the Core compares two identities.
    #[must_use]
    pub fn same_as(&self, other: &Self) -> bool {
        self.0.eq_ignore_ascii_case(&other.0)
    }
}

impl TryFrom<String> for EmailAddress {
    type Error = InputError;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(&value)
    }
}

impl From<EmailAddress> for String {
    fn from(value: EmailAddress) -> Self {
        value.0
    }
}

macro_rules! hex_id {
    ($name:ident, $prefix:literal, $doc:literal) => {
        #[doc = $doc]
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        #[serde(try_from = "String", into = "String")]
        pub struct $name(String);

        impl $name {
            /// Parse the textual form.
            ///
            /// # Errors
            ///
            /// [`InputError::InvalidIdentifier`].
            pub fn parse(raw: &str) -> Result<Self, InputError> {
                let ok = raw.strip_prefix($prefix).is_some_and(|hex| {
                    hex.len() == 16
                        && hex
                            .bytes()
                            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
                });
                if ok {
                    Ok(Self(raw.to_owned()))
                } else {
                    Err(InputError::InvalidIdentifier)
                }
            }

            /// Mint a new identifier from 8 random bytes.
            #[must_use]
            pub fn from_random(bytes: [u8; 8]) -> Self {
                Self(format!("{}{}", $prefix, hex::encode(bytes)))
            }

            /// The textual form.
            #[must_use]
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl TryFrom<String> for $name {
            type Error = InputError;
            fn try_from(value: String) -> Result<Self, Self::Error> {
                Self::parse(&value)
            }
        }

        impl From<$name> for String {
            fn from(value: $name) -> Self {
                value.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }
    };
}

hex_id!(
    InstallationId,
    "inst-",
    "One installation of Ocinye OS on one server (`inst-` + 16 hex). Not a secret; links the Instance, the journal and the receipt."
);
hex_id!(
    PlanId,
    "pl-",
    "One reviewed installation plan (`pl-` + 16 hex). A changed configuration is a new plan."
);

/// A lowercase SHA-256 in hex (64 characters).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Sha256Hex(String);

impl Sha256Hex {
    /// # Errors
    ///
    /// [`InputError::InvalidIdentifier`].
    pub fn parse(raw: &str) -> Result<Self, InputError> {
        if raw.len() == 64
            && raw
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            Ok(Self(raw.to_owned()))
        } else {
            Err(InputError::InvalidIdentifier)
        }
    }

    /// The hex form.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for Sha256Hex {
    type Error = InputError;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(&value)
    }
}

impl From<Sha256Hex> for String {
    fn from(value: Sha256Hex) -> Self {
        value.0
    }
}

impl fmt::Display for Sha256Hex {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn um_servidor_e_um_nome_ou_um_endereco_e_nada_mais() {
        assert_eq!(
            TargetHost::parse(" SRV-01.Empresa.TEST ").unwrap(),
            TargetHost::Name(HostNameValue::parse("srv-01.empresa.test").unwrap())
        );
        assert!(matches!(
            TargetHost::parse("192.0.2.10"),
            Ok(TargetHost::V4(_))
        ));
        assert!(matches!(
            TargetHost::parse("[2001:db8::10]"),
            Ok(TargetHost::V6(_))
        ));
        for mau in [
            "srv-01.empresa.test; rm",
            "srv-01.empresa.test && id",
            "$(id).empresa.test",
            "`id`.test",
            "srv 01.test",
            "../etc/passwd",
            "/srv/ocinye",
            "srv.test\nid.test",
            "srv.test|cat",
            "-oProxyCommand=x.test",
            "user@srv.test",
            "srv.test:22",
            "",
        ] {
            assert_eq!(
                TargetHost::parse(mau),
                Err(InputError::InvalidHost),
                "{mau:?}"
            );
        }
    }

    #[test]
    fn um_utilizador_ssh_nao_leva_metacaracteres() {
        assert!(SshUser::parse("operador").is_ok());
        assert!(SshUser::parse("_svc-1").is_ok());
        for mau in [
            "",
            "Root",
            "op erador",
            "op;id",
            "op$x",
            "-o",
            "a".repeat(33).as_str(),
            "root\n",
        ] {
            assert_eq!(SshUser::parse(mau), Err(InputError::InvalidUser), "{mau:?}");
        }
    }

    #[test]
    fn a_porta_e_um_numero_positivo() {
        assert_eq!(parse_port("22").unwrap().get(), 22);
        for mau in ["0", "-1", "65536", "22;", ""] {
            assert!(parse_port(mau).is_err(), "{mau}");
        }
    }

    #[test]
    fn os_textos_recusam_controlo_e_tamanho() {
        assert!(InstanceName::parse("Empresa Exemplo").is_ok());
        assert!(InstanceName::parse("  ").is_err());
        assert!(InstanceName::parse("a\u{7}b").is_err());
        assert!(InstanceName::parse(&"x".repeat(121)).is_err());
        assert_eq!(
            InstanceName::parse("Ciências & Engenharia")
                .unwrap()
                .derived_slug(),
            "ciencias-engenharia"
        );
    }

    #[test]
    fn o_endereco_de_correio_e_validado_e_comparado_sem_caixa() {
        let a = EmailAddress::parse("Ana@Empresa.TEST").unwrap();
        assert_eq!(a.as_str(), "Ana@empresa.test");
        assert!(a.same_as(&EmailAddress::parse("ana@empresa.test").unwrap()));
        for mau in [
            "ana",
            "ana@",
            "@x.test",
            "a b@x.test",
            "ana@x",
            "ana@x.test;rm",
        ] {
            assert!(EmailAddress::parse(mau).is_err(), "{mau}");
        }
    }

    #[test]
    fn os_identificadores_tem_forma_fixa() {
        let id = InstallationId::from_random([0xab; 8]);
        assert_eq!(id.as_str(), "inst-abababababababab");
        assert_eq!(InstallationId::parse(id.as_str()).unwrap(), id);
        assert!(InstallationId::parse("inst-ABABABABABABABAB").is_err());
        assert!(PlanId::parse("pl-0123456789abcdef").is_ok());
        assert!(PlanId::parse("inst-0123456789abcdef").is_err());
        assert!(Sha256Hex::parse(&"a".repeat(64)).is_ok());
        assert!(Sha256Hex::parse(&"A".repeat(64)).is_err());
    }
}
