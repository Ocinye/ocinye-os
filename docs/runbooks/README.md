# Runbooks

## Identidade e acesso — `CURRENT`

Estes descrevem procedimentos que existem e foram exercitados contra o Core e
PostgreSQL reais.

| Runbook | Quando |
|---|---|
| [Bootstrap do primeiro administrador](bootstrap-first-administrator.md) | Instalação nova, sem administrador |
| [Criar um membro](create-member.md) | Alguém passa a precisar de acesso |
| [Repor a palavra-passe de um membro](reset-member-password.md) | Acesso perdido, credencial expirada, suspeita de compromisso |
| [Suspender ou desactivar um membro](suspend-or-disable-member.md) | Licença, disputa, saída |
| [Offboarding de um membro](offboard-member.md) | Alguém deixa a instituição |
| [Recuperar acesso administrativo](recover-administrative-access.md) | Não há administrador capaz de entrar |

Todos eles exigem entrega de credenciais por canal seguro, e nenhum permite a um
administrador escolher ou consultar a palavra-passe de outra pessoa.

## Correio institucional

| Runbook | Quando |
|---|---|
| [Configurar o serviço de correio](configure-mail-service.md) | O Ocinye OS passa a ter correio |
| [Diagnosticar o serviço de correio](diagnose-mail-service.md) | Não aparece, não envia, ou um membro reporta erro |
| [Criar e gerir uma caixa partilhada](create-shared-mailbox.md) | Uma unidade ou função precisa de endereço comum |

O primeiro descreve uma configuração que **ainda não foi feita**: nesta
instalação o correio não está configurado. O terceiro descreve uma operação cujo
ecrã é `PLANNED` — o modelo existe, a interface que o gere não.

Nenhum deles permite a um administrador ler correspondência alheia
([ADR-0404](../adrs/0404-mail-privacy-boundary.md)).

## Continuidade institucional

| Runbook | Quando |
|---|---|
| [Mudar a Ocinye para outro servidor](migrate-to-another-server.md) | Migração planeada, ou recuperação depois de perder a máquina |

Os passos da base de dados foram **executados** a 2026-08-28, incluindo o
controlo negativo que distingue restaurar de recriar
([ADR-0700](../adrs/0700-institutional-continuity-and-portability.md)). Os do
Object Storage **não**: essa metade continua por exercitar, e o runbook di-lo
no sítio onde está.

## Runbooks necessários antes de qualquer deployment

**Estes não existem**, porque nada está deployado. Escrevê-los agora produziria
documentação que descreve uma realidade inexistente (`CLAUDE.md` §69).

| Runbook | Porquê |
|---|---|
| **Restore de Object Storage** | O comando existe — `verify-objects` — e nunca correu contra um bucket acessível. |
| **Backup agendado e cópia fora do servidor** | O procedimento de migração existe; a cópia periódica não. Sem ela o RPO é «desde o último que alguém correu à mão». |
| **Rotação da chave de selagem** | `OCINYE_MAIL_KEY` viaja como está; trocá-la exige reselar `mailbox_credentials`. |
| **Resposta a credencial comprometida** | Revogar papéis, revogar credenciais de nó, rever a auditoria. |
| **Rotação de credencial de nó** | Sem interromper o nó. |
| **Migration falhada em produção** | O serviço recusa arrancar por desenho; é preciso um caminho para a frente. |
| **Outbox encravado** | Diagnóstico e reprocessamento. |
| **Reclassificação de material exposto** | Quem foi notificado, o que foi registado. |

## Procedimentos que já existem

Estes estão documentados e exercitados:

| Procedimento | Onde |
|---|---|
| Levantar a stack local | [docs/development/](../development/README.md) |
| Aplicar migrations | [docs/development/](../development/README.md) |
| Compilar capacidades WASM | `scripts/build-capabilities.sh` |
| Registar e enrolar um nó | [docs/node-protocol/](../node-protocol/README.md) |
| Criar o primeiro administrador | [docs/development/](../development/README.md) |
| Descrever e verificar o estado institucional | [docs/backups/](../backups/README.md) |
