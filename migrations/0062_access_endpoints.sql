-- ---------------------------------------------------------------------------
-- D010 · Claude Design; integrado e corrigido pela Code (ver os blocos «Code»).
-- 0062 · Pontos de acesso (K1)
-- ---------------------------------------------------------------------------
--
-- Hoje: um OCINYE_WORKSPACE_PUBLIC_URL; qualquer Host chega à mesma Instância.
-- Depois: anfitrião → ponto de acesso → Instância (+ Distribuição opcional); desconhecido falha
-- fechado. A semente (M4) NÃO é SQL: o arranque do Workspace (ou `ocinye endpoint seed`) cria o
-- ponto canónico genérico a partir do anfitrião de OCINYE_WORKSPACE_PUBLIC_URL quando a tabela está
-- vazia, e regista-o no Registo de auditoria. Depois disso a variável só serve para essa semente.

CREATE TABLE access_endpoints (
    id              UUID PRIMARY KEY,
    organisation_id UUID NOT NULL REFERENCES organisations (id) ON DELETE CASCADE,
    hostname        VARCHAR(253) NOT NULL,
    binding_distribution VARCHAR(16),
    state           VARCHAR(16) NOT NULL CHECK (state IN ('active', 'disabled', 'unverified')),
    canonical       BOOLEAN NOT NULL DEFAULT false,
    dns_observation VARCHAR(24) NOT NULL DEFAULT 'not_observed'
        CHECK (dns_observation IN ('not_observed', 'resolves_here', 'resolves_elsewhere')),
    tls_observation VARCHAR(16) NOT NULL DEFAULT 'not_observed'
        CHECK (tls_observation IN ('not_observed', 'pending', 'valid', 'invalid')),
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    created_by_id   UUID REFERENCES people (id) ON DELETE SET NULL,
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_by_id   UUID REFERENCES people (id) ON DELETE SET NULL,
    -- normalizado pelo Core (Hostname::parse); a base de dados recusa o que escape
    CHECK (hostname = lower(hostname) AND hostname !~ '[/:*\s]' AND hostname NOT LIKE '%.'),
    CHECK (NOT canonical OR state = 'active'),
    FOREIGN KEY (organisation_id, binding_distribution)
        REFERENCES instance_distributions (organisation_id, distribution)
);

-- Um nome serve uma só Instância em todo o servidor (sem ambiguidade de destino).
CREATE UNIQUE INDEX ux_access_endpoints_hostname ON access_endpoints (hostname);
-- Exactamente um canónico por Instância.
CREATE UNIQUE INDEX ux_access_endpoints_canonical ON access_endpoints (organisation_id) WHERE canonical;

COMMENT ON TABLE access_endpoints IS
    'Pontos de acesso (ADR-0020). O anfitrião escolhe o destino; nunca concede autoridade.';
