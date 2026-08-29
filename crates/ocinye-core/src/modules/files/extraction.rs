//! Extracção de conteúdo: tornar o corpo de um ficheiro pesquisável.
//!
//! # A propriedade
//!
//! > **Um `FileVersion` guardado pode produzir uma representação textual
//! > derivada, reconstruível e ligada à versão exacta; essa representação torna
//! > o corpo pesquisável sem transformar o índice em autoridade e sem alterar a
//! > validade do ficheiro se o processamento falhar.**
//!
//! # O que isto não faz
//!
//! Não afirma conhecimento. Extrair «a temperatura foi 82 °C» de um PDF produz
//! texto pesquisável — não um `Result`, não uma observação, não uma afirmação
//! científica. A distinção não é filosófica: é a diferença entre uma pessoa ter
//! declarado um resultado e um parser ter lido uma frase.
//!
//! Não autoriza. Um chunk não decide quem o vê. A pesquisa usa-o para
//! **descobrir** candidatos, e a visibilidade decide-se a seguir, contra o
//! `File` e o ambiente, no estado corrente.
//!
//! Não substitui o ficheiro. É uma leitura, feita por um extractor concreto,
//! numa versão concreta — e é por isso que essa identidade fica guardada.

use serde_json::json;
use uuid::Uuid;

use crate::error::{CoreError, CoreResult};
use crate::storage::ObjectStore;
use crate::Tx;
use ocinye_observability::CorrelationIds;

/// O nome do evento que põe uma versão na fila.
///
/// A identidade é a **versão**: uma versão nova não reinterpreta a anterior, e
/// um evento por ficheiro tornaria impossível dizer qual das duas foi lida.
pub const EVENT_EXTRACT: &str = "file_version.extract_requested";

/// O maior objecto que se lê para extrair.
///
/// Um limite explícito, e não a memória da máquina a decidir por acidente.
pub const MAX_SOURCE_BYTES: i64 = 128 * 1024 * 1024;

/// O maior chunk. Acima disto parte-se, para que um resultado de pesquisa
/// caiba numa citação em vez de ser um capítulo.
pub const MAX_CHUNK_CHARS: usize = 2_000;

/// O estado da extracção, que **não** é o estado do armazenamento.
///
/// `storage_objects.status` diz se os bytes estão guardados. Isto diz se o
/// corpo foi lido. Um ficheiro guardado cuja extracção falhou continua válido,
/// legível e descarregável — e a interface tem de o poder dizer assim.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Estado {
    /// Na fila.
    Queued,
    /// Um worker pegou nela.
    Processing,
    /// Há texto, e é pesquisável.
    Available,
    /// O formato não tem extractor. É estado normal, não falha.
    Unsupported,
    /// Havia extractor, e não conseguiu.
    Failed,
}

impl Estado {
    /// Como a base o guarda.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "QUEUED",
            Self::Processing => "PROCESSING",
            Self::Available => "AVAILABLE",
            Self::Unsupported => "UNSUPPORTED",
            Self::Failed => "FAILED",
        }
    }

    /// Lê o que a base guardou.
    ///
    /// Um valor que não se reconheça é `Failed` e não `Available`: um estado
    /// ilegível não pode passar por sucesso.
    #[must_use]
    pub fn parse(raw: &str) -> Self {
        match raw {
            "QUEUED" => Self::Queued,
            "PROCESSING" => Self::Processing,
            "AVAILABLE" => Self::Available,
            "UNSUPPORTED" => Self::Unsupported,
            _ => Self::Failed,
        }
    }
}

/// Quem leu os bytes.
///
/// O nome e a versão viajam para a base para que daqui a dois anos a pergunta
/// «porque é que este chunk existe desta forma?» tenha resposta em vez de
/// arqueologia.
#[derive(Debug, Clone, Copy)]
pub struct Extractor {
    /// O nome.
    pub name: &'static str,
    /// A versão.
    pub version: &'static str,
}

/// O extractor de PDF.
pub const PDF: Extractor = Extractor {
    name: "pdf-extract",
    version: "0.12",
};

/// O extractor de texto simples: descodificar UTF-8 e mais nada.
pub const PLAIN: Extractor = Extractor {
    name: "utf8-text",
    version: "1",
};

