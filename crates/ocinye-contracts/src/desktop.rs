//! A disposição do Desktop de um membro, e as regras que o Core lhe aplica.
//!
//! O Desktop é uma grelha de quatro colunas em que **a ordem é a posição**: cada
//! widget tem um tipo e um tamanho de entre os permitidos do tipo. O desenho é do
//! Claude Design (`apps/workspace/src/ui/screens/home/registry.rs`); este módulo
//! é a parte desse registo que tem força de regra — os tipos, os tamanhos, os
//! obrigatórios e os só-de-administração —, para que o Core valide cada escrita
//! sem depender do Workspace. Um teste do Workspace prende as duas tabelas uma à
//! outra: se o Design mudar um tamanho, esse teste falha até esta tabela seguir.
//!
//! # O que uma disposição não é
//!
//! Uma preferência de apresentação do próprio membro. Não concede nem retira
//! acesso a nada: um widget na grelha pede os seus dados ao Core como qualquer
//! outro pedido, e o Core decide o que devolve.

use serde::{Deserialize, Serialize};

/// Um tipo de widget, como o Core o valida.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WidgetKindRule {
    /// O identificador estável (`kind` no contrato).
    pub id: &'static str,
    /// Os tamanhos permitidos, colunas × linhas.
    pub sizes: &'static [(u8, u8)],
    /// Não pode faltar numa disposição.
    pub mandatory: bool,
    /// Só uma sessão com autoridade de administração o pode colocar.
    pub admin_only: bool,
}

const fn rule(id: &'static str, sizes: &'static [(u8, u8)]) -> WidgetKindRule {
    WidgetKindRule {
        id,
        sizes,
        mandatory: false,
        admin_only: false,
    }
}

/// Os catorze tipos de widget, pela ordem do registo do Design.
pub const WIDGET_KINDS: &[WidgetKindRule] = &[
    rule("kpis", &[(4, 1)]),
    WidgetKindRule {
        mandatory: true,
        ..rule("notice", &[(2, 1)])
    },
    rule("continue", &[(2, 1), (2, 2)]),
    rule("tasks", &[(1, 2), (1, 1), (2, 2)]),
    rule("calendar", &[(1, 2), (2, 1), (2, 2)]),
    rule("notes", &[(1, 1), (2, 1)]),
    rule("files", &[(2, 1), (2, 2)]),
    rule("mail", &[(1, 2), (2, 1)]),
    rule("activity", &[(1, 2), (2, 2)]),
    rule("projects", &[(2, 1), (1, 1)]),
    rule("ideas", &[(1, 1), (2, 1)]),
    rule("datasets", &[(1, 1)]),
    rule("storage", &[(1, 1)]),
    rule("health", &[(1, 1), (2, 1)]),
];

/// Os fundos (lista fechada). A fotografia própria fica de fora até haver um
/// contrato de carregamento de imagem.
pub const WALLPAPERS: &[&str] = &["ocinye", "dusk", "org", "mist", "slate", "sand"];

/// Como a imagem de fundo ocupa o ecrã. Reservado para a fotografia.
pub const FITS: &[&str] = &["fill", "fit"];

/// O escurecimento máximo do fundo, em percentagem.
pub const MAX_DIM: u8 = 60;

/// Um widget colocado. A ordem no vector é a posição na grelha.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlacedWidget {
    /// O identificador (hoje igual ao tipo: um de cada).
    pub id: String,
    /// O tipo.
    pub kind: String,
    /// Colunas.
    pub w: u8,
    /// Linhas.
    pub h: u8,
    /// Recolhido: só o cabeçalho.
    #[serde(default)]
    pub minimized: bool,
}

/// A disposição do Desktop de um membro.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DesktopLayout {
    /// O fundo.
    pub wallpaper: String,
    /// Como o fundo ocupa o ecrã.
    #[serde(default = "fill")]
    pub fit: String,
    /// Escurecimento, 0–60, em passos de 5.
    pub dim: u8,
    /// Os widgets, por ordem.
    pub widgets: Vec<PlacedWidget>,
}

fn fill() -> String {
    "fill".to_owned()
}

