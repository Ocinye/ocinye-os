# 00 — Executive Summary (Pré-IA)

**Auditoria:** baseline determinista do Ocinye OS, antes do primeiro runtime de
IA/GPU. **Data:** 2026-09-16. **`main`:** `99cb02a`. **Produção:** `99cb02a99a58`
(== `main`). **Estado agregado:** ver [gate.md](gate.md) — **não declarado READY**;
a auditoria está em curso, faseada, e a certificação agregada só se declara quando
a evidência a sustentar (fail-closed).

## O que esta auditoria estabeleceu, com evidência

- **Produção == `main` certificado**, migrações 47/47 aplicadas, só o proxy é
  público, conversão endurecida a correr — verificado read-only no host
  ([11](11-production-verification.md)).
- **Sem broken access control e sem IDOR** em toda a superfície Core — toda a
  mutação passa por autoridade de servidor, toda a leitura por-id por um portão,
  recusas são `NotFound` ([05](05-security-review.md)).
- **Sem mock de IA em produção** (§84): o fornecedor de teste é compile-time-gated
  e não atravessa para o release; produção fixa `NoProvider`; guarda com `cargo
  tree` ([05](05-security-review.md)).
- **Superfície Workspace coerente:** nenhuma rota-página inalcançável, nenhuma
  entrada de navegação/Criar para rota inexistente, nenhum dado de demonstração
  fixo; as afordâncias «ainda não» são declaradas, não enganosas ([02](02-feature-inventory.md)).
- **Fronteira de conversão de conteúdo hostil** re-certificada ponta-a-ponta em
  produção (PDF/Office/vídeo → PNG sob endurecimento total, ADR-0609).
- **Segurança operacional** reforçada nesta auditoria: rollback rápido de release
  (`rollback-production.sh` + runbook) e um guarda que impede o *stage drift* do
  Dockerfile que causou um outage (`compose_build_targets.py`).

## Defeitos e disposição

| Sev | Encontrados | Estado |
|---|---|---|
| P0 | 1 (outage por *stage drift*) | **FIXED** + guardado |
| P1 | 0 | — |
| P2 | 1 (descarga institucional partida) | **FIXED** antes desta pass |
| P3 | 4 | 1 FIXED (rollback), 1 WONTFIX-por-desenho (assist 503), 2 em avaliação |

Detalhe em [findings.md](findings.md).

## Os dois limites honestos desta auditoria

1. **Render autenticado em produção** — não me autentico (sem credenciais, login
   proibido). As jornadas autenticadas provam-se na CI contra base efémera (106
   viagens de browser); a aceitação visual autenticada em produção é do Fidel.
2. **Rollback ao vivo** — o script está escrito e a sua descoberta verificada
   read-only; o flip ao vivo precisa do Fidel ou de uma regra de permissão (o
   classificador bloqueia SSH que muta produção fora do deploy autorizado).

## Bloqueado só no runtime de IA

Enumerado e pequeno ([12](12-ai-runtime-gap.md)): nó físico, GPU, processo de
inferência, pesos, medição, conformidade com modelo real. Nenhum defeito
determinista. Handoff: [`AI_RUNTIME_HANDOFF.md`](../../operations/AI_RUNTIME_HANDOFF.md).
