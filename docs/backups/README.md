# Backups

## Estado actual

| Estado | Situação |
|---|---|
| **Configurado** | Não |
| **Executado** | Não |
| **Restore testado** | Não |

**Nenhum backup existe.** Não há nada a restaurar, e nada aqui deve ser lido como
se houvesse.

Esta distinção em três estados é obrigatória (`CLAUDE.md` §63): um backup só é
operacionalmente confiável quando existe um procedimento de restore **testado**.
Um script que existe não é um backup.

## O que precisará de backup

| Activo | Porquê | Sensibilidade |
|---|---|---|
| PostgreSQL | Fonte canónica de metadados, proveniência e auditoria. | Alta |
| Object Storage | Documentos e ficheiros de datasets. | Alta |
| Tabelas `credentials` e `sessions` | Verificadores de palavra-passe e sessões vivas. Nunca palavras-passe. | Muito alta |
| Configuração e segredos | Necessários para reconstruir. | Muito alta |

## Requisitos quando for implementado

- **Cifrados.** Um backup não cifrado da base de dados é uma cópia de tudo o que
  a instituição classificou.
- **Restore testado, com data registada.** Sem isso, nada pode ser chamado
  validado.
- **Auditoria incluída.** Um backup que omita `audit_events` destrói a evidência
  precisamente quando ela é mais necessária.
- **Consistência entre PostgreSQL e Object Storage.** Um documento sem o seu
  objecto, ou um objecto sem a sua linha, é um artefacto perdido. Os checksums
  permitem verificar.

## Objectivo futuro: 3-2-1

Três cópias, dois meios, uma fora do local.

**Não declarar 3-2-1 antes de existir.**

## Restore

Sem runbook. Quando existir, ficará em [docs/runbooks/](../runbooks/README.md) e
terá de ser exercitado, não apenas escrito.
