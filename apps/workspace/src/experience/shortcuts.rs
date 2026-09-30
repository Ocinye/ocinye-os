//! Os atalhos de teclado que o Ocinye declara (D007 · HP-02).
//!
//! Uma lista, e só uma: a Ajuda desenha-a, a paleta da Nye e o alternador de
//! janelas lêem daqui a sua dica. Cada atalho existe porque um script da casca o
//! trata — o guarda `cada_atalho_declarado_tem_quem_o_trate` procura o
//! tratamento no JS, e uma entrada sem ele (ou um atalho novo no JS sem entrada
//! aqui, quando se acrescentar à sonda) falha o teste.
//!
//! As teclas são neutras de plataforma, como a dica do alternador (ADR-0611):
//! `Ctrl` lê-se `⌘` num Mac, e o `runtime.js` é o único a perguntar ao ambiente.

/// Um atalho: as teclas e a chave i18n do que faz.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Shortcut {
    /// As teclas, como se mostram.
    pub keys: &'static str,
    /// O que faz (chave i18n).
    pub what_key: &'static str,
}

/// Abrir a Nye (`oc-shell.js`).
pub const NYE: Shortcut = Shortcut {
    keys: "Ctrl K",
    what_key: "help.k.nye",
};

/// O alternador de janelas (`wm-engine.js`).
pub const SWITCHER: Shortcut = Shortcut {
    keys: "Alt + W",
    what_key: "prod.help.k.switcher",
};

/// Todos, pela ordem em que a Ajuda os mostra.
pub const SHORTCUTS: [Shortcut; 7] = [
    NYE,
    Shortcut {
        keys: "Ctrl J",
        what_key: "prod.help.k.launcher",
    },
    SWITCHER,
    Shortcut {
        keys: "Esc",
        what_key: "help.k.esc",
    },
    Shortcut {
        keys: "↑ ↓",
        what_key: "help.k.lists",
    },
    Shortcut {
        keys: "Home · End",
        what_key: "help.k.ends",
    },
    Shortcut {
        keys: "Ctrl Enter",
        what_key: "prod.help.k.send",
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    fn js(name: &str) -> String {
        std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("static")
                .join(name),
        )
        .unwrap_or_else(|e| panic!("{name}: {e}"))
    }

    /// Cada atalho declarado tem, num script da casca, o código que o trata.
    #[test]
    fn cada_atalho_declarado_tem_quem_o_trate() {
        let sonda: [(&str, &str, &str); 7] = [
            ("Ctrl K", "oc-shell.js", "k === 'k' && palette"),
            ("Ctrl J", "oc-shell.js", "k === 'j' && launcher"),
            ("Alt + W", "wm-engine.js", "e.code !== 'KeyW'"),
            ("Esc", "oc-apps.js", "e.key === 'Escape'"),
            ("↑ ↓", "oc-apps.js", "e.key === 'ArrowDown'"),
            ("Home · End", "oc-apps.js", "e.key === 'End'"),
            (
                "Ctrl Enter",
                "oc-apps.js",
                "e.key === 'Enter' && (e.metaKey || e.ctrlKey)",
            ),
        ];
        assert_eq!(sonda.len(), SHORTCUTS.len());
        for (s, (keys, file, handler)) in SHORTCUTS.iter().zip(sonda) {
            assert_eq!(s.keys, keys);
            assert!(js(file).contains(handler), "{keys}: {file} não o trata");
        }
    }

    /// Nenhum atalho aponta para uma chave de texto que não exista.
    #[test]
    fn cada_atalho_diz_o_que_faz() {
        for s in SHORTCUTS {
            assert_ne!(crate::i18n::t(s.what_key), s.what_key, "{}", s.what_key);
        }
    }
}
