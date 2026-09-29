/* Ocinye OS · D003 · Nye (Claude Design). Só apresentação e mecânica de cliente.
 *
 * O MOTOR NÃO ESTÁ AQUI. Inferência, orquestração, pesquisa, persistência de
 * conversas, autorização, execução, STT/TTS e encaminhamento de fornecedores
 * são do Code / Core / AI Fabric. Este ficheiro:
 *   1. superfície universal (a paleta D001, que oc-shell.js continua a abrir,
 *      filtrar e fechar): tipo de pedido → marcador do campo; ↓/↑ entre os
 *      resultados; prioridade das camadas globais;
 *   2. aplicação: gavetas (conversas, painel), composer (Enter envia,
 *      Shift+Enter muda de linha), região viva sem anunciar cada token;
 *   3. fluxo da resposta (NYE-03): EventSource same-origin em [data-oc="nye-stream"]
 *      com eventos «delta» {text}, «done», «error» {code}; o texto entra por
 *      textContent (nunca HTML). No fim recarrega para a versão segura do servidor;
 *   4. confirmação forte: foco preso, Esc = Cancelar, foco devolvido;
 *   5. voz, premir para falar: emite intenções «oc:nye»; o runtime decide.
 *
 * Contrato das intenções (document, CustomEvent «oc:nye», cancelável):
 *   {op:'voice-start'|'voice-stop'|'voice-stop-speaking'|'voice-replay'|'voice-lang', lang?}
 * Sem ouvinte (nenhum preventDefault), os botões de voz não fazem nada além do
 * estado visual e ficam honestos: o servidor só os desenha activos quando
 * NyeAvailability.voice_input == Available.
 * Nada é guardado em localStorage. */