/// Um pedaço do corpo, com onde ele está.
#[derive(Debug, Clone)]
pub struct Chunk {
    /// A ordem dentro da extracção. Começa em 0.
    pub ordinal: i32,
    /// O texto.
    pub text: String,
    /// Onde está, na linguagem do formato. `{"page": 4}` para PDF.
    pub locator: serde_json::Value,
}

/// O que uma tentativa de extracção produziu.
pub enum Leitura {
    /// Texto, em pedaços localizados.
    Texto {
        /// Quem leu.
        extractor: Extractor,
        /// Os pedaços.
        chunks: Vec<Chunk>,
    },
    /// Não há extractor para este formato. Estado normal.
    SemExtractor,
    /// Havia extractor, e não conseguiu ler.
    Falhou(String),
}

/// Se este tipo tem extractor.
#[must_use]
pub fn tem_extractor(content_type: &str) -> bool {
    content_type == "application/pdf" || e_texto(content_type)
}

fn e_texto(content_type: &str) -> bool {
    content_type.starts_with("text/")
        || matches!(
            content_type,
            "application/json" | "application/xml" | "application/x-yaml" | "application/yaml"
        )
}

/// Lê os bytes e produz pedaços de texto localizados.
///
/// É pura: não toca na base, não toca no armazenamento, e por isso pode ser
/// exercida directamente com bytes de prova.
///
/// # Pânico
///
/// Não entra em pânico: um parser a rebentar com um PDF hostil é apanhado e
/// devolvido como [`Leitura::Falhou`]. Um ficheiro mal-formado é um estado
/// normal do mundo, e não pode derrubar o worker que o está a ler.
#[must_use]
pub fn extrair(content_type: &str, bytes: &[u8]) -> Leitura {
    if content_type == "application/pdf" {
        // `catch_unwind` porque isto lê um formato binário que veio de fora. O
        // parser é Rust e não corrompe memória, mas entra em pânico com
        // documentos estranhos — e um pânico aqui levaria o worker consigo.
        let lido = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            pdf_extract::extract_text_from_mem_by_pages(bytes)
        }));

        return match lido {
            Ok(Ok(paginas)) => Leitura::Texto {
                extractor: PDF,
                chunks: em_pedacos_por_pagina(&paginas),
            },
            Ok(Err(erro)) => Leitura::Falhou(format!("o PDF não pôde ser lido: {erro}")),
            Err(_) => Leitura::Falhou(
                "o PDF fez o leitor entrar em pânico; o ficheiro continua guardado".to_owned(),
            ),
        };
    }

    if e_texto(content_type) {
        return match std::str::from_utf8(bytes) {
            Ok(texto) => Leitura::Texto {
                extractor: PLAIN,
                chunks: em_pedacos(texto, &json!({})),
            },
            Err(_) => Leitura::Falhou("o conteúdo não é UTF-8 legível.".to_owned()),
        };
    }

    Leitura::SemExtractor
}

/// Uma página por localizador, e páginas grandes partidas em vários pedaços.
fn em_pedacos_por_pagina(paginas: &[String]) -> Vec<Chunk> {
    let mut todos = Vec::new();
    for (indice, pagina) in paginas.iter().enumerate() {
        let numero = indice + 1;
        for mut pedaco in em_pedacos(pagina, &json!({ "page": numero })) {
            pedaco.ordinal = i32::try_from(todos.len()).unwrap_or(i32::MAX);
            todos.push(pedaco);
        }
    }
    todos
}

/// Parte um texto em pedaços, sem cortar palavras a meio quando pode evitá-lo.
fn em_pedacos(texto: &str, locator: &serde_json::Value) -> Vec<Chunk> {
    let limpo = normalizar(texto);
    if limpo.is_empty() {
        return Vec::new();
    }

    let mut pedacos = Vec::new();
    let mut actual = String::new();

    for palavra in limpo.split(' ') {
        if !actual.is_empty()
            && actual.chars().count() + palavra.chars().count() + 1 > MAX_CHUNK_CHARS
        {
            pedacos.push(Chunk {
                ordinal: i32::try_from(pedacos.len()).unwrap_or(i32::MAX),
                text: std::mem::take(&mut actual),
                locator: locator.clone(),
            });
        }
        if !actual.is_empty() {
            actual.push(' ');
        }
        actual.push_str(palavra);
    }

    if !actual.is_empty() {
        pedacos.push(Chunk {
            ordinal: i32::try_from(pedacos.len()).unwrap_or(i32::MAX),
            text: actual,
            locator: locator.clone(),
        });
    }

    pedacos
}

