//! D005 · Dados. Um dataset é um recurso lógico de investigação; as suas
//! versões são revisões do dataset — **não** versões de um ficheiro — e os
//! ficheiros de uma versão mostram-se pelo caminho lógico, nunca pelo objecto
//! guardado. Não há pré-visualização de conteúdo.

use std::collections::HashMap;

use serde_json::Value;

use super::state;
use crate::controllers::desktop::{bytes, instant, text, Clock};
use crate::i18n::{t, tf};
use crate::ui::view_models::{
    DatasetFileVm, DatasetStatus, DatasetVersionStatus, DatasetVersionVm, ResItemVm, ResOptionVm,
    ResStateVm, ResTone,
};

/// O estado do dataset, um a um.
#[must_use]
pub fn status(v: &str) -> Option<DatasetStatus> {
    Some(match v {
        "draft" => DatasetStatus::Draft,
        "active" => DatasetStatus::Active,
        "deprecated" => DatasetStatus::Deprecated,
        "archived" => DatasetStatus::Archived,
        _ => return None,
    })
}

/// O rótulo e o tom do estado do dataset.
#[must_use]
pub const fn state_vm(s: DatasetStatus) -> ResStateVm {
    match s {
        DatasetStatus::Draft => state("data.state.draft", ResTone::Neutral),
        DatasetStatus::Active => state("data.state.active", ResTone::Done),
        DatasetStatus::Deprecated => state("data.state.deprecated", ResTone::Attention),
        DatasetStatus::Archived => state("data.state.archived", ResTone::Closed),
    }
}

/// O estado de uma **versão do dataset** — outro vocabulário, outro tipo.
#[must_use]
pub fn version_status(v: &str) -> Option<DatasetVersionStatus> {
    Some(match v {
        "draft" => DatasetVersionStatus::Draft,
        "published" => DatasetVersionStatus::Published,
        "withdrawn" => DatasetVersionStatus::Withdrawn,
        _ => return None,
    })
}

/// A origem (vocabulário fechado), traduzida.
#[must_use]
pub fn origin_key(v: &str) -> Option<&'static str> {
    Some(match v {
        "collected_by_ocinye" => "data.origin.collected_by_ocinye",
        "derived" => "data.origin.derived",
        "third_party_open" => "data.origin.third_party_open",
        "third_party_licensed" => "data.origin.third_party_licensed",
        "partner_provided" => "data.origin.partner_provided",
        "simulated" => "data.origin.simulated",
        _ => return None,
    })
}

/// As origens para o formulário.
#[must_use]
pub fn origins(selected: &str) -> Vec<ResOptionVm> {
    [
        "collected_by_ocinye",
        "derived",
        "third_party_open",
        "third_party_licensed",
        "partner_provided",
        "simulated",
    ]
    .into_iter()
    .filter_map(|v| {
        Some(ResOptionVm {
            value: v.to_owned(),
            label: t(origin_key(v)?).to_owned(),
            selected: v == selected,
        })
    })
    .collect()
}

/// O pedido ao Core para o catálogo.
#[must_use]
pub fn list_path(workspace: Option<&str>, page: u32) -> String {
    let w = workspace
        .filter(|w| uuid::Uuid::parse_str(w).is_ok())
        .map(|w| format!("&workspace_id={w}"))
        .unwrap_or_default();
    format!("/api/v1/datasets?page={page}&page_size=25{w}")
}

/// A lista com o filtro corrente.
#[must_use]
pub fn list_href(workspace: Option<&str>) -> String {
    workspace
        .filter(|w| uuid::Uuid::parse_str(w).is_ok())
        .map_or_else(
            || "/datasets".to_owned(),
            |w| format!("/datasets?workspace={w}"),
        )
}

/// As colunas.
pub const COLUMNS: [&str; 2] = ["data.col.workspace", "data.col.origin"];

/// As linhas do catálogo.
#[must_use]
pub fn items(
    list: &[Value],
    open: Option<&str>,
    workspaces: &HashMap<String, String>,
) -> Vec<ResItemVm> {
    list.iter()
        .filter_map(|d| {
            let id = text(d, "id");
            if id.is_empty() {
                return None;
            }
            Some(ResItemVm {
                title: text(d, "title").to_owned(),
                code: Some(text(d, "code").to_owned()).filter(|c| !c.is_empty()),
                state: status(text(d, "state")).map(state_vm),
                cells: vec![
                    workspaces.get(text(d, "workspace_id")).cloned(),
                    origin_key(text(d, "origin")).map(|k| t(k).to_owned()),
                ],
                overdue: false,
                priority: None,
                href: format!("/datasets/{id}"),
                active: open == Some(id),
            })
        })
        .collect()
}

/// «4 ficheiros · 1,2 GB», na língua.
#[must_use]
pub fn totals(count: u64, size: u64) -> String {
    let key = if count == 1 {
        "prod.data.totals.one"
    } else {
        "prod.data.totals.other"
    };
    tf(key, &[("n", &count.to_string()), ("size", &bytes(size))])
}

