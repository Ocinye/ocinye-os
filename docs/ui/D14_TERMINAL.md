# D14 · Terminal

CSS: `static/ods-d14-terminal.css`. Ícones: `static/ods-icons-d14.svg`. i18n: `DS_TERMINAL`.
Referência: `design-reference/Ocinye Terminal.dc.html` (rail esquerdo = ecrãs A–T e percursos J1–J5).

## Identidade
- Nome na UI: **Terminal**. Identidade de apoio: **Ocinye Terminal**. Shell: **ocsh**.
- Aplicação de sistema de primeira classe. Categoria nova no Lançador: **Sistema** (`system`). Multi-janela.
- Atalho: **Ctrl/⌘ + Shift + `** (`e.code === 'Backquote'`). Ctrl+` fica para o seletor de janelas (D5). Mostrar o atalho no Lançador (`ods-kbd`) e na barra da janela.
- Não é widget de Desktop. Pode ser afixado na barra lateral (`apps pin terminal`).
- Superfície **escura por omissão**, mesmo com o sistema em modo claro (decisão de Design). Tema claro opcional nas preferências.
- Tipo: `--ods-font-mono` (IBM Plex Mono) para o viewport; `--ods-font-sans` para cromados, preferências e respostas do Nye.

## Anatomia
```
ods-window (D5)                          ← barra da janela do Gestor de Janelas: ícone, "Terminal", contexto, atalho, min/max/fechar
└ .ods-term  [data-theme="dark|light"] [data-density="compact|normal|comfy"] [data-cursor="block|bar|under"]
  ├ .ods-term__band            (só com sessão elevada)
  ├ .ods-term__tabs            separadores + novo + dividir V/H + inspector + preferências
  ├ .ods-term__alert           (só desligado / a religar)
  ├ .ods-term__body
  │ ├ .ods-term__panes [data-split="row|col"]
  │ │ └ .ods-term-pane ×1..2
  │ │   ├ .ods-term-pane__head   (só com divisão)
  │ │   └ .ods-term-pane__view  role="log" aria-live="polite"
  │ │       ├ .ods-term-entry ×n  (prompt + blocos de saída)
  │ │       └ .ods-term-input     (prompt vivo + textarea + popovers)
  │ └ .ods-term-inspector    (opcional)
  ├ .ods-term__keys            (só mobile: teclas acessórias)
  ├ .ods-term__status          barra de estado
  └ .ods-term-prefs            folha lateral de preferências
```

## Estrutura
```html
<section class="ods-term" data-oc="terminal" data-theme="dark" data-density="normal" data-cursor="block">
  <div class="ods-term__band" data-oc="term-elev-band" role="status" hidden>
    <svg class="ods-icon"><use href="/static/ods-icons.svg#ods-lock"/></svg>
    <span class="ods-term__band-title">SESSÃO DE ADMINISTRAÇÃO</span>
    <span class="ods-term__band-caps">members · resources · policies · apps.admin</span>
    <span class="ods-term__band-exp" data-oc="term-elev-expiry">expira em 14:32</span>
    <button class="ods-term__band-btn" data-oc="term-elev-drop">Terminar</button>
  </div>

  <div class="ods-term__tabs">
    <div class="ods-term__tablist" role="tablist" data-oc="term-tabs">
      <div class="ods-term-tab" role="tab" aria-selected="true" draggable="true" data-oc="term-tab" data-tab-id="t1">
        <span class="ods-term-tab__dot" data-kind="run|elev" hidden></span>
        <span class="ods-term-tab__name">Terminal 1</span>
        <button class="ods-term-tab__close" data-oc="term-tab-close" aria-label="Fechar separador">…close…</button>
      </div>
    </div>
    <button class="ods-term__tool" data-oc="term-tab-new" aria-label="Novo separador">…plus…</button>
    <span class="ods-term__spacer"></span>
    <button class="ods-term__tool" data-oc="term-split-v" aria-pressed="false" aria-label="Dividir na vertical">…split-v…</button>
    <button class="ods-term__tool" data-oc="term-split-h" aria-pressed="false" aria-label="Dividir na horizontal">…split-h…</button>
    <button class="ods-term__tool" data-oc="term-inspector" aria-pressed="false" aria-label="Inspector de comandos" hidden>…inspector…</button>
    <button class="ods-term__tool" data-oc="term-prefs" aria-label="Preferências do Terminal">…settings…</button>
  </div>

  <div class="ods-term__alert" data-oc="term-offline" role="alert" hidden>…offline… <strong>Sem ligação ao Core</strong> <span>a religar · tentativa 3 · a saída anterior mantém-se visível</span> <button data-oc="term-retry">Tentar agora</button></div>

  <div class="ods-term__body">
    <div class="ods-term__panes" data-split="row">
      <div class="ods-term-pane" data-oc="term-pane" data-session-id="s1" data-focused>
        <div class="ods-term-pane__view" data-ocs role="log" aria-live="polite" aria-label="Saída do terminal">
          <!-- entradas (ver «Entrada») -->
          <div class="ods-term-input" data-oc="term-input" data-mode="normal|confirm-word|confirm-yn|mfa|search|running|offline">
            <span class="ods-term-prompt">…</span>
            <div class="ods-term-input__field">
              <div class="ods-term-input__mirror" aria-hidden="true"><span class="ods-term-input__typed">…</span><span class="ods-term-input__ghost">…</span><span class="ods-term-input__cursor"></span></div>
              <textarea rows="1" spellcheck="false" autocomplete="off" autocapitalize="off" aria-label="Linha de comando" data-oc="term-line"></textarea>
            </div>
          </div>
        </div>
      </div>
    </div>
    <aside class="ods-term-inspector" data-oc="term-inspector-panel" hidden>…</aside>
  </div>

  <div class="ods-term__status" data-oc="term-status">
    <span class="ods-term__conn" data-state="ok|retry|lost"><i></i>Ligado ao Core</span>
    <span>UENR-001</span><span>Administrador</span><span class="ods-term__spacer"></span><span>IA: sem recurso</span><span>ocsh 1.0</span>
  </div>
</section>
```

