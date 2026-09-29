/* Ocinye OS · D002 · janelas geridas (Claude Design). Só apresentação.
 *
 * O MOTOR NÃO ESTÁ AQUI. Ordem, foco, geometria, encaixe, persistência,
 * política de lançamento e autorização são do Claude Code. Este ficheiro:
 *   1. aplica a apresentação que o motor escreve em data-* (CSSOM, sem style inline);
 *   2. transforma gestos e teclas em intenções («oc:wm»), que o motor decide;
 *   3. dá os comportamentos locais de apresentação: alternador, escolha de
 *      janela, menu de contexto do Desktop, diálogo de alterações, pré-visualização
 *      do encaixe durante o arrastar.
 * Sem motor (nenhum ouvinte chama preventDefault), cada controlo cai no seu
 * fallback real: POST /wm/{id} ou a ligação da janela.
 *
 * Contrato das intenções (document, CustomEvent «oc:wm», cancelável):
 *   {op:'focus'|'minimize'|'maximize'|'restore'|'close'|'open', win?, app?, href?}
 *   {op:'move',   win, phase:'start'|'move'|'end', dx, dy, zone:null|'left'|'right'|'max'}
 *   {op:'resize', win, phase, edge, dx, dy}
 *   {op:'snap',   win, zone}
 * API para o motor: window.OcWm = { apply(el), snap(zone|null), size(el, w, h, atMin),
 *   pulse(win), announce(key), openSwitcher(), closeSwitcher() }. */
