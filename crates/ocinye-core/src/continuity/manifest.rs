//! O que o snapshot contém, de forma verificável.
//!
//! # Porque contagens não bastam
//!
//! Sessenta e três tabelas antes e sessenta e três depois não prova nada. Cento
//! e vinte datasets antes e cento e vinte depois também não: podem ser outros
//! cento e vinte.
//!
//! O que prova é **identidade**. Um `Result` criado em 2026 continua o mesmo
//! `Result` depois de dez migrações de servidor, e a proveniência que o liga
//! continua a ligar os mesmos recursos. Se os identificadores mudassem, teríamos
//! importado uma instituição parecida em vez de mudado a nossa de sítio.
//!
//! > **Server migration moves institutional state; it does not recreate
//! > institutional history.**
//!
//! Por isso o manifesto não conta linhas: recolhe os identificadores do estado
//! institucional e as somas do conteúdo binário, e a comparação exige que
//! coincidam elemento a elemento.

use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::CoreResult;

/// A versão do formato.
///
/// Um manifesto sem versão é um manifesto que ninguém sabe ler daqui a três
/// anos. Sobe quando o significado de um campo mudar, e não quando um campo é
/// acrescentado.
pub const FORMATO: u32 = 1;

/// Os identificadores de uma família de recursos.
///
/// Ordenados, sempre. Duas leituras da mesma base têm de produzir o mesmo
/// manifesto, ou a comparação passa a depender da ordem de varrimento.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Familia {
    /// Que recursos são.
    pub tabela: String,
    /// Quantos há. Redundante com `identidades`, e útil a quem lê.
    pub quantos: usize,
    /// Os identificadores, por ordem.
    pub identidades: Vec<Uuid>,
}

/// Um objecto guardado, e a soma do que ele contém.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Objecto {
    /// A identidade institucional do objecto.
    pub id: Uuid,
    /// A chave sob a qual está guardado.
    ///
    /// Uma chave, e não uma URL: o endereço do serviço é configuração da
    /// instalação, e prendê-lo aqui faria o conhecimento depender do sítio.
    pub chave: String,
    /// A soma SHA-256 do conteúdo, tal como a base a registou.
    pub sha256: String,
    /// Quantos bytes.
    pub bytes: i64,
}

/// Uma aresta de proveniência, pelas suas pontas.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Aresta {
    /// De onde parte.
    pub de: Uuid,
    /// O que afirma.
    pub relacao: String,
    /// Para onde vai.
    pub para: Uuid,
    /// Observada pela operação, ou declarada por alguém.
    pub origem: String,
}

/// O que um snapshot institucional contém.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifesto {
    /// A versão do formato deste manifesto.
    pub formato: u32,
    /// O nível de migrations da base descrita.
    ///
    /// Restaurar para um esquema mais recente sem passar pelo compatível
    /// confunde uma falha de transporte com uma falha de evolução. O nível fica
    /// registado para que a diferença seja visível antes de doer.
    pub migrations: i64,
    /// As famílias de recursos institucionais, com as suas identidades.
    pub familias: Vec<Familia>,
    /// Os objectos guardados, com as suas somas.
    pub objectos: Vec<Objecto>,
    /// As arestas de proveniência.
    pub proveniencia: Vec<Aresta>,
    /// Quantos eventos de auditoria, e o mais antigo e o mais recente.
    ///
    /// A auditoria não se compara evento a evento — são muitos — mas um restore
    /// que a recriasse teria datas novas, e isso vê-se aqui.
    pub auditoria: Auditoria,
}

/// O que se guarda sobre a auditoria, sem a copiar inteira.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Auditoria {
    /// Quantos eventos.
    pub eventos: i64,
    /// O primeiro, em ISO-8601. `None` quando não há nenhum.
    pub primeiro: Option<String>,
    /// O último.
    pub ultimo: Option<String>,
}

/// As famílias que constituem estado institucional autoritativo.
///
/// # Porque uma lista escrita, e não «todas as tabelas»
///
/// Porque nem toda a tabela guarda identidade institucional. `search_documents`
/// é uma projecção; `sessions` é efémero. Compará-las obrigaria um snapshot
/// legítimo a falhar por uma sessão ter expirado entre as duas leituras.
///
/// O que está aqui é o que tem de sobreviver com a **mesma identidade**.
const FAMILIAS: &[&str] = &[
    "organisations",
    "people",
    "units",
    "research_workspaces",
    "ideas",
    "projects",
    "sources",
    "notes",
    "documents",
    "datasets",
    "dataset_versions",
    "hypotheses",
    "methodologies",
    "methodology_versions",
    "studies",
    "study_executions",
    "results",
    "result_validations",
    "tasks",
    "calendar_events",
    "conversations",
    "mailboxes",
    "storage_objects",
    "explicit_access_grants",
];

