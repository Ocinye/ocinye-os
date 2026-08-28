//! O vocabulário da proveniência científica, e o que cada verbo pode ligar.
//!
//! # Porque um vocabulário fechado não chega
//!
//! `research_links` já tinha relações fechadas: sete verbos, verificados por um
//! `CHECK`. E aceitava `«gato» produces «chapéu»`, porque os **tipos** das duas
//! pontas eram texto livre.
//!
//! Fechar os tipos também não chega. Com quinze verbos e vinte e cinco tipos há
//! nove mil combinações, e quase todas são absurdas: uma pessoa não é produzida
//! por um dataset, uma hipótese não substitui um nó de computação. Um sistema
//! que as aceite guarda afirmações sem sentido na memória institucional — e uma
//! afirmação falsa na linhagem é tão grave como um dado alterado, porque passa
//! a dizer oficialmente que uma coisa deriva de outra.
//!
//! Por isso o que se declara aqui é a **matriz**: para cada verbo, que tipos
//! podem estar de cada lado. O que não está declarado é recusado.
//!
//! > **Fail closed.** Uma combinação que ninguém pensou não é uma combinação
//! > permitida.
//!
//! # A direcção é sempre a mesma
//!
//! Cada aresta lê-se **da origem para o destino**, na ordem em que o verbo está
//! escrito:
//!
//! ```text
//! Resultado  --produced_by-->  Execução
//! Estudo     --tests-->        Hipótese
//! Execução   --executed_on-->  Nó
//! ```
//!
//! Misturar direcções por verbo tornaria a travessia impossível de ler: quem
//! percorre a montante teria de saber, verbo a verbo, se anda para a frente ou
//! para trás.

use serde::{Deserialize, Serialize};

use crate::agentic::ResourceKind;

/// O que uma aresta de proveniência afirma.
///
/// As sete primeiras existiam desde 2026-08 e ficam; as oito seguintes são o
/// ciclo científico. Nenhuma foi removida: uma relação retirada do vocabulário
/// tornaria ilegível a proveniência já escrita.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProvenanceRelation {
    /// Cita uma referência bibliográfica.
    Cites,
    /// Sustenta uma afirmação.
    Supports,
    /// Contradiz uma afirmação.
    Refutes,
    /// Deriva de outro artefacto.
    DerivedFrom,
    /// Utiliza um recurso.
    Uses,
    /// Produz um artefacto.
    Produces,
    /// Relaciona-se, sem que o vocabulário tenha verbo melhor.
    ///
    /// Deliberadamente vago, e deliberadamente mantido: obrigar toda a gente a
    /// escolher um verbo preciso quando a relação é imprecisa produz verbos
    /// precisos e errados.
    RelatesTo,
    /// Um estudo testa uma hipótese.
    Tests,
    /// Um estudo segue uma versão de metodologia.
    Follows,
    /// Uma versão de dataset entra numa execução.
    InputTo,
    /// Um resultado foi produzido por uma execução.
    ProducedBy,
    /// Uma execução correu num nó de computação.
    ExecutedOn,
    /// Uma validação sustenta um resultado.
    Validates,
    /// Uma execução reproduz outra.
    Reproduces,
    /// Uma versão substitui a anterior.
    Supersedes,
}

