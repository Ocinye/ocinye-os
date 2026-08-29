//! O Calendário e o Centro Temporal.
//!
//! # Uma verdade, quatro vistas
//!
//! Hoje, Semana, Mês e Agenda são **projecções** do mesmo conjunto autorizado.
//! Nenhuma delas consulta nada por si: recebem os itens que o Core devolveu para
//! o intervalo pedido e escolhem como os desenhar. A apresentação muda; a
//! autorização não (ADR-0410).
//!
//! # Porque um prazo não é um evento
//!
//! Um prazo de tarefa aparece aqui, e continua a ser uma tarefa. A distinção é
//! visível de propósito: oferecer «cancelar» sobre um prazo levaria a uma
//! operação que não existe — a tarefa altera-se pelo seu próprio módulo.

use chrono::{DateTime, Datelike, Duration, NaiveDate, Timelike, Utc};
use leptos::prelude::*;
use ocinye_contracts::temporal::TimeZoneName;
use serde_json::Value;

use crate::ui::components::classification_badge;

/// Qual das vistas está a ser mostrada.
///
/// # `Hoje` não está aqui, e é de propósito
///
/// Estava, e era uma confusão de categorias. As vistas dizem **como** se olha
/// para o tempo — um dia, uma semana, um mês. `Hoje` diz **onde**: é uma
/// navegação, como o «anterior» e o «seguinte», e aparecia entre as vistas ao
/// mesmo tempo que aparecia ao lado delas como acção. A mesma palavra, duas
/// vezes na mesma barra, a significar coisas diferentes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CalendarView {
    /// Um dia, em linha do tempo.
    Day,
    /// Sete dias, em grelha.
    Week,
    /// Um mês, em grelha de dias.
    Month,
    /// Doze meses, para orientação anual.
    Year,
    /// Uma lista cronológica.
    Agenda,
}

impl CalendarView {
    /// O valor estável no endereço.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Day => "day",
            Self::Week => "week",
            Self::Month => "month",
            Self::Year => "year",
            Self::Agenda => "agenda",
        }
    }

    /// Interpreta o valor do endereço.
    ///
    /// Um valor desconhecido cai no Mês, e não num erro: um endereço estragado à
    /// mão não é razão para negar o calendário a alguém.
    ///
    /// `today` continua a ser entendido. Era o nome desta vista antes de `Hoje`
    /// passar a ser navegação, e há endereços guardados e ligações antigas que o
    /// usam — recusá-los partia coisas que funcionavam, e por nada.
    #[must_use]
    pub fn parse(value: &str) -> Self {
        match value {
            "day" | "today" => Self::Day,
            "week" => Self::Week,
            "year" => Self::Year,
            "agenda" => Self::Agenda,
            _ => Self::Month,
        }
    }

    /// Como se diz a uma pessoa.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Day => "Dia",
            Self::Week => "Semana",
            Self::Month => "Mês",
            Self::Year => "Ano",
            Self::Agenda => "Agenda",
        }
    }

    /// Quantos dias esta vista abrange a partir da âncora.
    #[must_use]
    pub const fn span_days(self) -> i64 {
        match self {
            Self::Day => 1,
            Self::Week => 7,
            Self::Month => 42,
            Self::Year => 366,
            Self::Agenda => 90,
        }
    }

    /// Todas, pela ordem em que aparecem.
    #[must_use]
    pub const fn all() -> [Self; 5] {
        [Self::Day, Self::Week, Self::Month, Self::Year, Self::Agenda]
    }
}

// ── Item ────────────────────────────────────────────────────────────────

/// Uma linha da agenda, já legível.
///
/// # Porque isto existe em vez de se ler o JSON na vista
///
/// Porque quatro vistas a interpretar o mesmo JSON são quatro sítios onde
/// alguém pode ler um campo de maneira diferente. Interpreta-se uma vez.
#[derive(Debug, Clone)]
pub struct Item {
    /// Evento, prazo ou lembrete.
    pub kind: String,
    /// Identificador do recurso de origem.
    pub id: String,
    /// O que mostrar.
    pub title: String,
    /// Se ocupa o dia inteiro.
    pub all_day: bool,
    /// Instante inicial, quando tem hora.
    pub starts_at: Option<DateTime<Utc>>,
    /// Instante final, quando tem hora.
    pub ends_at: Option<DateTime<Utc>>,
    /// Zona da intenção.
    pub timezone: Option<String>,
    /// Primeiro dia, quando é de dia inteiro.
    pub starts_on: Option<NaiveDate>,
    /// Dia a seguir ao último, exclusivo.
    pub ends_before: Option<NaiveDate>,
    /// Estado do recurso de origem.
    pub state: String,
    /// Classificação.
    pub classification: String,
}

impl Item {
    fn from_json(row: &Value) -> Option<Self> {
        Some(Self {
            kind: row.get("kind")?.as_str()?.to_owned(),
            id: row.get("id")?.as_str()?.to_owned(),
            title: row.get("title")?.as_str().unwrap_or("—").to_owned(),
            all_day: row.get("all_day").and_then(Value::as_bool).unwrap_or(false),
            starts_at: row
                .get("starts_at")
                .and_then(Value::as_str)
                .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
                .map(|d| d.with_timezone(&Utc)),
            ends_at: row
                .get("ends_at")
                .and_then(Value::as_str)
                .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
                .map(|d| d.with_timezone(&Utc)),
            timezone: row
                .get("timezone")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned),
            starts_on: row
                .get("starts_on")
                .and_then(Value::as_str)
                .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok()),
            ends_before: row
                .get("ends_before")
                .and_then(Value::as_str)
                .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok()),
            state: row
                .get("state")
                .and_then(Value::as_str)
                .unwrap_or("scheduled")
                .to_owned(),
            classification: row
                .get("classification")
                .and_then(Value::as_str)
                .unwrap_or("INTERNAL")
                .to_owned(),
        })
    }

    /// Em que dia civil isto cai, para efeitos de agrupamento.
    ///
    /// # Porque a zona é obrigatória
    ///
    /// Porque não existe «o dia» de um instante — existe o dia **onde se está a
    /// olhar**. Isto lia `date_naive()`, que é a data em Greenwich, e o
    /// Calendário mostrava um compromisso das 00:30 em Lisboa no dia anterior.
    ///
    /// Um dia inteiro não passa por aqui: já é uma data civil, e converter uma
    /// data que não tem hora seria inventar-lhe uma.
    #[must_use]
    pub fn day(&self, zona: TimeZoneName) -> NaiveDate {
        self.starts_on
            .or_else(|| self.starts_at.map(|i| crate::ui::tempo::dia_civil(i, zona)))
            .unwrap_or_else(|| crate::ui::tempo::hoje_civil(Utc::now(), zona))
    }

    /// A hora, quando tem, na zona de quem olha.
    fn clock(&self, zona: TimeZoneName) -> Option<String> {
        self.starts_at.map(|i| {
            crate::ui::tempo::hora_civil(i, zona)
                .format("%H:%M")
                .to_string()
        })
    }

    /// O que dizer sobre quando isto acontece.
    ///
    /// Um evento de um dia inteiro diz «Dia inteiro», e não «24 → 25». O
    /// intervalo meio-aberto é a forma de o guardar sem erros de um dia; não é
    /// forma de o contar a alguém.
    #[must_use]
    pub fn when(&self, zona: TimeZoneName) -> String {
        if self.all_day {
            let dias = self
                .starts_on
                .zip(self.ends_before)
                .map(|(inicio, fim)| (fim - inicio).num_days())
                .unwrap_or(1);
            if dias <= 1 {
                "Dia inteiro".to_owned()
            } else {
                format!("Dia inteiro · {dias} dias")
            }
        } else {
            let inicio = self.clock(zona).unwrap_or_else(|| "—".to_owned());
            let fim = self
                .ends_at
                .map(|i| i.format("%H:%M").to_string())
                .unwrap_or_default();
            let intervalo = if fim.is_empty() {
                inicio
            } else {
                format!("{inicio} – {fim}")
            };
            // A zona da intenção aparece quando não é UTC: «14:00 · Europe/Paris»
            // diz uma coisa que «14:00» sozinho não diz, e é a diferença entre
            // aparecer à hora certa e aparecer uma hora antes.
            match self.timezone.as_deref() {
                Some(zona) if zona != "UTC" => format!("{intervalo} · {zona}"),
                _ => intervalo,
            }
        }
    }

    /// Como se chama o que isto é.
    #[must_use]
    pub const fn kind_label(&self) -> &'static str {
        match self.kind.as_bytes() {
            b"task_due" => "Prazo",
            b"reminder" => "Lembrete",
            _ => "Evento",
        }
    }

    /// Se foi cancelado.
    #[must_use]
    pub fn cancelled(&self) -> bool {
        self.state == "cancelled"
    }

    /// Para onde leva.
    ///
    /// Um prazo leva à tarefa, no seu próprio módulo. Não há aqui um ecrã de
    /// prazo, porque um prazo não é uma entidade — é uma data de outra coisa.
    #[must_use]
    pub fn href(&self) -> String {
        match self.kind.as_str() {
            "task_due" => "/my-work".to_owned(),
            _ => format!("/calendar/events/{}", self.id),
        }
    }
}

/// Lê os itens que o Core devolveu.
#[must_use]
pub fn items_from(payload: &Value) -> Vec<Item> {
    payload
        .get("items")
        .and_then(Value::as_array)
        .map(|linhas| linhas.iter().filter_map(Item::from_json).collect())
        .unwrap_or_default()
}

// ── A página ────────────────────────────────────────────────────────────

/// O que a página precisa de saber.
pub struct CalendarPage<'a> {
    /// Que vista mostrar.
    pub view: CalendarView,
    /// O dia à volta do qual a vista se organiza.
    pub anchor: NaiveDate,
    /// Os itens autorizados do intervalo.
    pub items: &'a [Item],
    /// Se o membro pode marcar.
    pub may_create: bool,
    /// A razão pela qual não há itens para mostrar, quando a consulta falhou.
    ///
    /// `Some` é erro. `None` com lista vazia é uma agenda vazia — e as duas
    /// coisas nunca se dizem da mesma maneira.
    pub failure: Option<String>,
    /// A zona em que se está a olhar.
    ///
    /// Decide em que dia civil cada compromisso cai e a que horas se mostra.
    /// Não tem valor por omissão aqui de propósito: um calendário renderizado
    /// em Greenwich a quem está noutro sítio mostra as coisas no dia errado, e
    /// era exactamente isso que acontecia.
    pub zona: TimeZoneName,
}

/// O Calendário.
pub fn calendar(page: &CalendarPage<'_>) -> impl IntoView {
    let CalendarPage {
        view,
        anchor,
        items,
        may_create,
        failure,
        zona,
    } = page;
    let view = *view;
    let anchor = *anchor;
    let may_create = *may_create;
    let zona = *zona;

    view! {
        <div class="oc-page oc-page--calendar">
            <div class="oc-head">
                <div class="oc-head__text">
                    <h1>"Calendário"</h1>
                    <p>
                        "Os compromissos, prazos e lembretes a que tem acesso. Os prazos
                         vêm das tarefas e continuam a pertencer-lhes."
                    </p>
                </div>
                {may_create.then(|| view! {
                    <a class="oc-btn oc-btn--primary" href="/calendar/events/new">
                        "+ Nova actividade"
                    </a>
                })}
            </div>

            {toolbar(view, anchor)}

            {match failure {
                // Um erro é um erro. Dizer «nenhuma actividade» quando a consulta
                // falhou faria alguém faltar a uma reunião por acreditar no ecrã.
                Some(motivo) => view! {
                    <div class="oc-alert oc-alert--error" role="alert">
                        <strong>"Não foi possível ler a agenda."</strong>
                        <span>{motivo.clone()}</span>
                    </div>
                }.into_any(),

                // Sem actividades, a vista desenha-se na mesma.
                //
                // # Porque o vazio deixou de ter uma vista própria
                //
                // Havia aqui um `items.is_empty()` que substituía a vista inteira
                // por uma frase. Um mês sem compromissos deixava de ter grelha:
                // as setas continuavam lá, o período também, e no meio uma linha
                // de texto a dizer que não havia nada — a estrutura do tempo
                // desaparecia com o conteúdo.
                //
                // Um mês vazio é um calendário limpo, não uma página sem
                // conteúdo. As semanas, os dias e o dia de hoje continuam a
                // existir quando não há nada marcado, e é precisamente aí que
                // alguém vai procurar onde marcar. A Agenda é a excepção, e
                // di-lo dentro de si própria: é uma lista, e uma lista vazia não
                // tem estrutura nenhuma para preservar.
                None => match view {
                    CalendarView::Day => today_view(items, anchor, zona).into_any(),
                    CalendarView::Week => week_view(items, anchor, zona).into_any(),
                    CalendarView::Month => month_view(items, anchor, zona).into_any(),
                    CalendarView::Year => year_view(items, anchor, zona).into_any(),
                    CalendarView::Agenda => agenda_view(items, zona).into_any(),
                },
            }}
        </div>
    }
}

