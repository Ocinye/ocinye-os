//! D004 · Calendário: a agenda do Core (`GET /calendar/agenda`), posta em mês,
//! semana, dia e agenda **no fuso do membro**.
//!
//! Uma só chamada por vista, com o intervalo local convertido para UTC; as
//! posições (minuto, duração, coluna de sobreposição) calculam-se aqui, e a
//! vista só as escreve. Um evento que atravessa a meia-noite local aparece em
//! cada dia que toca. Os dias inteiros contam-se pela data local (o Core
//! filtra-os pela data UTC dos limites, por isso refiltram-se aqui).

use chrono::{DateTime, Datelike, Duration, NaiveDate, NaiveDateTime, Timelike, Utc};
use serde_json::Value;

use crate::controllers::desktop::{instant, text, Clock};
use crate::i18n::{t, tf};
use crate::ui::view_models::{CalDayVm, CalEventVm, CalFormVm, CalScope, CalView};
use ocinye_contracts::temporal::{resolve_local, TimeZoneName};

/// A vista pedida (`?view=`), com a agenda como omissão no móvel (a vista
/// decide por CSS; aqui a omissão é o mês).
#[must_use]
pub fn view_of(v: Option<&str>) -> CalView {
    match v {
        Some("week") => CalView::Week,
        Some("day") => CalView::Day,
        Some("agenda") => CalView::Agenda,
        _ => CalView::Month,
    }
}

const fn view_param(v: CalView) -> &'static str {
    match v {
        CalView::Month => "month",
        CalView::Week => "week",
        CalView::Day => "day",
        CalView::Agenda => "agenda",
    }
}

/// O dia local de hoje e o dia pedido (`?date=AAAA-MM-DD`).
#[must_use]
pub(crate) fn anchor(date: Option<&str>, clock: &Clock) -> (NaiveDate, NaiveDate) {
    let today = clock.now.with_timezone(&clock.zone.zone()).date_naive();
    let day = date
        .and_then(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").ok())
        .unwrap_or(today);
    (today, day)
}

fn monday_of(d: NaiveDate) -> NaiveDate {
    d - Duration::days(i64::from(d.weekday().num_days_from_monday()))
}

fn first_of_month(d: NaiveDate) -> NaiveDate {
    d.with_day(1).unwrap_or(d)
}

fn next_month(d: NaiveDate) -> NaiveDate {
    let f = first_of_month(d);
    if f.month() == 12 {
        NaiveDate::from_ymd_opt(f.year() + 1, 1, 1).unwrap_or(f)
    } else {
        NaiveDate::from_ymd_opt(f.year(), f.month() + 1, 1).unwrap_or(f)
    }
}

fn prev_month(d: NaiveDate) -> NaiveDate {
    let f = first_of_month(d);
    if f.month() == 1 {
        NaiveDate::from_ymd_opt(f.year() - 1, 12, 1).unwrap_or(f)
    } else {
        NaiveDate::from_ymd_opt(f.year(), f.month() - 1, 1).unwrap_or(f)
    }
}

/// Os dias locais que a vista mostra (primeiro, número de dias).
#[must_use]
pub fn days_of(view: CalView, day: NaiveDate) -> (NaiveDate, i64) {
    match view {
        CalView::Month => {
            let first = first_of_month(day);
            let start = monday_of(first);
            let last = next_month(day) - Duration::days(1);
            let end = monday_of(last) + Duration::days(7);
            (start, (end - start).num_days())
        }
        CalView::Week => (monday_of(day), 7),
        CalView::Day => (day, 1),
        CalView::Agenda => (day, 14),
    }
}

/// A meia-noite local de um dia, em UTC (a que existir, perto de uma mudança
/// de hora).
#[must_use]
pub fn local_midnight(d: NaiveDate, zone: TimeZoneName) -> DateTime<Utc> {
    for h in 0..3 {
        if let Ok(at) = resolve_local(d.and_hms_opt(h, 0, 0).unwrap_or_default(), zone) {
            return at;
        }
    }
    Utc::now()
}

/// O caminho da agenda do Core para os dias da vista.
#[must_use]
pub fn agenda_path(start: NaiveDate, days: i64, zone: TimeZoneName) -> String {
    let from = local_midnight(start, zone);
    let to = local_midnight(start + Duration::days(days), zone);
    format!(
        "/api/v1/calendar/agenda?from={}&to={}",
        from.to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
        to.to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
    )
}

fn scope_of(s: &str) -> CalScope {
    match s {
        "unit" => CalScope::Unit,
        "research_workspace" => CalScope::Workspace,
        "institution" => CalScope::Institution,
        _ => CalScope::Personal,
    }
}

/// «seg», «Mon», «lun»: o nome curto do dia da semana, do nome longo.
fn weekday_short(d: NaiveDate) -> String {
    let long = t(&format!("wm.wd.{}.long", d.weekday().number_from_monday()));
    let short: String = long.chars().take(3).collect();
    let mut c = short.chars();
    c.next()
        .map(|f| f.to_uppercase().collect::<String>() + c.as_str())
        .unwrap_or_default()
}

fn month_long(m: u32) -> &'static str {
    t(&format!("wm.month.{m}"))
}

