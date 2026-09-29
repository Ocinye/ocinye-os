//! D004 · Calendário. Mês, semana, dia e agenda, no fuso do membro.
//!
//! As posições (minuto de início, duração, coluna de sobreposição) chegam
//! calculadas pelo Code no fuso do membro; a vista escreve-as em `data-*` e o
//! `oc-apps.js` aplica-as como propriedades CSS (CSP: sem `style` na marcação).
//! Sem JS, os eventos ficam em lista dentro do dia. Arrastar para reagendar
//! não existe: editar é o formulário. Cores só pelos âmbitos do Core.

use leptos::prelude::*;

use super::{empty, frame, load_state, nye};
use crate::i18n::t;
use crate::ui::components::icon;
use crate::ui::view_models::{
    CalDayVm, CalEventDetailsVm, CalEventVm, CalFormVm, CalScope, CalView, CalendarVm,
};

const fn scope(s: CalScope) -> &'static str {
    match s {
        CalScope::Personal => "personal",
        CalScope::Unit => "unit",
        CalScope::Workspace => "workspace",
        CalScope::Institution => "institution",
    }
}

fn chip(e: &CalEventVm) -> impl IntoView {
    view! {
        <a class="oc-cal-ev" href=e.href.clone() data-scope=scope(e.scope) data-cancelled=e.cancelled.then_some("") data-all-day=e.all_day.then_some("")>
            {(!e.all_day).then(|| view! { <span class="oc-cal-ev__time">{e.time.clone()}</span> })}
            <span class="oc-cal-ev__title">{e.title.clone()}</span>
            {e.cancelled.then(|| view! { <span class="oc-sr">{" · "}{t("cal.cancelled")}</span> })}
        </a>
    }
}

fn block(e: &CalEventVm) -> impl IntoView {
    let (start, dur) = e.span.unwrap_or((0, 60));
    view! {
        <a class="oc-cal-blk" href=e.href.clone() data-scope=scope(e.scope) data-cancelled=e.cancelled.then_some("")
            data-start=start.to_string() data-dur=dur.to_string() data-lane=e.lane.0.to_string() data-lanes=e.lane.1.max(1).to_string() data-part="cal-block">
            <span class="oc-cal-blk__title">{e.title.clone()}</span>
            <span class="oc-cal-blk__time">{e.time.clone()}{e.location.clone().map(|l| format!(" · {l}"))}</span>
            {e.cancelled.then(|| view! { <span class="oc-sr">{" · "}{t("cal.cancelled")}</span> })}
        </a>
    }
}

fn month(vm: &CalendarVm) -> impl IntoView {
    view! {
        <div class="oc-cal-month" role="grid" aria-label=vm.range_label.clone() data-part="cal-grid">
            <div class="oc-cal-month__head" role="row">
                {vm.weekdays.iter().map(|d| view! { <span role="columnheader">{d.clone()}</span> }).collect_view()}
            </div>
            <div class="oc-cal-month__days">
                {vm.days.chunks(7).map(|week| view! {
                    <div class="oc-cal-month__row" role="row">
                        {week.iter().map(|d: &CalDayVm| view! {
                            <div class="oc-cal-day" role="gridcell" data-today=d.today.then_some("") data-outside=d.outside.then_some("") aria-selected=if d.selected { "true" } else { "false" }>
                                <a class="oc-cal-day__n" href=d.href.clone() aria-label=d.full.clone() aria-current=d.today.then_some("date")>{d.label.clone()}</a>
                                <div class="oc-cal-day__evs">
                                    {d.events.iter().map(chip).collect_view()}
                                    {(d.more > 0).then(|| view! { <a class="oc-cal-more" href=d.href.clone()>{format!("+{}", d.more)}</a> })}
                                </div>
                            </div>
                        }).collect_view()}
                    </div>
                }).collect_view()}
            </div>
        </div>
    }
}

