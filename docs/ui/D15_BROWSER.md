# D15 · Browser

CSS: `static/ods-d15-browser.css`. Referência: `Ocinye Browser.dc.html` (F–Y, AB).

## Identidade
- Nome: **Browser**; identidade de apoio **Ocinye Browser**. Aplicação de sistema, categoria **Sistema** no Lançador, multi-janela, afixável.
- Ícone `ods-globe`. Fundo do ícone navy com traço dourado, como o Terminal.

## Abas ≠ janelas
```
Gestor de Janelas ── Janela Browser (ods-window, D5)
                      └ Browser Manager ── aba 1 · aba 2 · aba 3
```
- O Gestor de Janelas só conhece janelas (`window list` mostra «Browser · 3 abas»).
- As abas são do Browser Manager. Várias janelas Browser são independentes (uma aba nunca passa a janela sozinha).
- Janela privada = janela Browser com `data-private`; nunca partilha abas, cookies nem histórico com as outras.

## Anatomia
```
ods-window (D5)  [título: «Browser · 3 abas» · contexto · min/max/fechar]
└ .ods-browser [data-private] [data-runtime="web|desktop|dedicated"]
  ├ .ods-browser__private          (só privada)
  ├ .ods-browser__tabs             abas + nova aba
  ├ .ods-browser__toolbar          voltar · avançar · recarregar/parar · barra de endereço · transferências · Nye · acções
  ├ .ods-browser__offline          (só com o Ocinye inacessível)
  ├ .ods-browser__find             (Ctrl+F)
  ├ .ods-browser__body
  │ ├ .ods-browser__viewport       progresso · página (webview no Desktop; página interna ou recurso no Web)
  │ └ .ods-browser-nye             (painel do Nye, opcional)
  └ diálogos: .ods-browser-dialog  (transferir, carregar, guardar no Knowledge) · diálogos nativos do anfitrião
```

## Estrutura
```html
<section class="ods-browser" data-oc="browser" data-window-id="browser-1" data-runtime="web">
  <div class="ods-browser__tabs">
    <div class="ods-browser__tablist" role="tablist" data-oc="browser-tabs" data-ocs>
      <div class="ods-browser-tab" role="tab" aria-selected="true" draggable="true" data-oc="browser-tab" data-tab-id="t1" title="ocinye/ocinye-os · https://github.com/ocinye/ocinye-os">
        <span class="ods-browser-tab__fav" data-kind="site|internal|search" aria-hidden="true">G</span>
        <span class="ods-browser-tab__spin" hidden></span>
        <span class="ods-browser-tab__title">ocinye/ocinye-os</span>
        <svg class="ods-icon ods-browser-tab__mic" hidden aria-label="A usar o microfone"><use href="/static/ods-icons.svg#ods-mic"/></svg>
        <button class="ods-browser-tab__close" data-oc="browser-tab-close" aria-label="Fechar aba">…close…</button>
      </div>
    </div>
    <button class="ods-iconbtn ods-iconbtn--sm" data-oc="browser-tab-new" aria-label="Nova aba (Ctrl+T)">…plus…</button>
  </div>

  <div class="ods-browser__toolbar">
    <button class="ods-iconbtn" data-oc="browser-back" aria-label="Voltar">…arrow-l…</button>
    <button class="ods-iconbtn" data-oc="browser-forward" aria-label="Avançar">…arrow-r…</button>
    <button class="ods-iconbtn" data-oc="browser-reload" data-state="idle|loading" aria-label="Recarregar">…reload | close…</button>
    <form class="ods-omnibox" data-oc="browser-omnibox" role="search" data-focused>
      <button type="button" class="ods-omnibox__sec" data-oc="browser-site-info" data-sec="https|http|cert|internal|private|none" aria-label="Informação do site">…lock…<span>Não seguro</span></button>
      <span class="ods-omnibox__pretty" aria-hidden="true"><span class="ods-omnibox__scheme">http://</span><span class="ods-omnibox__host">github.com</span><span class="ods-omnibox__path">/ocinye/ocinye-os</span></span>
      <input class="ods-omnibox__input" name="q" autocomplete="off" spellcheck="false" aria-label="Endereço e pesquisa" placeholder="Pesquisar ou escrever endereço">
      <span class="ods-omnibox__chip" data-kind="mic|zoom" hidden>…</span>
      <button type="button" class="ods-iconbtn ods-iconbtn--sm" data-oc="browser-bookmark" aria-pressed="false" aria-label="Marcador">…star…</button>
      <div class="ods-omnibox__suggest ods-menu" role="listbox" hidden>…</div>
    </form>
    <button class="ods-iconbtn" data-oc="browser-downloads" aria-label="Transferências"><svg class="ods-icon">…download…</svg><span class="ods-browser__dlbar" hidden><i></i></span></button>
    <button class="ods-iconbtn ods-browser__nyebtn" data-oc="browser-nye" aria-pressed="false" aria-label="Nye">…nye…</button>
    <button class="ods-iconbtn" data-oc="browser-actions" aria-haspopup="menu" aria-label="Acções da página">…more…</button>
  </div>
  …
</section>
```

