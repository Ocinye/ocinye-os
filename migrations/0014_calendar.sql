-- Centro Temporal e Calendário nativo: eventos, lembretes e notificações.
--
-- O que estas tabelas guardam, e o que deliberadamente não guardam.
--
-- Guardam: compromissos futuros da instituição, quem os marcou, em que âmbito,
-- e quando alguém pediu para ser lembrado.
--
-- Não guardam: prazos de tarefas. Uma `Task` com `due_on` já é um compromisso
-- temporal e vive em Collaboration; o calendário mostra-a por projecção. Copiá-la
-- para aqui criaria duas datas para o mesmo prazo, e uma delas ficaria errada sem
-- ninguém saber qual (ADR-0410).

-- ── Eventos ─────────────────────────────────────────────────────────────

CREATE TABLE calendar_events (
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    organisation_id     UUID NOT NULL REFERENCES organisations (id) ON DELETE CASCADE,

    -- A quem pertence. `personal` e `institution` não têm contentor; `unit` e
    -- `research_workspace` têm, e a restrição abaixo exige-o. Um evento «de uma
    -- unidade» que não diz qual deixaria a autorização sem nada contra que
    -- decidir.
    scope               VARCHAR(32) NOT NULL,
    unit_id             UUID REFERENCES units (id) ON DELETE CASCADE,
    workspace_id        UUID REFERENCES research_workspaces (id) ON DELETE CASCADE,

    -- De quem é, quando é de alguém.
    --
    -- Separado de `created_by_id` de propósito: quem criou é proveniência, quem
    -- é dono é autoridade. Hoje coincidem quase sempre; no dia em que um
    -- assistente marcar a agenda de outra pessoa deixam de coincidir, e é a
    -- autoridade que decide quem lê. Conflacioná-los agora seria escolher o
    -- campo errado para autorizar sem dar por isso.
    --
    -- Obrigatório em `personal` e proibido nos outros: um evento de unidade não
    -- tem dono individual, tem contentor.
    owner_id            UUID REFERENCES people (id) ON DELETE CASCADE,

    title               VARCHAR(255) NOT NULL,
    description         TEXT,
    location            TEXT,

    -- Um evento tem hora, ou é de dia inteiro. Nunca as duas coisas, nunca
    -- nenhuma — a restrição `ck_calendar_events_occurrence` fecha isso.
    all_day             BOOLEAN NOT NULL,

    -- Com hora: o instante canónico, e a zona da intenção.
    --
    -- Os dois, e não só o primeiro. «14:00 em Paris» é um ponto na linha do
    -- tempo *e* uma intenção humana; guardar só o instante chega para mostrar,
    -- e não chega para editar — quem a mudar para as 15:00 tem de saber 15:00
    -- de onde.
    starts_at           TIMESTAMPTZ,
    ends_at             TIMESTAMPTZ,
    timezone            VARCHAR(64),

    -- De dia inteiro: datas civis, sem hora.
    --
    -- Não `00:00 UTC`. Um prazo guardado como instante cai no dia anterior para
    -- quem está a leste, e um prazo que muda de dia consoante quem o lê não é
    -- um prazo.
    -- Meio-aberto: `[starts_on, ends_before)`. Um evento de um dia é
    -- `24 → 25`. Inclusivo obrigaria toda a gente a somar um dia, e um dia
    -- alguém não somaria.
    starts_on           DATE,
    ends_before         DATE,

    state               VARCHAR(32) NOT NULL DEFAULT 'scheduled',
    classification      VARCHAR(32) NOT NULL DEFAULT 'INTERNAL',

    created_by_id       UUID REFERENCES people (id) ON DELETE SET NULL,
    updated_by_id       UUID REFERENCES people (id) ON DELETE SET NULL,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT now(),

    CONSTRAINT ck_calendar_events_scope
        CHECK (scope IN ('personal', 'unit', 'research_workspace', 'institution')),

    CONSTRAINT ck_calendar_events_state
        CHECK (state IN ('scheduled', 'cancelled')),

    -- O contentor existe exactamente onde o âmbito o exige, e não noutro sítio.
    CONSTRAINT ck_calendar_events_container CHECK (
        (scope = 'unit'               AND unit_id IS NOT NULL AND workspace_id IS NULL AND owner_id IS NULL)
     OR (scope = 'research_workspace' AND workspace_id IS NOT NULL AND owner_id IS NULL)
     OR (scope = 'institution'        AND unit_id IS NULL AND workspace_id IS NULL AND owner_id IS NULL)
     OR (scope = 'personal'           AND unit_id IS NULL AND workspace_id IS NULL AND owner_id IS NOT NULL)
    ),

    -- Com hora tem instantes e zona e não tem datas civis; de dia inteiro é o
    -- contrário. Um estado a meio caminho seria um evento que a aplicação teria
    -- de adivinhar como ler.
    CONSTRAINT ck_calendar_events_occurrence CHECK (
        (all_day = FALSE
            AND starts_at IS NOT NULL AND ends_at IS NOT NULL AND timezone IS NOT NULL
            AND starts_on IS NULL AND ends_before IS NULL)
     OR (all_day = TRUE
            AND starts_on IS NOT NULL AND ends_before IS NOT NULL
            AND starts_at IS NULL AND ends_at IS NULL AND timezone IS NULL)
    ),

    -- Estritamente maior, nos dois casos.
    --
    -- Um evento sem duração não é um compromisso: é um instante, e um
    -- compromisso é algo que ocupa tempo de alguém. Aceitar duração zero daria
    -- duas maneiras de representar a mesma coisa, e a interface teria de
    -- escolher uma delas para mostrar.
    --
    -- Isto **não** quer dizer que todo o instante seja um lembrete. Um lembrete
    -- é uma intenção de notificar. Um marco temporal sem duração que não
    -- pretenda avisar ninguém é outra coisa, e no dia em que fizer falta nasce
    -- como entidade própria — não espremido dentro de `reminders`.
    CONSTRAINT ck_calendar_events_order CHECK (
        (all_day = FALSE AND ends_at > starts_at)
     OR (all_day = TRUE  AND ends_before > starts_on)
    )
);