/// Lê o que esta instalação contém.
///
/// # Errors
///
/// Devolve erro quando a base não responde. Uma descrição parcial seria pior do
/// que nenhuma: comparar-se-ia contra ela e a diferença passaria por igualdade.
pub async fn descrever(pool: &PgPool) -> CoreResult<Manifesto> {
    let migrations: i64 =
        sqlx::query_scalar("SELECT COALESCE(MAX(version), 0) FROM _sqlx_migrations")
            .fetch_one(pool)
            .await?;

    let mut familias = Vec::with_capacity(FAMILIAS.len());
    for tabela in FAMILIAS {
        // `ORDER BY id` e não a ordem de varrimento: duas leituras da mesma base
        // têm de dar o mesmo manifesto, ou a comparação mede o planeador de
        // consultas em vez do conteúdo.
        let identidades: Vec<Uuid> =
            sqlx::query_scalar(&format!("SELECT id FROM {tabela} ORDER BY id"))
                .fetch_all(pool)
                .await?;
        familias.push(Familia {
            tabela: (*tabela).to_owned(),
            quantos: identidades.len(),
            identidades,
        });
    }

    let objectos: Vec<Objecto> = sqlx::query_as::<_, (Uuid, String, String, i64)>(
        "SELECT id, object_key, checksum_sha256, size_bytes
           FROM storage_objects ORDER BY id",
    )
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(|(id, chave, sha256, bytes)| Objecto {
        id,
        chave,
        sha256,
        bytes,
    })
    .collect();

    let proveniencia: Vec<Aresta> = sqlx::query_as::<_, (Uuid, String, Uuid, String)>(
        "SELECT source_id, relation, target_id, origin
           FROM research_links ORDER BY source_id, relation, target_id",
    )
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(|(de, relacao, para, origem)| Aresta {
        de,
        relacao,
        para,
        origem,
    })
    .collect();

    let (eventos, primeiro, ultimo): (i64, Option<String>, Option<String>) = sqlx::query_as(
        "SELECT COUNT(*),
                    to_char(MIN(occurred_at) AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SSZ'),
                    to_char(MAX(occurred_at) AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SSZ')
               FROM audit_events",
    )
    .fetch_one(pool)
    .await?;

    Ok(Manifesto {
        formato: FORMATO,
        migrations,
        familias,
        objectos,
        proveniencia,
        auditoria: Auditoria {
            eventos,
            primeiro,
            ultimo,
        },
    })
}

/// Uma diferença entre o que se esperava e o que chegou.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Divergencia {
    /// Onde.
    pub onde: String,
    /// O quê.
    pub o_que: String,
}

/// Compara o que se levou com o que chegou.
///
/// # O que isto exige, e o que não aceita
///
/// Exige identidade elemento a elemento. Não aceita «o mesmo número»: cento e
/// vinte recursos antes e cento e vinte depois podem ser outros cento e vinte, e
/// um restore que recriasse tudo com identificadores novos passaria por uma
/// comparação de contagens sem ninguém notar que a proveniência ficou a apontar
/// para o nada.
#[must_use]
pub fn comparar(antes: &Manifesto, depois: &Manifesto) -> Vec<Divergencia> {
    let mut divergencias = Vec::new();

    if antes.formato != depois.formato {
        divergencias.push(Divergencia {
            onde: "formato".to_owned(),
            o_que: format!(
                "o snapshot é do formato {} e esta leitura é do {}; os campos \
                 podem não significar o mesmo",
                antes.formato, depois.formato
            ),
        });
        // Sem formatos iguais, o resto da comparação não é de confiança.
        return divergencias;
    }

    if antes.migrations != depois.migrations {
        divergencias.push(Divergencia {
            onde: "migrations".to_owned(),
            o_que: format!(
                "o snapshot foi tirado no nível {} e esta base está no {}. \
                 Restaurar para um esquema diferente confunde uma falha de \
                 transporte com uma falha de evolução: restaurar no nível \
                 compatível, verificar, e só depois evoluir",
                antes.migrations, depois.migrations
            ),
        });
    }

    for esperada in &antes.familias {
        let Some(chegada) = depois.familias.iter().find(|f| f.tabela == esperada.tabela) else {
            divergencias.push(Divergencia {
                onde: esperada.tabela.clone(),
                o_que: "a família não existe do outro lado".to_owned(),
            });
            continue;
        };

        let antes_ids: std::collections::BTreeSet<&Uuid> = esperada.identidades.iter().collect();
        let depois_ids: std::collections::BTreeSet<&Uuid> = chegada.identidades.iter().collect();

        let perdidos: Vec<&&Uuid> = antes_ids.difference(&depois_ids).take(5).collect();
        let aparecidos: Vec<&&Uuid> = depois_ids.difference(&antes_ids).take(5).collect();

        if !perdidos.is_empty() {
            divergencias.push(Divergencia {
                onde: esperada.tabela.clone(),
                o_que: format!(
                    "{} identidade(s) não chegaram, por exemplo {perdidos:?}",
                    antes_ids.difference(&depois_ids).count()
                ),
            });
        }
        if !aparecidos.is_empty() {
            divergencias.push(Divergencia {
                onde: esperada.tabela.clone(),
                o_que: format!(
                    "{} identidade(s) apareceram do nada, por exemplo \
                     {aparecidos:?}. Um restore move estado; não cria história",
                    depois_ids.difference(&antes_ids).count()
                ),
            });
        }
    }

    // ── Os bytes, e não só a referência ─────────────────────────────────
    //
    // Um objecto cuja linha sobreviveu e cujo conteúdo mudou é pior do que um
    // objecto em falta: a instituição continua a citar um dataset que já não é
    // aquele, e nada o diz.
    for esperado in &antes.objectos {
        match depois.objectos.iter().find(|o| o.id == esperado.id) {
            None => divergencias.push(Divergencia {
                onde: "storage_objects".to_owned(),
                o_que: format!("o objecto {} não chegou", esperado.id),
            }),
            Some(chegado) if chegado.sha256 != esperado.sha256 => {
                divergencias.push(Divergencia {
                    onde: "storage_objects".to_owned(),
                    o_que: format!(
                        "o objecto {} chegou com outro conteúdo: esperava \
                         {}…, chegou {}…",
                        esperado.id,
                        &esperado.sha256[..esperado.sha256.len().min(12)],
                        &chegado.sha256[..chegado.sha256.len().min(12)]
                    ),
                });
            }
            Some(chegado) if chegado.chave != esperado.chave => {
                divergencias.push(Divergencia {
                    onde: "storage_objects".to_owned(),
                    o_que: format!(
                        "o objecto {} mudou de chave: «{}» → «{}»",
                        esperado.id, esperado.chave, chegado.chave
                    ),
                });
            }
            Some(_) => {}
        }
    }

    // ── A proveniência ──────────────────────────────────────────────────
    //
    // Comparada como conjunto de arestas, incluindo a origem: uma aresta que
    // chegasse marcada `declared` onde era `operation` diria que alguém a
    // afirmou, quando foi o Core que a observou.
    let antes_arestas: std::collections::BTreeSet<&Aresta> = antes.proveniencia.iter().collect();
    let depois_arestas: std::collections::BTreeSet<&Aresta> = depois.proveniencia.iter().collect();
    let perdidas = antes_arestas.difference(&depois_arestas).count();
    if perdidas > 0 {
        divergencias.push(Divergencia {
            onde: "research_links".to_owned(),
            o_que: format!(
                "{perdidas} aresta(s) de proveniência não chegaram; a linhagem \
                 deixou de poder ser percorrida por elas"
            ),
        });
    }

    // ── A auditoria ─────────────────────────────────────────────────────
    if depois.auditoria.eventos < antes.auditoria.eventos {
        divergencias.push(Divergencia {
            onde: "audit_events".to_owned(),
            o_que: format!(
                "chegaram {} eventos de {}",
                depois.auditoria.eventos, antes.auditoria.eventos
            ),
        });
    }
    if antes.auditoria.primeiro != depois.auditoria.primeiro {
        divergencias.push(Divergencia {
            onde: "audit_events".to_owned(),
            o_que: format!(
                "o evento mais antigo mudou de data: {:?} → {:?}. Um restore que \
                 recriasse a auditoria teria datas de hoje, e a evidência \
                 desapareceria exactamente quando é mais precisa",
                antes.auditoria.primeiro, depois.auditoria.primeiro
            ),
        });
    }

    divergencias
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vazio() -> Manifesto {
        Manifesto {
            formato: FORMATO,
            migrations: 19,
            familias: vec![Familia {
                tabela: "results".to_owned(),
                quantos: 0,
                identidades: Vec::new(),
            }],
            objectos: Vec::new(),
            proveniencia: Vec::new(),
            auditoria: Auditoria {
                eventos: 0,
                primeiro: None,
                ultimo: None,
            },
        }
    }

    fn com_resultado(id: Uuid) -> Manifesto {
        let mut m = vazio();
        m.familias[0].quantos = 1;
        m.familias[0].identidades = vec![id];
        m
    }

    /// Dois manifestos iguais não divergem.
    ///
    /// O controlo positivo: sem ele, os testes abaixo passariam com um
    /// comparador que reportasse divergência em tudo.
    #[test]
    fn o_mesmo_estado_nao_diverge() {
        let id = Uuid::new_v4();
        assert!(comparar(&com_resultado(id), &com_resultado(id)).is_empty());
    }

    /// O mesmo número de recursos com identidades diferentes **diverge**.
    ///
    /// É a propriedade central. Um restore que recriasse tudo com
    /// identificadores novos passaria por qualquer comparação de contagens, e
    /// deixaria a proveniência a apontar para o nada.
    #[test]
    fn a_mesma_contagem_com_outras_identidades_e_uma_divergencia() {
        let divergencias = comparar(
            &com_resultado(Uuid::new_v4()),
            &com_resultado(Uuid::new_v4()),
        );
        assert_eq!(
            divergencias.len(),
            2,
            "esperava uma perda e um aparecimento, e vieram {divergencias:?}"
        );
        assert!(divergencias
            .iter()
            .any(|d| d.o_que.contains("não chegaram")));
        assert!(divergencias
            .iter()
            .any(|d| d.o_que.contains("apareceram do nada")));
    }

    /// Um objecto com a linha certa e outro conteúdo diverge.
    #[test]
    fn um_objecto_com_outro_conteudo_e_uma_divergencia() {
        let id = Uuid::new_v4();
        let objecto = |sha: &str| Objecto {
            id,
            chave: "research/a/dados.parquet".to_owned(),
            sha256: sha.to_owned(),
            bytes: 10,
        };
        let mut antes = vazio();
        antes.objectos = vec![objecto(&"a".repeat(64))];
        let mut depois = vazio();
        depois.objectos = vec![objecto(&"b".repeat(64))];

        let divergencias = comparar(&antes, &depois);
        assert!(
            divergencias
                .iter()
                .any(|d| d.o_que.contains("outro conteúdo")),
            "um dataset que mudou de conteúdo passou por igual: {divergencias:?}"
        );
    }

    /// Uma auditoria recriada hoje diverge, mesmo com o mesmo número.
    #[test]
    fn uma_auditoria_com_datas_novas_e_uma_divergencia() {
        let mut antes = vazio();
        antes.auditoria = Auditoria {
            eventos: 500,
            primeiro: Some("2026-08-23T10:00:00Z".to_owned()),
            ultimo: Some("2026-08-28T22:00:00Z".to_owned()),
        };
        let mut depois = vazio();
        depois.auditoria = Auditoria {
            eventos: 500,
            primeiro: Some("2026-09-01T09:00:00Z".to_owned()),
            ultimo: Some("2026-09-01T09:00:00Z".to_owned()),
        };

        let divergencias = comparar(&antes, &depois);
        assert!(
            divergencias
                .iter()
                .any(|d| d.o_que.contains("mais antigo mudou de data")),
            "uma auditoria recriada passou por preservada: {divergencias:?}"
        );
    }

    /// Uma aresta de proveniência em falta diverge.
    #[test]
    fn uma_aresta_perdida_e_uma_divergencia() {
        let mut antes = vazio();
        antes.proveniencia = vec![Aresta {
            de: Uuid::new_v4(),
            relacao: "produced_by".to_owned(),
            para: Uuid::new_v4(),
            origem: "operation".to_owned(),
        }];
        let divergencias = comparar(&antes, &vazio());
        assert!(divergencias.iter().any(|d| d.onde == "research_links"));
    }

    /// Restaurar para outro nível de esquema é dito, e não descoberto.
    #[test]
    fn um_nivel_de_migrations_diferente_e_dito() {
        let mut depois = vazio();
        depois.migrations = 28;
        let divergencias = comparar(&vazio(), &depois);
        assert!(
            divergencias.iter().any(|d| d.onde == "migrations"),
            "restaurar para outro esquema passou em silêncio"
        );
    }
}
