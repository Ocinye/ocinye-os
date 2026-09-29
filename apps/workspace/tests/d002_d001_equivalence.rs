//! Guarda constitucional da D002: sem gestor de janelas (`wm: None`) e sem
//! painéis (`TopPanels::default()`), a casca, o Desktop, `app_pending` e as
//! páginas de erro são a D001.2.1.
//!
//! As referências em `tests/golden/d001_2_1/` foram geradas por este mesmo
//! ficheiro na árvore da D001.2.1 (`main` @ `ca5cbde`, árvore igual a
//! `e579d3c`), com `OCINYE_TEST_D001_GOLDEN_WRITE=1`. Aqui a saída da D002 é
//! comparada byte a byte depois de retirar só as três adições que a D002
//! declara neutras para a D001 (HANDOFF «D001_COMPONENT_EXTENSION»):
//!
//! 1. `oc-wm.css` e `oc-wm.js` no documento da casca;
//! 2. `data-app` em cada aplicação fixada na barra de aplicações;
//! 3. o menu de contexto do Desktop (`data-oc="desk-ctx"`, `hidden`) no fim de `home`.
//!
//! A D003 declara mais uma adição ao documento da casca: `oc-nye.css` e
//! `oc-nye.js` (a superfície da Nye). Com `nye: None` a marcação é a mesma; o
//! CSS carregado prova-se sem fuga no browser, contra a D002.1 a correr.
//!
//! Os dois lados passam por [`canonical`]: atributos por ordem alfabética e sem
//! os marcadores `<!>`, que não mudam o DOM que o CSS e o JS vêem.
//!
//! Qualquer outra diferença falha e é `D002_D001_REGRESSION`.

use std::path::PathBuf;

use ocinye_workspace::ui::document;
use ocinye_workspace::ui::screens::{error, home, home::registry};
use ocinye_workspace::ui::shell;
use ocinye_workspace::ui::view_models::{
    Ago, AppTile, Backup, ContinueItem, ContinueKind, CoreError, Count, DefaultSource, DeskWidget,
    DesktopDefault, DesktopVm, Distribution, DocumentVm, DoorVm, ErrorKind, ErrorVm, Health,
    HealthVm, Load, Metric, ShellVm, StorageUse, Surface, Theme, Wallpaper, WidgetContent,
    WidgetItem, WidgetKind,
};

fn tile(id: &'static str, href: &'static str, label: &str, pinned: bool, active: bool) -> AppTile {
    AppTile {
        id,
        href,
        label: label.into(),
        description: format!("{label} · descrição"),
        pinned,
        pinnable: true,
        active,
    }
}

fn shell_vm(d: Distribution, unread: Option<u32>, core: Health, ai: Option<Health>) -> ShellVm {
    ShellVm {
        display_name: "Fidel Monteiro".into(),
        email: "fidel@ocinye.test".into(),
        apps: vec![
            tile("files", "/files", "Ficheiros", true, false),
            tile("notes", "/notes", "Notas", true, true),
            tile("mail", "/mail", "Correio", true, false),
            tile("calendar", "/calendar", "Calendário", false, false),
            tile("tasks", "/tasks", "Tarefas", false, false),
        ],
        unread,
        core: Some(core),
        ai,
        query: String::new(),
        distribution: Some(d),
        crumb: "Desktop".into(),
        wallpaper: Wallpaper::Ocinye,
        dim: 20,
        ..Default::default()
    }
}

fn items(n: usize, base: &str) -> Vec<WidgetItem> {
    (1..=n)
        .map(|i| WidgetItem {
            title: format!("{base} {i}"),
            meta: format!("{i:02}/10"),
            href: format!("/{}/{i}", base.to_lowercase()),
        })
        .collect()
}

