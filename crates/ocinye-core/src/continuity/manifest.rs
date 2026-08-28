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

/// Como cada tabela do esquema entra — ou não entra — na comparação.
///
/// Existe para que a pergunta não tenha resposta implícita. Uma tabela nova
/// aparece numa migration; se a decisão sobre ela pudesse ficar por tomar, o
/// manifesto continuaria a passar por completo enquanto deixava de cobrir
/// aquilo que a tabela guarda.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Comparacao {
    /// Identidade a identidade, pela coluna `id`.
    Identidades,
    /// Comparada por outro mecanismo deste manifesto, nomeado aqui.
    Noutro(&'static str),
    /// Deliberadamente fora da comparação, com a razão escrita.
    Fora(&'static str),
}

/// O esquema inteiro, e a decisão de continuidade de cada tabela.
///
/// # Porque a lista é exaustiva, e não uma selecção
///
/// Porque uma selecção envelhece em silêncio. A versão anterior deste ficheiro
/// comparava vinte e quatro tabelas de sessenta e duas e não dizia nada sobre
/// as outras trinta e oito — entre elas `person_roles`, `unit_memberships` e
/// `credentials`. Um restore que perdesse todas as filiações passaria por
/// íntegro: as famílias comparadas chegariam completas, e ninguém conseguiria
/// entrar.
///
/// Aqui, uma tabela que não esteja nesta lista faz o portão fechar. A decisão
/// pode ser «fora», e várias são — mas tem de ser tomada por alguém.
const ESQUEMA: &[(&str, Comparacao)] = &[
    // ── Identidade e autoridade ─────────────────────────────────────────
    //
    // Perder uma linha destas não perde conhecimento: perde quem lhe pode
    // chegar. Um restore que trouxesse a investigação inteira e nenhuma
    // filiação seria um arquivo, e não uma instituição a funcionar.
    ("organisations", Comparacao::Identidades),
    ("people", Comparacao::Identidades),
    ("credentials", Comparacao::Identidades),
    ("person_roles", Comparacao::Identidades),
    ("units", Comparacao::Identidades),
    ("unit_memberships", Comparacao::Identidades),
    ("workspace_memberships", Comparacao::Identidades),
    ("explicit_access_grants", Comparacao::Identidades),
    ("invitations", Comparacao::Identidades),
    // ── Investigação ────────────────────────────────────────────────────
    ("research_workspaces", Comparacao::Identidades),
    ("ideas", Comparacao::Identidades),
    ("projects", Comparacao::Identidades),
    ("tasks", Comparacao::Identidades),
    ("activity_entries", Comparacao::Identidades),
    ("comments", Comparacao::Identidades),
    // ── Conhecimento e dados ────────────────────────────────────────────
    ("sources", Comparacao::Identidades),
    ("notes", Comparacao::Identidades),
    ("note_revisions", Comparacao::Identidades),
    ("documents", Comparacao::Identidades),
    ("datasets", Comparacao::Identidades),
    ("dataset_versions", Comparacao::Identidades),
    ("dataset_files", Comparacao::Identidades),
    // ── Ciência e proveniência ──────────────────────────────────────────
    ("hypotheses", Comparacao::Identidades),
    ("methodologies", Comparacao::Identidades),
    ("methodology_versions", Comparacao::Identidades),
    ("studies", Comparacao::Identidades),
    ("study_executions", Comparacao::Identidades),
    ("results", Comparacao::Identidades),
    ("result_validations", Comparacao::Identidades),
    // ── Comunicação institucional ───────────────────────────────────────
    ("conversations", Comparacao::Identidades),
    ("conversation_participants", Comparacao::Identidades),
    ("messages", Comparacao::Identidades),
    ("calendar_events", Comparacao::Identidades),
    ("reminders", Comparacao::Identidades),
    ("notifications", Comparacao::Identidades),
    ("mailboxes", Comparacao::Identidades),
    ("shared_mailbox_memberships", Comparacao::Identidades),
    ("mail_drafts", Comparacao::Identidades),
    ("mail_draft_attachments", Comparacao::Identidades),
    ("mail_outbox", Comparacao::Identidades),
    ("mail_provider_settings", Comparacao::Identidades),
    // ── Plataforma ──────────────────────────────────────────────────────
    ("storage_backends", Comparacao::Identidades),
    ("storage_objects", Comparacao::Identidades),
    ("compute_nodes", Comparacao::Identidades),
    ("node_credentials", Comparacao::Identidades),
    ("ai_agents", Comparacao::Identidades),
    ("ai_models", Comparacao::Identidades),
    ("ai_jobs", Comparacao::Identidades),
    ("action_plans", Comparacao::Identidades),
    // ── Comparadas por outro mecanismo ──────────────────────────────────
    (
        "research_links",
        Comparacao::Noutro(
            "aresta a aresta, com a origem, no campo `proveniencia`: uma \
             relação é as suas duas pontas e quem a afirmou, e não um `id`",
        ),
    ),
    (
        "audit_events",
        Comparacao::Noutro(
            "pelo número e pelas datas extremas, no campo `auditoria`: são \
             demasiados para enumerar, e uma auditoria recriada denuncia-se \
             pela data do primeiro evento",
        ),
    ),
    // ── Deliberadamente fora ────────────────────────────────────────────
    (
        "search_documents",
        Comparacao::Fora(
            "projecção de pesquisa, reconstruível a partir do que indexa. \
             Exigir que coincidisse faria uma reindexação legítima passar por \
             perda de memória",
        ),
    ),
    (
        "sessions",
        Comparacao::Fora(
            "quem estava autenticado volta a entrar. Identidade persiste; \
             autoridade restabelece-se (ADR-0411). Uma sessão que expire \
             entre as duas leituras faria um restore correcto falhar",
        ),
    ),
    (
        "authentication_attempts",
        Comparacao::Fora(
            "registo de throttling com prazo. Não é memória institucional, e \
             a janela move-se sozinha entre as duas leituras",
        ),
    ),
    (
        "outbox_events",
        Comparacao::Fora(
            "fila de entrega, não estado. Drena-se antes do corte — está no \
             runbook — e comparar uma fila em movimento mediria o worker",
        ),
    ),
    (
        "mail_messages",
        Comparacao::Fora(
            "índice sobre correio que vive no fornecedor, não arquivo \
             (ADR-0407). Volta a preencher-se da caixa de origem",
        ),
    ),
    (
        "reminder_deliveries",
        Comparacao::Fora(
            "registo de entregas já feitas, sem identidade própria. Existe \
             para não repetir um aviso, e não para o recordar",
        ),
    ),
    (
        "mailbox_credentials",
        Comparacao::Fora(
            "não tem `id`: é a credencial selada da caixa a que pertence, e \
             viaja com ela. O que decide se chega legível é a chave de \
             selagem, que não está na base",
        ),
    ),
    (
        "mail_preferences",
        Comparacao::Fora("linha-filha sem identidade própria; viaja com a caixa"),
    ),
    (
        "action_approvals",
        Comparacao::Fora("linha-filha sem identidade própria; viaja com o plano"),
    ),
    (
        "calendar_event_participants",
        Comparacao::Fora("linha-filha sem identidade própria; viaja com o evento"),
    ),
    (
        "message_mentions",
        Comparacao::Fora("linha-filha sem identidade própria; viaja com a mensagem"),
    ),
    (
        "message_reactions",
        Comparacao::Fora("linha-filha sem identidade própria; viaja com a mensagem"),
    ),
];

/// As famílias comparadas identidade a identidade.
///
/// Derivada de [`ESQUEMA`], e nunca escrita à mão em paralelo: duas listas do
/// mesmo facto divergem, e a que ninguém lê é a que fica errada.
fn familias_por_identidade() -> Vec<&'static str> {
    ESQUEMA
        .iter()
        .filter(|(_, como)| matches!(como, Comparacao::Identidades))
        .map(|(tabela, _)| *tabela)
        .collect()
}

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

    let por_identidade = familias_por_identidade();
    let mut familias = Vec::with_capacity(por_identidade.len());
    for tabela in por_identidade {
        // `ORDER BY id` e não a ordem de varrimento: duas leituras da mesma base
        // têm de dar o mesmo manifesto, ou a comparação mede o planeador de
        // consultas em vez do conteúdo.
        //
        // O nome da tabela é interpolado, e pode sê-lo: vem de `ESQUEMA`, que é
        // `&'static str` escrito em código. Nenhum valor de entrada chega aqui,
        // e nenhum deve passar a chegar — um nome de tabela vindo de fora
        // deixaria de ser uma consulta parametrizável para passar a ser
        // estrutura escolhida por quem a enviou.
        let identidades: Vec<Uuid> =
            sqlx::query_scalar(&format!("SELECT id FROM {tabela} ORDER BY id"))
                .fetch_all(pool)
                .await?;
        familias.push(Familia {
            tabela: tabela.to_owned(),
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

/// Como se escreve um instante que pode não existir.
///
/// Uma base sem auditoria nenhuma não tem primeiro evento, e «`None`» é a
/// palavra de uma linguagem de programação, não a de quem está a migrar um
/// servidor às três da manhã.
fn quando(instante: Option<&str>) -> String {
    instante.map_or_else(|| "nenhum".to_owned(), ToOwned::to_owned)
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
    // E o sentido contrário. Um objecto que não estava no snapshot e está aqui
    // foi criado algures entre a saída e a chegada — por um serviço que ficou a
    // correr contra a base antiga, ou por um restore que escreveu por cima de
    // uma instalação que já tinha vida. Nas famílias isto já era divergência;
    // aqui era silêncio.
    let esperados: std::collections::BTreeSet<Uuid> = antes.objectos.iter().map(|o| o.id).collect();
    let intrusos: Vec<Uuid> = depois
        .objectos
        .iter()
        .map(|o| o.id)
        .filter(|id| !esperados.contains(id))
        .collect();
    if !intrusos.is_empty() {
        divergencias.push(Divergencia {
            onde: "storage_objects".to_owned(),
            o_que: format!(
                "{} objecto(s) apareceram do nada, por exemplo {:?}. O destino \
                 não estava vazio, ou alguém continuou a escrever na origem \
                 depois do snapshot",
                intrusos.len(),
                &intrusos[..intrusos.len().min(5)]
            ),
        });
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
    let inventadas = depois_arestas.difference(&antes_arestas).count();
    if inventadas > 0 {
        divergencias.push(Divergencia {
            onde: "research_links".to_owned(),
            o_que: format!(
                "{inventadas} aresta(s) de proveniência apareceram do nada. Uma \
                 relação que o snapshot não continha afirma uma origem que \
                 ninguém observou nem declarou deste lado"
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
                "o evento mais antigo mudou de data: {} → {}. Um restore que \
                 recriasse a auditoria teria datas de hoje, e a evidência \
                 desapareceria exactamente quando é mais precisa",
                quando(antes.auditoria.primeiro.as_deref()),
                quando(depois.auditoria.primeiro.as_deref())
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

    /// Toda a tabela do esquema tem uma decisão de continuidade, e só uma.
    ///
    /// # O defeito que isto guarda
    ///
    /// Alguém acrescenta uma tabela numa migration. O manifesto continua a ser
    /// produzido, a comparação continua a passar, e o que a tabela nova guarda
    /// deixa de ser verificado — sem erro nenhum, porque a lista que a devia
    /// conter é escrita à mão e não sabe que ficou incompleta.
    ///
    /// Foi assim que `person_roles`, `unit_memberships`, `workspace_memberships`
    /// e `credentials` ficaram trinta e oito tabelas fora de uma verificação que
    /// se dizia de estado institucional.
    #[test]
    fn toda_a_tabela_do_esquema_tem_uma_decisao() {
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

        let mut esquema: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
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
                        esquema.insert(nome);
                    }
                }
            }
        }
        assert!(
            esquema.len() > 40,
            "só {} tabelas encontradas; a leitura das migrations partiu-se",
            esquema.len()
        );

        let decididas: std::collections::BTreeSet<String> =
            ESQUEMA.iter().map(|(t, _)| (*t).to_owned()).collect();

        let sem_decisao: Vec<&String> = esquema.difference(&decididas).collect();
        assert!(
            sem_decisao.is_empty(),
            "estas tabelas existem no esquema e ninguém decidiu o que lhes \
             acontece numa migração de servidor: {sem_decisao:?}. \
             Acrescentar a `ESQUEMA` com `Identidades`, `Noutro` ou `Fora` — \
             qualquer uma serve, desde que seja uma decisão"
        );

        let fantasmas: Vec<&String> = decididas.difference(&esquema).collect();
        assert!(
            fantasmas.is_empty(),
            "estas tabelas estão decididas e já não existem no esquema: \
             {fantasmas:?}. Uma entrada que não corresponde a nada dá a \
             impressão de cobertura que não existe"
        );

        assert_eq!(
            decididas.len(),
            ESQUEMA.len(),
            "há uma tabela decidida duas vezes em `ESQUEMA`, e a segunda \
             decisão nunca se lê"
        );
    }

    /// Uma decisão de exclusão diz sempre porquê.
    ///
    /// Sem isto, a exaustividade acima seria satisfeita por uma tabela
    /// acrescentada com `Fora("")` para calar o portão — uma cópia da decisão
    /// em vez da decisão.
    #[test]
    fn nenhuma_excepcao_e_muda() {
        for (tabela, como) in ESQUEMA {
            let razao = match como {
                Comparacao::Identidades => continue,
                Comparacao::Noutro(r) | Comparacao::Fora(r) => r,
            };
            assert!(
                razao.split_whitespace().count() >= 6,
                "`{tabela}` sai da comparação sem razão que se leia: «{razao}»"
            );
        }
    }

    /// A autoridade viaja comparada, e não por confiança.
    ///
    /// As filiações e os verificadores de senha são o que separa uma
    /// instituição a funcionar de um arquivo que ninguém abre. Este teste
    /// nomeia-os um a um para que reclassificá-los seja um acto deliberado.
    #[test]
    fn a_autoridade_e_comparada_identidade_a_identidade() {
        for tabela in [
            "people",
            "credentials",
            "person_roles",
            "unit_memberships",
            "workspace_memberships",
            "explicit_access_grants",
        ] {
            let (_, como) = ESQUEMA
                .iter()
                .find(|(t, _)| *t == tabela)
                .unwrap_or_else(|| panic!("`{tabela}` desapareceu de `ESQUEMA`"));
            assert_eq!(
                *como,
                Comparacao::Identidades,
                "`{tabela}` deixou de ser comparada: um restore que a perdesse \
                 chegaria com a investigação toda e sem ninguém que lhe pudesse \
                 chegar"
            );
        }
    }

    /// Um objecto que apareceu do nada diverge.
    ///
    /// O destino não estava vazio, ou a origem continuou a receber escritas
    /// depois do snapshot. Nos dois casos, o que está no servidor novo não é o
    /// que se levou, e a diferença é a favor de ninguém.
    #[test]
    fn um_objecto_aparecido_do_nada_e_uma_divergencia() {
        let mut depois = vazio();
        depois.objectos = vec![Objecto {
            id: Uuid::new_v4(),
            chave: "research/z/intruso.bin".to_owned(),
            sha256: "c".repeat(64),
            bytes: 4,
        }];
        let divergencias = comparar(&vazio(), &depois);
        assert!(
            divergencias
                .iter()
                .any(|d| d.onde == "storage_objects" && d.o_que.contains("apareceram do nada")),
            "um objecto criado do lado de lá passou por igual: {divergencias:?}"
        );
    }

    /// Uma aresta de proveniência que ninguém afirmou diverge.
    #[test]
    fn uma_aresta_inventada_e_uma_divergencia() {
        let mut depois = vazio();
        depois.proveniencia = vec![Aresta {
            de: Uuid::new_v4(),
            relacao: "validates".to_owned(),
            para: Uuid::new_v4(),
            origem: "declared".to_owned(),
        }];
        let divergencias = comparar(&vazio(), &depois);
        assert!(
            divergencias
                .iter()
                .any(|d| d.onde == "research_links" && d.o_que.contains("apareceram do nada")),
            "uma relação inventada no destino passou por proveniência: {divergencias:?}"
        );
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