/// Espaço colapsado, para que a mesma frase se pesquise igual venha de onde vier.
///
/// Um PDF traz quebras de linha onde a coluna acabou, e não onde a frase
/// acabou. Sem isto, «coeficiente termo-\neléctrico» seria duas palavras que
/// ninguém procura.
fn normalizar(texto: &str) -> String {
    texto.split_whitespace().collect::<Vec<_>>().join(" ")
}

// ── Persistência ────────────────────────────────────────────────────────

/// Põe uma versão na fila, dentro da transacção que a criou.
///
/// A linha e o evento nascem **com** a versão: se o carregamento reverter, não
/// fica uma extracção a apontar para uma versão que nunca existiu. É o mesmo
/// raciocínio do outbox, e é por isso que se usa o outbox.
///
/// Idempotente: pedir duas vezes deixa a linha como está. Um evento
/// reentregue é o caso normal, não a excepção.
///
/// # Errors
///
/// Devolve erro quando a inserção falha, o que aborta também a criação da
/// versão.
pub async fn queue(tx: &mut Tx<'_>, file_version_id: Uuid, ids: &CorrelationIds) -> CoreResult<()> {
    sqlx::query(
        "INSERT INTO file_extractions (file_version_id, status)
         VALUES ($1, 'QUEUED')
         ON CONFLICT (file_version_id) DO NOTHING",
    )
    .bind(file_version_id)
    .execute(&mut **tx)
    .await?;

    crate::outbox::emit(
        tx,
        EVENT_EXTRACT,
        "file_version",
        file_version_id,
        &ids.correlation_id,
        json!({ "file_version_id": file_version_id }),
    )
    .await?;

    Ok(())
}

/// O que o worker precisa de saber para ler uma versão.
pub struct Trabalho {
    /// A versão.
    pub file_version_id: Uuid,
    /// Onde estão os bytes.
    pub object_key: String,
    /// O tipo declarado no carregamento.
    pub content_type: String,
    /// Quantos bytes.
    pub size_bytes: i64,
    /// A soma, que fica guardada com a extracção.
    pub checksum_sha256: String,
}

/// Marca a versão como em processamento e devolve o que é preciso para a ler.
///
/// Devolve `None` quando já não há nada a fazer — porque a extracção já está
/// disponível, ou porque a versão desapareceu. Um evento reentregue passa por
/// aqui e sai sem trabalho, que é o comportamento certo.
///
/// # Errors
///
/// Devolve erro quando a consulta falha.
pub async fn claim(tx: &mut Tx<'_>, file_version_id: Uuid) -> CoreResult<Option<Trabalho>> {
    // `FOR UPDATE` na linha da extracção: dois workers com o mesmo evento
    // esperam um pelo outro, e o segundo vê o estado que o primeiro deixou.
    let estado: Option<String> = sqlx::query_scalar(
        "SELECT status FROM file_extractions WHERE file_version_id = $1 FOR UPDATE",
    )
    .bind(file_version_id)
    .fetch_optional(&mut **tx)
    .await?;

    let Some(estado) = estado else {
        return Ok(None);
    };

    // Já lido. Reprocessar é uma operação deliberada, e não o que acontece
    // porque um evento chegou outra vez.
    if matches!(
        Estado::parse(&estado),
        Estado::Available | Estado::Unsupported
    ) {
        return Ok(None);
    }

    let linha: Option<(String, String, i64, String)> = sqlx::query_as(
        "SELECT o.object_key, o.content_type, o.size_bytes, o.checksum_sha256
           FROM file_versions v
           JOIN storage_objects o ON o.id = v.storage_object_id
          WHERE v.id = $1",
    )
    .bind(file_version_id)
    .fetch_optional(&mut **tx)
    .await?;

    let Some((object_key, content_type, size_bytes, checksum_sha256)) = linha else {
        return Ok(None);
    };

    sqlx::query(
        "UPDATE file_extractions
            SET status = 'PROCESSING', updated_at = now()
          WHERE file_version_id = $1",
    )
    .bind(file_version_id)
    .execute(&mut **tx)
    .await?;

    Ok(Some(Trabalho {
        file_version_id,
        object_key,
        content_type,
        size_bytes,
        checksum_sha256,
    }))
}

