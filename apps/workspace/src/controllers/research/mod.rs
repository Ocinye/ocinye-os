//! D005 · Investigação e trabalho: o que as cinco aplicações partilham —
//! Projectos, O Meu Trabalho, Ideias, Dados e Conhecimento.
//!
//! O Core decide; aqui só se traduz o que ele devolveu para os ViewModels do
//! Design. Três regras atravessam tudo:
//!
//! - **Os estados mapeiam-se um a um.** Um valor que o Core ainda não tinha
//!   quando isto se escreveu não vira «em curso» nem «outro»: não tem
//!   rótulo, e o detalhe diz que não se consegue mostrar.
//! - **As transições são as que o Core devolve** (`available_transitions`);
//!   aqui escolhe-se só o rótulo do botão, nunca se o botão existe.
//! - **Uma relação só aparece se o Core resolveu as duas pontas** para este
//!   membro. Lêem-se pela linhagem (`/lineage`), que resolve cada nó com a
//!   política de quem pergunta; a tabela `research_links` não o faz.

pub mod datasets;
pub mod ideas;
pub mod knowledge;
pub mod projects;
pub mod work;

use serde_json::Value;

use crate::controllers::desktop::text;
use crate::i18n::t;
use crate::ui::view_models::{
    AppNavVm, AppNyeVm, AppPageVm, ResClassification, ResKind, ResLinkVm, ResOptionVm, ResPersonVm,
    ResRelation, ResStateVm, ResTone,
};

/// A classificação que o Core escreveu. Um valor desconhecido lê-se como a
/// mais restrita: na dúvida, o cadeado aparece.
#[must_use]
pub fn classification(v: &str) -> ResClassification {
    match v.to_ascii_uppercase().as_str() {
        "PUBLIC" => ResClassification::Public,
        "INTERNAL" => ResClassification::Internal,
        "CONFIDENTIAL" => ResClassification::Confidential,
        _ => ResClassification::Restricted,
    }
}

/// Uma data do Core (`AAAA-MM-DD`), mostrada `DD/MM/AAAA`.
#[must_use]
pub fn date(v: &str) -> Option<String> {
    chrono::NaiveDate::parse_from_str(v, "%Y-%m-%d")
        .ok()
        .map(|d| d.format("%d/%m/%Y").to_string())
}

/// Um instante como data no fuso do membro (`DD/MM/AAAA`).
#[must_use]
pub(crate) fn day(
    at: chrono::DateTime<chrono::Utc>,
    clock: &crate::controllers::desktop::Clock,
) -> String {
    at.with_timezone(&clock.zone.zone())
        .format("%d/%m/%Y")
        .to_string()
}