/// A barra do Calendário.
///
/// # Uma linha, duas metades
///
/// Estavam em duas linhas e sem relação declarada: um separador de vistas em
/// cima, e por baixo as setas, o período e o `Hoje`. Ler aquilo obrigava a
/// perceber sozinho que uma linha muda **como** se olha e a outra muda **para
/// onde** — e a palavra `Hoje` aparecia nas duas a significar coisas
/// diferentes.
///
/// Agora é uma barra só, com a fronteira desenhada: à esquerda o tempo — recuar,
/// o período onde se está, avançar, e voltar a hoje — e à direita a forma de o
/// ver. A separação é a informação.
fn toolbar(current: CalendarView, anchor: NaiveDate) -> impl IntoView {
    use crate::ui::icon::{icon, Icon};

    let anterior = step(current, anchor, false);
    let seguinte = step(current, anchor, true);
    let texto = period_text(current, anchor);

    view! {
        <div class="oc-cal-bar">
            <div class="oc-cal-bar__tempo">
                <a
                    class="oc-cal-nav"
                    href=format!("/calendar?view={}&on={anterior}", current.as_str())
                    aria-label=format!("{}, período anterior", current.label())
                    rel="prev"
                >
                    // O mesmo galo do sistema, virado. Acrescentar duas setas ao
                    // sprite seria acrescentar iconografia para dizer o que a
                    // que já lá está diz, noutra direcção.
                    <span class="oc-cal-nav__glifo oc-cal-nav__glifo--anterior">
                        {icon(Icon::ChevronUp, 16)}
                    </span>
                </a>
                <h2 class="oc-cal-bar__periodo" aria-live="polite">{texto}</h2>
                <a
                    class="oc-cal-nav"
                    href=format!("/calendar?view={}&on={seguinte}", current.as_str())
                    aria-label=format!("{}, período seguinte", current.label())
                    rel="next"
                >
                    <span class="oc-cal-nav__glifo oc-cal-nav__glifo--seguinte">
                        {icon(Icon::ChevronUp, 16)}
                    </span>
                </a>
                <a
                    class="oc-cal-hoje"
                    href=format!("/calendar?view={}", current.as_str())
                >
                    "Hoje"
                </a>
            </div>

            <nav class="oc-cal-vistas" aria-label="Vistas do calendário">
                {CalendarView::all().into_iter().map(|vista| {
                    let activa = vista == current;
                    view! {
                        <a
                            class=if activa { "oc-cal-vista oc-cal-vista--activa" }
                                  else { "oc-cal-vista" }
                            href=format!("/calendar?view={}&on={}", vista.as_str(), anchor)
                            aria-current=activa.then_some("page")
                        >
                            {vista.label()}
                        </a>
                    }
                }).collect_view()}
            </nav>
        </div>
    }
}

/// O que a vista mostra, dito em português.
#[must_use]
pub fn period_text(view: CalendarView, anchor: NaiveDate) -> String {
    use crate::ui::tempo;
    match view {
        CalendarView::Day => tempo::dia_por_extenso(anchor),
        CalendarView::Week => {
            let inicio = week_start(anchor);
            tempo::intervalo_da_semana(inicio, inicio + Duration::days(6))
        }
        CalendarView::Month => tempo::mes_e_ano(anchor),
        CalendarView::Year => anchor.year().to_string(),
        CalendarView::Agenda => "Próximos 90 dias".to_owned(),
    }
}

/// Um passo para trás ou para a frente, na unidade da vista.
///
/// # Porque não é uma subtracção de dias
///
/// Era: `anchor - span_days()`. Como o Mês abrange 42 dias de grelha, carregar
/// em «anterior» num mês recuava seis semanas — passava um mês inteiro ao lado
/// e aterrava a meio do anterior. A Semana andava sete dias, que por acaso está
/// certo, e por isso ninguém reparou.
///
/// Cada vista anda na sua própria unidade. O Mês anda meses, e o `day(1)`
/// existe porque 31 de Janeiro menos um mês não tem resposta óbvia: ancorar no
/// dia 1 dá sempre o mês certo, que é a única coisa que a grelha precisa.
#[must_use]
pub fn step(view: CalendarView, anchor: NaiveDate, adiante: bool) -> NaiveDate {
    let sinal = if adiante { 1 } else { -1 };
    match view {
        CalendarView::Day => anchor + Duration::days(sinal),
        CalendarView::Week => anchor + Duration::days(7 * sinal),
        CalendarView::Agenda => anchor + Duration::days(CalendarView::Agenda.span_days() * sinal),
        CalendarView::Year => {
            NaiveDate::from_ymd_opt(anchor.year() + sinal as i32, 1, 1).unwrap_or(anchor)
        }
        CalendarView::Month => {
            let primeiro = anchor.with_day(1).unwrap_or(anchor);
            if adiante {
                let (ano, mes) = if primeiro.month() == 12 {
                    (primeiro.year() + 1, 1)
                } else {
                    (primeiro.year(), primeiro.month() + 1)
                };
                NaiveDate::from_ymd_opt(ano, mes, 1).unwrap_or(anchor)
            } else {
                let (ano, mes) = if primeiro.month() == 1 {
                    (primeiro.year() - 1, 12)
                } else {
                    (primeiro.year(), primeiro.month() - 1)
                };
                NaiveDate::from_ymd_opt(ano, mes, 1).unwrap_or(anchor)
            }
        }
    }
}

/// O primeiro dia da semana que contém esta data. Segunda-feira.
#[must_use]
pub fn week_start(day: NaiveDate) -> NaiveDate {
    let desde_segunda = day.weekday().num_days_from_monday();
    day - Duration::days(i64::from(desde_segunda))
}

/// O primeiro dia da grelha do mês: a segunda-feira da semana do dia 1.
#[must_use]
pub fn month_grid_start(day: NaiveDate) -> NaiveDate {
    let primeiro = day.with_day(1).unwrap_or(day);
    week_start(primeiro)
}

// ── As quatro vistas ────────────────────────────────────────────────────

/// Quantas actividades cabem numa célula do Mês antes de o resto se contar.
///
/// A constante desapareceu quando a `week_view` antiga foi substituída — vivia
/// entre as duas funções. Está aqui porque pertence ao Mês, e é aqui que quem
/// a procura vai olhar.
const MONTH_CELL_LIMIT: usize = 3;

// ── Geometria temporal ──────────────────────────────────────────────────
//
// A Semana e o Dia são a mesma coisa com larguras diferentes: um eixo de horas
// à esquerda, colunas de dias à direita, e cada actividade colocada onde
// acontece. O que se segue é essa aritmética, escrita uma vez.

/// A primeira e a última hora do eixo.
///
/// As vinte e quatro estão sempre lá — esconder a madrugada esconderia o turno
/// de quem trabalha nela. O que a vista faz é **abrir** perto do início do dia
/// útil, e isso é posição de deslocamento, não conteúdo em falta.
const HORAS: std::ops::Range<u32> = 0..24;

/// A altura de uma hora, em unidades da folha de estilo.
///
/// Vive aqui e no CSS como uma variável só (`--oc-cal-hora`), porque a posição
/// de um evento é calculada em percentagem do dia inteiro: a altura muda no
/// desenho sem que esta aritmética saiba dela.
const MINUTOS_DO_DIA: f64 = 24.0 * 60.0;

/// Quantas faixas de meia hora tem um dia.
const FAIXAS: usize = 48;

/// Quantas colunas de sobreposição a grelha desenha antes de as juntar.
///
/// Quatro chega para o que a instituição marca, e cada coluna a mais é uma
/// coluna mais estreita: a quinta actividade simultânea deixaria de ter título
/// legível. Passado o limite partilham a última coluna — continuam visíveis, e
/// quem precisa de as separar abre o dia.
const COLUNAS_MAX: usize = 4;

/// Onde uma actividade começa e quantas meias-horas ocupa.
///
/// # Porque meias-horas e não percentagem
///
/// Porque a percentagem só chega ao browser dentro de um atributo `style`, e a
/// `Content-Security-Policy` deste Workspace é `style-src 'self'` — sem
/// `unsafe-inline`. Um bloco posicionado assim seria descartado, e os eventos
/// do dia empilhavam-se todos no topo sem que nada o dissesse.
///
/// A grelha tem quarenta e oito linhas e cada bloco declara a sua por classe.
/// A granularidade é de apresentação: uma reunião às 08:07 desenha-se na linha
/// das 08:00, e a hora que se lê continua a ser a que o domínio deu.
///
/// # Porque devolve uma duração mínima
///
/// Um evento sem fim declarado, ou com fim igual ao início, não tem altura — e
/// um bloco de zero linhas é um evento que existe e não se vê.
fn faixa_do_dia(item: &Item, dia: NaiveDate, zona: TimeZoneName) -> Option<(usize, usize)> {
    const MINIMO: f64 = 30.0;

    // A hora civil, e não a de Greenwich.
    //
    // Isto lia `inicio.date_naive()` e `inicio.time()` — o instante em UTC — e
    // punha os blocos na linha errada da grelha para toda a gente que não
    // estivesse em Greenwich. Um compromisso das 00:30 aparecia às 22:30.
    let inicio = crate::ui::tempo::hora_civil(item.starts_at?, zona);
    let inicio_local = inicio.date();

    // Um evento que começou ontem e atravessa a meia-noite ocupa esta coluna
    // desde o topo. Cortá-lo faria desaparecer da grelha um compromisso que
    // ainda está a decorrer.
    let comeca = if inicio_local < dia {
        0.0
    } else if inicio_local > dia {
        return None;
    } else {
        f64::from(inicio.time().num_seconds_from_midnight()) / 60.0
    };

    let acaba = match item.ends_at.map(|f| crate::ui::tempo::hora_civil(f, zona)) {
        Some(fim) if fim.date() > dia => MINUTOS_DO_DIA,
        Some(fim) => f64::from(fim.time().num_seconds_from_midnight()) / 60.0,
        None => comeca + MINIMO,
    };

    let acaba = acaba.max(comeca + MINIMO).min(MINUTOS_DO_DIA);

    let linha = ((comeca / 30.0).floor() as usize).min(FAIXAS - 1);
    let fim = ((acaba / 30.0).ceil() as usize).min(FAIXAS);
    Some((linha, (fim - linha).max(1)))
}

/// Uma actividade já colocada: onde começa, que altura tem, e com quantas
/// divide a largura.
struct Colocado {
    item: Item,
    linha: usize,
    faixas: usize,
    coluna: usize,
    colunas: usize,
}