/// A regra de um tipo, se o tipo existir.
#[must_use]
pub fn kind_rule(id: &str) -> Option<&'static WidgetKindRule> {
    WIDGET_KINDS.iter().find(|r| r.id == id)
}

impl DesktopLayout {
    /// Valida a disposição contra as regras do registo.
    ///
    /// `admin` diz se quem escreve tem **agora** autoridade de administração; só
    /// então pode colocar um tipo `admin_only`.
    ///
    /// # Errors
    ///
    /// A primeira regra violada, numa frase escrita para quem a lê.
    pub fn validate(&self, admin: bool) -> Result<(), String> {
        if !WALLPAPERS.contains(&self.wallpaper.as_str()) {
            return Err("Fundo do Desktop desconhecido.".to_owned());
        }
        if !FITS.contains(&self.fit.as_str()) {
            return Err("Ajuste do fundo desconhecido.".to_owned());
        }
        if self.dim > MAX_DIM || !self.dim.is_multiple_of(5) {
            return Err("O escurecimento vai de 0 a 60, em passos de 5.".to_owned());
        }
        if self.widgets.len() > WIDGET_KINDS.len() {
            return Err("Há mais widgets do que tipos de widget.".to_owned());
        }
        let mut vistos = std::collections::BTreeSet::new();
        for w in &self.widgets {
            let Some(regra) = kind_rule(&w.kind) else {
                return Err("Tipo de widget desconhecido.".to_owned());
            };
            if w.id != w.kind {
                return Err("O identificador de um widget é o seu tipo.".to_owned());
            }
            if !vistos.insert(w.kind.as_str()) {
                return Err("Um widget não pode aparecer duas vezes.".to_owned());
            }
            if !regra.sizes.contains(&(w.w, w.h)) {
                return Err("Tamanho não permitido para este widget.".to_owned());
            }
            if regra.admin_only && !admin {
                return Err("Este widget é só para administradores.".to_owned());
            }
        }
        if let Some(falta) = WIDGET_KINDS
            .iter()
            .find(|r| r.mandatory && !vistos.contains(r.id))
        {
            return Err(format!("O widget «{}» é obrigatório.", falta.id));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn layout(widgets: &[(&str, u8, u8)]) -> DesktopLayout {
        DesktopLayout {
            wallpaper: "ocinye".to_owned(),
            fit: "fill".to_owned(),
            dim: 20,
            widgets: widgets
                .iter()
                .map(|&(k, w, h)| PlacedWidget {
                    id: k.to_owned(),
                    kind: k.to_owned(),
                    w,
                    h,
                    minimized: false,
                })
                .collect(),
        }
    }

    #[test]
    fn uma_disposicao_valida_passa() {
        assert_eq!(
            layout(&[("notice", 2, 1), ("tasks", 1, 2)]).validate(false),
            Ok(())
        );
    }

    #[test]
    fn o_obrigatorio_nao_pode_faltar() {
        assert!(layout(&[("tasks", 1, 2)]).validate(false).is_err());
    }

    #[test]
    fn tipo_tamanho_repeticao_e_fundo_sao_validados() {
        assert!(layout(&[("notice", 2, 1), ("nye", 1, 1)])
            .validate(false)
            .is_err());
        assert!(layout(&[("notice", 2, 1), ("kpis", 1, 1)])
            .validate(false)
            .is_err());
        assert!(layout(&[("notice", 2, 1), ("notice", 2, 1)])
            .validate(false)
            .is_err());
        let mut l = layout(&[("notice", 2, 1)]);
        l.wallpaper = "photo".to_owned();
        assert!(l.validate(false).is_err());
        let mut l = layout(&[("notice", 2, 1)]);
        l.dim = 65;
        assert!(l.validate(false).is_err());
        l.dim = 7;
        assert!(l.validate(false).is_err());
    }

    #[test]
    fn o_identificador_e_o_tipo() {
        let mut l = layout(&[("notice", 2, 1)]);
        l.widgets[0].id = "outro".to_owned();
        assert!(l.validate(false).is_err());
    }
}