fn timeline(vm: &CalendarVm) -> impl IntoView {
    let (h0, h1) = vm.hours;
    let hours: Vec<u8> = (h0..h1).collect();
    let any_all_day = vm.days.iter().any(|d| d.events.iter().any(|e| e.all_day));
    view! {
        <div class="oc-cal-tl" data-part="cal-tl" data-h0=h0.to_string() data-h1=h1.to_string() data-cols=vm.days.len().to_string()>
            <div class="oc-cal-tl__head">
                <span class="oc-cal-tl__gutter" aria-hidden="true"></span>
                {vm.days.iter().map(|d| view! {
                    <a class="oc-cal-tl__day" href=d.href.clone() data-today=d.today.then_some("") aria-label=d.full.clone() aria-current=d.today.then_some("date")>{d.label.clone()}</a>
                }).collect_view()}
            </div>
            {any_all_day.then(|| view! {
                <div class="oc-cal-tl__allday">
                    <span class="oc-cal-tl__gutter">{t("cal.all_day")}</span>
                    {vm.days.iter().map(|d| view! { <div class="oc-cal-tl__adcol">{d.events.iter().filter(|e| e.all_day).map(chip).collect_view()}</div> }).collect_view()}
                </div>
            })}
            <div class="oc-cal-tl__scroll" data-part="cal-scroll">
                <div class="oc-cal-tl__grid">
                    <div class="oc-cal-tl__hours" aria-hidden="true">
                        {hours.iter().map(|h| view! { <span>{format!("{h:02}:00")}</span> }).collect_view()}
                    </div>
                    {vm.days.iter().map(|d| view! {
                        <div class="oc-cal-tl__col" data-today=d.today.then_some("") aria-label=d.full.clone() role="group">
                            {d.events.iter().filter(|e| !e.all_day).map(block).collect_view()}
                            {(d.today).then_some(vm.now_minute).flatten().map(|m| view! { <span class="oc-cal-now" data-part="cal-now" data-start=m.to_string() aria-hidden="true"></span> })}
                        </div>
                    }).collect_view()}
                </div>
            </div>
        </div>
    }
}

fn agenda(vm: &CalendarVm) -> AnyView {
    if vm.agenda.iter().all(|(_, e)| e.is_empty()) {
        return empty(
            "calendar",
            "cal.empty.range",
            vm.new_href.is_some().then_some("cal.empty.hint"),
        )
        .into_any();
    }
    view! {
        <ol class="oc-cal-agenda">
            {vm.agenda.iter().filter(|(_, es)| !es.is_empty()).map(|(date, es)| view! {
                <li class="oc-cal-agenda__day">
                    <h3 class="oc-cal-agenda__date">{date.clone()}</h3>
                    <ul>{es.iter().map(|e| view! {
                        <li>
                            <a class="oc-cal-row" href=e.href.clone() data-scope=scope(e.scope) data-cancelled=e.cancelled.then_some("")>
                                <span class="oc-cal-row__time">{e.time.clone()}</span>
                                <span class="oc-cal-row__title">{e.title.clone()}</span>
                                {e.location.clone().map(|l| view! { <span class="oc-cal-row__loc">{l}</span> })}
                            </a>
                        </li>
                    }).collect_view()}</ul>
                </li>
            }).collect_view()}
        </ol>
    }
    .into_any()
}

fn details(d: &CalEventDetailsVm) -> impl IntoView {
    let e = &d.event;
    let kv = |k: &'static str, v: String| view! { <div class="oc-app-kv"><dt>{t(k)}</dt><dd>{v}</dd></div> };
    view! {
        <div class="oc-app-insp" data-part="cal-details">
            <header class="oc-app-insp__head">
                <span class="oc-cal-dot" data-scope=scope(e.scope) aria-hidden="true"></span>
                <h2 class="oc-app-insp__title">{e.title.clone()}</h2>
                <a class="oc-app__icon" href="?" data-oc="app-insp-close" aria-label=t("app.details.close")>{icon("close")}</a>
            </header>
            {e.cancelled.then(|| view! { <p class="oc-app-note" data-tone="warn">{icon("close")}<span>{t("cal.cancelled")}</span></p> })}
            <dl class="oc-app-kvs">
                {kv("cal.when", format!("{} · {}", d.date, e.time))}
                {e.origin_tz.clone().map(|z| kv("cal.origin_tz", z))}
                {e.location.clone().map(|l| kv("cal.location", l))}
                {d.context.clone().map(|c| kv("cal.context", c))}
                {(!d.participants.is_empty()).then(|| kv("cal.participants", d.participants.join(", ")))}
            </dl>
            {d.description.clone().map(|x| view! { <p class="oc-cal-desc">{x}</p> })}
            <div class="oc-app-insp__actions">
                {d.edit_href.clone().map(|h| view! { <a class="oc-app-btn" href=h>{icon("edit")}{t("cal.edit")}</a> })}
                {d.cancel_action.clone().map(|a| view! { <form method="post" action=a><button type="submit" class="oc-app-btn oc-app-btn--danger">{t("cal.cancel_event")}</button></form> })}
                {d.nye.as_ref().map(nye)}
            </div>
        </div>
    }
}

