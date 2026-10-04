/* Ocinye OS · D004 · aplicações de produtividade (Claude Design). Só apresentação.
 *
 * O MOTOR NÃO ESTÁ AQUI: envio multipart, hash incremental, mover, gravar,
 * enviar correio e autorização são do Code/Core. Este ficheiro:
 *   1. gavetas (navegação lateral em janelas estreitas);
 *   2. estado sujo dos documentos [data-oc="app-doc"] → data-dirty na janela;
 *      fechar uma janela suja monta o diálogo D002 (wm::dirty_close, que o
 *      servidor entrega em <template data-part="app-dirty" data-win>) no sítio
 *      global de sempre e liga-o com OcWm.bindDirty;
 *   3. barra de formatação (Markdown restrito) nos textarea;
 *   4. Ficheiros: selecção (caixa, Espaço, Shift+↑/↓), barra de selecção,
 *      ↑/↓ entre itens, «Nova pasta», arrastar: ficheiros de fora → evento
 *      oc:files-upload; item do Ocinye sobre pasta → evento oc:files-move;
 *      o selector «Enviar» emite o mesmo oc:files-upload (arrastar nunca é o
 *      único caminho); cancelar/tentar de novo → oc:files-upload-cancel|retry;
 *   5. Calendário: posições das semanas/dias a partir de data-* (CSP: nada
 *      inline no HTML; aqui é CSSOM), rolagem inicial para a hora actual.
 * D006: credencial temporária (Mostrar/Copiar, sem guardar) e a confirmação
 * partilhada das acções privilegiadas (foco em Cancelar, Tab contido, Esc).
 * Eventos: CustomEvent em document, cancelável, com detail tipado. Sem
 * localStorage. */
