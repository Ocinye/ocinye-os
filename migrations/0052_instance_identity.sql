-- Ocinye OS — the Instance becomes explicit.
--
-- An Ocinye Instance is one independently governed Ocinye OS environment. The
-- domain has always been bounded by `organisations`: every institutional row
-- carries `organisation_id`, and authorization refuses to cross it. What was
-- missing is the statement of *which* organisation this installation serves.
-- Until now it came from an environment variable that defaulted to the slug
-- `ocinye` — the product assuming its first customer.
--
-- The default deployment model stays one Instance per installation; this is
-- not multi-tenancy (ADR-0013). The table is a singleton: at most one row, ever.
--
-- `id` is the Instance's durable identity. It is not the organisation's id and
-- not the server's: it is what stays the same when the Instance moves from one
-- host to another (ADR-0700).

CREATE TABLE instance_identity (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    organisation_id UUID NOT NULL UNIQUE REFERENCES organisations (id),
    singleton       BOOLEAN NOT NULL DEFAULT TRUE,
    recorded_at     TIMESTAMPTZ NOT NULL DEFAULT now(),

    CONSTRAINT ck_instance_identity_singleton CHECK (singleton),
    CONSTRAINT uq_instance_identity_singleton UNIQUE (singleton)
);

COMMENT ON TABLE instance_identity IS
    'The one Instance this installation serves. Singleton; id is the Instance identity that survives a move between hosts.';

-- ── The existing installation maps to the first Instance ────────────────
--
-- Lossless and unambiguous only when there is exactly one organisation. A
-- database with several (a development database full of test fixtures) is left
-- unrecorded: the Core then resolves the Instance from explicit configuration,
-- and refuses to guess.
INSERT INTO instance_identity (organisation_id)
SELECT id FROM organisations
 WHERE (SELECT count(*) FROM organisations) = 1;

-- ── The bootstrap gave organisations their slug as their name ───────────
--
-- `bootstrap_organisation(pool, slug, slug, …)`: every Instance created so far
-- is called by its URL-safe identifier. The name is what people read — in the
-- TOTP app, the mail signature, the top bar. Only rows still carrying that
-- defect change; a name someone chose is never touched.
UPDATE organisations
   SET name = initcap(replace(slug, '-', ' ')),
       updated_at = now()
 WHERE name = slug;
