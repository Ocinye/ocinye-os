//! The fingerprint of an OpenPGP public key, computed here and not by `gpg`.
//!
//! When the Installer has to install Docker (P04), it downloads Docker's apt
//! key and **compares its fingerprint with the one the release manifest
//! carries** (`prerequisites.docker_repo_key_fingerprint`). HTTPS is not trust:
//! the key is accepted only if the fingerprints are equal, and the bytes that
//! were checked are the bytes that are written to the keyring.
//!
//! Computing the fingerprint in Rust removes a dependency on `gpg` being on a
//! minimal server, and makes the rule testable: an RFC 4880 v4 fingerprint is
//! the SHA-1 of `0x99 ‖ u16 length ‖ public-key packet body`.
//!
//! The file must hold **exactly one primary key**. apt trusts every key in a
//! `Signed-By` keyring, so a second primary key appended by someone in the
//! path would otherwise ride along with the right one.

use base64::Engine as _;
use sha1::{Digest, Sha1};

/// Why a key file was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyError {
    /// Not an ASCII-armored public key block.
    NotArmored,
    /// The base64 or the packet framing is malformed.
    Malformed,
    /// The first packet is not a v4 public key.
    NotV4PublicKey,
    /// More than one primary key in the file.
    SeveralPrimaryKeys,
}

impl std::fmt::Display for KeyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::NotArmored => "not an armored OpenPGP public key",
            Self::Malformed => "malformed OpenPGP data",
            Self::NotV4PublicKey => "not a v4 OpenPGP public key",
            Self::SeveralPrimaryKeys => "more than one primary key",
        })
    }
}

impl std::error::Error for KeyError {}

fn dearmor(text: &str) -> Result<Vec<u8>, KeyError> {
    const BEGIN: &str = "-----BEGIN PGP PUBLIC KEY BLOCK-----";
    const END: &str = "-----END PGP PUBLIC KEY BLOCK-----";
    let start = text.find(BEGIN).ok_or(KeyError::NotArmored)?;
    let rest = &text[start + BEGIN.len()..];
    let end = rest.find(END).ok_or(KeyError::NotArmored)?;
    // A second block after the first would be a second key the check never saw.
    if rest[end + END.len()..].contains("-----BEGIN") || text[..start].contains("-----BEGIN") {
        return Err(KeyError::SeveralPrimaryKeys);
    }
    let mut lines = rest[..end].lines();
    // Armor headers end at the first blank line.
    let mut body = String::new();
    let mut in_headers = true;
    for line in lines.by_ref() {
        let line = line.trim();
        if in_headers {
            if line.is_empty() {
                in_headers = false;
            } else if !line.contains(':') {
                in_headers = false;
                body.push_str(line);
            }
            continue;
        }
        if line.starts_with('=') {
            break; // CRC-24 checksum line
        }
        body.push_str(line);
    }
    base64::engine::general_purpose::STANDARD
        .decode(body)
        .map_err(|_| KeyError::Malformed)
}

