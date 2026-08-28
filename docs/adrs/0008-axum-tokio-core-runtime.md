# ADR-0008 — Axum + Tokio para o Core Runtime

- **Estado:** Accepted
- **Domínio:** Foundation
- **Impacto:** MEDIUM
- **Data:** 2026-08-22

## Context

Sob o princípio Rust-first (ADR-0004), o Ocinye Core precisa de um servidor HTTP
para expor a API versionada. Requisitos: middleware componível (correlação,
cabeçalhos de segurança, limites de corpo), extractores tipados, integração
natural com `tower`, e maturidade suficiente para uma base institucional.

## Decision

**Axum** sobre **Tokio**, com `tower` e `tower-http` para middleware.

- Estado partilhado da aplicação injectado via `State`.
- Autorização em extractores e serviços, nunca em middleware genérico: uma
  decisão de autorização precisa do contexto do recurso, que só o serviço tem.
- Erros convertidos num envelope único (`ocinye-contracts::ErrorBody`) por uma
  implementação de `IntoResponse`, para que nenhuma rota invente o seu formato.

## Alternatives

| Alternativa | Porque foi rejeitada |
|---|---|
| **Actix Web** | Desempenho excelente e maturidade comparável. Modelo de actores e ergonomia de estado menos alinhados com o resto do ecossistema `tower`, que também serve o cliente HTTP e o Workspace. |
| **Rocket** | Ergonomia agradável, mas ecossistema de middleware mais fechado e cadência de lançamentos historicamente mais lenta. |
| **Poem / Salvo** | Menor adopção; para uma base institucional de longa duração, a dimensão da comunidade é um critério legítimo. |
| **`hyper` directamente** | Reinventaria routing, extracção e middleware sem benefício — contra o corolário do ADR-0004. |

## Consequences

**Positivas** — middleware partilhado com o Workspace (também Axum);
extractores tipados eliminam uma classe de erros de parsing; `tower-http`
fornece limites de corpo e cabeçalhos de segurança testados.

**Negativas** — mensagens de erro do compilador em handlers com traits
complexos são notoriamente difíceis de ler; mitigado mantendo handlers finos e
a lógica nos serviços.

## Referências

`CLAUDE.md` §23 · ADR-0004 · ADR-0600
