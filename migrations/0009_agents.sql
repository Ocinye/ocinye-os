-- Ocinye OS — AI agent definitions.
--
-- An agent is a *definition*: name, purpose, instructions, scope, and what
-- knowledge it may draw on. Defining one needs no model, so this table exists
-- and is usable with zero AI nodes registered. What a missing model prevents is
-- *execution*, and that is a derived state, not a column (briefing §9).
--
-- Before this migration the Ocinye Workspace had an agent builder and an agent
-- list with no backing store at all: the list rendered `ai_models` under agent
-- column headers, and the create form posted to a route that did not exist.

CREATE TABLE ai_agents (
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    organisation_id     UUID NOT NULL REFERENCES organisations (id) ON DELETE RESTRICT,

    name                VARCHAR(128) NOT NULL,
    purpose             TEXT,
    -- What the agent should do and what it must not. Bounded: an agent whose
    -- instructions are a whole corpus is a retrieval problem wearing a prompt.
    instructions        TEXT,

    -- The capability the agent asks for, never a model name. The AI Gateway
    -- maps capability to model as configuration (ADR-0300, briefing §11).
    -- Maiúsculas, como `AiCapability::as_str()` e como `ai_models.capabilities`.
    -- Um segundo vocabulário para a mesma coisa seria uma fonte silenciosa de
    -- divergência entre o contrato e a base.
    capability          VARCHAR(32) NOT NULL DEFAULT 'GENERAL',

    -- Where the agent may be used, and by whom.
    scope               VARCHAR(32) NOT NULL DEFAULT 'personal',
    -- The unit or research workspace it belongs to. NULL for personal and
    -- institutional agents, which have no narrower home.
    scope_id            UUID,

    -- Ceiling on what the agent may ever retrieve. Never above the creator's
    -- own reach; enforced in the service, and capped again at retrieval time.
    max_classification  VARCHAR(32) NOT NULL DEFAULT 'INTERNAL',

    -- Knowledge sources the agent may draw on. Booleans rather than a join
    -- table: the set is small, closed, and part of the agent's definition.
    uses_bibliography   BOOLEAN NOT NULL DEFAULT false,
    uses_documents      BOOLEAN NOT NULL DEFAULT false,
    uses_datasets       BOOLEAN NOT NULL DEFAULT false,

    -- Whether its owner wants it usable. Distinct from whether it *can* run:
    -- an enabled agent with no serving capability is `configured`, not broken.
    enabled             BOOLEAN NOT NULL DEFAULT true,

    created_by_id       UUID NOT NULL REFERENCES people (id) ON DELETE RESTRICT,
    updated_by_id       UUID REFERENCES people (id) ON DELETE SET NULL,
    archived_at         TIMESTAMPTZ,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT now(),

    CONSTRAINT ck_ai_agents_capability
        CHECK (capability IN ('GENERAL', 'CODING', 'REASONING', 'EMBEDDING')),

    CONSTRAINT ck_ai_agents_scope
        CHECK (scope IN ('personal', 'workspace', 'unit', 'institutional')),

    -- A scoped agent must name its home; a personal or institutional one must
    -- not. Same rule as explicit access grants, for the same reason: an agent
    -- that cannot name its scope has no scope.
    CONSTRAINT ck_ai_agents_scope_id_agrees CHECK (
        (scope IN ('personal', 'institutional') AND scope_id IS NULL)
        OR (scope IN ('workspace', 'unit') AND scope_id IS NOT NULL)
    ),

    CONSTRAINT ck_ai_agents_classification
        CHECK (max_classification IN ('PUBLIC', 'INTERNAL', 'CONFIDENTIAL', 'RESTRICTED')),

    CONSTRAINT ck_ai_agents_name_is_substantive
        CHECK (char_length(btrim(name)) >= 2)
);

-- Names are unique within their scope so two "Assistente de Pesquisa" agents
-- cannot sit side by side in the same workspace. Archived ones do not reserve
-- a name: the point is to avoid confusion among live agents.
CREATE UNIQUE INDEX uq_ai_agents_name_in_scope
    ON ai_agents (
        organisation_id,
        scope,
        coalesce(scope_id, '00000000-0000-0000-0000-000000000000'::uuid),
        lower(name)
    )
    WHERE archived_at IS NULL;

CREATE INDEX ix_ai_agents_organisation ON ai_agents (organisation_id) WHERE archived_at IS NULL;
CREATE INDEX ix_ai_agents_creator ON ai_agents (created_by_id) WHERE archived_at IS NULL;
CREATE INDEX ix_ai_agents_scope ON ai_agents (scope, scope_id) WHERE archived_at IS NULL;

COMMENT ON TABLE ai_agents IS
    'Agent definitions. Usable with zero AI nodes: what a missing model prevents '
    'is execution, which is derived from capability availability, never stored.';

COMMENT ON COLUMN ai_agents.capability IS
    'A capability, never a model name. The Gateway maps capability to model as '
    'configuration (ADR-0300).';

COMMENT ON COLUMN ai_agents.max_classification IS
    'Ceiling on retrieval. An agent never exceeds its actor: effective access is '
    'the intersection of actor, agent and resource policy (briefing §81).';