(() => {
  'use strict';
  const layer = document.querySelector('[data-oc="wm-layer"]');
  const $$ = (s, r = document) => Array.from(r.querySelectorAll(s));
  const emit = (detail) => document.dispatchEvent(new CustomEvent('oc:wm', { detail, cancelable: true }));
  const handled = (detail) => !emit(detail);
  const free = () => window.matchMedia('(min-width: 1100px)').matches;
  const strings = {};
  if (layer) {
    const tpl = layer.querySelector('template[data-part="wm-strings"]');
    if (tpl) Array.from(tpl.content.querySelectorAll('[data-key]')).forEach((s) => { strings[s.dataset.key] = s.textContent; });
  }

  /* 1 · apresentação a partir de data-* */
  const apply = (el) => {
    const px = (k) => (el.dataset[k] ? el.dataset[k] + 'px' : '');
    el.style.setProperty('--x', px('x'));
    el.style.setProperty('--y', px('y'));
    el.style.setProperty('--w', px('w'));
    el.style.setProperty('--h', px('h'));
    el.style.setProperty('--z', el.dataset.z || '1');
  };
  const wins = () => (layer ? $$('[data-oc="win"]', layer) : []);
  const snapEl = layer ? layer.querySelector('[data-part="snap-preview"]') : null;
  const live = layer ? layer.querySelector('[data-part="wm-live"]') : null;
  const snap = (zone) => {
    if (!snapEl) return;
    if (!zone) { snapEl.hidden = true; snapEl.removeAttribute('data-zone'); return; }
    snapEl.dataset.zone = zone; snapEl.hidden = false;
  };
  const size = (el, w, h, atMin) => {
    const s = el.querySelector('[data-part="win-size"]');
    if (s) s.textContent = w && h ? Math.round(w) + ' × ' + Math.round(h) : '';
    el.toggleAttribute('data-at-min', !!atMin);
  };
  const announce = (key) => { if (live && strings[key]) { live.textContent = ''; setTimeout(() => { live.textContent = strings[key]; }, 30); } };
  const pulse = (id) => {
    const el = layer && layer.querySelector('[data-win="' + CSS.escape(id) + '"][data-oc="win"]');
    if (!el) return;
    el.removeAttribute('data-pulse'); void el.offsetWidth; el.setAttribute('data-pulse', '');
    setTimeout(() => el.removeAttribute('data-pulse'), 700);
    announce('focused');
  };

  function presentation() {
    if (!layer) return;
    layer.setAttribute('data-js', '');
    wins().forEach(apply);
    new MutationObserver((ms) => ms.forEach((m) => {
      if (m.type === 'attributes' && m.target.matches('[data-oc="win"]')) apply(m.target);
      if (m.type === 'childList') m.addedNodes.forEach((n) => { if (n.nodeType === 1 && n.matches('[data-oc="win"]')) apply(n); });
    })).observe(layer, { subtree: true, childList: true, attributes: true, attributeFilter: ['data-x', 'data-y', 'data-w', 'data-h', 'data-z'] });
  }

  /* 2 · controlos e gestos → intenções */
  function controls() {
    document.addEventListener('click', (e) => {
      const b = e.target.closest('button[data-oc="wm"]');
      if (b) {
        const win = b.closest('[data-oc="win"]');
        const op = b.dataset.op;
        if (win && handled({ op, win: win.dataset.win })) e.preventDefault();
        return;
      }
      const f = e.target.closest('[data-oc="wm-focus"]');
      if (f) {
        closeChoosers();
        if (handled({ op: 'focus', win: f.dataset.win, href: f.getAttribute('href') })) { e.preventDefault(); closeSwitcher(); }
        return;
      }
      const o = e.target.closest('[data-oc="wm-open"]');
      if (o && handled({ op: 'open', app: o.dataset.app, href: o.getAttribute('href') })) { e.preventDefault(); closeChoosers(); }
    });
    if (!layer) return;
    layer.addEventListener('mousedown', (e) => {
      const win = e.target.closest('[data-oc="win"]');
      if (win && !win.hasAttribute('data-active')) emit({ op: 'focus', win: win.dataset.win });
    }, true);
    layer.addEventListener('dblclick', (e) => {
      const bar = e.target.closest('[data-part="win-drag"]');
      if (!bar || e.target.closest('button, a') || !free()) return;
      const win = bar.closest('[data-oc="win"]');
      emit({ op: win.dataset.state === 'normal' ? 'maximize' : 'restore', win: win.dataset.win });
    });
  }

  function gestures() {
    if (!layer) return;
    let g = null;
    const EDGE = 16;
    const zoneAt = (x, y) => {
      const r = layer.getBoundingClientRect();
      if (y - r.top < EDGE) return 'max';
      if (x - r.left < EDGE) return 'left';
      if (r.right - x < EDGE) return 'right';
      return null;
    };
    layer.addEventListener('pointerdown', (e) => {
      if (e.button !== 0 || !free()) return;
      const rz = e.target.closest('[data-part="win-resize"]');
      const bar = e.target.closest('[data-part="win-drag"]');
      if (!rz && (!bar || e.target.closest('button, a, input'))) return;
      const win = e.target.closest('[data-oc="win"]');
      if (!win) return;
      g = { win, kind: rz ? 'resize' : 'move', edge: rz ? rz.dataset.edge : null, x: e.clientX, y: e.clientY, id: e.pointerId, started: false };
      (rz || bar).setPointerCapture(e.pointerId);
    });
    layer.addEventListener('pointermove', (e) => {
      if (!g || e.pointerId !== g.id) return;
      const dx = e.clientX - g.x, dy = e.clientY - g.y;
      if (!g.started) {
        if (Math.abs(dx) + Math.abs(dy) < 4) return;
        g.started = true;
        g.win.setAttribute(g.kind === 'move' ? 'data-dragging' : 'data-resizing', '');
        emit({ op: g.kind, win: g.win.dataset.win, phase: 'start', edge: g.edge, dx: 0, dy: 0, zone: null });
      }
      const zone = g.kind === 'move' ? zoneAt(e.clientX, e.clientY) : null;
      if (g.kind === 'move' && zone !== g.zone) { g.zone = zone; snap(zone); if (zone) announce('snap-' + zone); }
      emit({ op: g.kind, win: g.win.dataset.win, phase: 'move', edge: g.edge, dx, dy, zone });
    });
    const end = (e) => {
      if (!g || e.pointerId !== g.id) return;
      if (g.started) {
        emit({ op: g.kind, win: g.win.dataset.win, phase: 'end', edge: g.edge, dx: e.clientX - g.x, dy: e.clientY - g.y, zone: g.zone || null });
        if (g.kind === 'move' && g.zone) emit({ op: 'snap', win: g.win.dataset.win, zone: g.zone });
      }
      g.win.removeAttribute('data-dragging'); g.win.removeAttribute('data-resizing'); size(g.win, 0, 0, false);
      snap(null); g = null;
    };
    layer.addEventListener('pointerup', end);
    layer.addEventListener('pointercancel', end);
  }

  /* 3a · alternador: setas, Enter, Esc. O atalho de abrir é do motor (openSwitcher). */
  const sw = document.querySelector('[data-oc="switcher"]');
  function openSwitcher() {
    if (!sw) return;
    sw.setAttribute('data-open', '');
    const items = $$('[data-part="switcher-item"]', sw);
    const start = items.findIndex((a) => a.getAttribute('aria-current') === 'true');
    const next = items[(start + 1) % Math.max(items.length, 1)] || items[0];
    if (next) next.focus();
  }
  function closeSwitcher() {
    if (!sw) return;
    sw.removeAttribute('data-open');
    if (location.hash === '#oc-switcher') history.replaceState(null, '', location.pathname + location.search);
  }
  function switcher() {
    if (!sw) return;
    $$('[data-oc="switcher-open"], a[href="#oc-switcher"]').forEach((a) => a.addEventListener('click', (e) => { e.preventDefault(); openSwitcher(); }));
    $$('a[href="#"]', sw).forEach((a) => a.addEventListener('click', (e) => { e.preventDefault(); closeSwitcher(); }));
    sw.addEventListener('keydown', (e) => {
      const items = $$('[data-part="switcher-item"]', sw);
      const i = items.indexOf(document.activeElement);
      const cols = Math.max(1, Math.round(sw.querySelector('[data-part="switcher-list"]')?.clientWidth / 190) || 1);
      const go = (n) => { e.preventDefault(); const t = items[(n + items.length) % items.length]; if (t) t.focus(); };
      if (e.key === 'ArrowRight' || (e.key === 'Tab' && !e.shiftKey)) go(i + 1);
      else if (e.key === 'ArrowLeft' || (e.key === 'Tab' && e.shiftKey)) go(i - 1);
      else if (e.key === 'ArrowDown') go(i + cols);
      else if (e.key === 'ArrowUp') go(i - cols);
      else if (e.key === 'Escape') { e.preventDefault(); closeSwitcher(); }
    });
    if (location.hash === '#oc-switcher') openSwitcher();
  }

  /* 3b · escolha da janela a partir da barra de aplicações */
  const choosers = () => $$('[data-oc="chooser"]');
  function closeChoosers() { choosers().forEach((c) => { c.hidden = true; }); }
  function dockApps() {
    $$('[data-oc="dock-app"]').forEach((a) => a.addEventListener('click', (e) => {
      const n = Number(a.dataset.windows || '0');
      if (n > 1) {
        const c = document.querySelector('[data-oc="chooser"][data-app="' + CSS.escape(a.dataset.app) + '"]');
        if (!c) return;
        e.preventDefault();
        const open = c.hidden;
        closeChoosers();
        if (open) {
          const r = a.getBoundingClientRect(), p = c.offsetParent ? c.offsetParent.getBoundingClientRect() : { top: 0 };
          c.style.setProperty('--chooser-y', Math.max(8, r.top - p.top - 8) + 'px');
          c.hidden = false;
          const first = c.querySelector('a'); if (first) first.focus();
        }
      } else if (n === 1) {
        const w = layer && layer.querySelector('[data-oc="win"][data-app="' + CSS.escape(a.dataset.app) + '"]');
        if (w && handled({ op: 'focus', win: w.dataset.win, href: a.getAttribute('href') })) e.preventDefault();
      }
    }));
    document.addEventListener('click', (e) => { if (!e.target.closest('[data-oc="chooser"], [data-oc="dock-app"]')) closeChoosers(); });
    document.addEventListener('keydown', (e) => { if (e.key === 'Escape') closeChoosers(); });
  }

  /* 3c · menu de contexto do Desktop: só no fundo do Desktop */
  function deskMenu() {
    const menu = document.querySelector('[data-oc="desk-ctx"]');
    const main = document.getElementById('oc-main');
    if (!menu || !main) return;
    const items = () => $$('[role="menuitem"]', menu);
    const close = () => { menu.hidden = true; };
    const open = (x, y) => {
      menu.hidden = false;
      const w = menu.offsetWidth, h = menu.offsetHeight;
      menu.style.setProperty('--ctx-x', Math.min(x, window.innerWidth - w - 8) + 'px');
      menu.style.setProperty('--ctx-y', Math.min(y, window.innerHeight - h - 8) + 'px');
      const f = items()[0]; if (f) f.focus();
    };
    const onDesk = (t) => main.contains(t) && !t.closest('.oc-dw, .oc-win, .oc-editbar, .oc-sheet, .oc-desk-pencil, .oc-toast, [data-oc="desk-ctx"], dialog, a, button, input');
    main.addEventListener('contextmenu', (e) => { if (!onDesk(e.target)) return; e.preventDefault(); open(e.clientX, e.clientY); });
    main.addEventListener('keydown', (e) => {
      if ((e.key === 'ContextMenu' || (e.shiftKey && e.key === 'F10')) && onDesk(e.target)) { e.preventDefault(); const r = main.getBoundingClientRect(); open(r.left + 40, r.top + 40); }
    });
    menu.addEventListener('keydown', (e) => {
      const list = items(), i = list.indexOf(document.activeElement);
      if (e.key === 'ArrowDown') { e.preventDefault(); list[(i + 1) % list.length].focus(); }
      else if (e.key === 'ArrowUp') { e.preventDefault(); list[(i - 1 + list.length) % list.length].focus(); }
      else if (e.key === 'Escape') { e.preventDefault(); close(); main.focus(); }
    });
    menu.addEventListener('click', (e) => {
      const b = e.target.closest('[data-oc="ctx-item"]');
      if (!b) return;
      close();
      const target = document.querySelector('[data-oc="' + CSS.escape(b.dataset.proxy) + '"]:not([data-oc="ctx-item"])');
      if (target) target.click();
    });
    document.addEventListener('click', (e) => { if (!menu.contains(e.target)) close(); });
    window.addEventListener('blur', close);
    if (!main.hasAttribute('tabindex')) main.setAttribute('tabindex', '-1');
  }

  /* 3d · diálogo de alterações: foco preso no diálogo, Esc = Cancelar */
  function dirty() {
    const d = document.querySelector('[data-oc="dirty-close"]');
    if (!d) return;
    const focusables = () => $$('button:not([aria-disabled="true"]), [href]', d);
    const save = d.querySelector('[data-oc="dirty-save"]') || focusables()[0];
    if (save) save.focus();
    d.addEventListener('keydown', (e) => {
      if (e.key === 'Escape') { e.preventDefault(); const c = d.querySelector('[data-oc="dirty-cancel"]'); if (c) c.click(); }
      if (e.key !== 'Tab') return;
      const f = focusables(); if (!f.length) return;
      const first = f[0], last = f[f.length - 1];
      if (e.shiftKey && document.activeElement === first) { e.preventDefault(); last.focus(); }
      else if (!e.shiftKey && document.activeElement === last) { e.preventDefault(); first.focus(); }
    });
  }

  window.OcWm = { apply, snap, size, pulse, announce, openSwitcher, closeSwitcher };
  const init = () => { presentation(); controls(); gestures(); switcher(); dockApps(); deskMenu(); dirty(); };
  if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', init);
  else init();
})();
