/* Ocinye OS Installer · validação do renderer real com as 60 vistas.
   window.OI_VALIDATE(lang) desenha cada estado nesse idioma, no tamanho
   corrente da janela, e devolve as falhas. Verificações (as da referência
   do Design, agora sobre o produto):
   1 nenhuma chave de texto em falta; 2 nenhum atributo style;
   3 exactamente um h1; 4 sem transbordo horizontal da página;
   5 o botão principal, quando existe, está visível e dentro da janela;
   6 todo o input/select tem nome acessível; 7 há um alvo de foco inicial;
   8 todo o distintivo tem texto; 9 diálogos com aria-modal e título;
   10 nenhuma chave privada no DOM, e a credencial só à vista depois de
   revelada pelo operador. */
(function () {
  'use strict';
  function visible(el) {
    const r = el.getBoundingClientRect();
    return r.width > 0 && r.height > 0 && r.right <= innerWidth + 1 && r.bottom <= innerHeight + 1 && r.left >= -1 && r.top >= -1;
  }
  function named(el) {
    if (el.type === 'hidden') return true;
    if (el.getAttribute('aria-label')) return true;
    if (el.id && document.querySelector(`label[for="${el.id}"]`)) return true;
    return !!el.closest('label');
  }
  function check(name, lang) {
    const v = window.OI_VIEWS[name]();
    v.lang = lang;
    const missing = window.OI_APP.render(v);
    const root = document.getElementById('root');
    const f = [];
    if (missing.length) f.push('chaves em falta: ' + missing.join(','));
    if (root.querySelector('[style]')) f.push('atributo style');
    const h1 = root.querySelectorAll('h1').length;
    if (h1 !== 1) f.push('h1=' + h1);
    if (document.documentElement.scrollWidth > innerWidth + 1) f.push('transbordo horizontal ' + document.documentElement.scrollWidth + '>' + innerWidth);
    const main = root.querySelector('.oi-main');
    if (main && main.scrollWidth > main.clientWidth + 1) f.push('transbordo horizontal no conteúdo');
    const primary = root.querySelector('.oi-foot .oi-btn--primary');
    if (primary && !visible(primary)) f.push('botão principal fora da janela');
    root.querySelectorAll('input,select').forEach((el) => { if (!named(el)) f.push('sem nome acessível: ' + (el.name || el.id)); });
    if (!root.querySelector('[data-autofocus]') && !root.querySelector('#oi-h1[tabindex]')) f.push('sem alvo de foco');
    root.querySelectorAll('.oi-badge').forEach((b) => { if (!b.textContent.trim()) f.push('distintivo sem texto'); });
    root.querySelectorAll('[role="dialog"]').forEach((d) => { if (d.getAttribute('aria-modal') !== 'true' || !d.getAttribute('aria-labelledby')) f.push('diálogo sem aria'); });
    if (/BEGIN [A-Z ]*PRIVATE KEY/.test(root.innerHTML)) f.push('chave privada no DOM');
    const sv = root.querySelector('.oi-secret__v');
    if (sv && !/^[•]+$/.test(sv.textContent.trim())) f.push('credencial à vista sem ter sido revelada');
    return f;
  }
  window.OI_VALIDATE = function (lang) {
    const names = Object.keys(window.OI_VIEWS);
    const out = { size: innerWidth + 'x' + innerHeight, lang, states: names.length, checks: 0, failures: {} };
    for (const n of names) {
      out.checks++;
      const f = check(n, lang);
      if (f.length) out.failures[n] = f;
    }
    out.passed = out.checks - Object.keys(out.failures).length;
    return out;
  };
  window.OI_SHOW = (name, lang) => { const v = window.OI_VIEWS[name](); v.lang = lang || 'pt'; return window.OI_APP.render(v); };
  const q = new URLSearchParams(location.search);
  if (q.get('state')) window.OI_SHOW(q.get('state'), q.get('lang') || 'pt');
})();
