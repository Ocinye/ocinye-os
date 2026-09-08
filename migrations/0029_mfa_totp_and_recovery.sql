-- MFA obrigatório para identidades privilegiadas: seed TOTP e códigos de
-- recuperação, e o estado de sessão que os exige (ADR-0107).
--
-- # As três coisas que esta migração acrescenta
--
-- Uma palavra-passe, por si só, deixa de estabelecer autoridade privilegiada.
-- Depois de a palavra-passe ser aceite, uma identidade com MFA obrigatório fica
-- numa sessão `mfa_required` — que **não** permite trabalho ordinário — até
-- provar o segundo factor. O portão vive na fronteira central; esta migração só
-- lhe dá o estado onde assentar.
--
-- O **seed TOTP** é selado, não resumido: tem de ser recuperável para recalcular
-- o código. Sela-se com a subchave `mfa-totp` derivada da raiz institucional de
-- selagem (`OCINYE_SEALING_KEY`), nunca com a raiz directa nem com a subchave do
-- correio (ADR-0107). `confirmed_at` fica nulo até um código válido provar o
-- enrolamento — um seed por confirmar não satisfaz nenhum desafio.
--
-- Os **códigos de recuperação** são o oposto do seed: guarda-se só o verificador
-- Argon2id, nunca o código, nunca cifrado para leitura posterior. Cada um é de
-- uso único, e o consumo é atómico (um UPDATE condicional no estado), para que
-- dois pedidos concorrentes não gastem o mesmo.

-- ---------------------------------------------------------------------------
-- 1. O estado de sessão do portão de MFA
-- ---------------------------------------------------------------------------
-- Entre a palavra-passe aceite e o segundo factor satisfeito. Como
-- `password_change_required`, não permite trabalho ordinário — e, como ele, uma
-- sessão nunca se transiciona no lugar: satisfazer o factor revoga esta e cria
-- uma `active`.
ALTER TABLE sessions DROP CONSTRAINT ck_sessions_state;
ALTER TABLE sessions ADD CONSTRAINT ck_sessions_state
    CHECK (state IN ('password_change_required', 'mfa_required', 'active', 'revoked'));

-- ---------------------------------------------------------------------------
-- 2. O seed TOTP, selado, um por pessoa
-- ---------------------------------------------------------------------------
CREATE TABLE mfa_totp_secrets (
    person_id     UUID PRIMARY KEY REFERENCES people (id) ON DELETE CASCADE,

    -- O criptograma e o nonce com que o seed foi selado. A raiz não está na
    -- base: sem ela, estas linhas chegam íntegras e ilegíveis (ADR-0107). O
    -- `ciphertext` leva à frente o byte de esquema do formato selado, por isso
    -- é sempre maior que o corpo cifrado — a verificação de comprimento não
    -- muda com isso.
    nonce         BYTEA NOT NULL,
    ciphertext    BYTEA NOT NULL,

    -- Nulo até o enrolamento ser confirmado por um código válido. Um seed por
    -- confirmar existe, mas não autentica: enrolar é provar que o autenticador
    -- do membro já gera o código certo antes de a porta fechar atrás dele.
    confirmed_at  TIMESTAMPTZ,

    created_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at    TIMESTAMPTZ NOT NULL DEFAULT now(),

    CONSTRAINT ck_mfa_totp_nonce  CHECK (octet_length(nonce) = 12),
    CONSTRAINT ck_mfa_totp_cipher CHECK (octet_length(ciphertext) > 0)
);

COMMENT ON TABLE mfa_totp_secrets IS
    'O seed TOTP de cada identidade com MFA, selado com a subchave mfa-totp '
    'derivada de OCINYE_SEALING_KEY. Recuperável por desenho: um verificador '
    'não serve, porque o código tem de ser recalculado a cada 30 segundos.';

-- ---------------------------------------------------------------------------
-- 3. Códigos de recuperação: verificadores, uso único
-- ---------------------------------------------------------------------------
CREATE TABLE mfa_recovery_codes (
    id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    person_id    UUID NOT NULL REFERENCES people (id) ON DELETE CASCADE,

    -- Só o verificador Argon2id, no formato PHC. Nunca o código, nunca cifrado:
    -- um código de recuperação prova-se, não se lê de volta.
    verifier     TEXT NOT NULL,

    state        VARCHAR(16) NOT NULL DEFAULT 'active',
    consumed_at  TIMESTAMPTZ,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),

    CONSTRAINT ck_mfa_recovery_verifier CHECK (verifier LIKE '$argon2id$%'),
    CONSTRAINT ck_mfa_recovery_state CHECK (state IN ('active', 'consumed')),
    CONSTRAINT ck_mfa_recovery_consumed_agrees
        CHECK ((state = 'consumed') = (consumed_at IS NOT NULL))
);

-- Os códigos vivos de uma pessoa: o que o consumo atómico percorre.
CREATE INDEX ix_mfa_recovery_live
    ON mfa_recovery_codes (person_id) WHERE state = 'active';

COMMENT ON TABLE mfa_recovery_codes IS
    'Verificadores Argon2id de códigos de recuperação de uso único. Regenerar '
    'invalida os anteriores; consumir é atómico.';
