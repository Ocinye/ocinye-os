/*
 * Ocinye Workspace — camada de interacção.
 *
 * As páginas são renderizadas no servidor (Leptos SSR). Este ficheiro trata
 * apenas do que é comportamento de DOM e não pertence ao servidor: abrir a
 * command palette, colapsar a sidebar, abrir o menu "+ Criar", alternar tabs
 * locais e a densidade das tabelas.
 *
 * O que este ficheiro NUNCA faz, por desenho:
 *   - decidir autorização;
 *   - guardar tokens ou dados institucionais;
 *   - obter dados — isso é do servidor, com a sessão dele.
 *
 * É progressive enhancement: sem JavaScript, a navegação, os formulários e
 * todas as tabelas continuam a funcionar. Ver ADR-0019.
 */

(() => {
  'use strict';

  /* ── A zona de quem olha ────────────────────────────────────────────────
   *
   * O servidor renderiza o Calendário, e para agrupar dias precisa de saber
   * onde a pessoa está. Sem isto agrupava em UTC, e um compromisso das 00:30
   * em Lisboa aparecia no dia anterior, às 23:30.
   *
   * Vai num cookie porque a renderização é do servidor: um valor que só o
   * browser conhecesse chegava tarde de mais para decidir o que desenhar.
   * O que ele leva é o nome de um fuso — não é segredo e não identifica
   * ninguém. `SameSite=Lax` porque não tem nada que atravessar sítios. */
  (function declararZona() {
    try {
      var zona = Intl.DateTimeFormat().resolvedOptions().timeZone;
      if (!zona) return;
      if (document.cookie.indexOf('oc_tz=' + zona) !== -1) return;
      document.cookie =
        'oc_tz=' + encodeURIComponent(zona) + ';path=/;max-age=31536000;SameSite=Lax';
    } catch (erro) {
      /* Sem zona resolvida, o servidor fica em UTC — que é a resposta menos
       * errada quando não se sabe onde a pessoa está. */
    }
  })();


  const $ = (sel, root = document) => root.querySelector(sel);
  const $$ = (sel, root = document) => Array.from(root.querySelectorAll(sel));

  /* ── Preferências, por utilizador e por browser ────────────────────── */

  const PREFS = 'ocinye.prefs';

  const readPrefs = () => {
    try { return JSON.parse(localStorage.getItem(PREFS) || '{}'); }
    catch { return {}; }
  };
  const writePref = (key, value) => {
    try {
      const prefs = readPrefs();
      prefs[key] = value;
      localStorage.setItem(PREFS, JSON.stringify(prefs));
    } catch { /* modo privado ou armazenamento bloqueado: não é fatal */ }
  };

  /* ── Sidebar colapsável ───────────────────────────────────────────── */

  function initSidebar() {
    const shell = $('.oc-shell');
    const toggle = $('[data-oc="collapse"]');
    if (!shell || !toggle) return;

    // O nome do botão é a acção que ele executa, e a acção inverte-se: quando a
    // barra está recolhida, este botão abre-a. Deixar o rótulo fixo em
    // «Colapsar navegação» faria o leitor de ecrã anunciar o oposto do que
    // acontece — `aria-expanded` diz o estado, mas o nome tem de dizer o gesto.
    const apply = (collapsed) => {
      shell.dataset.side = collapsed ? 'collapsed' : 'expanded';
      toggle.setAttribute('aria-expanded', String(!collapsed));
      const name = collapsed ? 'Expandir navegação' : 'Colapsar navegação';
      toggle.setAttribute('aria-label', name);
      toggle.setAttribute('title', name);
    };

    apply(readPrefs().sidebarCollapsed === true);

    toggle.addEventListener('click', () => {
      const collapsed = shell.dataset.side !== 'collapsed';
      apply(collapsed);
      writePref('sidebarCollapsed', collapsed);
    });
  }

  /* ── Conta e sessão ───────────────────────────────────────────────── */

  /*
   * Uma divulgação, não um menu ARIA.
   *
   * O conteúdo são ligações e um formulário normais, e por isso o `Tab`
   * percorre-os sozinho — que é o que quem usa teclado espera de uma superfície
   * com links. `role="menu"` obrigaria a navegação por setas e faria o `Tab`
   * saltar a superfície inteira: seria prometer um teclado que não existe.
   *
   * O que o JavaScript acrescenta é o que o HTML não sabe fazer: abrir, fechar
   * ao clicar fora, fechar com `Escape` e devolver o foco a quem o abriu.
   */
  function initAccountMenu() {
    const wrap = $('[data-oc="account"]');
    if (!wrap) return;

    const button = $('[data-oc="account-toggle"]', wrap);
    const menu = $('[data-oc="account-menu"]', wrap);
    if (!button || !menu) return;

    const close = ({ restoreFocus = false } = {}) => {
      if (menu.hidden) return;
      menu.hidden = true;
      button.setAttribute('aria-expanded', 'false');
      // Só se devolve o foco quando ele estava aqui dentro. Fechar por um
      // clique noutro sítio não deve arrastar o cursor de volta ao rodapé.
      if (restoreFocus) button.focus();
    };

    const open = () => {
      // Duas superfícies deste tipo abertas ao mesmo tempo seria uma a tapar a
      // outra; a que já estava fecha-se primeiro.
      if (window.ocCloseCreate) window.ocCloseCreate();
      menu.hidden = false;
      button.setAttribute('aria-expanded', 'true');
      const first = $('.oc-account__item', menu);
      if (first) first.focus();
    };

    button.addEventListener('click', (event) => {
      event.stopPropagation();
      menu.hidden ? open() : close({ restoreFocus: true });
    });

    document.addEventListener('click', (event) => {
      if (!menu.hidden && !wrap.contains(event.target)) close();
    });

    // O foco pode sair pelo teclado sem passar por um clique. `focusout`
    // dispara antes de o novo alvo receber o foco, daí o adiamento.
    wrap.addEventListener('focusout', () => {
      window.setTimeout(() => {
        if (!wrap.contains(document.activeElement)) close();
      }, 0);
    });

    window.ocCloseAccount = () => close({ restoreFocus: true });
  }

  /* ── O sino ─────────────────────────────────────────────────────────────
   *
   * Um painel, e não uma página: ver o que chegou é um relance. A página
   * continua a existir, e é para onde o rodapé leva — um painel mostra o que é
   * recente, e um histórico é outra coisa.
   *
   * Fecha como o painel da conta fecha: por clique fora, por `Escape`, e
   * quando o foco sai. É o mesmo gesto, e tem de ter o mesmo comportamento. */
  function initSino() {
    const wrap = $('.oc-sino');
    if (!wrap) return;

    const button = $('[data-oc="abrir-notificacoes"]', wrap);
    const menu = $('[data-oc="notificacoes"]', wrap);
    const lista = $('[data-oc="notificacoes-lista"]', wrap);
    if (!button || !menu || !lista) return;

    const close = ({ restoreFocus = false } = {}) => {
      if (menu.hidden) return;
      menu.hidden = true;
      button.setAttribute('aria-expanded', 'false');
      if (restoreFocus) button.focus();
    };

    const open = () => {
      if (window.ocCloseAccount) window.ocCloseAccount();
      if (window.ocCloseCreate) window.ocCloseCreate();
      menu.hidden = false;
      button.setAttribute('aria-expanded', 'true');
      carregar();
    };

    let aCarregar = false;
    function carregar() {
      if (aCarregar) return;
      aCarregar = true;
      fetch('/notifications/recent', { headers: { Accept: 'application/json' } })
        .then((r) => r.json())
        .then((dados) => desenhar((dados && dados.notifications) || []))
        .catch(() => {
          /* Sem resposta, diz-se. Uma lista vazia diria «não tem nada», que é
           * outra coisa, e faria uma pessoa concluir que não recebeu. */
          lista.textContent = '';
          const erro = document.createElement('p');
          erro.className = 'oc-pop__empty';
          erro.textContent = 'Não foi possível ler as notificações.';
          lista.appendChild(erro);
        })
        .then(() => { aCarregar = false; });
    }

    function desenhar(linhas) {
      lista.textContent = '';

      if (!linhas.length) {
        const vazio = document.createElement('p');
        vazio.className = 'oc-pop__empty';
        vazio.textContent = 'Nada por ler.';
        lista.appendChild(vazio);
        return;
      }

      linhas.forEach((linha) => {
        const destino = destinoDe(linha);
        const item = document.createElement(destino ? 'a' : 'div');
        item.className = 'oc-pop__item oc-sino__linha';
        if (!linha.read) item.className += ' oc-sino__linha--por-ler';
        if (destino) item.href = destino;

        if (!linha.read) {
          const ponto = document.createElement('span');
          ponto.className = 'oc-sino__ponto';
          ponto.setAttribute('aria-hidden', 'true');
          item.appendChild(ponto);
        }

        /* Ícone, título e subtítulo — o mesmo ritmo das linhas do painel da
         * conta. Sem eles a linha é uma frase solta, e o painel parece menos
         * acabado do que os outros mesmo com a superfície igual. */
        item.appendChild(iconeDe(linha));

        const texto = document.createElement('span');
        texto.className = 'oc-sino__texto';
        const titulo = document.createElement('b');
        /* `textContent`: o título de uma notificação é escrito por pessoas. */
        titulo.textContent = linha.title || '';
        texto.appendChild(titulo);

        const legenda = document.createElement('em');
        legenda.textContent = linha.body || generoDe(linha);
        texto.appendChild(legenda);
        item.appendChild(texto);

        if (linha.created_at) {
          const quando = document.createElement('span');
          quando.className = 'oc-sino__quando';
          quando.textContent = relativo(linha.created_at);
          item.appendChild(quando);
        }

        lista.appendChild(item);
      });
    }

    /* O símbolo do sprite, por tipo. Um `<use>` como o resto da interface —
     * uma segunda maneira de desenhar ícones seria uma segunda biblioteca. */
    function iconeDe(linha) {
      const simbolos = {
        message_received: 'oc-messaging',
        message_mention: 'oc-messaging',
        reminder: 'oc-bell',
        event_invited: 'oc-calendar',
        event_cancelled: 'oc-calendar',
      };
      const svg = document.createElementNS('http://www.w3.org/2000/svg', 'svg');
      svg.setAttribute('width', '14');
      svg.setAttribute('height', '14');
      svg.setAttribute('aria-hidden', 'true');
      svg.setAttribute('class', 'oc-sino__icone');
      const use = document.createElementNS('http://www.w3.org/2000/svg', 'use');
      use.setAttribute('href', '/static/icons.svg#' + (simbolos[linha.kind] || 'oc-bell'));
      svg.appendChild(use);
      return svg;
    }

    /* O que é, dito em duas palavras. A linha de baixo do painel da conta diz o
     * que cada acção faz; aqui diz o que cada aviso é. */
    function generoDe(linha) {
      const nomes = {
        message_received: 'Mensagem nova',
        message_mention: 'Mencionaram-no',
        reminder: 'Lembrete',
        event_invited: 'Convite para um compromisso',
        event_cancelled: 'Compromisso cancelado',
      };
      return nomes[linha.kind] || 'Aviso';
    }

    function destinoDe(linha) {
      if (!linha.resource_id) return null;
      if (linha.resource_type === 'conversation') return '/messages/' + linha.resource_id;
      if (linha.resource_type === 'calendar_event') return '/calendar/events/' + linha.resource_id;
      if (linha.resource_type === 'task') return '/my-work';
      return null;
    }

    /* Quanto tempo faz, dito como uma pessoa o diria. Um carimbo completo em
     * cada linha de um painel que se lê de relance é ruído. */
    function relativo(quando) {
      const entao = new Date(quando);
      if (isNaN(entao.getTime())) return '';
      const minutos = Math.round((Date.now() - entao.getTime()) / 60000);
      if (minutos < 1) return 'agora';
      if (minutos < 60) return minutos + 'm';
      const horas = Math.round(minutos / 60);
      if (horas < 24) return horas + 'h';
      const dias = Math.round(horas / 24);
      if (dias < 7) return dias + 'd';
      return entao.toLocaleDateString('pt-PT', { day: '2-digit', month: '2-digit' });
    }

    button.addEventListener('click', (event) => {
      event.stopPropagation();
      menu.hidden ? open() : close({ restoreFocus: true });
    });

    document.addEventListener('click', (event) => {
      if (!menu.hidden && !wrap.contains(event.target)) close();
    });

    document.addEventListener('keydown', (event) => {
      if (event.key === 'Escape' && !menu.hidden) close({ restoreFocus: true });
    });

    wrap.addEventListener('focusout', () => {
      window.setTimeout(() => {
        if (!wrap.contains(document.activeElement)) close();
      }, 0);
    });

    window.ocCloseSino = () => close({ restoreFocus: true });
  }

  /* ── Menu "+ Criar" ───────────────────────────────────────────────── */

  function initCreateMenu() {
    const wrap = $('[data-oc="create"]');
    if (!wrap) return;

    const button = $('[data-oc="create-toggle"]', wrap);
    const menu = $('[data-oc="create-menu"]', wrap);
    if (!button || !menu) return;

    const close = () => {
      menu.hidden = true;
      button.setAttribute('aria-expanded', 'false');
    };
    const open = () => {
      menu.hidden = false;
      button.setAttribute('aria-expanded', 'true');
      const first = $('.oc-create__item', menu);
      if (first) first.focus();
    };

    button.addEventListener('click', (event) => {
      event.stopPropagation();
      menu.hidden ? open() : close();
    });

    document.addEventListener('click', (event) => {
      if (!menu.hidden && !wrap.contains(event.target)) close();
    });

    menu.addEventListener('keydown', (event) => {
      if (event.key !== 'ArrowDown' && event.key !== 'ArrowUp') return;
      event.preventDefault();
      const items = $$('.oc-create__item', menu);
      const at = items.indexOf(document.activeElement);
      const next = event.key === 'ArrowDown' ? at + 1 : at - 1;
      const target = items[(next + items.length) % items.length];
      if (target) target.focus();
    });

    window.ocCloseCreate = close;
  }

  /* ── Command palette ──────────────────────────────────────────────── */

  function initPalette() {
    const palette = $('[data-oc="palette"]');
    if (!palette) return;

    /* Seleccionado pelo seu `data-oc` e não por ser o único `input` dentro da
     * palette: a ligação fica explícita e greppável, e um teste consegue
     * distinguir um campo ligado de um campo órfão. */
    const input = $('[data-oc="palette-input"]', palette);
    const items = $$('.oc-palette__item', palette);
    let restoreFocusTo = null;

    const visible = () => items.filter((item) => !item.hidden);

    const highlight = (item) => {
      items.forEach((other) => other.removeAttribute('data-active'));
      if (item) {
        item.setAttribute('data-active', '');
        item.scrollIntoView({ block: 'nearest' });
      }
    };

    const open = () => {
      restoreFocusTo = document.activeElement;
      palette.hidden = false;
      if (input) { input.value = ''; input.focus(); }
      items.forEach((item) => { item.hidden = false; });
      $$('.oc-palette__group', palette).forEach((g) => { g.hidden = false; });
      highlight(visible()[0]);
    };

    const close = () => {
      palette.hidden = true;
      // Devolver o foco a quem abriu: sem isto, quem navega por teclado fica
      // perdido no topo do documento.
      if (restoreFocusTo && restoreFocusTo.focus) restoreFocusTo.focus();
      restoreFocusTo = null;
    };

    const filter = (query) => {
      const needle = query.trim().toLowerCase();
      items.forEach((item) => {
        item.hidden = needle !== '' && !item.dataset.label.toLowerCase().includes(needle);
      });
      // Um grupo sem itens visíveis desaparece, em vez de ficar um título só.
      $$('[data-oc="palette-group"]', palette).forEach((group) => {
        const label = $('.oc-palette__group', group);
        const any = $$('.oc-palette__item', group).some((item) => !item.hidden);
        if (label) label.hidden = !any;
      });
      highlight(visible()[0]);
    };

    if (input) input.addEventListener('input', () => filter(input.value));

    palette.addEventListener('mousedown', (event) => {
      if (event.target === palette) close();
    });

    palette.addEventListener('keydown', (event) => {
      const shown = visible();
      const at = shown.indexOf($('[data-active]', palette));

      if (event.key === 'ArrowDown') {
        event.preventDefault();
        highlight(shown[Math.min(at + 1, shown.length - 1)]);
      } else if (event.key === 'ArrowUp') {
        event.preventDefault();
        highlight(shown[Math.max(at - 1, 0)]);
      } else if (event.key === 'Enter') {
        const active = $('[data-active]', palette);
        if (active) { event.preventDefault(); active.click(); }
      }
    });

    $$('[data-oc="palette-open"]').forEach((trigger) => {
      trigger.addEventListener('click', open);
    });

    window.addEventListener('keydown', (event) => {
      const key = event.key.toLowerCase();
      if ((event.metaKey || event.ctrlKey) && key === 'k') {
        event.preventDefault();
        palette.hidden ? open() : close();
      }
      if (event.key === 'Escape') {
        if (!palette.hidden) close();
        if (window.ocCloseCreate) window.ocCloseCreate();
        if (window.ocCloseAccount) window.ocCloseAccount();
      }
    });
  }

  /* ── Tabs locais ──────────────────────────────────────────────────── */

  /*
   * Tabs cujo conteúdo já veio do servidor. As que precisam de dados
   * diferentes navegam para um URL próprio e não passam por aqui, para que
   * continuem a funcionar sem JavaScript.
   */
  function initLocalTabs() {
    $$('[data-oc="tabs"]').forEach((group) => {
      const tabs = $$('[role="tab"]', group);

      const select = (tab) => {
        tabs.forEach((other) => {
          const on = other === tab;
          other.setAttribute('aria-selected', String(on));
          other.tabIndex = on ? 0 : -1;
          const panel = document.getElementById(other.getAttribute('aria-controls') || '');
          if (panel) panel.hidden = !on;
        });
      };

      tabs.forEach((tab) => {
        if (!tab.getAttribute('aria-controls')) return;
        tab.addEventListener('click', (event) => { event.preventDefault(); select(tab); });
      });

      group.addEventListener('keydown', (event) => {
        if (event.key !== 'ArrowRight' && event.key !== 'ArrowLeft') return;
        const at = tabs.indexOf(document.activeElement);
        if (at < 0) return;
        event.preventDefault();
        const step = event.key === 'ArrowRight' ? 1 : -1;
        const target = tabs[(at + step + tabs.length) % tabs.length];
        target.focus();
        if (target.getAttribute('aria-controls')) select(target);
      });
    });
  }

  /* ── Densidade das tabelas ────────────────────────────────────────── */

  function initDensity() {
    const apply = (dense) => {
      $$('.oc-table').forEach((table) => { table.dataset.dense = String(dense); });
      $$('[data-oc="density"]').forEach((button) => {
        button.setAttribute('aria-pressed', String(dense));
      });
    };

    apply(readPrefs().denseRows === true);

    $$('[data-oc="density"]').forEach((button) => {
      button.addEventListener('click', () => {
        const dense = button.getAttribute('aria-pressed') !== 'true';
        apply(dense);
        writePref('denseRows', dense);
      });
    });
  }

  /* ── Linhas de tabela navegáveis ──────────────────────────────────── */

  function initRowLinks() {
    $$('[data-oc-href]').forEach((row) => {
      row.addEventListener('click', () => { window.location.href = row.dataset.ocHref; });
    });
  }

  /* ── Relógio ──────────────────────────────────────────────────────── */

  /*
   * A hora do computador de quem está a ver, como num ambiente de trabalho.
   *
   * Não vem do Core, não precisa de API e não é persistida — e **nunca decide
   * nada**. Carimbos de auditoria, expiração de sessões e prazos continuam a
   * vir do Core e da base de dados: a hora do browser é escolhida por quem o
   * usa, e usá-la para autorização seria deixar decidir quem mexe no relógio.
   *
   * Dois formatos, porque há dois sítios: o login mostra uma linha, e a topbar
   * empilha a hora sobre a data para não crescer em altura.
   */
  function initClock() {
    const clocks = $$('[data-oc="clock"]');
    if (!clocks.length) return;

    const tick = () => {
      const now = new Date();
      const date = now.toLocaleDateString('pt-PT', {
        weekday: 'short', day: '2-digit', month: 'short',
      });
      const time = now.toLocaleTimeString('pt-PT', { hour: '2-digit', minute: '2-digit' });

      clocks.forEach((clock) => {
        const hora = $('b', clock);
        const dia = $('span', clock);

        if (hora && dia) {
          // A topbar: duas linhas.
          hora.textContent = time;
          dia.textContent = date.replace(/\.$/, '');
          clock.hidden = false;
        } else {
          // O login: uma linha.
          clock.textContent = `${date.toUpperCase()} · ${time}`;
        }

        if (clock.tagName === 'TIME') {
          // Só a data, sem hora: um `datetime` com minutos ficaria errado no
          // minuto seguinte, e ninguém o reescreve entre ticks.
          clock.dateTime = now.toISOString().slice(0, 10);
          clock.title = now.toLocaleDateString('pt-PT', {
            weekday: 'long', day: 'numeric', month: 'long', year: 'numeric',
          });
        }
      });

      // O cabeçalho do Centro Temporal, quando está montado. A zona é a que o
      // browser diz ter — apresentação, não autoridade: o Core continua a
      // decidir o que «14:00 em Paris» significa.
      const painelData = $('[data-oc="temporal-date"]');
      const painelHora = $('[data-oc="temporal-clock"]');
      const painelZona = $('[data-oc="temporal-zone"]');
      if (painelData) {
        painelData.textContent = now.toLocaleDateString('pt-PT', {
          weekday: 'long', day: 'numeric', month: 'long',
        });
      }
      if (painelHora) painelHora.textContent = time;
      if (painelZona) {
        painelZona.textContent =
          Intl.DateTimeFormat().resolvedOptions().timeZone || 'UTC';
      }
    };

    tick();
    // Uma vez por minuto. Segundos dariam movimento permanente sem utilidade
    // nenhuma num relógio que existe para se saber as horas de relance.
    setInterval(tick, 30_000);
  }

  /**
   * O Centro Temporal.
   *
   * O relógio é um `button` a sério, e este é o painel que ele abre. Escape
   * fecha, um clique fora fecha, e o foco volta ao relógio — porque quem abriu
   * com o teclado não deve ficar perdido no fim da página.
   *
   * O conteúdo vem do servidor, já autorizado. Este ficheiro não decide o que se
   * pode ver: só mostra e esconde.
   */
  function initTemporalCentre() {
    const trigger = $('[data-oc="clock"]');
    const panel = document.getElementById('oc-temporal-centre');
    if (!trigger || !panel || trigger.tagName !== 'BUTTON') return;

    const close = ({ restoreFocus = false } = {}) => {
      if (trigger.getAttribute('aria-expanded') !== 'true') return;
      panel.hidden = true;
      trigger.setAttribute('aria-expanded', 'false');
      if (restoreFocus) trigger.focus();
    };

    const open = () => {
      panel.hidden = false;
      trigger.setAttribute('aria-expanded', 'true');
      // O primeiro elemento focável do painel, para quem chegou por teclado.
      const primeiro = panel.querySelector('a, button');
      if (primeiro) primeiro.focus();
    };

    panel.hidden = true;

    trigger.addEventListener('click', (event) => {
      event.stopPropagation();
      if (trigger.getAttribute('aria-expanded') === 'true') {
        close({ restoreFocus: true });
      } else {
        open();
      }
    });

    document.addEventListener('keydown', (event) => {
      if (event.key === 'Escape') close({ restoreFocus: true });
    });

    document.addEventListener('click', (event) => {
      if (!panel.contains(event.target) && event.target !== trigger) close();
    });
  }


  /* ── A disposição do Correio ──────────────────────────────────────────

     Quem manda no espaço de trabalho é quem está a trabalhar. O sistema
     protege a usabilidade: nunca deixa um painel chegar a um estado onde
     deixa de servir, e nunca perde o caminho de volta.

     # Porque proporções e não pixéis

     Porque uma preferência guardada em pixéis quebra noutro ecrã. Setecentos
     pixéis de lista são metade de um portátil e um quinto de um monitor
     grande. Guarda-se a **fracção** da largura, e ao ler volta-se a
     limitá-la aos mínimos deste ecrã.

     # Porque isto vive aqui e não no Core

     Porque a largura de um painel não é um facto institucional. O Core não
     sabe — nem deve saber — como é que alguém arrumou as colunas para ler. */
  function initMail() {
    const mail = document.querySelector('[data-oc="mail"]');
    if (!mail) return;

    const CHAVE = 'oc-mail-disposicao';

    /* Mínimos em pixéis: abaixo disto um painel deixa de mostrar o que existe
       para mostrar. As pastas precisam de caber o ícone, «Caixa de entrada» e
       a contagem — 168 é onde o rótulo mais longo ainda cabe inteiro, medido e
       não estimado. */
    const MINIMOS = { pastas: 168, lista: 240, leitura: 380 };

    const larguraTotal = () => mail.getBoundingClientRect().width;

    /* O que se pode dar a um painel sem tirar aos outros o que eles precisam. */
    function limitar(qual, pedido) {
      const total = larguraTotal();

      /* Não conseguir medir não é licença para não limitar.

         Estava `if (!total) return pedido` — e o pedido cru era aplicado. O
         mínimo de cada painel é uma constante e não precisa de medida nenhuma:
         desistir dele porque a **outra** metade do cálculo não é possível
         deixa a coluna abaixo do que a torna utilizável, e lá fica, porque
         `normalizar` também desiste sem total.

         Uma largura de zero acontece: o contentor ainda não foi disposto, a
         página está escondida, o browser está com trabalho a mais. É
         exactamente quando um guarda tem de continuar a valer. */
      if (!total) return Math.max(MINIMOS[qual], pedido);
      const outro = qual === 'pastas' ? lidas().lista : lidas().pastas;
      const recolhidas = mail.dataset.ocPastas === 'recolhido';
      const ocupado = qual === 'pastas' ? outro : (recolhidas ? 0 : outro);
      const maximo = total - ocupado - MINIMOS.leitura - 24;
      return Math.max(MINIMOS[qual], Math.min(pedido, Math.max(MINIMOS[qual], maximo)));
    }

    function lidas() {
      const estilo = getComputedStyle(mail);
      return {
        pastas: parseFloat(estilo.getPropertyValue('--oc-mail-pastas')) || 208,
        lista: parseFloat(estilo.getPropertyValue('--oc-mail-lista')) || 330,
      };
    }

    function aplicar(qual, pixeis) {
      mail.style.setProperty('--oc-mail-' + qual, limitar(qual, pixeis) + 'px');
      normalizar();
    }

    /* A garantia final: a leitura tem sempre onde acontecer.

       `limitar` protege cada painel no momento em que ele muda, e isso não
       chega. Duas larguras aceitáveis uma a uma podem ser inaceitáveis juntas
       — e é o que acontece quando uma preferência guardada num monitor grande
       é reposta num portátil: cada uma cabe, as duas não, e o painel de
       leitura fica com cinquenta pixéis.

       Aqui olha-se para o conjunto. Quem encolhe é a lista, porque é a que
       tem mais para dar; se ainda não chegar, encolhem as pastas. */
    function normalizar() {
      const total = larguraTotal();

      /* Sem total não há excesso a corrigir — mas há mínimos a manter, e esses
         não dependem de medir nada. É a segunda metade da mesma lição. */
      if (!total) {
        const sem_medida = lidas();
        if (sem_medida.lista < MINIMOS.lista) {
          mail.style.setProperty('--oc-mail-lista', MINIMOS.lista + 'px');
        }
        if (sem_medida.pastas < MINIMOS.pastas) {
          mail.style.setProperty('--oc-mail-pastas', MINIMOS.pastas + 'px');
        }
        return;
      }

      const recolhidas = mail.dataset.ocPastas === 'recolhido';
      const actual = lidas();
      const pastas = recolhidas ? 0 : actual.pastas;
      let lista = actual.lista;

      const disponivel = total - MINIMOS.leitura - 24;
      if (pastas + lista <= disponivel) return;

      /* Encolher, e só encolher.
         Estava `Math.max(MINIMOS.lista, disponivel - pastas)` — sem o `min`,
         a garantia **crescia** a lista sempre que sobrava espaço, e recolher
         as pastas dava o espaço à lista em vez de o dar a quem lê. Uma
         garantia que redistribui é outra coisa: aqui só se corrige o
         excesso. */
      lista = Math.max(MINIMOS.lista, Math.min(lista, disponivel - pastas));
      mail.style.setProperty('--oc-mail-lista', lista + 'px');

      if (!recolhidas && pastas + lista > disponivel) {
        const cabem = Math.max(MINIMOS.pastas, disponivel - lista);
        mail.style.setProperty('--oc-mail-pastas', cabem + 'px');
      }
    }

    /* ── A preferência ──────────────────────────────────────────────── */

    function guardar() {
      const total = larguraTotal();
      if (!total) return;
      const actual = lidas();
      try {
        window.localStorage.setItem(CHAVE, JSON.stringify({
          pastas: actual.pastas / total,
          lista: actual.lista / total,
          recolhidas: mail.dataset.ocPastas === 'recolhido',
        }));
      } catch (erro) {
        /* Sem armazenamento — janela privada, política do browser — a
           aplicação continua. Uma preferência que não se guarda é uma
           conveniência perdida, não uma avaria. */
      }
    }

    function repor() {
      let guardada = null;
      try {
        guardada = JSON.parse(window.localStorage.getItem(CHAVE) || 'null');
      } catch (erro) {
        guardada = null;
      }
      if (!guardada) return;

      const total = larguraTotal();
      if (!total) return;

      /* Limitado ao ler, e não só ao escrever: a preferência veio de outro
         ecrã, e o que era confortável lá pode aqui não caber. */
      if (typeof guardada.pastas === 'number') aplicar('pastas', guardada.pastas * total);
      if (typeof guardada.lista === 'number') aplicar('lista', guardada.lista * total);
      if (guardada.recolhidas) alternarPastas(true);
      normalizar();
    }

    /* ── Recolher as pastas ─────────────────────────────────────────── */

    const botaoPastas = document.querySelector('[data-oc="alternar-pastas"]');

    function alternarPastas(recolher) {
      const alvo = typeof recolher === 'boolean'
        ? recolher
        : mail.dataset.ocPastas !== 'recolhido';
      if (alvo) mail.dataset.ocPastas = 'recolhido';
      else delete mail.dataset.ocPastas;
      if (botaoPastas) {
        botaoPastas.setAttribute('aria-pressed', String(alvo));
        botaoPastas.title = alvo ? 'Mostrar as pastas' : 'Recolher as pastas';
      }
    }

    if (botaoPastas) {
      botaoPastas.addEventListener('click', () => {
        alternarPastas();
        guardar();
      });
    }

    /* ── Dar o ecrã à leitura ───────────────────────────────────────── */

    const botaoLeitura = document.querySelector('[data-oc="focar-leitura"]');
    let anterior = null;

    if (botaoLeitura) {
      botaoLeitura.addEventListener('click', () => {
        const activo = botaoLeitura.getAttribute('aria-pressed') === 'true';

        if (activo) {
          /* Repor **o que estava**, e não um valor bonito: quem arrumou as
             colunas de uma maneira não quer que voltar atrás as arrume de
             outra. */
          if (anterior) {
            mail.style.setProperty('--oc-mail-pastas', anterior.pastas + 'px');
            mail.style.setProperty('--oc-mail-lista', anterior.lista + 'px');
            alternarPastas(anterior.recolhidas);
          }
          botaoLeitura.setAttribute('aria-pressed', 'false');
          botaoLeitura.title = 'Dar o ecrã à leitura';
        } else {
          const actual = lidas();
          anterior = {
            pastas: actual.pastas,
            lista: actual.lista,
            recolhidas: mail.dataset.ocPastas === 'recolhido',
          };
          alternarPastas(true);
          aplicar('lista', MINIMOS.lista);
          botaoLeitura.setAttribute('aria-pressed', 'true');
          botaoLeitura.title = 'Repor a disposição';
        }
        guardar();
      });
    }

    /* ── Arrastar ───────────────────────────────────────────────────── */

    mail.querySelectorAll('[data-oc="separador"]').forEach((separador) => {
      const qual = separador.dataset.ocSeparador;

      separador.addEventListener('pointerdown', (evento) => {
        evento.preventDefault();
        separador.setPointerCapture(evento.pointerId);
        separador.dataset.ocActivo = 'true';
        mail.dataset.ocArrastar = 'true';

        const inicio = evento.clientX;
        const partida = lidas()[qual];

        function mover(e) {
          aplicar(qual, partida + (e.clientX - inicio));
        }

        function largar() {
          separador.removeEventListener('pointermove', mover);
          separador.removeEventListener('pointerup', largar);
          delete separador.dataset.ocActivo;
          delete mail.dataset.ocArrastar;
          guardar();
        }

        separador.addEventListener('pointermove', mover);
        separador.addEventListener('pointerup', largar);
      });

      /* As setas movem o separador. Redimensionar só com o rato exclui quem
         não usa rato — e um `role="separator"` que não responde ao teclado
         promete uma operação que não entrega. */
      separador.addEventListener('keydown', (evento) => {
        const passo = evento.shiftKey ? 48 : 12;
        if (evento.key === 'ArrowLeft') aplicar(qual, lidas()[qual] - passo);
        else if (evento.key === 'ArrowRight') aplicar(qual, lidas()[qual] + passo);
        else return;
        evento.preventDefault();
        guardar();
      });
    });

    /* Estreitar a janela é o mesmo problema que repor noutra janela: o que
       cabia deixa de caber. A garantia volta a correr. */
    window.addEventListener('resize', normalizar);

    /* Repor **depois** de tudo estar ligado: repor antes deixaria a
       preferência escrita e os controlos por sincronizar. */
    repor();
  }

  /* Repor a disposição do Correio, a partir das Definições.
     Vive fora de `initMail` porque o ecrã das definições não tem os painéis:
     `initMail` sai logo à entrada quando não os encontra, e um botão preso lá
     dentro nunca chegaria a ser ligado. */
  function initReporDisposicao() {
    const botao = document.querySelector('[data-oc="repor-disposicao"]');
    if (!botao) return;

    botao.addEventListener('click', () => {
      try {
        window.localStorage.removeItem('oc-mail-disposicao');
      } catch (erro) {
        /* Sem armazenamento não há nada guardado para repor. */
      }
      const aviso = document.querySelector('[data-oc="disposicao-reposta"]');
      if (aviso) aviso.removeAttribute('hidden');
      botao.disabled = true;
    });
  }


  /* ── O compositor ─────────────────────────────────────────────────────

     Mover, redimensionar, expandir, e as fichas de destinatário.

     # O rascunho nunca se perde

     Nada aqui volta a desenhar o formulário. Mover e redimensionar mexem em
     variáveis CSS; expandir mexe num atributo. O `textarea` e os campos são
     os mesmos elementos do princípio ao fim, e por isso o que lá está
     continua lá — não por cuidado de cada gesto, mas porque não há nenhum
     gesto que os substitua.

     # As fichas são uma camada

     O campo de texto continua a ser quem submete. O script lê-o, desenha as
     fichas, e volta a escrevê-lo a cada alteração. Sem script, fica um campo
     com endereços separados por vírgula — feio, e a funcionar. */
  function initCompositor() {
    const janela = document.querySelector('[data-oc="compositor"]');
    if (!janela) return;

    /* ── Mover ────────────────────────────────────────────────────────── */

    const pega = janela.querySelector('[data-oc="compositor-pega"]');
    if (pega) {
      pega.addEventListener('pointerdown', (evento) => {
        if (evento.target.closest('button, a')) return;
        evento.preventDefault();
        pega.setPointerCapture(evento.pointerId);

        const caixa = janela.getBoundingClientRect();
        const dx = evento.clientX;
        const dy = evento.clientY;
        const direita0 = window.innerWidth - caixa.right;
        const fundo0 = window.innerHeight - caixa.bottom;

        function mover(e) {
          /* Mover, medir, e corrigir se saiu.

             Duas tentativas de calcular o limite antes de mexer falharam, e
             pela mesma razão: dependiam de qual largura é a autoritativa —
             `innerWidth`, a do elemento, a do viewport emulado — e sob
             emulação elas não coincidem. Uma janela arrastada para fora fica
             inalcançável e leva o rascunho com ela, e isso é grave de mais
             para depender de acertar na medida certa.

             Aqui não se prevê: aplica-se o movimento, pergunta-se ao browser
             onde a janela ficou, e puxa-se de volta o que passou. O browser é
             a autoridade sobre a sua própria geometria. */
          janela.style.setProperty(
            '--oc-comp-direita',
            Math.max(0, direita0 - (e.clientX - dx)) + 'px',
          );
          janela.style.setProperty(
            '--oc-comp-fundo',
            Math.max(0, fundo0 - (e.clientY - dy)) + 'px',
          );

          const onde = janela.getBoundingClientRect();
          if (onde.left < 0) {
            const direita = parseFloat(
              getComputedStyle(janela).getPropertyValue('--oc-comp-direita'),
            ) || 0;
            janela.style.setProperty('--oc-comp-direita', Math.max(0, direita + onde.left) + 'px');
          }
          if (onde.top < 0) {
            const fundo = parseFloat(
              getComputedStyle(janela).getPropertyValue('--oc-comp-fundo'),
            ) || 0;
            janela.style.setProperty('--oc-comp-fundo', Math.max(0, fundo + onde.top) + 'px');
          }
        }

        function largar() {
          pega.removeEventListener('pointermove', mover);
          pega.removeEventListener('pointerup', largar);
        }

        pega.addEventListener('pointermove', mover);
        pega.addEventListener('pointerup', largar);
      });
    }

    /* ── Redimensionar ────────────────────────────────────────────────── */

    const MIN_LARGURA = 380;
    const MIN_ALTURA = 320;
    const puxador = janela.querySelector('[data-oc="compositor-puxador"]');

    if (puxador) {
      puxador.addEventListener('pointerdown', (evento) => {
        evento.preventDefault();
        puxador.setPointerCapture(evento.pointerId);

        const caixa = janela.getBoundingClientRect();
        const dx = evento.clientX;
        const dy = evento.clientY;

        function mover(e) {
          /* O puxador está no canto superior esquerdo: arrastar para a
             esquerda alarga, arrastar para cima aumenta a altura. */
          const largura = Math.min(
            Math.max(MIN_LARGURA, caixa.width - (e.clientX - dx)),
            window.innerWidth - 32,
          );
          const altura = Math.min(
            Math.max(MIN_ALTURA, caixa.height - (e.clientY - dy)),
            window.innerHeight - 60,
          );
          janela.style.setProperty('--oc-comp-largura', largura + 'px');
          janela.style.setProperty('--oc-comp-altura', altura + 'px');
        }

        function largar() {
          puxador.removeEventListener('pointermove', mover);
          puxador.removeEventListener('pointerup', largar);
        }

        puxador.addEventListener('pointermove', mover);
        puxador.addEventListener('pointerup', largar);
      });
    }

    /* ── Expandir ─────────────────────────────────────────────────────── */

    const expandir = janela.querySelector('[data-oc="compositor-expandir"]');
    if (expandir) {
      expandir.addEventListener('click', () => {
        const activo = janela.dataset.ocExpandido === 'true';
        if (activo) delete janela.dataset.ocExpandido;
        else janela.dataset.ocExpandido = 'true';
        expandir.setAttribute('aria-pressed', String(!activo));
        expandir.title = activo ? 'Expandir' : 'Repor o tamanho';
      });
    }

    /* ── Cc ───────────────────────────────────────────────────────────── */

    const mostrarCc = janela.querySelector('[data-oc="mostrar-cc"]');
    const linhaCc = janela.querySelector('[data-oc="linha-cc"]');
    if (mostrarCc && linhaCc) {
      mostrarCc.addEventListener('click', () => {
        linhaCc.removeAttribute('hidden');
        mostrarCc.setAttribute('hidden', '');
        const entrada = linhaCc.querySelector('[data-oc="destino-entrada"]');
        if (entrada) entrada.focus();
      });
    }

    /* ── Fichas e sugestões ───────────────────────────────────────────── */

    janela.querySelectorAll('[data-oc="destinatarios"]').forEach((linha) => {
      const entrada = linha.querySelector('[data-oc="destino-entrada"]');
      const fichas = linha.querySelector('[data-oc="fichas"]');
      const sugestoes = linha.querySelector('[data-oc="sugestoes"]');
      if (!entrada || !fichas || !sugestoes) return;

      /* Os endereços aceites. O campo de texto continua a ser a verdade que
         se submete; isto é a leitura dele. */
      let aceites = entrada.value
        .split(',')
        .map((parte) => parte.trim())
        .filter(Boolean);

      /* Um campo escondido leva o valor, e o visível fica para escrever.
         Sem isto, escrever «jes» deixava «jes» no campo submetido. */
      const submetido = document.createElement('input');
      submetido.type = 'hidden';
      submetido.name = entrada.name;
      entrada.removeAttribute('name');
      entrada.value = '';
      entrada.parentNode.appendChild(submetido);

      function sincronizar() {
        submetido.value = aceites.join(', ');
        fichas.textContent = '';
        aceites.forEach((endereco, indice) => {
          const ficha = document.createElement('span');
          ficha.className = 'oc-chip';
          const texto = document.createElement('span');
          /* `textContent`: um endereço vem de fora e não é marcação. */
          texto.textContent = endereco;
          const tirar = document.createElement('button');
          tirar.type = 'button';
          tirar.setAttribute('aria-label', 'Retirar ' + endereco);
          tirar.textContent = '×';
          tirar.addEventListener('click', () => {
            aceites.splice(indice, 1);
            sincronizar();
            entrada.focus();
          });
          ficha.appendChild(texto);
          ficha.appendChild(tirar);
          fichas.appendChild(ficha);
        });
        if (aceites.length) fichas.removeAttribute('hidden');
        else fichas.setAttribute('hidden', '');
      }

      function aceitar(endereco) {
        const limpo = (endereco || '').trim().replace(/,$/, '');
        if (!limpo) return;
        if (!aceites.includes(limpo)) aceites.push(limpo);
        entrada.value = '';
        esconderSugestoes();
        sincronizar();
      }

      function esconderSugestoes() {
        sugestoes.setAttribute('hidden', '');
        sugestoes.textContent = '';
      }

      function desenhar(pessoas) {
        sugestoes.textContent = '';
        if (!pessoas.length) {
          esconderSugestoes();
          return;
        }
        pessoas.forEach((pessoa) => {
          const item = document.createElement('li');
          const botao = document.createElement('button');
          botao.type = 'button';
          botao.className = 'oc-sugestao';
          const nome = document.createElement('b');
          nome.textContent = pessoa.name || pessoa.email || '';
          const endereco = document.createElement('em');
          endereco.textContent = pessoa.email || '';
          botao.appendChild(nome);
          botao.appendChild(endereco);
          botao.addEventListener('click', () => aceitar(pessoa.email));
          item.appendChild(botao);
          sugestoes.appendChild(item);
        });
        sugestoes.removeAttribute('hidden');
      }

      let pedido = null;
      entrada.addEventListener('input', () => {
        const termo = entrada.value.trim();
        if (termo.length < 2) {
          esconderSugestoes();
          return;
        }
        clearTimeout(pedido);
        pedido = setTimeout(() => {
          fetch('/mail/people?q=' + encodeURIComponent(termo))
            .then((resposta) => resposta.json())
            .then((dados) => desenhar((dados && dados.people) || []))
            .catch(() => esconderSugestoes());
        }, 140);
      });

      entrada.addEventListener('keydown', (evento) => {
        if (evento.key === 'Enter' || evento.key === ',') {
          /* Enter aceita o destinatário; **nunca** envia a mensagem. Enviar
             por engano ao confirmar um nome é o erro que não se desfaz. */
          evento.preventDefault();
          const primeira = sugestoes.querySelector('.oc-sugestao em');
          if (!sugestoes.hasAttribute('hidden') && primeira) {
            aceitar(primeira.textContent);
          } else {
            aceitar(entrada.value);
          }
        } else if (evento.key === 'Backspace' && !entrada.value && aceites.length) {
          aceites.pop();
          sincronizar();
        } else if (evento.key === 'Escape') {
          esconderSugestoes();
        }
      });

      /* Sair do campo aceita o que lá estiver: quem escreveu um endereço e
         carregou em Enviar espera que ele conte. */
      entrada.addEventListener('blur', () => {
        setTimeout(() => {
          if (entrada.value.trim()) aceitar(entrada.value);
          esconderSugestoes();
        }, 160);
      });

      sincronizar();
    });

    /* ── Enviar uma vez ───────────────────────────────────────────────── */

    const forma = janela.querySelector('form');
    const enviar = janela.querySelector('[data-oc="compositor-enviar"]');
    if (forma && enviar) {
      forma.addEventListener('submit', (evento) => {
        /* A assistência submete o mesmo formulário para outra rota, e não é
           um envio: bloqueá-la seria bloquear o botão errado. */
        if (evento.submitter && evento.submitter !== enviar) return;
        if (forma.dataset.ocEnviando === 'true') {
          evento.preventDefault();
          return;
        }
        forma.dataset.ocEnviando = 'true';
        enviar.disabled = true;
        enviar.textContent = 'A enviar…';
      });
    }
  }

  /* ── Arranque ─────────────────────────────────────────────────────── */

  const start = () => {
    initSidebar();
    initSino();
    initCreateMenu();
    initAccountMenu();
    initPalette();
    initLocalTabs();
    initTemporalCentre();
    initDensity();
    initRowLinks();
    initClock();
    initMail();
    initReporDisposicao();
    initCompositor();
  };

  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', start);
  } else {
    start();
  }
})();

