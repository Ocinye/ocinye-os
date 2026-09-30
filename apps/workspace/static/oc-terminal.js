/* Ocinye OS · D008-A · Ocinye Terminal (ocsh) — cliente. Claude Design. DESIGN_LOCKED.
 *
 * ocsh é a shell de comandos governada do Ocinye OS. Não dá acesso à shell do
 * anfitrião: opera capabilities do Ocinye através do Core (ADR-0312).
 *
 * Este ficheiro NÃO decide nada. Não faz o parse que conta, não autoriza, não
 * escolhe a capability, não guarda nada fora da memória desta janela:
 *   • cada linha vai inteira para POST /terminal/exec (same-origin, JSON); o Core
 *     faz lexer + parse + registo + autorização + execução e devolve blocos;
 *   • tudo o que vem de dados é desenhado com textContent; caracteres de controlo
 *     (C0, C1, ESC, bidi) ficam visíveis como símbolos, nunca interpretados;
 *   • um comando desconhecido é mostrado como o Core o devolve (127); nunca vai
 *     para a Nye — só `nye ask …` e `? …`, que o Core também recebe como linha;
 *   • histórico: só na memória desta janela, só a linha REDIGIDA (`echo` da
 *     resposta, feita por ocsh::redact no Workspace); nunca localStorage;
 *   • colar várias linhas: nada é executado; a primeira vai para a linha, as
 *     outras ficam à espera, visíveis;
 *   • autocompletar: só famílias/subcomandos do <template data-part="term-registry">
 *     que o servidor desenhou para esta pessoa (descoberta, não autoridade);
 *   • confirmação (quando o Core devolver `plan`): o diálogo partilhado D006
 *     mostra o plano congelado; confirmar envia só o id do plano.
 * Eventos: oc:term-exit (o comando `exit`), cancelável. Sem eval, sem innerHTML. */