fn month_short(m: u32) -> String {
    month_long(m).chars().take(3).collect()
}

/// «terça-feira, 29 de setembro», «Tuesday, 29 September», «mardi 29 septembre».
#[must_use]
pub fn full_date(d: NaiveDate) -> String {
    let wd = t(&format!("wm.wd.{}.long", d.weekday().number_from_monday()));
    let m = month_long(d.month());
    match crate::i18n::current() {
        ocinye_contracts::Locale::En => format!("{wd}, {} {m}", d.day()),
        ocinye_contracts::Locale::Fr => format!("{wd} {} {m}", d.day()),
        _ => format!("{wd}, {} de {m}", d.day()),
    }
}

/// O título do intervalo.
#[must_use]
pub fn range_label(view: CalView, day: NaiveDate) -> String {
    match view {
        CalView::Month => tf(
            "wm.clock.month",
            &[
                ("month", month_long(day.month())),
                ("year", &day.year().to_string()),
            ],
        ),
        CalView::Week | CalView::Agenda => {
            let (start, n) = days_of(view, day);
            let end = start + Duration::days(n - 1);
            format!(
                "{} {} – {} {}",
                start.day(),
                month_short(start.month()),
                end.day(),
                month_short(end.month())
            )
        }
        CalView::Day => full_date(day),
    }
}

/// Anterior, seguinte e hoje, mantendo a vista.
#[must_use]
pub fn nav_hrefs(view: CalView, day: NaiveDate, today: NaiveDate) -> (String, String, String) {
    let (prev, next) = match view {
        CalView::Month => (prev_month(day), next_month(day)),
        CalView::Week => (day - Duration::days(7), day + Duration::days(7)),
        CalView::Day => (day - Duration::days(1), day + Duration::days(1)),
        CalView::Agenda => (day - Duration::days(14), day + Duration::days(14)),
    };
    let v = view_param(view);
    let href = |d: NaiveDate| format!("/calendar?view={v}&date={}", d.format("%Y-%m-%d"));
    (href(prev), href(next), href(today))
}

/// Um item da agenda do Core, já no fuso do membro.
struct Item {
    id: String,
    title: String,
    event: bool,
    all_day: bool,
    cancelled: bool,
    scope: CalScope,
    location: Option<String>,
    /// Dias inteiros: [início, fim) em datas locais.
    dates: Option<(NaiveDate, NaiveDate)>,
    /// Com hora: início e fim locais.
    local: Option<(NaiveDateTime, NaiveDateTime)>,
    origin_tz: Option<String>,
}

