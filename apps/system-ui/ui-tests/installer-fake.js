/* Bancada da janela do instalador: um motor de faz-de-conta que fala em linhas
   e espera por linhas, com as mesmas marcas do verdadeiro (títulos `== `,
   perguntas terminadas em ` > `, texto hostil incluído). Não instala nada. */
(function () {
  'use strict';
  const F = window.OI_FAKE_STATE = { out: '', sent: [], step: 0 };
  const script = [
    '\r\n== Ocinye OS · Ambiente de instalação · Bem-vindo\r\nEste suporte instala o Ocinye OS neste computador.\r\nNada é escrito em disco até confirmar.\r\n[Enter] Continuar · [L] Idioma > ',
    '\r\n== Ocinye OS · Ambiente de instalação · Disco\r\n[1]  /dev/vda    64.0 GB  <img src=x onerror=alert(1)>  —\r\nEscolher (1–1) > ',
    'Escreva o código VDA-7Q2K para confirmar > ',
  ];
  F.out = script[0];
  window.OI_FAKE = async (cmd, args) => {
    if (cmd === 'installer_poll') { const text = F.out; F.out = ''; return { state: 'OPEN', text }; }
    if (cmd === 'installer_send') { F.sent.push(args.line); F.step += 1; F.out = script[F.step] || '  [OK] instalado\r\n'; return true; }
    return null;
  };
})();