-- A consulta do calendário é sempre por intervalo. Estes dois índices são o que
-- a torna uma leitura de intervalo em vez de uma varredura da instituição.
CREATE INDEX ix_calendar_events_timed
    ON calendar_events (organisation_id, starts_at, ends_at)
    WHERE all_day = FALSE;

CREATE INDEX ix_calendar_events_all_day
    ON calendar_events (organisation_id, starts_on, ends_before)
    WHERE all_day = TRUE;

CREATE INDEX ix_calendar_events_unit      ON calendar_events (unit_id)      WHERE unit_id IS NOT NULL;
CREATE INDEX ix_calendar_events_workspace ON calendar_events (workspace_id) WHERE workspace_id IS NOT NULL;
-- A agenda pessoal pergunta sempre a mesma coisa: o que é meu.
CREATE INDEX ix_calendar_events_owner     ON calendar_events (owner_id) WHERE owner_id IS NOT NULL;

-- ── Participantes ───────────────────────────────────────────────────────
--
-- Referências institucionais tipadas, e não nomes escritos à mão. Uma pessoa da
-- Ocinye guardada como texto deixa de ser a pessoa e passa a ser uma etiqueta
-- que ninguém pode autorizar nem notificar.

CREATE TABLE calendar_event_participants (
    event_id        UUID NOT NULL REFERENCES calendar_events (id) ON DELETE CASCADE,
    person_id       UUID NOT NULL REFERENCES people (id) ON DELETE CASCADE,
    added_at        TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (event_id, person_id)
);

CREATE INDEX ix_calendar_event_participants_person ON calendar_event_participants (person_id);

-- ── Lembretes ───────────────────────────────────────────────────────────
--
-- Um lembrete refere um recurso; não o copia. Duplicar o conteúdo faria com que
-- mudar o evento deixasse o lembrete a dizer outra coisa — e o lembrete é o que
-- a pessoa lê primeiro.

CREATE TABLE reminders (
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    organisation_id     UUID NOT NULL REFERENCES organisations (id) ON DELETE CASCADE,

    -- De quem é. Um lembrete é sempre de uma pessoa: não há lembretes de
    -- unidades, porque não é a unidade que se esquece.
    owner_id            UUID NOT NULL REFERENCES people (id) ON DELETE CASCADE,

    -- Sobre o quê. Exactamente um dos dois, ou nenhum quando é um lembrete
    -- solto — «rever o relatório» sem recurso ligado é legítimo.
    event_id            UUID REFERENCES calendar_events (id) ON DELETE CASCADE,
    task_id             UUID REFERENCES tasks (id) ON DELETE CASCADE,

    -- O que dizer. Curto de propósito: o lembrete aponta, não transcreve.
    note                VARCHAR(255),

    -- Quando dispara. Instante canónico: o worker não lê relógios de browser.
    trigger_at          TIMESTAMPTZ NOT NULL,

    state               VARCHAR(32) NOT NULL DEFAULT 'scheduled',

    -- Quantas vezes o mecanismo tentou.
    --
    -- Isto é contabilidade do worker, e não parte da intenção temporal — vive
    -- aqui porque é esta a linha que o worker tranca, e não porque o lembrete
    -- precise de saber. Serve para um lembrete que falha sempre deixar de ser
    -- tentado para sempre, como o `outbox` já faz.
    --
    -- **Quando** foi entregue não está aqui: esse facto vive em
    -- `reminder_deliveries`, uma vez por canal. Duplicá-lo daria duas respostas
    -- à mesma pergunta.
    attempts            INTEGER NOT NULL DEFAULT 0,

    created_at          TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT now(),

    CONSTRAINT ck_reminders_state
        CHECK (state IN ('scheduled', 'delivered', 'snoozed', 'dismissed', 'cancelled')),

    -- No máximo um recurso. Dois seria um lembrete que aponta para dois sítios
    -- e abre um deles à sorte.
    CONSTRAINT ck_reminders_single_resource
        CHECK (NOT (event_id IS NOT NULL AND task_id IS NOT NULL)),

    -- Um lembrete sem recurso tem de dizer o que é. Sem isto seria uma
    -- notificação em branco à hora marcada.
    CONSTRAINT ck_reminders_standalone_says_something
        CHECK (event_id IS NOT NULL OR task_id IS NOT NULL OR note IS NOT NULL)
);

