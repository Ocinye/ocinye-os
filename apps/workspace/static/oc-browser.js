/* Ocinye OS · D008-B · Ocinye Browser — Browser Manager do cliente (Web) + ligação ao adaptador nativo.
 * Claude Design. DESIGN_LOCKED.
 *
 * Cromado do Browser = interface de confiança do Ocinye. Conteúdo web = NÃO confiável.
 * Este ficheiro nunca:
 *   • injecta JavaScript numa página, lê o DOM de uma página, ou clica/preenche por alguém;
 *   • passa a sessão, cookies, cabeçalhos ou tokens do Ocinye a um site (o <iframe> vem do
 *     servidor com sandbox sem allow-same-origin/allow-top-navigation, allow="" e
 *     referrerpolicy=no-referrer; não se muda aqui);
 *   • usa um proxy: o site é pedido pelo navegador da pessoa, directamente;
 *   • envia texto de uma página para o ocsh (não há ponte Browser → Terminal);
 *   • decide autorização: abrir em Ficheiros, perguntar à Nye e afins são intents tipados
 *     que outra parte do Ocinye governa.
 * Esquemas: só http/https (parser WHATWG, `new URL`). javascript:, data:, file:, blob:, e
 * esquemas próprios são recusados antes de qualquer navegação. Sem pesquisa: texto que não é
 * endereço fica como «Não é um endereço» (não há motor de pesquisa configurado).
 *
 * Web (window.ocinyeRuntime.mode === 'web'): a aba conhece só o endereço PEDIDO; não há recuar,
 * avançar nem título (origem cruzada). O recurso honesto é «Abrir num separador do navegador»
 * (noopener,noreferrer), sempre presente.
 * Desktop/Dedicado: cada intent vai para `ocinyeRuntime.browser` (ponte tipada ADR-0703 §2,
 * `browser.*`). Sem ponte, o estado é «indisponível» — nunca um sucesso simulado. */
