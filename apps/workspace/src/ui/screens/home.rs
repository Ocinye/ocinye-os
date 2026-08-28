//! Home / Dashboard.
//!
//! Responde a uma pergunta: **o que precisa da minha atenção?**
//! Sem vanity metrics (`design/README.md` §6.2).

use leptos::prelude::*;
use serde_json::Value;

use crate::ui::components::{
    badge, button, card, kpi_card, pill, section_head, Button, Kpi, Tone, Variant,
};

/// Tudo o que o painel mostra, já autorizado pelo Core.
pub struct Dashboard {
    /// Saudação, dependente da hora.
    pub greeting: String,
    /// Nome do membro.
    pub name: String,
    /// Contadores institucionais.
    pub kpis: Vec<Kpi>,
    /// Research workspaces a continuar.
    pub workspaces: Value,
    /// Tarefas atribuídas e abertas.
    pub tasks: Value,
    /// Actividade recente.
    pub activity: Value,
    /// Estado do Intelligence Plane.
    pub intelligence: Value,
    /// Se o membro pode mesmo criar uma ideia.
    ///
    /// O Home oferecia «Nova Ideia» a toda a gente, enquanto a topbar já
    /// escondia «+ Criar» a quem não tem a permissão. Quem não a tem chegava
    /// ao formulário e era recusado — um botão para uma recusa (briefing §52).
    pub can_create_idea: bool,
}

fn text(row: &Value, key: &str) -> String {
    row.get(key)
        .and_then(Value::as_str)
        .unwrap_or("—")
        .to_owned()
}

fn items(payload: &Value) -> Vec<Value> {
    payload
        .get("items")
        .and_then(Value::as_array)
        .or_else(|| payload.as_array())
        .cloned()
        .unwrap_or_default()
}

/// O painel.
pub fn home(data: Dashboard) -> impl IntoView {
    let Dashboard {
        greeting,
        name,
        kpis,
        workspaces,
        tasks,
        activity,
        intelligence,
        can_create_idea,
    } = data;

    let open_tasks = items(&tasks).len();
    let in_review = items(&workspaces)
        .iter()
        .filter(|w| text(w, "kind") == "idea")
        .count();

    let subtitle = summary(open_tasks, in_review);

    view! {
        <div class="oc-page oc-page--home">
            <div class="oc-head">
                <div class="oc-head__text">
                    <h1 class="oc-head--lg">{format!("{greeting}, {name}")}</h1>
                    <p>{subtitle}</p>
                </div>
                <div class="oc-head__actions">
                    // Visível sempre, e declarada quando não se pode usar.
                    {button(if can_create_idea {
                        Button::new("Nova Ideia", Variant::Secondary).href("/ideas/new")
                    } else {
                        Button::new("Nova Ideia", Variant::Secondary)
                            .unavailable_because("Não tem autorização para criar ideias.")
                    })}
                    {button(Button::new("Novo Projecto", Variant::Secondary).not_yet_available())}
                    {button(
                        Button::new("Prompt Ocinye", Variant::Primary).href("/ai/prompt").with_dot(),
                    )}
                </div>
            </div>

            <div class="oc-grid oc-grid--4 oc-mb-5" >
                {kpis.into_iter().map(kpi_card).collect_view()}
            </div>

            <div class="oc-grid oc-grid--main">
                <div>
                    {continue_work(&workspaces)}
                    {pending_tasks(&tasks)}
                </div>
                <div>
                    {ai_card(&intelligence)}
                    {recent_activity(&activity)}
                    {quick_access(can_create_idea)}
                </div>
            </div>
        </div>
    }
}

/// O subtítulo do painel, construído a partir do que existe.
fn summary(tasks: usize, ideas: usize) -> String {
    match (tasks, ideas) {
        (0, 0) => "Nada precisa da sua atenção neste momento.".to_owned(),
        (0, i) => format!("Tem {i} itens de investigação a que tem acesso."),
        (1, 0) => "Tem 1 tarefa atribuída.".to_owned(),
        (t, 0) => format!("Tem {t} tarefas atribuídas."),
        (1, i) => format!("Tem 1 tarefa atribuída e {i} itens de investigação em curso."),
        (t, i) => format!("Tem {t} tarefas atribuídas e {i} itens de investigação em curso."),
    }
}

