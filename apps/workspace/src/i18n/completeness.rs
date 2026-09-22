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