/// As versões do dataset (`GET /datasets/{id}/versions`), da mais recente para
/// a mais antiga. As acções de rascunho só aparecem quando a versão é rascunho
/// **e** o ambiente deixa criar (`may_create`); publicar exige ainda ficheiros.
/// O Core volta a decidir cada uma.
#[must_use]
pub(crate) fn versions(
    list: &[Value],
    dataset: &str,
    open: Option<&str>,
    may_create: bool,
    clock: &Clock,
) -> Vec<DatasetVersionVm> {
    let aberta = open
        .map(str::to_owned)
        .or_else(|| list.first().map(|v| text(v, "label").to_owned()));
    list.iter()
        .filter_map(|v| {
            let id = text(v, "id");
            let label = text(v, "label");
            let status = version_status(text(v, "status"))?;
            let rascunho = status == DatasetVersionStatus::Draft && may_create;
            let n = v.get("file_count").and_then(Value::as_u64).unwrap_or(0);
            let size = v
                .get("total_size_bytes")
                .and_then(Value::as_u64)
                .unwrap_or(0);
            Some(DatasetVersionVm {
                label: format!("v{label}"),
                status,
                published: instant(v, "published_at").map(|at| clock.when(at)),
                notes: super::opt(v, "notes"),
                provenance: super::opt(v, "provenance"),
                // O Core não devolve a versão de origem (FG-D5-45 parcial).
                derived_from: None,
                withdrawn_reason: None,
                totals: (n > 0).then(|| totals(n, size)),
                files: v
                    .get("files")
                    .and_then(Value::as_array)
                    .map(|f| {
                        f.iter()
                            .map(|f| DatasetFileVm {
                                path: text(f, "path").to_owned(),
                                size: f.get("size_bytes").and_then(Value::as_u64).map(bytes),
                                // Um ficheiro de dataset é um objecto guardado,
                                // não um recurso de Ficheiros: sem elo (FG-D5-44).
                                href: None,
                            })
                            .collect()
                    })
                    .unwrap_or_default(),
                add_file_action: rascunho
                    .then(|| format!("/datasets/{dataset}/versions/{id}/files")),
                publish_action: (rascunho && n > 0)
                    .then(|| format!("/datasets/{dataset}/versions/{id}/publish")),
                open: aberta.as_deref() == Some(label),
                href: format!("/datasets/{dataset}?v={label}"),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn relogio() -> Clock {
        Clock {
            now: chrono::Utc::now(),
            zone: ocinye_contracts::temporal::TimeZoneName::parse("UTC").unwrap(),
            core_ok: true,
            is_admin: false,
        }
    }

    #[test]
    fn a_versao_do_dataset_nao_e_a_versao_de_um_ficheiro() {
        let vs = json!([
            { "id": "v2", "label": "2", "status": "draft", "file_count": 1, "total_size_bytes": 10,
              "files": [{ "id": "f1", "path": "medicoes/set.csv", "size_bytes": 10,
                          "storage_object_id": "o1", "checksum_sha256": "ab" }] },
            { "id": "v1", "label": "1", "status": "published", "file_count": 0, "total_size_bytes": 0, "files": [] }
        ]);
        let v = versions(vs.as_array().unwrap(), "d", None, true, &relogio());
        // Endereços da versão pelo rótulo, no dataset; nunca por um ficheiro.
        assert_eq!(v[0].href, "/datasets/d?v=2");
        assert!(v[0].open && !v[1].open);
        assert_eq!(
            v[0].add_file_action.as_deref(),
            Some("/datasets/d/versions/v2/files")
        );
        assert_eq!(
            v[0].publish_action.as_deref(),
            Some("/datasets/d/versions/v2/publish")
        );
        // O ficheiro é caminho lógico, sem elo e sem o objecto guardado.
        assert_eq!(v[0].files[0].path, "medicoes/set.csv");
        assert_eq!(v[0].files[0].href, None);
        // Publicada: sem acções de rascunho.
        assert!(v[1].add_file_action.is_none() && v[1].publish_action.is_none());
        // Rascunho sem poder criar: sem acções.
        let v = versions(vs.as_array().unwrap(), "d", None, false, &relogio());
        assert!(v[0].add_file_action.is_none() && v[0].publish_action.is_none());
    }

    #[test]
    fn estados_do_dataset_e_da_versao_sao_vocabularios_separados() {
        assert_eq!(
            status("published"),
            None,
            "«publicado» é de versão, não de dataset"
        );
        assert_eq!(
            version_status("active"),
            None,
            "«activo» é de dataset, não de versão"
        );
        assert_eq!(status("active"), Some(DatasetStatus::Active));
        assert_eq!(
            version_status("withdrawn"),
            Some(DatasetVersionStatus::Withdrawn)
        );
    }

    #[test]
    fn rascunho_sem_ficheiros_nao_oferece_publicar() {
        let vs =
            json!([{ "id": "v1", "label": "1", "status": "draft", "file_count": 0, "files": [] }]);
        let v = versions(vs.as_array().unwrap(), "d", None, true, &relogio());
        assert!(v[0].add_file_action.is_some() && v[0].publish_action.is_none());
    }
}
