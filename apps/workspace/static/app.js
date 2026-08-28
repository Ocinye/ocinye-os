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

  /* ── Arranque ─────────────────────────────────────────────────────── */

  const start = () => {
    initSidebar();
    initCreateMenu();
    initAccountMenu();
    initPalette();
    initLocalTabs();
    initTemporalCentre();
    initDensity();
    initRowLinks();
    initClock();
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
        var elegivel =
          !jaEscolhidos[botao.dataset.id] &&
          (botao.dataset.nome || '').toLowerCase().indexOf(termo) >= 0;
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
