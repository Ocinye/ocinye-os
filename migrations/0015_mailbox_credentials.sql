-- ── Credenciais de caixa ────────────────────────────────────────────────
--
-- Uma pessoa liga a sua própria caixa, e a senha dela fica aqui, cifrada
-- (ADR-0409).
--
-- # Porque é uma tabela e não colunas em `mailboxes`
--
-- Porque `mailboxes` é lida em todos os ecrãs de correio, e um segredo numa
-- linha lida constantemente é um segredo que atravessa constantemente o
-- processo. Aqui é lido num sítio só: quando se abre uma sessão.
--
-- # Porque o nonce vive ao lado do criptograma
--
-- Porque separados não servem para nada, e guardá-los em sítios diferentes é a
-- maneira de um deles se perder. Não é secreto — é único, e é isso que se lhe
-- pede.
--
-- A chave **não está aqui**. Vive na configuração da instalação. Quem obtiver um
-- despejo desta tabela obtém criptogramas, e não senhas.

CREATE TABLE mailbox_credentials (
    mailbox_id      UUID PRIMARY KEY REFERENCES mailboxes (id) ON DELETE CASCADE,

    -- Com que nome esta caixa se autentica. Quase sempre o endereço, mas não
    -- necessariamente: há serviços que separam a conta do endereço.
    username        VARCHAR(320) NOT NULL,

    -- O nonce, único por registo. Doze bytes, o que o ChaCha20-Poly1305 pede.
    nonce           BYTEA NOT NULL,

    -- O criptograma, com a etiqueta de autenticação incluída.
    ciphertext      BYTEA NOT NULL,

    -- Quem a ligou. Não é o dono da caixa por definição: uma caixa partilhada é
    -- ligada por quem tem autoridade sobre ela, e saber quem foi é o que permite
    -- perguntar-lhe quando ela deixar de funcionar.
    connected_by    UUID NOT NULL REFERENCES people (id),

    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now(),

    -- Um nonce de doze bytes, e não outro tamanho. A base recusa o que o
    -- algoritmo não aceita, em vez de deixar a falha para o momento de abrir.
    CONSTRAINT ck_mailbox_credentials_nonce CHECK (octet_length(nonce) = 12),

    -- Um criptograma vazio é uma senha que não existe. Escrever um seria
    -- registar uma caixa como ligada sem ter com que a abrir.
    CONSTRAINT ck_mailbox_credentials_ciphertext CHECK (octet_length(ciphertext) > 0)
);

CREATE INDEX ix_mailbox_credentials_connected_by
    ON mailbox_credentials (connected_by);
