/* Ocinye OS · autenticação (Claude Design). Só apresentação.
 * - data-oc="login-steps": endereço → palavra-passe no browser; o formulário é
 *   sempre um único POST /login com os dois campos (nenhum pedido entre passos).
 * - data-oc="otp": espelha o campo único nas seis células.
 * - data-oc="reveal": mostra/oculta a palavra-passe nova (primeiro acesso).
 * - data-oc="save-codes": descarrega os códigos de recuperação num .txt local. */
(() => {
  'use strict';

  function loginSteps(form) {
    const user = form.querySelector('#login-email');
    const pass = form.querySelector('#login-password');
    const next = form.querySelector('[data-oc="login-next"]');
    const who = form.querySelector('[data-oc="login-change"]');
    if (!user || !pass || !next || !who) return;
    const emailEl = who.querySelector('[data-part="login-email"]');
    const initEl = who.querySelector('[data-part="login-initial"]');
    next.hidden = false;
    const go = (step, focus) => {
      form.setAttribute('data-step', step);
      who.hidden = step !== 'pw';
      if (step === 'pw') {
        const v = user.value.trim();
        emailEl.textContent = v;
        initEl.textContent = (v[0] || '').toUpperCase();
        if (focus) pass.focus({ preventScroll: true });
      } else if (focus) user.focus({ preventScroll: true });
    };
    form.setAttribute('data-ready', '');
    go(pass.value || (user.value && form.querySelector('[role="alert"]')) ? 'pw' : 'id', false);
    next.addEventListener('click', () => { if (user.reportValidity()) go('pw', true); });
    user.addEventListener('keydown', (e) => {
      if (e.key === 'Enter' && form.getAttribute('data-step') === 'id') {
        e.preventDefault();
        if (user.reportValidity()) go('pw', true);
      }
    });
    pass.addEventListener('input', () => { if (pass.value && user.value && form.getAttribute('data-step') === 'id') go('pw', false); });
    who.addEventListener('click', () => go('id', true));
  }

  function otp(box) {
    const input = box.querySelector('[data-part="otp-input"]');
    const cells = box.querySelectorAll('[data-part="otp-cell"]');
    if (!input || !cells.length) return;
    box.setAttribute('data-ready', '');
    const sync = () => {
      const digits = (input.value || '').replace(/\D/g, '').slice(0, cells.length);
      if (digits !== input.value) input.value = digits;
      const focused = document.activeElement === input;
      cells.forEach((c, i) => {
        c.textContent = digits[i] || '';
        if (focused && i === Math.min(digits.length, cells.length - 1)) c.setAttribute('data-active', '');
        else c.removeAttribute('data-active');
      });
    };
    ['input', 'focus', 'blur', 'keyup', 'change'].forEach((ev) => input.addEventListener(ev, sync));
    sync();
  }

  function reveal() {
    document.addEventListener('click', (e) => {
      const b = e.target.closest('[data-oc="reveal"]');
      if (!b) return;
      const field = document.getElementById(b.dataset.ocTarget || '');
      if (!field) return;
      const shown = field.type === 'text';
      field.type = shown ? 'password' : 'text';
      const label = b.querySelector('[data-part="reveal-label"]');
      if (label) label.textContent = shown ? b.dataset.labelShow : b.dataset.labelHide;
      b.setAttribute('aria-pressed', String(!shown));
    });
  }

  function saveCodes() {
    document.addEventListener('click', (e) => {
      const b = e.target.closest('[data-oc="save-codes"]');
      if (!b) return;
      const blob = new Blob([(b.dataset.codes || '') + '\n'], { type: 'text/plain' });
      const a = document.createElement('a');
      a.href = URL.createObjectURL(blob);
      a.download = b.dataset.filename || 'codes.txt';
      document.body.appendChild(a);
      a.click();
      setTimeout(() => { URL.revokeObjectURL(a.href); a.remove(); }, 0);
    });
  }

  const init = () => {
    document.querySelectorAll('form[data-oc="login-steps"]').forEach(loginSteps);
    document.querySelectorAll('[data-oc="otp"]').forEach(otp);
    reveal();
    saveCodes();
  };
  if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', init);
  else init();
})();