/* Alternador «Mostrar palavra-passe» num campo de entrada.
 *
 * Existe porque um gestor de palavras-passe escreve numa caixa que o membro não
 * consegue ler, e uma tentativa falhada conta para o throttle. */
document.addEventListener('click', (event) => {
  const toggle = event.target.closest('[data-oc="reveal"]');
  if (!toggle) return;
  const field = document.getElementById(toggle.dataset.ocTarget);
  if (!field) return;
  const shown = field.type === 'text';
  field.type = shown ? 'password' : 'text';
  toggle.textContent = shown ? 'Mostrar' : 'Ocultar';
  toggle.setAttribute('aria-pressed', String(!shown));
});

/* Credencial temporária: revelar e copiar.
 *
 * Coberta por omissão — um ecrã partilhado não deve revelá-la sozinho. O valor
 * vive num atributo desta página e em mais lado nenhum: não há endpoint que o
 * leia de volta. */
document.addEventListener('click', (event) => {
  const holder = event.target.closest('.oc-credential__value');
  if (!holder) return;
  const secret = holder.querySelector('[data-oc="secret"]');
  if (!secret) return;
  const value = secret.dataset.ocValue || '';

  if (event.target.closest('[data-oc="secret-toggle"]')) {
    const button = event.target.closest('[data-oc="secret-toggle"]');
    const shown = button.getAttribute('aria-pressed') === 'true';
    secret.textContent = shown ? '•'.repeat(value.length) : value;
    button.textContent = shown ? 'Mostrar' : 'Ocultar';
    button.setAttribute('aria-pressed', String(!shown));
    return;
  }

  if (event.target.closest('[data-oc="secret-copy"]') && navigator.clipboard) {
    const button = event.target.closest('[data-oc="secret-copy"]');
    navigator.clipboard.writeText(value).then(() => {
      button.textContent = 'Copiado';
      setTimeout(() => { button.textContent = 'Copiar'; }, 2000);
    });
  }
});

