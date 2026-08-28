-- Ocinye OS — Ocinye Mail.
--
-- ADR-0400 (arquitectura) · ADR-0401 (fonte de verdade e cache).
--
-- # O que esta base guarda, e o que não guarda
--
-- O **provider é a fonte de verdade da mailbox**. O que está aqui é índice,
-- metadata e estado de integração: o suficiente para listar, procurar e
-- autorizar sem ir à rede a cada ecrã.
--
-- Corpos de mensagem **não** são guardados. São procurados a pedido, saneados
-- no servidor e entregues limpos. Guardar uma segunda cópia do correio
-- institucional criaria uma segunda superfície a proteger, a cifrar, a fazer
-- backup e a apagar — pelo mesmo conteúdo (ADR-0401).
--
-- Rascunhos são a excepção deliberada: um rascunho é do Ocinye até ser enviado.

-- ---------------------------------------------------------------------------
-- Ligação do provider
-- ---------------------------------------------------------------------------

CREATE TABLE mail_provider_settings (
    id                      UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    organisation_id         UUID NOT NULL REFERENCES organisations (id) ON DELETE RESTRICT,

    -- `imap_smtp` hoje. O adapter é escolhido por este valor, e acrescentar um
    -- fornecedor é acrescentar um adapter, não alterar o domínio.
    adapter                 VARCHAR(32) NOT NULL DEFAULT 'imap_smtp',

    imap_host               VARCHAR(255),
    imap_port               INTEGER,
    smtp_host               VARCHAR(255),
    smtp_port               INTEGER,

    -- Domínios da instituição, para distinguir destinatário interno de externo.
    -- Vazio significa **tudo externo**: uma instalação por configurar não pode
    -- concluir que o correio fica dentro de casa.
    institutional_domains   TEXT[] NOT NULL DEFAULT '{}',

    -- Tamanho máximo do conjunto de anexos aceite pelo serviço.
    max_attachment_bytes    BIGINT NOT NULL DEFAULT 26214400,

    configured_by_id        UUID REFERENCES people (id) ON DELETE SET NULL,
    created_at              TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at              TIMESTAMPTZ NOT NULL DEFAULT now(),

    CONSTRAINT uq_mail_provider_per_org UNIQUE (organisation_id),
    CONSTRAINT ck_mail_provider_adapter CHECK (adapter IN ('imap_smtp')),
    CONSTRAINT ck_mail_provider_ports CHECK (
        (imap_port IS NULL OR imap_port BETWEEN 1 AND 65535)
        AND (smtp_port IS NULL OR smtp_port BETWEEN 1 AND 65535)
    ),
    CONSTRAINT ck_mail_provider_attachment_limit CHECK (max_attachment_bytes > 0)
);

COMMENT ON TABLE mail_provider_settings IS
    'Configuração do serviço de correio. NUNCA contém credenciais: essas vêm da '
    'estratégia de secrets do deployment (ADR-0400).';

COMMENT ON COLUMN mail_provider_settings.institutional_domains IS
    'Vazio = tudo externo. Falhar fechado: uma instalação por configurar não '
    'pode concluir que um destinatário é interno.';

-- ---------------------------------------------------------------------------
-- Mailboxes
-- ---------------------------------------------------------------------------

CREATE TABLE mailboxes (
    id                      UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    organisation_id         UUID NOT NULL REFERENCES organisations (id) ON DELETE RESTRICT,

    address                 VARCHAR(320) NOT NULL,
    display_name            VARCHAR(255),
    kind                    VARCHAR(16) NOT NULL DEFAULT 'personal',

    -- O dono de uma mailbox pessoal. NULL numa partilhada, que pertence à
    -- instituição e é alcançada por membership explícita.
    owner_id                UUID REFERENCES people (id) ON DELETE RESTRICT,

    -- Estado da última sincronização. `sync_cursor` é opaco: cada adapter
    -- guarda aqui o que o seu protocolo precisa (UIDVALIDITY/UIDNEXT, um
    -- change token, um cursor). O domínio nunca o interpreta.
    sync_cursor             TEXT,
    last_synced_at          TIMESTAMPTZ,
    last_sync_error         TEXT,

    connected               BOOLEAN NOT NULL DEFAULT true,
    created_at              TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at              TIMESTAMPTZ NOT NULL DEFAULT now(),

    CONSTRAINT ck_mailboxes_kind CHECK (kind IN ('personal', 'shared')),

    -- Uma mailbox pessoal tem dono; uma partilhada não tem. É esta regra que
    -- torna a fronteira de privacidade um facto da base e não uma convenção.
    CONSTRAINT ck_mailboxes_ownership_agrees CHECK (
        (kind = 'personal' AND owner_id IS NOT NULL)
        OR (kind = 'shared' AND owner_id IS NULL)
    ),

    CONSTRAINT ck_mailboxes_address_shape CHECK (position('@' in address) > 1)
);

