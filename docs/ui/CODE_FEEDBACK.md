# CODE_FEEDBACK — integração da D003 (Nye)

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
