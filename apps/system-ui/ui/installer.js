/* Ocinye OS · instalador (ADR-0031, D013 · proposta oficial de design).
   Um cliente: desenha o estado tipado que o Motor de Instalação envia e
   devolve-lhe acções tipadas. Não sabe instalar, não escolhe discos e não
   decide se uma confirmação está certa — pergunta, e mostra a resposta.
   Marcação só de classes (CSP style-src 'self'); todo o valor que vem do motor
   ou da pessoa passa por esc(). */
(function () {
  'use strict';
  const T = window.OI_T;
  const ipc = window.__TAURI_INTERNALS__;
  const invoke = (cmd, args) => (ipc ? ipc.invoke(cmd, args || {}) : window.OI_FAKE(cmd, args || {}));

  const S = {
    lang: 'pt',
    link: 'WAITING',       // WAITING | OPEN | CLOSED — a ligação ao motor
    engine: null,          // o último passo que o motor disse
    page: 'welcome',       // antes de o motor conduzir: welcome | language
    chosen: null,          // by_path do cartão marcado; só vai ao motor em «Continuar»
    dialog: false,         // o diálogo de confirmação está aberto
    typed: '',
    checks: [],            // textos enviados a verificar, pela ordem em que foram
    checked: null,         // { typed, valid } — a última resposta do motor
    refusedConfirm: false,
    stages: [],            // { name, ok } pela ordem em que acabaram
    details: [],
    showDetails: false,
    busy: false,
    focusNext: null,       // o controlo que deve receber o foco no próximo desenho
    langChosen: false,
  };
  const UNRESOLVED = [];
  const MISSING = [];
  window.OI_STATE = S;
  window.OI_UNRESOLVED = UNRESOLVED;
  window.OI_MISSING = MISSING;

  // ── textos
  const LI = () => ({ pt: 0, en: 1, fr: 2 }[S.lang] || 0);
  const LOCALE = () => ({ pt: 'pt-PT', en: 'en-GB', fr: 'fr-FR' }[S.lang]);
  const esc = (s) => String(s == null ? '' : s).replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;');
  const t = (k, v) => {
    const e = T[k];
    if (!e) { MISSING.push(k); return esc(k); }
    let s = e[LI()];
    if (v) for (const n in v) s = s.split('{' + n + '}').join(String(v[n]));
    // Um campo que ninguém preencheu não vai para o ecrã: fica registado, e o
    // teste que percorre o assistente recusa-o.
    if (/\{[a-z_]+\}|\$\{/.test(s)) { UNRESOLVED.push(k); s = s.replace(/\$?\{[a-z_]*\}?/g, ''); }
    return esc(s);
  };
  const size = (n) => (n >= 1e12 ? (n / 1e12).toFixed(1) + ' TB' : (n / 1e9).toFixed(1) + ' GB');
  const mem = (n) => (n / 1073741824).toFixed(1) + ' GiB';
  const ic = (id) => `<svg class="sx-icon" aria-hidden="true" focusable="false"><use href="icons.svg#${id}"></use></svg>`;
  const btn = (kind, label, act, o = {}) => `<button class="oi-btn${kind ? ' oi-btn--' + kind : ''}${o.xl ? ' oi-btn--xl' : ''}" type="${o.submit ? 'submit' : 'button'}" id="b-${act || 'submit'}"${act ? ` data-act="${act}"` : ''}${o.disabled || S.busy ? ' disabled' : ''}${o.af ? ' data-af' : ''}>${o.before ? ic(o.before) : ''}${label}${o.after ? ic(o.after) : ''}</button>`;
  const badge = (tone, text, icon) => `<span class="oi-badge" data-tone="${tone}">${icon ? ic(icon) : ''}${text}</span>`;
  const row = (label, value) => `<div class="oi-row"><span class="oi-row__l">${label}</span><span class="oi-row__v">${value}</span></div>`;
  const note = (tone, icon, text) => `<div class="oi-note"${tone ? ` data-tone="${tone}"` : ''}>${ic(icon)}<span>${text}</span></div>`;

  // ── o motor
  async function send(action) {
    try { return await invoke('installer_send', { action }); } catch (e) { return false; }
  }
  function take(event) {
    if (event.event === 'step') {
      const was = S.engine && S.engine.step;
      S.engine = event.state;
      S.busy = false;
      if (event.state.step !== was) {
        S.dialog = false; S.typed = ''; S.checks = []; S.checked = null; S.refusedConfirm = false;
        if (event.state.step === 'WELCOME') { S.page = 'welcome'; S.chosen = null; S.stages = []; S.details = []; S.showDetails = false; }
      }
      // O motor volta a listar os discos em cada passagem: uma escolha que já
      // não existe, ou que deixou de poder ser feita, cai.
      if (event.state.step === 'STORAGE' && !event.state.disks.some((d) => d.by_path === S.chosen && selectable(d))) S.chosen = null;
    } else if (event.event === 'confirmation_checked') {
      // Responde, pela ordem, ao que foi perguntado. Uma recusa sem pergunta
      // pendente é a de uma confirmação que o motor não aceitou.
      const asked = S.checks.shift();
      if (asked === undefined) { S.refusedConfirm = !event.valid; S.checked = { typed: S.typed, valid: false }; S.busy = false; }
      else S.checked = { typed: asked, valid: event.valid };
    } else if (event.event === 'stage') {
      S.stages.push({ name: event.name, ok: event.ok });
    } else if (event.event === 'detail') {
      S.details.push(event.line);
      if (S.details.length > 400) S.details.shift();
    }
  }
  const selectable = (d) => d.protection === 'ELIGIBLE' || d.protection === 'AMBIGUOUS';
  const step = () => (S.engine ? S.engine.step : null);

  // ── o trilho
  const RAIL = ['welcome', 'language', 'computer', 'disk', 'review', 'install', 'done'];
  function at() {
    const e = step();
    if (!e || e === 'WELCOME') return S.page === 'language' ? 1 : 0;
    return { DRIVEN_ELSEWHERE: 1, NOT_INSTALLING: 0, COMPATIBILITY: 2, STORAGE: 3, REVIEW: 4, KEY_OFFER: 5, INSTALLING: 5, INSTALLED: 6, FAILED: 5 }[e];
  }
  const kicker = () => `<div class="oi-k">${t('step.of', { n: at() + 1, m: RAIL.length })}</div>`;
  function rail() {
    const cur = at();
    return RAIL.map((id, i) => `<li class="oi-step"${i === cur ? ' aria-current="step"' : ''} data-at="${i < cur ? 'done' : i === cur ? 'cur' : 'todo'}"><span class="oi-step__n">${i < cur ? ic('check') : i + 1}</span>${t('s.' + id)}</li>`).join('');
  }

  // ── os ecrãs
  const PAGES = {
    welcome() {
      const ready = S.link === 'OPEN' && step() === 'WELCOME';
      return { center: true, body: `<div class="oi-logo"><img src="ocinye-logo.png" alt=""></div>
        <div class="oi-k">${t('w.k')}</div>
        <h1 class="oi-h oi-h--xl" id="oi-h">${t('w.h')}</h1>
        <p class="oi-d oi-d--xl">${t('w.d')}</p>
        ${ready ? btn('gold', t('w.start'), 'start', { xl: true, after: 'arrow-r', af: true }) : `<p class="oi-d" role="status">${t('w.wait')}</p>`}` };
    },
    language() {
      const opt = (code) => `<button class="oi-opt" type="button" role="radio" id="l-${code}" aria-checked="${S.lang === code}" data-act="lang" data-arg="${code}"><span><span class="oi-opt__t">${t('l.' + code)}</span> <span class="oi-opt__code">— ${code}</span></span>${code === 'pt' ? `<span class="oi-opt__badges">${badge('gold', t('l.canonical'))}</span>` : ''}</button>`;
      return { body: `${kicker()}<h1 class="oi-h" id="oi-h">${t('l.h')}</h1><p class="oi-d">${t('l.d')}</p>
        <div class="oi-opts" role="radiogroup" aria-labelledby="oi-h">${['pt', 'en', 'fr'].map(opt).join('')}</div>`,
        foot: `${btn('', t('b.back'), 'to-welcome', { before: 'arrow-l' })}<span class="oi-foot__gap"></span>${btn('gold', t('b.next'), 'begin', { after: 'arrow-r', af: true })}` };
    },
    DRIVEN_ELSEWHERE() {
      return { center: true, body: `${kicker()}<h1 class="oi-h" id="oi-h" tabindex="-1" data-af>${t('o.h')}</h1><p class="oi-d">${t('o.d')}</p>${btn('', t('o.retry'), 'begin', { xl: true })}` };
    },
    NOT_INSTALLING() {
      return { center: true, body: `<h1 class="oi-h" id="oi-h" tabindex="-1" data-af>${t('n.h')}</h1><p class="oi-d">${t('n.d')}</p>` };
    },
    COMPATIBILITY() {
      const e = S.engine;
      const good = (ok, yes, no) => (ok ? badge('ok', yes, 'check') : badge('err', no, 'warn'));
      const blocked = e.blocked;
      return { body: `${kicker()}<h1 class="oi-h" id="oi-h">${t('c.h')}</h1><p class="oi-d">${t('c.d')}</p>
        <div class="oi-rows">
          ${row(t('c.firmware'), good(e.uefi, t('c.uefi'), t('c.bios')))}
          ${row(t('c.arch'), good(e.architecture_supported, t('c.supported'), t('c.unsupported')))}
          ${row(t('c.memory'), `${esc(t('c.memoryv', { have: mem(e.memory_bytes), need: mem(e.memory_required) }))} ${e.memory_bytes >= e.memory_required ? '' : badge('err', t('c.unsupported'), 'warn')}`)}
          ${row(t('c.protect'), good(e.storage_protected, t('c.protected'), t('c.unprotected')))}
        </div>
        ${blocked ? note('err', 'warn', `<strong>${t('blocked.h')}</strong><br>${t(T['err.' + blocked.code] ? 'err.' + blocked.code : 'err.other')} ${t('blocked.safe')}<br><span class="oi-mono">${t('e.code')}: ${esc(blocked.code)}</span>`) : note('', 'shield', t('c.protectnote'))}`,
        foot: blocked
          ? `<span class="oi-foot__gap"></span>${btn('gold', t('b.restart'), 'restart', { before: 'restart', af: true })}`
          : `<span class="oi-foot__gap"></span>${btn('gold', t('b.next'), 'continue', { after: 'arrow-r', af: true })}` };
    },
    STORAGE() {
      const e = S.engine;
      const content = (d) => (d.partitions === 0 ? t('d.empty') : t(d.partitions === 1 ? 'd.parts1' : 'd.parts', { n: d.partitions, fs: d.filesystems.length ? d.filesystems.join(', ') : T['d.nofs'][LI()] }));
      let first = true;
      const card = (d) => {
        const can = selectable(d);
        const on = S.chosen === d.by_path;
        const name = d.model && d.model.trim() ? esc(d.model.trim()) : t('d.unnamed', { device: d.device });
        const tone = can ? 'ok' : d.protection === 'INSTALLATION_MEDIA' ? 'gold' : 'mute';
        const af = can && (on || (first && !S.chosen));
        if (can) first = false;
        return `<div><button class="oi-opt" type="button" role="radio" id="d-${esc(d.device)}" aria-checked="${on}"${can ? ` data-act="choose" data-arg="${esc(d.by_path)}"` : ' aria-disabled="true" disabled'}${af ? ' data-af' : ''}>
            <span class="oi-opt__ic">${ic(can ? 'disk' : 'lock')}</span>
            <span><span class="oi-opt__t">${name}</span><span class="oi-opt__s">${size(d.bytes)} · ${can || d.protection !== 'INSTALLATION_MEDIA' ? content(d) : t('p.INSTALLATION_MEDIA')}</span></span>
            <span class="oi-opt__badges">${badge(tone, t('p.' + d.protection), can ? '' : 'lock')}${can ? '' : badge('mute', t('p.protected'))}</span>
          </button>
          <details class="oi-details"><summary>${t('d.details')}</summary><dl><dt>${t('d.device')}</dt><dd>${esc(d.device)}</dd><dt>${t('d.serial')}</dt><dd>${esc(d.serial || '—')}</dd><dt>${t('d.path')}</dt><dd>${esc(d.by_path || '—')}</dd></dl></details></div>`;
      };
      const any = e.disks.some(selectable);
      return { body: `${kicker()}<h1 class="oi-h" id="oi-h">${t('d.h')}</h1><p class="oi-d">${t('d.d')} ${t('d.min', { min: size(e.minimum_bytes) })}</p>
        ${e.refused ? note('warn', 'warn', `${t('d.refused')} <span class="oi-mono">${esc(e.refused.code)}</span>`) : ''}
        ${any ? '' : note('warn', 'warn', t('d.none'))}
        <div class="oi-opts" role="radiogroup" aria-labelledby="oi-h">${e.disks.map(card).join('')}</div>
        <p class="oi-d"><button class="oi-link" type="button" data-act="rescan"${any ? '' : ' data-af'}>${ic('restart')}${t('d.rescan')}</button></p>`,
        foot: `${btn('quiet', t('b.cancel'), 'restart')}<span class="oi-foot__gap"></span>${btn('gold', t('b.next'), 'select', { after: 'arrow-r', disabled: !S.chosen })}` };
    },
    REVIEW() {
      const d = S.engine.disk;
      const name = d.model && d.model.trim() ? esc(d.model.trim()) : t('d.unnamed', { device: d.device });
      return { body: `${kicker()}<h1 class="oi-h" id="oi-h">${t('r.h')}</h1><p class="oi-d">${t('r.d')}</p>
        <div class="oi-rows">
          ${row(t('r.disk'), `${name} <span class="oi-mono">${esc(d.serial || d.device)}</span>`)}
          ${row(t('r.size'), size(d.bytes))}
          ${row(t('r.now'), d.partitions === 0 ? t('d.empty') : t(d.partitions === 1 ? 'd.parts1' : 'd.parts', { n: d.partitions, fs: d.filesystems.length ? d.filesystems.join(', ') : T['d.nofs'][LI()] }))}
          ${row(t('r.layout'), t('r.layoutv'))}
          ${S.release ? row(t('r.release'), `<span class="oi-mono">${esc(S.release)}</span>`) : ''}
        </div>
        ${note('', 'shield', `<strong>${t('r.remote')}.</strong> ${t('r.remotev')}`)}`,
        foot: `${btn('', t('b.back'), 'back', { before: 'arrow-l' })}${btn('quiet', t('b.cancel'), 'restart')}<span class="oi-foot__gap"></span>${btn('navy', t('r.go'), 'open-confirm', { af: !S.dialog })}`,
        over: S.dialog ? confirmDialog() : '' };
    },
    KEY_OFFER() {
      return { body: `${kicker()}<h1 class="oi-h" id="oi-h">${t('k.h')}</h1><p class="oi-d">${t('k.d')}</p>
        <div class="oi-rows">${row(t('k.fp'), `<span class="oi-mono">${esc(S.engine.fingerprint)}</span>`)}</div>`,
        foot: `<span class="oi-foot__gap"></span>${btn('', t('k.no'), 'key-no', { af: true })}${btn('gold', t('k.yes'), 'key-yes')}` };
    },
    INSTALLING() {
      const ORDER = ['media_check', 'install', 'verify', 'first_boot'];
      const done = S.stages.filter((s) => s.ok).map((s) => s.name);
      const failed = S.stages.filter((s) => !s.ok).map((s) => s.name);
      const next = ORDER.find((n) => !done.includes(n) && !failed.includes(n));
      const rows = ORDER.map((n) => {
        const state = done.includes(n) ? 'done' : failed.includes(n) ? 'failed' : n === next ? 'run' : 'wait';
        return `<li class="oi-stage-row" data-at="${state}"><span class="oi-stage-row__dot"></span><span>${t('st.' + n)}${state === 'run' ? `<span class="oi-stage-row__s">${t('st.' + n + '.s')}</span>` : ''}</span></li>`;
      }).join('');
      return { center: true, body: `${kicker()}<h1 class="oi-h" id="oi-h" tabindex="-1" data-af>${t('i.h')}</h1><p class="oi-d">${t('i.d')}</p>
        <div class="oi-progress"><progress max="${ORDER.length}" value="${done.length}" aria-labelledby="oi-h"></progress></div>
        <ul class="oi-stages">${rows}</ul>${detailsBlock()}` };
    },
    INSTALLED() {
      return { center: true, body: `<span class="oi-done">${ic('check')}</span><h1 class="oi-h oi-h--xl" id="oi-h">${t('f.h')}</h1><p class="oi-d oi-d--xl">${t('f.d')}</p>
        ${btn('gold', t('b.restart'), 'restart', { xl: true, before: 'restart', af: true })}` };
    },
    FAILED() {
      const e = S.engine;
      const known = T['err.' + e.error.code] ? 'err.' + e.error.code : 'err.other';
      return { center: true, body: `<span class="oi-done" data-tone="err">${ic('warn')}</span>
        <h1 class="oi-h" id="oi-h">${t(e.disk_touched ? 'e.h.touched' : 'e.h.safe')}</h1>
        <p class="oi-d">${t(known)}</p>
        ${note(e.disk_touched ? 'warn' : '', e.disk_touched ? 'warn' : 'shield', t(e.disk_touched ? 'e.touched' : 'e.safe'))}
        <p class="oi-d"><span class="oi-mono">${t('e.code')}: ${esc(e.error.code)}</span></p>
        ${detailsBlock()}
        ${btn('gold', t('b.restart'), 'restart', { xl: true, before: 'restart', af: true })}` };
    },
  };

  function detailsBlock() {
    if (!S.details.length) return '';
    return `<p class="oi-d"><button class="oi-link" type="button" data-act="details" aria-expanded="${S.showDetails}">${t(S.showDetails ? 'b.nodetails' : 'b.details')}</button></p>
      ${S.showDetails ? `<div class="oi-log" tabindex="0">${S.details.map(esc).join('\n')}</div>` : ''}`;
  }

  function confirmDialog() {
    const e = S.engine;
    const d = e.disk;
    // O botão só fica activo com a resposta do motor a este texto exacto.
    const answered = S.checked && S.checked.typed === S.typed;
    const valid = answered && S.checked.valid;
    const hint = !S.typed ? '' : !answered ? t('x.checking') : valid ? t('x.valid') : t('x.invalid');
    const tone = !S.typed || !answered ? '' : valid ? 'ok' : 'err';
    const bySerial = d.serial && e.confirmation_hint === d.serial;
    return `<div class="oi-veil"><form class="oi-dialog" role="alertdialog" aria-modal="true" aria-labelledby="oi-x" data-form="confirm">
        <div class="oi-dialog__k">${ic('shield')}${t('x.k')}</div>
        <h2 id="oi-x">${t('x.h')}</h2>
        <div class="oi-dialog__disk"><span class="oi-dialog__serial">${esc(e.confirmation_hint)}</span><span class="oi-dialog__size">${size(d.bytes)}</span></div>
        <p><strong>${t('x.p')}</strong></p>
        <ul class="oi-layout"><li>GPT</li><li>EFI 1 GiB</li><li>ext4</li><li>${S.lang === 'pt' ? 'sem swap' : S.lang === 'fr' ? 'sans swap' : 'no swap'}</li></ul>
        <label class="oi-field" for="oi-typed"><span>${bySerial ? t('x.type', { n: e.confirmation_length }) : t('x.typetoken')} ${bySerial ? '' : `<b>${esc(e.confirmation_hint)}</b>`}</span>
          <input class="oi-input" id="oi-typed" data-f="typed" type="text" value="${esc(S.typed)}" autocomplete="off" autocapitalize="characters" spellcheck="false" maxlength="64"${tone === 'err' ? ' aria-invalid="true"' : ''} aria-describedby="oi-typed-h" data-af>
          <span class="oi-field__hint" id="oi-typed-h" data-tone="${tone}" role="status">${hint}</span></label>
        <div class="oi-dialog__acts">${btn('', t('x.cancel'), 'close-confirm')}${btn('danger', t('x.go'), null, { submit: true, disabled: !valid })}</div>
      </form></div>`;
  }

  // ── desenhar
  let lastHtml = '';
  let lastFocus = '';
  function render() {
    const e = step();
    const key = !e || e === 'WELCOME' ? S.page : e;
    const page = (PAGES[key] || PAGES.welcome)();
    const clock = new Intl.DateTimeFormat(LOCALE(), { hour: '2-digit', minute: '2-digit' }).format(new Date());
    const html = `<div class="oi">
      <div class="oi-top"><span class="oi-top__k">Ocinye OS · ${t('top.k')}</span><span class="oi-top__clock">${clock}</span></div>
      <div class="oi-stage"><div class="oi-card">
        <aside class="oi-aside"><div class="oi-brand"><span class="oi-brand__logo"><img src="ocinye-logo.png" alt=""></span><div><div class="oi-brand__n">OCINYE OS</div><div class="oi-brand__v">${S.release ? t('brand.v', { release: S.release }) : t('top.k')}</div></div></div>
          <ol class="oi-steps" aria-label="${t('top.k')}">${rail()}</ol>
          <div class="oi-aside__foot">${S.development ? t('brand.dev') + '<br>' : ''}pt · en · fr</div></aside>
        <section class="oi-main"><div class="oi-body${page.center ? ' oi-body--center' : ''}">${page.body}</div>${page.foot ? `<div class="oi-foot">${page.foot}</div>` : ''}</section>
        ${page.over || ''}
      </div></div></div>`;
    document.documentElement.lang = LOCALE();
    if (html === lastHtml) return;
    lastHtml = html;
    const root = document.getElementById('root');
    const active = document.activeElement;
    const keep = active && active.id ? { id: active.id, a: active.selectionStart, b: active.selectionEnd } : null;
    const scroll = root.querySelector('.oi-body') ? root.querySelector('.oi-body').scrollTop : 0;
    root.innerHTML = html;
    const focusKey = key + '|' + (S.dialog ? 'x' : '');
    const moved = focusKey !== lastFocus;
    lastFocus = focusKey;
    const scope = root.querySelector('.oi-dialog') || root.querySelector('.oi-main');
    let target = null;
    if (S.focusNext && document.getElementById(S.focusNext) && !document.getElementById(S.focusNext).disabled) target = document.getElementById(S.focusNext);
    else if (moved) target = scope.querySelector('[data-af]') || scope.querySelector('button:not([disabled])') || root.querySelector('#oi-h');
    else {
      if (root.querySelector('.oi-body')) root.querySelector('.oi-body').scrollTop = scroll;
      if (keep) target = document.getElementById(keep.id);
      // Nada tinha o foco (o controlo deste passo só agora apareceu, ou o que
      // o tinha deixou de existir): o foco vai para a acção natural do passo.
      if (!target || target.disabled) target = scope.querySelector('[data-af]:not([disabled])');
    }
    S.focusNext = null;
    if (target) {
      target.focus();
      if (!moved && keep && keep.a != null && target.setSelectionRange) { try { target.setSelectionRange(keep.a, keep.b); } catch (err) { /* sem selecção */ } }
    }
    const h = root.querySelector('#oi-h, #oi-x');
    if (moved && h) document.getElementById('live').textContent = h.textContent;
  }

  // ── acções
  const ACTIONS = {
    start: () => { S.page = 'language'; },
    'to-welcome': () => { S.page = 'welcome'; },
    lang: (arg) => { S.lang = arg; S.langChosen = true; S.focusNext = 'l-' + arg; },
    begin: () => { S.busy = true; send({ action: 'begin' }); },
    continue: () => { S.busy = true; send({ action: 'continue' }); },
    rescan: () => { send({ action: 'rescan' }); },
    // Escolhido o disco, o passo natural é continuar: o foco vai para lá.
    choose: (arg) => { S.chosen = arg; S.focusNext = 'b-select'; },
    select: () => { if (S.chosen) { S.busy = true; send({ action: 'select_disk', by_path: S.chosen }); } },
    back: () => { S.busy = true; send({ action: 'back' }); },
    'open-confirm': () => { S.dialog = true; S.typed = ''; S.checked = null; S.refusedConfirm = false; },
    'close-confirm': () => { S.dialog = false; S.typed = ''; S.checked = null; },
    'key-yes': () => { S.busy = true; send({ action: 'answer_key', authorise: true }); },
    'key-no': () => { S.busy = true; send({ action: 'answer_key', authorise: false }); },
    details: () => { S.showDetails = !S.showDetails; },
    restart: () => { S.busy = true; send({ action: 'restart' }); },
  };

  document.addEventListener('click', (e) => {
    const el = e.target.closest('[data-act]');
    if (!el || el.disabled) return;
    const f = ACTIONS[el.dataset.act];
    if (f) { f(el.dataset.arg); render(); }
  });
  document.addEventListener('input', (e) => {
    const el = e.target.closest('[data-f="typed"]');
    if (!el) return;
    S.typed = el.value;
    S.refusedConfirm = false;
    if (S.typed) { S.checks.push(S.typed); send({ action: 'check_confirmation', typed: S.typed }); }
    render();
  });
  document.addEventListener('submit', (e) => {
    e.preventDefault();
    const form = e.target.closest('[data-form="confirm"]');
    if (!form) return;
    // Só o que o motor já disse estar certo se confirma; e é o motor que,
    // ao recebê-lo, decide outra vez.
    if (!(S.checked && S.checked.typed === S.typed && S.checked.valid)) return;
    S.busy = true;
    send({ action: 'confirm', typed: S.typed });
    render();
  });
  document.addEventListener('keydown', (e) => {
    if (e.key === 'Escape' && S.dialog) { S.dialog = false; S.typed = ''; S.checked = null; render(); }
  });
  document.addEventListener('contextmenu', (e) => e.preventDefault());

  async function poll() {
    let got = null;
    try { got = await invoke('installer_poll'); } catch (err) { got = null; }
    if (got) {
      S.link = got.state;
      for (const event of got.events || []) {
        if (event.event === 'step' && event.state.step === 'WELCOME') {
          S.release = event.state.release_id; S.development = event.state.development;
          if (!S.langChosen && T['l.' + event.state.language]) S.lang = event.state.language;
        }
        take(event);
      }
      if (got.state === 'CLOSED') { S.engine = null; S.page = 'welcome'; S.busy = false; S.dialog = false; }
    }
    render();
    setTimeout(poll, 250);
  }

  render();
  poll();
  setInterval(render, 20000);
})();