fn form(f: &CalFormVm, tz: Option<String>) -> impl IntoView {
    view! {
        <form class="oc-app-insp oc-cal-form" method="post" action=f.action.clone() data-oc="app-doc" data-state="clean" aria-labelledby="oc-cal-form-t">
            <header class="oc-app-insp__head">
                <h2 class="oc-app-insp__title" id="oc-cal-form-t">{t(if f.title.is_empty() { "cal.new" } else { "cal.edit" })}</h2>
                <a class="oc-app__icon" href="?" data-oc="app-insp-close" aria-label=t("app.details.close")>{icon("close")}</a>
            </header>
            {f.error.map(|k| view! { <p class="oc-app-note" data-tone="warn" role="alert">{icon("warning")}<span>{t(k)}</span></p> })}
            <label class="oc-app-field"><span>{t("cal.f.title")}</span><input name="title" value=f.title.clone() required="" autocomplete="off" /></label>
            <label class="oc-app-check"><input type="checkbox" name="all_day" checked=f.all_day data-oc="cal-allday" /><span>{t("cal.all_day")}</span></label>
            <div class="oc-app-row2">
                <label class="oc-app-field"><span>{t("cal.f.start")}</span><input type=if f.all_day { "date" } else { "datetime-local" } name="start" value=f.start.clone() required="" data-part="cal-dt" /></label>
                <label class="oc-app-field"><span>{t("cal.f.end")}</span><input type=if f.all_day { "date" } else { "datetime-local" } name="end" value=f.end.clone() required="" data-part="cal-dt" /></label>
            </div>
            {tz.map(|z| view! { <p class="oc-app-note">{icon("clock")}<span>{z}</span></p> })}
            <label class="oc-app-field"><span>{t("cal.location")}</span><input name="location" value=f.location.clone() autocomplete="off" /></label>
            <label class="oc-app-field"><span>{t("cal.f.scope")}</span>
                <select name="scope">{f.scopes.iter().map(|(v, l)| view! { <option value=v.clone() selected={*v == f.scope}>{l.clone()}</option> }).collect_view()}</select>
            </label>
            <label class="oc-app-field"><span>{t("cal.f.description")}</span><textarea name="description" rows="3">{crate::text::rcdata(&f.description)}</textarea></label>
            <div class="oc-app-insp__actions">
                <a class="oc-app-btn" href="?">{t("app.cancel")}</a>
                <span class="oc-app__spacer"></span>
                <button type="submit" class="oc-app-primary">{icon("check")}<span>{t("cal.save")}</span></button>
            </div>
        </form>
    }
}