/// Distribui as actividades de um dia por colunas que não se sobrepõem.
///
/// # O algoritmo, e porque é este
///
/// Ordena por início e vai colocando cada uma na primeira coluna livre — a
/// primeira cujo último ocupante já acabou. É determinístico: a mesma lista dá
/// sempre a mesma disposição, o que importa porque uma grelha que reorganiza
/// entre dois carregamentos faz duvidar do que se leu.
///
/// Não é um escalonador. Não resolve prioridades nem sugere horários; resolve
/// **legibilidade**, que é o problema que a vista tem.
fn dispor(items: &[Item], dia: NaiveDate, zona: TimeZoneName) -> Vec<Colocado> {
    let mut faixas: Vec<(Item, usize, usize)> = items
        .iter()
        .filter(|i| !i.all_day)
        .filter_map(|i| faixa_do_dia(i, dia, zona).map(|(l, n)| (i.clone(), l, n)))
        .collect();
    faixas.sort_by_key(|(_, linha, _)| *linha);

    // Um grupo é um conjunto que se toca: enquanto houver sobreposição com
    // alguma das já colocadas, a largura continua a dividir-se. Fechado o
    // grupo, a largura volta a ser inteira.
    let mut colocados: Vec<Colocado> = Vec::new();
    let mut grupo: Vec<usize> = Vec::new();
    let mut fim_do_grupo = 0usize;

    for (item, linha, faixas_do_item) in faixas {
        if linha >= fim_do_grupo && !grupo.is_empty() {
            let largura = grupo
                .iter()
                .map(|i| colocados[*i].coluna + 1)
                .max()
                .unwrap_or(1);
            for i in &grupo {
                colocados[*i].colunas = largura;
            }
            grupo.clear();
            fim_do_grupo = 0;
        }

        // A primeira coluna onde nada do grupo ainda está a decorrer.
        let mut coluna = 0;
        loop {
            let ocupada = grupo.iter().any(|i| {
                colocados[*i].coluna == coluna && colocados[*i].linha + colocados[*i].faixas > linha
            });
            if !ocupada || coluna + 1 >= COLUNAS_MAX {
                break;
            }
            coluna += 1;
        }

        fim_do_grupo = fim_do_grupo.max(linha + faixas_do_item);
        grupo.push(colocados.len());
        colocados.push(Colocado {
            item,
            linha,
            faixas: faixas_do_item,
            coluna,
            colunas: 1,
        });
    }

    let largura = grupo
        .iter()
        .map(|i| colocados[*i].coluna + 1)
        .max()
        .unwrap_or(1);
    for i in &grupo {
        colocados[*i].colunas = largura;
    }

    colocados
}

/// A que altura do dia estamos agora, em percentagem — ou nada, se o dia não é
/// hoje.
fn agora_no_dia(dia: NaiveDate, zona: TimeZoneName) -> Option<usize> {
    // O agora de quem olha. Em Greenwich, a linha do «agora» aparecia à hora
    // errada — e no dia errado, uma vez por dia.
    let agora = crate::ui::tempo::hora_civil(Utc::now(), zona);
    (agora.date() == dia).then(|| {
        let minutos = f64::from(agora.time().num_seconds_from_midnight()) / 60.0;
        ((minutos / 30.0).floor() as usize).min(FAIXAS - 1)
    })
}

/// O eixo das horas, à esquerda da grelha.
fn eixo_das_horas() -> impl IntoView {
    view! {
        <div class="oc-cal-eixo" aria-hidden="true">
            {HORAS.map(|h| view! {
                <span class="oc-cal-eixo__hora">{format!("{h:02}:00")}</span>
            }).collect_view()}
        </div>
    }
}

/// As linhas de fundo de uma coluna de dia, uma por hora.
fn linhas_das_horas() -> impl IntoView {
    view! {
        <div class="oc-cal-linhas" aria-hidden="true">
            {HORAS.map(|_| view! { <span></span> }).collect_view()}
        </div>
    }
}

/// Uma coluna de dia com as suas actividades colocadas.
fn coluna_do_dia(items: &[Item], dia: NaiveDate, zona: TimeZoneName) -> impl IntoView {
    let colocados = dispor(items, dia, zona);
    let agora = agora_no_dia(dia, zona);

    view! {
        <div class="oc-cal-coluna" data-oc-dia=dia.to_string()>
            {linhas_das_horas()}
            {agora.map(|faixa| view! {
                <div class=format!("oc-cal-agora oc-cal-l{faixa}") aria-hidden="true"></div>
            })}
            {colocados.into_iter().map(|c| {
                // A posição vai por classe, e não por `style`: a CSP deste
                // Workspace é `style-src 'self'`, e um atributo de estilo seria
                // descartado sem uma palavra.
                let classes = format!(
                    "oc-cal-bloco oc-cal-l{} oc-cal-f{} oc-cal-c{}de{}",
                    c.linha, c.faixas, c.coluna + 1, c.colunas
                );
                let hora = c.item.clock(zona).unwrap_or_default();
                view! {
                    <a
                        class=classes
                        href=c.item.href()
                        data-kind=c.item.kind.clone()
                        title=c.item.title.clone()
                    >
                        <span class="oc-cal-bloco__hora">{hora}</span>
                        <span class="oc-cal-bloco__titulo">{c.item.title.clone()}</span>
                    </a>
                }
            }).collect_view()}
        </div>
    }
}

/// A faixa de dia inteiro, por cima do eixo.
///
/// Só aparece quando há alguma: uma faixa vazia permanente rouba altura ao
/// dia todos os dias por causa dos poucos em que é usada.
fn faixa_de_dia_inteiro(dias: &[(NaiveDate, Vec<Item>)]) -> Option<impl IntoView> {
    let algum = dias
        .iter()
        .any(|(_, items)| items.iter().any(|i| i.all_day));
    algum.then(|| {
        let dias = dias.to_vec();
        view! {
            <div class="oc-cal-diainteiro">
                <span class="oc-cal-diainteiro__rotulo">"Dia inteiro"</span>
                <div class="oc-cal-diainteiro__dias">
                    {dias.into_iter().map(|(_, items)| view! {
                        <div class="oc-cal-diainteiro__dia">
                            {items.iter().filter(|i| i.all_day).map(|item| view! {
                                <a
                                    class="oc-cal-bloco oc-cal-bloco--diainteiro"
                                    href=item.href()
                                    data-kind=item.kind.clone()
                                    title=item.title.clone()
                                >
                                    <span class="oc-cal-bloco__titulo">{item.title.clone()}</span>
                                </a>
                            }).collect_view()}
                        </div>
                    }).collect_view()}
                </div>
            </div>
        }
    })
}

/// O Dia, em linha do tempo.
///
/// É a Semana com uma coluna. Partilha o eixo, a colocação, a sobreposição e o
/// indicador da hora — porque são o mesmo problema, e duas implementações do
/// mesmo problema divergem sempre.
///
/// Era «Ao longo do dia» seguido de uma lista, e nada nela dizia a que horas as
/// coisas aconteciam nem quanto duravam.
fn today_view(items: &[Item], anchor: NaiveDate, zona: TimeZoneName) -> impl IntoView {
    let do_dia: Vec<Item> = items
        .iter()
        .filter(|i| i.day(zona) == anchor)
        .cloned()
        .collect();
    let dias = vec![(anchor, do_dia.clone())];

    view! {
        <div class="oc-cal-tempo oc-cal-tempo--dia">
            {faixa_de_dia_inteiro(&dias)}
            <div class="oc-cal-cabecas">
                <span class="oc-cal-cabecas__canto" aria-hidden="true"></span>
                <div class="oc-cal-cabeca oc-cal-cabeca--hoje">
                    <span class="oc-cal-cabeca__dia">
                        {crate::ui::tempo::dia_da_semana(anchor)}
                    </span>
                    <span class="oc-cal-cabeca__numero">{anchor.day().to_string()}</span>
                </div>
            </div>
            <div class="oc-cal-corpo" data-oc="linha-do-tempo">
                {eixo_das_horas()}
                <div class="oc-cal-colunas oc-cal-colunas--uma">
                    {coluna_do_dia(&do_dia, anchor, zona)}
                </div>
            </div>
        </div>
    }
}

/// A Agenda, em lista cronológica.
///
/// É a única vista deliberadamente sem grelha, e a única que pode dizer por
/// palavras que não tem nada: as outras têm estrutura temporal para preservar,
/// e uma lista vazia não tem estrutura nenhuma.
///
/// Funciona com teclado, com leitor de ecrã e numa janela estreita sem depender
/// de geometria — é a vista mais robusta que o Calendário tem, e é por isso que
/// existe além das outras.
fn agenda_view(items: &[Item], zona: TimeZoneName) -> AnyView {
    if items.is_empty() {
        return view! {
            <div class="oc-cal-agenda oc-cal-agenda--vazia">
                <p>"Nenhuma actividade para este período."</p>
            </div>
        }
        .into_any();
    }

    let mut dias: Vec<(NaiveDate, Vec<Item>)> = Vec::new();
    for item in items {
        let dia = item.day(zona);
        match dias.last_mut() {
            Some((anterior, lista)) if *anterior == dia => lista.push(item.clone()),
            _ => dias.push((dia, vec![item.clone()])),
        }
    }

    let hoje = crate::ui::tempo::hoje_civil(Utc::now(), zona);

    view! {
        <div class="oc-cal-agenda">
            {dias.into_iter().map(|(dia, lista)| {
                let e_hoje = dia == hoje;
                let classe = if e_hoje {
                    "oc-cal-grupo oc-cal-grupo--hoje"
                } else {
                    "oc-cal-grupo"
                };
                view! {
                    <section class=classe>
                        <div class="oc-cal-grupo__data">
                            <span class="oc-cal-grupo__numero">{dia.day().to_string()}</span>
                            <span class="oc-cal-grupo__dia">
                                {crate::ui::tempo::dia_da_semana(dia)}
                            </span>
                            <span class="oc-cal-grupo__mes">
                                {crate::ui::tempo::mes(dia)}
                            </span>
                        </div>
                        <ul class="oc-cal-grupo__itens">
                            {lista.iter().map(|item| view! {
                                <li>
                                    <a
                                        class="oc-cal-linha"
                                        href=item.href()
                                        data-kind=item.kind.clone()
                                    >
                                        <span class="oc-cal-linha__hora">
                                            {item.clock(zona).unwrap_or_else(|| "Dia inteiro".to_owned())}
                                        </span>
                                        <span class="oc-cal-linha__titulo">
                                            {item.title.clone()}
                                        </span>
                                        <span class="oc-cal-linha__tipo">
                                            {item.kind_label()}
                                        </span>
                                        {classification_badge(&item.classification)}
                                    </a>
                                </li>
                            }).collect_view()}
                        </ul>
                    </section>
                }
            }).collect_view()}
        </div>
    }
    .into_any()
}

fn week_view(items: &[Item], anchor: NaiveDate, zona: TimeZoneName) -> impl IntoView {
    let inicio = week_start(anchor);
    let hoje = crate::ui::tempo::hoje_civil(Utc::now(), zona);

    let dias: Vec<(NaiveDate, Vec<Item>)> = (0..7)
        .map(|offset| {
            let dia = inicio + Duration::days(offset);
            let do_dia = items
                .iter()
                .filter(|i| i.day(zona) == dia)
                .cloned()
                .collect();
            (dia, do_dia)
        })
        .collect();

    view! {
        <div class="oc-cal-tempo oc-cal-tempo--semana">
            {faixa_de_dia_inteiro(&dias)}
            <div class="oc-cal-cabecas">
                <span class="oc-cal-cabecas__canto" aria-hidden="true"></span>
                {dias.iter().map(|(dia, _)| {
                    let dia = *dia;
                    let classe = if dia == hoje {
                        "oc-cal-cabeca oc-cal-cabeca--hoje"
                    } else {
                        "oc-cal-cabeca"
                    };
                    view! {
                        <a
                            class=classe
                            href=format!("/calendar?view=day&on={dia}")
                            aria-label=format!("Ver {}", crate::ui::tempo::data_por_extenso(dia))
                        >
                            <span class="oc-cal-cabeca__dia">
                                {crate::ui::tempo::dia_da_semana_curto(dia)}
                            </span>
                            <span class="oc-cal-cabeca__numero">{dia.day().to_string()}</span>
                        </a>
                    }
                }).collect_view()}
            </div>
            <div class="oc-cal-corpo" data-oc="linha-do-tempo">
                {eixo_das_horas()}
                <div class="oc-cal-colunas">
                    {dias.iter().map(|(dia, do_dia)| coluna_do_dia(do_dia, *dia, zona)).collect_view()}
                </div>
            </div>
        </div>
    }
}

