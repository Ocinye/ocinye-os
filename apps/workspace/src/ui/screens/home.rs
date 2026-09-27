//! Home / Dashboard.
//!
//! Responde a uma pergunta: **o que precisa da minha atenção?**
//! Sem vanity metrics (`design/README.md` §6.2).

use leptos::prelude::*;
use serde_json::Value;

use crate::ui::components::Kpi;
use crate::ui::ods;

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
    /// O perfil da Instância, para o Desktop saber que predefinição é a sua.
    pub perfil: Option<String>,
    /// A agenda que a barra de topo já leu, para o widget do calendário.
    pub agenda: Vec<crate::ui::screens::calendar::Item>,
    /// Se a leitura da agenda falhou: falha não é agenda vazia.
    pub agenda_falhou: bool,
    /// O fuso de quem vê, para as horas da agenda.
    pub zona: ocinye_contracts::temporal::TimeZoneName,
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
        perfil,
        agenda,
        agenda_falhou,
        zona,
        ..
    } = data;

    // A disposição é fixa, servida pelo servidor (D4): enquanto não houver
    // Desktop persistente (G-02), personalizar, minimizar e mover estão
    // desligados, e nada se guarda no browser.
    view! {
        <section class="ods-desktop" data-oc="desktop" data-profile=perfil.unwrap_or_default()>
            <button
                type="button"
                class="ods-btn ods-btn--sm ods-desktop__edit-btn"
                data-oc="desktop-edit"
                aria-disabled="true"
                data-tip=crate::i18n::t("ods.state.pending_contract")
            >
                {crate::i18n::t("desktop.customize")}
            </button>
            <div class="ods-desktop__scroll" data-ods-scroll>
                <div class="ods-desktop__grid" data-oc="widget-grid">
                    {indicadores(kpis)}
                    {widget_agenda(&agenda, agenda_falhou, zona)}
                    {widget_continuar(&workspaces)}
                    {widget_tarefas(&tasks)}
                    {widget_actividade(&activity)}
                </div>
            </div>
            {nye(greeting_key, &name)}
        </section>
    }
}