/// O conteúdo de cada tipo, variando os estados de `Load` para os cobrir todos.
fn content(kind: WidgetKind, i: usize) -> WidgetContent {
    use WidgetKind as K;
    let failed = || CoreError {
        reference: format!("OC-{i:08}"),
    };
    match kind {
        K::Kpis => WidgetContent::Metrics(Load::Ready(vec![
            Metric {
                icon: "units",
                label: "Unidades".into(),
                value: "4".into(),
                qualifier: "activas".into(),
                href: "/units".into(),
            },
            Metric {
                icon: "ideas",
                label: "Ideias".into(),
                value: "12".into(),
                qualifier: "em curso".into(),
                href: "/ideas".into(),
            },
            Metric {
                icon: "projects",
                label: "Projectos".into(),
                value: "3".into(),
                qualifier: "em curso".into(),
                href: "/projects".into(),
            },
            Metric {
                icon: "datasets",
                label: "Datasets".into(),
                value: "9".into(),
                qualifier: "".into(),
                href: "/datasets".into(),
            },
        ])),
        K::Continue => WidgetContent::Continue(Load::Ready(vec![
            ContinueItem {
                kind: ContinueKind::Note,
                title: "Notas de campo".into(),
                href: "/notes/n1".into(),
                progress: None,
                when: Ago::Minutes(5),
            },
            ContinueItem {
                kind: ContinueKind::File,
                title: "Relatório Q3.pdf".into(),
                href: "/files/f1".into(),
                progress: Some(40),
                when: Ago::Date("27/09".into()),
            },
        ])),
        K::Health => WidgetContent::Health(Load::Ready(HealthVm {
            state: Health::Operational,
            nodes_up: 3,
            nodes_total: 4,
            backup: Backup::Unknown,
            admin_href: Some("/admin/monitor".into()),
        })),
        K::Storage => WidgetContent::Storage(Load::Ready(StorageUse {
            used: "12 GB".into(),
            total: "50 GB".into(),
            percent: 24,
        })),
        K::Ideas | K::Datasets => WidgetContent::Count(Load::Ready(Count {
            value: "12".into(),
            qualifier: "em investigação".into(),
        })),
        K::Notice => WidgetContent::List(Load::Unavailable),
        K::Mail => WidgetContent::List(Load::Failed(failed())),
        K::Activity => WidgetContent::List(Load::Empty),
        K::Projects => WidgetContent::List(Load::Denied),
        K::Files => WidgetContent::List(Load::Loading),
        _ => WidgetContent::List(Load::Ready(items(3, kind.as_str()))),
    }
}

fn desktop(d: Distribution, admin: bool, can: bool, source: DefaultSource) -> DesktopVm {
    let mut placed = registry::system_default(d);
    // Todos os tipos do registo aparecem pelo menos uma vez.
    for spec in registry::KINDS {
        if !placed.iter().any(|p| p.kind == spec.kind) {
            let (w, h) = spec.sizes[0];
            placed.push(ocinye_workspace::ui::view_models::PlacedWidget {
                id: spec.kind.as_str().into(),
                kind: spec.kind,
                w,
                h,
                minimized: spec.kind == WidgetKind::Activity,
            });
        }
    }
    DesktopVm {
        shell: shell_vm(d, Some(3), Health::Operational, Some(Health::Operational)),
        version: 7,
        widgets: placed
            .into_iter()
            .enumerate()
            .map(|(i, p)| DeskWidget {
                content: content(p.kind, i),
                placed: p,
            })
            .collect(),
        default: Some(DesktopDefault {
            source,
            name: if source == DefaultSource::Instance {
                "Predefinição da Instância".into()
            } else {
                String::new()
            },
            version: 1,
            published: if source == DefaultSource::Instance {
                "27/09/2026".into()
            } else {
                String::new()
            },
            wallpaper: Wallpaper::Ocinye,
            dim: 20,
            widgets: registry::system_default(d),
        }),
        base_version: None,
        is_admin: admin,
        can_customise: can,
    }
}

fn doc(surface: Surface) -> DocumentVm {
    DocumentVm {
        title: "Ocinye OS".into(),
        surface,
        theme: Theme::Light,
    }
}