/// O Ano, em doze meses.
///
/// # O que esta vista serve
///
/// Orientação. Não mostra títulos: num ano cabem centenas de actividades e
/// nenhuma delas se lê a esta escala. O que ela responde é «em que meses há
/// coisas» e «onde estou no ano» — e daí abre-se o mês.
///
/// # Porque não são doze cartões
///
/// Porque um ano é contínuo. Doze superfícies com sombra própria diriam que
/// Janeiro e Fevereiro são objectos separados, e a única coisa que os separa é
/// uma linha.
fn year_view(items: &[Item], anchor: NaiveDate, zona: TimeZoneName) -> impl IntoView {
    let hoje = crate::ui::tempo::hoje_civil(Utc::now(), zona);
    let ano = anchor.year();

    // Que dias do ano têm alguma coisa. Um conjunto, e não uma contagem por
    // célula: a Year não diz quantas, diz se há.
    let ocupados: std::collections::BTreeSet<NaiveDate> =
        items.iter().map(|i| i.day(zona)).collect();

    view! {
        <div class="oc-cal-ano">
            {(1..=12u32).map(|m| {
                let primeiro = NaiveDate::from_ymd_opt(ano, m, 1).unwrap_or(anchor);
                let inicio = month_grid_start(primeiro);
                view! {
                    <section class="oc-cal-mini">
                        <a
                            class="oc-cal-mini__nome"
                            href=format!("/calendar?view=month&on={primeiro}")
                        >
                            {crate::ui::tempo::mes(primeiro)}
                        </a>
                        <div class="oc-cal-mini__semana" aria-hidden="true">
                            {crate::ui::tempo::cabecalhos_da_semana().into_iter()
                                .map(|d| view! { <span>{d.chars().next().unwrap_or(' ').to_string()}</span> })
                                .collect_view()}
                        </div>
                        <div class="oc-cal-mini__dias">
                            {(0..42).map(|offset| {
                                let dia = inicio + Duration::days(offset);
                                let fora = dia.month() != m;
                                let mut classes = String::from("oc-cal-mini__dia");
                                if fora {
                                    classes.push_str(" oc-cal-mini__dia--fora");
                                }
                                if dia == hoje {
                                    classes.push_str(" oc-cal-mini__dia--hoje");
                                }
                                if !fora && ocupados.contains(&dia) {
                                    classes.push_str(" oc-cal-mini__dia--ocupado");
                                }
                                view! {
                                    <a
                                        class=classes
                                        href=format!("/calendar?view=day&on={dia}")
                                        aria-label=crate::ui::tempo::data_por_extenso(dia)
                                    >
                                        {dia.day().to_string()}
                                    </a>
                                }
                            }).collect_view()}
                        </div>
                    </section>
                }
            }).collect_view()}
        </div>
    }
}

fn month_view(items: &[Item], anchor: NaiveDate, zona: TimeZoneName) -> impl IntoView {
    let inicio = month_grid_start(anchor);
    let mes = anchor.month();
    let hoje = crate::ui::tempo::hoje_civil(Utc::now(), zona);

    view! {
        <div class="oc-cal-month" role="table" aria-label="Mês">
            <div class="oc-cal-month__weekdays" role="row">
                {["Seg", "Ter", "Qua", "Qui", "Sex", "Sáb", "Dom"].into_iter()
                    .map(|nome| view! { <span role="columnheader">{nome}</span> })
                    .collect_view()}
            </div>
            {(0..42).map(|offset| {
                let dia = inicio + Duration::days(offset);
                let do_dia: Vec<Item> =
                    items.iter().filter(|item| item.day(zona) == dia).cloned().collect();
                let excedente = do_dia.len().saturating_sub(MONTH_CELL_LIMIT);
                let fora_do_mes = dia.month() != mes;

                let mut classes = String::from("oc-cal-month__cell");
                if fora_do_mes {
                    classes.push_str(" oc-cal-month__cell--outside");
                }
                if dia == hoje {
                    classes.push_str(" oc-cal-month__cell--today");
                }
                // O dia escolhido não é o dia de hoje.
                //
                // São duas perguntas diferentes — «que dia é hoje» e «que dia
                // estou a ver» — e há um caso em que coincidem. Se partilhassem
                // o mesmo tratamento, abrir o dia 12 mostraria o 12 exactamente
                // como mostra hoje, e ninguém saberia qual é qual.
                //
                // A âncora é a data que o endereço traz. Ela já governa a
                // grelha inteira; o que faltava era dizê-lo na célula.
                if dia == anchor {
                    classes.push_str(" oc-cal-month__cell--selected");
                }

                view! {
                    // A célula leva a sua data. Sem ela, quem quisesse abrir
                    // uma actividade neste dia teria de a extrair do endereço
                    // do número — e um endereço é para navegar, não para ser
                    // lido como dado.
                    <div class=classes role="cell" data-oc-dia=dia.to_string()>
                        <a
                            class="oc-cal-month__date"
                            href=format!("/calendar?view=today&on={dia}")
                            aria-label=format!("Ver {}", crate::ui::tempo::data_por_extenso(dia))
                        >
                            {dia.day().to_string()}
                        </a>
                        {do_dia.iter().take(MONTH_CELL_LIMIT).map(|item| view! {
                            <a
                                class="oc-cal-month__item"
                                href=item.href()
                                data-kind=item.kind.clone()
                                title=item.title.clone()
                            >
                                <span class="oc-cal-month__time">
                                    {item.clock(zona).unwrap_or_default()}
                                </span>
                                // O título num elemento próprio, e não num nó
                                // de texto solto: um nó de texto não recebe
                                // `min-width: 0`, recusa-se a encolher e as
                                // reticências nunca aparecem. Cortava a meio da
                                // palavra sem dizer que tinha cortado.
                                <span class="oc-cal-month__titulo">
                                    {item.title.clone()}
                                </span>
                            </a>
                        }).collect_view()}
                        {(excedente > 0).then(|| view! {
                            // Abre o dia inteiro, e não uma lista truncada: o
                            // «+3» é uma promessa de que os três estão em
                            // algum lado.
                            <a
                                class="oc-cal-month__more"
                                href=format!("/calendar?view=today&on={dia}")
                            >
                                {format!("+{excedente}")}
                            </a>
                        })}
                    </div>
                }
            }).collect_view()}
        </div>
    }
}

// ── Centro Temporal ─────────────────────────────────────────────────────

/// O calendário compacto do sistema, ancorado no relógio da barra.
///
/// # O que mostra, e o que deliberadamente não mostra
///
/// A data por extenso, o mês corrente em miniatura, o dia de hoje, e uma porta
/// para o Calendário. Mais nada.
///
/// Não carrega actividades. Isso não é uma omissão de capacidade: é a fronteira.
/// Um painel da barra que lesse a agenda passaria a ter uma segunda opinião
/// sobre o que a pessoa tem marcado — e duas superfícies a responder à mesma
/// pergunta acabam por discordar. Quem quer ver o que tem marcado abre o
/// Calendário, que é onde há espaço para o mostrar bem.
///
/// Sendo apresentação pura, também não depende do domínio do Calendário: a
/// grelha do mês corrente sai da data, e a data sai do relógio.
///
/// # Porque o mês é calculado no servidor
///
/// Porque o servidor sabe que dia é. O relógio da barra é do lado do cliente
/// porque mostra a hora **local** de quem vê; a grelha do mês não muda com o
/// fuso a esse ponto, e desenhá-la aqui evita um segundo calendário escrito em
/// JavaScript.
pub fn system_calendar(hoje: NaiveDate) -> impl IntoView {
    use crate::ui::tempo;

    let inicio = month_grid_start(hoje);
    let semana_de_hoje = week_start(hoje);

    view! {
        <div
            class="oc-pop oc-datepop"
            id="oc-temporal-centre"
            data-oc="temporal-centre"
            role="dialog"
            aria-label="Calendário do sistema"
            hidden
        >
            <header class="oc-datepop__cabeca">
                <span class="oc-datepop__dia">{tempo::dia_da_semana(hoje)}</span>
                <span class="oc-datepop__data">{tempo::data_por_extenso(hoje)}</span>
            </header>

            <div class="oc-datepop__mes">{tempo::mes_e_ano(hoje)}</div>

            <div class="oc-datepop__semana" aria-hidden="true">
                {tempo::cabecalhos_da_semana().into_iter()
                    .map(|d| view! { <span>{d}</span> })
                    .collect_view()}
            </div>

            <div class="oc-datepop__dias">
                {(0..42).map(|offset| {
                    let dia = inicio + Duration::days(offset);
                    let mut classes = String::from("oc-datepop__dia-cel");
                    if dia.month() != hoje.month() {
                        classes.push_str(" oc-datepop__dia-cel--fora");
                    }
                    if week_start(dia) == semana_de_hoje {
                        classes.push_str(" oc-datepop__dia-cel--semana");
                    }
                    if dia == hoje {
                        classes.push_str(" oc-datepop__dia-cel--hoje");
                    }
                    view! { <span class=classes>{dia.day().to_string()}</span> }
                }).collect_view()}
            </div>

            <footer class="oc-datepop__accoes">
                <a class="oc-datepop__abrir" href=CALENDAR_ROUTE>"Abrir Calendário"</a>
            </footer>
        </div>
    }
}

/// O endereço do Calendário, tal como o catálogo de rotas o declara.
///
/// Escrito uma vez. Um endereço repetido à mão em cada sítio que liga para lá é
/// um endereço que deixa de coincidir com o catálogo sem ninguém reparar.
const CALENDAR_ROUTE: &str = "/calendar";

/// O horário que o editor propõe para uma actividade marcada agora.
///
/// Vive aqui, ao lado do editor que o usa, e a rota chama-o para poder aplicar
/// a mesma política a um dia escolhido no Calendário.
#[must_use]
pub fn tempo_proposto(agora: chrono::NaiveDateTime) -> chrono::NaiveDateTime {
    crate::ui::tempo::proximo_meio_periodo(agora)
}

/// O horário com que o editor abre, dado o que o Calendário trouxe consigo.
///
/// # A precedência, e porquê
///
/// ```text
/// hora explícita  →  dia escolhido  →  agora
/// ```
///
/// Quem carregou nas 14:00 de quinta quer marcar às 14:00 de quinta. Quem
/// carregou no dia 28 quer marcar no dia 28, e a hora é a que a política de
/// omissão propõe — aplicada **a esse dia**, e não a hoje. Substituir a data
/// escolhida pela de hoje seria descartar a única coisa que a pessoa já disse.
///
/// Nada disto é estado institucional: é o que uma pessoa está a olhar, viaja no
/// endereço, e morre quando o formulário fecha.
#[must_use]
pub fn horario_do_editor(
    dia: Option<NaiveDate>,
    hora: Option<chrono::NaiveTime>,
    agora: chrono::NaiveDateTime,
) -> chrono::NaiveDateTime {
    let proposta = crate::ui::tempo::proximo_meio_periodo(agora);
    match (dia, hora) {
        (Some(dia), Some(hora)) => dia.and_time(hora),
        (Some(dia), None) => dia.and_time(proposta.time()),
        _ => proposta,
    }
}

