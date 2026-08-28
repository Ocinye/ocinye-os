-- O endereço institucional passa a ser a credencial única (ADR-0106).
--
-- O username sai. Não como campo escondido, não como alternativa aceite em
-- silêncio, e não como coluna reservada para o caso de: uma coluna que ninguém
-- escreve é uma coluna que alguém acaba por escrever, e a partir daí há duas
-- maneiras de entrar outra vez sem que nada o tenha decidido.
--
-- # O que **não** sai
--
-- `mailbox_credentials.username` fica. É o nome de utilizador com que uma caixa
-- se autentica noutro sistema, e quem o define não é o Ocinye (ADR-0409). Ter o
-- mesmo nome de coluna é coincidência, e confundi-los partiria o correio.

-- ── As pessoas ──────────────────────────────────────────────────────────

-- O endereço já era único por organização? Garante-se, porque passa a ser a
-- credencial: dois endereços iguais seriam duas contas a responder ao mesmo
-- pedido de autenticação.
CREATE UNIQUE INDEX IF NOT EXISTS uq_people_email_lower
    ON people (organisation_id, lower(email));

DROP INDEX IF EXISTS uq_people_username_lower;
ALTER TABLE people DROP CONSTRAINT IF EXISTS ck_people_username_shape;
ALTER TABLE people DROP COLUMN IF EXISTS username;

COMMENT ON COLUMN people.email IS
    'O endereço institucional. É a identidade e a credencial de autenticação '
    '(ADR-0106). O identificador estável continua a ser `id`: mudar de endereço '
    'não cria uma conta nova.';

-- ── As tentativas ───────────────────────────────────────────────────────

-- A limitação de tentativas contava por username. Passa a contar por endereço,
-- pela mesma razão e com a mesma forma: travar tentativas repetidas contra uma
-- conta, e não apenas contra um endereço de rede.
--
-- Renomear em vez de apagar e criar: o histórico de tentativas continua a
-- valer, e o que lá está guardado é o que foi apresentado na altura.
ALTER TABLE authentication_attempts RENAME COLUMN username TO email;

DROP INDEX IF EXISTS ix_authentication_attempts_username;
CREATE INDEX ix_authentication_attempts_email
    ON authentication_attempts (lower(email), attempted_at DESC);

COMMENT ON COLUMN authentication_attempts.email IS
    'O endereço tal como foi apresentado, em minúsculas. Guardado porque a '
    'limitação por conta precisa de saber contra qual conta se tentou.';
