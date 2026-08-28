//! O ciclo científico: hipótese, metodologia, estudo, execução, resultado.
//!
//! # Porque um módulo novo e não uma extensão de `knowledge`
//!
//! Porque `knowledge` responde a «o que a instituição sabe» — referências,
//! notas, documentos. Este responde a «como a instituição chegou a sabê-lo»,
//! que é outra pergunta e tem outros invariantes: uma versão de metodologia é
//! imutável, uma execução é irrepetível, um resultado tem estado.
//!
//! A proveniência que os liga continua em `knowledge`, onde `research_links`
//! sempre esteve. Não se move o que já funciona só para ficar arrumado ao pé
//! do que é novo.

pub mod lineage;
pub mod model;
pub mod repository;
pub mod service;

pub use lineage::{percorrer, Linhagem, Passo, Sentido, PROFUNDIDADE_MAXIMA};
pub use model::{
    Hypothesis, Methodology, MethodologyVersion, Result, ResultValidation, Study, StudyExecution,
};
pub use service::{
    create_hypothesis, create_methodology, create_result, create_study, get_execution,
    get_hypothesis, get_methodology, get_methodology_version, get_result, get_study,
    list_executions, list_hypotheses, list_methodologies, list_methodology_versions, list_results,
    list_studies, list_validations, publish_methodology_version, record_execution,
    record_validation, ExecutionRecord,
};
