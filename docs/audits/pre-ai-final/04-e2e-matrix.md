# 04 — E2E & Coverage Matrix (Pré-IA)

Uma suite de browser E2E **real** existe — `apps/workspace/tests/browser.rs`, que
conduz o Chrome (`chromiumoxide`), arranca Core + Workspace por teste, e emite a
marca de execução `VIAGEM LEVANTADA` (106 viagens activas; 4 `#[ignore]` são
geradores de captura, não verificação). Corre na CI com BD + MinIO. O resto da
cobertura é integração Core/HTTP.

**Limite honesto:** estas jornadas correm contra **base efémera na CI**, não contra
produção autenticada — não me autentico em produção. A aceitação visual
autenticada em produção é do Fidel.

## Jornadas canónicas → cobertura

| J# | Jornada | Browser E2E | Nota |
|---|---|---|---|
| J1 | Primeiro login (temporária→definitiva) | **COBERTA** | + arranque/login |
| J2 | Admin de membro + unidade | **COBERTA** | detalhe, faixa privilegiada, recarregar |
| J3 | Pertença a Research Workspace | **COBERTA** | líder gere; não-líder não recebe controlos |
| J4 | Ideia → projecto | **PARCIAL** | criação de ideia coberta; a *promoção* a projecto não é exercitada no browser |
| J5 | Ciclo de vida de Notas | **COBERTA** | escrita, clobber, imagem, pesquisa, etiquetas, pastas, partilha, histórico, lixo |
| J6 | Ficheiros pessoais | **COBERTA** | percorrer, largar, retomar em partes |
| J7 | Preview/conversão de Ficheiros | **COBERTA (preview)** | extracção+citação; a *conversão* de miniatura prova-se no host (11) |
| J8 | Autorização de Ficheiros institucionais | **COBERTA** | vista agregada, IDOR no browser, imagem same-origin |
| J9 | Quota de recursos | **em falta (browser)** | coberta no Core (`resource*.rs`) |
| J10 | Mail inbox/ler/não-lido | **COBERTA** | ligar caixa, arrumar sem partir, rolar por dentro |
| J11 | Mail compor/rascunho/anexo/enviar | **PARCIAL** | rascunho+envio no browser; anexo coberto no Core (`mail_attachments`) |
| J12 | Calendário | **COBERTA** | ~20 viagens: marcar, editar, cancelar, DST, fuso, lembrete pelo worker |
| J13 | Mensagens | **COBERTA** | conversa, grupo, sino |
| J14 | Bibliografia | **COBERTA** | validar (WASM), partida explicada, hostil como texto |
| J15 | Dataset | **PARCIAL** | criação coberta; versões só no Core |
| J16 | Conhecimento | **COBERTA** | navegação por pertença, cadeia científica |
| J17 | Pesquisa/Criar global | **PARCIAL** | criar por-módulo coberto; superfície unificada de comando não tem viagem própria |
| J18 | Prompt zero-IA | **em falta (browser)** | coberta no HTTP (`prompt_http`) — 200 tipado degradado |
| J19 | Prompt com contexto de ficheiro autorizado | **em falta (browser)** | coberta no Core (`agentic_file_content`) |
| J20 | Prompt nega contexto não autorizado | **em falta (browser)** | coberta no Core (`agentic`/`authorization`) |
| J21 | Segurança/revogação de admin | **COBERTA** | revogar sessão, MFA E2E, código de recuperação, suspensão a meio da sessão |
| J22 | Backup/restauro | operacional | scripts `institutional-backup/-restore/-verify`; não é jornada de browser |
| J23 | Deploy/rollback | operacional | `deploy-production.sh`, `rollback-production.sh`; não é jornada de browser |

**Resumo:** 13 cobertas no browser, 4 parciais (J4/J11/J15/J17), 6 em falta no
browser (J9/J18/J19/J20 cobertas ao nível Core/HTTP; J22/J23 são operacionais por
natureza).

## Lacunas de browser E2E que valem a pena fechar

Trabalho declarado desta auditoria, não bloqueante do determinista:

- **J18 Prompt zero-IA no browser** — provar na UI que o composer aceita, a
  resposta chega tipada (SYSTEM/DEGRADED) e o composer continua usável. Hoje só ao
  nível HTTP.
- **J9 quota no browser** — «Meus Recursos» a mostrar uso/limite/estado.
- **J4 promoção ideia→projecto**, **J11 anexo de mail no composer**, **J15 versões
  de dataset** — completar as parciais.

## Contrato de enumeração

`scripts/test-enumeration.sh` impede uma suite de passar por verde sem correr:
para cada suite crítica exige `esperados == passados`, `descobertos == passados +
ignorados`, `saltados == 0`, e `marcas de execução == execuções esperadas`
(browser: 106 viagens == 106 marcas `VIAGEM LEVANTADA`). Os `esperados` vivem numa
tabela deliberada; um número só muda por decisão. A CI adiciona um piso
(`MINIMUM_WORKSPACE_TESTS`) e falha se aparecer qualquer `SKIPPED/SALTADO`.
