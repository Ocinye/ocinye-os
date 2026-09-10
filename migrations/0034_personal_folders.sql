-- Pastas de uma pessoa, para arrumar as suas notas.
--
-- # A mesma primitiva, com dono
--
-- Uma pasta pessoal é uma pasta — identidade, nome, e nada que decida
-- autoridade (ADR-0204). Em vez de duplicar a primitiva, os `folders` passam a
-- servir dois donos, como as `notes` e os `files` já fazem: uma pasta é de um
-- ambiente **ou** de uma pessoa, nunca de ninguém, e um `CHECK` impõe-o. As
-- pastas de ambiente que já existem têm ambiente e satisfazem-no.
--
-- As pastas pessoais são **planas** nesta fatia: sem aninhamento. O esquema
-- mantém o `parent_id` para as pastas de ambiente; uma pasta pessoal deixa-o
-- nulo, e o aninhamento pessoal, se for preciso, é uma fatia posterior.

ALTER TABLE folders
    ALTER COLUMN workspace_id DROP NOT NULL,
    ADD COLUMN owner_id UUID REFERENCES people (id) ON DELETE CASCADE;

ALTER TABLE folders
    ADD CONSTRAINT ck_folders_owner_or_workspace
        CHECK (owner_id IS NOT NULL OR workspace_id IS NOT NULL);

-- A unicidade de nome entre irmãos por ambiente passa a aplicar-se **só** às
-- pastas de ambiente. Sem isto, duas pessoas não podiam ter uma pasta com o
-- mesmo nome: todas as pastas pessoais partilham a chave de `workspace_id` nulo,
-- e colidiriam entre donos.
DROP INDEX uq_folders_sibling_name;
CREATE UNIQUE INDEX uq_folders_sibling_name
    ON folders (workspace_id, COALESCE(parent_id, '00000000-0000-0000-0000-000000000000'::uuid), lower(name))
    WHERE workspace_id IS NOT NULL;

-- E a unicidade das pastas pessoais, por dono. Planas: sem `parent_id`.
CREATE UNIQUE INDEX uq_folders_owner_name
    ON folders (owner_id, lower(name))
    WHERE owner_id IS NOT NULL;

-- A nota aponta para a pasta que a arruma. `ON DELETE SET NULL`: apagar a pasta
-- não apaga a nota — desarruma-a. A posse (a pasta é do mesmo dono que a nota) é
-- validada no serviço, como a referência de imagem.
ALTER TABLE notes ADD COLUMN folder_id UUID REFERENCES folders (id) ON DELETE SET NULL;
CREATE INDEX ix_notes_folder ON notes (owner_id, folder_id) WHERE owner_id IS NOT NULL;