fn continue_work(payload: &Value) -> impl IntoView {
    let rows = items(payload);
    let empty = rows.is_empty();

    let body = if empty {
        // Os tiles do ramo cheio trazem o seu próprio `oc-card__body`; o estado
        // vazio não tem tiles, por isso precisa do seu.
        view! {
            <div class="oc-card__body">
                <p class="oc-muted">
                    "Ainda não há trabalho de investigação a que tenha acesso. Crie uma ideia para
                     começar."
                </p>
            </div>
        }
        .into_any()
    } else {
        view! {
            <div class="oc-split oc-split--2" >
                {rows
                    .iter()
                    .take(3)
                    .map(|row| {
                        let id = text(row, "id");
                        let kind = text(row, "kind").to_uppercase();
                        let classification = text(row, "classification");
                        view! {
                            <a
                                href=format!("/workspaces/{id}")
                                class="oc-card__body oc-card__body--tile"
                            >
                                <div class="oc-row oc-gap-5" >
                                    {pill(kind)}
                                    <span class="oc-mono" >
                                        {text(row, "code")}
                                    </span>
                                </div>
                                <div class="oc-fill oc-t-item" >
                                    {text(row, "title")}
                                </div>
                                <div class="oc-row oc-gap-5" >
                                    {crate::ui::components::classification_badge(&classification)}
                                </div>
                            </a>
                        }
                    })
                    .collect_view()}
            </div>
        }
        .into_any()
    };

    view! {
        <section class="oc-card oc-mb-5" >
            // A etiqueta é a do dossier (§6.2); o «Ver tudo» é nosso, e fica:
            // o cartão mostra três, e há mais para lá deles.
            {section_head(
                "Continuar trabalho",
                Some(("Ver tudo".into(), "/my-work".into())),
                Some("RESEARCH WORKSPACES".to_owned()),
            )}
            {body}
        </section>
    }
}

fn pending_tasks(payload: &Value) -> impl IntoView {
    let rows = items(payload);

    let body = if rows.is_empty() {
        view! { <p class="oc-muted">"Não tem tarefas abertas."</p> }.into_any()
    } else {
        view! {
            <div>
                {rows
                    .iter()
                    .take(6)
                    .map(|row| {
                        let state = text(row, "state");
                        let due = row.get("due_on").and_then(Value::as_str);
                        let workspace = text(row, "workspace_id");
                        view! {
                            <a
                                href=format!("/workspaces/{workspace}")
                                class="oc-list__row"
                            >
                                <span class="oc-fill oc-truncate oc-t-cell" >
                                    {text(row, "title")}
                                </span>
                                {badge(state.clone(), Tone::of(&state))}
                                <span class="oc-mono oc-list__meta" >
                                    {due.unwrap_or("sem prazo").to_owned()}
                                </span>
                            </a>
                        }
                    })
                    .collect_view()}
            </div>
        }
        .into_any()
    };

    card(
        section_head(
            "Tarefas pendentes",
            Some(("Ver tudo".into(), "/my-work".into())),
            None,
        ),
        body,
    )
}

fn recent_activity(payload: &Value) -> impl IntoView {
    let rows = items(payload);

    let body = if rows.is_empty() {
        view! { <p class="oc-muted">"Ainda não há actividade."</p> }.into_any()
    } else {
        view! {
            <div class="oc-col oc-gap-8" >
                {rows
                    .iter()
                    .take(8)
                    .map(|row| {
                        view! {
                            <div class="oc-row oc-gap-6" >
                                <i
                                    aria-hidden="true"
                                    class="oc-dot"
                                ></i>
                                <div class="oc-fill" >
                                    <div class="oc-t-note" >
                                        {text(row, "summary")}
                                    </div>
                                    <div class="oc-mono oc-t-ghost" >
                                        {text(row, "actor_name")}
                                    </div>
                                </div>
                            </div>
                        }
                    })
                    .collect_view()}
            </div>
        }
        .into_any()
    };

    view! {
        <section class="oc-card oc-mb-5" >
            {section_head("Actividade recente", Some(("Ver tudo".into(), "/activity".into())), None)}
            <div class="oc-card__body">{body}</div>
        </section>
    }
}