/* Filtro local das linhas de uma tabela.
 *
 * Filtra o que já está renderizado; não fala com o Core. É deliberado: o
 * Workspace pede uma página de resultados e este campo ajuda a encontrar dentro
 * dela. Quando a lista exceder uma página, o filtro passa a ter de viajar até
 * ao Core — e nessa altura este handler é substituído, não estendido. */
document.addEventListener('input', (event) => {
  const field = event.target.closest('[data-oc="table-filter"]');
  if (!field) return;

  const table = field.closest('.oc-table');
  if (!table) return;

  const needle = field.value.trim().toLowerCase();
  const rows = table.querySelectorAll('.oc-table__row');
  let shown = 0;

  rows.forEach((row) => {
    const match = !needle || row.textContent.toLowerCase().includes(needle);
    row.hidden = !match;
    if (match) shown += 1;
  });

  /* O rodapé passa a contar o que está visível, para que a contagem nunca
   * contradiga o que se vê. */
  const count = table.querySelector('.oc-table__count');
  if (count) {
    if (!count.dataset.ocTotal) count.dataset.ocTotal = count.textContent;
    count.textContent = needle
      ? `${shown} de ${rows.length} visíveis`
      : count.dataset.ocTotal;
  }
});

/* Atalhos das acções da command palette.
 *
 * A palette mostrava `⌘⇧I`, `⌘⇧A`, `⌘⇧P` e `⌘⇧C` ao lado de cada acção e
 * nenhum deles fazia coisa nenhuma: era uma promessa ao membro que o teclado
 * não cumpria. Só `⌘K` estava ligado.
 *
 * As ligações vêm das linhas renderizadas, e não de uma lista repetida aqui.
 * O servidor já filtra as acções pela permissão de quem está a ver, portanto
 * um atalho para algo que a pessoa não pode fazer não chega sequer a existir —
 * e a lista nunca fica dessincronizada da interface. */
