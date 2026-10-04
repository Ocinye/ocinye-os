//! Canonical JSON: the bytes a hash is taken over.
//!
//! The release manifest and the installation plan are identified by the
//! SHA-256 of their canonical serialisation (D011_FINAL_REVIEW_CORRECTION §5):
//! UTF-8, keys sorted lexicographically (by bytes) at every level, no
//! insignificant whitespace, integers only, arrays in the order given.
//!
//! The ordering is done here, explicitly, and not inherited from whatever map
//! `serde_json` happens to be built with: a feature flag elsewhere in the
//! dependency graph must never be able to change a hash.

use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};

/// Why a value has no canonical form.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CanonicalError {
    /// A floating-point number: canonical JSON carries integers only.
    Float,
    /// The value could not be represented as JSON at all.
    Unserialisable(String),
}

impl std::fmt::Display for CanonicalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Float => f.write_str("canonical JSON carries integers only"),
            Self::Unserialisable(why) => write!(f, "not serialisable: {why}"),
        }
    }
}

impl std::error::Error for CanonicalError {}

/// The canonical bytes of a serialisable value.
///
/// # Errors
///
/// [`CanonicalError::Float`] for any non-integer number.
pub fn to_canonical<T: Serialize>(value: &T) -> Result<String, CanonicalError> {
    let value =
        serde_json::to_value(value).map_err(|e| CanonicalError::Unserialisable(e.to_string()))?;
    let mut out = String::new();
    write(&value, &mut out)?;
    Ok(out)
}

/// The SHA-256 (lowercase hex) of the canonical bytes.
///
/// # Errors
///
/// As [`to_canonical`].
pub fn sha256_of<T: Serialize>(value: &T) -> Result<String, CanonicalError> {
    Ok(sha256_hex(to_canonical(value)?.as_bytes()))
}

/// SHA-256 of raw bytes, lowercase hex.
#[must_use]
pub fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn write(value: &Value, out: &mut String) -> Result<(), CanonicalError> {
    match value {
        Value::Null => out.push_str("null"),
        Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Value::Number(n) => {
            if n.is_f64() {
                return Err(CanonicalError::Float);
            }
            out.push_str(&n.to_string());
        }
        Value::String(s) => out.push_str(&quote(s)),
        Value::Array(items) => {
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write(item, out)?;
            }
            out.push(']');
        }
        Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort_by(|a, b| a.as_bytes().cmp(b.as_bytes()));
            out.push('{');
            for (i, key) in keys.into_iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                out.push_str(&quote(key));
                out.push(':');
                write(&map[key], out)?;
            }
            out.push('}');
        }
    }
    Ok(())
}

fn quote(s: &str) -> String {
    // serde_json's string escaping is deterministic (RFC 8259, minimal escapes).
    serde_json::to_string(s).unwrap_or_else(|_| String::from("\"\""))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn as_chaves_ordenam_se_a_todos_os_niveis_e_sem_espacos() {
        let v = json!({"b": 1, "a": {"z": [3, 2, {"y": true, "x": null}], "c": "é\n"}});
        assert_eq!(
            to_canonical(&v).unwrap(),
            r#"{"a":{"c":"é\n","z":[3,2,{"x":null,"y":true}]},"b":1}"#
        );
    }

    #[test]
    fn o_mesmo_valor_logico_da_os_mesmos_bytes() {
        let a: Value = serde_json::from_str(r#"{ "x": 1,  "y": [ "a" ] }"#).unwrap();
        let b: Value = serde_json::from_str(r#"{"y":["a"],"x":1}"#).unwrap();
        assert_eq!(to_canonical(&a).unwrap(), to_canonical(&b).unwrap());
        assert_eq!(sha256_of(&a).unwrap(), sha256_of(&b).unwrap());
    }

    #[test]
    fn um_numero_decimal_nao_tem_forma_canonica() {
        assert_eq!(to_canonical(&json!({"x": 1.5})), Err(CanonicalError::Float));
    }

    #[test]
    fn a_soma_e_estavel() {
        // Fixa os bytes: mudar o serializador muda isto, e um plano confirmado
        // deixaria de conferir.
        assert_eq!(
            sha256_of(&json!({"b": [1, 2], "a": "x"})).unwrap(),
            sha256_hex(br#"{"a":"x","b":[1,2]}"#)
        );
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }
}
