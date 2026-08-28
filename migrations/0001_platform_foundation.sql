-- Ocinye OS — platform foundation.
--
-- Organisation, people, units, memberships, the audit trail and the
-- transactional outbox. Everything else builds on these.

CREATE EXTENSION IF NOT EXISTS "pgcrypto";

-- ---------------------------------------------------------------------------
-- Organisation and people
-- ---------------------------------------------------------------------------

CREATE TABLE organisations (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    slug            VARCHAR(64)  NOT NULL UNIQUE,
    name            VARCHAR(255) NOT NULL,
    legal_name      VARCHAR(255),
    country         CHAR(2),
    description     TEXT,
    created_at      TIMESTAMPTZ  NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ  NOT NULL DEFAULT now()
);

COMMENT ON TABLE organisations IS
    'The institution. Single-tenant today; modelled explicitly so scope is never implicit.';

CREATE TABLE people (
    id                      UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    organisation_id         UUID NOT NULL REFERENCES organisations (id) ON DELETE RESTRICT,
    -- Verified OIDC subject. NULL until the invited person first signs in.
    -- Ocinye Core never stores credentials of any kind (ADR-0102).
    oidc_subject            VARCHAR(255) UNIQUE,
    email                   VARCHAR(320) NOT NULL UNIQUE,
    full_name               VARCHAR(255) NOT NULL,
    display_name            VARCHAR(128),
    -- Institutional truth. Carries no authorization power (ADR-0100).
    institutional_position  VARCHAR(64),
    orcid                   VARCHAR(32),
    biography               TEXT,
    status                  VARCHAR(32) NOT NULL DEFAULT 'invited',
    last_seen_at            TIMESTAMPTZ,
    deactivated_at          TIMESTAMPTZ,
    created_at              TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at              TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT ck_people_status
        CHECK (status IN ('invited', 'active', 'suspended', 'departed'))
);

CREATE INDEX ix_people_organisation ON people (organisation_id);

COMMENT ON COLUMN people.institutional_position IS
    'Founder, director, researcher... Never consulted by the authorization policy.';

CREATE TABLE person_roles (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    person_id       UUID NOT NULL REFERENCES people (id) ON DELETE CASCADE,
    role            VARCHAR(64) NOT NULL,
    granted_reason  TEXT,
    granted_by_id   UUID REFERENCES people (id) ON DELETE SET NULL,
    revoked_at      TIMESTAMPTZ,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT ck_person_roles_role CHECK (role IN (
        'platform_admin', 'organisation_admin', 'unit_manager',
        'research_member', 'collaborator', 'auditor'
    ))
);

-- A role may be granted, revoked and granted again; only one may be live.
CREATE UNIQUE INDEX uq_person_roles_live
    ON person_roles (person_id, role) WHERE revoked_at IS NULL;

CREATE TABLE invitations (
    id                      UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    organisation_id         UUID NOT NULL REFERENCES organisations (id) ON DELETE RESTRICT,
    email                   VARCHAR(320) NOT NULL,
    full_name               VARCHAR(255) NOT NULL,
    institutional_position  VARCHAR(64),
    -- SHA-256 of the token. The plaintext is shown once and never persisted.
    token_digest            CHAR(64) NOT NULL UNIQUE,
    status                  VARCHAR(32) NOT NULL DEFAULT 'pending',
    expires_at              TIMESTAMPTZ NOT NULL,
    accepted_at             TIMESTAMPTZ,
    accepted_person_id      UUID REFERENCES people (id) ON DELETE SET NULL,
    created_by_id           UUID REFERENCES people (id) ON DELETE SET NULL,
    created_at              TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at              TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT ck_invitations_status
        CHECK (status IN ('pending', 'accepted', 'revoked', 'expired'))
);

CREATE INDEX ix_invitations_email ON invitations (email);

-- ---------------------------------------------------------------------------
-- Units
-- ---------------------------------------------------------------------------

CREATE TABLE units (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    organisation_id UUID NOT NULL REFERENCES organisations (id) ON DELETE RESTRICT,
    code            VARCHAR(16)  NOT NULL,
    name            VARCHAR(255) NOT NULL,
    description     TEXT,
    research_areas  TEXT[] NOT NULL DEFAULT '{}',
    status          VARCHAR(32) NOT NULL DEFAULT 'active',
    archived_at     TIMESTAMPTZ,
    created_by_id   UUID REFERENCES people (id) ON DELETE SET NULL,
    updated_by_id   UUID REFERENCES people (id) ON DELETE SET NULL,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT uq_units_org_code UNIQUE (organisation_id, code),
    CONSTRAINT ck_units_status CHECK (status IN ('active', 'archived'))
);

