//! O portão de entrada do primeiro modelo institucional.
//!
//! > **No first institutional model without continuity.**
//!
//! # Porque isto existe antes de existir um modelo
//!
//! Porque a dívida que fica escrita num documento é a dívida que se descobre
//! no dia em que um `.safetensors` importante está perdido num SSD de GPU.
//!
//! A [ADR-0203](../../../../docs/adrs/0203-institutional-model-artifacts.md)
//! decidiu que um modelo treinado pela Ocinye é estado institucional durável.
//! Não construiu o registo — não há nó, não há treino, não há um único
//! artefacto — e a forma correcta dessas tabelas depende do que só se sabe ao
//! afinar o primeiro modelo.
//!
//! O que este módulo faz é impedir que essa ordem se inverta: **no dia em que
//! o esquema ganhar tabelas de artefacto de modelo, estas perguntas têm de
//! estar respondidas.** Não antes; e nunca depois.
//!
//! > **An Ocinye-trained model must not be promoted to durable institutional
//! > status until its artifact, exact base-model dependency, training lineage,
//! > required runtime components, classification, evaluation evidence and
//! > restore path are governed by the continuity system.**
//!
//! # O que este módulo **não** é
//!
//! Não é um registo de modelos, nem o desenho de um. Não tem tabelas, não
//! guarda estado e não sabe nada sobre treino. É uma lista de perguntas e um
//! portão.

/// O estado de uma pergunta da continuidade de modelos.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Resposta {
    /// Ainda não foi respondida. Não há modelo nenhum para a responder.
    PorResponder,
    /// Respondida, com a evidência que a sustenta.
    ///
    /// A evidência é texto de propósito: quem a lê tem de conseguir ir
    /// verificá-la. «sim» não é evidência.
    Provada(&'static str),
}

impl Resposta {
    /// Se conta como respondida.
    #[must_use]
    pub const fn respondida(self) -> bool {
        matches!(self, Self::Provada(_))
    }
}

/// Uma pergunta que o primeiro modelo institucional tem de responder.
#[derive(Debug, Clone, Copy)]
pub struct Pergunta {
    /// O que se pergunta.
    pub pergunta: &'static str,
    /// Porque é obrigatória — o que se perde se ficar sem resposta.
    pub porque: &'static str,
    /// Onde está.
    pub resposta: Resposta,
}

