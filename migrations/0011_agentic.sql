-- Agentic Control Plane: action plans, approvals.
--
-- O que estas tabelas guardam, e o que deliberadamente não guardam.
--
-- Guardam: o plano — que capabilities, sobre que recursos, por que ordem —,
-- quem o pediu, quem o confirmou, e como cada passo terminou.
--
-- Não guardam: o prompt do membro, o raciocínio do modelo, nem o contexto
-- recuperado. Esses carregam as palavras do próprio membro e material de
-- outras pessoas, e mantê-los construiria uma segunda cópia da instituição
-- dentro de uma tabela que ninguém audita (briefing §48, §177).

-- ── Planos ──────────────────────────────────────────────────────────────

CREATE TABLE action_plans (
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    organisation_id     UUID NOT NULL REFERENCES organisations (id) ON DELETE CASCADE,

    -- Quem pediu. Um plano pertence à pessoa que o pediu, e é por aqui que a
    -- consulta filtra: conhecer o identificador de um plano alheio não o torna
    -- alcançável.
    requested_by        UUID NOT NULL REFERENCES people (id) ON DELETE CASCADE,

    -- Que agente o construiu. Nulo para o Main Agent, que não é uma linha em
    -- `ai_agents`: é o orquestrador do próprio sistema.
    agent_id            UUID REFERENCES ai_agents (id) ON DELETE SET NULL,

    -- O que o membro quis, como o agente entendeu. Limitado: é uma frase, não
    -- uma transcrição.
    intent              TEXT NOT NULL,

    -- Os passos, com o resultado real de cada um depois de executados.
    steps               JSONB NOT NULL DEFAULT '[]'::jsonb,

    state               VARCHAR(24) NOT NULL DEFAULT 'proposed',

    -- Digest do que o plano faz. Uma aprovação liga-se a este valor; alterar o
    -- que o plano faz altera o digest, o que invalida a aprovação
    -- (briefing §100, §101).
    digest              VARCHAR(64) NOT NULL,

    created_at          TIMESTAMPTZ NOT NULL DEFAULT now(),
    settled_at          TIMESTAMPTZ,

    CONSTRAINT ck_action_plans_state CHECK (state IN (
        'proposed', 'awaiting_approval', 'approved', 'executing',
        'completed', 'partially_completed', 'failed',
        'rejected', 'expired', 'cancelled'
    )),

    -- Um plano terminado tem momento de conclusão; um em curso não.
    CONSTRAINT ck_action_plans_settled_agrees CHECK (
        (state IN ('completed', 'partially_completed', 'failed',
                   'rejected', 'expired', 'cancelled') AND settled_at IS NOT NULL)
        OR
        (state IN ('proposed', 'awaiting_approval', 'approved', 'executing')
         AND settled_at IS NULL)
    ),

    CONSTRAINT ck_action_plans_intent_is_bounded CHECK (char_length(intent) <= 500)
);

COMMENT ON TABLE action_plans IS
    'Planos de acção agentic. Não contêm prompts nem raciocínio do modelo.';
COMMENT ON COLUMN action_plans.digest IS
    'Digest do efeito do plano. Uma aprovação é válida apenas para este valor.';
COMMENT ON COLUMN action_plans.agent_id IS
    'Nulo para o Main Agent, que é o orquestrador e não uma definição de agente.';

CREATE INDEX ix_action_plans_by_person
    ON action_plans (requested_by, created_at DESC);

-- ── Aprovações ──────────────────────────────────────────────────────────

CREATE TABLE action_approvals (
    -- Uma aprovação por plano. Reconfirmar substitui a anterior em vez de
    -- acumular, para que não existam duas confirmações válidas com digests
    -- diferentes.
    plan_id         UUID PRIMARY KEY REFERENCES action_plans (id) ON DELETE CASCADE,

    -- Quem confirmou. Comparado com o actor no momento de executar: uma
    -- confirmação não é um vale que outra pessoa possa gastar (briefing §158).
    approved_by     UUID NOT NULL REFERENCES people (id) ON DELETE CASCADE,

    -- O digest no momento da confirmação.
    digest          VARCHAR(64) NOT NULL,

    approved_at     TIMESTAMPTZ NOT NULL DEFAULT now(),

    -- Uma confirmação é consentimento dado com uma situação em mente. Uma hora
    -- depois, a situação é outra e quem disse que sim não está a olhar
    -- (briefing §99).
    expires_at      TIMESTAMPTZ NOT NULL,

    CONSTRAINT ck_action_approvals_expires_after_approval
        CHECK (expires_at > approved_at)
);

COMMENT ON TABLE action_approvals IS
    'Confirmações humanas. Ligadas à pessoa, ao plano e ao momento — as três.';
