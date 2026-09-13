-- The resource usage ledger is append-only (ADR-0108): its rows are never
-- updated or deleted, enforced by database triggers. That invariant conflicts
-- with the `ON DELETE SET NULL` foreign keys the ledger's context columns were
-- given: deleting a referenced row (a model, a node, a person) fires an UPDATE
-- on the ledger to null the reference, and the append-only trigger refuses it —
-- so the referenced row can no longer be deleted at all.
--
-- In production this bites immediately: `ai_models` is deleted and reinserted on
-- every node report, so the first recorded AI usage would wedge the next report.
--
-- The correct model for an immutable ledger is that its context ids are
-- **provenance**: a usage event records that an access used model X on node Y,
-- and that historical fact must survive X and Y being removed, not be mutated
-- when they are. So these references become plain provenance UUIDs — the value
-- stays, the row is never touched, and deleting a model or a node no longer
-- collides with the ledger's immutability.
--
-- `organisation_id` keeps its `ON DELETE CASCADE`: deleting an organisation is a
-- full teardown, not a routine operation, and is out of scope here.

ALTER TABLE resource_usage_events
    DROP CONSTRAINT IF EXISTS resource_usage_events_actor_person_id_fkey,
    DROP CONSTRAINT IF EXISTS resource_usage_events_unit_id_fkey,
    DROP CONSTRAINT IF EXISTS resource_usage_events_workspace_id_fkey,
    DROP CONSTRAINT IF EXISTS resource_usage_events_project_id_fkey,
    DROP CONSTRAINT IF EXISTS resource_usage_events_compute_node_id_fkey,
    DROP CONSTRAINT IF EXISTS resource_usage_events_model_id_fkey;
