-- Ocinye OS — object storage, bibliography, notes and documents.

-- ---------------------------------------------------------------------------
-- Storage: metadata here, blobs in S3-compatible object storage (ADR-0200)
-- ---------------------------------------------------------------------------

CREATE TABLE storage_backends (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    code            VARCHAR(64)  NOT NULL UNIQUE,
    kind            VARCHAR(32)  NOT NULL DEFAULT 's3_compatible',
    display_name    VARCHAR(128) NOT NULL,
    -- Human label of the physical location. Never a claim of ownership.
    location_label  VARCHAR(128) NOT NULL,
    region          VARCHAR(64),
    bucket          VARCHAR(128) NOT NULL,
    -- Physical residency, explicitly declared. UNDECLARED is the honest default
    -- while Ocinye owns no infrastructure (ADR-0201).
    residency       VARCHAR(32) NOT NULL DEFAULT 'UNDECLARED',
    migration_state VARCHAR(32) NOT NULL DEFAULT 'stable',
    is_default      BOOLEAN NOT NULL DEFAULT FALSE,
    is_active       BOOLEAN NOT NULL DEFAULT TRUE,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT ck_storage_backends_residency CHECK (
        residency IN ('UNDECLARED', 'THIRD_PARTY_CLOUD', 'OCINYE_CAMAMA', 'OCINYE_COLOCATION')
    ),
    CONSTRAINT ck_storage_backends_migration_state
        CHECK (migration_state IN ('stable', 'migration_planned', 'migrating'))
);

CREATE UNIQUE INDEX uq_storage_backends_single_default
    ON storage_backends ((TRUE)) WHERE is_default;

CREATE TABLE storage_objects (
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    backend_id          UUID NOT NULL REFERENCES storage_backends (id) ON DELETE RESTRICT,
    organisation_id     UUID NOT NULL,
    unit_id             UUID,
    workspace_id        UUID,
    -- System-generated and opaque. Knowing it grants nothing: every download is
    -- authorised by the Core and served by a short-lived signed URL.
    object_key          VARCHAR(512) NOT NULL,
    -- The user-supplied name, normalised. Metadata only; never the key.
    original_filename   VARCHAR(255) NOT NULL,
    content_type        VARCHAR(128) NOT NULL,
    size_bytes          BIGINT NOT NULL,
    checksum_sha256     CHAR(64) NOT NULL,
    classification      VARCHAR(32) NOT NULL DEFAULT 'INTERNAL',
    status              VARCHAR(32) NOT NULL DEFAULT 'pending',
    -- NULL means "not scanned", never "clean" (CLAUDE.md §69).
    scanned_at          TIMESTAMPTZ,
    scan_result         VARCHAR(64),
    created_by_id       UUID REFERENCES people (id) ON DELETE SET NULL,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT uq_storage_objects_backend_key UNIQUE (backend_id, object_key),
    CONSTRAINT ck_storage_objects_status CHECK (status IN ('pending', 'stored', 'quarantined')),
    CONSTRAINT ck_storage_objects_size CHECK (size_bytes >= 0),
    CONSTRAINT ck_storage_objects_classification
        CHECK (classification IN ('PUBLIC', 'INTERNAL', 'CONFIDENTIAL', 'RESTRICTED'))
);

CREATE INDEX ix_storage_objects_workspace ON storage_objects (workspace_id);
CREATE INDEX ix_storage_objects_checksum ON storage_objects (checksum_sha256);

-- ---------------------------------------------------------------------------
-- Documents
-- ---------------------------------------------------------------------------

CREATE TABLE documents (
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    organisation_id     UUID NOT NULL,
    unit_id             UUID NOT NULL REFERENCES units (id) ON DELETE RESTRICT,
    workspace_id        UUID NOT NULL REFERENCES research_workspaces (id) ON DELETE CASCADE,
    storage_object_id   UUID NOT NULL REFERENCES storage_objects (id) ON DELETE RESTRICT,
    kind                VARCHAR(48)  NOT NULL DEFAULT 'other',
    title               VARCHAR(255) NOT NULL,
    description         TEXT,
    document_date       DATE,
    classification      VARCHAR(32) NOT NULL DEFAULT 'INTERNAL',
    created_by_id       UUID REFERENCES people (id) ON DELETE SET NULL,
    updated_by_id       UUID REFERENCES people (id) ON DELETE SET NULL,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT ck_documents_classification
        CHECK (classification IN ('PUBLIC', 'INTERNAL', 'CONFIDENTIAL', 'RESTRICTED'))
);

CREATE INDEX ix_documents_workspace ON documents (workspace_id);

-- ---------------------------------------------------------------------------
-- Bibliography
-- ---------------------------------------------------------------------------