/// As perguntas, e o estado de cada uma.
///
/// Ordenadas pela sequência em que um modelo as encontraria: primeiro existir
/// fora do nó, depois saber de onde veio, depois poder voltar, e por fim não
/// se tornar um problema de autorização.
pub const PERGUNTAS: &[Pergunta] = &[
    Pergunta {
        pergunta: "Os artefactos sobrevivem à perda do nó que os treinou?",
        porque: "Se não sobreviverem, meses de aprendizagem institucional \
                 dependem de um SSD.",
        resposta: Resposta::PorResponder,
    },
    Pergunta {
        pergunta: "Os pesos vivem fora do nó de computação?",
        porque: "O nó **produz** artefactos institucionais; não os detém. É a \
                 inversão que `ai_models` faz hoje.",
        resposta: Resposta::PorResponder,
    },
    Pergunta {
        pergunta: "Cada versão liga ao modelo base exacto, com soma e licença?",
        porque: "Um adaptador sem o base exacto é ruído com a forma certa, e \
                 um modelo afinado não perde as obrigações da licença de que \
                 deriva.",
        resposta: Resposta::PorResponder,
    },
    Pergunta {
        pergunta: "As versões de dataset que treinaram o modelo são preservadas?",
        porque: "Os pesos não substituem evidência, auditoria nem \
                 reprodutibilidade. «Já está nos pesos» não é uma razão para \
                 apagar a fonte.",
        resposta: Resposta::PorResponder,
    },
    Pergunta {
        pergunta: "A receita de treino é preservada?",
        porque: "Sem código, versões, hiperparâmetros e pré-processamento, o \
                 resultado não se explica nem se repete.",
        resposta: Resposta::PorResponder,
    },
    Pergunta {
        pergunta: "Tokenizer, configuração e adaptadores acompanham os pesos?",
        porque: "Um ficheiro com a soma certa pode ser inútil se faltar o que \
                 o torna carregável.",
        resposta: Resposta::PorResponder,
    },
    Pergunta {
        pergunta: "Os checkpoints seguem uma política de retenção escrita?",
        porque: "Guardar tudo para sempre não é política; é a ausência dela, e \
                 custa terabytes.",
        resposta: Resposta::PorResponder,
    },
    Pergunta {
        pergunta: "O modelo restaurado mantém as somas idênticas?",
        porque: "É a mesma exigência que já se faz aos objectos: identidade \
                 igual não chega se os bytes forem outros.",
        resposta: Resposta::PorResponder,
    },
    Pergunta {
        pergunta: "A linhagem inspecciona-se sem o servidor GPU original?",
        porque: "Se for preciso a GPU para saber de onde o modelo veio, a \
                 memória institucional continua presa ao hardware.",
        resposta: Resposta::PorResponder,
    },
    Pergunta {
        pergunta: "O conhecimento continua acessível com o modelo em baixo?",
        porque: "É a invariante fundadora: o Ocinye OS é AI-native, não \
                 AI-dependent.",
        resposta: Resposta::Provada(
            "É a arquitectura actual, e está exercitada: `ai_general` reporta \
             `no_resource` e a pesquisa, o conhecimento e a cadeia científica \
             abrem à mesma. Verificado no servidor B do ensaio de 2026-08-29.",
        ),
    },
    Pergunta {
        pergunta: "Um modelo treinado sobre dados sensíveis não vira contorno \
                   de autorização?",
        porque: "O treino pode memorizar material `RESTRICTED`. A \
                 classificação de um modelo derivado é uma decisão de \
                 política, e nunca uma herança automática.",
        resposta: Resposta::PorResponder,
    },
];

/// Os nomes de tabela que significam «existe um registo de artefactos».
///
/// Não é uma adivinhação sobre o esquema futuro: é a lista de conceitos que a
/// [ADR-0203] nomeia. Uma tabela com outro nome que faça a mesma coisa escapa
/// a este portão — e é por isso que a decisão está escrita na ADR e não só
/// aqui.
///
/// [ADR-0203]: ../../../../docs/adrs/0203-institutional-model-artifacts.md
#[cfg(test)]
const TABELAS_DE_ARTEFACTO: &[&str] = &[
    "model_versions",
    "model_artifacts",
    "training_runs",
    "evaluation_runs",
    "model_checkpoints",
];

