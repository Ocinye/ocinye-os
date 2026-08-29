-- Extracção de conteúdo e pesquisa lexical de corpo.
--
-- > **Um `FileVersion` guardado pode produzir uma representação textual
-- > derivada, reconstruível e ligada à versão exacta; essa representação torna o
-- > corpo pesquisável sem transformar o índice em autoridade e sem alterar a
-- > validade do ficheiro se o processamento falhar.**
--
-- Três coisas que estas tabelas deliberadamente **não** são:
--
-- Não são autoridade. Um chunk não decide quem o vê: a visibilidade decide-se
-- contra o `File` e o ambiente, no momento da consulta.
--
-- Não são conhecimento. Extrair «a temperatura foi 82 °C» de um PDF não cria um
-- Result, uma Observation nem uma afirmação científica. É conteúdo pesquisável,
-- e nada mais.
--
-- Não são o ficheiro. São uma leitura do ficheiro feita por um extractor
-- concreto, numa versão concreta — e por isso essa identidade fica guardada.

-- A extracção de uma versão.
--
-- A identidade é a **versão**, nunca o ficheiro: carregar uma versão nova não
-- reinterpreta a anterior, e a extracção da v1 continua a descrever a v1.
CREATE TABLE file_extractions (
    id                     UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- `UNIQUE`: uma versão tem uma extracção corrente. Reprocessar substitui-a,
    -- e a identidade do extractor guardada abaixo é o que permite responder
    -- «porque é que este chunk existe desta forma».
    file_version_id        UUID NOT NULL UNIQUE
                           REFERENCES file_versions (id) ON DELETE CASCADE,

    -- O estado da **extracção**, que não é o estado do armazenamento.
    --
    -- `STORED` vive em `storage_objects`. Um ficheiro cuja extracção falhou
    -- continua guardado, legível e descarregável — e a interface tem de poder
    -- dizer «Ficheiro guardado. Não foi possível tornar o conteúdo pesquisável»
    -- em vez de «o carregamento falhou».
    status                 VARCHAR(16) NOT NULL DEFAULT 'QUEUED',

    -- Quem leu os bytes, e em que versão. Sem isto, um chunk estranho daqui a
    -- dois anos é um mistério em vez de uma pergunta com resposta.
    extractor_name         VARCHAR(64),
    extractor_version      VARCHAR(32),

    -- A soma dos bytes de que esta leitura saiu. Prova de que a extracção
    -- descreve **estes** bytes e não outros quaisquer.
    source_checksum_sha256 CHAR(64),

    -- Só quando realmente detectada. `NULL` é honesto; adivinhar não é.
    language               VARCHAR(16),

    chunk_count            INTEGER NOT NULL DEFAULT 0,

    -- Porque falhou, em texto que uma pessoa lê. Não é o erro do parser.
    failure_reason         TEXT,

    extracted_at           TIMESTAMPTZ,
    created_at             TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at             TIMESTAMPTZ NOT NULL DEFAULT now(),

    CONSTRAINT ck_file_extractions_status CHECK (
        status IN ('QUEUED', 'PROCESSING', 'AVAILABLE', 'UNSUPPORTED', 'FAILED')
    ),
    -- Uma extracção disponível tem de dizer quem a fez. Sem isto, o campo
    -- ficaria opcional na prática e a proveniência erodia sem ninguém decidir.
    CONSTRAINT ck_file_extractions_provenance CHECK (
        status <> 'AVAILABLE'
        OR (extractor_name IS NOT NULL
            AND extractor_version IS NOT NULL
            AND source_checksum_sha256 IS NOT NULL
            AND extracted_at IS NOT NULL)
    ),
    CONSTRAINT ck_file_extractions_chunk_count CHECK (chunk_count >= 0)
);

CREATE INDEX ix_file_extractions_status ON file_extractions (status)
    WHERE status IN ('QUEUED', 'PROCESSING');

-- Um pedaço do corpo, com onde ele está no documento.
CREATE TABLE file_chunks (
    id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    extraction_id UUID NOT NULL REFERENCES file_extractions (id) ON DELETE CASCADE,

    -- A ordem dentro da extracção. Começa em 0.
    ordinal       INTEGER NOT NULL,

    text          TEXT NOT NULL,

    -- Onde isto está, na linguagem do formato: `{"page": 4}` para PDF.
    -- Chega para uma citação institucional útil, e cresce sem migração quando
    -- um formato novo precisar de outra coordenada.
    locator       JSONB NOT NULL DEFAULT '{}'::jsonb,

    -- 'simple', pela mesma razão de `search_documents`: o corpus é bilingue, e
    -- um stemmer de uma língua degradaria a outra.
    search_vector TSVECTOR GENERATED ALWAYS AS (to_tsvector('simple', text)) STORED,

    created_at    TIMESTAMPTZ NOT NULL DEFAULT now(),

    CONSTRAINT uq_file_chunks_ordinal UNIQUE (extraction_id, ordinal),
    CONSTRAINT ck_file_chunks_ordinal CHECK (ordinal >= 0),
    CONSTRAINT ck_file_chunks_text CHECK (length(text) > 0)
);

CREATE INDEX ix_file_chunks_vector ON file_chunks USING GIN (search_vector);
CREATE INDEX ix_file_chunks_extraction ON file_chunks (extraction_id);
