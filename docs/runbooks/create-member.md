# Runbook — Criar um membro

**Quando:** alguém passa a precisar de acesso ao Ocinye OS.
**Quem:** quem detiver `members.create` — hoje `PlatformAdmin` e
`OrganisationAdmin`.

## Antes

- [ ] Nome completo e email institucional confirmados.
- [ ] Nome de utilizador acordado (ver regras abaixo).
- [ ] Papel técnico decidido — e é **distinto** da posição institucional.
- [ ] Unidade inicial, se aplicável.
- [ ] Canal seguro combinado para entregar a credencial.

### Nome de utilizador

3 a 64 caracteres; letras ASCII, dígitos, `.`, `-`, `_`; começa por letra,
termina em letra ou dígito. Único por organização, insensível a maiúsculas.

Convenção sugerida: primeira inicial + apelido (`fmonteiro`). Escolha uma e
mantenha-a: um nome de utilizador não é alterável pelo titular.

### Papel técnico

| Papel | Para quem |
|---|---|
| `research_member` | Investigador comum. **A escolha por omissão.** |
| `research_lead` | Quem lidera ideias ou projectos |
| `unit_manager` | Quem gere uma unidade científica |
| `organisation_admin` | Quem administra pessoas e estrutura |
| `platform_admin` | Quem opera a plataforma. **Raro.** |
| `auditor` | Leitura de evidência, sem acesso a conteúdo |
| `collaborator` | Colaborador interno com âmbito estreito |
| `external_collaborator` | Externo. Deny-by-default no seu ponto mais forte |

Na dúvida, escolha o **mais estreito**. Alargar depois é um pedido; estreitar é
uma conversa.

> «Fundador» é uma posição institucional. **Não** é um papel técnico e não
> concede acesso a nada (`CLAUDE.md` §34).

## Passos

**Ocinye Workspace →** Administração → Membros → **Adicionar utilizador**.

O Core devolve uma credencial temporária, **apresentada uma única vez**. Ao
fechar, deixa de ser recuperável — por ninguém, incluindo o administrador
principal.

## Entrega

Como no [bootstrap](bootstrap-first-administrator.md#entrega): presencialmente,
por voz, ou por mensagem efémera cifrada. Nunca por email, SMS, ticket ou chat.

Diga também:

- que é **temporária** e vale 24 horas;
- que no primeiro acesso terá de definir a sua;
- que o mínimo são **15 caracteres**, e que uma frase serve;
- que ninguém, nunca, lhe vai pedir a palavra-passe.

## Verificação

Na lista de Membros: estado `invited`. Passa a `active` sozinho quando a pessoa
define a sua palavra-passe.

Se continuar `invited` passadas 24 horas, a credencial expirou: siga
[Repor a palavra-passe de um membro](reset-member-password.md).
