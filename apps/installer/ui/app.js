/* Ocinye OS Installer · renderer (D011).
   Desenha a vista que o controlador Rust entrega e envia acções; não decide
   nada. Marcação só de classes (CSP style-src 'self'): nenhum atributo style.
   Todo o valor que vem do servidor ou do operador passa por esc(). Estrutura e
   classes da referência viva do Design (reference/d011/fixture.js). */
(function () {
  'use strict';
  const T = window.OI_T;
  const ipc = window.__TAURI_INTERNALS__;
  let V = null;            // a vista corrente
  let LANG = 'pt';
  let busy = false;
  let lastJson = '';
  let lastScreen = null;
  let ui = { forget: false, credential: null, details: false, auth: null };

  // ── textos
  const LI = () => ({ pt: 0, en: 1, fr: 2 }[LANG] || 0);
  const esc = (s) => String(s == null ? '' : s).replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;');
  const raw = (h) => ({ __html: h });
  const MISSING = [];
  const t = (k, v) => {
    const e = T[k];
    if (!e) { MISSING.push(k); return esc(k); }
    let s = esc(e[LI()]);
    if (v) for (const n in v) {
      const x = v[n];
      s = s.split('{' + n + '}').join(x && x.__html !== undefined ? x.__html : esc(x));
    }
    return s;
  };
  const mono = (s) => raw('<span class="oi-mono">' + esc(s) + '</span>');

  // ── peças (as da referência)
  const ic = (id, c) => `<svg class="oc-icon${c ? ' ' + c : ''}" aria-hidden="true" focusable="false"><use href="icons.svg#${id}"></use></svg>`;
  const BICON = { will: 'download', ext: 'warning', apply: 'shield', pass: 'check', warn: 'warning', block: 'close', na: 'minus', info: 'help', run: 'refresh', future: 'clock', dev: 'warning' };
  const badge = (st, txt) => `<span class="oi-badge" data-st="${st}">${ic(BICON[st] || 'status')}${txt}</span>`;
  const STB = { pass: 'st.pass', warn: 'st.warn', block: 'st.block', na: 'st.na', will: 'st.willInstall', ext: 'st.external', apply: 'st.willApply' };
  const btn = (kind, label, o = {}) => `<button class="oi-btn oi-btn--${kind}${o.sm ? ' oi-btn--sm' : ''}" type="${o.submit ? 'submit' : 'button'}"${o.act ? ` data-act="${o.act}"` : ''}${o.arg ? ` data-arg="${esc(o.arg)}"` : ''}${o.disabled || busy ? ' disabled' : ''}${o.af ? ' data-autofocus' : ''}${o.kbd ? ` aria-keyshortcuts="${o.kbd}"` : ''}>${o.icon ? ic(o.icon) : ''}${label}</button>`;
  const alert = (tone, title, body, actions, icon) => `<div class="oi-alert" data-tone="${tone}" role="${tone === 'error' ? 'alert' : 'status'}"${tone === 'error' ? ' tabindex="-1" data-autofocus' : ''}>${ic(icon || { error: 'close', warn: 'warning', info: 'help', ok: 'check' }[tone])}<div><p class="oi-alert__t">${title}</p>${body ? `<p class="oi-alert__b">${body}</p>` : ''}${actions ? `<div class="oi-alert__a">${actions}</div>` : ''}</div></div>`;
  const kv = (rows) => `<dl class="oi-kv">${rows.map(([a, b]) => `<dt>${a}</dt><dd>${b}</dd>`).join('')}</dl>`;
  // `gid`: the card is a named group of fields (its title labels them), for
  // forms where two cards repeat the same field names.
  const card = (title, inner, right, cls, gid) => `<section class="oi-card${cls ? ' ' + cls : ''}"${gid ? ` role="group" aria-labelledby="${gid}"` : ''}>${title ? `<div class="oi-card__head"><h2 class="oi-card__title"${gid ? ` id="${gid}"` : ''}>${title}</h2>${right || ''}</div>` : ''}${inner}</section>`;
  const field = (fid, label, value, o = {}) => {
    const id = 'oi-f-' + fid;
    return `<div class="oi-field"><label for="${id}">${label}</label><input class="oi-input${o.mono ? ' oi-input--mono' : ''}" id="${id}" name="${fid}" type="${o.type || 'text'}" value="${esc(value)}"${o.err ? ` aria-invalid="true" aria-describedby="${id}-e"` : o.hint ? ` aria-describedby="${id}-h"` : ''}${o.af ? ' data-autofocus' : ''} autocomplete="${o.ac || 'off'}" spellcheck="false"${o.ro ? ' readonly' : ''}>${o.err ? `<span class="oi-err" id="${id}-e" role="alert">${ic('warning')}${o.err}</span>` : ''}${o.hint ? `<span class="oi-hint" id="${id}-h">${o.hint}</span>` : ''}</div>`;
  };
  const check = (st, title, detail, icon) => `<li class="oi-check" data-st="${st}">${ic(icon || BICON[st])}<div><div class="oi-check__t">${title}</div>${detail ? `<div class="oi-check__d">${detail}</div>` : ''}</div>${badge(st === 'run' ? 'run' : st, t(st === 'run' ? 'st.running' : STB[st] || 'st.todo'))}</li>`;
  const todo = (title) => `<li class="oi-check" data-st="todo">${ic('clock')}<div><div class="oi-check__t">${title}</div></div>${badge('na', t('st.todo'))}</li>`;
  const grp = (k) => `<li class="oi-checks__group" aria-hidden="true">${t(k)}</li>`;
  const gb = (bytes) => bytes == null ? '—' : (bytes >= 1e12 ? (bytes / 1e12).toFixed(1) + ' TB' : Math.round(bytes / 1e9) + ' GB');

  // ── trilho e página
  const STEPS = [['prepare', ['release', 'server', 'trust', 'preflight', 'hardware']], ['configure', ['instance', 'dists', 'endpoints', 'tls', 'admin']], ['install', ['review', 'install', 'verify', 'done']]];
  const ORDER = STEPS.flatMap((g) => g[1]);
  function rail(active, o = {}) {
    const ai = ORDER.indexOf(active);
    let n = 0;
    const groups = STEPS.map(([g, ids]) => `<li class="oi-steps__group" aria-hidden="true">${t('rail.' + g)}</li>` + ids.map((id) => {
      n++; const i = ORDER.indexOf(id);
      let s = i < ai ? 'done' : i === ai ? 'active' : 'todo';
      if (o.blocked === id) s = 'blocked';
      const num = s === 'done' ? ic('check') : s === 'blocked' ? ic('close') : n;
      const cur = s === 'active' || s === 'blocked' ? ' aria-current="step"' : '';
      const sr = s === 'done' ? `<span class="oc-sr">${t('st.done')}</span>` : '';
      return `<li class="oi-step" data-s="${s}"${cur}><span class="oi-step__n">${num}</span><span>${t('step.' + id)}${sr}</span>${s === 'blocked' ? `<span class="oi-step__st">${t('rail.blocked')}</span>` : ''}</li>`;
    }).join('')).join('');
    const host = V.draft && V.draft.host;
    const lang = ['pt', 'en', 'fr'].map((l) => `<button class="oi-btn oi-btn--ghost oi-btn--sm" type="button" data-act="lang" data-arg="${l}"${l === LANG ? ' aria-pressed="true"' : ''}>${l.toUpperCase()}</button>`).join('');
    return `<nav class="oi-rail" aria-label="${t('app.name')}"><div class="oi-brand"><img src="ocinye-logo.png" alt="" width="30" height="30"><div><div class="oi-brand__name">Ocinye OS</div><div class="oi-brand__sub">${t('app.sub')}</div></div></div><ol class="oi-steps">${groups}</ol><div class="oi-rail__foot"><span>${t('rail.version')} <b>${esc(V.app.version)}</b></span><span>${t('rail.platform')} <b>${esc(V.app.platform)}</b></span>${ai >= 2 && host ? `<span>${t('rail.target')} <b>${esc(host)}</b></span>` : ''}<span role="group" aria-label="${t('x.lang')}">${lang}</span></div></nav>`;
  }
  function page(s) {
    const k = s.kicker || (s.step ? t('kicker.step', { n: ORDER.indexOf(s.step) + 1 }) + ' · ' + t('rail.' + STEPS.find((g) => g[1].includes(s.step))[0]) : t('kicker.start'));
    const dev = V.release && V.release.build === 'proof' ? `<div class="oi-devbar" role="note">${ic('warning')}${t('release.dev')}</div>` : '';
    return `<div class="oi-app"><div class="oi-win" lang="${{ pt: 'pt-PT', en: 'en', fr: 'fr' }[LANG]}"><div class="oi-titlebar" data-tauri-drag-region><span>${t('app.name')}</span></div><div class="oi-body">${rail(s.step || 'none', s.rail || {})}<form class="oi-main" novalidate aria-labelledby="oi-h1" data-primary="${s.primary || ''}">${dev}<header class="oi-head"><div class="oi-kicker">${k}</div><h1 class="oi-h1" id="oi-h1" tabindex="-1">${s.title}</h1>${s.lead ? `<p class="oi-lead">${s.lead}</p>` : ''}</header><div class="oi-content">${s.body}</div><footer class="oi-foot">${s.foot}</footer></form></div>${s.modal || ''}</div></div>`;
  }
  const foot = (o = {}) => `${o.cancel === false ? '' : btn('ghost', o.cancelLabel || t('a.cancel'), { kbd: 'Escape', act: o.cancelAct || 'back' })}${o.note ? `<span class="oi-foot__note">${o.note}</span>` : ''}<span class="oi-foot__end">${o.extra || ''}${o.back === false ? '' : btn('secondary', t('a.back'), { icon: 'arrow-l', act: 'back' })}${o.primary === null ? '' : btn('primary', o.primary || t('a.continue'), { submit: true, disabled: o.disabled, af: o.af, kbd: 'Enter' })}</span>`;
  const fieldErr = (f) => {
    const c = V.field_errors && V.field_errors[f];
    if (!c) return '';
    return { INPUT_INVALID_HOST: t('server.hostInvalid', { v: V.draft.host }), INPUT_INVALID_PORT: t('x.field.port'), INPUT_INVALID_USER: t('x.field.user'), KEY_REQUIRED: t('x.field.key'), PASSWORD_REQUIRED: t('x.field.pw'), INPUT_INVALID_TEXT: t('x.field.text'), NO_DISTRIBUTION: t('dist.noneErr'), ADMIN_NOT_DISTINCT: t('adm.distinctErr'), SUDO_REFUSED: t('x.sudoRefused'), ENDPOINT_DISTRIBUTION: t('x.field.endpoint') }[c] || t('x.field.invalid');
  };
  const A = () => V.alert || null;

  // ── ecrãs
  const S = {};
  S.welcome = () => page({ title: t('welcome.title'), lead: t('welcome.lead'), primary: 'start', body: `<div class="oi-welcome">${card(t('welcome.need'), `<ul class="oi-changes"><li>${ic('download')}<span>${t('welcome.need1')}</span></li><li>${ic('compute')}<span>${t('x.welcomeNeed2')}</span></li><li>${ic('ob-globe')}<span>${t('welcome.need3')}</span></li></ul>`)}${alert('info', t('welcome.safe'), '', '', 'shield')}${card(t('welcome.facts'), kv([[t('welcome.version'), `<span class="oi-mono">${esc(V.app.version)}</span>`], [t('welcome.platform'), `<span class="oi-mono">${esc(V.app.platform)}</span>`], [t('welcome.source'), t('welcome.sourceV')], [t('welcome.scope'), t('welcome.scopeV')]]))}</div>`, foot: foot({ back: false, primary: t('a.start'), af: true, cancelLabel: t('a.close'), cancelAct: 'none' }) });

  function relKv(rejected) {
    const r = V.release;
    if (!r) return '';
    const integ = rejected ? badge('block', t('st.invalid')) : badge('pass', t('st.valid')) + ' ' + t('release.integrityV', { n: r.files });
    return kv([[t('release.package'), `<span class="oi-mono">${esc(r.package)}</span>`], [t('release.id'), `<span class="oi-mono">${esc(r.id)}</span>`], [t('release.commit'), `<span class="oi-mono">${esc(r.commit)}</span>`], [t('release.build'), r.build === 'proof' ? badge('dev', t('st.dev')) : badge('info', t('st.release'))], [t('release.arch'), `<span class="oi-mono">${esc(r.arch)}</span>`], [t('release.target'), '<span class="oi-mono">linux</span>'], [t('release.migrations'), `<span class="oi-mono">${esc(r.migrations.count)} · 0001–${esc(r.migrations.latest)}</span>`], [t('release.artifacts'), `<span class="oi-mono">${r.artifacts.map(esc).join(' · ')}</span>`], [t('release.integrity'), integ], [t('release.signing'), `<span class="oi-small">${t('release.signingV')}</span>`]]);
  }
  S.release = () => {
    const a = A();
    let top = '';
    if (a) {
      const map = { RELEASE_CHECKSUM_FAILED: [t('release.checksumT'), t('release.checksumB', { f: mono(a.params.file) })], RELEASE_UNSUPPORTED_ARCH: [t('release.archT'), t('release.archB', { a: mono(a.params.arch) })], INVALID_MANIFEST: [t('release.manifestT'), t('release.manifestB', { k: mono(a.params.field) })] };
      const [tt, bb] = map[a.code] || [t('release.manifestT'), esc(a.params.detail || a.code)];
      top = alert('error', tt, bb + ' ' + t('release.stop'), btn('secondary', t('a.openBundle'), { sm: true, icon: 'download', act: 'choose_release' }));
    }
    const dev = V.release && V.release.build === 'proof' ? alert('warn', t('release.devAlertT'), t('release.devAlertB')) : '';
    const body = top + dev + (V.release && !a ? card(t('release.package'), relKv(false), btn('secondary', t('a.otherBundle'), { sm: true, act: 'choose_release' })) : card(t('release.package'), `<p class="oi-p">${t('release.lead')}</p>`, btn('secondary', t('a.openBundle'), { sm: true, icon: 'download', act: 'choose_release', af: !a })));
    return page({ step: 'release', rail: a ? { blocked: 'release' } : {}, title: t('release.title'), lead: t('release.lead'), primary: 'continue_release', body, foot: foot({ disabled: !V.release || !!a, af: !!V.release && !a }) });
  };

  function serverForm(o = {}) {
    const d = V.draft;
    // The operator's choice stays on this side until the test sends it: a
    // poll of the controller's view must not undo it.
    const auth = ui.auth || d.auth;
    const seg = ['agent', 'key', 'password'].map((x) => `<label class="oi-seg__opt"${x === auth ? ' data-on' : ''}><input type="radio" name="auth" value="${x}"${x === auth ? ' checked' : ''}>${ic(x === 'agent' ? 'key' : x === 'key' ? 'files' : 'lock')}${t('server.' + x)}</label>`).join('');
    const keyName = d.key_path ? d.key_path.split('/').pop() : '';
    const authBody = auth === 'agent' ? `<span class="oi-hint">${t('server.agentV')}</span>` : auth === 'key' ? `<div class="oi-file"${fieldErr('key') ? ' aria-invalid="true"' : ''}>${ic('key')}<span class="oi-file__name"${keyName ? '' : ' data-empty'}>${keyName ? esc(keyName) : t('tls.empty')}</span><span></span>${btn('secondary', t('a.choose'), { sm: true, act: 'choose_key' })}</div><span class="oi-hint">${fieldErr('key') || t('server.keyHint')}</span>` : field('pw', t('server.password'), '', { type: 'password', hint: t('server.pwHint'), err: fieldErr('pw') });
    const p = V.probe;
    const priv = V.elevation === 'Root' ? 'root' : V.elevation === 'SudoNoPassword' ? 'sudo' : V.elevation === 'SudoPassword' ? t('server.privSudo') : '—';
    const res = p ? card(t('server.tested'), kv([[t('server.hostname'), `<span class="oi-mono">${esc(p.hostname)}</span>`], [t('server.os'), `<span class="oi-mono">${esc(p.pretty)} · ${esc(p.machine)}</span>`], [t('server.privilege'), priv]]), badge('pass', t('st.readOnly'))) : '';
    return `<div class="oi-form"><div class="oi-fields2">${field('host', t('server.host'), d.host, { mono: true, hint: fieldErr('host') ? '' : t('server.hostHint'), err: fieldErr('host'), af: o.af })}${field('port', t('server.port'), d.port, { mono: true, err: fieldErr('port') })}</div>${field('user', t('server.user'), d.user, { mono: true, hint: t('server.userHint'), err: fieldErr('user') })}<fieldset class="oi-fieldset"><legend class="oi-legend">${t('server.auth')}</legend><div><span class="oi-seg" role="radiogroup">${seg}</span></div>${authBody}</fieldset><div class="oi-row">${btn('secondary', t('a.test'), { icon: 'refresh', act: 'test' })}</div>${res}</div>`;
  }
  S.server = () => {
    const a = A();
    let top = '';
    if (a) {
      const d = V.draft;
      if (a.code === 'SSH_UNREACHABLE') top = alert('error', t('server.unreachT'), t('server.unreachB', { h: d.host, p: d.port }), btn('secondary', t('a.retry'), { sm: true, icon: 'refresh', act: 'test' }));
      else if (a.code === 'SSH_AUTH_REFUSED') top = alert('error', t('server.authT'), t('server.authB', { u: d.user, h: d.host }), btn('secondary', t('a.retry'), { sm: true, icon: 'refresh', act: 'test' }));
      else if (a.code === 'RELEASE_UNSUPPORTED_TARGET') top = alert('error', t('release.targetT'), t('release.targetB', { a: mono(a.params.release), h: d.host, b: mono(a.params.server) }) + ' ' + t('release.stop'), btn('secondary', t('a.openBundle'), { sm: true, icon: 'download', act: 'choose_release' }) + btn('ghost', t('a.otherServer'), { sm: true, act: 'other_server' }));
      else top = alert('error', t('x.alert.transportT'), esc(a.params.detail || a.code));
    }
    const blocked = a && a.code === 'RELEASE_UNSUPPORTED_TARGET';
    return page({ step: 'server', rail: blocked ? { blocked: 'server' } : {}, title: t('server.title'), lead: t('server.lead'), primary: 'continue_server', body: top + serverForm({ af: !a }), foot: foot({ disabled: !V.probe || blocked }) });
  };

  S['host-trust'] = () => {
    const [alg, fp] = V.presented_key || ['', ''];
    const d = V.draft;
    return page({ step: 'trust', title: t('trust.title'), lead: t('trust.lead'), primary: 'trust', body: card('', kv([[t('trust.server'), `<span class="oi-mono">${esc(d.host)}:${esc(d.port)}</span>`], [t('trust.alg'), `<span class="oi-mono">${esc(alg)}</span>`]])) + `<div class="oi-field"><span class="oi-legend">${t('trust.fp')}</span><div class="oi-fp">${esc(fp)}</div></div><p class="oi-p">${t('trust.note')}</p>`, foot: foot({ primary: t('a.trust'), af: true }) });
  };
  S['host-mismatch'] = () => {
    const [known, presented] = V.mismatch || ['', ''];
    const modal = ui.forget ? `<div class="oi-scrim"><div class="oi-dialog" role="dialog" aria-modal="true" aria-labelledby="oi-dlg-t"><h2 class="oi-dialog__t" id="oi-dlg-t">${t('x.forgetT')}</h2><p class="oi-p">${t('x.forgetB')}</p><div class="oi-grid2"><div class="oi-field"><span class="oi-legend">${t('trust.known')}</span><div class="oi-fp">${esc(known)}</div></div><div class="oi-field"><span class="oi-legend">${t('trust.presented')}</span><div class="oi-fp" data-st="block">${esc(presented)}</div></div></div><div class="oi-dialog__a">${btn('ghost', t('a.cancel'), { kbd: 'Escape', act: 'forget_cancel', af: true })}${btn('danger', t('x.forgetConfirm'), { act: 'forget_server' })}</div></div></div>` : '';
    return page({ step: 'trust', rail: { blocked: 'trust' }, kicker: t('kicker.stopped'), title: t('trust.title'), body: alert('error', t('trust.mismatchT'), t('trust.mismatchB', { h: V.draft.host, d: '—' })) + `<div class="oi-grid2"><div class="oi-field"><span class="oi-legend">${t('trust.known')}</span><div class="oi-fp">${esc(known)}</div></div><div class="oi-field"><span class="oi-legend">${t('trust.presented')}</span><div class="oi-fp" data-st="block">${esc(presented)}</div></div></div><p class="oi-p">${t('trust.mismatchNext')}</p><div class="oi-row">${btn('ghost', t('x.forget'), { sm: true, act: 'forget_open' })}</div>`, foot: foot({ back: false, primary: null, cancelLabel: t('a.close'), cancelAct: 'other_server', extra: btn('secondary', t('a.otherServer'), { act: 'other_server' }) }), modal });
  };

  // ── verificação prévia: as 19 verificações em 15 linhas, como a referência
  const ITEM = (id) => (V.preflight && V.preflight.items.find((i) => i.id === id)) || null;
  const stOf = (it) => !it ? 'todo' : it.status === 'BLOCKED' ? 'block' : it.status === 'WARNING' ? (it.action === 'WILL_INSTALL' ? 'will' : it.action === 'WILL_APPLY' ? 'apply' : it.action === 'EXTERNAL_ACTION' ? 'ext' : 'warn') : it.status === 'NOT_APPLICABLE' ? 'na' : 'pass';
  const worst = (...its) => { const order = ['block', 'ext', 'warn', 'will', 'apply', 'na', 'pass', 'todo']; return its.map(stOf).sort((a, b) => order.indexOf(a) - order.indexOf(b))[0]; };
  const fmtMb = (mb) => (mb >= 1024 ? (mb / 1024).toFixed(1).replace('.', LANG === 'en' ? '.' : ',') + ' GB' : mb + ' MB');
  function preRows() {
    const o = (id) => (ITEM(id) || {}).observation || {};
    const rows = {};
    const os = o('PF_OS'), di = o('PF_DISTRO');
    const distroIt = ITEM('PF_DISTRO');
    rows.os = [worst(ITEM('PF_OS'), distroIt), t('pre.os'), distroIt && distroIt.status === 'BLOCKED' ? (di.graphical ? t('x.distroGui') : t('x.distroBlocked', { v: di.pretty || di.id })) : t('x.osD', { v: di.pretty || os.kernel_name, k: os.kernel_release })];
    const ar = o('PF_ARCH');
    rows.arch = [stOf(ITEM('PF_ARCH')), t('pre.arch'), stOf(ITEM('PF_ARCH')) === 'block' ? t('x.archBlocked', { a: ar.server, b: ar.release }) : t('pre.archD', { a: ar.server, b: ar.release })];
    rows.priv = [stOf(ITEM('PF_PRIV')), t('pre.priv'), t('pre.privD', { u: V.draft.user })];
    const ck = o('PF_TIME');
    rows.time = [stOf(ITEM('PF_TIME')), t('pre.time'), stOf(ITEM('PF_TIME')) === 'block' ? t('x.timeBlocked', { s: ck.skew_seconds }) : t('pre.timeD', { s: Math.abs(ck.skew_seconds || 0) })];
    const cpu = o('PF_CPU');
    rows.cpu = [stOf(ITEM('PF_CPU')), t('pre.cpu'), stOf(ITEM('PF_CPU')) === 'block' ? t('x.cpuBlocked', { n: cpu.count }) : t('pre.cpuD', { n: cpu.count })];
    const ram = o('PF_RAM');
    const ramSt = stOf(ITEM('PF_RAM'));
    rows.ram = [ramSt, t('pre.ram'), (ramSt === 'block' ? t('x.ramBlock', { v: fmtMb(ram.total_mb) }) : ramSt === 'warn' ? t('pre.ramWarn', { v: fmtMb(ram.total_mb) }) : t('pre.ramD', { v: fmtMb(ram.total_mb) })) + (ram.swap_mb ? ' · swap ' + esc(fmtMb(ram.swap_mb)) : '')];
    const disk = o('PF_DISK');
    rows.disk = [stOf(ITEM('PF_DISK')), t('pre.disk'), stOf(ITEM('PF_DISK')) === 'block' ? t('pre.diskBlock', { v: disk.free_gb + ' GB' }) : t('pre.diskD', { v: disk.free_gb + ' GB' })];
    rows.hw = [stOf(ITEM('PF_HWREAD')), t('pre.hwVisible'), stOf(ITEM('PF_HWREAD')) === 'pass' ? t('pre.hwVisibleD') : t('x.hwPartial')];
    const dk = o('PF_DOCKER'), pk = o('PF_PKG');
    let dd = t('pre.dockerD', { v: (dk.observed && dk.observed.server_version) || '' });
    if (dk.state === 'MISSING_INSTALLABLE') dd = t('x.preDockerInstall');
    if (dk.state === 'CONFLICTING_RUNTIME') dd = t('pre.dockerConflict');
    if (dk.state === 'INSTALLED_UNSUPPORTED_VERSION') dd = t('x.dockerOld', { v: (dk.observed && dk.observed.server_version) || '?' });
    if (dk.state === 'INSTALLATION_NOT_SUPPORTED') dd = t('x.dockerUnsupported');
    if (pk.missing && pk.missing.length) dd += ' ' + t('x.pkgMissing', { p: pk.missing.join(', ') });
    else if (pk.installable && pk.installable.length) dd += ' ' + t('x.pkgInstall', { p: pk.installable.join(', ') });
    rows.docker = [worst(ITEM('PF_DOCKER'), ITEM('PF_PKG')), t('pre.docker'), dd];
    const po = o('PF_PORTS'), px = o('PF_PROXY');
    let pd = po.busy && po.busy.length ? t('pre.portsBlock', { p: po.busy.map((b) => b.port + (b.process ? ' ' + b.process : '')).join(', ') }) : t('pre.portsD');
    if (px.units && px.units.length) pd += ' ' + t('x.proxyD', { u: px.units.join(', ') });
    rows.ports = [worst(ITEM('PF_PORTS'), ITEM('PF_PROXY')), t('pre.ports'), pd];
    const fw = o('PF_FW');
    const fwd = { FIREWALL_NOT_PRESENT: t('pre.fwNone'), REQUIRED_RULES_ALREADY_PRESENT: t('x.fwPresent'), REQUIRED_RULES_CAN_BE_APPLIED: t('x.fwApply', { r: ['80', '443'].filter((p) => !((fw.observed || {}).ufw_allows || []).includes(+p)).map((p) => p + '/tcp').join(', ') }), EXTERNAL_FIREWALL_ACTION_REQUIRED: t('pre.fwCustom'), FIREWALL_CONFLICT: t('x.fwConflict', { p: ((fw.observed || {}).ufw_denies || []).join(', ') }) }[fw.state] || '';
    rows.fw = [stOf(ITEM('PF_FW')), t('pre.fw'), fwd];
    rows.reg = [stOf(ITEM('PF_REGISTRY')), t('pre.registry'), stOf(ITEM('PF_REGISTRY')) === 'block' ? t('x.registryBlocked') : t('pre.registryD')];
    rows.sysd = [stOf(ITEM('PF_SYSTEMD')), t('pre.systemd'), stOf(ITEM('PF_SYSTEMD')) === 'pass' ? t('pre.systemdD') : t('x.systemdNone')];
    const ex = o('PF_EXIST');
    rows.exist = [worst(ITEM('PF_EXIST'), ITEM('PF_SRVDIR')), t('pre.existing'), stOf(ITEM('PF_EXIST')) === 'block' ? t('pre.existsB', { r: ex.release || '?' }) : stOf(ITEM('PF_SRVDIR')) === 'block' ? t('x.srvdirBlocked') : t('pre.existingD')];
    rows.journal = [stOf(ITEM('PF_JOURNAL')), t('pre.journal'), stOf(ITEM('PF_JOURNAL')) === 'pass' ? t('pre.journalD') : t('x.journalFound')];
    return rows;
  }
  function preChecks() {
    const order = [['pre.g.system', ['os', 'arch', 'priv', 'time']], ['pre.g.resources', ['cpu', 'ram', 'disk', 'hw']], ['pre.g.runtime', ['docker', 'ports', 'fw', 'reg', 'sysd']], ['pre.g.existing', ['exist', 'journal']]];
    if (!V.preflight) {
      const titles = { os: 'pre.os', arch: 'pre.arch', priv: 'pre.priv', time: 'pre.time', cpu: 'pre.cpu', ram: 'pre.ram', disk: 'pre.disk', hw: 'pre.hwVisible', docker: 'pre.docker', ports: 'pre.ports', fw: 'pre.fw', reg: 'pre.registry', sysd: 'pre.systemd', exist: 'pre.existing', journal: 'pre.journal' };
      return `<ul class="oi-checks" aria-live="polite">${order.map(([g, ks]) => grp(g) + ks.map((k) => todo(t(titles[k]))).join('')).join('')}</ul>`;
    }
    const rows = preRows();
    const c = { pass: 0, warn: 0, block: 0 };
    // As acções planeadas (SERÁ INSTALADO, SERÁ APLICADO, ACÇÃO EXTERNA) não
    // são avisos nem passagens: dizem-se na própria linha (como no Design).
    Object.values(rows).forEach((r) => { if (r[0] in c) c[r[0]]++; });
    const list = order.map(([g, ks]) => grp(g) + ks.map((k) => check(rows[k][0], rows[k][1], rows[k][2])).join('')).join('');
    return `<div class="oi-summary" role="status">${badge(c.block ? 'block' : c.warn ? 'warn' : 'pass', t('pre.sum', { p: c.pass, w: c.warn, b: c.block }))}</div><ul class="oi-checks" aria-live="polite">${list}</ul>`;
  }
  S.preflight = () => {
    const a = A();
    const rows = V.preflight ? preRows() : null;
    const blocked = rows && Object.values(rows).some((r) => r[0] === 'block');
    let top = '';
    if (a && a.code === 'PRIVILEGE_MISSING') top = alert('error', t('priv.missingT'), t('priv.missingB', { u: V.draft.user, h: V.draft.host }), btn('ghost', t('a.otherServer'), { sm: true, act: 'other_server' }));
    else if (a) top = alert('error', t('x.alert.bootstrapT'), t('x.alert.bootstrapB', { c: a.params.detail || a.code }), btn('secondary', t('a.recheck'), { sm: true, icon: 'refresh', act: 'preflight' }));
    else if (rows && rows.exist[0] === 'block') top = alert('error', t('pre.existsT'), t('pre.existsB', { r: ((ITEM('PF_EXIST') || {}).observation || {}).release || '?' }), btn('secondary', t('a.otherServer'), { sm: true, act: 'other_server' }));
    else if (blocked) top = alert('error', t('pre.blockedT'), t('pre.blockedB'), btn('secondary', t('a.recheck'), { sm: true, icon: 'refresh', act: 'preflight' }));
    else if (rows && rows.docker[0] === 'will') top = alert('info', t('pre.runtimeT'), t('pre.runtimeB'), '', 'download');
    else if (rows && rows.fw[0] === 'ext') top = alert('warn', t('pre.fwExtT'), t('pre.fwExtB', { ip: mono(V.draft.host) }), btn('secondary', t('a.recheck'), { sm: true, icon: 'refresh', act: 'preflight' })) + card(t('pre.fwTh'), '<ul class="oi-changes"><li>' + ic('shield') + '<span class="oi-mono">' + t('pre.fw80') + '</span></li><li>' + ic('shield') + '<span class="oi-mono">' + t('pre.fw443') + '</span></li></ul>');
    else if (rows && Object.values(rows).some((r) => r[0] === 'warn')) top = alert('warn', t('pre.warnT'), t('pre.warnB'));
    const modal = V.sudo_dialog ? `<div class="oi-scrim"><div class="oi-dialog" role="dialog" aria-modal="true" aria-labelledby="oi-dlg-t"><h2 class="oi-dialog__t" id="oi-dlg-t">${t('priv.title')}</h2><p class="oi-p">${t('priv.body', { u: V.draft.user })}</p>${field('sudo', t('priv.field', { u: V.draft.user }), '', { type: 'password', af: true, err: fieldErr('sudo') })}<div class="oi-dialog__a">${btn('ghost', t('a.cancel'), { kbd: 'Escape', act: 'cancel_sudo' })}${btn('primary', t('a.sudoOk'), { kbd: 'Enter', act: 'sudo' })}</div></div></div>` : '';
    return page({ step: 'preflight', rail: blocked || (a && a.code === 'PRIVILEGE_MISSING') ? { blocked: 'preflight' } : {}, title: t('pre.title'), lead: t('pre.lead'), primary: 'continue_preflight', body: top + preChecks(), foot: foot({ disabled: !V.preflight || blocked || !!a, af: !!V.preflight && !blocked, note: badge('info', t('st.readOnly')) }), modal });
  };

  S.incomplete = () => {
    const j = V.journal || {};
    const done = (j.completed || []).join(', ');
    const a = A();
    return page({ step: 'preflight', kicker: t('kicker.recovery'), title: t('inc.title'), lead: t('inc.lead'), primary: 'resume', body: (a && a.code === 'PLAN_NOT_AVAILABLE' ? alert('warn', t('x.alert.planT'), t('x.alert.planB')) : '') + card('', kv([[t('inc.id'), `<span class="oi-mono">${esc(j.installation_id)} · ${esc(j.plan_id)}</span>`], [t('inc.release'), `<span class="oi-mono">${esc(j.release)}</span>`], [t('inc.state'), badge('warn', esc(j.state) + (j.failed_code ? ' · ' + esc(j.failed_code) : ''))], [t('inc.phases'), esc(done || '—') + (j.current ? ' · ' + esc(j.current) : '')], [t('inc.when'), `<span class="oi-mono">${esc(j.updated_at)}</span>`]])) + alert('info', t('inc.resumeT'), t('inc.resumeB', { p: mono(j.plan_id) })) + `<p class="oi-p">${t('inc.removeNote')}</p>`, foot: foot({ back: false, primary: t('a.resume'), af: true, extra: j.instance_created ? '' : btn('danger', t('a.removeIncomplete'), { icon: 'trash', act: 'remove_incomplete' }) }) });
  };

  // ── hardware
  S.hardware = () => {
    const h = V.hardware;
    if (!h) return page({ step: 'hardware', title: t('hw.title'), lead: t('hw.lead'), body: `<ul class="oi-checks">${check('run', t('hw.title'), '', 'refresh')}</ul>`, foot: foot({ disabled: true }) });
    const c = h.cpu;
    const cells = `<div class="oi-hw"><div class="oi-hwc"><span class="oi-hwc__l">${ic('compute')}${t('hw.cpu')}</span><span class="oi-hwc__v">${esc(c.model || '—')}</span><span class="oi-hwc__s">${t('hw.cores', { c: c.cores || '—', t: c.threads, a: c.arch || '?' })}</span></div><div class="oi-hwc"><span class="oi-hwc__l">${ic('gauge')}${t('hw.mem')}</span><span class="oi-hwc__v">${gb(h.memory.total_bytes)}</span><span class="oi-hwc__s">${t('hw.memS', { f: gb(h.memory.available_bytes) })}</span></div><div class="oi-hwc"><span class="oi-hwc__l">${ic('storage')}${t('hw.storage')}</span><span class="oi-hwc__v">${t('hw.storageV', { v: gb(h.storage.free_bytes) })}</span><span class="oi-hwc__s">${t('hw.storageS', { fs: h.storage.filesystem || '—' })}</span></div><div class="oi-hwc"><span class="oi-hwc__l">${ic('ob-globe')}${t('hw.net')}</span><span class="oi-hwc__v">${h.network.interfaces_up}${h.network.max_link_mbps ? ' × ' + (h.network.max_link_mbps >= 1000 ? h.network.max_link_mbps / 1000 + ' Gb/s' : h.network.max_link_mbps + ' Mb/s') : ''}</span><span class="oi-hwc__s">${t('hw.netS', { n: V.draft.host })}</span></div></div>`;
    const g = h.gpu;
    let acc;
    if (g.status === 'NO_GPU') acc = alert('info', t('hw.noGpuT'), t('hw.noGpuB') + ' ' + (g.basic_display ? t('hw.noGpuDisplay', { m: g.basic_display }) : t('hw.cpuOnlyB')), '', 'compute');
    else if (g.status === 'GPU_DISCOVERY_ERROR') acc = alert('warn', t('hw.errT'), t('hw.errB', { c: mono(g.code) }));
    else {
      const rt = (x) => !x.runtime ? badge('na', t('st.unknown')) : x.runtime[1] === 'detected' ? badge('pass', t('st.detected')) + ` <span class="oi-small">${x.runtime[0] === 'rocm' ? t('hw.rt.rocm') : t('hw.rt.nvidia')}</span>` : badge('warn', t('st.unavailable')) + ` <span class="oi-small">${t('hw.rt.missing')}</span>`;
      const vendor = (v) => ({ nvidia: 'NVIDIA', amd: 'AMD', intel: 'Intel' }[v.vendor] || ('PCI ' + v.pci_id));
      const rows = g.gpus.map((x) => `<tr><td><b>GPU ${x.index}</b> · ${esc(x.model || x.pci_address)}</td><td>${esc(vendor(x.vendor))}</td><td class="oi-mono">${x.vram_bytes ? gb(x.vram_bytes) : '—'}</td><td class="oi-mono">${esc(x.driver ? x.driver.module + ' ' + (x.driver.version || '') : '—')}</td><td>${rt(x)}</td><td class="oi-mono">${esc(x.compute_capability || '—')}</td></tr>`).join('');
      acc = (g.status === 'GPU_RUNTIME_UNAVAILABLE' ? alert('warn', t('hw.rtUnavailT'), t('hw.rtUnavailB', { d: '' })) : '') + card(`${t('hw.gpus')} · ${g.gpus.length === 1 ? t('hw.gpuOne') : t('hw.gpuCount', { n: g.gpus.length })}`, `<table class="oi-table"><thead><tr><th scope="col">${t('hw.th.device')}</th><th scope="col">${t('hw.th.vendor')}</th><th scope="col">${t('hw.th.vram')}</th><th scope="col">${t('hw.th.driver')}</th><th scope="col">${t('hw.th.runtime')}</th><th scope="col">${t('hw.th.cap')}</th></tr></thead><tbody>${rows}</tbody></table>`);
    }
    const cc = (h.security.confidential || []).length ? t('hw.ccV') : t('hw.ccNone');
    const tpm = h.security.tpm === 'DETECTED' ? t('hw.tpmV') : t('hw.ccNone');
    const ready = { GPU_DETECTED: 'hw.ready.gpu', NO_GPU: 'hw.ready.cpu', GPU_RUNTIME_UNAVAILABLE: 'hw.ready.rt', GPU_DISCOVERY_ERROR: 'hw.ready.unknown' }[g.status];
    const sec = card('', kv([[t('hw.cc'), cc], [t('hw.tpm'), tpm], [t('hw.readiness'), t(ready)]]));
    const future = `<section class="oi-future" aria-labelledby="oi-cn"><div class="oi-future__k">${ic('clock')}${t('cn.k')}</div><div><h2 class="oi-future__t" id="oi-cn">${t('cn.t')}</h2><p class="oi-future__b">${t('cn.b')}</p></div><div class="oi-future__s"><span>${t('cn.state')}</span>${badge('future', t('st.notConfigured'))}<span>${t('cn.provider')}: <b>${t('cn.off')}</b></span><span>${t('cn.later')}</span></div></section>`;
    return page({ step: 'hardware', title: t('hw.title'), lead: t('hw.lead'), primary: 'continue_hardware', body: cells + acc + sec + future, foot: foot({ af: true, note: badge('info', t('st.readOnly')) }) });
  };

  S.instance = () => {
    const name = V.draft.instance_name;
    const slug = slugOf(name);
    return page({ step: 'instance', title: t('inst.title'), lead: t('inst.lead'), primary: 'continue_instance', body: `<div class="oi-form">${field('iname', t('inst.name'), name, { hint: t('inst.nameHint'), af: true, err: fieldErr('iname') })}${field('slug', t('inst.slug'), slug, { mono: true, ro: true, hint: t('inst.slugHint') })}${card('', kv([[t('inst.locale'), t('inst.localeV')]]))}</div>`, foot: foot() });
  };
  function slugOf(n) {
    const map = { 'á': 'a', 'à': 'a', 'â': 'a', 'ã': 'a', 'ä': 'a', 'é': 'e', 'è': 'e', 'ê': 'e', 'ë': 'e', 'í': 'i', 'ì': 'i', 'î': 'i', 'ï': 'i', 'ó': 'o', 'ò': 'o', 'ô': 'o', 'õ': 'o', 'ö': 'o', 'ú': 'u', 'ù': 'u', 'û': 'u', 'ü': 'u', 'ç': 'c', 'ñ': 'n' };
    let s = '';
    for (const ch of (n || '').trim().toLowerCase()) { const b = map[ch] || ch; if (/[a-z0-9]/.test(b)) s += b; else if (s && !s.endsWith('-')) s += '-'; }
    return s.replace(/-+$/, '').slice(0, 64);
  }

  const DISTS = ['research', 'business', 'personal', 'education'];
  S.distributions = () => {
    const sel = V.draft.distributions;
    const err = fieldErr('dist');
    const cards = DISTS.map((d) => { const on = sel.includes(d); const first = sel[0] === d; return `<label class="oi-dist"${on ? ' data-on' : ''}><input type="checkbox" name="dist" value="${d}"${on ? ' checked' : ''}${d === 'research' ? ' data-autofocus' : ''}${err ? ' aria-invalid="true" aria-describedby="dist-e"' : ''}><span class="oi-dist__ic">${ic('dist-' + d)}</span><span class="oi-dist__n">${t('dist.' + d)}</span><span class="oi-dist__d">${t('dist.' + d + 'D')}</span>${first ? badge('info', t('dist.first')) : ''}</label>`; }).join('');
    return page({ step: 'dists', title: t('dist.title'), lead: t('dist.lead'), primary: 'continue_distributions', body: `<fieldset class="oi-fieldset"><legend class="oc-sr">${t('dist.title')}</legend><div class="oi-dists">${cards}</div></fieldset><div class="oi-row" role="status">${err ? `<span class="oi-err" id="dist-e" role="alert">${ic('warning')}${err}</span>` : `<span class="oi-small">${t('dist.count', { n: sel.length })}</span>`}</div><p class="oi-p">${t('dist.admin')}</p>`, foot: foot({ disabled: sel.length === 0 }) });
  };

  S.endpoints = () => {
    const rows = V.draft.endpoints;
    const enabled = V.draft.distributions;
    // The address the operator reached the server at, when it is one: a
    // server can have several interfaces, and its first is not necessarily
    // the one the names must point to.
    const host = V.draft.host || '';
    const isV4 = /^\d{1,3}(\.\d{1,3}){3}$/.test(host);
    const isV6 = host.includes(':');
    const ip4 = isV4 ? host : (!isV6 && V.facts && V.facts.ipv4[0]) || (isV6 ? null : host);
    const ip6 = isV6 ? host : (!isV4 && V.facts && V.facts.ipv6[0]) || null;
    const sel = (i, d) => `<select class="oi-input" name="ep-dist-${i}" aria-label="${t('ep.th.binding')} ${i + 1}">${enabled.map((x) => `<option value="${x}"${x === d ? ' selected' : ''}>${t('ep.bound', { d: t('dist.' + x) })}</option>`).join('')}</select>`;
    const dnsSt = (s) => s === 'DNS_OK' ? badge('pass', t('st.dnsOk')) : s === 'DNS_WRONG_TARGET' ? badge('block', t('st.dnsWrong')) : s === 'DNS_UNRESOLVED' ? badge('warn', t('st.dnsUnresolved')) : badge('na', t('st.dnsRequired'));
    const body = rows.map((r, i) => `<tr><td><input class="oi-input oi-input--mono" name="ep-host-${i}" aria-label="${t('ep.th.host')} ${i + 1}" value="${esc(r.host)}"${i === 0 ? ' data-autofocus' : ''}${V.field_errors['ep' + i] || r.dns === 'DNS_WRONG_TARGET' ? ' aria-invalid="true"' : ''} spellcheck="false" autocomplete="off"></td><td>${i === 0 ? `<span class="oi-row"><span>${t('ep.generic')}</span>${badge('info', t('ep.canonical'))}</span>` : sel(i, r.distribution)}</td><td>${dnsSt(r.dns)}</td><td>${i === 0 ? '' : btn('ghost', t('a.remove'), { sm: true, act: 'ep_remove', arg: String(i) })}</td></tr>`).join('');
    const recs = rows.filter((r) => r.host).map((r) => `<tr><td class="oi-mono">${esc(r.host)}</td><td class="oi-mono">A</td><td class="oi-mono">${esc(ip4)}</td><td class="oi-mono">${esc((r.seen || []).join(', ') || '—')}</td><td>${dnsSt(r.dns)}</td></tr>` + (ip6 ? `<tr><td class="oi-mono">${esc(r.host)}</td><td class="oi-mono">AAAA</td><td class="oi-mono">${esc(ip6)}</td><td class="oi-mono">—</td><td>${badge('na', t('st.na'))}</td></tr>` : '')).join('');
    const wrong = rows.find((r) => r.dns === 'DNS_WRONG_TARGET');
    const top = wrong ? alert('error', t('ep.dnsWrongT'), t('ep.dnsWrongB', { h: mono(wrong.host), o: mono((wrong.seen || []).join(', ')), ip: mono(ip4) }), btn('secondary', t('a.recheck'), { sm: true, icon: 'refresh', act: 'check_dns' })) : rows.some((r) => r.dns !== 'DNS_OK') && rows.some((r) => r.dns !== 'DNS_REQUIRED') ? alert('warn', t('ep.dnsPendT'), t('ep.dnsPendB', { ip: mono(ip4) }), btn('secondary', t('a.recheck'), { sm: true, icon: 'refresh', act: 'check_dns' })) : '';
    const err = Object.keys(V.field_errors).some((k) => k.startsWith('ep')) ? `<span class="oi-err" role="alert">${ic('warning')}${t('x.field.endpoint')}</span>` : '';
    return page({ step: 'endpoints', title: t('ep.title'), lead: t('ep.lead', { s: mono(V.draft.host) }), primary: 'continue_endpoints', body: top + `<table class="oi-table"><thead><tr><th scope="col">${t('ep.th.host')}</th><th scope="col">${t('ep.th.binding')}</th><th scope="col">${t('ep.th.dns')}</th><th scope="col"><span class="oc-sr">${t('a.remove')}</span></th></tr></thead><tbody>${body}</tbody></table><div class="oi-row">${btn('secondary', t('a.addEndpoint'), { sm: true, icon: 'plus', act: 'ep_add', disabled: rows.length >= 9 })}<span class="oi-small">${t('ep.hint')}</span>${err}</div>` + card(t('ep.dnsT'), `<p class="oi-p">${t('ep.dnsB')}</p><br><table class="oi-table"><thead><tr><th scope="col">${t('ep.th.host')}</th><th scope="col">${t('ep.th.type')}</th><th scope="col">${t('ep.th.value')}</th><th scope="col">${t('ep.th.seen')}</th><th scope="col">${t('ep.th.dns')}</th></tr></thead><tbody>${recs}</tbody></table>`, btn('secondary', t('a.recheck'), { sm: true, icon: 'refresh', act: 'check_dns' })), foot: foot({ disabled: !!wrong }) });
  };

  S.tls = () => {
    const mode = V.draft.tls_mode;
    const v = V.tls_validation;
    const prov = mode !== 'self';
    const opt = (val, on) => `<label class="oi-choice"${on ? ' data-on' : ''}><input type="radio" name="tls" value="${val}"${on ? ' checked data-autofocus' : ''}><span><span class="oi-choice__t">${t('tls.' + val)}${val === 'self' ? badge('warn', t('st.test')) : ''}</span><span class="oi-choice__d">${t('tls.' + val + 'D')}</span></span></label>`;
    const name = (p) => p ? esc(p.split('/').pop()) : '';
    const ok = (c) => v && (v.checks.find((x) => x[0] === c) || [0, false])[1];
    const file = (k, path, o = {}) => `<div class="oi-field"><span class="oi-legend">${t('tls.' + k)}${o.opt ? ` <span class="oi-req">(${t('tls.optional')})</span>` : ''}</span><div class="oi-file"${o.bad ? ' aria-invalid="true"' : ''}>${ic(k === 'key' ? 'key' : 'shield')}<span class="oi-file__name"${path ? '' : ' data-empty'}>${path ? name(path) : t('tls.empty')}</span>${o.st || '<span></span>'}${btn('secondary', t('a.choose'), { sm: true, act: 'choose_tls', arg: k })}</div></div>`;
    let cks = '';
    if (v) {
      const hosts = V.draft.endpoints.map((r) => r.host).join(' · ');
      cks = [check(ok('PARSE') ? 'pass' : 'block', t('tls.c.parse')), check(ok('KEY_MATCH') ? 'pass' : 'block', ok('KEY_MATCH') ? t('tls.c.match') : t('tls.c.matchFail')), check(ok('NOT_EXPIRED') ? (v.expiring_soon ? 'warn' : 'pass') : 'block', t('tls.c.exp', { d: (v.not_after || '').slice(0, 10) })), check(ok('COVERS_NAMES') ? 'pass' : 'block', ok('COVERS_NAMES') ? t('tls.c.names', { h: mono(hosts) }) : t('tls.c.namesFail', { h: mono(v.uncovered.join(', ')) })), check(ok('CHAIN') ? 'pass' : 'block', t('tls.c.chain'))].join('') + (v.checks.some((c) => c[0] === 'STRONG_SIGNATURE' && !c[1]) ? check('block', t('x.tlsWeak')) : '');
    }
    const bad = v && v.checks.some((x) => !x[1]);
    const provBody = `<div class="oi-form oi-form--wide">${file('cert', V.draft.tls_cert, { st: v ? badge(ok('PARSE') ? 'pass' : 'block', t(ok('PARSE') ? 'st.valid' : 'st.invalid')) : '' })}${file('key', V.draft.tls_key, { bad: v && !ok('KEY_MATCH'), st: v ? badge(ok('KEY_MATCH') ? 'pass' : 'block', t(ok('KEY_MATCH') ? 'st.valid' : 'st.invalid')) : '' })}${file('chain', V.draft.tls_chain, { opt: true })}<span class="oi-hint">${t('tls.keyNote')}</span></div>` + (v ? card(t('tls.checks'), `<ul class="oi-checks">${cks}</ul>`) : '');
    return page({ step: 'tls', title: t('tls.title'), lead: t('tls.lead'), primary: 'continue_tls', body: (bad || (A() && A().code === 'TLS_INVALID') ? alert('error', t('tls.invalidT'), t('tls.invalidB')) : '') + `<fieldset class="oi-fieldset"><legend class="oc-sr">${t('tls.title')}</legend><div class="oi-choices">${opt('provided', prov)}${opt('self', !prov)}</div></fieldset><p class="oi-small">${t('tls.acme')}</p>` + (prov ? provBody : alert('warn', t('tls.selfAlertT'), t('tls.selfAlertB'))), foot: foot({ disabled: prov && (!v || bad) }) });
  };

  S.admin = () => {
    const a = V.draft.admin;
    return page({ step: 'admin', title: t('adm.title'), lead: t('adm.lead'), primary: 'continue_admin', body: `<div class="oi-grid2">${card(t('adm.person'), `<p class="oi-small">${t('adm.personD')}</p><br><div class="oi-form">${field('pn', t('adm.name'), a[0], { af: true, ac: 'name', err: fieldErr('pn') })}${field('pe', t('adm.email'), a[1], { type: 'email', mono: true, ac: 'email', err: fieldErr('pe') })}</div>`, '', '', 'oi-adm-person')}${card(t('adm.priv'), `<p class="oi-small">${t('adm.privD')}</p><br><div class="oi-form">${field('an', t('adm.name'), a[2], { err: fieldErr('an') })}${field('ae', t('adm.email'), a[3], { type: 'email', mono: true, err: fieldErr('ae') })}</div>`, '', '', 'oi-adm-priv')}</div>` + card(t('adm.after'), `<p class="oi-p">${t('adm.afterV')}</p>`), foot: foot() });
  };

  S.review = () => {
    const p = V.plan; const c = p.configuration; const h = V.hardware || {};
    const ch = (txt, icn) => `<li>${ic(icn)}<span>${txt}</span></li>`;
    const sc = p.system_changes;
    const docker = sc.find((x) => x.op === 'install_docker_from_official_repo');
    const pkgs = sc.find((x) => x.op === 'ensure_packages');
    const fw = sc.filter((x) => x.op === 'firewall_allow').map((x) => 'allow ' + x.port + '/tcp');
    const tlsTxt = c.tls.mode === 'SELF_SIGNED_TEST' ? t('c.tlsSelf') : t('c.tlsProvided');
    const gpus = h.gpu && h.gpu.gpus ? h.gpu.gpus.map((g) => esc(g.model || g.pci_address)).join(', ') : t('x.noneGpu');
    const changes = (docker ? ch(t('x.chPrereq', { p: docker.packages.join(', '), f: docker.repo_key_fingerprint }), 'download') : '') + (pkgs ? ch(t('x.chPkg', { p: pkgs.packages.join(', ') }), 'download') : '') + ch(t('rev.ch1'), 'folder') + ch(t('rev.ch2'), 'download') + ch(t('rev.ch3', { n: V.release.migrations.count }), 'data') + ch(t('rev.ch4'), 'shield') + (fw.length ? ch(t('x.chFw', { r: fw.join(', ') }), 'shield') : '') + ch(t('rev.ch5'), 'power') + ch(t('rev.ch7'), 'refresh') + ch(t('rev.ch6'), 'lock') + ch(t('rev.provider'), 'compute');
    const eps = [[esc(c.endpoints.canonical), `${t('ep.canonical')} · ${t('ep.noneDist')}`]].concat(c.endpoints.bound.map((b) => [esc(b.host), t('ep.bound', { d: t('dist.' + b.distribution) })])).concat([['TLS', tlsTxt]]);
    const dists = c.distributions.map((d, i) => t('dist.' + d) + (i === 0 ? ' (' + t('dist.first').toLowerCase() + ')' : '')).join(' · ');
    return page({ step: 'review', title: t('rev.title'), lead: t('rev.lead'), primary: 'install', body: `<div class="oi-planid">${badge('info', t('rev.plan'))}<span>${esc(p.plan_id)}</span><span>·</span><span>${t('rev.hash')} sha256:${esc(p.plan_sha256.slice(0, 4))}…${esc(p.plan_sha256.slice(-4))}</span></div><div class="oi-plan">${card(t('rev.release'), kv([[t('release.id'), `<span class="oi-mono">${esc(p.release.id)}</span>`], [t('release.build'), p.release.build === 'proof' ? badge('dev', t('st.dev')) : badge('info', t('st.release'))], [t('release.arch'), `<span class="oi-mono">${esc(p.release.arch)}</span>`]]), btn('ghost', t('a.change'), { sm: true, act: 'change', arg: 'release' }))}${card(t('rev.server'), kv([[t('trust.server'), `<span class="oi-mono">${esc(p.target.user)}@${esc(V.draft.host)}</span>`], [t('trust.fp'), `<span class="oi-mono">${esc(p.target.host_key_sha256.slice(0, 26))}…</span>`], [t('server.privilege'), esc(V.elevation === 'Root' ? 'root' : 'sudo')]]), btn('ghost', t('a.change'), { sm: true, act: 'change', arg: 'server' }))}${card(t('rev.hardware'), kv([[t('hw.cpu'), esc((h.cpu && h.cpu.model) || '—') + ' · ' + esc(p.hardware.threads) + 't'], [t('hw.mem'), esc(Math.round(p.hardware.memory_mb / 1024)) + ' GB'], [t('hw.gpus'), gpus], [t('cn.provider'), t('cn.off')]]))}${card(t('rev.instance'), kv([[t('inst.name'), esc(c.instance_name)], [t('dist.title'), dists]]), btn('ghost', t('a.change'), { sm: true, act: 'change', arg: 'instance' }))}${card(t('rev.endpoints'), kv(eps), btn('ghost', t('a.change'), { sm: true, act: 'change', arg: 'endpoints' }))}${card(t('rev.admin'), kv([[t('adm.person'), esc(c.admin.person.name) + ' · ' + esc(c.admin.person.email)], [t('adm.priv'), esc(c.admin.privileged.name) + ' · ' + esc(c.admin.privileged.email)]]), btn('ghost', t('a.change'), { sm: true, act: 'change', arg: 'admin' }))}${card(t('rev.changes'), `<ul class="oi-changes">${changes}</ul>`, '', 'oi-card--wide')}</div>${alert('info', t('rev.sealing'), t('rev.duration'), '', 'key')}`, foot: foot({ primary: t('a.install'), af: true }) });
  };

  // ── instalação
  const PH = ['P01', 'P02', 'P03', 'P04', 'P05', 'P06', 'P07', 'P08', 'P09', 'P10', 'P11', 'P12', 'P13', 'P14', 'P15', 'P16'];
  function opText(p, live) {
    if (p === 'P02' && live.upload) return t('x.upload', { a: gb(live.upload.done), b: gb(live.upload.total) });
    const o = live.ops && live.ops[p];
    if (!o) return '';
    const v = { p: o.path || o.port || (o.packages || []).join(' '), d: o.done, t: o.total, s: o.service, h: o.host };
    return T['x.op.' + o.op] ? t('x.op.' + o.op, v) : esc(o.op);
  }
  function phases(live) {
    const st = live.phases || {};
    const skipped = (V.plan && V.plan.phases.filter((x) => x.skip).map((x) => x.phase)) || [];
    return `<ol class="oi-phases" aria-label="${t('step.install')}">` + PH.map((p) => {
      const s = st[p] || (skipped.includes(p) ? 'skip' : 'todo');
      const icn = { done: 'check', active: 'refresh', fail: 'close', warn: 'warning', todo: 'clock', skip: 'minus' }[s];
      const lbl = { done: 'st.done', active: 'st.running', fail: 'st.failed', warn: 'st.warn', todo: 'st.todo', skip: 'st.na' }[s];
      const bs = { done: 'pass', active: 'run', fail: 'block', warn: 'warn', todo: 'na', skip: 'na' }[s];
      const op = s === 'active' || s === 'fail' ? opText(p, live) : '';
      return (p === 'P06' ? `<li class="oi-pnr" aria-hidden="true">${t('ins.pnr')}</li>` : '') + `<li class="oi-phase" data-s="${s}"${s === 'active' ? ' aria-current="step"' : ''}>${ic(icn)}<span class="oi-phase__id">${p}</span><span>${t('ph.' + p)}${op ? `<span class="oi-phase__op">${op}</span>` : ''}</span>${badge(bs, t(lbl))}</li>`;
    }).join('') + '</ol>';
  }
  function events(live) {
    const rows = (live.events || []).slice(0, 60).map((e) => {
      const ev = e.event || {};
      const name = ev.event || '';
      const phase = ev.phase || '';
      const lv = name === 'StepFailed' ? 'error' : name === 'StepWarning' ? 'warn' : '';
      const detail = ev.code || (ev.operation && ev.operation.op) || ev.detail || '';
      return `<tr><td>${esc((e.at || '').slice(11, 19))}</td><td>${esc(phase)}</td><td${lv ? ` data-lv="${lv}"` : ''}>${esc(name)}</td><td${lv ? ` data-lv="${lv}"` : ''}>${esc(detail)}${ev.detail && ev.code ? ' · ' + esc(ev.detail) : ''}</td></tr>`;
    }).join('');
    return `<details class="oi-details"${ui.details ? ' open' : ''}><summary>${ic('list')}${t('a.details')}</summary><table class="oi-events"><thead><tr><th scope="col">${t('ev.time')}</th><th scope="col">${t('ev.phase')}</th><th scope="col">${t('ev.event')}</th><th scope="col">${t('ev.detail')}</th></tr></thead><tbody>${rows}</tbody></table></details>`;
  }
  function overall(live, fail) {
    const st = live.phases || {};
    const done = PH.filter((p) => ['done', 'warn', 'skip'].includes(st[p])).length;
    const cur = live.current || 'P01';
    const n = +cur.slice(1);
    const pct = Math.min(100, Math.round(done / 16 * 10) * 10);
    const el = live.elapsed_s || 0;
    const mm = String(Math.floor(el / 60)).padStart(2, '0') + ':' + String(el % 60).padStart(2, '0');
    return `<div class="oi-overall" role="status" aria-live="polite"><div class="oi-overall__row"><span class="oi-overall__t">${t('ins.overall', { n, p: T['ph.' + cur] ? T['ph.' + cur][LI()] : cur })}</span><span class="oi-overall__m">${t('ins.elapsed', { t: mm })}</span></div><div class="oi-bar"${fail ? ' data-st="fail"' : ''} role="progressbar" aria-valuemin="0" aria-valuemax="100" aria-valuenow="${pct}" aria-label="${t('step.install')}"><span class="oi-bar__fill" data-w="${pct}"></span></div></div>`;
  }
  S.installing = () => {
    const live = V.live || {};
    if (V.stopping) return page({ step: 'install', title: t('cancel.title'), lead: t('cancel.body'), body: overall(live) + phases(live), foot: foot({ back: false, primary: null, cancel: false }) });
    return page({ step: 'install', title: t('ins.title'), lead: t('ins.lead'), body: overall(live) + phases(live) + events(live), foot: foot({ back: false, primary: null, cancelLabel: t('a.stop'), cancelAct: 'stop', note: live.warnings ? badge('warn', t('ins.warnings', { n: live.warnings })) : '' }) });
  };
  S.interrupted = () => {
    const live = V.live || {};
    return page({ step: 'install', title: t('int.title'), lead: t('int.lead'), body: alert('warn', t('int.title'), '', btn('secondary', t('a.reconnect'), { sm: true, icon: 'refresh', af: true, act: 'reconnect' }), 'warning') + card('', kv([[t('inc.id'), `<span class="oi-mono">${esc(V.plan && V.plan.installation_id)} · ${esc(V.plan && V.plan.plan_id)}</span>`]])) + `<p class="oi-p">${t('int.unknown')}</p>` + phases(live), foot: foot({ back: false, primary: null, cancelLabel: t('a.close'), cancelAct: 'none' }) });
  };
  S.failed = () => {
    const live = V.live || {};
    const e = V.ending || {};
    const ph = e.phase || '—';
    const code = e.code || e.outcome || '';
    const spec = { MIGRATION_FAILED: ['fail.migT', 'fail.migR', 'fail.migN'], TRANSFER_CHECKSUM_MISMATCH: ['fail.xferT', 'fail.xferR', 'fail.xferN'], ADMIN_BOOTSTRAP_REFUSED: ['fail.admT', 'fail.admR', 'fail.admN'], REPO_KEY_MISMATCH: ['x.repoKeyT', 'x.repoKeyR', 'x.failNoRetry'] }[code];
    const svc = code.startsWith('SERVICE_UNHEALTHY');
    const title = spec ? t(spec[0]) : svc ? t('fail.svcT') : t('fail.title');
    const reason = spec ? t(spec[1]) : t('x.failR', { p: ph, c: code });
    const next = spec ? t(spec[2]) : e.retryable ? t('x.failRetry') : t('x.failNoRetry');
    const noInstance = !(V.journal && V.journal.instance_created) && !['P12', 'P13', 'P14', 'P15', 'P16'].includes(ph);
    return page({ step: 'install', rail: { blocked: 'install' }, kicker: t('kicker.stopped'), title: t('fail.title'), body: alert('error', title, `${reason} <b>${t('fail.notOp')}</b>`) + card('', kv([[t('fail.phase'), `<span class="oi-mono">${esc(ph)}</span>${T['ph.' + ph] ? ' · ' + t('ph.' + ph) : ''}`], [t('fail.code'), `<span class="oi-mono">${esc(code)}</span>`], [t('fail.next'), next]])) + `<p class="oi-p">${t('fail.journal', { id: (V.plan && V.plan.installation_id) || '' })}</p>` + phases(live) + events(live), foot: foot({ back: false, primary: e.retryable ? t('a.retry') : null, cancelLabel: t('a.close'), cancelAct: 'none', extra: (noInstance && !e.retryable ? btn('danger', t('a.removeIncomplete'), { icon: 'trash', act: 'remove_incomplete' }) : '') + btn('secondary', t('a.saveReport'), { icon: 'download', act: 'save_receipt', af: !e.retryable }) }) }).replace('data-primary=""', 'data-primary="resume"');
  };

  function verifyList() {
    const items = (V.verification && V.verification.items) || [];
    const live = (V.live && V.live.verification) || [];
    const all = items.concat(live);
    const get = (id) => { for (let i = all.length - 1; i >= 0; i--) if (all[i].id === id) return all[i]; return null; };
    const row = (id, k, d) => { const it = get(id); if (!it) return todo(t(k)); const s = it.status === 'PASS' ? 'pass' : it.status === 'PENDING' ? 'warn' : it.status === 'NOT_RUN' ? 'na' : 'block'; return check(s, t(k), d ? d(it) : esc(it.evidence)); };
    const c = V.plan ? V.plan.configuration : null;
    return `<ul class="oi-checks" aria-live="polite">${grp('ver.g.server')}${row('V01', 'ver.services', () => t('ver.servicesD'))}${row('V02', 'ver.db', () => t('ver.dbD'))}${row('V03', 'ver.mig', (it) => esc(it.evidence))}${row('V04', 'ver.core', (it) => esc(it.evidence))}${row('V05', 'ver.instance', () => c ? t('ver.instanceD', { n: c.instance_name }) : '')}${row('V06', 'ver.dists', () => c ? t('ver.distsD', { d: c.distributions.map((d) => t('dist.' + d)).join(' · ') }) : '')}${row('V07', 'ver.registry', (it) => esc(it.evidence))}${row('V08', 'ver.storage', () => t('ver.storageD'))}${row('V12b', 'ver.ep', (it) => esc(it.evidence))}${grp('ver.g.local')}${row('V10', 'ver.dns', (it) => esc(it.evidence))}${row('VFw', 'ver.fw', () => t('ver.fwD'))}${row('V11', 'ver.https', (it) => esc(it.evidence))}${row('V12', 'ver.ep', (it) => esc(it.evidence))}${row('V13', 'ver.login', () => t('x.verLoginD', { u: 'https://' + ((V.plan && V.plan.configuration.endpoints.canonical) || '') }))}</ul>`;
  }
  S.verifying = () => {
    const live = V.live || {};
    const failed = A() && A().code === 'VERIFICATION_FAILED';
    if (failed) return page({ step: 'verify', rail: { blocked: 'verify' }, kicker: t('kicker.stopped'), title: t('ver.title'), lead: t('ver.lead'), body: alert('error', t('ver.failT'), t('x.verifyFailedB'), btn('secondary', t('a.recheck'), { sm: true, icon: 'refresh', act: 'recheck' }) + (V.credential ? btn('secondary', t('a.credential'), { sm: true, icon: 'key', act: 'show_credential' }) : '')) + `<p class="oi-p">${t('ver.failN')}</p>` + verifyList(), foot: foot({ back: false, primary: null, cancelLabel: t('a.close'), cancelAct: 'none', extra: btn('secondary', t('a.saveReport'), { icon: 'download', act: 'save_receipt' }) }) });
    return page({ step: 'verify', title: t('ver.title'), lead: t('ver.lead'), body: overall(live) + verifyList(), foot: foot({ back: false, primary: null, cancel: false }) });
  };

  S.complete = () => {
    const lc = V.lifecycle || { state: 'INSTALLATION_INCOMPLETE' };
    const dns = lc.state === 'ACTIVATION_PENDING';
    const test = lc.state === 'INSTALLED_TEST_MODE';
    const op = lc.state === 'OPERATIONAL';
    const lcRow = (k, st, v) => `<span class="oi-row"><span class="oi-card__label">${t(k)}</span>${badge(st, t(v))}</span>`;
    const reasons = (lc.reasons || []);
    const life = `<div class="oi-summary" role="status" aria-label="${t('lc.install')} · ${t('lc.activation')} · ${t('lc.operational')}">${lcRow('lc.install', 'pass', 'lc.complete')}${lcRow('lc.activation', dns || test ? 'warn' : op ? 'pass' : 'warn', dns ? 'lc.pendingDns' : test ? 'lc.testMode' : op ? 'lc.complete' : 'st.pending')}${lcRow('lc.operational', op ? 'pass' : 'block', op ? 'lc.yes' : 'lc.no')}</div>`;
    const c = V.plan.configuration;
    const firstPending = (V.verification.items.find((i) => i.id === 'V10') || {}).evidence || '';
    const pendHost = (firstPending.split(':')[1] || '').split(',').join(', ');
    const head = dns ? `<div class="oi-done" data-st="warn"><span class="oi-done__ic">${ic('warning')}</span><div><p class="oi-alert__t">${t('done.pendT')}</p><p class="oi-alert__b">${reasons.includes('DNS') ? t('done.pendD', { h: mono(pendHost || c.endpoints.canonical), ip: mono(V.draft.host) }) : esc(reasons.join(', '))}</p><div class="oi-alert__a">${btn('secondary', t('a.recheck'), { sm: true, icon: 'refresh', act: 'recheck' })}</div></div></div><p class="oi-small">${t('done.recheckNote')}</p>`
      : test ? `<div class="oi-done" data-st="warn"><span class="oi-done__ic">${ic('warning')}</span><div><p class="oi-alert__t">${t('done.testT')}</p><p class="oi-alert__b">${t('done.testD')}</p></div></div>`
        : op ? `<div class="oi-done"><span class="oi-done__ic">${ic('check')}</span><div><p class="oi-alert__t">${t('done.ok')}</p></div></div>`
          : alert('warn', t('fail.title'), esc(lc.state), btn('secondary', t('a.recheck'), { sm: true, icon: 'refresh', act: 'recheck' }));
    const v = (id) => (V.verification.items.find((i) => i.id === id) || {}).status;
    // An endpoint passes when its name resolves to the server (V10) and the
    // Workspace answers it (V12): V12 alone connects by address.
    const epSt = () => v('V12') === 'PASS' && v('V10') === 'PASS' ? badge('pass', t('st.pass')) : badge('warn', t('st.pending'));
    const epList = [c.endpoints.canonical].concat(c.endpoints.bound.map((b) => b.host)).map((h) => `<span class="oi-mono">${esc(h)}</span> ${epSt()}`).join('<br>');
    const h = V.hardware || {};
    const hwLine = esc((h.cpu && h.cpu.model) || '—') + ' · ' + gb(h.memory && h.memory.total_bytes) + ' · ' + (h.gpu && h.gpu.gpus ? h.gpu.gpus.length + ' GPU' : t('x.noneGpu'));
    const chips = `<div class="oi-row">${c.distributions.map((d) => `<span class="oi-chip">${ic('dist-' + d)}${t('dist.' + d)}</span>`).join('')}</div>`;
    const lost = (V.events || []).some((e) => e.event && e.event.code === 'CREDENTIAL_NOT_RECOVERABLE');
    return page({ step: 'done', title: t(dns ? 'done.titleInstalled' : test ? 'done.titleTest' : op ? 'done.title' : 'done.titleInstalled'), lead: dns || test ? '' : t('done.lead'), primary: 'open_ocinye', body: life + head + (lost ? alert('warn', t('x.credLost'), '') : '') + `<div class="oi-grid2">${card('', kv([[t('done.instance'), esc(c.instance_name)], [t('done.server'), `<span class="oi-mono">${esc(V.draft.host)}</span>`], [t('done.release'), `<span class="oi-mono">${esc(V.plan.release.id)}</span>`], ['TLS', c.tls.mode === 'SELF_SIGNED_TEST' ? t('c.tlsSelf') : t('x.tlsProvided', { d: (c.tls.not_after || '').slice(0, 10) })], [t('done.hw'), hwLine], [t('cn.provider'), t('cn.off')]]))}${card('', kv([[t('done.dists'), chips], [t('done.eps'), epList]]))}</div>` + card(t('done.next'), `<ul class="oi-changes"><li>${ic('key')}<span>${t('x.doneNext1', { u: mono('https://' + V.plan.configuration.endpoints.canonical) })}</span></li><li>${ic('lock')}<span>${t('done.next2')}</span></li><li>${ic('members')}<span>${t('done.next3')}</span></li></ul>`, `<span class="oi-row">${V.credential && !V.credential_acknowledged ? btn('secondary', t('a.credential'), { sm: true, icon: 'key', act: 'show_credential' }) : ''}${btn('ghost', t('a.receipt'), { sm: true, icon: 'list', act: 'show_receipt' })}</span>`), foot: foot({ back: false, cancelLabel: t('a.close'), cancelAct: 'none', note: `<span class="oi-small">${t('done.opens')}</span>`, extra: btn('secondary', t('a.saveReport'), { icon: 'download', act: 'save_receipt' }), primary: test ? t('done.openTest') : t('a.open'), disabled: !(op || test), af: op || test }) });
  };
  S.credential = () => {
    const cr = V.credential || {};
    const value = ui.credential;
    return page({ step: 'done', title: t('cred.title'), lead: t('cred.lead'), primary: 'acknowledge_credential', body: `<div class="oi-secret">${kv([[t('cred.user'), `<span class="oi-mono">${esc(cr.user)}</span>`], [t('cred.valid'), `<span class="oi-mono">${esc(cr.expires_at)}</span>`]])}<div class="oi-field"><span class="oi-legend">${t('cred.secret')}</span><div class="oi-secret__v" id="oi-secret">${value ? esc(value) : '••••••••••••'}</div><span class="oi-hint">${t('cred.clip')}</span></div><div class="oi-row">${value ? btn('secondary', t('a.copy'), { sm: true, icon: 'copy', act: 'copy_credential' }) : btn('secondary', t('a.show'), { sm: true, icon: 'key', act: 'reveal_credential', af: true })}</div></div><p class="oi-p">${t('cred.after')}</p>`, foot: foot({ back: false, cancel: false, primary: t('a.saved'), af: !!value }) });
  };
  S.receipt = () => {
    const r = V.receipt;
    if (!r) return page({ step: 'done', title: t('rcp.title'), body: '', foot: foot({ cancel: false, primary: null }) });
    const hw = r.hardware;
    const hwLine = esc(hw.cpu.arch || '?') + ' · ' + (hw.cpu.threads) + 't · ' + gb(hw.memory.total_bytes) + ' · ' + gb(hw.storage.total_bytes) + ' · ' + (hw.gpu.gpus ? hw.gpu.gpus.length + ' GPU' : t('x.noneGpu'));
    const passed = r.verification.items.filter((i) => i.status === 'PASS').length;
    return page({ step: 'done', title: t('rcp.title'), lead: t('rcp.lead'), primary: 'save_receipt', body: card('', kv([[t('rcp.id'), `<span class="oi-mono">${esc(r.installation_id)} · ${esc(r.plan_id)}</span>`], [t('done.release'), `<span class="oi-mono">${esc(r.release)} · ${esc(r.commit)}</span>`], [t('done.server'), `<span class="oi-mono">${esc(r.target_host)}</span>`], [t('rcp.hostKey'), `<span class="oi-mono">${esc(r.host_key_sha256)}</span>`], [t('done.instance'), esc(r.instance_name) + ' · ' + esc(r.instance_slug)], [t('done.dists'), esc(r.distributions.join(', '))], [t('done.eps'), `<span class="oi-mono">${esc(r.endpoints.canonical)} (canonical)${r.endpoints.bound.map((b) => ' · ' + esc(b.host) + ' → ' + esc(b.distribution)).join('')}</span>`], ['TLS', r.tls_mode === 'SELF_SIGNED_TEST' ? t('c.tlsSelf') : t('c.tlsProvided')], [t('done.hw'), hwLine], [t('cn.provider'), t('cn.off')], [t('rcp.started'), `<span class="oi-mono">${esc(r.started_at)}</span>`], [t('rcp.ended'), `<span class="oi-mono">${esc(r.ended_at)}</span>`], [t('rcp.verification'), badge(passed === r.verification.items.length ? 'pass' : 'warn', passed + '/' + r.verification.items.length)], [t('rcp.artifacts'), `<span class="oi-mono">${r.artifacts.slice(0, 3).map((a) => esc(a.path) + ' ' + esc(a.sha256.slice(0, 4)) + '…' + esc(a.sha256.slice(-4))).join(' · ')} · …</span>`], [t('rcp.file'), `<span class="oi-mono">ocinye-install-${esc(r.installation_id)}.json</span>`], [t('rcp.never'), `<span class="oi-small">${t('rcp.neverV')}</span>`]])), foot: foot({ back: true, cancel: false, primary: t('a.saveReport') }) });
  };

  // ── montagem
  function render(force) {
    if (!V) return;
    LANG = V.lang || 'pt';
    busy = !!V.busy;
    const j = JSON.stringify(V) + JSON.stringify(ui);
    if (!force && j === lastJson) return;
    lastJson = j;
    const root = document.getElementById('root');
    const active = document.activeElement && document.activeElement.id;
    const keep = {};
    root.querySelectorAll('input[type=text],input[type=email],input[type=password],select').forEach((el) => { if (el.name) keep[el.name] = el.value; });
    const screen = V.screen;
    const fn = S[screen] || S.welcome;
    root.innerHTML = fn();
    document.documentElement.lang = { pt: 'pt-PT', en: 'en', fr: 'fr' }[LANG];
    // What the operator typed survives any re-render of the same screen —
    // a poll, or a change such as the authentication method. A new screen
    // starts from the view.
    if (screen === lastScreen) root.querySelectorAll('input,select').forEach((el) => { if (el.name && keep[el.name] !== undefined) el.value = keep[el.name]; });
    lastScreen = screen;
    const back = active && document.getElementById(active);
    const af = back || root.querySelector('.oi-dialog [data-autofocus]') || root.querySelector('[data-autofocus]') || root.querySelector('#oi-h1');
    if (af && af.focus) af.focus({ preventScroll: true });
  }
  async function call(cmd, args) {
    if (!ipc) return;
    try {
      const v = await ipc.invoke(cmd, args || {});
      if (v && typeof v === 'object') { V = v; render(true); }
      return v;
    } catch (e) { /* the controller reports through the view */ }
  }
  const val = (name) => { const el = document.querySelector(`[name="${name}"]`); return el ? el.value : ''; };
  function endpointRows() {
    const rows = [];
    for (let i = 0; document.querySelector(`[name="ep-host-${i}"]`); i++) rows.push({ host: val('ep-host-' + i), distribution: i === 0 ? null : (val('ep-dist-' + i) || null), dns: '', seen: [] });
    return rows;
  }
  const primary = {
    start: () => call('start'),
    continue_release: () => call('continue_release'),
    continue_server: () => call('continue_server'),
    trust: () => call('trust'),
    continue_preflight: () => call('continue_preflight'),
    resume: () => call('resume'),
    continue_hardware: () => call('continue_hardware'),
    continue_instance: () => call('continue_instance', { name: val('iname') }),
    continue_distributions: () => call('continue_distributions'),
    continue_endpoints: async () => { await call('set_endpoints', { rows: endpointRows() }); return call('continue_endpoints'); },
    continue_tls: () => call('continue_tls'),
    continue_admin: () => call('continue_admin', { fields: [val('pn'), val('pe'), val('an'), val('ae')] }),
    install: () => call('install'),
    open_ocinye: () => call('open_ocinye'),
    acknowledge_credential: () => { ui.credential = null; return call('acknowledge_credential'); },
    save_receipt: () => call('save_receipt'),
  };
  const actions = {
    back: () => call('back'),
    none: () => {},
    lang: (a) => call('set_lang', { lang: a }),
    choose_release: () => call('choose_release'),
    choose_key: () => call('choose_key'),
    test: () => call('test_connection', { host: val('host'), port: val('port'), user: val('user'), auth: (document.querySelector('input[name=auth]:checked') || {}).value || 'agent', password: val('pw') || null }),
    other_server: () => call('other_server'),
    forget_open: () => { ui.forget = true; render(true); },
    forget_cancel: () => { ui.forget = false; render(true); },
    forget_server: () => { ui.forget = false; return call('forget_server'); },
    sudo: () => call('sudo', { password: val('sudo') }),
    cancel_sudo: () => call('cancel_sudo'),
    preflight: () => call('preflight'),
    remove_incomplete: () => call('remove_incomplete'),
    ep_add: async () => { const rows = endpointRows(); rows.push({ host: '', distribution: V.draft.distributions[0], dns: '', seen: [] }); await call('set_endpoints', { rows }); },
    ep_remove: async (a) => { const rows = endpointRows(); rows.splice(+a, 1); await call('set_endpoints', { rows }); },
    check_dns: async () => { await call('set_endpoints', { rows: endpointRows() }); return call('check_dns'); },
    choose_tls: (a) => call('choose_tls_file', { kind: a }),
    change: (a) => call('change', { section: a }),
    stop: () => call('stop'),
    reconnect: () => call('reconnect'),
    recheck: () => call('recheck'),
    show_credential: () => call('show_credential'),
    show_receipt: () => call('show_receipt'),
    save_receipt: () => call('save_receipt'),
    reveal_credential: async () => { ui.credential = await ipc.invoke('reveal_credential', {}); render(true); },
    copy_credential: () => { if (ui.credential && navigator.clipboard) navigator.clipboard.writeText(ui.credential); },
  };
  document.addEventListener('click', (e) => {
    const b = e.target.closest('[data-act]');
    if (b && !b.disabled) { e.preventDefault(); const f = actions[b.dataset.act] || primary[b.dataset.act]; if (f) f(b.dataset.arg); }
  });
  document.addEventListener('submit', (e) => {
    e.preventDefault();
    const p = e.target.dataset.primary;
    if (p && primary[p]) primary[p]();
  });
  document.addEventListener('change', (e) => {
    const el = e.target;
    if (el.name === 'auth') { ui.auth = el.value; render(true); }
    if (el.name === 'dist') {
      const cur = V.draft.distributions.slice();
      const next = el.checked ? cur.concat([el.value]) : cur.filter((d) => d !== el.value);
      call('set_distributions', { ordered: next });
    }
    if (el.name === 'tls') call('set_tls', { mode: el.value });
  });
  document.addEventListener('input', (e) => {
    if (e.target.name === 'iname') { const s = document.querySelector('[name=slug]'); if (s) s.value = slugOf(e.target.value); }
  });
  document.addEventListener('toggle', (e) => { if (e.target.classList && e.target.classList.contains('oi-details')) ui.details = e.target.open; }, true);
  document.addEventListener('keydown', (e) => {
    if (e.key !== 'Escape') return;
    const c = document.querySelector('.oi-dialog [aria-keyshortcuts="Escape"]') || document.querySelector('[aria-keyshortcuts="Escape"]');
    if (c && !c.disabled) { e.preventDefault(); c.click(); }
  });
  async function poll() {
    try {
      const v = await ipc.invoke('get_view', {});
      if (v) { V = v; render(false); }
    } catch (e) { /* keep polling */ }
    const fast = V && (V.busy || ['installing', 'verifying'].includes(V.screen));
    setTimeout(poll, fast ? 400 : 1500);
  }
  window.OI_APP = { render: (v) => { V = v; MISSING.length = 0; render(true); return MISSING.slice(); }, states: S };
  if (ipc) poll();
})();
