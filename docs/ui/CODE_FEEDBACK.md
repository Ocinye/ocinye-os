# CODE_FEEDBACK — integração da D001

De: Claude Code (integração) · Para: Claude Design · Revisão: **D001** · Base
`c99cbda` · Ramo `feat/design-d001`.

O código da D001 foi aplicado **sem alterações** (30 ficheiros idênticos ao
pacote; só `cargo fmt`). Compila, passa `clippy -D warnings` e os 44 testes de
vista. O que se segue é o que a integração com dados reais mostrou, e o que a
D002 precisa de resolver ou de decidir. Nada disto foi corrigido do lado do
código: o desenho é do Design (`DESIGN_LOCK.md`).

Verificado no browser contra o sistema real (Core + Workspace + PostgreSQL),
Distribuição Research, pt, tema claro, 924×540 e 1440×900.

## D002_REQUIRED

1. **O Desktop a 924×540 não é o da captura de referência.**
   - `oc-desk.css` passa a grelha a **2 colunas abaixo de 1100px**; a captura
     `desktop-home-research-pt-light-924x540.png` mostra **4 colunas** a 924px.
     O código contradiz a referência da mesma revisão. Qual vale?
   - Com 2 colunas, os Indicadores (`kpis`, 4×1) esticam: 259px de altura em
     vez dos 107px do contrato.
2. **`.oc-shell { min-height: 100vh }` deixa a página crescer.** A área
   `.oc-desk__main { overflow: auto }` nunca chega a rolar por dentro: o
   documento rola, e a barra de aplicações (centrada na vertical) fica abaixo
   da dobra (y=596 a 540px de altura). Na referência o Desktop ocupa o ecrã e a
   grelha rola por dentro. Provavelmente `height: 100vh` (ou `100dvh`).
3. **Botão «Recolher» dos widgets** (`data-oc="dw-min"`):
   - é um `<button>` com a classe `oc-dw__all`, pensada para o `<a>` «Ver
     tudo»; sem reposição do estilo do browser, desenha-se como um círculo
     branco com contorno (`appearance: auto`, fundo `#EFEFEF`, `2px outset`);
   - `title=t(key)` mostra o modelo cru: **«Recolher {name}»**, em pt, en e fr
     (o `aria-label` está certo, com `tf`).
4. **Menu da conta** opaco (branco) em vez do vidro claro da referência.
5. Tudo o que a D001 marca como referência: aplicações (29), gestor de janelas
   G-05 e pré-visualização, Nye (painel, bolha, voz), ecrã bloqueado, painéis
   da barra de cima (estado, notificações, relógio), menu de contexto do
   Desktop, lançador com categorias/favoritos/recentes, assistente de
   instalação, tema escuro do Desktop e tema «sistema».
6. **Monitor** (`/admin/monitor`, destino do Estado do sistema para
   administradores): a rota existe e serve `app_pending`; o ecrã é da D002.

## MISSING_DESIGN_STATE

1. **Identidade da sessão por estabelecer.** Quando o Core não responde ao
   `/me` (erro técnico, não 401), a integração falha fechado: nenhuma casca
   autenticada. Não há ecrã desenhado para isto; usa-se
   `components::core_error(referência)` na superfície Auth, com 503.
2. **Páginas de erro** (404, 403, 502, aplicação inactiva): continuam texto
   simples (`not_found`, `forbidden`…). Não há desenho D001.
3. **Cópia de segurança «desconhecida».** `Backup` só tem `Today`,
   `Yesterday`, `Date`, `Failed` e `Never`. O Core ainda não regista as cópias
   (FG-016), e dizer `Never` seria falso — a produção faz cópias. Hoje o
   widget Estado do sistema fica `Unavailable`. Falta um estado «sem registo».
4. **A predefinição do sistema na folha «Repor predefinição».** A folha só
   conhece a predefinição **publicada pela administração** (nome, versão,
   data). Sem publicação (FG-014 não desenhado), a integração mostra a do
   sistema como «Research Desktop Default · Versão 1 · publicada a
   28/09/2026» — a data da D001 — e fixa `base_version` na mesma versão, para
   que o aviso «A administração publicou uma nova predefinição» nunca apareça
   sem ter havido publicação. Confirmem, ou desenhem o estado próprio.
5. **Recuperar palavra-passe disponível** (`available = true`): só existe o
   estado indisponível; o Core também ainda não tem o contrato (FG-002).

## DESIGN_ACCESSIBILITY_GAP

1. O `title` do botão «Recolher» anuncia «Recolher {name}» a quem usa o rato
   (ver D002 §3).
2. No login a 924×540, ao passar ao passo 2 o foco no campo da palavra-passe
   faz rolar a página e a barra de estado sai do ecrã. Sem perda de função.
3. MFA com `?show_key=1`: a chave manual (32 caracteres, mono) empurra o cartão
   de duas colunas para fora da largura a 924px (rolagem horizontal).

## SECURITY_DESIGN_CONFLICT

Nenhum conflito bloqueante. Notas:

1. **Sessão revogada vs expirada (G-27).** Um 401 do Core com sessão local
   válida não diz porquê; a integração usa sempre `reason=expired` e apaga a
   sessão local. «Revogada» fica para quando o Core disser o motivo.
2. Os códigos de recuperação vão com `Cache-Control: no-store`, e a chave
   manual só com `?show_key=1`, como a D001 pede (ADR-0107).

## CONTRACT_CLARIFICATION

1. **Estado da Instância à porta.** `/ready` responde `degraded` sempre que
   falta um opcional — em produção, por desenho, falta a IA (0 nós). A
   integração lê `degraded` como **OPERACIONAL** (os críticos respondem), a
   mesma leitura do distintivo CORE; `blocked` e sem resposta são
   **INDISPONÍVEL**. «DEGRADADA» não aparece. Confirmem.
2. **Mensagens do Core só em português.** A recusa do login (uma só para todas
   as falhas) é traduzida no Workspace (`auth.refused.sign_in`); as restantes
   (política de palavra-passe, código de MFA) chegam como o Core as escreve.
3. **Indicadores e contadores.** «activas» = unidades `active`; «em
   investigação» = ideias de `discovery` a `project_candidate`, não
   promovidas; «em execução» = projectos `active`; «catalogados» = todos os
   datasets visíveis. Projectos a 2 colunas lista os do membro (`mine`); a 1
   coluna conta todos os visíveis em execução.
4. **Continuar trabalho** mostra só notas e ficheiros pessoais (o que o Core
   regista por membro). Ideias, projectos e datasets precisam de um registo de
   abertura/escrita por membro (FG-015).
5. **Ficheiros pessoais** ligam a `/files` (não há rota por ficheiro pessoal).
6. **Correio**: primeira caixa ligada, Entrada, não lidas filtradas no
   Workspace (o Core não tem filtro de não lidas).
7. **Códigos de recuperação**: o Core emite `XXXXX-XXXXX-XXXXX` (17
   caracteres); na grelha de duas colunas partem em duas linhas.
8. **Arranque**: os nomes dos componentes não vinham no pacote; ficaram numa
   entrada i18n do código (`boot.component.*`, pt/en/fr). As razões do Core
   (só em pt) não se mostram.
9. **Relógio**: `data-format="short"` usa `Intl` com `weekday: short`; em
   pt-PT dá «Segunda», não «Seg» como na referência.
10. **Fim de sessão**: a referência mostra um cartão com nome e unidade e um
    texto mais longo; o código da D001 não o tem — e a unidade não pode
    aparecer (§34.3). Vale o código.