/// O Nye (D7): o botão flutuante do Desktop e o popup circular.
///
/// O Nye funciona sem fornecedor de IA: o que responde é o plano do Core em
/// `/ask`. Sem JavaScript o formulário navega para lá; com ele, a resposta
/// aparece no círculo. Arrasta-se durante a sessão, e a posição não se guarda
/// até haver Desktop persistente (G-02). A voz espera pelo G-07.
fn nye(greeting_key: &'static str, nome: &str) -> impl IntoView {
    let saudacao = match greeting_key {
        "home.greeting.morning" => "nye.hello.morning",
        "home.greeting.afternoon" => "nye.hello.afternoon",
        _ => "nye.hello.evening",
    };
    let primeiro = nome.split_whitespace().next().unwrap_or(nome).to_owned();
    let ola = crate::i18n::tf(saudacao, &[("name", &primeiro)]);
    let nucleo = || {
        view! {
            <span class="ods-nye-btn__glow"></span>
            <span class="ods-nye-btn__ring"></span>
            <span class="ods-nye-btn__track"></span>
            <span class="ods-nye-btn__arc"></span>
            <span class="ods-nye-btn__core">{ods::icone("nye", "")}</span>
        }
    };

    view! {
        <div class="ods-nye-float" data-oc="nye-float" data-ods-float>
            <button
                type="button"
                class="ods-nye-btn"
                data-oc="nye-open"
                aria-haspopup="dialog"
                aria-label=format!("{} · {}", crate::i18n::t("nye.talk"), crate::i18n::t("nye.drag_hint"))
            >
                {nucleo()}
            </button>
        </div>

        <div class="ods-nye-orb" data-oc="nye-orb" role="dialog" aria-modal="true" aria-label=crate::i18n::t("nye.name") hidden>
            <div class="ods-scrim" data-oc="nye-close"></div>
            <div class="ods-nye-orb__circle">
                <span class="ods-nye-orb__ring ods-nye-orb__ring--track"></span>
                <span class="ods-nye-orb__ring ods-nye-orb__ring--arc"></span>
                <span class="ods-nye-orb__ring ods-nye-orb__ring--outer"></span>
                <button
                    type="button"
                    class="ods-iconbtn ods-iconbtn--round ods-nye-orb__close"
                    data-oc="nye-close"
                    aria-label=crate::i18n::t("ods.close")
                >
                    {ods::icone("close", "")}
                </button>
                <span class="ods-nye-btn" aria-hidden="true">{nucleo()}</span>

                <div data-state="idle">
                    <p class="ods-nye-orb__hello">{ola}</p>
                    <p class="ods-nye-orb__sub">{crate::i18n::t("nye.how_help")}</p>
                </div>
                <div data-state="listening" hidden>
                    <div class="ods-nye-wave"><span></span><span></span><span></span><span></span><span></span></div>
                    <p class="ods-label">{crate::i18n::t("nye.voice.listening")}</p>
                </div>
                <div data-state="answer" hidden aria-live="polite">
                    <p class="ods-nye-orb__q" data-oc="nye-q"></p>
                    <p class="ods-nye-orb__a" data-oc="nye-a"></p>
                    <a class="ods-btn ods-btn--sm ods-btn--ghost" data-oc="nye-full" href="/ask">
                        {crate::i18n::t("nye.full_conversation")}
                    </a>
                </div>

                <form class="ods-nye-orb__field" method="get" action="/ask" data-oc="nye-form" data-erro=crate::i18n::t("ods.state.error")>
                    <label class="ods-sr-only" for="nye-q">{crate::i18n::t("nye.placeholder")}</label>
                    <input id="nye-q" name="q" placeholder=crate::i18n::t("nye.placeholder") autocomplete="off" />
                    // G-07: a voz espera por contrato; nunca escuta sem clique.
                    <button
                        type="button"
                        class="ods-nye-orb__mic"
                        data-oc="nye-voice"
                        aria-disabled="true"
                        data-tip=crate::i18n::t("ods.state.pending_contract")
                        aria-label=crate::i18n::t("nye.talk")
                    >
                        {ods::icone("mic", "")}
                    </button>
                    <button type="submit" class="ods-nye-orb__send" aria-label=crate::i18n::t("nye.send")>
                        {ods::icone("arrow-r", "")}
                    </button>
                </form>
                <p class="ods-nye-orb__hint">"ENTER · ESC"</p>
            </div>
        </div>
    }
}

/// O ícone e a chave da etiqueta de cada indicador, pelo destino.
fn indicador_de(href: &str) -> (&'static str, &'static str, &'static str) {
    match href {
        "/units" => ("units", "desktop.kpi.units", "units"),
        "/ideas" => ("idea", "desktop.kpi.ideas", "ideas"),
        "/projects" => ("project", "desktop.kpi.projects", "projects"),
        _ => ("data", "desktop.kpi.datasets", "datasets"),
    }
}

/// Os indicadores (G-03): contagens das listagens, feitas no servidor. Uma
/// listagem que falhou mostra `—` e diz porquê — nunca `0`.
fn indicadores(kpis: Vec<Kpi>) -> impl IntoView {
    view! {
        <article class="ods-widget" data-kind="kpis" data-w="full" data-oc="widget" data-widget="kpis">
            <div class="ods-widget__body ods-kpis">
                {kpis
                    .into_iter()
                    .map(|kpi| {
                        let (icone, etiqueta, chave) = indicador_de(&kpi.href);
                        let falhou = kpi.value.is_none();
                        view! {
                            <a class="ods-kpi ods-widget-surface" href=kpi.href.clone() data-oc="kpi" data-kpi=chave>
                                <span class="ods-kpi__head">
                                    <span class="ods-widget__icon">{ods::icone(icone, "ods-icon--sm")}</span>
                                    {kpi.label.clone()}
                                    {ods::icone("arrow-r", "ods-icon--sm")}
                                </span>
                                <span>
                                    <span class="ods-kpi__value">{kpi.value.clone().unwrap_or_else(|| "—".to_owned())}</span>
                                    " "
                                    <span class="ods-kpi__label">{crate::i18n::t(etiqueta)}</span>
                                </span>
                                {falhou.then(|| ods::estado(ods::Estado::Erro, crate::i18n::t("ods.state.error").to_owned()))}
                            </a>
                        }
                    })
                    .collect_view()}
            </div>
        </article>
    }
}

