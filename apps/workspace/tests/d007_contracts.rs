//! Contratos estáticos da D007 (conclusão das aplicações).
//!
//! O que o código de produção **não** pode conter, e que uma viagem não vê:
//! nenhum segredo nem endereço de fornecedor num ViewModel da IA; nenhuma
//! execução (comando, shell, Docker, SSH) na Computação nem nos Agentes;
//! nenhum anexo nas Mensagens; nenhuma mutação nem exportação na Auditoria;
//! nenhuma chave genérica nas Definições; e a Ajuda não lê ficheiros.

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

/// O bloco de uma definição (`pub struct X {` até à chaveta que a fecha).
fn block<'a>(src: &'a str, head: &str) -> &'a str {
    let i = src.find(head).unwrap_or_else(|| panic!("{head}"));
    let j = src[i..].find("\n}").map_or(src.len(), |j| i + j);
    &src[i..j]
}

/// Os ViewModels da IA não têm onde pôr um segredo, e o adaptador só lê a
/// referência do segredo para dizer se existe.
#[test]
fn nenhum_segredo_de_fornecedor_chega_a_um_view_model() {
    let vm = read("src/ui/view_models.rs");
    for head in [
        "pub struct AiProviderVm {",
        "pub struct AiModelVm {",
        "pub struct AiVm {",
    ] {
        let b = block(&vm, head).to_lowercase();
        for proibido in ["secret", "endpoint", "token", "api_key", "password", "url"] {
            assert!(!b.contains(proibido), "{head} tem `{proibido}`");
        }
    }
    let ctl = production(&read("src/controllers/ops.rs"));
    assert!(!ctl.contains("endpoint_url"), "o adaptador lê o endereço");
    for l in ctl.lines().filter(|l| l.contains("secret_id")) {
        assert!(
            l.contains("is_some_and(|s| !s.is_null())"),
            "a referência do segredo usa-se para mais do que saber se existe: {l}"
        );
    }
    let routes = production(&read("src/routes/ops.rs"));
    assert!(!routes.contains("endpoint_url") && !routes.contains("secret_id"));
}

/// Nem a Computação nem os Agentes executam: não há campo de comando, nem
/// shell, Docker, SSH, SQL em bruto ou HTTP arbitrário no que a D007 trouxe.
#[test]
fn nada_da_d007_executa_comandos() {
    for f in [
        "src/ui/apps/fabric.rs",
        "src/controllers/ops.rs",
        "src/routes/ops.rs",
    ] {
        let src = production(&read(f)).to_lowercase();
        for proibido in [
            "name=\"command\"",
            "shell.exec",
            "process.exec",
            "raw_sql",
            "arbitrary_http",
            "docker",
            "ssh",
            "/terminal/exec",
            "std::process",
        ] {
            assert!(!src.contains(proibido), "{f}: `{proibido}`");
        }
    }
    // E o Core não ganhou capacidades genéricas.
    let core = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../crates/ocinye-core/src");
    let mut files = Vec::new();
    walk(&core, &mut files);
    for f in files {
        let src = std::fs::read_to_string(&f).expect("ler");
        for proibido in [
            "\"shell.exec\"",
            "\"process.exec\"",
            "\"raw_sql\"",
            "\"arbitrary_http\"",
        ] {
            assert!(!src.contains(proibido), "{}: {proibido}", f.display());
        }
    }
}

fn walk(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
    for e in std::fs::read_dir(dir).expect("dir").flatten() {
        let p = e.path();
        if p.is_dir() {
            walk(&p, out);
        } else if p.extension().is_some_and(|x| x == "rs") {
            out.push(p);
        }
    }
}

/// As rotas da D007 que mudam alguma coisa são estas, e só estas. A
/// Computação, a Actividade, a Auditoria, Meus Recursos e a Ajuda não têm
/// nenhuma; as Definições não têm uma chave genérica.
#[test]
fn as_rotas_da_d007_so_mudam_o_que_foi_desenhado() {
    let routes = read("src/routes.rs");
    let inv_start = routes.find("pub const ROUTES").expect("ROUTES");
    let inv = &routes[inv_start..inv_start + routes[inv_start..].find("];").expect("fim")];
    for prefixo in ["/compute", "/activity", "/audit", "/resources", "/help"] {
        let rotas: Vec<&str> = inv
            .lines()
            .map(|l| l.trim().trim_matches(|c| c == '"' || c == ','))
            .filter(|l| l.starts_with(prefixo))
            .collect();
        for r in &rotas {
            assert!(
                matches!(
                    *r,
                    "/compute"
                        | "/compute/nodes/{node_id}"
                        | "/activity"
                        | "/audit"
                        | "/resources"
                        | "/help"
                ),
                "rota nova em {prefixo}: {r}"
            );
        }
        assert!(
            !routes.contains(&format!("\"{prefixo}\", post(")),
            "{prefixo} aceita POST"
        );
    }
    assert!(
        !inv.contains("/messages/people"),
        "a procura no directório voltou"
    );
    assert!(
        !inv.lines().any(|l| l.contains("/settings/{")),
        "uma chave genérica nas Definições"
    );
    assert!(!inv.contains("/audit/export") && !inv.contains("/audit/{"));
}

