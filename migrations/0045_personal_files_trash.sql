-- Lixo dos ficheiros pessoais: apagar é reversível.
--
-- Um ficheiro pessoal apagado vai para o Lixo, não desaparece. `deleted_at`
-- marca o instante em que lá foi posto; nulo é um ficheiro vivo. Restaurar é
-- pôr `deleted_at` a nulo de novo; apagar definitivamente é outra operação, que
-- remove os bytes e a linha — e essa não passa por aqui.
--
-- Enquanto está no Lixo, o ficheiro continua a ocupar armazenamento: os bytes
-- não foram libertados, e por isso continuam a contar para a quota (ADR-0108,
-- ADR-0207). Só o apagar definitivo os liberta.
--
-- Aditiva: uma coluna nula por omissão. Nenhum ficheiro existente muda de
-- estado — todos nascem vivos, como estavam.

ALTER TABLE files ADD COLUMN deleted_at TIMESTAMPTZ;

-- O Lixo de uma pessoa lê-se por esta via: os seus ficheiros apagados, e só
-- esses. Parcial, porque a esmagadora maioria dos ficheiros está viva e não
-- precisa de entrar no índice.
CREATE INDEX ix_files_owner_deleted
    ON files (owner_id, deleted_at)
    WHERE owner_id IS NOT NULL AND deleted_at IS NOT NULL;
