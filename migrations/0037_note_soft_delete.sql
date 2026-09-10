-- Lixo das notas: apagar é reversível (ADR-0413 §7).
--
-- Apagar uma nota não a destrói: leva-a ao Lixo. `deleted_at` marca quando, e
-- `deleted_by_id` quem — para a história de quem mexeu, como as revisões. Do
-- Lixo, uma nota restaura-se (limpa `deleted_at`) ou elimina-se definitivamente
-- (aí sim, a linha desaparece, e as revisões e partilhas com ela, por CASCADE).
--
-- A preservação de dados tem prioridade (`CLAUDE.md` §58): o caminho normal é
-- soft delete; a eliminação definitiva é um segundo passo deliberado, a partir
-- do Lixo.
--
-- Aditivo: `deleted_at` nasce NULL em todas as linhas — uma nota viva é uma nota
-- com `deleted_at IS NULL`, que é o que toda a leitura passa a exigir.

ALTER TABLE notes
    ADD COLUMN deleted_at    TIMESTAMPTZ,
    ADD COLUMN deleted_by_id UUID REFERENCES people (id) ON DELETE SET NULL;

-- O Lixo de uma pessoa: as suas notas apagadas, da mais recente para trás. O
-- índice parcial serve esta listagem sem pesar nas notas vivas.
CREATE INDEX ix_notes_deleted ON notes (owner_id, deleted_at)
    WHERE deleted_at IS NOT NULL;
