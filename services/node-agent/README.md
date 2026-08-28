# `node-agent` — Node Runtime

O agente que corre num nó computacional da Ocinye.

## Estado: esqueleto

Este agente **enrola o nó, reporta recursos e faz heartbeat**. **Não executa
jobs.** O despacho de trabalho é `PLANNED`, e nenhum nó computacional da Ocinye
existe ([ADR-0500](../../docs/adrs/0500-compute-registry-node-agent.md)).

Está declarado como esqueleto em vez de ser descrito como um runtime que faz mais
do que faz.

## Forma de segurança

- **Identidade de máquina própria.** Nunca usa credenciais de uma pessoa.
- **Ligação apenas para fora.** O Core nunca abre ligação para o nó, e o nó não
  aceita tráfego de aplicação. A topologia futura é `VPS → WireGuard → nó`.
- **O token do agente** é lido de um ficheiro com permissões `0600` e nunca é
  registado nem incluído numa mensagem de erro.
- **Sem redirecções.** Seguir uma redirecção significaria enviar uma credencial
  de máquina para outro sítio.

## Ciclo de vida

1. **Enrolamento** — na primeira execução, troca um token de utilização única por
   uma credencial de agente, que persiste. Enrolar de novo em cada arranque
   falharia, porque os tokens de enrolamento são de utilização única por desenho.
2. **Heartbeat** — de 30 em 30 segundos por omissão, reporta versão do agente,
   recursos e modelos.

Um heartbeat falhado não é fatal: o nó continua a correr e o Core mostra-o como
offline, que é a verdade.

## O que não é descoberto

Descoberta de GPU e de modelos **não está implementada**: exige ferramentas de
fornecedor e um runtime de modelos que ainda não existem em nenhum nó Ocinye. O
agente reporta listas vazias e declara `not_implemented` no bloco de saúde, em
vez de valores plausíveis inventados.

## Configuração

| Variável | Significado |
|---|---|
| `OCINYE_CORE_URL` | Obrigatória. HTTPS fora de desenvolvimento local. |
| `OCINYE_NODE_ENROLLMENT_TOKEN` | Só necessária no primeiro arranque. |
| `OCINYE_NODE_TOKEN_PATH` | Onde a credencial persiste. Por omissão `/var/lib/ocinye/agent.token`. |
| `OCINYE_NODE_HEARTBEAT_SECONDS` | Intervalo entre heartbeats. |

## Execução e testes

```bash
cargo test -p ocinye-node-agent
```

3 testes, incluindo a verificação de que um nó sem modelos reporta zero e que a
descoberta não implementada é declarada, não simulada.
