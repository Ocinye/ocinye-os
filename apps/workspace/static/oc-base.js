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
          // D001.1: forma curta determinística (pt «Seg 28 set», en «Mon 28 Sep»,
          // fr «Lun 28 sept»). O Intl de pt-PT devolve «segunda» para weekday:'short':
          // o dia fica sempre com 3 letras; o mês é o curto do Intl, sem ponto.
          const bare = (s) => s.replace(/[.,]/g, '').trim();
          const wdS = Array.from(bare(d.toLocaleDateString(lang, { weekday: 'short' }))).slice(0, 3).join('').replace(/^./, (c) => c.toUpperCase());
          const moS = bare(d.toLocaleDateString(lang, { month: 'short' }));
          const date = wdS + ' ' + d.getDate() + ' ' + moS;
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
