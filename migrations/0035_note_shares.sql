-- Partilha de uma nota pessoal com outro membro, como Viewer ou Editor.
--
-- # A forma segue a das caixas partilhadas
--
-- Uma partilha é uma linha viva que liga uma nota a uma pessoa com um papel
-- (`shared_mailbox_memberships`, migração 0010). Pertencer a uma unidade ou ser
-- administrador **não** dá acesso a uma nota; só uma linha viva aqui dá. O
-- `PlatformAdmin` não lê uma nota por ser administrador (ADR-0413).
--
-- # O que a partilha concede, e o que não concede
--
-- Um `viewer` lê; um `editor` lê e grava o conteúdo. Arrumar em pastas, apagar,
-- e gerir as próprias partilhas continuam a ser do **dono** — a pasta é do dono,
-- e a partilha é a sua prerrogativa. A autoridade de um editor é reavaliada no
-- momento da gravação (ADR-0411): revogar a partilha recusa a gravação seguinte.

CREATE TABLE note_shares (
    id             UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    note_id        UUID NOT NULL REFERENCES notes (id) ON DELETE CASCADE,
    -- A pessoa com quem se partilha. É o `id` da linha `people` dessa pessoa —
    -- uma identidade privilegiada ligada tem o seu próprio `id` e não herda o
    -- que foi partilhado com a humana.
    person_id      UUID NOT NULL REFERENCES people (id) ON DELETE CASCADE,
    role           VARCHAR(16) NOT NULL,
    granted_by_id  UUID NOT NULL REFERENCES people (id) ON DELETE RESTRICT,
    granted_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
    -- Revogar não apaga a linha: guarda quem revogou e quando, para a história.
    revoked_at     TIMESTAMPTZ,
    revoked_by_id  UUID REFERENCES people (id) ON DELETE SET NULL,
    CONSTRAINT ck_note_shares_role CHECK (role IN ('viewer', 'editor')),
    CONSTRAINT ck_note_shares_revocation CHECK ((revoked_at IS NULL) = (revoked_by_id IS NULL))
);

-- Uma partilha viva por (nota, pessoa): partilhar duas vezes muda o papel, não
-- cria uma segunda linha.
CREATE UNIQUE INDEX uq_note_shares_live ON note_shares (note_id, person_id) WHERE revoked_at IS NULL;

-- «Partilhadas comigo»: as notas vivas partilhadas com uma pessoa.
CREATE INDEX ix_note_shares_person ON note_shares (person_id) WHERE revoked_at IS NULL;