### Prompt
```html
<span class="ods-term-prompt">
  <span class="ods-term-prompt__who">fidel@ocinye</span>
  <span class="ods-term-prompt__ctx">UENR-001</span>
  <span class="ods-term-prompt__cwd">~/files/Relatórios</span>
  <span class="ods-term-prompt__elev" hidden>ADMIN</span>
  <span class="ods-term-prompt__caret">›</span>
</span>
```
- Uma linha: `membro@instância  contexto  pasta  [ADMIN]  ›`. Em painéis estreitos o comando passa para a linha seguinte (`flex-wrap`), nunca encolhe abaixo de 16ch.
- `context personal` mostra **pessoal / personal / personnel** no campo de contexto.
- Nos modos especiais, o prompt é substituído por `.ods-term-input__mode`: «Escreva REVOGAR ›» (erro), «Continuar? [s/N] ›» (aviso), «Código MFA ›» (aviso, `-webkit-text-security: disc`), «(pesquisa) ›» (ouro), «a executar ›» (ouro, textarea `disabled`), «sem ligação» (erro, `disabled`).
- Continuação: linha a terminar em `\` + Enter, ou Shift+Enter. Linhas seguintes com goteira «…» (`.ods-term-input__gutter`).

### Entrada
```html
<div class="ods-term-entry" data-oc="term-entry" data-exit="0">
  <div class="ods-term-entry__line">
    <span class="ods-term-prompt">…instantâneo do prompt no momento…</span>
    <span class="ods-term-entry__cmd">projects list | filter active</span>
    <span class="ods-term-entry__meta" data-tone="err|ok|run">exit 127 · 48 ms</span>
  </div>
  <!-- blocos -->
