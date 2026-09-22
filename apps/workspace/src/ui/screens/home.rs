//! Home / Dashboard.
//!
//! Responde a uma pergunta: **o que precisa da minha atenção?**
//! Sem vanity metrics (`design/README.md` §6.2).

use leptos::prelude::*;
use serde_json::Value;

use crate::ui::components::{button, card, kpi_card, pill, section_head, Button, Kpi, Variant};

/// Tudo o que o painel mostra, já autorizado pelo Core.
pub struct Dashboard {
    /// A chave i18n da saudação, dependente da hora (`home.greeting.*`).
    ///
    /// Uma chave, e não a frase já feita: a saudação é a mesma verdade — a hora —
    /// dita na língua de quem olha, e resolve-se aqui, no idioma corrente.
    pub greeting_key: &'static str,
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
        greeting_key,
        name,
        kpis,
        workspaces,
        tasks,
        activity,
        intelligence,
        can_create_idea,
    } = data;
    use crate::i18n::{t, tf};
    let saudacao = tf(greeting_key, &[("name", &name)]);

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
                    <h1 class="oc-head--lg">{saudacao}</h1>
                    <p>{subtitle}</p>
                </div>
                <div class="oc-head__actions">
                    // Visível sempre, e declarada quando não se pode usar.
                    {button(if can_create_idea {
                        Button::new(t("home.new_idea"), Variant::Secondary).href("/ideas/new")
                    } else {
                        Button::new(t("home.new_idea"), Variant::Secondary)
                            .unavailable_because(t("home.no_permission.idea"))
                    })}
                    // O projecto cria-se em contexto (promove-se uma ideia); o
                    // botão leva à lista de projectos, em vez de se declarar
                    // «indisponível» quando existe (F-07).
                    {button(Button::new(t("home.new_project"), Variant::Secondary).href("/projects"))}
                    {button(
                        Button::new(t("home.prompt_ocinye"), Variant::Primary).href("/ai/prompt").with_dot(),
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

/// O subtítulo do painel, construído a partir do que existe, no idioma corrente.
///
/// Duas cláusulas independentes (tarefas, investigação), cada uma com o seu
/// plural, juntas por «e». Contar por partes traduz-se bem nas três línguas; uma
/// frase única com todas as combinações não (i18n §49).
fn summary(tasks: usize, ideas: usize) -> String {
    use crate::i18n::{t, tp};
    let tarefas = i64::try_from(tasks).unwrap_or(i64::MAX);
    let investigacao = i64::try_from(ideas).unwrap_or(i64::MAX);
    match (tasks, ideas) {
        (0, 0) => t("home.summary.empty").to_owned(),
        (_, 0) => format!(
            "{}{}",
            tp("home.summary.tasks", tarefas),
            t("home.summary.suffix")
        ),
        (0, _) => format!(
            "{}{}",
            tp("home.summary.research", investigacao),
            t("home.summary.suffix")
        ),
        (_, _) => format!(
            "{} {} {}{}",
            tp("home.summary.tasks", tarefas),
            t("home.summary.join"),
            tp("home.summary.research", investigacao),
            t("home.summary.suffix")
        ),
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
                <p class="oc-muted">{crate::i18n::t("home.continue.empty")}</p>
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
                                <div class="oc-fill oc-t-item" data-oc-content="1">
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
                crate::i18n::t("home.continue.title"),
                Some((crate::i18n::t("home.view_all").into(), "/my-work".into())),
                Some(crate::i18n::t("home.continue.aside").to_owned()),
            )}
            {body}
        </section>
    }
}

fn pending_tasks(payload: &Value) -> impl IntoView {
    let rows = items(payload);

    let body = if rows.is_empty() {
        view! { <p class="oc-muted">{crate::i18n::t("home.tasks.empty")}</p> }.into_any()
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
                                <span class="oc-fill oc-truncate oc-t-cell" data-oc-content="1">
                                    {text(row, "title")}
                                </span>
                                {crate::ui::components::task_state_badge(&state)}
                                <span class="oc-mono oc-list__meta" >
                                    {due.map_or_else(
                                        || crate::i18n::t("home.tasks.no_due").to_owned(),
                                        ToOwned::to_owned,
                                    )}
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
            crate::i18n::t("home.tasks.title"),
            Some((crate::i18n::t("home.view_all").into(), "/my-work".into())),
            None,
        ),
        body,
    )
}

fn recent_activity(payload: &Value) -> impl IntoView {
    let rows = items(payload);

    let body = if rows.is_empty() {
        view! { <p class="oc-muted">{crate::i18n::t("home.activity.empty")}</p> }.into_any()
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
                                    // A frase da actividade vem do Core (verbo +
                                    // título); marca-se como conteúdo até o feed
                                    // passar a evento semântico (i18n §11, §44).
                                    <div class="oc-t-note" data-oc-content="1">
                                        {text(row, "summary")}
                                    </div>
                                    <div class="oc-mono oc-t-ghost" data-oc-content="1">
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
            {section_head(
                crate::i18n::t("home.activity.title"),
                Some((crate::i18n::t("home.view_all").into(), "/activity".into())),
                None,
            )}
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
    // A mensagem do Core é prosa e vem já composta; até passar a código de razão
    // (i18n §31, §51), o que se localiza é o texto por omissão, e a mensagem do
    // Core marca-se como conteúdo. O título é nosso, e traduz-se.
    let mensagem_do_core = status
        .get("message")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned);
    let e_conteudo = mensagem_do_core.is_some().then_some("1");
    let mensagem =
        mensagem_do_core.unwrap_or_else(|| crate::i18n::t("home.ai.default_message").to_owned());

    let title = if available {
        crate::i18n::t("home.ai.available")
    } else {
        crate::i18n::t("home.ai.unavailable")
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
                {crate::i18n::t("home.ai.eyebrow")}
            </div>
            <h2>
                {title}
            </h2>
            <p data-oc-content=e_conteudo>
                {mensagem}
            </p>
            <div class="oc-row oc-gap-5" >
                {button(Button::new(crate::i18n::t("home.ai.open_prompt"), Variant::Gold).href("/ai/prompt"))}
                {button(Button::new(crate::i18n::t("home.ai.hub"), Variant::OnNavy).href("/ai"))}
            </div>
        </section>
    }
}