CREATE TABLE sources (
    id                      UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    organisation_id         UUID NOT NULL,
    unit_id                 UUID NOT NULL REFERENCES units (id) ON DELETE RESTRICT,
    workspace_id            UUID NOT NULL REFERENCES research_workspaces (id) ON DELETE CASCADE,
    source_type             VARCHAR(32)  NOT NULL DEFAULT 'article',
    title                   VARCHAR(512) NOT NULL,
    authors                 TEXT[] NOT NULL DEFAULT '{}',
    year                    INTEGER,
    container_title         VARCHAR(512),
    publisher               VARCHAR(255),
    doi                     VARCHAR(128),
    isbn                    VARCHAR(32),
    url                     VARCHAR(1024),
    abstract                TEXT,
    keywords                TEXT[] NOT NULL DEFAULT '{}',
    licence                 VARCHAR(128),
    -- The recorded legal basis for holding full content. Without one, Ocinye
    -- keeps metadata, citation, notes and an authorised link (briefing §30).
    content_right           VARCHAR(48) NOT NULL DEFAULT 'metadata_only',
    origin                  VARCHAR(255),
    citation_key            VARCHAR(128),
    -- Raw imported record (BibTeX fields, DOI response) kept for provenance.
    raw_metadata            JSONB NOT NULL DEFAULT '{}'::jsonb,
    classification          VARCHAR(32) NOT NULL DEFAULT 'INTERNAL',
    full_text_document_id   UUID REFERENCES documents (id) ON DELETE SET NULL,
    created_by_id           UUID REFERENCES people (id) ON DELETE SET NULL,
    updated_by_id           UUID REFERENCES people (id) ON DELETE SET NULL,
    created_at              TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at              TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT ck_sources_content_right CHECK (content_right IN (
        'metadata_only', 'open_licence', 'institutional_licence',
        'authored_by_ocinye', 'public_domain', 'permission_granted'
    )),
    -- Full text may only be attached where a legal basis was recorded.
    CONSTRAINT ck_sources_full_text_requires_basis CHECK (
        full_text_document_id IS NULL OR content_right <> 'metadata_only'
    ),
    CONSTRAINT ck_sources_classification
        CHECK (classification IN ('PUBLIC', 'INTERNAL', 'CONFIDENTIAL', 'RESTRICTED'))
);

CREATE INDEX ix_sources_workspace ON sources (workspace_id);
CREATE UNIQUE INDEX uq_sources_workspace_doi
    ON sources (workspace_id, doi) WHERE doi IS NOT NULL;

-- ---------------------------------------------------------------------------
-- Notes
-- ---------------------------------------------------------------------------

CREATE TABLE notes (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    organisation_id UUID NOT NULL,
    unit_id         UUID NOT NULL REFERENCES units (id) ON DELETE RESTRICT,
    workspace_id    UUID NOT NULL REFERENCES research_workspaces (id) ON DELETE CASCADE,
    title           VARCHAR(255) NOT NULL,
    body            TEXT NOT NULL DEFAULT '',
    tags            TEXT[] NOT NULL DEFAULT '{}',
    classification  VARCHAR(32) NOT NULL DEFAULT 'INTERNAL',
    revision        INTEGER NOT NULL DEFAULT 1,
    created_by_id   UUID REFERENCES people (id) ON DELETE SET NULL,
    updated_by_id   UUID REFERENCES people (id) ON DELETE SET NULL,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT ck_notes_classification
        CHECK (classification IN ('PUBLIC', 'INTERNAL', 'CONFIDENTIAL', 'RESTRICTED')),
    CONSTRAINT ck_notes_revision CHECK (revision >= 1)
);

CREATE INDEX ix_notes_workspace ON notes (workspace_id);

-- Immutable snapshot taken before each edit: a note's history is preserved.
CREATE TABLE note_revisions (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    note_id         UUID NOT NULL REFERENCES notes (id) ON DELETE CASCADE,
    revision        INTEGER NOT NULL,
    title           VARCHAR(255) NOT NULL,
    body            TEXT NOT NULL,
    authored_by_id  UUID REFERENCES people (id) ON DELETE SET NULL,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT uq_note_revisions UNIQUE (note_id, revision)
);

-- ---------------------------------------------------------------------------
-- Research links — the seed of the future Ocinye Knowledge Graph
-- ---------------------------------------------------------------------------

CREATE TABLE research_links (
    id                UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    organisation_id   UUID NOT NULL,
    workspace_id      UUID NOT NULL REFERENCES research_workspaces (id) ON DELETE CASCADE,
    source_type_name  VARCHAR(48) NOT NULL,
    source_id         UUID NOT NULL,
    relation          VARCHAR(48) NOT NULL,
    target_type_name  VARCHAR(48) NOT NULL,
    target_id         UUID NOT NULL,
    note              TEXT,
    created_by_id     UUID REFERENCES people (id) ON DELETE SET NULL,
    created_at        TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT uq_research_links_edge
        UNIQUE (source_type_name, source_id, relation, target_type_name, target_id),
    CONSTRAINT ck_research_links_relation CHECK (relation IN (
        'cites', 'supports', 'refutes', 'derived_from', 'uses', 'produces', 'relates_to'
    )),
    CONSTRAINT ck_research_links_no_self_loop
        CHECK (NOT (source_type_name = target_type_name AND source_id = target_id))
);

CREATE INDEX ix_research_links_source ON research_links (source_type_name, source_id);
CREATE INDEX ix_research_links_target ON research_links (target_type_name, target_id);

COMMENT ON TABLE research_links IS
    'Typed, first-class relations between research objects, so the Knowledge Graph '
    'can be built later without remodelling the domain (ADR-0005, briefing §26).';