fn parse(item: &Value, clock: &Clock) -> Option<Item> {
    let z = clock.zone.zone();
    let all_day = item
        .get("all_day")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let event = text(item, "kind") == "event";
    let (dates, local, origin_tz) = if all_day {
        let s = NaiveDate::parse_from_str(text(item, "starts_on"), "%Y-%m-%d").ok()?;
        let e = NaiveDate::parse_from_str(text(item, "ends_before"), "%Y-%m-%d").ok()?;
        (Some((s, e)), None, None)
    } else {
        let s = instant(item, "starts_at")?;
        let e = instant(item, "ends_at")?;
        let tz = text(item, "timezone");
        let origin = (!tz.is_empty() && tz != clock.zone.as_str())
            .then(|| {
                TimeZoneName::parse(tz)
                    .ok()
                    .map(|o| format!("{tz} · {}", s.with_timezone(&o.zone()).format("%H:%M")))
            })
            .flatten();
        (
            None,
            Some((
                s.with_timezone(&z).naive_local(),
                e.with_timezone(&z).naive_local(),
            )),
            origin,
        )
    };
    let scope = if event {
        scope_of(text(item, "scope"))
    } else {
        CalScope::Workspace
    };
    Some(Item {
        id: text(item, "id").to_owned(),
        title: text(item, "title").to_owned(),
        event,
        all_day,
        cancelled: text(item, "state") == "cancelled",
        scope,
        location: item
            .get("location")
            .and_then(Value::as_str)
            .filter(|l| !l.is_empty())
            .map(str::to_owned),
        dates,
        local,
        origin_tz,
    })
}

fn minutes(t: NaiveDateTime) -> u16 {
    u16::try_from(t.hour() * 60 + t.minute()).unwrap_or(0)
}

/// O evento como aparece num dia (com a parte desse dia, para os com hora).
fn on_day(it: &Item, d: NaiveDate, ctx: &str) -> Option<CalEventVm> {
    let href = if it.event {
        format!("/calendar/events/{}?{ctx}", it.id)
    } else {
        "/my-work".to_owned()
    };
    let base = |time: String, span: Option<(u16, u16)>| CalEventVm {
        id: it.id.clone(),
        title: it.title.clone(),
        time,
        location: it.location.clone(),
        scope: it.scope,
        cancelled: it.cancelled,
        all_day: it.all_day,
        span,
        lane: (0, 1),
        href: href.clone(),
        origin_tz: it.origin_tz.clone(),
    };
    if let Some((s, e)) = it.dates {
        return (s <= d && d < e).then(|| base(t("cal.all_day").to_owned(), None));
    }
    let (s, e) = it.local?;
    let d0 = d.and_hms_opt(0, 0, 0)?;
    let d1 = d0 + Duration::days(1);
    if e <= d0 || s >= d1 {
        return None;
    }
    let from = s.max(d0);
    let to = e.min(d1);
    let start = minutes(from);
    let dur = u16::try_from((to - from).num_minutes().clamp(1, 1440)).unwrap_or(60);
    let time = format!("{}–{}", s.format("%H:%M"), e.format("%H:%M"));
    Some(base(time, Some((start, dur))))
}

/// As colunas de sobreposição de um dia: cada grupo de eventos que se tocam
/// reparte a largura.
fn lanes(events: &mut [CalEventVm]) {
    let mut idx: Vec<usize> = (0..events.len())
        .filter(|&i| !events[i].all_day && events[i].span.is_some())
        .collect();
    idx.sort_by_key(|&i| events[i].span.map_or(0, |s| s.0));
    let mut cluster: Vec<usize> = Vec::new();
    let mut ends: Vec<u16> = Vec::new();
    let mut cluster_end = 0u16;
    let flush = |events: &mut [CalEventVm], cluster: &mut Vec<usize>, ends: &mut Vec<u16>| {
        let n = u8::try_from(ends.len().max(1)).unwrap_or(1);
        for &i in cluster.iter() {
            events[i].lane.1 = n;
        }
        cluster.clear();
        ends.clear();
    };
    for i in idx {
        let (start, dur) = events[i].span.unwrap_or((0, 0));
        if !cluster.is_empty() && start >= cluster_end {
            flush(events, &mut cluster, &mut ends);
        }
        let lane = ends.iter().position(|&e| e <= start).unwrap_or(ends.len());
        if lane == ends.len() {
            ends.push(start + dur);
        } else {
            ends[lane] = start + dur;
        }
        events[i].lane.0 = u8::try_from(lane).unwrap_or(0);
        cluster.push(i);
        cluster_end = cluster_end.max(start + dur);
        if cluster.len() == 1 {
            cluster_end = start + dur;
        }
    }
    flush(events, &mut cluster, &mut ends);
}