### Barra de endereço (`.ods-omnibox`)
- **Origem sempre visível.** Sem foco mostra `.ods-omnibox__pretty`: domínio a 600, caminho em `--ods-text-meta`. O `input` fica vazio e **sem placeholder** enquanto o `pretty` está visível (evita sobreposição). Com foco, o `input` mostra o URL completo seleccionado.
- Nunca se esconde o domínio, nem em ecrã inteiro da página (aí fica a pílula de saída com o domínio).
- Aceita URL, domínio, pesquisa e `ocinye://`. Sugestões (`role="listbox"`): 1.ª linha = acção directa («ir para …», «Pesquisar «…»», «Abrir no Ocinye: …»), depois marcadores e histórico (máx. 4). ↑↓ Enter Esc.
- A barra **não** é o Nye nem a Superfície Universal de Comandos: não há respostas de IA aqui.
- Indicador `data-sec`:

| sec | Ícone | Texto | Cor |
|---|---|---|---|
| https | lock | — | meta |
| http | warning | «Não seguro» | aviso |
| cert | warning | «Certificado inválido» | erro |
| internal | nye | «Ocinye» | navy sobre `--ods-surface-muted` |
| private | private | «Privado» | `--ods-browser-private` |
| none (DNS) | globe | — | meta |

- **Largura:** a caixa tem mínimo de 120px. O caminho corta-se primeiro; o domínio só se corta quando já não há caminho visível, e **pela esquerda** (`direction: rtl` + reticências), para o domínio registável («example.com») ficar sempre à vista. Em janelas com menos de 520px (lado a lado, mobile) a caixa mostra **só o domínio registável, inteiro e nunca cortado** (calculado com a Public Suffix List: «meet.example.com» → «example.com»; o host completo vai para `title`/`aria-label` e para o popover de informação do site); caminho e esquema escondem-se. Avançar, Transferências, o marcador (⌃D) e o botão do Nye passam para o menu da página e o indicador de segurança fica só com o ícone (o texto vai para o `title`). O domínio nunca desaparece.
- Popovers (informação do site, permissão) e sugestões são filhos da **barra**, não da caixa: `left: 8px`, largura `min(380px, 100% − 16px)` da barra.
- `browser-site-info` abre um popover: tipo de ligação, «Site externo: não recebe a sessão, os cookies nem os dados do Ocinye», permissões deste site, «As permissões da conta Ocinye não se aplicam a sites».

### Páginas (`.ods-browser__viewport`)
| data-page | Quando | Notas |
|---|---|---|
| `newtab` | nova aba / página inicial | pesquisa, 4 favoritos, 4 recentes. **Sem widgets, sem dashboard** |
| `site` | conteúdo externo | no Desktop é um webview do Browser Manager; no Web só quando o site permite incorporar |
| `loading` | a carregar | barra de 2px dourada no topo do viewport + esqueleto; recarregar vira **Parar** |
| `frame-blocked` | Web, site recusa incorporar | V · recurso honesto (abaixo) |
| `dns` | nome não resolvido | K |
| `cert` | certificado inválido | L |
| `history` `bookmarks` `downloads` `settings` | `ocinye://browser/*` | páginas internas com a superfície Ocinye e o indicador «Ocinye» |

### Recurso honesto no Web (`frame-blocked`)
- Título: «Este site não pode ser mostrado dentro do Ocinye Web». Texto: «{host} não permite ser incorporado noutras páginas. É uma regra de segurança do site e do navegador, não uma falha do Ocinye.»
- Acção principal: **Abrir num novo separador do navegador** → `window.open(url, '_blank', 'noopener,noreferrer')`. Secundária: Copiar ligação.
- Nota verde: «Este separador do Ocinye não muda: janelas, ficheiros abertos e sessão ficam como estão. No Ocinye Desktop, este site abre aqui dentro.»
- Informativo, não erro (ícone `ods-external`, fundo `--ods-state-info-bg`). Nunca navegar a aba do Ocinye para fora.
- Detecção: o Core não sabe se um site recusa; o Browser tenta o `iframe` sandbox, e sem evento `load` utilizável em 3 s ou com `X-Frame-Options`/`frame-ancestors` conhecido via G-18 (`probe`) mostra este estado. Lista de sites permitidos opcional por política.