/// O editor de actividade.
///
/// # A composição, e porque é esta
///
/// O que existia era um formulário HTML: rótulo colado ao campo, `fieldset` com
/// `legend` crua, quatro controlos temporais numa linha só, e três selectores de
/// pertença visíveis ao mesmo tempo — dois deles vazios e irrelevantes para o
/// âmbito escolhido. Marcar uma reunião obrigava a ler tudo para descobrir o que
/// não interessava.
///
/// Agora são três perguntas em sequência: **o que é**, **quando é**, **de quem
/// é**. Cada uma só mostra o que a resposta anterior tornou relevante.
///
/// # O que continua a não ser decidido aqui
///
/// Nada de institucional. A Experience prepara o pedido; é o Core que valida a
/// hora, interpreta a zona, autoriza o âmbito e decide o que fica escrito. Os
/// nomes dos campos são os mesmos de antes precisamente para que essa fronteira
/// não se mexa com uma alteração de desenho.
pub fn event_form(
    editing: Option<&Item>,
    units: &Value,
    workspaces: &Value,
    error: Option<String>,
    proposto: Option<chrono::NaiveDateTime>,
    pessoas: &Value,
    zona: TimeZoneName,
) -> impl IntoView {
    let a_alterar = editing.is_some();
    let titulo = editing.map(|i| i.title.clone()).unwrap_or_default();
    let accao = editing.map_or_else(
        || "/calendar/events/new".to_owned(),
        |i| format!("/calendar/events/{}/edit", i.id),
    );
    let hoje = crate::ui::tempo::hoje_civil(Utc::now(), zona);

    // Os campos abrem com um horário que se aceita sem pensar.
    //
    // Estavam vazios — `dd/mm/yyyy, --:--` nos dois — e marcar uma reunião banal
    // obrigava a quatro decisões antes de escrever o título.
    let (inicio_proposto, fim_proposto) = proposto
        .map(|i| {
            (
                crate::ui::tempo::para_campo(i),
                crate::ui::tempo::para_campo(
                    i + Duration::minutes(crate::ui::tempo::DURACAO_PADRAO_MINUTOS),
                ),
            )
        })
        .unwrap_or_default();

    let opcoes = |valores: &Value| -> Vec<(String, String)> {
        valores
            .as_array()
            .map(|linhas| {
                linhas
                    .iter()
                    .filter_map(|linha| {
                        Some((
                            linha.get("id")?.as_str()?.to_owned(),
                            linha
                                .get("name")
                                .or_else(|| linha.get("title"))
                                .and_then(Value::as_str)
                                .unwrap_or("—")
                                .to_owned(),
                        ))
                    })
                    .collect()
            })
            .unwrap_or_default()
    };
    let unidades = opcoes(units);
    let ambientes = opcoes(workspaces);

    // O universo de participantes é o que o Core devolveu a quem está a marcar.
    //
    // A Experience não decide quem pode participar: mostra o que lhe foi
    // autorizado, e o Core volta a verificar cada identificador antes de
    // escrever seja o que for.
    let participaveis: Vec<(String, String, String)> = pessoas
        .get("items")
        .or(Some(pessoas))
        .and_then(Value::as_array)
        .map(|linhas| {
            linhas
                .iter()
                .filter_map(|linha| {
                    Some((
                        linha.get("id")?.as_str()?.to_owned(),
                        linha
                            .get("display_name")
                            .or_else(|| linha.get("name"))
                            .or_else(|| linha.get("full_name"))
                            .and_then(Value::as_str)
                            .filter(|nome| !nome.is_empty())
                            .unwrap_or("—")
                            .to_owned(),
                        // O endereço institucional, que é a identidade humana
                        // desde o ADR-0106. Dois colegas podem chamar-se o
                        // mesmo; o endereço não.
                        linha
                            .get("email")
                            .and_then(Value::as_str)
                            .unwrap_or_default()
                            .to_owned(),
                    ))
                })
                .collect()
        })
        .unwrap_or_default();

    // Um âmbito sem nada onde o pôr não é uma escolha: é um beco. O Core devolve
    // o universo autorizado, e o que não tem destino não se oferece.
    let ha_unidades = !unidades.is_empty();
    let ha_ambientes = !ambientes.is_empty();

    view! {
        <div class="oc-page oc-editor">
            <header class="oc-editor__cabeca">
                <h1>{if a_alterar { "Alterar actividade" } else { "Nova actividade" }}</h1>
            </header>

            {error.map(|motivo| view! {
                <div class="oc-alert oc-alert--error" role="alert">{motivo}</div>
            })}

            <form class="oc-editor__form" method="post" action=accao data-oc="editor">
                <section class="oc-editor__bloco">
                    <label class="oc-campo oc-campo--principal">
                        <span class="oc-campo__rotulo">"Título"</span>
                        <input
                            class="oc-entrada oc-entrada--titulo"
                            name="title"
                            required=true
                            value=titulo
                            maxlength="255"
                            placeholder="Reunião do conselho"
                            autocomplete="off"
                        />
                    </label>

                    <label class="oc-campo">
                        <span class="oc-campo__rotulo">"Descrição"</span>
                        <textarea class="oc-entrada oc-entrada--texto" name="description" rows="3">
                        </textarea>
                    </label>

                    <label class="oc-campo">
                        <span class="oc-campo__rotulo">"Localização"</span>
                        <input
                            class="oc-entrada"
                            name="location"
                            maxlength="255"
                            placeholder="Sala, edifício ou ligação"
                        />
                    </label>
                </section>

                <section class="oc-editor__bloco">
                    <h2 class="oc-editor__seccao">"Quando"</h2>

                    <label class="oc-interruptor">
                        <input type="checkbox" name="all_day" value="1" data-oc="all-day" />
                        <span class="oc-interruptor__marca" aria-hidden="true"></span>
                        <span class="oc-interruptor__texto">"Dia inteiro"</span>
                    </label>

                    <div class="oc-quando" data-oc="timed-fields">
                        <label class="oc-campo">
                            <span class="oc-campo__rotulo">"Início"</span>
                            <input
                                class="oc-entrada"
                                type="datetime-local"
                                name="starts_at"
                                value=inicio_proposto
                                data-oc="inicio"
                            />
                        </label>
                        <label class="oc-campo">
                            <span class="oc-campo__rotulo">"Fim"</span>
                            <input
                                class="oc-entrada"
                                type="datetime-local"
                                name="ends_at"
                                value=fim_proposto
                                data-oc="fim"
                            />
                        </label>
                    </div>

                    <div class="oc-quando" data-oc="allday-fields" hidden>
                        <label class="oc-campo">
                            <span class="oc-campo__rotulo">"Primeiro dia"</span>
                            <input
                                class="oc-entrada"
                                type="date"
                                name="starts_on"
                                value=hoje.to_string()
                            />
                        </label>
                        <label class="oc-campo">
                            <span class="oc-campo__rotulo">"Último dia"</span>
                            // Inclusivo aqui, exclusivo na base. A pessoa escreve o
                            // último dia do evento; a conversão é nossa.
                            <input
                                class="oc-entrada"
                                type="date"
                                name="ends_on"
                                value=hoje.to_string()
                            />
                        </label>
                    </div>

                    // A zona é contexto, não um campo a preencher.
                    //
                    // Era uma caixa de texto livre com `UTC` lá dentro — a
                    // representação do armazenamento apresentada como se fosse a
                    // zona de quem marca, e editável para qualquer cadeia que o
                    // Core depois recusaria. O valor continua a ir no pedido, e
                    // continua a ser o Core a validá-lo; o que muda é que deixa
                    // de se pedir a alguém que o escreva.
                    <p class="oc-zona">
                        <span class="oc-zona__rotulo">"Zona horária"</span>
                        <span class="oc-zona__valor" data-oc="timezone-label">"UTC"</span>
                        <input type="hidden" name="timezone" data-oc="timezone" value="UTC" />
                    </p>
                </section>

                {(!a_alterar && !participaveis.is_empty()).then(|| view! {
                    <section class="oc-editor__bloco" data-oc="participantes">
                        <h2 class="oc-editor__seccao">"Participantes"</h2>

                        <label class="oc-campo oc-campo--estreito">
                            <span class="oc-campo__rotulo">"Procurar uma pessoa"</span>
                            <input
                                class="oc-entrada"
                                type="search"
                                data-oc="procura-pessoa"
                                placeholder="Nome ou endereço institucional"
                                autocomplete="off"
                                aria-controls="oc-pessoas"
                            />
                        </label>

                        // A lista completa vem do servidor e é filtrada no
                        // browser. É o universo que o Core já autorizou a esta
                        // pessoa, e filtrá-lo aqui evita um pedido por cada
                        // tecla — sem alargar o que ela pode ver.
                        <ul class="oc-pessoas" id="oc-pessoas" data-oc="lista-pessoas" hidden>
                            {participaveis.iter().map(|(id, nome, email)| view! {
                                <li>
                                    <button
                                        type="button"
                                        class="oc-pessoa"
                                        data-oc="pessoa"
                                        data-id=id.clone()
                                        data-nome=nome.clone()
                                        data-email=email.clone()
                                    >
                                        <b>{nome.clone()}</b>
                                        <em>{email.clone()}</em>
                                    </button>
                                </li>
                            }).collect_view()}
                        </ul>

                        <p class="oc-pessoas__nada" data-oc="sem-pessoas" hidden>
                            "Ninguém corresponde a essa procura."
                        </p>

                        <div class="oc-escolhidos" data-oc="escolhidos"></div>
                    </section>
                })}

                {(!a_alterar).then(|| view! {
                    <section class="oc-editor__bloco">
                        <h2 class="oc-editor__seccao">"Pertence a"</h2>

                        <label class="oc-campo oc-campo--estreito">
                            <span class="oc-campo__rotulo">"Âmbito"</span>
                            <select class="oc-entrada" name="scope" data-oc="scope">
                                <option value="personal">"Pessoal"</option>
                                {ha_unidades.then(|| view! {
                                    <option value="unit">"Unidade"</option>
                                })}
                                {ha_ambientes.then(|| view! {
                                    <option value="research_workspace">
                                        "Ambiente de investigação"
                                    </option>
                                })}
                                <option value="institution">"Instituição"</option>
                            </select>
                        </label>

                        {ha_unidades.then(|| view! {
                            <label class="oc-campo oc-campo--estreito" data-oc="unit-field" hidden>
                                <span class="oc-campo__rotulo">"Unidade"</span>
                                <select class="oc-entrada" name="unit_id">
                                    {unidades.into_iter().map(|(id, nome)| view! {
                                        <option value=id>{nome}</option>
                                    }).collect_view()}
                                </select>
                            </label>
                        })}

                        {ha_ambientes.then(|| view! {
                            <label
                                class="oc-campo oc-campo--estreito"
                                data-oc="workspace-field"
                                hidden
                            >
                                <span class="oc-campo__rotulo">"Ambiente de investigação"</span>
                                <select class="oc-entrada" name="workspace_id">
                                    {ambientes.into_iter().map(|(id, nome)| view! {
                                        <option value=id>{nome}</option>
                                    }).collect_view()}
                                </select>
                            </label>
                        })}
                    </section>
                })}

                <footer class="oc-editor__accoes">
                    <a class="oc-btn oc-btn--ghost" href=CALENDAR_ROUTE>"Cancelar"</a>
                    <button type="submit" class="oc-btn oc-btn--primary" data-oc="submeter">
                        {if a_alterar { "Guardar alterações" } else { "Criar actividade" }}
                    </button>
                </footer>
            </form>
        </div>
    }
}

pub fn event_detail(event: &Value, may_change: bool, zona: TimeZoneName) -> impl IntoView {
    let campo = |chave: &str| {
        event
            .get(chave)
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned()
    };
    let id = campo("id");
    let cancelado = campo("state") == "cancelled";
    let item = Item::from_json(&serde_json::json!({
        "kind": "event",
        "id": id.clone(),
        "title": campo("title"),
        "all_day": event.get("all_day").and_then(Value::as_bool).unwrap_or(false),
        "starts_at": event.get("starts_at").cloned().unwrap_or(Value::Null),
        "ends_at": event.get("ends_at").cloned().unwrap_or(Value::Null),
        "timezone": event.get("timezone").cloned().unwrap_or(Value::Null),
        "starts_on": event.get("starts_on").cloned().unwrap_or(Value::Null),
        "ends_before": event.get("ends_before").cloned().unwrap_or(Value::Null),
        "state": campo("state"),
        "classification": campo("classification"),
    }));

    view! {
        <div class="oc-page oc-page--detail">
            <div class="oc-head">
                <div class="oc-head__text">
                    <h1>{campo("title")}</h1>
                    <p>{item.as_ref().map(|i| i.when(zona)).unwrap_or_default()}</p>
                </div>
                {(may_change && !cancelado).then(|| view! {
                    <div class="oc-head__actions">
                        <a class="oc-btn" href=format!("/calendar/events/{id}/edit")>"Alterar"</a>
                        <form method="post" action=format!("/calendar/events/{id}/cancel")>
                            <button type="submit" class="oc-btn oc-btn--danger">"Cancelar"</button>
                        </form>
                    </div>
                })}
            </div>

            {cancelado.then(|| view! {
                // Cancelado continua visível. Um evento que desaparece não avisa
                // quem o esperava.
                <div class="oc-alert oc-alert--warning" role="status">
                    "Esta actividade foi cancelada. Fica visível para quem a esperava."
                </div>
            })}

            <dl class="oc-detail">
                <dt>"Quando"</dt>
                <dd>{item.as_ref().map(|i| i.when(zona)).unwrap_or_default()}</dd>
                {(!campo("timezone").is_empty()).then(|| view! {
                    <>
                        <dt>"Zona horária"</dt>
                        <dd>{campo("timezone")}</dd>
                    </>
                })}
                {(!campo("location").is_empty()).then(|| view! {
                    <>
                        <dt>"Local"</dt>
                        <dd>{campo("location")}</dd>
                    </>
                })}
                {(!campo("description").is_empty()).then(|| view! {
                    <>
                        <dt>"Descrição"</dt>
                        <dd>{campo("description")}</dd>
                    </>
                })}
                <dt>"Classificação"</dt>
                <dd>{classification_badge(&campo("classification"))}</dd>
            </dl>
        </div>
    }
}

