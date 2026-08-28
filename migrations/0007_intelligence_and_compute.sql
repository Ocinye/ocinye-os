-- Ocinye OS — the Intelligence and Compute Planes.
--
-- Both planes exist with zero providers and zero nodes. That is the current,
-- correct state and it is stored and reported as such (ADR-0300, ADR-0500).

-- ---------------------------------------------------------------------------
-- Compute Registry
-- ---------------------------------------------------------------------------

CREATE TABLE compute_nodes (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    organisation_id UUID NOT NULL,
    -- Supplied at registration. No node identifier is hardcoded anywhere.
    identifier      VARCHAR(24)  NOT NULL UNIQUE,
    display_name    VARCHAR(128) NOT NULL,
    kind            VARCHAR(32)  NOT NULL DEFAULT 'gpu',
    location_label  VARCHAR(128),
    status          VARCHAR(32)  NOT NULL DEFAULT 'pending_enrollment',
    -- Reported by the agent at heartbeat. Never assumed by the Core, and
    -- treated as untrusted input: a compromised node may lie about itself.
    cpu_cores       INTEGER,
    memory_bytes    BIGINT,
    storage_bytes   BIGINT,
    gpus            JSONB NOT NULL DEFAULT '[]'::jsonb,
    capabilities    JSONB NOT NULL DEFAULT '[]'::jsonb,
    agent_version   VARCHAR(32),
    -- Liveness is derived from this timestamp, never from a stored flag.
    last_seen_at    TIMESTAMPTZ,
    last_health     JSONB NOT NULL DEFAULT '{}'::jsonb,
    retired_at      TIMESTAMPTZ,
    notes           TEXT,
    created_by_id   UUID REFERENCES people (id) ON DELETE SET NULL,
    updated_by_id   UUID REFERENCES people (id) ON DELETE SET NULL,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT ck_compute_nodes_kind CHECK (kind IN ('gpu', 'cpu', 'hpc', 'storage')),
    CONSTRAINT ck_compute_nodes_status CHECK (status IN (
        'pending_enrollment', 'online', 'offline', 'draining', 'retired'
    ))
);

-- A node's own identity. A node is not a user and never reuses human
-- credentials (ADR-0500). Only digests are stored.
CREATE TABLE node_credentials (
    id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    node_id       UUID NOT NULL REFERENCES compute_nodes (id) ON DELETE CASCADE,
    -- 'enrollment' (single use, short-lived) or 'agent' (long-lived, rotatable).
    purpose       VARCHAR(32) NOT NULL,
    token_digest  CHAR(64) NOT NULL UNIQUE,
    expires_at    TIMESTAMPTZ,
    consumed_at   TIMESTAMPTZ,
    revoked_at    TIMESTAMPTZ,
    created_by_id UUID REFERENCES people (id) ON DELETE SET NULL,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT ck_node_credentials_purpose CHECK (purpose IN ('enrollment', 'agent'))
);

CREATE INDEX ix_node_credentials_node ON node_credentials (node_id);

-- ---------------------------------------------------------------------------
-- Model registry
-- ---------------------------------------------------------------------------

CREATE TABLE ai_models (
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    provider_kind       VARCHAR(32)  NOT NULL DEFAULT 'ocinye_node',
    provider_name       VARCHAR(128) NOT NULL,
    node_id             UUID REFERENCES compute_nodes (id) ON DELETE CASCADE,
    model_name          VARCHAR(128) NOT NULL,
    version             VARCHAR(64)  NOT NULL DEFAULT 'unknown',
    capabilities        JSONB NOT NULL DEFAULT '[]'::jsonb,
    context_limit       INTEGER,
    status              VARCHAR(32) NOT NULL DEFAULT 'unavailable',
    -- Ceiling on what may ever be sent to this model. Never widened implicitly.
    max_classification  VARCHAR(32) NOT NULL DEFAULT 'INTERNAL',
    enabled             BOOLEAN NOT NULL DEFAULT TRUE,
    reported_at         TIMESTAMPTZ,
    created_by_id       UUID REFERENCES people (id) ON DELETE SET NULL,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT uq_ai_models_identity UNIQUE (provider_name, model_name, version),
    CONSTRAINT ck_ai_models_provider_kind CHECK (provider_kind IN ('ocinye_node', 'external')),
    CONSTRAINT ck_ai_models_status CHECK (status IN ('available', 'unavailable', 'disabled')),
    CONSTRAINT ck_ai_models_max_classification
        CHECK (max_classification IN ('PUBLIC', 'INTERNAL', 'CONFIDENTIAL', 'RESTRICTED')),
    -- A node-hosted model must name its node; an external one must not.
    CONSTRAINT ck_ai_models_node_consistency CHECK (
        (provider_kind = 'ocinye_node' AND node_id IS NOT NULL)
        OR (provider_kind = 'external' AND node_id IS NULL)
    )
);

-- ---------------------------------------------------------------------------
-- AI jobs
-- ---------------------------------------------------------------------------

CREATE TABLE ai_jobs (
    id                UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    organisation_id   UUID NOT NULL,
    workspace_id      UUID,
    requested_by_id   UUID REFERENCES people (id) ON DELETE SET NULL,
    capability        VARCHAR(32) NOT NULL,
    model_id          UUID REFERENCES ai_models (id) ON DELETE SET NULL,
    scope             VARCHAR(48) NOT NULL,
    status            VARCHAR(32) NOT NULL DEFAULT 'queued',
    -- Why a job was refused — most often: no node provides the capability.
    rejection_reason  TEXT,
    -- References of retrieved artefacts, for provenance. Never their contents.
    -- Prompts and completions are deliberately not stored here.
    retrieved_refs    JSONB NOT NULL DEFAULT '[]'::jsonb,
    started_at        TIMESTAMPTZ,
    finished_at       TIMESTAMPTZ,
    created_at        TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at        TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT ck_ai_jobs_capability
        CHECK (capability IN ('GENERAL', 'CODING', 'REASONING', 'EMBEDDING')),
    CONSTRAINT ck_ai_jobs_status CHECK (status IN (
        'queued', 'running', 'succeeded', 'failed', 'rejected', 'cancelled'
    )),
    CONSTRAINT ck_ai_jobs_rejection_has_reason
        CHECK (status <> 'rejected' OR rejection_reason IS NOT NULL)
);

CREATE INDEX ix_ai_jobs_org_time ON ai_jobs (organisation_id, created_at DESC);
