# CODE_FEEDBACK — integração da D002.1

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
