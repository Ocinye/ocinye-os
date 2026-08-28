-- Ocinye OS — research: workspaces, ideas and projects.
--
-- The Research Workspace is the contextual container that holds everything an
-- idea or project accumulates. When an idea is promoted, the *same* workspace
-- carries over, so the knowledge gathered while exploring is not orphaned and
-- the lineage idea -> project stays intact on both sides (briefing §24, §25).

CREATE TABLE research_workspaces (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    organisation_id UUID NOT NULL REFERENCES organisations (id) ON DELETE RESTRICT,
    unit_id         UUID NOT NULL REFERENCES units (id) ON DELETE RESTRICT,
    code            VARCHAR(48)  NOT NULL,
    title           VARCHAR(255) NOT NULL,
    kind            VARCHAR(32)  NOT NULL DEFAULT 'idea',
    classification  VARCHAR(32)  NOT NULL DEFAULT 'INTERNAL',
    archived_at     TIMESTAMPTZ,
    created_by_id   UUID REFERENCES people (id) ON DELETE SET NULL,
    updated_by_id   UUID REFERENCES people (id) ON DELETE SET NULL,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT uq_research_workspaces_org_code UNIQUE (organisation_id, code),
    CONSTRAINT ck_research_workspaces_kind CHECK (kind IN ('idea', 'project')),
    CONSTRAINT ck_research_workspaces_classification
        CHECK (classification IN ('PUBLIC', 'INTERNAL', 'CONFIDENTIAL', 'RESTRICTED'))
);

CREATE INDEX ix_research_workspaces_unit ON research_workspaces (unit_id);

CREATE TABLE workspace_memberships (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    workspace_id    UUID NOT NULL REFERENCES research_workspaces (id) ON DELETE CASCADE,
    person_id       UUID NOT NULL REFERENCES people (id) ON DELETE CASCADE,
    role            VARCHAR(32) NOT NULL DEFAULT 'member',
    revoked_at      TIMESTAMPTZ,
    created_by_id   UUID REFERENCES people (id) ON DELETE SET NULL,
    updated_by_id   UUID REFERENCES people (id) ON DELETE SET NULL,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT uq_workspace_memberships UNIQUE (workspace_id, person_id),
    CONSTRAINT ck_workspace_memberships_role CHECK (role IN ('lead', 'member', 'viewer'))
);

CREATE INDEX ix_workspace_memberships_person
    ON workspace_memberships (person_id) WHERE revoked_at IS NULL;

CREATE TABLE projects (
    id                      UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    organisation_id         UUID NOT NULL REFERENCES organisations (id) ON DELETE RESTRICT,
    workspace_id            UUID NOT NULL UNIQUE
                                 REFERENCES research_workspaces (id) ON DELETE RESTRICT,
    code                    VARCHAR(32)  NOT NULL,
    title                   VARCHAR(255) NOT NULL,
    summary                 TEXT,
    objectives              TEXT,
    state                   VARCHAR(32) NOT NULL DEFAULT 'draft',
    -- Lineage. Set on promotion and never overwritten.
    origin_idea_id          UUID,
    responsible_person_id   UUID REFERENCES people (id) ON DELETE SET NULL,
    started_at              TIMESTAMPTZ,
    completed_at            TIMESTAMPTZ,
    created_by_id           UUID REFERENCES people (id) ON DELETE SET NULL,
    updated_by_id           UUID REFERENCES people (id) ON DELETE SET NULL,
    created_at              TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at              TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT uq_projects_org_code UNIQUE (organisation_id, code),
    CONSTRAINT ck_projects_state
        CHECK (state IN ('draft', 'active', 'on_hold', 'completed', 'archived'))
);

CREATE TABLE ideas (
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    workspace_id        UUID NOT NULL UNIQUE
                             REFERENCES research_workspaces (id) ON DELETE RESTRICT,
    title               VARCHAR(255) NOT NULL,
    summary             TEXT,
    research_question   TEXT,
    hypothesis          TEXT,
    motivation          TEXT,
    keywords            TEXT[] NOT NULL DEFAULT '{}',
    state               VARCHAR(32) NOT NULL DEFAULT 'discovery',
    -- Why an idea was rejected or archived. Institutional memory, not noise.
    outcome_note        TEXT,
    promoted_project_id UUID REFERENCES projects (id) ON DELETE SET NULL,
    created_by_id       UUID REFERENCES people (id) ON DELETE SET NULL,
    updated_by_id       UUID REFERENCES people (id) ON DELETE SET NULL,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT ck_ideas_state CHECK (state IN (
        'discovery', 'exploration', 'concept', 'review',
        'project_candidate', 'promoted', 'rejected', 'archived'
    )),
    -- A promoted idea must name its project, and only a promoted idea may.
    CONSTRAINT ck_ideas_promotion_consistency CHECK (
        (state = 'promoted' AND promoted_project_id IS NOT NULL)
        OR (state <> 'promoted' AND promoted_project_id IS NULL)
    ),
    -- Closing an idea requires a recorded reason.
    CONSTRAINT ck_ideas_closure_has_reason CHECK (
        state NOT IN ('rejected', 'archived') OR outcome_note IS NOT NULL
    )
);

ALTER TABLE projects
    ADD CONSTRAINT fk_projects_origin_idea
    FOREIGN KEY (origin_idea_id) REFERENCES ideas (id) ON DELETE SET NULL;

CREATE INDEX ix_projects_origin_idea ON projects (origin_idea_id);
