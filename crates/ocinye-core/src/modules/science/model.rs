//! Os objectos científicos entre a ideia e o dado.
//!
//! # O que estes tipos são, e o que não são
//!
//! São o mínimo para que uma cadeia científica seja rastreável: uma hipótese
//! que se pode testar, uma metodologia com identidade e versões, um estudo que
//! corre, uma execução concreta, um resultado, e a evidência de que alguém o
//! validou ou reproduziu.
//!
//! Não são um caderno de laboratório electrónico. Um ELN precisa de protocolos
//! passo a passo, inventário de reagentes, assinatura por etapa e calibração de
//! equipamento — e nada disso é preciso para responder à pergunta que esta
//! camada existe para responder: **de onde veio este resultado?**

use chrono::{DateTime, Utc};
use ocinye_contracts::Classification;
use serde::Serialize;
use sqlx::FromRow;
use uuid::Uuid;

/// Uma afirmação que se pode testar.
#[derive(Debug, Clone, FromRow, Serialize)]
pub struct Hypothesis {
    /// Identifier.
    pub id: Uuid,
    /// The unit it belongs to.
    pub unit_id: Uuid,
    /// The research environment, when it has one.
    pub workspace_id: Option<Uuid>,
    /// The project, when it has one.
    pub project_id: Option<Uuid>,
    /// What is being claimed.
    pub statement: String,
    /// Why it is worth testing.
    pub rationale: Option<String>,
    /// Where it stands.
    pub status: String,
    /// Its own classification.
    pub classification: String,
    /// Who wrote it.
    pub created_by_id: Option<Uuid>,
    /// When.
    pub created_at: DateTime<Utc>,
    /// Last change.
    pub updated_at: DateTime<Utc>,
}

impl Hypothesis {
    /// Como um membro lê este estado.
    ///
    /// A interface é em português (`CLAUDE.md` §52) e o vocabulário do domínio
    /// é em inglês. A tradução vive aqui, com o vocabulário, e não numa tabela
    /// do cliente — que seria um segundo vocabulário a divergir do primeiro no
    /// dia em que alguém acrescentasse um estado.
    #[must_use]
    pub fn status_label(&self) -> &'static str {
        match self.status.as_str() {
            "supported" => "Sustentada",
            "refuted" => "Refutada",
            "inconclusive" => "Inconclusiva",
            "withdrawn" => "Retirada",
            _ => "Aberta",
        }
    }

    /// The artefact's own classification.
    #[must_use]
    pub fn classification(&self) -> Classification {
        Classification::parse(&self.classification).unwrap_or(Classification::Internal)
    }
}

/// Uma forma de produzir resultados, com identidade própria.
///
/// Tem identidade porque a pergunta «que metodologia produziu isto?» tem de ter
/// resposta anos depois — e uma aresta para um texto solto não é resposta.
#[derive(Debug, Clone, FromRow, Serialize)]
pub struct Methodology {
    /// Identifier.
    pub id: Uuid,
    /// The unit it belongs to.
    pub unit_id: Uuid,
    /// The research environment, when it has one.
    pub workspace_id: Option<Uuid>,
    /// The project, when it has one.
    pub project_id: Option<Uuid>,
    /// What it is called.
    pub title: String,
    /// What it is for.
    pub purpose: Option<String>,
    /// Its own classification.
    pub classification: String,
    /// Who created it.
    pub created_by_id: Option<Uuid>,
    /// When.
    pub created_at: DateTime<Utc>,
    /// Last change.
    pub updated_at: DateTime<Utc>,
}

impl Methodology {
    /// The artefact's own classification.
    #[must_use]
    pub fn classification(&self) -> Classification {
        Classification::parse(&self.classification).unwrap_or(Classification::Internal)
    }
}