/// A grelha, as colunas ou a agenda da vista.
pub(crate) struct Laid {
    pub(crate) days: Vec<CalDayVm>,
    pub(crate) agenda: Vec<(String, Vec<CalEventVm>)>,
    pub(crate) weekdays: Vec<String>,
    pub(crate) now_minute: Option<u16>,
}

/// Põe os itens da agenda do Core na vista.
#[must_use]
pub(crate) fn lay_out(
    view: CalView,
    day: NaiveDate,
    today: NaiveDate,
    items: &[Value],
    clock: &Clock,
) -> Laid {
    let parsed: Vec<Item> = items.iter().filter_map(|i| parse(i, clock)).collect();
    let (start, n) = days_of(view, day);
    let ctx = format!("view={}&date={}", view_param(view), day.format("%Y-%m-%d"));
    let mut days = Vec::new();
    let mut agenda = Vec::new();
    for k in 0..n {
        let d = start + Duration::days(k);
        let mut evs: Vec<CalEventVm> = parsed.iter().filter_map(|it| on_day(it, d, &ctx)).collect();
        evs.sort_by_key(|e| (!e.all_day, e.span.map_or(0, |s| s.0)));
        lanes(&mut evs);
        if view == CalView::Agenda {
            agenda.push((full_date(d), evs));
            continue;
        }
        let (shown, more) = if view == CalView::Month && evs.len() > 3 {
            let more = u16::try_from(evs.len() - 3).unwrap_or(0);
            (evs.into_iter().take(3).collect(), more)
        } else {
            (evs, 0)
        };
        days.push(CalDayVm {
            label: if view == CalView::Month {
                d.day().to_string()
            } else {
                format!("{} {}", weekday_short(d), d.day())
            },
            full: full_date(d),
            today: d == today,
            outside: view == CalView::Month && d.month() != day.month(),
            selected: d == day && view != CalView::Month,
            events: shown,
            more,
            href: format!("/calendar?view=day&date={}", d.format("%Y-%m-%d")),
            new_href: Some(format!(
                "/calendar/events/new?date={}",
                d.format("%Y-%m-%d")
            )),
        });
    }
    let weekdays = (0..7)
        .map(|k| weekday_short(monday_of(day) + Duration::days(k)))
        .collect();
    let now_minute = (matches!(view, CalView::Week | CalView::Day) && days.iter().any(|d| d.today))
        .then(|| minutes(clock.now.with_timezone(&clock.zone.zone()).naive_local()));
    Laid {
        days,
        agenda,
        weekdays,
        now_minute,
    }
}