impl ProvenanceRelation {
    /// Representação estável, tal como fica na base de dados.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Cites => "cites",
            Self::Supports => "supports",
            Self::Refutes => "refutes",
            Self::DerivedFrom => "derived_from",
            Self::Uses => "uses",
            Self::Produces => "produces",
            Self::RelatesTo => "relates_to",
            Self::Tests => "tests",
            Self::Follows => "follows",
            Self::InputTo => "input_to",
            Self::ProducedBy => "produced_by",
            Self::ExecutedOn => "executed_on",
            Self::Validates => "validates",
            Self::Reproduces => "reproduces",
            Self::Supersedes => "supersedes",
        }
    }

    /// A partir da representação estável.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        Self::all().into_iter().find(|r| r.as_str() == value)
    }

    /// Todas.
    #[must_use]
    pub const fn all() -> [Self; 15] {
        [
            Self::Cites,
            Self::Supports,
            Self::Refutes,
            Self::DerivedFrom,
            Self::Uses,
            Self::Produces,
            Self::RelatesTo,
            Self::Tests,
            Self::Follows,
            Self::InputTo,
            Self::ProducedBy,
            Self::ExecutedOn,
            Self::Validates,
            Self::Reproduces,
            Self::Supersedes,
        ]
    }

    /// Como se lê, para quem não conhece o vocabulário.
    ///
    /// Aparece na interface entre os dois recursos: «Resultado — produzido por
    /// → Execução». Sem isto a superfície mostraria `produced_by`, e a
    /// proveniência passaria a ser uma ferramenta para quem conhece o esquema.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Cites => "cita",
            Self::Supports => "sustenta",
            Self::Refutes => "contradiz",
            Self::DerivedFrom => "deriva de",
            Self::Uses => "utiliza",
            Self::Produces => "produz",
            Self::RelatesTo => "relaciona-se com",
            Self::Tests => "testa",
            Self::Follows => "segue",
            Self::InputTo => "entra em",
            Self::ProducedBy => "produzido por",
            Self::ExecutedOn => "executado em",
            Self::Validates => "valida",
            Self::Reproduces => "reproduz",
            Self::Supersedes => "substitui",
        }
    }

    /// Se este verbo aceita esta origem e este destino.
    ///
    /// # Porque a lista é explícita
    ///
    /// Porque o contrário — permitir tudo o que ninguém proibiu — enche a
    /// memória institucional de afirmações sem sentido, e uma afirmação sem
    /// sentido na linhagem é indistinguível de uma afirmação errada.
    ///
    /// Acrescentar um par é uma decisão de domínio, e faz-se aqui.
    #[must_use]
    pub fn accepts(self, source: ResourceKind, target: ResourceKind) -> bool {
        use ResourceKind as K;

        match self {
            // ── O ciclo científico ──────────────────────────────────────
            //
            // Um estudo testa uma hipótese. Nada mais testa nada.
            Self::Tests => matches!((source, target), (K::Study, K::Hypothesis)),

            // Um estudo — ou uma execução — segue uma **versão** de
            // metodologia. Nunca a metodologia em si: a metodologia muda, e
            // uma aresta para ela deixaria de descrever o que foi feito.
            Self::Follows => matches!(
                (source, target),
                (K::Study, K::MethodologyVersion) | (K::StudyExecution, K::MethodologyVersion)
            ),

            // O que entra numa execução: uma versão de dataset, ou um
            // resultado anterior que sirva de entrada.
            Self::InputTo => matches!(
                (source, target),
                (K::DatasetVersion, K::StudyExecution) | (K::Result, K::StudyExecution)
            ),

            // Um resultado é produzido por uma execução. Uma versão de dataset
            // também pode ser: uma simulação produz dados.
            Self::ProducedBy => matches!(
                (source, target),
                (K::Result, K::StudyExecution) | (K::DatasetVersion, K::StudyExecution)
            ),

            Self::ExecutedOn => matches!((source, target), (K::StudyExecution, K::ComputeNode)),

            // Uma execução reproduz outra. Reprodução é entre corridas, e não
            // entre estudos: o mesmo estudo corre duas vezes e são as duas
            // corridas que se comparam.
            Self::Reproduces => {
                matches!((source, target), (K::StudyExecution, K::StudyExecution))
            }

            // Substituição é sempre entre iguais.
            Self::Supersedes => matches!(
                (source, target),
                (K::MethodologyVersion, K::MethodologyVersion)
                    | (K::DatasetVersion, K::DatasetVersion)
                    | (K::Result, K::Result)
            ),

            // ── Afirmações sobre o que se sabe ──────────────────────────
            //
            // Um resultado sustenta ou contradiz uma hipótese; uma referência
            // bibliográfica também. É o mesmo tipo de afirmação.
            Self::Supports | Self::Refutes => matches!(
                (source, target),
                (K::Result, K::Hypothesis)
                    | (K::Source, K::Hypothesis)
                    | (K::Result, K::Result)
                    | (K::Note, K::Hypothesis)
            ),

            Self::Validates => matches!(
                (source, target),
                (K::Result, K::Result) | (K::StudyExecution, K::Result)
            ),

            // ── As antigas, com os pares que já valiam ──────────────────
            Self::Cites => matches!(
                (source, target),
                (K::Note, K::Source)
                    | (K::Document, K::Source)
                    | (K::Idea, K::Source)
                    | (K::Result, K::Source)
                    | (K::MethodologyVersion, K::Source)
            ),

            Self::DerivedFrom => matches!(
                (source, target),
                (K::Idea, K::Source)
                    | (K::Idea, K::Note)
                    | (K::Dataset, K::Dataset)
                    | (K::DatasetVersion, K::DatasetVersion)
                    | (K::Result, K::Result)
                    | (K::Hypothesis, K::Result)
                    | (K::Hypothesis, K::Note)
                    | (K::Project, K::Idea)
            ),

            Self::Uses => matches!(
                (source, target),
                (K::Study, K::Dataset)
                    | (K::Study, K::DatasetVersion)
                    | (K::StudyExecution, K::DatasetVersion)
                    | (K::Note, K::Dataset)
                    | (K::Result, K::DatasetVersion)
                    | (K::MethodologyVersion, K::Dataset)
            ),

            Self::Produces => matches!(
                (source, target),
                (K::StudyExecution, K::Result)
                    | (K::StudyExecution, K::DatasetVersion)
                    | (K::Study, K::Result)
                    | (K::Project, K::Result)
            ),

            // O verbo vago aceita o que é vago: coisas do mesmo mundo
            // científico. Não aceita pessoas, caixas de correio nem
            // compromissos — «relaciona-se» não é razão para ligar o correio à
            // ciência (§76: a passagem de comunicação a conhecimento é
            // deliberada, e não automática).
            Self::RelatesTo => cientifico(source) && cientifico(target),
        }
    }
}

