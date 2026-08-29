-- Ficheiros institucionais: identidade que dura, bytes que mudam.
--
-- # O que faltava
--
-- O Ocinye guardava bytes desde a primeira semana. `storage_objects` responde
-- «onde estão os bytes e quais são», e responde bem: chave, tipo, tamanho,
-- soma, classificação. E `documents` responde «que documento é este», com
-- título, espécie e data.
--
-- Faltava o meio. Um `document` aponta para **um** `storage_object`, e
-- substituir o ficheiro obriga a escolher entre destruir o anterior ou criar
-- um documento novo. É assim que nascem os
--
--     relatorio.pdf
--     relatorio-final.pdf
--     relatorio-final-agora-sim.pdf
--
-- que toda a gente reconhece e ninguém consegue citar.
--
-- # As quatro perguntas, e quem responde a cada uma
--
--     StorageObject   onde estão os bytes, e quais são
--     File            que ficheiro institucional é este, ao longo do tempo
--     FileVersion     quais eram exactamente os bytes desta versão
--     Document        que este ficheiro tem leitura documental no Conhecimento
--
-- `Document` deixa de ser o recipiente universal de bytes. Uma fotografia de
-- uma montagem experimental é um ficheiro institucional legítimo e **não** é
-- um documento de conhecimento; forçá-la a sê-lo faria o módulo `knowledge`
-- passar a representar imagens, desenhos técnicos e binários que não têm
-- semântica documental nenhuma.
--
-- # Porque não se chama `DocumentVersion`
--
-- Porque a versão é material, e não semântica. Se a versão pertencesse ao
-- documento, uma imagem sem documento precisaria de `ImageVersion`, um anexo
-- de `AttachmentVersion`, e cada leitura institucional nova traria consigo mais
-- uma tabela de versões a dizer a mesma coisa.
--
-- A versão pertence ao **ficheiro**. O domínio por cima fica estável enquanto
-- os bytes mudam.
--
-- # Porque `dataset_versions` não serve, apesar de parecer
--
-- `dataset_versions` tem `sequence`, `published_at`, `withdrawn_reason` e
-- `derived_from_version_id`. É versionamento a sério — **de publicação de
-- dataset**. Um ficheiro institucional não se publica nem se retira com
-- motivo; tem versões e pronto.
--
-- Reutiliza-se a primitiva de bytes, que é comum. Não se reutiliza a semântica
-- de domínio, que não é. As duas árvores partilham a raiz e mais nada:
--
--     storage_objects
--        ├── dataset_files    → dataset_versions → datasets
--        └── file_versions    → files            → documents (opcional)
--
-- # O que esta migration deliberadamente **não** faz
--
-- Não toca no ciclo de vida de `storage_objects`. O `pending` do seu
-- check-constraint continua sem uso, e está certo: o carregamento síncrono
-- actual insere a linha e escreve os bytes dentro da mesma transacção, pelo
-- que nenhum leitor vê `stored` sem bytes por trás. O `pending` terá janela no
-- dia em que existir carregamento em duas fases — pedir destino, enviar,
-- confirmar — e essa é outra decisão.
--
-- Não remove `documents.storage_object_id`. Duas fontes convivem enquanto a
-- leitura migra, e um teste conta quantas divergem — que tem de ser zero.
--
-- Não move um único byte. O `StorageObject` de cada documento continua a ser
-- exactamente o mesmo, com o mesmo identificador.