/// A cabeça comum de um widget: ícone, título, e abrir a aplicação. Minimizar
/// espera pelo G-02.
fn cabeca(icone: &'static str, titulo: &'static str, destino: &'static str) -> impl IntoView {
    view! {
        <header class="ods-widget__head">
            <span class="ods-widget__icon">{ods::icone(icone, "")}</span>
            <span class="ods-widget__titles"><span class="ods-widget__title">{titulo}</span></span>
            <button
                type="button"
                class="ods-iconbtn"
                data-oc="widget-minimize"
                aria-pressed="false"
                aria-label=crate::i18n::t("desktop.widget.minimize")
                aria-disabled="true"
                data-tip=crate::i18n::t("ods.state.pending_contract")
            >
                {ods::icone("close", "ods-icon--sm")}
            </button>
            <a class="ods-iconbtn" href=destino aria-label=crate::i18n::t("desktop.widget.open")>
                {ods::icone("arrow-r", "")}
            </a>
        </header>
    }
}

/// Um corpo de widget: erro se o Core falhou, vazio se não há nada, e as linhas.
fn corpo(
    payload: &Value,
    vazio: &'static str,
    linhas: impl FnOnce(Vec<Value>) -> AnyView,
) -> AnyView {
    if payload.is_null() {
        return ods::estado(
            ods::Estado::Erro,
            crate::i18n::t("ods.state.error").to_owned(),
        )
        .into_any();
    }
    let rows = items(payload);
    if rows.is_empty() {
        return view! { <p class="ods-empty__body">{crate::i18n::t(vazio)}</p> }.into_any();
    }
    linhas(rows)
}

fn widget_agenda(
    agenda: &[crate::ui::screens::calendar::Item],
    falhou: bool,
    zona: ocinye_contracts::temporal::TimeZoneName,
) -> impl IntoView {
    let conteudo = if falhou {
        ods::estado(
            ods::Estado::Erro,
            crate::i18n::t("ods.state.error").to_owned(),
        )
        .into_any()
    } else if agenda.is_empty() {
        view! { <p class="ods-empty__body">{crate::i18n::t("desktop.widget.calendar.empty")}</p> }
            .into_any()
    } else {
        view! {
            <div class="ods-widget__list">
                {agenda
                    .iter()
                    .take(7)
                    .map(|item| view! {
                        <a class="ods-widget__row ods-widget__cal-row" href=item.href()>
                            <span class="ods-widget__row-meta">{item.clock(zona).unwrap_or_default()}</span>
                            <span class="ods-widget__row-main" data-oc-content="1">{item.title.clone()}</span>
                        </a>
                    })
                    .collect_view()}
            </div>
        }
        .into_any()
    };
    view! {
        <article class="ods-widget ods-widget-surface" data-kind="calendar" data-w="1" data-h="2" data-oc="widget" data-widget="calendar">
            {cabeca("calendar", crate::i18n::t("nav.calendar"), "/calendar")}
            <div class="ods-widget__body">{conteudo}</div>
        </article>
    }
}

fn widget_continuar(payload: &Value) -> impl IntoView {
    let conteudo = corpo(payload, "home.continue.empty", |rows| {
        view! {
            <div class="ods-widget__list">
                {rows
                    .iter()
                    .take(5)
                    .map(|row| {
                        let id = text(row, "id");
                        view! {
                            <a class="ods-widget__row" href=format!("/workspaces/{id}")>
                                <span class="ods-widget__row-main" data-oc-content="1">{text(row, "title")}</span>
                                <span class="ods-widget__row-meta">{text(row, "code")}</span>
                            </a>
                        }
                    })
                    .collect_view()}
            </div>
        }
        .into_any()
    });
    view! {
        <article class="ods-widget ods-widget-surface" data-kind="continue" data-w="2" data-h="1" data-oc="widget" data-widget="continue">
            {cabeca("work", crate::i18n::t("desktop.widget.continue"), "/my-work")}
            <div class="ods-widget__body">{conteudo}</div>
        </article>
    }
}

