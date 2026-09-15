-- Miniaturas de ficheiros — um derivado visual de uma versão.
--
-- > **Um `FileVersion` de imagem pode produzir uma miniatura derivada,
-- > reconstruível e ligada à versão exacta; essa miniatura deixa a grelha
-- > mostrar o conteúdo em vez de um ícone genérico, sem se tornar autoridade e
-- > sem alterar a validade do ficheiro se a geração falhar.**
--
-- O que esta tabela deliberadamente **não** é:
--
-- Não é uma versão. Uma miniatura é um objecto derivado, não um `FileVersion` —
-- não é citável, não entra no histórico, e nunca aparece onde uma versão
-- aparece. Por isso liga-se por uma tabela própria, e não reutilizando
-- `file_versions`.
--
-- Não é autoridade. Quem vê a miniatura é decidido pela posse da versão de
-- origem, no momento em que se serve — nunca por esta linha.
--
-- Não é o ficheiro. É uma leitura visual de uma versão concreta, feita por um
-- gerador concreto, e por isso essa identidade fica guardada.
CREATE TABLE file_thumbnails (
    id                     UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- `UNIQUE`: uma versão tem uma miniatura corrente. Regenerar substitui-a, e
    -- a identidade do gerador guardada abaixo responde «porque é que esta
    -- miniatura existe desta forma».
    file_version_id        UUID NOT NULL UNIQUE
                           REFERENCES file_versions (id) ON DELETE CASCADE,

    -- O objecto derivado, quando existe. `SET NULL` porque apagar o objecto não
    -- deve apagar o estado da geração — fica a dizer que houve miniatura e que
    -- os bytes já não estão, o que é honesto e reconstruível.
    thumbnail_object_id    UUID
                           REFERENCES storage_objects (id) ON DELETE SET NULL,

    -- O estado da **geração**, que não é o estado do armazenamento. Um ficheiro
    -- cuja miniatura falhou continua guardado, legível e descarregável.
    --   QUEUED       — pedida, à espera do worker
    --   READY        — gerada, com `thumbnail_object_id`
    --   UNSUPPORTED  — o tipo não produz miniatura (estado, não erro)
    --   FAILED       — a geração falhou sobre bytes que deviam servir
    status                 VARCHAR(16) NOT NULL DEFAULT 'QUEUED',

    -- Quem gerou, e como. Sem isto, uma miniatura estranha daqui a dois anos é
    -- um mistério em vez de uma pergunta com resposta.
    generator_name         VARCHAR(64),
    generator_version      VARCHAR(32),

    -- As dimensões do derivado, para quem o quiser dispor sem o descarregar.
    width                  INTEGER,
    height                 INTEGER,

    -- A soma dos bytes de origem de que esta miniatura saiu. Prova de que
    -- descreve **estes** bytes e não outros quaisquer.
    source_checksum_sha256 CHAR(64),

    -- Porque falhou, em texto que uma pessoa lê. Não é o erro do descodificador.
    failure_reason         TEXT,

    created_at             TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at             TIMESTAMPTZ NOT NULL DEFAULT now(),

    CONSTRAINT ck_file_thumbnails_status
        CHECK (status IN ('QUEUED', 'READY', 'UNSUPPORTED', 'FAILED'))
);

-- A geração pendente lê-se por estado. Uma parcial sobre `QUEUED` mantém o
-- índice pequeno — as prontas são a maioria e não precisam dele.
CREATE INDEX ix_file_thumbnails_pendentes
    ON file_thumbnails (created_at)
    WHERE status = 'QUEUED';
