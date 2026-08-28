-- Ocinye Mensagens — a parte durável.
--
-- O que está aqui é o que tem de sobreviver a tudo: conversas, quem pertence a
-- elas, o que foi dito, a quem se respondeu, quem foi mencionado, quem reagiu e
-- até onde cada pessoa leu.
--
-- O que **não** está aqui, e nunca estará (ADR-0012): presença, `typing`, e o
-- estado das ligações abertas. Isso vive no Redis, com TTL, porque ninguém
-- precisa de saber amanhã quem esteve online ontem — e guardar quem começou a
-- escrever e desistiu seria guardar uma hesitação.

-- ── Conversas ───────────────────────────────────────────────────────────

CREATE TABLE conversations (
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    organisation_id     UUID NOT NULL REFERENCES organisations (id) ON DELETE CASCADE,

    -- 'direct' entre duas pessoas, 'group' entre várias.
    --
    -- A distinção não é cosmética: uma directa não tem nome nem dono, tem
    -- exactamente duas pessoas, e não se entra nem se sai dela.
    kind                VARCHAR(16) NOT NULL,

    -- Só um grupo tem nome. Uma conversa directa apresenta-se pela pessoa do
    -- outro lado, que é quem ela é.
    name                VARCHAR(120),
    topic               VARCHAR(255),

    -- Assinatura das duas identidades de uma conversa directa, em ordem
    -- canónica. É isto que impede duas conversas directas entre as mesmas duas
    -- pessoas: sem uma chave, cada clique em «nova conversa» abriria outra, e
    -- o histórico partia-se em pedaços que ninguém volta a juntar.
    direct_key          TEXT,

    created_by_id       UUID REFERENCES people (id) ON DELETE SET NULL,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT now(),

    CONSTRAINT ck_conversations_kind
        CHECK (kind IN ('direct', 'group')),

    -- Uma directa tem chave e não tem nome; um grupo tem nome e não tem chave.
    -- Escrito como restrição e não como convenção: uma convenção sobrevive até
    -- ao dia em que alguém insere a linha por outro caminho.
    CONSTRAINT ck_conversations_shape CHECK (
        (kind = 'direct' AND direct_key IS NOT NULL AND name IS NULL)
        OR
        (kind = 'group' AND direct_key IS NULL AND name IS NOT NULL)
    )
);

CREATE UNIQUE INDEX uq_conversations_direct ON conversations (direct_key)
    WHERE direct_key IS NOT NULL;

CREATE INDEX ix_conversations_organisation ON conversations (organisation_id);

-- ── Participação ────────────────────────────────────────────────────────

CREATE TABLE conversation_participants (
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    conversation_id     UUID NOT NULL REFERENCES conversations (id) ON DELETE CASCADE,
    person_id           UUID NOT NULL REFERENCES people (id) ON DELETE CASCADE,

    -- O papel **dentro desta conversa**, e em mais lado nenhum.
    --
    -- Um 'owner' de grupo não é um administrador da instituição, não herda
    -- nada do RBAC, e não ganha autoridade fora daqui. São dois vocabulários
    -- que por acaso usam a mesma palavra.
    role                VARCHAR(16) NOT NULL DEFAULT 'member',

    -- Até onde esta pessoa leu. Move-se para a frente e nunca para trás.
    last_read_at        TIMESTAMPTZ,

    joined_at           TIMESTAMPTZ NOT NULL DEFAULT now(),

    -- Quando deixou de pertencer. A linha fica: as mensagens que ela escreveu
    -- continuam a ter autor, e apagá-la deixaria a conversa cheia de mensagens
    -- de ninguém.
    left_at             TIMESTAMPTZ,

    CONSTRAINT ck_participants_role
        CHECK (role IN ('owner', 'administrator', 'member'))
);

-- Uma pessoa pertence uma vez a uma conversa. Sair e voltar reactiva a linha
-- em vez de acrescentar uma segunda.
CREATE UNIQUE INDEX uq_participants_person
    ON conversation_participants (conversation_id, person_id);

-- A pergunta que a autorização faz a cada subscrição e a cada envio: esta
-- pessoa pertence a esta conversa agora? O índice parcial é o que a torna
-- barata (ADR-0012 §4).
CREATE INDEX ix_participants_current
    ON conversation_participants (person_id, conversation_id)
    WHERE left_at IS NULL;

