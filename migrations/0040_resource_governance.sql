-- Resource Governance domain — the institutional resource control plane.
--
-- Access authority (RBAC) and resource entitlement are different systems
-- (ADR-0108). RBAC answers *what* an actor may access; this answers *how much*
-- institutional capacity an actor may consume. An allocation grants no access,
-- and an institutional position grants no capacity — a Founder is not
-- automatically unlimited.
--
-- Four concepts are kept apart and never collapsed into one mutable counter:
-- capacity (what the institution has), entitlement (what a scope may consume),
-- reservation (capacity committed to an operation) and usage (what was actually
-- consumed). This migration lays the entitlement core — profiles, their typed
-- rules, the allocations derived from or overriding them — and the append-only
-- usage ledger. Capacity, reservations and admission arrive in later slices.
--
-- No enforcement is wired here. Personal storage enforcement, the member and
-- admin experiences, requests, capacity and admission each land in their own
-- reviewable slice.

-- ── Allocation profiles ──────────────────────────────────────────────────
--
-- A configurable, named bundle of entitlements. New members receive one; the
-- values are institutional configuration, editable by administrators, never
-- frozen in business logic.
CREATE TABLE resource_profiles (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    organisation_id UUID NOT NULL REFERENCES organisations (id) ON DELETE CASCADE,
    code            VARCHAR(64) NOT NULL,
    name            VARCHAR(120) NOT NULL,
    description     TEXT NOT NULL DEFAULT '',
    -- The profile assigned to a member with no explicit profile of their own.
    is_default      BOOLEAN NOT NULL DEFAULT FALSE,
    -- Scheduling priority the profile confers. Never a capacity or access grant.
    priority        VARCHAR(24) NOT NULL DEFAULT 'normal',
    status          VARCHAR(16) NOT NULL DEFAULT 'active',
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now(),

    CONSTRAINT ck_resource_profiles_priority
        CHECK (priority IN ('low', 'normal', 'high', 'critical_system')),
    CONSTRAINT ck_resource_profiles_status
        CHECK (status IN ('active', 'inactive')),
    CONSTRAINT uq_resource_profiles_code UNIQUE (organisation_id, code)
);
COMMENT ON TABLE resource_profiles IS
    'Configurable named entitlement bundles assigned to scopes (ADR-0108).';

-- At most one default profile per organisation.
CREATE UNIQUE INDEX uq_resource_profiles_one_default
    ON resource_profiles (organisation_id) WHERE is_default;

-- ── Profile rules ────────────────────────────────────────────────────────
--
-- One typed entitlement inside a profile: how much of a resource, in an
-- explicit unit. A number without a unit is a defect, so both are stored.
CREATE TABLE resource_profile_rules (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    profile_id      UUID NOT NULL REFERENCES resource_profiles (id) ON DELETE CASCADE,
    resource_type   VARCHAR(32) NOT NULL,
    quantity        BIGINT NOT NULL,
    unit            VARCHAR(24) NOT NULL,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),

    CONSTRAINT ck_resource_profile_rules_type CHECK (resource_type IN (
        'persistent_storage', 'cpu', 'ram', 'gpu', 'vram', 'gpu_time',
        'compute_time', 'concurrency', 'model_access', 'model_context',
        'job_duration', 'request_rate'
    )),
    CONSTRAINT ck_resource_profile_rules_unit CHECK (unit IN (
        'bytes', 'vcpu', 'gpu_count', 'gpu_seconds', 'cpu_seconds', 'requests',
        'tokens', 'context_tokens', 'seconds', 'count'
    )),
    CONSTRAINT ck_resource_profile_rules_qty CHECK (quantity >= 0),
    CONSTRAINT uq_resource_profile_rules UNIQUE (profile_id, resource_type)
);
COMMENT ON TABLE resource_profile_rules IS
    'A typed, unit-explicit entitlement within a profile.';

