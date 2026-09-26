# ADR-0015 — A fronteira entre o Core e as aplicações

- **Estado:** Accepted
- **Domínio:** Foundation
- **Impacto:** HIGH
- **Depende de:** [ADR-0014](0014-instance-profiles-and-application-activation.md) · [ADR-0006](0006-modular-monolith.md)
- **Data:** 2026-09-26

## Context

O Core e todas as aplicações vivem num crate e num binário
([ADR-0006](0006-modular-monolith.md)): um router junta vinte módulos sem
condição nenhuma. A [ADR-0014](0014-instance-profiles-and-application-activation.md)
deu às Instâncias a capacidade de desactivar aplicações opcionais — mas só o
Workspace as escondia. A API de uma aplicação desactivada continuava a responder
a qualquer cliente que a chamasse: um script, um separador antigo, um agente.

O programa de generalização pede ainda que uma aplicação possa falhar sem
derrubar o Core, e que retirar uma aplicação opcional não invalide autenticação,
pertença, autorização, contas de recursos, o Gestor de Aplicações, as
Definições nem a saúde do Core.

## Decision

**1. O Core governa; as aplicações implementam.** Pertencem ao Core — e nunca
se desactivam — identidade, autenticação, autorização e permissões, políticas e
invariantes, autoridade sobre o registo de aplicações, governação de recursos e
quotas, segredos, auditoria, configuração da Instância, autoridade sobre nós, e a
coordenação do estado persistente. Correio, Notas, Bibliografia, Projectos,
Calendário, o ecrã do Prompt e as outras são aplicações ou serviços de sistema
([sistema actual §7](../architecture/CURRENT_SYSTEM.md#7-classificação-do-código),
[fronteira](../architecture/CORE_AND_APPLICATIONS.md)).

**2. A recusa de uma aplicação inactiva é do Core.** Um middleware à frente de
toda a API mapeia o caminho para a aplicação a que pertence e recusa, com
`503 application_inactive`, se a Instância a tiver inactiva. Só os caminhos de
**uma** aplicação são mapeados: identidade, autenticação, saúde, configuração da
Instância, contentores partilhados (ambientes, tarefas, a cadeia científica) e a
autoridade sobre nós (enrolamento, heartbeat) nunca são recusados por activação
— recusá-los partiria o Core ou outra aplicação activa.

**3. Porquê `503` e não `404`.** É a mesma classe que o Core já usa para «a
capacidade existe e esta instalação não a tem de pé» (correio por configurar,
armazenamento em falta). A aplicação existe e os dados dela também; negar a sua
existência seria mentir, e o Workspace já mostra a razão que o Core dá a um `503`.

**4. Uma falha de aplicação não derruba o Core.** Um `panic` num handler é
apanhado por uma fronteira à volta do router e passa a um `500` com o envelope de
erro de sempre; a causa fica no log e nunca chega ao cliente, e as outras rotas
continuam a responder.

## Alternatives

**Separar as aplicações em processos.** Daria isolamento de memória e de
arranque, e custaria o modular monolith que a ADR-0006 escolheu por boas razões,
antes de haver uma aplicação de terceiros que o justifique. O manifesto de
aplicação (Parte 4) é o sítio onde esse passo se decide, se chegar a ser preciso.

**Recusar dentro de cada serviço.** Vinte sítios a lembrarem-se da mesma regra
envelhecem de vinte maneiras. Um middleware com um mapa testado é um sítio só.

## Consequences

- Desactivar uma aplicação passa a ter efeito para qualquer cliente, e não só no
  Workspace.
- Um caminho novo de uma aplicação que não entre no mapa não é recusado quando a
  aplicação está inactiva — falha aberto, mas só para aplicações opcionais e só
  na activação; a autorização continua inteira. O teste do mapa lista os
  caminhos partilhados que **não** devem ser recusados, e o manifesto (Parte 4)
  passa a declarar as rotas de cada aplicação.
- Cada pedido a uma aplicação opcional custa uma leitura pequena da configuração
  da Instância.