/// As Mensagens não têm anexos: o domínio não os tem.
#[test]
fn as_mensagens_nao_tem_anexos() {
    let src = production(&read("src/ui/apps/messages.rs")).to_lowercase();
    for proibido in ["type=\"file\"", "attach", "enctype", "upload"] {
        assert!(!src.contains(proibido), "{proibido}");
    }
}

/// A Auditoria é só leitura: o ecrã não tem formulário que mude nem exporte.
#[test]
fn a_auditoria_nao_tem_mutacao_nem_exportacao() {
    let src = production(&read("src/ui/apps/ledger.rs"));
    let audit = &src[src.find("fn audit_row(").expect("secção")..];
    assert!(!audit.contains("method=\"post\""));
    assert!(!audit.to_lowercase().contains("export"));
}

/// A lista branca da metadata de auditoria é positiva e não contém nenhuma
/// chave que diga *quem* ou *o quê* nem nada que se pareça com um segredo.
#[test]
fn a_lista_branca_da_auditoria_nao_deixa_passar_o_sensivel() {
    let src = production(&read("src/controllers/ops.rs"));
    let lista = block(&src, "pub const AUDIT_ALLOWED_KEYS");
    for proibido in [
        "password",
        "token",
        "secret",
        "credential\"",
        "email",
        "full_name",
        "reason",
        "session_id",
        "checksum",
        "person_id",
        "owner_id",
        "subject_id",
        "\"name\"",
        "\"label\"",
        "\"code\"",
        "api_key",
        "hint",
        "ciphertext",
        "nonce",
        "body",
        "content\"",
        "prompt",
        "payload",
    ] {
        assert!(
            !lista.contains(proibido),
            "a lista branca deixa passar {proibido}"
        );
    }
    // É uma lista de permitidos, não de proibidos.
    assert!(src.contains("AUDIT_ALLOWED_KEYS.contains("));
    assert!(!src.contains("DENIED_KEYS") && !src.contains("BLOCKED_KEYS"));
}

/// A Ajuda é conteúdo de primeira parte: nenhum ficheiro, caminho ou rede.
#[test]
fn a_ajuda_nao_le_ficheiros_nem_a_rede() {
    let routes = production(&read("src/routes/ops.rs"));
    let help = &routes[routes
        .find("pub(super) async fn help_page")
        .expect("help_page")..];
    for proibido in [
        "std::fs",
        "read_to_string",
        "Path(",
        "api::get",
        "caller(",
        "http",
    ] {
        assert!(!help.contains(proibido), "a Ajuda usa {proibido}");
    }
}

/// O registo não mudou: as nove continuam a ter uma janela só, e o Terminal
/// continua sem ecrã.
#[test]
fn as_nove_sao_de_uma_janela_e_o_terminal_espera() {
    use ocinye_contracts::{application::LaunchPolicy, ApplicationId};
    for id in [
        ApplicationId::Messages,
        ApplicationId::Ai,
        ApplicationId::Agents,
        ApplicationId::Compute,
        ApplicationId::Resources,
        ApplicationId::Activity,
        ApplicationId::Audit,
        ApplicationId::Settings,
        ApplicationId::Help,
    ] {
        assert_eq!(id.manifest().launch, LaunchPolicy::SingleInstance, "{id:?}");
        assert!(
            ocinye_workspace::controllers::windows::has_screen(id),
            "{id:?}"
        );
    }
    assert!(!ocinye_workspace::controllers::windows::has_screen(
        ApplicationId::Terminal
    ));
}

/// D007.1 · Nenhum caminho do Workspace elimina definitivamente sem uma
/// confirmação governada: os três que o faziam, sem ecrã que os usasse, saíram.
#[test]
fn nao_ha_eliminacao_definitiva_sem_confirmacao() {
    let routes = read("src/routes.rs");
    for rota in [
        "/notes/{note_id}/eliminar",
        "/me/files/purge",
        "/files/trash/empty",
    ] {
        assert!(!routes.contains(&format!("\"{rota}\"")), "{rota} voltou");
    }
    let src = production(&routes);
    for chamada in [
        "/api/v1/me/files/purge",
        "/api/v1/me/notes/{note_id}/purge",
        "/purge\"",
    ] {
        assert!(!src.contains(chamada), "o Workspace chama {chamada}");
    }
}

/// D007.1 · O registo tem 27 aplicações; o Browser é da D008, e Tarefas e
/// Histórico não são aplicações.
#[test]
fn o_registo_e_o_da_d007_1() {
    use ocinye_contracts::ApplicationId;
    assert_eq!(ApplicationId::ALL.len(), 27);
    for fora in ["browser", "tasks", "history"] {
        assert!(
            fora.parse::<ApplicationId>().is_err(),
            "{fora} está registado"
        );
    }
    for id in [
        ApplicationId::Monitor,
        ApplicationId::Results,
        ApplicationId::Trash,
    ] {
        assert!(
            ocinye_workspace::controllers::windows::has_screen(id),
            "{id:?}"
        );
        assert_eq!(
            id.manifest().launch,
            ocinye_contracts::application::LaunchPolicy::SingleInstance
        );
    }
}
