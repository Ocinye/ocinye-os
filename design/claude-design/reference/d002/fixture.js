/* REFERENCE_ONLY · D002 fixture. Gera a marcação que as funções Rust produzem,
 * com dados de exemplo, para as capturas de referência. Não é código de produção. */
/* Modo de apresentação simulado (?mode=free|tablet|mobile) para capturar
   referências no visor de 924×540: as media queries dos CSS de produção são
   reescritas para a largura do modo (free = ≥ 1100px). O sprite é inlinado para a
   captura. Só referência; em produção valem as media queries reais. */
(async () => {
  const qq = new URLSearchParams(location.search);
  const mode = qq.get('mode') || 'free';
  const vw = { free: 1440, tablet: 1024, mobile: 390 }[mode] || 1440;
  const links = Array.from(document.querySelectorAll('link[rel="stylesheet"][href*="static/"]'));
  for (const l of links) {
    let css = await (await fetch(l.href)).text();
    css = css.replace(/@font-face\s*\{[^}]*\}/g, '');
    css = css.replace(/@media \(max-width: (\d+(?:\.\d+)?)px\)/g, (m, n) => '@media (max-width: ' + (vw <= Number(n) ? 99999 : 0) + 'px)');
    css = css.replace(/@media \(hover: hover\) and \(min-width: (\d+)px\)/g, (m, n) => '@media (hover: hover) and (min-width: ' + (vw >= Number(n) ? 0 : 99999) + 'px)');
    const st = document.createElement('style'); st.textContent = css; l.replaceWith(st);
  }
  const svg = await (await fetch('../../implementation/apps/workspace/static/icons.svg')).text();
  const holder = document.createElement('div'); holder.hidden = true; holder.innerHTML = svg; document.body.prepend(holder);
  if (mode === 'mobile') { const fx = document.createElement('style'); fx.textContent = 'body{background:#0B1520}.oc-shell{width:390px;margin:0 auto}'; document.head.appendChild(fx); }
})().then(async () => {
  const IC = '#';
  const q = new URLSearchParams(location.search);
  const S = q.get('state') || 'one';
  const ic = (n) => `<svg class="oc-icon" aria-hidden="true" focusable="false"><use href="${IC}${n}"></use></svg>`;
  const APPS = { files: ['Ficheiros', 'files'], notes: ['Notas', 'notes'], terminal: ['Terminal', 'terminal'], browser: ['Browser', 'browser'], tasks: ['Tarefas', 'tasks'], calendar: ['Calendário', 'calendar'] };

  const bodies = {
    files: `<div class="fx-files">${['Relatório Q3.pdf|2,4 MB|há 2 h', 'Malha CFD caso B.zip|148 MB|ontem', 'Plano de trabalho 2026.docx|320 KB|22/09', 'Sensor 04 — leituras.csv|12 MB|20/09', 'Fotografias campo/|—|18/09'].map((r) => { const [a, b, c] = r.split('|'); return `<div class="fx-row">${ic(a.endsWith('/') ? 'folder' : 'files')}<span>${a}</span><small>${b}</small><small>${c}</small></div>`; }).join('')}</div>`,
    notes: `<article class="fx-note"><h3>Reunião de unidade · 29 set</h3><p>Rever o plano de trabalho antes de sexta. Confirmar com a equipa de campo as datas da recolha do sensor 04.</p><p>Pendentes: relatório intermédio v3, malha do caso B.</p></article>`,
    terminal: `<pre class="fx-term">ocsh 1.4 · Ocinye OS\n$ ls projectos/\nnzayilu  sensor-04  cfd-caso-b\n$ ▍</pre>`,
  };
  const win = (w) => {
    const [title, icn] = APPS[w.app];
    const max = ['maximized', 'snap-left', 'snap-right'].includes(w.state);
    const n = (k) => ({ min: `Minimizar ${title}`, max: `Maximizar ${title}`, res: `Restaurar ${title}`, close: `Fechar ${title}` }[k]);
    const body = w.content === 'loading'
      ? `<div class="oc-win__loading" role="status"><span class="oc-sr">A abrir…</span><span class="oc-win__skel"></span><span class="oc-win__skel"></span><span class="oc-win__skel oc-win__skel--short"></span></div>`
      : w.content === 'pending' ? `<p class="oc-pending oc-win__state" role="status">${ic('clock')}<span>Esta aplicação ainda não está disponível.</span></p>`
      : w.content === 'error' ? `<div class="oc-win__state"><p class="oc-state oc-state--error" role="alert">${ic('warning')}<span>Não foi possível carregar. Referência OC-7F3A</span></p></div>`
      : (bodies[w.app] || '');
    return `<section class="oc-win" id="win-${w.id}" data-oc="win" data-win="${w.id}" data-app="${w.app}" data-state="${w.state || 'normal'}" ${w.active ? 'data-active=""' : ''} ${w.dirty ? 'data-dirty=""' : ''} ${w.content === 'loading' ? 'data-loading=""' : ''} ${w.drag ? 'data-dragging=""' : ''} ${w.rz ? 'data-resizing=""' : ''} data-z="${w.z}" data-x="${w.x}" data-y="${w.y}" data-w="${w.w}" data-h="${w.h}" aria-labelledby="win-${w.id}-title">
      <header class="oc-win__bar" data-part="win-drag">
        <button type="submit" class="oc-win__back" data-oc="wm" data-op="minimize" aria-label="Voltar ao Desktop">${ic('chev-l')}</button>
        <span class="oc-win__icon" aria-hidden="true">${ic(icn)}</span>
        <span class="oc-win__names"><h2 class="oc-win__title" id="win-${w.id}-title">${title}</h2>${w.sub ? `<span class="oc-win__sub">${w.sub}</span>` : ''}</span>
        ${w.dirty ? '<span class="oc-win__dirty" title="Alterações por guardar"></span>' : ''}
        <a class="oc-win__switch" href="#oc-switcher" aria-label="Alternar janelas">${ic('windows')}</a>
        <form class="oc-win__controls" method="post" action="#">
          <button type="submit" class="oc-win__ctl" data-oc="wm" data-op="minimize" aria-label="${n('min')}" title="${n('min')}">${ic('win-min')}</button>
          <button type="submit" class="oc-win__ctl" data-part="win-max" data-oc="wm" data-op="${max ? 'restore' : 'maximize'}" aria-label="${n(max ? 'res' : 'max')}" title="${n(max ? 'res' : 'max')}">${ic(max ? 'win-restore' : 'win-max')}</button>
          <button type="submit" class="oc-win__ctl oc-win__ctl--close" data-oc="wm" data-op="close" aria-label="${n('close')}" title="${n('close')}">${ic('close')}</button>
        </form>
      </header>
      <div class="oc-win__body" data-part="win-body">${body}</div>
      <span class="oc-win__size" data-part="win-size" aria-hidden="true">${w.rz ? w.w + ' × ' + w.h : ''}</span>
      ${['n', 'e', 's', 'w', 'ne', 'nw', 'se', 'sw'].map((e) => `<span class="oc-win__rz" data-part="win-resize" data-edge="${e}"></span>`).join('')}
    </section>`;
  };
  const shelf = (ws) => `<nav class="oc-shelf" data-oc="shelf" aria-label="Janelas abertas"><a class="oc-shelf__all" href="#oc-switcher" data-oc="switcher-open" aria-label="Todas as janelas">${ic('windows')}</a><ul class="oc-shelf__list">${ws.map((w) => `<li><a class="oc-shelf__item" href="#" data-oc="wm-focus" data-win="${w.id}" data-state="${w.state || 'normal'}" ${w.active && w.state !== 'minimized' ? 'aria-current="true"' : ''}>${ic(APPS[w.app][1])}<span class="oc-shelf__name">${w.sub || APPS[w.app][0]}</span>${w.dirty ? '<span class="oc-shelf__dirty"></span>' : ''}</a></li>`).join('')}</ul></nav>`;
  const switcher = (ws, open) => `<div class="oc-overlay oc-overlay--center" id="oc-switcher" data-oc="switcher" ${open ? 'data-open=""' : ''} role="dialog" aria-modal="true" aria-labelledby="oc-switcher-title"><a class="oc-overlay__scrim" href="#" aria-label="Fechar"></a><div class="oc-switcher"><h2 class="oc-switcher__title" id="oc-switcher-title">Alternar janelas</h2><ul class="oc-switcher__list" data-part="switcher-list">${ws.slice().sort((a, b) => b.z - a.z).map((w, i) => `<li><a class="oc-switch" href="#" data-oc="wm-focus" data-part="switcher-item" ${w.active ? 'aria-current="true"' : ''} ${i === 1 ? 'data-selected=""' : ''}><span class="oc-switch__preview" aria-hidden="true"><span class="oc-switch__bar"></span><span class="oc-switch__glyph">${ic(APPS[w.app][1])}</span><span class="oc-switch__line"></span><span class="oc-switch__line oc-switch__line--short"></span></span><span class="oc-switch__title">${APPS[w.app][0]}</span><span class="oc-switch__sub">${w.sub || ''}</span>${w.state === 'minimized' ? '<span class="oc-switch__tag">Minimizada</span>' : w.state === 'maximized' ? '<span class="oc-switch__tag">Maximizada</span>' : ''}</a></li>`).join('')}</ul><p class="oc-switcher__hint">Alt Tab para avançar · Enter para abrir · Esc para sair</p></div></div>`;
  const chooser = (open) => `<div class="oc-chooser" data-oc="chooser" data-app="files" role="dialog" aria-label="Janelas de Ficheiros" ${open ? '' : 'hidden'} data-fx-chooser=""><p class="oc-chooser__title">Janelas de Ficheiros</p><ul class="oc-chooser__list"><li><a class="oc-chooser__item" href="#" aria-current="true">${ic('files')}<span class="oc-chooser__name">Projectos / Nzayilu</span></a></li><li><a class="oc-chooser__item" href="#" data-state="minimized">${ic('files')}<span class="oc-chooser__name">Relatório Q3.pdf</span><span class="oc-chooser__tag">Minimizada</span></a></li></ul><a class="oc-chooser__new" href="#">${ic('plus')}Nova janela</a></div>`;

  const panels = {
    status: `<div class="oc-panel" data-state="ok"><p class="oc-panel__head"><span class="oc-cap__dot"></span><strong>O Ocinye OS está operacional</strong></p><p class="oc-panel__kicker">Obrigatórias</p><ul class="oc-panel__caps"><li class="oc-cap" data-state="ok"><span class="oc-cap__dot"></span><span class="oc-cap__name">Core</span><span class="oc-cap__state">Operacional</span></li></ul><p class="oc-panel__kicker">Opcionais</p><ul class="oc-panel__caps"><li class="oc-cap" data-state="ok"><span class="oc-cap__dot"></span><span class="oc-cap__name">Computação</span><span class="oc-cap__state">Operacional<span class="oc-cap__detail"> · 3/4 nós</span></span></li><li class="oc-cap" data-state="unknown"><span class="oc-cap__dot"></span><span class="oc-cap__name">Cópias de segurança</span><span class="oc-cap__state">Sem registo</span></li><li class="oc-cap" data-state="down"><span class="oc-cap__dot"></span><span class="oc-cap__name">IA</span><span class="oc-cap__state">Indisponível</span></li></ul><p class="oc-panel__note">${ic('ai')}<span>Sem IA, o Ocinye OS continua a funcionar. Só as funções de IA ficam indisponíveis.</span></p><div class="oc-panel__storage"><span>Armazenamento pessoal · 42% usado</span><meter min="0" max="100" value="42" low="80" high="95" optimum="0"></meter></div></div>`,
    notif: `<div class="oc-panel"><div class="oc-panel__bar"><strong>Notificações</strong><form><button type="submit" class="oc-panel__link">Marcar todas como lidas</button></form></div><ul class="oc-panel__list">${[['Ana Sebastião comentou «Plano de trabalho 2026»', 'Rever a secção 3 antes de sexta.', '5 min', 0], ['Tarefa atribuída', 'Validar dados do sensor 04', '2 h', 0], ['Reunião amanhã às 09:30', 'Visita técnica Namibe', '1 d', 1]].map(([a, b, c, r]) => `<li><a class="oc-note" href="#" ${r ? 'data-read=""' : ''}>${r ? '' : '<span class="oc-note__dot"></span>'}<span class="oc-note__text"><strong>${a}</strong><span>${b}</span></span><span class="oc-note__when">${c}</span></a></li>`).join('')}</ul><a class="oc-panel__foot" href="#">Ver todas${ic('arrow-r')}</a></div>`,
    clock: (() => { let rows = '', d = 1; const lead = 1; for (let w = 0; w < 5; w++) { rows += '<tr>'; for (let i = 0; i < 7; i++) { const idx = w * 7 + i; if (idx < lead || d > 30) rows += '<td></td>'; else { rows += `<td ${d === 29 ? 'data-today="" aria-current="date"' : ''}>${d}</td>`; d++; } } rows += '</tr>'; } return `<div class="oc-panel"><p class="oc-panel__month">setembro de 2026</p><table class="oc-month"><thead><tr>${'STQQSSD'.split('').map((l) => `<th><abbr>${l}</abbr></th>`).join('')}</tr></thead><tbody>${rows}</tbody></table><p class="oc-panel__kicker">Hoje</p><ul class="oc-panel__list"><li><a class="oc-agenda" href="#"><span class="oc-agenda__when">15:00</span><span>Chamada com parceiros</span></a></li><li><a class="oc-agenda" href="#"><span class="oc-agenda__when">17:30</span><span>Revisão solver</span></a></li></ul><a class="oc-panel__foot" href="#">Abrir o Calendário${ic('arrow-r')}</a></div>`; })(),
  };
  const topbar = (open) => `<header class="oc-top">
    <details class="oc-menu"><summary class="oc-logo"><img src="../../implementation/apps/workspace/static/ocinye-logo.png" alt="" width="44" height="44"></summary></details>
    <details class="oc-menu"><summary class="oc-dist">Re</summary></details>
    <form class="oc-nyebar" role="search">${ic('nye')}<input type="search" placeholder="Pergunte ou peça à Nye…"><button type="button" class="oc-nyebar__mic" aria-disabled="true">${ic('mic')}</button><kbd class="oc-kbd">⌘K</kbd></form>
    <details class="oc-menu oc-create"><summary class="oc-create__btn">${ic('plus')}Criar</summary></details>
    <details class="oc-menu oc-panel-menu" ${open === 'status' ? 'open' : ''}><summary class="oc-status"><span class="oc-status__part" data-state="ok"><span class="oc-status__dot"></span>CORE</span><span class="oc-status__part" data-state="down"><span class="oc-status__dot"></span>IA</span></summary><div class="oc-menu__pop oc-panel__pop" role="dialog">${panels.status}</div></details>
    <details class="oc-menu oc-panel-menu" ${open === 'notif' ? 'open' : ''}><summary class="oc-round-btn">${ic('bell')}<span class="oc-badge">2</span></summary><div class="oc-menu__pop oc-panel__pop" role="dialog">${panels.notif}</div></details>
    <details class="oc-menu oc-panel-menu" ${open === 'clock' ? 'open' : ''}><summary class="oc-clock-btn"><span class="oc-clock"><span data-part="clock-date">Ter 29 set</span><span data-part="clock-time">10:42</span></span></summary><div class="oc-menu__pop oc-panel__pop" role="dialog">${panels.clock}</div></details>
  </header>`;
  const dock = (ws) => {
    const pinned = ['files', 'notes', 'calendar', 'tasks', 'terminal'];
    return `<nav class="oc-dock" data-oc="dock"><a class="oc-dock__btn" href="#">${ic('home')}</a><a class="oc-dock__btn oc-dock__brand" href="#">${ic('apps-brand-dark')}</a><span class="oc-dock__sep"></span>${pinned.map((a) => { const mine = ws.filter((w) => w.app === a); const act = mine.some((w) => w.active && w.state !== 'minimized'); return `<a class="oc-dock__btn" href="#" ${act ? 'aria-current="page"' : ''} ${mine.length ? `data-oc="dock-app" data-app="${a}" data-windows="${mine.length}"` : ''}>${ic(APPS[a][1])}${mine.length ? `<span class="oc-dock__run" data-n="${mine.length > 1 ? 2 : 1}" ${act ? 'data-active=""' : ''}></span>` : ''}</a>`; }).join('')}</nav>`;
  };
  const menu = (open) => `<div class="oc-ctx" data-oc="desk-ctx" role="menu" aria-label="Desktop" ${open ? '' : 'hidden'} data-fx-ctx=""><button type="button" class="oc-menu__item" role="menuitem">${ic('plus')}Adicionar widget</button><button type="button" class="oc-menu__item" role="menuitem">${ic('image')}Mudar o fundo</button><button type="button" class="oc-menu__item" role="menuitem">${ic('edit')}Personalizar o Desktop</button><button type="button" class="oc-menu__item" role="menuitem">${ic('restart')}Repor a predefinição…</button><hr class="oc-menu__sep"><a class="oc-menu__item" role="menuitem" href="#">${ic('refresh')}Actualizar</a></div>`;
  const dirty = `<div class="oc-overlay oc-overlay--center" data-open="" data-oc="dirty-close" role="alertdialog" aria-modal="true" aria-labelledby="oc-dirty-title"><form class="oc-dialog"><span class="oc-dialog__icon">${ic('warning')}</span><h2 class="oc-dialog__title" id="oc-dirty-title">Guardar as alterações em «Reunião de unidade · 29 set»?</h2><p class="oc-dialog__body">Se fechar sem guardar, as alterações perdem-se.</p><div class="oc-dialog__actions"><button type="submit" class="oc-btn-plain" data-oc="dirty-cancel">Cancelar</button><span class="oc-dialog__spacer"></span><button type="submit" class="oc-btn-line oc-btn-line--danger">Não guardar</button><button type="submit" class="oc-btn-gold" data-oc="dirty-save">Guardar</button></div></form></div>`;

  const F = { app: 'files', id: 'f1', sub: 'Projectos / Nzayilu', x: 28, y: 22, w: 500, h: 330, z: 1 };
  const N = { app: 'notes', id: 'n1', sub: 'Reunião de unidade · 29 set', x: 300, y: 84, w: 470, h: 320, z: 2 };
  const T = { app: 'terminal', id: 't1', sub: 'ocsh', x: 170, y: 150, w: 430, h: 250, z: 3 };
  const set = {
    one: [{ ...F, active: 1 }],
    two: [F, { ...N, active: 1 }],
    three: [F, { ...N }, { ...T, active: 1 }],
    max: [F, { ...N, active: 1, state: 'maximized' }],
    min: [{ ...F, active: 1 }, { ...N, state: 'minimized', dirty: 1 }],
    switcher: [F, { ...N, state: 'minimized' }, { ...T, active: 1 }],
    snap: [{ ...N }, { ...F, active: 1, drag: 1, x: 6, y: 70, z: 3 }],
    snapped: [{ ...F, state: 'snap-left' }, { ...N, state: 'snap-right', active: 1 }],
    resize: [F, { ...N, active: 1, rz: 1, w: 500, h: 360 }],
    dirty: [F, { ...N, active: 1, dirty: 1 }],
    chooser: [{ ...F, active: 1 }, { ...F, id: 'f2', sub: 'Relatório Q3.pdf', state: 'minimized', z: 0 }, { ...N }],
    ctx: [], status: [F], notif: [F], clock: [F],
    states: [{ ...F, content: 'loading', x: 20, y: 16, w: 390, h: 250 }, { ...N, app: 'browser', id: 'b1', sub: '', content: 'pending', x: 430, y: 30, w: 380, h: 240, z: 2 }, { ...T, app: 'tasks', id: 'k1', sub: '', content: 'error', x: 200, y: 200, w: 400, h: 200, z: 3, active: 1 }],
    tablet: [F, { ...N, active: 1 }], mobile: [F, { ...N, active: 1 }],
  }[S] || [];
  const desk = S === 'ctx' || S === 'status' || S === 'notif' || S === 'clock' || !set.length;
  document.getElementById('root').outerHTML = `<div class="oc-shell">${topbar(S)}<div class="oc-desk" data-wall="ocinye" data-dim="20" data-wm="">${dock(set)}<div class="oc-desk__work"><main class="oc-desk__main" id="oc-main">${desk ? '<div class="fx-desk-empty"></div>' : ''}${menu(S === 'ctx')}</main><div class="oc-wm" data-oc="wm-layer">${set.map(win).join('')}<div class="oc-snap" data-part="snap-preview" ${S === 'snap' ? 'data-zone="left"' : 'hidden'}></div>${set.length ? shelf(set) : ''}${chooser(S === 'chooser')}</div></div></div>${switcher(set, S === 'switcher')}${S === 'dirty' ? dirty : ''}</div>`;
  const st = document.createElement('link'); st.rel = 'stylesheet'; st.href = 'fixture.css'; document.head.appendChild(st);
  if (S === 'ctx') { const m = document.querySelector('[data-fx-ctx]'); m.style.setProperty('--ctx-x', '420px'); m.style.setProperty('--ctx-y', '180px'); }
  if (S === 'chooser') { document.querySelector('[data-fx-chooser]').style.setProperty('--chooser-y', '150px'); }
  for (const src of ['../../implementation/apps/workspace/static/oc-shell.js', '../../implementation/apps/workspace/static/oc-wm.js']) {
    await new Promise((ok) => { const sc = document.createElement('script'); sc.src = src; sc.onload = ok; document.body.appendChild(sc); });
  }
  if (S === 'switcher') document.querySelector('[data-oc="switcher"]').setAttribute('data-open', '');
  document.documentElement.setAttribute('data-fx-ready', '');
});
