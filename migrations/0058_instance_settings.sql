-- Ocinye OS — configuração e marca da Instância (Parte 14, ADR-0017).
--
-- O que distingue duas Instâncias do mesmo release, sem fork: a língua por
-- omissão, o fuso horário, as aplicações fixadas por omissão para quem nunca
-- escolheu, e o logótipo. O nome e o perfil já viviam na organização e em
-- `instance_applications`; isto completa o conjunto.
--
-- Nada aqui mexe em autorização. Uma marca não muda o que o produto faz: o
-- Ocinye OS continua identificável como a plataforma.

CREATE TABLE instance_settings (
    organisation_id  UUID PRIMARY KEY REFERENCES organisations (id) ON DELETE CASCADE,
    default_locale   VARCHAR(2) NOT NULL DEFAULT 'pt',
    timezone         VARCHAR(64) NOT NULL DEFAULT 'UTC',
    -- Aplicações fixadas por omissão, para membros que nunca escolheram. NULL
    -- é «o conjunto do produto»; uma lista, mesmo vazia, é a da Instância.
    default_pins     TEXT[],
    -- O logótipo é um objecto governado, e não um URL: nada de CSS remoto.
    logo_object_id   UUID REFERENCES storage_objects (id) ON DELETE SET NULL,
    updated_by_id    UUID REFERENCES people (id) ON DELETE SET NULL,
    updated_at       TIMESTAMPTZ NOT NULL DEFAULT now(),

    CONSTRAINT ck_instance_settings_locale CHECK (default_locale IN ('pt', 'en', 'fr'))
);

COMMENT ON TABLE instance_settings IS
    'Per-Instance configuration and branding (ADR-0017). Absent row = product defaults.';
