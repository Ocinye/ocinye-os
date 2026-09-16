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
| 8 | Sem P0/P1 abertos | [findings.md](findings.md) | **✓** |
| 9 | Rollback rápido provado (RTO medido ao vivo) | script escrito; `--list` verificado; **flip ao vivo por exercer** | **✗** |
| 10 | Backup/restauro provado nesta baseline | ensaio de 2026-08-29 existe; **não re-exercido nesta pass** | **✗** |
| 11 | Jornadas E2E — parciais fechadas (J4/J9/J11/J15/J17/J18) | [04](04-e2e-matrix.md) | **✗** (parciais) |
| 12 | Aceitação visual autenticada em produção | precisa do Fidel | **pendente (humano)** |
| 13 | Revisão dedicada de HTTP-headers/CSRF/XSS/SSRF por fixture hostil | testes existentes cobrem parte; fatia própria por correr | **✗** |

## Porque não está declarado

Sete critérios estão provados (1-8). Faltam: o **rollback ao vivo** (9, bloqueado
pela permissão de mutação de produção), o **re-exercício de backup/restauro** nesta
baseline (10), o **fecho das jornadas E2E parciais** (11), a **aceitação
autenticada** que é do humano (12), e uma **fatia de segurança dedicada** por
fixture hostil (13).

Declarar READY agora seria inventar evidência (§59, §69). O portão fica **fechado**
até estes critérios terem prova executável.

## Como se ligaria a evidência executável

Quando os critérios 9-13 fecharem, este portão deve ser um verificador que:
recusa se as suites canónicas não passam, se há P0/P1 aberto, se a manifesto de
evidência não está actual, se produção ≠ `main`, ou se a prova de rollback/backup
não existe — tal como os outros portões de `main` correm contra evidência, não
contra prosa.