/// O formulário de criação (vazio, no dia pedido) ou de edição (do detalhe do
/// Core), com as datas no fuso do membro. Só o âmbito pessoal se oferece para
/// criar: o Core não diz em que âmbitos o membro pode criar.
#[must_use]
pub(crate) fn form(detail: Option<&Value>, day: NaiveDate, clock: &Clock) -> CalFormVm {
    let personal = (
        "personal".to_owned(),
        t("prod.cal.scope.personal").to_owned(),
    );
    let Some(e) = detail else {
        let s = day.and_hms_opt(9, 0, 0).unwrap_or_default();
        return CalFormVm {
            action: "/calendar/events/new".to_owned(),
            title: String::new(),
            start: s.format("%Y-%m-%dT%H:%M").to_string(),
            end: (s + Duration::hours(1))
                .format("%Y-%m-%dT%H:%M")
                .to_string(),
            all_day: false,
            location: String::new(),
            description: String::new(),
            scopes: vec![personal],
            scope: "personal".to_owned(),
            error: None,
        };
    };
    let id = text(e, "id");
    let all_day = e.get("all_day").and_then(Value::as_bool).unwrap_or(false);
    let z = clock.zone.zone();
    let (start, end) = if all_day {
        let s = text(e, "starts_on").to_owned();
        let last = NaiveDate::parse_from_str(text(e, "ends_before"), "%Y-%m-%d")
            .map(|d| (d - Duration::days(1)).format("%Y-%m-%d").to_string())
            .unwrap_or_else(|_| s.clone());
        (s, last)
    } else {
        let f = |k| {
            instant(e, k)
                .map(|at| at.with_timezone(&z).format("%Y-%m-%dT%H:%M").to_string())
                .unwrap_or_default()
        };
        (f("starts_at"), f("ends_at"))
    };
    let scope = text(e, "scope").to_owned();
    CalFormVm {
        action: format!("/calendar/events/{id}/edit"),
        title: text(e, "title").to_owned(),
        start,
        end,
        all_day,
        location: e
            .get("location")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_owned(),
        description: e
            .get("description")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_owned(),
        // O âmbito de um evento não muda depois de criado (o Core não o aceita).
        scopes: vec![(
            scope.clone(),
            t(&format!("prod.cal.scope.{scope}")).to_owned(),
        )],
        scope,
        error: None,
    }
}

