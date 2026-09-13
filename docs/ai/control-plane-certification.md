# Certificação do plano de controlo de IA — `OCINYE_AI_CONTROL_PLANE_READY`

**Declarado a 2026-09-13.** O plano de controlo de IA está completo e
**independente de fornecedor**: quando a primeira GPU entrar, liga-se como um
recurso registado, sem redesenhar o Workspace nem o Core.

Esta certificação é **documental** — nada de runtime muda. O estado factual de
runtime mantém-se e é o esperado: `OCINYE_AI_RUNTIME_READY` permanece **falso**,
com 0 fornecedores, 0 nós, 0 modelos residentes e 0 GPU. Em produção, a conclusão
de um pedido de IA é sempre `SYSTEM`/`DEGRADED`. Um plano de controlo pronto sem
runtime é exactamente o estado que precede o primeiro nó.

## O que está provado, e onde

Cada linha é provada por testes que correm sem hardware de IA — o
`FixtureProvider` implementa o contrato canónico, não o formato de um fornecedor,
pelo que todo o caminho é exercido de ponta a ponta com zero GPU.

| Propriedade | Evidência |
|---|---|
| Prompt sempre operacional (input nunca desactivado por ausência de IA) | `apps/workspace` — testes do ecrã `prompt`; `services/core-server/tests/prompt_http.rs` |
| Envelope tipado (`origin`/`status`/`reason_code`, proveniência nula explícita) | `crates/ocinye-contracts` — testes de `intelligence`; `prompt_http.rs` |
| Capacidade seleccionável independente da disponibilidade | testes do ecrã `prompt` |
| Model Router: «zero candidatos» é resultado tipado, não excepção | `prompt_http.rs` (cenários 0–2) |
| Caminho de execução: fornecedor que serve → `MODEL`/`COMPLETED` | `prompt_http.rs` (cenário 3) |
| Hot-plug **sem reinício** (nó liga→roteia, desliga→degrada) | `prompt_http.rs` (passos 5–7, mesma `AppState`, `FixtureProvider` fixo) |
| Admissão de recursos fail-closed + reserva/libertação pela transacção | `crates/ocinye-core/tests/resource_ai.rs`; `prompt_http.rs` (cenário 8) |
| Ledger de uso imutável (proveniência que sobrevive à remoção) | migração 0043; `prompt_http.rs` (limpeza de inventário com uso registado) |
| Conversa persistida, owner-private, verdade histórica | `crates/ocinye-core/tests/ai_conversations.rs` |
| Soberania: nenhum pedido de inferência externo sem fornecedor | `OCINYE_AI_ALLOW_EXTERNAL_PROVIDERS=false`; `NoProvider` recusa |

## ADRs

- [ADR-0308](../adrs/0308-typed-ai-interaction-envelope.md) — envelope tipado
- [ADR-0304](../adrs/0304-canonical-inference-contract.md) — contrato de inferência (router e execução)
- [ADR-0109](../adrs/0109-ai-request-admission-and-immutable-usage-ledger.md) — admissão e ledger imutável
- [ADR-0309](../adrs/0309-ai-conversation-persistence-and-provenance.md) — persistência de conversas

## O que fica para o runtime (M5+ com hardware)

- Um adaptador de inferência real que sirva `GENERAL`/`CODING`/`REASONING`.
- Reservas persistidas para trabalhos de computação assíncronos, e a admissão de
  `gpu`/`vram`/`gpu_time`/`compute_time`.
- A superfície de histórico de conversa e a continuação a partir da interface.

Nenhum destes é uma lacuna do plano de controlo — são a ligação do primeiro
recurso físico, que a arquitectura já espera.