</div>
```
- Meta por omissão: nada em sucesso; `exit N` em falha. Com «Mostrar código de saída»: `✓ 0`/`✕ N`. Com «Mostrar duração»: `48 ms`. Enquanto corre: «a executar · ⌃C cancela».

### Blocos de saída (`.ods-term-out` + `data-kind`)
| data-kind | Uso | Notas |
|---|---|---|
| `lines` | texto, árvore, estado, recibos, ajuda | `.ods-term-out__line` com `style` proibido → recuo por `data-indent="2"`. Segmentos: `.ods-term-seg` com `data-tone="fg|dim|faint|ctx|key|ok|warn|err|gold|link|str|num"` |
| `table` | listagens tipadas | `<div role="table">` em grelha; cabeçalho `.ods-term-table__h` (maiúsculas, tracking); rodapé `.ods-term-table__foot` («5 projectos · contexto UENR-001»); cabeçalho de pipeline opcional `.ods-term-out__head` («pipeline: projects.list → filter(active) → sort(name)») |
| `json` | `--json` / `export json` | cabeçalho «application/json · 5 projectos» + botão Copiar (`data-oc="term-copy"`); tokens `data-tone`: chave=`ctx`, string=`str`, número/bool/null=`num`, pontuação=`faint`. Nunca em cartões |
| `progress` | upload, backup | `role="progressbar"` + `aria-valuenow`; barra 200×6; estados `run|done|cancelled` |
| `note` | erros, avisos, sucesso | glifo + título + detalhe + sugestões. `data-tone="ok ✓|warn !|err ✕|deny ⊘|info ·"`. **Sem caixa com borda lateral** |
| `confirm` | risco médio/alto, elevação, proposta do Nye | caixa com borda completa (`data-tone="warn|err"`): cabeçalho (título + risco), grelha chave/valor (Acção, Alvo, Efeitos, Preserva, Reversível, Capacidade, Auditoria), linha de estado `wait|ok|aborted|bad` |
| `logs` | `system logs --follow` | cabeçalho: estado (a seguir/em pausa/terminado), origem, campo filtrar, Pausa/Retomar; linhas `ts · nível · serviço · mensagem`; altura máx. 240px com scroll próprio |
| `nye` | respostas do Nye | hexágono ouro + «o Nye» + rota em `faint` («rota: local · Qwen 2.5 7B · node-02 · UENR-001»); texto em sans; «o Nye está a pensar…» com pulso |

- Ligações: `.ods-term-seg[data-go="run|fill|open"]`. `run` executa o comando, `fill` coloca na linha sem executar (sempre `fill` para comandos de risco), `open` abre a janela/recurso via Gestor de Janelas (`ocinye://projects/p-0142`).
- Semântica nunca só por cor: estados com glifo (● ◐ ○ ✓ ✕ ⊘).

### Popovers
- **Autocompletar** (`.ods-term-ac`, `role="listbox"`): acima da linha, alinhado à palavra actual. Linha: tipo (`cmd|sub|opt|recurso`), texto com a parte já escrita a negrito, descrição (sans), meta (ID/contexto). Rodapé: «Tab completa · ↑↓ navega · → aceita sugestão · Esc fecha». Sugestão fantasma inline (`.ods-term-input__ghost`) sempre que houver prefixo único.
- **Seletor de recursos**: `projects open <Tab>` lista projectos (ID · nome · contexto); `members revoke` só lista membros com sessão elevada.
- **Pesquisa no histórico** (`.ods-term-rs`): Ctrl+R. Cabeçalho «PESQUISA NO HISTÓRICO · n / total», correspondência sublinhada a ouro, Enter coloca na linha (não executa), Ctrl+R seguinte, Esc repõe.
- **Colar várias linhas** (`.ods-term-paste`): mostra as linhas; linhas de alto impacto a vermelho + aviso «cada uma pedirá confirmação escrita». Acções: Executar N linhas · Rever antes · Cancelar. Uma linha nunca mostra aviso.

### Barra de separadores
- Novo, fechar, renomear (duplo clique → input inline), reordenar (arrastar), menu de contexto: Renomear · Duplicar sessão · Dividir ao lado · Fechar. Nota no menu: «a sessão de administração nunca é duplicada».
- Ponto no separador: ouro = sessão elevada; verde a pulsar = comando a correr.
- Divisão: máx. 2 painéis, V (`row`) ou H (`col`). Painel focado com borda `--ods-term-line-2`; cabeçalho «1 · UENR-001 · a executar» + fechar painel.

### Inspector de comandos (`.ods-term-inspector`, 280px)
- Visível só a admin/operador ou com «Modo de depuração». Linhas: COMANDO · CAPACIDADE · ÂMBITO · AUTORIDADE · POLÍTICA · RESULTADO · DURAÇÃO · AUDITORIA. Nota: «O Terminal, o Nye e a interface chamam as mesmas capacidades do Core.»

### Preferências do Terminal (`.ods-term-prefs`, folha lateral 400px, superfície clara)
Nome «Preferências do Terminal» (nunca «perfil», para não colidir com Instance Profile).
| Secção | Linhas |
|---|---|
| Aparência | Tema (Escuro/Claro) · Tamanho da letra 11–18 · Cursor (Bloco/Barra/Sublinhado) · Densidade (Compacta/Normal/Ampla) |
| Sessão | Contexto predefinido · Ao abrir (Boas-vindas/Vazio) · Scrollback (1 000/5 000/10 000) |
| Histórico | Guardar histórico · Omitir segredos · Apagar histórico… (→ `history clear` com confirmação) |
| Saída | Mostrar duração · Mostrar código de saída · Copiar ao seleccionar · Modo de depuração |
| Segurança | Avisar ao colar várias linhas · Confirmações de alto impacto: **bloqueado** (cadeado, «Core») |
Preferências locais até G-15; nenhuma altera permissões.

