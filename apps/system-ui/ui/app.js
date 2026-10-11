/* Ocinye OS · interface do sistema (ADR-0031).
   Apresenta o que o Session Service e o System Service dizem e envia-lhes
   pedidos tipados; não decide nada. Marcação só de classes (CSP style-src
   'self'). Todo o valor que vem de um serviço ou da pessoa passa por esc().
   Estrutura e formas do Design (System Experience). */
(function () {
  'use strict';
  const T = window.SX_T;
  const ipc = window.__TAURI_INTERNALS__;
  // Fora da janela nativa (os testes da interface) quem responde é o duplo.
  const invoke = (cmd, args) => (ipc ? ipc.invoke(cmd, args || {}) : window.SX_FAKE(cmd, args || {}));

  const S = {
    lang: 'pt',
    langChosen: false,
    session: null,          // SessionState, ou null enquanto não se sabe
    sessionDown: false,     // o Session Service não responde
    serviceDown: false,     // o System Service não responde
    sys: {},                // capacidade de leitura → valor, ou { refused }
    panel: null,            // null | 'quick' | 'system'
    page: 'home',
    dialog: null,
    form: { user: '', pass: '', code: '', unlock: '', auth: '', new1: '', new2: '' },
    step: 1,                // 1 conta · 2 segundo factor · 3 palavra-passe nova
    busy: false,
    error: null,            // [chave, variáveis]
    toast: null,
    full: null,
  };
  const MISSING = [];
  window.SX_STATE = S;
  window.SX_MISSING = MISSING;

  // ── textos
  const LI = () => ({ pt: 0, en: 1, fr: 2 }[S.lang] || 0);
  const esc = (s) => String(s == null ? '' : s).replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;');
  const t = (k, v) => {
    const e = T[k];
    if (!e) { MISSING.push(k); return esc(k); }
    let s = esc(e[LI()]);
    if (v) for (const n in v) s = s.split('{' + n + '}').join(esc(v[n]));
    return s;
  };
  const LOCALE = () => ({ pt: 'pt-PT', en: 'en-GB', fr: 'fr-FR' }[S.lang]);
  const clock = () => new Intl.DateTimeFormat(LOCALE(), { hour: '2-digit', minute: '2-digit' }).format(new Date());
  const today = () => new Intl.DateTimeFormat(LOCALE(), { weekday: 'long', day: 'numeric', month: 'long' }).format(new Date());
  const bytes = (n) => (n >= 1e12 ? (n / 1e12).toFixed(1) + ' TB' : n >= 1e9 ? (n / 1e9).toFixed(1) + ' GB' : Math.round(n / 1e6) + ' MB');
  const initials = (name) => String(name || '?').trim().split(/\s+/).filter(Boolean).slice(0, 2).map((w) => w[0].toUpperCase()).join('') || '?';

  // ── peças
  const ic = (id, c) => `<svg class="sx-icon${c ? ' ' + c : ''}" aria-hidden="true" focusable="false"><use href="icons.svg#${id}"></use></svg>`;
  const btn = (kind, label, act, o = {}) => `<button class="sx-btn${kind ? ' sx-btn--' + kind : ''}${o.lg ? ' sx-btn--lg' : ''}" type="${o.submit ? 'submit' : 'button'}"${act ? ` data-act="${act}"` : ''}${o.arg != null ? ` data-arg="${esc(o.arg)}"` : ''}${o.disabled || S.busy ? ' disabled' : ''}${o.id ? ` id="${o.id}"` : ''}>${o.icon ? ic(o.icon) : ''}${label}</button>`;
  const status = (tone, text) => `<span class="sx-status" data-tone="${tone}">${text}</span>`;
  const alert = (tone, text) => `<div class="sx-alert" data-tone="${tone}" role="${tone === 'bad' ? 'alert' : 'status'}">${ic(tone === 'info' ? 'info' : 'warn')}<div>${text}</div></div>`;
  const field = (id, label, o = {}) => `<label class="sx-field" for="f-${id}"><span class="sx-field__l">${label}</span><input class="sx-input${o.code ? ' sx-input--code' : ''}" id="f-${id}" data-f="${id}" type="${o.type || 'text'}" value="${esc(S.form[id])}" autocomplete="off" autocapitalize="none" spellcheck="false"${o.code ? ' inputmode="numeric" maxlength="6"' : ''}${S.busy ? ' disabled' : ''}></label>`;
  const toggle = (on, act, o = {}) => `<button class="sx-toggle" type="button" role="switch" aria-checked="${on ? 'true' : 'false'}" aria-label="${o.label || ''}" data-act="${act}" data-arg="${on ? '0' : '1'}"${o.disabled || S.busy ? ' disabled' : ''}><span>${t(on ? 'q.on' : 'q.off')}</span><span class="sx-toggle__track"></span></button>`;
  const op = () => `<span class="sx-op">${ic('lock')}${t('k.operator')}</span>`;
  const row = (title, value, o = {}) => `<div class="sx-row"><div class="sx-row__l"><div class="sx-row__t">${title}${o.op ? op() : ''}</div>${o.sub ? `<div class="sx-row__s">${o.sub}</div>` : ''}</div>${value == null ? '' : `<div class="sx-row__v">${value}</div>`}</div>`;
  const note = (text) => `<div class="sx-row sx-row--note">${text}</div>`;
  const group = (title, rows) => `<section class="sx-group">${title ? `<h2>${title}</h2>` : ''}${rows.join('')}</section>`;

  // ── serviços
  const got = (cap) => { const v = S.sys[cap]; return v && !v.refused ? v : null; };
  const refusedText = (reason) => t('r.' + ((reason && reason.code) || 'SYSTEM_SERVICE'));
  const STATUS_OF = { health: 'system.health', info: 'system.info', inventory: 'system.capabilities', network: 'network.status', audio: 'audio.status', display: 'display.status', power: 'power.status', locale: 'locale.status', time: 'time.status', keyboard: 'keyboard.status', accessibility: 'accessibility.status', privacy: 'privacy.status', storage: 'storage.status', update: 'update.status', remote_access: 'remote_access.status', recovery: 'recovery.status', bluetooth: 'bluetooth.status' };

  async function call(capability, args, grant) {
    const request = Object.assign({ capability }, args || {});
    let answer;
    try { answer = await invoke('system', { request, grant: grant || null }); } catch (e) { answer = { unreachable: 'SYSTEM_SERVICE' }; }
    if (!answer || !answer.outcome) { S.serviceDown = true; return { refused: { code: 'SYSTEM_SERVICE' }, request }; }
    S.serviceDown = false;
    const o = answer.outcome;
    if (o.outcome === 'OK') {
      const p = o.payload || {};
      if (STATUS_OF[p.kind]) S.sys[STATUS_OF[p.kind]] = p.value;
      return { ok: p, request };
    }
    return { refused: o.reason, request };
  }

  async function read(caps) {
    await Promise.all(caps.map(async (cap) => {
      const r = await call(cap);
      if (r.refused) S.sys[cap] = { refused: r.refused };
    }));
    applyPreferences();
  }

  async function ask(request) {
    let answer;
    try { answer = await invoke('session', { request }); } catch (e) { answer = null; }
    if (!answer || !answer.reply) { S.sessionDown = true; return null; }
    S.sessionDown = false;
    const r = answer.reply;
    if (r.reply === 'state') S.session = r.state;
    return r;
  }

  function applyPreferences() {
    const a = got('accessibility.status');
    const root = document.documentElement;
    if (a) {
      root.classList.toggle('sx-scale-115', a.text_scale_percent === 115);
      root.classList.toggle('sx-scale-130', a.text_scale_percent === 130);
      root.classList.toggle('sx-scale-150', a.text_scale_percent === 150);
      root.classList.toggle('sx-contrast', !!a.high_contrast);
      root.classList.toggle('sx-still', !!a.reduce_motion);
    }
    const l = got('locale.status');
    if (l && !S.langChosen) S.lang = l.language;
    root.lang = LOCALE();
  }

  // Uma alteração: pede; se o serviço exigir prova fresca, mostra o pedido de
  // confiança e repete com a concessão que o Session Service emitir.
  async function change(capability, args) {
    S.busy = true; render();
    const r = await call(capability, args);
    S.busy = false;
    if (r.refused && r.refused.code === 'FRESH_AUTH_REQUIRED') {
      S.form.auth = '';
      S.dialog = { kind: 'auth', cls: r.refused.class, capability, args, error: null };
    } else if (r.refused) {
      toast(refusedText(r.refused), 'bad');
    } else {
      applyPreferences();
      if (r.ok.kind !== 'done') toast(t('k.saved'));
    }
    render();
    return r;
  }

  async function authorise() {
    const d = S.dialog;
    if (!d || d.kind !== 'auth') return;
    S.busy = true; render();
    const request = Object.assign({ capability: d.capability }, d.args || {});
    let binding = '';
    try { binding = await invoke('binding', { request }); } catch (e) { binding = ''; }
    const reply = await ask(d.cls === 'CONFIRM' ? { request: 'confirm', binding } : { request: 'reauthenticate', binding, password: S.form.auth });
    S.form.auth = '';
    S.busy = false;
    if (!reply) { d.error = t('why.SESSION_SERVICE'); return render(); }
    if (reply.reply !== 'granted') { d.error = whyText(reply.reason); return render(); }
    const r = await call(d.capability, d.args, reply.grant);
    S.dialog = null;
    if (r.refused) toast(refusedText(r.refused), 'bad'); else { applyPreferences(); toast(t('k.saved')); }
    render();
  }

  const whyText = (reason) => t('why.' + ((reason && reason.code) || 'SESSION_SERVICE'), { n: reason && reason.retry_after_secs });

  function toast(text, tone) {
    S.toast = { text, tone: tone || 'ok' };
    document.getElementById('live').textContent = text.replace(/&[a-z]+;/g, ' ');
    clearTimeout(toast.timer);
    toast.timer = setTimeout(() => { S.toast = null; render(); }, 5000);
  }

  // ── factos da barra e da entrada
  function netFact() {
    const n = got('network.status');
    if (!n) return { icon: 'netoff', label: t('bar.nonet'), tone: 'warn', key: 'fact.net.none' };
    const up = n.interfaces.filter((i) => i.carrier && i.addresses.length);
    if (n.wifi && n.wifi.connected) return { icon: 'wifi', label: esc(n.wifi.connected), tone: 'ok', key: 'fact.net.wifi' };
    if (up.some((i) => i.kind === 'ETHERNET') || up.length) return { icon: 'eth', label: t('bar.wired'), tone: 'ok', key: 'fact.net.eth' };
    return { icon: 'netoff', label: t('bar.nonet'), tone: 'warn', key: 'fact.net.none' };
  }
  const battery = () => { const p = got('power.status'); return p && p.battery ? p.battery : null; };
  const output = () => { const a = got('audio.status'); return a ? a.outputs.find((d) => d.default) || a.outputs[0] || null : null; };
  const instanceHealth = () => { const h = got('system.health'); const i = h && h.items.find((x) => x.part === 'instance'); return i ? i.health : 'UNKNOWN'; };
  const TONE = { OK: 'ok', DEGRADED: 'warn', FAILED: 'bad', UNKNOWN: 'mute' };

  // ── ecrãs fora da sessão
  function corner() {
    const n = netFact();
    const b = battery();
    return `<div class="sx-brand">${ic('mark')}${t('brand')}</div>
      <div class="sx-topright"><span>${ic(n.icon)}${n.label}</span>${b ? `<span>${ic('battery')}${b.charge}%</span>` : ''}</div>
      <div class="sx-corner">
        <button class="sx-round" type="button" data-act="contrast" aria-pressed="${document.documentElement.classList.contains('sx-contrast')}" aria-label="${t('q.contrast')}">${ic('person', 'sx-icon--lg')}</button>
        <button class="sx-round" type="button" data-act="power" aria-label="${t('q.power')}">${ic('power', 'sx-icon--lg')}</button>
      </div>`;
  }
  const langButton = () => `<button class="sx-btn sx-btn--glass" type="button" data-act="languages" aria-label="${t('lang.choose')}">${ic('globe')}${t('lang.' + S.lang)}</button>`;

  function login() {
    const n = netFact();
    const h = instanceHealth();
    const time = got('time.status');
    const k = got('keyboard.status');
    const ended = S.session && S.session.ended && S.session.ended !== 'SIGNED_OUT' ? t('ended.' + S.session.ended) : '';
    const facts = S.serviceDown
      ? `<li data-tone="bad">${t('fact.service')}</li>`
      : `<li data-tone="${TONE[h]}">${t('fact.instance.' + h)}</li><li data-tone="${n.tone}">${t(n.key)}</li>${time ? `<li data-tone="${time.synchronised ? 'ok' : 'warn'}">${t(time.synchronised ? 'fact.clock.yes' : 'fact.clock.no')}</li>` : ''}`;
    const body = S.step === 3
      ? `<div><div class="sx-card__step">${t('login.stepnew')}</div><h1 id="login-h">${t('login.hnew')}</h1></div>
         ${alert('info', t('login.newnote'))}
         ${S.error ? alert('bad', t(S.error[0], S.error[1])) : ''}
         ${field('new1', t('login.new1'), { type: 'password' })}
         ${field('new2', t('login.new2'), { type: 'password' })}
         ${btn('gold', t(S.busy ? 'login.busy' : 'login.go'), null, { submit: true, lg: true })}
         ${btn('ghost', t('login.back'), 'login-back')}`
      : S.step === 2
      ? `<div><div class="sx-card__step">${t('login.step2')}</div><h1 id="login-h">${t('login.h2')}</h1></div>
         ${S.error ? alert('bad', t(S.error[0], S.error[1])) : ''}
         ${field('code', t('login.code'), { code: true })}
         ${btn('gold', t(S.busy ? 'login.busy' : 'login.go'), null, { submit: true, lg: true })}
         ${btn('ghost', t('login.back'), 'login-back')}`
      : `<div><div class="sx-card__step">${t('login.step1')}</div><h1 id="login-h">${t('login.h')}</h1></div>
         ${S.sessionDown ? alert('bad', t('why.SESSION_SERVICE')) : ''}
         ${ended && !S.error ? alert('warn', ended) : ''}
         ${S.error ? alert('bad', t(S.error[0], S.error[1])) : ''}
         ${field('user', t('login.user'))}
         ${field('pass', t('login.pass'), { type: 'password' })}
         ${k ? `<div class="sx-card__row"><span>${ic('keyboard', 'sx-icon--sm')}${t('login.kbd', { layout: k.layout + (k.variant ? ' · ' + k.variant : '') })}</span></div>` : ''}
         ${btn('gold', t(S.busy ? 'login.busy' : 'login.go'), null, { submit: true, lg: true })}
         <p class="sx-card__note">${t('login.note')}</p>`;
    return `<main class="sx-screen">${corner()}
      <div class="sx-login">
        <div class="sx-hero">${ic('mark')}<h1>${t('login.title')}</h1><p>${t('login.lead')}</p><ul class="sx-facts">${facts}</ul></div>
        <form class="sx-card" data-form="login" aria-labelledby="login-h">${body}</form>
      </div>
      <div class="sx-cornerleft">${langButton()}</div>${overlay()}</main>`;
  }

  function lock() {
    const s = S.session;
    const wait = s.retry_after_secs || 0;
    const u = got('update.status');
    return `<main class="sx-screen">${corner()}
      <div class="sx-lock">
        <div class="sx-lock__time" aria-hidden="true">${clock()}</div>
        <div class="sx-lock__date">${esc(today())}</div>
        <div class="sx-lock__who">
          <div class="sx-avatar" aria-hidden="true">${esc(initials(s.member_name))}</div>
          <div class="sx-lock__name">${esc(s.member_name)}</div>
          <div class="sx-lock__sub">${t('lock.locked')}</div>
          <form class="sx-lock__form" data-form="unlock">
            <input class="sx-input" id="f-unlock" data-f="unlock" type="password" value="${esc(S.form.unlock)}" aria-label="${t('login.pass')}" placeholder="${t('login.pass')}" autocomplete="off"${S.busy ? ' disabled' : ''}>
            ${btn('gold', t('lock.unlock'), null, { submit: true, lg: true, disabled: wait > 0 })}
          </form>
          ${S.error ? `<p class="sx-lock__error" role="alert">${t(S.error[0], S.error[1])}</p>` : wait > 0 ? `<p class="sx-lock__error" role="status">${t('lock.wait', { n: wait })}</p>` : ''}
          <p class="sx-lock__note">${t(s.core === 'ONLINE' ? 'lock.note.online' : 'lock.note.offline')}</p>
          <button class="sx-btn sx-btn--glass sx-lock__out" type="button" data-act="signout">${t('lock.out')}</button>
        </div>
      </div>
      <div class="sx-cornerleft">${u && u.phase === 'RESTART_REQUIRED' ? `<div class="sx-pill" data-tone="warn">${ic('update')}${t('k.phase.RESTART_REQUIRED')}</div>` : ''}${langButton()}</div>${overlay()}</main>`;
  }

  // ── a sessão: barra, painel rápido, Sistema
  function bar() {
    const s = S.session;
    const n = netFact();
    const b = battery();
    const o = output();
    const banner = s.core !== 'ONLINE' ? `<span class="sx-bar__banner" data-tone="warn" role="status">${t('bar.offline')}</span>` : '';
    return `<header class="sx-bar">
      <div class="sx-bar__left">${ic('mark', 'sx-icon--lg')}
        <button class="sx-bar__btn" type="button" data-act="workspace">${ic('grid', 'sx-icon--sm')}${t('bar.apps')}</button>
        <button class="sx-bar__btn sx-bar__btn--line" type="button" data-act="open-system"${S.panel === 'system' ? ' aria-current="true"' : ''}>${ic('gear', 'sx-icon--sm')}${t('bar.system')}</button>
      </div>${banner}
      <div class="sx-bar__right">
        <button class="sx-bar__tray" type="button" data-act="tray" aria-expanded="${S.panel === 'quick'}" aria-label="${t('bar.tray')}">
          <span>${ic(n.icon)}${n.label}</span>${o ? `<span>${ic('sound')}${o.muted ? t('k.muted') : o.volume + '%'}</span>` : ''}${b ? `<span>${ic('battery')}${b.charge}%</span>` : ''}
          <span class="sx-bar__clock">${clock()}</span><span class="sx-bar__me" aria-hidden="true">${esc(initials(s.member_name))}</span>
        </button>
      </div></header>`;
  }

  function quick() {
    const s = S.session;
    const n = netFact();
    const o = output();
    const d = got('display.status');
    const a = got('accessibility.status');
    const tile = (icon, title, sub, o2 = {}) => `<${o2.act ? 'button type="button" data-act="' + o2.act + '"' + (o2.arg != null ? ' data-arg="' + o2.arg + '"' : '') : 'div'} class="sx-tile"${o2.on ? ' data-on="true"' : ''}${o2.off ? ' data-off="true"' : ''}>${ic(icon, 'sx-icon--lg')}<span><span class="sx-tile__t">${title}</span><br><span class="sx-tile__s">${sub}</span></span></${o2.act ? 'button' : 'div'}>`;
    const slider = (icon, label, pct, act) => `<label class="sx-slider">${ic(icon, 'sx-icon--lg')}<span class="sx-sr">${label}</span><input type="range" min="0" max="100" step="5" value="${pct}" data-act="${act}" aria-label="${label}"><span class="sx-slider__v">${pct}%</span></label>`;
    return `<div class="sx-scrim" data-act="close-panel"><section class="sx-panel" role="dialog" aria-label="${t('q.title')}" data-stop>
      <div class="sx-panel__head"><span class="sx-panel__title">${t('q.title')}</span><button class="sx-iconbtn" type="button" data-act="close-panel" aria-label="${t('q.close')}">${ic('x')}</button></div>
      <div class="sx-tiles">
        ${tile(n.icon, t('q.net'), n.label, { on: n.tone === 'ok', act: 'nav', arg: 'network' })}
        ${tile('sound', t('q.sound'), o ? (o.muted ? t('k.muted') : esc(o.name)) : t('q.unavailable'), { off: !o, act: 'nav', arg: 'sound' })}
        ${tile('person', t('q.contrast'), t(a && a.high_contrast ? 'q.on' : 'q.off'), { on: a && a.high_contrast, act: 'contrast' })}
        ${tile('lock', t('q.lock'), esc(s.member_name), { act: 'lock' })}
      </div>
      ${d && d.brightness != null ? slider('sun', t('q.bright'), d.brightness, 'brightness') : ''}
      ${o ? slider('sound', t('q.volume'), o.volume, 'volume') : ''}
      <div class="sx-panel__foot">
        <div class="sx-panel__who"><span class="sx-bar__me" aria-hidden="true">${esc(initials(s.member_name))}</span><div><div class="sx-panel__name">${esc(s.member_name)}</div><div class="sx-row__s">${t(s.core === 'ONLINE' ? 'q.signedin' : 'bar.offline')}</div></div></div>
        <div class="sx-panel__acts">
          <button class="sx-iconbtn" type="button" data-act="open-system" aria-label="${t('q.settings')}">${ic('gear')}</button>
          <button class="sx-iconbtn" type="button" data-act="power" aria-label="${t('q.power')}">${ic('power')}</button>
        </div>
      </div></section></div>`;
  }

  const NAV = [['home', 'home'], ['network', 'wifi'], ['sound', 'sound'], ['displays', 'monitor'], ['power', 'battery'], ['input', 'keyboard'], ['language', 'globe'], ['time', 'clock'], ['accessibility', 'person'], ['privacy', 'eye'], ['security', 'shield'], ['storage', 'disk'], ['updates', 'update'], ['diagnostics', 'pulse'], ['about', 'info']];
  const READS = {
    home: ['system.health', 'network.status', 'power.status', 'update.status'], network: ['network.status'], sound: ['audio.status'], displays: ['display.status'],
    power: ['power.status'], input: ['keyboard.status'], language: ['locale.status'], time: ['time.status'], accessibility: ['accessibility.status'],
    privacy: ['privacy.status'], security: ['system.info', 'remote_access.status', 'recovery.status'], storage: ['storage.status'], updates: ['update.status', 'recovery.status'],
    diagnostics: ['system.health', 'system.capabilities'], about: ['system.info'],
  };
  // O que uma leitura dá: o valor, ou a razão dita em vez dele.
  const need = (cap, draw) => { const v = S.sys[cap]; return !v ? [note(t('k.loading'))] : v.refused ? [note(refusedText(v.refused))] : draw(v); };
  const yes = (b, y, n) => t(b ? y : n);

  const PAGES = {
    home: () => [
      group(t('g.health'), need('system.health', (h) => h.items.map((i) => row(t('part.' + i.part), status(TONE[i.health], t('health.' + i.health)))))),
      group(t('g.session'), [row(t('k.member'), esc(S.session.member_name)), row(t('k.instance'), status(S.session.core === 'ONLINE' ? 'ok' : 'warn', t(S.session.core === 'ONLINE' ? 'k.online' : 'k.offline'))), row(t('k.lock.now'), btn('', t('q.lock'), 'lock', { icon: 'lock' }))]),
      group(t('g.release'), need('update.status', (u) => [row(t('k.current'), `<span class="sx-mono">${esc(u.current_release || t('k.unknown'))}</span>`), row(t('k.phase'), status(u.phase === 'UP_TO_DATE' ? 'ok' : 'warn', t('k.phase.' + u.phase)))])),
    ],
    network: () => [
      group(t('g.connections'), need('network.status', (n) => n.interfaces.map((i) => row(`${esc(i.name)} · ${t('k.kind.' + i.kind)}`, status(i.carrier ? 'ok' : 'mute', t(i.carrier ? 'k.carrier.yes' : 'k.carrier.no')), { sub: `<span class="sx-mono">${i.addresses.length ? i.addresses.map(esc).join(' · ') : t('k.noaddr')}${i.mac ? ' · ' + esc(i.mac) : ''}</span>` })))),
      group(t('g.routing'), need('network.status', (n) => [row(t('k.connectivity'), status(n.connectivity === 'LOCAL' ? 'ok' : 'warn', t('k.conn.' + n.connectivity))), row(t('k.gateway'), `<span class="sx-mono">${esc(n.gateway || '—')}</span>`), row(t('k.dns'), `<span class="sx-mono">${n.dns.length ? n.dns.map(esc).join(' · ') : '—'}</span>`), row(t('k.wifi'), n.wifi ? (n.wifi.connected ? esc(n.wifi.connected) : t(n.wifi.enabled ? 'q.on' : 'q.off')) : t('avail.ABSENT'), { sub: t('r.NOT_IMPLEMENTED') })])),
    ],
    sound: () => {
      const list = (kind) => need('audio.status', (a) => (a[kind].length ? a[kind].map((d) => row(esc(d.name) + (d.default ? ` · ${t('k.default')}` : ''), d.muted ? t('k.muted') : d.volume + '%')) : [note(t('k.none'))]));
      return [group(t('g.outputs'), list('outputs')), group(t('g.inputs'), list('inputs'))];
    },
    displays: () => [
      group(t('g.displays'), need('display.status', (d) => (d.outputs.length ? d.outputs.map((o) => row(esc(o.name), `${o.width} × ${o.height} · ${(o.refresh_mhz / 1000).toFixed(0)} Hz · ${o.scale_percent}%`, { sub: t(o.internal ? 'k.internal' : 'k.external') })) : [note(t('k.none'))]).concat(d.brightness != null ? [row(t('k.brightness'), `<label class="sx-slider"><input type="range" min="5" max="100" step="5" value="${d.brightness}" data-act="brightness" aria-label="${t('k.brightness')}"><span class="sx-slider__v">${d.brightness}%</span></label>`)] : []))),
    ],
    power: () => [
      group(t('g.power'), need('power.status', (p) => [row(t('k.source'), t('k.source.' + p.source))].concat(p.battery ? [row(t('k.battery'), `${p.battery.charge}%${p.battery.charging ? ' · ' + t('k.charging') : ''}`), p.battery.health != null ? row(t('k.battery.health'), p.battery.health + '%') : ''] : []))),
      group(t('g.actions'), need('power.status', (p) => [p.can_suspend ? row(t('power.suspend'), btn('', t('power.suspend'), 'power-do', { arg: 'suspend' })) : '', row(t('power.restart'), btn('', t('power.restart'), 'power-ask', { arg: 'restart', icon: 'restart' })), row(t('power.shutdown'), btn('danger', t('power.shutdown'), 'power-ask', { arg: 'shutdown', icon: 'power' }))])),
    ],
    input: () => [group(t('g.keyboard'), need('keyboard.status', (k) => [row(t('k.layout'), `<select class="sx-select" data-act="keyboard" aria-label="${t('k.layout')}">${k.offered.map((l) => `<option value="${esc(l)}"${l === k.layout ? ' selected' : ''}>${esc(l)}</option>`).join('')}</select>`, { sub: k.variant ? esc(k.variant) : '' })]))],
    language: () => [group(t('g.language'), need('locale.status', (l) => [row(t('k.language'), `<select class="sx-select" data-act="language" aria-label="${t('k.language')}">${['pt', 'en', 'fr'].map((c) => `<option value="${c}"${c === l.language ? ' selected' : ''}>${t('lang.' + c)}</option>`).join('')}</select>`), row(t('k.locale'), `<span class="sx-mono">${esc(l.system_locale)}</span>`)]))],
    time: () => [group(t('g.time'), need('time.status', (x) => [row(t('k.timezone'), `<span class="sx-mono">${esc(x.timezone)}</span>`, { op: true }), row(t('k.auto'), toggle(x.automatic, 'time-auto', { label: t('k.auto') }), { op: true }), row(t('k.synced'), status(x.synchronised ? 'ok' : 'warn', yes(x.synchronised, 'k.synced.yes', 'k.synced.no')))]))],
    accessibility: () => [group(t('g.vision'), need('accessibility.status', (a) => [
      row(t('k.textsize'), `<select class="sx-select" data-act="a11y-scale" aria-label="${t('k.textsize')}">${[100, 115, 130, 150].map((p) => `<option value="${p}"${p === a.text_scale_percent ? ' selected' : ''}>${p}%</option>`).join('')}</select>`),
      row(t('k.contrast'), toggle(a.high_contrast, 'a11y-contrast', { label: t('k.contrast') })),
      row(t('k.motion'), toggle(a.reduce_motion, 'a11y-motion', { label: t('k.motion') })),
      row(t('k.reader'), status('mute', t('k.reader.v'))),
    ]))],
    privacy: () => [group(t('g.devices'), need('privacy.status', (p) => [
      row(t('k.mic'), toggle(p.microphone_allowed, 'privacy-mic', { label: t('k.mic') }), { sub: p.microphone_in_use ? t('k.inuse') : '' }),
      row(t('k.cam'), toggle(p.camera_allowed, 'privacy-cam', { label: t('k.cam') }), { sub: p.camera_in_use ? t('k.inuse') : '' }),
    ]))],
    security: () => [
      group(t('g.session'), [row(t('k.member'), esc(S.session.member_name)), row(t('k.lock.now'), btn('', t('q.lock'), 'lock', { icon: 'lock' })), row(t('power.signout'), btn('', t('power.signout'), 'signout', { icon: 'logout' }))]),
      group(t('g.boot'), need('system.info', (i) => [row(t('k.uefi'), yes(i.uefi, 'k.uefi.yes', 'k.uefi.no')), row(t('k.secureboot'), status(i.secure_boot === 'ENABLED' ? 'ok' : 'mute', t('k.sb.' + i.secure_boot)))])),
      group(t('g.remote'), need('remote_access.status', (r) => [row(t('k.ssh'), toggle(r.ssh_enabled, 'ssh', { label: t('k.ssh') }), { op: true, sub: `${t('k.ssh.sub')} · ${t(r.ssh_active ? 'k.ssh.running' : 'k.ssh.stopped')}` })])),
    ],
    storage: () => [
      group(t('g.volumes'), need('storage.status', (s) => (s.volumes.length ? s.volumes.map((v) => `<div class="sx-row"><div class="sx-row__l sx-grow"><div class="sx-row__t"><span class="sx-mono">${esc(v.mount_point)}</span> · ${esc(v.filesystem)}</div><div class="sx-row__s">${t('k.free', { free: bytes(v.free_bytes), total: bytes(v.total_bytes) })}</div><div class="sx-meter"><progress max="${v.total_bytes}" value="${v.total_bytes - v.free_bytes}" aria-label="${esc(v.mount_point)}"></progress></div></div></div>`) : [note(t('k.none'))]))),
      group(t('g.disks'), need('storage.status', (s) => (s.disks.length ? s.disks.map((d) => row(`<span class="sx-mono">${esc(d.name)}</span>${d.model ? ' · ' + esc(d.model) : ''}`, bytes(d.size_bytes), { sub: d.removable ? t('k.removable') : '' })) : [note(t('k.none'))]))),
    ],
    updates: () => [
      group(t('g.release'), need('update.status', (u) => [row(t('k.current'), `<span class="sx-mono">${esc(u.current_release || t('k.unknown'))}</span>`), u.pending_release ? row(t('k.pending'), `<span class="sx-mono">${esc(u.pending_release)}</span>`) : '', row(t('k.phase'), status(u.phase === 'UP_TO_DATE' ? 'ok' : 'warn', t('k.phase.' + u.phase))), row(t('k.signing'), u.signing === 'DEV_ONLY' ? status('warn', t('k.signing.dev')) : status('ok', esc(u.signing)))])),
      group(t('g.recovery'), need('recovery.status', (r) => [row(t('k.rollback'), r.rollback_available ? `<span class="sx-mono">${esc(r.previous_release || '')}</span>` : t('k.rollback.none'))])),
    ],
    diagnostics: () => [
      group(t('g.health'), need('system.health', (h) => h.items.map((i) => row(t('part.' + i.part), status(TONE[i.health], t('health.' + i.health)))))),
      group(t('g.capabilities'), need('system.capabilities', (c) => c.hardware.filter((h) => h.hardware !== 'NONE').map((h) => row(`<span class="sx-mono">${esc(h.hardware)}</span>`, status(h.availability.state === 'PRESENT' ? 'ok' : 'mute', t('avail.' + h.availability.state)))))),
      group(t('g.actions'), [row(t('k.collect'), btn('', t('k.collect'), 'collect', { icon: 'pulse' }), { op: true, sub: t('p.diagnostics.sub') })]),
    ],
    about: () => [
      group(t('g.machine'), need('system.info', (i) => [row(t('k.hostname'), `<span class="sx-mono">${esc(i.hostname)}</span>`), row(t('k.cpu'), `${esc(i.cpu_model)} · ${i.cpu_count}`), row(t('k.memory'), bytes(i.memory_mib * 1048576)), row(t('k.arch'), `<span class="sx-mono">${esc(i.arch)}</span>`)])),
      group(t('g.system'), need('system.info', (i) => [row(t('k.release'), `<span class="sx-mono">${esc(i.release_id || t('k.unknown'))}</span>`), row(t('k.devbuild'), i.development_build ? status('warn', t('k.devbuild.yes')) : t('k.devbuild.no')), row(t('k.base'), esc(i.base_system)), row(t('k.kernel'), `<span class="sx-mono">${esc(i.kernel)}</span>`)])),
    ],
  };

  function system() {
    const info = got('system.info');
    const items = NAV.map(([key, icon]) => `<button class="sx-nav__item" type="button" data-act="nav" data-arg="${key}"${key === S.page ? ' aria-current="page"' : ''}>${ic(icon)}${t('nav.' + key)}</button>`).join('');
    const groups = PAGES[S.page]();
    const half = Math.ceil(groups.length / 2);
    return `<div class="sx-system"><nav class="sx-nav" aria-label="${t('nav.title')}">
        <div class="sx-nav__head"><span class="sx-nav__title">${t('nav.title')}</span><span class="sx-nav__host">${info ? esc(info.hostname) : ''}</span></div>${items}</nav>
      <main class="sx-page"><div class="sx-page__head"><div><h1 id="page-h" tabindex="-1">${t('nav.' + S.page)}</h1><p class="sx-page__sub">${t('p.' + S.page + '.sub')}</p></div>${btn('', t('nav.close'), 'close-panel', { icon: 'x' })}</div>
        <div class="sx-cols"><div class="sx-col">${groups.slice(0, half).join('')}</div><div class="sx-col">${groups.slice(half).join('')}</div></div></main></div>`;
  }

  function shell() {
    const body = S.panel === 'quick' ? quick() : S.panel === 'system' ? system() : S.dialog ? '<div class="sx-scrim"></div>' : '';
    return `<div class="sx-screen">${bar()}${body}${overlay()}</div>`;
  }

  // ── diálogos e avisos
  function overlay() {
    const d = S.dialog;
    let inner = '';
    if (d && d.kind === 'power') {
      const active = S.session && S.session.state === 'ACTIVE';
      inner = `<h2 id="dlg-h">${t('power.h')}</h2><div class="sx-menu">
        ${active ? btn('', t('q.lock'), 'lock', { icon: 'lock' }) + btn('', t('power.signout'), 'signout', { icon: 'logout' }) : ''}
        ${btn('', t('power.suspend'), 'power-do', { arg: 'suspend', icon: 'moon' })}${btn('', t('power.restart'), 'power-ask', { arg: 'restart', icon: 'restart' })}${btn('danger', t('power.shutdown'), 'power-ask', { arg: 'shutdown', icon: 'power' })}</div>
        <div class="sx-dialog__acts">${btn('ghost', t('power.cancel'), 'dialog-cancel')}</div>`;
    } else if (d && d.kind === 'power-confirm') {
      inner = `<h2 id="dlg-h">${t('power.confirm.' + d.action)}</h2><p>${t('power.confirm.note')}</p>
        <div class="sx-dialog__acts">${btn('ghost', t('power.cancel'), 'dialog-cancel')}${btn(d.action === 'shutdown' ? 'danger' : 'pri', t('power.' + d.action), 'power-do', { arg: d.action })}</div>`;
    } else if (d && d.kind === 'languages') {
      inner = `<h2 id="dlg-h">${t('lang.choose')}</h2><div class="sx-menu">${['pt', 'en', 'fr'].map((c) => btn(c === S.lang ? 'pri' : '', t('lang.' + c), 'lang', { arg: c })).join('')}</div>
        <div class="sx-dialog__acts">${btn('ghost', t('power.cancel'), 'dialog-cancel')}</div>`;
    } else if (d && d.kind === 'auth') {
      const head = `<div class="sx-dialog__trust">${ic('shield')}${t('auth.trusted')}</div>`;
      const error = d.error ? alert('bad', d.error) : '';
      if (d.cls === 'CONFIRM') {
        inner = `${head}<h2 id="dlg-h">${t('auth.confirm.h')}</h2><p class="sx-mono">${esc(d.capability)}</p>${error}
          <form data-form="auth" class="sx-dialog__acts">${btn('ghost', t('power.cancel'), 'dialog-cancel')}${btn('pri', t('auth.ok'), null, { submit: true })}</form>`;
      } else if (d.cls === 'LOCAL_REAUTH') {
        inner = `${head}<h2 id="dlg-h">${t('auth.reauth.h')}</h2><p>${t('auth.reauth.note')}</p><p class="sx-mono">${esc(d.capability)}</p>${error}
          <form data-form="auth">${field('auth', t('login.pass'), { type: 'password' })}<div class="sx-dialog__acts">${btn('ghost', t('power.cancel'), 'dialog-cancel')}${btn('pri', t('auth.ok'), null, { submit: true })}</div></form>`;
      } else {
        inner = `${head}<h2 id="dlg-h">${t('auth.reauth.h')}</h2>${alert('info', t('auth.stepup.note'))}<div class="sx-dialog__acts">${btn('pri', t('q.close'), 'dialog-cancel')}</div>`;
      }
    }
    const dialog = inner ? `<div class="sx-veil"><div class="sx-dialog" role="dialog" aria-modal="true" aria-labelledby="dlg-h">${inner}</div></div>` : '';
    const toastHtml = S.toast ? `<div class="sx-toast" data-tone="${S.toast.tone}">${ic(S.toast.tone === 'bad' ? 'warn' : 'check')}${S.toast.text}</div>` : '';
    return dialog + toastHtml;
  }

  // ── desenhar
  const view = () => (!S.session ? 'wait' : S.session.state === 'ACTIVE' ? 'shell' : S.session.state === 'LOCKED' ? 'lock' : 'login');
  let lastHtml = '';
  let lastFocusKey = '';
  let lastFocusId = '';
  function render() {
    const v = view();
    const html = v === 'shell' ? shell() : v === 'lock' ? lock() : v === 'login' ? login() : `<main class="sx-screen">${S.sessionDown ? `<div class="sx-login"><div class="sx-card">${alert('bad', t('why.SESSION_SERVICE'))}</div></div>` : ''}</main>`;
    const full = !(v === 'shell' && !S.panel && !S.dialog);
    if (full !== S.full) { S.full = full; Promise.resolve(invoke('surface', { full })).catch(() => {}); }
    if (html === lastHtml) return;
    lastHtml = html;
    const root = document.getElementById('root');
    const active = document.activeElement;
    // Um campo desactivado enquanto se espera perde o foco: o que conta é
    // onde a pessoa estava, e é para lá que o foco volta.
    const keep = active && active.id ? { id: active.id, start: active.selectionStart, end: active.selectionEnd } : lastFocusId ? { id: lastFocusId } : null;
    root.innerHTML = html;
    // O foco segue o que mudou: um diálogo novo, uma página nova, um ecrã novo;
    // de resto fica onde a pessoa o tinha.
    const focusKey = v + '|' + (S.dialog ? S.dialog.kind + (S.dialog.cls || '') : '') + '|' + (v === 'shell' ? S.panel + ':' + S.page : S.step);
    const moved = focusKey !== lastFocusKey;
    lastFocusKey = focusKey;
    let target = null;
    if (moved) {
      const scope = root.querySelector('.sx-dialog') || root.querySelector('.sx-panel') || root;
      target = (S.panel === 'system' && !S.dialog && root.querySelector('#page-h')) || scope.querySelector('input:not([disabled])') || scope.querySelector('button:not([disabled])');
    } else if (keep) {
      target = document.getElementById(keep.id);
      if (target && target.disabled) target = null;
    }
    if (target) {
      target.focus();
      if (!moved && keep && keep.start != null && target.setSelectionRange) { try { target.setSelectionRange(keep.start, keep.end); } catch (e) { /* tipos sem selecção */ } }
    }
  }

  // ── acções
  async function refresh() {
    await ask({ request: 'state' });
    const base = ['system.health', 'network.status', 'power.status', 'time.status', 'locale.status', 'accessibility.status', 'keyboard.status', 'audio.status', 'display.status', 'update.status'];
    const page = view() === 'shell' && S.panel === 'system' ? READS[S.page] : [];
    await read(Array.from(new Set(base.concat(page))));
    render();
  }

  async function signIn() {
    if (S.busy) return;
    if (!S.form.user.trim() || !S.form.pass) return;
    if (S.step === 3) {
      // O que se pode dizer sem perguntar a ninguém diz-se já; a política é do Core.
      if (S.form.new1 !== S.form.new2) { S.error = ['login.mismatch']; return render(); }
      if (S.form.new1.length < 15) { S.error = ['why.PASSWORD_NOT_ACCEPTED']; return render(); }
    }
    S.busy = true; S.error = null; render();
    const reply = await ask({ request: 'sign_in', username: S.form.user.trim(), password: S.form.pass, second_factor: S.form.code ? S.form.code : null, new_password: S.form.new1 ? S.form.new1 : null });
    S.busy = false;
    if (!reply) { S.error = ['why.SESSION_SERVICE']; }
    else if (reply.reply === 'refused') {
      const code = reply.reason.code;
      if (code === 'SECOND_FACTOR_REQUIRED' && S.step !== 2) { S.step = 2; S.form.code = ''; }
      else if (code === 'PASSWORD_CHANGE_REQUIRED' && S.step === 1) { S.step = 3; S.form.new1 = ''; S.form.new2 = ''; }
      else {
        S.error = ['why.' + code, { n: reply.reason.retry_after_secs }];
        S.form.code = '';
        if (S.step === 1) S.form.pass = '';
        if (code === 'PASSWORD_NOT_ACCEPTED') { S.step = 3; S.form.new1 = ''; S.form.new2 = ''; }
      }
    } else { S.form = { user: '', pass: '', code: '', unlock: '', auth: '', new1: '', new2: '' }; S.step = 1; S.panel = null; }
    render();
    if (view() === 'shell') { refresh(); invoke('open_workspace').catch(() => {}); }
  }

  async function unlock() {
    if (S.busy || !S.form.unlock) return;
    S.busy = true; S.error = null; render();
    const reply = await ask({ request: 'unlock', password: S.form.unlock });
    S.form.unlock = '';
    S.busy = false;
    if (!reply) S.error = ['why.SESSION_SERVICE'];
    else if (reply.reply === 'refused') { S.error = ['why.' + reply.reason.code, { n: reply.reason.retry_after_secs }]; await ask({ request: 'state' }); }
    render();
  }

  const ACTIONS = {
    'login-back': () => { S.step = 1; S.error = null; S.form.code = ''; S.form.pass = ''; S.form.new1 = ''; S.form.new2 = ''; },
    languages: () => { S.dialog = { kind: 'languages' }; },
    lang: async (arg) => { S.lang = arg; S.langChosen = true; S.dialog = null; document.documentElement.lang = LOCALE(); render(); const r = await call('locale.configure', { language: arg }); if (!r.refused) S.langChosen = false; },
    contrast: async () => { const a = got('accessibility.status') || { text_scale_percent: 100, high_contrast: false, reduce_motion: false }; const r = await call('accessibility.configure', Object.assign({}, a, { high_contrast: !a.high_contrast })); if (r.refused) document.documentElement.classList.toggle('sx-contrast'); else applyPreferences(); },
    power: () => { S.dialog = { kind: 'power' }; },
    'power-ask': (arg) => { S.dialog = { kind: 'power-confirm', action: arg }; },
    'power-do': async (arg) => { S.dialog = null; await change('power.' + arg); },
    'dialog-cancel': () => { S.dialog = null; S.form.auth = ''; },
    signout: async () => { S.dialog = null; S.panel = null; S.error = null; await ask({ request: 'sign_out' }); },
    lock: async () => { S.dialog = null; S.panel = null; S.error = null; await ask({ request: 'lock' }); },
    tray: () => { S.panel = S.panel === 'quick' ? null : 'quick'; },
    'open-system': () => { S.panel = S.panel === 'system' ? null : 'system'; if (S.panel) read(READS[S.page]).then(render); },
    'close-panel': () => { S.panel = null; },
    nav: (arg) => { S.panel = 'system'; S.page = arg; read(READS[arg]).then(render); },
    workspace: async () => { S.panel = null; render(); let ok = false; try { ok = await invoke('open_workspace'); } catch (e) { ok = false; } if (!ok) toast(t('bar.wsfail'), 'bad'); },
    collect: async () => { const r = await change('diagnostics.collect'); if (r.ok) toast(t('k.collected')); },
    brightness: (arg) => change('display.set_brightness', { percent: Number(arg) }),
    volume: (arg) => { const o = output(); return o ? change('audio.set_volume', { target: 'OUTPUT', percent: Number(arg) }) : null; },
    keyboard: (arg) => change('keyboard.configure', { layout: arg, variant: null }),
    language: async (arg) => { S.lang = arg; S.langChosen = false; await change('locale.configure', { language: arg }); },
    'time-auto': (arg) => change('time.configure', { timezone: null, automatic: arg === '1' }),
    'a11y-scale': (arg) => change('accessibility.configure', Object.assign({}, got('accessibility.status'), { text_scale_percent: Number(arg) })),
    'a11y-contrast': (arg) => change('accessibility.configure', Object.assign({}, got('accessibility.status'), { high_contrast: arg === '1' })),
    'a11y-motion': (arg) => change('accessibility.configure', Object.assign({}, got('accessibility.status'), { reduce_motion: arg === '1' })),
    'privacy-mic': (arg) => change('privacy.set_microphone', { allowed: arg === '1' }),
    'privacy-cam': (arg) => change('privacy.set_camera', { allowed: arg === '1' }),
    ssh: (arg) => change('remote_access.configure', { ssh_enabled: arg === '1' }),
  };

  async function act(name, arg) {
    const f = ACTIONS[name];
    if (!f) return;
    await f(arg);
    render();
  }

  document.addEventListener('click', (e) => {
    const el = e.target.closest('[data-act]');
    if (!el || el.disabled) return;
    if (el.matches('select, input')) return;
    // Um clique dentro do painel não é um clique no fundo que o fecha.
    if (el.classList.contains('sx-scrim') && e.target.closest('[data-stop]')) return;
    act(el.dataset.act, el.dataset.arg);
  });
  document.addEventListener('change', (e) => {
    const el = e.target.closest('select[data-act], input[type="range"][data-act]');
    if (el) act(el.dataset.act, el.value);
  });
  document.addEventListener('input', (e) => {
    const el = e.target.closest('[data-f]');
    if (el) S.form[el.dataset.f] = el.value;
  });
  document.addEventListener('submit', (e) => {
    e.preventDefault();
    const form = e.target.closest('[data-form]');
    if (!form) return;
    if (form.dataset.form === 'login') signIn();
    else if (form.dataset.form === 'unlock') unlock();
    else if (form.dataset.form === 'auth') authorise();
  });
  document.addEventListener('keydown', (e) => {
    if (e.key !== 'Escape') return;
    if (S.dialog) { S.dialog = null; S.form.auth = ''; render(); }
    else if (S.panel) { S.panel = null; render(); }
  });
  document.addEventListener('focusin', (e) => { if (e.target && e.target.id) lastFocusId = e.target.id; });
  document.addEventListener('contextmenu', (e) => e.preventDefault());

  refresh();
  setInterval(refresh, 5000);
  setInterval(render, 15000);
})();