fn widget_tarefas(payload: &Value) -> impl IntoView {
    let conteudo = corpo(payload, "desktop.widget.tasks.empty", |rows| {
        view! {
            <div class="ods-widget__list">
                {rows
                    .iter()
                    .take(8)
                    .map(|row| {
                        let workspace = text(row, "workspace_id");
                        let due = row.get("due_on").and_then(Value::as_str).unwrap_or("").to_owned();
                        view! {
                            <a class="ods-widget__row" href=format!("/workspaces/{workspace}")>
                                <span class="ods-widget__row-main" data-oc-content="1">{text(row, "title")}</span>
                                <span class="ods-widget__row-meta">{due}</span>
                            </a>
                        }
                    })
                    .collect_view()}
            </div>
        }
        .into_any()
    });
    view! {
        <article class="ods-widget ods-widget-surface" data-kind="tasks" data-w="1" data-h="2" data-oc="widget" data-widget="tasks">
            {cabeca("tasks", crate::i18n::t("home.tasks.title"), "/my-work")}
            <div class="ods-widget__body">{conteudo}</div>
        </article>
    }
}

fn widget_actividade(payload: &Value) -> impl IntoView {
    let conteudo = corpo(payload, "home.activity.empty", |rows| {
        view! {
            <div class="ods-widget__list">
                {rows
                    .iter()
                    .take(8)
                    .map(|row| view! {
                        <div class="ods-widget__row">
                            <span class="ods-widget__row-main" data-oc-content="1">{text(row, "summary")}</span>
                            <span class="ods-widget__row-meta" data-oc-content="1">{text(row, "actor_name")}</span>
                        </div>
                    })
                    .collect_view()}
            </div>
        }
        .into_any()
    });
    view! {
        <article class="ods-widget ods-widget-surface" data-kind="activity" data-w="2" data-h="1" data-oc="widget" data-widget="activity">
            {cabeca("activity", crate::i18n::t("home.activity.title"), "/activity")}
            <div class="ods-widget__body">{conteudo}</div>
        </article>
    }
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
            perfil: None,
            agenda: Vec::new(),
            agenda_falhou: false,
            zona: ocinye_contracts::temporal::TimeZoneName::utc(),
        }
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
            "Reprendre le travail",
            "Tâches en attente",
            "Activité récente",
            "Calendrier",
            "Personnaliser le bureau",
        ] {
            assert!(fr.contains(francesa), "fr: falta o chrome «{francesa}»");
        }
        for portuguesa in [
            "Continuar trabalho",
            "Tarefas pendentes",
            "Actividade recente",
            "Personalizar Desktop",
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

        // O card de IA deriva o corpo do estado, não da prosa do Core. Mesmo
        // que o Core mande uma mensagem já composta em português — como manda em
        // produção —, o card mostra-a no idioma de quem lê. Este é o defeito
        // exacto que o ecrã tinha: título francês, corpo português.
        let com_prosa = Dashboard {
            intelligence: json!({
                "available": false,
                "message": "O Prompt Ocinye está operacional em português."
            }),
            ..painel(true)
        };
        let fr_ia = with_locale(Locale::Fr, async { home(com_prosa).to_html() }).await;
        // O D4 não tem cartão de IA; fica a garantia de que a prosa do Core
        // em português não chega a um ecrã francês.
        assert!(
            !fr_ia.contains("está operacional"),
            "fr: a prosa do Core em português apareceu no card de IA"
        );
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
            perfil: None,
            agenda: Vec::new(),
            agenda_falhou: false,
            zona: ocinye_contracts::temporal::TimeZoneName::utc(),
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
        assert!(
            html.contains("ods-state--error"),
            "a falha não foi declarada"
        );
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
