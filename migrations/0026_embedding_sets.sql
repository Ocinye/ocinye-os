-- Conjuntos de embeddings sobre o conteúdo extraído.
--
-- > **Um conjunto nunca mistura embeddings produzidos por identidades de modelo
-- > ou perfis diferentes.**
--
-- Compatibilidade semântica **não** é «o mesmo tamanho de vector». Dois modelos
-- de 1024 dimensões produzem espaços diferentes, e compará-los dá números que
-- parecem distâncias e não são. Por isso a identidade guardada aqui é
-- `(provider, model, revision, dimensions)` inteira, e é ela que decide o que
-- se pode comparar com o quê.
--
-- A coluna `VECTOR(1024)` de `search_documents` é histórica e **não** determina
-- este domínio: modelos diferentes têm dimensões diferentes, e o esquema tem de
-- suportar isso sem que ninguém tenha de escolher entre falsificar o modelo e
-- falsificar o índice.

CREATE TABLE embedding_sets (
    id                UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- Sobre que versão. A identidade é a **versão**, como em toda a cadeia
    -- derivada: uma versão nova não reinterpreta a anterior.
    file_version_id   UUID NOT NULL REFERENCES file_versions (id) ON DELETE CASCADE,

    -- A extracção de que este conjunto saiu.
    --
    -- Sem isto, um conjunto sobreviveria a um reprocessamento da extracção e
    -- passaria a apontar para pedaços que já não existem — ou, pior, para
    -- pedaços diferentes com os mesmos números.
    extraction_id     UUID NOT NULL REFERENCES file_extractions (id) ON DELETE CASCADE,

    -- Quem produziu, com que modelo, em que revisão, e quantas dimensões.
    provider          VARCHAR(64) NOT NULL,
    model             VARCHAR(128) NOT NULL,
    revision          VARCHAR(64) NOT NULL,
    dimensions        INTEGER NOT NULL,

    -- De onde o provider é. Guardado porque a pergunta «isto saiu da
    -- instituição?» tem de ter resposta depois de o facto ter acontecido.
    locality          VARCHAR(32) NOT NULL,

    -- O perfil de indexação: como o texto foi preparado antes de ser embebido.
    -- Dois conjuntos com o mesmo modelo e preparações diferentes não são
    -- comparáveis, e é por isso que isto entra na identidade.
    profile           VARCHAR(64) NOT NULL DEFAULT 'chunks-v1',

    status            VARCHAR(16) NOT NULL DEFAULT 'QUEUED',
    failure_reason    TEXT,

    -- Quantos pedaços este conjunto tem de cobrir para ficar completo.
    expected_chunks   INTEGER NOT NULL DEFAULT 0,
    embedded_chunks   INTEGER NOT NULL DEFAULT 0,

    completed_at      TIMESTAMPTZ,
    created_at        TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at        TIMESTAMPTZ NOT NULL DEFAULT now(),

    CONSTRAINT ck_embedding_sets_status CHECK (
        status IN ('QUEUED', 'PROCESSING', 'AVAILABLE', 'FAILED')
    ),
    CONSTRAINT ck_embedding_sets_locality CHECK (
        locality IN ('ocinye_controlled', 'external')
    ),
    CONSTRAINT ck_embedding_sets_dimensions CHECK (dimensions BETWEEN 1 AND 16000),
    CONSTRAINT ck_embedding_sets_counts CHECK (
        embedded_chunks >= 0 AND expected_chunks >= 0
    ),
    -- Um conjunto disponível cobre tudo o que disse que ia cobrir.
    --
    -- > **A replacement embedding set becomes eligible for retrieval only after
    -- > the set is complete.**
    --
    -- Um conjunto com 37 de 92 pedaços não é «parcialmente útil»: é um conjunto
    -- que responde mal e não diz que está incompleto.
    CONSTRAINT ck_embedding_sets_complete CHECK (
        status <> 'AVAILABLE'
        OR (embedded_chunks = expected_chunks AND completed_at IS NOT NULL)
    ),
    -- Uma identidade por versão. Trocar de modelo cria **outro** conjunto; não
    -- reescreve este.
    CONSTRAINT uq_embedding_sets_identity
        UNIQUE (file_version_id, provider, model, revision, profile)
);

CREATE INDEX ix_embedding_sets_version ON embedding_sets (file_version_id);
CREATE INDEX ix_embedding_sets_pending ON embedding_sets (status)
    WHERE status IN ('QUEUED', 'PROCESSING');

-- O vector de um pedaço, dentro de um conjunto.
--
-- O tipo é `vector` sem dimensão fixa: a dimensão é uma propriedade do conjunto
-- e está declarada lá. Fixá-la aqui obrigaria a uma tabela por modelo, ou a
-- escolher um número e chamar-lhe arquitectura.
CREATE TABLE chunk_embeddings (
    id               UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    embedding_set_id UUID NOT NULL REFERENCES embedding_sets (id) ON DELETE CASCADE,
    chunk_id         UUID NOT NULL REFERENCES file_chunks (id) ON DELETE CASCADE,
    vector           VECTOR NOT NULL,
    created_at       TIMESTAMPTZ NOT NULL DEFAULT now(),

    -- Um pedaço tem um vector por conjunto. Reprocessar substitui; não duplica.
    CONSTRAINT uq_chunk_embeddings_member UNIQUE (embedding_set_id, chunk_id)
);

CREATE INDEX ix_chunk_embeddings_set ON chunk_embeddings (embedding_set_id);
