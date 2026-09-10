-- Notas pessoais: uma nota passa a poder pertencer a um membro, e não só a um
-- Research Workspace.
--
-- # Porque a mesma tabela, e não uma nova
--
-- O módulo de Notas é do foro pessoal — o dono é o membro —, mas as `notes` que
-- já existem são artefactos de um Research Workspace. Duas tabelas de notas
-- duplicariam a primitiva e as suas revisões. Em vez disso, a `notes` serve os
-- dois casos sem os confundir (ADR-0413): ganha um dono, e o ambiente deixa de
-- ser obrigatório.
--
-- # A invariante que a substitui
--
-- Uma nota é de alguém **ou** de um ambiente — nunca de ninguém. O `CHECK`
-- impõe-o: `owner_id` ou `workspace_id`, pelo menos um. As notas que já existem
-- têm ambiente e satisfazem-no; as pessoais nascem com dono e sem ambiente.
--
-- A autoridade não vem daqui. Quem lê uma nota continua a ser decidido pela
-- classificação e pela política — o dono, uma partilha (mais tarde), ou a
-- pertença ao ambiente —, e o `PlatformAdmin` não ganha leitura por ser
-- administrador.

ALTER TABLE notes
    ALTER COLUMN unit_id DROP NOT NULL,
    ALTER COLUMN workspace_id DROP NOT NULL,
    ADD COLUMN owner_id UUID REFERENCES people (id) ON DELETE CASCADE;

ALTER TABLE notes
    ADD CONSTRAINT ck_notes_owner_or_workspace
        CHECK (owner_id IS NOT NULL OR workspace_id IS NOT NULL);

-- Listar as notas de um membro é a consulta central do módulo pessoal.
CREATE INDEX ix_notes_owner ON notes (owner_id) WHERE owner_id IS NOT NULL;
