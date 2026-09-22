//! O idioma do produto.
//!
//! # Uma preferência de apresentação, e nada mais
//!
//! O idioma decide em que língua o Ocinye *se mostra* — nunca o que é permitido,
//! o que existe, ou o que uma coisa significa. Mudar de `pt` para `fr` não move
//! um projecto de estado, não altera uma permissão, não renomeia um ficheiro e
//! não muda o identificador de nada. É a mesma verdade institucional, dita noutra
//! língua (briefing i18n §26, §48).
//!
//! Por isso vive aqui, entre os contratos: é um tipo partilhado pelo Core e pelo
//! Workspace, sem I/O e compilável para `wasm32`. Uma decisão de autorização
//! **não** seria partilhável assim, e não é o que isto é.
//!
//! # Três identificadores, e só três
//!
//! O Ocinye conhece exactamente `pt`, `en` e `fr`. Não guarda `pt-PT`, `en-US`
//! nem `fr-FR`: essas variantes existem na fronteira — o que o browser ou o
//! sistema operativo declara — e normalizam-se para um dos três à entrada
//! ([`Locale::normalize`]). Dar a todas as línguas o mesmo modelo de
//! identificador é o que impede a deriva de nascer (briefing i18n §2).
//!
//! # O `pt` é canónico
//!
//! `pt` é **Português de Portugal**, e é a língua normativa do produto: o
//! significado, a terminologia e a semântica definem-se primeiro em português, e
//! o inglês e o francês são projecções fiéis dessa verdade. Quando falta uma
//! tradução, a resposta é `pt` — nunca uma chave crua (briefing i18n §1, §7).

use serde::{Deserialize, Serialize};

/// A língua canónica do produto: Português de Portugal.
pub const CANONICAL: Locale = Locale::Pt;

/// Um idioma suportado pelo Ocinye.
///
/// Sempre exactamente um de três. A serialização é o código de duas letras
/// (`"pt"`, `"en"`, `"fr"`), que é também o que a base de dados guarda e o que a
/// política de segurança valida contra uma lista fechada.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub enum Locale {
    /// Português de Portugal — a língua canónica.
    #[default]
    Pt,
    /// English.
    En,
    /// Français de France.
    Fr,
}

impl Locale {
    /// Os três idiomas suportados, na ordem em que se apresentam.
    ///
    /// O português vem primeiro por ser o canónico e o predefinido; o inglês e o
    /// francês seguem-no.
    pub const ALL: [Locale; 3] = [Locale::Pt, Locale::En, Locale::Fr];

    /// O código de duas letras — a identidade interna, e o que se persiste.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Locale::Pt => "pt",
            Locale::En => "en",
            Locale::Fr => "fr",
        }
    }

    /// O nome do idioma na sua própria língua.
    ///
    /// Uma língua identifica-se pelo seu próprio nome, e não por uma bandeira nem
    /// por um código: no selector, «Português», «English» e «Français» dizem-se a
    /// si mesmos a quem os procura (briefing i18n §57, §58).
    #[must_use]
    pub const fn native_name(self) -> &'static str {
        match self {
            Locale::Pt => "Português",
            Locale::En => "English",
            Locale::Fr => "Français",
        }
    }

    /// A etiqueta BCP-47 completa, para quando uma fronteira externa a exige.
    ///
    /// O atributo `lang` de um documento e as APIs de formatação (`Intl`) querem
    /// a região: `pt-PT`, `en`, `fr-FR`. Isto é uma projecção **de saída** do
    /// identificador interno — nunca se guarda nem se compara nesta forma.
    #[must_use]
    pub const fn bcp47(self) -> &'static str {
        match self {
            Locale::Pt => "pt-PT",
            Locale::En => "en",
            Locale::Fr => "fr-FR",
        }
    }

    /// Normaliza um valor externo (browser, SO, cookie) para um dos três.
    ///
    /// Aceita o código com ou sem região, com `-` ou `_`, em qualquer caixa:
    /// `pt`, `pt-PT`, `pt_PT` → `pt`; `en`, `en-US`, `en_GB` → `en`; `fr`,
    /// `fr-FR`, `fr-CA` → `fr`. Uma língua não suportada (`de`, `es`, vazio)
    /// devolve `None`, e quem chama decide a queda — que é sempre para `pt`
    /// (briefing i18n §2, §15, §68).
    #[must_use]
    pub fn normalize(raw: &str) -> Option<Locale> {
        // Só a subetiqueta primária conta: `fr-CA` é francês, e a região não
        // muda o idioma interno. Corta-se no primeiro separador, seja `-` ou `_`.
        let primary = raw
            .trim()
            .split(['-', '_'])
            .next()
            .unwrap_or("")
            .to_ascii_lowercase();
        match primary.as_str() {
            "pt" => Some(Locale::Pt),
            "en" => Some(Locale::En),
            "fr" => Some(Locale::Fr),
            _ => None,
        }
    }

    /// Normaliza, caindo para o canónico (`pt`) quando não é suportado.
    ///
    /// É a forma segura de aceitar entrada não confiável: nunca falha, nunca
    /// devolve uma língua a mais, e uma língua desconhecida vira português em vez
    /// de um erro ou de uma chave crua.
    #[must_use]
    pub fn normalize_or_canonical(raw: &str) -> Locale {
        Self::normalize(raw).unwrap_or(CANONICAL)
    }

    /// Escolhe o idioma a partir de um cabeçalho `Accept-Language`.
    ///
    /// Percorre as preferências por ordem e fica no primeiro que o Ocinye
    /// suporta; se nenhuma corresponder, devolve `None` (e quem chama cai em
    /// `pt`). Os pesos `;q=` do cabeçalho ignoram-se: a ordem já exprime a
    /// preferência, e um `q` mal formado não deve decidir a língua de ninguém.
    #[must_use]
    pub fn from_accept_language(header: &str) -> Option<Locale> {
        header
            .split(',')
            .filter_map(|parte| {
                let etiqueta = parte.split(';').next().unwrap_or("").trim();
                Self::normalize(etiqueta)
            })
            .next()
    }
}

