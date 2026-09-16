# Handoff — Ligar o primeiro runtime de IA

**Para quando existir hardware de GPU.** Descreve o que falta para a inferência
real entrar, **partindo da baseline determinista pré-IA certificada**. Não repete
o que já está feito — o plano de controlo (fronteira `InferenceProvider`, Model
Router, envelope tipado ADR-0308, admissão de recursos, persistência de conversa,
hot-plug) já existe e está testado com o `FixtureProvider`
([auditoria pré-IA](../audits/pre-ai-final/12-ai-runtime-gap.md)).

**Invariante:** nenhum módulo determinista do Ocinye precisa de redesenho. A IA
liga-se como recurso.

## Passos

1. **Aprovisionar e enrolar o nó GPU.** Instalar o Node Agent no nó; trocar o
   token de enrolamento (uso único) por uma credencial de máquina. Ver
   [`connect-compute-node.md`](../runbooks/connect-compute-node.md).
2. **Fechar o caminho de rede.** WireGuard entre o VPS e o nó; o nó **nunca**
   aceita tráfego de aplicação público (§24, §30). Só ligação para fora.
3. **Verificar GPU/driver/runtime** no nó: a GPU é vista, o driver e o runtime de
   inferência estão instalados e saudáveis. Não declarar nada online sem health
   real (§7).
4. **Instalar o runtime de inferência aprovado** (vLLM/SGLang ou equivalente) como
   um fornecedor que implementa o contrato canónico (ADR-0304). O adapter traduz;
   nada específico do fornecedor alcança o Core.
5. **Obter os pesos de um modelo open-weight aprovado.** Registá-lo como artefacto
   (a continuidade já distingue o modelo base readquirível do artefacto treinado).
6. **Registar o fornecedor/modelo** — o nó reporta o inventário; o Model Router
   lê-o a cada pedido.
7. **Mapear capacidades** (`GENERAL`/`CODING`/`REASONING`/`EMBEDDING`) para o(s)
   modelo(s) — é configuração, nunca código (§41).
8. **Health/readiness:** `OCINYE_AI_RUNTIME_READY` passa a verdadeiro só quando um
   fornecedor serve; a saúde do Core determinista **não** depende disto (§52).
9. **Conformidade:** correr a Provider Conformance Suite contra o fornecedor real
   (ADR-0305). Passar torna-o utilizável, não confiável.
10. **Inferência medida:** primeiras respostas reais, tokens/s e VRAM medidos —
    substituem as estimativas.
11. **Admissão de recursos:** confirmar que a admissão de `model_access` cobra o
    ledger só quando um modelo responde (fail-closed já implementado, ADR-0109).
12. **Activação controlada em produção:** ligar por hot-plug, sem reinício;
    verificar que um pedido conclui `origin=MODEL`/`status=COMPLETED`, e que
    desligar o nó volta a `SYSTEM`/`DEGRADED`.

## Soberania (mantém-se)

`OCINYE_AI_ALLOW_EXTERNAL_PROVIDERS=false`. Nenhum fornecedor externo é ligado em
substituição. Qualquer fornecedor externo futuro é decisão explícita, com ADR e
análise de residência de dados (§41).

## O que **não** fazer

- Não declarar `CAM-01 online`, `GPU disponível` ou `IA local operacional` sem
  evidência real e verificada (§7).
- Não pôr o `FixtureProvider` no caminho de produção — está compile-time-gated de
  propósito (§84).
- Não começar a integração de IA dentro da PR da auditoria pré-IA — parar na
  fronteira limpa, a partir de uma baseline determinista conhecida-boa.