document.addEventListener('keydown', (event) => {
  if (!(event.metaKey || event.ctrlKey) || !event.shiftKey) return;

  const letra = event.key.toLowerCase();
  const alvo = Array.from(
    document.querySelectorAll('[data-shortcut]'),
  ).find((el) => {
    const atalho = el.dataset.shortcut || '';
    return atalho.slice(-1).toLowerCase() === letra;
  });

  if (!alvo) return;
  event.preventDefault();
  window.location.assign(alvo.getAttribute('href'));
});

/* A entrega do arranque não deve deixar rasto no histórico.
 *
 * O arranque segue para o destino com `<meta http-equiv="refresh">`, e isso é
 * deliberado: tem de funcionar antes de este ficheiro existir. Mas um meta
 * refresh com atraso **acrescenta** uma entrada ao histórico em vez de a
 * substituir, e o `/boot` fica na pilha. Retroceder aterrava no arranque, que
 * reentregava — a pessoa carregava em «voltar» e via um ecrã que não pediu.
 *
 * Onde há JavaScript, a mesma entrega faz-se com `location.replace`, que ocupa
 * a entrada actual em vez de criar outra: o `/boot` desaparece da pilha e
 * retroceder devolve a pessoa a onde ela estava mesmo.
 *
 * O destino é lido do próprio meta, e não escrito outra vez aqui. Já foi
 * validado contra o catálogo de rotas no servidor e já foi escapado uma vez
 * para caber no atributo; passá-lo por um segundo contexto de escape seria
 * criar uma superfície nova para nada.
 *
 * O meta é retirado antes do temporizador para os dois não correrem à vez. Se
 * este ficheiro não chegar, o meta continua lá e o arranque entrega na mesma. */
