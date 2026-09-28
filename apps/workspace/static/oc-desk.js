/* Ocinye OS · Desktop (Claude Design). Só apresentação e o PUT da disposição;
 * o Core valida tudo (tamanhos, obrigatórios, política) e é quem guarda.
 *
 * data-oc="desk"            raiz; data-version = versão para o PUT
 * data-oc="desk-edit"       entra/sai de «Personalizar»
 * data-oc="desk-edit-done"  sai
 * data-oc="dw-min"         recolher/expandir (sempre disponível)
 * data-oc="dw-left|dw-right|dw-resize|dw-remove"  ferramentas de cada widget
 * data-oc="desk-lib-open|desk-bg-open|desk-restore-open"  abrem as folhas
 * data-oc="lib-cat|lib-add" biblioteca; data-oc="bg-wall|bg-dim" fundo
 * data-oc="desk-restore-form"  POST /me/desktop/restore (com JS guarda o anterior para «Anular»)
 * data-oc="desk-undo"       repõe a disposição anterior com PUT
 * data-oc="dialog-close"    fecha a folha
 *
 * PUT /me/desktop {version, wallpaper, fit, dim, widgets:[{id, kind, w, h, minimized}]}
 * → 200 {version, …} · 409 conflito (outra sessão) · 4xx recusa do Core. */
