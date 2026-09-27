# Ocinye Terminal e ocsh — arquitectura

> Proposta M0. As decisões com impacto arquitectural ficam na ADR-0312 (ocsh) e
> na ADR-0610 (a aplicação Terminal). Este documento é o mapa.

## Terminologia (congelada)

| Termo | É | Não é |
|---|---|---|
| **Ocinye Terminal** | a aplicação gráfica (Workspace) | uma consola do anfitrião |
| **ocsh** | a Ocinye Shell: gramática, parser, registo de comandos, execução governada | bash, sh, zsh ou um intérprete de scripts genérico |
| **Comando** | uma instrução tipada do ocsh (`files mkdir "Relatórios"`) | uma linha passada a um processo |
| **Capability** | a acção governada do Core que o comando invoca | o comando |
| **Host Shell** | um conceito futuro, separado, privilegiado, de operador | parte deste programa |

## A regra

```text
UI ─────────┐
Nye ────────┼──►  mesma Capability  ──►  mesma Core Operation
Terminal ───┘
```

O Terminal é **mais um cliente do Core**. Não tem autoridade própria, não tem
atalhos, e não conhece o anfitrião.

## Fluxo

```text
Terminal (browser)
  │  linha de texto + contexto do separador
  ▼
Workspace BFF  POST /terminal/exec        (sessão no servidor, Origin verificado)
  │
  ▼
Core  POST /api/v1/commands/exec
  │  1. parse determinístico (ocsh)       → AST tipada, ou erro de uso (exit 2/127)
  │  2. registo de comandos               → definição: família, sub, args, opções, capability
  │  3. invocação tipada                  → CapabilityRequest
  │  4. executor agentic                  → autoridade fresca · política · esquema · risco · auditoria
  │     └ risco que exige confirmação     → ActionPlan (digest, pessoa, 15 min, uso único)
  │  5. pipeline tipado (se houver)       → filter / sort / head / count sobre a saída
  ▼
resultado estruturado  {exit, blocks[], capability, audit_id, ms}
  ▼
Terminal renderiza (tabela, linhas, JSON, nota, confirmação) — sempre por nós de texto
```

**O parse autoritativo é o do Core.** O cliente pode ter uma projecção do
registo para ajuda e autocompletar imediatos, mas nunca decide o que executa.

## Onde vive o código

| Peça | Crate | Porquê |
|---|---|---|
| Lexer, parser, AST, registo de comandos (dados puros), códigos de saída, redacção de argumentos sensíveis | `ocinye-contracts` (módulo `ocsh`) | partilhado pelo Core, pelo Workspace e por um futuro CLI `ocinye`; sem IO |
| Execução: AST → `CapabilityRequest`, pipelines, confirmações, contexto | `ocinye-core` (módulo `terminal`) | só o Core decide |
| Rotas `/api/v1/commands/*` | `services/core-server` | como as outras |
| Página `/terminal`, BFF, JS do terminal | `apps/workspace` | cliente |

## Decisões principais

1. **Só capabilities.** Um comando não-local invoca exactamente uma capability
   do registo agentic (ou uma sequência tipada delas). Onde falta a capability,
   cria-se — com descritor, permissão, risco e testes — e ela passa a servir
   também o Nye. Não há executor paralelo nem chamada directa a serviços.
2. **Parser determinístico.** Nenhum modelo lê a linha. Comando desconhecido é
   `127`, e nunca vai para o Nye. Só `nye …` (e `? …`) entra em linguagem
   natural, e o Nye propõe — não executa.
3. **Sem anfitrião.** Não há `sh -c`, `exec`, globbing, substituição de
   comandos, redirecção para ficheiros, nem variáveis do processo. `sudo`,
   `bash`, `ssh`, … são reconhecidos e recusados (`126`).
4. **Confirmações = planos.** Risco médio/alto cria um `ActionPlan` de um passo;
   a confirmação é a aprovação existente (pessoa + digest + 15 min) e a execução
   é o `execute` existente (uso único por `UPDATE` condicional). A palavra
   escrita (REVOGAR/REVOKE/RÉVOQUER) é exigida pelo Core, não pelo cliente.
5. **Contexto por separador.** Cada sessão de terminal leva o seu contexto
   (pessoal ou um ambiente); o Core reautoriza-o em cada comando.
6. **Espaço de nomes virtual.** `~` é o contexto; `~/files` é o armazenamento do
   Ocinye resolvido por IDs. Nenhum caminho vira caminho do anfitrião.
7. **Saída estruturada.** O Core devolve blocos tipados; o cliente nunca
   interpreta HTML vindo do Core.
8. **Sem Gestor de Janelas não há `window`/`desktop`.** `open` navega para a
   aplicação (nova janela do browser) até o G-05 existir.

## Fases (M0–M10)

| Fase | Conteúdo | Depende de |
|---|---|---|
| M0 | discovery, arquitectura, modelo de comandos | — |
| M1 | lexer + parser + registo (contracts), testes de propriedade e negativos | — |
| M2 | execução no Core, sessão e contexto, `whoami`/`context` | M1 |
| M3 | ajuda, histórico (sessão), autocompletar governado | M2 |
| M4 | comandos de leitura (`apps`, `projects`, `tasks`, `storage`, `system`, `ai`) | M2 |
| M5 | `files` (capabilities novas), `apps pin/unpin`, `open` | M4 |
| M6 | mutações com confirmação por plano | M5 |
| M7 | ponte Nye explícita | M6 |
| M8 | pipelines tipados, depois `.ocsh` limitado | M4 |
| M9 | integração do D14 (visual) | M3 |
| M10 | E2E, segurança, navegadores, locales, certificação | tudo |

Streaming e Ctrl+C sobre trabalhos longos (G-12) e elevação de sessão (G-14)
dependem de infra que ainda não existe (cancelamento de trabalhos, sessão
elevada) e entram como fatias próprias com ADR.
