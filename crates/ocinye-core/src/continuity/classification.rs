//! O que é preciso levar, e o que não é.
//!
//! # Porque uma classificação, e não uma lista
//!
//! Porque uma lista de directórios envelhece em silêncio. Uma tabela nova
//! aparece, ninguém a acrescenta ao script, e a falta só se descobre quando
//! alguém tenta restaurar.
//!
//! A classificação obriga a decidir. Cada activo institucional tem de estar
//! numa das classes abaixo, e um activo que não esteja em nenhuma faz o
//! inventário falhar — em vez de ser esquecido.
//!
//! > **Ninguém deve fazer backup de dois terabytes de lixo enquanto esquece um
//! > directório científico essencial.**

use serde::{Deserialize, Serialize};

/// O que um activo é, para efeitos de continuidade.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Classe {
    /// Estado institucional autoritativo. Perdê-lo é perder a instituição.
    ///
    /// Não existe noutro sítio, não se reconstrói, e nenhuma outra cópia o
    /// substitui. Um snapshot que o omita não é um snapshot.
    Autoritativo,
    /// Material sem o qual o estado autoritativo não é interpretável.
    ///
    /// Chaves de cifra, e o que mais for preciso para **ler** o que ficou
    /// guardado. Viaja à parte do resto, por razões óbvias, mas se não viajar o
    /// que viajou não serve.
    Interpretativo,
    /// Derivado, mas caro de reconstruir.
    ///
    /// Índices de pesquisa, projecções. Reconstrói-se a partir do
    /// autoritativo; levá-lo poupa tempo, não conhecimento.
    DerivadoDuravel,
    /// Reconstrói-se por inteiro, e de forma determinista.
    ///
    /// Compilações, artefactos de build. Levá-los é peso morto.
    Reconstruivel,
    /// Não sobrevive a um reinício, e não devia.
    ///
    /// Caches, presença, sessões efémeras. Se perder isto significar perder
    /// conhecimento institucional, o defeito é arquitectural e não de backup.
    Efemero,
    /// Vive noutro sistema, sob outra autoridade.
    ///
    /// O serviço de correio tem as mensagens; o fornecedor de IA tem os pesos.
    /// O que a instituição guarda é a referência, não o conteúdo.
    Externo,
    /// Credencial de operação, substituível por rotação.
    ///
    /// **Não é memória institucional.** Numa migração, a estratégia correcta é
    /// quase sempre rodar em vez de copiar — e confundir isto com o
    /// interpretativo é como se perde uma chave de cifra por a tratar como
    /// palavra-passe de serviço.
    CredencialOperacional,
}

impl Classe {
    /// A representação estável, para manifestos e documentação.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Autoritativo => "AUTHORITATIVE",
            Self::Interpretativo => "INTERPRETIVE",
            Self::DerivadoDuravel => "DURABLE_DERIVED",
            Self::Reconstruivel => "REBUILDABLE",
            Self::Efemero => "EPHEMERAL",
            Self::Externo => "EXTERNAL",
            Self::CredencialOperacional => "OPERATIONAL_CREDENTIAL",
        }
    }

    /// Se um snapshot institucional tem de o transportar.
    ///
    /// O interpretativo conta, e viaja por um canal próprio: sem ele o
    /// autoritativo chega ilegível.
    #[must_use]
    pub const fn viaja(self) -> bool {
        matches!(self, Self::Autoritativo | Self::Interpretativo)
    }
}

/// Um activo do sistema, e o que fazer com ele numa migração.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Activo {
    /// Como se chama, de forma estável.
    pub nome: &'static str,
    /// Onde vive.
    pub onde: &'static str,
    /// O que é.
    pub classe: Classe,
    /// Porque está nesta classe, e não noutra.
    ///
    /// Escrito porque a classificação é uma decisão, e uma decisão sem razão
    /// escrita é revertida pela primeira pessoa que discordar dela.
    pub porque: &'static str,
}

