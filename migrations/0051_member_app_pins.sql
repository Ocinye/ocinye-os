-- ---------------------------------------------------------------------------
-- As aplicações que cada membro fixou na barra lateral
-- ---------------------------------------------------------------------------
--
-- A barra lateral do Ocinye OS deixou de ser o catálogo completo de aplicações:
-- é navegação essencial mais as aplicações que o membro **fixou**. A fixação é
-- uma preferência do membro — não altera autorização nenhuma —, e vive no Core
-- para que o mesmo conjunto o siga entre o portátil, outro browser e outra
-- máquina (briefing do Gestor de Aplicações §11).
--
-- Uma linha por membro, com a lista **ordenada** dos identificadores técnicos
-- das aplicações (`files`, `notes`, …) — os do registo do Workspace, opacos para
-- o Core, que aqui só os guarda e devolve. A ordem do array é a ordem na barra.
--
-- A ausência de linha e uma lista vazia são coisas **diferentes**: sem linha, o
-- membro nunca escolheu, e o Workspace aplica o conjunto por omissão; uma lista
-- vazia é uma escolha explícita — o membro tirou tudo — e respeita-se. Por isso
-- a coluna não tem default de conteúdo: quem escreve, escreve a escolha inteira.

CREATE TABLE member_app_pins (
    person_id        UUID PRIMARY KEY REFERENCES people (id) ON DELETE CASCADE,

    -- Os ids das aplicações fixadas, pela ordem em que aparecem na barra. São os
    -- identificadores técnicos estáveis do registo do Workspace; o Core não os
    -- interpreta. Um id que deixe de existir no registo é simplesmente ignorado
    -- pelo Workspace ao renderizar — guardá-lo não faz mal, e apagá-lo obrigaria
    -- o Core a conhecer o catálogo, que é do Workspace.
    pinned_app_ids   TEXT[] NOT NULL,

    updated_at       TIMESTAMPTZ NOT NULL DEFAULT now()
);

COMMENT ON TABLE member_app_pins IS
    'Preferência de apresentação por membro: as aplicações fixadas na barra '
    'lateral, ordenadas. Não é autorização — o Core continua a decidir o acesso.';
COMMENT ON COLUMN member_app_pins.pinned_app_ids IS
    'Ids técnicos do registo de aplicações do Workspace, pela ordem da barra. '
    'Sem linha = nunca escolheu (o Workspace aplica o conjunto por omissão); '
    'array vazio = escolheu não fixar nenhuma.';
