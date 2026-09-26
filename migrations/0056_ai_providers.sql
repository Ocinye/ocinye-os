-- Ocinye OS — the AI Fabric's provider registry (Part 7, ADR-0310).
--
-- Until now the only way a model reached an Instance was a node reporting it.
-- An Instance can also connect providers: OpenAI, Anthropic, Google, Mistral, or
-- any OpenAI-compatible endpoint — Ollama or vLLM on its own host, for example.
--
-- A provider is identity, endpoint, residency and a reference to its credential
-- in the Secrets Authority (ADR-0110) — never the credential. Its models live in
-- the same model registry as the ones nodes report (`ai_models`), separate from
-- the provider's identity: a model is what serves a capability; a provider is who
-- runs it.

CREATE TABLE ai_providers (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    organisation_id UUID NOT NULL REFERENCES organisations (id),
    kind            VARCHAR(32)  NOT NULL,
    label           VARCHAR(128) NOT NULL,
    endpoint_url    VARCHAR(512) NOT NULL,
    -- `external`: data leaves the Instance. `local`: it stays on infrastructure
    -- the Instance controls (its own host, its own node).
    residency       VARCHAR(16)  NOT NULL,
    secret_id       UUID REFERENCES instance_secrets (id) ON DELETE SET NULL,
    enabled         BOOLEAN NOT NULL DEFAULT TRUE,
    health          VARCHAR(16) NOT NULL DEFAULT 'unknown',
    last_checked_at TIMESTAMPTZ,
    created_by_id   UUID REFERENCES people (id) ON DELETE SET NULL,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now(),

    CONSTRAINT ck_ai_providers_kind
        CHECK (kind IN ('openai', 'anthropic', 'google', 'mistral', 'openai_compatible')),
    CONSTRAINT ck_ai_providers_residency CHECK (residency IN ('external', 'local')),
    CONSTRAINT ck_ai_providers_health
        CHECK (health IN ('unknown', 'healthy', 'unreachable', 'refused'))
);

CREATE INDEX ix_ai_providers_organisation ON ai_providers (organisation_id);

-- A model belongs to a node or to a provider. Provider models are registered by
-- an administrator; node models are reported, as before.
ALTER TABLE ai_models
    ADD COLUMN provider_id UUID REFERENCES ai_providers (id) ON DELETE CASCADE;

-- `local`: a provider model that runs on infrastructure the Instance controls.
ALTER TABLE ai_models DROP CONSTRAINT ck_ai_models_provider_kind;
ALTER TABLE ai_models
    ADD CONSTRAINT ck_ai_models_provider_kind
        CHECK (provider_kind IN ('ocinye_node', 'external', 'local'));

-- A model belongs to exactly one of a node or a provider. Provider models use
-- the provider's id as `provider_name`, so the existing global identity
-- (provider_name, model_name, version) never collides across Instances.
ALTER TABLE ai_models DROP CONSTRAINT ck_ai_models_node_consistency;
ALTER TABLE ai_models
    ADD CONSTRAINT ck_ai_models_node_consistency CHECK (
        (provider_kind = 'ocinye_node' AND node_id IS NOT NULL AND provider_id IS NULL)
        OR (provider_kind IN ('external', 'local') AND node_id IS NULL AND provider_id IS NOT NULL)
    );

COMMENT ON TABLE ai_providers IS
    'AI providers an Instance connects. Credentials by reference to instance_secrets, never stored here (ADR-0310).';
