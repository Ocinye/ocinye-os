//! A linhagem científica: uma projecção navegável sobre a proveniência.
//!
//! # O que isto **não** é
//!
//! Não é uma segunda base de dados. Não há tabela de linhagem, não há grafo
//! paralelo, não há cache. Cada travessia lê `research_links` e os recursos que
//! ela nomeia, agora.
//!
//! Guardar o grafo para acelerar a interface criaria uma segunda fonte de
//! verdade — e duas fontes de verdade acabam por discordar, normalmente no dia
//! em que alguém precisa da resposta certa.
//!
//! > **A linhagem deriva da proveniência registada. Não é uma fonte
//! > independente de verdade científica.**
//!
//! # A regra do nó oculto
//!
//! Se um nó intermédio não é legível por quem percorre, **a travessia termina
//! nessa fronteira**.
//!
//! Não se atravessa por trás dele para mostrar o que vem depois, e não se
//! devolve nada sobre ele: nem identificador, nem tipo, nem título, nem
//! ambiente, nem uma contagem que confirme que existe. A forma do grafo é ela
//! própria informação — «este resultado depende de mais três coisas que não
//! podes ver» já diz que há três coisas.
//!
//! É a escolha conservadora, e é deliberada: é a que se audita, e a que não
//! transforma a topologia num canal lateral.
//!
//! # Os limites
//!
//! Profundidade limitada e conjunto de visitados. A proveniência científica
//! pode formar ciclos legítimos — um resultado que sustenta uma hipótese que
//! gera um estudo que produz outro resultado — e uma travessia sem memória
//! andaria à volta deles para sempre.

use std::collections::BTreeSet;

use ocinye_contracts::agentic::{ResourceKind as AgenticKind, ResourceRef};
use ocinye_contracts::provenance::ProvenanceRelation;
use ocinye_domain::Principal;
use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::CoreResult;

/// Até onde uma travessia vai.
///
/// Cinco saltos chegam para a cadeia que esta camada existe para mostrar —
/// hipótese, metodologia, estudo, dados, execução, resultado — e param bem
/// antes de uma consulta se tornar um problema para a base de dados.
///
/// Não é uma preferência: é um limite. Uma travessia sem tecto responde
/// depressa nos dados de hoje e deixa de responder nos de daqui a dois anos.
pub const PROFUNDIDADE_MAXIMA: u8 = 5;

/// Para que lado se anda.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sentido {
    /// De onde isto veio. Segue as arestas a partir deste recurso.
    Montante,
    /// O que dependeu disto. Segue as arestas que apontam para este recurso.
    Jusante,
}

impl Sentido {
    /// Como se lê.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Montante => "Montante",
            Self::Jusante => "Jusante",
        }
    }
}

/// Um passo da linhagem: uma relação, e o recurso do outro lado.
#[derive(Debug, Clone, Serialize)]
pub struct Passo {
    /// A que distância do ponto de partida.
    pub profundidade: u8,
    /// O recurso de onde a aresta parte, tal como é legível.
    pub de: ResourceRef,
    /// O que a aresta afirma.
    pub relacao: String,
    /// Como se lê a afirmação.
    pub relacao_legivel: String,
    /// O recurso do outro lado.
    pub para: ResourceRef,
    /// De onde veio a afirmação: declarada, ou observada pela operação.
    pub origem: String,
}

/// O que uma travessia encontrou.
#[derive(Debug, Clone, Serialize)]
pub struct Linhagem {
    /// O recurso de onde se partiu.
    pub raiz: ResourceRef,
    /// Para que lado se andou.
    pub sentido: &'static str,
    /// Os passos, em ordem determinista: primeiro por profundidade, depois
    /// pelo que a aresta afirma, depois pelo identificador.
    ///
    /// A ordem é fixa porque uma linhagem que muda de ordem entre duas
    /// leituras é uma linhagem que ninguém consegue comparar — nem uma pessoa
    /// a reler, nem um teste.
    pub passos: Vec<Passo>,
    /// Se a travessia chegou ao limite de profundidade.
    ///
    /// **Só isso.** Nunca indica que existem recursos para lá de uma fronteira
    /// de autorização: um nó que a política recuse é indistinguível de uma
    /// folha, e marcar a linhagem como truncada por causa dele diria «há mais
    /// coisas que não podes ver» — o contrário do que a fronteira existe para
    /// fazer.
    ///
    /// Dito quando é verdade, porque uma linhagem truncada em silêncio afirma
    /// que não há mais nada.
    pub truncada: bool,
}

