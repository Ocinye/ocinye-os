# ADR-0601 — O Workspace como Backend-for-Frontend

- **Estado:** Accepted
- **Domínio:** Workspace
- **Impacto:** HIGH
- **Data:** 2026-08-22

## Context

O Ocinye Workspace autentica membros por OIDC e chama o Ocinye Core em nome
deles. Alguém tem de deter o token de acesso.

A escolha habitual — guardá-lo no browser — expõe-no a qualquer script na página.
Num sistema que detém investigação classificada, essa exposição é
desproporcionada face à conveniência que traz.

O `CLAUDE.md` §4 já estabelecia que o browser nunca é autoridade. Falta decidir
onde vive o token, e onde vive a sessão.

## Decision

O Workspace é um **Backend-for-Frontend**.

1. Executa o fluxo OIDC Authorization Code com PKCE.
2. **Guarda os tokens do lado do servidor.** O browser recebe apenas um
   identificador de sessão opaco, de 256 bits de entropia do sistema operativo,
   num cookie `HttpOnly`, `Secure`, `SameSite=Lax`.
3. Chama o Core com o bearer, propagando o `X-Correlation-ID`.

Consequências de segurança que isto compra:

- **XSS não rende um token.** Não há token na página para roubar.
- **A revogação é imediata e local.** Terminar sessão destrói o estado do
  servidor; o token torna-se inalcançável, em vez de meramente esquecido pelo
  browser.
- **`SameSite=Lax` cobre CSRF** para as submissões de formulário do Workspace,
  sem impedir o retorno do redirect OIDC.
- **O logout também termina a sessão no IdP** quando este o suporta. Terminar só
  a sessão local deixaria o membro autenticado no fornecedor, e o próximo início
  de sessão passaria sem re-autenticação — surpreendente, e errado numa máquina
  partilhada.

### Armazenamento das sessões

Em memória do processo. **Um reinício termina todas as sessões.**

Isto é uma limitação aceite e declarada, não escondida: mover o armazenamento
para Redis é `PLANNED` e é uma mudança contida atrás do tipo `SessionStore`. Para
uma instituição com dezenas de membros e uma única instância, o custo real é
pedir novo início de sessão após um deploy.

## Alternatives

| Alternativa | Porque foi rejeitada |
|---|---|
| **Token no `localStorage`** | Legível por qualquer script. Um único XSS entrega credenciais institucionais. |
| **Token num cookie `HttpOnly`** | Melhor, mas o token continua a atravessar o browser e a estar sujeito ao limite de tamanho e ao comportamento de cookies. O BFF evita ambos. |
| **Sessão em JWT assinado no cookie** | Sem estado, mas irrevogável antes da expiração — o oposto do que se quer quando uma filiação é suspensa. |
| **Sessões em Redis desde já** | É o destino. Adia-se porque a mudança é contida e o benefício actual, com uma instância, é pequeno. |
| **SPA a chamar o Core directamente** | Colocaria o browser a deter credenciais institucionais e a decidir o que mostrar com base num token que possui. |

## Consequences

**Positivas** — nenhuma credencial no browser; revogação imediata; o Workspace
torna-se uma trust boundary explícita e documentada.

**Negativas, aceites** — o Workspace passa a ter estado, o que complica correr
mais do que uma instância antes de as sessões irem para Redis; um reinício
termina as sessões.

### Nota de 2026-08-23 — `SameSite=Lax` não cobre CSRF sozinho

A decisão acima não muda, e esta nota não a reescreve: o Workspace continua a
ser um Backend-for-Frontend com um cookie opaco `SameSite=Lax`. O que se
regista é uma consequência que a decisão declarou e que a
[Security Baseline v1](../security/2026-08-23-security-baseline-v1.md) verificou
ser incompleta.

`SameSite` compara o **domínio registável**, não a origem. Uma página em
`ocinye.com` — que o `CLAUDE.md` §5 reserva para o futuro website público — é
*same-site* com `workspace.ocinye.com`, e o browser envia-lhe o cookie da
sessão. O mesmo vale para um XSS em qualquer subdomínio irmão. Um subdomínio não
é uma fronteira de confiança.

Em consequência, uma escrita autenticada passou a exigir também que o `Origin`
seja esta origem. O cookie continua `SameSite=Lax` — a razão original,
não impedir o retorno do redirect OIDC, mantém-se —, mas deixou de ser a única
coisa entre uma página hostil e um `POST`.

## Referências

`CLAUDE.md` §4 · briefing §17 · ADR-0600 · ADR-0102
