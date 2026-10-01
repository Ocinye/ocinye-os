-- ---------------------------------------------------------------------------
-- D010 · Claude Design; integrado e corrigido pela Code (ver os blocos «Code»).
-- 0060 · Distribuições activadas por Instância (substitui organisations.profile como verdade)
-- ---------------------------------------------------------------------------
--
-- Antes: organisations.profile = um valor. Depois: uma linha por Distribuição que a Instância
-- activou alguma vez; `state` diz se está activada. Desactivar nunca apaga a linha.
-- A migração é determinística: cada Instância recebe exactamente a sua Distribuição actual.
-- Nenhuma Instância ganha as quatro.
--
-- Compatibilidade de rolagem: organisations.profile FICA (só leitura para o código novo) durante
-- uma versão, para que um binário anterior continue a arrancar. O código novo mantém-no igual à
-- Distribuição activada mais antiga, e 0064 (versão seguinte) retira-o. Reverter o esquema
-- depois de activar uma segunda Distribuição perde essa activação: não é reversível sem perda,
-- e diz-se.

CREATE TABLE instance_distributions (
    organisation_id UUID NOT NULL REFERENCES organisations (id) ON DELETE CASCADE,
    distribution    VARCHAR(16) NOT NULL
        CHECK (distribution IN ('research', 'business', 'personal', 'education')),
    state           VARCHAR(16) NOT NULL CHECK (state IN ('enabled', 'disabled')),
    enabled_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    enabled_by_id   UUID REFERENCES people (id) ON DELETE SET NULL,
    disabled_at     TIMESTAMPTZ,
    disabled_by_id  UUID REFERENCES people (id) ON DELETE SET NULL,
    PRIMARY KEY (organisation_id, distribution),
    CHECK ((state = 'disabled') = (disabled_at IS NOT NULL))
);

INSERT INTO instance_distributions (organisation_id, distribution, state)
SELECT id, profile, 'enabled' FROM organisations;

-- A invariante «pelo menos uma activada» é do Core (EnabledDistributions::new), com um guarda
-- de base de dados para escritas fora do caminho do produto:
CREATE FUNCTION instance_distributions_keep_one() RETURNS trigger AS $$
BEGIN
    -- Code: a organização a ser apagada leva as suas linhas em cascata; isso
    -- não é «ficar sem Distribuição», é deixar de haver Instância.
    IF NOT EXISTS (SELECT 1 FROM organisations
                   WHERE id = COALESCE(NEW.organisation_id, OLD.organisation_id)) THEN
        RETURN NULL;
    END IF;
    IF NOT EXISTS (SELECT 1 FROM instance_distributions
                   WHERE organisation_id = COALESCE(NEW.organisation_id, OLD.organisation_id)
                     AND state = 'enabled') THEN
        RAISE EXCEPTION 'instance must keep at least one enabled distribution';
    END IF;
    RETURN NULL;
END $$ LANGUAGE plpgsql;

CREATE CONSTRAINT TRIGGER tr_instance_distributions_keep_one
    AFTER UPDATE OR DELETE ON instance_distributions
    DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION instance_distributions_keep_one();

COMMENT ON TABLE instance_distributions IS
    'Distribuições activadas por Instância (ADR-0019). Não é autorização. Desactivar guarda o estado.';

-- ---------------------------------------------------------------------------
-- Acesso de cada membro a cada Distribuição (C2). Não é um papel (ADR-0019 §4).
-- ---------------------------------------------------------------------------
-- Migração: todo o membro existente recebe acesso à Distribuição que a Instância já tinha —
-- é o comportamento de hoje. Activar outra Distribuição mais tarde não dá acesso a ninguém
-- além de quem a activou; o resto é concedido explicitamente e auditado.

-- Code: o membro pertence à organização da linha — garantido pela base, não
-- só pela migração (sem acesso de uma pessoa a outra Instância).
ALTER TABLE people ADD CONSTRAINT uq_people_id_organisation UNIQUE (id, organisation_id);

CREATE TABLE member_distribution_access (
    organisation_id UUID NOT NULL,
    person_id       UUID NOT NULL,
    distribution    VARCHAR(16) NOT NULL,
    granted_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    granted_by_id   UUID REFERENCES people (id) ON DELETE SET NULL,
    PRIMARY KEY (organisation_id, person_id, distribution),
    FOREIGN KEY (organisation_id, distribution)
        REFERENCES instance_distributions (organisation_id, distribution),
    FOREIGN KEY (person_id, organisation_id)
        REFERENCES people (id, organisation_id) ON DELETE CASCADE
);

-- Fase B (main b87d26f): a tabela `organisations` PODE ter várias linhas (0052; resolve_instance
-- passos 3–4). O que é singular é a Instância (instance_identity, 0052). Membro da Instância =
-- linha de `people` com people.organisation_id = organização da Instância (0001, NOT NULL).
-- Por isso cada membro recebe a Distribuição da SUA organização, por junção explícita — sem
-- contar organizações e sem produto cartesiano. Exacto com uma ou várias organizações.
INSERT INTO member_distribution_access (organisation_id, person_id, distribution)
SELECT p.organisation_id, p.id, o.profile
  FROM people p JOIN organisations o ON o.id = p.organisation_id;
-- Estado (invited/active/suspended/departed) não entra: hoje o acesso à única Distribuição não
-- depende dele; a entrada continua a ser decidida pela sessão/estado, como antes.

COMMENT ON TABLE member_distribution_access IS
    'Que Distribuições activadas cada membro pode abrir (ADR-0019). Não é um papel nem uma permissão.';

-- ---------------------------------------------------------------------------
-- Code (D010): os caminhos que criam linhas fora do produto
-- ---------------------------------------------------------------------------
-- Uma organização nova nasce com a Distribuição do seu `profile` activada —
-- a mesma regra da migração, para o `bootstrap-admin`, o arranque do Core e
-- as fixtures que inserem directamente. Sem isto, uma Instância nova ficava
-- sem Distribuição nenhuma.
CREATE FUNCTION organisations_enable_profile() RETURNS trigger AS $$
BEGIN
    INSERT INTO instance_distributions (organisation_id, distribution, state)
    VALUES (NEW.id, NEW.profile, 'enabled')
    ON CONFLICT DO NOTHING;
    RETURN NULL;
END $$ LANGUAGE plpgsql;

CREATE TRIGGER tr_organisations_enable_profile
    AFTER INSERT ON organisations
    FOR EACH ROW EXECUTE FUNCTION organisations_enable_profile();

-- Um membro novo de uma Instância com **uma só** Distribuição activada recebe
-- acesso a ela: é o comportamento de hoje, e não há escolha a fazer. Com
-- várias activadas não recebe nenhuma — o acesso concede-se explicitamente
-- (Administração › Acesso a Distribuições), nunca «todas por omissão».
CREATE FUNCTION people_grant_single_distribution() RETURNS trigger AS $$
DECLARE
    unica VARCHAR(16);
BEGIN
    SELECT min(distribution) INTO unica FROM instance_distributions
     WHERE organisation_id = NEW.organisation_id AND state = 'enabled'
    HAVING count(*) = 1;
    IF unica IS NOT NULL THEN
        INSERT INTO member_distribution_access (organisation_id, person_id, distribution)
        VALUES (NEW.organisation_id, NEW.id, unica)
        ON CONFLICT DO NOTHING;
    END IF;
    RETURN NULL;
END $$ LANGUAGE plpgsql;

CREATE TRIGGER tr_people_grant_single_distribution
    AFTER INSERT ON people
    FOR EACH ROW EXECUTE FUNCTION people_grant_single_distribution();