-- ── O ficheiro ──────────────────────────────────────────────────────────
--
-- A identidade que sobrevive às versões. Mudar de nome, de pasta ou de
-- conteúdo não a altera — é isso que a torna citável.
--
-- Sem `current_version_id`: a versão actual é a de maior `sequence`. Uma
-- coluna a apontar para a versão corrente seria uma segunda fonte da mesma
-- verdade, e as duas discordariam no primeiro caminho de escrita que
-- esquecesse de a actualizar. Quando existir estado real de versão — rascunho,
-- publicada, retirada — a pergunta «qual é a corrente» deixa de ter resposta
-- óbvia e ganha o direito a uma coluna.
CREATE TABLE files (
    id                UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    organisation_id   UUID NOT NULL,
    -- O contexto institucional onde o ficheiro vive. Igual ao de `documents`,
    -- porque é o mesmo contexto: enquanto a governação não mudar de sítio,
    -- duplicá-la aqui seria criar uma segunda autoridade.
    unit_id           UUID NOT NULL REFERENCES units (id) ON DELETE RESTRICT,
    workspace_id      UUID NOT NULL REFERENCES research_workspaces (id) ON DELETE CASCADE,
    -- O nome que a pessoa vê. Não é o caminho, não é a chave, e não é
    -- identidade: muda sem consequências.
    name              VARCHAR(255) NOT NULL,
    created_by_id     UUID REFERENCES people (id) ON DELETE SET NULL,
    created_at        TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at        TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX ix_files_workspace ON files (workspace_id, lower(name));
CREATE INDEX ix_files_organisation ON files (organisation_id);

-- ── A versão ────────────────────────────────────────────────────────────
--
-- Imutável depois de escrita. Uma versão nova é uma linha nova; nunca um
-- `UPDATE` que troque o objecto por baixo de uma referência que já existe.
--
-- É esta a identidade que a proveniência científica vai querer apontar: um
-- resultado sustentado por «a versão 2 do relatório» continua a dizer a versão
-- 2 depois de existirem sete.
CREATE TABLE file_versions (
    id                UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    file_id           UUID NOT NULL REFERENCES files (id) ON DELETE CASCADE,
    -- Começa em 1 e cresce. A corrente é a maior.
    sequence          INTEGER NOT NULL,
    -- `RESTRICT` e não `CASCADE`: apagar os bytes por baixo de uma versão que
    -- alguém cita é a perda silenciosa que este modelo existe para impedir. A
    -- base recusa, e quem quiser mesmo apagar tem de tratar da referência
    -- primeiro.
    storage_object_id UUID NOT NULL REFERENCES storage_objects (id) ON DELETE RESTRICT,
    -- Notas de quem carregou esta versão: «corrigido o gráfico da página 4».
    -- Opcional, porque obrigar a justificar cada carregamento faz com que se
    -- escreva «actualização» para sempre.
    note              TEXT,
    created_by_id     UUID REFERENCES people (id) ON DELETE SET NULL,
    created_at        TIMESTAMPTZ NOT NULL DEFAULT now(),
    -- Duas versões com o mesmo número no mesmo ficheiro tornariam «a corrente»
    -- ambígua, e é a base que o impede — não a boa vontade de quem escreve.
    CONSTRAINT uq_file_versions_sequence UNIQUE (file_id, sequence),
    -- Um mesmo objecto não pode estar em duas versões do mesmo ficheiro: seria
    -- uma versão que não mudou nada a dizer que mudou.
    CONSTRAINT uq_file_versions_object UNIQUE (file_id, storage_object_id),
    CONSTRAINT ck_file_versions_sequence CHECK (sequence >= 1)
);

CREATE INDEX ix_file_versions_file ON file_versions (file_id, sequence DESC);
CREATE INDEX ix_file_versions_object ON file_versions (storage_object_id);

-- ── O documento passa a interpretar um ficheiro ─────────────────────────
--
-- `storage_object_id` fica. Não por indecisão: fica porque há leitores que
-- ainda dependem dela, e retirá-la no mesmo movimento em que se acrescenta a
-- nova faria a migração e a mudança de leitura acontecerem juntas, sem forma
-- de saber qual delas partiu alguma coisa.
--
-- Enquanto as duas existirem, um teste exige que **concordem**: o objecto que
-- o documento aponta directamente tem de ser o mesmo que a sua versão corrente
-- aponta. Duas fontes da mesma verdade só são aceitáveis enquanto alguém as
-- confronta.
ALTER TABLE documents ADD COLUMN file_id UUID REFERENCES files (id) ON DELETE RESTRICT;

-- ── O preenchimento ─────────────────────────────────────────────────────
--
-- Cria estrutura sobre história que já existe. **Não cria história**: os
-- identificadores dos documentos e dos objectos não mudam, nenhum byte é
-- reescrito, e as datas saem do que já lá estava.
--
-- Um ficheiro que existe desde Março tem de continuar a dizer Março. Se o
-- `created_at` viesse do relógio da migração, a instituição inteira passaria a
-- ter nascido no mesmo segundo — e o primeiro a reparar seria alguém a tentar
-- perceber uma ordem cronológica daqui a dois anos.
INSERT INTO files (id, organisation_id, unit_id, workspace_id, name,
                   created_by_id, created_at, updated_at)
SELECT gen_random_uuid(), d.organisation_id, d.unit_id, d.workspace_id,
       -- O nome do ficheiro é o do objecto, que é o que a pessoa carregou. O
       -- título do documento é outra coisa e continua no documento.
       o.original_filename,
       d.created_by_id, d.created_at, d.updated_at
  FROM documents d
  JOIN storage_objects o ON o.id = d.storage_object_id
 WHERE d.file_id IS NULL;

-- A ligação faz-se pelo par (documento, objecto), que é único: cada documento
-- tinha exactamente um objecto.
UPDATE documents d
   SET file_id = f.id
  FROM files f, storage_objects o
 WHERE d.file_id IS NULL
   AND o.id = d.storage_object_id
   AND f.workspace_id = d.workspace_id
   AND f.name = o.original_filename
   AND f.created_at = d.created_at
   AND NOT EXISTS (SELECT 1 FROM documents d2 WHERE d2.file_id = f.id);

INSERT INTO file_versions (file_id, sequence, storage_object_id, created_by_id, created_at)
SELECT d.file_id, 1, d.storage_object_id, o.created_by_id, o.created_at
  FROM documents d
  JOIN storage_objects o ON o.id = d.storage_object_id
 WHERE d.file_id IS NOT NULL
   AND NOT EXISTS (SELECT 1 FROM file_versions v WHERE v.file_id = d.file_id);

-- Depois do preenchimento, nenhum documento pode ficar sem ficheiro. Sem esta
-- linha, um documento que a junção não alcançasse passaria despercebido até
-- alguém o tentar abrir.
DO $$
DECLARE orfaos INTEGER;
BEGIN
    SELECT count(*) INTO orfaos FROM documents WHERE file_id IS NULL;
    IF orfaos > 0 THEN
        RAISE EXCEPTION 'a migração deixou % documento(s) sem ficheiro', orfaos;
    END IF;
END $$;