/// Cada cena: nome e o documento completo.
fn scenes() -> Vec<(String, String)> {
    let mut out = Vec::new();
    for d in [
        Distribution::Research,
        Distribution::Business,
        Distribution::Personal,
        Distribution::Education,
    ] {
        for (admin, can, source, tag) in [
            (false, true, DefaultSource::System, "member"),
            (true, true, DefaultSource::Instance, "admin"),
            (false, false, DefaultSource::System, "policy"),
        ] {
            let vm = desktop(d, admin, can, source);
            out.push((
                format!("desktop-{}-{tag}", d.as_str()),
                document::render(&doc(Surface::Shell), home::home(&vm)),
            ));
        }
    }
    for (core, ai, unread, tag) in [
        (Health::Operational, Some(Health::Operational), None, "ok"),
        (
            Health::Operational,
            Some(Health::Unavailable),
            Some(0),
            "no-ai",
        ),
        (Health::Degraded, None, Some(12), "degraded"),
        (
            Health::Unavailable,
            Some(Health::Degraded),
            Some(120),
            "down",
        ),
    ] {
        let vm = shell_vm(Distribution::Research, unread, core, ai);
        out.push((
            format!("app-pending-{tag}"),
            document::render(
                &doc(Surface::Shell),
                shell::app_pending(&vm, "Ficheiros".into(), "/files"),
            ),
        ));
    }
    let vm = shell_vm(Distribution::Research, Some(2), Health::Operational, None);
    for kind in [
        ErrorKind::NotFound,
        ErrorKind::Forbidden,
        ErrorKind::Upstream,
    ] {
        let e = ErrorVm {
            kind,
            reference: (kind == ErrorKind::Upstream).then(|| "OC-1A2B3C4D".into()),
            retry_href: (kind == ErrorKind::Upstream).then(|| "/files".into()),
        };
        out.push((
            format!("error-shell-{}", kind.code()),
            document::render(&doc(Surface::Shell), error::in_shell(&vm, &e)),
        ));
        out.push((
            format!("error-door-{}", kind.code()),
            document::render(&doc(Surface::Auth), error::at_door(&DoorVm::default(), &e)),
        ));
    }
    out
}

fn golden_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/golden/d001_2_1")
}

/// Retira a etiqueta inteira que contém `needle` (`<link …>` ou `<script …></script>`).
fn drop_tag(html: &str, needle: &str) -> String {
    let Some(at) = html.find(needle) else {
        return html.to_owned();
    };
    let start = html[..at].rfind('<').expect("início da etiqueta");
    let mut end = at + html[at..].find('>').expect("fim da etiqueta") + 1;
    if html[start..].starts_with("<script") && html[end..].starts_with("</script>") {
        end += "</script>".len();
    }
    format!("{}{}", &html[..start], &html[end..])
}

/// Os atributos de uma etiqueta de abertura, pela ordem do texto. `None` se
/// `tag` não for uma etiqueta de abertura (`</x>`, `<!>`, `<!-- -->`).
fn attrs(tag: &str) -> Option<(&str, Vec<&str>)> {
    let inner = tag.strip_prefix('<')?.strip_suffix('>')?;
    if !inner.starts_with(|c: char| c.is_ascii_alphabetic()) {
        return None;
    }
    let inner = inner.strip_suffix('/').unwrap_or(inner).trim_end();
    let name_end = inner.find(' ').unwrap_or(inner.len());
    let (name, mut rest) = inner.split_at(name_end);
    let mut out = Vec::new();
    loop {
        rest = rest.trim_start();
        if rest.is_empty() {
            break;
        }
        let key_end = rest.find(['=', ' ']).unwrap_or(rest.len());
        if rest[key_end..].starts_with("=\"") {
            let close = key_end + 2 + rest[key_end + 2..].find('"').expect("aspas") + 1;
            out.push(&rest[..close]);
            rest = &rest[close..];
        } else {
            out.push(&rest[..key_end]);
            rest = &rest[key_end..];
        }
    }
    Some((name, out))
}

/// Reescreve cada etiqueta de abertura com os atributos por ordem alfabética
/// e retira os marcadores `<!>` do Leptos. Em HTML a ordem dos atributos não
/// tem significado (o Leptos muda-a consoante o atributo é estático ou vem de
/// um fecho), e `<!>` é um comentário vazio: não conta para `:empty`, para os
/// selectores de irmãos nem para a disposição.
fn canonical(html: &str) -> String {
    let html = html.replace("<!>", "");
    let mut out = String::with_capacity(html.len());
    let mut rest = html.as_str();
    while let Some(i) = rest.find('<') {
        out.push_str(&rest[..i]);
        let end = i + rest[i..].find('>').expect("fim de etiqueta") + 1;
        let tag = &rest[i..end];
        match attrs(tag) {
            Some((name, mut list)) => {
                list.sort_unstable();
                out.push('<');
                out.push_str(name);
                for a in list {
                    out.push(' ');
                    out.push_str(a);
                }
                out.push('>');
            }
            None => out.push_str(tag),
        }
        rest = &rest[end..];
    }
    out.push_str(rest);
    out
}

