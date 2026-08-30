//! A interface do Ocinye Workspace.
//!
//! Implementa o dossier de design em [`design/`](../../../../design/README.md).
//! Os valores visuais vivem em `static/ocinye.css` como custom properties: nenhum
//! componente aqui define cor, tamanho ou raio soltos.
//!
//! # O que está aqui
//!
//! Componentes Leptos renderizados no servidor. Nenhuma decisão de autorização,
//! nenhuma regra institucional: uma vista decide o que *mostrar*, nunca o que é
//! *permitido* (ADR-0602).

pub mod components;
pub mod icon;
pub mod screens;
pub mod shell;
pub mod tempo;

use leptos::prelude::*;

/// Renderiza uma árvore de componentes num documento HTML completo.
///
/// `lang="pt"` porque a interface é em português europeu e os leitores de ecrã
/// precisam de o saber para a pronunciarem correctamente.
pub fn document(title: &str, body: impl IntoView + 'static) -> String {
    document_com_cabeca(title, body, None)
}

/// O mesmo documento, com marcação adicional na cabeça.
///
/// Existe para o arranque: quando o Core deixa seguir, a página traz uma
/// actualização de meta para o destino. Vai na cabeça porque é onde os browsers
/// a lêem, e sem JavaScript — o arranque é o momento em que menos se pode
/// assumir que há scripts a correr.
#[must_use]
pub fn document_com_cabeca(
    title: &str,
    body: impl IntoView + 'static,
    cabeca: Option<String>,
) -> String {
    let rendered = body.to_html();

    let mut out = String::with_capacity(rendered.len() + 1024);
    out.push_str(
        "<!doctype html>\n\
         <html lang=\"pt\">\n\
         <head>\n\
         <meta charset=\"utf-8\">\n\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n\
         <meta name=\"robots\" content=\"noindex, nofollow\">\n\
         <link rel=\"icon\" href=\"/static/ocinye_logo.png\">\n\
         <link rel=\"preconnect\" href=\"https://fonts.googleapis.com\">\n\
         <link rel=\"preconnect\" href=\"https://fonts.gstatic.com\" crossorigin>\n\
         <link rel=\"stylesheet\" href=\"https://fonts.googleapis.com/css2?\
         family=IBM+Plex+Mono:wght@400;500;600&\
         family=IBM+Plex+Sans:wght@400;500;600;700&display=swap\">\n\
         <link rel=\"stylesheet\" href=\"/static/ocinye.css\">\n\
         <title>",
    );
    out.push_str(&escape(title));
    out.push_str(" · Ocinye Workspace</title>\n");
    if let Some(extra) = cabeca {
        out.push_str(&extra);
        out.push('\n');
    }
    out.push_str(
        "</head>\n\
         <body>\n",
    );
    out.push_str(&rendered);
    out.push_str("\n<script src=\"/static/app.js\" defer></script>\n</body>\n</html>");
    out
}

/// Escapa texto para inclusão segura em HTML.
///
/// Usado nos poucos sítios onde texto é interpolado fora da árvore de
/// componentes; dentro dela, o Leptos escapa.
#[must_use]
pub fn escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

