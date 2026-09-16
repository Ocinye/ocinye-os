-- Uma identidade surrogate para `file_favourites`, para a continuidade a poder
-- enumerar.
--
-- # Porque isto existe
--
-- A `file_favourites` nasceu (migração 0047) com chave composta `(person_id,
-- file_id)` — a marca é o par, e é a semântica certa para o upsert. Mas é uma
-- tabela que **viaja** (`Comparacao::Identidades`), e o manifesto de continuidade
-- enumera cada tabela que viaja por `SELECT id FROM {tabela} ORDER BY id`. Sem
-- uma coluna `id`, o `snapshot` — e com ele o `institutional-backup` e o
-- `verify-snapshot` — falhava com «column "id" does not exist». Toda a
-- convenção do esquema é que uma tabela que viaja tem um `id UUID`; a 0047 foi a
-- única a quebrá-la, e produção deixou de conseguir produzir uma cópia
-- verificável.
--
-- # O que muda, e o que não muda
--
-- Acrescenta-se `id`, com um valor por linha estável e único. A chave primária
-- continua a ser `(person_id, file_id)`: o upsert e a posse não mudam, e a marca
-- continua a desaparecer com a pessoa ou com o ficheiro. O `id` serve a
-- enumeração da continuidade, e viaja com a linha — pelo que o conjunto de
-- identidades da origem reaparece igual no restauro.

ALTER TABLE file_favourites
    ADD COLUMN id UUID NOT NULL DEFAULT gen_random_uuid();

-- Único, para a enumeração ser determinística e para o `id` ser uma identidade
-- a sério, e não um rótulo repetível.
ALTER TABLE file_favourites
    ADD CONSTRAINT uq_file_favourites_id UNIQUE (id);