/// Retira `data-app` das ligações da barra de aplicações (`class="oc-dock__btn"`).
fn drop_dock_data_app(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut rest = html;
    while let Some(i) = rest.find("<a ") {
        out.push_str(&rest[..i]);
        let end = i + rest[i..].find('>').expect("fim de <a>") + 1;
        let tag = &rest[i..end];
        let (_, list) = attrs(tag).expect("<a> é uma etiqueta de abertura");
        if list.contains(&r#"class="oc-dock__btn""#) {
            out.push_str("<a");
            for a in list.iter().filter(|a| !a.starts_with("data-app=")) {
                out.push(' ');
                out.push_str(a);
            }
            out.push('>');
        } else {
            out.push_str(tag);
        }
        rest = &rest[end..];
    }
    out.push_str(rest);
    out
}

/// Retira o menu de contexto do Desktop (um `<div>` sem `<div>` dentro).
fn drop_desk_ctx(html: &str) -> String {
    let Some(at) = html
        .find(r#"data-oc="desk-ctx""#)
        .map(|i| html[..i].rfind("<div").unwrap())
    else {
        return html.to_owned();
    };
    let end = at + html[at..].find("</div>").expect("fim do menu") + "</div>".len();
    assert!(
        !html[at + 4..end].contains("<div"),
        "o menu passou a ter <div> dentro"
    );
    format!("{}{}", &html[..at], &html[end..])
}

fn d001_view(d002: &str) -> String {
    let s = drop_tag(d002, r#"href="/static/oc-wm.css""#);
    let s = drop_tag(&s, r#"src="/static/oc-wm.js""#);
    let s = drop_tag(&s, r#"href="/static/oc-nye.css""#);
    let s = drop_tag(&s, r#"src="/static/oc-nye.js""#);
    canonical(&drop_desk_ctx(&drop_dock_data_app(&s)))
}

#[test]
fn sem_janelas_nem_paineis_a_d002_e_a_d001_2_1() {
    let dir = golden_dir();
    if std::env::var_os("OCINYE_TEST_D001_GOLDEN_WRITE").is_some() {
        std::fs::create_dir_all(&dir).unwrap();
        for (name, html) in scenes() {
            std::fs::write(dir.join(format!("{name}.html")), html).unwrap();
        }
        return;
    }
    let scenes = scenes();
    assert_eq!(
        scenes.len(),
        22,
        "as cenas mudaram sem regenerar as referências"
    );
    let mut compared = 0;
    for (name, html) in scenes {
        let golden = std::fs::read_to_string(dir.join(format!("{name}.html")))
            .unwrap_or_else(|e| panic!("referência D001.2.1 em falta para {name}: {e}"));
        let now = d001_view(&html);
        let golden = canonical(&golden);
        if now != golden {
            let at = now
                .bytes()
                .zip(golden.bytes())
                .position(|(a, b)| a != b)
                .unwrap_or(now.len().min(golden.len()));
            let from = at.saturating_sub(120);
            panic!(
                "D002_D001_REGRESSION em {name} (byte {at}):\n  D001.2.1: …{}…\n  D002:     …{}…",
                &golden[from..(at + 160).min(golden.len())],
                &now[from..(at + 160).min(now.len())]
            );
        }
        compared += 1;
    }
    assert_eq!(compared, 22);
}

/// As três adições existem mesmo: sem isto, a normalização podia estar a
/// esconder uma diferença que nunca apareceu.
#[test]
fn as_adicoes_retiradas_estao_mesmo_la() {
    let all = scenes();
    let desk = &all
        .iter()
        .find(|(n, _)| n == "desktop-research-member")
        .unwrap()
        .1;
    assert!(desk.contains("/static/oc-wm.css") && desk.contains("/static/oc-wm.js"));
    assert!(desk.contains("/static/oc-nye.css") && desk.contains("/static/oc-nye.js"));
    assert!(desk.contains(r#"data-oc="desk-ctx""#));
    assert!(desk.contains(r#"<a href="/files" aria-label="Ficheiros" title="Ficheiros" data-app="files" class="oc-dock__btn">"#));
    let door = &all.iter().find(|(n, _)| n == "error-door-404").unwrap().1;
    assert!(
        !door.contains("oc-wm"),
        "a porta não carrega o gestor de janelas"
    );
}
