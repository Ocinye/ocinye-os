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
    fb: { step: 'region', tz: null, kb: null, net: 'AUTOMATIC', iface: '' }, // o primeiro arranque, passo a passo
    form: { user: '', pass: '', code: '', unlock: '', auth: '', new1: '', new2: '', iname: '', domain: '', pname: '', pemail: '', aname: '', aemail: '', wifipass: '', naddr: '', nprefix: '24', ngw: '', ndns: '' },
    net: { iface: null, mode: 'DHCP' }, // o formulário de endereçamento: interface e modo escolhidos
    dist: { research: true, business: false, education: false, personal: false },
    credential: null,       // a credencial do primeiro administrador, enquanto está no ecrã
    enrolKey: null,         // a chave do autenticador, enquanto se inscreve o segundo factor
    recovery: null,         // os códigos de recuperação, enquanto estão no ecrã
    step: 1,                // 1 conta · 2 segundo factor · 3 palavra-passe nova · 4 inscrever o segundo factor
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
  const btn = (kind, label, act, o = {}) => `<button class="sx-btn${kind ? ' sx-btn--' + kind : ''}${o.lg ? ' sx-btn--lg' : ''}" type="${o.submit ? 'submit' : 'button'}"${act ? ` data-act="${act}"` : ''}${o.arg != null ? ` data-arg="${esc(o.arg)}"` : ''}${o.disabled || S.busy ? ' disabled' : ''}${o.af ? ' data-af' : ''}${o.id ? ` id="${o.id}"` : ''}>${o.icon ? ic(o.icon) : ''}${label}</button>`;
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
  const refusedText = (reason) => (reason && reason.code === 'UPDATE_REFUSED' ? t('ur.' + reason.update) : t('r.' + ((reason && reason.code) || 'SYSTEM_SERVICE')));
  const STATUS_OF = { wifi_scan: 'wifi.scan', health: 'system.health', info: 'system.info', inventory: 'system.capabilities', network: 'network.status', audio: 'audio.status', display: 'display.status', power: 'power.status', locale: 'locale.status', time: 'time.status', keyboard: 'keyboard.status', accessibility: 'accessibility.status', privacy: 'privacy.status', storage: 'storage.status', update: 'update.status', update_history: 'update.history', base: 'base.status', remote_access: 'remote_access.status', recovery: 'recovery.status', bluetooth: 'bluetooth.status', activation: 'activation.status' };

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

  // Depois de uma alteração, o que mais mudou com ela e se lê outra vez: a
  // lista de redes Wi-Fi não vem na resposta de ligar, desligar ou esquecer.
  function afterChange(capability) {
    if (capability.indexOf('wifi.') === 0 && capability !== 'wifi.scan') read(['wifi.scan']).then(render);
    if (capability === 'privacy.set_microphone') read(['audio.status']).then(render);
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
      afterChange(capability);
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
    if (r.refused) toast(refusedText(r.refused), 'bad'); else { applyPreferences(); toast(t('k.saved')); afterChange(d.capability); }
    render();
  }

  // Procurar (redes, dispositivos): uma leitura que demora; nada a confirmar.
  async function look(capability) {
    S.busy = true; render();
    const r = await call(capability);
    S.busy = false;
    if (r.refused) toast(refusedText(r.refused), 'bad');
    render();
  }

  // Ligar a uma rede Wi-Fi: a palavra-passe sai do formulário no momento em
  // que é enviada, e não fica em lado nenhum da interface.
  async function joinWifi() {
    const d = S.dialog;
    if (!d || d.kind !== 'wifi' || S.busy) return;
    const passphrase = d.ask ? S.form.wifipass : '';
    S.form.wifipass = '';
    if (d.ask && (passphrase.length < 8 || passphrase.length > 63)) { d.error = t('wifi.pass.len'); return render(); }
    S.busy = true; d.error = null; render();
    const r = await call('wifi.connect', { ssid: d.ssid, passphrase: d.ask ? passphrase : null });
    S.busy = false;
    if (r.refused) { d.error = refusedText(r.refused); return render(); }
    S.dialog = null;
    toast(t('wifi.joined', { ssid: d.ssid }));
    afterChange('wifi.connect');
    render();
  }

  // Endereçamento de uma interface com fios: os valores vão como foram
  // escritos; é o serviço que os valida e recusa.
  function addressing() {
    const f = S.form;
    if (S.net.mode === 'DHCP') return { mode: 'DHCP' };
    const dns = f.ndns.split(',').map((x) => x.trim()).filter(Boolean);
    return { mode: 'STATIC', address: f.naddr.trim(), prefix: Number(f.nprefix), gateway: f.ngw.trim(), dns };
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

  // ── primeiro arranque: este computador cria a sua Instância
  const activation = () => got('activation.status');
  const pairing = () => `<p class="sx-card__note">${t('act.pairing')}</p>`;
  // Os passos do primeiro arranque. A língua, o fuso e a rede aplicam-se ao
  // sair do segundo passo (activation.configure); a Instância só no fim.
  const FB = ['region', 'network', 'instance', 'people', 'summary'];
  const ZONES = ['Africa/Luanda', 'Europe/Lisbon', 'Atlantic/Azores', 'Atlantic/Madeira', 'Africa/Maputo', 'Africa/Kinshasa', 'Africa/Lagos', 'America/Sao_Paulo', 'Europe/Paris', 'Europe/Brussels', 'Europe/London', 'UTC'];
  const kbName = (code) => (T['kbn.' + code] ? t('kbn.' + code) : `<span class="sx-mono">${esc(code)}</span>`);
  function fbDefaults() {
    const time = got('time.status');
    const k = got('keyboard.status');
    const n = got('network.status');
    if (S.fb.tz === null) S.fb.tz = time && ZONES.includes(time.timezone) ? time.timezone : 'Africa/Luanda';
    if (S.fb.kb === null) S.fb.kb = k ? k.layout : 'pt';
    if (!S.fb.iface && n) { const w = n.interfaces.find((i) => i.kind === 'ETHERNET'); S.fb.iface = w ? w.name : ''; }
  }
  function setup(a) {
    fbDefaults();
    const at = FB.indexOf(S.fb.step);
    const head = (h, lead) => `<div><div class="sx-card__step">${t('fb.of', { n: at + 1, m: FB.length })} · ${t('act.step')}</div><h1 id="login-h">${h}</h1></div>${lead ? `<p>${lead}</p>` : ''}
      ${a.phase === 'FAILED' ? alert('bad', t('act.failed')) : ''}
      ${S.error ? alert('bad', t(S.error[0], S.error[1])) : ''}`;
    const nav = (next, o = {}) => `<div class="sx-fbnav">${at > 0 ? btn('ghost', t('login.back'), 'fb-back') : ''}${btn('gold', t(S.busy ? 'login.busy' : next), null, { submit: true, lg: true, af: o.af })}</div>`;
    const seg = (cur, act, values, label) => `<div class="sx-seg" role="radiogroup" aria-label="${label}">${values.map(([v, l]) => `<button class="sx-seg__opt" type="button" role="radio" aria-checked="${cur === v}" data-act="${act}" data-arg="${esc(v)}"${S.busy ? ' disabled' : ''}>${l}</button>`).join('')}</div>`;
    const select = (id, cur, values, label) => `<label class="sx-field" for="fb-${id}"><span class="sx-field__l">${label}</span><select class="sx-input" id="fb-${id}" data-fb="${id}"${S.busy ? ' disabled' : ''}>${values.map(([v, l]) => `<option value="${esc(v)}"${cur === v ? ' selected' : ''}>${l}</option>`).join('')}</select></label>`;
    if (S.fb.step === 'region') {
      const k = got('keyboard.status');
      const layouts = [...new Set([S.fb.kb].concat(k ? k.offered : []))];
      return `${head(t('fb.region.h'), t('fb.region.lead'))}
        <div class="sx-field"><span class="sx-field__l">${t('lang.choose')}</span>${seg(S.lang, 'fb-lang', [['pt', t('lang.pt')], ['en', t('lang.en')], ['fr', t('lang.fr')]], t('lang.choose'))}</div>
        <div class="sx-form2">${select('tz', S.fb.tz, ZONES.map((z) => [z, esc(z.replace('_', ' '))]), t('fb.tz'))}${select('kb', S.fb.kb, layouts.map((c) => [c, T['kbn.' + c] ? t('kbn.' + c) : esc(c)]), t('fb.kb'))}</div>
        ${nav('login.go', { af: true })}${pairing()}`;
    }
    if (S.fb.step === 'network') {
      const n = got('network.status');
      const ifaces = n ? n.interfaces : [];
      const wired = ifaces.filter((i) => i.kind === 'ETHERNET');
      const line = (i) => `<li><span>${ic(i.kind === 'WIFI' ? 'wifi' : 'eth', 'sx-icon--sm')}<span class="sx-mono">${esc(i.name)}</span></span><span>${i.carrier ? t('fb.link') : t('fb.nolink')}${i.addresses.length ? ` · <span class="sx-mono">${i.addresses.map(esc).join(' ')}</span>` : ''}</span></li>`;
      const fixed = S.fb.net === 'STATIC' && wired.length
        ? `<div class="sx-form2">${select('iface', S.fb.iface, wired.map((i) => [i.name, esc(i.name)]), t('fb.iface'))}${field('naddr', t('fb.addr'))}</div>
           <div class="sx-form2">${field('nprefix', t('fb.prefix'))}${field('ngw', t('fb.gw'))}</div>${field('ndns', t('fb.dns'))}`
        : '';
      return `${head(t('fb.net.h'), t('fb.net.lead'))}
        <ul class="sx-ifaces">${ifaces.length ? ifaces.map(line).join('') : `<li>${t('fb.noiface')}</li>`}</ul>
        ${wired.length ? `<div class="sx-field"><span class="sx-field__l">${t('fb.wired')}</span>${seg(S.fb.net, 'fb-net', [['AUTOMATIC', t('fb.auto')], ['STATIC', t('fb.static')]], t('fb.wired'))}</div>` : ''}
        ${fixed}
        ${ifaces.some((i) => i.kind === 'WIFI') ? `<p class="sx-card__note">${t('fb.wifi.later')}</p>` : ''}
        ${nav('login.go', { af: S.fb.net !== 'STATIC' })}`;
    }
    if (S.fb.step === 'instance') {
      const check = (key) => `<label class="sx-check"><input type="checkbox" data-act="dist" data-arg="${key}"${S.dist[key] ? ' checked' : ''}${S.busy ? ' disabled' : ''}><span>${t('act.dist.' + key)}</span></label>`;
      return `${head(t('act.h'), t('act.lead'))}
        <div class="sx-form2">${field('iname', t('act.iname'))}${field('domain', t('act.domain'))}</div>
        <fieldset class="sx-checks"><legend class="sx-field__l">${t('act.dists')}</legend>${['research', 'business', 'education', 'personal'].map(check).join('')}</fieldset>
        ${nav('login.go')}`;
    }
    if (S.fb.step === 'people') {
      return `${head(t('fb.people.h'), t('fb.people.lead'))}
        <div class="sx-form2">${field('pname', t('act.pname'))}${field('pemail', t('act.pemail'))}</div>
        <div class="sx-form2">${field('aname', t('act.aname'))}${field('aemail', t('act.aemail'))}</div>
        ${nav('login.go')}`;
    }
    const f = S.form;
    const dists = Object.keys(S.dist).filter((k) => S.dist[k]).map((k) => t('act.dist.' + k)).join(', ');
    const netv = S.fb.net === 'STATIC' ? `<span class="sx-mono">${esc(S.fb.iface)} · ${esc(f.naddr.trim())}/${esc(f.nprefix.trim())}</span>` : t('fb.auto');
    return `${head(t('fb.sum.h'), t('fb.sum.lead'))}
      <dl class="sx-kv">
        <dt>${t('lang.choose')}</dt><dd>${t('lang.' + S.lang)}</dd>
        <dt>${t('fb.tz')}</dt><dd>${esc(S.fb.tz)}</dd>
        <dt>${t('fb.kb')}</dt><dd>${kbName(S.fb.kb)}</dd>
        <dt>${t('fb.wired')}</dt><dd>${netv}</dd>
        <dt>${t('act.iname')}</dt><dd>${esc(f.iname.trim())}</dd>
        <dt>${t('fb.domain')}</dt><dd class="sx-mono">${esc(f.domain.trim().toLowerCase())}</dd>
        <dt>${t('act.dists')}</dt><dd>${dists}</dd>
        <dt>${t('act.pname')}</dt><dd>${esc(f.pname.trim())} · <span class="sx-mono">${esc(f.pemail.trim())}</span></dd>
        <dt>${t('act.aname')}</dt><dd>${esc(f.aname.trim())} · <span class="sx-mono">${esc(f.aemail.trim())}</span></dd>
      </dl>
      ${alert('info', t('fb.sum.note'))}
      ${nav('act.go', { af: true })}`;
  }
  const running = (a) => `<div><div class="sx-card__step">${t('act.step')}</div><h1 id="login-h" tabindex="-1">${t('act.running.h')}</h1></div>
      <p role="status">${t('act.running.step', { n: a.steps_begun })}</p>
      <div class="sx-meter sx-meter--busy"><progress aria-label="${t('act.running.h')}"></progress></div>
      <p class="sx-card__note">${t('act.running.note')}</p>`;
  const reveal = () => (S.credential
    ? `<div><div class="sx-card__step">${t('act.step')}</div><h1 id="login-h" tabindex="-1">${t('act.cred.h')}</h1></div>
       ${alert('warn', t('act.cred.once'))}
       <dl class="sx-kv"><dt>${t('login.user')}</dt><dd class="sx-mono">${esc(S.credential.username)}</dd><dt>${t('act.cred.pass')}</dt><dd class="sx-mono sx-secret">${esc(S.credential.password)}</dd></dl>
       <p class="sx-card__note">${t('act.cred.note')}</p>
       ${btn('gold', t('act.cred.done'), 'credential-done', { lg: true })}`
    : `<div><div class="sx-card__step">${t('act.step')}</div><h1 id="login-h" tabindex="-1">${t('act.done.h')}</h1></div>
       <p>${t('act.done.lead')}</p>
       ${S.error ? alert('bad', t(S.error[0], S.error[1])) : ''}
       ${btn('gold', t('act.cred.show'), 'credential-show', { lg: true })}`);

  function login() {
    const n = netFact();
    const h = instanceHealth();
    const time = got('time.status');
    const k = got('keyboard.status');
    const ended = S.session && S.session.ended && S.session.ended !== 'SIGNED_OUT' ? t('ended.' + S.session.ended) : '';
    const facts = S.serviceDown
      ? `<li data-tone="bad">${t('fact.service')}</li>`
      : `<li data-tone="${TONE[h]}">${t('fact.instance.' + h)}</li><li data-tone="${n.tone}">${t(n.key)}</li>${time ? `<li data-tone="${time.synchronised ? 'ok' : 'warn'}">${t(time.synchronised ? 'fact.clock.yes' : 'fact.clock.no')}</li>` : ''}`;
    const grouped = (k) => esc(String(k).replace(/(.{4})/g, '$1 ').trim());
    const body = S.recovery
      ? `<div><div class="sx-card__step">${t('enrol.step')}</div><h1 id="login-h" tabindex="-1">${t('enrol.codes.h')}</h1></div>
         ${alert('warn', t('enrol.codes.once'))}
         <ul class="sx-codes">${S.recovery.map((c) => `<li class="sx-mono">${esc(c)}</li>`).join('')}</ul>
         <p class="sx-card__note">${t('enrol.codes.note')}</p>
         ${btn('gold', t('enrol.codes.done'), 'recovery-done', { lg: true })}`
      : S.step === 4
      ? `<div><div class="sx-card__step">${t('enrol.step')}</div><h1 id="login-h">${t('enrol.h')}</h1></div>
         <p>${t('enrol.lead')}</p>
         ${S.error ? alert('bad', t(S.error[0], S.error[1])) : ''}
         ${S.enrolKey ? `<dl class="sx-kv"><dt>${t('enrol.key')}</dt><dd class="sx-mono sx-secret">${grouped(S.enrolKey)}</dd></dl>` : alert('info', t('login.busy'))}
         ${field('code', t('enrol.code'), { code: true })}
         ${btn('gold', t(S.busy ? 'login.busy' : 'login.go'), null, { submit: true, lg: true, disabled: !S.enrolKey })}
         ${btn('ghost', t('login.back'), 'login-back')}`
      : S.step === 3
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
    const a = activation();
    const stage = S.recovery ? 'login' : S.credential || (a && a.phase === 'ACTIVATED' && a.credential_waiting) ? 'reveal'
      : a && a.phase === 'RUNNING' ? 'running'
        : a && (a.phase === 'READY' || a.phase === 'FAILED') ? 'setup' : 'login';
    const card = stage === 'reveal' ? reveal() : stage === 'running' ? running(a) : stage === 'setup' ? setup(a)
      : (a && a.phase === 'NOT_OFFERED' ? alert('info', t('act.none')) : '') + body;
    return `<main class="sx-screen">${corner()}
      <div class="sx-login">
        <div class="sx-hero">${ic('mark')}<h1>${t('login.title')}</h1><p>${t(stage === 'login' ? 'login.lead' : stage === 'reveal' ? 'act.hero.done' : 'act.hero')}</p><ul class="sx-facts">${stage === 'login' ? facts : ''}</ul></div>
        <form class="sx-card${stage === 'setup' ? ' sx-card--setup' : ''}" data-form="${stage === 'setup' ? 'setup' : stage === 'login' ? 'login' : 'none'}" aria-labelledby="login-h">${card}</form>
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
      <div class="sx-cornerleft">${u && u.phase === 'AWAITING_REBOOT' ? `<div class="sx-pill" data-tone="warn">${ic('update')}${t('up.AWAITING_REBOOT')}</div>` : ''}${langButton()}</div>${overlay()}</main>`;
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
      <div class="sx-bar__right">${updNotice() ? `<button class="sx-chip" type="button" data-act="upd-pop" aria-haspopup="dialog">${ic('update', 'sx-icon--sm')}${t('u.chip')}</button>` : ''}
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

  const NAV = [['home', 'home'], ['network', 'wifi'], ['bluetooth', 'bt'], ['sound', 'sound'], ['displays', 'monitor'], ['power', 'battery'], ['input', 'keyboard'], ['language', 'globe'], ['time', 'clock'], ['accessibility', 'person'], ['privacy', 'eye'], ['security', 'shield'], ['storage', 'disk'], ['updates', 'update'], ['diagnostics', 'pulse'], ['about', 'info']];
  const READS = {
    home: ['system.health', 'network.status', 'power.status', 'update.status'], network: ['network.status'], bluetooth: ['bluetooth.status'], sound: ['audio.status', 'privacy.status'], displays: ['display.status'],
    power: ['power.status'], input: ['keyboard.status'], language: ['locale.status'], time: ['time.status'], accessibility: ['accessibility.status'],
    privacy: ['privacy.status'], security: ['system.info', 'remote_access.status', 'recovery.status'], storage: ['storage.status'], updates: ['update.status', 'update.history', 'base.status', 'recovery.status'],
    diagnostics: ['system.health', 'system.capabilities'], about: ['system.info'],
  };
  // O que uma leitura dá: o valor, ou a razão dita em vez dele.
  const need = (cap, draw) => { const v = S.sys[cap]; return !v ? [note(t('k.loading'))] : v.refused ? [note(refusedText(v.refused))] : draw(v); };
  const yes = (b, y, n) => t(b ? y : n);
  const acts = (items) => `<span class="sx-acts">${items.filter(Boolean).join('')}</span>`;

  // Wi-Fi: o rádio, a rede em uso e as redes ao alcance, pela ordem do sinal.
  function wifiRows(n) {
    if (!n.wifi) return [note(t('k.wifi.none'))];
    const rows = [row(t('k.wifi.radio'), toggle(n.wifi.enabled, 'wifi-radio', { label: t('k.wifi.radio') }))];
    if (!n.wifi.enabled) return rows;
    if (n.wifi.connected) rows.push(row(esc(n.wifi.connected), btn('', t('wifi.disconnect'), 'wifi-disconnect'), { sub: t('wifi.connected') }));
    rows.push(row(t('wifi.networks'), btn('', t('wifi.scan'), 'wifi-scan', { icon: 'search' })));
    const v = S.sys['wifi.scan'];
    if (!v) rows.push(note(t('wifi.notscanned')));
    else if (v.refused) rows.push(note(refusedText(v.refused)));
    else if (!v.networks.length) rows.push(note(t('wifi.empty')));
    else v.networks.forEach((w, i) => rows.push(row(esc(w.ssid),
      w.connected ? status('ok', t('wifi.connected')) : acts([w.known ? btn('ghost', t('wifi.forget'), 'wifi-forget', { arg: i }) : '', btn('', t('wifi.connect'), 'wifi-pick', { arg: i, disabled: w.security === 'ENTERPRISE' })]),
      { sub: `${t('wifi.sec.' + w.security)} · ${t('wifi.signal', { n: w.signal })}${w.known ? ' · ' + t('wifi.known') : ''}` })));
    return rows;
  }

  // Endereçamento de uma interface com fios. A do Wi-Fi vem da rede.
  function addressRows(n) {
    const wired = n.interfaces.filter((i) => i.kind !== 'WIFI');
    if (!wired.length) return [note(t('addr.none'))];
    if (!wired.some((i) => i.name === S.net.iface)) S.net.iface = wired[0].name;
    const select = (act, label, options, value) => `<select class="sx-select" data-act="${act}" aria-label="${label}"${S.busy ? ' disabled' : ''}>${options.map(([v, l]) => `<option value="${esc(v)}"${v === value ? ' selected' : ''}>${l}</option>`).join('')}</select>`;
    const rows = [
      row(t('k.iface'), select('net-iface', t('k.iface'), wired.map((i) => [i.name, esc(i.name)]), S.net.iface), { op: true }),
      row(t('k.mode'), select('net-mode', t('k.mode'), [['DHCP', t('k.mode.DHCP')], ['STATIC', t('k.mode.STATIC')]], S.net.mode)),
    ];
    if (S.net.mode === 'STATIC') {
      rows.push(`<div class="sx-addr"><div class="sx-form2">${field('naddr', t('k.address'))}${field('nprefix', t('k.prefix'))}</div><div class="sx-form2">${field('ngw', t('k.gateway'))}${field('ndns', t('k.dnslist'))}</div></div>`);
    }
    rows.push(row(t('addr.apply'), btn('', t('addr.apply'), 'net-apply'), { sub: t('addr.note') }));
    return rows;
  }

  function btRow(d) {
    const a = { arg: d.address };
    return row(esc(d.name), acts([
      !d.paired ? btn('', t('bt.pair'), 'bt-pair', a) : '',
      d.paired && !d.connected ? btn('', t('bt.connect'), 'bt-connect', a) : '',
      d.connected ? btn('', t('bt.disconnect'), 'bt-disconnect', a) : '',
      d.paired ? btn('ghost', t('bt.remove'), 'bt-remove', a) : '',
    ]), { sub: `${t('bt.class.' + d.class)} · <span class="sx-mono">${esc(d.address)}</span>${d.connected ? ' · ' + t('bt.connected') : d.paired ? ' · ' + t('bt.paired') : ''}` });
  }

  // ── actualizações (ADR-0032): o motor decide; esta página mostra e pede
  const BUSY_PHASES = ['CHECKING', 'DOWNLOADING', 'DOWNLOADED', 'VERIFYING', 'VERIFIED', 'PREFLIGHT', 'STAGED', 'INSTALLING', 'ACTIVATING', 'BOOT_VALIDATING', 'COMMITTING', 'ROLLING_BACK'];
  const FAILED_PHASES = ['DOWNLOAD_FAILED', 'VERIFICATION_FAILED', 'PREFLIGHT_FAILED', 'INSTALL_FAILED', 'ACTIVATION_FAILED', 'BOOT_VALIDATION_FAILED'];
  const AT_REST_QUIET = ['IDLE', 'COMPLETED'];
  const phaseTone = (p) => (AT_REST_QUIET.includes(p) ? 'ok' : p === 'RECOVERY_REQUIRED' || FAILED_PHASES.includes(p) ? 'bad' : 'warn');
  const when = (iso) => (iso ? new Intl.DateTimeFormat(LOCALE(), { dateStyle: 'medium', timeStyle: 'short' }).format(new Date(iso)) : '');
  const note3 = (n) => (n ? esc(n[S.lang] || n.pt) : '');
  // A notificação de actualização: uma por estado e versão, nunca repetida.
  const updKey = (u) => (u ? u.phase + ':' + (u.available ? u.available.version : '') : '');
  const NOTIFY_PHASES = ['UPDATE_AVAILABLE', 'AWAITING_INSTALL', 'AWAITING_REBOOT', 'ROLLED_BACK', 'RECOVERY_REQUIRED'].concat(FAILED_PHASES);
  const updNotice = () => { const u = got('update.status'); return u && NOTIFY_PHASES.includes(u.phase) && S.updSeen !== updKey(u) ? u : null; };

  function updatesPage() {
    const u = got('update.status');
    const busy = u && BUSY_PHASES.includes(u.phase);
    const a = u && u.available;
    const acts = (u) => {
      const p = u.phase;
      if (p === 'UPDATE_AVAILABLE') return btn('gold', t('u.download'), 'upd-download', { icon: 'down' });
      if (p === 'AWAITING_INSTALL') return btn('ghost', t('u.cancel'), 'upd-cancel') + btn('gold', t('u.install'), 'upd-install', { icon: 'update' });
      if (p === 'AWAITING_REBOOT') return btn('ghost', t('u.later'), 'upd-later') + btn('gold', t('u.restart'), 'power-ask', { arg: 'restart', icon: 'restart' });
      if (p === 'DOWNLOADING') return btn('ghost', t('u.cancel'), 'upd-cancel');
      if (FAILED_PHASES.includes(p) && a) return btn('', t('u.retry'), p === 'INSTALL_FAILED' || p === 'ACTIVATION_FAILED' || p === 'BOOT_VALIDATION_FAILED' ? 'upd-check' : 'upd-download', { icon: 'restart' });
      if (busy) return '';
      return btn('', t('u.check'), 'upd-check', { icon: 'restart', disabled: !u.repository_configured });
    };
    const state = need('update.status', (u) => {
      const title = u.phase === 'UPDATE_AVAILABLE' && a ? t('u.available', { v: a.version }) : t('up.' + u.phase);
      const sub = u.phase === 'IDLE' || u.phase === 'COMPLETED'
        ? (u.last_check ? t('u.lastcheck', { when: when(u.last_check) }) : u.repository_configured ? t('u.nocheck') : t('u.norepo'))
        : u.reason ? t('ur.' + u.reason) : '';
      const pct = u.progress && u.progress.total ? Math.min(100, Math.round((u.progress.done / u.progress.total) * 100)) : null;
      const meter = u.progress ? `<div class="sx-row"><div class="sx-row__l sx-grow"><div class="sx-row__s">${t('u.progress', { pct: pct == null ? '…' : pct, done: bytes(u.progress.done), total: bytes(u.progress.total) })}</div><div class="sx-meter"><progress max="${u.progress.total || 1}" value="${u.progress.done}" aria-label="${t('up.' + u.phase)}"></progress></div></div></div>` : busy ? `<div class="sx-row"><div class="sx-row__l sx-grow"><div class="sx-meter sx-meter--busy"><progress aria-label="${t('up.' + u.phase)}"></progress></div></div></div>` : '';
      const actions = acts(u);
      const head = `<div class="sx-row sx-row--hero"><div class="sx-row__l"><div class="sx-row__t">${status(phaseTone(u.phase), '')} ${title}</div>${sub ? `<div class="sx-row__s">${sub}</div>` : ''}</div>${actions ? `<div class="sx-acts">${actions}</div>` : ''}</div>`;
      const extra = [];
      if (u.phase === 'RECOVERY_REQUIRED') extra.push(alert('bad', t('u.recovery')));
      if (u.phase === 'ROLLED_BACK') extra.push(alert('info', t('u.rolledback')));
      if (u.signing === 'DEV_ONLY') extra.push(note(`${t('k.signing')}: ${status('warn', t('k.signing.dev'))}`));
      return [head, meter].concat(extra);
    });
    const release = a ? group(t('u.g.available'), [
      row(t('u.version'), `<span class="sx-mono">${esc(a.version)}</span>`, { sub: t('u.number', { n: a.release_number }) }),
      row(t('u.size'), bytes(a.download_bytes), { sub: t('u.space', { n: bytes(a.install_bytes) }) }),
      row(t('u.restartq'), a.reboot_required ? status('warn', t('u.yes')) : t('u.no')),
      a.security ? row(t('u.security'), status('warn', t('u.yes'))) : '',
      row(t('u.migrations'), t('um.' + a.migrations.kind)),
      a.synthetic ? row(t('u.synthetic'), status('warn', t('u.synthetic.v'))) : '',
      `<div class="sx-row"><div class="sx-row__l sx-grow"><div class="sx-row__t">${t('u.notes')}</div><div class="sx-row__s">${note3(a.notes)}</div></div></div>`,
      row(t('u.source'), t('us.' + a.source)),
    ]) : '';
    const version = group(t('u.g.version'), need('update.status', (u) => [
      row(t('u.current'), `<span class="sx-mono">${esc(u.current ? u.current.version : t('k.unknown'))}</span>`, { sub: u.current ? t('u.number', { n: u.current.release_number }) + (u.current.activated_at ? ' · ' + when(u.current.activated_at) : '') : '' }),
      row(t('u.channel'), `<span class="sx-nowrap">${t('ch.' + u.channel)}</span>`, { op: true, sub: t('u.channel.sub') }),
    ]));
    const details = u ? group(t('u.g.details'), [row(t('u.details'), btn('ghost', t(S.updDetails ? 'u.details.hide' : 'u.details.show'), 'upd-details', { icon: 'chev' })),
      S.updDetails ? `<div class="sx-row"><dl class="sx-kv"><dt>${t('k.phase')}</dt><dd class="sx-mono">${esc(u.phase)}</dd><dt>${t('u.reason')}</dt><dd class="sx-mono">${esc(u.reason || '—')}</dd><dt>${t('u.outcome')}</dt><dd>${u.last_outcome ? t('uo.' + u.last_outcome) : '—'}</dd><dt>${t('u.repo')}</dt><dd>${t(u.repository_configured ? 'u.yes' : 'u.no')}</dd></dl></div>` : '']) : '';
    const offline = u && u.offline_bundles.length ? group(t('u.g.offline'), u.offline_bundles.map((b) => row(`<span class="sx-mono">${esc(b.name)}</span>`, btn('', t('u.import'), 'upd-import', { arg: b.name, disabled: busy }), { op: true, sub: bytes(b.bytes) + ' · ' + t(b.removable ? 'u.removable' : 'u.inbox') }))) : '';
    const back = u && u.previous ? group(t('u.g.back'), [row(t('u.previous'), btn('', t('u.rollback'), 'upd-rollback', { disabled: busy || !['IDLE', 'COMPLETED', 'ROLLED_BACK', 'UPDATE_AVAILABLE', 'RECOVERY_REQUIRED'].includes(u.phase), icon: 'restart' }), { op: true, sub: `<span class="sx-mono">${esc(u.previous.version)}</span>` }), note(t('u.rollback.note'))]) : '';
    const history = group(t('u.g.history'), need('update.history', (h) => (h.entries.length ? h.entries.slice(0, 10).map((e) => row(`<span class="sx-mono">${esc(e.version)}</span>`, status(e.outcome === 'COMMITTED' ? 'ok' : e.outcome === 'ROLLED_BACK' ? 'warn' : 'bad', t('uo.' + e.outcome)), { sub: when(e.finished_at) + (e.from ? ' · ' + t('u.from', { v: e.from }) : '') + (e.reason ? ' · ' + t('ur.' + e.reason) : '') })) : [note(t('u.history.none'))])));
    const base = group(t('u.g.base'), need('base.status', (b) => [
      row(t('u.base.pending'), b.last_check ? t('u.base.count', { n: b.pending, s: b.security }) : t('u.nocheck'), { sub: b.last_check ? t('u.lastcheck', { when: when(b.last_check) }) : t('u.base.sub') }),
      b.reboot_required ? row(t('u.restartq'), status('warn', t('u.base.reboot'))) : '',
      b.phase === 'INTERRUPTED' ? alert('bad', t('u.base.interrupted')) : '',
      !b.sources_approved ? alert('bad', t('u.base.sources')) : '',
      row(t('u.base.actions'), `<div class="sx-acts">${btn('', t('u.check'), 'base-check', { disabled: busy || !b.sources_approved })}${btn('', t('u.base.apply'), 'base-apply', { disabled: busy || !b.sources_approved || !b.pending })}</div>`, { op: true }),
      note(t('u.base.limits')),
    ]));
    const auto = u ? group(t('u.g.auto'), [
      row(t('u.autocheck'), toggle(u.auto_check, 'upd-autocheck', { label: t('u.autocheck'), disabled: !u.repository_configured }), { op: true, sub: t('u.autocheck.sub') }),
      row(t('u.autodownload'), toggle(u.auto_download, 'upd-autodownload', { label: t('u.autodownload'), disabled: !u.repository_configured }), { op: true, sub: t('u.autodownload.sub') }),
    ]) : '';
    return [group(t('u.g.state'), state), release, version, offline, back, history, base, auto, details].filter(Boolean);
  }

  const PAGES = {
    home: () => [
      group(t('g.health'), need('system.health', (h) => h.items.map((i) => row(t('part.' + i.part), status(TONE[i.health], t('health.' + i.health)))))),
      group(t('g.session'), [row(t('k.member'), esc(S.session.member_name)), row(t('k.instance'), status(S.session.core === 'ONLINE' ? 'ok' : 'warn', t(S.session.core === 'ONLINE' ? 'k.online' : 'k.offline'))), row(t('k.lock.now'), btn('', t('q.lock'), 'lock', { icon: 'lock' }))]),
      group(t('g.release'), need('update.status', (u) => [row(t('u.current'), `<span class="sx-mono">${esc(u.current ? u.current.version : t('k.unknown'))}</span>`), row(t('k.phase'), status(phaseTone(u.phase), t('up.' + u.phase)), { sub: u.available && !AT_REST_QUIET.includes(u.phase) ? `<span class="sx-mono">${esc(u.available.version)}</span>` : '' })])),
    ],
    network: () => [
      group(t('g.wifi'), need('network.status', wifiRows)),
      group(t('g.connections'), need('network.status', (n) => n.interfaces.map((i) => row(`${esc(i.name)} · ${t('k.kind.' + i.kind)}`, status(i.carrier ? 'ok' : 'mute', t(i.carrier ? 'k.carrier.yes' : 'k.carrier.no')), { sub: `<span class="sx-mono">${i.addresses.length ? i.addresses.map(esc).join(' · ') : t('k.noaddr')}${i.mac ? ' · ' + esc(i.mac) : ''}</span>` })))),
      group(t('g.routing'), need('network.status', (n) => [row(t('k.connectivity'), status(n.connectivity === 'LOCAL' ? 'ok' : 'warn', t('k.conn.' + n.connectivity))), row(t('k.gateway'), `<span class="sx-mono">${esc(n.gateway || '—')}</span>`), row(t('k.dns'), `<span class="sx-mono">${n.dns.length ? n.dns.map(esc).join(' · ') : '—'}</span>`)])),
      group(t('g.addressing'), need('network.status', addressRows)),
    ],
    bluetooth: () => [
      group(t('g.bluetooth'), need('bluetooth.status', (b) => [
        row(t('k.bt.power'), toggle(b.powered, 'bt-power', { label: t('k.bt.power') })),
        row(t('bt.scan'), btn('', t('bt.scan'), 'bt-scan', { icon: 'search', disabled: !b.powered }), { sub: t('bt.scan.sub') }),
      ])),
      group(t('g.btdevices'), need('bluetooth.status', (b) => (b.devices.length ? b.devices.map(btRow) : [note(t('bt.none'))]).concat([note(t('bt.note'))]))),
    ],
    sound: () => {
      const p = got('privacy.status');
      const blocked = p && !p.microphone_allowed;
      const slider = (d, act, label) => `<label class="sx-slider"><input type="range" min="0" max="100" step="5" value="${d.volume}" data-act="${act}" aria-label="${label}"${S.busy ? ' disabled' : ''}><span class="sx-slider__v">${d.volume}%</span></label>`;
      const outputs = need('audio.status', (a) => (a.outputs.length ? a.outputs.flatMap((d) => (d.default
        ? [row(`${esc(d.name)} · ${t('k.default')}`, slider(d, 'volume', t('q.volume'))), row(t('k.sound.on'), toggle(!d.muted, 'mute-out', { label: t('k.sound.on') }))]
        : [row(esc(d.name), btn('', t('snd.use'), 'snd-output', { arg: d.id }), { sub: d.muted ? t('k.muted') : d.volume + '%' })])) : [note(t('k.none'))]));
      const inputs = need('audio.status', (a) => {
        const d = a.inputs.find((x) => x.default) || a.inputs[0];
        if (!d) return [note(t('k.none'))];
        return [
          row(esc(d.name), slider(d, 'volume-in', t('k.capture.level'))),
          row(t('k.capture'), toggle(!d.muted && !blocked, 'mute-in', { label: t('k.capture'), disabled: blocked }), { sub: blocked ? t('snd.micblocked') : '' }),
          note(t('snd.input.note')),
        ];
      });
      return [group(t('g.outputs'), outputs), group(t('g.inputs'), inputs)];
    },
    displays: () => [
      group(t('g.displays'), need('display.status', (d) => (d.outputs.length ? d.outputs.map((o) => row(esc(o.name), [o.width ? `${o.width} × ${o.height}` : t('k.nomode'), o.refresh_mhz ? `${(o.refresh_mhz / 1000).toFixed(0)} Hz` : '', o.scale_percent ? `${o.scale_percent}%` : ''].filter(Boolean).join(' · '), { sub: t(o.internal ? 'k.internal' : 'k.external') + (o.enabled ? '' : ' · ' + t('k.disabled')) })) : [note(t('k.none'))]).concat(d.brightness != null ? [row(t('k.brightness'), `<label class="sx-slider"><input type="range" min="5" max="100" step="5" value="${d.brightness}" data-act="brightness" aria-label="${t('k.brightness')}"${S.busy ? ' disabled' : ''}><span class="sx-slider__v">${d.brightness}%</span></label>`)] : []).concat([note(t('disp.note'))]))),
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
      row(t('k.mic'), toggle(p.microphone_allowed, 'privacy-mic', { label: t('k.mic') }), { sub: !p.microphone_present ? t('k.absent') : p.microphone_in_use ? t('k.inuse') : '' }),
      row(t('k.cam'), toggle(p.camera_allowed, 'privacy-cam', { label: t('k.cam') }), { sub: !p.camera_present ? t('k.absent') : p.camera_in_use ? t('k.inuse') : '' }),
      note(t('privacy.note')),
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
    updates: () => updatesPage(),
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
    } else if (d && d.kind === 'update') {
      const u = got('update.status');
      const a = u && u.available;
      const reboot = u && u.phase === 'AWAITING_REBOOT';
      inner = `<h2 id="dlg-h">${t('u.pop.t')}</h2><p>${u ? (u.phase === 'UPDATE_AVAILABLE' && a ? t('u.available', { v: a.version }) : t('up.' + u.phase)) : ''}${u && u.reason ? ' — ' + t('ur.' + u.reason) : ''}</p>
        ${u ? `<dl class="sx-kv"><dt>${t('u.pop.os')}</dt><dd class="sx-mono">${esc(u.current ? u.current.version : '—')}${a ? ' → ' + esc(a.version) : ''}</dd>${a ? `<dt>${t('u.restartq')}</dt><dd>${t(a.reboot_required ? 'u.yes' : 'u.no')}</dd>` : ''}</dl>` : ''}
        <div class="sx-dialog__acts">${btn('ghost', t('u.later'), 'upd-later')}${reboot ? btn('gold', t('u.restart'), 'power-ask', { arg: 'restart', icon: 'restart', af: true }) : btn('pri', t('u.open'), 'upd-open', { af: true })}</div>`;
    } else if (d && d.kind === 'languages') {
      inner = `<h2 id="dlg-h">${t('lang.choose')}</h2><div class="sx-menu">${['pt', 'en', 'fr'].map((c) => btn(c === S.lang ? 'pri' : '', t('lang.' + c), 'lang', { arg: c })).join('')}</div>
        <div class="sx-dialog__acts">${btn('ghost', t('power.cancel'), 'dialog-cancel')}</div>`;
    } else if (d && d.kind === 'wifi') {
      inner = `<div class="sx-dialog__trust">${ic('shield')}${t('auth.trusted')}</div><h2 id="dlg-h">${t('wifi.dialog.h', { ssid: d.ssid })}</h2>
        <p>${t(d.ask ? 'wifi.dialog.note' : 'wifi.dialog.known')}</p>${d.error ? alert('bad', d.error) : ''}
        <form data-form="wifi">${d.ask ? field('wifipass', t('wifi.pass'), { type: 'password' }) : ''}<div class="sx-dialog__acts">${btn('ghost', t('power.cancel'), 'dialog-cancel')}${btn('pri', t(S.busy ? 'wifi.joining' : 'wifi.connect'), null, { submit: true })}</div></form>`;
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
  // Os códigos de recuperação ficam no ecrã, sozinhos, até serem guardados:
  // a sessão já existe, mas o Workspace espera por isso.
  const view = () => (!S.session ? 'wait' : S.recovery ? 'login' : S.session.state === 'ACTIVE' ? 'shell' : S.session.state === 'LOCKED' ? 'lock' : 'login');
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
    const focusKey = v + '|' + ((got('activation.status') || {}).phase || '') + (S.credential ? 'c' : '') + (S.recovery ? 'r' : '') + (S.enrolKey ? 'k' : '') + '|' + (S.dialog ? S.dialog.kind + (S.dialog.cls || '') : '') + '|' + (v === 'shell' ? S.panel + ':' + S.page : S.step + ':' + S.fb.step);
    const moved = focusKey !== lastFocusKey;
    lastFocusKey = focusKey;
    let target = null;
    if (moved) {
      // Dentro do que acabou de aparecer: o diálogo, o painel, ou o cartão do
      // ecrã — nunca os botões do canto, que estão antes dele no documento.
      const scope = root.querySelector('.sx-dialog') || root.querySelector('.sx-panel') || root.querySelector('.sx-card') || root.querySelector('.sx-lock__who') || root;
      target = (S.panel === 'system' && !S.dialog && root.querySelector('#page-h')) || scope.querySelector('[data-af]:not([disabled])') || scope.querySelector('input:not([disabled])') || scope.querySelector('button:not([disabled])') || scope.querySelector('[tabindex="-1"]');
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
    const base = ['activation.status', 'system.health', 'network.status', 'power.status', 'time.status', 'locale.status', 'accessibility.status', 'keyboard.status', 'audio.status', 'display.status', 'update.status'];
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
      // Chegar ao segundo factor quer dizer que a palavra-passe nova, quando a
      // houve, já é a da conta: daqui em diante é ela que se apresenta.
      if ((code === 'SECOND_FACTOR_REQUIRED' || code === 'SECOND_FACTOR_ENROLLMENT_REQUIRED') && S.form.new1) { S.form.pass = S.form.new1; S.form.new1 = ''; S.form.new2 = ''; }
      if (code === 'SECOND_FACTOR_ENROLLMENT_REQUIRED' && S.step !== 4) { S.step = 4; S.form.code = ''; S.enrolKey = null; render(); await beginEnrollment(); }
      else if (code === 'SECOND_FACTOR_REQUIRED' && S.step !== 2) { S.step = 2; S.form.code = ''; }
      else if (code === 'PASSWORD_CHANGE_REQUIRED' && S.step === 1) { S.step = 3; S.form.new1 = ''; S.form.new2 = ''; }
      else {
        S.error = ['why.' + code, { n: reply.reason.retry_after_secs }];
        S.form.code = '';
        if (S.step === 1) S.form.pass = '';
        if (code === 'PASSWORD_NOT_ACCEPTED') { S.step = 3; S.form.new1 = ''; S.form.new2 = ''; }
      }
    } else {
      if (reply.reply === 'enrolled') { S.session = reply.state; S.recovery = reply.recovery_codes; }
      S.form = Object.assign(S.form, { user: '', pass: '', code: '', unlock: '', auth: '', new1: '', new2: '' }); S.step = 1; S.panel = null; S.enrolKey = null;
    }
    render();
    if (view() === 'shell') { refresh(); invoke('open_workspace').catch(() => {}); }
  }

  async function beginEnrollment() {
    const reply = await ask({ request: 'begin_second_factor_enrollment', username: S.form.user.trim(), password: S.form.pass });
    if (reply && reply.reply === 'second_factor_secret') S.enrolKey = reply.secret_base32;
    else S.error = [reply ? 'why.' + reply.reason.code : 'why.SESSION_SERVICE'];
    render();
  }

  // Cada passo valida o que é seu; o motor (o System Service) decide outra
  // vez quando recebe. A língua, o fuso e a rede vão juntos, uma vez.
  async function fbNext() {
    if (S.busy) return;
    const f = S.form;
    S.error = null;
    const step = S.fb.step;
    if (step === 'region') {
      const k = got('keyboard.status');
      if (!k || k.layout !== S.fb.kb) {
        S.busy = true; render();
        const r = await call('keyboard.configure', { layout: S.fb.kb, variant: null });
        S.busy = false;
        if (r.refused) { S.error = ['r.' + r.refused.code]; return render(); }
      }
    } else if (step === 'network') {
      const ip = (v) => /^(\d{1,3})\.(\d{1,3})\.(\d{1,3})\.(\d{1,3})$/.test(v) && v.split('.').every((x) => Number(x) <= 255);
      let network = null;
      if (S.fb.net === 'STATIC') {
        const dns = f.ndns.split(/[\s,]+/).filter(Boolean);
        const prefix = Number(f.nprefix);
        if (!S.fb.iface || !ip(f.naddr.trim()) || !ip(f.ngw.trim()) || !(prefix >= 1 && prefix <= 32) || !dns.every(ip) || dns.length > 4) { S.error = ['fb.net.invalid']; return render(); }
        network = { interface: S.fb.iface, addressing: { mode: 'STATIC', address: f.naddr.trim(), prefix, gateway: f.ngw.trim(), dns } };
      }
      S.busy = true; render();
      const r = await call('activation.configure', { language: S.lang, timezone: S.fb.tz, network });
      S.busy = false;
      if (r.refused) { S.error = [r.refused.code === 'VALUE_OUT_OF_RANGE' ? 'fb.net.invalid' : 'r.' + r.refused.code]; return render(); }
      S.langChosen = false;
      await read(['network.status', 'time.status', 'locale.status']);
    } else if (step === 'instance') {
      if (!f.iname.trim() || !/^[a-z0-9-]+(\.[a-z0-9-]+)+$/.test(f.domain.trim().toLowerCase()) || !Object.keys(S.dist).some((k) => S.dist[k])) { S.error = ['act.missing']; return render(); }
    } else if (step === 'people') {
      if ([f.pname, f.pemail, f.aname, f.aemail].some((v) => !v.trim()) || ![f.pemail, f.aemail].every((v) => /^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(v.trim()))) { S.error = ['act.missing']; return render(); }
    }
    S.fb.step = FB[Math.min(FB.length - 1, FB.indexOf(step) + 1)];
    render();
  }

  async function activate() {
    if (S.busy) return;
    const f = S.form;
    const distributions = Object.keys(S.dist).filter((k) => S.dist[k]);
    const missing = [f.iname, f.domain, f.pname, f.pemail, f.aname, f.aemail].some((v) => !v.trim());
    if (missing || !distributions.length) { S.error = ['act.missing']; return render(); }
    S.busy = true; S.error = null; render();
    const r = await call('activation.start', { domain: f.domain.trim().toLowerCase(), instance_name: f.iname.trim(), distributions, person_name: f.pname.trim(), person_email: f.pemail.trim(), admin_name: f.aname.trim(), admin_email: f.aemail.trim() });
    S.busy = false;
    if (r.refused) S.error = [r.refused.code === 'VALUE_OUT_OF_RANGE' ? 'act.invalid' : 'r.' + r.refused.code];
    render();
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
    dist: (arg) => { S.dist[arg] = !S.dist[arg]; },
    'fb-lang': (arg) => { S.lang = arg; S.langChosen = true; document.documentElement.lang = LOCALE(); },
    'fb-net': (arg) => { S.fb.net = arg; },
    'fb-back': () => { S.error = null; S.fb.step = FB[Math.max(0, FB.indexOf(S.fb.step) - 1)]; },
    'credential-show': async () => {
      S.error = null;
      const r = await call('activation.credential');
      if (r.ok && r.ok.kind === 'initial_credential') { S.credential = r.ok.value; S.form.user = r.ok.value.username; await read(['activation.status']); }
      else S.error = ['act.cred.gone'];
    },
    'credential-done': () => { S.credential = null; S.step = 1; },
    'recovery-done': () => { S.recovery = null; refresh(); invoke('open_workspace').catch(() => {}); },
    'login-back': () => { S.step = 1; S.error = null; S.form.code = ''; S.form.pass = ''; S.form.new1 = ''; S.form.new2 = ''; S.enrolKey = null; },
    languages: () => { S.dialog = { kind: 'languages' }; },
    lang: async (arg) => { S.lang = arg; S.langChosen = true; S.dialog = null; document.documentElement.lang = LOCALE(); render(); const r = await call('locale.configure', { language: arg }); if (!r.refused) S.langChosen = false; },
    contrast: async () => { const a = got('accessibility.status') || { text_scale_percent: 100, high_contrast: false, reduce_motion: false }; const r = await call('accessibility.configure', Object.assign({}, a, { high_contrast: !a.high_contrast })); if (r.refused) document.documentElement.classList.toggle('sx-contrast'); else applyPreferences(); },
    power: () => { S.dialog = { kind: 'power' }; },
    'power-ask': (arg) => { S.dialog = { kind: 'power-confirm', action: arg }; },
    'power-do': async (arg) => { S.dialog = null; await change('power.' + arg); },
    'dialog-cancel': () => { S.dialog = null; S.form.auth = ''; S.form.wifipass = ''; },
    signout: async () => { S.dialog = null; S.panel = null; S.error = null; await ask({ request: 'sign_out' }); },
    lock: async () => { S.dialog = null; S.panel = null; S.error = null; await ask({ request: 'lock' }); },
    tray: () => { S.panel = S.panel === 'quick' ? null : 'quick'; },
    'open-system': () => { S.panel = S.panel === 'system' ? null : 'system'; if (S.panel) read(READS[S.page]).then(render); },
    'close-panel': () => { S.panel = null; },
    nav: (arg) => {
      S.panel = 'system'; S.page = arg;
      // Procurar redes demora segundos: uma vez ao abrir a página, e depois
      // só quando se pede — nunca a cada actualização.
      const extra = arg === 'network' && !S.sys['wifi.scan'] ? ['wifi.scan'] : [];
      read(READS[arg].concat(extra)).then(render);
    },
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
    'wifi-radio': (arg) => change('wifi.set_radio', { enabled: arg === '1' }),
    'wifi-scan': () => look('wifi.scan'),
    'wifi-disconnect': () => change('wifi.disconnect'),
    'wifi-pick': (arg) => {
      const v = got('wifi.scan'); const w = v && v.networks[Number(arg)];
      if (!w) return;
      S.form.wifipass = '';
      S.dialog = { kind: 'wifi', ssid: w.ssid, ask: w.security === 'PASSPHRASE' && !w.known, error: null };
    },
    'wifi-forget': (arg) => { const v = got('wifi.scan'); const w = v && v.networks[Number(arg)]; return w ? change('wifi.forget', { ssid: w.ssid }) : null; },
    'net-iface': (arg) => { S.net.iface = arg; },
    'net-mode': (arg) => { S.net.mode = arg === 'STATIC' ? 'STATIC' : 'DHCP'; },
    'net-apply': () => change('network.configure', { interface: S.net.iface, addressing: addressing() }),
    'bt-power': (arg) => change('bluetooth.set_power', { powered: arg === '1' }),
    'bt-scan': () => look('bluetooth.scan'),
    'bt-pair': (arg) => change('bluetooth.pair', { address: arg }),
    'bt-connect': (arg) => change('bluetooth.connect', { address: arg }),
    'bt-disconnect': (arg) => change('bluetooth.disconnect', { address: arg }),
    'bt-remove': (arg) => change('bluetooth.remove', { address: arg }),
    'volume-in': (arg) => change('audio.set_volume', { target: 'INPUT', percent: Number(arg) }),
    'mute-out': (arg) => change('audio.set_mute', { target: 'OUTPUT', muted: arg !== '1' }),
    'mute-in': (arg) => change('audio.set_mute', { target: 'INPUT', muted: arg !== '1' }),
    'snd-output': (arg) => change('audio.set_output', { id: Number(arg) }),
    'privacy-mic': (arg) => change('privacy.set_microphone', { allowed: arg === '1' }),
    'privacy-cam': (arg) => change('privacy.set_camera', { allowed: arg === '1' }),
    ssh: (arg) => change('remote_access.configure', { ssh_enabled: arg === '1' }),
    'upd-check': async () => { const r = await change('update.check'); await read(['update.status']); if (!r.refused) toast(t('up.' + (got('update.status') || {}).phase)); },
    'upd-download': async () => { await change('update.download'); await read(['update.status']); },
    'upd-install': async () => { await change('update.install'); await read(['update.status']); },
    'upd-cancel': async () => { await change('update.cancel'); await read(['update.status']); },
    'upd-rollback': async () => { await change('update.rollback'); await read(['update.status', 'update.history']); },
    'upd-import': async (arg) => { await change('update.import', { bundle: arg }); await read(['update.status']); },
    'upd-autocheck': (arg) => change('update.configure', { change: { auto_check: arg === '1' } }),
    'upd-autodownload': (arg) => change('update.configure', { change: { auto_download: arg === '1' } }),
    'upd-details': () => { S.updDetails = !S.updDetails; },
    'upd-later': () => { S.updSeen = updKey(got('update.status')); S.dialog = null; },
    'upd-pop': () => { S.dialog = { kind: 'update' }; },
    'upd-open': () => { const u = got('update.status'); S.updSeen = updKey(u); S.dialog = null; S.panel = 'system'; S.page = 'updates'; return read(READS.updates); },
    'base-check': async () => { await change('base.check'); await read(['base.status']); },
    'base-apply': async () => { await change('base.apply'); await read(['base.status']); },
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
    const fb = e.target.closest('select[data-fb]');
    if (fb) { S.fb[fb.dataset.fb] = fb.value; render(); return; }
    const el = e.target.closest('select[data-act], input[type="range"][data-act]');
    if (el) return act(el.dataset.act, el.value);
    const box = e.target.closest('input[type="checkbox"][data-act]');
    if (box) act(box.dataset.act, box.dataset.arg);
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
    else if (form.dataset.form === 'setup') { if (S.fb.step === 'summary') activate(); else fbNext(); }
    else if (form.dataset.form === 'unlock') unlock();
    else if (form.dataset.form === 'auth') authorise();
    else if (form.dataset.form === 'wifi') joinWifi();
  });
  document.addEventListener('keydown', (e) => {
    if (e.key !== 'Escape') return;
    if (S.dialog) { S.dialog = null; S.form.auth = ''; S.form.wifipass = ''; render(); }
    else if (S.panel) { S.panel = null; render(); }
  });
  document.addEventListener('focusin', (e) => { if (e.target && e.target.id) lastFocusId = e.target.id; });
  document.addEventListener('contextmenu', (e) => e.preventDefault());

  refresh();
  setInterval(refresh, 5000);
  setInterval(render, 15000);
})();