/// As palavras-chave de um campo `keywords` (lista de texto).
#[must_use]
pub fn keywords(v: &Value) -> Vec<String> {
    v.get("keywords")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(Value::as_str)
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

/// Um campo de texto opcional, sem vazio.
#[must_use]
pub fn opt(v: &Value, k: &str) -> Option<String> {
    let s = text(v, k).trim();
    (!s.is_empty()).then(|| s.to_owned())
}

/// Uma lista separada por vírgulas (o campo de palavras-chave do formulário).
#[must_use]
pub fn split_keywords(s: &str) -> Vec<String> {
    s.split(',')
        .map(str::trim)
        .filter(|k| !k.is_empty())
        .map(str::to_owned)
        .collect()
}

/// A paginação por página do Core, na forma do D004 (`AppPageVm`): a
/// página seguinte é o cursor. `base` já traz os filtros correntes.
#[must_use]
pub fn page(v: &Value, base: &str) -> AppPageVm {
    let n = v.get("page").and_then(Value::as_u64).unwrap_or(1);
    let total = v.get("total_pages").and_then(Value::as_u64).unwrap_or(1);
    let sep = if base.contains('?') { '&' } else { '?' };
    AppPageVm {
        more_href: (n < total).then(|| format!("{base}{sep}page={}", n + 1)),
        summary: None,
    }
}

/// Uma entrada de navegação.
#[must_use]
pub fn nav(label_key: &str, icon: &'static str, href: String, active: bool) -> AppNavVm {
    AppNavVm {
        label: t(label_key).to_owned(),
        icon,
        href,
        count: None,
        active,
    }
}

/// A Nye contextual com uma referência tipada (a mesma de D004).
#[must_use]
pub fn nye(kind: &str, id: &str, label_key: &'static str) -> AppNyeVm {
    crate::controllers::productivity::nye(kind, id, label_key)
}

/// As pessoas de um ambiente, pelo nome — só as que a leitura do ambiente
/// (`GET /workspaces/{id}`) devolve; nunca o directório da Instância.
#[must_use]
pub fn people(overview: &Value) -> Vec<ResPersonVm> {
    overview
        .get("members")
        .and_then(Value::as_array)
        .map(|m| {
            m.iter()
                .map(|p| ResPersonVm {
                    name: text(p, "full_name").to_owned(),
                    role: match text(p, "role") {
                        "lead" => Some(t("prod.res.role.lead").to_owned()),
                        "member" => Some(t("prod.res.role.member").to_owned()),
                        "viewer" => Some(t("prod.res.role.viewer").to_owned()),
                        _ => None,
                    },
                })
                .filter(|p| !p.name.is_empty())
                .collect()
        })
        .unwrap_or_default()
}

/// As pessoas de um ambiente como opções de um selector governado (atribuir,
/// responsável). O Core volta a validar a escolha.
#[must_use]
pub fn member_options(overview: &Value, selected: Option<&str>) -> Vec<ResOptionVm> {
    overview
        .get("members")
        .and_then(Value::as_array)
        .map(|m| {
            m.iter()
                .filter(|p| text(p, "role") != "viewer")
                .map(|p| ResOptionVm {
                    value: text(p, "person_id").to_owned(),
                    label: text(p, "full_name").to_owned(),
                    selected: Some(text(p, "person_id")) == selected,
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Se um identificador é de uma pessoa deste ambiente que pode receber
/// trabalho (lead ou member). A BFF só aceita estes; o Core volta a decidir.
#[must_use]
pub fn is_member(overview: &Value, person_id: &str) -> bool {
    member_options(overview, None)
        .iter()
        .any(|o| o.value == person_id)
}

/// O tipo de um nó da linhagem, e a ligação canónica quando tem ecrã. Um nó
/// sem ecrã fica texto — nunca um elo morto.
#[must_use]
pub fn kind_and_href(kind: &str, id: &str) -> (ResKind, Option<String>, Option<String>) {
    match kind {
        "project" => (ResKind::Project, Some(format!("/projects/{id}")), None),
        "idea" => (ResKind::Idea, Some(format!("/ideas/{id}")), None),
        "task" => (ResKind::Task, Some(format!("/my-work/{id}")), None),
        "dataset" => (ResKind::Dataset, Some(format!("/datasets/{id}")), None),
        "source" => (
            ResKind::Source,
            Some(format!("/knowledge/sources/{id}")),
            None,
        ),
        "document" => (
            ResKind::Document,
            Some(format!("/knowledge/documents/{id}")),
            None,
        ),
        // A versão de um dataset não tem endereço próprio sem o dataset; as
        // notas de ambiente e os ficheiros institucionais não têm ecrã.
        "dataset_version" => (ResKind::DatasetVersion, None, None),
        "note" => (ResKind::Note, None, None),
        "file" | "file_version" => (ResKind::File, None, None),
        other => (ResKind::Other, None, Some(other_label(other))),
    }
}

fn other_label(kind: &str) -> String {
    match kind {
        "hypothesis" => t("prod.res.kind.hypothesis"),
        "methodology" | "methodology_version" => t("prod.res.kind.methodology"),
        "study" | "study_execution" => t("prod.res.kind.study"),
        "result" => t("prod.res.kind.result"),
        _ => t("res.kind.other"),
    }
    .to_owned()
}

/// A relação tipada da linhagem (`ProvenanceRelation`, vocabulário fechado).
#[must_use]
pub fn relation(v: &str) -> Option<ResRelation> {
    Some(match v {
        "cites" => ResRelation::Cites,
        "supports" => ResRelation::Supports,
        "refutes" => ResRelation::Refutes,
        "derived_from" => ResRelation::DerivedFrom,
        "uses" => ResRelation::Uses,
        "produces" => ResRelation::Produces,
        "relates_to" => ResRelation::RelatesTo,
        "tests" => ResRelation::Tests,
        "follows" => ResRelation::Follows,
        "input_to" => ResRelation::InputTo,
        "produced_by" => ResRelation::ProducedBy,
        "executed_on" => ResRelation::ExecutedOn,
        "validates" => ResRelation::Validates,
        "reproduces" => ResRelation::Reproduces,
        "supersedes" => ResRelation::Supersedes,
        _ => return None,
    })
}

/// As relações de uma linhagem de profundidade 1: o outro extremo de cada
/// passo. Só chegam passos cujos dois nós o Core resolveu para este membro;
/// «registada pela operação» é o `origem` que o Core guardou, nunca um palpite.
#[must_use]
pub fn links(lineage: &Value) -> Vec<ResLinkVm> {
    let raiz = lineage.get("raiz");
    let raiz_id = raiz.map(|r| text(r, "id")).unwrap_or_default();
    lineage
        .get("passos")
        .and_then(Value::as_array)
        .map(|p| {
            p.iter()
                .filter(|s| s.get("profundidade").and_then(Value::as_u64) == Some(1))
                .filter_map(|s| {
                    let de = s.get("de")?;
                    let para = s.get("para")?;
                    let outro = if text(de, "id") == raiz_id { para } else { de };
                    let (kind, href, kind_label) =
                        kind_and_href(text(outro, "kind"), text(outro, "id"));
                    let title = text(outro, "label").trim().to_owned();
                    Some(ResLinkVm {
                        kind,
                        kind_label,
                        title: if title.is_empty() {
                            t("res.kind.other").to_owned()
                        } else {
                            title
                        },
                        meta: None,
                        relation: relation(text(s, "relacao")),
                        by_operation: text(s, "origem") == "operation",
                        href,
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Codifica um valor para uma pergunta de URL (só os caracteres não
/// reservados passam; o resto vai `%XX`).
#[must_use]
pub fn encode(value: &str) -> String {
    value
        .bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                char::from(b).to_string()
            }
            other => format!("%{other:02X}"),
        })
        .collect()
}

/// Um estado já com rótulo e tom.
#[must_use]
pub const fn state(key: &'static str, tone: ResTone) -> ResStateVm {
    ResStateVm { key, tone }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn uma_classificacao_desconhecida_le_se_como_restrita() {
        assert_eq!(classification("INTERNAL"), ResClassification::Internal);
        assert_eq!(
            classification("confidential"),
            ResClassification::Confidential
        );
        assert_eq!(classification("SEGREDO"), ResClassification::Restricted);
    }

    #[test]
    fn as_relacoes_sao_o_outro_extremo_e_a_origem_e_a_do_core() {
        let l = json!({
            "raiz": { "kind": "project", "id": "p1" },
            "passos": [
                { "profundidade": 1, "de": { "kind": "project", "id": "p1", "label": "P" },
                  "relacao": "uses", "para": { "kind": "dataset", "id": "d1", "label": "Medições" },
                  "origem": "operation" },
                { "profundidade": 1, "de": { "kind": "source", "id": "s1", "label": "Artigo" },
                  "relacao": "supports", "para": { "kind": "project", "id": "p1", "label": "P" },
                  "origem": "declared" },
                { "profundidade": 1, "de": { "kind": "project", "id": "p1" },
                  "relacao": "relates_to", "para": { "kind": "note", "id": "n1", "label": "Nota" },
                  "origem": "declared" },
                { "profundidade": 2, "de": { "kind": "dataset", "id": "d1" },
                  "relacao": "uses", "para": { "kind": "idea", "id": "i9", "label": "Longe" },
                  "origem": "declared" }
            ]
        });
        let v = links(&l);
        assert_eq!(v.len(), 3, "só a profundidade 1");
        assert_eq!(v[0].title, "Medições");
        assert_eq!(v[0].href.as_deref(), Some("/datasets/d1"));
        assert!(v[0].by_operation);
        assert_eq!(v[1].href.as_deref(), Some("/knowledge/sources/s1"));
        assert!(!v[1].by_operation);
        // Uma nota de ambiente não tem ecrã: texto, não elo.
        assert_eq!(v[2].href, None);
        assert_eq!(v[2].kind, ResKind::Note);
    }

    #[test]
    fn so_membros_que_trabalham_sao_candidatos() {
        let o = json!({ "members": [
            { "person_id": "a", "full_name": "Ana", "role": "lead" },
            { "person_id": "b", "full_name": "Bia", "role": "viewer" },
            { "person_id": "c", "full_name": "Caio", "role": "member" }
        ]});
        let c = member_options(&o, Some("c"));
        assert_eq!(c.len(), 2);
        assert!(c.iter().any(|o| o.value == "c" && o.selected));
        assert!(is_member(&o, "a") && !is_member(&o, "b") && !is_member(&o, "z"));
    }

    #[test]
    fn a_pagina_seguinte_guarda_os_filtros() {
        let p = page(&json!({ "page": 1, "total_pages": 3 }), "/my-work?nav=all");
        assert_eq!(p.more_href.as_deref(), Some("/my-work?nav=all&page=2"));
        assert_eq!(
            page(&json!({ "page": 3, "total_pages": 3 }), "/x").more_href,
            None
        );
    }
}