## Estados
| Estado | Ecrã | Tratamento |
|---|---|---|
| primeira abertura | A | «Ocinye Terminal» / «Powered by ocsh 1.0 · instância Ocinye · contexto UENR-001» / «Escreva help para começar.» + 4 exemplos clicáveis. Sem tutorial |
| vazio | — | lista vazia → linha `dim` («Sem projectos neste contexto.») |
| a carregar | G, Q | bloco `progress` a actualizar; prompt «a executar»; outros separadores continuam utilizáveis |
| erro | H | `note err` + «Quis dizer …?» clicável (distância ≤ 2). Palavras POSIX (`ls`, `cd`, `rm`…) → sugerem `files …` |
| negado | I | `note deny` «Permissão negada» sem revelar a capacidade; comando ausente do autocompletar |
| requer elevação | K | `note warn` «Requer sessão de administração» + `session elevate` |
| indisponível (IA) | J2 sem IA | `note warn` «O Nye não está disponível — sem recurso de inferência» + equivalente determinístico + `ai status` (exit 69) |
| desligado | P | faixa `alert`, ponto vermelho a pulsar, textarea `disabled` com placeholder «Sem ligação ao Core — os comandos não são aceites». Saída anterior visível. Ao religar: «● Ligação restabelecida · sessão retomada» |
| cancelado | Q | `note warn` «Cancelado aos 41 %» com o que **realmente** ficou feito (ex.: «3 de 8 volumes já copiados ficam no destino, marcados como snapshot parcial. A origem não foi alterada.») exit 130. Nunca dizer «cancelado» depois de efeito irreversível |

## Teclado
| Tecla | Acção |
|---|---|
| Enter | executar (linhas múltiplas sem `\` → executa em sequência) |
| Shift+Enter · `\`+Enter | continuação |
| Tab | completar; 2+ opções → prefixo comum + popover |
| → (fim da linha) | aceitar sugestão fantasma |
| ↑ / ↓ | histórico (ou navegar popover) |
| Ctrl+R | pesquisa no histórico |
| Ctrl+C | com selecção → copiar; a correr → cancelar; em confirmação → abortar; senão → `^C` e nova linha |
| Ctrl+L · `clear` | limpar o viewport (não apaga o histórico) |
| Esc | fechar popover / abortar confirmação |
Os atalhos tratados fazem `stopPropagation` para não chegarem ao shell (Ctrl+L não bloqueia o ecrã dentro do Terminal).

## Mobile / tablet (D11)
- Mobile: ecrã inteiro, sem janela redimensionável. Cabeçalho 54px (voltar · «Terminal» · `fidel@ocinye · UENR-001` · contador de separadores · preferências). Teclas acessórias 44×44 com scroll horizontal: Tab ⌃C ⌃R ↑ ↓ | - ~ / " ?. Letra ≤ 13px. Inspector escondido.
- Tablet: janela inteira; divisão só se cada painel tiver ≥ 360px.

## Acessibilidade
- Viewport `role="log" aria-live="polite"`; tabela `role="table"`; progresso `role="progressbar"`; popovers `role="listbox"`/`option` com `aria-selected`; separadores `role="tab"`.
- Foco: `--ods-focus-ring` em todos os botões; a textarea não tem anel (o cursor é o indicador).
- `prefers-reduced-motion`: sem piscar do cursor, sem pulsos, sem transições de janela.
- Texto seleccionável em todos os blocos; `::selection` `rgba(143,184,224,.34)`.

## Ponte com o Gestor de Janelas (D5)
O Terminal **não manipula o DOM** de outras janelas. Chama o gestor do cliente (mesmo módulo que o Lançador usa):
| Comando | Chamada | Resposta no Terminal |
|---|---|---|
| `open files` / `apps open files` | `wm.open('files')` | «✓ Ficheiros aberto  janela files-1» |
| `open project nzayilu` | `wm.open('projects', {id})` | «✓ Projecto aberto: NzaYilu  ocinye://projects/p-0142» |
| `window list` | `wm.list()` | tabela ID · Aplicação · Estado (em foco/aberta/minimizada) |
| `window tile files notes` | `wm.snap(a,'left')`, `wm.snap(b,'right')` | «✓ Ficheiros ▌▐ Notas lado a lado»; janela em falta → erro + `open notes` |
| `window focus|minimize X` | `wm.focus/minimize` | nota ok |
| `desktop reset` | fluxo D4 «Repor» (confirmação [s/N]) | «Desktop reposto · versão 5» |
| «Nye, abre o Terminal» | `wm.open('terminal')` | sem caso especial |
