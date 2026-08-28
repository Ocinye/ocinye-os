# Pesquisa

Decisão: [ADR-0202](../adrs/0202-search-fts-pgvector.md).

## O invariante

> **A pesquisa não é uma via para contornar permissões.**

O predicado de autorização faz **parte da query**. `LIMIT`, `OFFSET` e `COUNT`
operam apenas sobre o conjunto autorizado.

Isto inclui contagens, facetas, sugestões e autocomplete: nenhum deles pode
revelar a existência de um artefacto que quem pesquisa não pode ver. Um total que
incluísse linhas escondidas seria, ele próprio, uma divulgação.

Coberto por teste contra base de dados real, que verifica também o inverso — que
um membro do workspace **vê** o artefacto — para não passar só porque nada
correspondeu.

## Camadas

| Camada | Estado |
|---|---|
| PostgreSQL Full Text Search | `CURRENT` |
| pgvector, coluna preparada | `CURRENT` — vazia |
| Pesquisa semântica | **Indisponível** — sem embeddings, porque sem nó de IA |
| Híbrido lexical + semântico | `PLANNED` |

`GET /api/v1/search/semantic-availability` reporta o estado verdadeiro em vez de
devolver resultados lexicais rotulados como semânticos.

## O índice

Uma linha por objecto indexável em `search_documents`, transportando o contexto
de autorização (organização, unidade, workspace, classificação) **ao lado** do
texto.

A indexação acontece **dentro da transacção que a originou**, para que o índice
nunca descreva um artefacto que foi revertido.

O excerto guardado é limitado a 400 caracteres: o índice é um instrumento de
descoberta, não uma segunda cópia do corpus.

### O que está indexado, e o que não está

| Indexado | Não indexado |
|---|---|
| `idea` · `project` · `note` · `source` · `document` · `dataset` | Tarefas · comentários · actividade · pessoas · unidades · correio |

As ausências são deliberadas, não pendentes.

- **Tarefas** respondem a uma pergunta sobre um ambiente, não sobre o acervo;
  `collaboration.task.list` serve-as, com a política de quem pergunta aplicada.
- **Correio** tem a sua própria pesquisa, dentro da fronteira de privacidade do
  módulo: a caixa de uma pessoa não entra no índice institucional
  ([ADR-0404](../adrs/0404-mail-privacy-boundary.md)).
- **Pessoas e unidades** têm listagens próprias, já autorizadas.

De um **documento**, só título e descrição são indexados. Extrair o corpo é uma
decisão separada e explicitamente autorizada.

### Pesquisar não é perguntar

Duas operações distintas, e a diferença importa mais do que parece:

| | Pesquisar | Perguntar |
|---|---|---|
| O que faz | Encontra artefactos | Sintetiza a partir deles |
| Precisa de modelo | **Não** | Sim |
| Funciona hoje | **Sim** | Não — zero nós de IA |
| Devolve | Referências e excertos | Uma resposta, com as fontes que a informaram |

É esta separação que torna o Ocinye OS *AI-native* sem ser *AI-dependent*:
`knowledge.search` é uma capability de risco zero que não toca no Intelligence
Plane, e é a razão de a superfície de comando responder nesta instalação.

## Configuração `simple`, sem stemming

O corpus é bilingue: conteúdo em português, terminologia em inglês. Um stemmer de
uma só língua degradaria a outra. É um compromisso consciente que custa recall.

## Quando existirem embeddings

A coluna `vector(1024)` já existe. Falta produzir embeddings e acrescentar um
índice ANN por migration — o tipo de índice depende do modelo, que não existe.

Não foi criado um índice vazio: não ganharia nada e fixaria uma escolha antes de
haver informação para a fazer.

## O ecrã de pesquisa

`GET /search` no Ocinye Workspace, acrescentado na auditoria de 2026-08-22.

Até lá o Core servia `/api/v1/search` e **não havia por onde lá chegar**: a caixa
«Pesquisar no Ocinye…» da barra superior abria a command palette, que filtra
navegação localmente e não procura em nada. Um endpoint implementado e
inalcançável de um lado, uma promessa por cumprir do outro.

Agora a caixa é uma ligação para `/search`, e o `⌘K` continua a abrir a palette.
São duas coisas distintas, e cada uma faz o que anuncia.

### O que o ecrã mostra

- Cada resultado com a sua **classificação visível**: um artefacto `RESTRICTED`
  não pode parecer igual a um `PUBLIC`.
- O tipo de entidade, traduzido; um tipo desconhecido é mostrado tal como veio,
  nunca escondido — esconder resultados que o Core devolveu seria filtrar depois.
- Um destino apenas quando o Workspace tem ecrã para ele. Sem ecrã, o resultado
  não finge uma ligação.

### Pesquisa semântica

Declarada, não escondida. Sem capacidade de embeddings o modo aparece como
«Semântica — ainda não disponível», com a razão do Core no `title`, e **não é
seleccionável**. Estado: `NO_RESOURCE`, dependente de `ai.embedding`.

### Sem termo não se pesquisa

Uma consulta vazia devolveria tudo o que o membro pode ver, o que não é uma
pesquisa e custa uma varredura. O ecrã convida a escrever, e distingue isso de
«não encontrei nada».