/// Uma versão de metodologia. **Um recurso, e não um campo.**
///
/// É o que torna a proveniência honesta: um resultado produzido com a versão 2
/// continua a dizer «versão 2» depois de a versão 5 existir. Se a aresta
/// apontasse para a metodologia, a linhagem passaria a descrever outra coisa
/// no dia em que alguém melhorasse o método — sem que ninguém a alterasse, e
/// sem que nada o dissesse.
#[derive(Debug, Clone, FromRow, Serialize)]
pub struct MethodologyVersion {
    /// Identifier.
    pub id: Uuid,
    /// The methodology it versions.
    pub methodology_id: Uuid,
    /// Monotonic order.
    pub sequence: i32,
    /// What it is called — «v1», «2024-rev-b».
    pub label: String,
    /// What this version says, in short.
    pub summary: String,
    /// The full document, when one exists.
    pub document_id: Option<Uuid>,
    /// Draft, published, superseded.
    pub status: String,
    /// The version that replaced it.
    pub superseded_by_id: Option<Uuid>,
    /// When it was published.
    pub published_at: Option<DateTime<Utc>>,
    /// Who wrote it.
    pub created_by_id: Option<Uuid>,
    /// When.
    pub created_at: DateTime<Utc>,
}

impl MethodologyVersion {
    /// Como um membro lê o estado desta versão.
    ///
    /// «Em vigor» e não «publicada»: publicada é o que ela é; em vigor é o que
    /// importa a quem escolhe uma. Duas versões publicadas e uma substituída
    /// lêem-se como uma escolha; duas «publicadas» lêem-se como um empate.
    #[must_use]
    pub fn status_label(&self) -> &'static str {
        if self.superseded_by_id.is_some() {
            return "Substituída";
        }
        match self.status.as_str() {
            "published" => "Em vigor",
            "superseded" => "Substituída",
            _ => "Rascunho",
        }
    }
}

/// Um estudo: experimento físico, simulação ou análise.
///
/// # Porque uma entidade e não três
///
/// Porque partilham tudo o que importa a esta camada: pertencem a um ambiente,
/// testam uma hipótese, seguem uma metodologia, consomem dados, executam-se e
/// produzem resultados. O que os distingue — bancada, malha, série temporal —
/// é detalhe de cada disciplina, e não pertence ao núcleo.
///
/// Três tabelas obrigariam a triplicar cada consulta de linhagem, e a decidir
/// em qual procurar antes de saber o que se procura.
#[derive(Debug, Clone, FromRow, Serialize)]
pub struct Study {
    /// Identifier.
    pub id: Uuid,
    /// The unit it belongs to.
    pub unit_id: Uuid,
    /// The research environment, when it has one.
    pub workspace_id: Option<Uuid>,
    /// The project, when it has one.
    pub project_id: Option<Uuid>,
    /// The hypothesis it tests, when it tests one.
    pub hypothesis_id: Option<Uuid>,
    /// What it is called.
    pub title: String,
    /// Physical experiment, simulation, or analysis.
    pub kind: String,
    /// What it sets out to find.
    pub objective: Option<String>,
    /// Where it stands.
    pub status: String,
    /// Its own classification.
    pub classification: String,
    /// Who created it.
    pub created_by_id: Option<Uuid>,
    /// When.
    pub created_at: DateTime<Utc>,
    /// Last change.
    pub updated_at: DateTime<Utc>,
}

impl Study {
    /// The artefact's own classification.
    #[must_use]
    pub fn classification(&self) -> Classification {
        Classification::parse(&self.classification).unwrap_or(Classification::Internal)
    }

    /// Como um membro lê este estado.
    #[must_use]
    pub fn status_label(&self) -> &'static str {
        match self.status.as_str() {
            "running" => "A correr",
            "completed" => "Concluído",
            "abandoned" => "Abandonado",
            _ => "Planeado",
        }
    }

    /// What a member calls this kind.
    #[must_use]
    pub fn kind_label(&self) -> &'static str {
        match self.kind.as_str() {
            "simulation" => "Simulação",
            "analysis" => "Análise",
            _ => "Experimento",
        }
    }
}