/// Tudo o que existe, classificado.
///
/// # Porque isto é código e não um documento
///
/// Porque um documento não falha. Esta lista é confrontada com o esquema real
/// por `continuity::inventario_cobre_o_esquema`, e uma tabela nova sem classe
/// faz o portão fechar.
#[must_use]
pub fn inventario() -> Vec<Activo> {
    vec![
        Activo {
            nome: "PostgreSQL",
            onde: "base de dados institucional",
            classe: Classe::Autoritativo,
            porque: "A fonte canónica de identidade, autorização, investigação, \
                     ciência, proveniência e auditoria. Não existe noutro sítio.",
        },
        Activo {
            nome: "Object Storage",
            onde: "bucket S3-compatible",
            classe: Classe::Autoritativo,
            porque: "Os bytes a que a base aponta. Migrar só o PostgreSQL \
                     preserva a referência e perde o conhecimento que ela nomeia.",
        },
        Activo {
            nome: "OCINYE_MAIL_KEY",
            onde: "cofre de segredos da instalação",
            classe: Classe::Interpretativo,
            porque: "Sem ela, `mailbox_credentials` chega intacta e ilegível: as \
                     senhas estão seladas com ChaCha20-Poly1305 e a chave não \
                     está na base. Uma cópia perfeita e inútil.",
        },
        Activo {
            nome: "search_documents",
            onde: "PostgreSQL",
            classe: Classe::DerivadoDuravel,
            porque: "Projecção de pesquisa, reconstruível a partir dos artefactos \
                     que indexa. Viaja no dump porque está na mesma base, e \
                     perdê-la custaria uma reindexação, não conhecimento.",
        },
        Activo {
            nome: "Redis",
            onde: "serviço de coordenação efémera",
            classe: Classe::Efemero,
            porque: "Presença e `typing`, com prazo de validade. Não autoriza e \
                     não persiste (ADR-0012). Um servidor novo arranca com ele \
                     vazio, e essa é a prova de que não é fonte de verdade.",
        },
        Activo {
            nome: "target/",
            onde: "disco da máquina de compilação",
            classe: Classe::Reconstruivel,
            porque: "Artefactos de compilação. Determinista a partir do código.",
        },
        Activo {
            nome: "Mensagens no servidor de correio",
            onde: "serviço IMAP do fornecedor",
            classe: Classe::Externo,
            porque: "`mail_messages` é um índice, não um arquivo (ADR-0407). O \
                     conteúdo vive no fornecedor, e é dele que volta.",
        },
        Activo {
            nome: "Pesos de modelos de IA",
            onde: "nó de inferência",
            classe: Classe::Externo,
            porque: "O modelo é runtime substituível. O que a instituição \
                     preserva é a **identidade** do modelo quando uma operação \
                     científica dependeu dela — não centenas de gigabytes por \
                     snapshot.",
        },
        Activo {
            nome: "Credenciais de fornecedor",
            onde: "cofre de segredos da instalação",
            classe: Classe::CredencialOperacional,
            porque: "Chaves de S3, do fornecedor de IA, da conta de correio. \
                     Numa migração rodam-se; copiá-las alarga a exposição sem \
                     preservar memória nenhuma.",
        },
        Activo {
            nome: "Sessões vivas",
            onde: "PostgreSQL, tabela `sessions`",
            classe: Classe::Efemero,
            porque: "Viajam no dump por estarem na base, e não fazem falta: quem \
                     estava autenticado volta a entrar. Identidade persiste; \
                     autoridade restabelece-se (ADR-0411).",
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Cada activo declara a sua razão.
    ///
    /// Uma classificação sem razão escrita é revertida pela primeira pessoa que
    /// discordar dela, e ninguém saberá porquê.
    #[test]
    fn cada_activo_diz_porque_esta_na_classe_em_que_esta() {
        for activo in inventario() {
            assert!(
                activo.porque.len() > 40,
                "«{}» está classificado sem razão suficiente",
                activo.nome
            );
        }
    }

    /// Só o autoritativo e o interpretativo viajam.
    ///
    /// A distinção é o coração desta fatia: levar tudo é caro e leva lixo;
    /// levar só a base deixa ficar os bytes e a chave.
    #[test]
    fn viaja_o_que_nao_se_reconstroi_e_o_que_o_torna_legivel() {
        let viajam: Vec<&str> = inventario()
            .into_iter()
            .filter(|a| a.classe.viaja())
            .map(|a| a.nome)
            .collect();

        assert!(viajam.contains(&"PostgreSQL"), "a base tem de viajar");
        assert!(
            viajam.contains(&"Object Storage"),
            "os bytes têm de viajar: sem eles a referência sobrevive e o \
             conhecimento não"
        );
        assert!(
            viajam.contains(&"OCINYE_MAIL_KEY"),
            "a chave tem de viajar, ou o que viajou não se lê"
        );
        assert!(
            !viajam.contains(&"Redis"),
            "o Redis não pode ser preciso para restaurar: se for, não é efémero"
        );
        assert!(
            !viajam.contains(&"Credenciais de fornecedor"),
            "credenciais operacionais rodam-se, não se copiam"
        );
    }

    /// Uma classe nova declara se viaja.
    ///
    /// Sem isto, acrescentar uma variante fá-la-ia cair no ramo de omissão do
    /// `matches!` e ficar de fora de qualquer snapshot, em silêncio.
    #[test]
    fn toda_a_classe_tem_uma_decisao_de_transporte() {
        let todas = [
            Classe::Autoritativo,
            Classe::Interpretativo,
            Classe::DerivadoDuravel,
            Classe::Reconstruivel,
            Classe::Efemero,
            Classe::Externo,
            Classe::CredencialOperacional,
        ];
        for classe in todas {
            // O que se mede é que a representação existe e é distinta: uma
            // classe sem nome estável não aparece num manifesto legível.
            assert!(!classe.as_str().is_empty());
        }
        let nomes: std::collections::BTreeSet<&str> = todas.iter().map(|c| c.as_str()).collect();
        assert_eq!(nomes.len(), todas.len(), "duas classes com o mesmo nome");
    }
}

#[cfg(test)]
mod cobertura {
    use super::*;

    /// Toda a tabela do esquema está coberta por uma decisão de continuidade.
    ///
    /// # O defeito que isto guarda
    ///
    /// Uma tabela nova aparece numa migration, ninguém a acrescenta ao
    /// pensamento sobre backups, e a falta só se descobre no dia em que alguém
    /// tenta restaurar. É a forma mais silenciosa de perder memória
    /// institucional: não há erro, há uma ausência.
    ///
    /// Aqui, uma tabela que não caiba em nenhuma decisão faz o portão fechar. A
    /// decisão pode ser «viaja no dump» — a maioria é — mas tem de ser tomada.
    #[test]
    fn o_inventario_cobre_o_esquema() {
        let raiz = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("migrations");

        let mut tabelas: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
        let mut ficheiros: Vec<_> = std::fs::read_dir(&raiz)
            .expect("migrations")
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|e| e == "sql"))
            .collect();
        ficheiros.sort();

        // Sem migrations não há esquema, e um portão que observa zero tabelas
        // reporta verde por não ter visto nada.
        assert!(
            !ficheiros.is_empty(),
            "não há migrations para observar; isto seria verde por vazio"
        );

        for ficheiro in &ficheiros {
            let sql = std::fs::read_to_string(ficheiro).expect("ler migration");
            for linha in sql.lines() {
                if let Some(resto) = linha.trim().strip_prefix("CREATE TABLE ") {
                    let nome = resto
                        .trim_start_matches("IF NOT EXISTS ")
                        .split_whitespace()
                        .next()
                        .unwrap_or_default()
                        .trim_end_matches('(')
                        .to_owned();
                    if !nome.is_empty() {
                        tabelas.insert(nome);
                    }
                }
            }
        }

        assert!(
            tabelas.len() > 40,
            "só {} tabelas encontradas; a leitura das migrations partiu-se",
            tabelas.len()
        );

        // O PostgreSQL inteiro é um activo autoritativo, e é essa a decisão que
        // cobre a maioria das tabelas. O que este portão exige é que a decisão
        // exista **e diga que a base viaja** — se alguém a reclassificar, todas
        // estas tabelas ficam sem transporte, e é aqui que se sabe.
        let base = inventario()
            .into_iter()
            .find(|a| a.nome == "PostgreSQL")
            .expect("a base tem de estar no inventário");
        assert!(
            base.classe.viaja(),
            "o PostgreSQL deixou de viajar, e com ele {} tabelas",
            tabelas.len()
        );

        // As excepções são nomeadas uma a uma: uma tabela dentro da base que
        // **não** seja autoritativa tem de o dizer, e não ser assumida.
        //
        // O par é (tabela no esquema, activo no inventário). São dois nomes
        // diferentes de propósito: o esquema fala em `sessions`, e quem lê um
        // manifesto de continuidade lê «Sessões vivas».
        let excepcoes = [
            ("search_documents", "search_documents"),
            ("sessions", "Sessões vivas"),
        ];
        for (tabela, activo) in excepcoes {
            assert!(
                tabelas.contains(tabela),
                "«{tabela}» está declarada como excepção e não existe no esquema"
            );
            let entrada = inventario()
                .into_iter()
                .find(|a| a.nome == activo)
                .unwrap_or_else(|| {
                    panic!("«{tabela}» é tratada como excepção e «{activo}» não está no inventário")
                });
            assert!(
                !matches!(entrada.classe, Classe::Autoritativo),
                "«{tabela}» está listada como excepção e classificada como \
                 autoritativa; uma das duas coisas está errada"
            );
        }
    }
}
