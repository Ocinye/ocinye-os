-- Ocinye OS — identity, credentials, sessions and explicit access grants.
--
-- ADR-0103 moves authentication into the Core: username and password, with the
-- Core as the authority. ADR-0102 (Keycloak over OIDC) is superseded.
--
-- Nothing here stores a password. Every credential column holds an Argon2id
-- PHC verifier, and the one plaintext that ever exists — a generated temporary
-- credential — lives in one HTTP response and nowhere else (briefing §95).

-- ---------------------------------------------------------------------------
-- Username
-- ---------------------------------------------------------------------------

-- Case-insensitive: `FMonteiro` and `fmonteiro` must not be two people. Stored
-- as typed so the interface can show the form the person chose; compared and
-- constrained in lower case (briefing §36).
ALTER TABLE people ADD COLUMN username VARCHAR(64);

CREATE UNIQUE INDEX uq_people_username_lower
    ON people (organisation_id, lower(username))
    WHERE username IS NOT NULL;

ALTER TABLE people ADD CONSTRAINT ck_people_username_shape CHECK (
    username IS NULL OR (
        char_length(username) BETWEEN 3 AND 64
        -- Letters, digits, dot, hyphen, underscore. Must start with a letter,
        -- must not end with a separator. No spaces, no case-ambiguity beyond
        -- the index above, nothing that needs escaping in a URL.
        AND username ~ '^[A-Za-z][A-Za-z0-9._-]{1,62}[A-Za-z0-9]$'
    )
);

COMMENT ON COLUMN people.username IS
    'Sign-in name. Unique per organisation, case-insensitively. Changed only by '
    'an administrative flow, never by the holder (briefing §36).';

-- `oidc_subject` stays: ADR-0103 keeps the column so that federating a future
-- identity provider does not need a migration, and so existing rows keep their
-- history. It is no longer how anyone signs in.
COMMENT ON COLUMN people.oidc_subject IS
    'Vestigial under ADR-0103: retained for future federation. Not used to '
    'authenticate. Authentication is username plus password, in credentials.';

-- The account lifecycle gains `disabled` and drops `departed`, which conflated
-- "left the institution" with "cannot sign in" (briefing §39, §41).
ALTER TABLE people DROP CONSTRAINT ck_people_status;

UPDATE people SET status = 'disabled' WHERE status = 'departed';

ALTER TABLE people ADD CONSTRAINT ck_people_status
    CHECK (status IN ('invited', 'active', 'suspended', 'disabled'));

-- ---------------------------------------------------------------------------
-- Credentials
-- ---------------------------------------------------------------------------

CREATE TABLE credentials (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    person_id       UUID NOT NULL REFERENCES people (id) ON DELETE CASCADE,

    kind            VARCHAR(16) NOT NULL,
    state           VARCHAR(16) NOT NULL DEFAULT 'active',

    -- Argon2id PHC string. Carries algorithm, version, cost parameters and a
    -- per-hash salt, which is what makes transparent rehashing possible.
    verifier        TEXT NOT NULL,

    -- Only ever set for a temporary credential. A permanent password does not
    -- expire: forced periodic rotation is explicitly rejected (briefing §8).
    expires_at      TIMESTAMPTZ,
    consumed_at     TIMESTAMPTZ,
    revoked_at      TIMESTAMPTZ,

    -- Who issued it. NULL for the bootstrap administrator, who by definition
    -- has nobody above them.
    issued_by_id    UUID REFERENCES people (id) ON DELETE SET NULL,
    issued_reason   VARCHAR(64),

    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now(),

    CONSTRAINT ck_credentials_kind  CHECK (kind IN ('temporary', 'permanent')),
    CONSTRAINT ck_credentials_state
        CHECK (state IN ('active', 'consumed', 'expired', 'revoked')),

    -- A temporary credential without an expiry is a permanent password wearing
    -- the wrong label. The database refuses to hold one (briefing §20).
    CONSTRAINT ck_credentials_temporary_expires
        CHECK (kind <> 'temporary' OR expires_at IS NOT NULL),

    -- And a permanent password must not carry an expiry, which would
    -- reintroduce rotation by the back door.
    CONSTRAINT ck_credentials_permanent_never_expires
        CHECK (kind <> 'permanent' OR expires_at IS NULL),

    -- A verifier that is not an Argon2id PHC string is not a verifier.
    CONSTRAINT ck_credentials_verifier_is_argon2id
        CHECK (verifier LIKE '$argon2id$%'),

    -- State and timestamps must agree, so a row can never claim to be active
    -- while carrying the moment it was consumed.
    CONSTRAINT ck_credentials_consumed_agrees
        CHECK ((state = 'consumed') = (consumed_at IS NOT NULL)),
    CONSTRAINT ck_credentials_revoked_agrees
        CHECK ((state = 'revoked') = (revoked_at IS NOT NULL))
);

-- At most one live credential of each kind per person. This is the constraint
-- that makes "issuing a reset invalidates the previous one" a database fact
-- rather than an application convention.
CREATE UNIQUE INDEX uq_credentials_live
    ON credentials (person_id, kind) WHERE state = 'active';

CREATE INDEX ix_credentials_person ON credentials (person_id);
CREATE INDEX ix_credentials_expiry
    ON credentials (expires_at) WHERE state = 'active' AND expires_at IS NOT NULL;

COMMENT ON TABLE credentials IS
    'Password verifiers only. No plaintext, ever, of any kind (briefing §95).';

-- ---------------------------------------------------------------------------
-- Sessions
-- ---------------------------------------------------------------------------

