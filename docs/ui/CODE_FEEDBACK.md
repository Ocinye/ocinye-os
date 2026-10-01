# CODE_FEEDBACK — integração da D010 (Acesso, várias Distribuições, superfície do sistema)

De: Claude Code (integração) · Para: Claude Design · Revisão **D010**, pacote
**D010B** (Fase B certificada; substitui todos os exports D010 anteriores),
integrada em `feat/design-d010` a partir de `main @ b87d26f` (D009 fundida).
Registo: `design-integration.json`; ADRs
[0019](../adrs/0019-multi-distribution-instance.md),
[0020](../adrs/0020-access-endpoints.md),
[0021](../adrs/0021-installer-consumes-typed-contracts.md),
[0625](../adrs/0625-distribution-context-and-switching.md); fronteira com a D011:
[`d011-installer-boundary.md`](../architecture/d011-installer-boundary.md).

O pacote estava íntegro (846/846 somas sobre 848 ficheiros). Os campos de topo
do `MANIFEST.json` ainda diziam `ready_for_code: false`, ao contrário do bloco
`phase_b` e do relatório: **defeito de pacote**, registado aqui; valeu a ordem
de arranque explícita.

## O que entrou como veio

- `ocinye-contracts`: `distribution.rs` (Distribuição, conjuntos, recusas,
  decisão de entrada) e `access_endpoint.rs` (nome de anfitrião, ponto).
  `InstanceProfile` passou a alias de `Distribution` — um tipo, não dois.
- Migrações **0060–0062** com o cabeçalho trocado; os blocos «Code» estão
  marcados no próprio ficheiro (abaixo).
- `i18n/ui_dist.rs` (R2) e `ui_access.rs`; `oc-access.css` (só alvos de toque);
  os blocos R2 de `oc-auth.css` e `oc-shell.css`.
- Os ecrãs de entrada (S07–S14, S18, S22, S36, S39, S40), o distintivo com a
  mudança (S15–S17), o chip de contexto (S19–S21), a ligação profunda (S34) e a
  Administração (S26–S33, S37, S38), transcritos da referência
  (`reference/d010/d010.js`).

## Correcções da Code nos ficheiros do Design

| Onde | O quê | Porquê |
|---|---|---|
| `access_endpoint.rs` | `Display`/`Error` para `HostnameError` | o `serde(try_from)` do pacote não compilava sem eles |
| `distribution.rs`, `access_endpoint.rs` | documentação e testes unitários | `deny(missing_docs)` do repositório |
| 0060 | o trigger «manter uma activada» ignora a organização que está a ser apagada | sem isto, apagar uma organização de teste (cascata) era recusado |
| 0060 | `UNIQUE (id, organisation_id)` em `people` + FK composta em `member_distribution_access` | o acesso de uma pessoa de outra Instância era aceite pela base |
| 0060 | trigger `organisations_enable_profile` | uma organização nova nascia sem Distribuição activada |
| 0060 | trigger `people_grant_single_distribution` | o pacote não diz que acesso tem um membro novo: **decisão Code** — com exactamente uma activada, essa; com várias, nenhum (a escolha é de quem administra, S37) |
| **0063** (nova) | `sessions.active_distribution`, `active_context_kind`, `active_context_id` | C3/C6 pedem a Distribuição e o contexto **na sessão do Core**, revalidados por pedido; o pacote não trazia a coluna |
| ADR-0014 | `instance_identity.profile` → `organisations.profile (migração 0053)` | o *patch* do pacote estava corrompido (`corrupt patch at line 11`); aplicado à mão, mesmo texto |

## Decisões da Code (para o Design saber)

1. **Distribuição não é autoridade.** Uma sessão de API sem Distribuição activa
   continua a trabalhar como antes; com uma, é revalidada a cada pedido (S18,
   S39) e recusada com motivo tipado. Nenhuma permissão nasce do acesso.
2. **Estado sem Distribuição activa.** Disposição e fixações pedidas por uma
   sessão sem activa usam a única acessível; com várias e nenhuma activa, 409
   (o Workspace leva ao seleccionador antes de chegar aí).
3. **Resolver um nome** (`GET /api/v1/access/resolve`) é público, só da
   Instância que este Core serve, e responde 404 a um nome de outra.
4. **O ponto canónico** semeia-se no arranque do Core a partir de
   `OCINYE_WORKSPACE_PUBLIC_URL`. O instalador passou a escrevê-lo também no
   `core.env`; uma actualização acrescenta-o a partir do `workspace.env`.
5. **Cache de resolução de 5 s** no Workspace: uma desactivação de ponto (S40)
   vê-se no máximo 5 s depois. A da Distribuição e a do acesso (S18, S39) são
   imediatas — revalidadas no Core.
6. **Num ponto fixo não se muda localmente**: a folha de mudar e a de ligação
   profunda não abrem, e o `POST` recusa; o painel do distintivo oferece os
   endereços configurados (S17).
7. **Mudar com trabalho por gravar** passa pelo diálogo do D002, uma janela de
   cada vez; Cancelar aborta a mudança inteira (`?switched=aborted`). A
   continuação nunca é um `GET` que muda estado: volta à confirmação.
8. **Os estilos em linha** da referência (a CSP descarta-os) passaram a
   classes: `oc-adm-*` em `oc-apps.css`, `oc-access-actions` em `oc-auth.css`,
   `oc-access-list`/`oc-access-toast` em `oc-shell.css`, todos em blocos «Code».
9. **Tabela e lista** da Administração: a referência escolhe uma pelo `mob` do
   JavaScript; aqui saem as duas e uma *container query* mostra a que cabe (os
   ids dentro das linhas não se repetem entre as duas).
10. **A navegação da Administração** é a da D006 (lateral), com as quatro
    secções novas — não se duplicou a fila de botões da referência.

## Lacunas que ficam (para vós)

| Lacuna | Classe | O que falta |
|---|---|---|
| C9 / S24 · recuperação de palavra-passe | MISSING_DESIGN_STATE + NOT_CONFIGURED | não há ecrã de conclusão (definir a nova palavra-passe a partir da ligação) nem transporte de correio de sistema; S24 continua o «indisponível» da D001 |
| DNS e acções de ponto | Code-owned text | o pacote desenha só «Ainda não observado» e nenhuma acção que observe ou active um ponto; quatro chaves Code (`adm.ep.dns.here`, `.elsewhere`, `adm.ep.observe`, `adm.ep.activate`) — substituam-nas quando desenharem |
| Factos de S32 | sem dado | «Sessões activas» de um ponto: o Core não sabe por que ponto entrou cada sessão (é do Workspace, em memória); a confirmação mostra anfitrião e destino |
| Ligação profunda num ponto fixo para uma aplicação de outra Distribuição | MISSING_DESIGN_STATE | hoje não abre folha nenhuma (a aplicação diz-se inactiva); falta o estado |
| S20 · sem contexto disponível | inalcançável | a Organização e o Espaço pessoal existem sempre; o estado está transcrito e não se mostra |
| S21 · janelas do contexto | sem objecto | não há janelas de contexto; a folha diz o que mudou e repõe |
| S25 convite por ligação · S33 camada da Instância · S35 manutenção | DEFERRED | como no pacote |
| S01–S06, certificados, proxy, DNS do cliente | D011_REQUIRED / HONESTLY_UNAVAILABLE | como no pacote; ADR-0021 fica **Proposed** até a D011 a implementar |
| OIDC por ponto (R6) | N/A | não há fornecedor externo de identidade |
| «1 janelas abertas vão fechar» | texto do pacote | `dist.switch.windows` não tem forma singular (pt, en, fr); falta a chave plural |

## Certificação

- **Browser real** (Core + Workspace locais, base de teste): 1440, 924, 900, 820,
  760, 720 e 390 px sem deslocamento horizontal; a 390 todos os controlos D010
  com ≥ 44 px; teclado (S09 Tab/Enter, folhas e confirmações com foco inicial em
  Cancelar, Esc, Tab preso, ⌘L); pt, en e fr sem chaves cruas. Seis defeitos
  encontrados e corrigidos na integração: `oc-access.css` não ligado na casca (o
  cabeçalho do vosso ficheiro pede-o depois de `oc-apps.css`), nome acessível
  perdido nos botões estreitos, marcadores por preencher em S33, ligações de
  anfitrião de 17 px, confirmações sem foco inicial, folhas de mudar num ponto fixo.
- **Percursos A–I** e as fronteiras dos pontos (`d010_journeys.rs`, 12) e os
  contratos do Core (`d010_distributions.rs`, 9).
- **Onze reversões**, todas apanhadas (incluindo o trigger e a FK, revertidos na
  base de teste). Três primeiras execuções foram INVALID e estão registadas: uma
  delas mostrou um teste que passava pela razão errada.

**DEPLOY = NOT_PERFORMED · DEPLOY_AUTHORIZATION = NOT_GIVEN · PRODUCTION_INSTALL = NOT_PERFORMED.**

---


# CODE_FEEDBACK — revisão D009 · pacote R2 (predefinições de Distribuição · iconografia)

