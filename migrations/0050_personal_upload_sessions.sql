-- Sessões de carregamento em partes para o espaço pessoal.
--
-- Até aqui uma sessão de carregamento pertencia sempre a um research workspace
-- (`workspace_id NOT NULL`). Um ficheiro **pessoal** grande — acima do tecto que
-- a borda impõe a um único pedido — precisa exactamente do mesmo caminho por
-- partes, mas o seu destino é o **dono**, não um ambiente.
--
-- Torna-se o `workspace_id` opcional e acrescenta-se o `owner_id`. Exactamente um
-- dos dois governa cada sessão, e a base recusa qualquer linha que não escolha um
-- — uma sessão sem destino, ou com dois, não é uma sessão. As linhas que já
-- existem têm `workspace_id` e `owner_id` nulo, e passam a verificação tal como
-- estão.

ALTER TABLE upload_sessions
    ALTER COLUMN workspace_id DROP NOT NULL,
    ADD COLUMN owner_id UUID REFERENCES people(id) ON DELETE CASCADE;

ALTER TABLE upload_sessions
    ADD CONSTRAINT ck_upload_sessions_target
        CHECK ((workspace_id IS NULL) <> (owner_id IS NULL));