impl std::fmt::Display for Locale {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl From<Locale> for String {
    fn from(locale: Locale) -> Self {
        locale.as_str().to_owned()
    }
}

/// O erro de uma língua fora da lista fechada.
///
/// Guardado com o valor recusado para que um log diga o que chegou, sem que o
/// valor cru alguma vez alcance a base de dados ou o ecrã.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnsupportedLocale(pub String);

impl std::fmt::Display for UnsupportedLocale {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "idioma não suportado: {:?}", self.0)
    }
}

impl std::error::Error for UnsupportedLocale {}

impl TryFrom<String> for Locale {
    type Error = UnsupportedLocale;

    /// Estrito de propósito: aceita **só** `pt`, `en`, `fr`.
    ///
    /// Isto é a fronteira da persistência e da validação de segurança, e não a da
    /// entrada do browser. Aqui uma variante regional é um erro: quem guarda um
    /// idioma já o normalizou antes, e um `pt-PT` a chegar aqui é um sinal de que
    /// alguém saltou a normalização — melhor recusar que gravar deriva.
    fn try_from(value: String) -> Result<Self, Self::Error> {
        match value.as_str() {
            "pt" => Ok(Locale::Pt),
            "en" => Ok(Locale::En),
            "fr" => Ok(Locale::Fr),
            _ => Err(UnsupportedLocale(value)),
        }
    }
}

impl std::str::FromStr for Locale {
    type Err = UnsupportedLocale;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Locale::try_from(s.to_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn o_canonico_e_portugues() {
        assert_eq!(CANONICAL, Locale::Pt);
        assert_eq!(CANONICAL.as_str(), "pt");
    }

    #[test]
    fn ha_exactamente_tres_idiomas() {
        assert_eq!(Locale::ALL.len(), 3);
        assert_eq!(Locale::ALL, [Locale::Pt, Locale::En, Locale::Fr]);
    }

    #[test]
    fn normaliza_variantes_regionais_para_o_identificador_interno() {
        for (entrada, esperado) in [
            ("pt", Locale::Pt),
            ("pt-PT", Locale::Pt),
            ("pt_PT", Locale::Pt),
            ("PT", Locale::Pt),
            ("en", Locale::En),
            ("en-US", Locale::En),
            ("en_GB", Locale::En),
            ("en-CA", Locale::En),
            ("fr", Locale::Fr),
            ("fr-FR", Locale::Fr),
            ("fr-CA", Locale::Fr),
            ("fr_BE", Locale::Fr),
        ] {
            assert_eq!(
                Locale::normalize(entrada),
                Some(esperado),
                "{entrada} devia normalizar para {esperado}"
            );
        }
    }

    #[test]
    fn uma_lingua_nao_suportada_nao_se_inventa() {
        for cru in ["de", "es-ES", "zh", "", "  ", "xx"] {
            assert_eq!(Locale::normalize(cru), None, "{cru:?} não é suportado");
            // A forma segura nunca falha, e cai no canónico.
            assert_eq!(Locale::normalize_or_canonical(cru), Locale::Pt);
        }
    }

    #[test]
    fn a_persistencia_recusa_variantes_regionais() {
        // A fronteira estrita: quem grava já normalizou. Um `pt-PT` aqui é deriva.
        assert!(Locale::try_from("pt-PT".to_owned()).is_err());
        assert!(Locale::try_from("en-US".to_owned()).is_err());
        assert!("de".parse::<Locale>().is_err());
        assert_eq!("pt".parse::<Locale>().unwrap(), Locale::Pt);
        assert_eq!("fr".parse::<Locale>().unwrap(), Locale::Fr);
    }

    #[test]
    fn accept_language_fica_na_primeira_lingua_suportada() {
        assert_eq!(
            Locale::from_accept_language("de-DE,de;q=0.9,en;q=0.8"),
            Some(Locale::En),
            "salta o alemão e fica no inglês"
        );
        assert_eq!(
            Locale::from_accept_language("fr-FR,fr;q=0.9"),
            Some(Locale::Fr)
        );
        assert_eq!(Locale::from_accept_language("es-ES,ca"), None);
    }

    #[test]
    fn o_nome_de_cada_lingua_e_o_seu_proprio() {
        assert_eq!(Locale::Pt.native_name(), "Português");
        assert_eq!(Locale::En.native_name(), "English");
        assert_eq!(Locale::Fr.native_name(), "Français");
    }

    #[test]
    fn serializa_como_o_codigo_de_duas_letras() {
        assert_eq!(serde_json::to_string(&Locale::Fr).unwrap(), "\"fr\"");
        assert_eq!(
            serde_json::from_str::<Locale>("\"en\"").unwrap(),
            Locale::En
        );
        // Uma variante regional na desserialização é recusada, como na gravação.
        assert!(serde_json::from_str::<Locale>("\"pt-PT\"").is_err());
    }
}
