-- O índice de pesquisa ganha um dono, para as notas pessoais entrarem sem fugir.
--
-- # Porque faltava
--
-- Até aqui, a visibilidade de uma linha indexada decidia-se por `(unit_id,
-- workspace_id, classification)` — o modelo institucional. Uma nota pessoal é
-- `INTERNAL` e sem ambiente (ADR-0413), pelo que, indexada como está, a cláusula
-- de `INTERNAL` mostrá-la-ia a **toda** a organização. Foi por isto que a fatia
-- A não indexou as notas pessoais: faltava a dimensão do dono.
--
-- Agora uma linha pode ser de uma **pessoa**. Uma linha com dono é visível só ao
-- dono; as cláusulas de classificação e de filiação nunca lhe tocam (o predicado
-- guarda-as com `owner_id IS NULL`). As linhas institucionais que já existem têm
-- `owner_id` nulo e continuam a decidir-se exactamente como antes.

ALTER TABLE search_documents
    ADD COLUMN owner_id UUID REFERENCES people (id) ON DELETE CASCADE;

-- A pesquisa de um membro nas suas notas passa por aqui.
CREATE INDEX ix_search_documents_owner ON search_documents (owner_id) WHERE owner_id IS NOT NULL;