(() => {
  'use strict';
  const $$ = (s, r = document) => Array.from(r.querySelectorAll(s));
  const emit = (name, detail) => document.dispatchEvent(new CustomEvent(name, { detail, cancelable: true }));
  const say = (app, text) => { const l = app.querySelector('[data-part="app-live"]'); if (l) { l.textContent = ''; setTimeout(() => { l.textContent = text; }, 30); } };

  function drawer(app) {
    const btn = app.querySelector('[data-oc="app-drawer"]');
    const side = app.querySelector('[data-part="app-side"]');
    if (!btn || !side) return;
    let back = null;
    const open = () => { back = document.activeElement; side.setAttribute('data-open', ''); btn.setAttribute('aria-expanded', 'true'); const f = side.querySelector('a[href], button, input'); if (f) f.focus(); };
    const close = () => { if (!side.hasAttribute('data-open')) return; side.removeAttribute('data-open'); btn.setAttribute('aria-expanded', 'false'); if (back && back.focus) back.focus(); };
    btn.addEventListener('click', () => (side.hasAttribute('data-open') ? close() : open()));
    app.addEventListener('keydown', (e) => { if (e.key === 'Escape' && side.hasAttribute('data-open')) { e.stopPropagation(); close(); } });
    app.addEventListener('click', (e) => { if (side.hasAttribute('data-open') && !side.contains(e.target) && !btn.contains(e.target)) close(); });
  }

  /* 2 · documentos e fecho com alterações (D002) */
  function docs(app) {
    const win = app.closest('[data-oc="win"]');
    $$('[data-oc="app-doc"]', app).forEach((form) => {
      const mark = () => {
        if (form.dataset.state === 'dirty') return;
        form.dataset.state = 'dirty';
        if (win) win.setAttribute('data-dirty', '');
        // O servidor tem de saber que esta janela tem trabalho por gravar:
        // fechá-la, mudar de Distribuição ou fechar tudo passam pelo diálogo
        // do D002 só se ele souber (A001-H001). Uma vez por estado sujo.
        const id = win && win.dataset.win;
        if (id && /^w[0-9]+$/.test(id)) {
          fetch('/wm/' + encodeURIComponent(id) + '/state', {
            method: 'POST',
            credentials: 'same-origin',
            headers: { 'Content-Type': 'application/x-www-form-urlencoded', Accept: 'application/json' },
            // `can_save=false`: guardar a partir do diálogo não chega à aplicação
            // (nada lho pede); o diálogo oferece Não guardar ou Cancelar, e a
            // pessoa grava na janela, onde o botão funciona.
            body: 'dirty=true&can_save=false',
          }).catch(() => {});
        }
      };
      if (form.dataset.state === 'dirty' && win) win.setAttribute('data-dirty', '');
      form.addEventListener('input', mark);
      form.addEventListener('submit', () => {
        form.dataset.state = 'saving';
        if (win) win.removeAttribute('data-dirty');
        const s = form.querySelector('.oc-app-save');
        if (s && form.dataset.savingText) { s.dataset.state = 'saving'; s.lastElementChild.textContent = form.dataset.savingText; }
      });
    });
  }
  document.addEventListener('click', (e) => {
    const closeBtn = e.target.closest('[data-oc="wm"][data-op="close"]');
    if (!closeBtn) return;
    const win = closeBtn.closest('[data-oc="win"]');
    if (!win || !win.hasAttribute('data-dirty')) return;
    const tpl = document.querySelector('template[data-part="app-dirty"][data-win="' + CSS.escape(win.dataset.win || '') + '"]');
    if (!tpl) return; // sem modelo: o gestor decide no servidor, como na D002
    e.preventDefault();
    e.stopImmediatePropagation();
    const dlg = tpl.content.firstElementChild.cloneNode(true);
    const shell = document.querySelector('.oc-shell');
    (shell && shell.parentNode ? shell.parentNode : document.body).insertBefore(dlg, shell ? shell.nextSibling : null);
    dlg.addEventListener('click', (ev) => {
      if (ev.target.closest('[data-oc="dirty-cancel"]')) { ev.preventDefault(); dlg.remove(); if (closeBtn.focus) closeBtn.focus(); }
    });
    if (window.OcWm && window.OcWm.bindDirty) window.OcWm.bindDirty(dlg);
  }, true);

  /* 3 · formatação: insere sintaxe; nunca HTML */
  const MD = { b: ['**', '**'], i: ['_', '_'], code: ['`', '`'], a: ['[', '](https://)'], h: ['\n## ', ''], ul: ['\n- ', ''], q: ['\n> ', ''] };
  document.addEventListener('click', (e) => {
    const b = e.target.closest('[data-oc="md"]');
    if (!b) return;
    const scope = b.closest('form') || document;
    const ta = scope.querySelector('textarea[data-part="notes-body"]');
    const m = MD[b.dataset.md];
    if (!ta || !m || ta.readOnly) return;
    const s = ta.selectionStart, en = ta.selectionEnd, v = ta.value;
    ta.value = v.slice(0, s) + m[0] + v.slice(s, en) + m[1] + v.slice(en);
    ta.focus();
    ta.setSelectionRange(s + m[0].length, en + m[0].length);
    ta.dispatchEvent(new Event('input', { bubbles: true }));
  });

  /* 4 · Ficheiros */
  function files(app) {
    const bar = app.querySelector('[data-part="files-selbar"]');
    const n = app.querySelector('[data-part="files-sel-n"]');
    const checks = () => $$('[data-part="files-check"]', app);
    const sync = () => {
      const on = checks().filter((c) => c.checked);
      checks().forEach((c) => { const it = c.closest('[data-part="files-item"]'); if (it) it.setAttribute('aria-selected', c.checked ? 'true' : 'false'); });
      if (bar) {
        bar.hidden = on.length === 0;
        $$('input[type="hidden"][name="item"]', bar).forEach((x) => x.remove());
        on.forEach((c) => { const h = document.createElement('input'); h.type = 'hidden'; h.name = 'item'; h.value = c.value; bar.appendChild(h); });
      }
      if (n && n.dataset.tpl) n.textContent = n.dataset.tpl.replace('{n}', String(on.length));
      const all = app.querySelector('[data-oc="files-check-all"]');
      if (all) { all.checked = on.length > 0 && on.length === checks().length; all.indeterminate = on.length > 0 && on.length < checks().length; }
    };
    if (n && !n.dataset.tpl) n.dataset.tpl = n.textContent.replace(/\d+/, '{n}');
    app.addEventListener('change', (e) => {
      if (e.target.matches('[data-oc="files-check-all"]')) checks().forEach((c) => { c.checked = e.target.checked; });
      if (e.target.matches('[data-part="files-check"], [data-oc="files-check-all"]')) { sync(); say(app, n ? n.textContent : ''); }
    });
    const clear = app.querySelector('[data-oc="files-sel-clear"]');
    if (clear) clear.addEventListener('click', () => { checks().forEach((c) => { c.checked = false; }); sync(); });
    // ↑/↓ entre itens; Espaço selecciona; Shift+↑/↓ alarga a selecção. Enter abre (ligação).
    const links = () => $$('[data-part="files-open"]', app).filter((a) => a.getClientRects().length);
    app.addEventListener('keydown', (e) => {
      const a = e.target.closest('[data-part="files-open"]');
      if (!a) return;
      const l = links(), i = l.indexOf(a);
      const box = (el) => el.closest('[data-part="files-item"]').querySelector('[data-part="files-check"]');
      if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
        e.preventDefault();
        const next = l[i + (e.key === 'ArrowDown' ? 1 : -1)];
        if (!next) return;
        if (e.shiftKey) { box(a).checked = true; box(next).checked = true; sync(); }
        next.focus();
      } else if (e.key === ' ') {
        e.preventDefault(); const c = box(a); c.checked = !c.checked; sync();
      }
    });
    const nf = app.querySelector('[data-oc="files-new-folder"]');
    if (nf) nf.addEventListener('click', () => {
      const inp = nf.form.querySelector('[data-part="files-nf"]');
      if (inp.hidden) { inp.hidden = false; inp.focus(); } else if (inp.value.trim()) nf.form.requestSubmit(); else inp.focus();
    });
    const up = app.querySelector('[data-oc="files-upload"]');
    if (up) up.addEventListener('change', () => { if (up.files.length) emit('oc:files-upload', { files: Array.from(up.files), folder: up.dataset.folder || null }); up.value = ''; });
    app.addEventListener('click', (e) => {
      const b = e.target.closest('[data-oc^="files-up-"]');
      if (b) emit('oc:files-' + b.dataset.oc.slice(6), { id: b.dataset.id });
    });
    // Arrastar: de fora = enviar para esta pasta; um item do Ocinye sobre uma pasta = mover.
    const drop = app.querySelector('[data-part="files-drop"]');
    if (!drop) return;
    let dragId = null;
    drop.addEventListener('dragstart', (e) => { const a = e.target.closest('[data-part="files-open"][draggable]'); if (a) { dragId = a.dataset.id; e.dataTransfer.effectAllowed = 'move'; } });
    drop.addEventListener('dragend', () => { dragId = null; $$('[data-drop-target]', drop).forEach((x) => x.removeAttribute('data-drop-target')); });
    drop.addEventListener('dragover', (e) => {
      const external = !dragId && e.dataTransfer && Array.from(e.dataTransfer.types || []).includes('Files');
      const folder = e.target.closest('[data-part="files-item"][data-kind="folder"]');
      if (external && drop.dataset.folder) { e.preventDefault(); drop.setAttribute('data-drag', ''); }
      else if (dragId && folder) { e.preventDefault(); $$('[data-drop-target]', drop).forEach((x) => x.removeAttribute('data-drop-target')); folder.setAttribute('data-drop-target', ''); }
    });
    drop.addEventListener('dragleave', (e) => { if (!drop.contains(e.relatedTarget)) drop.removeAttribute('data-drag'); });
    drop.addEventListener('drop', (e) => {
      drop.removeAttribute('data-drag');
      const folder = e.target.closest('[data-part="files-item"][data-kind="folder"]');
      if (dragId && folder) { e.preventDefault(); emit('oc:files-move', { id: dragId, target: folder.querySelector('[data-part="files-open"]').dataset.id }); }
      else if (!dragId && e.dataTransfer.files.length && drop.dataset.folder) { e.preventDefault(); emit('oc:files-upload', { files: Array.from(e.dataTransfer.files), folder: drop.dataset.folder }); }
      dragId = null;
    });
  }

  /* 5 · Calendário: posições a partir de data-* */
  function calendar(app) {
    const tl = app.querySelector('[data-part="cal-tl"]');
    if (!tl) return;
    const h0 = Number(tl.dataset.h0 || 0), h1 = Number(tl.dataset.h1 || 24);
    const hour = parseFloat(getComputedStyle(tl).getPropertyValue('--oc-cal-hour')) || 44;
    tl.style.setProperty('--oc-cal-cols', tl.dataset.cols || '7');
    const px = (min) => ((Math.max(min, h0 * 60) - h0 * 60) / 60) * hour;
    $$('.oc-cal-tl__col', tl).forEach((c) => c.style.setProperty('min-height', ((h1 - h0) * hour) + 'px'));
    $$('[data-part="cal-block"]', tl).forEach((b) => {
      const s = Number(b.dataset.start), d = Number(b.dataset.dur);
      b.style.setProperty('--oc-cal-top', px(s) + 'px');
      b.style.setProperty('--oc-cal-h', Math.max(22, (d / 60) * hour - 2) + 'px');
      b.style.setProperty('--oc-cal-lane', b.dataset.lane || '0');
      b.style.setProperty('--oc-cal-lanes', b.dataset.lanes || '1');
    });
    const now = tl.querySelector('[data-part="cal-now"]');
    if (now) now.style.setProperty('--oc-cal-top', px(Number(now.dataset.start)) + 'px');
    const sc = tl.querySelector('[data-part="cal-scroll"]');
    if (sc) sc.scrollTop = Math.max(0, (now ? px(Number(now.dataset.start)) : px(8 * 60)) - hour * 1.5);
    const allday = app.querySelector('[data-oc="cal-allday"]');
    if (allday) allday.addEventListener('change', () => $$('[data-part="cal-dt"]', app).forEach((i) => { const v = i.value; i.type = allday.checked ? 'date' : 'datetime-local'; if (allday.checked) i.value = v.slice(0, 10); }));
  }

  // ── D005 · listas densas: ↑/↓ entre linhas, Home/End; Enter abre (é um elo) ──
  // O foco anda; a selecção não muda (foco ≠ selecção ≠ aberto).
  function reslist(app) {
    $$('[data-oc="res-list"]', app).forEach((tbl) => {
      tbl.addEventListener('keydown', (e) => {
        const a = e.target.closest('[data-part="res-open"]');
        if (!a) return;
        const l = $$('[data-part="res-open"]', tbl), i = l.indexOf(a);
        let n = -1;
        if (e.key === 'ArrowDown') n = Math.min(l.length - 1, i + 1);
        else if (e.key === 'ArrowUp') n = Math.max(0, i - 1);
        else if (e.key === 'Home') n = 0;
        else if (e.key === 'End') n = l.length - 1;
        if (n < 0) return;
        e.preventDefault();
        l[n].focus();
      });
    });
    // O filtro por ambiente submete ao mudar; o botão fica para quem não tem JS.
    $$('.oc-res-filter select', app).forEach((s) => s.addEventListener('change', () => s.form && s.form.requestSubmit()));
  }


  // ── D006 · a credencial temporária (Mostrar/Copiar) e a confirmação partilhada ──
  // Nada se guarda: nem localStorage, nem sessionStorage, nem o histórico.
  function credential(app) {
    $$('[data-oc="org-secret-show"]', app).forEach((b) => b.addEventListener('click', () => {
      const i = document.getElementById(b.getAttribute('aria-controls')); if (!i) return;
      const on = i.type === 'password'; i.type = on ? 'text' : 'password';
      b.setAttribute('aria-pressed', on ? 'true' : 'false');
      const l = b.querySelector('span'); if (l) l.textContent = on ? (b.dataset.hide || l.textContent) : (b.dataset.show || l.textContent);
    }));
    $$('[data-oc="org-secret-copy"]', app).forEach((b) => b.addEventListener('click', async () => {
      const i = document.getElementById(b.getAttribute('aria-controls')); if (!i) return;
      try { await navigator.clipboard.writeText(i.value); say(app, b.dataset.done || ''); }
      catch (_) { const was = i.type; i.type = 'text'; i.select(); i.type = was; }
    }));
  }
  const confirms = new WeakSet();
  function confirmDlg(dlg) {
    if (confirms.has(dlg)) return;
    confirms.add(dlg);
    const box = dlg.querySelector('form');
    const cancel = dlg.querySelector('[data-oc="org-confirm-cancel"]');
    const focusables = () => $$('a[href], button:not([disabled]), textarea, input:not([type="hidden"]), select', dlg).filter((el) => el.getClientRects().length);
    // Foco inicial: «Cancelar» (nunca o botão que retira acesso).
    if (cancel) setTimeout(() => cancel.focus(), 0);
    dlg.addEventListener('keydown', (e) => {
      if (e.key === 'Escape' && cancel) { e.preventDefault(); cancel.click(); return; }
      if (e.key !== 'Tab') return;
      const f = focusables(); if (!f.length) return;
      const first = f[0], last = f[f.length - 1];
      if (e.shiftKey && document.activeElement === first) { e.preventDefault(); last.focus(); }
      else if (!e.shiftKey && document.activeElement === last) { e.preventDefault(); first.focus(); }
    });
    // Um envio só: o Core é quem garante a idempotência; isto evita o duplo clique.
    if (box) box.addEventListener('submit', (e) => {
      if (box.getAttribute('aria-busy') === 'true') { e.preventDefault(); return; }
      box.setAttribute('aria-busy', 'true');
    });
  }


  // ── D007 · medidores (data-pct → --pct, CSSOM), fio de mensagens no fim, ⌘/Ctrl+Enter envia ──
  function ops(app) {
    $$('.oc-ops-meter__fill[data-pct]', app).forEach((f) => { f.style.setProperty('--pct', Math.max(0, Math.min(100, Number(f.dataset.pct) || 0)) + '%'); if (f.dataset.after) f.style.setProperty('--after', Math.max(0, Math.min(100, Number(f.dataset.after) || 0)) + '%'); });
    const sc = app.querySelector('[data-part="msg-scroll"]');
    if (sc) sc.scrollTop = sc.scrollHeight;
    const body = app.querySelector('[data-part="msg-body"]');
    if (body) body.addEventListener('keydown', (e) => { if (e.key === 'Enter' && (e.metaKey || e.ctrlKey) && body.value.trim() && body.form) { e.preventDefault(); body.form.requestSubmit(); } });
  }

  // D004.1 · D4-J1: idempotente e por raiz — `OcApps.init(root)` liga só as aplicações
  // dentro de `root` (ou `root` ele próprio) e nunca liga a mesma duas vezes.
  const bound = new WeakSet();
  const bind = (app) => {
    if (bound.has(app)) return;
    bound.add(app);
    app.setAttribute('data-js', '');
    drawer(app); docs(app); files(app); calendar(app); reslist(app); credential(app); ops(app);
  };
  const init = (root) => {
    const r = root && root.querySelectorAll ? root : document;
    if (r.matches && r.matches('[data-oc="app"]')) bind(r);
    $$('[data-oc="app"]', r).forEach(bind);
    // D006 · a confirmação é desenhada fora das aplicações (depois da casca).
    if (r.matches && r.matches('[data-oc="org-confirm"]')) confirmDlg(r);
    $$('[data-oc="org-confirm"]', r).forEach(confirmDlg);
  };
  if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', () => init()); else init();
  window.OcApps = { init };
})();
