//! O registo de widgets do Desktop, as predefinições do sistema por
//! distribuição e a comparação com a predefinição publicada. Decisão do Design.
//!
//! A hierarquia: predefinição do sistema (aqui) → predefinição da distribuição
//! publicada pelo administrador → disposição do membro. O Core valida um `PUT`
//! contra [`KINDS`]: tamanho permitido, obrigatórios presentes, só
//! administradores em `admin_only`.

use crate::ui::view_models::{Distribution, PlacedWidget, WidgetKind};

/// A categoria na biblioteca de widgets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Category {
    /// Produtividade.
    Productivity,
    /// Investigação.
    Research,
    /// Comunicação.
    Communication,
    /// Ficheiros.
    Files,
    /// Sistema.
    System,
    /// Organização.
    Organisation,
}

impl Category {
    /// Todas, pela ordem da biblioteca.
    pub const ALL: [Self; 6] = [
        Self::Productivity,
        Self::Research,
        Self::Communication,
        Self::Files,
        Self::System,
        Self::Organisation,
    ];

    /// O identificador em `data-cat`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Productivity => "prod",
            Self::Research => "res",
            Self::Communication => "comm",
            Self::Files => "files",
            Self::System => "sys",
            Self::Organisation => "org",
        }
    }

    /// A chave do rótulo.
    #[must_use]
    pub const fn label_key(self) -> &'static str {
        match self {
            Self::Productivity => "desk.cat.prod",
            Self::Research => "desk.cat.res",
            Self::Communication => "desk.cat.comm",
            Self::Files => "desk.cat.files",
            Self::System => "desk.cat.sys",
            Self::Organisation => "desk.cat.org",
        }
    }
}

/// O que se sabe de cada tipo de widget.
#[derive(Clone, Copy, Debug)]
pub struct KindSpec {
    /// O tipo.
    pub kind: WidgetKind,
    /// A chave do título.
    pub title_key: &'static str,
    /// A chave da descrição na biblioteca.
    pub desc_key: &'static str,
    /// O ícone do sprite.
    pub icon: &'static str,
    /// A categoria.
    pub category: Category,
    /// Os tamanhos permitidos (colunas × linhas); o primeiro é o de entrada.
    pub sizes: &'static [(u8, u8)],
    /// Não se pode retirar.
    pub mandatory: bool,
    /// Só para administradores.
    pub admin_only: bool,
    /// Aparece na biblioteca.
    pub in_library: bool,
    /// «Ver tudo».
    pub href: Option<&'static str>,
    /// A linha fixa por baixo do título (nunca a unidade).
    pub subtitle_key: Option<&'static str>,
}

const fn k(
    kind: WidgetKind,
    key: &'static str,
    icon: &'static str,
    category: Category,
    sizes: &'static [(u8, u8)],
    href: Option<&'static str>,
) -> KindSpec {
    KindSpec {
        kind,
        title_key: key,
        desc_key: key,
        icon,
        category,
        sizes,
        mandatory: false,
        admin_only: false,
        in_library: true,
        href,
        subtitle_key: None,
    }
}