/// Iniciais para um avatar, a partir de um nome.
///
/// Duas letras no máximo: o design reserva 24–52px e três iniciais deixam de
/// caber legivelmente.
#[must_use]
pub fn initials(name: &str) -> String {
    let parts: Vec<&str> = name.split_whitespace().filter(|p| !p.is_empty()).collect();

    match parts.as_slice() {
        [] => "?".to_owned(),
        [only] => only.chars().take(2).collect::<String>().to_uppercase(),
        [first, .., last] => {
            let mut out = String::new();
            out.extend(first.chars().take(1));
            out.extend(last.chars().take(1));
            out.to_uppercase()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn texto_interpolado_nao_injecta_markup() {
        let out = escape("<script>alert('x')</script>");
        assert!(!out.contains('<'));
        assert!(out.contains("&lt;script&gt;"));
    }

    #[test]
    fn iniciais_usam_primeiro_e_ultimo_nome() {
        assert_eq!(initials("João Manuel"), "JM");
        assert_eq!(initials("Maria da Silva Santos"), "MS");
        assert_eq!(initials("Ana"), "AN");
        assert_eq!(initials(""), "?");
    }

    #[test]
    fn o_documento_carrega_o_stylesheet_e_o_script() {
        let html = document("Teste", leptos::prelude::view! { <p>"olá"</p> });
        assert!(html.contains("/static/ocinye.css"));
        assert!(html.contains("/static/app.js"));
        assert!(html.contains("lang=\"pt\""));
        assert!(html.contains("IBM+Plex+Sans"));
    }
}

#[cfg(test)]
mod render_tests {
    use super::*;
    use crate::ui::shell::{Screen, Viewer};
    use serde_json::json;

    /// Um membro que pode tudo, para os testes que verificam a estrutura da
    /// shell em vez da filtragem por permissão.
    fn viewer() -> Viewer {
        Viewer {
            zona: "UTC".to_owned().try_into().expect("fuso conhecido"),
            avatar: ocinye_contracts::AvatarChoice::Initials,
            email: Some("jmanuel@ocinye.com".to_owned()),
            session_expires_in: Some(std::time::Duration::from_secs(8 * 3600)),
            name: "João Manuel".to_owned(),
            organisation: "Ocinye".to_owned(),
            core_status: crate::ui::shell::CoreStatus::Ok,
            temporal: Vec::new(),
            temporal_failure: None,
            unread: 0,
            capabilities: ocinye_contracts::Permission::all()
                .into_iter()
                .map(|p| p.as_str().to_owned())
                .collect(),
            // Todos os módulos relevantes: estas pré-visualizações existem para
            // ver os ecrãs, e um catálogo que escondesse metade deles não
            // mostraria o que há para rever.
            modules: [
                "units",
                "ideas",
                "projects",
                "knowledge",
                "files",
                "bibliography",
                "datasets",
            ]
            .into_iter()
            .map(ToOwned::to_owned)
            .collect(),
        }
    }

    /// Despeja ecrãs reais como HTML, para verificação visual num browser.
    ///
    /// Não afirma nada: existe para que uma mudança visual possa ser **vista**
    /// antes de ser dada como feita. Os ficheiros são servidos com a mesma
    /// Content-Security-Policy que o servidor envia — abrir por `file://` não
    /// tem CSP nenhuma, e teria pintado as tabelas correctamente mesmo quando
    /// estavam partidas.
    ///
    ///     cargo test -p ocinye-workspace despejar_ecras -- --ignored --nocapture
    #[test]
    #[ignore = "arnês de verificação visual; corre-se de propósito"]
    fn despejar_ecras_para_verificacao_visual() {
        use crate::ui::screens;

        let destino = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/verify");
        std::fs::create_dir_all(&destino).expect("criar destino");

        let v = viewer();
        let vazio = json!({"items": [], "total": 0});
        let membros = json!({
            "items": [{
                "full_name": "Fidel Monteiro",
                "email": "fidel@ocinye.com",
                "unit_name": "Investigação",
                "role": "founder",
                "created_at": "2026-08-23",
                "status": "active",
                "last_seen_at": "2026-08-23",
            }],
            "total": 1
        });

        let ecras: Vec<(&str, String)> = vec![
            (
                "login",
                document("Entrar", screens::login::login(true, None)),
            ),
            (
                "membros",
                document(
                    "Membros",
                    shell::shell(
                        &v,
                        Screen::Admin,
                        Vec::new(),
                        Screen::Admin.label(),
                        screens::lists::members(&v, &membros),
                    ),
                ),
            ),
            (
                "agentes",
                document(
                    "Agentes",
                    shell::shell(
                        &v,
                        Screen::Agents,
                        Vec::new(),
                        Screen::Agents.label(),
                        screens::lists::agents(&v, &vazio),
                    ),
                ),
            ),
            (
                "correio",
                document(
                    "Correio",
                    shell::shell(
                        &v,
                        Screen::Mail,
                        Vec::new(),
                        Screen::Mail.label(),
                        screens::mail::mail(
                            &v,
                            &screens::mail::MailView {
                                status: json!({
                                    "can_read": false,
                                    "can_send": false,
                                    "detail": "O correio institucional ainda não foi \
                                               configurado nesta instalação do Ocinye OS.",
                                }),
                                sync_notice: None,
                                mailboxes: json!({"items": []}),
                                active_mailbox: None,
                                folder: "inbox".to_owned(),
                                query: String::new(),
                            },
                            &json!({"items": []}),
                            None,
                            None,
                        ),
                    ),
                ),
            ),
            (
                "unidades",
                document(
                    "Unidades",
                    shell::shell(
                        &v,
                        Screen::Units,
                        Vec::new(),
                        Screen::Units.label(),
                        screens::lists::units(&v, &vazio),
                    ),
                ),
            ),
        ];

        for (nome, html) in &ecras {
            let caminho = destino.join(format!("{nome}.html"));
            std::fs::write(&caminho, html).expect("escrever");
            println!("escrito: {}", caminho.display());
        }
    }

    /// Cada ecrã renderiza as secções que o dossier lhe atribui.
    ///
    /// Faltavam dois painéis no «O Meu Trabalho» — `Documentos recentes` e
    /// `Unidades seguidas` — e a ausência passou despercebida porque nada a
    /// verificava: o ecrã parecia completo por si.
    ///
    /// A verificação é sobre o **HTML renderizado**, e não sobre o
    /// código-fonte. Uma primeira versão deste teste lia os ficheiros `.rs` e
    /// dava-se por satisfeita ao encontrar a frase dentro de um comentário —
    /// passava com o painel apagado.
    ///
    /// E é pela **secção**, não pelo texto do protótipo: o protótipo traz dados
    /// de demonstração — nomes de pessoas, códigos de ideias — que não
    /// pertencem ao código e cuja ausência não é lacuna nenhuma.
    #[test]
    fn cada_ecra_renderiza_as_seccoes_que_o_dossier_lhe_atribui() {
        use crate::ui::screens;

        let vazio = json!({"items": [], "total": 0});

        let ecras: Vec<(&str, String, Vec<&str>)> = vec![
            (
                "O Meu Trabalho",
                screens::my_work::my_work(&vazio, &vazio, &vazio).to_html(),
                vec![
                    "Tarefas atribuídas",
                    "Documentos recentes",
                    "Unidades seguidas",
                ],
            ),
            (
                "Home",
                screens::home::home(screens::home::Dashboard {
                    greeting: "Bom dia".to_owned(),
                    name: "Teste".to_owned(),
                    kpis: Vec::new(),
                    workspaces: vazio.clone(),
                    tasks: vazio.clone(),
                    activity: json!([]),
                    intelligence: json!({"configured": false}),
                    can_create_idea: true,
                })
                .to_html(),
                vec![
                    "Continuar trabalho",
                    "RESEARCH WORKSPACES",
                    "Actividade recente",
                    "Acesso rápido",
                ],
            ),
            (
                "Criar Agente IA",
                screens::ai::new_agent(&json!({}), None).to_html(),
                vec!["IDENTIDADE", "ÂMBITO DE ACESSO", "CONHECIMENTO"],
            ),
        ];

        let mut faltam = Vec::new();
        for (ecra, html, seccoes) in &ecras {
            for seccao in seccoes {
                if !html.contains(seccao) {
                    faltam.push(format!("{ecra}: «{seccao}»"));
                }
            }
        }

        assert!(
            faltam.is_empty(),
            "secções do dossier que o ecrã não renderiza:\n  {}",
            faltam.join("\n  ")
        );
    }

    /// A shell é a mesma em todos os ecrãs autenticados; se um destes
    /// elementos desaparecer, o design deixou de ser cumprido.
    #[test]
    fn a_shell_traz_sempre_navegacao_pesquisa_criar_e_palette() {
        let html = document(
            "Teste",
            shell::shell(
                &viewer(),
                Screen::Home,
                Vec::new(),
                Screen::Home.label(),
                leptos::prelude::view! { <p></p> },
            ),
        );

        for expected in [
            "oc-shell",
            "OCINYE OS",
            "PESSOAL",
            "INVESTIGAÇÃO",
            "CONHECIMENTO",
            "INTELIGÊNCIA",
            "INSTITUCIONAL",
            // A Universal Command Surface substituiu a barra de pesquisa: uma
            // barra, três intenções (briefing §29).
            "Pesquisar, perguntar ou executar no Ocinye…",
            "⌘K",
            "data-oc=\"create\"",
            "data-oc=\"palette\"",
            "data-oc=\"collapse\"",
            "CORE OK",
        ] {
            assert!(html.contains(expected), "falta na shell: {expected}");
        }
    }

    /// As sete acções do menu `+ Criar`, com os atalhos do design.
    #[test]
    fn o_menu_criar_tem_as_sete_accoes_do_design() {
        let html = document(
            "Teste",
            shell::shell(
                &viewer(),
                Screen::Home,
                Vec::new(),
                Screen::Home.label(),
                leptos::prelude::view! { <p></p> },
            ),
        );

        for action in [
            "Nova Ideia",
            "Novo Projecto",
            "Nova Nota",
            "Nova Referência",
            "Novo Dataset",
            "Nova Tarefa",
            "Novo Agente IA",
        ] {
            assert!(html.contains(action), "falta no menu Criar: {action}");
        }
    }

    /// O ecrã activo é marcado, e apenas um.
    #[test]
    fn apenas_um_item_de_navegacao_esta_activo() {
        let html = document(
            "Teste",
            shell::shell(
                &viewer(),
                Screen::Ideas,
                Vec::new(),
                Screen::Ideas.label(),
                leptos::prelude::view! { <p></p> },
            ),
        );
        assert_eq!(html.matches("aria-current=\"page\"").count(), 1);
    }

    /// Um ecrã de detalhe mostra o trilho até ele, e termina em si próprio.
    #[test]
    fn o_trilho_mostra_o_caminho_ate_ao_ecra() {
        let html = document(
            "Teste",
            shell::shell(
                &viewer(),
                Screen::Ideas,
                vec![shell::Crumb::to(Screen::Ideas)],
                "Ideia AI-IDEA-001",
                leptos::prelude::view! { <p></p> },
            ),
        );
        assert!(html.contains("OCINYE"));
        assert!(html.contains(">Ideias</a>"));
        // A página fecha o trilho com o seu próprio nome, e não com o do ecrã:
        // «Ideias / Ideias» não diz onde se está.
        assert!(
            html.contains("<b>Ideia AI-IDEA-001</b>"),
            "o trilho não termina no nome da página"
        );
    }

    /// Todos os ecrãs de lista partilham o mesmo componente de tabela.
    #[test]
    fn os_ecras_de_lista_usam_a_mesma_tabela() {
        let payload = json!({"items": [], "total": 0});

        let screens = [
            screens::lists::units(&viewer(), &payload).to_html(),
            screens::lists::ideas(&viewer(), &payload, screens::lists::Slice::default()).to_html(),
            screens::lists::projects(&viewer(), &payload, screens::lists::Slice::default())
                .to_html(),
            screens::lists::bibliography(&viewer(), &payload).to_html(),
            screens::lists::datasets(&viewer(), &payload).to_html(),
            screens::lists::agents(&viewer(), &payload).to_html(),
            screens::lists::members(&viewer(), &payload).to_html(),
            screens::lists::audit(&viewer(), &payload).to_html(),
        ];

        for html in &screens {
            assert!(
                html.contains("oc-table"),
                "um ecrã de lista não usa a tabela partilhada"
            );
            assert!(
                html.contains("oc-table__foot"),
                "falta o rodapé de paginação"
            );
            assert!(html.contains("Filtrar"), "falta o botão Filtrar");
        }
    }

    /// A classificação aparece onde o design a exige.
    #[test]
    fn a_classificacao_e_visivel_em_ideias_e_datasets() {
        let ideas = screens::lists::ideas(&viewer(), &json!({
            "items": [{"id": "1", "title": "X", "state": "concept", "classification": "RESTRICTED"}],
            "total": 1
        }), screens::lists::Slice::default())
        .to_html();
        assert!(ideas.contains("RESTRICTED"));

        let datasets = screens::lists::datasets(
            &viewer(),
            &json!({
                "items": [{"title": "D", "classification": "CONFIDENTIAL", "state": "active"}],
                "total": 1
            }),
        )
        .to_html();
        assert!(datasets.contains("CONFIDENTIAL"));
    }

    /// Sem infraestrutura, os ecrãs de IA e computação dizem-no.
    #[test]
    fn a_ausencia_de_infraestrutura_e_declarada_e_nao_simulada() {
        let ai = screens::ai::hub(
            &json!({"available": false, "providers": 0, "message": "Nenhum nó de IA Ocinye está actualmente disponível."}),
            &json!({"items": []}),
        )
        .to_html();
        assert!(ai.contains("Nenhum nó de IA Ocinye está actualmente disponível"));

        let compute = screens::compute::compute(
            &json!({"registered_nodes": 0, "online_nodes": 0}),
            &json!({"items": []}),
        )
        .to_html();
        assert!(compute.contains("0 nós registados"));
        assert!(!compute.contains("CAM-01"));
    }
}

#[cfg(test)]
pub(crate) mod link_tests {
    use super::*;
    use crate::routes::ROUTES;
    use crate::ui::shell::{Screen, Viewer};
    use serde_json::json;

    /// Um membro que pode tudo, para os testes que verificam a estrutura da
    /// shell em vez da filtragem por permissão.
    fn viewer() -> Viewer {
        Viewer {
            zona: "UTC".to_owned().try_into().expect("fuso conhecido"),
            avatar: ocinye_contracts::AvatarChoice::Initials,
            email: Some("jmanuel@ocinye.com".to_owned()),
            session_expires_in: Some(std::time::Duration::from_secs(8 * 3600)),
            name: "João Manuel".to_owned(),
            organisation: "Ocinye".to_owned(),
            core_status: crate::ui::shell::CoreStatus::Ok,
            temporal: Vec::new(),
            temporal_failure: None,
            unread: 0,
            capabilities: ocinye_contracts::Permission::all()
                .into_iter()
                .map(|p| p.as_str().to_owned())
                .collect(),
            // Todos os módulos relevantes: estas pré-visualizações existem para
            // ver os ecrãs, e um catálogo que escondesse metade deles não
            // mostraria o que há para rever.
            modules: [
                "units",
                "ideas",
                "projects",
                "knowledge",
                "files",
                "bibliography",
                "datasets",
            ]
            .into_iter()
            .map(ToOwned::to_owned)
            .collect(),
        }
    }

    /// Um caminho corresponde a uma rota registada, incluindo parâmetros.
    fn matches_route(path: &str) -> bool {
        ROUTES.iter().any(|route| {
            let route_parts: Vec<&str> = route.split('/').collect();
            let path_parts: Vec<&str> = path.split('/').collect();
            route_parts.len() == path_parts.len()
                && route_parts
                    .iter()
                    .zip(&path_parts)
                    .all(|(r, p)| r.starts_with('{') || r == p)
        })
    }

    /// Extrai os caminhos internos de um HTML.
    fn hrefs(html: &str) -> Vec<String> {
        html.split("href=\"")
            .skip(1)
            .filter_map(|part| part.split('"').next())
            .filter(|href| {
                href.starts_with('/') && !href.starts_with("//") && !href.starts_with("/static/")
            })
            .map(|href| href.split('?').next().unwrap_or(href).to_owned())
            .collect()
    }

    /// Um contexto de correio para as pré-visualizações.
    ///
    /// `service` liga ou desliga o serviço; `ai` a assistência de escrita. Os
    /// dois são independentes de propósito — é exactamente a distinção que a
    /// interface tem de saber mostrar.
    fn mail_view(service: bool, ai: bool) -> screens::mail::MailView {
        screens::mail::MailView {
            status: json!({
                "can_read": service,
                "can_send": service,
                "ai_assist_available": ai,
                "may_use_ai": true,
                "adapter": if service { "imap+smtp" } else { "unconfigured" },
                "endpoints": if service {
                    json!(["imap.exemplo.org:993", "smtp.exemplo.org:587"])
                } else {
                    json!([])
                },
                "detail": if service {
                    "Correio institucional configurado."
                } else {
                    "O correio institucional ainda não foi configurado nesta instalação do Ocinye OS."
                },
            }),
            mailboxes: if service {
                json!([{
                    "id": "33333333-3333-3333-3333-333333333333",
                    "address": "ana@ocinye.com",
                    "display_name": "Ana Fernandes",
                    "kind": "personal",
                    "may_send": true,
                    "may_reply": true,
                    "last_synced_at": "2026-08-22T09:20:00Z",
                    "unread": [
                        {"folder": "inbox", "label": "Caixa de entrada", "unread": 3},
                        {"folder": "sent", "label": "Enviados", "unread": 0}
                    ]
                }])
            } else {
                json!([])
            },
            sync_notice: None,
            active_mailbox: None,
            folder: "inbox".to_owned(),
            query: String::new(),
        }
    }

    /// Cada ecrã da interface, já dentro da shell e pronto a ver.
    ///
    /// Uma só lista serve dois fins: o varrimento de ligações mortas e o
    /// despejo para inspecção visual. Se um ecrã novo não for acrescentado
    /// aqui, deixa de ser coberto por ambos.
    /// O ambiente onde a cadeia científica acontece.
    fn ambiente_cientifico() -> serde_json::Value {
        json!({
            "id": "55555555-5555-5555-5555-555555555555",
            "code": "AI-PROJ-001",
            "unit_code": "AI"
        })
    }

    /// Um estudo com tudo o que os ecrãs lêem dele.
    fn estudo_de_referencia() -> serde_json::Value {
        json!({
            "id": "77777777-7777-7777-7777-777777777773",
            "workspace_id": "55555555-5555-5555-5555-555555555555",
            "title": "Ensaio de carga",
            "kind": "physical_experiment",
            "kind_label": "Experimento",
            "status_label": "Concluído",
            "objective": "Medir a queda de resistência sob carga.",
            "status": "completed",
            "classification": "INTERNAL"
        })
    }

    /// Uma corrida concreta desse estudo.
    fn execucao_de_referencia() -> serde_json::Value {
        json!({
            "id": "77777777-7777-7777-7777-777777777776",
            "study_id": "77777777-7777-7777-7777-777777777773",
            "sequence": 3,
            "status": "succeeded",
            "environment": "Bancada 2",
            "software_name": "OpenFOAM",
            "software_version": "11"
        })
    }

    pub(crate) fn catalogue() -> Vec<(&'static str, String)> {
        let empty = json!({"items": [], "total": 0});

        // Um workspace onde a política actual permite criar.
        let destinos = json!({
            "items": [{
                "id": "55555555-5555-5555-5555-555555555555",
                "code": "AI-PROJ-001",
                "title": "Projecto de referência",
                "kind": "project",
                "classification": "INTERNAL",
                "may_create": true
            }],
            "total": 1
        });

        // Uma ideia já marcada como candidata a projecto.
        let candidatas = json!({
            "items": [{
                "id": "66666666-6666-6666-6666-666666666666",
                "code": "AI-IDEA-002",
                "title": "Ideia candidata",
                "kind": "idea",
                "classification": "INTERNAL",
                "may_create": true
            }],
            "total": 1
        });

        // Uma lista que o Core truncou: três linhas de duzentas.
        //
        // É a fixture que distingue «filtrar a lista» de «filtrar a página», e
        // sem ela essa diferença nunca aparece no HTML renderizado.
        let muitas = json!({
            "items": [
                {"id": "aaaaaaaa-0000-0000-0000-000000000001", "code": "AI-IDEA-001",
                 "title": "Primeira", "state": "exploration", "classification": "INTERNAL",
                 "unit_code": "AI", "kind": "idea", "may_create": true},
                {"id": "aaaaaaaa-0000-0000-0000-000000000002", "code": "AI-IDEA-002",
                 "title": "Segunda", "state": "exploration", "classification": "INTERNAL",
                 "unit_code": "AI", "kind": "idea", "may_create": true},
                {"id": "aaaaaaaa-0000-0000-0000-000000000003", "code": "BIO-IDEA-001",
                 "title": "Terceira", "state": "project_candidate", "classification": "PUBLIC",
                 "unit_code": "BIO", "kind": "idea", "may_create": false}
            ],
            "total": 200
        });

        // Uma unidade activa, onde uma ideia pode nascer.
        let unidades = json!({
            "items": [{
                "id": "77777777-7777-7777-7777-777777777777",
                "code": "AI",
                "name": "Inteligência Artificial",
                "status": "active"
            }],
            "total": 1
        });
        let ai_status = json!({
            "available": false, "providers": 0,
            "message": "Nenhum nó de IA Ocinye está actualmente disponível.",
            "capabilities": [{"capability": "GENERAL", "available": false}]
        });

        let workspace_overview = json!({
            "workspace": {
                "id": "11111111-1111-1111-1111-111111111111",
                "code": "AI-IDEA-001",
                "classification": "INTERNAL",
                "unit_code": "AI"
            },
            "idea": {"id": "22222222-2222-2222-2222-222222222222", "title": "Ideia", "state": "exploration"},
            "project": null,
            "members": []
        });

        /// Envolve um ecrã na shell e produz o documento completo.
        macro_rules! page {
            ($name:expr, $screen:expr, $body:expr) => {
                (
                    $name,
                    document(
                        $name,
                        shell::shell(&viewer(), $screen, Vec::new(), $screen.label(), $body),
                    ),
                )
            };
        }

        let dashboard = screens::home::Dashboard {
            greeting: "Bom dia".to_owned(),
            name: "João".to_owned(),
            kpis: Vec::new(),
            workspaces: empty.clone(),
            tasks: empty.clone(),
            activity: json!([]),
            intelligence: ai_status.clone(),
            can_create_idea: true,
        };

        vec![
            (
                "not-found",
                document("not-found", screens::notice::not_found()),
            ),
            (
                "access-denied",
                document("access-denied", screens::notice::access_denied()),
            ),
            // Nem o login nem o primeiro acesso usam a shell: são os dois
            // momentos anteriores a haver Workspace.
            (
                "first-access",
                document(
                    "first-access",
                    screens::first_access::first_access("João Manuel", "joao", None),
                ),
            ),
            (
                "login",
                document("Iniciar sessão", screens::login::login(true, None)),
            ),
            page!("home", Screen::Home, screens::home::home(dashboard)),
            page!(
                "my-work",
                Screen::MyWork,
                screens::my_work::my_work(&empty, &empty, &empty)
            ),
            page!(
                "units",
                Screen::Units,
                screens::lists::units(&viewer(), &empty)
            ),
            page!(
                "ideas",
                Screen::Ideas,
                screens::lists::ideas(&viewer(), &empty, screens::lists::Slice::default())
            ),
            page!(
                "ideas-new",
                Screen::Ideas,
                screens::lists::new_idea(&empty, None)
            ),
            page!(
                "projects",
                Screen::Projects,
                screens::lists::projects(&viewer(), &empty, screens::lists::Slice::default())
            ),
            page!(
                "bibliography",
                Screen::Bibliography,
                screens::lists::bibliography(&viewer(), &empty)
            ),
            page!(
                "datasets",
                Screen::Datasets,
                screens::lists::datasets(&viewer(), &empty)
            ),
            page!(
                "agents",
                Screen::Agents,
                screens::lists::agents(&viewer(), &empty)
            ),
            page!(
                "members",
                Screen::Admin,
                screens::lists::members(&viewer(), &empty)
            ),
            page!(
                "audit",
                Screen::Audit,
                screens::lists::audit(&viewer(), &empty)
            ),
            page!(
                "activity",
                Screen::Activity,
                screens::activity::activity(&json!([]))
            ),
            page!("ai", Screen::Ai, screens::ai::hub(&ai_status, &empty)),
            page!(
                "ai-agent-new",
                Screen::Agents,
                screens::ai::new_agent(&empty, None)
            ),
            page!(
                "ai-prompt",
                Screen::Prompt,
                screens::prompt::prompt(screens::prompt::context_from(&ai_status, None), None,)
            ),
            page!(
                "compute",
                Screen::Compute,
                screens::compute::compute(&json!({"registered_nodes": 0}), &empty)
            ),
            page!(
                "knowledge",
                Screen::Knowledge,
                screens::knowledge::knowledge(screens::knowledge::KnowledgeCounts {
                    bibliography: serde_json::Value::Null,
                    documents: serde_json::Value::Null,
                    datasets: empty.clone(),
                    recent: empty.clone(),
                    // O estado desta instalação: nenhum nó de IA. A superfície
                    // de assistência aparece e declara-se indisponível.
                    inference_available: false,
                    may_use_assistance: true,
                })
            ),
            // A fixture é uma Ideia, e o router escolhe o ecrã pelo tipo do
            // workspace; a pré-visualização tem de escolher o mesmo, ou mostra
            // um trilho que o produto nunca produz.
            page!(
                "research-workspace",
                Screen::Ideas,
                screens::workspaces::research_workspace(screens::workspaces::WorkspaceView {
                    overview: workspace_overview.clone(),
                    sources: empty.clone(),
                    notes: json!([]),
                    documents: json!([]),
                    datasets: empty.clone(),
                    tasks: empty.clone(),
                    activity: json!([]),
                    inference_available: false,
                    may_use_assistance: true,
                    gestao: screens::workspaces::tests::gestao_de_prova(),
                })
            ),
            // A cadeia científica, com trabalho lá dentro. Uma fixture vazia
            // mostraria o estado vazio e nenhum dos guardas veria uma linha —
            // que é onde os links, os badges e a classificação vivem.
            page!(
                "science-chain",
                Screen::Ideas,
                screens::science::scientific_chain(screens::science::ChainView {
                    overview: workspace_overview.clone(),
                    hypotheses: json!([{
                        "id": "77777777-7777-7777-7777-777777777771",
                        "statement": "A dopagem reduz a resistência de contacto",
                        "status": "open", "status_label": "Aberta",
                        "classification": "INTERNAL"
                    }]),
                    methodologies: json!([{
                        "id": "77777777-7777-7777-7777-777777777772",
                        "title": "Medição a quatro pontas",
                        "status": "active",
                        "classification": "INTERNAL"
                    }]),
                    studies: json!([{
                        "id": "77777777-7777-7777-7777-777777777773",
                        "title": "Ensaio de carga",
                        "status": "completed", "status_label": "Concluído",
                        "classification": "INTERNAL"
                    }]),
                    results: json!([{
                        "id": "77777777-7777-7777-7777-777777777774",
                        "title": "A resistência caiu 18%",
                        "status": "draft", "status_label": "Registado",
                        "classification": "INTERNAL"
                    }]),
                    may_create: true,
                })
            ),
            // O mesmo ecrã sem nada registado: o estado vazio é uma superfície
            // própria, e tem o seu próprio texto conforme quem lê possa ou não
            // começar a cadeia.
            page!(
                "science-chain-vazia",
                Screen::Ideas,
                screens::science::scientific_chain(screens::science::ChainView {
                    overview: workspace_overview.clone(),
                    hypotheses: json!([]),
                    methodologies: json!([]),
                    studies: json!([]),
                    results: json!([]),
                    may_create: false,
                })
            ),
            // Um resultado com proveniência dos dois lados, e com as duas
            // origens: uma aresta que a operação observou e uma que alguém
            // declarou. A distinção é visível, e o guarda vê-a.
            page!(
                "result-detail",
                Screen::Ideas,
                screens::science::result_detail(screens::science::ResultView {
                    result: json!({
                        "id": "77777777-7777-7777-7777-777777777774",
                        "workspace_id": "55555555-5555-5555-5555-555555555555",
                        "title": "A resistência caiu 18%",
                        "summary": "Três corridas independentes, mesma direcção.",
                        "status": "draft", "status_label": "Registado",
                        "classification": "INTERNAL"
                    }),
                    validations: json!([{
                        "id": "77777777-7777-7777-7777-777777777775",
                        "label": "Reprodução confirmou",
                        "note": "Segunda corrida, outro operador."
                    }]),
                    upstream: json!({
                        "truncada": false,
                        "passos": [{
                            "profundidade": 1,
                            "de": {"kind": "result", "label": "A resistência caiu 18%"},
                            "relacao_legivel": "produzido por",
                            "para": {"kind": "study_execution", "label": "Ensaio de carga · execução 3"},
                            "origem": "operation"
                        }, {
                            "profundidade": 2,
                            "de": {"kind": "study_execution", "label": "Ensaio de carga · execução 3"},
                            "relacao_legivel": "usou a metodologia",
                            "para": {"kind": "methodology_version", "label": "Medição a quatro pontas · v2"},
                            "origem": "declared"
                        }]
                    }),
                    downstream: json!({"truncada": true, "passos": []}),
                    direction: "upstream",
                    may_validate: true,
                })
            ),
            // O mesmo resultado a jusante, para quem não pode validar: sem
            // botão, porque o Core recusaria e prometer uma recusa é pior do
            // que não prometer nada.
            page!(
                "result-detail-jusante",
                Screen::Ideas,
                screens::science::result_detail(screens::science::ResultView {
                    result: json!({
                        "id": "77777777-7777-7777-7777-777777777774",
                        "workspace_id": "55555555-5555-5555-5555-555555555555",
                        "title": "A resistência caiu 18%",
                        "summary": "Três corridas independentes, mesma direcção.",
                        "status": "draft", "status_label": "Registado",
                        "classification": "INTERNAL"
                    }),
                    validations: json!([]),
                    upstream: json!({"truncada": false, "passos": []}),
                    downstream: json!({"truncada": false, "passos": []}),
                    direction: "downstream",
                    may_validate: false,
                })
            ),
            // Os seis ecrãs por onde uma pessoa constrói a cadeia.
            //
            // Entram aqui pela mesma razão que todos os outros: os quatro
            // guardas percorrem este catálogo, e um ecrã que não esteja nele
            // não é auditado — nem os botões, nem os campos, nem as âncoras.
            page!(
                "science-nova-hipotese",
                Screen::Ideas,
                screens::science::nova_hipotese(screens::science::Contexto {
                    workspace: ambiente_cientifico(),
                    message: None,
                })
            ),
            page!(
                "science-nova-metodologia",
                Screen::Ideas,
                screens::science::nova_metodologia(screens::science::Contexto {
                    workspace: ambiente_cientifico(),
                    message: Some("A classificação pedida excede a do ambiente.".to_owned()),
                })
            ),
            page!(
                "science-metodologia",
                Screen::Ideas,
                screens::science::metodologia(screens::science::MetodologiaView {
                    methodology: json!({
                        "id": "88888888-8888-8888-8888-888888888881",
                        "workspace_id": "55555555-5555-5555-5555-555555555555",
                        "title": "Medição a quatro pontas",
                        "purpose": "Separar a resistência de contacto da do material.",
                        "classification": "INTERNAL"
                    }),
                    versions: json!([
                        {"id": "88888888-8888-8888-8888-888888888883", "label": "v2",
                         "summary": "Corrente reduzida para 1 mA.", "status": "published", "status_label": "Em vigor"},
                        {"id": "88888888-8888-8888-8888-888888888882", "label": "v1",
                         "summary": "Primeira redacção.", "status": "superseded", "status_label": "Substituída",
                         "superseded_by_id": "88888888-8888-8888-8888-888888888883"}
                    ]),
                    may_create: true,
                })
            ),
            page!(
                "science-nova-versao",
                Screen::Ideas,
                screens::science::nova_versao(screens::science::NovaVersaoView {
                    methodology: json!({
                        "id": "88888888-8888-8888-8888-888888888881",
                        "title": "Medição a quatro pontas"
                    }),
                    em_vigor: Some(json!({"label": "v2"})),
                    message: None,
                })
            ),
            page!(
                "science-novo-estudo",
                Screen::Ideas,
                screens::science::novo_estudo(screens::science::NovoEstudoView {
                    workspace: ambiente_cientifico(),
                    hypotheses: json!([{
                        "id": "77777777-7777-7777-7777-777777777771",
                        "statement": "A dopagem reduz a resistência de contacto"
                    }]),
                    methodology_versions: vec![(
                        "88888888-8888-8888-8888-888888888883".to_owned(),
                        "Medição a quatro pontas · v2".to_owned(),
                    )],
                    message: None,
                })
            ),
            // O mesmo formulário sem nenhuma versão publicada: o selector diz
            // porque está vazio em vez de o parecer por engano.
            page!(
                "science-novo-estudo-sem-versoes",
                Screen::Ideas,
                screens::science::novo_estudo(screens::science::NovoEstudoView {
                    workspace: ambiente_cientifico(),
                    hypotheses: json!([]),
                    methodology_versions: Vec::new(),
                    message: None,
                })
            ),
            page!(
                "science-estudo",
                Screen::Ideas,
                screens::science::estudo(screens::science::EstudoView {
                    study: estudo_de_referencia(),
                    executions: json!([{
                        "id": "77777777-7777-7777-7777-777777777776",
                        "sequence": 3,
                        "status": "succeeded",
                        "environment": "Bancada 2"
                    }]),
                    may_create: true,
                })
            ),
            page!(
                "science-nova-execucao",
                Screen::Ideas,
                screens::science::nova_execucao(screens::science::NovaExecucaoView {
                    study: estudo_de_referencia(),
                    methodology_versions: vec![(
                        "88888888-8888-8888-8888-888888888883".to_owned(),
                        "Medição a quatro pontas · v2".to_owned(),
                    )],
                    dataset_versions: vec![(
                        "99999999-9999-9999-9999-999999999991".to_owned(),
                        "SCADA Parque A · v4".to_owned(),
                    )],
                    message: None,
                })
            ),
            page!(
                "science-execucao",
                Screen::Ideas,
                screens::science::execucao(screens::science::ExecucaoView {
                    execution: execucao_de_referencia(),
                    study: estudo_de_referencia(),
                    results: json!([{
                        "id": "77777777-7777-7777-7777-777777777774",
                        "title": "A resistência caiu 18%",
                        "classification": "INTERNAL"
                    }]),
                    may_create: true,
                })
            ),
            page!(
                "science-novo-resultado",
                Screen::Ideas,
                screens::science::novo_resultado(screens::science::NovoResultadoView {
                    execution: execucao_de_referencia(),
                    study: estudo_de_referencia(),
                    message: None,
                })
            ),
            // O formulário de validação com prova disponível: a reprodução é
            // escolhível porque há uma execução que a sustenta.
            page!(
                "result-validate",
                Screen::Ideas,
                screens::science::validate_result(screens::science::ValidateView {
                    result: json!({
                        "id": "77777777-7777-7777-7777-777777777774",
                        "title": "A resistência caiu 18%",
                        "execution_id": "77777777-7777-7777-7777-777777777776"
                    }),
                    executions: json!([{
                        "id": "77777777-7777-7777-7777-777777777776",
                        "sequence": 3,
                        "status": "succeeded"
                    }]),
                    message: None,
                })
            ),
            // O mesmo formulário sem execução nenhuma: a reprodução aparece
            // desactivada **com o motivo**, e não simplesmente ausente. Quem
            // procura a opção precisa de saber porque não a pode usar.
            page!(
                "result-validate-sem-prova",
                Screen::Ideas,
                screens::science::validate_result(screens::science::ValidateView {
                    result: json!({
                        "id": "77777777-7777-7777-7777-777777777774",
                        "title": "A resistência caiu 18%"
                    }),
                    executions: json!([]),
                    message: Some(
                        "Uma reprodução precisa da execução que a reproduziu.".to_owned(),
                    ),
                })
            ),
            // O mesmo ecrã para quem não pode usar assistência: a superfície
            // não aparece de todo. É a alínea B do contrato — o Core recusaria
            // na mesma, e mostrar o campo seria convidar a uma recusa.
            page!(
                "research-workspace-sem-assistencia",
                Screen::Ideas,
                screens::workspaces::research_workspace(screens::workspaces::WorkspaceView {
                    overview: workspace_overview.clone(),
                    sources: empty.clone(),
                    notes: json!([]),
                    documents: json!([]),
                    datasets: empty.clone(),
                    tasks: empty.clone(),
                    activity: json!([]),
                    inference_available: false,
                    may_use_assistance: false,
                    gestao: screens::workspaces::tests::gestao_de_prova(),
                })
            ),
            // E com inferência disponível, para que o caminho em que a
            // assistência funciona seja tão coberto como aquele em que não.
            page!(
                "research-workspace-com-ia",
                Screen::Ideas,
                screens::workspaces::research_workspace(screens::workspaces::WorkspaceView {
                    overview: workspace_overview,
                    sources: empty.clone(),
                    notes: json!([]),
                    documents: json!([]),
                    datasets: empty.clone(),
                    tasks: empty.clone(),
                    activity: json!([]),
                    inference_available: true,
                    may_use_assistance: true,
                    gestao: screens::workspaces::tests::gestao_de_prova(),
                })
            ),
            // Correio: quatro estados que não podem regredir — serviço ausente,
            // caixa com mensagens, leitura com conteúdo remoto bloqueado, e o
            // composer com a assistência disponível.
            page!(
                "mail-unavailable",
                Screen::Mail,
                screens::mail::mail(
                    &viewer(),
                    &mail_view(false, false),
                    &json!(null),
                    None,
                    None
                )
            ),
            page!(
                "mail",
                Screen::Mail,
                screens::mail::mail(
                    &viewer(),
                    &mail_view(true, true),
                    &json!({"items": [{
                        "id": "44444444-4444-4444-4444-444444444444",
                        "mailbox_id": "33333333-3333-3333-3333-333333333333",
                        "folder": "inbox",
                        "from_address": "parceiro@exemplo.com",
                        "from_display_name": "Instituto Parceiro",
                        "subject": "Proposta de colaboração",
                        "snippet": "Na sequência da reunião de ontem, envio…",
                        "sent_at": "2026-08-22T09:14:00Z",
                        "is_read": false,
                        "is_starred": false,
                        "has_attachments": true
                    }]}),
                    None,
                    None,
                )
            ),
            page!(
                "mail-message",
                Screen::Mail,
                screens::mail::mail(
                    &viewer(),
                    &mail_view(true, true),
                    &json!({"items": []}),
                    Some(&json!({
                        "message": {
                            "id": "44444444-4444-4444-4444-444444444444",
                            "mailbox_id": "33333333-3333-3333-3333-333333333333",
                            "folder": "inbox",
                            "from_address": "parceiro@exemplo.com",
                            "from_display_name": "Instituto Parceiro",
                            "subject": "Proposta de colaboração",
                            "sent_at": "2026-08-22T09:14:00Z",
                            "is_read": true,
                            "is_starred": false
                        },
                        "body_html": "<p>Bom dia.</p>",
                        "blocked_remote_count": 2,
                        "inline_image_count": 0,
                        "linked_domains": ["exemplo.com"],
                        "attachments": [{
                            "part_id": "2",
                            "filename": "proposta.pdf",
                            "content_type": "application/pdf",
                            "size_bytes": 481_200
                        }],
                        "to": ["ana@ocinye.com"],
                        "cc": []
                    })),
                    None,
                )
            ),
            page!(
                "mail-synced",
                Screen::Mail,
                screens::mail::mail(
                    &viewer(),
                    &{
                        let mut view = mail_view(true, true);
                        view.sync_notice = Some("3 mensagem(ns) actualizada(s).".to_owned());
                        view
                    },
                    &json!({"items": []}),
                    None,
                    None,
                )
            ),
            page!(
                "mail-compose",
                Screen::Mail,
                screens::mail::compose(
                    &mail_view(true, true),
                    &screens::mail::ComposeDraft {
                        mailbox_id: "33333333-3333-3333-3333-333333333333".to_owned(),
                        to: "parceiro@exemplo.com".to_owned(),
                        subject: "Re: Proposta de colaboração".to_owned(),
                        ..Default::default()
                    },
                )
            ),
            page!(
                "mail-compose-no-ai",
                Screen::Mail,
                screens::mail::compose(
                    &mail_view(true, false),
                    &screens::mail::ComposeDraft::default(),
                )
            ),
            page!(
                "mail-settings",
                Screen::Mail,
                screens::mail::settings(
                    &mail_view(true, true),
                    &json!({"signature": "Ana Fernandes · Ocinye", "remote_content_policy": "block"}),
                )
            ),
            // A Universal Command Surface, nos quatro estados que não podem
            // regredir: por perguntar, com resultados, com um plano à espera
            // de confirmação, e indisponível.
            page!(
                "ask-empty",
                Screen::Ask,
                screens::ask::ask(&screens::ask::AskView {
                    query: String::new(),
                    intent: "search".to_owned(),
                    outcome: json!(null),
                    may_use_ai: true,
                })
            ),
            page!(
                "ask-results",
                Screen::Ask,
                screens::ask::ask(&screens::ask::AskView {
                    query: "hidrogénio".to_owned(),
                    intent: "search".to_owned(),
                    outcome: json!({
                        "kind": "results",
                        "withheld_from_inference": 2,
                        "sources": [{
                            "entity_type": "idea",
                            "entity_id": "11111111-1111-1111-1111-111111111111",
                            "title": "Produção de hidrogénio verde em Angola",
                            "classification": "INTERNAL",
                            "excerpt": "Estudo preliminar de viabilidade com energia solar."
                        }]
                    }),
                    may_use_ai: true,
                })
            ),
            page!(
                "ask-plan",
                Screen::Ask,
                screens::ask::ask(&screens::ask::AskView {
                    query: "prepara uma resposta ao Carlos".to_owned(),
                    intent: "act".to_owned(),
                    outcome: json!({
                        "kind": "planned",
                        "requires_approval": true,
                        "plan": {
                            "id": "22222222-2222-2222-2222-222222222222",
                            "intent": "prepara uma resposta ao Carlos",
                            "steps": [
                                {"summary": "Procurar a última mensagem do Carlos", "risk": "read_only"},
                                {"summary": "Preparar uma resposta", "risk": "low_impact"},
                                {"summary": "Enviar a resposta", "risk": "external_effect"}
                            ]
                        }
                    }),
                    may_use_ai: true,
                })
            ),
            page!(
                "ask-unavailable",
                Screen::Ask,
                screens::ask::ask(&screens::ask::AskView {
                    query: "resume este projecto".to_owned(),
                    intent: "ask".to_owned(),
                    outcome: json!({
                        "kind": "unavailable",
                        "reason": "O planeamento a partir de linguagem natural depende de um nó de IA do Ocinye OS, que ainda não foi registado.",
                        "alternative": "A pesquisa e todas as acções do Workspace continuam disponíveis."
                    }),
                    may_use_ai: true,
                })
            ),
            page!(
                "search",
                Screen::Search,
                screens::search::search(
                    "hidrogénio",
                    &json!({"items": [
                        {"entity_type":"idea","entity_id":"11111111-1111-1111-1111-111111111111",
                         "title":"Produção de hidrogénio verde em Angola",
                         "excerpt":"Estudo preliminar de viabilidade com energia solar.",
                         "classification":"INTERNAL"},
                        {"entity_type":"dataset","entity_id":"22222222-2222-2222-2222-222222222222",
                         "workspace_id":"11111111-1111-1111-1111-111111111111",
                         "title":"Irradiância solar 2010–2024",
                         "excerpt":"Série horária de sete estações.",
                         "classification":"RESTRICTED"}
                    ], "total": 2}),
                    // Um resultado do corpo, para a passagem visual mostrar a
                    // citação com versão e página.
                    &json!({"items": [
                        {"file_id":"33333333-3333-3333-3333-333333333333",
                         "file_version_id":"44444444-4444-4444-4444-444444444444",
                         "sequence": 2,
                         "name":"Ensaio de irradiância — Março.pdf",
                         "excerpt":"…o coeficiente termoeléctrico medido foi de 719 µV/K…",
                         "locator": {"page": 4},
                         "classification":"INTERNAL",
                         "workspace_id":"11111111-1111-1111-1111-111111111111"}
                    ], "total": 1}),
                    &json!({"available": false, "embedded_documents": 0,
                            "message": "A pesquisa semântica depende de uma capacidade de embeddings, que não está actualmente disponível."})
                )
            ),
            page!(
                "admin-new-member",
                Screen::Admin,
                screens::administration::new_member(&empty, None)
            ),
            page!(
                "admin-credential",
                Screen::Admin,
                screens::administration::issued_credential(
                    "afernandes",
                    "MS2b-PZB8-ED2u-727v-dZN8-fFNm",
                    "2026-08-23T10:28:00Z",
                )
            ),
            page!(
                "admin-member-detail",
                Screen::Admin,
                screens::administration::member_detail(
                    &json!({
                        "full_name": "Ana Fernandes",
                        "email": "ana@ocinye.com",
                        "status": "active",
                        "institutional_position": "founder"
                    }),
                    &json!({
                        "account_status": "active",
                        "has_permanent_password": true,
                        "password_changed_at": "2026-08-22T09:14:00Z",
                        "last_successful_sign_in": "2026-08-22T10:31:00Z",
                        "recent_failed_attempts": 0,
                        "live_sessions": [{
                            "state": "active",
                            "user_agent": "Firefox 142 · macOS",
                            "ip_prefix": "10.20.30.0/24"
                        }]
                    }),
                    &json!({
                        "roles": ["research_member"],
                        "grants": [],
                        "institution_permissions": [
                            {"permission": "ideas.view", "source": "technical_role"},
                            {"permission": "projects.view", "source": "technical_role"},
                            {"permission": "ai.use", "source": "technical_role"}
                        ]
                    }),
                )
            ),
            page!(
                "unit-detail",
                Screen::Units,
                screens::workspaces::unit_detail(
                    &json!({"id": "33333333-3333-3333-3333-333333333333", "name": "Unidade", "code": "AI", "status": "active"}),
                    &json!({"items": [
                        {"person_id": "44444444-4444-4444-4444-444444444444",
                         "full_name": "Ana Ferreira", "email": "ana@ocinye.com",
                         "role": "manager"},
                        {"person_id": "55555555-5555-5555-5555-555555555555",
                         "full_name": "Bruno Cardoso", "email": "bruno@ocinye.com",
                         "role": "member"}
                    ]}),
                    &empty,
                    // Com gestão: a passagem visual existe para rever os
                    // controlos que alteram autoridade, e escondê-los aqui
                    // deixaria a área mais sensível do ecrã por rever.
                    &screens::workspaces::GestaoDePessoas {
                        pode_gerir: true,
                        candidatos: vec![(
                            "66666666-6666-6666-6666-666666666666".to_owned(),
                            "Carla Nunes · carla@ocinye.com".to_owned(),
                        )],
                        aviso: None,
                    },
                )
            ),
            // Os ecrãs de criação e os do próprio membro entraram no catálogo
            // depois de terem sido escritos, e nesse intervalo escaparam a
            // tudo: à varredura de ligações mortas, à de botões sem
            // comportamento e ao despejo visual. Estavam correctos, mas por
            // sorte — nada os obrigava a estar. Um ecrã que não está aqui não
            // é verificado, e é assim que uma ligação morta sobrevive.
            page!("units-new", Screen::Units, screens::lists::new_unit(None)),
            // Os formulários de criação têm duas formas — com destinos e sem
            // eles — e só a primeira mostra o botão de submissão. Com fixtures
            // vazias os botões «Criar Referência», «Criar Dataset» e «Promover
            // a Projecto» nunca chegavam a ser renderizados, e por isso nunca
            // eram verificados. Uma varredura que só vê o estado vazio prova
            // metade do ecrã.
            page!(
                "sources-new-com-destino",
                Screen::Bibliography,
                screens::lists::new_source(&destinos, None)
            ),
            page!(
                "datasets-new-com-destino",
                Screen::Datasets,
                screens::lists::new_dataset(&destinos, None)
            ),
            page!(
                "projects-new-com-candidata",
                Screen::Projects,
                screens::lists::new_project(&candidatas, None, None)
            ),
            page!(
                "ideas-new-com-unidade",
                Screen::Ideas,
                screens::lists::new_idea(&unidades, None)
            ),
            page!(
                "projects-new",
                Screen::Projects,
                screens::lists::new_project(&empty, None, None)
            ),
            page!(
                "sources-new",
                Screen::Bibliography,
                screens::lists::new_source(&empty, None)
            ),
            page!(
                "datasets-new",
                Screen::Datasets,
                screens::lists::new_dataset(&empty, None)
            ),
            page!(
                "settings-account",
                Screen::Settings,
                screens::settings::account(
                    &json!({
                        "id": "44444444-4444-4444-4444-444444444444",
                        "full_name": "João Manuel",
                        "email": "jmanuel@ocinye.com",
                        "status": "active"
                    }),
                    &json!({"name": "Ocinye", "code": "OCY"}),
                    &ocinye_contracts::AvatarChoice::Initials,
                    None,
                    false,
                )
            ),
            // A mesma conta com um avatar do produto escolhido. As duas formas
            // do ecrã têm de entrar no catálogo: com fixture só de um lado, a
            // varredura provaria metade da superfície de escolha.
            page!(
                "settings-account-com-preset",
                Screen::Settings,
                screens::settings::account(
                    &json!({
                        "id": "44444444-4444-4444-4444-444444444444",
                        "display_name": "João Manuel",
                        "email": "jmanuel@ocinye.com",
                        "status": "active"
                    }),
                    &json!({"name": "Ocinye", "code": "OCY"}),
                    &ocinye_contracts::AvatarChoice::Preset {
                        preset: "compute-01".to_owned()
                    },
                    None,
                    true,
                )
            ),
            page!(
                "settings-security",
                Screen::Settings,
                screens::settings::security(Some(&empty), None, None)
            ),
            // O mesmo ecrã com a lista de sessões por ler. É o estado que a
            // varredura não via: sem esta fixture, «não há outras sessões» e
            // «não conseguimos saber» renderizam ambos e nada os distingue.
            page!(
                "settings-security-sem-lista",
                Screen::Settings,
                screens::settings::security(None, None, None)
            ),
            page!("help", Screen::Help, screens::help::help()),
            // Os dois recortes de Ideias, e uma lista truncada.
            //
            // Uma varredura que só rende o recorte de origem nunca vê a tab
            // «Minhas» como ligação — vê-a sempre como o item seleccionado — e
            // por isso não prova que ela leva a lado nenhum. E uma lista que
            // cabe toda na página nunca mostra o campo de filtro a dizer que só
            // filtra a página.
            page!(
                "ideas-minhas",
                Screen::Ideas,
                screens::lists::ideas(
                    &viewer(),
                    &muitas,
                    screens::lists::Slice {
                        mine: true,
                        ..Default::default()
                    }
                )
            ),
            page!(
                "ideas-truncada",
                Screen::Ideas,
                screens::lists::ideas(&viewer(), &muitas, screens::lists::Slice::default())
            ),
            page!(
                "projects-meus",
                Screen::Projects,
                screens::lists::projects(
                    &viewer(),
                    &muitas,
                    screens::lists::Slice {
                        mine: true,
                        ..Default::default()
                    }
                )
            ),
        ]
    }

    /// Despeja os ecrãs em `target/preview/` para inspecção visual.
    ///
    /// Ignorado por omissão — escreve ficheiros, o que uma suite de testes não
    /// deve fazer sem ser pedido. Correr com:
    /// `cargo test -p ocinye-workspace -- --ignored despeja_os_ecras`
    #[test]
    #[ignore = "escreve ficheiros; serve a passagem visual, não a verificação"]
    fn despeja_os_ecras_para_inspeccao() {
        let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/preview");
        std::fs::create_dir_all(&out).expect("criar target/preview");

        for (name, html) in catalogue() {
            // O despejo é visto directamente do disco, sem servidor: os
            // caminhos absolutos de `/static/` não resolveriam.
            let html = html.replace("\"/static/", "\"../../apps/workspace/static/");
            std::fs::write(out.join(format!("{name}.html")), html).expect("escrever ecrã");
        }

        eprintln!("ecrãs em {}", out.display());
    }

    /// Nenhuma ligação renderizada aponta para um ecrã que não existe.
    ///
    /// Existe por uma razão concreta: o dossier especifica acções cujo ecrã de
    /// destino não está entre os 20 que especifica — `Novo Projecto`,
    /// `Nova Nota`, `Novo Dataset`, `Convidar Membro`, entre outras.
    /// Renderizá-las como ligações produziria 404s silenciosos.
    ///
    /// Estão implementadas como visivelmente indisponíveis. Se alguém as ligar
    /// a um ecrã inexistente, este teste falha.
    #[test]
    fn nenhuma_ligacao_aponta_para_um_ecra_inexistente() {
        let every_screen: String = catalogue().into_iter().map(|(_, html)| html).collect();
        let mut dead: Vec<String> = hrefs(&every_screen)
            .into_iter()
            .filter(|href| !matches_route(href))
            .collect();
        dead.sort_unstable();
        dead.dedup();

        assert!(
            dead.is_empty(),
            "ligações para ecrãs inexistentes:\n  {}\n\
             Ou o ecrã é implementado, ou a acção é marcada com `not_yet_available()`.",
            dead.join("\n  ")
        );
    }

    /// Uma consulta falhada nunca se parece com uma consulta vazia.
    ///
    /// # Os quatro factos
    ///
    /// «A consulta correu e devolveu zero», «a consulta falhou», «não tem
    /// autorização» e «a capacidade não está configurada» são quatro coisas
    /// diferentes, e a única que autoriza a frase *não há nada* é a primeira.
    ///
    /// Este teste cobre a família onde o defeito reapareceu: a Segurança dizia
    /// «Não há sessões activas para além desta» quando o Core não tinha
    /// respondido — uma afirmação sobre a segurança da conta de alguém, feita
    /// sem saber. E o feed institucional dizia que não se tinha passado nada na
    /// instituição quando o que se tinha passado era o Core não responder.
    #[test]
    fn uma_consulta_falhada_nao_se_parece_com_uma_vazia() {
        // Zero sessões, lidas com sucesso.
        let vazia = screens::settings::security(Some(&json!([])), None, None).to_html();
        // A lista não pôde ser lida.
        let falhada = screens::settings::security(None, None, None).to_html();

        assert!(
            vazia.contains("Não há sessões activas"),
            "zero sessões deixou de ser dito como zero"
        );
        assert!(
            !falhada.contains("Não há sessões activas"),
            "uma lista por ler afirma que não há outras sessões:\n{falhada}"
        );
        // Um fragmento contíguo: o texto quebra em várias linhas no HTML, e
        // procurar uma frase inteira falharia por causa da indentação.
        assert!(
            falhada.contains("não pôde ser lida"),
            "uma lista por ler não diz que não sabe:\n{falhada}"
        );
        assert_ne!(
            vazia, falhada,
            "os dois estados renderizam exactamente o mesmo documento"
        );
    }

    /// O conteúdo principal de um ecrã não é lido com `optional`.
    ///
    /// `optional` transforma uma falha do Core em `null`, e um ecrã que
    /// renderize `null` como lista mostra zero. Para dados secundários — uma
    /// contagem no cabeçalho, um estado de IA — isso é aceitável: a página tem
    /// outra coisa para dizer. Para o conteúdo, não: a página inteira passa a
    /// afirmar vazio.
    ///
    /// A Actividade estava assim.
    #[test]
    fn o_conteudo_principal_nao_e_lido_com_optional() {
        let rotas = include_str!("../routes.rs");

        for (handler, endpoint) in [
            ("async fn activity(", "/api/v1/activity?page_size=100"),
            ("async fn bibliography(", "/api/v1/sources?page_size=50"),
            ("async fn settings_security(", "/api/v1/auth/sessions"),
        ] {
            let corpo = rotas
                .split(handler)
                .nth(1)
                .and_then(|resto| resto.split("\n}").next())
                .unwrap_or_default();
            assert!(
                corpo.contains(endpoint),
                "o handler mudou de consulta; este teste ficou a olhar para o sítio errado"
            );
            assert!(
                !corpo.contains(&format!(r#"optional(&state, &member, "{endpoint}")"#)),
                "{handler} lê o seu conteúdo com `optional`: uma falha do Core vira lista vazia"
            );
        }
    }

    /// Uma mensagem dentro de um cartão não usa a classe do ecrã inteiro.
    ///
    /// # O que se via
    ///
    /// `oc-notice` é a classe dos ecrãs de excepção — 404, recusa, falha — que
    /// vivem sozinhos numa página: `margin: 96px auto`, centrados, largura
    /// máxima de 520px. Aplicada a uma linha dentro do cartão da imagem de
    /// perfil, abria um vazio da altura de um ecrã com a frase suspensa ao meio
    /// e o avatar empurrado para o fundo.
    ///
    /// Nenhum teste de comportamento apanha isto: o texto está certo, a
    /// mensagem é a correcta, o HTML é válido. Só se vê a olho — como a colisão
    /// de cascata no rodapé da barra.
    #[test]
    fn uma_mensagem_em_cartao_nao_usa_a_classe_do_ecra_inteiro() {
        let mut trocadas: Vec<String> = Vec::new();

        for (ecra, html) in catalogue() {
            // Um ecrã de excepção é a casa legítima de `oc-notice` — e
            // reconhece-se por não ter shell: 404 e recusa vivem sozinhos numa
            // página, precisamente porque a shell exige um contexto que nesses
            // momentos pode não existir. A distinção é estrutural, e não uma
            // lista de nomes que envelheceria com o próximo ecrã.
            if !html.contains(r#"class="oc-shell""#) {
                continue;
            }
            for pedaco in html.split(r#"class="oc-notice"#).skip(1) {
                let resto = pedaco.split('"').next().unwrap_or_default();
                // `oc-notice__tile` e `oc-notice__reference` pertencem ao ecrã.
                if resto.starts_with("__") {
                    continue;
                }
                trocadas.push(format!("{ecra}: class=\"oc-notice{resto}\""));
            }
        }

        assert!(
            trocadas.is_empty(),
            "mensagens em cartão a usar a classe do ecrã de excepção:\n  {}\n\
             `oc-notice` centra e afasta 96px; dentro de um cartão abre um vazio.",
            trocadas.join("\n  "),
        );
    }

    /// A paginação leva a páginas reais, e não perde os filtros.
    ///
    /// # A linha 51
    ///
    /// As listas pediam uma página e mostravam «1–50 de 213». Era honesto e não
    /// resolvia nada: a quinquagésima primeira linha não tinha caminho nenhum
    /// até ela — nem pela lista, nem pelo filtro, que só via as cinquenta.
    #[test]
    fn a_paginacao_leva_a_paginas_reais() {
        let muitas = |pagina: i64, total_paginas: i64| {
            json!({
                "items": [{"id": "aaaaaaaa-0000-0000-0000-000000000001", "code": "AI-IDEA-001",
                           "title": "Uma", "state": "exploration", "classification": "INTERNAL",
                           "unit_code": "AI", "kind": "idea"}],
                "page": pagina, "page_size": 50, "total": 213, "total_pages": total_paginas
            })
        };

        // Primeira página: há seguinte, não há anterior.
        let primeira =
            screens::lists::ideas(&viewer(), &muitas(1, 5), screens::lists::Slice::default())
                .to_html();
        assert!(
            primeira.contains(r#"href="/ideas?page=2""#),
            "a primeira página não leva à segunda"
        );
        assert!(
            !primeira.contains("Anterior"),
            "a primeira página oferece um «anterior» que não existe"
        );

        // Página do meio: os dois lados.
        let meio =
            screens::lists::ideas(&viewer(), &muitas(3, 5), screens::lists::Slice::default())
                .to_html();
        assert!(meio.contains(r#"href="/ideas?page=2""#), "falta o anterior");
        assert!(meio.contains(r#"href="/ideas?page=4""#), "falta o seguinte");

        // Última página: há anterior, não há seguinte.
        let ultima =
            screens::lists::ideas(&viewer(), &muitas(5, 5), screens::lists::Slice::default())
                .to_html();
        assert!(
            ultima.contains(r#"href="/ideas?page=4""#),
            "falta o anterior"
        );
        assert!(
            !ultima.contains("Seguinte"),
            "a última página oferece um «seguinte» que não existe"
        );

        // E o recorte viaja com a página.
        let minhas = screens::lists::ideas(
            &viewer(),
            &muitas(1, 5),
            screens::lists::Slice {
                mine: true,
                ..Default::default()
            },
        )
        .to_html();
        // `&amp;` e não `&`: num atributo HTML o «e» comercial é escapado, e é
        // assim que tem de estar. O browser lê `&`.
        assert!(
            minhas.contains(r#"href="/ideas?mine=true&amp;page=2""#),
            "a segunda página perdeu o recorte «minhas»:\n{}",
            minhas
                .split("oc-page-link")
                .nth(1)
                .unwrap_or("sem controlos de página")
                .chars()
                .take(120)
                .collect::<String>()
        );
    }

    /// Uma lista que cabe numa página não mostra controlos de página.
    ///
    /// Um «seguinte» que não leva a lado nenhum é a mesma promessa por cumprir
    /// que os controlos anteriores faziam.
    #[test]
    fn uma_lista_de_uma_pagina_nao_mostra_paginacao() {
        let uma = json!({
            "items": [{"id": "aaaaaaaa-0000-0000-0000-000000000001", "code": "AI-IDEA-001",
                       "title": "Uma", "state": "exploration", "classification": "INTERNAL",
                       "unit_code": "AI", "kind": "idea"}],
            "page": 1, "page_size": 50, "total": 1, "total_pages": 1
        });
        let html =
            screens::lists::ideas(&viewer(), &uma, screens::lists::Slice::default()).to_html();
        assert!(!html.contains("Anterior"));
        assert!(!html.contains("Seguinte"));
        assert!(
            html.contains("1–1 de 1 ideia"),
            "o rodapé não conta a linha única"
        );
    }

    /// Uma resposta sem forma de página não ganha controlos inventados.
    ///
    /// Unidades e Agentes chegam inteiras do Core: não há segunda página para
    /// onde ir, e pôr o controlo lá seria prometer uma que não existe.
    #[test]
    fn uma_lista_nao_paginada_nao_ganha_controlos() {
        let inteira = json!({
            "items": [{"id": "1", "code": "AI", "name": "Unidade", "status": "active"}],
            "total": 1
        });
        let html = screens::lists::units(&viewer(), &inteira).to_html();
        assert!(!html.contains("Anterior"));
        assert!(!html.contains("Seguinte"));
    }

    /// O Workspace nunca inventa uma unidade principal.
    ///
    /// # Os três casos
    ///
    /// > **The Workspace never invents a primary Unit. A query requiring Unit
    /// > scope either has one unambiguous eligible Unit or requires an explicit
    /// > authorised choice.**
    ///
    /// Sem unidades, o recorte não existe e a tab diz porquê. Com uma, não há
    /// ambiguidade e ela escolhe-se sozinha — obrigar a escolher entre uma
    /// opção é cerimónia. Com várias, a escolha é do membro: nem a primeira,
    /// nem a mais antiga, nem a de nome alfabeticamente primeiro.
    ///
    /// Uma instituição não tem unidade principal só porque uma consulta precisa
    /// de uma.
    #[test]
    fn o_recorte_por_unidade_nunca_escolhe_por_si() {
        use screens::lists::Slice;

        let lista = json!({
            "items": [], "page": 1, "page_size": 50, "total": 0, "total_pages": 0
        });

        // Sem unidades: a tab está lá, esbatida, e diz porquê.
        let nenhuma = screens::lists::ideas(&viewer(), &lista, Slice::default()).to_html();
        assert!(nenhuma.contains("Da Unidade"));
        assert!(
            nenhuma.contains("Não pertence a nenhuma unidade"),
            "sem unidades, a tab não diz porquê"
        );
        assert!(
            !nenhuma.contains(r#"href="/ideas?unit"#),
            "sem unidades, a tab oferece um recorte que não existe"
        );

        // Com uma: leva ao recorte, e o pedido resolve-se sem perguntar.
        let uma = Slice {
            units: vec![(
                "11111111-1111-1111-1111-111111111111".to_owned(),
                "Energia".to_owned(),
            )],
            ..Slice::default()
        };
        let com_uma = screens::lists::ideas(&viewer(), &lista, uma).to_html();
        assert!(
            com_uma.contains(r#"href="/ideas?unit=true""#),
            "com uma unidade, a tab não leva ao recorte"
        );

        // Com várias, e sem escolha: nem lista filtrada, nem lista inteira.
        let varias = Slice {
            units: vec![
                (
                    "11111111-1111-1111-1111-111111111111".to_owned(),
                    "Energia".to_owned(),
                ),
                (
                    "22222222-2222-2222-2222-222222222222".to_owned(),
                    "Sistemas".to_owned(),
                ),
            ],
            awaiting_unit: true,
            ..Slice::default()
        };
        let a_escolher =
            screens::lists::ideas(&viewer(), &serde_json::Value::Null, varias.clone()).to_html();
        assert!(
            a_escolher.contains("Nenhuma unidade escolhida"),
            "com várias unidades, o ecrã não pede a escolha"
        );
        assert!(
            a_escolher.contains("Energia") && a_escolher.contains("Sistemas"),
            "o selector não oferece as unidades elegíveis"
        );
        assert!(
            !a_escolher.contains("0 ideias"),
            "uma escolha por fazer aparece como zero resultados"
        );

        // E nenhuma delas foi escolhida por si: nada está seleccionado.
        let seleccionadas = a_escolher.matches("selected").count();
        assert!(
            seleccionadas <= 1,
            "o selector já vem com uma unidade escolhida: {seleccionadas} opções marcadas"
        );
    }

    /// Escolhida a unidade, ela viaja com a página.
    ///
    /// Uma segunda página que largasse a unidade devolveria a segunda página da
    /// instituição inteira sob um cabeçalho que diz o nome de uma unidade.
    #[test]
    fn a_unidade_escolhida_viaja_com_a_pagina() {
        use screens::lists::Slice;

        let lista = json!({
            "items": [{"id": "aaaaaaaa-0000-0000-0000-000000000001", "code": "AI-IDEA-001",
                       "title": "Uma", "state": "exploration", "classification": "INTERNAL",
                       "unit_code": "AI", "kind": "idea"}],
            "page": 1, "page_size": 50, "total": 213, "total_pages": 5
        });
        let escolhida = Slice {
            unit_id: Some("11111111-1111-1111-1111-111111111111".to_owned()),
            units: vec![(
                "11111111-1111-1111-1111-111111111111".to_owned(),
                "Energia".to_owned(),
            )],
            ..Slice::default()
        };

        let html = screens::lists::ideas(&viewer(), &lista, escolhida).to_html();
        assert!(
            html.contains(
                r#"href="/ideas?unit_id=11111111-1111-1111-1111-111111111111&amp;page=2""#
            ),
            "a segunda página perdeu a unidade escolhida"
        );
    }

    /// Nenhuma tab é decorativa.
    ///
    /// # A categoria proibida
    ///
    /// Cada tab visível é uma de três coisas: o recorte que está a ser
    /// mostrado, uma ligação para outro recorte real, ou uma capacidade que o
    /// produto ainda não tem — e esta última tem de dizer **qual**.
    ///
    /// A quarta hipótese — um `<span>` esbatido sem destino e sem razão — é a
    /// que este teste existe para impedir. Um recorte que não muda nada e não
    /// explica porquê é mobiliário com aspecto de escolha.
    #[test]
    fn nenhuma_tab_e_decorativa() {
        let mut mudas: Vec<String> = Vec::new();

        for (ecra, html) in catalogue() {
            for pedaco in html.split(r#"role="tab""#).skip(1) {
                let etiqueta = pedaco.split('>').next().unwrap_or_default();
                let rotulo: String = pedaco
                    .split('>')
                    .nth(1)
                    .unwrap_or_default()
                    .split('<')
                    .next()
                    .unwrap_or_default()
                    .trim()
                    .to_owned();

                let seleccionada = etiqueta.contains(r#"aria-selected="true""#);
                let ligacao = pedaco
                    .split('<')
                    .next()
                    .unwrap_or_default()
                    .contains("href=")
                    || etiqueta.contains("href=");
                let declarada = etiqueta.contains("title=") && etiqueta.contains("aria-disabled");

                if !(seleccionada || ligacao || declarada) {
                    mudas.push(format!("{ecra}: «{rotulo}»"));
                }
            }
        }

        assert!(
            mudas.is_empty(),
            "tabs sem recorte e sem razão:\n  {}\n\
             Ou estão seleccionadas, ou levam a um recorte real, ou dizem porquê não podem.",
            mudas.join("\n  "),
        );
    }

    /// Uma capacidade em falta diz qual é, e não «não está disponível».
    ///
    /// A Ajuda distingue cinco estados para o membro, e a barra tem de os
    /// distinguir também. «Este recorte ainda não está disponível» servia para
    /// tudo: uma capacidade que o produto não tem, uma consulta que o Core não
    /// expõe, e uma dependência por configurar. Quem lê fica sem saber se
    /// espera, se pede, ou se desiste.
    #[test]
    fn uma_capacidade_em_falta_diz_qual_e() {
        // Procurada no documento inteiro, e não dentro de uma etiqueta: a
        // ordem dos atributos é escolha do renderizador, e a primeira versão
        // deste teste partia dela — saltava todas as tabs em silêncio e passava
        // com a frase de volta. Um teste que não sabe encontrar aquilo que
        // proíbe é um teste que aprova tudo.
        const GENERICA: &str = "Este recorte da lista ainda não está disponível";
        let mut vagas: Vec<&'static str> = Vec::new();

        for (ecra, html) in catalogue() {
            if html.contains(GENERICA) {
                vagas.push(ecra);
            }
        }

        assert!(
            vagas.is_empty(),
            "ecrãs onde uma capacidade em falta não diz qual é: {vagas:?}\n\
             «não está disponível» serve para uma capacidade que não existe, uma \
             configuração que falta e uma avaria — e as três pedem coisas diferentes."
        );
    }

    /// O campo de filtro não promete mais do que a página tem.
    ///
    /// # O que estava errado
    ///
    /// Todas as listas diziam «Pesquisar datasets…» e filtravam, no browser, as
    /// linhas que a página tinha recebido — no máximo cinquenta. Com duzentos
    /// datasets, escrever o nome do centésimo dava zero resultados, e nada na
    /// interface dizia que a pesquisa nunca o tinha visto.
    ///
    /// O rodapé já dizia «1–50 de 200». O campo é que não dizia nada.
    #[test]
    fn o_filtro_da_lista_nao_promete_o_que_nao_ve() {
        let mut mentiras: Vec<String> = Vec::new();

        for (ecra, html) in catalogue() {
            let Some(campo) = html.split(r#"data-oc="table-filter""#).nth(1) else {
                continue;
            };
            let etiqueta = campo.split('>').next().unwrap_or_default();
            let truncada = html.contains(" de 200 ") || html.contains(" de 213 ");

            // Nunca «Pesquisar»: o controlo filtra, e filtrar não é pesquisar.
            if etiqueta.contains("Pesquisar") {
                mentiras.push(format!("{ecra}: o campo diz «Pesquisar» e só filtra"));
            }
            if truncada && !etiqueta.contains("nesta página") {
                mentiras.push(format!("{ecra}: a lista está truncada e o campo não o diz"));
            }
        }

        assert!(
            mentiras.is_empty(),
            "campos de filtro que prometem mais do que vêem:\n  {}",
            mentiras.join("\n  "),
        );
    }

    /// Nenhum elemento interactivo sem contrato.
    ///
    /// A invariante principal desta auditoria: se um membro vê uma opção, essa
    /// opção tem comportamento definido. Para cada `<button>` renderizado, uma
    /// destas tem de ser verdade:
    ///
    /// - submete um formulário (`type="submit"`);
    /// - está ligado à camada de interacção (`data-oc`);
    /// - está declarado indisponível (`aria-disabled` ou `disabled`).
    ///
    /// Um `<button type="button">` sem nenhuma das três é um botão que não faz
    /// nada — exactamente o que este teste existe para impedir de voltar.
    #[test]
    fn nenhum_botao_existe_sem_comportamento() {
        let mut mudos: Vec<String> = Vec::new();

        for (screen, html) in catalogue() {
            for tag in buttons(&html) {
                // `type="submit"` só é comportamento se houver formulário à
                // volta: fora de um, submeter não é coisa nenhuma. Sem esta
                // metade, trocar `type="button"` por `type="submit"` faria os
                // botões mudos passarem a verdes sem passarem a funcionar.
                let submete = tag.contains(r#"type="submit""#) && dentro_de_formulario(&html, &tag);
                let ligado = tag.contains("data-oc=");
                let declarado = tag.contains("aria-disabled") || tag.contains("disabled");

                if !(submete || ligado || declarado) {
                    mudos.push(format!("{screen}: {tag}"));
                }
            }
        }

        assert!(
            mudos.is_empty(),
            "botões sem comportamento definido:\n  {}\n\
             Ou submetem, ou têm handler, ou declaram-se indisponíveis.",
            mudos.join("\n  ")
        );
    }

    /// Nenhum `<input>` ou `<select>` fora de um formulário e sem handler.
    ///
    /// Um campo que não submete e que nenhum script lê é um campo onde se
    /// escreve para nada.
    #[test]
    fn nenhum_campo_existe_sem_destino() {
        let mut orfaos: Vec<String> = Vec::new();

        for (screen, html) in catalogue() {
            for tag in tags(&html, "input") {
                // Escondidos e desactivados não recolhem nada de ninguém.
                if tag.contains(r#"type="hidden""#) || tag.contains("disabled") {
                    continue;
                }
                if !tag.contains("data-oc=") && !dentro_de_formulario(&html, &tag) {
                    orfaos.push(format!("{screen}: {tag}"));
                }
            }
        }

        assert!(
            orfaos.is_empty(),
            "campos sem destino:\n  {}",
            orfaos.join("\n  ")
        );
    }

    /// Nenhuma âncora vazia ou para `#`.
    #[test]
    fn nenhuma_ancora_leva_a_lado_nenhum() {
        for (screen, html) in catalogue() {
            for href in hrefs(&html) {
                assert!(
                    href != "#" && !href.is_empty(),
                    "{screen} tem uma âncora para «{href}»"
                );
            }
        }
    }

    /// Extrai as tags de abertura de um elemento.
    fn tags(html: &str, name: &str) -> Vec<String> {
        let open = format!("<{name}");
        let mut found = Vec::new();
        let mut rest = html;
        while let Some(start) = rest.find(&open) {
            let after = &rest[start..];
            let Some(end) = after.find('>') else { break };
            found.push(after[..=end].to_owned());
            rest = &after[end..];
        }
        found
    }

    fn buttons(html: &str) -> Vec<String> {
        tags(html, "button")
    }

    /// Se a tag aparece entre `<form` e `</form>`.
    fn dentro_de_formulario(html: &str, tag: &str) -> bool {
        let Some(position) = html.find(tag) else {
            return false;
        };
        let before = &html[..position];
        let abertos = before.matches("<form").count();
        let fechados = before.matches("</form>").count();
        abertos > fechados
    }

    /// As acções sem ecrã continuam visíveis, e declaradas como indisponíveis.
    ///
    /// O oposto de as esconder: o design exige que estejam lá.
    #[test]
    fn as_accoes_sem_ecra_ficam_visiveis_e_declaradas() {
        let html: String = catalogue().into_iter().map(|(_, html)| html).collect();

        for action in [
            "Novo Projecto",
            "Nova Nota",
            "Nova Referência",
            "Nova Tarefa",
        ] {
            assert!(
                html.contains(action),
                "a acção {action} deixou de estar visível"
            );
        }
        assert!(
            html.contains("Ainda não disponível"),
            "as acções sem ecrã devem declarar-se indisponíveis"
        );
    }
}