// ── Notificações ────────────────────────────────────────────────────────

/// O centro de notificações.
///
/// # Porque a notificação não abre nada por si
///
/// Ela aponta. Quando alguém carrega, o Core reautoriza o recurso nesse
/// momento — uma notificação de ontem sobre um evento a que a pessoa deixou de
/// ter acesso leva a uma recusa, e não ao conteúdo (ADR-0410).
pub fn notifications(payload: &Value, failure: Option<String>) -> impl IntoView {
    let linhas = payload
        .get("notifications")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let por_ler = payload.get("unread").and_then(Value::as_i64).unwrap_or(0);
    let vazio = linhas.is_empty();

    view! {
        <div class="oc-page oc-page--feed">
            <div class="oc-head">
                <div class="oc-head__text">
                    <h1>"Notificações"</h1>
                    <p>
                        {if por_ler > 0 {
                            format!("{por_ler} por ler.")
                        } else {
                            "Nada por ler.".to_owned()
                        }}
                    </p>
                </div>
            </div>

            {match failure {
                Some(motivo) => view! {
                    <div class="oc-alert oc-alert--error" role="alert">
                        <strong>"Não foi possível ler as notificações."</strong>
                        <span>{motivo}</span>
                    </div>
                }.into_any(),
                None if vazio => view! {
                    <div class="oc-empty"><p>"Ainda não há notificações."</p></div>
                }.into_any(),
                None => view! {
                    <ul class="oc-notifications">
                        {linhas.into_iter().map(|linha| {
                            let campo = |chave: &str| {
                                linha.get(chave).and_then(Value::as_str).unwrap_or_default().to_owned()
                            };
                            let id = campo("id");
                            let lida = linha.get("read").and_then(Value::as_bool).unwrap_or(false);
                            let destino = match campo("resource_type").as_str() {
                                "calendar_event" => Some(format!("/calendar/events/{}", campo("resource_id"))),
                                "task" => Some("/my-work".to_owned()),
                                "conversation" => Some(format!(
                                    "{}/{}",
                                    crate::ui::screens::messaging::ROUTE,
                                    campo("resource_id")
                                )),
                                _ => None,
                            };

                            view! {
                                <li class=if lida {
                                    "oc-notification"
                                } else {
                                    "oc-notification oc-notification--unread"
                                }>
                                    <span class="oc-notification__title">{campo("title")}</span>
                                    {(!lida).then(|| view! {
                                        <span class="oc-notification__dot" aria-label="Por ler"></span>
                                    })}
                                    <span class="oc-notification__actions">
                                        {destino.map(|href| view! {
                                            <a class="oc-btn oc-btn--ghost" href=href>"Abrir"</a>
                                        })}
                                        {(!lida).then(|| view! {
                                            <form method="post" action=format!("/notifications/{id}/read")>
                                                <button type="submit" class="oc-btn oc-btn--ghost">
                                                    "Marcar como lida"
                                                </button>
                                            </form>
                                        })}
                                    </span>
                                </li>
                            }
                        }).collect_view()}
                    </ul>
                }.into_any(),
            }}
        </div>
    }
}

#[cfg(test)]
mod grelha_do_mes {
    use super::*;

    /// A zona destes testes.
    ///
    /// Explícita, e não «a do sistema»: um teste que dependesse do fuso da
    /// máquina passaria aqui e falharia em CI, ou ao contrário.
    fn zona_de_teste() -> TimeZoneName {
        "UTC".to_owned().try_into().expect("fuso conhecido")
    }

    fn dia(a: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(a, m, d).expect("data válida")
    }

    fn pagina(items: &[Item], anchor: NaiveDate) -> String {
        calendar(&CalendarPage {
            view: CalendarView::Month,
            anchor,
            items,
            may_create: true,
            failure: None,
            zona: zona_de_teste(),
        })
        .to_html()
    }

    fn evento(titulo: &str, quando: NaiveDate, hora: u32) -> Item {
        use chrono::TimeZone;
        Item {
            kind: "event".to_owned(),
            id: format!("id-{titulo}"),
            title: titulo.to_owned(),
            all_day: false,
            starts_at: Some(
                Utc.with_ymd_and_hms(quando.year(), quando.month(), quando.day(), hora, 0, 0)
                    .single()
                    .expect("instante"),
            ),
            ends_at: None,
            timezone: Some("Europe/Lisbon".to_owned()),
            starts_on: None,
            ends_before: None,
            state: "confirmed".to_owned(),
            classification: "INTERNAL".to_owned(),
        }
    }

    /// Um mês sem nada marcado continua a ser um mês.
    ///
    /// # Porque este é o teste que mais importa aqui
    ///
    /// Porque o defeito existia e era total: um `items.is_empty()` substituía a
    /// vista inteira pela frase «nenhuma actividade para este período». As setas
    /// ficavam, o título do mês ficava, e no meio não havia calendário nenhum —
    /// a estrutura do tempo desaparecia com o conteúdo.
    ///
    /// Um mês vazio é um calendário limpo. É precisamente quando não há nada
    /// marcado que alguém vai à procura de onde marcar.
    #[test]
    fn um_mes_vazio_desenha_a_grelha_inteira() {
        let html = pagina(&[], dia(2026, 8, 26));

        assert_eq!(
            html.matches("oc-cal-month__cell").count()
                - html.matches("oc-cal-month__cell--").count(),
            42,
            "a grelha de um mês vazio não tem as seis semanas"
        );
        for cabecalho in crate::ui::tempo::cabecalhos_da_semana() {
            assert!(
                html.contains(&format!(">{cabecalho}<")),
                "falta o cabeçalho «{cabecalho}» num mês vazio"
            );
        }
        assert!(
            !html.contains("Nenhuma actividade"),
            "o mês vazio voltou a substituir a grelha por uma frase"
        );
    }

    /// Cada actividade aparece no dia em que acontece, e em mais nenhum.
    ///
    /// # Porque compara a data da célula e não o número visível
    ///
    /// A primeira versão partia o HTML por `oc-cal-month__cell` e procurava o
    /// número do dia no pedaço que continha o título. Movi o filtro da grelha um
    /// dia para o lado — a reversão que este teste existe para apanhar — e ele
    /// passou na mesma: as células com modificador contêm o literal duas vezes,
    /// os pedaços desalinham-se, e o número que ele encontrava não era o da
    /// célula onde o item estava.
    ///
    /// Cada célula traz a sua data por extenso no endereço do dia. É contra ela
    /// que a colocação se verifica, porque é a mesma data que a grelha usou para
    /// decidir.
    #[test]
    fn os_eventos_caem_no_dia_certo() {
        let anchor = dia(2026, 8, 26);
        let items = [
            evento("Reunião do conselho", dia(2026, 8, 26), 9),
            evento("Defesa de projecto", dia(2026, 8, 29), 10),
        ];
        let html = pagina(&items, anchor);

        // Uma célula começa no seu endereço e acaba onde a seguinte começa.
        let celulas: Vec<&str> = html
            .split(r#"<a href="/calendar?view=today&amp;on="#)
            .collect();
        assert!(
            celulas.len() >= 43,
            "só {} pedaços de célula: a grelha não tem 42 dias",
            celulas.len() - 1
        );

        for (titulo, esperado) in [
            ("Reunião do conselho", "2026-08-26"),
            ("Defesa de projecto", "2026-08-29"),
        ] {
            let onde: Vec<&str> = celulas
                .iter()
                .filter(|c| c.contains(titulo))
                .copied()
                .collect();
            assert_eq!(
                onde.len(),
                1,
                "«{titulo}» aparece em {} células",
                onde.len()
            );
            let data = &onde[0][..10];
            assert_eq!(
                data, esperado,
                "«{titulo}» está na célula de {data} e devia estar na de {esperado}"
            );
        }
    }

    /// O dia escolhido e o dia de hoje são coisas diferentes.
    #[test]
    fn o_dia_escolhido_nao_se_confunde_com_hoje() {
        let hoje = crate::ui::tempo::hoje_civil(Utc::now(), zona_de_teste());
        // Outro dia **do mesmo mês**, e é por isso que a direcção depende do
        // dia. `hoje + 3` parece inofensivo e não é: nos últimos dias do mês
        // cai no mês seguinte, a grelha passa a ser a desse mês, e hoje deixa
        // de aparecer nela. O teste falhava três dias por mês, todos os meses,
        // e foi assim que apanhou o `verify.sh` a 29 de Agosto.
        let outro = if hoje.day() > 15 {
            hoje - Duration::days(3)
        } else {
            hoje + Duration::days(3)
        };
        assert_eq!(
            outro.month(),
            hoje.month(),
            "o dia escolhido saiu do mês de hoje, e a grelha deixaria de o conter"
        );
        let html = pagina(&[], outro);

        assert!(
            html.contains("oc-cal-month__cell--today"),
            "a grelha deixou de marcar hoje"
        );
        assert!(
            html.contains("oc-cal-month__cell--selected"),
            "a grelha deixou de marcar o dia escolhido"
        );

        // E não na mesma célula, porque não é o mesmo dia.
        for pedaco in html.split("oc-cal-month__cell") {
            let e_hoje = pedaco.starts_with("--today");
            let escolhido = pedaco.starts_with("--selected");
            assert!(
                !(e_hoje && escolhido),
                "hoje e o dia escolhido saíram na mesma célula sendo dias diferentes"
            );
        }
    }

    /// Quando coincidem, continuam os dois legíveis.
    #[test]
    fn hoje_escolhido_diz_as_duas_coisas() {
        let hoje = crate::ui::tempo::hoje_civil(Utc::now(), zona_de_teste());
        let html = pagina(&[], hoje);

        assert!(html.contains("oc-cal-month__cell--today"));
        assert!(html.contains("oc-cal-month__cell--selected"));
    }

    /// O que não cabe conta-se, e a contagem é a verdadeira.
    #[test]
    fn o_excedente_conta_o_que_ficou_de_fora() {
        let quando = dia(2026, 8, 26);
        let items: Vec<Item> = (0..9)
            .map(|i| evento(&format!("Compromisso {i}"), quando, 8 + i as u32))
            .collect();
        let html = pagina(&items, quando);

        let esperado = items.len() - MONTH_CELL_LIMIT;
        assert!(
            html.contains(&format!(">+{esperado}<")),
            "com {} itens e um limite de {MONTH_CELL_LIMIT}, faltava «+{esperado}»",
            items.len()
        );
        assert_eq!(
            html.matches("oc-cal-month__item").count(),
            MONTH_CELL_LIMIT,
            "a célula mostrou mais itens do que o limite"
        );
    }

    /// Andar para trás e para a frente anda em meses.
    ///
    /// # Porque isto era um defeito e não uma preferência
    ///
    /// A conta era `anchor - span_days()`, e o Mês abrange 42 dias de grelha:
    /// «anterior» recuava seis semanas e aterrava a meio do mês errado. A
    /// Semana andava sete dias, que por acaso está certo, e por isso ninguém
    /// reparou.
    #[test]
    fn a_navegacao_anda_na_unidade_da_vista() {
        let agosto = dia(2026, 8, 26);
        assert_eq!(step(CalendarView::Month, agosto, false).month(), 7);
        assert_eq!(step(CalendarView::Month, agosto, true).month(), 9);

        // A passagem de ano, nos dois sentidos.
        assert_eq!(
            step(CalendarView::Month, dia(2026, 12, 15), true),
            dia(2027, 1, 1)
        );
        assert_eq!(
            step(CalendarView::Month, dia(2026, 1, 15), false),
            dia(2025, 12, 1)
        );

        // E um dia que não existe no mês vizinho não faz a conta rebentar.
        assert_eq!(
            step(CalendarView::Month, dia(2026, 3, 31), false),
            dia(2026, 2, 1)
        );

        assert_eq!(step(CalendarView::Day, agosto, true), dia(2026, 8, 27));
        assert_eq!(step(CalendarView::Week, agosto, true), dia(2026, 9, 2));
    }

    /// `Hoje` aparece uma vez, e como acção.
    #[test]
    fn hoje_nao_e_uma_vista() {
        let html = pagina(&[], dia(2026, 8, 26));

        assert_eq!(
            html.matches(">Hoje<").count(),
            1,
            "«Hoje» aparece mais do que uma vez na barra"
        );
        for vista in CalendarView::all() {
            assert_ne!(vista.label(), "Hoje", "«Hoje» voltou a ser uma vista");
        }
        assert!(
            html.contains("oc-cal-hoje"),
            "«Hoje» deixou de ser uma acção"
        );
    }

    /// Nenhum rótulo da grelha sai em inglês.
    #[test]
    fn a_grelha_fala_portugues() {
        let html = pagina(&[], dia(2026, 8, 26));
        for palavra in ["August", "Monday", "Mon", "Sunday", "Sun", "Tue", "Wed"] {
            assert!(
                !html.contains(&format!(">{palavra}<")),
                "«{palavra}» apareceu na grelha"
            );
        }
        assert!(
            html.contains("Agosto 2026"),
            "o título do mês não está em português"
        );
    }
}

#[cfg(test)]
mod vistas_temporais {
    use super::*;