-- ── Allocations (effective entitlement) ──────────────────────────────────
--
-- What a scope may consume. Derived from its profile, or an explicit override,
-- or a time-bounded temporary grant. Historical allocations are never
-- overwritten: a change supersedes the prior row, preserving the record of what
-- was granted and by whom.
CREATE TABLE resource_allocations (
    id               UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    organisation_id  UUID NOT NULL REFERENCES organisations (id) ON DELETE CASCADE,
    resource_type    VARCHAR(32) NOT NULL,
    unit             VARCHAR(24) NOT NULL,
    -- The scope is a (type, id) pair so a further scope needs no new column.
    scope_type       VARCHAR(24) NOT NULL,
    scope_id         UUID NOT NULL,
    quantity         BIGINT NOT NULL,
    source           VARCHAR(16) NOT NULL,
    -- The profile this was derived from, when source = profile.
    profile_id       UUID REFERENCES resource_profiles (id) ON DELETE SET NULL,
    starts_at        TIMESTAMPTZ NOT NULL DEFAULT now(),
    -- NULL means standing (no expiry). A temporary grant sets this.
    expires_at       TIMESTAMPTZ,
    reason           TEXT NOT NULL DEFAULT '',
    status           VARCHAR(16) NOT NULL DEFAULT 'active',
    created_by_id    UUID REFERENCES people (id) ON DELETE SET NULL,
    created_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
    superseded_at    TIMESTAMPTZ,
    superseded_by_id UUID REFERENCES resource_allocations (id) ON DELETE SET NULL,

    CONSTRAINT ck_resource_allocations_type CHECK (resource_type IN (
        'persistent_storage', 'cpu', 'ram', 'gpu', 'vram', 'gpu_time',
        'compute_time', 'concurrency', 'model_access', 'model_context',
        'job_duration', 'request_rate'
    )),
    CONSTRAINT ck_resource_allocations_unit CHECK (unit IN (
        'bytes', 'vcpu', 'gpu_count', 'gpu_seconds', 'cpu_seconds', 'requests',
        'tokens', 'context_tokens', 'seconds', 'count'
    )),
    CONSTRAINT ck_resource_allocations_scope CHECK (scope_type IN (
        'organization', 'member', 'unit', 'research_workspace', 'project'
    )),
    CONSTRAINT ck_resource_allocations_source
        CHECK (source IN ('profile', 'override', 'temporary')),
    CONSTRAINT ck_resource_allocations_status
        CHECK (status IN ('active', 'superseded', 'revoked', 'expired')),
    CONSTRAINT ck_resource_allocations_qty CHECK (quantity >= 0),
    CONSTRAINT ck_resource_allocations_window
        CHECK (expires_at IS NULL OR expires_at > starts_at)
);
COMMENT ON TABLE resource_allocations IS
    'What a scope may consume, from profile, override or temporary grant (ADR-0108).';

-- The hot lookup: a scope's live allocations for a resource.
CREATE INDEX ix_resource_allocations_scope
    ON resource_allocations (scope_type, scope_id, resource_type)
    WHERE status = 'active';

