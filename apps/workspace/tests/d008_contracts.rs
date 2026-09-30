//! Guardas estáticas da D008: o que o Terminal e o Browser nunca podem vir a
//! fazer, lido do código que os implementa. Não precisam de base de dados.

use std::path::Path;

fn read(p: &str) -> String {
    std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join(p))
        .unwrap_or_else(|e| panic!("{p}: {e}"))
}

/// O código JavaScript sem as linhas de comentário.
fn code(js: &str) -> String {
    js.lines()
        .filter(|l| {
            let t = l.trim_start();
            !(t.starts_with("//") || t.starts_with("/*") || t.starts_with('*'))
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Os clientes do Terminal e do Browser desenham texto com nós de texto e não
/// interpretam nada: sem `eval`, sem HTML construído, sem armazenamento.
#[test]
fn os_clientes_nao_interpretam_nada() {
    for f in ["static/oc-terminal.js", "static/oc-browser.js"] {
        let js = code(&read(f));
        assert!(!js.trim().is_empty(), "{f} vazio");
        for proibido in [
            "eval(",
            "new Function",
            "innerHTML",
            "outerHTML",
            "insertAdjacentHTML",
            "document.write",
            "localStorage",
            "sessionStorage",
            "indexedDB",
            "postMessage",
        ] {
            assert!(!js.contains(proibido), "{f} usa {proibido}");
        }
    }
    // O histórico do Terminal guarda o eco do servidor (redigido).
    assert!(read("static/oc-terminal.js").contains("res.echo"));
}

/// Cada cliente só na sua rota: o documento do Design não os carrega em todas
/// as páginas (ADR-0623 §5).
#[test]
fn nenhum_cliente_e_global() {
    let doc = read("src/ui/document.rs");
    assert!(doc.contains("/static/oc-apps.js"), "controlo positivo");
    for f in ["oc-terminal.js", "oc-browser.js"] {
        assert!(!doc.contains(f), "{f} em todas as páginas");
    }
}

/// A moldura da Web: nunca a origem do Ocinye, nunca a navegação de topo.
#[test]
fn a_moldura_web_e_a_do_design() {
    use ocinye_workspace::ui::apps::browser::WEB_SANDBOX;
    assert_eq!(
        WEB_SANDBOX,
        "allow-scripts allow-forms allow-popups allow-popups-to-escape-sandbox"
    );
    assert!(!WEB_SANDBOX.contains("same-origin") && !WEB_SANDBOX.contains("top-navigation"));
}

/// ADR-0312 emendada: nenhuma palavra escrita, e nada do catálogo fala de
/// separadores, painéis, elevação ou sessão de administração.
#[test]
fn sem_palavra_escrita_nem_semantica_obsoleta() {
    let catalogo = read("src/i18n/catalog.rs");
    let sistema = read("src/i18n/ui_sys.rs");
    for f in [&catalogo, &sistema] {
        for obsoleto in [
            "\"terminal.tab.",
            "\"terminal.split.",
            "\"terminal.elev.",
            "\"terminal.inspector",
            "\"terminal.prefs.",
            "\"terminal.mode.type_word\"",
            "\"ocsh.confirm.word.",
            "\"ocsh.elev.",
            "REVOGAR",
            "RÉVOQUER",
            "Fecha este separador",
            "Closes this tab",
        ] {
            assert!(!f.contains(obsoleto), "{obsoleto} voltou ao catálogo");
        }
    }
    // Nenhuma chave do Terminal fala de separadores.
    for linha in catalogo.lines().filter(|l| {
        let t = l.trim_start();
        t.starts_with("\"ocsh.") || t.starts_with("\"terminal.")
    }) {
        assert!(
            !linha.contains("separador") && !linha.contains("onglet"),
            "{linha}"
        );
    }
    // A explicação POSIX não aponta para uma família que não existe.
    let posix = ocinye_workspace::i18n::t_in(ocinye_contracts::Locale::Pt, "ocsh.err.posix");
    assert!(!posix.contains("files"), "{posix}");
}

/// O Browser entrou no registo exactamente como a D008 o propôs.
#[test]
fn o_browser_no_registo() {
    use ocinye_contracts::application::{
        ApplicationCategory, LaunchPolicy, NetworkUse, StorageUse,
    };
    use ocinye_contracts::ApplicationId;
    assert_eq!(ApplicationId::ALL.len(), 28);
    let m = ApplicationId::Browser.manifest();
    assert_eq!(m.id, ApplicationId::Browser);
    assert_eq!(m.route, "/browser");
    assert_eq!(m.category, ApplicationCategory::System);
    assert_eq!(m.launch, LaunchPolicy::SingleInstance);
    assert_eq!(m.network, NetworkUse::ClientWeb);
    assert_eq!(m.storage, StorageUse::None);
    assert!(m.api_prefixes.is_empty(), "o Browser não tem API própria");
    assert!(m.can_pin && !m.default_pin);
    for fora in ["tasks", "history", "teams"] {
        assert!(fora.parse::<ApplicationId>().is_err(), "{fora}");
    }
}
