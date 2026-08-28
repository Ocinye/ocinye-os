-- Ocinye OS — tasks, comments and the activity feed.
--
-- Activity is not the audit trail (briefing §45). Audit exists for security and
-- evidence: append-only and access-restricted. Activity exists for
-- collaboration and carries only what a colleague may already see.

CREATE TABLE tasks (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    organisation_id UUID NOT NULL,
    unit_id         UUID NOT NULL REFERENCES units (id) ON DELETE RESTRICT,
    workspace_id    UUID NOT NULL REFERENCES research_workspaces (id) ON DELETE CASCADE,
    title           VARCHAR(255) NOT NULL,
    description     TEXT,
    state           VARCHAR(32) NOT NULL DEFAULT 'todo',
    priority        VARCHAR(16) NOT NULL DEFAULT 'normal',
    assignee_id     UUID REFERENCES people (id) ON DELETE SET NULL,
    due_on          DATE,
    closed_at       TIMESTAMPTZ,
    classification  VARCHAR(32) NOT NULL DEFAULT 'INTERNAL',
    created_by_id   UUID REFERENCES people (id) ON DELETE SET NULL,
    updated_by_id   UUID REFERENCES people (id) ON DELETE SET NULL,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT ck_tasks_state CHECK (state IN (
        'todo', 'in_progress', 'blocked', 'in_review', 'done', 'cancelled'
    )),
    CONSTRAINT ck_tasks_priority CHECK (priority IN ('low', 'normal', 'high', 'critical')),
    CONSTRAINT ck_tasks_classification
        CHECK (classification IN ('PUBLIC', 'INTERNAL', 'CONFIDENTIAL', 'RESTRICTED')),
    -- A closed task carries a closing timestamp, and only a closed task does.
    CONSTRAINT ck_tasks_closure_consistency CHECK (
        (state IN ('done', 'cancelled') AND closed_at IS NOT NULL)
        OR (state NOT IN ('done', 'cancelled') AND closed_at IS NULL)
    )
);

CREATE INDEX ix_tasks_workspace ON tasks (workspace_id);
CREATE INDEX ix_tasks_assignee_open
    ON tasks (assignee_id, due_on) WHERE state NOT IN ('done', 'cancelled');

CREATE TABLE comments (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    organisation_id UUID NOT NULL,
    unit_id         UUID NOT NULL REFERENCES units (id) ON DELETE RESTRICT,
    workspace_id    UUID NOT NULL REFERENCES research_workspaces (id) ON DELETE CASCADE,
    subject_type    VARCHAR(48) NOT NULL,
    subject_id      UUID NOT NULL,
    body            TEXT NOT NULL,
    classification  VARCHAR(32) NOT NULL DEFAULT 'INTERNAL',
    -- Comments are withdrawn, not erased: the conversation is part of the record.
    withdrawn_at    TIMESTAMPTZ,
    created_by_id   UUID REFERENCES people (id) ON DELETE SET NULL,
    updated_by_id   UUID REFERENCES people (id) ON DELETE SET NULL,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT ck_comments_classification
        CHECK (classification IN ('PUBLIC', 'INTERNAL', 'CONFIDENTIAL', 'RESTRICTED'))
);

CREATE INDEX ix_comments_subject ON comments (subject_type, subject_id, created_at);

CREATE TABLE activity_entries (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    organisation_id UUID NOT NULL,
    unit_id         UUID,
    workspace_id    UUID NOT NULL REFERENCES research_workspaces (id) ON DELETE CASCADE,
    actor_person_id UUID REFERENCES people (id) ON DELETE SET NULL,
    kind            VARCHAR(32)  NOT NULL,
    subject_type    VARCHAR(48)  NOT NULL,
    subject_id      UUID,
    summary         VARCHAR(512) NOT NULL,
    classification  VARCHAR(32) NOT NULL DEFAULT 'INTERNAL',
    context         JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT ck_activity_entries_kind CHECK (kind IN (
        'created', 'updated', 'state_changed', 'commented',
        'member_added', 'attached', 'published'
    )),
    CONSTRAINT ck_activity_entries_classification
        CHECK (classification IN ('PUBLIC', 'INTERNAL', 'CONFIDENTIAL', 'RESTRICTED'))
);

CREATE INDEX ix_activity_entries_workspace_time
    ON activity_entries (workspace_id, created_at DESC);
CREATE INDEX ix_activity_entries_org_time ON activity_entries (organisation_id, created_at DESC);