CREATE UNIQUE INDEX uq_mailboxes_address
    ON mailboxes (organisation_id, lower(address));

-- Uma pessoa tem no máximo uma mailbox pessoal.
CREATE UNIQUE INDEX uq_mailboxes_personal_owner
    ON mailboxes (owner_id) WHERE kind = 'personal';

CREATE INDEX ix_mailboxes_owner ON mailboxes (owner_id) WHERE connected = true;

COMMENT ON TABLE mailboxes IS
    'Associação entre uma identidade Ocinye e uma mailbox existente. O Ocinye '
    'não provisiona contas de correio: associa as que existem (briefing §65).';

-- ---------------------------------------------------------------------------
-- Membros de mailbox partilhada
-- ---------------------------------------------------------------------------

CREATE TABLE shared_mailbox_memberships (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    mailbox_id      UUID NOT NULL REFERENCES mailboxes (id) ON DELETE CASCADE,
    person_id       UUID NOT NULL REFERENCES people (id) ON DELETE CASCADE,

    role            VARCHAR(16) NOT NULL DEFAULT 'reader',

    granted_by_id   UUID NOT NULL REFERENCES people (id) ON DELETE RESTRICT,
    granted_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    revoked_at      TIMESTAMPTZ,
    revoked_by_id   UUID REFERENCES people (id) ON DELETE SET NULL,

    CONSTRAINT ck_shared_mailbox_role
        CHECK (role IN ('reader', 'responder', 'sender', 'manager')),
    CONSTRAINT ck_shared_mailbox_revocation_agrees
        CHECK ((revoked_at IS NULL) = (revoked_by_id IS NULL))
);

CREATE UNIQUE INDEX uq_shared_mailbox_live_membership
    ON shared_mailbox_memberships (mailbox_id, person_id) WHERE revoked_at IS NULL;

CREATE INDEX ix_shared_mailbox_person
    ON shared_mailbox_memberships (person_id) WHERE revoked_at IS NULL;

COMMENT ON TABLE shared_mailbox_memberships IS
    'Pertencer a uma unidade NÃO dá acesso a uma mailbox partilhada. Só uma '
    'linha viva aqui dá (briefing §28).';

-- ---------------------------------------------------------------------------
-- Índice de mensagens
-- ---------------------------------------------------------------------------

CREATE TABLE mail_messages (
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    mailbox_id          UUID NOT NULL REFERENCES mailboxes (id) ON DELETE CASCADE,

    -- O identificador do provider. Opaco para o domínio, necessário para ir
    -- buscar o corpo quando alguém abre a mensagem.
    provider_id         TEXT NOT NULL,
    -- `Message-ID` do RFC 5322, quando existe.
    message_id          TEXT,
    -- Identidade de conversa derivada de `References`/`In-Reply-To`, nunca do
    -- assunto: dois emails com o mesmo assunto não são a mesma conversa.
    thread_key          TEXT,

    folder              VARCHAR(16) NOT NULL DEFAULT 'inbox',

    from_address        VARCHAR(320) NOT NULL,
    from_display_name   VARCHAR(255),
    -- Destinatários como metadata de listagem. O detalhe completo vem do
    -- provider ao abrir a mensagem.
    to_addresses        TEXT[] NOT NULL DEFAULT '{}',
    cc_addresses        TEXT[] NOT NULL DEFAULT '{}',

    subject             TEXT,
    -- Excerto curto para a lista. Um índice é um instrumento de busca, não uma
    -- segunda cópia do corpus.
    snippet             VARCHAR(512),

    sent_at             TIMESTAMPTZ NOT NULL,
    is_read             BOOLEAN NOT NULL DEFAULT false,
    is_starred          BOOLEAN NOT NULL DEFAULT false,
    has_attachments     BOOLEAN NOT NULL DEFAULT false,
    size_bytes          BIGINT,

    -- Índice textual sobre assunto, remetente e excerto. `simple` e não uma
    -- configuração de língua: o correio é multilingue.
    search_vector       tsvector GENERATED ALWAYS AS (
        setweight(to_tsvector('simple', coalesce(subject, '')), 'A') ||
        setweight(to_tsvector('simple', coalesce(from_address, '')), 'B') ||
        setweight(to_tsvector('simple', coalesce(from_display_name, '')), 'B') ||
        setweight(to_tsvector('simple', coalesce(snippet, '')), 'C')
    ) STORED,

    indexed_at          TIMESTAMPTZ NOT NULL DEFAULT now(),

    CONSTRAINT ck_mail_messages_folder
        CHECK (folder IN ('inbox','starred','drafts','sent','archive','spam','trash'))
);

