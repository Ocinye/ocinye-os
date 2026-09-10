-- Ficheiros que uma pessoa possui, para as imagens e anexos das notas pessoais.
--
-- # Porque a mesma primitiva, e não uma nova
--
-- Uma imagem colada numa nota é bytes com identidade e uma história de versões
-- imutável — exactamente o que `files`/`file_versions`/`storage_objects` já são
-- (ADR-0204). Duplicar a primitiva para o caso pessoal criaria uma segunda
-- verdade sobre os mesmos bytes. Em vez disso, `files` passa a servir dois donos
-- sem os confundir, tal como `notes` já faz (ADR-0413 §8, emenda 2026-09-10): um
-- ficheiro é de um **ambiente** ou de uma **pessoa**, nunca de ninguém, e o
-- `CHECK` impõe-o. Os ficheiros que já existem têm ambiente e satisfazem-no.
--
-- # O que muda, e o que não muda
--
-- A autoridade de um ficheiro pessoal vem do **dono**, não da composição com um
-- ambiente que não existe: quem lê é o dono, e o `PlatformAdmin` não ganha
-- leitura por ser administrador. Os caminhos de leitura por ambiente filtram por
-- `workspace_id`, pelo que um ficheiro pessoal (ambiente nulo) simplesmente não
-- aparece numa listagem institucional — é invisível ao ecrã de Ficheiros, e é
-- isso que se quer. A classificação de um ficheiro pessoal é a sua própria, sem
-- ambiente por cima; nasce `INTERNAL`, como a nota que a contém.
--
-- `storage_objects` já tinha `unit_id`/`workspace_id` nuláveis; ganha só o dono,
-- para a chave do objecto e para a governação saber de quem é.

ALTER TABLE files
    ALTER COLUMN unit_id DROP NOT NULL,
    ALTER COLUMN workspace_id DROP NOT NULL,
    ADD COLUMN owner_id UUID REFERENCES people (id) ON DELETE CASCADE;

ALTER TABLE files
    ADD CONSTRAINT ck_files_owner_or_workspace
        CHECK (owner_id IS NOT NULL OR workspace_id IS NOT NULL);

ALTER TABLE storage_objects
    ADD COLUMN owner_id UUID REFERENCES people (id) ON DELETE SET NULL;

-- Listar e resolver os ficheiros de uma pessoa é a consulta do módulo pessoal.
CREATE INDEX ix_files_owner ON files (owner_id) WHERE owner_id IS NOT NULL;
