# ADR-0308 — O envelope tipado de interacção de IA

- **Estado:** Accepted
- **Domínio:** AI
- **Impacto:** HIGH
- **Data:** 2026-09-13
- **Complementa:** [ADR-0300](0300-ai-gateway.md) · [ADR-0301](0301-agentic-control-plane.md) · [ADR-0304](0304-canonical-inference-contract.md)

## Context

O [ADR-0300](0300-ai-gateway.md) decidiu que a IA reporta disponibilidade em vez
de fingir. O [ADR-0304](0304-canonical-inference-contract.md) definiu o que um
*fornecedor* devolve. Faltava a peça na fronteira com o membro: **o que é que a
superfície de comando devolve quando não há inferência para executar?**

A resposta implícita era um erro. `POST /ai/prompt` respondia
`503 CapabilityUnavailable` — um código de falha — a um pedido que tinha sido
recebido, autorizado e processado correctamente. Nada estava avariado: não havia
simplesmente um modelo. Chamar falha a esse estado é errado por três razões:

1. **Confunde degradação com avaria.** `providers = 0` não é o Core em baixo nem
   um modelo a falhar. É um estado operacional válido — a pré-condição do portão
   `OCINYE_STABLE_PRE_AI_READY`, não uma lacuna dele.
2. **Empurra o Workspace a desactivar-se.** Com a resposta a chegar como erro, a
   interface tratava a ausência de IA como «indisponível» e desligava o input —
   contradizendo o princípio de que o Prompt é uma *superfície de comando*, não
   um widget de modelo.
3. **Não distingue os «porquês».** Um `503` opaco não diz se falta um fornecedor,
   um modelo compatível, se um modelo está a carregar, ou se a política o desliga.
   Sem vocabulário, cada caminho de execução futuro inventaria a sua própria
   string.

E a resposta não tinha **origem**. Uma frase escrita pela plataforma porque não
havia inferência era indistinguível, na forma, de uma resposta que um modelo
tivesse produzido. A distinção é constitucional (§8): a saída de um modelo nunca
é estado do sistema, e uma resposta do sistema nunca se pode fazer passar por um
modelo.

## Decision

Uma interacção de IA conclui num **envelope tipado**, devolvido com HTTP de
sucesso quando o pedido foi processado — mesmo que nenhuma inferência o tenha
executado. O envelope vive em `ocinye-contracts` e tem quatro peças de
vocabulário novas.

### `InteractionOrigin` — quem redigiu

`SYSTEM` · `MODEL` · `TOOL` · `AGENT`. Uma resposta determinística da plataforma
é `SYSTEM`, e nunca `MODEL`. O leitor lê a origem para saber o que tem à frente.

### `InteractionStatus` — como concluiu

`COMPLETED` (um modelo respondeu) · `DEGRADED` (processado, respondido
deterministicamente sem inferência). `DEGRADED` é uma conclusão **bem-sucedida**,
não um erro. Os erros — permissão, validação — continuam a ser
[`ErrorCode`](../../crates/ocinye-contracts/src/error.rs), com o seu estado HTTP;
reservam-se para falhas reais.

### `AiReasonCode` — o porquê, legível por máquina

Um conjunto fechado, em `SCREAMING_SNAKE_CASE`, estabelecido inteiro agora ainda
que só um seja alcançável hoje:

`AI_NO_PROVIDER_AVAILABLE` · `AI_NO_COMPATIBLE_MODEL` · `AI_CAPACITY_UNAVAILABLE`
· `AI_MODEL_HARDWARE_NOT_SATISFIED` · `AI_PROVIDER_UNHEALTHY` · `AI_MODEL_LOADING`
· `AI_PERMISSION_DENIED` · `AI_MODEL_NOT_ENTITLED` · `AI_RESOURCE_QUOTA_EXCEEDED`
· `AI_DISABLED_BY_POLICY`.

O estado verdadeiro desta instalação é `AI_NO_PROVIDER_AVAILABLE`. Os restantes
nomeiam as condições futuras distintas para que os caminhos de execução que
M5.2/M5.3 constroem classifiquem sem inventar strings.

### `AiInteractionResponse` — uma forma para todas as conclusões

`{ origin, status, reason_code?, model, provider, compute_node, content }`. Os
três campos de proveniência serializam-se como `null` quando ausentes —
**deliberadamente presentes**, para se *provar* que nenhum modelo esteve
envolvido, em vez de o inferir de uma omissão.

Uma conclusão de modelo será `origin = MODEL, status = COMPLETED`, a nomear o
modelo, o fornecedor e o nó. A conclusão de hoje é
`origin = SYSTEM, status = DEGRADED, reason_code = AI_NO_PROVIDER_AVAILABLE`, com
a proveniência nula.

## Alternatives

- **Manter o `503`.** Rejeitado: chama falha a um estado que não é falha, e força
  a interface a desligar-se.
- **Um segundo domínio de «Notice» no Workspace.** Rejeitado: já havia um envelope
  de resposta a estender. Inventar um segundo canal de apresentação para a
  resposta de IA duplicaria o vocabulário e divergiria dele.
- **Só uma string de razão, sem código-máquina.** Rejeitado: a prosa é para o
  membro; um código estável é para o cliente, o log e o operador. Os dois
  respondem a perguntas diferentes.
- **Adiar o vocabulário até haver um segundo estado alcançável.** Rejeitado:
  estabelecer o conjunto fechado agora impede que cada caminho futuro cunhe o seu
  próprio.

## Consequences

- `POST /ai/prompt` devolve `200` com o envelope; o Prompt mantém-se operacional
  com zero fornecedores, e a resposta de sistema aparece na conversa com a origem
  explícita.
- A procura continua a ficar registada no ledger `ai_jobs` como recusa, agora com
  o **código-máquina** da razão — a evidência de demanda que justifica um nó, sem
  consumir tokens nem reservar GPU.
- `AgenticOutcome::Unavailable` (a superfície agentic) ainda **não** carrega o
  `reason_code`; unificá-lo é trabalho de M5.2, e o vocabulário já existe para o
  receber.
- O envelope é a fundação de `OCINYE_AI_CONTROL_PLANE_READY`, que continua
  distinto de `OCINYE_AI_RUNTIME_READY` — este permanece falso enquanto
  `providers = 0`.
