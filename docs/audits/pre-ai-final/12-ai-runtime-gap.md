# 12 — AI Runtime Gap (Pré-IA)

O que fica **legitimamente** por fazer porque exige GPU/modelo real. Lista pequena
e precisa: nada aqui é um defeito de produto determinista.

## O que já está pronto (não é bloqueio)

O **plano de controlo de IA** está implementado e testado sem hardware
(`OCINYE_AI_CONTROL_PLANE_READY`): a fronteira `InferenceProvider`, o Model Router
(«zero candidatos» é resultado tipado, não excepção), o envelope tipado
(`origin`·`status`·`reason_code`, ADR-0308), a persistência de conversa com
proveniência por turno, a admissão de recursos fail-closed, e o hot-plug provado
com o `FixtureProvider` (nó liga→roteia, desliga→degrada, sem reinício). O
`FixtureProvider` está compile-time-gated e nunca é roteável em produção (§84,
verificado). Produção fixa `NoProvider` e conclui sempre `SYSTEM`/`DEGRADED`.

## O que está bloqueado no runtime real

| # | Bloqueio | Porquê é intrínseco |
|---|---|---|
| 1 | Enrolamento de um nó físico | Não existe hardware; o Node Agent enrola e faz heartbeat, mas não há nó |
| 2 | Descoberta de GPU/VRAM/driver/runtime | Exige a máquina e o driver reais |
| 3 | Processo de inferência (vLLM/SGLang ou equivalente) | Exige GPU e o runtime instalado |
| 4 | Pesos de um modelo aprovado (open-weight) | Não há artefacto de modelo; `ai_models` é inventário reportado pelo nó |
| 5 | Capacidade de VRAM/tokens medida | Só se mede com o modelo a correr |
| 6 | Conformidade de inferência (Provider Conformance Suite) com modelo real | O contrato está testado com fixture; falta com um fornecedor real |
| 7 | Comportamento de tool-call específico do modelo | Depende do modelo escolhido |
| 8 | Saída real do `assist` de Mail/Messaging, e do Prompt/Ask/Act | Todos funcionam **quando** um fornecedor serve; sem ele, degradam tipado/honesto |

## O que **não** está nesta lista

Nenhum bug de produto determinista. Se durante a auditoria algo determinista não
funcionasse «porque falta a GPU», seria um defeito a corrigir, não um item aqui.
Nenhum foi encontrado: Mail, Notas, Ficheiros, Calendário, Mensagens, Bibliografia,
Datasets, Conhecimento, unidades/investigação/projectos, recursos, admin, pesquisa
lexical e o Prompt-como-superfície funcionam sem inferência.

Ver o runbook de handoff:
[`docs/operations/AI_RUNTIME_HANDOFF.md`](../../operations/AI_RUNTIME_HANDOFF.md).