(function () {
  var meta = document.querySelector('meta[http-equiv="refresh"]');
  if (!meta) return;

  var conteudo = meta.getAttribute('content') || '';
  var marca = conteudo.indexOf('url=');
  if (marca < 0) return;

  var destino = conteudo.slice(marca + 4);
  if (!destino || destino.charAt(0) !== '/' || destino.charAt(1) === '/') return;

  var atraso = parseFloat(conteudo) * 1000;
  if (!isFinite(atraso) || atraso < 0) atraso = 0;

  meta.parentNode.removeChild(meta);
  window.setTimeout(function () {
    window.location.replace(destino);
  }, atraso);
})();

/* A Semana e o Dia abrem perto do dia útil, e não às 00:00.
 *
 * As vinte e quatro horas estão sempre lá — esconder a madrugada esconderia o
 * turno de quem trabalha nela. O que muda é onde a vista **abre**: uma semana
 * que abre à meia-noite mostra oito horas vazias e obriga a rolar para ver o
 * que está marcado.
 *
 * Corre antes da primeira pintura, e não depois: `defer` executa com o
 * documento já construído e antes de o browser desenhar, portanto a posição
 * inicial é a posição, e não um salto que se vê acontecer.
 *
 * Prefere a primeira actividade do período; sem nenhuma, abre às sete. Assim a
 * vista abre onde há alguma coisa em vez de num sítio fixo que pode não ter
 * nada. */
(function () {
  var corpo = document.querySelector('[data-oc="linha-do-tempo"]');
  if (!corpo) return;

  var bloco = corpo.querySelector('.oc-cal-bloco');
  if (bloco) {
    /* Uma faixa acima do primeiro evento, para ele não ficar colado ao topo e
       se perceber que há espaço antes dele. */
    var faixa = bloco.offsetHeight > 0 ? bloco.offsetTop : 0;
    corpo.scrollTop = Math.max(0, faixa - 24);
    return;
  }

  var eixo = corpo.querySelector('.oc-cal-eixo');
  if (!eixo || !eixo.children.length) return;
  var sete = eixo.children[7];
  if (sete) corpo.scrollTop = sete.offsetTop;
})();

/* O editor de actividade responde ao que se escolhe.
 *
 * Os ganchos `data-oc` já existiam na marcação e não havia código nenhum a
 * usá-los: o `Dia inteiro` era uma caixa que não fazia nada, e os selectores de
 * Unidade e de Ambiente apareciam os dois ao mesmo tempo, vazios, fosse qual
 * fosse o âmbito. Controlos visíveis sem efeito.
 *
 * Nada aqui decide autoridade. O que se mostra muda; o que o Core recebe e
 * valida é o mesmo. */
(function () {
  var form = document.querySelector('[data-oc="editor"]');
  if (!form) return;

  function mostrar(elemento, visivel) {
    if (!elemento) return;
    if (visivel) elemento.removeAttribute('hidden');
    else elemento.setAttribute('hidden', '');
  }

  /* ── A zona de quem marca ──────────────────────────────────────────────
   *
   * `UTC` é como o Core guarda, não é onde a pessoa está. O browser sabe a
   * zona real; escrevê-la aqui é dizer a verdade sobre a hora que se acabou de
   * escrever. Continua a ir no pedido e continua a ser o Core a validá-la. */
  var zona = form.querySelector('[data-oc="timezone"]');
  var zonaTexto = form.querySelector('[data-oc="timezone-label"]');
  try {
    var resolvida = Intl.DateTimeFormat().resolvedOptions().timeZone;
    if (resolvida) {
      if (zona) zona.value = resolvida;
      if (zonaTexto) zonaTexto.textContent = resolvida;
    }
  } catch (erro) {
    /* Sem zona resolvida fica `UTC`, que é o que o servidor já tinha escrito. */
  }

  /* ── Dia inteiro ───────────────────────────────────────────────────────
   *
   * As horas deixam de ser relevantes, e por isso deixam de estar lá. Manter os
   * campos visíveis e inertes seria pedir que se preenchessem coisas que não
   * vão a lado nenhum. */
  var diaInteiro = form.querySelector('[data-oc="all-day"]');
  var comHora = form.querySelector('[data-oc="timed-fields"]');
  var semHora = form.querySelector('[data-oc="allday-fields"]');
  var linhaZona = form.querySelector('.oc-zona');

  function aplicarDiaInteiro() {
    var inteiro = diaInteiro && diaInteiro.checked;
    mostrar(comHora, !inteiro);
    mostrar(semHora, !!inteiro);
    mostrar(linhaZona, !inteiro);
  }

  if (diaInteiro) {
    diaInteiro.addEventListener('change', aplicarDiaInteiro);
    aplicarDiaInteiro();
  }

  /* ── Pertence a ────────────────────────────────────────────────────────
   *
   * Cada âmbito tem um destino, e só um. Mostrar os outros é oferecer escolhas
   * que não pertencem à pergunta que se acabou de responder. */
  var ambito = form.querySelector('[data-oc="scope"]');
  var campoUnidade = form.querySelector('[data-oc="unit-field"]');
  var campoAmbiente = form.querySelector('[data-oc="workspace-field"]');

  function aplicarAmbito() {
    var valor = ambito ? ambito.value : 'personal';
    mostrar(campoUnidade, valor === 'unit');
    mostrar(campoAmbiente, valor === 'research_workspace');
  }

  if (ambito) {
    ambito.addEventListener('change', aplicarAmbito);
    aplicarAmbito();
  }

  /* ── Um envio, uma actividade ──────────────────────────────────────────
   *
   * Dois cliques no mesmo botão marcavam a mesma reunião duas vezes. O botão
   * desactiva-se no primeiro e diz que está a marcar — desactivar sem dizer
   * nada parece uma interface que deixou de responder. */
  form.addEventListener('submit', function () {
    var botao = form.querySelector('[data-oc="submeter"]');
    if (!botao) return;
    /* Depois do envio começar: desactivar antes impediria o próprio envio de
       levar o botão consigo em alguns browsers. */
    window.setTimeout(function () {
      botao.disabled = true;
      botao.dataset.ocAntes = botao.textContent;
      botao.textContent = 'A criar…';
    }, 0);
  });
})();

/* Participantes, e o horário que acompanha quem o muda.
 *
 * Nada aqui decide quem pode participar. A lista é o universo que o Core já
 * autorizou a quem está a marcar, e cada identificador volta a ser verificado
 * do outro lado antes de alguma coisa ficar escrita. */
