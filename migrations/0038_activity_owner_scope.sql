-- Actividade de uma nota pessoal (ADR-0413 §7): quem fez o quê, e quando.
--
-- A `activity_entries` nasceu ligada a um Research Workspace (`workspace_id NOT
-- NULL`). Uma nota pessoal não tem workspace — tem **dono** —, e por isso a
-- tabela ganha a mesma dimensão de dono que as notas, as pastas e a pesquisa já
-- ganharam: uma linha pertence a um ambiente **ou** a uma pessoa, nunca a
-- nenhum. Aditivo; as linhas existentes continuam workspace-scoped.
--
-- E acrescentam-se os verbos que faltavam para a vida de uma nota partilhada:
-- partilhar, revogar, apagar, restaurar.

ALTER TABLE activity_entries
    ADD COLUMN owner_id UUID REFERENCES people (id) ON DELETE CASCADE;

ALTER TABLE activity_entries
    ALTER COLUMN workspace_id DROP NOT NULL;

ALTER TABLE activity_entries
    ADD CONSTRAINT ck_activity_entries_scope
        CHECK (owner_id IS NOT NULL OR workspace_id IS NOT NULL);

-- Widen the verb vocabulary. Drop and re-add — um CHECK não se estende no sítio.
ALTER TABLE activity_entries DROP CONSTRAINT ck_activity_entries_kind;
ALTER TABLE activity_entries
    ADD CONSTRAINT ck_activity_entries_kind CHECK (kind IN (
        'created', 'updated', 'state_changed', 'commented',
        'member_added', 'attached', 'published',
        'shared', 'revoked', 'deleted', 'restored'
    ));

-- A actividade de uma pessoa, do mais recente para trás. Índice parcial, para
-- não pesar nas linhas workspace-scoped.
CREATE INDEX ix_activity_entries_owner_time
    ON activity_entries (owner_id, created_at DESC)
    WHERE owner_id IS NOT NULL;