/// Guarda o resultado de uma leitura.
///
/// Os chunks anteriores são apagados antes de os novos entrarem: correr duas
/// vezes tem de dar o mesmo estado, e não o dobro dos pedaços.
///
/// # Errors
///
/// Devolve erro quando a escrita falha.
pub async fn record(tx: &mut Tx<'_>, trabalho: &Trabalho, leitura: Leitura) -> CoreResult<Estado> {
    let extraction_id: Uuid =
        sqlx::query_scalar("SELECT id FROM file_extractions WHERE file_version_id = $1")
            .bind(trabalho.file_version_id)
            .fetch_one(&mut **tx)
            .await?;

    sqlx::query("DELETE FROM file_chunks WHERE extraction_id = $1")
        .bind(extraction_id)
        .execute(&mut **tx)
        .await?;

    match leitura {
        Leitura::Texto { extractor, chunks } => {
            for chunk in &chunks {
                sqlx::query(
                    "INSERT INTO file_chunks (extraction_id, ordinal, text, locator)
                     VALUES ($1, $2, $3, $4)",
                )
                .bind(extraction_id)
                .bind(chunk.ordinal)
                .bind(&chunk.text)
                .bind(&chunk.locator)
                .execute(&mut **tx)
                .await?;
            }

            // Um PDF de imagens digitalizadas lê-se sem erro e não tem texto
            // nenhum. Dizer `AVAILABLE` com zero chunks seria afirmar que o
            // corpo está pesquisável quando não está.
            let estado = if chunks.is_empty() {
                Estado::Unsupported
            } else {
                Estado::Available
            };

            sqlx::query(
                "UPDATE file_extractions
                    SET status = $2,
                        extractor_name = $3,
                        extractor_version = $4,
                        source_checksum_sha256 = $5,
                        chunk_count = $6,
                        failure_reason = NULL,
                        extracted_at = now(),
                        updated_at = now()
                  WHERE id = $1",
            )
            .bind(extraction_id)
            .bind(estado.as_str())
            .bind(extractor.name)
            .bind(extractor.version)
            .bind(&trabalho.checksum_sha256)
            .bind(i32::try_from(chunks.len()).unwrap_or(i32::MAX))
            .execute(&mut **tx)
            .await?;

            Ok(estado)
        }

        Leitura::SemExtractor => {
            marcar(tx, extraction_id, Estado::Unsupported, None).await?;
            Ok(Estado::Unsupported)
        }

        Leitura::Falhou(razao) => {
            marcar(tx, extraction_id, Estado::Failed, Some(&razao)).await?;
            Ok(Estado::Failed)
        }
    }
}