(() => {
  'use strict';
  const root = document.querySelector('[data-oc="desk"]');
  if (!root) return;
  const $ = (s, r = root) => r.querySelector(s);
  const $$ = (s, r = root) => Array.from(r.querySelectorAll(s));
  const grid = $('[data-part="desk-grid"]');
  const bar = $('[data-part="desk-editbar"]');
  const status = $('[data-part="desk-status"]');
  const desk = document.querySelector('.oc-desk');
  const strings = {};
  const tpl = $('template[data-part="desk-strings"]');
  if (tpl) Array.from(tpl.content.querySelectorAll('[data-key]')).forEach((s) => { strings[s.dataset.key] = s.textContent; });
  const UNDO_KEY = 'oc.desk.undo';

  const look = () => ({ wallpaper: desk ? desk.getAttribute('data-wall') : 'ocinye', dim: desk ? Number(desk.getAttribute('data-dim')) : 20 });
  const layout = () => Object.assign({
    version: Number(root.dataset.version || 0),
    fit: 'fill',
    widgets: $$('[data-part="desk-widget"]', grid).map((el) => ({ id: el.dataset.id, kind: el.dataset.kind, w: Number(el.dataset.w), h: Number(el.dataset.h), minimized: el.hasAttribute('data-min') })),
  }, look());

  const say = (key, tone) => {
    if (!status) return;
    status.textContent = strings[key] || '';
    if (tone) status.setAttribute('data-tone', tone); else status.removeAttribute('data-tone');
  };

  let timer = null;
  let pendingReload = false;
  async function put(body) {
    say('saving');
    try {
      const r = await fetch('/me/desktop', { method: 'PUT', credentials: 'same-origin', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(body) });
      if (r.status === 409) { say('conflict', 'err'); return false; }
      if (!r.ok) throw new Error(String(r.status));
      const saved = await r.json().catch(() => null);
      if (saved && typeof saved.version === 'number') root.dataset.version = String(saved.version);
      say('saved');
      if (pendingReload) location.reload();
      return true;
    } catch (_) {
      say('failed', 'err');
      return false;
    }
  }
  const save = (reload) => {
    if (reload) pendingReload = true;
    clearTimeout(timer);
    timer = setTimeout(() => put(layout()), reload ? 0 : 450);
  };

  /* edição */
  const editBtn = $('[data-oc="desk-edit"]');
  const setEditing = (on) => {
    if (on) root.setAttribute('data-editing', ''); else root.removeAttribute('data-editing');
    if (bar) bar.hidden = !on;
    if (editBtn) editBtn.setAttribute('aria-pressed', String(on));
    $$('[data-part="desk-widget"]', grid).forEach((el) => { el.draggable = on; });
    if (on && bar) { const first = bar.querySelector('button'); if (first) first.focus(); }
  };
  if (editBtn) editBtn.addEventListener('click', () => setEditing(!root.hasAttribute('data-editing')));
  root.addEventListener('keydown', (e) => { if (e.key === 'Escape' && root.hasAttribute('data-editing') && !document.querySelector('dialog[open]')) setEditing(false); });

  const setSize = (el, w, h) => {
    el.classList.remove('oc-dw--c1', 'oc-dw--c2', 'oc-dw--c3', 'oc-dw--c4', 'oc-dw--r1', 'oc-dw--r2');
    el.classList.add('oc-dw--c' + w, 'oc-dw--r' + h);
    el.dataset.w = String(w);
    el.dataset.h = String(h);
  };

  root.addEventListener('click', (e) => {
    const b = e.target.closest('[data-oc]');
    if (!b || !root.contains(b)) return;
    const op = b.dataset.oc;
    const el = b.closest('[data-part="desk-widget"]');
    if (op === 'desk-edit-done') { setEditing(false); if (editBtn) editBtn.focus(); }
    else if (op === 'dw-min' && el) {
      const min = !el.hasAttribute('data-min');
      if (min) el.setAttribute('data-min', ''); else el.removeAttribute('data-min');
      b.setAttribute('aria-expanded', String(!min));
      b.setAttribute('aria-label', min ? b.dataset.labelExpand : b.dataset.labelCollapse);
      const use = b.querySelector('use');
      if (use) use.setAttribute('href', '/static/icons.svg#' + (min ? 'chev-d' : 'chev-u'));
      save();
    }
    else if (op === 'dw-left' && el && el.previousElementSibling) { grid.insertBefore(el, el.previousElementSibling); b.focus(); save(); }
    else if (op === 'dw-right' && el && el.nextElementSibling) { grid.insertBefore(el.nextElementSibling, el); b.focus(); save(); }
    else if (op === 'dw-resize' && el) {
      const sizes = (el.dataset.sizes || '').split(',').filter(Boolean);
      const i = sizes.indexOf(el.dataset.w + 'x' + el.dataset.h);
      const [w, h] = (sizes[(i + 1) % sizes.length] || '1x1').split('x').map(Number);
      setSize(el, w, h);
      save();
    } else if (op === 'dw-remove' && el && !el.hasAttribute('data-mandatory')) {
      const next = el.nextElementSibling || el.previousElementSibling;
      const add = $('[data-oc="lib-add"][data-kind="' + el.dataset.kind + '"]');
      if (add) add.setAttribute('aria-pressed', 'false');
      el.remove();
      say('removed');
      if (next) { const t = next.querySelector('[data-oc="dw-remove"],[data-oc="dw-right"]'); if (t) t.focus(); }
      save();
    } else if (op === 'desk-lib-open') open('desk-library');
    else if (op === 'desk-bg-open') open('desk-background');
    else if (op === 'desk-restore-open') open('desk-restore');
    else if (op === 'dialog-close') { const d = b.closest('dialog'); if (d) d.close(); }
    else if (op === 'lib-cat') filterLib(b);
    else if (op === 'lib-add' && b.getAttribute('aria-pressed') !== 'true') addWidget(b);
    else if (op === 'bg-wall') setWall(b);
    else if (op === 'desk-undo') undo();
  });

  /* arrastar (rato); o teclado usa dw-left/dw-right */
  let dragging = null;
  grid.addEventListener('dragstart', (e) => {
    const el = e.target.closest('[data-part="desk-widget"]');
    if (!el || !root.hasAttribute('data-editing')) { e.preventDefault(); return; }
    dragging = el;
    el.setAttribute('data-dragging', '');
    e.dataTransfer.effectAllowed = 'move';
    try { e.dataTransfer.setData('text/plain', el.dataset.id); } catch (_) { /* Safari */ }
  });
  grid.addEventListener('dragover', (e) => {
    if (!dragging) return;
    e.preventDefault();
    const over = e.target.closest('[data-part="desk-widget"]');
    $$('[data-over]', grid).forEach((x) => { if (x !== over) x.removeAttribute('data-over'); });
    if (over && over !== dragging) over.setAttribute('data-over', '');
  });
  grid.addEventListener('drop', (e) => {
    if (!dragging) return;
    e.preventDefault();
    const over = e.target.closest('[data-part="desk-widget"]');
    if (over && over !== dragging) {
      const items = $$('[data-part="desk-widget"]', grid);
      grid.insertBefore(dragging, items.indexOf(dragging) < items.indexOf(over) ? over.nextElementSibling : over);
      save();
    }
  });
  grid.addEventListener('dragend', () => {
    if (dragging) dragging.removeAttribute('data-dragging');
    $$('[data-over]', grid).forEach((x) => x.removeAttribute('data-over'));
    dragging = null;
  });

  /* folhas */
  function open(part) {
    const d = $('dialog[data-part="' + part + '"]');
    if (d && typeof d.showModal === 'function') { d.showModal(); const q = d.querySelector('input[type="search"]'); if (q) q.focus(); }
  }
  $$('dialog', root).forEach((d) => d.addEventListener('click', (e) => { if (e.target === d) d.close(); }));

  /* biblioteca */
  const libQ = $('[data-part="lib-q"]');
  let libCat = 'all';
  function filterLib(btn) {
    if (btn) {
      libCat = btn.dataset.cat;
      $$('[data-oc="lib-cat"]').forEach((c) => c.setAttribute('aria-pressed', String(c === btn)));
    }
    const q = libQ ? libQ.value.trim().toLowerCase() : '';
    let n = 0;
    $$('[data-part="lib-item"]').forEach((it) => {
      const hit = (libCat === 'all' || it.dataset.cat === libCat) && (!q || (it.dataset.search || '').includes(q));
      it.hidden = !hit;
      if (hit) n += 1;
    });
    const empty = $('[data-part="lib-empty"]');
    if (empty) empty.hidden = n > 0;
  }
  if (libQ) libQ.addEventListener('input', () => filterLib(null));
  function addWidget(b) {
    // O conteúdo vem do servidor: acrescenta à disposição, grava e recarrega.
    const body = layout();
    body.widgets.push({ id: b.dataset.kind, kind: b.dataset.kind, w: Number(b.dataset.w), h: Number(b.dataset.h), minimized: false });
    b.setAttribute('aria-pressed', 'true');
    pendingReload = true;
    clearTimeout(timer);
    put(body);
  }

  /* fundo */
  function setWall(b) {
    if (!desk) return;
    desk.setAttribute('data-wall', b.dataset.wall);
    $$('[data-oc="bg-wall"]').forEach((x) => x.setAttribute('aria-pressed', String(x === b)));
    save();
  }
  const dim = $('[data-oc="bg-dim"]');
  const dimOut = $('[data-part="bg-dim-out"]');
  if (dim && desk) {
    dim.addEventListener('input', () => {
      const v = Math.round(Number(dim.value) / 5) * 5;
      desk.style.setProperty('--oc-dim', String(v / 100));
      desk.setAttribute('data-dim', String(v));
      if (dimOut) dimOut.textContent = v + ' %';
    });
    dim.addEventListener('change', () => save());
  }

  /* repor com «Anular» */
  const form = $('[data-oc="desk-restore-form"]');
  if (form) form.addEventListener('submit', async (e) => {
    e.preventDefault();
    const before = layout();
    try {
      const r = await fetch(form.action, { method: 'POST', credentials: 'same-origin', headers: { Accept: 'application/json' } });
      if (!r.ok) throw new Error(String(r.status));
      try { sessionStorage.setItem(UNDO_KEY, JSON.stringify(before)); } catch (_) { /* sem armazenamento */ }
      location.reload();
    } catch (_) {
      form.submit();
    }
  });
  if (location.hash === '#appearance') { history.replaceState(null, '', location.pathname + location.search); open('desk-background'); }
  document.querySelectorAll('[data-oc="appearance-open"]').forEach((a) => a.addEventListener('click', (e) => {
    if (!$('dialog[data-part="desk-background"]')) return;
    e.preventDefault();
    const d = a.closest('details'); if (d) d.open = false;
    open('desk-background');
  }));
  const toast = $('[data-part="desk-toast"]');
  let undoState = null;
  try { undoState = JSON.parse(sessionStorage.getItem(UNDO_KEY) || 'null'); sessionStorage.removeItem(UNDO_KEY); } catch (_) { undoState = null; }
  if (undoState && toast) {
    const text = $('[data-part="desk-toast-text"]');
    if (text) text.textContent = strings.restored || '';
    toast.hidden = false;
    setTimeout(() => { toast.hidden = true; }, 10000);
  }
  async function undo() {
    if (!undoState) return;
    const body = Object.assign({}, undoState, { version: Number(root.dataset.version || 0) });
    pendingReload = true;
    if (toast) toast.hidden = true;
    await put(body);
  }
})();