-- ── Mensagens ───────────────────────────────────────────────────────────

CREATE TABLE messages (
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    conversation_id     UUID NOT NULL REFERENCES conversations (id) ON DELETE CASCADE,

    -- Quem escreveu. Decidido pelo Core a partir do principal, e nunca lido do
    -- pedido: um cliente que escolhesse o autor podia escrever como qualquer
    -- pessoa da instituição.
    author_id           UUID NOT NULL REFERENCES people (id) ON DELETE RESTRICT,

    body                TEXT NOT NULL,

    -- A mensagem a que esta responde. Tem de estar na mesma conversa — e isso
    -- é verificado no Core, porque uma chave estrangeira sozinha aceitaria
    -- qualquer mensagem do sistema inteiro.
    reply_to_id         UUID REFERENCES messages (id) ON DELETE SET NULL,

    -- A chave que torna o envio idempotente. Um duplo-clique, ou um `retry` de
    -- ligação, traz a mesma e não escreve uma segunda mensagem.
    idempotency_key     TEXT,

    edited_at           TIMESTAMPTZ,
    deleted_at          TIMESTAMPTZ,

    created_at          TIMESTAMPTZ NOT NULL DEFAULT now(),

    CONSTRAINT ck_messages_body_not_empty
        CHECK (deleted_at IS NOT NULL OR length(btrim(body)) > 0)
);

-- A ordenação de uma conversa. `(created_at, id)` e não só `created_at`: duas
-- mensagens no mesmo microssegundo precisam de um desempate estável, senão a
-- página seguinte repete ou salta uma.
CREATE INDEX ix_messages_conversation
    ON messages (conversation_id, created_at DESC, id DESC);

CREATE UNIQUE INDEX uq_messages_idempotency
    ON messages (conversation_id, author_id, idempotency_key)
    WHERE idempotency_key IS NOT NULL;

CREATE INDEX ix_messages_reply_to ON messages (reply_to_id)
    WHERE reply_to_id IS NOT NULL;

-- ── Menções ─────────────────────────────────────────────────────────────

-- Uma menção é uma referência a uma identidade, e não um pedaço de texto.
--
-- Procurar `@Fidel` no corpo depois de o guardar daria a menção a quem se
-- chamasse assim e a quem escrevesse o nome por acaso — e deixaria de a dar
-- a quem mudasse de nome.
CREATE TABLE message_mentions (
    message_id          UUID NOT NULL REFERENCES messages (id) ON DELETE CASCADE,
    person_id           UUID NOT NULL REFERENCES people (id) ON DELETE CASCADE,

    PRIMARY KEY (message_id, person_id)
);

CREATE INDEX ix_mentions_person ON message_mentions (person_id);

-- ── Reacções ────────────────────────────────────────────────────────────

CREATE TABLE message_reactions (
    message_id          UUID NOT NULL REFERENCES messages (id) ON DELETE CASCADE,
    person_id           UUID NOT NULL REFERENCES people (id) ON DELETE CASCADE,

    -- O emoji, tal como Unicode o define. Curto de propósito: uma reacção é um
    -- gesto, e um campo largo aqui seria uma segunda caixa de texto.
    emoji               VARCHAR(32) NOT NULL,

    created_at          TIMESTAMPTZ NOT NULL DEFAULT now(),

    -- A mesma pessoa não reage duas vezes com o mesmo emoji à mesma mensagem.
    -- É a base a recusá-lo, e não só o serviço: uma corrida entre dois cliques
    -- passa pelo serviço duas vezes e pela restrição uma.
    PRIMARY KEY (message_id, person_id, emoji),

    CONSTRAINT ck_reactions_emoji_not_empty
        CHECK (length(btrim(emoji)) > 0)
);

CREATE INDEX ix_reactions_message ON message_reactions (message_id);

-- ── Notificações ────────────────────────────────────────────────────────

-- O sino já existe (`0014_calendar.sql`). Uma menção é uma razão nova para
-- tocar, e não um sino novo.
ALTER TABLE notifications DROP CONSTRAINT ck_notifications_kind;
ALTER TABLE notifications ADD CONSTRAINT ck_notifications_kind
    CHECK (kind IN ('reminder', 'event_cancelled', 'event_invited', 'message_mention'));