/// Uma aresta, tal como está guardada.
#[derive(Debug, sqlx::FromRow)]
struct Aresta {
    source_type_name: String,
    source_id: Uuid,
    relation: String,
    target_type_name: String,
    target_id: Uuid,
    origin: String,
}

/// Percorre a proveniência a partir de um recurso.
///
/// # O que garante
///
/// - Cada nó que aparece foi resolvido pelo serviço que o detém, com a política
///   de quem percorre. Um nó que essa política recuse não aparece, e a travessia
///   não continua por trás dele.
/// - Nenhum nó aparece duas vezes, e nenhum ciclo prende a consulta.
/// - A profundidade tem tecto, e quando o tecto é atingido isso é dito.
///
/// # Errors
///
/// Devolve erro quando o recurso de partida não é alcançável — a mesma resposta
/// que daria se ele não existisse.
pub async fn percorrer(
    pool: &PgPool,
    principal: &Principal,
    raiz: &ResourceRef,
    sentido: Sentido,
    profundidade: u8,
) -> CoreResult<Linhagem> {
    // A raiz é resolvida antes de tudo. Se quem pergunta não a alcança, não há
    // travessia — e a resposta é a mesma que daria a um identificador
    // inventado.
    let resolvida = crate::resources::resolve(pool, principal, raiz).await?;

    let tecto = profundidade.clamp(1, PROFUNDIDADE_MAXIMA);
    let mut passos = Vec::new();
    let mut visitados: BTreeSet<(String, Uuid)> = BTreeSet::new();
    let mut truncada = false;

    visitados.insert((raiz.kind.as_str().to_owned(), raiz.id));

    // A fronteira do nível actual: os recursos de onde ainda há arestas por
    // seguir. Percorre-se em largura para que a ordem seja a distância, que é
    // como uma pessoa lê uma linhagem: primeiro o que está mais perto.
    let mut fronteira: Vec<(AgenticKind, Uuid, ResourceRef)> =
        vec![(raiz.kind, raiz.id, resolvida.reference.clone())];

    for nivel in 1..=tecto {
        if fronteira.is_empty() {
            break;
        }

        let mut seguinte = Vec::new();

        for (kind, id, referencia_de) in &fronteira {
            let arestas = arestas_de(pool, principal.organisation_id, *kind, *id, sentido).await?;

            for aresta in arestas {
                let (outro_tipo, outro_id) = match sentido {
                    Sentido::Montante => (aresta.target_type_name.clone(), aresta.target_id),
                    Sentido::Jusante => (aresta.source_type_name.clone(), aresta.source_id),
                };

                // Um tipo que o domínio já não conhece é uma aresta antiga que
                // ninguém consegue ler. Não se inventa uma leitura para ela.
                let Some(outro_kind) = AgenticKind::parse(&outro_tipo) else {
                    continue;
                };

                if !visitados.insert((outro_tipo.clone(), outro_id)) {
                    continue;
                }

                // ── A fronteira do que se pode ver ──────────────────────
                //
                // O nó é resolvido pelo serviço que o detém. Se a política de
                // quem percorre o recusar, a travessia **termina aqui**: o
                // passo não entra, e as arestas que partem dele não são
                // seguidas.
                //
                // Não se devolve nada sobre ele — nem que existe. «Depende de
                // mais três coisas que não podes ver» já diz que há três.
                let Ok(outro) = crate::resources::resolve(
                    pool,
                    principal,
                    &ResourceRef {
                        kind: outro_kind,
                        id: outro_id,
                        label: None,
                    },
                )
                .await
                else {
                    continue;
                };

                let Some(verbo) = ProvenanceRelation::parse(&aresta.relation) else {
                    continue;
                };

                let (de, para) = match sentido {
                    Sentido::Montante => (referencia_de.clone(), outro.reference.clone()),
                    Sentido::Jusante => (outro.reference.clone(), referencia_de.clone()),
                };

                passos.push(Passo {
                    profundidade: nivel,
                    de,
                    relacao: verbo.as_str().to_owned(),
                    relacao_legivel: verbo.label().to_owned(),
                    para,
                    origem: aresta.origin.clone(),
                });

                seguinte.push((outro_kind, outro_id, outro.reference.clone()));
            }
        }

        // ── `truncada` fala só do grafo que esta pessoa vê ─────────────
        //
        // `seguinte` contém apenas nós que **resolveram**: um nó que a política
        // recusou nunca lá entra. Portanto uma fronteira de autorização não
        // pode marcar a linhagem como truncada — e é a propriedade que importa
        // aqui, porque `truncada = true` diria «há mais coisas para lá do
        // último recurso que vês», que é exactamente o que a fronteira existe
        // para não dizer.
        //
        // Um nó oculto tem de ser indistinguível de uma folha. Se A tem um
        // filho escondido e B não tem filho nenhum, as duas respostas têm de
        // ser iguais: mesma forma, mesma contagem, mesmo `truncada`.
        //
        // Isto sustenta-se porque o `resolve` acontece **antes** de qualquer
        // coisa entrar em `seguinte`. Fica escrito porque uma propriedade que
        // depende da ordem de duas linhas é uma propriedade que a próxima
        // pessoa a mexer aqui não sabe que existe.
        //
        // O que `truncada` significa, e só isso: *entre os recursos que esta
        // pessoa pode observar, a consulta chegou ao limite de profundidade.*
        if nivel == tecto && !seguinte.is_empty() {
            truncada = true;
        }

        fronteira = seguinte;
    }

    // Ordem determinista. Sem isto, duas leituras da mesma linhagem podiam vir
    // por ordens diferentes, e ninguém — nem uma pessoa, nem um teste — as
    // conseguiria comparar.
    passos.sort_by(|a, b| {
        a.profundidade
            .cmp(&b.profundidade)
            .then_with(|| a.relacao.cmp(&b.relacao))
            .then_with(|| a.para.id.cmp(&b.para.id))
    });

    Ok(Linhagem {
        raiz: resolvida.reference,
        sentido: sentido.label(),
        passos,
        truncada,
    })
}

/// As arestas que saem de — ou entram em — um recurso.
async fn arestas_de(
    pool: &PgPool,
    organisation_id: Uuid,
    kind: AgenticKind,
    id: Uuid,
    sentido: Sentido,
) -> CoreResult<Vec<Aresta>> {
    // Duas consultas e não uma com `OR`: o índice de cada ponta é próprio, e um
    // `OR` sobre os dois lados não usa nenhum deles.
    let sql = match sentido {
        Sentido::Montante => {
            "SELECT source_type_name, source_id, relation, target_type_name, target_id, origin
               FROM research_links
              WHERE organisation_id = $1 AND source_type_name = $2 AND source_id = $3
              ORDER BY relation, target_id"
        }
        Sentido::Jusante => {
            "SELECT source_type_name, source_id, relation, target_type_name, target_id, origin
               FROM research_links
              WHERE organisation_id = $1 AND target_type_name = $2 AND target_id = $3
              ORDER BY relation, source_id"
        }
    };

    let arestas = sqlx::query_as::<_, Aresta>(sql)
        .bind(organisation_id)
        .bind(kind.as_str())
        .bind(id)
        .fetch_all(pool)
        .await?;

    Ok(arestas)
}
