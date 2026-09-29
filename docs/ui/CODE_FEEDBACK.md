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