(function () {
  var form = document.querySelector('[data-oc="editor"]');
  if (!form) return;

  /* ── Participantes ─────────────────────────────────────────────────── */
  var procura = form.querySelector('[data-oc="procura-pessoa"]');
  var lista = form.querySelector('[data-oc="lista-pessoas"]');
  var nada = form.querySelector('[data-oc="sem-pessoas"]');
  var escolhidos = form.querySelector('[data-oc="escolhidos"]');

  if (procura && lista && escolhidos) {
    var jaEscolhidos = {};

    function mostrarLista(visivel) {
      if (visivel) lista.removeAttribute('hidden');
      else lista.setAttribute('hidden', '');
    }

    function filtrar() {
      var termo = procura.value.trim().toLowerCase();
      if (!termo) {
        mostrarLista(false);
        nada.setAttribute('hidden', '');
        return;
      }
      var visiveis = 0;
      Array.prototype.forEach.call(lista.children, function (li) {
        var botao = li.querySelector('[data-oc="pessoa"]');
        if (!botao) return;
        /* Quem já foi escolhido sai da procura: oferecê-lo outra vez é oferecer
           uma acção que não faz nada. */
        /* Nome **ou** endereço institucional. Só o nome deixava de encontrar
           quem se procura pelo endereço — que é a identidade humana desde o
           ADR-0106, e o que uma pessoa tem à mão quando já lhe escreveu. */
        var texto = (
          (botao.dataset.nome || '') + ' ' + (botao.dataset.email || '')
        ).toLowerCase();
        var elegivel = !jaEscolhidos[botao.dataset.id] && texto.indexOf(termo) >= 0;
        li.hidden = !elegivel;
        if (elegivel) visiveis += 1;
      });
      mostrarLista(visiveis > 0);
      if (visiveis > 0) nada.setAttribute('hidden', '');
      else nada.removeAttribute('hidden');
    }

    function remover(id) {
      delete jaEscolhidos[id];
      var marca = escolhidos.querySelector('[data-id="' + id + '"]');
      if (marca) marca.remove();
      filtrar();
      procura.focus();
    }

    function escolher(id, nome) {
      /* A mesma pessoa duas vezes é a mesma actividade com as mesmas pessoas.
         O Core recusa o duplicado na chave primária; impedi-lo aqui evita
         transformar um clique a mais numa mensagem de erro. */
      if (jaEscolhidos[id]) return;
      jaEscolhidos[id] = true;

      var marca = document.createElement('span');
      marca.className = 'oc-escolhido';
      marca.dataset.id = id;

      var texto = document.createElement('span');
      texto.textContent = nome;
      marca.appendChild(texto);

      var campo = document.createElement('input');
      campo.type = 'hidden';
      campo.name = 'participants';
      campo.value = id;
      marca.appendChild(campo);

      var tirar = document.createElement('button');
      tirar.type = 'button';
      tirar.className = 'oc-escolhido__tirar';
      tirar.setAttribute('aria-label', 'Retirar ' + nome);
      tirar.textContent = '×';
      tirar.addEventListener('click', function () { remover(id); });
      marca.appendChild(tirar);

      escolhidos.appendChild(marca);
      procura.value = '';
      filtrar();
    }

    procura.addEventListener('input', filtrar);
    procura.addEventListener('keydown', function (event) {
      if (event.key !== 'Escape') return;
      procura.value = '';
      filtrar();
    });

    lista.addEventListener('click', function (event) {
      var botao = event.target.closest('[data-oc="pessoa"]');
      if (!botao) return;
      escolher(botao.dataset.id, botao.dataset.nome);
    });
  }

  /* ── O fim acompanha o início, enquanto ninguém lhe tocar ──────────────
   *
   * Mudar o início para as 21:00 quando o fim ainda é o que a aplicação propôs
   * deve dar 21:30, e não deixar um fim anterior ao início. Mas se a pessoa já
   * escolheu o fim, essa escolha é dela — e não se sobrepõe.
   *
   * O estado é explícito, e não uma heurística: uma marca no próprio campo,
   * posta quando a pessoa o edita. */
  var inicio = form.querySelector('[data-oc="inicio"]');
  var fim = form.querySelector('[data-oc="fim"]');

  if (inicio && fim) {
    var MINUTOS = 30;

    fim.addEventListener('input', function () {
      fim.dataset.ocEscolhido = 'sim';
    });

    inicio.addEventListener('change', function () {
      if (fim.dataset.ocEscolhido === 'sim') return;
      if (!inicio.value) return;

      var quando = new Date(inicio.value);
      if (isNaN(quando.getTime())) return;
      quando.setMinutes(quando.getMinutes() + MINUTOS);

      var dois = function (n) { return String(n).padStart(2, '0'); };
      fim.value =
        quando.getFullYear() + '-' + dois(quando.getMonth() + 1) + '-' + dois(quando.getDate()) +
        'T' + dois(quando.getHours()) + ':' + dois(quando.getMinutes());
    });
  }


})();

/* As Mensagens vivem no seu próprio bloco.
 *
 * Estiveram dentro do bloco do editor de calendário, que começa por
 * `if (!form) return;` — e em `/messages` não há editor nenhum, portanto nada
 * disto corria. O `+` abria o diálogo por acaso (o elemento já lá estava, só
 * escondido) e a procura não respondia a tecla nenhuma.
 *
 * Um bloco que serve um ecrã sai cedo quando esse ecrã não está montado. Pôr
 * outro ecrã lá dentro é herdar a saída dele. */
/* ══ MENSAGENS ═══════════════════════════════════════════════════════════
 *
 * O que aqui está é interacção: enviar sem recarregar, responder, mencionar,
 * reagir, escrever, e ouvir o que chega. A autoridade continua toda do lado
 * do Core — este ficheiro nunca decide quem pode o quê, e nunca escreve na
 * base nem no Redis.
 *
 * O socket serve para **saber que algo mudou**. Quando sabe, vai buscar ao
 * Core — que autoriza outra vez. Nunca desenha conteúdo que o socket
 * transporte, porque o socket não transporta conteúdo nenhum. */