-- O worker procura sempre a mesma coisa: o que está pendente e já passou da
-- hora. O índice é parcial porque o que já foi entregue não volta a ser lido.
CREATE INDEX ix_reminders_due
    ON reminders (trigger_at)
    WHERE state IN ('scheduled', 'snoozed');

CREATE INDEX ix_reminders_owner ON reminders (owner_id, state);
CREATE INDEX ix_reminders_event ON reminders (event_id) WHERE event_id IS NOT NULL;
CREATE INDEX ix_reminders_task  ON reminders (task_id)  WHERE task_id IS NOT NULL;

-- ── Notificações ────────────────────────────────────────────────────────
--
-- O destino visível de um lembrete. Sem isto, um lembrete que dispara é um
-- lembrete que não foi entregue.
--
-- A notificação guarda um título e uma referência, e **não** o conteúdo do
-- recurso: quando alguém a abre, o Core reautoriza o recurso nesse momento. Uma
-- notificação não é uma cópia autorizada de nada.

CREATE TABLE notifications (
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    organisation_id     UUID NOT NULL REFERENCES organisations (id) ON DELETE CASCADE,
    recipient_id        UUID NOT NULL REFERENCES people (id) ON DELETE CASCADE,

    kind                VARCHAR(32) NOT NULL,

    -- O que mostrar na lista. Curto, e sem conteúdo sensível: a lista aparece
    -- num painel que qualquer pessoa por trás da cadeira consegue ler.
    title               VARCHAR(255) NOT NULL,
    body                VARCHAR(500),

    -- Para onde ir. Tipo e identificador, resolvidos e reautorizados na
    -- abertura.
    resource_type       VARCHAR(32),
    resource_id         UUID,

    read_at             TIMESTAMPTZ,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT now(),

    CONSTRAINT ck_notifications_kind
        CHECK (kind IN ('reminder', 'event_cancelled', 'event_invited')),

    CONSTRAINT ck_notifications_resource
        CHECK ((resource_type IS NULL) = (resource_id IS NULL))
);

-- O sino pergunta uma única coisa: quantas por ler. O índice parcial é o que
-- torna essa pergunta barata em vez de uma contagem sobre tudo o que já foi lido.
CREATE INDEX ix_notifications_unread
    ON notifications (recipient_id, created_at DESC)
    WHERE read_at IS NULL;

CREATE INDEX ix_notifications_recipient ON notifications (recipient_id, created_at DESC);

-- Um lembrete entrega-se uma vez **por canal**.
--
-- A chave inclui o canal de propósito. Com `PRIMARY KEY (reminder_id)` sozinha,
-- a tabela afirmava «um lembrete tem exactamente uma entrega, e ela é uma
-- notificação» — e acrescentar correio amanhã obrigaria a alterar a semântica de
-- uma tabela já em uso. Assim, um canal novo é uma linha nova.
--
-- É também a garantia de que dois workers não entregam o mesmo lembrete duas
-- vezes: o segundo encontra esta restrição, em vez de depender de o `SKIP
-- LOCKED` ter chegado a tempo.
CREATE TABLE reminder_deliveries (
    reminder_id     UUID NOT NULL REFERENCES reminders (id) ON DELETE CASCADE,
    channel         VARCHAR(32) NOT NULL,

    -- Preenchido pelo canal in-app. Um canal que não produza notificação — o
    -- correio, amanhã — deixa-o nulo e regista a entrega à mesma.
    notification_id UUID REFERENCES notifications (id) ON DELETE CASCADE,

    delivered_at    TIMESTAMPTZ NOT NULL DEFAULT now(),

    PRIMARY KEY (reminder_id, channel),

    CONSTRAINT ck_reminder_deliveries_channel CHECK (channel IN ('in_app')),

    -- O canal in-app existe para produzir uma notificação. Sem ela, «entregue»
    -- seria uma afirmação sem nada que a pessoa possa ver.
    CONSTRAINT ck_reminder_deliveries_in_app_has_notification
        CHECK (channel <> 'in_app' OR notification_id IS NOT NULL)
);
