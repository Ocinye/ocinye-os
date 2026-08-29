-- O ficheiro passa a ser o recurso governado.
--
-- # A pergunta que a auditoria fechou
--
-- Quem decide hoje se alguém pode ler um documento? A resposta, medida e não
-- suposta: `documents.classification` combinada com a do ambiente que o
-- contém, pela mais restritiva das duas. E `storage_objects.classification`
-- **não participa** — é escrita e nunca lida para decidir. Zero ocorrências
-- em qualquer consulta de autorização.
--
-- Portanto a mudança é de representante, e não de política:
--
--     antes   Document + ResearchWorkspace
--     depois  File     + ResearchWorkspace
--
-- com a mesma composição por `most_restrictive`, e com
-- `classification_allows_read` a decidir com exactamente os mesmos papéis. Uma
-- migração que aproveitasse a passagem para «melhorar» a política tornaria
-- impossível saber se uma diferença de comportamento veio da estrutura ou da
-- regra.
--
-- # Porque a classificação entra agora, e não na 0020
--
-- Porque na 0020 o ficheiro ainda não governava nada, e uma terceira coluna de
-- classificação sem autoridade seria uma terceira fonte a poder discordar das
-- outras duas. Agora entra porque vai passar a ser **a** fonte.
--
-- O valor sai de `documents.classification`, e de mais lado nenhum. Não se
-- deduz do MIME, nem do objecto guardado, nem do nome: um relatório não é
-- confidencial por ser PDF.

ALTER TABLE files ADD COLUMN classification VARCHAR(32);

UPDATE files f
   SET classification = d.classification
  FROM documents d
 WHERE d.file_id = f.id
   AND f.classification IS NULL;

-- Um ficheiro que não venha de documento nenhum ainda não existe — a operação
-- genérica é desta mesma milestone —, mas se existisse, o valor seguro é o mais
-- restritivo por omissão e não o mais permissivo.
UPDATE files SET classification = 'INTERNAL' WHERE classification IS NULL;

ALTER TABLE files ALTER COLUMN classification SET NOT NULL;
ALTER TABLE files ADD CONSTRAINT ck_files_classification
    CHECK (classification IN ('PUBLIC', 'INTERNAL', 'CONFIDENTIAL', 'RESTRICTED'));

-- ── As duas raízes de contexto não se podem contradizer ─────────────────
--
-- `files` guarda `unit_id` e `workspace_id`, e a autorização usa os dois para
-- resolver papéis. Um ambiente pertence a exactamente uma unidade — a coluna é
-- `NOT NULL` —, pelo que a unidade do ficheiro é uma desnormalização da do
-- ambiente.
--
-- Hoje nenhum ficheiro as contradiz, porque ambas vieram do mesmo sítio. Mas
-- «ninguém escreveu isto errado até agora» não é uma invariante: bastaria uma
-- consulta de manutenção para ficar um ficheiro cuja unidade diz A e cujo
-- ambiente pertence a B — duas raízes de autorização a discordar, e nada a
-- acusar.
--
-- A chave estrangeira composta torna esse estado **impossível**, e não apenas
-- improvável.
ALTER TABLE research_workspaces ADD CONSTRAINT uq_research_workspaces_id_unit
    UNIQUE (id, unit_id);

ALTER TABLE files ADD CONSTRAINT fk_files_workspace_unit
    FOREIGN KEY (workspace_id, unit_id)
    REFERENCES research_workspaces (id, unit_id) ON DELETE CASCADE;
