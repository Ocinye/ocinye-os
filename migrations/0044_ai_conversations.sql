-- AI conversation provenance (ADR-0309).
--
-- Unlike `ai_jobs` — the operational ledger, which deliberately stores neither
-- prompt nor completion — a conversation *is* the member's own history, and its
-- turns are the content: what the member asked, and what answered. It is
-- personal and private to its owner (like a note), and the response turn carries
-- **typed provenance**: who authored it (member / system / model / tool / agent),
-- how it concluded, and, when a model answered, which model, provider and node.
--
-- The historical truth this exists to keep: a request answered by the system
-- because no inference could run is recorded as a `system` turn, and a later
-- request answered by a model is a `model` turn — the first is never
-- retroactively represented as a model response.

CREATE TABLE ai_conversations (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    organisation_id UUID NOT NULL REFERENCES organisations (id) ON DELETE CASCADE,
    -- The conversation is the member's own; it dies with them.
    owner_id        UUID NOT NULL REFERENCES people (id) ON DELETE CASCADE,
    -- Bound to a Research Workspace when opened from inside one; provenance only.
    workspace_id    UUID REFERENCES research_workspaces (id) ON DELETE SET NULL,
    -- A short human label, derived from the first prompt. Never a summary.
    title           VARCHAR(200) NOT NULL DEFAULT '',
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX ix_ai_conversations_owner_time
    ON ai_conversations (owner_id, updated_at DESC);

CREATE TABLE ai_conversation_turns (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    conversation_id UUID NOT NULL REFERENCES ai_conversations (id) ON DELETE CASCADE,
    -- Order within the conversation, 1-based.
    seq             INTEGER NOT NULL,
    -- Who authored this turn. `member` is the person's own prompt; the rest are
    -- the typed origin of a response (ADR-0308).
    role            VARCHAR(16) NOT NULL,
    -- The prompt, or the answer, in the author's own words.
    content         TEXT NOT NULL,
    -- Present on a response turn: how it concluded, and the machine reason when
    -- degraded. Null on a member turn.
    status          VARCHAR(16),
    reason_code     VARCHAR(48),
    -- Model provenance, present only when a model answered. Plain provenance
    -- strings/ids: they survive the model or node being removed.
    model           VARCHAR(128),
    provider        VARCHAR(128),
    compute_node_id UUID,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),

    CONSTRAINT uq_ai_conversation_turn_seq UNIQUE (conversation_id, seq),
    CONSTRAINT ck_ai_conversation_turn_role
        CHECK (role IN ('member', 'system', 'model', 'tool', 'agent')),
    CONSTRAINT ck_ai_conversation_turn_status
        CHECK (status IS NULL OR status IN ('completed', 'degraded'))
);

CREATE INDEX ix_ai_conversation_turns_conversation
    ON ai_conversation_turns (conversation_id, seq);

COMMENT ON TABLE ai_conversations IS
    'Owner-private AI conversation history with typed turn provenance (ADR-0309).';
COMMENT ON TABLE ai_conversation_turns IS
    'One turn of an AI conversation: a member prompt or a typed-origin response (ADR-0309).';
