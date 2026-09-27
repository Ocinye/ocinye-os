/*
 * Ocinye Terminal — a camada de interacção do D13 (ADR-0312).
 *
 * O que este ficheiro faz: separadores, linha de comando, histórico da sessão,
 * completar, pesquisa no histórico, e desenhar o que o Core respondeu.
 *
 * O que este ficheiro NUNCA faz, por desenho:
 *   - decidir se um comando corre: cada linha vai ao Core, que faz o parse
 *     que conta e autoriza;
 *   - entregar uma linha desconhecida ao Nye;
 *   - interpretar HTML vindo do servidor: tudo é desenhado com nós de texto
 *     (`textContent`), incluindo o que o membro escreveu;
 *   - guardar o histórico fora da memória desta página (G-15 ainda não existe),
 *     e nunca com o valor de uma opção sensível.
 */

(() => {
  'use strict';

  const root = document.querySelector('[data-oc="terminal"]');
  if (!root) return;

  const $ = (sel, el) => (el || root).querySelector(sel);
  const T = (name) => root.dataset[name] || '';
  const template = $('template[data-oc="term-session-template"]');
  const tablist = $('[data-oc="term-tabs"]');
  const panes = $('[data-oc="term-panes"]');
  const conn = $('[data-oc="term-conn"]');
  const offline = $('[data-oc="term-offline"]');
  const statusCtx = $('[data-part="status-context"]');

  /* ── Preferências: conveniência local, nunca autoridade ─────────────── */

  const PREF_KEY = 'ocinye.terminal.prefs';
  const prefs = { theme: 'dark', fs: '13', cursor: 'block', density: 'normal', dur: false, exit: false };
  try {
    Object.assign(prefs, JSON.parse(localStorage.getItem(PREF_KEY) || '{}'));
  } catch (_) { /* armazenamento indisponível: ficam as omissões */ }

  function applyPrefs() {
    root.dataset.theme = prefs.theme === 'light' ? 'light' : 'dark';
    root.dataset.cursor = ['block', 'bar', 'under'].includes(prefs.cursor) ? prefs.cursor : 'block';
    root.dataset.density = ['compact', 'normal', 'comfy'].includes(prefs.density) ? prefs.density : 'normal';
    const fs = Math.min(18, Math.max(11, parseInt(prefs.fs, 10) || 13));
    root.style.setProperty('--ods-term-fs', fs + 'px');
    root.querySelectorAll('[data-oc="term-pref"]').forEach((el) => {
      const v = prefs[el.dataset.pref];
      if (el.type === 'radio') el.checked = el.value === v;
      else if (el.type === 'checkbox') el.checked = !!v;
      else el.value = String(v);
    });
  }

  function savePrefs() {
    try { localStorage.setItem(PREF_KEY, JSON.stringify(prefs)); } catch (_) { /* idem */ }
  }

  root.addEventListener('change', (e) => {
    const el = e.target.closest('[data-oc="term-pref"]');
    if (!el) return;
    prefs[el.dataset.pref] = el.type === 'checkbox' ? el.checked : el.value;
    applyPrefs();
    savePrefs();
  });
  root.addEventListener('input', (e) => {
    const el = e.target.closest('[data-oc="term-pref"][type="range"]');
    if (!el) return;
    prefs.fs = el.value;
    applyPrefs();
    savePrefs();
  });

  const sheet = $('[data-oc="term-prefs-sheet"]');
  const scrim = $('.ods-term-prefs__scrim');
  function prefsOpen(open) {
    sheet.hidden = !open;
    scrim.hidden = !open;
    if (!open) focusLine();
  }

  /* ── Redacção: o histórico nunca guarda um segredo ──────────────────── */

  const SENSITIVE = ['password', 'token', 'secret', 'key', 'api-key', 'mfa', 'code'];
  function redact(line) {
    const words = line.split(/(\s+)/);
    let hide = false;
    return words.map((w) => {
      if (/^\s+$/.test(w) || w === '') return w;
      if (hide) { hide = false; return '••••'; }
      const m = /^--([a-z-]+)(=.*)?$/.exec(w);
      if (m && SENSITIVE.includes(m[1])) {
        if (m[2]) return '--' + m[1] + '=••••';
        hide = true;
      }
      return w;
    }).join('');
  }

  /* ── Sessões (uma por separador) ────────────────────────────────────── */

  const sessions = [];
  let active = null;
  let seq = 0;
  let registry = null; // [[uso, descrição], …] — o `help` que o Core filtrou

  function tabName(n) { return T('tTab').replace('{n}', String(n)); }

  function newSession() {
    seq += 1;
    const frag = template.content.cloneNode(true);
    const pane = frag.querySelector('[data-oc="term-pane"]');
    pane.dataset.sessionId = 's' + seq;
    const s = {
      id: 's' + seq,
      name: tabName(seq),
      context: null,
      contextLabel: T('personal'),
      history: [],
      hIdx: null,
      draft: '',
      busy: false,
      pane,
      view: pane.querySelector('.ods-term-pane__view'),
      input: pane.querySelector('[data-oc="term-input"]'),
      line: pane.querySelector('[data-oc="term-line"]'),
      typed: pane.querySelector('.ods-term-input__typed'),
      ghost: pane.querySelector('.ods-term-input__ghost'),
      prompt: pane.querySelector('[data-part="prompt"]'),
      popover: null,
      search: null,
    };
    const tab = document.createElement('div');
    tab.className = 'ods-term-tab';
    tab.setAttribute('role', 'tab');
    tab.dataset.oc = 'term-tab';
    tab.dataset.tabId = s.id;
    const name = document.createElement('span');
    name.className = 'ods-term-tab__name';
    name.textContent = s.name;
    const close = document.createElement('button');
    close.type = 'button';
    close.className = 'ods-term-tab__close';
    close.dataset.oc = 'term-tab-close';
    close.setAttribute('aria-label', T('tTabClose'));
    close.textContent = '×';
    tab.append(name, close);
    tablist.append(tab);
    s.tab = tab;
    panes.append(pane);
    sessions.push(s);
    wireSession(s);
    select(s);
    return s;
  }

  function select(s) {
    active = s;
    sessions.forEach((x) => {
      const on = x === s;
      x.tab.setAttribute('aria-selected', on ? 'true' : 'false');
      x.pane.hidden = !on;
      if (on) x.pane.dataset.focused = ''; else delete x.pane.dataset.focused;
    });
    statusCtx.textContent = s.contextLabel;
    focusLine();
  }

  function closeSession(s) {
    const i = sessions.indexOf(s);
    if (i < 0) return;
    sessions.splice(i, 1);
    s.tab.remove();
    s.pane.remove();
    if (!sessions.length) { newSession(); return; }
    if (active === s) select(sessions[Math.max(0, i - 1)]);
  }

  function focusLine() {
    if (active && !active.line.disabled) active.line.focus({ preventScroll: true });
  }

  /* ── Desenhar ───────────────────────────────────────────────────────── */

  function el(tag, cls, text) {
    const n = document.createElement(tag);
    if (cls) n.className = cls;
    if (text !== undefined && text !== null) n.textContent = String(text);
    return n;
  }
  function seg(text, tone, go, line) {
    const n = go ? el('button', 'ods-term-seg', text) : el('span', 'ods-term-seg', text);
    if (tone) n.dataset.tone = tone;
    if (go) {
      n.type = 'button';
      n.dataset.go = go;
      n.dataset.oc = 'term-go';
      n.dataset.line = line;
    }
    return n;
  }
  function outLine(parts, indent) {
    const d = el('div', 'ods-term-out__line');
    if (indent) d.dataset.indent = '2';
    parts.forEach((p) => d.append(typeof p === 'string' ? document.createTextNode(p) : p));
    return d;
  }

  const GLYPH = { ok: '✓', warn: '!', err: '✕', deny: '⊘', info: '·' };

  function renderNote(b) {
    const d = el('div', 'ods-term-out ods-term-note');
    d.dataset.kind = 'note';
    d.dataset.tone = b.tone;
    d.append(el('span', 'ods-term-note__glyph', GLYPH[b.tone] || '·'), el('span', 'ods-term-note__title', b.title));
    const extra = [];
    if (b.detail) extra.push(el('div', 'ods-term-note__detail', b.detail));
    if (b.hint) extra.push(el('div', 'ods-term-note__hint', b.hint));
    if (b.did_you_mean && b.suggestions && b.suggestions[0]) {
      const h = el('div', 'ods-term-note__hint');
      h.append(seg(b.did_you_mean, 'key', 'fill', b.suggestions[0]));
      extra.push(h);
    } else if (b.suggestions && b.suggestions.length) {
      const h = el('div', 'ods-term-note__hint');
      b.suggestions.forEach((c, i) => {
        if (i) h.append(document.createTextNode('  '));
        h.append(seg(c, 'key', 'fill', c));
      });
      extra.push(h);
    }
    extra.forEach((x) => { d.append(el('span'), x); });
    return d;
  }

  function renderTable(b) {
    const wrap = el('div', 'ods-term-out ods-term-table');
    wrap.dataset.kind = 'table';
    if (b.pipeline) wrap.append(el('div', 'ods-term-out__head', 'pipeline: ' + b.pipeline));
    if (!b.rows.length) {
      wrap.append(outLine([seg(b.empty || '', 'dim')]));
      return wrap;
    }
    const grid = el('div', 'ods-term-table__grid');
    grid.setAttribute('role', 'table');
    grid.style.setProperty('grid-template-columns',
      'repeat(' + (b.columns.length - 1) + ', max-content) minmax(max-content, 1fr)');
    const head = el('div', 'ods-term-table__row');
    head.setAttribute('role', 'row');
    b.columns.forEach((c) => {
      const h = el('div', 'ods-term-table__h', c);
      h.setAttribute('role', 'columnheader');
      head.append(h);
    });
    grid.append(head);
    b.rows.forEach((r) => {
      const row = el('div', 'ods-term-table__row');
      row.setAttribute('role', 'row');
      r.forEach((cell) => {
        const c = el('div', 'ods-term-table__c', cell);
        c.setAttribute('role', 'cell');
        row.append(c);
      });
      grid.append(row);
    });
    wrap.append(grid);
    return wrap;
  }

  function renderFacts(b) {
    const d = el('div', 'ods-term-out');
    d.dataset.kind = 'lines';
    const w = Math.max(0, ...b.rows.map((r) => r[0].length));
    b.rows.forEach((r) => d.append(outLine([seg(r[0].padEnd(w + 2, ' '), 'dim'), seg(r[1], 'fg')])));
    return d;
  }

  function renderHelp(b) {
    const d = el('div', 'ods-term-out');
    d.dataset.kind = 'lines';
    if (b.tagline) d.append(outLine([seg('ocsh', 'key'), ' — ' + b.tagline]));
    const w = Math.max(0, ...b.entries.map((e) => e[0].length));
    b.entries.forEach((e) => {
      const first = e[0].split(' [')[0].split(' <')[0];
      d.append(outLine([seg(e[0], 'key', 'fill', first + ' '), ' '.repeat(w - e[0].length + 2), seg(e[1], 'dim')], true));
    });
    if (b.footer) d.append(outLine([seg(b.footer, 'faint')]));
    return d;
  }

  function jsonTokens(value, indent, out) {
    const pad = '  '.repeat(indent);
    if (Array.isArray(value)) {
      out.push(['[', 'faint']);
      value.forEach((v, i) => {
        out.push(['\n' + pad + '  ', null]);
        jsonTokens(v, indent + 1, out);
        if (i < value.length - 1) out.push([',', 'faint']);
      });
      out.push([(value.length ? '\n' + pad : '') + ']', 'faint']);
    } else if (value && typeof value === 'object') {
      const keys = Object.keys(value);
      out.push(['{', 'faint']);
      keys.forEach((k, i) => {
        out.push(['\n' + pad + '  ', null], [JSON.stringify(k), 'ctx'], [': ', 'faint']);
        jsonTokens(value[k], indent + 1, out);
        if (i < keys.length - 1) out.push([',', 'faint']);
      });
      out.push([(keys.length ? '\n' + pad : '') + '}', 'faint']);
    } else if (typeof value === 'string') {
      out.push([JSON.stringify(value), 'str']);
    } else {
      out.push([JSON.stringify(value), 'num']);
    }
    return out;
  }

  function renderJson(b) {
    const d = el('div', 'ods-term-out');
    d.dataset.kind = 'json';
    const head = el('div', 'ods-term-out__head', 'application/json');
    const copy = el('button', 'ods-term-out__copy', b.copy);
    copy.type = 'button';
    copy.dataset.oc = 'term-copy';
    const text = JSON.stringify(b.value, null, 2);
    copy.addEventListener('click', () => {
      if (navigator.clipboard) navigator.clipboard.writeText(text).then(() => { copy.textContent = b.copied; });
    });
    head.append(copy);
    const body = el('div', 'ods-term-out__line');
    jsonTokens(b.value, 0, []).forEach(([t, tone]) => body.append(tone ? seg(t, tone) : document.createTextNode(t)));
    d.append(head, body);
    return d;
  }

  function renderHistory(s) {
    const d = el('div', 'ods-term-out');
    d.dataset.kind = 'lines';
    s.history.forEach((h, i) => d.append(outLine([seg(String(i + 1).padStart(4, ' ') + '  ', 'faint'), seg(h, 'key', 'fill', h)])));
    return d;
  }

  function promptSnapshot(s) {
    const p = s.prompt.cloneNode(true);
    delete p.dataset.part;
    return p;
  }

  function newEntry(s, line) {
    const e = el('div', 'ods-term-entry');
    e.dataset.oc = 'term-entry';
    const head = el('div', 'ods-term-entry__line');
    const meta = el('span', 'ods-term-entry__meta', T('tRunning'));
    meta.dataset.tone = 'run';
    head.append(promptSnapshot(s), el('span', 'ods-term-entry__cmd', line), meta);
    e.append(head);
    s.view.insertBefore(e, s.input);
    return { e, meta };
  }

  function finish(entry, exit, ms) {
    entry.e.dataset.exit = String(exit);
    const parts = [];
    if (prefs.exit) parts.push(exit === 0 ? '✓ 0' : '✕ ' + exit);
    else if (exit !== 0) parts.push('exit ' + exit);
    if (prefs.dur && typeof ms === 'number') parts.push(ms + ' ms');
    entry.meta.textContent = parts.join(' · ');
    entry.meta.dataset.tone = exit === 0 ? 'ok' : 'err';
  }

  function scrollEnd(s) { s.view.scrollTop = s.view.scrollHeight; }

  /* ── Executar ───────────────────────────────────────────────────────── */

  function setContext(s, ctx) {
    if (!ctx) return;
    s.context = ctx.id || null;
    s.contextLabel = ctx.label;
    s.prompt.querySelector('.ods-term-prompt__ctx').textContent = ctx.label;
    if (s === active) statusCtx.textContent = ctx.label;
  }

  function setConnected(ok) {
    conn.dataset.state = ok ? 'ok' : 'lost';
    offline.hidden = ok;
  }

  async function post(line, context) {
    const r = await fetch('/terminal/exec', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      credentials: 'same-origin',
      body: JSON.stringify({ line, context }),
    });
    if (r.status === 401) { window.location.reload(); throw new Error('sem sessão'); }
    if (!r.ok) throw new Error('HTTP ' + r.status);
    return r.json();
  }

  async function run(s, line) {
    const shown = redact(line);
    if (line.trim() && s.history[s.history.length - 1] !== shown) s.history.push(shown);
    if (s.history.length > 500) s.history.shift();
    const entry = newEntry(s, shown);
    s.busy = true;
    s.input.dataset.mode = 'running';
    s.line.disabled = true;
    scrollEnd(s);
    let res;
    try {
      res = await post(line, s.context);
      setConnected(true);
    } catch (_) {
      setConnected(false);
      entry.e.append(renderNote({ tone: 'err', title: T('tNetwork') }));
      finish(entry, 1, null);
      done(s);
      return;
    }
    setContext(s, res.context);
    for (const b of res.blocks || []) {
      if (b.kind === 'client') {
        if (b.action === 'clear') { clearView(s); entry.e.remove(); done(s); return; }
        if (b.action === 'exit') { done(s); closeSession(s); return; }
        if (b.action === 'history') entry.e.append(renderHistory(s));
        continue;
      }
      const n = b.kind === 'note' ? renderNote(b)
        : b.kind === 'table' ? renderTable(b)
        : b.kind === 'facts' ? renderFacts(b)
        : b.kind === 'help' ? renderHelp(b)
        : b.kind === 'json' ? renderJson(b)
        : null;
      if (n) entry.e.append(n);
    }
    finish(entry, res.exit, res.ms);
    done(s);
  }

  function done(s) {
    s.busy = false;
    s.input.dataset.mode = 'normal';
    s.line.disabled = false;
    scrollEnd(s);
    if (s === active) focusLine();
  }

  function clearView(s) {
    Array.from(s.view.children).forEach((c) => { if (c !== s.input) c.remove(); });
  }

  async function submit(s) {
    const raw = s.line.value;
    setLine(s, '');
    closePopover(s);
    s.hIdx = null;
    // `\` no fim de uma linha continua-a; as outras quebras separam comandos.
    const lines = raw.replace(/\\\n/g, ' ').split('\n');
    if (lines.every((l) => !l.trim())) {
      newEntry(s, '').meta.textContent = '';
      scrollEnd(s);
      return;
    }
    for (const l of lines) {
      if (l.trim()) await run(s, l); // eslint-disable-line no-await-in-loop
    }
  }

  /* ── Linha: espelho, fantasma, completar ────────────────────────────── */

  function setLine(s, v) {
    s.line.value = v;
    syncMirror(s);
  }

  function syncMirror(s) {
    const v = s.line.value;
    s.typed.textContent = v;
    const end = s.line.selectionStart === v.length && s.line.selectionEnd === v.length;
    if (end) s.input.dataset.caretEnd = ''; else delete s.input.dataset.caretEnd;
    s.ghost.textContent = end ? ghostFor(v) : '';
    s.line.style.setProperty('height', 'auto');
    s.line.style.setProperty('height', s.line.scrollHeight + 'px');
  }

  async function loadRegistry() {
    if (registry) return registry;
    try {
      const res = await post('help', null);
      const help = (res.blocks || []).find((b) => b.kind === 'help');
      registry = help ? help.entries : [];
    } catch (_) {
      registry = [];
    }
    return registry;
  }

  /* Candidatos para a palavra actual, a partir do `help` filtrado pelo Core:
   * o que a pessoa não pode usar não aparece aqui. */
  function candidates(v) {
    if (!registry || v.includes('|') || v.includes('\n')) return [];
    const words = v.split(/\s+/);
    const cur = words[words.length - 1];
    const before = words.slice(0, -1);
    const seen = new Map();
    registry.forEach(([usage, desc]) => {
      const u = usage.split(/\s+/);
      const fixed = u.filter((w) => !w.startsWith('[') && !w.startsWith('<'));
      if (before.length === 0) {
        if (!seen.has(fixed[0])) seen.set(fixed[0], { text: fixed[0], kind: 'cmd', desc });
      } else if (before.length === 1 && fixed[0] === before[0] && fixed[1]) {
        seen.set(fixed[1], { text: fixed[1], kind: 'sub', desc });
      } else if (cur.startsWith('-') && fixed[0] === before[0] && (fixed.length < 2 || fixed[1] === before[1])) {
        u.filter((w) => w.startsWith('[--')).forEach((o) => {
          const name = o.replace(/^\[/, '').replace(/\]$/, '').split(' ')[0];
          seen.set(name, { text: name, kind: 'opt', desc: '' });
        });
      }
    });
    return Array.from(seen.values()).filter((c) => c.text.startsWith(cur) && c.text !== cur);
  }

  function ghostFor(v) {
    if (!v || /\s$/.test(v)) return '';
    const c = candidates(v);
    if (c.length !== 1) return '';
    const cur = v.split(/\s+/).pop();
    return c[0].text.slice(cur.length);
  }

  function acceptWord(s, text) {
    const v = s.line.value;
    const cur = v.split(/\s+/).pop();
    setLine(s, v.slice(0, v.length - cur.length) + text + ' ');
  }

  function closePopover(s) {
    if (s.popover) { s.popover.remove(); s.popover = null; }
  }

  function openAc(s, list) {
    closePopover(s);
    const box = el('div', 'ods-term-ac');
    box.setAttribute('role', 'listbox');
    box.dataset.oc = 'term-ac';
    const cur = s.line.value.split(/\s+/).pop();
    list.forEach((c, i) => {
      const o = el('div', 'ods-term-ac__opt');
      o.setAttribute('role', 'option');
      o.setAttribute('aria-selected', i === 0 ? 'true' : 'false');
      o.dataset.oc = 'term-ac-opt';
      o.dataset.value = c.text;
      const word = el('span');
      word.append(el('span', 'ods-term-ac__match', c.text.slice(0, cur.length)), document.createTextNode(c.text.slice(cur.length)));
      o.append(el('span', 'ods-term-ac__kind', c.kind), word, el('span', 'ods-term-ac__desc', c.desc), el('span', 'ods-term-ac__meta', ''));
      box.append(o);
    });
    box.append(el('div', 'ods-term-ac__foot', T('tAcHint')));
    s.input.append(box);
    s.popover = box;
  }

  function moveSel(s, delta) {
    const opts = Array.from(s.popover.querySelectorAll('[role="option"]'));
    if (!opts.length) return;
    let i = opts.findIndex((o) => o.getAttribute('aria-selected') === 'true');
    i = (i + delta + opts.length) % opts.length;
    opts.forEach((o, j) => o.setAttribute('aria-selected', j === i ? 'true' : 'false'));
  }

  function selected(s) {
    return s.popover && s.popover.querySelector('[role="option"][aria-selected="true"]');
  }

  function commonPrefix(list) {
    return list.reduce((a, b) => { let i = 0; while (i < a.length && a[i] === b[i]) i += 1; return a.slice(0, i); });
  }

  async function complete(s) {
    await loadRegistry();
    const list = candidates(s.line.value);
    if (!list.length) return;
    if (list.length === 1) { acceptWord(s, list[0].text); closePopover(s); return; }
    const pre = commonPrefix(list.map((c) => c.text));
    const cur = s.line.value.split(/\s+/).pop();
    if (pre.length > cur.length) setLine(s, s.line.value.slice(0, s.line.value.length - cur.length) + pre);
    openAc(s, candidates(s.line.value).length ? candidates(s.line.value) : list);
  }

  /* ── Pesquisa no histórico (Ctrl+R) ─────────────────────────────────── */

  function searchRender(s) {
    closePopover(s);
    const q = s.line.value;
    const hits = s.history.map((h, i) => [h, i]).filter(([h]) => q && h.includes(q)).reverse();
    s.search.hits = hits;
    s.search.i = Math.min(s.search.i, Math.max(0, hits.length - 1));
    const box = el('div', 'ods-term-rs');
    box.setAttribute('role', 'listbox');
    box.dataset.oc = 'term-rs';
    const head = el('div', 'ods-term-rs__head', T('tRsTitle') + ' · ' + hits.length + ' / ' + s.history.length);
    box.append(head);
    if (!hits.length) box.append(el('div', 'ods-term-rs__foot', T('tRsNone')));
    hits.slice(0, 8).forEach(([h, n], j) => {
      const o = el('div', 'ods-term-rs__opt');
      o.setAttribute('role', 'option');
      o.setAttribute('aria-selected', j === s.search.i ? 'true' : 'false');
      o.dataset.oc = 'term-rs-opt';
      o.dataset.value = h;
      const at = h.indexOf(q);
      const text = el('span');
      text.append(document.createTextNode(h.slice(0, at)), el('mark', null, q), document.createTextNode(h.slice(at + q.length)));
      o.append(el('span', 'ods-term-rs__n', String(n + 1)), text);
      box.append(o);
    });
    box.append(el('div', 'ods-term-rs__foot', T('tRsHint')));
    s.input.append(box);
    s.popover = box;
  }

  function searchStart(s) {
    if (s.search) { s.search.i += 1; searchRender(s); return; }
    s.search = { saved: s.line.value, i: 0, hits: [] };
    s.input.dataset.mode = 'search';
    const mode = el('span', 'ods-term-input__mode', T('tSearch') + ' ›');
    s.prompt.hidden = true;
    s.input.insertBefore(mode, s.prompt);
    s.search.mode = mode;
    setLine(s, '');
    searchRender(s);
  }

  function searchEnd(s, value) {
    s.search.mode.remove();
    s.prompt.hidden = false;
    s.input.dataset.mode = 'normal';
    const v = value === undefined ? s.search.saved : value;
    s.search = null;
    closePopover(s);
    setLine(s, v);
  }

  /* ── Teclado e rato ─────────────────────────────────────────────────── */

  function wireSession(s) {
    s.line.addEventListener('input', () => {
      syncMirror(s);
      if (s.search) searchRender(s);
      else if (s.popover) {
        const list = candidates(s.line.value);
        if (list.length > 1) openAc(s, list); else closePopover(s);
      }
    });
    ['keyup', 'click', 'select'].forEach((ev) => s.line.addEventListener(ev, () => syncMirror(s)));

    s.line.addEventListener('keydown', (e) => {
      if (s.busy) { e.preventDefault(); return; }
      const ctrl = e.ctrlKey || e.metaKey;

      if (s.search) {
        if (e.key === 'Enter') { e.preventDefault(); const h = s.search.hits[s.search.i]; searchEnd(s, h ? h[0] : s.search.saved); return; }
        if (e.key === 'Escape') { e.preventDefault(); searchEnd(s); return; }
        if (ctrl && e.key.toLowerCase() === 'r') { e.preventDefault(); e.stopPropagation(); searchStart(s); return; }
        return;
      }

      if (e.key === 'Enter' && !e.shiftKey) {
        const v = s.line.value;
        if (s.popover && selected(s)) { e.preventDefault(); acceptWord(s, selected(s).dataset.value); closePopover(s); return; }
        if (v.endsWith('\\')) return; // continuação: a quebra entra na linha
        e.preventDefault();
        submit(s);
        return;
      }
      if (e.key === 'Tab') { e.preventDefault(); complete(s); return; }
      if (e.key === 'Escape') { if (s.popover) { e.preventDefault(); closePopover(s); } return; }
      if (e.key === 'ArrowRight' && s.ghost.textContent && s.line.selectionStart === s.line.value.length) {
        e.preventDefault();
        setLine(s, s.line.value + s.ghost.textContent);
        return;
      }
      if (e.key === 'ArrowUp' || e.key === 'ArrowDown') {
        if (s.popover) { e.preventDefault(); moveSel(s, e.key === 'ArrowUp' ? -1 : 1); return; }
        const v = s.line.value;
        const pos = s.line.selectionStart;
        if (e.key === 'ArrowUp' && v.slice(0, pos).includes('\n')) return;
        if (e.key === 'ArrowDown' && v.slice(pos).includes('\n')) return;
        if (!s.history.length) return;
        e.preventDefault();
        if (s.hIdx === null) { if (e.key === 'ArrowDown') return; s.draft = v; s.hIdx = s.history.length; }
        s.hIdx += e.key === 'ArrowUp' ? -1 : 1;
        if (s.hIdx < 0) s.hIdx = 0;
        if (s.hIdx >= s.history.length) { s.hIdx = null; setLine(s, s.draft); return; }
        setLine(s, s.history[s.hIdx]);
        return;
      }
      if (ctrl && !e.shiftKey && e.key.toLowerCase() === 'l') { e.preventDefault(); e.stopPropagation(); clearView(s); return; }
      if (ctrl && !e.shiftKey && e.key.toLowerCase() === 'r') { e.preventDefault(); e.stopPropagation(); searchStart(s); return; }
      if (ctrl && !e.shiftKey && e.key.toLowerCase() === 'c') {
        if (s.line.selectionStart !== s.line.selectionEnd) return; // copiar
        if (String(window.getSelection && window.getSelection())) return;
        e.preventDefault();
        const entry = newEntry(s, s.line.value + '^C');
        entry.meta.textContent = '';
        setLine(s, '');
        closePopover(s);
        scrollEnd(s);
      }
    });

    s.view.addEventListener('click', (e) => {
      const go = e.target.closest('[data-oc="term-go"]');
      if (go) {
        e.stopPropagation();
        if (go.dataset.go === 'run' && !s.busy) { run(s, go.dataset.line); return; }
        setLine(s, go.dataset.line);
        focusLine();
        return;
      }
      const opt = e.target.closest('[data-oc="term-ac-opt"]');
      if (opt) { acceptWord(s, opt.dataset.value); closePopover(s); focusLine(); return; }
      const rs = e.target.closest('[data-oc="term-rs-opt"]');
      if (rs) { searchEnd(s, rs.dataset.value); focusLine(); return; }
      if (!String(window.getSelection && window.getSelection())) focusLine();
    });
  }

  /* ── Cromados ───────────────────────────────────────────────────────── */

  root.addEventListener('click', (e) => {
    const t = e.target.closest('[data-oc]');
    if (!t) return;
    switch (t.dataset.oc) {
      case 'term-tab-new': newSession(); break;
      case 'term-tab-close': {
        const s = sessions.find((x) => x.tab === t.closest('[data-oc="term-tab"]'));
        if (s) closeSession(s);
        e.stopPropagation();
        break;
      }
      case 'term-tab': {
        const s = sessions.find((x) => x.tab === t);
        if (s) select(s);
        break;
      }
      case 'term-prefs': prefsOpen(true); break;
      case 'term-prefs-close': prefsOpen(false); break;
      default: break;
    }
  });

  root.addEventListener('keydown', (e) => {
    if (e.key === 'Escape' && !sheet.hidden) { e.preventDefault(); prefsOpen(false); }
  });

  applyPrefs();
  newSession();
  loadRegistry();
})();