### Erros
- **DNS:** «Não foi possível encontrar {host}». Acções: Tentar de novo · Pesquisar «{host}». Nota: «A falha é deste site. O Ocinye continua ligado.»
- **Certificado:** «A ligação a {host} não é privada». Detalhe com a causa (expirado há n dias). Acções: **Voltar à segurança** (primária) · «Continuar mesmo assim (não seguro)» (texto vermelho, secundário). Nota: «O Ocinye nunca envia a sua sessão nem os seus dados para sites externos.» Nunca «continuar» por omissão; a exceção vale só para esta aba e sessão.

### Menu de acções da página (`data-oc="browser-actions"`)
Grupos `ods-menu` com rótulos:
- **OCINYE** (só em páginas externas; desactivadas sem ligação ao Ocinye, com o motivo no `title`): Guardar página no Knowledge · Adicionar à Bibliografia · Criar nota a partir da página · Anexar a projecto… · Enviar ao Nye.
- **PÁGINA**: Procurar na página ⌃F · Copiar ligação · Imprimir… ⌃P (diálogo do anfitrião/navegador; nenhum motor próprio) · Ecrã inteiro da página F11 · Abrir no navegador do sistema (Desktop) / Abrir num novo separador do navegador (Web).
- **BROWSER**: Nova janela ⌃N · Nova janela privada ⇧⌃N · Histórico ⌃H · Marcadores · Transferências · Definições do Browser.
- Rodapé: zoom − 100 % + (a percentagem aparece também como chip na barra quando ≠ 100 %).
- Só aparecem acções suportadas e autorizadas. Uma acção Ocinye é do membro: o site não sabe que foi usada.

### Permissões de site (`.ods-browser-perm`)
Popover ancorado à barra de endereço:
```
meet.example.com quer usar:
[🎤 Microfone]
[Bloquear] [Permitir uma vez] [Permitir]
A sua conta Ocinye não dá acesso automático a sites. …
```
- Três respostas; «Permitir» guarda por site (G-19 quando existir, local até lá). Em uso: chip vermelho de microfone na barra e ícone na aba.
- Tipos: microfone, câmara, localização, notificações, área de transferência, pop-ups, transferências. Pop-ups permitidos abrem **numa aba controlada**, nunca numa superfície privilegiada do Ocinye.
- No Web, o prompt é o do navegador anfitrião (o Ocinye não o substitui).

### Transferências
- **Desktop — diálogo `.ods-browser-dialog`** «Guardar report.pdf»: ficheiro (tipo, tamanho, origem) + escolha:
  - **Ocinye Files** — «Meus ficheiros / Downloads · fica disponível em Ficheiros, Correio, Projectos, Knowledge e Nye» (primeira opção; omissão salvo política). Desactivada sem ligação ao Ocinye.
  - **Este computador** — abre o diálogo nativo de guardar (G-20 não é preciso para este caminho).
- Progresso no botão de transferências (barra de 2px) e no painel; conclusão em toast: «report.pdf guardado no Ocinye Files · Meus ficheiros / Downloads» [Abrir] [Mostrar em Ficheiros].
- **Web**: o diálogo explica que o navegador faz a transferência para o computador e que o Ocinye Web não acede ao disco; sugere carregar depois em Ficheiros. Nunca fingir acesso ao disco.

### Carregamentos (sites que pedem ficheiro)
- Desktop: «{site} pede um ficheiro» — Ocinye Files · Este computador. Ocinye Files abre um seletor do Ocinye (só leitura, 1 ficheiro); confirmação «Partilhar 1 ficheiro com {site}», registado na actividade do ficheiro. O site nunca recebe acesso ao Ocinye Files, só o ficheiro escolhido.
- Web: só o seletor do navegador.

### Janela privada
- Faixa `.ods-browser__private` (fundo `--ods-browser-private`, texto claro): «Janela privada · histórico e cookies temporários · apagados ao fechar». Sem dramatismo: sem óculos, sem máscara.
- Página inicial própria com a explicação: histórico, cookies e dados de sites temporários; marcadores e transferências ficam; o Nye não usa estas páginas sem perguntar.
- Ao fechar: toast «Janela privada fechada · n páginas, cookies e dados de sites apagados». Nada passa para o histórico normal.

