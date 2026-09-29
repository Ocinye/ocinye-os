/* Ocinye OS · D002 · o motor do Gestor de Janelas no cliente (Claude Code, ADR-0618).
 *
 * O oc-wm.js (Design) transforma gestos e teclas em intenções «oc:wm». Este
 * ficheiro é o ouvinte que o Design deixou a Code (WM-5): valida a forma de
 * cada intenção, pede ao servidor, e aplica o estado que o servidor responde.
 *
 * O servidor é a fonte da verdade (/wm, window_manager.rs). Este ficheiro não
 * decide nada que fique: durante um arrastar mostra a posição provisória, e ao
 * largar pergunta; o que o servidor devolve é o que fica. Uma intenção com
 * forma inválida é ignorada, e o controlo cai no seu fallback real
 * (POST /wm/{id} ou a ligação da janela).
 *
 * Carregado só nas páginas com janelas (routes::shell_page), depois do oc-wm.js. */
(() => {
  'use strict';
  const layer = document.querySelector('[data-oc="wm-layer"]');
  if (!layer || !window.OcWm) return;

  const WIN = /^w[0-9]{1,9}$/;
  const APP = /^[a-z][a-z0-9_-]{0,31}$/;
  const PHASES = new Set(['start', 'move', 'end']);
  const EDGES = new Set(['n', 'e', 's', 'w', 'ne', 'nw', 'se', 'sw']);
  const ZONES = new Set(['left', 'right', 'max']);
  const MIN_W = 360, MIN_H = 240, GRIP_W = 120, GRIP_H = 40;

  const winEl = (id) => layer.querySelector('[data-oc="win"][data-win="' + CSS.escape(id) + '"]');
  const num = (v) => (Number.isFinite(v) ? Math.round(v) : null);
  const area = () => {
    const r = layer.getBoundingClientRect();
    return { w: Math.round(r.width), h: Math.round(r.height) };
  };

  /* Os pedidos seguem por ordem: a ordem das intenções é a ordem das
     transições no servidor. */
  let queue = Promise.resolve();
  const send = (url, fields) => {
    const run = async () => {
      const r = await fetch(url, {
        method: 'POST',
        credentials: 'same-origin',
        headers: { Accept: 'application/json', 'Content-Type': 'application/x-www-form-urlencoded' },
        body: new URLSearchParams(fields),
      });
      let data = null;
      try { data = await r.json(); } catch (_) { data = null; }
      return { status: r.status, data };
    };
    const p = queue.then(run, run);
    queue = p.catch(() => {});
    return p;
  };

  /* A página que o servidor desenha para o estado dado. */
  const go = (href) => {
    if (typeof href === 'string' && href.startsWith('/') && !href.startsWith('//')) location.replace(href);
    else location.reload();
  };

  /* Aplica o estado do servidor. Foco, ordem e geometria mudam no lugar;
     qualquer outra mudança (uma janela a mais ou a menos, um estado novo)
     volta a desenhar a página, para que a marcação seja sempre a do Design
     para aquele estado. */
  const reconcile = (state) => {
    if (!state || !Array.isArray(state.windows)) { location.reload(); return; }
    const els = Array.from(layer.querySelectorAll('[data-oc="win"]'));
    const byId = new Map(state.windows.map((w) => [w.id, w]));
    const structural = els.length !== state.windows.length ||
      els.some((el) => { const w = byId.get(el.dataset.win); return !w || w.state !== el.dataset.state; });
    if (structural) { go(state.href); return; }
    const rank = state.windows.slice().sort((a, b) => a.z - b.z).map((w) => w.id);
    els.forEach((el) => {
      const w = byId.get(el.dataset.win);
      el.dataset.z = String(rank.indexOf(w.id) + 1);
      el.dataset.x = String(w.x); el.dataset.y = String(w.y);
      el.dataset.w = String(w.w); el.dataset.h = String(w.h);
      el.toggleAttribute('data-active', !!w.active);
      if (w.active) el.setAttribute('aria-current', 'true'); else el.removeAttribute('aria-current');
    });
    document.querySelectorAll('[data-oc="wm-focus"][data-win]').forEach((a) => {
      const w = byId.get(a.dataset.win);
      if (w && w.active) a.setAttribute('aria-current', 'true'); else a.removeAttribute('aria-current');
    });
    const active = state.windows.find((w) => w.active);
    document.querySelectorAll('[data-oc="dock-app"]').forEach((a) => {
      const run = a.querySelector('.oc-dock__run');
      if (run) run.toggleAttribute('data-active', state.windows.some((w) => w.app_id === a.dataset.app && w.active));
    });
    /* «Activa» na barra de aplicações e na barra de cima é a aplicação da
       janela da frente, como o servidor a desenharia nessa rota. */
    if (active) {
      document.querySelectorAll('.oc-dock__btn[data-app]').forEach((a) => {
        if (a.dataset.app === active.app_id) a.setAttribute('aria-current', 'page'); else a.removeAttribute('aria-current');
      });
      const el = winEl(active.id);
      const title = el && el.querySelector('.oc-win__title');
      const crumb = document.querySelector('.oc-crumb--app');
      if (title && crumb) crumb.textContent = title.textContent;
      if (title) document.title = document.title.replace(/^[^·]*·/, title.textContent + ' ·');
    }
    if (typeof state.href === 'string' && location.pathname + location.search !== state.href) {
      history.replaceState(null, '', state.href);
    }
  };

  const answer = (r) => {
    if (r.status === 200) reconcile(r.data);
    else if (r.status === 409 && r.data && r.data.reason === 'dirty') go(r.data.confirm);
    else if (r.status === 401) location.reload();
    else location.reload();
  };

  const op = (win, fields) => send('/wm/' + encodeURIComponent(win), fields).then(answer, () => location.reload());

  /* O arrastar: posição provisória, só apresentação, até ao largar. */
  let drag = null;
  const clampGeom = (g, a) => {
    const w = Math.max(MIN_W, Math.min(g.w, a.w));
    const h = Math.max(MIN_H, Math.min(g.h, a.h));
    return {
      x: Math.max(-(w - GRIP_W), Math.min(g.x, a.w - GRIP_W)),
      y: Math.max(0, Math.min(g.y, a.h - GRIP_H)),
      w, h,
    };
  };
  const base = (el) => ({
    x: Number(el.dataset.x) || 0, y: Number(el.dataset.y) || 0,
    w: Number(el.dataset.w) || 760, h: Number(el.dataset.h) || 480,
    state: el.dataset.state,
  });
  const resized = (b, edge, dx, dy) => {
    let { x, y, w, h } = b;
    if (edge.includes('e')) w = b.w + dx;
    if (edge.includes('s')) h = b.h + dy;
    if (edge.includes('w')) { w = b.w - dx; x = b.x + dx; }
    if (edge.includes('n')) { h = b.h - dy; y = b.y + dy; }
    if (w < MIN_W) { if (edge.includes('w')) x -= MIN_W - w; w = MIN_W; }
    if (h < MIN_H) { if (edge.includes('n')) y -= MIN_H - h; h = MIN_H; }
    return { x, y, w, h };
  };

  const gesture = (d) => {
    const el = winEl(d.win);
    const dx = num(d.dx), dy = num(d.dy);
    if (!el || dx === null || dy === null || !PHASES.has(d.phase)) return;
    const a = area();
    if (d.phase === 'start') { drag = { win: d.win, base: base(el) }; return; }
    if (!drag || drag.win !== d.win) return;
    const b = drag.base;
    if (d.op === 'move') {
      const g = clampGeom({ x: b.x + dx, y: b.y + dy, w: b.w, h: b.h }, a);
      if (d.phase === 'move') {
        if (b.state !== 'normal') el.dataset.state = 'normal';
        el.dataset.x = String(g.x); el.dataset.y = String(g.y);
        return;
      }
      drag = null;
      // Largado numa zona: o oc-wm.js envia a seguir a intenção «snap».
      if (d.zone && ZONES.has(d.zone)) return;
      const r = { op: 'move', x: g.x, y: g.y, area_w: a.w, area_h: a.h };
      // Soltar uma maximizada ou encaixada muda o estado: a página volta a
      // desenhar-se com os controlos certos.
      if (b.state !== 'normal') send('/wm/' + encodeURIComponent(d.win), r).then((x) => go(x.data && x.data.href), () => location.reload());
      else op(d.win, r);
      return;
    }
    if (!EDGES.has(d.edge)) return;
    const raw = resized(b, d.edge, dx, dy);
    const g = clampGeom(raw, a);
    if (d.phase === 'move') {
      el.dataset.x = String(g.x); el.dataset.y = String(g.y);
      el.dataset.w = String(g.w); el.dataset.h = String(g.h);
      window.OcWm.size(el, g.w, g.h, g.w <= MIN_W || g.h <= MIN_H);
      return;
    }
    drag = null;
    op(d.win, { op: 'resize', x: g.x, y: g.y, w: g.w, h: g.h, area_w: a.w, area_h: a.h });
  };

  document.addEventListener('oc:wm', (e) => {
    const d = e.detail;
    if (!d || typeof d !== 'object') return;
    if (d.op === 'open') {
      if (typeof d.app !== 'string' || !APP.test(d.app) || typeof d.href !== 'string') return;
      const u = new URL(d.href, location.origin);
      if (u.origin !== location.origin) return;
      e.preventDefault();
      const fields = { app_id: d.app, href: u.pathname };
      if (u.searchParams.get('window') === 'new') fields.window = 'new';
      send('/wm', fields).then((r) => {
        if (r.status !== 200 || !r.data) { location.assign(d.href); return; }
        if (r.data.existing) { try { sessionStorage.setItem('oc-wm-pulse', r.data.id); } catch (_) { /* sem armazenamento: sem pulso */ } }
        go(r.data.href);
      }, () => location.assign(d.href));
      return;
    }
    if (typeof d.win !== 'string' || !WIN.test(d.win) || !winEl(d.win)) return;
    switch (d.op) {
      case 'focus': {
        e.preventDefault();
        const el = winEl(d.win);
        if (el.hasAttribute('data-active') && el.dataset.state !== 'minimized') { window.OcWm.closeSwitcher(); return; }
        op(d.win, { op: 'focus' });
        return;
      }
      case 'minimize': case 'maximize': case 'restore': case 'close':
        e.preventDefault();
        op(d.win, { op: d.op });
        return;
      case 'snap':
        if (!ZONES.has(d.zone)) return;
        e.preventDefault();
        op(d.win, { op: 'snap', zone: d.zone });
        return;
      case 'move': case 'resize':
        e.preventDefault();
        gesture(d);
        return;
      default:
    }
  });

  /* O diálogo de fechar (FG-026): a decisão vai ao servidor sem recarregar.
     «Cancelar» tira o diálogo no lugar, e o oc-wm.js devolve o foco ao
     controlo anterior (ou ao fechar da janela); as outras decisões voltam a
     desenhar a página com o estado que o servidor decidiu. Sem JavaScript, o
     formulário faz o mesmo por POST/redirect. */
  document.addEventListener('submit', (e) => {
    const d = e.target.closest('[data-oc="dirty-close"]');
    if (!d || !WIN.test(d.dataset.win || '')) return;
    const b = e.submitter;
    const decision = b && b.name === 'decision' ? b.value : null;
    if (!['save', 'discard', 'cancel'].includes(decision)) return;
    e.preventDefault();
    send('/wm/' + encodeURIComponent(d.dataset.win) + '/close', { decision }).then((r) => {
      if (r.status !== 200 || !r.data) { location.reload(); return; }
      // O diálogo sai sempre: «Guardar» espera que a aplicação guarde (a janela
      // fecha quando ela o disser), e «Não guardar» fecha já.
      d.remove();
      const u = new URL(location.href);
      u.searchParams.delete('close');
      history.replaceState(null, '', u.pathname + u.search);
      if (decision !== 'cancel') reconcile(r.data);
    }, () => location.reload());
  });

  /* O atalho do alternador (FG-028): Alt + W, a mesma tecla física em todos os
     teclados. Fora de campos de escrita, para nunca roubar uma letra. */
  document.addEventListener('keydown', (e) => {
    if (!e.altKey || e.ctrlKey || e.metaKey || e.code !== 'KeyW') return;
    const t = e.target;
    if (t && (t.isContentEditable || /^(INPUT|TEXTAREA|SELECT)$/.test(t.tagName))) return;
    e.preventDefault();
    remember();
    window.OcWm.openSwitcher();
  });

  /* Foco devolvido: ao fechar o alternador (Esc, fundo, escolha), o foco
     volta ao controlo que o tinha antes de ele abrir, e não fica num
     elemento escondido. O controlo anterior guarda-se no momento de abrir
     (atalho ou controlo do Design), e não por eventos de foco. */
  const sw = document.querySelector('[data-oc="switcher"]');
  let before = null;
  const remember = () => {
    const a = document.activeElement;
    if (sw && a && a !== document.body && !sw.contains(a)) before = a;
  };
  if (sw) {
    document.addEventListener('click', (e) => {
      if (e.target.closest('[data-oc="switcher-open"], a[href="#oc-switcher"]')) remember();
    }, true);
    new MutationObserver(() => {
      if (sw.hasAttribute('data-open')) return;
      const lost = !document.activeElement || document.activeElement === document.body || sw.contains(document.activeElement);
      if (lost && before && before.isConnected) before.focus();
    }).observe(sw, { attributes: true, attributeFilter: ['data-open'] });
  }

  /* Uma aplicação de uma janela lançada de novo: a janela que já estava aberta
     pisca uma vez e anuncia-se (HANDOFF «data-pulse»). */
  let pulse = null;
  try { pulse = sessionStorage.getItem('oc-wm-pulse'); sessionStorage.removeItem('oc-wm-pulse'); } catch (_) { pulse = null; }
  if (pulse && WIN.test(pulse) && winEl(pulse)) window.OcWm.pulse(pulse);

  /* D004 · O corpo das outras janelas (WindowContent::Loading). O servidor só
     desenha o da janela do pedido; as outras aplicações com ecrã pedem o seu
     por «?frame=1» — o endereço da janela, que o servidor guarda. Um corpo que
     não chega (erro, sessão acabada, recusa) não entra: a janela passa a ser
     uma ligação para o seu endereço, onde o servidor diz porquê. */
  const frameOf = (href) => href + (href.includes('?') ? '&' : '?') + 'frame=1';
  const wire = (body) => {
    /* O init do Design (oc-apps.js) liga todas as aplicações da página. As
       que já estão ligadas escondem-se durante a chamada, para não receberem
       os ouvintes duas vezes (CODE_FEEDBACK D004: pedir init(root)). */
    if (!window.OcApps || !body.querySelector('[data-oc="app"]')) return;
    const wired = Array.from(document.querySelectorAll('[data-oc="app"]')).filter((a) => !body.contains(a));
    wired.forEach((a) => a.setAttribute('data-oc', 'app-wired'));
    try { window.OcApps.init(); } finally { wired.forEach((a) => a.setAttribute('data-oc', 'app')); }
  };
  layer.querySelectorAll('[data-oc="win"]').forEach((w) => {
    const body = w.querySelector('[data-part="win-body"]');
    const href = w.dataset.href || '';
    if (!body || !body.querySelector('.oc-win__loading') || !href.startsWith('/') || href.startsWith('//')) return;
    const fallback = () => {
      const a = document.createElement('a');
      a.className = 'oc-win__state';
      a.href = href;
      const t = w.querySelector('.oc-win__title, [data-part="win-title"]');
      a.textContent = (t && t.textContent.trim()) || href;
      body.replaceChildren(a);
    };
    fetch(frameOf(href), { credentials: 'same-origin', headers: { Accept: 'text/html' }, redirect: 'manual' })
      .then(async (r) => {
        const html = r.ok && r.type !== 'opaqueredirect' ? await r.text() : '';
        /* Um corpo de janela, e não uma página inteira (um erro vem na casca). */
        if (!html || /<html[\s>]/i.test(html)) return fallback();
        const tpl = document.createElement('template');
        tpl.innerHTML = html;
        tpl.content.querySelectorAll('template[data-part="win-title"]').forEach((x) => x.remove());
        body.replaceChildren(tpl.content);
        wire(body);
      })
      .catch(fallback);
  });
})();
