/* Ocinye OS · comportamento comum (Claude Design).
 * Só apresentação: nada aqui decide autorização nem guarda dados.
 * Liga-se por data-oc / data-part. Corre depois de runtime.js. */
(() => {
  'use strict';

  /* data-oc="clock": data e hora no idioma do documento («SEGUNDA-FEIRA, 28/09 · 12:20»);
   com data-format="short", «Seg 28 set» em clock-date e «15:17» em clock-time. */
  function clock() {
    const els = document.querySelectorAll('[data-oc="clock"]');
    if (!els.length) return;
    const lang = document.documentElement.lang || 'pt-PT';
    const tick = () => {
      const d = new Date();
      const wd = d.toLocaleDateString(lang, { weekday: 'long' }).toUpperCase();
      const dm = String(d.getDate()).padStart(2, '0') + '/' + String(d.getMonth() + 1).padStart(2, '0');
      const hm = d.toLocaleTimeString(lang, { hour: '2-digit', minute: '2-digit', hour12: false });
      els.forEach((el) => {
        if (el.dataset.format === 'short') {
          // «Seg 28 set» + «15:17» (barra de cima)
          const cap = (s) => s.replace('.', '').replace(/^./, (c) => c.toUpperCase());
          const date = cap(d.toLocaleDateString(lang, { weekday: 'short' })) + ' ' + d.getDate() + ' ' + d.toLocaleDateString(lang, { month: 'short' }).replace('.', '');
          const dEl = el.querySelector('[data-part="clock-date"]');
          const tEl = el.querySelector('[data-part="clock-time"]');
          if (dEl) dEl.textContent = date;
          if (tEl) tEl.textContent = hm;
        } else {
          el.textContent = wd + ', ' + dm + ' · ' + hm;
        }
      });
    };
    tick();
    setInterval(tick, 15000);
  }

  /* data-oc="copy" data-copy-target="id": copia o texto de um elemento. */
  function copy() {
    document.addEventListener('click', (e) => {
      const b = e.target.closest('[data-oc="copy"]');
      if (!b) return;
      const src = document.getElementById(b.dataset.copyTarget || '');
      if (!src || !navigator.clipboard) return;
      navigator.clipboard.writeText(src.textContent.trim()).then(() => {
        b.setAttribute('data-copied', '');
        setTimeout(() => b.removeAttribute('data-copied'), 1600);
      });
    });
  }

  const init = () => { clock(); copy(); };
  if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', init);
  else init();
})();
