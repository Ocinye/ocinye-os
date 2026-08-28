-- O ciclo científico, e a proveniência que o torna memória.
--
-- # O que faltava
--
-- O Ocinye tinha Ideias, Projectos, Ambientes de investigação, Conhecimento e
-- Dados. Tinha `research_links` — arestas tipadas entre recursos, com
-- vocabulário fechado. Não tinha aquilo que liga as duas coisas: **os objectos
-- científicos entre a ideia e o dado**.
--
-- O catálogo dizia-o por extenso. `knowledge::create_result` estava lá,
-- declarado `not_implemented`, com a razão «A entidade Resultado ainda não
-- existe no domínio». Esta migration é a que a faz existir, e com ela o que um
-- resultado precisa para ser mais do que uma frase.
--
-- # Porque a versão é um recurso, e não uma coluna
--
-- Um resultado produzido com a metodologia M tem de continuar a dizer **qual
-- M** daqui a três anos, mesmo que M tenha mudado seis vezes. A tentação é uma
-- coluna `version` na aresta; a resposta certa é que a versão **é** um recurso
-- com identidade própria — como `dataset_versions` já era desde 2026-08.
--
-- A aresta aponta para `methodology_version` ou `dataset_version`, e fica
-- correcta para sempre sem que ninguém tenha de a manter.

-- ── Hipótese ────────────────────────────────────────────────────────────
--
-- Uma afirmação que se pode testar, e cujo desfecho é institucionalmente
-- valioso mesmo quando é «não». Uma hipótese refutada poupa a quem vier a
-- seguir o trabalho de a voltar a testar — e é por isso que `refuted` é um
-- estado normal, e não uma falha a esconder.

CREATE TABLE hypotheses (
    id               UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    organisation_id  UUID NOT NULL REFERENCES organisations (id) ON DELETE CASCADE,
    unit_id          UUID NOT NULL REFERENCES units (id) ON DELETE CASCADE,
    workspace_id     UUID REFERENCES research_workspaces (id) ON DELETE SET NULL,
    project_id       UUID REFERENCES projects (id) ON DELETE SET NULL,
    statement        TEXT NOT NULL,
    rationale        TEXT,
    status           VARCHAR(24) NOT NULL DEFAULT 'open',
    classification   VARCHAR(16) NOT NULL DEFAULT 'INTERNAL',
    created_by_id    UUID REFERENCES people (id) ON DELETE SET NULL,
    created_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT ck_hypotheses_status CHECK (status IN (
        'open', 'supported', 'refuted', 'inconclusive', 'withdrawn'
    )),
    CONSTRAINT ck_hypotheses_classification CHECK (classification IN (
        'PUBLIC', 'INTERNAL', 'CONFIDENTIAL', 'RESTRICTED'
    )),
    CONSTRAINT ck_hypotheses_statement CHECK (length(btrim(statement)) > 0)
);

CREATE INDEX ix_hypotheses_workspace ON hypotheses (workspace_id);
CREATE INDEX ix_hypotheses_project ON hypotheses (project_id);

COMMENT ON COLUMN hypotheses.status IS
    'Um desfecho, e não um progresso. `refuted` é um resultado institucional '
    'legítimo: poupa a quem vier a seguir o trabalho de voltar a testar.';

-- ── Metodologia, e as suas versões ──────────────────────────────────────
--
-- A metodologia tem identidade porque a pergunta «que metodologia produziu
-- isto?» tem de ter resposta anos depois. E tem versões porque a pergunta
-- verdadeira é «**qual versão** dela», e a resposta não pode mudar quando
-- alguém melhora o método.
--
-- O conteúdo grande vive onde os conteúdos grandes vivem: um documento em
-- object storage. Aqui fica o resumo e a referência.

CREATE TABLE methodologies (
    id               UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    organisation_id  UUID NOT NULL REFERENCES organisations (id) ON DELETE CASCADE,
    unit_id          UUID NOT NULL REFERENCES units (id) ON DELETE CASCADE,
    workspace_id     UUID REFERENCES research_workspaces (id) ON DELETE SET NULL,
    project_id       UUID REFERENCES projects (id) ON DELETE SET NULL,
    title            VARCHAR(200) NOT NULL,
    purpose          TEXT,
    classification   VARCHAR(16) NOT NULL DEFAULT 'INTERNAL',
    created_by_id    UUID REFERENCES people (id) ON DELETE SET NULL,
    created_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT ck_methodologies_classification CHECK (classification IN (
        'PUBLIC', 'INTERNAL', 'CONFIDENTIAL', 'RESTRICTED'
    )),
    CONSTRAINT ck_methodologies_title CHECK (length(btrim(title)) > 0)
);

