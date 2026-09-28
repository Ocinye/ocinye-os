-- ---------------------------------------------------------------------------
-- A disposição do Desktop de cada membro (Claude Design D001, FG-017)
-- ---------------------------------------------------------------------------
--
-- O Desktop é uma grelha de widgets em que a ordem é a posição; o membro muda
-- a ordem, o tamanho, o fundo e o escurecimento. É uma preferência de
-- apresentação do próprio membro — não altera autorização nenhuma — e vive no
-- Core para o seguir entre browsers e máquinas, como as aplicações fixadas
-- (0051).
--
-- Sem linha, o membro nunca personalizou, e o Workspace desenha a predefinição
-- da distribuição da Instância. Repor a predefinição apaga a linha: o membro
-- volta a seguir a predefinição, e não uma cópia dela congelada no dia em que
-- repôs.
--
-- `version` é concorrência optimista: cada escrita diz a versão que leu, e o
-- Core recusa com 409 quando outra sessão gravou entretanto. Duas janelas
-- abertas não se sobrepõem em silêncio.
--
-- A forma da disposição (`layout`) é validada pelo Core antes de gravar, contra
-- `ocinye_contracts::desktop`: tipos conhecidos, tamanhos permitidos,
-- obrigatórios presentes. O JSON guarda a escolha, não a regra.

CREATE TABLE member_desktop_layouts (
    person_id     UUID PRIMARY KEY REFERENCES people (id) ON DELETE CASCADE,
    version       INTEGER NOT NULL CHECK (version > 0),
    layout        JSONB NOT NULL CHECK (jsonb_typeof(layout) = 'object'),
    updated_at    TIMESTAMPTZ NOT NULL DEFAULT now()
);

COMMENT ON TABLE member_desktop_layouts IS
    'Preferência de apresentação por membro: a disposição do Desktop (widgets, '
    'tamanhos, fundo, escurecimento). Não é autorização. Sem linha = segue a '
    'predefinição da distribuição.';
COMMENT ON COLUMN member_desktop_layouts.version IS
    'Concorrência optimista: cada PUT traz a versão lida; divergência = 409.';
