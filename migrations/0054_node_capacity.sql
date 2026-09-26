-- Ocinye OS — node capacity: physical, reserved, allocatable, allocated, consumed.
--
-- A node already reported what it has (cpu_cores, memory_bytes, storage_bytes,
-- gpus). What was missing is the rest of the model an Instance needs to govern
-- it (Part 5 of the generalization programme):
--
--   physical     what the node reports it has                  (existing)
--   reserved     what the operator holds back for the host      (new, set by an admin)
--   allocatable  physical − reserved                            (derived)
--   allocated    what jobs hold                                 (derived; 0 until job dispatch exists)
--   consumed     what the node reports in use                   (new, reported)
--
-- Reserved is the operator's decision; consumed is the node's report, and like
-- everything a node reports it is untrusted input used for operators and never
-- for an authorization decision.

ALTER TABLE compute_nodes
    ADD COLUMN reserved_cpu_cores     INTEGER NOT NULL DEFAULT 0,
    ADD COLUMN reserved_memory_bytes  BIGINT  NOT NULL DEFAULT 0,
    ADD COLUMN reserved_storage_bytes BIGINT  NOT NULL DEFAULT 0,
    ADD COLUMN memory_used_bytes      BIGINT,
    ADD COLUMN storage_used_bytes     BIGINT,
    ADD CONSTRAINT ck_compute_nodes_reserved_non_negative
        CHECK (reserved_cpu_cores >= 0 AND reserved_memory_bytes >= 0
               AND reserved_storage_bytes >= 0);
