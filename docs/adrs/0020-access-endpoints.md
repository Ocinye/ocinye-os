# ADR-0020 — Pontos de acesso: o anfitrião escolhe o destino, nunca a autoridade

- **Estado:** Accepted
- **Domínio:** Foundation
- **Impacto:** HIGH
- **Depende de:** [ADR-0019](0019-multi-distribution-instance.md) · [ADR-0601](0601-workspace-bff-session.md) · [ADR-0605](0605-first-production-deployment.md)
- **Data:** 2026-09-30

## Context

Hoje há um `OCINYE_WORKSPACE_PUBLIC_URL`; `routes::origin_is_ours` compara a origem com ele; qualquer
`Host` chega à mesma Instância. Cookie do Workspace host-only, `HttpOnly`, `SameSite=Lax`; cookie do
Core `SameSite=Strict`.

## Decision

1. **Ponto de acesso** (`access_endpoints`): anfitrião normalizado → Instância, e opcionalmente →
   Distribuição (fixo). Estados: activo, desactivado, por verificar. Um canónico por Instância.
   Domínios do cliente são de primeira classe; não se exige `*.ocinye.com`.
2. **O anfitrião não é autorização, pertença, papel nem contexto.** O Core autentica e decide sempre.
3. **Resolução por configuração tipada.** Nunca por texto do nome (`contains("business")`). Um nome
   pertence a uma só Instância no servidor.
4. **Desconhecido falha fechado** (S13): sem identidade da Instância, sem redireccionamento, sem
   Instância por omissão, sem adivinhar pelo subdomínio. Desactivado → S40/S13; por verificar → S14.
5. **Proxy de confiança:** `X-Forwarded-Host` só é lido quando o par TCP está numa lista CIDR tipada
   (`OCINYE_TRUSTED_PROXIES`); um só valor; senão usa-se `Host`. Cabeçalhos de origem não confiável
   são ignorados, nunca combinados.
6. **Sessão por anfitrião.** O cookie continua host-only; não se alarga a `.empresa.com`. Mudar de
   ponto de acesso é navegação explícita para outro anfitrião **configurado** (por `EndpointId`,
   nunca por URL do pedido) e pode pedir nova entrada.
7. **Origem/CSRF:** `origin_is_ours` passa a comparar a `Origin` com `https://{anfitrião do ponto
   resolvido para este pedido}` — o próprio, não qualquer ponto da Instância.
8. **SSO/OIDC:** se houver fornecedor externo, cada ponto tem URI de retorno exacto registado; sem
   curingas. Hoje o Workspace não expõe `redirect_uri` configurável — portão de verificação (R6).
9. **DNS e TLS:** o Core só **observa** (resolução, estado do certificado). Não gere DNS; não emite
   certificados; não escreve configuração do proxy. Isso é da instalação (D011).
10. **Semente:** com a tabela vazia, o ponto canónico genérico nasce do anfitrião de
    `OCINYE_WORKSPACE_PUBLIC_URL`, auditado. Desactivar o canónico/último activo é recusado.

## Consequences

- Middleware de resolução no Workspace antes de qualquer rota; página de anfitrião desconhecido
  servida sem dados da Instância.
- nginx `server_name` passa a ser gerado da lista de pontos pela instalação (D011); até lá, a
  configuração manual é acção externa honesta.