-- ── Usage ledger ─────────────────────────────────────────────────────────
--
-- The canonical, append-only record of what was actually consumed. Materialised
-- counters may exist for performance, but they must reconcile against this. A
-- usage event is a completed fact — never a running counter — so it is written
-- once, at settlement, and never updated. Standing storage is measured directly
-- from `storage_objects`, not from this ledger; the ledger records consumable
-- flows (compute time, requests) attributed to one canonical charge scope.
CREATE TABLE resource_usage_events (
    id                UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    organisation_id   UUID NOT NULL REFERENCES organisations (id) ON DELETE CASCADE,
    resource_type     VARCHAR(32) NOT NULL,
    quantity          BIGINT NOT NULL,
    unit              VARCHAR(24) NOT NULL,
    actor_person_id   UUID REFERENCES people (id) ON DELETE SET NULL,
    -- Exactly one canonical charge scope; other columns are descriptive context.
    charge_scope_type VARCHAR(24) NOT NULL,
    charge_scope_id   UUID NOT NULL,
    unit_id           UUID REFERENCES units (id) ON DELETE SET NULL,
    workspace_id      UUID REFERENCES research_workspaces (id) ON DELETE SET NULL,
    project_id        UUID REFERENCES projects (id) ON DELETE SET NULL,
    compute_node_id   UUID REFERENCES compute_nodes (id) ON DELETE SET NULL,
    model_id          UUID REFERENCES ai_models (id) ON DELETE SET NULL,
    job_id            UUID,
    correlation_id    VARCHAR(64),
    source            VARCHAR(32) NOT NULL DEFAULT 'measured',
    reason            TEXT NOT NULL DEFAULT '',
    started_at        TIMESTAMPTZ NOT NULL DEFAULT now(),
    ended_at          TIMESTAMPTZ,
    created_at        TIMESTAMPTZ NOT NULL DEFAULT now(),

    CONSTRAINT ck_resource_usage_type CHECK (resource_type IN (
        'persistent_storage', 'cpu', 'ram', 'gpu', 'vram', 'gpu_time',
        'compute_time', 'concurrency', 'model_access', 'model_context',
        'job_duration', 'request_rate'
    )),
    CONSTRAINT ck_resource_usage_unit CHECK (unit IN (
        'bytes', 'vcpu', 'gpu_count', 'gpu_seconds', 'cpu_seconds', 'requests',
        'tokens', 'context_tokens', 'seconds', 'count'
    )),
    CONSTRAINT ck_resource_usage_scope CHECK (charge_scope_type IN (
        'organization', 'member', 'unit', 'research_workspace', 'project'
    ))
);
COMMENT ON TABLE resource_usage_events IS
    'Append-only canonical ledger of consumed resources (ADR-0108).';

CREATE INDEX ix_resource_usage_charge
    ON resource_usage_events (charge_scope_type, charge_scope_id, started_at);
CREATE INDEX ix_resource_usage_actor
    ON resource_usage_events (actor_person_id, started_at)
    WHERE actor_person_id IS NOT NULL;

-- The ledger is evidence: the application may not rewrite or erase it. As with
-- `audit_events`, this is enforced in the database, not merely in code.
CREATE OR REPLACE FUNCTION resource_usage_events_are_append_only()
    RETURNS TRIGGER AS $$
BEGIN
    RAISE EXCEPTION 'resource_usage_events is append-only'
        USING ERRCODE = 'insufficient_privilege';
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trg_resource_usage_no_update
    BEFORE UPDATE ON resource_usage_events
    FOR EACH ROW EXECUTE FUNCTION resource_usage_events_are_append_only();

CREATE TRIGGER trg_resource_usage_no_delete
    BEFORE DELETE ON resource_usage_events
    FOR EACH ROW EXECUTE FUNCTION resource_usage_events_are_append_only();

-- ── Default profile for existing organisations ───────────────────────────
--
-- Every organisation that already exists gets a conservative default profile so
-- members resolve an entitlement from day one. New organisations receive theirs
-- when they are provisioned (member-lifecycle slice). The storage default is
-- 10 GiB — deliberately conservative for an institution with no declared
-- physical storage yet, and editable by administrators; it is institutional
-- configuration, not a value frozen in code (ADR-0108, briefing §52).
INSERT INTO resource_profiles (organisation_id, code, name, description, is_default, priority)
SELECT o.id, 'MEMBER_STANDARD', 'Membro padrão',
       'Perfil de recursos por omissão de um membro da instituição.', TRUE, 'normal'
  FROM organisations o
 WHERE NOT EXISTS (
     SELECT 1 FROM resource_profiles p
      WHERE p.organisation_id = o.id AND p.is_default
 );

INSERT INTO resource_profile_rules (profile_id, resource_type, quantity, unit)
SELECT p.id, 'persistent_storage', 10737418240, 'bytes'
  FROM resource_profiles p
 WHERE p.code = 'MEMBER_STANDARD'
   AND NOT EXISTS (
       SELECT 1 FROM resource_profile_rules r
        WHERE r.profile_id = p.id AND r.resource_type = 'persistent_storage'
   );