CREATE UNIQUE INDEX uq_mail_messages_provider
    ON mail_messages (mailbox_id, provider_id);

CREATE INDEX ix_mail_messages_folder
    ON mail_messages (mailbox_id, folder, sent_at DESC);
CREATE INDEX ix_mail_messages_unread
    ON mail_messages (mailbox_id, folder) WHERE is_read = false;
CREATE INDEX ix_mail_messages_thread
    ON mail_messages (mailbox_id, thread_key) WHERE thread_key IS NOT NULL;
CREATE INDEX ix_mail_messages_search
    ON mail_messages USING gin (search_vector);

COMMENT ON TABLE mail_messages IS
    'Índice, não arquivo. Corpos e anexos ficam no provider e são procurados a '
    'pedido (ADR-0401).';

COMMENT ON COLUMN mail_messages.thread_key IS
    'Derivada de References/In-Reply-To. Nunca do assunto (briefing §31).';

-- ---------------------------------------------------------------------------
-- Rascunhos
-- ---------------------------------------------------------------------------

CREATE TABLE mail_drafts (
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    mailbox_id          UUID NOT NULL REFERENCES mailboxes (id) ON DELETE CASCADE,
    author_id           UUID NOT NULL REFERENCES people (id) ON DELETE RESTRICT,

    -- A identidade sob a qual será enviado. Verificada contra as identidades
    -- autorizadas no envio, para que ninguém escreva em nome de outro.
    sender_address      VARCHAR(320) NOT NULL,

    to_addresses        TEXT[] NOT NULL DEFAULT '{}',
    cc_addresses        TEXT[] NOT NULL DEFAULT '{}',
    bcc_addresses       TEXT[] NOT NULL DEFAULT '{}',

    subject             TEXT,
    body                TEXT NOT NULL DEFAULT '',

    -- A mensagem a que este rascunho responde, quando responde a alguma.
    in_reply_to_id      UUID REFERENCES mail_messages (id) ON DELETE SET NULL,

    -- Como foi escrito. Metadata institucional, não um selo na mensagem.
    origin              VARCHAR(24) NOT NULL DEFAULT 'manual',

    created_at          TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT now(),

    CONSTRAINT ck_mail_drafts_origin
        CHECK (origin IN ('manual', 'ai_generated', 'ai_transformed'))
);

CREATE INDEX ix_mail_drafts_author ON mail_drafts (author_id, updated_at DESC);
CREATE INDEX ix_mail_drafts_mailbox ON mail_drafts (mailbox_id, updated_at DESC);

COMMENT ON TABLE mail_drafts IS
    'A excepção deliberada ao ADR-0401: um rascunho é do Ocinye até ser '
    'enviado, e não pode perder-se porque o provider não respondeu.';

-- ---------------------------------------------------------------------------
-- Anexos de rascunho
-- ---------------------------------------------------------------------------

