-- Ocinye OS — as unidades institucionais iniciais, e o índice de pesquisa delas.
--
-- # Porque isto é seed, e não um enum
--
-- Uma instituição de investigação não começa como um formulário genérico vazio:
-- começa com unidades sensatas. Estas quatro são **dados** — linhas — e não uma
-- enumeração em que a lógica de negócio ramifica. Nada no sistema faz
-- `if code = 'UAI-001'`, e nada deve (§2, §7). Uma instalação renomeia-as,
-- arquiva-as, ou acrescenta às suas; uma unidade criada depois, que não existia
-- quando o Ocinye foi compilado, é tão de primeira classe como estas.
--
-- Os códigos `-001` explícitos são os identificadores escolhidos pela
-- instituição, e não o que o gerador produziria (ele abreviaria «Inteligência
-- Artificial» para `UIA`, não `UAI`). O gerador serve as unidades seguintes.
--
-- # Idempotência
--
-- `ON CONFLICT (organisation_id, code) DO NOTHING`: uma instalação que já tenha
-- estes códigos não é tocada. Uma unidade semeada não tem autor humano
-- (`created_by_id` nulo) nem gestor — um administrador da organização adopta-a e
-- nomeia quem a lidera.
--
-- Esta migração semeia as **organizações que já existem** (a produção, com a sua
-- única organização). Uma instalação nova não tem organização nenhuma no momento
-- em que esta migração corre; essa é semeada pelo `bootstrap-admin`, que chama o
-- mesmo caminho idempotente depois de criar a organização.

INSERT INTO units (organisation_id, code, name, description, research_areas)
SELECT o.id, v.code, v.name, v.description, v.research_areas
  FROM organisations o
  CROSS JOIN (
      VALUES
        (
          'UAI-001',
          'Inteligência Artificial',
          'Investigação e engenharia de sistemas de inteligência artificial.',
          ARRAY['Aprendizagem automática', 'Processamento de linguagem natural', 'Visão computacional']::text[]
        ),
        (
          'UCS-001',
          'Computação e Sistemas',
          'Sistemas computacionais, infraestrutura e engenharia de software.',
          ARRAY['Sistemas distribuídos', 'Computação de alto desempenho', 'Engenharia de software']::text[]
        ),
        (
          'UDC-001',
          'Dados e Conhecimento',
          'Ciência de dados, gestão de conhecimento e recuperação de informação.',
          ARRAY['Ciência de dados', 'Bases de dados', 'Recuperação de informação']::text[]
        ),
        (
          'UID-001',
          'Investigação e Desenvolvimento',
          'Investigação aplicada e transferência tecnológica transversais.',
          ARRAY['Investigação aplicada', 'Transferência tecnológica', 'Prototipagem']::text[]
        )
  ) AS v(code, name, description, research_areas)
ON CONFLICT (organisation_id, code) DO NOTHING;

-- Índice de pesquisa das unidades.
--
-- Antes desta fatia, as unidades não eram indexadas. Este backfill cobre tanto
-- as recém-semeadas como quaisquer unidades criadas à mão antes de a indexação
-- existir. Uma unidade é encontrada por nome e por código; as áreas de
-- investigação entram no vector para uma unidade surgir pela área em que
-- trabalha. A linha é `INTERNAL` e sem âmbito de dono/unidade/ambiente — a sua
-- existência é visível a todo o membro activo, exactamente como `list_units`.
--
-- Só unidades activas: uma unidade arquivada não é estrutura corrente e sai do
-- índice. A construção do vector e do excerto espelha o caminho de runtime
-- (`unit_index_text`): título = nome; texto = código, nome, descrição, áreas.
INSERT INTO search_documents
    (organisation_id, entity_type, entity_id, title, excerpt, classification,
     search_vector, indexed_at)
SELECT
    u.organisation_id,
    'unit',
    u.id,
    u.name,
    left(
        concat_ws(E'\n', u.code, u.name, u.description,
                  array_to_string(u.research_areas, E'\n')),
        400
    ),
    'INTERNAL',
    to_tsvector(
        'simple',
        concat_ws(E'\n', u.name, u.code, u.name, u.description,
                  array_to_string(u.research_areas, E'\n'))
    ),
    now()
  FROM units u
 WHERE u.status = 'active'
   AND NOT EXISTS (
       SELECT 1 FROM search_documents sd
        WHERE sd.entity_type = 'unit' AND sd.entity_id = u.id
   )
ON CONFLICT (entity_type, entity_id) DO NOTHING;