CREATE INDEX ix_methodologies_workspace ON methodologies (workspace_id);

-- Uma versão não se altera depois de publicada.
--
-- É o que faz a proveniência valer: se a versão que um resultado usou puder
-- mudar, a linhagem passa a descrever outra coisa sem que ninguém o note. Uma
-- correcção é uma versão nova, e a anterior fica — marcada como substituída,
-- e não apagada.
CREATE TABLE methodology_versions (
    id                UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    methodology_id    UUID NOT NULL REFERENCES methodologies (id) ON DELETE CASCADE,
    sequence          INTEGER NOT NULL,
    label             VARCHAR(32) NOT NULL,
    summary           TEXT NOT NULL,
    -- O documento completo, quando existe. O binário é do object storage.
    document_id       UUID REFERENCES documents (id) ON DELETE SET NULL,
    status            VARCHAR(24) NOT NULL DEFAULT 'draft',
    superseded_by_id  UUID REFERENCES methodology_versions (id) ON DELETE SET NULL,
    published_at      TIMESTAMPTZ,
    created_by_id     UUID REFERENCES people (id) ON DELETE SET NULL,
    created_at        TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT uq_methodology_versions_sequence UNIQUE (methodology_id, sequence),
    CONSTRAINT uq_methodology_versions_label UNIQUE (methodology_id, label),
    CONSTRAINT ck_methodology_versions_status CHECK (status IN (
        'draft', 'published', 'superseded'
    )),
    CONSTRAINT ck_methodology_versions_not_self
        CHECK (superseded_by_id IS NULL OR superseded_by_id <> id),
    CONSTRAINT ck_methodology_versions_summary CHECK (length(btrim(summary)) > 0)
);

CREATE INDEX ix_methodology_versions_methodology
    ON methodology_versions (methodology_id, sequence DESC);

-- ── Estudo: experimento, simulação ou análise ───────────────────────────
--
-- Uma entidade com tipo explícito, e não três tabelas.
--
-- Um experimento físico, uma simulação numérica e uma análise partilham tudo
-- o que importa a esta camada: pertencem a um ambiente, testam uma hipótese,
-- usam uma metodologia, consomem dados, executam-se e produzem resultados. O
-- que os distingue — bancada, malha, série temporal — é detalhe de cada
-- disciplina, e não pertence ao núcleo.
--
-- Três tabelas obrigariam a triplicar cada consulta de linhagem, e a decidir
-- em qual delas procurar antes de saber o que se procura.

CREATE TABLE studies (
    id               UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    organisation_id  UUID NOT NULL REFERENCES organisations (id) ON DELETE CASCADE,
    unit_id          UUID NOT NULL REFERENCES units (id) ON DELETE CASCADE,
    workspace_id     UUID REFERENCES research_workspaces (id) ON DELETE SET NULL,
    project_id       UUID REFERENCES projects (id) ON DELETE SET NULL,
    hypothesis_id    UUID REFERENCES hypotheses (id) ON DELETE SET NULL,
    title            VARCHAR(200) NOT NULL,
    kind             VARCHAR(24) NOT NULL,
    objective        TEXT,
    status           VARCHAR(24) NOT NULL DEFAULT 'planned',
    classification   VARCHAR(16) NOT NULL DEFAULT 'INTERNAL',
    created_by_id    UUID REFERENCES people (id) ON DELETE SET NULL,
    created_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT ck_studies_kind CHECK (kind IN (
        'physical_experiment', 'simulation', 'analysis'
    )),
    CONSTRAINT ck_studies_status CHECK (status IN (
        'planned', 'running', 'completed', 'abandoned'
    )),
    CONSTRAINT ck_studies_classification CHECK (classification IN (
        'PUBLIC', 'INTERNAL', 'CONFIDENTIAL', 'RESTRICTED'
    )),
    CONSTRAINT ck_studies_title CHECK (length(btrim(title)) > 0)
);

CREATE INDEX ix_studies_workspace ON studies (workspace_id);
CREATE INDEX ix_studies_hypothesis ON studies (hypothesis_id);