### Histórico · Marcadores · Transferências · Definições
- **Histórico** (`ocinye://browser/history`): pesquisa, grupos Hoje/Ontem/data, hora · título · domínio · remover; «Limpar: Última hora · Hoje · Tudo». Nota fixa: «É privado: não faz parte do Audit Log e os administradores não o vêem.» Linha de política da instância (retenção máxima). Na janela privada, grupo «Esta janela privada · temporário».
- **Marcadores**: pastas (Favoritos + pastas do membro) · lista · remover · nova pasta. Sincronizados com a instância (G-19).
- **Transferências**: ficheiro · destino (Ocinye Files / computador) · estado.
- **Definições do Browser**: Pesquisa (motor predefinido, independente dos fornecedores de IA) · Transferências (destino predefinido; política pode fixar) · Histórico (guardar, retenção, máximo da instância) · Privacidade (cookies de terceiros, não rastrear, «sessão do Ocinye com sites: Nunca» bloqueado) · Permissões de sites (omissões + excepções por site com repor) · Navegação privada (permitir; «o Nye pergunta sempre» bloqueado) · Aparência (tema das páginas do Browser) · Atalhos (hierarquia). Subtítulo: «Separadas da conta Ocinye e do início de sessão.»
- **Perfis de browser** (Standard/Work/Personal/Private) ficam para depois; nunca chamar «perfil» a nada que colida com o Instance Profile.

### Nye + Browser (`.ods-browser-nye`)
- Painel à direita, 360px (máx. 42% da janela). Abre pelo botão Nye da barra ou «Enviar ao Nye».
- **Chip de contexto** por cima do campo: «A usar a página actual · {título}» [×]. Sem chip, o Nye não lê a página. «+ Incluir página actual» volta a juntá-la.
- Fluxo (W, J80): pergunta → passos visíveis: «Contexto: página actual · {host} · n palavras» → «Política de IA: permitido · modelo local · sem envio externo» → «A processar localmente…» → resposta + **FONTES** (URL da página).
- Sem modelo compatível: aviso âmbar «Sem modelo compatível para resumir. O Browser continua a funcionar. Sem IA, posso:» Guardar no Knowledge · Criar nota com a ligação · Procurar na página.
- **Página privada**: o chip não se liga sozinho; pedir com a página abre «Esta página está numa janela privada. Incluí-la no pedido? Só este pedido a usa, segundo a política de IA. Nada fica no histórico.» [Sem a página] [Incluir só desta vez].
- «Nye, abre o GitHub» → Browser Manager abre uma aba e o Nye responde «Abri github.com num novo separador deste Browser», fonte `browser.open`.
- Procurar na página (Ctrl+F) é local: nada vai ao Nye (nota na barra de procura).

### Ecrã inteiro
| Estado | Quem | Saída |
|---|---|---|
| Maximizar janela | Gestor de Janelas | botão/duplo clique |
| Ecrã inteiro da página | Browser (F11) | pílula fixa no topo «{host} em ecrã inteiro · Sair (Esc)» — nunca escondida |
| Full Workspace | Ocinye Desktop | Ctrl+Shift+F; aviso ao entrar |
| Ecrã inteiro do anfitrião | sistema | do sistema |
Estados não se aninham sem saída visível: Esc sai sempre do mais interior.

### Teclado (hierarquia)
1. **Browser em foco**: ⌃T nova aba · ⌃W fechar · ⌃L barra · ⌃R recarregar · ⌃F procurar · ⌃D marcador · ⌃H histórico · ⌃± / ⌃0 zoom · Alt+← → · Esc.
2. **Sempre do Ocinye** (uma página nunca os intercepta): ⌘/Ctrl+J Lançador · ⌘/Ctrl+K Nye · Ctrl+Shift+` Terminal · Ctrl+Shift+F Full Workspace (Desktop).
3. **Sempre do anfitrião**: Alt+Tab, ⌘Tab, tecla Windows.
No Web, atalhos que o navegador anfitrião reserva (⌃T, ⌃W, ⌃N) não se podem interceptar: mostram-se só no Desktop; no Web os mesmos comandos estão nos botões.

## Estados obrigatórios (cinco + os do Browser)
vazio (nova aba sem recentes) · a carregar · erro (DNS/cert) · negado (política bloqueia o site: «Bloqueado pela política da instância» + contacto) · indisponível (Web sem incorporação; Ocinye offline para acções Ocinye).

## Acessibilidade
`role="tablist/tab"`, `aria-selected`; barra `role="search"`; sugestões `listbox/option`; progresso da aba `aria-busy` na aba; popovers `role="dialog"` com foco inicial no primeiro botão; nunca só cor (texto no indicador de segurança); `prefers-reduced-motion` corta spinners e transições.
