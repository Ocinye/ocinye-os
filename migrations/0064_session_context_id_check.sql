-- ---------------------------------------------------------------------------
-- A001 · o par (tipo, identificador) do contexto da sessão, sem buraco
-- ---------------------------------------------------------------------------
--
-- A restrição da 0063 comparava `(kind IN ('unit','project')) = (id IS NOT
-- NULL)`. Com `kind` nulo o lado esquerdo é NULL, a comparação é NULL, e um
-- CHECK que dá NULL passa: uma sessão sem contexto podia guardar um
-- identificador órfão (A001-L006). A regra é a mesma; só deixa de ter a
-- excepção que ninguém quis.
--
-- Nenhuma linha válida muda: o Core escreve sempre os dois juntos.

ALTER TABLE sessions DROP CONSTRAINT ck_sessions_context_id;

ALTER TABLE sessions
    ADD CONSTRAINT ck_sessions_context_id
        CHECK (COALESCE(active_context_kind IN ('unit', 'project'), false)
               = (active_context_id IS NOT NULL));
