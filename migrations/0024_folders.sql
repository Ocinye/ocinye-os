-- Pastas: organização humana, sem autoridade nenhuma.
--
-- # O que uma pasta é, e o que não é
--
-- É uma estrutura de navegação dentro de um contentor de autoridade. Ajuda uma
-- pessoa a encontrar o que procura, e mais nada.
--
-- **Não tem classificação, nem grants, nem herança de segurança.** Um ficheiro
-- `RESTRICTED` arrastado para uma pasta chamada «Público» continua
-- `RESTRICTED`, e um ficheiro `INTERNAL` numa pasta chamada «Confidencial»
-- continua `INTERNAL`. O nome de uma pasta é uma etiqueta escrita por alguém;
-- a protecção de um artefacto é uma decisão institucional.
--
-- > **A folder is a navigation structure inside an authority container; moving
-- > a File between authority containers is not a folder operation.**
--
-- Dar classificação às pastas seria criar uma segunda autoridade que discordaria
-- da primeira no dia em que alguém arrastasse alguma coisa. Já vimos, nesta
-- mesma milestone, o que custa ter duas fontes da mesma verdade.
--
-- # Porque a pasta vive num ambiente, e não flutua
--
-- Porque a autoridade vive lá. Uma pasta que atravessasse ambientes seria uma
-- forma de mover artefactos entre fronteiras de autorização com um arrasto — e
-- isso não é organizar, é transferir. Se algum dia existir, será uma operação
-- institucional explícita, com a sua própria decisão.

CREATE TABLE folders (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    organisation_id UUID NOT NULL,
    -- O contentor de autoridade. Uma pasta não o atravessa.
    workspace_id    UUID NOT NULL REFERENCES research_workspaces (id) ON DELETE CASCADE,
    -- A pasta que a contém. `NULL` é a raiz do ambiente.
    parent_id       UUID REFERENCES folders (id) ON DELETE RESTRICT,
    name            VARCHAR(255) NOT NULL,
    created_by_id   UUID REFERENCES people (id) ON DELETE SET NULL,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    -- Dois irmãos com o mesmo nome tornam um caminho ambíguo, e um caminho
    -- ambíguo é uma pessoa a abrir a pasta errada.
    --
    -- `COALESCE` porque `NULL` não é igual a `NULL` num índice único, e sem ele
    -- a raiz aceitaria duas pastas com o mesmo nome.
    CONSTRAINT ck_folders_name CHECK (btrim(name) <> ''),
    -- Uma pasta dentro de si própria seria um ciclo de um passo. Os ciclos
    -- maiores não se apanham aqui — apanha-os quem move.
    CONSTRAINT ck_folders_not_self CHECK (parent_id IS NULL OR parent_id <> id)
);

CREATE UNIQUE INDEX uq_folders_sibling_name
    ON folders (workspace_id, COALESCE(parent_id, '00000000-0000-0000-0000-000000000000'::uuid), lower(name));

CREATE INDEX ix_folders_parent ON folders (parent_id);

-- ── Onde o ficheiro está, para quem o procura ───────────────────────────
--
-- `NULL` é a raiz do ambiente, e é onde tudo o que já existe fica: nenhum
-- ficheiro muda de sítio por esta migration.
--
-- `ON DELETE RESTRICT` porque apagar uma pasta com ficheiros dentro é uma
-- decisão sobre os ficheiros, e não sobre a pasta.
ALTER TABLE files ADD COLUMN folder_id UUID REFERENCES folders (id) ON DELETE RESTRICT;

-- A pasta tem de ser do mesmo ambiente que o ficheiro. Sem isto, um ficheiro
-- poderia aparecer na navegação de um ambiente onde não é governado — e
-- arrastar seria uma forma de atravessar fronteiras de autorização.
ALTER TABLE folders ADD CONSTRAINT uq_folders_id_workspace UNIQUE (id, workspace_id);
ALTER TABLE files ADD CONSTRAINT fk_files_folder_workspace
    FOREIGN KEY (folder_id, workspace_id)
    REFERENCES folders (id, workspace_id) ON DELETE RESTRICT;

CREATE INDEX ix_files_folder ON files (workspace_id, folder_id, lower(name));
