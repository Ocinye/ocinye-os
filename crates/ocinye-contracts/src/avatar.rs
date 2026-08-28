//! Como um membro é representado visualmente.
//!
//! Três estados, e um deles nunca falha. As iniciais derivam-se do nome: não
//! dependem de storage, de rede, de escolha nem de sorte, e por isso são o
//! fundo de todos os outros. Quando a fotografia não carrega, quando o preset
//! desaparece do catálogo, quando o membro nunca escolheu nada — há sempre
//! alguma coisa que o representa.
//!
//! # O que um avatar não é
//!
//! > **A profile image is presentation metadata, never identity or
//! > authorization evidence.**
//!
//! Nada no sistema pode ler «tem fotografia» como «é de confiança», nem
//! «a fotografia bate certo» como «a identidade foi verificada». A identidade
//! é estabelecida pela autenticação, e só por ela.

use serde::{Deserialize, Serialize};

/// O catálogo de avatares do produto.
///
/// # Porque é uma lista fechada
///
/// O identificador vem do cliente, e um identificador que viesse a ser usado
/// como caminho seria um caminho escolhido por quem o envia. Aqui não pode ser:
/// só existe o que está nesta lista, e o que não está é recusado antes de
/// chegar a qualquer sistema de ficheiros.
///
/// Quatro famílias com três variações cada, todas na mesma linguagem gráfica —
/// geometria, ciência, computação e energia, em Deep Navy e Sunrise Gold. Não
/// representam pessoas, não vêm de bancos de imagens e não dependem de nenhum
/// serviço externo.
pub const AVATAR_PRESETS: &[(&str, &str)] = &[
    // O logótipo da instituição, e o primeiro da grelha.
    //
    // É o único que não é uma abstracção: quem não quiser escolher um motivo
    // nem pôr uma fotografia fica com a marca da casa. Continua a ser uma
    // escolha — o estado de origem são as iniciais, e é o nome da pessoa que
    // aparece enquanto ninguém escolher nada.
    ("ocinye", "ocinye.png"),
    // Compute — nós, circuitos, matrizes.
    ("compute-01", "compute-01.svg"),
    ("compute-02", "compute-02.svg"),
    ("compute-03", "compute-03.svg"),
    // Science — órbitas, células, estruturas.
    ("science-01", "science-01.svg"),
    ("science-02", "science-02.svg"),
    ("science-03", "science-03.svg"),
    // Engineering — malhas, superfícies, geometria.
    ("engineering-01", "engineering-01.svg"),
    ("engineering-02", "engineering-02.svg"),
    ("engineering-03", "engineering-03.svg"),
    // Energy — ondas, fluxo, campos.
    ("energy-01", "energy-01.svg"),
    ("energy-02", "energy-02.svg"),
    ("energy-03", "energy-03.svg"),
];
/// Como o membro escolheu ser representado.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AvatarChoice {
    /// Iniciais derivadas do nome. O estado de origem, e o refúgio de todos os
    /// outros.
    Initials,
    /// Um avatar do catálogo do produto.
    ///
    /// Guarda-se o identificador, não uma cópia: escolher um preset não é um
    /// upload, e tratá-lo como tal duplicaria doze ficheiros por cada membro
    /// da instituição para representar uma escolha que cabe numa palavra.
    Preset {
        /// Identificador do catálogo. Sempre um de [`AVATAR_PRESETS`].
        preset: String,
    },
    /// Uma fotografia carregada pelo próprio membro.
    Custom {
        /// Versão do conteúdo, para o URL de leitura.
        ///
        /// É o checksum do que ficou guardado — e o que ficou guardado é o
        /// resultado da normalização, não o ficheiro original. Muda quando a
        /// fotografia muda, e é isso que permite ao browser guardar a imagem
        /// para sempre sem nunca mostrar a anterior.
        ///
        /// Conhecê-lo não concede nada: a rota de leitura autentica a sessão e
        /// confirma que a versão pedida é a do próprio principal.
        version: String,
    },
}

impl AvatarChoice {
    /// Se o identificador pertence ao catálogo do produto.
    #[must_use]
    pub fn is_known_preset(preset: &str) -> bool {
        AVATAR_PRESETS.iter().any(|(id, _)| *id == preset)
    }