/// Uma corrida concreta de um estudo.
///
/// É aqui que a reprodutibilidade mora: o mesmo estudo corre duas vezes e dá
/// duas execuções, e são elas — não o estudo — que se comparam.
#[derive(Debug, Clone, FromRow, Serialize)]
pub struct StudyExecution {
    /// Identifier.
    pub id: Uuid,
    /// The study it runs.
    pub study_id: Uuid,
    /// Monotonic order within the study.
    pub sequence: i32,
    /// Where it stands.
    pub status: String,
    /// When it started.
    pub started_at: Option<DateTime<Utc>>,
    /// When it finished.
    pub finished_at: Option<DateTime<Utc>>,
    /// The compute node it ran on, when it ran on one the Ocinye knows.
    pub compute_node_id: Option<Uuid>,
    /// Free description of where it ran.
    pub environment: Option<String>,
    /// The software that ran it.
    pub software_name: Option<String>,
    /// Its version.
    pub software_version: Option<String>,
    /// Its commit.
    pub software_commit: Option<String>,
    /// The image digest, when containerised.
    pub image_digest: Option<String>,
    /// The configuration used.
    pub configuration: Option<String>,
    /// Anything else worth recording.
    pub notes: Option<String>,
    /// Who recorded it.
    pub created_by_id: Option<Uuid>,
    /// When.
    pub created_at: DateTime<Utc>,
}

/// Um resultado científico.
///
/// Não é uma publicação: publicar é um desfecho posterior e possível. Não é um
/// protótipo. É a evidência ou a conclusão que o trabalho produziu — incluindo
/// quando essa conclusão é que a hipótese não se sustenta.
#[derive(Debug, Clone, FromRow, Serialize)]
pub struct Result {
    /// Identifier.
    pub id: Uuid,
    /// The unit it belongs to.
    pub unit_id: Uuid,
    /// The research environment, when it has one.
    pub workspace_id: Option<Uuid>,
    /// The project, when it has one.
    pub project_id: Option<Uuid>,
    /// The execution that produced it, when one did.
    pub execution_id: Option<Uuid>,
    /// What it is called.
    pub title: String,
    /// What it says.
    pub summary: String,
    /// Where it stands.
    pub status: String,
    /// Its own classification.
    pub classification: String,
    /// The result that replaced it.
    pub superseded_by_id: Option<Uuid>,
    /// Who recorded it.
    pub created_by_id: Option<Uuid>,
    /// When.
    pub created_at: DateTime<Utc>,
    /// Last change.
    pub updated_at: DateTime<Utc>,
}

impl Result {
    /// Como um membro lê este estado.
    #[must_use]
    pub fn status_label(&self) -> &'static str {
        match self.status.as_str() {
            "under_review" => "Em revisão",
            "validated" => "Validado",
            "superseded" => "Substituído",
            "invalidated" => "Invalidado",
            _ => "Registado",
        }
    }

    /// The artefact's own classification.
    #[must_use]
    pub fn classification(&self) -> Classification {
        Classification::parse(&self.classification).unwrap_or(Classification::Internal)
    }
}

/// A prova de que alguém validou ou reproduziu um resultado.
///
/// Reprodutibilidade é evidência, e não um rótulo: um resultado não fica
/// «reproduzido» porque alguém escreveu que o pretendia reproduzir. Fica
/// reproduzido quando existe outra execução e alguém registou o que ela
/// mostrou — incluindo quando mostrou o contrário.
#[derive(Debug, Clone, FromRow, Serialize)]
pub struct ResultValidation {
    /// Identifier.
    pub id: Uuid,
    /// The result it speaks about.
    pub result_id: Uuid,
    /// Validation, or reproduction.
    pub kind: String,
    /// Confirmed, contradicted, inconclusive.
    pub outcome: String,
    /// The execution that served as proof, when there was one.
    pub execution_id: Option<Uuid>,
    /// The exact methodology version used, when it differs.
    pub methodology_version_id: Option<Uuid>,
    /// What was observed.
    pub note: Option<String>,
    /// Who did it.
    pub performed_by_id: Option<Uuid>,
    /// When.
    pub created_at: DateTime<Utc>,
}

impl ResultValidation {
    /// What a member reads.
    #[must_use]
    pub fn label(&self) -> String {
        let genero = if self.kind == "reproduction" {
            "Reprodução"
        } else {
            "Validação"
        };
        let desfecho = match self.outcome.as_str() {
            "confirmed" => "confirmou",
            "contradicted" => "contradisse",
            _ => "foi inconclusiva",
        };
        format!("{genero} {desfecho}")
    }
}