/// Se um tipo pertence ao mundo científico que a proveniência descreve.
///
/// Pessoas, mensagens, compromissos e caixas de correio ficam de fora **por
/// decisão**: são infraestrutura horizontal de colaboração, e transformar uma
/// mensagem em evidência científica é um acto deliberado da instituição, não
/// uma consequência de alguém ter escrito sobre o assunto.
#[must_use]
pub const fn cientifico(kind: ResourceKind) -> bool {
    use ResourceKind as K;
    matches!(
        kind,
        K::Idea
            | K::Project
            | K::Workspace
            | K::Source
            | K::Note
            | K::Document
            | K::Dataset
            | K::DatasetVersion
            | K::Hypothesis
            | K::Methodology
            | K::MethodologyVersion
            | K::Study
            | K::StudyExecution
            | K::Result
    )
}

/// De onde veio a afirmação que uma aresta faz.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProvenanceOrigin {
    /// Alguém a afirmou explicitamente.
    Declared,
    /// A própria operação do Core a conhecia sem ambiguidade.
    ///
    /// Criar um resultado a partir de uma execução **é** a relação: não há
    /// nada a inferir, e pedir a alguém que a declare a seguir seria pedir que
    /// repetisse o que acabou de fazer.
    Operation,
}

impl ProvenanceOrigin {
    /// Representação estável.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Declared => "declared",
            Self::Operation => "operation",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Ida e volta em todos os verbos.
    #[test]
    fn cada_relacao_sobrevive_a_ida_e_volta() {
        for relacao in ProvenanceRelation::all() {
            assert_eq!(ProvenanceRelation::parse(relacao.as_str()), Some(relacao));
        }
        assert_eq!(ProvenanceRelation::parse("inventada"), None);
    }

    /// As sete relações originais continuam a existir.
    ///
    /// # Porque isto é um teste e não um comentário
    ///
    /// Porque a proveniência já escrita usa-as. Retirar uma do vocabulário não
    /// apaga as arestas que a usam: torna-as ilegíveis, e a memória
    /// institucional passa a conter afirmações que o sistema não sabe ler.
    #[test]
    fn as_relacoes_originais_nao_desaparecem() {
        for antiga in [
            "cites",
            "supports",
            "refutes",
            "derived_from",
            "uses",
            "produces",
            "relates_to",
        ] {
            assert!(
                ProvenanceRelation::parse(antiga).is_some(),
                "«{antiga}» existia e desapareceu do vocabulário — a proveniência \
                 já escrita com ela deixou de se poder ler"
            );
        }
    }