/// O que o formulário do Design envia, validado, como a ocorrência do Core:
/// com hora, no fuso do membro; de dia inteiro, com o último dia inclusivo.
///
/// # Errors
///
/// A chave de catálogo do erro de validação.
pub(crate) fn occurrence(
    all_day: bool,
    start: &str,
    end: &str,
    zone: TimeZoneName,
) -> Result<Value, &'static str> {
    if all_day {
        let first = NaiveDate::parse_from_str(start.get(..10).unwrap_or(start), "%Y-%m-%d")
            .map_err(|_| "prod.cal.err.dates")?;
        let last =
            NaiveDate::parse_from_str(end.get(..10).unwrap_or(end), "%Y-%m-%d").unwrap_or(first);
        if last < first {
            return Err("prod.cal.err.order");
        }
        return Ok(serde_json::json!({
            "kind": "all_day",
            "starts_on": first,
            "ends_before": last + Duration::days(1),
        }));
    }
    let parse = |v: &str| NaiveDateTime::parse_from_str(v, "%Y-%m-%dT%H:%M").ok();
    let (Some(s), Some(e)) = (parse(start), parse(end)) else {
        return Err("prod.cal.err.dates");
    };
    if e <= s {
        return Err("prod.cal.err.order");
    }
    Ok(serde_json::json!({
        "kind": "timed",
        "starts_at": s.format("%Y-%m-%dT%H:%M:00").to_string(),
        "ends_at": e.format("%Y-%m-%dT%H:%M:00").to_string(),
        "timezone": zone.as_str(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use serde_json::json;

    fn clock() -> Clock {
        Clock {
            now: Utc.with_ymd_and_hms(2026, 9, 29, 10, 30, 0).unwrap(),
            zone: TimeZoneName::parse("Africa/Luanda").unwrap(),
            core_ok: true,
            is_admin: false,
        }
    }

    fn d(y: i32, m: u32, dd: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, dd).unwrap()
    }

    #[test]
    fn o_mes_comeca_a_segunda_e_fecha_semanas_inteiras() {
        let (start, n) = days_of(CalView::Month, d(2026, 9, 15));
        assert_eq!(start, d(2026, 8, 31));
        assert_eq!(n % 7, 0);
        assert!(n == 35 || n == 42);
        assert_eq!(days_of(CalView::Week, d(2026, 10, 1)), (d(2026, 9, 28), 7));
    }

    #[test]
    fn a_agenda_do_core_pede_se_em_utc_a_partir_da_meia_noite_local() {
        let p = agenda_path(
            d(2026, 9, 29),
            1,
            TimeZoneName::parse("Africa/Luanda").unwrap(),
        );
        assert!(
            p.contains("from=2026-09-28T23:00:00Z") && p.contains("to=2026-09-29T23:00:00Z"),
            "{p}"
        );
    }

    #[test]
    fn um_evento_fica_no_dia_e_na_hora_locais() {
        let c = clock();
        // 08:00–09:00 UTC = 09:00–10:00 em Luanda.
        let items = vec![json!({
            "kind": "event", "id": "e1", "title": "Reunião", "all_day": false,
            "starts_at": "2026-09-29T08:00:00Z", "ends_at": "2026-09-29T09:00:00Z",
            "timezone": "Europe/Lisbon", "state": "scheduled", "scope": "unit", "location": "Sala 2"
        })];
        let l = lay_out(CalView::Day, d(2026, 9, 29), d(2026, 9, 29), &items, &c);
        let e = &l.days[0].events[0];
        assert_eq!(e.span, Some((540, 60)));
        assert_eq!(e.time, "09:00–10:00");
        assert_eq!(e.scope, CalScope::Unit);
        assert_eq!(e.origin_tz.as_deref(), Some("Europe/Lisbon · 09:00"));
        assert_eq!(l.now_minute, Some(11 * 60 + 30));
    }

    #[test]
    fn a_meia_noite_local_parte_o_evento_nos_dois_dias() {
        let c = clock();
        let items = vec![json!({
            "kind": "event", "id": "e1", "title": "Noite", "all_day": false,
            "starts_at": "2026-09-29T21:00:00Z", "ends_at": "2026-09-30T01:00:00Z",
            "timezone": "Africa/Luanda", "state": "scheduled", "scope": "personal"
        })];
        let l = lay_out(CalView::Week, d(2026, 9, 29), d(2026, 9, 29), &items, &c);
        let ter = &l.days[1].events[0];
        let qua = &l.days[2].events[0];
        assert_eq!(ter.span, Some((22 * 60, 120)));
        assert_eq!(qua.span, Some((0, 120)));
    }

    #[test]
    fn os_sobrepostos_repartem_a_largura() {
        let c = clock();
        let ev = |id: &str, s: &str, e: &str| {
            json!({
                "kind": "event", "id": id, "title": id, "all_day": false,
                "starts_at": s, "ends_at": e, "timezone": "Africa/Luanda", "state": "scheduled", "scope": "personal"
            })
        };
        let items = vec![
            ev("a", "2026-09-29T08:00:00Z", "2026-09-29T10:00:00Z"),
            ev("b", "2026-09-29T09:00:00Z", "2026-09-29T11:00:00Z"),
            ev("c", "2026-09-29T13:00:00Z", "2026-09-29T14:00:00Z"),
        ];
        let l = lay_out(CalView::Day, d(2026, 9, 29), d(2026, 9, 29), &items, &c);
        let lanes: Vec<(u8, u8)> = l.days[0].events.iter().map(|e| e.lane).collect();
        assert_eq!(lanes, vec![(0, 2), (1, 2), (0, 1)]);
    }

    #[test]
    fn o_dia_inteiro_conta_se_pela_data_local() {
        let c = clock();
        let items = vec![json!({
            "kind": "event", "id": "e1", "title": "Feriado", "all_day": true,
            "starts_on": "2026-09-30", "ends_before": "2026-10-01", "state": "scheduled", "scope": "institution"
        })];
        let l = lay_out(CalView::Week, d(2026, 9, 29), d(2026, 9, 29), &items, &c);
        assert!(l.days[1].events.is_empty());
        assert_eq!(l.days[2].events[0].title, "Feriado");
    }

    #[test]
    fn o_formulario_valida_e_usa_o_fuso_do_membro() {
        let z = TimeZoneName::parse("Africa/Luanda").unwrap();
        let o = occurrence(false, "2026-09-29T09:00", "2026-09-29T10:00", z).unwrap();
        assert_eq!(o["timezone"], "Africa/Luanda");
        assert_eq!(
            occurrence(false, "2026-09-29T10:00", "2026-09-29T09:00", z),
            Err("prod.cal.err.order")
        );
        let o = occurrence(true, "2026-09-29", "2026-09-30", z).unwrap();
        assert_eq!(o["ends_before"], "2026-10-01");
    }
}
