-- The resource profile a member is assigned.
--
-- Every member resolves an entitlement from a profile (ADR-0108). This column
-- names the one they are assigned; NULL means "the organisation's default",
-- which a member resolves from day one without materialising anything. An
-- administrator may assign a different profile explicitly (a later slice), and
-- that is what sets this column away from NULL.
--
-- ON DELETE SET NULL, not RESTRICT: retiring a profile falls its members back to
-- the default rather than blocking the profile's removal — an entitlement is
-- never lost by a profile going away.
ALTER TABLE people
    ADD COLUMN resource_profile_id UUID REFERENCES resource_profiles (id) ON DELETE SET NULL;

COMMENT ON COLUMN people.resource_profile_id IS
    'The resource allocation profile assigned to this member; NULL = the organisation default (ADR-0108).';

CREATE INDEX ix_people_resource_profile
    ON people (resource_profile_id) WHERE resource_profile_id IS NOT NULL;