    /// A matriz recusa o que não declarou.
    ///
    /// # O que isto guarda
    ///
    /// Quinze verbos e vinte e cinco tipos dão nove mil combinações. Quase
    /// todas são absurdas, e um sistema que as aceite guarda afirmações sem
    /// sentido na memória institucional — indistinguíveis de afirmações
    /// erradas.
    #[test]
    fn a_matriz_recusa_o_que_nao_declarou() {
        use ResourceKind as K;

        // O caso que a versão anterior aceitava, com tipos livres.
        assert!(!ProvenanceRelation::Produces.accepts(K::Person, K::Dataset));

        // Absurdos de estrutura.
        assert!(!ProvenanceRelation::Tests.accepts(K::Hypothesis, K::Hypothesis));
        assert!(!ProvenanceRelation::ExecutedOn.accepts(K::Result, K::ComputeNode));
        assert!(!ProvenanceRelation::Supersedes.accepts(K::Result, K::Hypothesis));

        // E o correio não entra na ciência por «relaciona-se».
        assert!(!ProvenanceRelation::RelatesTo.accepts(K::MailMessage, K::Result));
        assert!(!ProvenanceRelation::RelatesTo.accepts(K::Result, K::Conversation));
    }

    /// E aceita o que o ciclo científico exige.
    ///
    /// O controlo positivo: sem ele, uma matriz que recusasse tudo passaria
    /// nos testes de recusa e tornaria a proveniência impossível de escrever.
    #[test]
    fn a_matriz_aceita_a_cadeia_cientifica() {
        use ResourceKind as K;

        assert!(ProvenanceRelation::Tests.accepts(K::Study, K::Hypothesis));
        assert!(ProvenanceRelation::Follows.accepts(K::Study, K::MethodologyVersion));
        assert!(ProvenanceRelation::InputTo.accepts(K::DatasetVersion, K::StudyExecution));
        assert!(ProvenanceRelation::ProducedBy.accepts(K::Result, K::StudyExecution));
        assert!(ProvenanceRelation::ExecutedOn.accepts(K::StudyExecution, K::ComputeNode));
        assert!(ProvenanceRelation::Supports.accepts(K::Result, K::Hypothesis));
        assert!(ProvenanceRelation::Refutes.accepts(K::Result, K::Hypothesis));
        assert!(ProvenanceRelation::Reproduces.accepts(K::StudyExecution, K::StudyExecution));
    }

    /// A metodologia liga-se pela versão, e não por si.
    ///
    /// # A propriedade que esta milestone existe para garantir
    ///
    /// Se um estudo pudesse seguir «a metodologia M», a linhagem passaria a
    /// descrever outra coisa no dia em que M mudasse — sem que ninguém
    /// alterasse a aresta, e sem que nada o dissesse.
    #[test]
    fn a_metodologia_liga_se_pela_versao() {
        use ResourceKind as K;

        assert!(ProvenanceRelation::Follows.accepts(K::Study, K::MethodologyVersion));
        assert!(
            !ProvenanceRelation::Follows.accepts(K::Study, K::Methodology),
            "um estudo pôde seguir a metodologia mutável — a linhagem passa a \
             descrever outra coisa assim que ela for melhorada"
        );
    }

    /// E o dataset também.
    #[test]
    fn o_dataset_entra_numa_execucao_pela_versao() {
        use ResourceKind as K;

        assert!(ProvenanceRelation::InputTo.accepts(K::DatasetVersion, K::StudyExecution));
        assert!(
            !ProvenanceRelation::InputTo.accepts(K::Dataset, K::StudyExecution),
            "um dataset mutável pôde entrar numa execução — o resultado deixa \
             de dizer com que dados foi calculado"
        );
    }
}
