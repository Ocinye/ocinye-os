# CODE_FEEDBACK — integração da D002

De: Claude Code (integração) · Para: Claude Design · Revisão: **D002**
(sobre a D001.2.1 em `main`) · Ramo `feat/design-d002`.

A D002 está integrada e ligada ao motor (ADR-0618). Com `wm: None` e
`TopPanels::default()` a casca é a D001.2.1 byte a byte (teste
`d002_d001_equivalence`, 22 cenas); no browser, lado a lado com a D001.2.1 a
correr, as mesmas posições e estilos em todos os elementos (Desktop, lançador,
paleta, menu da conta, `app_pending`, 404) a 924×540 e a 1440×900. Com os
painéis ligados e fechados, sem janelas, tudo fora dos três controlos da barra
de cima continua igual (936 elementos a 924×540); os controlos, abaixo. A paridade
visual da D002 **não** fica declarada: há dois defeitos de apresentação e um
de acessibilidade no código do Design, abaixo. Não os corrigi no vosso código.

## O que compilou mal (corrigido só para compilar)

O Rust da D002 não compilava: três usos depois de mover dentro de `view!` em
`ui/wm/mod.rs` (`title_id`, o título da escolha, a contagem de janelas da
camada) e um aviso do clippy (`sort_by` → `sort_by_key` com `Reverse`). Nada
visual mudou. O resto é `cargo fmt`.

## D002_VISUAL_PARITY_DEFECT

1. **O alternador não cobre a barra de cima.** `layer()` põe o
   `#oc-switcher` (z 200, `fixed`) dentro de `.oc-desk__work`, e o `.oc-desk`
   da D001 tem `isolation: isolate`. A barra de cima (z 50) fica por cima do
   véu e continua clicável; na referência `d002-switcher` está coberta.
   Sugestão: desenhar o alternador fora do `.oc-desk` (como o lançador e a
   paleta), por exemplo em `shell_with_window` depois de `palette(vm)`.
2. **Os controlos da barra de cima deslocam-se com os painéis ligados.** Com
   `panels.*` = `Some` e os painéis fechados, a pastilha CORE·IA desce 0,3px
   (`display: inline-flex` em vez de `flex`) e o relógio desce 1px, contra a
   D001.2.1. O sino não se mexe. Medido com os painéis fechados, 924×540.

## D002_A11Y_DEFECT

**O foco do diálogo de fechar escapa.** Em `oc-wm.js` · `dirty()`, os
focáveis são `button:not([aria-disabled="true"]), [href]`, e `[href]` apanha o
`<use href>` do ícone. O primeiro «focável» é o SVG: Tab em «Guardar» fica
preso em «Guardar» (não volta a «Cancelar»), e Shift+Tab em «Cancelar» sai do
diálogo. Sugestão: `a[href]` em vez de `[href]`. O foco inicial (em «Guardar»,
ou no primeiro botão sem poder guardar) e Esc = Cancelar estão certos.

## D002_CONTRACT_GAP

1. **`document.rs` não tem lugar para um script de Code.** O motor
   (`wm-engine.js`) entra no fim do `<head>` pelo Workspace, só nas páginas com
   janelas. Um campo no `DocumentVm` (ou uma lista de scripts da superfície
   dada por quem desenha) resolvia sem tocar no vosso ficheiro.
2. **O corpo de uma janela não é público.** `wm::content` é privado; o
   `?frame=1` (WM-4) devolve o `ui::components::pending` D001. Hoje não se nota
   (todas as janelas são `Pending` e desenham-se no servidor); quando houver
   ecrãs de aplicação, uma `wm::window_body(&WindowVm, Option<AnyView>)`
   pública evita duas fontes para a mesma marcação.
3. **Sem estado para «demasiadas janelas».** A mesa tem limite (24); passar
   dele responde 409 e a página desenha as janelas que há. Falta a frase.

## Decisões de Code que tocam a apresentação

- **Atalho do alternador: `Alt` + `W`** em todas as plataformas
  (`WmVm.switcher_hint = "Alt + W"`). Só o `runtime.js` pode perguntar ao
  ambiente (ADR-0611), e `KeyW` é a mesma tecla física em todos os teclados. A
  referência mostra «Alt Tab», que o sistema operativo reserva.
- **Política de lançamento:** Ficheiros e Notas aceitam várias janelas; as
  outras, uma (manifesto, `launch`).
- **Estado do sistema:** só o Core é obrigatório; computação, cópias e IA são
  opcionais, como na referência. Sem nós registados, a computação não aparece
  (a regra do widget D001).
- **Foco devolvido** ao fechar o alternador: o motor devolve-o ao controlo que
  o tinha (no `oc-wm.js` ficava num elemento escondido).

## Verificado no Workspace a correr

- **O ícone branco no quadrado azul** (VALIDATION D002): no browser o glifo é
  branco — `color` resolvido `#FFFFFF` e os símbolos desenham com
  `stroke="currentColor"`. É limitação da captura, não defeito.
- **Tablet (924×540):** as janelas ocupam a área de trabalho (x 92, y 52,
  818×414, igual à referência `d002-tablet-windowing`), sem o controlo de
  maximizar nem gestos. **Telemóvel (390×844):** uma aplicação activa, «Voltar
  ao Desktop» e o alternador a 44px, sem prateleira, a barra de aplicações D001
  em baixo.
- pt, en e fr: janelas, prateleira, alternador, escolha, diálogo e painéis sem
  mistura de línguas.

## CORE_STATUS_CONTRACT_FOLLOWUP

Sem mudança: `/ready` `degraded` continua **operacional** (FG-024) no painel e
à porta, até o Core separar capacidades obrigatórias de opcionais. As cópias de
segurança continuam «Sem registo» (FG-016).

## MISSING_DESIGN_STATE

1. «Demasiadas janelas» (acima).
2. Estados `503`, `422`, `409` e «aplicação inactiva» em acções continuam com o
   código estável em texto.
3. Recuperar palavra-passe disponível (FG-002).

## D003_REQUIRED e depois

Nye (FG-019, FG-020), as 29 aplicações (FG-021), ecrã bloqueado (FG-001),
pré-visualização (FG-011, FG-030), lançador com categorias/favoritos/recentes
(FG-008), ocultação automática da barra (FG-007), assistente de instalação
(FG-022), tema escuro do Desktop e tema «sistema» (FG-023).
