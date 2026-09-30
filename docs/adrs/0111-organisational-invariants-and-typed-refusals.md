# ADR-0111 — As invariantes de governo da organização vivem no Core, sob concorrência, e recusam com um motivo tipado

- **Estado:** Accepted
- **Domínio:** Identity
- **Impacto:** HIGH
- **Depende de:** [ADR-0100](0100-authorization-model.md) · [ADR-0101](0101-permissions-scopes-and-grants.md) · [ADR-0103](0103-core-owned-authentication.md) · [ADR-0107](0107-mandatory-mfa-sessions-and-recovery.md)
- **Data:** 2026-09-30

## Context

O Claude Design entregou na D006 a Administração (Membros, Papéis, Instância)
e as Unidades. Ao ligá-las, verificou-se o que o Core impunha de facto:

1. **Último gestor de uma unidade (U-12).** Retirar o último gestor era
   recusado; **despromovê-lo a membro**, pelo mesmo `upsert` que acrescenta,
   não era — a unidade ficava sem ninguém que a pudesse voltar a gerir. E a
   recusa da remoção contava gestores **retirados**, pelo que um gestor antigo
   no histórico deixava sair o último vivo.
2. **Concorrência.** Nenhuma das guardas (último gestor, último administrador
   da plataforma) trancava nada: dois gestores a despromoverem-se um ao outro,
   ou dois administradores a retirarem o papel um ao outro, viam cada um o
   outro ainda lá, e as duas escritas gravavam.
3. **Ciclo de vida da conta.** A documentação diz que `invited` só nasce da
   criação e que `disabled` é permanente; a operação de estado aceitava
   qualquer dos quatro, e só a interface os recusava.
4. **Recusas por prosa.** O Core recusava com texto em português; um cliente
   que quisesse dizer «a unidade ficaria sem gestor» teria de ler a frase.
5. **Autoridade de apresentação.** A interface precisava de saber quem pode
   arquivar e criar unidades, e isso depende da política, não de um papel.

## Decision

**As invariantes são do Core, e valem para todos os caminhos que as tocam.**
Uma unidade mantém pelo menos um gestor **vivo**: a remoção e a despromoção
passam pela mesma guarda (`ensure_keeps_a_manager`), que conta só pertenças
com `revoked_at IS NULL`. A instituição mantém um administrador da plataforma
capaz de entrar: suspender, desactivar, apagar e revogar o papel continuam a
passar por `ensure_not_sole_platform_admin`. O próprio não se suspende, não se
desactiva e não se apaga.

**Cada guarda serializa as mudanças que a podem violar.** A do gestor tranca a
linha da unidade (`SELECT … FOR UPDATE`) antes de contar; a do administrador
toma uma tranca consultiva por instituição (`pg_advisory_xact_lock`). A segunda
de duas mudanças simultâneas espera pela primeira e lê o que ela gravou.

**O ciclo de vida documentado é imposto pela operação.** `invited` não é um
destino e `disabled` não tem volta; uma e outra são uma transição inválida
tipada (`DomainError::InvalidTransition`).

**Uma invariante recusa com um motivo estável.** `CoreError::Invariant { code,
reason, message }` mantém o código HTTP que a operação sempre teve (`422`,
`409`) e acrescenta `details.reason` ao envelope: `last_platform_admin`,
`last_unit_manager`, `self_lockout`. A lista é estreita de propósito: só as
invariantes que um cliente tem de explicar. O Workspace lê-as como
`ApiFailure::Refused`; nunca interpreta a prosa.

**A autoridade de apresentação é perguntada ao Core.** `GET /units/{id}` diz
`may_manage_members` e `may_archive`; `GET /units/capabilities` diz
`may_create`; `…/security` e `…/access` dizem o que o actor pode fazer sobre uma
conta. Cada sinal é calculado pela política que a operação aplica, e é
cortesia de renderização: a operação decide outra vez.

**Do lado do Workspace, três regras:**

- A confirmação partilhada só abre para uma acção que o Core oferece agora, leva
  o alvo e o destino fixos no formulário, e não é autorização.
- A credencial temporária só existe na resposta do POST que a emitiu
  (`no-store`); nunca num endereço, num registo, noutro ecrã ou no
  armazenamento do browser.
- Nenhum ecrã enumera pessoas para escolher: sem um contrato de candidatos
  elegíveis, acrescentar a uma unidade fica indisponível e explicado.

## Alternatives

| Alternativa | Porque não |
|---|---|
| Esconder «despromover» no último gestor, na interface | Qualquer cliente do Core (CLI, agente, pedido forjado) passava por baixo. |
| Contar gestores sem trancar, e confiar no isolamento `SERIALIZABLE` | Mudaria o nível de isolamento de todo o Core para duas guardas; a tranca é local e explícita. |
| Códigos de erro novos por invariante (`LAST_UNIT_MANAGER` como `ErrorCode`) | Mudava o código HTTP e o contrato de todos os clientes; o motivo em `details` é aditivo. |
| Ler o motivo da mensagem do Core | A mensagem é para pessoas, traduzível e mutável; um cliente que a lê parte na primeira revisão. |
| Deduzir `may_archive` e `may_create` do papel do membro | Uma segunda política no cliente, que diverge da primeira em silêncio. |

## Consequences

- Os testes que esperavam `CoreError::Validation` nestas recusas passaram a
  esperar a recusa tipada (mais estreita), com o mesmo código.
- Uma capability agentic que encontre uma invariante reporta-a como recusa de
  validação, com a mensagem do Core.
- Faltam no Core, e ficam registados: candidatos elegíveis para uma unidade,
  «papéis que o actor pode conceder», pesquisa no roster, convites por token
  (lista e revogação). Nenhum tem controlo morto na interface.
- As provas: `crates/ocinye-core/tests/organisation_invariants.rs` (cada guarda
  provada por reversão, incluindo as trancas) e
  `apps/workspace/tests/d006_journeys.rs`.
