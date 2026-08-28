-- Ocinye OS — the institutional search index.
--
-- One row per indexable research object, carrying the authorization context
-- alongside the text. The authorization predicate is part of every query that
-- reads this table, so LIMIT, OFFSET and COUNT all operate on the authorised
-- set only (ADR-0202).

-- pgvector: the column exists so semantic search can be added without
-- remodelling. With no Ocinye AI node there are no embeddings, and semantic
-- search is reported unavailable rather than simulated.
CREATE EXTENSION IF NOT EXISTS "vector";

CREATE TABLE search_documents (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    organisation_id UUID NOT NULL,
    unit_id         UUID,
    workspace_id    UUID REFERENCES research_workspaces (id) ON DELETE CASCADE,
    entity_type     VARCHAR(48)  NOT NULL,
    entity_id       UUID NOT NULL,
    title           VARCHAR(512) NOT NULL,
    -- A bounded excerpt for rendering results. The index is a finding aid, not
    -- a second copy of the corpus.
    excerpt         TEXT,
    classification  VARCHAR(32) NOT NULL DEFAULT 'INTERNAL',
    -- 'simple' rather than a language configuration: the corpus is bilingual
    -- (Portuguese content, English terminology) and a single-language stemmer
    -- would degrade the other.
    search_vector   TSVECTOR,
    embedding       VECTOR(1024),
    embedded_at     TIMESTAMPTZ,
    indexed_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT uq_search_documents_entity UNIQUE (entity_type, entity_id),
    CONSTRAINT ck_search_documents_classification
        CHECK (classification IN ('PUBLIC', 'INTERNAL', 'CONFIDENTIAL', 'RESTRICTED'))
);

CREATE INDEX ix_search_documents_vector ON search_documents USING GIN (search_vector);
CREATE INDEX ix_search_documents_scope
    ON search_documents (organisation_id, classification, entity_type);
CREATE INDEX ix_search_documents_workspace ON search_documents (workspace_id);

-- No ANN index on `embedding` yet: an empty column would gain nothing, and the
-- right index type depends on the embedding model that does not exist. Adding
-- it is a migration, made when the first Ocinye AI node is enrolled.
