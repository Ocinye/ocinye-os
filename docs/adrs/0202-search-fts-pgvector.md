# ADR-0202 — Pesquisa: PostgreSQL FTS agora, pgvector preparado

- **Estado:** Accepted
- **Domínio:** Knowledge
- **Impacto:** MEDIUM
- **Data:** 2026-08-22

## Context

A pesquisa institucional tem de ser útil desde o primeiro dia e evoluir para
semântica quando existirem embeddings. Hoje não existe nó de IA, logo não
existem embeddings.

O invariante que domina esta decisão não é de desempenho: **a pesquisa não pode
ser um caminho para contornar permissões** (`CLAUDE.md` §28).

## Decision

Índice materializado próprio, `search_documents`, com uma linha por objecto
indexável, transportando o contexto de autorização (organização, unidade,
workspace, classificação) **ao lado** do texto.

Camadas:

1. **Agora:** PostgreSQL Full Text Search, `tsvector` com índice GIN.
2. **Preparado:** coluna `embedding vector(1024)` do pgvector, nullable e vazia.
   Sem nó de IA não há embeddings — e a pesquisa semântica é reportada como
   indisponível, não simulada.
3. **Futuro:** híbrido lexical + semântico com re-ranking.

Regras invioláveis:

- O predicado de autorização faz parte da query. `LIMIT`, `OFFSET` e `COUNT`
  operam **apenas** sobre o conjunto autorizado.
- Contagens, facetas, sugestões e autocomplete estão sujeitos ao mesmo filtro:
  nenhum deles pode revelar a existência de um artefacto não autorizado.
- O excerto guardado é **limitado**. O índice é um instrumento de descoberta, não
  uma segunda cópia do corpus.
- Corpos de documentos não são extraídos para o índice sem decisão explícita e
  separadamente autorizada.
- Configuração de texto `simple`, sem stemming: o corpus é bilingue (conteúdo em
  português, terminologia em inglês) e um stemmer de uma só língua degradaria a
  outra.

## Alternatives

| Alternativa | Porque foi rejeitada |
|---|---|
| **Elasticsearch / OpenSearch** | Melhor relevância, mas duplica dados sensíveis num segundo sistema com o seu próprio modelo de segurança, e acrescenta operação significativa. Reavaliável quando o corpus o justificar. |
| **Meilisearch / Typesense** | Ergonomia excelente; mesma objecção de duplicação de dados classificados fora da fonte canónica. |
| **Pesquisar tudo e filtrar na aplicação** | Explicitamente proibido: totais e paginação passariam a revelar o que a política nega. |
| **`tsvector` calculado nas tabelas de origem** | Espalharia lógica de pesquisa por todos os domínios e dificultaria aplicar um único filtro de autorização. |

## Consequences

**Positivas** — um único sistema com os dados; autorização aplicada em SQL;
pgvector adicionável sem remodelar o índice.

**Negativas, aceites** — a relevância do FTS do PostgreSQL é inferior à de um
motor dedicado; a ausência de stemming penaliza recall; o índice tem de ser
mantido sincronizado, o que é responsabilidade do worker e é testado.

## Referências

`CLAUDE.md` §28 · briefing §47, §48 · ADR-0009 · ADR-0300