    /// A zona destes testes.
    ///
    /// Explícita, e não «a do sistema»: um teste que dependesse do fuso da
    /// máquina passaria aqui e falharia em CI, ou ao contrário.
    fn zona_de_teste() -> TimeZoneName {
        "UTC".to_owned().try_into().expect("fuso conhecido")
    }

    fn dia(a: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(a, m, d).expect("data válida")
    }

    fn evento(titulo: &str, quando: NaiveDate, h: u32, min: u32, dura: i64) -> Item {
        use chrono::TimeZone;
        let inicio = Utc
            .with_ymd_and_hms(quando.year(), quando.month(), quando.day(), h, min, 0)
            .single()
            .expect("instante");
        Item {
            kind: "event".to_owned(),
            id: format!("id-{titulo}"),
            title: titulo.to_owned(),
            all_day: false,
            starts_at: Some(inicio),
            ends_at: Some(inicio + Duration::minutes(dura)),
            timezone: Some("UTC".to_owned()),
            starts_on: None,
            ends_before: None,
            state: "confirmed".to_owned(),
            classification: "INTERNAL".to_owned(),
        }
    }

    fn render(view: CalendarView, items: &[Item], anchor: NaiveDate) -> String {
        calendar(&CalendarPage {
            view,
            anchor,
            items,
            may_create: true,
            failure: None,
            zona: zona_de_teste(),
        })
        .to_html()
    }

    /// A Semana tem sete dias, em português, e na ordem certa.
    #[test]
    fn a_semana_tem_sete_colunas_portuguesas() {
        let html = render(CalendarView::Week, &[], dia(2026, 8, 26));

        assert_eq!(
            html.matches("oc-cal-cabeca\"").count() + html.matches("oc-cal-cabeca ").count(),
            7,
            "a semana não tem sete cabeçalhos"
        );
        for (i, nome) in crate::ui::tempo::cabecalhos_da_semana().iter().enumerate() {
            assert!(
                html.contains(&format!(">{nome}<")),
                "falta a coluna «{nome}» ({i})"
            );
        }
        for ingles in ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"] {
            assert!(
                !html.contains(&format!(">{ingles}<")),
                "«{ingles}» na semana"
            );
        }
    }

    /// O eixo tem as vinte e quatro horas. Esconder a madrugada esconderia
    /// o turno de quem trabalha nela.
    #[test]
    fn o_eixo_tem_o_dia_inteiro() {
        let html = render(CalendarView::Week, &[], dia(2026, 8, 26));
        for h in 0..24u32 {
            assert!(
                html.contains(&format!(">{h:02}:00<")),
                "falta a hora {h:02}:00 no eixo"
            );
        }
    }

    /// Uma actividade ocupa a faixa da sua hora e a altura da sua duração.
    #[test]
    fn o_bloco_ocupa_o_tempo_que_a_actividade_ocupa() {
        let quando = dia(2026, 8, 26);
        let items = [evento("Conselho", quando, 9, 0, 90)];
        let html = render(CalendarView::Day, &items, quando);

        // 09:00 é a faixa 18 de quarenta e oito; 90 minutos são três faixas.
        assert!(
            html.contains("oc-cal-l18"),
            "as 09:00 não estão na faixa 18"
        );
        assert!(
            html.contains("oc-cal-f3"),
            "90 minutos não ocupam três faixas"
        );
    }

    /// Actividades ao mesmo tempo dividem a largura, e nenhuma desaparece.
    #[test]
    fn as_sobreposicoes_dividem_a_largura() {
        let quando = dia(2026, 8, 26);
        let items = [
            evento("Primeira", quando, 10, 0, 60),
            evento("Segunda", quando, 10, 30, 60),
            evento("Terceira", quando, 10, 0, 30),
        ];
        let html = render(CalendarView::Day, &items, quando);

        for titulo in ["Primeira", "Segunda", "Terceira"] {
            assert!(
                html.contains(titulo),
                "«{titulo}» desapareceu da sobreposição"
            );
        }
        assert!(
            html.contains("de3") || html.contains("de2"),
            "as actividades simultâneas não dividiram a largura"
        );
        // E cada uma numa coluna própria: duas na mesma coluna sobrepunham-se.
        assert!(html.contains("oc-cal-c1de"), "falta a primeira coluna");
        assert!(html.contains("oc-cal-c2de"), "falta a segunda coluna");
    }

    /// O Ano tem doze meses, de Janeiro a Dezembro.
    #[test]
    fn o_ano_tem_doze_meses() {
        let html = render(CalendarView::Year, &[], dia(2026, 6, 15));

        assert_eq!(
            html.matches("oc-cal-mini\"").count(),
            12,
            "o ano não tem doze meses"
        );
        for m in 1..=12u32 {
            let nome = crate::ui::tempo::mes(dia(2026, m, 1));
            assert!(html.contains(&format!(">{nome}<")), "falta {nome}");
        }
        // E na ordem: Janeiro antes de Dezembro.
        let janeiro = html.find(">Janeiro<").expect("Janeiro");
        let dezembro = html.find(">Dezembro<").expect("Dezembro");
        assert!(janeiro < dezembro, "os meses estão fora de ordem");
    }