-- ── Execução ────────────────────────────────────────────────────────────
--
-- Uma corrida concreta de um estudo. É aqui que a reprodutibilidade mora: o
-- mesmo estudo corre duas vezes e dá duas execuções, e são elas — não o
-- estudo — que se comparam.
--
-- O nó de computação é referenciado quando existe; não é exigido. Uma medição
-- de bancada não corre em GPU nenhuma, e obrigar a um nó tornaria metade da
-- ciência da instituição inexprimível.
--
-- O software fica em campos simples e opcionais: nome, versão, *commit*,
-- digest da imagem. Não é telemetria — é o mínimo para responder «com que
-- código foi isto feito».

CREATE TABLE study_executions (
    id                UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    organisation_id   UUID NOT NULL REFERENCES organisations (id) ON DELETE CASCADE,
    study_id          UUID NOT NULL REFERENCES studies (id) ON DELETE CASCADE,
    sequence          INTEGER NOT NULL,
    status            VARCHAR(24) NOT NULL DEFAULT 'recorded',
    started_at        TIMESTAMPTZ,
    finished_at       TIMESTAMPTZ,
    -- Onde correu, quando correu em algum lado que o Ocinye conhece.
    compute_node_id   UUID REFERENCES compute_nodes (id) ON DELETE SET NULL,
    environment       VARCHAR(200),
    software_name     VARCHAR(120),
    software_version  VARCHAR(64),
    software_commit   VARCHAR(64),
    image_digest      VARCHAR(128),
    configuration     TEXT,
    notes             TEXT,
    created_by_id     UUID REFERENCES people (id) ON DELETE SET NULL,
    created_at        TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT uq_study_executions_sequence UNIQUE (study_id, sequence),
    CONSTRAINT ck_study_executions_status CHECK (status IN (
        'recorded', 'running', 'succeeded', 'failed', 'aborted'
    )),
    CONSTRAINT ck_study_executions_window
        CHECK (finished_at IS NULL OR started_at IS NULL OR finished_at >= started_at)
);

CREATE INDEX ix_study_executions_study ON study_executions (study_id, sequence DESC);

-- ── Resultado ───────────────────────────────────────────────────────────
--
-- O objecto que o catálogo declarava em falta.
--
-- Um resultado não é uma publicação: publicar é um desfecho posterior e
-- possível. Também não é um protótipo. É a evidência ou a conclusão que o
-- trabalho produziu — incluindo quando essa conclusão é que a hipótese não se
-- sustenta.

CREATE TABLE results (
    id                UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    organisation_id   UUID NOT NULL REFERENCES organisations (id) ON DELETE CASCADE,
    unit_id           UUID NOT NULL REFERENCES units (id) ON DELETE CASCADE,
    workspace_id      UUID REFERENCES research_workspaces (id) ON DELETE SET NULL,
    project_id        UUID REFERENCES projects (id) ON DELETE SET NULL,
    execution_id      UUID REFERENCES study_executions (id) ON DELETE SET NULL,
    title             VARCHAR(200) NOT NULL,
    summary           TEXT NOT NULL,
    status            VARCHAR(24) NOT NULL DEFAULT 'draft',
    classification    VARCHAR(16) NOT NULL DEFAULT 'INTERNAL',
    superseded_by_id  UUID REFERENCES results (id) ON DELETE SET NULL,
    created_by_id     UUID REFERENCES people (id) ON DELETE SET NULL,
    created_at        TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at        TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT ck_results_status CHECK (status IN (
        'draft', 'under_review', 'validated', 'superseded', 'invalidated'
    )),
    CONSTRAINT ck_results_classification CHECK (classification IN (
        'PUBLIC', 'INTERNAL', 'CONFIDENTIAL', 'RESTRICTED'
    )),
    CONSTRAINT ck_results_not_self
        CHECK (superseded_by_id IS NULL OR superseded_by_id <> id),
    CONSTRAINT ck_results_title CHECK (length(btrim(title)) > 0),
    CONSTRAINT ck_results_summary CHECK (length(btrim(summary)) > 0)
);

CREATE INDEX ix_results_workspace ON results (workspace_id);
CREATE INDEX ix_results_execution ON results (execution_id);

-- ── Validação e reprodução ──────────────────────────────────────────────
--
-- Reprodutibilidade é evidência, e não um rótulo.
--
-- Um resultado não fica «reproduzido» porque alguém escreveu que o pretendia
-- reproduzir. Fica reproduzido quando existe **outra execução** e alguém
-- registou o que ela mostrou — incluindo quando mostrou o contrário.
--
-- Por isso `outcome` não tem valor por omissão que signifique sucesso, e a
-- execução que serviu de prova é uma referência real.