(function mensagens() {
  var $ = function (sel, root) { return (root || document).querySelector(sel); };
  var $$ = function (sel, root) {
    return Array.prototype.slice.call((root || document).querySelectorAll(sel));
  };

  var raiz = $('[data-oc="mensagens"]');
  if (!raiz) return;

  var painel = $('[data-oc="conversa-aberta"]');
  var conversa = painel ? painel.dataset.ocId : null;

  /* ── Recarregar a conversa ──────────────────────────────────────────
   *
   * Ir buscar o HTML e trocar só o painel. Um `location.reload()` faria
   * piscar a barra lateral inteira a cada mensagem que chegasse. */
  var aRecarregar = false;
  function recarregar() {
    if (aRecarregar) return;
    aRecarregar = true;
    fetch(window.location.pathname, { headers: { 'X-Requested-With': 'oc' } })
      .then(function (r) { return r.text(); })
      .then(function (html) {
        var novo = new DOMParser().parseFromString(html, 'text/html');
        var painelNovo = novo.querySelector('[data-oc="conversa-aberta"]');
        var listaNova = novo.querySelector('.oc-msg__conversas');
        var painelActual = $('[data-oc="conversa-aberta"]');
        var listaActual = $('.oc-msg__conversas');

        /* A posição do scroll antes de trocar: se a pessoa estava a ler
         * mensagens antigas, não se lhe arranca a página debaixo dos olhos. */
        var fluxo = $('[data-oc="fluxo"]');
        var noFundo = fluxo
          ? fluxo.scrollHeight - fluxo.scrollTop - fluxo.clientHeight < 80
          : true;
        var altura = fluxo ? fluxo.scrollHeight : 0;
        var topo = fluxo ? fluxo.scrollTop : 0;

        if (painelNovo && painelActual) painelActual.replaceWith(painelNovo);
        if (listaNova && listaActual) listaActual.replaceWith(listaNova);

        ligar();
        var novoFluxo = $('[data-oc="fluxo"]');
        if (novoFluxo) {
          if (noFundo) {
            novoFluxo.scrollTop = novoFluxo.scrollHeight;
          } else {
            /* Mensagens novas entram por baixo; manter o que se lia no
             * mesmo sítio é manter a distância ao fim. */
            novoFluxo.scrollTop = topo + (novoFluxo.scrollHeight - altura);
            mostrarAviso();
          }
        }
      })
      .catch(function () { /* Sem rede, fica o que está. */ })
      .then(function () { aRecarregar = false; });
  }

  function mostrarAviso() {
    var fluxo = $('[data-oc="fluxo"]');
    if (!fluxo || $('[data-oc="aviso-novas"]')) return;
    var botao = document.createElement('button');
    botao.type = 'button';
    botao.className = 'oc-msg__aviso-novas';
    botao.dataset.oc = 'aviso-novas';
    botao.textContent = 'Novas mensagens ↓';
    botao.addEventListener('click', function () {
      fluxo.scrollTop = fluxo.scrollHeight;
      botao.remove();
    });
    fluxo.parentElement.appendChild(botao);
  }

  /* ── Enviar ─────────────────────────────────────────────────────────── */

  function enviar() {
    var caixa = $('[data-oc="texto"]');
    if (!caixa || !conversa) return;
    var texto = caixa.value.trim();
    if (!texto) return;

    var respondeA = raiz.dataset.ocResponderA || '';
    var mencoes = raiz.dataset.ocMencoes || '';

    var dados = new URLSearchParams();
    dados.set('body', texto);
    if (respondeA) dados.set('reply_to', respondeA);
    if (mencoes) dados.set('mentions', mencoes);
    /* A chave de idempotência é deste envio, e não desta sessão: um
     * duplo-clique traz a mesma e o Core devolve a mensagem que a primeira
     * escreveu. */
    dados.set('idempotency_key', chaveDeEnvio(texto));

    caixa.disabled = true;
    fetch('/messages/' + conversa + '/send', {
      method: 'POST',
      headers: { 'Content-Type': 'application/x-www-form-urlencoded' },
      body: dados.toString(),
    })
      .then(function () {
        caixa.value = '';
        caixa.style.height = '';
        cancelarResposta();
        raiz.dataset.ocMencoes = '';
        recarregar();
      })
      .catch(function () { /* Fica escrito; a pessoa tenta outra vez. */ })
      .then(function () {
        caixa.disabled = false;
        caixa.focus();
      });
  }

  var ultimaChave = null;
  var ultimoTexto = null;
  function chaveDeEnvio(texto) {
    if (texto !== ultimoTexto) {
      ultimoTexto = texto;
      ultimaChave =
        Date.now().toString(36) + '-' + Math.random().toString(36).slice(2, 10);
    }
    return ultimaChave;
  }

  /* ── Responder ──────────────────────────────────────────────────────── */

  function responderA(mensagem) {
    var painelResposta = $('[data-oc="a-responder"]');
    if (!painelResposta) return;
    var autor = mensagem.querySelector('.oc-msg__mensagem-autor');
    var texto = mensagem.querySelector('.oc-msg__texto');

    raiz.dataset.ocResponderA = mensagem.dataset.ocId;
    var quem = $('[data-oc="resposta-quem"]');
    var excerto = $('[data-oc="resposta-excerto"]');
    /* `textContent` e nunca `innerHTML`: o que a pessoa escreveu continua a
     * ser texto, aqui como em todo o lado. */
    if (quem) quem.textContent = autor ? autor.textContent : 'Mensagem';
    if (excerto) excerto.textContent = texto ? texto.textContent : '';
    painelResposta.hidden = false;

    var caixa = $('[data-oc="texto"]');
    if (caixa) caixa.focus();
  }

  function cancelarResposta() {
    raiz.dataset.ocResponderA = '';
    var painelResposta = $('[data-oc="a-responder"]');
    if (painelResposta) painelResposta.hidden = true;
  }

  /* ── Menções ────────────────────────────────────────────────────────── */

  function abrirMencoes(aberto) {
    var lista = $('[data-oc="mencoes"]');
    if (lista) lista.hidden = !aberto;
  }

  function inserirMencao(botao) {
    var caixa = $('[data-oc="texto"]');
    if (!caixa) return;
    var nome = botao.dataset.ocNome || '';
    var quem = botao.dataset.ocQuem || '';

    /* Substitui o `@` que a pessoa escreveu, e guarda a **identidade**. O
     * texto renderizado continua texto; quem é mencionado é uma referência,
     * e não o resultado de procurar um nome depois. */
    var valor = caixa.value;
    var corte = valor.lastIndexOf('@');
    caixa.value =
      (corte >= 0 ? valor.slice(0, corte) : valor) + '@' + nome + ' ';

    var actuais = (raiz.dataset.ocMencoes || '').split(',').filter(Boolean);
    if (actuais.indexOf(quem) === -1) actuais.push(quem);
    raiz.dataset.ocMencoes = actuais.join(',');

    abrirMencoes(false);
    caixa.focus();
  }

  /* ── Ligar tudo ao que está no ecrã ─────────────────────────────────── */

  function ligar() {
    painel = $('[data-oc="conversa-aberta"]');
    conversa = painel ? painel.dataset.ocId : null;

    var caixa = $('[data-oc="texto"]');
    if (caixa && !caixa.dataset.ocLigada) {
      caixa.dataset.ocLigada = 'sim';

      caixa.addEventListener('input', function () {
        /* Cresce com o texto, até ao limite que o CSS impõe. */
        caixa.style.height = 'auto';
        caixa.style.height = caixa.scrollHeight + 'px';
        /* O `@` no fim de uma palavra abre a lista de quem se pode
         * mencionar — e só de quem participa nesta conversa. */
        abrirMencoes(/(^|\s)@\S*$/.test(caixa.value));
        escrevendo();
      });

      caixa.addEventListener('keydown', function (evento) {
        if (evento.key === 'Enter' && !evento.shiftKey) {
          evento.preventDefault();
          enviar();
        }
        if (evento.key === 'Escape') {
          abrirMencoes(false);
          fecharTudo();
        }
      });
    }

    var fluxo = $('[data-oc="fluxo"]');
    if (fluxo && !fluxo.dataset.ocLigado) {
      fluxo.dataset.ocLigado = 'sim';
      fluxo.scrollTop = fluxo.scrollHeight;
    }
  }

  /* Delegação: o painel é substituído a cada recarga, e ouvir na raiz
   * dispensa voltar a ligar cada botão. */
  raiz.addEventListener('click', function (evento) {
    var alvo = evento.target.closest('[data-oc]');
    if (!alvo) return;
    var accao = alvo.dataset.oc;

    if (accao === 'enviar') { enviar(); return; }

    if (accao === 'nova-conversa') { abrirNova(); return; }
    if (accao === 'fechar-nova') { fecharNova(); return; }
    if (accao === 'modo') { escolherModo(alvo.dataset.ocModo); return; }
    if (accao === 'escolher-pessoa') { escolherPessoa(alvo); return; }
    if (accao === 'retirar-escolhido') { retirarEscolhido(alvo.dataset.ocQuem); return; }
    if (accao === 'criar-conversa') { criarConversa(); return; }

    if (accao === 'responder') {
      var mensagem = alvo.closest('[data-oc="mensagem"]');
      if (mensagem) responderA(mensagem);
      return;
    }
    if (accao === 'cancelar-resposta') { cancelarResposta(); return; }

    if (accao === 'copiar') {
      var msg = alvo.closest('[data-oc="mensagem"]');
      var texto = msg ? msg.querySelector('.oc-msg__texto') : null;
      /* O texto, e não a marcação: colar isto noutro sítio tem de dar o
       * que a pessoa escreveu. */
      if (texto && navigator.clipboard) {
        navigator.clipboard.writeText(texto.textContent).catch(function () {});
      }
      return;
    }

    if (accao === 'abrir-emoji') {
      var paleta = $('[data-oc="emoji"]');
      if (paleta) {
        paleta.hidden = !paleta.hidden;
        alvo.setAttribute('aria-expanded', String(!paleta.hidden));
      }
      return;
    }

    if (accao === 'emoji-item') {
      var entrada = $('[data-oc="texto"]');
      if (entrada) {
        entrada.value += alvo.dataset.ocEmoji;
        entrada.focus();
      }
      var p = $('[data-oc="emoji"]');
      if (p) p.hidden = true;
      return;
    }

    if (accao === 'abrir-reaccoes') {
      var mensagemAlvo = alvo.closest('[data-oc="mensagem"]');
      if (mensagemAlvo) abrirPaletaDeReaccao(mensagemAlvo, alvo);
      return;
    }

    if (accao === 'reagir') {
      var m = alvo.closest('[data-oc="mensagem"]');
      if (m) reagir(m.dataset.ocId, alvo.dataset.ocEmoji);
      return;
    }

    if (accao === 'mencao') { inserirMencao(alvo); return; }

    if (accao === 'abrir-assist') {
      var menu = $('[data-oc="assist-menu"]');
      if (menu) {
        menu.hidden = !menu.hidden;
        alvo.setAttribute('aria-expanded', String(!menu.hidden));
      }
      return;
    }

    if (accao === 'assist') { pedirAoOcinye(alvo.dataset.ocAccao); return; }

    if (accao === 'usar-sugestao') {
      var campo = $('[data-oc="texto"]');
      var sugerido = $('[data-oc="sugestao-texto"]');
      if (campo && sugerido) campo.value = sugerido.textContent;
      esconderSugestao();
      if (campo) campo.focus();
      return;
    }
    if (accao === 'manter-original') { esconderSugestao(); return; }

    if (accao === 'detalhes-do-grupo') {
      var detalhes = $('[data-oc="detalhes"]');
      if (detalhes) {
        detalhes.hidden = !detalhes.hidden;
        alvo.setAttribute('aria-expanded', String(!detalhes.hidden));
      }
      return;
    }

    if (accao === 'citada') {
      var destino = document.getElementById('mensagem-' + alvo.dataset.ocAlvo);
      if (destino) {
        evento.preventDefault();
        destino.scrollIntoView({ block: 'center' });
        var bloco = destino.closest('[data-oc="mensagem"]');
        if (bloco) {
          bloco.classList.add('oc-msg__mensagem--realcada');
          window.setTimeout(function () {
            bloco.classList.remove('oc-msg__mensagem--realcada');
          }, 1600);
        }
      }
    }
  });

  function fecharTudo() {
    ['emoji', 'assist-menu', 'mencoes'].forEach(function (nome) {
      var elemento = $('[data-oc="' + nome + '"]');
      if (elemento) elemento.hidden = true;
    });
  }

  /* ── Reacções ───────────────────────────────────────────────────────── */

  var REACCOES = ['👍', '❤️', '😂', '🎉', '👀', '✅'];

  function abrirPaletaDeReaccao(mensagem, ancora) {
    var existente = $('[data-oc="paleta-reaccao"]');
    if (existente) existente.remove();

    var paleta = document.createElement('div');
    paleta.className = 'oc-msg__emoji oc-msg__emoji--reaccao';
    paleta.dataset.oc = 'paleta-reaccao';
    paleta.setAttribute('role', 'menu');
    REACCOES.forEach(function (emoji) {
      var botao = document.createElement('button');
      botao.type = 'button';
      botao.className = 'oc-msg__emoji-item';
      botao.setAttribute('role', 'menuitem');
      botao.textContent = emoji;
      botao.addEventListener('click', function () {
        reagir(mensagem.dataset.ocId, emoji);
        paleta.remove();
      });
      paleta.appendChild(botao);
    });
    ancora.parentElement.appendChild(paleta);
  }

  function reagir(mensagem, emoji) {
    if (!conversa || !mensagem) return;
    var dados = new URLSearchParams();
    dados.set('message', mensagem);
    dados.set('emoji', emoji);
    fetch('/messages/' + conversa + '/react', {
      method: 'POST',
      headers: { 'Content-Type': 'application/x-www-form-urlencoded' },
      body: dados.toString(),
    })
      .then(recarregar)
      .catch(function () {});
  }

  /* ── Assistência ────────────────────────────────────────────────────── */

  function esconderSugestao() {
    var caixa = $('[data-oc="sugestao"]');
    if (caixa) caixa.hidden = true;
  }

  /* Pede ao Ocinye. **Nunca envia**: o que volta é uma proposta, o original
   * fica onde está, e é a pessoa que escolhe e carrega em Enviar. */
  function pedirAoOcinye(accao) {
    var campo = $('[data-oc="texto"]');
    if (!campo || !campo.value.trim()) return;
    fecharTudo();

    var caixa = $('[data-oc="sugestao"]');
    var texto = $('[data-oc="sugestao-texto"]');
    var titulo = $('[data-oc="sugestao-titulo"]');
    if (!caixa || !texto) return;

    texto.textContent = 'A pensar…';
    if (titulo) titulo.textContent = rotuloDaAccao(accao);
    caixa.hidden = false;

    var dados = new URLSearchParams();
    dados.set('action', accao);
    dados.set('draft', campo.value);
    fetch('/messages/assist', {
      method: 'POST',
      headers: { 'Content-Type': 'application/x-www-form-urlencoded' },
      body: dados.toString(),
    })
      .then(function (r) { return r.json(); })
      .then(function (resposta) {
        texto.textContent = resposta && resposta.text
          ? resposta.text
          : 'O Ocinye não conseguiu ajudar desta vez.';
      })
      .catch(function () {
        texto.textContent = 'O Ocinye não está disponível neste momento.';
      });
  }

  function rotuloDaAccao(accao) {
    var nomes = {
      corrigir: 'Corrigido',
      melhorar: 'Melhorado',
      formal: 'Mais formal',
      curto: 'Mais curto',
      claro: 'Mais claro',
      traduzir: 'Traduzido',
    };
    return nomes[accao] || 'Sugestão';
  }

  /* ── Tempo real ─────────────────────────────────────────────────────── */

  var socket = null;
  var espera = 1000;
  var aEscrever = false;
  var travao = null;

  function escrevendo() {
    if (!socket || socket.readyState !== 1 || !conversa) return;
    /* Um evento por keystroke seria uma tempestade. Um a abrir, e um a
     * fechar quando as teclas param. */
    if (!aEscrever) {
      aEscrever = true;
      socket.send(JSON.stringify({
        tipo: 'typing', conversation_id: conversa, a_escrever: true,
      }));
    }
    window.clearTimeout(travao);
    travao = window.setTimeout(function () {
      aEscrever = false;
      if (socket && socket.readyState === 1) {
        socket.send(JSON.stringify({
          tipo: 'typing', conversation_id: conversa, a_escrever: false,
        }));
      }
    }, 2500);
  }

  function ligarSocket() {
    var protocolo = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
    try {
      socket = new WebSocket(protocolo + '//' + window.location.host + '/realtime');
    } catch (erro) {
      return;
    }

    socket.addEventListener('open', function () {
      espera = 1000;
      if (conversa) {
        socket.send(JSON.stringify({
          tipo: 'subscribe', canal: { canal: 'conversation', id: conversa },
        }));
      }
      window.setInterval(function () {
        if (socket && socket.readyState === 1) {
          socket.send(JSON.stringify({ tipo: 'heartbeat' }));
        }
      }, 20000);
    });

    socket.addEventListener('message', function (evento) {
      var dados;
      try { dados = JSON.parse(evento.data); } catch (erro) { return; }

      /* O evento diz **que** algo mudou. O conteúdo vem do Core, que
       * autoriza outra vez — o socket não transporta mensagens. */
      if (dados.tipo === 'message_created' ||
          dados.tipo === 'message_updated' ||
          dados.tipo === 'reaction_changed' ||
          dados.tipo === 'participation_changed') {
        recarregar();
        return;
      }
      if (dados.tipo === 'typing_changed') { actualizarEscrita(); }
    });

    socket.addEventListener('close', function () {
      socket = null;
      /* Recuo exponencial, com tecto: uma rede em baixo não se martela. */
      espera = Math.min(espera * 2, 30000);
      window.setTimeout(ligarSocket, espera);
    });
  }

  function actualizarEscrita() {
    if (!conversa) return;
    fetch('/messages/' + conversa + '/typing')
      .then(function (r) { return r.json(); })
      .then(function (dados) {
        var linha = $('[data-oc="a-escrever"]');
        if (!linha) return;
        if (dados && dados.phrase) {
          linha.textContent = dados.phrase;
          linha.hidden = false;
        } else {
          linha.textContent = '';
          linha.hidden = true;
        }
      })
      .catch(function () {});
  }


  /* ── Nova conversa ──────────────────────────────────────────────────
   *
   * Um diálogo, e não uma página: começar a falar com alguém é um gesto.
   * A pesquisa é do servidor — uma instituição não cabe num `select`, e
   * carregá-la inteira para filtrar aqui seria mandar a lista de toda a
   * gente para cada pessoa que abre as Mensagens. */

  var modo = 'directa';
  var escolhidos = [];

  function dialogo() { return $('[data-oc="nova-conversa-dialogo"]'); }

  function abrirNova() {
    var caixa = dialogo();
    if (!caixa) return;
    caixa.hidden = false;
    escolhidos = [];
    escolherModo('directa');
    var procura = $('[data-oc="procurar-pessoa"]');
    if (procura) { procura.value = ''; procura.focus(); }
    desenharResultados([]);
    desenharEscolhidos();
  }

  function fecharNova() {
    var caixa = dialogo();
    if (caixa) caixa.hidden = true;
  }

  function escolherModo(novo) {
    modo = novo === 'grupo' ? 'grupo' : 'directa';
    $$('[data-oc="modo"]').forEach(function (botao) {
      var activo = botao.dataset.ocModo === modo;
      botao.classList.toggle('oc-msg__modo--activo', activo);
      botao.setAttribute('aria-selected', String(activo));
    });
    var campoNome = $('[data-oc="campo-nome"]');
    if (campoNome) campoNome.hidden = modo !== 'grupo';
    /* Trocar de modo limpa a escolha: uma directa é com uma pessoa, e
     * arrastar quatro nomes para lá seria pedir o impossível. */
    escolhidos = [];
    desenharEscolhidos();
    validarCriar();
  }

  function desenharResultados(pessoas) {
    var lista = $('[data-oc="resultados"]');
    var estado = $('[data-oc="estado-da-procura"]');
    if (!lista) return;
    lista.textContent = '';

    if (!pessoas.length) {
      if (estado) estado.hidden = false;
      return;
    }
    if (estado) estado.hidden = true;

    pessoas.forEach(function (pessoa) {
      var botao = document.createElement('button');
      botao.type = 'button';
      botao.className = 'oc-msg__resultado';
      botao.setAttribute('role', 'option');
      botao.setAttribute('aria-selected', 'false');
      botao.dataset.oc = 'escolher-pessoa';
      botao.dataset.ocQuem = pessoa.id;
      botao.dataset.ocNome = pessoa.name;

      var inicial = document.createElement('span');
      inicial.className = 'oc-avatar oc-avatar--sm';
      inicial.setAttribute('aria-hidden', 'true');
      inicial.textContent = iniciaisDe(pessoa.name);

      var nome = document.createElement('span');
      nome.className = 'oc-msg__resultado-nome';
      /* `textContent`: um nome é texto, e continua a sê-lo aqui. */
      nome.textContent = pessoa.name;

      var email = document.createElement('span');
      email.className = 'oc-msg__resultado-email';
      email.textContent = pessoa.email || '';

      botao.appendChild(inicial);
      botao.appendChild(nome);
      botao.appendChild(email);
      lista.appendChild(botao);
    });
  }

  function iniciaisDe(nome) {
    var partes = String(nome || '').trim().split(/\s+/).filter(Boolean);
    if (!partes.length) return '';
    var primeira = partes[0].charAt(0);
    var ultima = partes.length > 1 ? partes[partes.length - 1].charAt(0) : '';
    return (primeira + ultima).toUpperCase();
  }

  function escolherPessoa(botao) {
    var quem = botao.dataset.ocQuem;
    var nome = botao.dataset.ocNome;

    if (modo === 'directa') {
      /* Numa directa não há passo intermédio: escolher é começar. */
      criarDirecta(quem);
      return;
    }
    if (!escolhidos.some(function (p) { return p.id === quem; })) {
      escolhidos.push({ id: quem, name: nome });
    }
    desenharEscolhidos();
    validarCriar();

    var procura = $('[data-oc="procurar-pessoa"]');
    if (procura) { procura.value = ''; procura.focus(); }
    desenharResultados([]);
  }

  function retirarEscolhido(quem) {
    escolhidos = escolhidos.filter(function (p) { return p.id !== quem; });
    desenharEscolhidos();
    validarCriar();
  }

  function desenharEscolhidos() {
    var caixa = $('[data-oc="escolhidos"]');
    if (!caixa) return;
    caixa.textContent = '';
    caixa.hidden = escolhidos.length === 0;

    escolhidos.forEach(function (pessoa) {
      var etiqueta = document.createElement('span');
      etiqueta.className = 'oc-msg__escolhido';

      var nome = document.createElement('span');
      nome.textContent = pessoa.name;

      var tirar = document.createElement('button');
      tirar.type = 'button';
      tirar.className = 'oc-msg__escolhido-tirar';
      tirar.dataset.oc = 'retirar-escolhido';
      tirar.dataset.ocQuem = pessoa.id;
      tirar.title = 'Retirar ' + pessoa.name;
      tirar.textContent = '×';

      etiqueta.appendChild(nome);
      etiqueta.appendChild(tirar);
      caixa.appendChild(etiqueta);
    });
  }

  function validarCriar() {
    var botao = $('[data-oc="criar-conversa"]');
    if (!botao) return;
    var nome = $('[data-oc="nome-do-grupo"]');
    var temNome = nome && nome.value.trim().length > 0;
    botao.disabled = !(modo === 'grupo' && temNome && escolhidos.length > 0);
  }

  function criarDirecta(quem) {
    var dados = new URLSearchParams();
    dados.set('with', quem);
    submeterNova(dados);
  }

  function criarConversa() {
    if (modo !== 'grupo') return;
    var nome = $('[data-oc="nome-do-grupo"]');
    var dados = new URLSearchParams();
    dados.set('name', nome ? nome.value.trim() : '');
    dados.set('members', escolhidos.map(function (p) { return p.id; }).join(','));
    submeterNova(dados);
  }

  function submeterNova(dados) {
    var botao = $('[data-oc="criar-conversa"]');
    if (botao) botao.disabled = true;

    /* Um formulário a sério, e não `fetch`: o Core responde com um
     * encaminhamento para a conversa, e deixar o browser segui-lo é mais
     * simples do que reconstruir a navegação aqui. */
    var formulario = document.createElement('form');
    formulario.method = 'post';
    formulario.action = '/messages/start';
    dados.forEach(function (valor, chave) {
      var campo = document.createElement('input');
      campo.type = 'hidden';
      campo.name = chave;
      campo.value = valor;
      formulario.appendChild(campo);
    });
    document.body.appendChild(formulario);
    formulario.submit();
  }

  var travaoDaProcura = null;
  function ligarProcura() {
    var procura = $('[data-oc="procurar-pessoa"]');
    if (!procura || procura.dataset.ocLigada) return;
    procura.dataset.ocLigada = 'sim';

    procura.addEventListener('input', function () {
      window.clearTimeout(travaoDaProcura);
      var termo = procura.value.trim();
      procura.setAttribute('aria-expanded', String(termo.length >= 2));
      if (termo.length < 2) { desenharResultados([]); return; }

      /* Um pedido por tecla seria uma tempestade. Espera-se que as teclas
       * parem. */
      travaoDaProcura = window.setTimeout(function () {
        fetch('/messages/people?q=' + encodeURIComponent(termo))
          .then(function (r) { return r.json(); })
          .then(function (dados) {
            desenharResultados((dados && dados.people) || []);
            var estado = $('[data-oc="estado-da-procura"]');
            if (estado && (!dados.people || !dados.people.length)) {
              estado.textContent = 'Ninguém corresponde a «' + termo + '».';
              estado.hidden = false;
            }
          })
          .catch(function () {});
      }, 200);
    });

    procura.addEventListener('keydown', function (evento) {
      if (evento.key === 'Escape') fecharNova();
    });

    var nome = $('[data-oc="nome-do-grupo"]');
    if (nome && !nome.dataset.ocLigada) {
      nome.dataset.ocLigada = 'sim';
      nome.addEventListener('input', validarCriar);
    }
  }

  ligarProcura();

  ligar();
  ligarSocket();

  /* Ao abrir uma conversa, ela fica lida até agora. */
  if (conversa) {
    var dados = new URLSearchParams();
    dados.set('until', new Date().toISOString());
    fetch('/messages/' + conversa + '/read', {
      method: 'POST',
      headers: { 'Content-Type': 'application/x-www-form-urlencoded' },
      body: dados.toString(),
    }).catch(function () {});
  }
})();

