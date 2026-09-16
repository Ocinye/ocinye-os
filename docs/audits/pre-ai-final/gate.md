# Aggregate Gate — `OCINYE_PRE_AI_SYSTEM_READY`

**Estado: NÃO DECLARADO.** A auditoria pré-IA está em curso. Este portão é
**fail-closed**: só se declara READY quando **todos** os critérios abaixo têm
evidência, e não por editar uma linha de Markdown (§93).

Semântica pretendida: *«A baseline determinista completa do Ocinye OS foi auditada
e provada. A inferência de modelo real é a única camada de runtime intencionalmente
indisponível.»*

## Critérios e estado

| # | Critério | Evidência | Estado |
|---|---|---|---|
| 1 | Produção corre o `main` certificado | release `99cb02a99a58` == `main @ 99cb02a` | **✓** |
| 2 | Migrações consistentes | 47/47 na BD == 47 na árvore | **✓** |
| 3 | Suites canónicas passam (CI) | portão canónico verde em `99cb02a` | **✓** |
| 4 | Sem broken access control / IDOR | [05](05-security-review.md) | **✓** |
| 5 | Sem mock de IA roteável em produção (§84) | `test-fixtures` gated + `test_supply_chain.py` | **✓** |
| 6 | Fronteira de conversão endurecida, provada em produção | [11](11-production-verification.md) | **✓** |
| 7 | Guarda contra *stage drift* do Dockerfile | `compose_build_targets.py` (PR #111) | **✓** |
| 8 | P0 abertos = 0, P1 abertos = 0, P2 abertos = 0 (salvo deferimento autorizado) | [findings.md](findings.md) | **✓** (abertos: P0=0, P1=0, P2=0) |
| 9 | Rollback rápido provado (RTO medido ao vivo) | script escrito; `--list` verificado; **flip ao vivo por exercer** | **✗** (autorização única no fim) |
| 10 | Backup/restauro provado nesta baseline | **provado** num ensaio isolado (base); apanhou e corrigiu um P1 ([09](09-backup-restore.md)) | **✓** (metade dos objectos a re-exercer) |
| 11 | Jornadas E2E — parciais/em-falta fechadas (J4/J9/J11/J15/J18-20) | [04](04-e2e-matrix.md) | **✗** (parciais) |
| 12 | Aceitação visual autenticada em produção | precisa do Fidel | **pendente (humano)** |
| 13 | Fixtures hostis (XSS/CSRF/IDOR já cobertos; bomba de imagem e endurecimento acrescentados) | [05](05-security-review.md); #114; lacunas #7/#8/#13/#15 por fechar | **parcial** |
| 14 | Plano de controlo de IA pré-runtime provado (hot-plug, §84, degradado) | [14](14-ai-control-plane.md) | **✓** |

## Porque não está declarado

Dez critérios estão provados (1-8, 10, 14 — incluindo o backup/restauro, que
apanhou e corrigiu um P1). Faltam: o **rollback ao vivo** (9, uma autorização
controlada única no fim), o **fecho das jornadas E2E parciais/em-falta** (11), a
**aceitação autenticada** que é do humano (12), e o **fecho das fixtures hostis
restantes** (13 — #7/#8/#13/#15).

Declarar READY agora seria inventar evidência (§59, §69). O portão fica **fechado**
até estes critérios terem prova executável.

## Como se liga a evidência executável (fail-closed)

Este portão não se declara por prosa. Quando os critérios 9-13 fecharem, será um
verificador que **recusa** se qualquer destes falhar:

- required CI verde e portão canónico de `main` verde;
- manifesto de E2E canónico válido (`test-enumeration.sh`);
- migrações consistentes (árvore == BD);
- guarda de verdade documental (`section-one-contract.py`);
- **P0 abertos = 0, P1 abertos = 0, P2 abertos = 0** (salvo deferimento autorizado);
- evidência de backup/restauro re-exercida nesta baseline;
- evidência de rollback (RTO medido);
- evidência de smoke de produção;
- **zero `FixtureProvider` no binário de produção** (`test_supply_chain.py`);
- a lista de bloqueios de IA limitada a hardware/inferência de modelo
  ([12](12-ai-runtime-gap.md)).

Tal como os outros portões de `main`, corre contra evidência, não contra uma linha
de Markdown.