COMMENT ON TABLE units IS
    'Scientific units are rows, never hardcoded: a new unit needs no code change.';

CREATE TABLE unit_memberships (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    unit_id         UUID NOT NULL REFERENCES units (id) ON DELETE CASCADE,
    person_id       UUID NOT NULL REFERENCES people (id) ON DELETE CASCADE,
    role            VARCHAR(32) NOT NULL DEFAULT 'member',
    -- Membership is revoked, never deleted: that a person belonged to a unit is
    -- institutional memory (CLAUDE.md §58).
    revoked_at      TIMESTAMPTZ,
    created_by_id   UUID REFERENCES people (id) ON DELETE SET NULL,
    updated_by_id   UUID REFERENCES people (id) ON DELETE SET NULL,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT uq_unit_memberships UNIQUE (unit_id, person_id),
    CONSTRAINT ck_unit_memberships_role CHECK (role IN ('manager', 'member'))
);

CREATE INDEX ix_unit_memberships_person ON unit_memberships (person_id) WHERE revoked_at IS NULL;

-- ---------------------------------------------------------------------------
-- Audit trail
-- ---------------------------------------------------------------------------

CREATE TABLE audit_events (
    id                UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    occurred_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
    organisation_id   UUID,
    actor_person_id   UUID REFERENCES people (id) ON DELETE RESTRICT,
    actor_subject     VARCHAR(255),
    action            VARCHAR(64) NOT NULL,
    resource_type     VARCHAR(64) NOT NULL,
    resource_id       UUID,
    unit_id           UUID,
    workspace_id      UUID,
    classification    VARCHAR(32),
    outcome           VARCHAR(16) NOT NULL DEFAULT 'success',
    request_id        VARCHAR(64),
    correlation_id    VARCHAR(64),
    -- Bounded, non-sensitive detail: changed field names, state labels, reasons.
    -- Never document content, dataset contents or full payloads.
    metadata          JSONB NOT NULL DEFAULT '{}'::jsonb,
    CONSTRAINT ck_audit_events_outcome CHECK (outcome IN ('success', 'denied', 'failure'))
);

CREATE INDEX ix_audit_events_occurred_at ON audit_events (occurred_at DESC);
CREATE INDEX ix_audit_events_resource ON audit_events (resource_type, resource_id);
CREATE INDEX ix_audit_events_actor_time ON audit_events (actor_person_id, occurred_at DESC);

-- The audit trail is append-oriented. This trigger stops the application from
-- rewriting its own history, including by mistake (CLAUDE.md §37). Deliberate
-- retention work must run as a privileged migration that drops and restores it.
CREATE OR REPLACE FUNCTION audit_events_are_append_only() RETURNS TRIGGER AS $$
BEGIN
    RAISE EXCEPTION 'audit_events is append-only: % is not permitted', TG_OP
        USING ERRCODE = 'insufficient_privilege';
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trg_audit_events_no_update
    BEFORE UPDATE ON audit_events
    FOR EACH ROW EXECUTE FUNCTION audit_events_are_append_only();

CREATE TRIGGER trg_audit_events_no_delete
    BEFORE DELETE ON audit_events
    FOR EACH ROW EXECUTE FUNCTION audit_events_are_append_only();

-- ---------------------------------------------------------------------------
-- Transactional outbox
-- ---------------------------------------------------------------------------

CREATE TABLE outbox_events (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name            VARCHAR(128) NOT NULL,
    aggregate_type  VARCHAR(64)  NOT NULL,
    aggregate_id    UUID NOT NULL,
    -- Identifiers and state transitions only. Never content.
    payload         JSONB NOT NULL DEFAULT '{}'::jsonb,
    correlation_id  VARCHAR(64),
    occurred_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
    available_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    published_at    TIMESTAMPTZ,
    attempts        INTEGER NOT NULL DEFAULT 0,
    last_error      TEXT
);

-- Supports the worker's `FOR UPDATE SKIP LOCKED` drain of pending events.
CREATE INDEX ix_outbox_events_pending
    ON outbox_events (available_at) WHERE published_at IS NULL;