fn quick_access(can_create_idea: bool) -> impl IntoView {
    // Cada acção do acesso rápido leva ao ecrã onde se cria — o projecto e o
    // dataset criam-se em contexto (a lista é a porta), e por isso navegam para
    // lá em vez de se declararem «indisponíveis», que dizia que não existiam
    // quando existem (F-07). Só a falta de **permissão** desactiva um item, e aí
    // a razão é essa, não a do vizinho.
    let sem_permissao = crate::i18n::t("home.no_permission.idea");

    let actions: [(&str, Option<&str>, &str); 4] = [
        (
            crate::i18n::t("home.new_idea"),
            can_create_idea.then_some("/ideas/new"),
            sem_permissao,
        ),
        (crate::i18n::t("home.new_project"), Some("/projects"), ""),
        (crate::i18n::t("home.new_dataset"), Some("/datasets"), ""),
        (crate::i18n::t("home.quick.prompt"), Some("/ai/prompt"), ""),
    ];

    card(
        section_head(crate::i18n::t("home.quick.title"), None, None),
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

/// A chave i18n da saudação correspondente à hora local.
///
/// Devolve a chave (`home.greeting.*`), não a frase: a frase resolve-se no idioma
/// corrente, com o nome interpolado.
#[must_use]
pub fn greeting_for(hour: u32) -> &'static str {
    match hour {
        5..=12 => "home.greeting.morning",
        13..=19 => "home.greeting.afternoon",
        _ => "home.greeting.evening",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_saudacao_segue_a_hora() {
        assert_eq!(greeting_for(9), "home.greeting.morning");
        assert_eq!(greeting_for(15), "home.greeting.afternoon");
        assert_eq!(greeting_for(23), "home.greeting.evening");
        assert_eq!(greeting_for(3), "home.greeting.evening");
    }

    #[test]
    fn o_subtitulo_concorda_em_numero() {
        assert_eq!(summary(1, 0), "Tem 1 tarefa atribuída.");
        assert_eq!(summary(6, 0), "Tem 6 tarefas atribuídas.");
        assert!(summary(0, 0).contains("Nada precisa da sua atenção"));
    }

    fn painel(can_create_idea: bool) -> Dashboard {
        Dashboard {
            greeting_key: "home.greeting.evening",
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

    /// Pureza de idioma: uma língua activa, um só idioma no chrome (i18n §2, §41).
    ///
    /// Rende o Home em francês e exige que o chrome seja francês por inteiro —
    /// não a barra em francês e o corpo em português, que é o defeito que motivou
    /// esta pass. Prova por marcas: as frases francesas têm de aparecer, e
    /// nenhuma marca portuguesa de chrome pode ficar.
    #[tokio::test]
    async fn o_home_nao_mistura_linguas() {
        use crate::i18n::{with_locale, Locale};

        let fr = with_locale(Locale::Fr, async { home(painel(true)).to_html() }).await;
        for francesa in [
            "Continuer le travail",
            "Tâches en attente",
            "Activité récente",
            "Accès rapide",
            "Bonsoir, Fidel Monteiro",
            "Nouveau projet",
        ] {
            assert!(fr.contains(francesa), "fr: falta o chrome «{francesa}»");
        }
        for portuguesa in [
            "Continuar trabalho",
            "Tarefas pendentes",
            "Actividade recente",
            "Acesso rápido",
            "Boa noite",
            "Novo Projecto",
        ] {
            assert!(
                !fr.contains(portuguesa),
                "fr: chrome português por traduzir «{portuguesa}»"
            );
        }

        // E o inglês, pela mesma medida.
        let en = with_locale(Locale::En, async { home(painel(true)).to_html() }).await;
        assert!(en.contains("Continue work") && en.contains("Pending tasks"));
        assert!(!en.contains("Continuar trabalho"));
    }

    /// Com a permissão, os dois caminhos voltam.
    #[test]
    fn com_a_permissao_o_home_leva_ao_formulario() {
        let html = home(painel(true)).to_html();
        assert_eq!(html.matches(r#"href="/ideas/new""#).count(), 2);
    }

    /// Uma acção implementada leva ao seu ecrã; só a falta de permissão a
    /// desactiva, e aí com a sua própria razão — não «indisponível».
    ///
    /// «Novo Projecto» e «Novo Dataset» criam-se em contexto e navegam para a
    /// lista respectiva; «Ainda não disponível» dizia que não existiam (F-07).
    #[test]
    fn cada_accao_indisponivel_diz_a_sua_propria_razao() {
        // Sem permissão para ideias: a Ideia desactiva-se com a sua razão.
        let html = home(painel(false)).to_html();
        assert!(html.contains("Não tem autorização para criar ideias."));
        // Mas as acções implementadas continuam a levar ao seu ecrã, nunca
        // declaradas «indisponíveis».
        assert!(!html.contains("Ainda não disponível"));
        assert!(html.contains(r#"href="/projects""#));
        assert!(html.contains(r#"href="/datasets""#));
    }
}

#[cfg(test)]
mod integridade {
    use super::*;
    use serde_json::json;

    fn painel(kpis: Vec<crate::ui::components::Kpi>) -> Dashboard {
        Dashboard {
            greeting_key: "home.greeting.morning",
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
