//! A identidade do cliente web servido (D15 G-17, ADR-0617).
//!
//! # Porque um resumo dos estáticos, e não o SHA do release
//!
//! O HTML é sempre renderizado de novo a cada pedido; o que um separador aberto
//! pode ter de antigo são os **estáticos** — o `app.js`, as folhas de estilo.
//! A pergunta «há uma versão nova do cliente?» é portanto «os estáticos
//! mudaram?», e responde-se com um resumo deles, calculado no arranque. Não
//! depende de nenhuma variável de ambiente que um deploy pudesse esquecer, e
//! não muda quando só o servidor mudou.
//!
//! # Porque não um SHA-256
//!
//! Não é uma prova de integridade: é um nome para «estes estáticos». O hasher
//! da biblioteca padrão (chaves fixas) é determinístico para o mesmo binário, e
//! não acrescenta uma dependência de produção à Experience.

use std::path::Path;
use std::sync::OnceLock;

use std::hash::{DefaultHasher, Hasher};

static ID: OnceLock<String> = OnceLock::new();

/// Calcula a identidade a partir do directório dos estáticos. Idempotente: a
/// primeira chamada decide.
pub fn init(static_dir: &Path) {
    ID.get_or_init(|| digest(static_dir).unwrap_or_else(|| "dev".to_owned()));
}

/// A identidade do cliente servido: 12 hexadecimais, ou `dev` sem estáticos.
#[must_use]
pub fn id() -> &'static str {
    ID.get().map_or("dev", String::as_str)
}

fn digest(root: &Path) -> Option<String> {
    let mut ficheiros = Vec::new();
    let mut pilha = vec![root.to_path_buf()];
    while let Some(dir) = pilha.pop() {
        for entrada in std::fs::read_dir(&dir).ok()? {
            let caminho = entrada.ok()?.path();
            if caminho.is_dir() {
                pilha.push(caminho);
            } else {
                ficheiros.push(caminho);
            }
        }
    }
    if ficheiros.is_empty() {
        return None;
    }
    ficheiros.sort();
    let mut h = DefaultHasher::new();
    for f in &ficheiros {
        let relativo = f.strip_prefix(root).ok()?;
        h.write(relativo.to_string_lossy().as_bytes());
        h.write_u8(0);
        h.write(&std::fs::read(f).ok()?);
        h.write_u8(0);
    }
    Some(format!("{:012x}", h.finish() & 0xFFFF_FFFF_FFFF))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn muda_quando_um_estatico_muda_e_so_entao() {
        let dir = std::env::temp_dir().join(format!("ocinye-build-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("sub")).unwrap();
        std::fs::write(dir.join("app.js"), "a").unwrap();
        std::fs::write(dir.join("sub/x.css"), "b").unwrap();
        let um = digest(&dir).unwrap();
        assert_eq!(um.len(), 12);
        assert_eq!(digest(&dir).unwrap(), um, "não é determinístico");
        std::fs::write(dir.join("app.js"), "a2").unwrap();
        assert_ne!(digest(&dir).unwrap(), um, "não viu a mudança");
        std::fs::remove_dir_all(&dir).ok();
    }
}