(() => {
  'use strict';
  const $$ = (s, r = document) => Array.from(r.querySelectorAll(s));
  const emit = (detail) => !document.dispatchEvent(new CustomEvent('oc:nye', { detail, cancelable: true }));
  const visible = (el) => !!el && !el.closest('[hidden], [inert]') && el.getClientRects().length > 0;

  /* ── camadas globais ───────────────────────────────────────────────
   * Ordem fixa na árvore: .oc-desk → .oc-top → lançador → Nye (paleta) →
   * alternador → diálogo bloqueante (alterações por guardar | confirmação Nye).
   * Um diálogo bloqueante aberto impede ⌘K/⌘J de abrir outra camada por cima
   * do pedido de decisão. Abrir o alternador fecha a Nye. */
  const blocking = () => $$('[data-oc="dirty-close"][data-open], [data-oc="nye-confirm"][data-open]').some((d) => !d.hidden);
  window.addEventListener('keydown', (e) => {
    const k = (e.key || '').toLowerCase();
    if ((e.metaKey || e.ctrlKey) && (k === 'k' || k === 'j') && blocking()) { e.preventDefault(); e.stopImmediatePropagation(); }
  }, true);
  const palette = document.querySelector('[data-oc="palette"][data-nye]');
  const switcher = document.querySelector('[data-oc="switcher"]');
  if (palette && switcher && 'MutationObserver' in window) {
    new MutationObserver(() => {
      if (switcher.hasAttribute('data-open')) palette.removeAttribute('data-open');
    }).observe(switcher, { attributes: true, attributeFilter: ['data-open'] });
  }

  /* ── A · superfície universal ──────────────────────────────────── */
  function surface(form) {
    const q = form.querySelector('[data-part="palette-q"]');
    const modes = $$('[data-part="nye-intent"]', form);
    const set = () => {
      const m = modes.find((r) => r.checked);
      const v = (m && m.value) || 'auto';
      form.dataset.intent = v;
      const ph = q && q.dataset['ph' + v.charAt(0).toUpperCase() + v.slice(1)];
      if (ph) q.placeholder = ph;
    };
    modes.forEach((r) => r.addEventListener('change', () => { set(); if (q) q.focus(); }));
    set();
    // Grupo de aplicações (D001) desaparece quando o filtro não deixa nenhuma.
    const apps = form.querySelector('[data-part="nye-apps"]');
    const syncApps = () => { if (apps) apps.hidden = !$$('[data-part="palette-item"]', apps).some((li) => !li.hidden); };
    if (q) q.addEventListener('input', () => setTimeout(syncApps, 0));
    // ↓/↑: do campo para os resultados e entre eles; Esc no resultado volta ao campo.
    const hits = () => $$('[data-part="nye-hit"]', form).filter(visible);
    form.addEventListener('keydown', (e) => {
      if (e.key !== 'ArrowDown' && e.key !== 'ArrowUp') return;
      const list = hits();
      if (!list.length) return;
      const i = list.indexOf(document.activeElement);
      if (i === -1 && document.activeElement !== q) return;
      e.preventDefault();
      if (e.key === 'ArrowDown') (list[i + 1] || list[0]).focus();
      else if (i <= 0) { if (q) q.focus(); } else list[i - 1].focus();
    });
    // Aberta pelo servidor (GET /ask?q=…): foco no campo, sem apagar o pedido.
    const ov = form.closest('[data-oc="palette"]');
    if (ov && ov.hasAttribute('data-open') && q) setTimeout(() => { q.focus(); q.setSelectionRange(q.value.length, q.value.length); }, 0);
  }
  $$('[data-oc="nye-surface"]').forEach(surface);

  /* ── C · aplicação: gavetas ────────────────────────────────────── */
  function drawers(app) {
    const rail = app.querySelector('#oc-nye-rail');
    const btn = app.querySelector('[data-oc="nye-drawer"]');
    if (!rail || !btn) return;
    let back = null;
    const open = () => { back = document.activeElement; rail.setAttribute('data-open', ''); btn.setAttribute('aria-expanded', 'true'); const f = rail.querySelector('a[href], button, input'); if (f) f.focus(); };
    const close = () => { if (!rail.hasAttribute('data-open')) return; rail.removeAttribute('data-open'); btn.setAttribute('aria-expanded', 'false'); if (back && back.focus) back.focus(); };
    btn.addEventListener('click', () => (rail.hasAttribute('data-open') ? close() : open()));
    $$('[data-oc="nye-drawer-close"]', rail).forEach((b) => b.addEventListener('click', close));
    app.addEventListener('keydown', (e) => { if (e.key === 'Escape' && rail.hasAttribute('data-open')) { e.stopPropagation(); close(); } });
    app.addEventListener('click', (e) => { if (rail.hasAttribute('data-open') && !rail.contains(e.target) && !btn.contains(e.target)) close(); });
  }

  /* composer: Enter envia, Shift+Enter muda de linha; altura acompanha o texto */
  function composer(form) {
    const ta = form.querySelector('[data-part="nye-input"]');
    if (!ta) return;
    const fit = () => { if (CSS.supports && CSS.supports('field-sizing', 'content')) return; ta.style.height = 'auto'; ta.style.height = Math.min(ta.scrollHeight, 180) + 'px'; };
    ta.addEventListener('input', fit); fit();
    ta.addEventListener('keydown', (e) => {
      if (e.key === 'Enter' && !e.shiftKey && !e.isComposing) {
        e.preventDefault();
        if (ta.value.trim() && form.dataset.state !== 'submitting') { form.dataset.state = 'submitting'; form.requestSubmit ? form.requestSubmit() : form.submit(); }
      }
    });
  }

  /* NYE-03 · fluxo da resposta. Região viva: anuncia início e fim, não os tokens. */
  function stream(body, live) {
    const src = body.dataset.src;
    if (!src || !('EventSource' in window) || new URL(src, location.href).origin !== location.origin) return;
    const caret = body.querySelector('.oc-nye-caret');
    let para = body.querySelector('p:last-of-type');
    if (!para || (caret && para.compareDocumentPosition(caret) & Node.DOCUMENT_POSITION_PRECEDING)) { para = document.createElement('p'); body.insertBefore(para, caret); }
    const es = new EventSource(src);
    const end = (key) => { es.close(); body.removeAttribute('aria-busy'); if (live && live.dataset[key]) live.textContent = live.dataset[key]; };
    es.addEventListener('delta', (e) => { try { const d = JSON.parse(e.data); if (typeof d.text === 'string') para.append(document.createTextNode(d.text)); } catch (_) { /* ignora */ } });
    es.addEventListener('done', () => { end('done'); location.reload(); });
    es.addEventListener('error', () => { if (es.readyState === EventSource.CLOSED) end('done'); });
  }

  $$('[data-oc="nye-app"]').forEach((app) => {
    drawers(app);
    $$('[data-oc="nye-composer"]', app).forEach(composer);
    const live = app.querySelector('[data-part="nye-live"]');
    $$('[data-oc="nye-stream"]', app).forEach((b) => stream(b, live));
    const sc = app.querySelector('[data-part="nye-scroll"]');
    if (sc) sc.scrollTop = sc.scrollHeight;
  });
  $$('[data-oc="palette"] [data-oc="nye-stream"]').forEach((b) => stream(b, null));

  /* ── confirmação forte ─────────────────────────────────────────── */
  const FOCUSABLE = 'a[href], button:not([disabled]), input:not([disabled]):not([type="hidden"]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])';
  $$('[data-oc="nye-confirm"][data-open]').forEach((dlg) => {
    const back = document.activeElement;
    const items = () => $$(FOCUSABLE, dlg).filter((el) => el.getAttribute('aria-disabled') !== 'true' && visible(el));
    const cancel = dlg.querySelector('button[data-oc="nye-confirm-cancel"]');
    setTimeout(() => { if (cancel) cancel.focus(); }, 0); // foco inicial no gesto seguro
    dlg.addEventListener('keydown', (e) => {
      if (e.key === 'Escape') { e.preventDefault(); if (cancel) cancel.click(); return; }
      if (e.key !== 'Tab') return;
      const l = items(); if (!l.length) return;
      const i = l.indexOf(document.activeElement);
      if (e.shiftKey && i <= 0) { e.preventDefault(); l[l.length - 1].focus(); }
      else if (!e.shiftKey && (i === l.length - 1 || i === -1)) { e.preventDefault(); l[0].focus(); }
    });
    new MutationObserver(() => {
      if (!document.contains(dlg) || dlg.hidden || !dlg.hasAttribute('data-open')) { if (back && back.focus) back.focus(); }
    }).observe(dlg, { attributes: true });
  });

  /* ── D · voz: premir para falar ────────────────────────────────── */
  $$('[data-oc="nye-voice"]').forEach((root) => {
    const ptt = root.querySelector('button[data-oc="nye-ptt"]');
    if (ptt) {
      let held = false; let t0 = 0; let wasOn = false;
      const on = () => { ptt.setAttribute('aria-pressed', 'true'); emit({ op: 'voice-start' }); };
      const off = () => { ptt.setAttribute('aria-pressed', 'false'); emit({ op: 'voice-stop' }); };
      const pressed = () => ptt.getAttribute('aria-pressed') === 'true';
      // Premir e manter: liga ao premir e desliga ao largar. Toque curto: alterna
      // (liga; o toque seguinte desliga). O estado visível é sempre aria-pressed.
      ptt.addEventListener('pointerdown', (e) => {
        if (e.button !== 0) return;
        held = true; t0 = Date.now(); wasOn = pressed();
        if (!wasOn) on();
        if (ptt.setPointerCapture) ptt.setPointerCapture(e.pointerId);
      });
      ptt.addEventListener('pointerup', () => {
        if (!held) return;
        held = false;
        const long = Date.now() - t0 > 350;
        if (pressed() && (long || wasOn)) off();
      });
      ptt.addEventListener('pointercancel', () => { if (held && pressed()) off(); held = false; });
      // Teclado (Enter/Espaço chegam como click sem ponteiro): alterna.
      ptt.addEventListener('click', (e) => { if (e.detail === 0) { if (pressed()) off(); else on(); } });
      ptt.addEventListener('keydown', (e) => { if (e.key === 'Escape' && pressed()) { e.preventDefault(); off(); } });
    }
    const act = (sel, op) => { const b = root.querySelector(sel); if (b) b.addEventListener('click', () => emit({ op })); };
    act('[data-oc="nye-voice-stop-speaking"]', 'voice-stop-speaking');
    act('[data-oc="nye-voice-replay"]', 'voice-replay');
    const lang = root.querySelector('[data-oc="nye-voice-lang"]');
    if (lang) lang.addEventListener('change', () => emit({ op: 'voice-lang', lang: lang.value }));
    // Sair do modo de voz desliga o microfone antes de navegar.
    const close = root.querySelector('[data-oc="nye-voice-close"]');
    if (close) close.addEventListener('click', () => { if (ptt && ptt.getAttribute('aria-pressed') === 'true') emit({ op: 'voice-stop' }); });
    window.addEventListener('pagehide', () => { if (ptt && ptt.getAttribute('aria-pressed') === 'true') emit({ op: 'voice-stop' }); });
  });

  window.OcNye = {
    /* API para o runtime: reflecte um estado de voz vindo do backend. */
    voiceState(state, text) {
      $$('[data-oc="nye-voice"]').forEach((r) => {
        r.dataset.state = state;
        const ptt = r.querySelector('button[data-oc="nye-ptt"]');
        if (ptt) ptt.setAttribute('aria-pressed', state === 'listening' ? 'true' : 'false');
        const rec = r.querySelector('.oc-nye-voice__rec');
        if (rec) { rec.toggleAttribute('data-on', state === 'listening'); if (text) rec.lastElementChild.textContent = text; }
      });
    },
  };
})();