/// Quantas perguntas continuam por responder.
#[must_use]
pub fn por_responder() -> Vec<&'static Pergunta> {
    PERGUNTAS
        .iter()
        .filter(|p| !p.resposta.respondida())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Nenhuma pergunta é muda, e nenhuma resposta é «sim».
    ///
    /// Uma pergunta sem razão escrita não sobrevive a uma discussão sobre
    /// prazos, e uma resposta sem evidência é uma opinião com aspecto de facto.
    #[test]
    fn cada_pergunta_diz_porque_e_obrigatoria() {
        for p in PERGUNTAS {
            assert!(
                p.porque.split_whitespace().count() >= 8,
                "«{}» é obrigatória sem dizer porquê",
                p.pergunta
            );
            if let Resposta::Provada(evidencia) = p.resposta {
                assert!(
                    evidencia.split_whitespace().count() >= 10,
                    "«{}» dá-se por respondida sem evidência que se possa ir \
                     verificar: «{evidencia}»",
                    p.pergunta
                );
            }
        }
    }

    /// O portão: nenhum registo de artefactos sem as perguntas respondidas.
    ///
    /// # O que isto guarda
    ///
    /// A ordem. Hoje não há modelo nenhum, e por isso dez destas perguntas não
    /// têm como ser respondidas — construir o registo antes de existir o
    /// primeiro modelo seria desenhar contra imaginação.
    ///
    /// Mas no dia em que alguém escrever a migration que cria
    /// `model_artifacts`, a dívida deixa de ser futura. Este teste falha nesse
    /// dia, e não no dia em que os pesos se perdem.
    #[test]
    fn nenhum_registo_de_artefactos_sem_continuidade_respondida() {
        let raiz = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("migrations");

        let mut ficheiros: Vec<_> = std::fs::read_dir(&raiz)
            .expect("migrations")
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|e| e == "sql"))
            .collect();
        ficheiros.sort();
        assert!(
            !ficheiros.is_empty(),
            "não há migrations para observar; isto seria verde por vazio"
        );

        let mut encontradas: Vec<&str> = Vec::new();
        for ficheiro in &ficheiros {
            let sql = std::fs::read_to_string(ficheiro).expect("ler migration");
            for linha in sql.lines() {
                let Some(resto) = linha.trim().strip_prefix("CREATE TABLE ") else {
                    continue;
                };
                let nome = resto
                    .trim_start_matches("IF NOT EXISTS ")
                    .split_whitespace()
                    .next()
                    .unwrap_or_default()
                    .trim_end_matches('(');
                if let Some(alvo) = TABELAS_DE_ARTEFACTO.iter().find(|t| **t == nome) {
                    encontradas.push(alvo);
                }
            }
        }

        if encontradas.is_empty() {
            // O estado de hoje. Não é um verde por omissão: é a ausência da
            // condição que o portão observa, e o teste seguinte prova que o
            // portão sabe reagir quando ela aparecer.
            return;
        }

        let abertas = por_responder();
        assert!(
            abertas.is_empty(),
            "o esquema ganhou {encontradas:?}, e {} pergunta(s) da continuidade \
             de modelos continuam por responder:\n{}\n\
             Um modelo treinado pela Ocinye não pode ser promovido a estado \
             institucional durável antes de o artefacto, a dependência do \
             modelo base, a linhagem de treino, os componentes de runtime, a \
             classificação, a evidência de avaliação e o caminho de restauro \
             estarem governados pela continuidade.",
            abertas.len(),
            abertas
                .iter()
                .map(|p| format!("  · {}", p.pergunta))
                .collect::<Vec<_>>()
                .join("\n")
        );
    }

    /// O portão sabe reagir. Se não soubesse, o teste acima seria decorativo.
    ///
    /// Controlo positivo do controlo: exercita a decisão que o portão toma
    /// quando encontra uma tabela de artefactos, sem esperar que ela exista.
    #[test]
    fn o_portao_fecha_quando_a_tabela_aparece() {
        let encontradas = ["model_artifacts"];
        let abertas = por_responder();
        assert!(
            !abertas.is_empty(),
            "todas as perguntas estão respondidas e nenhum modelo foi treinado; \
             ou o primeiro treino aconteceu, ou alguém respondeu sem evidência"
        );
        assert!(
            !encontradas.is_empty() && !abertas.is_empty(),
            "esta é a condição em que o portão tem de fechar"
        );
    }

    /// Uma pergunta já está respondida, e é a fundadora.
    #[test]
    fn o_conhecimento_ja_sobrevive_a_ausencia_de_modelo() {
        let respondidas: Vec<&Pergunta> = PERGUNTAS
            .iter()
            .filter(|p| p.resposta.respondida())
            .collect();
        assert_eq!(
            respondidas.len(),
            1,
            "esperava exactamente uma pergunta respondida — a de que o \
             conhecimento sobrevive à ausência de modelo. Se este número \
             mudou, mudou o que a instituição consegue provar, e isso não \
             acontece por acidente"
        );
        assert!(respondidas[0].pergunta.contains("modelo em baixo"));
    }
}
