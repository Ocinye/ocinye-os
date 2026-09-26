-- Ocinye OS — AI policy and routing across providers (Part 8, ADR-0311).
--
-- Two decisions an Instance takes about its AI, and that until now lived only
-- in environment variables, if anywhere:
--
--   1. How far data may travel. `external_max_classification` is the highest
--      classification an **external** provider may ever receive. `NONE` means
--      no external AI at all. A model's own ceiling still applies on top: the
--      stricter of the two wins.
--   2. Who answers each capability first. A preferred provider per capability,
--      and whether the Router may fall back to another candidate when the
--      preferred one does not answer.

CREATE TABLE instance_ai_policy (
    organisation_id              UUID PRIMARY KEY REFERENCES organisations (id) ON DELETE CASCADE,
    external_max_classification  VARCHAR(16) NOT NULL DEFAULT 'INTERNAL',
    updated_by_id                UUID REFERENCES people (id) ON DELETE SET NULL,
    updated_at                   TIMESTAMPTZ NOT NULL DEFAULT now(),

    CONSTRAINT ck_instance_ai_policy_external CHECK (
        external_max_classification IN ('NONE', 'PUBLIC', 'INTERNAL', 'CONFIDENTIAL', 'RESTRICTED')
    )
);

CREATE TABLE ai_routing_preferences (
    organisation_id        UUID NOT NULL REFERENCES organisations (id) ON DELETE CASCADE,
    capability             VARCHAR(16) NOT NULL,
    preferred_provider_id  UUID REFERENCES ai_providers (id) ON DELETE SET NULL,
    allow_fallback         BOOLEAN NOT NULL DEFAULT TRUE,
    updated_by_id          UUID REFERENCES people (id) ON DELETE SET NULL,
    updated_at             TIMESTAMPTZ NOT NULL DEFAULT now(),

    PRIMARY KEY (organisation_id, capability),
    CONSTRAINT ck_ai_routing_preferences_capability
        CHECK (capability IN ('GENERAL', 'CODING', 'REASONING', 'EMBEDDING'))
);

COMMENT ON TABLE instance_ai_policy IS
    'How far an Instance lets data travel to external AI (ADR-0311). Absent row = defaults.';
COMMENT ON TABLE ai_routing_preferences IS
    'Preferred provider and fallback per capability (ADR-0311). Preference, never permission.';
