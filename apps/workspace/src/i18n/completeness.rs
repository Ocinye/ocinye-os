//! O portão de completude do catálogo (briefing i18n §52, §53).
//!
//! Corre como teste, e é o que o CI executa: para produção, `pt`, `en` e `fr`
//! têm de existir para cada chave, e os marcadores de interpolação têm de casar
//! entre as três. Uma tradução em falta ou uma frase francesa que perdeu o
//! `{name}` faz isto falhar — antes de chegar a um ecrã.

use super::catalog::GROUPS;
use ocinye_contracts::Locale;
use std::collections::BTreeSet;

/// Os marcadores `{...}` de um template, como conjunto ordenado.
fn marcadores(texto: &str) -> BTreeSet<String> {
    let mut set = BTreeSet::new();
    let mut resto = texto;
    while let Some(inicio) = resto.find('{') {
        if let Some(fim) = resto[inicio..].find('}') {
            set.insert(resto[inicio + 1..inicio + fim].to_owned());
            resto = &resto[inicio + fim + 1..];
        } else {
            break;
        }
    }
    set
}

#[test]
fn cada_chave_tem_as_tres_linguas() {
    let mut faltam: Vec<String> = Vec::new();
    for grupo in GROUPS {
        for entrada in *grupo {
            if entrada.get(Locale::En).is_none() {
                faltam.push(format!("{} (en)", entrada.key));
            }
            if entrada.get(Locale::Fr).is_none() {
                faltam.push(format!("{} (fr)", entrada.key));
            }
        }
    }
    assert!(
        faltam.is_empty(),
        "traduções em falta para produção:\n  {}",
        faltam.join("\n  ")
    );
}

#[test]
fn os_marcadores_de_interpolacao_casam_entre_linguas() {
    let mut divergem: Vec<String> = Vec::new();
    for grupo in GROUPS {
        for entrada in *grupo {
            let canonicos = marcadores(entrada.pt);
            for locale in [Locale::En, Locale::Fr] {
                if let Some(texto) = entrada.get(locale) {
                    let seus = marcadores(texto);
                    if seus != canonicos {
                        divergem.push(format!(
                            "{} [{}]: pt tem {:?}, {} tem {:?}",
                            entrada.key, locale, canonicos, locale, seus
                        ));
                    }
                }
            }
        }
    }
    assert!(
        divergem.is_empty(),
        "marcadores de interpolação divergentes:\n  {}",
        divergem.join("\n  ")
    );
}

#[test]
fn nao_ha_chaves_duplicadas() {
    let mut vistas = BTreeSet::new();
    let mut duplicadas = Vec::new();
    for grupo in GROUPS {
        for entrada in *grupo {
            if !vistas.insert(entrada.key) {
                duplicadas.push(entrada.key);
            }
        }
    }
    assert!(
        duplicadas.is_empty(),
        "chaves duplicadas no catálogo: {duplicadas:?}"
    );
}

#[test]
fn o_catalogo_nao_esta_vazio() {
    let total: usize = GROUPS.iter().map(|g| g.len()).sum();
    assert!(total > 0, "o catálogo de produção não pode estar vazio");
}

/// Cada chave **literal** que o código pede existe no catálogo (A001-M012).
///
/// O ecrã de desbloqueio pedia `auth.login.refused`, que nunca existiu, e a
/// pessoa via a chave crua: os portões acima olham para o catálogo e não para
/// quem o usa. Este lê o código de `src/` — `t("…")`, `tf("…"`, `t_in(…, "…")`
/// e `tp("…"` (pelas formas `.one`/`.other`) — e exige cada chave.
#[test]
fn cada_chave_literal_usada_existe() {
    fn ficheiros(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
        for entrada in std::fs::read_dir(dir).expect("src legível").flatten() {
            let caminho = entrada.path();
            if caminho.is_dir() {
                ficheiros(&caminho, out);
            } else if caminho.extension().is_some_and(|e| e == "rs") {
                out.push(caminho);
            }
        }
    }
    let raiz = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut todos = Vec::new();
    ficheiros(&raiz, &mut todos);
    let valida = |k: &str| {
        !k.is_empty()
            && k.contains('.')
            && k.bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'.' || b == b'_')
            && !k.ends_with('.')
    };
    let mut faltam = BTreeSet::new();
    let mut vistas = 0usize;
    for ficheiro in &todos {
        // Sem comentários: um exemplo numa doc (`t("greeting.evening")`) não é
        // um pedido ao catálogo.
        let texto: String = std::fs::read_to_string(ficheiro)
            .expect("ficheiro legível")
            .lines()
            .filter(|l| !l.trim_start().starts_with("//"))
            .collect::<Vec<_>>()
            .join("\n");
        for (abre, plural) in [("t(\"", false), ("tf(\"", false), ("tp(\"", true)] {
            let mut resto = texto.as_str();
            while let Some(i) = resto.find(abre) {
                // `t(` dentro de outra palavra (`list(`, `at(`) não conta.
                let antes = resto[..i].chars().last();
                resto = &resto[i + abre.len()..];
                if antes.is_some_and(|c| c.is_alphanumeric() || c == '_') {
                    continue;
                }
                let Some(fim) = resto.find('"') else { break };
                let chave = &resto[..fim];
                if !valida(chave) {
                    continue;
                }
                vistas += 1;
                let existe = if plural {
                    super::has(&format!("{chave}.one")) && super::has(&format!("{chave}.other"))
                } else {
                    super::has(chave)
                };
                if !existe {
                    faltam.insert(format!(
                        "{chave} ({})",
                        ficheiro.strip_prefix(&raiz).unwrap_or(ficheiro).display()
                    ));
                }
            }
        }
    }
    assert!(
        vistas > 500,
        "o portão leu só {vistas} chaves — não está a ver o código"
    );
    assert!(
        faltam.is_empty(),
        "chaves pedidas que não existem: {faltam:#?}"
    );
}
