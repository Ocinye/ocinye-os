-- Carregamento em partes: a sessão que atravessa a fronteira do edge.
--
-- # Porque isto existe
--
-- O Ocinye Files aceita ficheiros até 512 MiB. A Cloudflare, no plano em que a
-- instituição está, recusa pedidos proxied acima de ~100 MB. Sem partes, ou se
-- reduzia silenciosamente a capacidade institucional para caber no limite de um
-- fornecedor de edge, ou se tirava `api.ocinye.com` de trás da Cloudflare — e
-- nenhuma das duas é uma decisão que um limite de infraestrutura deva tomar.
--
-- # O que uma sessão **não** é
--
-- Não é um ficheiro. Enquanto a sessão está aberta não existe `File` nem
-- `FileVersion`: existe uma intenção autorizada e bytes por montar. Um
-- carregamento interrompido não deixa meio ficheiro na instituição — deixa uma
-- sessão que expira.

CREATE TABLE upload_sessions (
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    organisation_id     UUID NOT NULL REFERENCES organisations(id),
    workspace_id        UUID NOT NULL REFERENCES research_workspaces(id),

    -- Presente quando o carregamento é uma **nova versão** de um ficheiro que
    -- já existe. Ausente quando é um ficheiro novo.
    file_id             UUID REFERENCES files(id),
    folder_id           UUID REFERENCES folders(id),

    filename            TEXT NOT NULL,
    content_type        TEXT NOT NULL,
    classification      TEXT,

    -- O tamanho que quem carrega declarou, e o tamanho de cada pedaço. Os dois
    -- são fixados na abertura: mudá-los a meio permitiria contornar o limite
    -- que foi autorizado.
    declared_size_bytes BIGINT NOT NULL CHECK (declared_size_bytes > 0),
    chunk_size_bytes    INTEGER NOT NULL CHECK (chunk_size_bytes > 0),
    total_parts         INTEGER NOT NULL CHECK (total_parts > 0),

    -- O destino no armazenamento, escolhido pelo Core. Quem carrega nunca o vê:
    -- o browser fala com o Core, e o Core fala com o armazenamento.
    storage_object_id   UUID NOT NULL,
    storage_key         TEXT NOT NULL,
    storage_upload_id   TEXT NOT NULL,

    created_by_id       UUID NOT NULL REFERENCES people(id),
    created_at          TIMESTAMPTZ NOT NULL DEFAULT now(),
    expires_at          TIMESTAMPTZ NOT NULL,

    state               TEXT NOT NULL DEFAULT 'open',
    finalised_at        TIMESTAMPTZ,

    CONSTRAINT ck_upload_sessions_state
        CHECK (state IN ('open', 'finalised', 'abandoned')),

    -- Uma sessão finalizada tem de dizer quando. Uma aberta não pode dizer.
    CONSTRAINT ck_upload_sessions_finalised
        CHECK ((state = 'finalised') = (finalised_at IS NOT NULL))
);

CREATE INDEX ix_upload_sessions_expiry ON upload_sessions (state, expires_at);
CREATE INDEX ix_upload_sessions_actor ON upload_sessions (created_by_id, state);

-- As partes que já chegaram.
--
-- A chave primária composta é o que torna o carregamento **idempotente**: um
-- pedaço reenviado — porque a rede caiu a meio da resposta, e quem carrega não
-- sabe se chegou — encontra a linha que já lá está em vez de escrever uma
-- segunda.
CREATE TABLE upload_parts (
    session_id   UUID NOT NULL REFERENCES upload_sessions(id) ON DELETE CASCADE,
    part_number  INTEGER NOT NULL CHECK (part_number >= 1),
    size_bytes   INTEGER NOT NULL CHECK (size_bytes > 0),

    -- A soma do pedaço, verificada à chegada. Um pedaço corrompido é recusado
    -- quando chega, e não no fim: quem carrega repete um pedaço, e não meio
    -- gigabyte.
    sha256       TEXT NOT NULL,

    -- A etiqueta que o armazenamento devolveu. É com ela que o objecto se monta.
    etag         TEXT NOT NULL,
    received_at  TIMESTAMPTZ NOT NULL DEFAULT now(),

    PRIMARY KEY (session_id, part_number)
);
