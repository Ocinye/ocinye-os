-- Ocinye OS — datasets, versions and files.
--
-- A dataset version is never silently overwritten. Publishing creates a new
-- immutable row with its own checksums and provenance; earlier versions remain
-- readable and citable (briefing §31).

CREATE TABLE datasets (
    id                      UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    organisation_id         UUID NOT NULL,
    unit_id                 UUID NOT NULL REFERENCES units (id) ON DELETE RESTRICT,
    workspace_id            UUID NOT NULL REFERENCES research_workspaces (id) ON DELETE CASCADE,
    code                    VARCHAR(64)  NOT NULL,
    title                   VARCHAR(255) NOT NULL,
    description             TEXT,
    origin                  VARCHAR(48) NOT NULL DEFAULT 'collected_by_ocinye',
    licence                 VARCHAR(128),
    -- Contractual or ethical limits on use. Free text by design: these are
    -- institutional facts a human must read, not machine rules.
    usage_restrictions      TEXT,
    responsible_person_id   UUID REFERENCES people (id) ON DELETE SET NULL,
    acquisition_date        DATE,
    keywords                TEXT[] NOT NULL DEFAULT '{}',
    classification          VARCHAR(32) NOT NULL DEFAULT 'INTERNAL',
    state                   VARCHAR(32) NOT NULL DEFAULT 'draft',
    created_by_id           UUID REFERENCES people (id) ON DELETE SET NULL,
    updated_by_id           UUID REFERENCES people (id) ON DELETE SET NULL,
    created_at              TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at              TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT uq_datasets_org_code UNIQUE (organisation_id, code),
    CONSTRAINT ck_datasets_origin CHECK (origin IN (
        'collected_by_ocinye', 'derived', 'third_party_open',
        'third_party_licensed', 'partner_provided', 'simulated'
    )),
    CONSTRAINT ck_datasets_state
        CHECK (state IN ('draft', 'active', 'deprecated', 'archived')),
    CONSTRAINT ck_datasets_classification
        CHECK (classification IN ('PUBLIC', 'INTERNAL', 'CONFIDENTIAL', 'RESTRICTED'))
);

CREATE INDEX ix_datasets_workspace ON datasets (workspace_id);

CREATE TABLE dataset_versions (
    id                      UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    dataset_id              UUID NOT NULL REFERENCES datasets (id) ON DELETE CASCADE,
    label                   VARCHAR(32) NOT NULL,
    sequence                INTEGER NOT NULL,
    status                  VARCHAR(32) NOT NULL DEFAULT 'draft',
    notes                   TEXT,
    -- How this version was produced and from what.
    provenance              TEXT,
    derived_from_version_id UUID REFERENCES dataset_versions (id) ON DELETE SET NULL,
    total_size_bytes        BIGINT NOT NULL DEFAULT 0,
    file_count              INTEGER NOT NULL DEFAULT 0,
    published_at            TIMESTAMPTZ,
    -- Withdrawn versions are retained, never deleted: they are provenance.
    withdrawn_at            TIMESTAMPTZ,
    withdrawn_reason        TEXT,
    created_by_id           UUID REFERENCES people (id) ON DELETE SET NULL,
    updated_by_id           UUID REFERENCES people (id) ON DELETE SET NULL,
    created_at              TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at              TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT uq_dataset_versions_label UNIQUE (dataset_id, label),
    CONSTRAINT uq_dataset_versions_sequence UNIQUE (dataset_id, sequence),
    CONSTRAINT ck_dataset_versions_status
        CHECK (status IN ('draft', 'published', 'withdrawn')),
    CONSTRAINT ck_dataset_versions_published_has_timestamp
        CHECK (status <> 'published' OR published_at IS NOT NULL),
    CONSTRAINT ck_dataset_versions_withdrawal_has_reason
        CHECK (status <> 'withdrawn' OR withdrawn_reason IS NOT NULL),
    CONSTRAINT ck_dataset_versions_counters
        CHECK (file_count >= 0 AND total_size_bytes >= 0)
);

CREATE INDEX ix_dataset_versions_dataset ON dataset_versions (dataset_id, sequence DESC);

CREATE TABLE dataset_files (
    id                UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    version_id        UUID NOT NULL REFERENCES dataset_versions (id) ON DELETE CASCADE,
    storage_object_id UUID NOT NULL REFERENCES storage_objects (id) ON DELETE RESTRICT,
    -- Logical path inside the dataset, independent of the opaque object key.
    path              VARCHAR(512) NOT NULL,
    created_by_id     UUID REFERENCES people (id) ON DELETE SET NULL,
    created_at        TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT uq_dataset_files_version_path UNIQUE (version_id, path)
);