/* Duplo clique numa data abre uma actividade nesse dia.
 *
 * # Porque duplo e não simples
 *
 * Porque um clique simples numa célula já significa outra coisa: abrir o dia,
 * ou abrir a actividade em que se carregou. Um calendário onde tocar sem
 * querer cria uma marcação é um calendário em que se deixa de tocar.
 *
 * # Porque não é `cursor: pointer`
 *
 * A célula não se anuncia como botão. Isto é um atalho para quem o conhece, e
 * a criação continua a ter o seu botão visível — `+ Nova actividade` — que é o
 * caminho que qualquer pessoa encontra sem ser ensinada.
 *
 * Nada aqui cria estado: leva a pessoa ao editor com a data já escolhida, e é
 * ela que decide se marca. */
(function () {
  var calendario = document.querySelector('.oc-page--calendar');
  if (!calendario) return;

  /* Um dia inteiro, sem hora: o Mês não a tem. A política de omissão escolhe-a
     no servidor, aplicada a este dia. */
  function abrirNoDia(dia) {
    window.location.assign('/calendar/events/new?on=' + encodeURIComponent(dia));
  }

  /* Uma faixa concreta da Semana ou do Dia: a hora vem de onde se carregou. */
  function abrirNaFaixa(dia, coluna, y) {
    var altura = coluna.offsetHeight;
    if (!altura) return abrirNoDia(dia);

    /* Quarenta e oito faixas de meia hora, as mesmas que a grelha desenha. */
    var faixa = Math.floor((y / altura) * 48);
    if (faixa < 0) faixa = 0;
    if (faixa > 47) faixa = 47;

    var horas = Math.floor(faixa / 2);
    var minutos = faixa % 2 === 0 ? '00' : '30';
    var dois = function (n) { return String(n).padStart(2, '0'); };

    window.location.assign(
      '/calendar/events/new?on=' + encodeURIComponent(dia) +
      '&at=' + encodeURIComponent(dois(horas) + ':' + minutos),
    );
  }

  calendario.addEventListener('dblclick', function (event) {
    /* Duplo clique **numa actividade** é para a abrir, e não para marcar outra
       por cima dela. */
    if (event.target.closest('.oc-cal-bloco, .oc-cal-month__item, .oc-cal-month__more, a, button')) {
      return;
    }

    var coluna = event.target.closest('[data-oc-dia].oc-cal-coluna');
    if (coluna) {
      var caixa = coluna.getBoundingClientRect();
      abrirNaFaixa(coluna.dataset.ocDia, coluna, event.clientY - caixa.top);
      return;
    }

    var celula = event.target.closest('[data-oc-dia]');
    if (celula) abrirNoDia(celula.dataset.ocDia);
  });
})();
