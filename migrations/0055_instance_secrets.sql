-- Ocinye OS — the Secrets Authority's store (Part 6 of the generalization).
--
-- Credentials an Instance configures for its providers and integrations — an AI
-- provider's API key, an OpenAI-compatible endpoint's token, a connector's
-- credential — are sealed by the Core with the institutional sealing root, under
-- their own HKDF domain (`ocinye/sealing/instance-secrets/v1`, ADR-0110). The
-- plaintext never leaves the Core: no route returns it, and only a Core service
-- whose scope the secret names can open it for the moment of use.
--
-- Revoking wipes the ciphertext; it does not just flag the row. Rotating
-- replaces it and bumps the version; the old value stops existing.

CREATE TABLE instance_secrets (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    organisation_id UUID NOT NULL REFERENCES organisations (id),
    -- What the secret is for, e.g. `ai_provider`.
    kind            VARCHAR(48)  NOT NULL,
    -- A human label, e.g. `OpenAI (produção)`. Never the secret.
    label           VARCHAR(128) NOT NULL,
    -- Which Core service may open it: `ai_gateway`, `mail`, `connector`.
    scope           VARCHAR(32)  NOT NULL,
    nonce           BYTEA,
    ciphertext      BYTEA,
    -- The last four characters, for recognition («…7f2a»). Never more.
    hint            VARCHAR(4),
    version         INTEGER NOT NULL DEFAULT 1,
    status          VARCHAR(16) NOT NULL DEFAULT 'active',
    created_by_id   UUID REFERENCES people (id) ON DELETE SET NULL,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    rotated_at      TIMESTAMPTZ,
    revoked_at      TIMESTAMPTZ,
    last_used_at    TIMESTAMPTZ,

    CONSTRAINT ck_instance_secrets_status CHECK (status IN ('active', 'revoked')),
    CONSTRAINT ck_instance_secrets_scope CHECK (scope IN ('ai_gateway', 'mail', 'connector')),
    -- An active secret has its ciphertext; a revoked one has none left.
    CONSTRAINT ck_instance_secrets_material
        CHECK ((status = 'active') = (ciphertext IS NOT NULL AND nonce IS NOT NULL))
);

CREATE INDEX ix_instance_secrets_organisation ON instance_secrets (organisation_id, status);

COMMENT ON TABLE instance_secrets IS
    'Sealed provider and integration credentials. Plaintext never returned; revocation wipes the ciphertext (ADR-0110).';
