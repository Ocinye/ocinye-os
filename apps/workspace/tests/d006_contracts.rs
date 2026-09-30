//! Contratos estáticos da D006 (Organização e Administração).
//!
//! Guardas que um teste de comportamento não vê: o que o código de produção
//! **não** pode conter. A credencial temporária não é guardada, registada nem
//! lida da área de transferência; o catálogo de papéis nunca é uma tabela
//! escrita à mão (a fixture de referência tem listas ilustrativas); e nenhuma
//! pessoa da fixture chega ao produto.

use std::path::Path;

fn read(p: &str) -> String {
    std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join(p))
        .unwrap_or_else(|e| panic!("{p}: {e}"))
}

/// O código antes do primeiro módulo de testes, sem linhas de comentário.
fn production(src: &str) -> String {
    src.split("#[cfg(test)]")
        .next()
        .unwrap_or_default()
        .lines()
        .filter(|l| {
            let t = l.trim_start();
            !(t.starts_with("//") || t.starts_with("/*") || t.starts_with('*'))
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn rust_sources(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
    for e in std::fs::read_dir(dir).expect("dir").flatten() {
        let p = e.path();
        if p.is_dir() {
            rust_sources(&p, out);
        } else if p.extension().is_some_and(|x| x == "rs") {
            out.push(p);
        }
    }
}

/// O script da credencial e da confirmação só mostra, copia a pedido e
/// anuncia: nada vai para `localStorage`/`sessionStorage`, para a consola, nem
/// se lê a área de transferência.
#[test]
fn a_credencial_nao_e_guardada_nem_registada_no_browser() {
    let js = read("static/oc-apps.js");
    let start = js.find("function credential(").expect("credential()");
    let end = js[start..]
        .find("function confirmDlg(")
        .map_or(js.len(), |i| start + i);
    let bloco = &js[start..end];
    for proibido in [
        "localStorage",
        "sessionStorage",
        "indexedDB",
        "console.",
        "readText",
        "history.",
        "location",
        "fetch(",
        "document.cookie",
    ] {
        assert!(
            !bloco.contains(proibido),
            "o tratamento da credencial usa `{proibido}`"
        );
    }
    // Copiar só por acção explícita (um clique), nunca ao carregar.
    assert!(bloco.contains("addEventListener('click'"));
    assert!(!js.contains("clipboard.readText"));
}

/// Nenhum registo das rotas e controladores da organização menciona a
/// credencial: o segredo nunca entra num log.
#[test]
fn a_credencial_nunca_entra_num_registo() {
    for f in ["src/routes/org.rs", "src/controllers/org.rs"] {
        let src = production(&read(f));
        for linha in src.lines() {
            let l = linha.to_lowercase();
            let regista = l.contains("tracing::")
                || l.contains("println!")
                || l.contains("eprintln!")
                || l.contains("dbg!")
                || l.contains("log::");
            assert!(
                !(regista
                    && (l.contains("secret")
                        || l.contains("temporary_password")
                        || l.contains("credential"))),
                "{f}: {linha}"
            );
        }
        assert!(!src.contains("dbg!("), "{f} tem dbg!");
    }
}

/// O catálogo de papéis vem do Core: o único sítio de produção que constrói um
/// `OrgRoleDefVm` lê-o de JSON (`role_catalogue`). Uma tabela escrita à mão
/// (como as listas ilustrativas da fixture) seria um segundo lugar.
#[test]
fn o_catalogo_de_papeis_so_vem_do_core() {
    let mut files = Vec::new();
    rust_sources(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("src"),
        &mut files,
    );
    let mut sitios = Vec::new();
    for f in files {
        let src = production(&std::fs::read_to_string(&f).expect("ler"));
        // Construções, não a definição do tipo.
        let construcoes = src.matches("OrgRoleDefVm {").count()
            - src.matches("struct OrgRoleDefVm {").count();
        if construcoes > 0 {
            sitios.push(f.display().to_string());
        }
    }
    assert_eq!(sitios.len(), 1, "{sitios:?}");
    assert!(sitios[0].ends_with("controllers/org.rs"), "{sitios:?}");
    let ctl = production(&read("src/controllers/org.rs"));
    let catalogo = &ctl[ctl.find("pub fn role_catalogue").expect("role_catalogue")..];
    let catalogo = &catalogo[..catalogo.find("\n}\n").expect("fim")];
    assert!(catalogo.contains(r#".get("permissions")"#));
    assert!(!catalogo.contains("vec![\""), "permissões escritas à mão");
}

/// Ninguém da fixture de referência chega ao produto.
#[test]
fn nenhuma_pessoa_da_fixture_no_produto() {
    const FIXTURE: [&str; 14] = [
        "Ana Sebastião",
        "Beatriz Almeida",
        "Carlos Neto",
        "Daniel Costa",
        "Helena Vaz",
        "Joana Bento",
        "Joaquim Ramos",
        "Jorge Mendes",
        "Marta Quintas",
        "Paulo Lemos",
        "Rui Tavares",
        "Sofia Lopes",
        "Tomás Faria",
        "@inst.ao",
    ];
    let mut files = Vec::new();
    rust_sources(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("src"),
        &mut files,
    );
    for f in files {
        let src = production(&std::fs::read_to_string(&f).expect("ler"));
        for n in FIXTURE {
            assert!(!src.contains(n), "{}: {n}", f.display());
        }
    }
    for f in ["static/oc-apps.js", "static/oc-apps.css"] {
        let src = read(f);
        for n in FIXTURE {
            assert!(!src.contains(n), "{f}: {n}");
        }
    }
}