CREATE TABLE sessions (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    person_id       UUID NOT NULL REFERENCES people (id) ON DELETE CASCADE,

    -- SHA-256 of the opaque session token. The token itself is in one cookie
    -- and is never persisted: a database leak must not hand over live sessions.
    token_digest    CHAR(64) NOT NULL UNIQUE,

    -- `password_change_required` is the restricted bootstrap session. The Core
    -- refuses ordinary work on it, server-side (briefing §23, §24).
    state           VARCHAR(32) NOT NULL DEFAULT 'active',

    issued_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_seen_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    expires_at      TIMESTAMPTZ NOT NULL,
    revoked_at      TIMESTAMPTZ,
    revoked_reason  VARCHAR(64),

    -- Operational context. Deliberately coarse: enough to recognise one's own
    -- sessions in the security tab, not a browsing history (briefing §38).
    user_agent      VARCHAR(255),
    ip_prefix       VARCHAR(64),

    CONSTRAINT ck_sessions_state
        CHECK (state IN ('password_change_required', 'active', 'revoked')),
    CONSTRAINT ck_sessions_revoked_agrees
        CHECK ((state = 'revoked') = (revoked_at IS NOT NULL))
);

CREATE INDEX ix_sessions_person_live
    ON sessions (person_id) WHERE state <> 'revoked';
CREATE INDEX ix_sessions_expiry ON sessions (expires_at) WHERE state <> 'revoked';

COMMENT ON COLUMN sessions.ip_prefix IS
    'Network prefix, not the full address: enough to spot an unfamiliar origin, '
    'short of logging where a researcher physically is.';

-- ---------------------------------------------------------------------------
-- Authentication attempts
-- ---------------------------------------------------------------------------

CREATE TABLE authentication_attempts (
    id              BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    attempted_at    TIMESTAMPTZ NOT NULL DEFAULT now(),

    -- Lower-cased username as presented. Kept because throttling by account is
    -- the only defence against password spraying, and because a burst against
    -- one name is exactly what an operator needs to see. Never accompanied by
    -- the password, its hash, or its length.
    username        VARCHAR(64),
    ip_prefix       VARCHAR(64),
    outcome         VARCHAR(32) NOT NULL,

    CONSTRAINT ck_authentication_attempts_outcome CHECK (outcome IN (
        'succeeded', 'bad_credentials', 'account_not_authenticable',
        'credential_expired', 'rate_limited'
    ))
);

CREATE INDEX ix_authentication_attempts_username
    ON authentication_attempts (lower(username), attempted_at DESC);
CREATE INDEX ix_authentication_attempts_ip
    ON authentication_attempts (ip_prefix, attempted_at DESC);

COMMENT ON TABLE authentication_attempts IS
    'Throttling signal and operational evidence. Retention is a runbook '
    'concern; see docs/runbooks/.';

-- ---------------------------------------------------------------------------
-- Explicit access grants
-- ---------------------------------------------------------------------------

CREATE TABLE explicit_access_grants (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    organisation_id UUID NOT NULL REFERENCES organisations (id) ON DELETE RESTRICT,
    subject_id      UUID NOT NULL REFERENCES people (id) ON DELETE CASCADE,

    permission      VARCHAR(64) NOT NULL,
    scope           VARCHAR(32) NOT NULL,
    -- Which unit, workspace or resource. NULL only for an institution grant.
    scope_id        UUID,

    -- Why this person needs this. Required: a grant nobody can justify later is
    -- a grant nobody can review later (briefing §63).
    reason          TEXT NOT NULL,

    granted_by_id   UUID NOT NULL REFERENCES people (id) ON DELETE RESTRICT,
    granted_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    expires_at      TIMESTAMPTZ,
    revoked_at      TIMESTAMPTZ,
    revoked_by_id   UUID REFERENCES people (id) ON DELETE SET NULL,
    revoked_reason  TEXT,

    CONSTRAINT ck_grants_scope
        CHECK (scope IN ('institution', 'unit', 'research_workspace', 'resource')),

    -- A scoped grant must name its target; an institution grant must not.
    CONSTRAINT ck_grants_scope_id_agrees CHECK (
        (scope = 'institution' AND scope_id IS NULL)
        OR (scope <> 'institution' AND scope_id IS NOT NULL)
    ),

    CONSTRAINT ck_grants_revoked_agrees
        CHECK ((revoked_at IS NULL) = (revoked_by_id IS NULL)),

    CONSTRAINT ck_grants_reason_is_substantive
        CHECK (char_length(btrim(reason)) >= 8)
);

CREATE UNIQUE INDEX uq_grants_live
    ON explicit_access_grants (subject_id, permission, scope, coalesce(scope_id, '00000000-0000-0000-0000-000000000000'::uuid))
    WHERE revoked_at IS NULL;

CREATE INDEX ix_grants_subject_live
    ON explicit_access_grants (subject_id) WHERE revoked_at IS NULL;

COMMENT ON TABLE explicit_access_grants IS
    'The only way to reach RESTRICTED material without membership. Always '
    'attributable, always reviewable, optionally expiring (briefing §62, §63).';

-- ---------------------------------------------------------------------------
-- Role vocabulary
-- ---------------------------------------------------------------------------

-- `research_lead` and `external_collaborator` join the technical roles.
ALTER TABLE person_roles DROP CONSTRAINT ck_person_roles_role;

ALTER TABLE person_roles ADD CONSTRAINT ck_person_roles_role CHECK (role IN (
    'platform_admin', 'organisation_admin', 'unit_manager', 'research_lead',
    'research_member', 'collaborator', 'external_collaborator', 'auditor'
));