    /// Fevereiro de um ano bissexto tem vinte e nove.
    #[test]
    fn o_ano_bissexto_tem_o_dia_a_mais() {
        let html = render(CalendarView::Year, &[], dia(2028, 1, 1));
        let fevereiro = html
            .split(">Fevereiro<")
            .nth(1)
            .and_then(|p| p.split("oc-cal-mini\"").next())
            .unwrap_or_default();
        assert!(
            fevereiro.contains(r#"on=2028-02-29""#),
            "Fevereiro de 2028 não tem o dia 29"
        );
    }

    /// A Agenda agrupa por dia, em português, sem concatenar.
    #[test]
    fn a_agenda_agrupa_por_dia() {
        let items = [
            evento("Primeira", dia(2026, 8, 26), 9, 0, 60),
            evento("Segunda", dia(2026, 8, 26), 11, 0, 60),
            evento("Terceira", dia(2026, 8, 28), 15, 0, 60),
        ];
        let html = render(CalendarView::Agenda, &items, dia(2026, 8, 26));

        assert_eq!(
            html.matches("oc-cal-grupo\"").count() + html.matches("oc-cal-grupo ").count(),
            2,
            "três actividades em dois dias não deram dois grupos"
        );
        assert!(
            html.contains("Quarta-feira"),
            "a agenda não diz o dia em português"
        );
        for ingles in ["Wednesday", "Friday", "August"] {
            assert!(!html.contains(ingles), "«{ingles}» na agenda");
        }
    }

    /// Uma agenda vazia di-lo. É a única vista que pode.
    #[test]
    fn a_agenda_vazia_diz_que_esta_vazia() {
        let html = render(CalendarView::Agenda, &[], dia(2026, 8, 26));
        assert!(html.contains("Nenhuma actividade para este período"));

        // E as outras não o dizem: desenham-se.
        for vista in [CalendarView::Month, CalendarView::Week, CalendarView::Day] {
            let html = render(vista, &[], dia(2026, 8, 26));
            assert!(
                !html.contains("Nenhuma actividade para este período"),
                "{vista:?} substituiu a estrutura por uma frase"
            );
        }
    }

    /// O calendário da barra mostra o mês e não a agenda.
    #[test]
    fn o_calendario_do_sistema_nao_mostra_actividades() {
        let hoje = dia(2026, 8, 26);
        let html = system_calendar(hoje).to_html();

        assert!(html.contains("Agosto 2026"), "não diz em que mês estamos");
        assert!(html.contains("Quarta-feira"), "não diz que dia da semana é");
        assert!(html.contains("oc-datepop__dia-cel--hoje"), "não marca hoje");
        assert!(html.contains("Abrir Calendário"), "não abre o Calendário");

        // E abre-o por um endereço que o catálogo de rotas declara.
        //
        // # Porque a comparação é contra o catálogo e não contra a constante
        //
        // Comparar `CALENDAR_ROUTE` consigo próprio prova que uma constante é
        // igual a si mesma. O que interessa é que o endereço que sai daqui seja
        // um dos que a aplicação serve: um `/calendario` escrito à mão compila,
        // renderiza, e leva a pessoa a um 404.
        // A ordem dos atributos no render não é a da fonte: procura-se o
        // elemento, e dentro dele o endereço.
        let elemento = html
            .split("<a ")
            .find(|p| p.contains("oc-datepop__abrir"))
            .unwrap_or_default();
        let destino = elemento
            .split(r#"href=""#)
            .nth(1)
            .and_then(|p| p.split('"').next())
            .unwrap_or_default();
        assert!(
            crate::routes::ROUTES.contains(&destino),
            "«{destino}» não está no catálogo de rotas"
        );

        // 42 células, e nenhuma actividade: a grelha sai da data, não do domínio.
        assert_eq!(
            html.matches("oc-datepop__dia-cel").count()
                - html.matches("oc-datepop__dia-cel--").count(),
            42
        );
        for marca in ["oc-cal-item", "oc-cal-bloco", "oc-cal-linha"] {
            assert!(
                !html.contains(marca),
                "o painel trouxe «{marca}»: leu a agenda"
            );
        }
    }

    /// A navegação do Ano anda em anos.
    #[test]
    fn a_navegacao_do_ano_anda_em_anos() {
        let d = dia(2026, 8, 26);
        assert_eq!(step(CalendarView::Year, d, true).year(), 2027);
        assert_eq!(step(CalendarView::Year, d, false).year(), 2025);
    }

    /// As cinco vistas estão no selector, e `Hoje` não é uma delas.
    #[test]
    fn o_selector_tem_as_cinco_vistas() {
        let rotulos: Vec<&str> = CalendarView::all().iter().map(|v| v.label()).collect();
        assert_eq!(rotulos, vec!["Dia", "Semana", "Mês", "Ano", "Agenda"]);
        assert!(!rotulos.contains(&"Hoje"));
    }
}

#[cfg(test)]
mod editor_de_actividade {
    use super::*;

    /// A zona destes testes.
    ///
    /// Explícita, e não «a do sistema»: um teste que dependesse do fuso da
    /// máquina passaria aqui e falharia em CI, ou ao contrário.
    fn zona_de_teste() -> TimeZoneName {
        "UTC".to_owned().try_into().expect("fuso conhecido")
    }

    fn render(units: &Value, workspaces: &Value) -> String {
        event_form(
            None,
            units,
            workspaces,
            None,
            None,
            &pessoas(),
            zona_de_teste(),
        )
        .to_html()
    }

    fn pessoas() -> Value {
        serde_json::json!([
            {"id": "11111111-1111-1111-1111-111111111111", "display_name": "Ana Mucai"},
            {"id": "22222222-2222-2222-2222-222222222222", "display_name": "Carlos Neto"},
        ])
    }

    fn lista(nomes: &[&str]) -> Value {
        serde_json::json!(nomes
            .iter()
            .enumerate()
            .map(|(i, n)| serde_json::json!({"id": format!("id-{i}"), "name": n}))
            .collect::<Vec<_>>())
    }

    /// Um âmbito sem destino não se oferece.
    ///
    /// # Porque isto era um defeito visível
    ///
    /// O editor mostrava «Unidade» e «Ambiente de investigação» ao mesmo tempo,
    /// os dois com listas vazias, fosse qual fosse o âmbito escolhido. Oferecia
    /// escolhas que não levavam a lado nenhum e obrigava a lê-las para descobrir
    /// que não interessavam.
    #[test]
    fn um_ambito_sem_destino_nao_aparece() {
        let vazio = serde_json::json!([]);
        let html = render(&vazio, &vazio);

        assert!(
            html.contains(r#"value="personal""#),
            "falta o âmbito pessoal"
        );
        assert!(
            !html.contains(r#"value="unit""#),
            "«Unidade» é oferecida sem existir nenhuma"
        );
        assert!(
            !html.contains(r#"value="research_workspace""#),
            "«Ambiente» é oferecido sem existir nenhum"
        );
        assert!(
            !html.contains(r#"name="unit_id""#),
            "o selector de unidade está lá sem unidades"
        );
    }

    /// Com destinos, aparecem — e escondidos até serem escolhidos.
    #[test]
    fn os_selectores_comecam_escondidos() {
        let html = render(&lista(&["Unidade A"]), &lista(&["Ambiente B"]));

        assert!(
            html.contains(r#"value="unit""#),
            "«Unidade» não é oferecida"
        );
        assert!(
            html.contains(r#"name="unit_id""#),
            "falta o selector de unidade"
        );

        // O campo existe e está escondido: é o âmbito que o revela.
        let campo = html
            .split("<label ")
            .find(|p| p.contains(r#"data-oc="unit-field""#))
            .unwrap_or_default();
        assert!(
            campo.contains("hidden"),
            "o selector de unidade aparece antes de alguém escolher «Unidade»"
        );
    }

    /// A zona é contexto, não uma caixa onde se escreve o que se quiser.
    ///
    /// Era um campo de texto livre com `UTC` lá dentro — a representação do
    /// armazenamento apresentada como se fosse a zona de quem marca. O valor
    /// continua a ir no pedido e continua a ser o Core a validá-lo.
    #[test]
    fn a_zona_nao_e_uma_caixa_de_texto() {
        let vazio = serde_json::json!([]);
        let html = render(&vazio, &vazio);

        let campo = html
            .split("<input ")
            .find(|p| p.contains(r#"name="timezone""#))
            .unwrap_or_default();
        assert!(
            campo.contains(r#"type="hidden""#),
            "a zona horária voltou a ser um campo que se escreve à mão"
        );
        assert!(
            html.contains(r#"data-oc="timezone-label""#),
            "a zona deixou de ser mostrada como contexto"
        );
    }

    /// A acção diz o que faz.
    #[test]
    fn a_accao_principal_diz_o_que_faz() {
        let vazio = serde_json::json!([]);
        let html = render(&vazio, &vazio);
        assert!(
            html.contains("Criar actividade"),
            "a acção não diz o que faz"
        );
        assert!(!html.contains(">Marcar<"), "«Marcar» voltou");
        assert!(html.contains("Cancelar"), "não há como desistir");
    }

    /// O conteúdo que a pessoa escreve é texto, e nunca marcação.
    #[test]
    fn o_editor_nao_escreve_marcacao() {
        let hostil = "<script>window.__x=1</script>";
        let item = Item {
            kind: "event".to_owned(),
            id: "id".to_owned(),
            title: hostil.to_owned(),
            all_day: false,
            starts_at: None,
            ends_at: None,
            timezone: None,
            starts_on: None,
            ends_before: None,
            state: "confirmed".to_owned(),
            classification: "INTERNAL".to_owned(),
        };
        let vazio = serde_json::json!([]);
        let html = event_form(
            Some(&item),
            &vazio,
            &vazio,
            None,
            None,
            &serde_json::Value::Null,
            zona_de_teste(),
        )
        .to_html();

        assert!(
            !html.contains("<script>window.__x"),
            "o título hostil virou marcação no editor"
        );
        assert!(
            html.contains("&lt;script&gt;"),
            "o título hostil desapareceu em vez de aparecer como texto"
        );
    }
}

#[cfg(test)]
mod horario_e_participantes {
    use super::*;

    /// A zona destes testes.
    ///
    /// Explícita, e não «a do sistema»: um teste que dependesse do fuso da
    /// máquina passaria aqui e falharia em CI, ou ao contrário.
    fn zona_de_teste() -> TimeZoneName {
        "UTC".to_owned().try_into().expect("fuso conhecido")
    }

    fn quando(a: i32, m: u32, d: u32, h: u32, min: u32) -> chrono::NaiveDateTime {
        NaiveDate::from_ymd_opt(a, m, d)
            .expect("data")
            .and_hms_opt(h, min, 0)
            .expect("hora")
    }

    fn pessoas() -> Value {
        serde_json::json!([
            {"id": "11111111-1111-1111-1111-111111111111", "display_name": "Ana Mucai"},
            {"id": "22222222-2222-2222-2222-222222222222", "display_name": "Carlos Neto"},
        ])
    }

    fn editor(proposto: Option<chrono::NaiveDateTime>, gente: &Value) -> String {
        let vazio = serde_json::json!([]);
        event_form(None, &vazio, &vazio, None, proposto, gente, zona_de_teste()).to_html()
    }

    /// O editor abre com um horário que se aceita sem pensar.
    ///
    /// # O que isto substitui
    ///
    /// Dois campos vazios — `dd/mm/yyyy, --:--` — e quatro decisões antes de
    /// escrever o título. Marcar uma reunião banal custava mais do que a reunião.
    #[test]
    fn o_editor_abre_com_horario_proposto() {
        let html = editor(Some(quando(2026, 8, 26, 19, 30)), &pessoas());

        assert!(
            html.contains(r#"value="2026-08-26T19:30""#),
            "o início não vem preenchido"
        );
        assert!(
            html.contains(r#"value="2026-08-26T20:00""#),
            "o fim não vem meia hora depois do início"
        );
    }

    /// Sem contexto nenhum, os campos ficam vazios em vez de inventarem uma data.
    #[test]
    fn sem_proposta_os_campos_ficam_vazios() {
        let html = editor(None, &pessoas());
        assert!(
            !html.contains("T19:30"),
            "apareceu um horário que ninguém propôs"
        );
    }

    /// A duração proposta é de meia hora, e vem de um sítio só.
    #[test]
    fn a_duracao_proposta_vem_da_primitiva() {
        assert_eq!(crate::ui::tempo::DURACAO_PADRAO_MINUTOS, 30);

        for (h, m) in [(9, 0), (13, 30), (23, 30)] {
            let inicio = quando(2026, 8, 26, h, m);
            let html = editor(Some(inicio), &pessoas());
            let fim = inicio + Duration::minutes(crate::ui::tempo::DURACAO_PADRAO_MINUTOS);
            assert!(
                html.contains(&format!(r#"value="{}""#, crate::ui::tempo::para_campo(fim))),
                "às {h:02}:{m:02} o fim proposto não é o da primitiva"
            );
        }
    }

    /// A secção de participantes oferece o universo autorizado.
    #[test]
    fn os_participantes_vem_do_universo_autorizado() {
        let html = editor(None, &pessoas());

        assert!(html.contains("Participantes"), "falta a secção");
        assert!(html.contains("Ana Mucai") && html.contains("Carlos Neto"));
        assert!(
            html.contains(r#"data-oc="procura-pessoa""#),
            "não há como procurar uma pessoa"
        );
        assert!(
            html.contains("Ninguém corresponde a essa procura"),
            "uma procura sem resultados não diz nada"
        );
    }

    /// Sem ninguém para convidar, a secção não aparece.
    ///
    /// Uma procura sobre um universo vazio é um controlo que não faz nada.
    #[test]
    fn sem_universo_nao_ha_seccao_de_participantes() {
        let html = editor(None, &serde_json::json!([]));
        assert!(!html.contains(r#"data-oc="participantes""#));
    }

    /// Cada pessoa traz a sua referência institucional, e não o seu nome.
    ///
    /// Um nome escrito à mão deixa de ser a pessoa e passa a ser uma etiqueta
    /// que ninguém pode autorizar nem notificar.
    #[test]
    fn cada_participante_viaja_por_identificador() {
        let html = editor(None, &pessoas());
        assert!(
            html.contains(r#"data-id="11111111-1111-1111-1111-111111111111""#),
            "o participante não traz o seu identificador"
        );
    }
}

#[cfg(test)]
mod precedencia_do_horario {
    use super::*;

    fn dia(a: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(a, m, d).expect("data")
    }

    fn hora(h: u32, m: u32) -> chrono::NaiveTime {
        chrono::NaiveTime::from_hms_opt(h, m, 0).expect("hora")
    }

    /// Uma hora escolhida numa faixa da Semana é a hora, tal e qual.
    #[test]
    fn a_hora_explicita_vence_tudo() {
        let escolhido = horario_do_editor(
            Some(dia(2026, 8, 28)),
            Some(hora(14, 0)),
            dia(2026, 8, 26).and_time(hora(19, 7)),
        );
        assert_eq!(escolhido, dia(2026, 8, 28).and_time(hora(14, 0)));
    }

    /// Um dia escolhido no Mês mantém-se; a hora é a que a política propõe.
    ///
    /// Este é o caso que a reversão apanha: substituir o dia escolhido pelo de
    /// hoje descarta a única coisa que a pessoa já tinha dito.
    #[test]
    fn o_dia_escolhido_nao_e_substituido_por_hoje() {
        let hoje = dia(2026, 8, 26).and_time(hora(19, 7));
        let escolhido = horario_do_editor(Some(dia(2026, 8, 28)), None, hoje);

        assert_eq!(
            escolhido.date(),
            dia(2026, 8, 28),
            "o dia escolhido foi substituído pelo de hoje"
        );
        assert_eq!(
            escolhido.time(),
            hora(19, 30),
            "a hora não veio da política"
        );
    }

    /// Sem contexto nenhum, é agora — arredondado.
    #[test]
    fn sem_contexto_e_agora() {
        let agora = dia(2026, 8, 26).and_time(hora(19, 7));
        assert_eq!(
            horario_do_editor(None, None, agora),
            dia(2026, 8, 26).and_time(hora(19, 30))
        );
    }

    /// Uma hora sem dia não inventa um dia: é agora.
    #[test]
    fn uma_hora_sem_dia_nao_basta() {
        let agora = dia(2026, 8, 26).and_time(hora(19, 7));
        assert_eq!(
            horario_do_editor(None, Some(hora(14, 0)), agora),
            dia(2026, 8, 26).and_time(hora(19, 30))
        );
    }
}