/// A aplicação Calendário.
pub fn app(vm: &CalendarVm) -> AnyView {
    let tab = |v: CalView, href: &'static str, key: &'static str| {
        view! { <a class="oc-app-seg__opt" href=href aria-current=(vm.view == v).then_some("page")>{t(key)}</a> }
    };
    let toolbar = view! {
        <a class="oc-app-btn" href=vm.today_href.clone()>{t("cal.today")}</a>
        <a class="oc-app__icon" href=vm.prev_href.clone() aria-label=t("cal.prev")>{icon("chev-l")}</a>
        <a class="oc-app__icon" href=vm.next_href.clone() aria-label=t("cal.next")>{icon("chev-r")}</a>
        <h2 class="oc-app__title" aria-live="polite">{vm.range_label.clone()}</h2>
        {vm.timezone.clone().map(|z| view! { <span class="oc-cal-tz" title=t("cal.tz")>{icon("clock")}{z}</span> })}
        <span class="oc-app__spacer"></span>
        <nav class="oc-app-seg" aria-label=t("cal.views")>
            {tab(CalView::Month, "?view=month", "cal.view.month")}
            {tab(CalView::Week, "?view=week", "cal.view.week")}
            {tab(CalView::Day, "?view=day", "cal.view.day")}
            {tab(CalView::Agenda, "?view=agenda", "cal.view.agenda")}
        </nav>
        {vm.new_href.clone().map(|h| view! { <a class="oc-app-primary" href=h>{icon("plus")}<span>{t("cal.new")}</span></a> })}
    }
    .into_any();
    let main = load_state(vm.load, 6).unwrap_or_else(|| match vm.view {
        CalView::Month => month(vm).into_any(),
        CalView::Week | CalView::Day => {
            let none = vm.days.iter().all(|d| d.events.is_empty());
            view! {
                {timeline(vm)}
                {(none && vm.view == CalView::Day).then(|| view! { <p class="oc-app-note oc-cal-emptyday">{icon("calendar")}<span>{t("cal.empty.day")}</span></p> })}
            }
            .into_any()
        }
        CalView::Agenda => agenda(vm),
    });
    let view_attr = match vm.view {
        CalView::Month => "month",
        CalView::Week => "week",
        CalView::Day => "day",
        CalView::Agenda => "agenda",
    };
    let insp = match (&vm.form, &vm.details) {
        (Some(f), _) => Some(form(f, vm.timezone.clone()).into_any()),
        (None, Some(d)) => Some(details(d).into_any()),
        _ => None,
    };
    let main = view! { <div class="oc-cal" data-view=view_attr>{main}</div> }.into_any();
    frame(
        "calendar",
        t("nav.calendar").to_owned(),
        toolbar,
        None,
        main,
        insp,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::testing::assert_contracts;
    use crate::ui::view_models::AppLoad;

    fn ev(title: &str, span: (u16, u16), lane: (u8, u8)) -> CalEventVm {
        CalEventVm {
            id: title.into(),
            title: title.into(),
            time: "09:00–10:00".into(),
            location: None,
            scope: CalScope::Unit,
            cancelled: false,
            all_day: false,
            span: Some(span),
            lane,
            href: "/calendar/events/x".into(),
            origin_tz: None,
        }
    }

    fn vm(view: CalView) -> CalendarVm {
        CalendarVm {
            view,
            range_label: "Ter 29 set".into(),
            prev_href: "?d=28".into(),
            next_href: "?d=30".into(),
            today_href: "?".into(),
            weekdays: vec![],
            days: vec![CalDayVm {
                label: "Ter 29".into(),
                full: "terça-feira, 29 de setembro".into(),
                today: true,
                outside: false,
                selected: true,
                events: vec![ev("A", (540, 60), (0, 2)), ev("B", (570, 60), (1, 2))],
                more: 0,
                href: "?d=29".into(),
                new_href: None,
            }],
            agenda: vec![],
            now_minute: Some(642),
            hours: (7, 20),
            timezone: Some("Africa/Luanda".into()),
            load: AppLoad::Ready,
            details: None,
            form: None,
            new_href: Some("/calendar/events/new".into()),
        }
    }

    #[test]
    fn sobreposicoes_e_hora_actual_sem_style_inline() {
        let html = app(&vm(CalView::Day)).to_html();
        assert_contracts(&html);
        assert!(
            html.contains(r#"data-lane="1" data-lanes="2""#)
                && html.contains(r#"data-part="cal-now""#)
        );
        assert!(!html.contains("style="));
        assert!(html.contains("Africa/Luanda"));
    }

    #[test]
    fn agenda_vazia_diz_o_que_falta() {
        let mut v = vm(CalView::Agenda);
        v.days = vec![];
        assert!(app(&v).to_html().contains(t("cal.empty.range")));
    }
}