/// O cartão de IA.
///
/// Sem nó enrolado, explica o estado real em vez de anunciar uma capacidade que
/// não existe.
fn ai_card(status: &Value) -> impl IntoView {
    let available = status
        .get("available")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let message = status
        .get("message")
        .and_then(Value::as_str)
        .unwrap_or("Nenhum nó de IA Ocinye está actualmente disponível.")
        .to_owned();

    let title = if available {
        "Inteligência disponível"
    } else {
        "Inteligência ainda não disponível"
    };

    view! {
        <section
            class="oc-card oc-ai-panel"
        >
            <span
                aria-hidden="true"
                class="oc-ai-panel__ring"
            ></span>

            <div class="oc-t-group oc-t-group--gold" >
                "OCINYE AI"
            </div>
            <h2>
                {title}
            </h2>
            <p>
                {message}
            </p>
            <div class="oc-row oc-gap-5" >
                {button(Button::new("Abrir Prompt", Variant::Gold).href("/ai/prompt"))}
                {button(Button::new("Hub de IA", Variant::OnNavy).href("/ai"))}
            </div>
        </section>
    }
}

fn quick_access(can_create_idea: bool) -> impl IntoView {
    // Só as acções cujo ecrã existe navegam; o dossier lista quatro, e dois
    // dos ecrãs ainda não foram especificados.
    //
    // A razão viaja com a acção. Dizer «ainda não disponível» a quem apenas não
    // tem a permissão seria falso — o ecrã existe, e é o acesso que falta.
    const POR_ESPECIFICAR: &str = "Ainda não disponível";
    const SEM_PERMISSAO: &str = "Não tem autorização para criar ideias.";

    let actions: [(&str, Option<&str>, &str); 4] = [
        (
            "Nova Ideia",
            can_create_idea.then_some("/ideas/new"),
            SEM_PERMISSAO,
        ),
        ("Novo Projecto", None, POR_ESPECIFICAR),
        ("Novo Dataset", None, POR_ESPECIFICAR),
        ("Prompt IA", Some("/ai/prompt"), POR_ESPECIFICAR),
    ];

    card(
        section_head("Acesso rápido", None, None),
        view! {
            <div class="oc-grid oc-grid--2 oc-grid--tight" >
                {actions
                    .iter()
                    .map(|(label, href, reason)| {
                        href.map_or_else(
                            || {
                                view! {
                                    <span
                                        class="oc-quick oc-unavailable"
                                        aria-disabled="true"
                                        title=*reason
                                    >
                                        <span class="oc-btn__dot"></span>
                                        {*label}
                                    </span>
                                }
                                    .into_any()
                            },
                            |href| {
                                view! {
                                    <a
                                        class="oc-quick"
                                        href=href
                                    >
                                        <span class="oc-btn__dot"></span>
                                        {*label}
                                    </a>
                                }
                                    .into_any()
                            },
                        )
                    })
                    .collect_view()}
            </div>
        },
    )
}

