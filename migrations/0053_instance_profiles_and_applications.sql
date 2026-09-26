-- Ocinye OS — Instance profiles and per-instance application activation.
--
-- A profile decides defaults — which optional applications start active,
-- whether the initial units are seeded — and never authority (ADR-0014). It is
-- an attribute of the Instance, and the Instance is its governing organisation
-- (ADR-0013), so it lives on `organisations`.
--
-- The column default is `research`: it is the profile of the installation that
-- existed before profiles, and of rows written outside the product path (test
-- fixtures). The product path — creating an Instance — always names the profile
-- explicitly, and refuses to guess one.

ALTER TABLE organisations
    ADD COLUMN profile VARCHAR(16) NOT NULL DEFAULT 'research',
    ADD CONSTRAINT ck_organisations_profile
        CHECK (profile IN ('research', 'business', 'education', 'personal'));

-- An explicit decision, per Instance, about one optional application. No row
-- means «whatever the profile says». Essential applications never get a row:
-- the Core refuses to record one, and the application catalogue is in code
-- (`ocinye_contracts::ApplicationId`), so the identifier is validated there.
CREATE TABLE instance_applications (
    organisation_id UUID NOT NULL REFERENCES organisations (id) ON DELETE CASCADE,
    application_id  VARCHAR(32) NOT NULL,
    active          BOOLEAN NOT NULL,
    updated_by_id   UUID REFERENCES people (id) ON DELETE SET NULL,
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now(),

    PRIMARY KEY (organisation_id, application_id)
);

COMMENT ON TABLE instance_applications IS
    'Explicit per-instance activation of optional applications. No row: the profile decides (ADR-0014).';
