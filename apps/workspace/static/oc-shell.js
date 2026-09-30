/* Ocinye OS · casca (Claude Design). Só apresentação.
 * - data-oc="menu": fecha os <details> ao clicar fora e com Esc; um aberto de cada vez.
 * - data-oc="launcher" / "palette": abre com [data-open] (⌘J / ⌘K ou a âncora),
 *   filtra por data-search, fecha com Esc. Sem JS abrem por :target.
 * - data-oc="dock" / "dock-toggle": esconde a barra e mostra o botão para a trazer.
 *   Preferência só visual, guardada em sessionStorage (não é dado do membro). */
(() => {
  'use strict';
  const $$ = (s, r = document) => Array.from(r.querySelectorAll(s));

  function menus() {
    const all = $$('details[data-oc="menu"]');
    document.addEventListener('click', (e) => {
      all.forEach((d) => { if (d.open && !d.contains(e.target)) d.open = false; });
    });
    all.forEach((d) => d.addEventListener('toggle', () => {
      if (d.open) all.forEach((o) => { if (o !== d) o.open = false; });
    }));
    document.addEventListener('keydown', (e) => {
      if (e.key === 'Escape') all.forEach((d) => { d.open = false; });
    });
  }

  function overlay(id, inputPart, itemPart, emptyPart) {
    const el = document.querySelector('[data-oc="' + id + '"]');
    if (!el) return null;
    const q = el.querySelector('[data-part="' + inputPart + '"]');
    const items = $$('[data-part="' + itemPart + '"]', el);
    const empty = emptyPart ? el.querySelector('[data-part="' + emptyPart + '"]') : null;
    const filter = () => {
      const v = (q.value || '').trim().toLowerCase();
      let n = 0;
      items.forEach((it) => { const hit = !v || (it.dataset.search || '').includes(v); it.hidden = !hit; if (hit) n += 1; });
      if (empty) empty.hidden = n > 0;
    };
    // Code (D009 · teclado): fechar devolve o foco a quem abriu, em vez de o largar no <body>.
    let opener = null;
    const open = () => { if (!el.hasAttribute('data-open')) opener = document.activeElement; el.setAttribute('data-open', ''); q.value = ''; filter(); setTimeout(() => q.focus(), 0); };
    const close = () => {
      const was = el.hasAttribute('data-open');
      el.removeAttribute('data-open');
      if (location.hash === '#' + el.id) history.replaceState(null, '', location.pathname + location.search);
      if (was && opener && opener.isConnected && el.contains(document.activeElement)) opener.focus();
      if (was) opener = null;
    };
    q.addEventListener('input', filter);
    $$('a[href="#"]', el).forEach((a) => a.addEventListener('click', (e) => { e.preventDefault(); close(); }));
    return { el, open, close };
  }

  function overlays() {
    const launcher = overlay('launcher', 'launcher-q', 'launcher-item', 'launcher-empty');
    const palette = overlay('palette', 'palette-q', 'palette-item');
    $$('[data-oc="launcher-open"]').forEach((a) => a.addEventListener('click', (e) => { if (launcher) { e.preventDefault(); launcher.open(); } }));
    document.addEventListener('keydown', (e) => {
      const k = (e.key || '').toLowerCase();
      if ((e.metaKey || e.ctrlKey) && k === 'k' && palette) { e.preventDefault(); palette.open(); }
      else if ((e.metaKey || e.ctrlKey) && k === 'j' && launcher) { e.preventDefault(); launcher.open(); }
      else if (k === 'escape') { if (launcher) launcher.close(); if (palette) palette.close(); }
    });
    if (location.hash === '#oc-launcher' && launcher) launcher.open();
    if (location.hash === '#oc-palette' && palette) palette.open();
  }

  function dock() {
    const desk = document.querySelector('.oc-desk');
    const toggle = document.querySelector('[data-oc="dock-toggle"]');
    const bar = document.querySelector('[data-oc="dock"]');
    if (!desk || !toggle || !bar) return;
    const set = (hidden) => {
      desk.setAttribute('data-dock', hidden ? 'hidden' : 'shown');
      toggle.hidden = !hidden;
      try { sessionStorage.setItem('oc.dock', hidden ? 'hidden' : 'shown'); } catch (_) { /* sem armazenamento */ }
    };
    let pref = 'shown';
    try { pref = sessionStorage.getItem('oc.dock') || 'shown'; } catch (_) { /* sem armazenamento */ }
    set(pref === 'hidden');
    toggle.addEventListener('click', () => set(false));
    bar.addEventListener('dblclick', () => set(true));
  }

  /* data-oc="pin" data-app: fixa/desafixa com PUT /apps/pins {"pinned": [...]} e recarrega. */
  function pins() {
    const buttons = $$('[data-oc="pin"]');
    if (!buttons.length) return;
    buttons.forEach((b) => b.addEventListener('click', async () => {
      const on = b.getAttribute('aria-pressed') === 'true';
      b.setAttribute('aria-pressed', String(!on));
      const list = buttons.filter((x) => x.getAttribute('aria-pressed') === 'true').map((x) => x.dataset.app);
      try {
        const r = await fetch('/apps/pins', { method: 'PUT', credentials: 'same-origin', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ pinned: list }) });
        if (!r.ok) throw new Error(String(r.status));
        location.reload();
      } catch (_) {
        b.setAttribute('aria-pressed', String(on));
        b.setAttribute('data-failed', '');
        setTimeout(() => b.removeAttribute('data-failed'), 2000);
      }
    }));
  }

  const init = () => { menus(); overlays(); dock(); pins(); };
  if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', init);
  else init();
})();
