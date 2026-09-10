-- Notas pessoais, com um documento estruturado como fonte de verdade.
--
-- # Porque a mesma tabela, e não uma nova
--
-- O módulo de Notas é do foro pessoal — o dono é o membro —, mas as `notes` que
-- já existem são artefactos de um Research Workspace. Duas tabelas de notas
-- duplicariam a primitiva e as suas revisões. Em vez disso, a `notes` serve os
-- dois casos sem os confundir (ADR-0413): ganha um dono, e o ambiente deixa de
-- ser obrigatório. Uma nota é de alguém **ou** de um ambiente — nunca de
-- ninguém —, e o `CHECK` impõe-o. As notas que já existem têm ambiente e
-- satisfazem-no.
--
-- # Porque um documento estruturado, e não HTML
--
-- A fonte de verdade de uma nota rica passa a ser um **documento estruturado**
-- versionado (`document` em JSONB, com `schema_version`), e não HTML higienizado
-- (ADR-0413 §2, emenda 2026-09-10). O HTML e o texto simples derivam-se dele; o
-- `body` fica a guardar a **projecção de texto simples**, que é o que já vai ao
-- índice de pesquisa. Uma revisão histórica nunca depende da interpretação de
-- HTML arbitrário, e a IA futura endereça uma revisão estruturada exacta.
--
-- As notas antigas ficam com `document` a NULL: são texto simples no `body`, e
-- lêem-se como tal. Nada é reinterpretado em silêncio — quando uma nota antiga
-- for editada no editor novo, nasce-lhe um documento estruturado nessa revisão.
--
-- A autoridade não vem daqui. Quem lê uma nota continua a ser decidido pelo dono,
-- pela classificação e pela política — e o `PlatformAdmin` não ganha leitura por
-- ser administrador.

ALTER TABLE notes
    ALTER COLUMN unit_id DROP NOT NULL,
    ALTER COLUMN workspace_id DROP NOT NULL,
    ADD COLUMN owner_id UUID REFERENCES people (id) ON DELETE CASCADE,
    ADD COLUMN document JSONB,
    ADD COLUMN schema_version INTEGER;

ALTER TABLE notes
    ADD CONSTRAINT ck_notes_owner_or_workspace
        CHECK (owner_id IS NOT NULL OR workspace_id IS NOT NULL),
    -- Um documento estruturado traz sempre a versão do esquema com que foi
    -- escrito, e uma versão sem documento não quer dizer nada.
    ADD CONSTRAINT ck_notes_document_versioned
        CHECK ((document IS NULL) = (schema_version IS NULL));

ALTER TABLE note_revisions
    ADD COLUMN document JSONB,
    ADD COLUMN schema_version INTEGER;

ALTER TABLE note_revisions
    ADD CONSTRAINT ck_note_revisions_document_versioned
        CHECK ((document IS NULL) = (schema_version IS NULL));

-- Listar as notas de um membro é a consulta central do módulo pessoal.
CREATE INDEX ix_notes_owner ON notes (owner_id) WHERE owner_id IS NOT NULL;
