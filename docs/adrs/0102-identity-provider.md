# ADR-0102 — Identity Provider dedicado (Keycloak)

- **Estado:** Superseded
- **Domínio:** Identity
- **Impacto:** HIGH
- **Substituído por:** [ADR-0103](0103-core-owned-authentication.md)
- **Data:** 2026-08-22

> **Substituído em 2026-08-22.** O [ADR-0103](0103-core-owned-authentication.md)
> move a autenticação para dentro do Ocinye Core: nome de utilizador e
> palavra-passe, com verificadores Argon2id e sessões server-side. O texto
> abaixo fica intacto — a decisão foi tomada com esta análise e continua a ser a
> leitura correcta do problema que resolvia (`CLAUDE.md` §68).
>
> Em particular, **a exigência de MFA registada abaixo deixou de vigorar**. O
> estado actual é `MFA = NOT IMPLEMENTED`; ver ADR-0103 e `docs/security/`.

## Context

O `CLAUDE.md` §33 proíbe autenticação caseira e exige OIDC, MFA, recuperação
segura, gestão de sessão, SSO e preparação para WebAuthn/passkeys. Falta
escolher o produto.

## Decision

**Keycloak** como Identity Provider dedicado, integrado por **OIDC**.

O Ocinye Core:

- **nunca** vê, guarda, encaminha ou regista passwords;
- verifica o access token contra o **JWKS publicado pelo IdP**, validando
  assinatura, `iss`, `aud` e `exp`;
- deriva o `Principal` do `sub` verificado **mais o estado institucional em base
  de dados**. Papéis e memberships nunca vêm de claims do token: são factos
  institucionais, não afirmações que um cliente possa influenciar.

**MFA é exigido no IdP**, não implementado no Core. O realm de produção deve ter
MFA obrigatório; isso é configuração operacional, documentada em
`docs/identity/`, e o seu estado real é `PLANNED` até haver realm de produção.

Não existe qualquer via de desenvolvimento que contorne a verificação. Os testes
substituem a *dependência* de autenticação, para que nenhum caminho enfraquecido
exista no binário distribuído.

## Alternatives

| Alternativa | Porque foi rejeitada |
|---|---|
| **Authentik** | Interface moderna e agradável de operar; foi um candidato sério. Keycloak preferido por maturidade em produção, maturidade de federação/SSO e por ter o percurso de operação melhor documentado a longo prazo. Revisitável por ADR se a operação se revelar pesada. |
| **Ory Hydra + Kratos** | Excelente arquitectura e afinidade com Rust/Go, mas exige montar mais peças (UI, recuperação, MFA) — mais superfície a operar para uma equipa pequena. |
| **Zitadel** | Promissor; comunidade e histórico de produção ainda menores. |
| **Autenticação no Core** | Proibido pelo `CLAUDE.md` §33 e pelo corolário do ADR-0004. |
| **IdP SaaS (Auth0, Entra)** | Colocaria identidades institucionais angolanas sob um fornecedor externo, contra o princípio de controlo institucional (`CLAUDE.md` §8). |

## Consequences

**Positivas** — MFA, recuperação, sessões e passkeys resolvidos por software
maduro; o Core não tem superfície de credenciais; SSO futuro para JupyterHub,
Forgejo e outras ferramentas fica disponível sem trabalho adicional.

**Negativas, aceites** — mais um componente com estado a operar, fazer backup e
manter; consumo de memória não trivial; a configuração do realm passa a ser
infraestrutura crítica, com runbook próprio.

## Referências

`CLAUDE.md` §33 · briefing §37 · ADR-0100