(() => {
  'use strict';
  const $ = (s, r) => r.querySelector(s);
  const $$ = (s, r) => Array.from(r.querySelectorAll(s));
  const ALLOWED = ['https:', 'http:'];

  /** Classifica o que a pessoa escreveu. Não navega. */
  function classify(text) {
    const raw = String(text || '').trim();
    if (!raw) return { kind: 'empty' };
    const scheme = /^([a-z][a-z0-9+.-]*):/i.exec(raw);
    if (scheme && !/^[a-z0-9.-]+:\d+(\/|$)/i.test(raw)) {
      let u; try { u = new URL(raw); } catch (_) { return { kind: 'invalid', text: raw }; }
      if (!ALLOWED.includes(u.protocol)) return { kind: 'blocked', scheme: u.protocol };
      if (!u.hostname) return { kind: 'invalid', text: raw };
      return { kind: 'url', url: u };
    }
    if (/\s/.test(raw) || !/\.[a-z0-9-]{2,}(:\d+)?(\/|$|\?|#)/i.test(raw)) return { kind: 'invalid', text: raw };
    try { return { kind: 'url', url: new URL('https://' + raw) }; } catch (_) { return { kind: 'invalid', text: raw }; }
  }

  const runtime = () => (window.ocinyeRuntime && window.ocinyeRuntime.mode) || 'web';
  const native = () => (window.ocinyeRuntime && window.ocinyeRuntime.browser) || null; // só a casca o define (ADR-0703)

  function bind(root) {
    if (root.hasAttribute('data-js')) return;
    root.setAttribute('data-js', '');
    const addr = $('[data-part="brw-address"]', root);
    const field = addr && $('input', addr);
    const view = $('[data-part="brw-view"]', root);
    const tabs = $('[data-part="brw-tablist"]', root);
    const live = $('[data-part="brw-live"]', root);
    const say = (t) => { if (live) { live.textContent = ''; setTimeout(() => { live.textContent = t; }, 30); } };
    const tpl = (name) => { const t = $('template[data-part="brw-tpl-' + name + '"]', root); return t ? t.content.firstElementChild.cloneNode(true) : null; };
    const fill = (node, map) => { Object.entries(map).forEach(([k, v]) => $$('[data-slot="' + k + '"]', node).forEach((x) => { x.textContent = String(v); })); return node; };
    const show = (node) => { if (!view || !node) return; view.textContent = ''; view.setAttribute('data-empty', ''); view.appendChild(node); };

    const navigate = (text) => {
      const c = classify(text);
      if (c.kind === 'empty') return;
      if (c.kind === 'blocked') { show(fill(tpl('blocked'), { scheme: c.scheme })); say(root.dataset.tBlocked || ''); return; }
      if (c.kind === 'invalid') { show(fill(tpl('invalid'), { text: c.text })); return; }
      const url = c.url;
      if (runtime() === 'web') {
        // A aba conhece o endereço pedido — e só esse.
        const frame = tpl('frame'); if (!frame) return;
        const ifr = frame.tagName === 'IFRAME' ? frame : $('iframe', frame);
        view.textContent = ''; view.removeAttribute('data-empty');
        ifr.src = url.href; view.appendChild(frame);
        $$('[data-part="brw-origin"]', root).forEach((x) => { x.textContent = url.host; });
        $$('[data-part="brw-sec"]', root).forEach((x) => { x.closest('[data-sec]').dataset.sec = url.protocol === 'https:' ? 'https' : 'http'; x.textContent = url.protocol === 'https:' ? root.dataset.tHttps : root.dataset.tHttp; });
        $$('[data-part="brw-external"]', root).forEach((a) => { a.href = url.href; });
        field.value = url.href;
        return;
      }
      const n = native();
      if (!n) { show(tpl('no-runtime')); return; }
      n.navigate({ tab: activeTab(), url: url.href }); // intent tipado; o estado volta por eventos da casca
    };

    $$('.oc-brw-dl__bar > [data-pct]', root).forEach((x) => x.style.setProperty('--pct', Math.max(0, Math.min(100, Number(x.dataset.pct) || 0)) + '%'));
    if (addr) addr.addEventListener('submit', (e) => { e.preventDefault(); navigate(field.value); });

    // Abrir fora: sai da área controlada; o Ocinye não controla esse separador.
    root.addEventListener('click', (e) => {
      const a = e.target.closest('[data-part="brw-external"]');
      if (a) { e.preventDefault(); const w = window.open(a.href, '_blank', 'noopener,noreferrer'); if (w) w.opener = null; say(root.dataset.tOpened || ''); return; }
      const t = e.target.closest('[data-oc]'); if (!t || !root.contains(t)) return;
      const op = t.dataset.oc;
      if (op === 'brw-reload' && runtime() === 'web') { const f = $('iframe', view); if (f) { const s = f.src; f.src = 'about:blank'; f.src = s; } }
      else if (op === 'brw-edit') { const a = t.closest('[data-mobile-compact]'); if (a) { a.removeAttribute('data-mobile-compact'); if (field) { field.focus(); field.select(); } } }
      else if (op === 'brw-switcher') { const s = $('[data-part="brw-switch"]', root); if (s) { s.hidden = !s.hidden; t.setAttribute('aria-expanded', String(!s.hidden)); const body = $('[data-part="brw-body"]', root), edge = $('[data-part="brw-edge"]', root); if (body) body.hidden = !s.hidden; if (edge) edge.hidden = !s.hidden; const f = !s.hidden && $('button', s); if (f) f.focus(); } }
      else if (op === 'brw-side') { const b = $('[data-part="brw-body"]', root); const side = $('[data-part="' + t.dataset.side + '"]', root); if (b && side) { const open = side.hidden; $$('.oc-brw-side', root).forEach((x) => { x.hidden = true; }); $$('[data-oc="brw-side"]', root).forEach((x) => x.setAttribute('aria-expanded', 'false')); side.hidden = !open; open ? b.setAttribute('data-side', '') : b.removeAttribute('data-side'); t.setAttribute('aria-expanded', String(open)); } }
      else if (native() && ['brw-back', 'brw-forward', 'brw-reload', 'brw-stop', 'brw-new', 'brw-close'].includes(op)) native()[op.slice(4)]({ tab: t.dataset.tab || activeTab() });
      else if (op === 'brw-perm') { if (native()) native().decidePermission({ tab: activeTab(), request: t.dataset.request, decision: t.dataset.decision }); }
    });

    // ── abas: roving tabindex, ← → Home End; Delete fecha ──
    const activeTab = () => { const s = tabs && $('[aria-selected="true"]', tabs); return s ? s.dataset.tab : null; };
    if (tabs) tabs.addEventListener('keydown', (e) => {
      const list = $$('[role="tab"]', tabs); const i = list.indexOf(document.activeElement); if (i < 0) return;
      let j = null;
      if (e.key === 'ArrowRight') j = (i + 1) % list.length; else if (e.key === 'ArrowLeft') j = (i - 1 + list.length) % list.length; else if (e.key === 'Home') j = 0; else if (e.key === 'End') j = list.length - 1;
      if (j != null) { e.preventDefault(); list.forEach((x, k) => x.setAttribute('tabindex', k === j ? '0' : '-1')); list[j].focus(); }
    });

    // ── atalhos: só com o foco no cromado do Browser, e só onde o runtime os entrega ──
    // Web: Ctrl/Cmd+L/T/W/R pertencem ao navegador anfitrião e não se interceptam.
    // Desktop/Dedicado: a casca entrega-os ao Browser Manager (HANDOFF §B-17); Ctrl/Cmd+W fecha a
    // ABA activa; fechar a última aba deixa uma Nova aba — nunca fecha a janela do Ocinye.
    root.addEventListener('keydown', (e) => {
      if (runtime() === 'web' || !(e.ctrlKey || e.metaKey) || e.altKey) return;
      const k = e.key.toLowerCase();
      if (k === 'l' && field) { e.preventDefault(); field.focus(); field.select(); }
      else if (['t', 'w', 'r'].includes(k) && native()) { e.preventDefault(); native()[{ t: 'new', w: 'close', r: 'reload' }[k]]({ tab: activeTab() }); }
    });
  }

  const init = (r) => { const root = r && r.querySelectorAll ? r : document; if (root.matches && root.matches('[data-oc="brw"]')) bind(root); $$('[data-oc="brw"]', root).forEach(bind); };
  if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', () => init()); else init();
  window.OcBrowser = { init, classify };
})();