/// One packet: (tag, body).
fn packets(data: &[u8]) -> Result<Vec<(u8, &[u8])>, KeyError> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < data.len() {
        let header = data[i];
        if header & 0x80 == 0 {
            return Err(KeyError::Malformed);
        }
        i += 1;
        let (tag, len) = if header & 0x40 != 0 {
            let tag = header & 0x3f;
            let o1 = *data.get(i).ok_or(KeyError::Malformed)?;
            i += 1;
            let len = match o1 {
                0..=191 => usize::from(o1),
                192..=223 => {
                    let o2 = *data.get(i).ok_or(KeyError::Malformed)?;
                    i += 1;
                    ((usize::from(o1) - 192) << 8) + usize::from(o2) + 192
                }
                255 => {
                    let b = data.get(i..i + 4).ok_or(KeyError::Malformed)?;
                    i += 4;
                    usize::try_from(u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
                        .map_err(|_| KeyError::Malformed)?
                }
                // Partial body lengths are not used in key material.
                _ => return Err(KeyError::Malformed),
            };
            (tag, len)
        } else {
            let tag = (header >> 2) & 0x0f;
            let len = match header & 0x03 {
                0 => {
                    let b = *data.get(i).ok_or(KeyError::Malformed)?;
                    i += 1;
                    usize::from(b)
                }
                1 => {
                    let b = data.get(i..i + 2).ok_or(KeyError::Malformed)?;
                    i += 2;
                    usize::from(u16::from_be_bytes([b[0], b[1]]))
                }
                2 => {
                    let b = data.get(i..i + 4).ok_or(KeyError::Malformed)?;
                    i += 4;
                    usize::try_from(u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
                        .map_err(|_| KeyError::Malformed)?
                }
                _ => return Err(KeyError::Malformed),
            };
            (tag, len)
        };
        let body = data.get(i..i + len).ok_or(KeyError::Malformed)?;
        i += len;
        out.push((tag, body));
    }
    Ok(out)
}

/// The v4 fingerprint (40 uppercase hex) of the single primary key in an
/// ASCII-armored public key file.
///
/// # Errors
///
/// [`KeyError`] for anything but exactly one v4 primary public key.
pub fn primary_fingerprint(armored: &str) -> Result<String, KeyError> {
    let data = dearmor(armored)?;
    let packets = packets(&data)?;
    let primaries: Vec<&[u8]> = packets
        .iter()
        .filter(|(tag, _)| *tag == 6)
        .map(|(_, body)| *body)
        .collect();
    match (packets.first(), primaries.as_slice()) {
        (Some((6, _)), [body]) => {
            if body.first() != Some(&4) {
                return Err(KeyError::NotV4PublicKey);
            }
            let len = u16::try_from(body.len()).map_err(|_| KeyError::Malformed)?;
            let mut h = Sha1::new();
            h.update([0x99]);
            h.update(len.to_be_bytes());
            h.update(body);
            Ok(hex::encode_upper(h.finalize()))
        }
        (_, [_, _, ..]) => Err(KeyError::SeveralPrimaryKeys),
        _ => Err(KeyError::NotV4PublicKey),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DOCKER: &str = include_str!("../tests/fixtures/docker-ubuntu.asc");
    /// Docker's published fingerprint (docs.docker.com/engine/install/ubuntu).
    const DOCKER_FP: &str = "9DC858229FC7DD38854AE2D88D81803C0EBFCD88";

    #[test]
    fn a_chave_publica_do_docker_tem_a_impressao_publicada() {
        assert_eq!(primary_fingerprint(DOCKER).unwrap(), DOCKER_FP);
    }

    #[test]
    fn uma_chave_alterada_nao_tem_a_mesma_impressao() {
        // Trocar um carácter do corpo muda o material ou parte o pacote.
        let i = DOCKER.find("mQINB").unwrap() + 40;
        let mut bytes = DOCKER.as_bytes().to_vec();
        bytes[i] = if bytes[i] == b'A' { b'B' } else { b'A' };
        let altered = String::from_utf8(bytes).unwrap();
        assert_ne!(
            primary_fingerprint(&altered).ok().as_deref(),
            Some(DOCKER_FP)
        );
    }

    #[test]
    fn uma_segunda_chave_colada_e_recusada() {
        let two = format!("{DOCKER}\n{DOCKER}");
        assert_eq!(primary_fingerprint(&two), Err(KeyError::SeveralPrimaryKeys));
    }

    #[test]
    fn lixo_nao_e_uma_chave() {
        assert_eq!(
            primary_fingerprint("<html>503</html>"),
            Err(KeyError::NotArmored)
        );
        assert!(primary_fingerprint(
            "-----BEGIN PGP PUBLIC KEY BLOCK-----\n\n!!!\n-----END PGP PUBLIC KEY BLOCK-----"
        )
        .is_err());
    }
}