/// O registo completo, pela ordem da biblioteca.
pub const KINDS: &[KindSpec] = &[
    KindSpec {
        desc_key: "desk.d.kpis",
        ..k(
            WidgetKind::Kpis,
            "desk.w.kpis",
            "grid",
            Category::Research,
            &[(4, 1)],
            None,
        )
    },
    // Code (D009 · G9-18): os avisos não têm fonte no Core (FG-013); oferecê-
    // los na biblioteca seria oferecer um cartão sempre indisponível. Quem já
    // o tem na disposição mantém-no e pode retirá-lo (deixou de ser
    // obrigatório); volta à biblioteca quando houver fonte.
    KindSpec {
        in_library: false,
        desc_key: "desk.d.notice",
        subtitle_key: Some("desk.sub.notice"),
        ..k(
            WidgetKind::Notice,
            "desk.w.notice",
            "bell",
            Category::Organisation,
            &[(2, 1)],
            Some("/notifications"),
        )
    },
    KindSpec {
        desc_key: "desk.d.continue",
        ..k(
            WidgetKind::Continue,
            "desk.w.continue",
            "work",
            Category::Productivity,
            &[(2, 1), (2, 2)],
            Some("/files"),
        )
    },
    KindSpec {
        desc_key: "desk.d.tasks",
        subtitle_key: Some("desk.sub.tasks"),
        ..k(
            WidgetKind::Tasks,
            "desk.w.tasks",
            "tasks",
            Category::Productivity,
            &[(1, 2), (1, 1), (2, 2)],
            Some("/my-work"),
        )
    },
    KindSpec {
        desc_key: "desk.d.calendar",
        ..k(
            WidgetKind::Calendar,
            "desk.w.calendar",
            "calendar",
            Category::Productivity,
            &[(1, 2), (2, 1), (2, 2)],
            Some("/calendar"),
        )
    },
    KindSpec {
        desc_key: "desk.d.notes",
        ..k(
            WidgetKind::Notes,
            "desk.w.notes",
            "notes",
            Category::Productivity,
            &[(1, 1), (2, 1)],
            Some("/notes"),
        )
    },
    KindSpec {
        desc_key: "desk.d.files",
        ..k(
            WidgetKind::Files,
            "desk.w.files",
            "files",
            Category::Files,
            &[(2, 1), (2, 2)],
            Some("/files"),
        )
    },
    KindSpec {
        desc_key: "desk.d.mail",
        ..k(
            WidgetKind::Mail,
            "desk.w.mail",
            "mail",
            Category::Communication,
            &[(1, 2), (2, 1)],
            Some("/mail"),
        )
    },
    KindSpec {
        desc_key: "desk.d.activity",
        ..k(
            WidgetKind::Activity,
            "desk.w.activity",
            "activity",
            Category::Communication,
            &[(1, 2), (2, 2)],
            Some("/activity"),
        )
    },
    KindSpec {
        desc_key: "desk.d.projects",
        ..k(
            WidgetKind::Projects,
            "desk.w.projects",
            "project",
            Category::Research,
            &[(2, 1), (1, 1)],
            Some("/projects"),
        )
    },
    KindSpec {
        desc_key: "desk.d.ideas",
        ..k(
            WidgetKind::Ideas,
            "desk.w.ideas",
            "idea",
            Category::Research,
            &[(1, 1), (2, 1)],
            Some("/ideas"),
        )
    },
    KindSpec {
        desc_key: "desk.d.datasets",
        ..k(
            WidgetKind::Datasets,
            "desk.w.datasets",
            "data",
            Category::Research,
            &[(1, 1)],
            Some("/datasets"),
        )
    },
    KindSpec {
        desc_key: "desk.d.storage",
        ..k(
            WidgetKind::Storage,
            "desk.w.storage",
            "storage",
            Category::System,
            &[(1, 1)],
            Some("/files"),
        )
    },
    KindSpec {
        desc_key: "desk.d.health",
        href: None,
        subtitle_key: Some("desk.sub.health"),
        ..k(
            WidgetKind::Health,
            "desk.w.health",
            "status",
            Category::System,
            &[(1, 1), (2, 1)],
            Some("/activity"),
        )
    },
];

/// A especificação de um tipo.
#[must_use]
pub fn spec(kind: WidgetKind) -> &'static KindSpec {
    KINDS
        .iter()
        .find(|s| s.kind == kind)
        .expect("todos os tipos estão no registo")
}

/// Um indicador: (chave do título, chave do qualificativo para `tp`, ícone, rota).
pub type KpiSpec = (&'static str, &'static str, &'static str, &'static str);

/// Os indicadores do widget Indicadores, por ordem. O qualificativo concorda com o
/// número: `tp(q, n)` escolhe `.one`/`.other` («1 activa» / «4 activas»).
pub const KPIS: &[KpiSpec] = &[
    // Code (D009): cada indicador leva o ícone canónico da sua aplicação
    // (`experience::iconography`); o de Unidades passou a `org-tree`.
    ("desk.kpi.units", "desk.kpi.units_q", "org-tree", "/units"),
    ("desk.kpi.ideas", "desk.kpi.ideas_q", "idea", "/ideas"),
    (
        "desk.kpi.projects",
        "desk.kpi.projects_q",
        "project",
        "/projects",
    ),
    (
        "desk.kpi.datasets",
        "desk.kpi.datasets_q",
        "data",
        "/datasets",
    ),
];

/// `true` se `w×h` é um tamanho permitido do tipo.
#[must_use]
pub fn size_allowed(kind: WidgetKind, w: u8, h: u8) -> bool {
    spec(kind).sizes.contains(&(w, h))
}

#[cfg(test)]
fn place(kind: WidgetKind, w: u8, h: u8) -> PlacedWidget {
    PlacedWidget {
        id: kind.as_str().to_owned(),
        kind,
        w,
        h,
        minimized: false,
    }
}

/// A predefinição de cada Distribuição. D009: vem de
/// `experience::distribution::DEFAULTS` (configuração de produto tipada e
/// versionada); o nome fica para as chamadas existentes.
#[must_use]
pub fn system_default(d: Distribution) -> Vec<PlacedWidget> {
    crate::experience::distribution::widgets(Some(d))
}

/// A chave da saudação pela hora local da Instância (0–23):
/// 05–11 manhã, 12–19 tarde, 20–04 noite. Interpola `{name}`.
#[must_use]
pub const fn greeting_key(hour: u8) -> &'static str {
    match hour {
        5..=11 => "home.greeting.morning",
        12..=19 => "home.greeting.afternoon",
        _ => "home.greeting.evening",
    }
}