CREATE TABLE mail_draft_attachments (
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    draft_id            UUID NOT NULL REFERENCES mail_drafts (id) ON DELETE CASCADE,

    filename            VARCHAR(255) NOT NULL,
    content_type        VARCHAR(128) NOT NULL,
    size_bytes          BIGINT NOT NULL,
    checksum_sha256     CHAR(64) NOT NULL,

    -- Anexo vindo de um artefacto do Ocinye. A classificação viaja com ele, e
    -- é o que a política de envio externo consulta (briefing §35).
    document_id         UUID REFERENCES documents (id) ON DELETE RESTRICT,
    classification      VARCHAR(32),

    -- Objecto no armazenamento, para ficheiros carregados no composer.
    storage_object_id   UUID REFERENCES storage_objects (id) ON DELETE RESTRICT,

    created_at          TIMESTAMPTZ NOT NULL DEFAULT now(),

    CONSTRAINT ck_mail_attachment_size CHECK (size_bytes >= 0),
    CONSTRAINT ck_mail_attachment_classification
        CHECK (classification IS NULL
               OR classification IN ('PUBLIC','INTERNAL','CONFIDENTIAL','RESTRICTED')),

    -- Um anexo tem uma origem, e exactamente uma: um artefacto do Ocinye ou um
    -- ficheiro carregado. Sem origem não há bytes para enviar.
    CONSTRAINT ck_mail_attachment_has_one_source CHECK (
        (document_id IS NOT NULL AND storage_object_id IS NULL)
        OR (document_id IS NULL AND storage_object_id IS NOT NULL)
    ),

    -- Um nome de ficheiro nunca contém separadores de caminho: é escrito no
    -- cabeçalho MIME e lido por clientes que não controlamos.
    CONSTRAINT ck_mail_attachment_filename_is_safe CHECK (
        filename !~ '[/\\\\]' AND filename <> '.' AND filename <> '..'
        AND char_length(btrim(filename)) > 0
    )
);

CREATE INDEX ix_mail_draft_attachments ON mail_draft_attachments (draft_id);

-- ---------------------------------------------------------------------------
-- Outbox
-- ---------------------------------------------------------------------------

CREATE TABLE mail_outbox (
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    mailbox_id          UUID NOT NULL REFERENCES mailboxes (id) ON DELETE CASCADE,
    author_id           UUID NOT NULL REFERENCES people (id) ON DELETE RESTRICT,

    -- O rascunho de origem. Preservado até o envio ser confirmado: um envio
    -- falhado nunca perde o que foi escrito (briefing §45).
    draft_id            UUID REFERENCES mail_drafts (id) ON DELETE SET NULL,

    state               VARCHAR(16) NOT NULL DEFAULT 'queued',
    attempts            INTEGER NOT NULL DEFAULT 0,
    -- Razão da falha, em linguagem institucional. Nunca o erro cru do provider.
    last_error          TEXT,

    -- Contagens, para auditoria e evidência. Nunca os endereços: quem escreveu
    -- a quem é conteúdo, e o rasto de auditoria guarda referências.
    recipient_count     INTEGER NOT NULL DEFAULT 0,
    external_recipients INTEGER NOT NULL DEFAULT 0,

    queued_at           TIMESTAMPTZ NOT NULL DEFAULT now(),
    sent_at             TIMESTAMPTZ,

    CONSTRAINT ck_mail_outbox_state
        CHECK (state IN ('queued', 'sending', 'sent', 'failed')),
    CONSTRAINT ck_mail_outbox_sent_agrees
        CHECK ((state = 'sent') = (sent_at IS NOT NULL))
);

CREATE INDEX ix_mail_outbox_pending
    ON mail_outbox (queued_at) WHERE state IN ('queued', 'sending');
CREATE INDEX ix_mail_outbox_author ON mail_outbox (author_id, queued_at DESC);

-- ---------------------------------------------------------------------------
-- Definições de correio de cada membro
-- ---------------------------------------------------------------------------

CREATE TABLE mail_preferences (
    person_id               UUID PRIMARY KEY REFERENCES people (id) ON DELETE CASCADE,

    signature               TEXT,
    -- Bloquear é o padrão: uma imagem remota diz ao remetente que a mensagem
    -- foi aberta, de onde e quando (briefing §12).
    remote_content_policy   VARCHAR(24) NOT NULL DEFAULT 'block',

    updated_at              TIMESTAMPTZ NOT NULL DEFAULT now(),

    CONSTRAINT ck_mail_remote_content_policy
        CHECK (remote_content_policy IN ('block', 'allow_once', 'allow_known_senders'))
);

COMMENT ON COLUMN mail_preferences.remote_content_policy IS
    'Bloquear por omissão. Um valor corrompido lê-se como bloquear.';
