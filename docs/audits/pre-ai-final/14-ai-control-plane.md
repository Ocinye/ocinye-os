# 14 — AI Control Plane (pré-runtime)

Não se implementa inferência aqui. Audita-se que **tudo à volta dela está pronto**,
para que a primeira GPU se ligue como recurso, sem redesenhar Workspace nem Core.
Provado por cobertura existente, verificada nesta auditoria.

## A transição completa está testada (hot-plug, §25/directiva 12)

`services/core-server/tests/prompt_http.rs::o_router_classifica_o_inventario_e_a_execucao_roteia`
exercita a sequência inteira contra o router real, **sem reinício**:

- **Sem nó** → `origin=SYSTEM`, degradado («sem nó, é do sistema»).
- Modelos existem mas nenhum serve a capacidade → `AI_NO_COMPATIBLE_MODEL`.
- **Nó liga-se** (reporta um modelo que serve) → o **mesmo** núcleo passa a
  `origin=MODEL`, `status=COMPLETED` («o pedido devia rotear sem reinício»).
- **Nó desliga-se** → volta a `SYSTEM`/degradado («nó desligado, volta a ser do
  sistema»).

O envelope tipado (`origin`·`status`·`reason_code`, ADR-0308) e a proveniência de
modelo nula-e-explícita quando não há modelo estão assertados no mesmo teste e em
`prompt_http.rs`/`ai_conversations.rs`.

## Sem fornecedor falso em produção (§84) — verificado

O `FixtureProvider` (o fornecedor determinístico de teste, com uma variante
hostil) está atrás de `#[cfg(feature = "test-fixtures")]`, e essa *feature* vive
**só** em `[dev-dependencies]` do core-server — com o resolver 2, não atravessa
para o binário de release. Produção fixa `NoProvider`. O guarda
`scripts/test_supply_chain.py` confirma-o com `cargo tree` (provado por reversão:
pôr `test-fixtures` em `[dependencies]` fica vermelho). Zero-IA em produção conclui
sempre `SYSTEM`/`DEGRADED` — nunca inteligência falsa, nunca fallback externo.

## Health/readiness distintos da IA (§52) — verificado

`/ready` (`readiness_http.rs`) responde sobre a **infraestrutura** — base de pé,
contrato de compatibilidade — e **não** sobre a IA. A ausência de fornecedor de
inferência não torna o `/ready` nem o Core não-saudáveis; degrada de forma tipada
no caminho do pedido. `OCINYE_AI_RUNTIME_READY` é um estado de capacidade
separado.

## Segurança do caminho agentic — verificada

`agentic.rs` prova que um modelo subvertido **não** produz efeito
(`a_fully_subverted_model_produces_nothing`), que uma falha antes da execução não
tem efeito colateral, que um fornecedor não pode baixar o risco de uma capability,
reivindicar aprovação prévia, nem fabricar um resultado de execução, e que a
identidade de modelo hostil é normalizada na fronteira. O acesso agentic é a
intersecção actor ∩ agente ∩ recurso, reautorizada a cada passo.

## Conclusão

O plano de controlo de IA pré-runtime está **provado**. O que falta é
exclusivamente o **runtime de modelo real** — enumerado em
[12-ai-runtime-gap.md](12-ai-runtime-gap.md), com o handoff em
[`AI_RUNTIME_HANDOFF.md`](../../operations/AI_RUNTIME_HANDOFF.md). Nenhum defeito
determinista se esconde aqui.