    /// O ficheiro que representa um preset.
    ///
    /// O nome vem da tabela e nunca do identificador que o cliente enviou. É a
    /// diferença entre escolher de uma lista e escrever um caminho: um
    /// identificador desconhecido não devolve ficheiro nenhum, em vez de
    /// devolver o ficheiro que ele nomeia.
    #[must_use]
    pub fn preset_file(preset: &str) -> Option<&'static str> {
        AVATAR_PRESETS
            .iter()
            .find(|(id, _)| *id == preset)
            .map(|(_, file)| *file)
    }

    /// Reconstrói a escolha a partir das colunas guardadas.
    ///
    /// Um preset que já não exista no catálogo — porque o produto o retirou
    /// entre duas versões — cai para as iniciais em vez de produzir uma
    /// referência para um ficheiro que ninguém publica. É o mesmo princípio da
    /// fotografia que não carrega: a representação degrada, a identidade não.
    #[must_use]
    pub fn from_columns(kind: &str, preset: Option<&str>, version: Option<&str>) -> Self {
        match kind {
            "preset" => preset.filter(|value| Self::is_known_preset(value)).map_or(
                Self::Initials,
                |value| Self::Preset {
                    preset: value.to_owned(),
                },
            ),
            "custom" => version.map_or(Self::Initials, |value| Self::Custom {
                version: value.to_owned(),
            }),
            _ => Self::Initials,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn o_catalogo_tem_o_logotipo_e_quatro_familias_de_tres() {
        assert_eq!(AVATAR_PRESETS.len(), 13);
        assert_eq!(
            AVATAR_PRESETS.first().map(|(id, _)| *id),
            Some("ocinye"),
            "o logótipo deixou de ser o primeiro da grelha"
        );
        for familia in ["compute", "science", "engineering", "energy"] {
            let quantos = AVATAR_PRESETS
                .iter()
                .filter(|(id, _)| id.starts_with(familia))
                .count();
            assert_eq!(quantos, 3, "a família {familia} não tem três variações");
        }
    }

    /// O ficheiro vem da tabela, e não do que o cliente escreveu.
    #[test]
    fn um_identificador_desconhecido_nao_nomeia_ficheiro() {
        assert_eq!(AvatarChoice::preset_file("ocinye"), Some("ocinye.png"));
        assert_eq!(
            AvatarChoice::preset_file("compute-01"),
            Some("compute-01.svg")
        );
        assert_eq!(AvatarChoice::preset_file("../../etc/passwd"), None);
        assert_eq!(AvatarChoice::preset_file("ocinye.png"), None);
        assert_eq!(AvatarChoice::preset_file(""), None);
    }

    #[test]
    fn os_identificadores_nao_podem_virar_caminhos() {
        for (id, file) in AVATAR_PRESETS {
            assert!(
                id.chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'),
                "{id} tem caracteres que não pertencem a um identificador"
            );
            assert!(!id.contains(".."), "{id} parece um caminho");
            assert!(
                !file.contains('/') && !file.contains(".."),
                "{file} sai da pasta"
            );
        }
    }

    #[test]
    fn um_preset_desconhecido_cai_para_iniciais() {
        assert_eq!(
            AvatarChoice::from_columns("preset", Some("../../etc/passwd"), None),
            AvatarChoice::Initials
        );
        assert_eq!(
            AvatarChoice::from_columns("preset", Some("compute-99"), None),
            AvatarChoice::Initials
        );
        assert_eq!(
            AvatarChoice::from_columns("preset", Some("compute-01"), None),
            AvatarChoice::Preset {
                preset: "compute-01".to_owned()
            }
        );
    }

    #[test]
    fn sem_escolha_ficam_as_iniciais() {
        assert_eq!(
            AvatarChoice::from_columns("initials", None, None),
            AvatarChoice::Initials
        );
        // `custom` sem versão não é uma fotografia: é uma promessa por cumprir.
        assert_eq!(
            AvatarChoice::from_columns("custom", None, None),
            AvatarChoice::Initials
        );
    }
}