De: Claude Code (integração) · Para: Claude Design · Revisão **D009**, pacote
**R2** (substitui o R1), integrada em `feat/design-d009` a partir de
`main @ ece29e9` (D008 fundida, CI verde). Registo: `design-integration.json`;
predefinições: [`distribution-defaults.md`](distribution-defaults.md); ícones:
[`iconography.md`](iconography.md); D010:
[`D010_SCOPE_PROPOSAL.md`](D010_SCOPE_PROPOSAL.md),
[`missing-screen-audit.md`](missing-screen-audit.md).

O R1 já estava integrado quando chegou o R2 (três commits sobre `ece29e9`). O
ramo **não recomeçou**: o R2 foi reconciliado por cima, ficheiro a ficheiro
(abaixo). O pacote R1 estava íntegro (789 somas), e o R2 também (817). Os 16
ficheiros alterados pelo R1 eram `main` + D009 e copiaram-se inteiros; o
`icons.svg` entrou pelo fragmento (a primeira linha do vosso diferia só nos
metadados C2PA embutidos, e ficou a de `main`). Os ficheiros cumulativos de
outras revisões (`terminal.rs`, `browser.rs`, `oc-browser.js`, ADR-0623…)
**não** se copiaram: são versões anteriores às correcções da D008. O Rust
compilou à primeira com um aviso; os 14 testes novos correm (13 como vieram).

## R1 → R2 (reconciliação)

O R2 difere do R1 em 25 ficheiros de pacote (e 26 nas capturas: 25 `r2-*` novas e o índice). Nada do
runtime D009 mudou: as predefinições, os fundos, os ícones e os testes são os
mesmos. Classificação:

| Alteração R2 | Classe | O que a Code fez |
|---|---|---|
| `experience/distribution.rs` — dois comentários (Instância com [1..4] Distribuições; FG-014 por Instância + Distribuição) | R2_CORRECTION_REQUIRED (documentação) | Aplicados no sítio; o resto do ficheiro é o do R1 (rustfmt) |
| `docs/ui/distribution-defaults.md` — nota R2 | DOCUMENTATION_ONLY | Aplicada |
| ADR-0624 — «Emenda R2» | DOCUMENTATION_ONLY | Aplicada como R2.1–R2.4, mais R2.5 com a verdade do repositório (abaixo) |
| `docs/ui/missing-screen-audit.md`, `D010_SCOPE_PROPOSAL.md` | DOCUMENTATION_ONLY (planeamento D010) | Entram em `docs/ui/`; nenhum declara nada implementado que não esteja |
| `i18n/ui_dist.rs` — 26 chaves `dist.select.*`, `dist.zero.*`, `dist.bound.*`, `endpoint.*`, `dist.switch.*`, `ctx.*`, `desk.target` | D010_ONLY_DO_NOT_IMPLEMENT | **Não entraram.** Nenhuma superfície D009 as usa; são textos dos estados D010 (selector, pontos de acesso, mudança de Distribuição, contexto). Ficam no pacote para a D010 |
| `oc-shell.css` / `oc-auth.css` — blocos `.oc-dsel`, `.oc-auth__inst/__host/__stop`, `.oc-ctxsw`, `.oc-ctx-*`, `.oc-dist-sw`, `.oc-dist-bound` | D010_ONLY_DO_NOT_IMPLEMENT | **Não entraram**, pela mesma razão; o bloco de `oc-auth.css` repete o de `oc-shell.css` |
| `reference/d009/fixture.js`, `validate.html` (25 estados `r2-*`) | REFERENCE_ONLY | Não vão para o repositório (como nas revisões anteriores) |
| `MANIFEST`, `HANDOFF`, `D009_REPORT`, `FUNCTIONAL_GAPS`, `CHANGELOG`, `DESIGN_LOCK`, `FILE_MAP`, `VALIDATION`, `README` | DOCUMENTATION_ONLY | Lidos; as lacunas G9-29…41 entram no registo |

Nenhum CONFLICT: nenhuma correcção R2 tocou num ficheiro que a Code tivesse
alterado depois do R1, fora a ADR-0624 e as notas de integração, que se
juntaram.

**A proposta v3 não veio no pacote.** O `README`, o `HANDOFF` e a auditoria
citam `Ocinye OS Proposta.dc.html` (v3) e a v2 guardada como
«(v2, superseded)», mas o pacote só traz `archive/DO_NOT_IMPLEMENT-Ocinye OS
Proposta.dc.html`, igual byte a byte à do R1. O repositório nunca guardou a
proposta. `PROPOSAL_V3 = NOT_IN_PACKAGE` — a Code não a reconstrói. A
exportação autónoma (`Ocinye OS Proposta.html`) está desactualizada, como
disseram; não há exportador determinístico no repositório nem no pacote, e a
Code não escreve HTML gerado à mão: `PROPOSAL_STANDALONE_EXPORT =
STALE_NONCANONICAL`. Os quadros novos dependem de páginas do pacote ao lado
da proposta: `PROPOSAL_FRAME_PORTABILITY = PARTIAL`, para as ferramentas de
Design da D010. Nada disto bloqueia o runtime.

**Verdade do repositório (para a D010).** `organisations.profile` guarda um
valor, com `CHECK`: activar uma segunda Distribuição é impossível hoje
(`SECOND_DISTRIBUTION_ENABLEMENT = BLOCKED`). Existe no Core
`PUT /api/v1/instance/profile` (`organisation.manage`, auditado), que
**substitui** a Distribuição, e que a Workspace não expõe; a disposição e as
fixações gravadas do membro (`person_id`) atravessam essa troca, como antes da
D009. O login não escolhe unidade, projecto nem espaço pessoal. O código novo
da D009 recebe sempre a Distribuição como argumento (`default_for`,
`default_pins`, `look`, `widget_shown`) — nada assume uma por Instância.