(() => {
  'use strict';
  const $ = (s, r) => r.querySelector(s);
  const $$ = (s, r) => Array.from(r.querySelectorAll(s));
  const CTRL = /[\u0000-\u0008\u000B-\u001F\u007F-\u009F\u200E\u200F\u202A-\u202E\u2066-\u2069]/g;
  const visible = (s) => String(s == null ? '' : s).replace(CTRL, (c) => {
    const n = c.charCodeAt(0);
    if (n === 0x1b) return '␛';
    if (n < 0x20) return String.fromCharCode(0x2400 + n);
    if (n === 0x7f) return '␡';
    return '⟨U+' + n.toString(16).toUpperCase().padStart(4, '0') + '⟩';
  });
  const SENSITIVE = ['password', 'token', 'secret', 'key', 'api-key', 'mfa', 'code'];
  // Só para o eco imediato, antes de o Core responder. O que fica no histórico é o `echo` do servidor.
  const redactLocal = (line) => line.replace(/(--([a-z-]+))(=|\s+)("[^"]*"|'[^']*'|\S+)/gi, (m, opt, name, sep, v) => (SENSITIVE.includes(name.toLowerCase()) ? opt + sep + '••••' : m));
  const el = (tag, cls, text) => { const e = document.createElement(tag); if (cls) e.className = cls; if (text != null) e.textContent = visible(text); return e; };
  const icon = (name) => { const s = document.createElementNS('http://www.w3.org/2000/svg', 'svg'); s.setAttribute('class', 'oc-icon'); s.setAttribute('aria-hidden', 'true'); s.setAttribute('focusable', 'false'); const u = document.createElementNS('http://www.w3.org/2000/svg', 'use'); u.setAttribute('href', '#' + name); s.appendChild(u); return s; };
  const TONE = { 0: 'ok', 1: 'err', 2: 'warn', 69: 'warn', 77: 'deny', 126: 'deny', 127: 'warn', 130: 'info' };
  const TONE_ICON = { ok: 'check', info: 'status', warn: 'warning', err: 'warning', deny: 'lock' };

  function bind(root) {
    if (root.hasAttribute('data-js')) return;
    root.setAttribute('data-js', '');
    const form = $('[data-part="term-prompt"]', root);
    const input = $('[data-part="term-input"]', root);
    const log = $('[data-part="term-log"]', root);
    const scroll = $('[data-part="term-scroll"]', root);
    const live = $('[data-part="term-live"]', root);
    const S = (k) => root.dataset[k] || '';
    const history = []; let hpos = -1; let draft = '';
    let context = S('context') || null;
    const say = (t) => { if (live) { live.textContent = ''; setTimeout(() => { live.textContent = t; }, 30); } };
    const bottom = () => { scroll.scrollTop = scroll.scrollHeight; };

    // ── blocos (forma: HANDOFF §T-06, igual a crate::terminal::localize) ──
    const note = (b) => {
      const p = el('div', 'oc-term-note'); p.dataset.tone = b.tone || 'info'; p.appendChild(icon(TONE_ICON[b.tone] || 'status'));
      p.appendChild(el('p', 'oc-term-note__t', b.text || ''));
      if (b.detail) p.appendChild(el('p', 'oc-term-note__b', b.detail));
      if (b.suggestions && b.suggestions.length) {
        const ul = el('ul', 'oc-term-note__sugg');
        b.suggestions.forEach((s) => { const li = el('li'); const btn = el('button', 'oc-term-sugg', s); btn.type = 'button'; btn.dataset.oc = 'term-suggest'; li.appendChild(btn); ul.appendChild(li); });
        p.appendChild(ul);
      }
      return p;
    };
    const table = (b) => {
      const w = el('div', 'oc-term-x'); const t = el('table', 'oc-term-table');
      const tr = el('tr'); (b.columns || []).forEach((c) => { const th = el('th', null, c.label || c); th.scope = 'col'; tr.appendChild(th); });
      const th = el('thead'); th.appendChild(tr); t.appendChild(th);
      const tb = el('tbody'); (b.rows || []).forEach((r) => { const x = el('tr'); r.forEach((c) => x.appendChild(el('td', null, c))); tb.appendChild(x); }); t.appendChild(tb);
      w.appendChild(t);
      const frag = document.createDocumentFragment();
      if (b.pipeline) frag.appendChild(el('p', 'oc-term-pipe', b.pipeline));
      if (b.empty) frag.appendChild(el('p', 'oc-term-pipe', b.empty)); else frag.appendChild(w);
      return frag;
    };
    const facts = (b) => { const d = el('dl', 'oc-term-facts'); (b.rows || []).forEach(([k, v]) => { d.appendChild(el('dt', null, k)); d.appendChild(el('dd', null, v)); }); return d; };
    const help = (b) => { const d = el('dl', 'oc-term-help'); (b.entries || []).forEach((e) => { if (e.group) { d.appendChild(el('dt', 'oc-term-help__group', e.group)); return; } d.appendChild(el('dt', null, e.usage)); d.appendChild(el('dd', null, e.text)); }); return d; };
    const link = (b) => {
      // Uma referência a um recurso: abre na aplicação dona (rota do servidor), que reautoriza.
      const a = el('a', 'oc-res-link'); a.href = b.href; a.dataset.part = 'res-link';
      const ic = el('span', 'oc-res-link__ic'); ic.setAttribute('aria-hidden', 'true'); ic.appendChild(icon(b.icon || 'link')); a.appendChild(ic);
      const m = el('span', 'oc-res-link__main'); m.appendChild(el('span', 'oc-res-link__title', b.title)); const meta = el('span', 'oc-res-link__meta'); meta.appendChild(el('span', null, b.app_label || '')); m.appendChild(meta); a.appendChild(m);
      const li = el('li'); li.appendChild(a); return li;
    };
    const nye = (b) => {
      const c = el('section', 'oc-term-card'); c.dataset.kind = 'nye';
      const h = el('p', 'oc-term-card__h'); h.appendChild(icon('nye')); h.appendChild(el('span', null, S('tNye'))); c.appendChild(h);
      (b.paragraphs || []).forEach((p) => c.appendChild(el('p', null, p)));
      if (b.sources && b.sources.length) { const ul = el('ul', 'oc-res-links'); b.sources.forEach((s) => ul.appendChild(link(s))); c.appendChild(ul); }
      c.appendChild(el('p', 'oc-term-card__note', S('tNyeNote')));
      return c;
    };
    const render = (b) => {
      switch (b.kind) {
        case 'note': return note(b);
        case 'table': return table(b);
        case 'facts': return facts(b);
        case 'help': return help(b);
        case 'links': { const ul = el('ul', 'oc-res-links'); (b.items || []).forEach((i) => ul.appendChild(link(i))); return ul; }
        case 'nye': return nye(b);
        case 'json': return el('pre', 'oc-term-pipe', JSON.stringify(b.value, null, 2));
        default: return note({ tone: 'err', text: S('tUnknownBlock') });
      }
    };

    const entry = (echo) => {
      const li = el('li', 'oc-term-entry'); li.dataset.part = 'term-entry';
      const p = el('p', 'oc-term-echo');
      p.appendChild(el('span', 'oc-term-echo__ctx', root.dataset.contextLabel || ''));
      p.appendChild(el('span', 'oc-term-echo__line', echo));
      const meta = el('span', 'oc-term-echo__meta'); meta.appendChild(el('span', 'oc-term-st', S('tRunning'))); p.appendChild(meta);
      li.appendChild(p); const out = el('div', 'oc-term-out'); li.appendChild(out); log.appendChild(li); bottom();
      return { li, meta, out };
    };
    const finish = (e, res) => {
      const tone = TONE[res.exit] || 'err';
      e.meta.textContent = '';
      const st = el('span', 'oc-term-st'); st.dataset.tone = tone; st.appendChild(icon(TONE_ICON[tone])); st.appendChild(el('span', null, (root.dataset['exit' + res.exit] || '') )); st.appendChild(el('span', 'oc-term-st__code', (S('tExit') || 'exit {code}').replace('{code}', String(res.exit)))); e.meta.appendChild(st);
      if (res.ms != null) e.meta.appendChild(el('span', null, (S('tMs') || '{ms} ms').replace('{ms}', String(res.ms))));
      (res.blocks || []).forEach((b) => {
        if (b.kind === 'client') return client(b.action, res);
        e.out.appendChild(render(b));
      });
      if (res.plan) confirmPlan(res.plan);
      if (res.context && res.context.label) { root.dataset.contextLabel = res.context.label; context = res.context.id || null; $$('[data-part="term-ctx"]', root).forEach((x) => { x.textContent = visible(res.context.label); }); }
      say(e.li.querySelector('.oc-term-echo__line').textContent + ' — ' + (root.dataset['exit' + res.exit] || res.exit));
      bottom();
    };
    const client = (action) => {
      if (action === 'clear') { log.textContent = ''; const n = note({ tone: 'info', text: S('tCleared') }); const li = el('li', 'oc-term-entry'); li.appendChild(n); log.appendChild(li); }
      else if (action === 'history') {
        const li = el('li', 'oc-term-entry'); const out = el('div', 'oc-term-out');
        if (!history.length) out.appendChild(note({ tone: 'info', text: S('tHistoryEmpty') }));
        else { const ol = el('ol', 'oc-term-help'); history.forEach((h, i) => { ol.appendChild(el('dt', null, String(i + 1))); ol.appendChild(el('dd', null, h)); }); out.appendChild(ol); out.appendChild(el('p', 'oc-term-pipe', S('tHistoryNote'))); }
        li.appendChild(out); log.appendChild(li);
      } else if (action === 'exit') { document.dispatchEvent(new CustomEvent('oc:term-exit', { detail: { root }, cancelable: true })); }
    };

    const send = async (line) => {
      const shown = redactLocal(line);
      const e = entry(shown);
      if (root.dataset.core === 'offline') { finish(e, { exit: 1, blocks: [{ kind: 'note', tone: 'err', text: S('tOffline') }] }); return; }
      let res;
      try {
        const r = await fetch(form.action, { method: 'POST', credentials: 'same-origin', headers: { 'Content-Type': 'application/json', Accept: 'application/json' }, body: JSON.stringify({ line, context }) });
        res = r.status === 401 ? { exit: 77, blocks: [{ kind: 'note', tone: 'deny', text: S('tNoSession') }] } : await r.json();
      } catch (_) { res = { exit: 1, blocks: [{ kind: 'note', tone: 'err', text: S('tNetwork') }] }; }
      // O histórico só recebe a linha redigida pelo servidor; sem ela, a redacção local.
      const echo = typeof res.echo === 'string' ? res.echo : shown;
      e.li.querySelector('.oc-term-echo__line').textContent = visible(echo);
      history.push(echo); if (history.length > 200) history.shift();
      finish(e, res);
    };

    form.addEventListener('submit', (ev) => {
      ev.preventDefault();
      if (root.hasAttribute('data-confirming')) return;
      const line = input.value.trim(); if (!line) return;
      input.value = ''; hpos = -1; draft = ''; closeComplete();
      send(line);
      nextPasted();
    });

    // ── histórico ↑/↓ (só a sessão) ──
    input.addEventListener('keydown', (ev) => {
      const box = $('[data-part="term-complete"]', root);
      if (box && !box.hidden) {
        const opts = $$('[role="option"]', box); let i = opts.findIndex((o) => o.getAttribute('aria-selected') === 'true');
        if (ev.key === 'ArrowDown' || ev.key === 'ArrowUp') { ev.preventDefault(); i = (i + (ev.key === 'ArrowDown' ? 1 : -1) + opts.length) % opts.length; opts.forEach((o, k) => o.setAttribute('aria-selected', String(k === i))); input.setAttribute('aria-activedescendant', opts[i].id); return; }
        if (ev.key === 'Tab' || (ev.key === 'Enter' && i >= 0)) { ev.preventDefault(); accept(opts[Math.max(0, i)]); return; }
        if (ev.key === 'Escape') { ev.preventDefault(); closeComplete(); return; }
      }
      if (ev.key === 'Tab' && !ev.shiftKey && input.value.trim()) { ev.preventDefault(); complete(); return; }
      if (ev.key === 'ArrowUp' && history.length) { ev.preventDefault(); if (hpos === -1) { draft = input.value; hpos = history.length; } hpos = Math.max(0, hpos - 1); input.value = history[hpos]; }
      if (ev.key === 'ArrowDown' && hpos !== -1) { ev.preventDefault(); hpos += 1; if (hpos >= history.length) { hpos = -1; input.value = draft; } else input.value = history[hpos]; }
    });

    // ── autocompletar: só o que o servidor pôs no registo desta pessoa ──
    const reg = $$('template[data-part="term-registry"]', root).flatMap((t) => $$('[data-cmd]', t.content)).map((x) => ({ cmd: x.dataset.cmd, help: x.textContent }));
    const closeComplete = () => { const b = $('[data-part="term-complete"]', root); if (b) b.hidden = true; input.setAttribute('aria-expanded', 'false'); input.removeAttribute('aria-activedescendant'); };
    const accept = (o) => { if (!o) return; input.value = o.dataset.value + ' '; closeComplete(); input.focus(); };
    const complete = () => {
      const q = input.value.replace(/\s+/g, ' ').trimStart();
      const hits = reg.filter((r) => r.cmd.startsWith(q)).slice(0, 8);
      if (hits.length === 1) { accept({ dataset: { value: hits[0].cmd } }); return; }
      const box = $('[data-part="term-complete"]', root); if (!box || !hits.length) return;
      $$('[role="option"]', box).forEach((o) => o.remove());
      hits.forEach((h, i) => { const li = el('li'); li.setAttribute('role', 'option'); li.id = 'oc-term-opt-' + i; li.dataset.value = h.cmd; li.setAttribute('aria-selected', String(i === 0)); li.appendChild(el('b', null, h.cmd)); li.appendChild(el('span', null, h.help)); li.addEventListener('mousedown', (e) => { e.preventDefault(); accept(li); }); box.insertBefore(li, box.lastElementChild); });
      box.hidden = false; input.setAttribute('aria-expanded', 'true'); input.setAttribute('aria-activedescendant', 'oc-term-opt-0');
    };

    // ── colar: nunca executa; linhas extra ficam à espera ──
    const pasted = [];
    const paste = $('[data-part="term-paste"]', root);
    const drawPaste = () => {
      if (!paste) return; paste.hidden = !pasted.length; const ol = $('ol', paste); if (!ol) return; ol.textContent = '';
      pasted.forEach((l) => ol.appendChild(el('li', null, redactLocal(l))));
      const n = $('[data-part="term-paste-n"]', paste); if (n) n.textContent = (paste.dataset.title || '{n}').replace('{n}', String(pasted.length + 1));
    };
    const nextPasted = () => { if (pasted.length && !input.value) { input.value = pasted.shift(); drawPaste(); } };
    input.addEventListener('paste', (ev) => {
      const text = (ev.clipboardData || window.clipboardData).getData('text');
      const lines = text.split(/\r\n|\r|\n/).map((l) => l.trim()).filter(Boolean);
      if (lines.length < 2) return;
      ev.preventDefault(); input.value = lines.shift(); pasted.splice(0, pasted.length, ...lines); drawPaste();
    });
    root.addEventListener('click', (ev) => {
      const t = ev.target.closest('[data-oc]'); if (!t || !root.contains(t)) return;
      const op = t.dataset.oc;
      if (op === 'term-suggest') { input.value = t.textContent; input.focus(); }
      else if (op === 'term-paste-next') { input.value = pasted.shift() || ''; drawPaste(); input.focus(); }
      else if (op === 'term-paste-discard') { pasted.length = 0; drawPaste(); input.focus(); }
      else if (op === 'term-clear') client('clear');
      else if (op === 'term-help') { input.value = 'help'; form.requestSubmit(); }
      else if (op === 'term-find') toggleFind();
    });

    // ── procurar no ecrã (só apresentação) ──
    const find = $('[data-part="term-find"]', root);
    let hits = [], at = -1;
    const toggleFind = () => { if (!find) return; find.hidden = !find.hidden; const f = $('input', find); if (!find.hidden && f) f.focus(); else { clearHits(); input.focus(); } };
    const clearHits = () => { $$('[data-hit]', log).forEach((x) => x.removeAttribute('data-hit')); hits = []; at = -1; };
    const go = (d) => { if (!hits.length) return; at = (at + d + hits.length) % hits.length; hits.forEach((h, i) => (i === at ? h.setAttribute('data-hit', '') : h.removeAttribute('data-hit'))); scroll.scrollTop = hits[at].offsetTop - 12; const n = $('[data-part="term-find-n"]', find); if (n) n.textContent = (find.dataset.of || '{n}/{total}').replace('{n}', String(at + 1)).replace('{total}', String(hits.length)); };
    if (find) {
      const f = $('input', find);
      f.addEventListener('input', () => { clearHits(); const q = f.value.trim().toLowerCase(); if (q) hits = $$('[data-part="term-entry"]', log).filter((x) => x.textContent.toLowerCase().includes(q)); const n = $('[data-part="term-find-n"]', find); if (n) n.textContent = hits.length ? '' : (q ? find.dataset.none : ''); go(1); });
      find.addEventListener('keydown', (e) => { if (e.key === 'Enter') { e.preventDefault(); go(e.shiftKey ? -1 : 1); } if (e.key === 'Escape') { e.preventDefault(); toggleFind(); } });
      $$('[data-oc="term-find-prev"], [data-oc="term-find-next"]', find).forEach((b) => b.addEventListener('click', () => go(b.dataset.oc.endsWith('prev') ? -1 : 1)));
    }

    // ── confirmação por plano (contrato TERMINAL-11) ──
    const confirmPlan = (plan) => {
      const tpl = $('template[data-part="term-confirm"]', root); if (!tpl) return;
      const node = tpl.content.firstElementChild.cloneNode(true);
      const set = (part, v) => { const x = $('[data-part="' + part + '"]', node); if (x) x.textContent = visible(v); };
      set('term-confirm-title', (tpl.dataset.title || '{action}').replace('{action}', plan.action_label));
      set('term-confirm-line', plan.echo); set('term-confirm-cap', plan.capability); set('term-confirm-risk', plan.risk_label);
      set('term-confirm-ctx', plan.context_label); set('term-confirm-target', plan.target_label); set('term-confirm-plan', plan.id_short); set('term-confirm-valid', plan.valid_until);
      const id = $('input[name="plan"]', node); if (id) id.value = plan.id;
      root.setAttribute('data-confirming', ''); input.disabled = true;
      document.body.appendChild(node);
      node.addEventListener('click', (e) => { if (e.target.closest('[data-oc="org-confirm-cancel"]')) { e.preventDefault(); node.remove(); root.removeAttribute('data-confirming'); input.disabled = false; input.focus(); } });
      if (window.OcApps) window.OcApps.init(node);
    };
    bottom();
  }

  const init = (r) => { const root = r && r.querySelectorAll ? r : document; if (root.matches && root.matches('[data-oc="term"]')) bind(root); $$('[data-oc="term"]', root).forEach(bind); };
  if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', () => init()); else init();
  window.OcTerminal = { init };
})();