/// A saudação correspondente à hora local.
#[must_use]
pub fn greeting_for(hour: u32) -> &'static str {
    match hour {
        5..=12 => "Bom dia",
        13..=19 => "Boa tarde",
        _ => "Boa noite",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_saudacao_segue_a_hora() {
        assert_eq!(greeting_for(9), "Bom dia");
        assert_eq!(greeting_for(15), "Boa tarde");
        assert_eq!(greeting_for(23), "Boa noite");
        assert_eq!(greeting_for(3), "Boa noite");
    }

    #[test]
    fn o_subtitulo_concorda_em_numero() {
        assert_eq!(summary(1, 0), "Tem 1 tarefa atribuída.");
        assert_eq!(summary(6, 0), "Tem 6 tarefas atribuídas.");
        assert!(summary(0, 0).contains("Nada precisa da sua atenção"));
    }

    fn painel(can_create_idea: bool) -> Dashboard {
        Dashboard {
            greeting: "Boa noite".to_owned(),
            name: "Fidel Monteiro".to_owned(),
            kpis: Vec::new(),
            workspaces: json!({"items": []}),
            tasks: json!({"items": []}),
            activity: json!([]),
            intelligence: json!({"configured": false}),
            can_create_idea,
        }
    }

    /// O Home não oferece o que o Core vai recusar.
    ///
    /// A topbar já escondia «+ Criar» a quem não tem a permissão, mas o Home
    /// mostrava «Nova Ideia» a toda a gente — no cabeçalho e no acesso rápido.
    /// Um `platform_admin` sem filiação numa unidade via os dois activos,
    /// carregava, e era recusado. Admin não é root, e um botão para uma recusa
    /// é pior do que não haver botão (briefing §52).
    #[test]
    fn o_home_nao_oferece_criar_ideia_a_quem_nao_pode() {
        let html = home(painel(false)).to_html();
        assert!(
            !html.contains(r#"href="/ideas/new""#),
            "o Home levou a criar uma ideia sem a permissão que isso exige"
        );
        // Continua listada, mas dizendo a verdade sobre porquê.
        assert!(html.contains("Não tem autorização para criar ideias."));
        assert!(html.contains("Nova Ideia"));
    }

    /// Com a permissão, os dois caminhos voltam.
    #[test]
    fn com_a_permissao_o_home_leva_ao_formulario() {
        let html = home(painel(true)).to_html();
        assert_eq!(html.matches(r#"href="/ideas/new""#).count(), 2);
    }

    /// A razão de cada acção indisponível é a sua, e não a do vizinho.
    ///
    /// «Ainda não disponível» é verdade para «Novo Projecto», cujo ecrã não
    /// existe, e mentira para quem apenas não tem acesso.
    #[test]
    fn cada_accao_indisponivel_diz_a_sua_propria_razao() {
        let html = home(painel(false)).to_html();
        assert!(html.contains("Ainda não disponível"));
        assert!(html.contains("Não tem autorização para criar ideias."));
    }
}

#[cfg(test)]
mod integridade {
    use super::*;
    use serde_json::json;

    fn painel(kpis: Vec<crate::ui::components::Kpi>) -> Dashboard {
        Dashboard {
            greeting: "Bom dia".to_owned(),
            name: "Fidel".to_owned(),
            kpis,
            workspaces: json!({"items": []}),
            tasks: json!({"items": []}),
            activity: json!([]),
            intelligence: json!({"configured": false}),
            can_create_idea: true,
        }
    }

    fn indicador(label: &str, value: Option<&str>) -> crate::ui::components::Kpi {
        crate::ui::components::Kpi {
            label: label.to_owned(),
            value: value.map(ToOwned::to_owned),
            delta: None,
            hint: "activas".to_owned(),
            href: "/units".to_owned(),
        }
    }

    /// Uma falha do Core não se apresenta como zero.
    ///
    /// São três estados, e a Home tem de os separar:
    ///
    /// | | |
    /// |---|---|
    /// | `N` | a consulta correu e encontrou N |
    /// | `0` | a consulta correu e não encontrou nada |
    /// | `—` | a consulta **não correu** |
    ///
    /// Um `0` numa falha é a mentira mais fácil de contar: parece um sistema
    /// vazio, e um sistema vazio parece funcionar. O cartão também não
    /// desaparece — sumir não informa ninguém de que algo falhou.
    #[test]
    fn uma_falha_do_core_nao_vira_zero() {
        let html = home(painel(vec![indicador("UNIDADES", None)])).to_html();

        assert!(
            html.contains("UNIDADES"),
            "o cartão desapareceu em vez de se declarar"
        );
        assert!(html.contains("indisponível"), "a falha não foi declarada");
        assert!(
            !html.contains(">0<"),
            "uma contagem que falhou foi apresentada como zero"
        );
    }

    /// Zero continua a ser zero quando a consulta correu.
    #[test]
    fn zero_e_zero_quando_a_consulta_correu() {
        let html = home(painel(vec![indicador("UNIDADES", Some("0"))])).to_html();
        assert!(html.contains("UNIDADES"));
        assert!(
            html.contains(">0<"),
            "um zero verdadeiro deixou de aparecer"
        );
        assert!(
            !html.contains("indisponível"),
            "um zero verdadeiro foi marcado como indisponível"
        );
    }

    /// Ideias e Projectos são contadores distintos.
    ///
    /// Os dois consumiam `/workspaces` sem filtro e mostravam sempre o mesmo
    /// total. Com números diferentes, uma regressão que volte a partilhar a
    /// consulta torna-se visível de imediato.
    #[test]
    fn ideias_e_projectos_sao_contadores_distintos() {
        let html = home(painel(vec![
            indicador("IDEIAS", Some("2")),
            indicador("PROJECTOS", Some("1")),
        ]))
        .to_html();

        assert!(html.contains("IDEIAS") && html.contains("PROJECTOS"));
        assert!(html.contains(">2<"), "a contagem de ideias não apareceu");
        assert!(html.contains(">1<"), "a contagem de projectos não apareceu");
    }
}
