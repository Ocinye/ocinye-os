//! The keyboard layout the machine is set to, from `/etc/default/keyboard`.
//!
//! The installer writes that file, and the System Service writes it when the
//! layout changes in Settings (`keyboard.configure`). The compositor reads it
//! at start and again whenever it changes, so a new layout applies to the
//! open session without restarting the screen. Without the file, or with a
//! value that is not an XKB name, xkbcommon's defaults stand.

use std::path::Path;
use std::time::SystemTime;

/// Where the layout lives.
pub const FILE: &str = "/etc/default/keyboard";

/// The XKB names the file sets.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Keymap {
    pub model: String,
    pub layout: String,
    pub variant: String,
    pub options: Option<String>,
}

/// Letters, digits and the punctuation XKB names use; nothing else reaches
/// xkbcommon.
fn xkb_name(v: &str) -> bool {
    v.len() <= 128
        && v.bytes().all(|b| {
            b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b',' | b':' | b'+' | b'(' | b')')
        })
}

/// Parse the `KEY="value"` lines of the file.
#[must_use]
pub fn parse(text: &str) -> Keymap {
    let mut k = Keymap::default();
    for line in text.lines() {
        let Some((key, value)) = line.trim().split_once('=') else {
            continue;
        };
        let value = value.trim().trim_matches('"').trim_matches('\'');
        if !xkb_name(value) {
            continue;
        }
        match key.trim() {
            "XKBMODEL" => k.model = value.to_owned(),
            "XKBLAYOUT" => k.layout = value.to_owned(),
            "XKBVARIANT" => k.variant = value.to_owned(),
            "XKBOPTIONS" if !value.is_empty() => k.options = Some(value.to_owned()),
            _ => {}
        }
    }
    k
}

/// The file now, with its modification time.
#[must_use]
pub fn read(path: &Path) -> (Keymap, Option<SystemTime>) {
    let when = std::fs::metadata(path).and_then(|m| m.modified()).ok();
    let map = std::fs::read_to_string(path)
        .map(|t| parse(&t))
        .unwrap_or_default();
    (map, when)
}

impl Keymap {
    /// The configuration for smithay. Empty names leave xkbcommon's default.
    #[must_use]
    pub fn config(&self) -> smithay::input::keyboard::XkbConfig<'_> {
        smithay::input::keyboard::XkbConfig {
            model: &self.model,
            layout: &self.layout,
            variant: &self.variant,
            options: self.options.clone(),
            ..Default::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn le_o_ficheiro_do_instalador() {
        let k = parse("# Chosen in the Ocinye OS installer.\nXKBMODEL=\"pc105\"\nXKBLAYOUT=\"fr\"\nXKBVARIANT=\"\"\nXKBOPTIONS=\"\"\nBACKSPACE=\"guess\"\n");
        assert_eq!(
            k,
            Keymap {
                model: "pc105".into(),
                layout: "fr".into(),
                variant: String::new(),
                options: None,
            }
        );
    }

    #[test]
    fn um_valor_que_nao_e_um_nome_xkb_nao_passa() {
        let k = parse("XKBLAYOUT=\"pt;rm -rf /\"\nXKBVARIANT=\"nodeadkeys\"\nXKBOPTIONS=\"grp:alt_shift_toggle\"\n");
        assert_eq!(k.layout, "");
        assert_eq!(k.variant, "nodeadkeys");
        assert_eq!(k.options.as_deref(), Some("grp:alt_shift_toggle"));
    }
}