CREATE TABLE result_validations (
    id                UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    organisation_id   UUID NOT NULL REFERENCES organisations (id) ON DELETE CASCADE,
    result_id         UUID NOT NULL REFERENCES results (id) ON DELETE CASCADE,
    kind              VARCHAR(24) NOT NULL,
    outcome           VARCHAR(24) NOT NULL,
    -- A execução que serviu de prova, quando houve uma.
    execution_id      UUID REFERENCES study_executions (id) ON DELETE SET NULL,
    -- A metodologia exacta usada a validar, quando difere da original.
    methodology_version_id UUID REFERENCES methodology_versions (id) ON DELETE SET NULL,
    note              TEXT,
    performed_by_id   UUID REFERENCES people (id) ON DELETE SET NULL,
    created_at        TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT ck_result_validations_kind CHECK (kind IN (
        'validation', 'reproduction'
    )),
    CONSTRAINT ck_result_validations_outcome CHECK (outcome IN (
        'confirmed', 'contradicted', 'inconclusive'
    ))
);

CREATE INDEX ix_result_validations_result ON result_validations (result_id);

COMMENT ON TABLE result_validations IS
    'Reprodutibilidade é evidência, não um rótulo: um resultado só se diz '
    'reproduzido quando existe outra execução e alguém registou o que ela '
    'mostrou — incluindo quando mostrou o contrário.';

-- ── A proveniência cresce; não é duplicada ──────────────────────────────
--
-- `research_links` já era a aresta tipada certa: par (tipo, identificador) nas
-- duas pontas, vocabulário fechado, unicidade, sem auto-ligação. Não se cria
-- uma segunda tabela de proveniência — cria-se uma segunda fonte de verdade,
-- e duas fontes de verdade acabam sempre por discordar.
--
-- Duas coisas mudam.
--
-- **O ambiente deixa de ser obrigatório.** A linhagem institucional atravessa
-- ambientes: um resultado de um projecto sustenta o protótipo de outro. Com
-- `workspace_id NOT NULL` isso era inexprimível. Passa a ser onde a relação
-- foi declarada — e a autorização deixa de vir dele: vem de cada ponta,
-- resolvida na operação.
--
-- **O vocabulário cresce**, e cresce fechado. As sete relações antigas ficam
-- todas.
ALTER TABLE research_links ALTER COLUMN workspace_id DROP NOT NULL;

COMMENT ON COLUMN research_links.workspace_id IS
    'O ambiente onde a relação foi declarada, quando houve um. Não é a fonte '
    'da autorização: essa vem de cada ponta, resolvida na operação — porque '
    'autorizar o ambiente nunca foi autorizar o que se liga a partir dele.';

ALTER TABLE research_links DROP CONSTRAINT ck_research_links_relation;
ALTER TABLE research_links ADD CONSTRAINT ck_research_links_relation CHECK (relation IN (
    -- As sete originais.
    'cites', 'supports', 'refutes', 'derived_from', 'uses', 'produces', 'relates_to',
    -- O ciclo científico.
    'tests',            -- um estudo testa uma hipótese
    'follows',          -- um estudo segue uma versão de metodologia
    'input_to',         -- uma versão de dataset entra numa execução
    'produced_by',      -- um resultado foi produzido por uma execução
    'executed_on',      -- uma execução correu num nó de computação
    'validates',        -- uma validação sustenta um resultado
    'reproduces',       -- uma execução reproduz outra
    'supersedes'        -- uma versão substitui a anterior
));

-- Quem declarou a aresta, e como.
--
-- Uma relação inferida por um modelo e uma relação afirmada por uma pessoa não
-- são a mesma coisa, e a memória institucional tem de as distinguir. A sugestão
-- de um modelo não é proveniência: `origin` diz de onde veio a afirmação.
ALTER TABLE research_links ADD COLUMN origin VARCHAR(24) NOT NULL DEFAULT 'declared';
ALTER TABLE research_links ADD CONSTRAINT ck_research_links_origin CHECK (origin IN (
    -- Alguém afirmou a relação explicitamente.
    'declared',
    -- A própria operação do Core conhecia a relação sem ambiguidade: criar um
    -- resultado a partir de uma execução **é** a relação.
    'operation'
));

COMMENT ON COLUMN research_links.origin IS
    'De onde veio a afirmação. Uma sugestão de modelo não entra aqui: a saída '
    'de um modelo não se torna proveniência institucional por ter sido gerada.';