async fn marcar(
    tx: &mut Tx<'_>,
    extraction_id: Uuid,
    estado: Estado,
    razao: Option<&str>,
) -> CoreResult<()> {
    sqlx::query(
        "UPDATE file_extractions
            SET status = $2, failure_reason = $3, chunk_count = 0, updated_at = now()
          WHERE id = $1",
    )
    .bind(extraction_id)
    .bind(estado.as_str())
    .bind(razao)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// Lê uma versão de ponta a ponta: reclama, busca os bytes, extrai, guarda.
///
/// É esta a função que o worker chama. Vive aqui e não no worker porque a
/// decisão de o que fazer com um formato sem extractor, ou com um parser que
/// falha, é do domínio — e um worker é um sítio onde se executa, não onde se
/// decide.
///
/// # Errors
///
/// Devolve erro quando a base ou o armazenamento não respondem. Um formato sem
/// extractor e um parser que falha **não** são erros: são estados, e ficam
/// registados como tal.
pub async fn process(
    tx: &mut Tx<'_>,
    store: &ObjectStore,
    file_version_id: Uuid,
) -> CoreResult<Option<Estado>> {
    let Some(trabalho) = claim(tx, file_version_id).await? else {
        return Ok(None);
    };

    if !tem_extractor(&trabalho.content_type) {
        let estado = record(tx, &trabalho, Leitura::SemExtractor).await?;
        return Ok(Some(estado));
    }

    if trabalho.size_bytes > MAX_SOURCE_BYTES {
        let estado = record(
            tx,
            &trabalho,
            Leitura::Falhou(format!(
                "o ficheiro tem {} bytes e o limite de extracção é {MAX_SOURCE_BYTES}",
                trabalho.size_bytes
            )),
        )
        .await?;
        return Ok(Some(estado));
    }

    // Se o armazenamento não responde, isto **é** um erro: o outbox volta a
    // tentar. Não se marca `FAILED`, que afirmaria que o conteúdo não se
    // consegue ler quando o que aconteceu foi o disco não atender.
    let bytes = store.get(&trabalho.object_key).await?;

    let leitura = extrair(&trabalho.content_type, &bytes);
    let estado = record(tx, &trabalho, leitura).await?;
    Ok(Some(estado))
}

/// O estado da extracção de uma versão, para quem o quiser mostrar.
///
/// # Errors
///
/// Devolve erro quando a consulta falha.
pub async fn status<'e>(
    executor: impl sqlx::PgExecutor<'e>,
    file_version_id: Uuid,
) -> CoreResult<Option<(Estado, i64)>> {
    let linha: Option<(String, i32)> = sqlx::query_as(
        "SELECT status, chunk_count FROM file_extractions WHERE file_version_id = $1",
    )
    .bind(file_version_id)
    .fetch_optional(executor)
    .await?;
    Ok(linha.map(|(estado, chunks)| (Estado::parse(&estado), i64::from(chunks))))
}

/// Um erro que não é do domínio: o armazenamento não respondeu.
///
/// Existe para o worker distinguir «não consegui ler» de «não havia nada para
/// ler», que são coisas diferentes e merecem tratamento diferente.
#[must_use]
pub const fn e_indisponibilidade(erro: &CoreError) -> bool {
    matches!(erro, CoreError::StorageUnavailable(_))
}

/// O estado da extracção da versão **corrente** de um ficheiro.
///
/// Quem chama tem de ter autorizado o ficheiro primeiro: isto não decide nada.
///
/// # Errors
///
/// Devolve erro quando a consulta falha.
pub async fn status_of_current<'e>(
    executor: impl sqlx::PgExecutor<'e>,
    file_id: Uuid,
) -> CoreResult<Option<(Estado, i64)>> {
    let linha: Option<(String, i32)> = sqlx::query_as(
        "SELECT e.status, e.chunk_count
           FROM file_versions v
           JOIN file_extractions e ON e.file_version_id = v.id
          WHERE v.file_id = $1
          ORDER BY v.sequence DESC
          LIMIT 1",
    )
    .bind(file_id)
    .fetch_optional(executor)
    .await?;
    Ok(linha.map(|(estado, chunks)| (Estado::parse(&estado), i64::from(chunks))))
}

/// O texto extraído da versão corrente, em ordem.
///
/// Quem chama tem de ter autorizado o ficheiro primeiro: isto não decide nada,
/// e por isso não recebe um principal. A porta autorizada é
/// [`crate::modules::files::content`].
///
/// # Errors
///
/// Devolve erro quando a consulta falha.
pub async fn text_of_current<'e>(
    executor: impl sqlx::PgExecutor<'e>,
    file_id: Uuid,
    max_chars: usize,
) -> CoreResult<Option<String>> {
    let pedacos: Vec<String> = sqlx::query_scalar(
        "SELECT c.text
           FROM file_chunks c
           JOIN file_extractions e ON e.id = c.extraction_id
           JOIN file_versions v ON v.id = e.file_version_id
          WHERE v.file_id = $1
            AND v.sequence = (
                SELECT max(sequence) FROM file_versions WHERE file_id = $1
            )
          ORDER BY c.ordinal",
    )
    .bind(file_id)
    .fetch_all(executor)
    .await?;

    if pedacos.is_empty() {
        return Ok(None);
    }

    let mut texto = String::new();
    for pedaco in pedacos {
        if texto.chars().count() >= max_chars {
            break;
        }
        if !texto.is_empty() {
            texto.push_str("\n\n");
        }
        texto.push_str(&pedaco);
    }
    Ok(Some(texto.chars().take(max_chars).collect()))
}