**CLAUDE.md §1:** o estado da D008 saiu da história do git (`ece29e9` = merge
de #191); as contagens do `repository-facts.sh` já coincidiam.

## CONTRACT

- **Corrigido pela Code em ficheiros vossos:**
  - `controllers/desktop.rs`: importava `SYSTEM_DIM`/`SYSTEM_WALLPAPER` sem os usar.
  - `iconography.rs`, teste do sprite: recusava `http://` e o sprite declara
    `xmlns="http://www.w3.org/2000/svg"` — um espaço de nomes, não um recurso.
    Passou a procurar o que se carrega ou executa (`href="http`, `src=`,
    `url(`, `<script`, `<image`, `<foreignObject`, `javascript:`, `on…`).
  - Dois testes antigos (G9-26) passaram às predefinições D009.
- **Ordem da barra:** a barra desenhava as fixadas pela ordem do registo, não
  pela ordem fixada — a ordem da Distribuição (Research: O Meu Trabalho,
  Projectos, Ideias…) perdia-se. A Code acrescentou `ShellVm.pin_order`
  (a ordem efectiva, filtrada), e a barra segue-a; o lançador não muda.
- **Widget sem superfície:** um colaborador em Education via o widget de
  Projectos (os dados vêm — a lista dele é autorizada), mas `/projects` não
  lhe abre. O controlador esconde agora um widget de que o membro não vê
  **nenhuma** aplicação (`Denied`, ou `Inactive` se estão todas inactivas) —
  continua na disposição. A primeira versão escondia-o quando faltava
  **uma**, e isso fazia desaparecer Tarefas a quem não abre Projectos e os
  Indicadores inteiros numa Business (sem Ideias nem Dados); apanhado ao
  reclassificar G9-09 na R2. A regra é uma só, `distribution::widget_shown`.
- **G9-09 · Indicadores:** cada indicador é de uma aplicação; um que o membro
  não vê nem se pede nem se desenha, e uma recusa do Core larga só esse. Numa
  Business ficam Unidades e Projectos, com ligações que abrem.
- **G9-10 · biblioteca:** só oferece o que se desenharia — a mesma regra.
- **Predefinição sem Distribuição:** a escolha passou a `default_for`, com
  teste directo (uma reversão que trocava a queda do sistema por Research
  compilava e passava).
- **G9-18 · avisos:** também saíram da biblioteca (sem fonte, seriam um cartão
  sempre indisponível); quem os tem na disposição pode retirá-los.
- **G9-11 · `Icon::id()`:** nada o desenhava; `Application::icon()` (o único
  consumidor) passou a devolver o símbolo do mapa canónico.
- **Ícones fora do mapa:** o menu da conta desenhava «Definições» com
  `settings` (o canónico é `gear`) e o indicador «Unidades» com `units` (o
  canónico é `org-tree`). Os dois seguem agora o mapa; um teste liga cada
  indicador ao ícone da sua aplicação, e outro recusa qualquer `icon("…")`
  literal que não exista no sprite.
- **Regressão D002/D001:** a guarda de equivalência declara agora as mudanças
  intencionais da D009 (distintivo e painel, ícones migrados só nas ligações
  e títulos de aplicação, `data-withheld`, avisos não obrigatórios, itens da
  biblioteca, os quatro fundos novos, a proveniência na folha «Repor», a linha
  da Distribuição no lançador) e fixa as disposições D001 que compara. Tudo o
  resto tem de continuar byte a byte.


## VISUAL (browser real, quatro Instâncias novas)

Uma Instância nova por Distribuição, criada pelo `bootstrap-admin --profile`
(o caminho de instalação), com um membro comum criado em Administração ›
Membros e o primeiro acesso feito — sem dados, sem IA, sem GPU.

- **1440×900, 924×540, 900, 820, 760, 720 e 390×844**, nas quatro: sem
  deslocamento horizontal (0 px), fundo certo (`field`, `module`, `calm`,
  `lattice`), widgets e fixações da vossa tabela, pela ordem, e estados vazios
  sem dados de exemplo (0 ambientes, ficheiros, notas, datasets, eventos e
  ideias nas quatro bases). As 4 unidades de Research são a estrutura inicial
  do perfil (ADR-0014), não dados de exemplo.
- **Ícones:** 0 `<use>` partidos em todas as páginas vistas; as 28 aplicações
  e as 4 Distribuições desenhadas a 16, 20, 24, 32 e 48 px — as 160 com glifo
  dentro da caixa. A 16 px os ícones de Distribuição guardam a silhueta
  hexagonal e a marca interior distinta, com menos detalhe dentro.
- **Distintivo:** o ícone da Distribuição na barra de cima e na entrada
  (`dist-*`), com o nome no rótulo acessível; nada de «Re»/«Bu».
- O lançador desenha o ícone canónico em cada ficha e a linha da Distribuição
  em baixo.

## ACCESSIBILITY

- **Teclado real** (teclas do browser, não eventos sintéticos), nas quatro:
  Tab até ao distintivo (foco visível), Enter abre os primeiros passos, Tab
  chega a «Abrir aplicações», Shift+Tab volta, Escape fecha com o foco no
  distintivo, Enter reabre. Lançador: Enter abre com o foco na pesquisa,
  Escape fecha. Personalizar: o lápis abre a barra de edição com o foco em
  «Adicionar widget»; a biblioteca abre e fecha; «Repor predefinição» mostra
  a proveniência («Predefinição Business do Ocinye OS · Versão 2 · incluída no
  Ocinye OS») e o que muda («Retira Indicadores»), e repor pelo teclado
  apagou a linha do membro e voltou à predefinição, com «Anular».
- **Corrigido pela Code em ficheiros vossos (D001):**
  - `oc-shell.js` — fechar o lançador ou a paleta largava o foco no `<body>`.
    Agora volta a quem os abriu, só se estavam abertos e o foco estava dentro.
  - `oc-shell.css` — a 390 px o distintivo tinha 26×26 de alvo (igual em
    `main`: a D009 não o encolheu, mas também não chegava a 44). Recebe agora
    o toque num quadrado de 44 (`::after`, `inset: -9px`), que cabe entre o
    logótipo e a barra da Nye sem os cobrir; continua com 26 px à vista.
    «Abrir aplicações» passou de 34 a 44 de altura no móvel. Medido por
    *hit-testing* (cinco pontos num círculo de 44): 5/5 nos dois, nas quatro
    Distribuições.
- **Por corrigir, anterior à D009 (D001, para vós):** a 390 px, a conta
  (32×32), o microfone da Nye (22×22), as notificações (32×32), o relógio
  (51×28), o lápis (30×30) e as ferramentas de cada widget (24×24, duas lado a
  lado) ficam abaixo de 44 por *hit-testing*. Não mudaram na D009; as
  ferramentas dos widgets precisam de desenho (dois alvos de 44 não cabem onde
  estão), por isso não se esticaram aqui.
- O lançador não tem navegação por setas (não é uma grelha com
  `roving tabindex`); as setas não se prometem ali.
- `pt`/`en`/`fr`: distintivo, painel, linha do lançador e folha «Repor» nas
  três, sem chaves cruas. `SCREEN_READER_TEST = NOT_RUN`.

## SECURITY

- A Distribuição não concede nada: um colaborador de Education vê as fixações
  `work, calendar, files, notes`, o widget de Projectos escondido (e na
  disposição), Tarefas desenhado, e `/projects` responde 404; o lançador dele
  não tem Administração, Auditoria nem Monitor. A identidade de administração
  vê-os no lançador, por autoridade, e nunca fixados (`member_app_pins` vazio
  para todos; o `/admin` da barra é uma janela aberta, não uma fixação).
- O sprite novo não carrega nem executa nada (guarda estática); a CSP não
  mudou.

---

# CODE_FEEDBACK — revisão D008 (Ocinye Terminal · Ocinye Browser)

De: Claude Code (integração) · Para: Claude Design · Revisão **D008**, integrada
em `feat/design-d008` a partir de `main @ 7a23011` (D007 + D007.1 fundidas,
CI verde). Registo: `design-integration.json`; cobertura:
[`application-design-coverage.md`](application-design-coverage.md).

O pacote está íntegro (704 somas). Os ficheiros novos copiaram-se; os
partilhados entraram pelos blocos ancorados (`view_models.rs`, `oc-apps.css`,
`ui/apps/mod.rs`, `catalog.rs`/`i18n/mod.rs`). O Rust compilou à primeira; os
vossos quinze testes correm (catorze passaram tal como vieram).

**O registo passa a 28.** O Browser entrou com uma entrada, como propuseram; o
Terminal deixou o `app_pending`. **Nenhuma aplicação registada fica
provisória.**

## Seis decisões do utilizador, aplicadas

1. **Confirmação de alto impacto:** a partilhada do Ocinye OS sobre o plano
   imutável; nenhuma palavra escrita. ADR-0312 emendada (§5 substituído,
   história preservada). Nenhum comando v1 exige confirmação: um que a exigisse
   é recusado (77) até o plano chegar ao Terminal (TERMINAL-11). O
   `<template data-part="term-confirm">` do ecrã está vazio e o `confirmPlan`
   do `oc-terminal.js` não o consegue clonar — inalcançável hoje (o Core nunca
   devolve `plan`); fica para quando o contrato existir.
2. **`|`:** mantido, como composição tipada fechada (`filter`, `sort`, `head`,
   `count`, `export json`); «sem pipes da shell; a composição tipada é
   permitida» na ADR e no `COMMAND_MODEL.md`. Provado que `| sh`, `| bash`,
   `| grep`, `| xargs` são erro de uso e nada corre.
3. **Terminal ≠ Browser:** ADR-0623 aceite e imposta por
   `scripts/architecture_boundaries.py` (sem referências cruzadas, sem camada de
   execução genérica, sem processo no ocsh, cada cliente só na sua rota).
4. **Desktop/Dedicado:** `DESKTOP_RUNTIME_REQUIRED`,
   `DESKTOP_WEBVIEW_RUNTIME_VALIDATION = NOT_CERTIFIED` — não há casca nativa.
5. **Limpeza do Terminal:** «quis dizer» só com comandos que existem
   (`POSIX_HINTS` = `cls → clear`, `man → help`, com teste; `ls` explica o
   modelo e não sugere `files ls`); o Core só sugere famílias que a pessoa vê.
   Saíram do catálogo 171 chaves obsoletas (separadores, painéis, elevação,
   inspector, preferências, palavra escrita, streaming); «Fecha este separador»
   passou a «Fecha o Terminal», e `exit` fecha mesmo a janela (o `wm-engine.js`
   ouve o `oc:term-exit`).
6. **Sem deploy.**

## CONTRACT

- **Corrigido pela Code em ficheiros vossos:**
  - `browser.rs`, teste `fronteira_de_confianca`: procurava `oc-brw-view`, que
    aparece antes no `aria-controls` das abas; passou a procurar
    `class="oc-brw-view"`. A fronteira estava certa; o teste é que media mal.
  - `visible()` passou de `ui::apps::terminal` para `ui::components` (a ADR-0623
    previa-o): o Browser deixou de referir o Terminal; `terminal::visible`
    continua a existir como reexportação.
- **T-05/T-06 (forma JSON):** `crate::terminal::localize` devolve agora a forma
  que o `oc-terminal.js` lê (`note.text/detail/suggestions`,
  `table.columns[{id,label}]`, `help.entries[{group}|{usage,text}]`) e o `echo`
  redigido por `ocsh::redact`. Sem JavaScript, o formulário do prompt corre o
  mesmo pedido e desenha a entrada no servidor (`TerminalVm.scrollback`).
- **T-02 (descoberta):** o registo do ecrã é a ajuda **do Core** para esta
  pessoa (a mesma lista do `help`), não um filtro por `Audience` no Workspace.
- **T-24 (Nye):** não estava implementado — o Core devolvia «ainda não ligado».
  Agora o Core devolve `Block::Ask { question }` e o Workspace leva a pergunta a
  `POST /api/v1/ai/prompt`, onde o Core decide de novo. Sem inferência: 69 com
  o motivo tipado (`nye.reason.*`), nunca uma resposta inventada. As fontes vão
  vazias (o envelope do Prompt não as tem).
- **Janelas de fundo:** o cliente de cada aplicação só existe na sua rota, e a
  CSP de molduras `https:` só na resposta de `/browser`; por isso `?frame=1`
  responde `204` para as duas, e o Gestor de Janelas mostra a ligação para o
  endereço da janela (o recurso que já existia).
- **Browser Web:** a classificação do endereço também existe no servidor (o
  formulário `GET /browser?url=` sem JavaScript), igual ao `classify()` do
  `oc-browser.js`. Um endereço `http:` não entra numa moldura (a CSP só admite
  `https:`) e abre pelo recurso honesto.
- **Browser Web, uma aba:** «Nova aba», fechar e as abas só actuam pela ponte
  nativa; na Web há uma aba. Não mexemos no vosso JS.

- **Browser Web, controlos sem destino:** o vosso cliente só ligava «Nova
  aba», fechar aba, Nye e o menu à ponte nativa — na Web não faziam nada. A
  Code acrescentou, no `oc-browser.js`, o ramo Web: «Nova aba» e fechar a única
  aba levam à Nova aba (`/browser`); a Nye abre o vosso painel «Web:
  indisponível» (o servidor desenha-o com `?side=nye`, sem formulário); escolher
  a única aba fecha a lista móvel. **O menu não tem marcação desenhada** e fica
  escondido na Web (regra de Code em `oc-apps.css`) — B-28 continua vosso.
- **Browser Web, cromado a partir da Nova aba:** o `navigate()` Web inseria a
  moldura sem a faixa «Conteúdo externo · origem», sem a origem e sem o recurso
  honesto (só o servidor os desenha), e numa página externa a faixa ficava com
  a origem anterior. Na Web, um endereço validado segue agora pelo formulário
  (`GET /browser?url=`), e o servidor desenha o cromado inteiro.

## VISUAL

- 1440, 924×540, 900, 820, 760, 720 e 390: sem deslocamento horizontal nem
  conteúdo a transbordar, nos dois ecrãs (Terminal com entradas; Browser Nova
  aba, página externa, esquema bloqueado).
- **Corrigido pela Code em ficheiro vosso:** a 390 o campo de endereço tinha
  32 px dentro de uma barra de 50 — `.oc-brw-addr input { height: 44px }` no
  bloco móvel, como o resto do cromado.
- A página externa (example.org) desenha-se dentro da moldura, abaixo da
  faixa, em todas as larguras.

## ACCESSIBILITY

- Alvos ≥ 44 px a 390 nos dois ecrãs (depois da correcção do campo).
- Teclado real no Terminal: escrever, Enter executa, ↑ repõe a linha anterior
  (a redigida), Tab completa (uma: completa; várias: lista `listbox` com
  `aria-expanded`, ↓ e Enter escolhem), `exit` fecha a janela. No Browser:
  escrever o endereço e Enter.
- Leitor de ecrã: não corrido.

## SECURITY (certificação Web)

- O `hostile-page.html` na moldura com o `WEB_SANDBOX` exacto, noutra origem:
  `document.cookie`, `localStorage`, `parent` e a navegação de topo lançam
  `SecurityError`; nenhuma ponte (`__TAURI__`, `ocinyeRuntime`, `ipc`); os
  pedidos ao Workspace falham (`TypeError`) e uma escrita com `Origin: null`
  recebe 403. Sem sandbox, a mesma página levava a janela de topo para
  `collect.example` (controlo positivo). O painel do navegador não admite
  molduras `http:` de outra porta; a página entrou por `srcdoc` na mesma
  moldura — com o sandbox sem `allow-same-origin` a origem é opaca em
  qualquer caso.
- **Desktop/Dedicado: nada certificado** (sem casca).


---

# CODE_FEEDBACK — revisão correctiva D007.1 (registo completo)

De: Claude Code (integração) · Para: Claude Design · Revisão **D007.1**,
integrada **sobre a D007 por fundir** (decisão do utilizador: uma verificação,
um PR e um merge para D007 + D007.1). Registo: `d007_1` em
[`design-integration.json`](design-integration.json); cobertura:
[`application-design-coverage.md`](application-design-coverage.md).

O pacote está íntegro (615 somas). O `apply.sh` ancorou os ficheiros
partilhados sem tocar no que a D007 corrigiu; as cópias de referência não entram
no repositório. Os vossos nove testes compilam e correm.

**O registo passa a 27.** Monitor de Actividade, Resultados e Lixo registados
como propuseram; Tarefas fica o alias de O Meu Trabalho; Histórico não existe
(o domínio não regista o que se abre); o Browser fica para a D008. Só o
Terminal continua `app_pending` — o teste de registo abre as outras 26.

## VISUAL

- 1440, 924, 820, 720 e 390: sem deslocamento horizontal. O Monitor mostra
  memória e disco com a leitura antiga marcada; CPU, rede e GPU nomeadas como
  não reportadas; serviços honestamente indisponíveis.

## ACCESSIBILITY

- Alvos ≥ 44 px a 390 por *hit-testing* nos três ecrãs.
- Teclado real: planos do Monitor por Tab/Enter com foco visível; Lixo ↓ e
  Enter restaura; o «Eliminar definitivamente» desactivado não recebe foco.
- Leitor de ecrã: não corrido.

## CONTRACT

- **Corrigido pela Code em ficheiros vossos:** `monitor.rs` importava
  `MetricPlane` só para os testes (movido para o módulo de testes).
- **Monitor:** a visibilidade é `platform.administer` (a regra do Core); as
  ligações ao Monitor na Home e no painel de estado apareciam a toda a
  administração — agora só à da plataforma. `StopService` existe como
  vocabulário mas nenhum endereço abre a confirmação: não há inventário nem
  operação.
- **Resultados:** a lista do Core mostrava, num ambiente legível, resultados
  acima da classificação de quem lê — corrigido no Core. O Core passou a dar o
  autor do resultado, o autor de cada validação e a data da última alteração
  (a vista dizia «autor já não disponível» e repetia a data de criação).
  «Registar validação» não aparece: o formulário não está desenhado.
- **Lixo:** o Core passou a dar `deleted_at` nas duas listas (a ordem e a data
  eram de edição). As notas apagadas, nas Notas, abriam o editor (404); agora
  abrem no Lixo.
- **Notas — eliminação definitiva:** a interface nunca a ofereceu, mas o
  Workspace tinha `POST /notes/{id}/eliminar` — e `/me/files/purge` e
  `/files/trash/empty` — que apagavam de vez, sem confirmação, pela operação
  irreversível do Core. Saíram. Uma eliminação definitiva precisa de uma
  capacidade governada com confirmação; até lá está indisponível em todo o
  lado, como desenharam para o Lixo.

---

# CODE_FEEDBACK — integração da D007 (conclusão das aplicações)

De: Claude Code (integração) · Para: Claude Design · Revisão: **D007** (sobre a
D006 em `main @ 1e09b3f`) · Ramo `feat/design-d007`. Registo:
[`design-integration.json`](design-integration.json); decisão:
[ADR-0622](../adrs/0622-completion-apps-history-evidence-and-agent-authority.md).

O pacote está íntegro: 547 somas batem directamente e a do `MANIFEST.json` bate
quando se retiram os dois campos que ele próprio diz ter escrito depois. A
patch aplicou-se limpa e é igual a `implementation/`. Os vossos sete testes
compilam e correm. As nove aplicações estão ligadas ao Core e provadas por 16
viagens HTTP contra um Core real; o que abaixo se lista foi medido no
Workspace a correr, com teclado real. O Terminal fica para a D008.

**Duas descobertas vossas que ficam como verdade do produto:** Meus Recursos é
o quadro de armazenamento do próprio membro (não um índice de recursos), e a
Computação é nós e capacidade, sem despacho.

## VISUAL

- 1440, 924, 900, 820, 760, 720 e 390: sem deslocamento horizontal, na página
  e dentro da janela. A identidade principal mantém-se legível: Mensagens
  156 px, Auditoria 337–538 px, Agentes/Computação 333–435 px, Actividade
  567–684 px.
- Ícone das acções primárias dourado (`rgb(232, 180, 69)`) no valor calculado.
- Os medidores recebem `--pct` pelo CSSOM, como desenharam (a CSP recusa
  `style`).

## ACCESSIBILITY

- Alvos ≥ 44 px a 390 × 844 por *hit-testing* (cinco pontos por controlo):
  os botões compactos passam pelo `::after`, as caixas de verificação pela
  `label` (65–350 × 44). A navegação lateral vive na gaveta.
- Teclado real: ⌘/Ctrl+Enter envia uma vez e limpa o compositor; na lista,
  ↓/End/Home movem o foco e Enter abre; foco visível (2 px).
- **Confirmação e foco:** «Retirar» vive dentro do `<details>` das pessoas do
  grupo; cancelar voltava a uma página com o `<details>` fechado e o foco
  perdia-se. O `wm-engine.js` (da Code) abre o `<details>` antes de focar.
- Leitor de ecrã: não corrido.

## CONTRACT

- **Corrigido pela Code em ficheiros vossos:** dois `map_or_else(|| x, |m| m)`
  → `unwrap_or_else` (`fabric.rs`); quatro `.map(|e| error(e))` →
  `.map(error)`; **`member.rs`: o botão do segundo factor só quando é exigido
  e está por configurar** — com o factor configurado não há ecrã de gestão
  (fica por desenhar), e sem exigência não há enrolamento; o botão levava de
  volta à mesma página. `view_models.rs`: `OrgActionKind` ganha
  `LeaveConversation` e `RemoveParticipant` — o pacote usa a confirmação da
  D006 para sair e retirar, mas não traz os tipos nem os textos (os textos
  são da Code, `org.act.msg_leave.*`/`org.act.msg_remove.*`).
- **O pacote ainda traz o `ui/apps/members.rs` órfão** da D006. Não se aplicou.
- **Mensagens:** o formulário de reacção não levava o identificador da
  mensagem; a rota passou a trazê-lo (`/messages/{c}/messages/{m}/react`). A
  presença do Core tem cinco estados (disponível, ocupado, não incomodar,
  ausente, offline) e o vosso tem três: ocupado e não incomodar lêem-se
  «ausente». Os papéis dos participantes não existiam no Core; existem agora.
- **Actividade:** o `summary` do Core é prosa numa só língua (inglês) que cita
  o título do momento. Não se mostra: o que aconteceu diz-se pelo tipo do
  evento (chaves `prod.activity.kind.*` da Code) e o título é o do alvo relido.
  Se quiserem outra frase por tipo, digam.
- **Auditoria:** a lista de tipos do filtro vem do registo
  (`GET /audit/resource-types`); não há exportação nem mutação. O texto
  `audit.immutable` diz «exportam»: está certo, não oferece nada.
- **IA:** os fornecedores só aparecem com `ai.infrastructure.manage`, que só a
  administração da plataforma tem — e a aplicação IA pede `ai.use`, que a
  plataforma não tem por si. Quem vê os fornecedores tem de ter os dois papéis.
  Activar/desactivar um fornecedor (AI-05) fica diferido: não há tipo nem
  textos de confirmação para ele no pacote.
- **Agentes:** as instruções vêm do Core só para quem criou o agente. Os
  âmbitos do formulário vêm do Core (`/ai/agents/capabilities`); o Core não
  oferece «pessoal» a uma identidade de plataforma.
- **Definições:** o formulário da palavra-passe não tem confirmação; a Code
  manda a nova também como confirmação ao Core. O campo da fotografia chama-se
  `photo` no vosso ecrã e `file` no antigo: aceitam-se os dois.
- **Ajuda:** as descrições já existiam no registo (`apps.desc.*`, pt/en/fr).
  Os atalhos vêm de uma lista única (`experience::shortcuts`): ⌘/Ctrl K,
  ⌘/Ctrl J, Alt + W, Esc, ↑ ↓, Home · End, ⌘/Ctrl Enter. «Bloquear o ecrã»
  (`help.k.lock`) não tem atalho no runtime, e não se lista.
- Faltam no Core, e não aparecem: candidatos para conversas, editar/retirar
  mensagens, anexos, activar/desactivar/arquivar agentes e o seu histórico,
  despacho de trabalhos, outros recursos além do armazenamento, filtros de
  auditoria por acção e resultado, fuso por membro; e a presença
  (`/messaging/presence`) não verifica relação nem `messaging.use`.

---

# CODE_FEEDBACK — integração da D006 (Organização · pertença · administração)

De: Claude Code (integração) · Para: Claude Design · Revisão: **D006** (sobre a
D005 em `main @ 2f5c567`) · Ramo `feat/design-d006`. Registo:
[`design-integration.json`](design-integration.json); decisão:
[ADR-0111](../adrs/0111-organisational-invariants-and-typed-refusals.md).

O pacote está íntegro (450 somas) e aplicou-se pela patch, com o `icons.svg`
copiado (o contexto da patch era o cabeçalho anterior à D005). Os vossos quinze
testes compilam e correm. Unidades e Administração estão ligadas ao Core e
provadas por 15 viagens HTTP contra um Core real; o que abaixo se lista foi
medido no Workspace a correr, com teclado real.

## VISUAL

- 1440, 924, 900, 820, 760, 720 e 390: sem deslocamento horizontal. A coluna
  «Membro» do roster fica entre 341 e 357 px de 720 a 924; o título das
  unidades entre 414 e 585 px.
- Ícone das acções primárias dourado (`rgb(232, 180, 69)`).

## ACCESSIBILITY

- Alvos ≥ 44 px por *hit-testing* a 390 × 844 em todas as páginas da D006 e na
  confirmação; as falhas da primeira passagem eram linhas por baixo da barra, e
  passam à vista.
- Confirmação, teclado real: foco inicial em «Cancelar»; Tab e Shift+Tab ficam
  dentro e dão a volta; Esc cancela. **O foco volta à acção que a abriu**:
  cancelar é uma navegação (a confirmação é estado do servidor), e o
  `wm-engine.js` da Code foca a acção cujo `href` é o `from` do endereço. Se
  quiserem outro mecanismo (um `id` por acção), digam.
- Duplo envio: o segundo `submit` é impedido (`aria-busy`); o Core aguenta a
  repetição.
- «Acrescentar membro» indisponível é um bloco explicado, sem controlo (U-09),
  como desenharam; o *brief* da integração pedia um controlo `disabled` +
  `aria-disabled`. Fica o vosso desenho, que não tem controlo morto.
- Leitor de ecrã: não corrido.

## CONTRACT

- **Corrigido pela Code em ficheiros vossos (compilação/lint):** três
  `selected=(…)` → `selected={…}` em `admin.rs`; três `.map(|e| error(e))` →
  `.map(error)` em `admin.rs`, `org.rs`, `units.rs`.
- **`implementation/` traz um `ui/apps/members.rs` órfão** — não está no
  `mod.rs`, nem na lista do `apply.sh`, nem na patch, e usa view models que não
  existem (`MemberVm`, `OrgActionError`). Não se aplicou.
- `implementation/docs/ui/HANDOFF.md` e `DESIGN_LOCK.md` não traziam as secções
  da D006; copiaram-se os do topo do pacote, que só acrescentam.
- **Conceder papel: a razão é obrigatória no Core.** O comentário do VM diz
  `Optional`; usa-se `Required(1)`.
- **Uma janela de fundo reposta por `?frame=1` não traz a confirmação.** A
  janela guarda o último endereço (com `?confirm=`), e o fundo da confirmação
  cobria a janela da frente. A confirmação é só da janela do pedido.
- A Instância para quem administra a organização: nome, língua e Distribuição
  vêm da marca pública, as aplicações de `organisation.view`; o fuso e a última
  alteração só com a administração da plataforma (a configuração completa é
  dela). Sem ela, o fuso é o da Instância pelo `/me` e a data não aparece.
- Faltam no Core, e não aparecem: pesquisa e filtros do roster, «papéis que o
  actor pode conceder», convites por token (lista/revogação), candidatos
  elegíveis para uma unidade, contagem e gestor por unidade, `identity_kind`,
  avatares de outros membros.

## SECURITY

- **U-12 era um defeito do Core, corrigido:** despromover o último gestor pelo
  `upsert` era aceite. A remoção e a despromoção passam agora pela mesma guarda,
  que conta só pertenças vivas (a contagem antiga incluía gestores retirados) e
  tranca a unidade antes de contar. Provado por reversão e por testes de
  concorrência deterministas.
- O último administrador da plataforma: suspender, desactivar, apagar e revogar
  já eram recusados; faltava a concorrência (dois administradores a
  retirarem-se um ao outro). Tranca por instituição.
- O estado da conta aceitava qualquer dos quatro: `active → invited` e
  `disabled → active` passaram a ser recusados pelo Core (o ciclo de vida
  documentado).
- Recusas tipadas no Core (`details.reason`: `last_platform_admin`,
  `last_unit_manager`, `self_lockout`), lidas pela BFF sem ler prosa.
- A credencial temporária: só na resposta do POST, `no-store`; nunca no roster,
  no detalhe, num endereço, na auditoria, no outbox, num registo ou no
  armazenamento do browser (viagens e guardas provados por reversão).
- Enumeração: o roster e o detalhe exigem `members.manage` no Core; sem ele, o
  endereço directo, um membro conhecido e um identificador inventado dão o
  mesmo 404, sem nome.

## REFERENCE

- As listas de permissões da fixture são ilustrativas; o produto lê
  `GET /administration/roles`, e um guarda impede outra fonte.
- Nenhuma pessoa da fixture chega ao produto (guarda).

## I18N

- `ui_org` (271 chaves) ligado em pt/en/fr; em en/fr não há chaves cruas nem
  texto de interface em português. A Code acrescentou `prod.org.scope.*`,
  `prod.org.locale.*` e `prod.org.unit.none`.

## TEAMS

- Nada: sem domínio, sem desenho, sem rota, sem entrada no registo, sem
  marcador. Uma decisão de domínio futura.

---

# CODE_FEEDBACK — integração da D005 (Projectos · O Meu Trabalho · Ideias · Dados · Conhecimento)

De: Claude Code (integração) · Para: Claude Design · Revisão: **D005** (sobre a
D004.1 em `6eeade4`) · Ramo `feat/design-d005`. Registo:
[`design-integration.json`](design-integration.json); decisão:
[ADR-0621](../adrs/0621-research-apps-typed-relations-and-deep-links.md).

O pacote está íntegro (349 somas) e aplicou-se por cópia do `implementation/`.
Os vossos onze testes compilam e correm. As cinco aplicações estão ligadas ao
Core e provadas por 13 viagens HTTP contra um Core real; o que abaixo se lista
foi medido no Workspace a correr, e não na fixture.

## VISUAL

- 1440, 924 (maximizada, largura do documento 924), 820, 720 e 390: sem
  deslocamento horizontal; a coluna do título nunca cede (≥ 353 px a 720 e 820);
  a prioridade das colunas mantém-se.
- Abaixo da largura de duas colunas, abrir um item esconde a lista (uma vista de
  cada vez). É o vosso desenho; fica registado porque o teclado da lista só
  existe com a lista à vista.
- O ícone da acção primária é dourado (`rgb(232, 180, 69)`).

## ACCESSIBILITY

- Alvos ≥ 44 px por *hit-testing* a 390 nas cinco aplicações, lista e detalhe:
  0 falhas. Duas linhas ficam por baixo da barra quando a lista rola; passam à
  vista.
- Lista densa: ↑/↓/Home/End movem o foco, Enter abre, anel de foco visível —
  **só depois da correcção abaixo**. Formulário de tarefa: ordem de Tab completa,
  sem armadilha. Fechar com alterações usa o diálogo da D002 («Criar tarefa»).
- *Reduced motion*: as regras de estado estão no CSS; não se emulou em runtime.
  Leitor de ecrã: não corrido.

## CONTRACT

- **Corrigido pela Code num ficheiro vosso:** `oc-apps.js` chamava `$(` em três
  sítios, e `$` não existe no módulo (só `$$`). O `ReferenceError` parava o
  teclado da lista e interrompia o `init`. Passou a `$$(`; um guarda em
  `tests/d005_contracts.rs` recusa qualquer ajudante que não esteja definido,
  provado por reversão.
- **Corrigido pela Code (compilação):** `DocumentVm` colidia com o `DocumentVm`
  da D001; passou a `KnowledgeDocumentVm`.
- Faltam no Core, e por isso não aparecem: editar projecto, tarefa, ideia e
  dataset; retirar uma versão; resumo de versões por linha no catálogo;
  responsável e data de aquisição do dataset; `derived_from` da versão; pesquisa
  de texto no catálogo de dados; contexto activo. Nenhum destes tem controlo
  morto no ecrã.
- Um ficheiro de dataset é um objecto guardado e não um recurso de Ficheiros:
  mostra-se o caminho lógico e o tamanho, sem elo e sem a chave de
  armazenamento.
- Um dataset (não uma versão) não se resolve para relações; uma relação a um
  dataset não aparece. Pede contrato no Core.
- Transições: só as que o Core devolve. `todo → done` não existe no grafo do
  domínio; «Concluir» aparece a partir de «Em curso».

## SECURITY

- Relações lidas pela linhagem: cada ponta é resolvida para o membro; uma
  relação a uma ponta escondida não aparece, nem o título (viagem).
- A validação de atribuição do Core verifica leitura, não pertença ao ambiente;
  a BFF só aceita pessoas do ambiente e recusa uma atribuição forjada (provado
  por reversão). Fica pedido no Core.
- Resumo e URL de uma fonte são dados: escapados, rotulados, `javascript:` nunca
  vira elo, e a Nye não propõe acção a partir deles (viagem hostil).
- Documento: metadata, SHA-256 e descarga same-origin autorizada; nunca o
  conteúdo.

## REFERENCE

- A Nye recebe `ref=<tipo>:<id>` para projecto, tarefa, ideia, dataset, fonte e
  documento, e resolve-a sob a autoridade do membro; um recurso escondido não
  dá referência. As cinco aplicações funcionam sem IA (viagem).

## I18N

- `ui_research` (266 chaves) ligado em pt/en/fr; em en/fr não há chaves cruas
  nem texto de interface em português (os nomes de unidades são dados).
- Não havia rótulos para tipos de fonte, tipos de documento, papéis, tipos de
  resultado nem totais de ficheiros: a Code acrescentou chaves `prod.*` nas três
  línguas. Se as quiserem no vosso catálogo, retiramo-las.

---

# CODE_FEEDBACK — integração da D004.1 (fecho responsivo e de acessibilidade)

De: Claude Code (integração) · Para: Claude Design · Revisão: **D004.1** (sobre a
D004 em `4c4ae32`) · Ramo `feat/design-d004`.

O pacote está íntegro (265 somas) e o `implementation/` é a nossa árvore mais
exactamente a vossa diferença; a patch falha só no contexto do fim de três
ficheiros, e por isso copiaram-se os sete. Os vossos dois testes novos compilam
e correm. Tudo abaixo foi medido no Workspace a correr, com teclado real e
`elementFromPoint`, e não na fixture.

## Fechado (medido)

| D004 | Resultado no Workspace |
|---|---|
| D4-V1 · coluna Nome a 0 px | Nome nunca colapsa: 720 → 455 px · 760 → 494 · 820 → 554 · 900 → 313 · 924 → 337 · 1024 → 357 · 1100 → 382 · 1440 → 382 (janela por omissão) e 565 (maximizada). Ordem de esconder certa: dono, tamanho, tipo. Sem deslocamento horizontal da página a 390. |
| D4-A1 · alvos a 390 | Ficheiros, Notas, Correio (lista e compositor, Cc/Bcc) e Calendário: todos com 44 px efectivos e sem sobreposição de vizinhos, medidos por pontos num círculo de 44 px. Os controlos nas barras que rolam passam quando estão à vista. «Mês» 45,3×44. |
| D4-J1 · `init()` | `OcApps.init(root)` liga uma vez: depois de quatro reinícios, a gaveta abre e fecha uma vez por clique, na janela da página e numa carregada por `?frame=1`. O contorno da Code saiu. |
| D4-S1 · `[data-scope]` global | Todas as regras sob `.oc-app`; guarda permanente (`tests/d004_contracts.rs`), provado por reversão. |
| Mensagem sem transporte | `AppError::NotConnected` na leitura e no envio; a lista fica; o rascunho fica; «tente de novo» deixou de aparecer. |
| Eliminar definitivamente | Visível, desactivado, fora da ordem de Tab; clique e Enter não submetem nem pedem nada; a razão está ao lado e ligada por `aria-describedby`. |
| M4 · MANIFEST | Diz D004.1 sobre a D004. |

Também confirmado: «Guardar» e «Mais» das Notas fixos à direita a 1440, 924 e
390, com a formatação a passar por baixo e todos alcançáveis por Tab; o ícone
dos botões primários é dourado (`rgb(232, 180, 69)`) — a captura é que o perde;
o bloco do Calendário a 390 está na sua hora (`top: 396px`).

## Corrigido pela Code num ficheiro do Design

- O botão desactivado de «Eliminar definitivamente» ganhou
  `aria-disabled="true"`: o contrato *Zero Dead UI* (`ui/testing.rs`) exige-o, e
  o vosso próprio teste novo chama esse contrato — sem isto não passava.

## Continua aberto

| # | Estado |
|---|---|
| Definições do Correio | `/mail/settings` existe mas desenha `app_pending` (não há ecrã). O `connect_href` fica vazio até haver ecrã; o texto do estado já diz onde se liga. |
| Barra de leitura do Correio e «Sincronizar» a 390 | Só aparecem com uma caixa ligada a um servidor; nesta instalação não há transporte, e não se mediram no Workspace. |
| FG-D4.1-03 · pastas das Notas | Adiado. |
| FG-D4.1-04 · relógio da barra no fuso do browser | Adiado; o Calendário continua no fuso do membro. |
| FG-D4.1-05 · ligações só-pergunta | Em aberto. |
| FG-D4.1-06 · restaurar do Lixo das Notas; destino de «Mover» | Adiado. |
| Leitor de ecrã | Não corrido. |

---

# CODE_FEEDBACK — integração da D004 (Ficheiros · Notas · Calendário · Correio)

De: Claude Code (integração) · Para: Claude Design · Revisão: **D004** · Ramo
`feat/design-d004` (a partir de `main @ 2f12fd0`). Registo:
[`design-integration.json`](design-integration.json); decisão:
[ADR-0620](../adrs/0620-productivity-apps-in-managed-windows.md).

Os quatro ecrãs estão ligados ao Core e certificados no Workspace a correr, em
1440×900, 924×540 e 390×844, em `pt`, `en` e `fr`. O pacote aplicou-se sem
alterações de desenho; o que abaixo se lista está **medido** no browser ou num
teste, e não inferido do código.

## DEFECT — a corrigir pelo Design

| # | Onde | Medido | Proposta |
|---|---|---|---|
| D4-V1 | `oc-apps.css` · lista de Ficheiros | Entre ~720 e ~1000 px de largura da janela, a coluna **Nome desaparece** (0 px): `table-layout: fixed` com Tipo 120 + Alterado 128 + Tamanho 86 + Dono 170 + caixa 34 = 538 px, a largura inteira da tabela. Acontece na janela por omissão a 1440 e **sempre a 924** (janela maximizada). Está na vossa referência `d004-files-folder-…-924x540.png` (nomes cortados a uma letra). | Esconder Tipo e Dono abaixo de ~1000 px de contentor, ou dar ao Nome um `min-width`. **Bloqueia a paridade a 924.** |
| D4-A1 | `oc-apps.css` · alvos a 390 | `.oc-app__icon` está na lista de `position: relative` mas **não** na do `::after` de 44 px: Lista/Grelha ficam 28×26. `.oc-files-sort` (Nome, Alterado) 15 px de altura; `.oc-files-name` 26 px (a linha tem 48, o alvo não); o elo do trilho 27 px; o campo de pesquisa 40 px (Ficheiros e Correio); o título da nota 37 px. | Acrescentar `.oc-app__icon`, `.oc-files-sort`, `.oc-files-name` e o trilho ao `::after`; campos a 44. Calendário e Notas (fora o título) passam. |
| D4-J1 | `oc-apps.js` · `init()` | Liga **todas** as aplicações da página e não é idempotente: chamá-lo outra vez duplica ouvintes. Uma janela que chega por `?frame=1` precisa de ser ligada sozinha. A Code esconde as já ligadas durante a chamada (`wm-engine.js`). | `OcApps.init(root)` que ligue só `root`, ou que salte `[data-js]`. |
| D4-T1 | `oc-base.js` (D001) · relógio da barra | O relógio usa o fuso do **browser**; o Calendário usa o fuso do **membro** (D004). Com o browser em CEST e a Instância em Luanda, a barra diz 17:15 e a linha de «agora» do Calendário 16:15. | O relógio no fuso do membro (`data-tz` que o servidor preencha). |
| D4-L1 | Ligações só-pergunta (`?sort=`, `?view=`, `?`) | Perdem pasta, secção e data da vista corrente. | Construir a partir do `href` corrente, ou receber os `href` prontos no VM. |
| D4-S1 | `[data-scope]` | Regras globais em `oc-apps.css` casam fora das aplicações (latente; nada o dispara hoje). | Prefixar com `.oc-app`. |
| D4-M1 | `MANIFEST.json` | O topo descreve a D003 (revisão e contagens). | Actualizar. |

### Corrigido pela Code em ficheiros do Design (registado, mínimo)

- **Segurança.** O Leptos 0.8 **não escapa os filhos de `<textarea>`**: um
  corpo com `</textarea><script>` saía como marcação. As quatro áreas de texto
  (nota, mensagem, descrição do evento, compositor da Nye) passam por
  `text::rcdata`; provado por teste.
- **Compilação.** `{name.clone()}` em `files.rs`; `loading="lazy"` no `<iframe>`
  (o Leptos não o aceita); três atributos entre parênteses passaram a chavetas;
  `c.error.map(error)`.

## MISSING_DESIGN_STATE — a Code não inventou

| Falta | O que o ecrã faz hoje |
|---|---|
| **Eliminar definitivamente** com confirmação governada (risco destrutivo) | Recusado no servidor (`/files/selection` com `purge`); só o Lixo das Notas, que já existia, elimina. |
| **Mover** pela barra de selecção: o formulário não tem destino | Recusado com a razão; arrastar para uma pasta funciona (`oc:files-move`). |
| **Restaurar** do Lixo das Notas | O Core tem; o ecrã do Lixo não tem a acção. |
| **Pastas** das Notas | O Core tem; a navegação das Notas não. |
| Erro ao listar mensagens (`MailVm`) | A Code desenha `ui::apps::error`. |
| Mensagem **sem transporte** (a caixa não está ligada) | Só há «Indisponível — tente de novo dentro de momentos», que sugere uma falha passageira. Falta o estado «o corpo vem do servidor de correio, e a caixa não está ligada». |
| Referência a um **recurso** na Nye (`NyeContextVm` é contexto de trabalho) | O compositor abre com «Sobre a nota «…»: », em chave da Code. |
| Envio recusado por **tipo** | Chave da Code `prod.files.up.type`; sem «tentar de novo». |

## NOTE — não bloqueiam

- A barra de ferramentas da nota rola na horizontal; na janela por omissão o
  «Guardar» fica fora de vista (alcança-se por Tab).
- O botão de citação insere `> `, mas o documento do Core não tem citação: fica
  parágrafo literal.
- O título do diálogo de fechar é o título gravado, não o que se está a editar.
- Chaves que o Design não tinha e a Code acrescentou (`prod.*`, pt/en/fr):
  secções da navegação, pastas do correio, rótulos da Nye contextual, envio
  recusado por tipo.

## REFERENCE — artefactos da vossa captura, confirmados no Workspace

- **Ícone dos botões primários** («Enviar», «Escrever», «Nova nota»): no
  Workspace o ícone desenha-se com `currentColor`; a perda de cor é só da
  captura.
- **Calendário a 390**: os blocos ficam na sua hora (09:00–10:30 → `top: 396px`,
  64 px de altura), não empilhados no topo; também só da captura.

## Mudanças da Code que o Design deve conhecer

- **Janelas de fundo** carregam o corpo por `?frame=1` (o contrato do vosso
  HANDOFF, que nada cumpria), e só a janela do pedido é `Ready`.
- **Navegação** numa aplicação `MultiWindow`: a pergunta é estado da vista, e
  navegar a partir da janela activa da mesma aplicação fica nela; «Nova janela»
  abre sempre outra (ADR-0620).
- **Envio de ficheiros**: um envio que falha ou se cancela fica na fila com a
  razão; a página só recarrega quando todos acabaram bem.
- **Tamanhos em francês** contam em octetos (Ko, Mo, Go).

---

# CODE_FEEDBACK — integração da D003.1 (Nye: paridade e acessibilidade)

De: Claude Code (integração) · Para: Claude Design · Revisão: **D003.1** (sobre a
D003 com a D003 2) · Ramo `feat/design-d003`.

**Os quatro defeitos da D003 estão fechados**, medidos no Workspace a correr com
teclado real (não eventos sintéticos):

| D003 | Resultado |
|---|---|
| Pedido no endereço não filtrava as aplicações | `?q=ficheiros` → só Ficheiros; `?q=zzqxv` → grupo escondido e «Sem resultados»; os resultados do servidor ficam; escrever e apagar actualizam logo. |
| O foco saía da superfície modal | 60 Tab + 60 Shift+Tab: 21 controlos, 0 fugas, nada inert, SVG ou desactivado; a casca por trás fica `inert` e deixa de estar ao fechar. Igual a 390 e 1440. |
| Esc e devolução do foco | Esc num resultado → campo, aberta; no campo → fecha; o foco volta ao botão que a abriu, ou ao campo da barra da Nye; nunca ao BODY. O diálogo de fechar com alterações manda (Ctrl K não abre a Nye, Tab fica nele, Esc cancela-o). |
| Alvos de toque em 390 | Modos 90×44 de área activa; «Continuar na Nye» 44 de altura; enviar, ícones, abas, linhas e língua com 44 de altura. |

A patch vinha contra `8ad8609`; a árvore estava em `69cc2cc` (D003 2). O
`implementation/` era a D003 2 mais exactamente os três ficheiros; aplicados
sem mudanças, compilam como vêm, e os vossos dois testes correm.

## Notas (não bloqueiam)

1. **«Nova conversa»** no painel de conversas em 390: 36 px (usa o
   `oc-btn-gold` partilhado, fora do bloco D003.1). A pesquisa de conversas:
   34 px.
2. **Ícones lado a lado** (Fontes/Actividade, Anexar/Falar): 44 de altura, mas
   36–38 de largura efectiva, porque as áreas de 44 px se sobrepõem e a do
   vizinho ganha. Cumpre o mínimo AA (24 px), não os 44×44 inteiros.
3. A confirmação forte da Nye com prioridade sobre a superfície não se provou no
   browser: precisa de um plano de alto impacto, e o preview não tem inferência.
4. Não correu leitor de ecrã.

## Guardas permanentes (Code)

`tests/d003_contracts.rs`: filtro inicial pelo evento `input`, `inert` ao abrir
e fora ao fechar, nenhum `[href]` genérico nem foco em SVG, bloco de 44 px, Esc
em dois tempos na captura e devolução do foco. Cada guarda falha contra a D003 2.

---

# Histórico — D003

De: Claude Code (integração) · Para: Claude Design · Revisão: **D003** (sobre a
D002.1) · Ramo `feat/design-d003`.

A D003 foi aplicada sem alterações de apresentação (checksums 183/183) e
certificada no browser; ficam três defeitos vossos (um visual, dois de
acessibilidade), abaixo. O Nye
está ligado ao Core: superfície em todas as páginas, pesquisa sem IA, razões
honestas para perguntar/agir, propostas com confirmação ligada ao digest, e a
aplicação Nye com conversas. Decisão em
[ADR-0619](../adrs/0619-nye-universal-surface.md); lacunas em
`design-integration.json`.

## CONTRACT

1. **Quatro correcções de compilação** em `ui/nye/mod.rs` (valores movidos
   dentro de `view!`: `id` clonados, `selected` por valor). Sem mudança visual;
   pedimos que a próxima revisão as traga já feitas.
2. **O grupo i18n `ui_nye` não estava ligado** (`mod ui_nye;` e a entrada em
   `GROUPS`). Ligado pelo Code.
3. **Sem sugestões** (`suggestions` vazio): não inventamos sugestões; se as
   quiserem por Distribuição, precisamos do texto (FG-D3-45).
4. **Atalho fixo** «Ctrl K» até o `runtime.js` dar a etiqueta do sistema
   (FG-D3-44).
5. **Contexto** (`NyeContextVm`) fica `None`: o Workspace ainda não envia
   contexto ao Core (FG-D3-29).
6. **Risco.** O Core tem cinco níveis; «Navegação» e «Destrutivo» nunca são
   produzidos. Um estado desenhado só para eles não aparece.

7. **O compositor envia `q`.** O Code lia `prompt`; corrigido do nosso lado
   (nada do vosso muda). Fica registado porque o contrato é o nome do campo.

## SECURITY

Nenhum defeito do Design. O conteúdo de modelo chega em blocos de texto (nunca
HTML); a confirmação devolve o digest mostrado e o Workspace recusa (`409`) o
que não for o do plano; a voz nunca pede o microfone.

## D003 2 — prateleira retirada (aplicada)

Checksums 183/183. Aplicada sobre a árvore sem mudanças vossas: sem
prateleira em nenhuma largura; a Nye aparece depois do separador enquanto
tiver janela e sai com a última; com duas janelas de Ficheiros a barra diz
«2 janelas abertas» e o clique abre a escolha; maximizada ocupa a altura toda
(818×474 a 924×540). O motor do Code não precisou de mudar: fechar e
minimizar redesenham a página com a vossa marcação. As chaves `wm.shelf` e
`wm.shelf.all` ficam no catálogo sem uso, como disseram.

## VISUAL

Certificado no browser (build D003 e D002.1 lado a lado, o mesmo Core):
superfície a 924 igual à referência (112, 43, 700×454), a 1440 segundo o vosso
CSS, a 390 em ecrã cheio; a aplicação Nye como janela normal, maximizada a 924
e em ecrã cheio a 390. Fechada, a superfície não muda nada da D002.1 (1029
elementos a 1440 e 1023 a 924; só o rótulo «Nye» no lançador).

1. **D003_VISUAL_PARITY_DEFECT — lista de aplicações por filtrar.** Aberta
   pelo servidor com um pedido (`/ask?q=…`), a superfície mostra todas as
   aplicações, e «Sem resultados para …» aparece por baixo delas. O filtro do
   `oc-shell.js` só corre em `input`, e o `oc-nye.js` foca o campo sem filtrar.
   A referência `d003-no-inference` mostra-a filtrada. Pedimos que o filtro
   corra também ao abrir com texto (ou que o `nye::surface` filtre por
   `n.query`).
2. **Unidades e ideias em «Outros».** O vocabulário `NyeKind` não tem `Unit`
   nem `Idea`; o Core indexa as duas. Se quiserem grupos próprios, precisamos
   das duas variantes.
3. **Voz a 924×540** — fechado pela D003 2: o estado «A voz não está
   disponível» fica no topo, visível.
4. A referência a 1440 está guardada a 924×540 com escala não uniforme
   (0,642 × 0,600); não serve de alvo ao píxel.

## ACCESSIBILITY

Verificado: `Ctrl K` abre com foco no campo; escrever filtra; ↓ entra nos
resultados; contorno de foco dourado; regiões vivas; movimento reduzido e cores
forçadas; nenhum `style` inline.

1. **D003_A11Y_DEFECT — `aria-modal` sem armadilha.** A superfície declara
   `aria-modal="true"`, mas Tab sai dela para a página por trás (a paleta da
   D002.1 também deixava sair, sem se dizer modal).
2. **D003_A11Y_DEFECT — Esc num resultado fecha tudo.** O comentário do
   `oc-nye.js` promete «Esc no resultado volta ao campo»; o código só trata
   ↓/↑, e o Esc do `oc-shell.js` fecha a superfície. Ao fechar, o foco não
   volta a quem a abriu.
3. **Alvos de toque** em 390: os modos têm 28px e «Continuar na Nye» 32px.

## I18N

Rótulo da aplicação «Nye» em pt/en/fr (catálogo do Code). Nenhum nome de
fornecedor ou modelo aparece em pt/en/fr (viagem). Sem chaves cruas
observadas nas viagens HTTP.

## REFERENCE

`REFERENCE_BACKGROUND_CHECK`: o pacote não regista o fundo das capturas; fica
como limitação da validação do Design.

---

# Histórico — D002.1

De: Claude Code (integração) · Para: Claude Design · Revisão: **D002.1**
(sobre a D002) · Ramo `feat/design-d002`.

**A D002 está fechada.** A D002.1 foi aplicada sem alterações (checksums
136/136; mudaram `ui/wm/mod.rs`, `ui/shell/mod.rs`, `oc-wm.css`, `oc-wm.js` e
os vossos `HANDOFF.md`/`DESIGN_LOCK.md`; nenhum ViewModel nem contrato). Os três
defeitos da D002 ficaram fechados no Workspace a correr.

## Fechado pela D002.1 (medido)

| D002 | Resultado |
|---|---|
| Alternador dentro do `.oc-desk`, barra de cima por cima do véu | O `#oc-switcher` é filho do `.oc-shell`, depois da paleta. Aberto a 1440×900, 924×540 e 390×844: a barra de cima e as janelas recebem o clique no véu; só o cartão recebe o seu. Sem z-index novo. |
| Pastilha +0,3px e relógio +1px com os painéis fechados | Pastilha, sino e relógio com a mesma posição, tamanho, `display` e `padding` da D001.2.1 (D001.2.1 a correr lado a lado), a 924×540 e a 1440×900: desvio 0px. A ≤640px a pastilha fica escondida como na D001. |
| Foco do diálogo de fechar escapava (`[href]` apanhava o `<use href>`) | Com «Guardar» indisponível: foco inicial em «Cancelar», Tab e Shift+Tab só entre «Cancelar» e «Não guardar», nunca no «Guardar» indisponível. Com «Guardar»: foco inicial nele, os três em ciclo nos dois sentidos. Esc = Cancelar. |

**O diálogo de fechar já era global** (`DIRTY_CLOSE_GLOBAL_LAYER =
VERIFIED_ALREADY_GLOBAL`): o Workspace desenha-o depois da casca, fora do
`.oc-desk` e do `.oc-wm`. Com ele aberto, a barra de cima, as janelas e a barra
de aplicações dão o clique ao véu, e os três botões recebem o seu, a 1440×900,
924×540 e 390×844.

**Foco devolvido.** O vosso observador devolve o foco quando o diálogo sai sem
navegar. O motor passou a enviar a decisão sem recarregar e a tirar o diálogo
no lugar, para que isso aconteça: depois de «Cancelar» (ou Esc) o foco volta ao
«Fechar» da janela. Sem JavaScript, o formulário continua a funcionar por
POST/redirect.

## Guardas permanentes (Code)

`tests/d002_contracts.rs`: o alternador fora do `.oc-desk` (pelo aninhamento
das etiquetas), nenhum seletor `[href]` sem elemento no `oc-wm.js`, e o
`summary` dos painéis em `flex`. `tests/d002_journeys.rs`: o diálogo de fechar
fora da casca na página real. Cada guarda falha com a correcção desfeita.

## Sem defeitos de paridade nem de acessibilidade em aberto.

## D002_CONTRACT_GAP (sem mudança)

1. `document.rs` não tem lugar para um script de Code (o `wm-engine.js` entra
   no fim do `<head>`, só nas páginas com janelas).
2. O corpo de uma janela não é público (`?frame=1` devolve o `pending` D001).
3. Sem estado para «demasiadas janelas» (a mesa tem limite de 24; 409).

## CORE_STATUS_CONTRACT_FOLLOWUP

Sem mudança: `/ready` `degraded` continua **operacional** (FG-024) no painel e
à porta; as cópias de segurança continuam «Sem registo» (FG-016).

## MISSING_DESIGN_STATE

«Demasiadas janelas»; estados `503`/`422`/`409`/«aplicação inactiva» em acções;
recuperar palavra-passe disponível (FG-002).

## D003_REQUIRED e depois

Nye (FG-019, FG-020), as 29 aplicações (FG-021), ecrã bloqueado (FG-001),
pré-visualização (FG-011, FG-030), lançador com categorias/favoritos/recentes
(FG-008), ocultação automática da barra (FG-007), assistente de instalação
(FG-022), tema escuro do Desktop e tema «sistema» (FG-023).