/// Uma mudança de tamanho: (tipo, actual, predefinido).
pub type Resize = (WidgetKind, (u8, u8), (u8, u8));

/// O que muda ao repor a predefinição.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DeskDiff {
    /// Tipos que voltam.
    pub added: Vec<WidgetKind>,
    /// Tipos que saem.
    pub removed: Vec<WidgetKind>,
    /// Tipos que mudam de tamanho: (tipo, actual, predefinido).
    pub resized: Vec<Resize>,
    /// Quantos widgets voltam à posição predefinida.
    pub moved: usize,
    /// O fundo ou o escurecimento mudam.
    pub look_changes: bool,
}

impl DeskDiff {
    /// `true` se repor não muda nada.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.added.is_empty()
            && self.removed.is_empty()
            && self.resized.is_empty()
            && self.moved == 0
            && !self.look_changes
    }
}

/// Compara a disposição actual com a predefinida. O conteúdo dos widgets e os
/// dados do membro não entram: repor muda só a disposição.
#[must_use]
pub fn diff(current: &[PlacedWidget], default: &[PlacedWidget], look_changes: bool) -> DeskDiff {
    let has = |list: &[PlacedWidget], k: WidgetKind| list.iter().any(|p| p.kind == k);
    let added = default
        .iter()
        .filter(|d| !has(current, d.kind))
        .map(|d| d.kind)
        .collect();
    let removed = current
        .iter()
        .filter(|c| !has(default, c.kind))
        .map(|c| c.kind)
        .collect();
    let resized = default
        .iter()
        .filter_map(|d| {
            let c = current.iter().find(|c| c.kind == d.kind)?;
            ((c.w, c.h) != (d.w, d.h)).then_some((d.kind, (c.w, c.h), (d.w, d.h)))
        })
        .collect();
    let common_cur: Vec<_> = current
        .iter()
        .filter(|c| has(default, c.kind))
        .map(|c| c.kind)
        .collect();
    let common_def: Vec<_> = default
        .iter()
        .filter(|d| has(current, d.kind))
        .map(|d| d.kind)
        .collect();
    let moved = common_def
        .iter()
        .zip(&common_cur)
        .filter(|(a, b)| a != b)
        .count();
    DeskDiff {
        added,
        removed,
        resized,
        moved,
        look_changes,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn o_registo_e_coerente() {
        for s in KINDS {
            assert!(crate::i18n::has(s.title_key), "{}", s.title_key);
            assert!(crate::i18n::has(s.desc_key), "{}", s.desc_key);
            assert!(s.subtitle_key.is_none_or(crate::i18n::has));
            assert!(!s.sizes.is_empty());
            assert!(s
                .sizes
                .iter()
                .all(|&(w, h)| (1..=4).contains(&w) && (1..=2).contains(&h)));
        }
        assert_eq!(KINDS.len(), WidgetKind::ALL.len());
        for (l, q, _, _) in KPIS {
            assert!(crate::i18n::has(l));
            assert!(
                crate::i18n::has(&format!("{q}.one")) && crate::i18n::has(&format!("{q}.other"))
            );
        }
    }

    #[test]
    fn as_predefinicoes_do_sistema_so_usam_tamanhos_permitidos_e_tem_os_obrigatorios() {
        for d in [
            Distribution::Research,
            Distribution::Business,
            Distribution::Education,
            Distribution::Personal,
        ] {
            let list = system_default(d);
            assert!(list.iter().all(|p| size_allowed(p.kind, p.w, p.h)));
            assert!(KINDS
                .iter()
                .filter(|s| s.mandatory)
                .all(|s| list.iter().any(|p| p.kind == s.kind)));
            assert!(list.iter().all(|p| !spec(p.kind).admin_only));
        }
    }

    #[test]
    fn a_comparacao_ve_o_que_entra_sai_muda_e_se_move() {
        let def: Vec<_> = [
            (WidgetKind::Kpis, 4, 1),
            (WidgetKind::Calendar, 1, 2),
            (WidgetKind::Notice, 2, 1),
            (WidgetKind::Tasks, 1, 2),
            (WidgetKind::Ideas, 1, 1),
        ]
        .iter()
        .map(|&(k, w, h)| place(k, w, h))
        .collect();
        let mut cur = def.clone();
        cur.retain(|p| p.kind != WidgetKind::Ideas);
        cur.swap(0, 1);
        cur.push(place(WidgetKind::Mail, 1, 2));
        cur.iter_mut()
            .find(|p| p.kind == WidgetKind::Tasks)
            .unwrap()
            .h = 1;
        let d = diff(&cur, &def, false);
        assert_eq!(d.added, vec![WidgetKind::Ideas]);
        assert_eq!(d.removed, vec![WidgetKind::Mail]);
        assert_eq!(d.resized, vec![(WidgetKind::Tasks, (1, 1), (1, 2))]);
        assert_eq!(d.moved, 2);
        assert!(diff(&def, &def, false).is_empty());
    }
}
