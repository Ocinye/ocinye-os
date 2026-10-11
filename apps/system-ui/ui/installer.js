/* Ocinye OS · a janela do ambiente de instalação (ADR-0031, D013).
   Mostra o que o Motor de Instalação diz e devolve-lhe uma linha de cada vez:
   a mesma conversa que ele tem num terminal. O que se pergunta, o que uma
   resposta quer dizer e o que pode ser escrito num disco são dele. Todo o texto
   entra no documento como texto (textContent), nunca como marcação. */
(function () {
  'use strict';
  const ipc = window.__TAURI_INTERNALS__;
  const invoke = (cmd, args) => (ipc ? ipc.invoke(cmd, args || {}) : window.OI_FAKE(cmd, args || {}));
  const transcript = document.getElementById('transcript');
  const form = document.getElementById('answer');
  const prompt = document.getElementById('prompt');
  const line = document.getElementById('line');
  const status = document.getElementById('status');
  // O motor termina uma pergunta com isto, sem mudar de linha.
  const ASKS = ' > ';
  let pending = '';   // o que chegou e ainda não fechou uma linha
  let asking = false;

  function say(text) {
    const p = document.createElement('p');
    const title = text.startsWith('== ');
    p.className = title ? 'oi-line oi-line--title' : 'oi-line';
    p.textContent = title ? text.slice(3) : text;
    transcript.appendChild(p);
    // Um ecrã de conversa, não um arquivo: o que saiu de vista há muito sai.
    while (transcript.childElementCount > 400) transcript.removeChild(transcript.firstChild);
  }

  function ask(text) {
    asking = true;
    prompt.textContent = text;
    form.hidden = false;
    line.value = '';
    line.focus();
  }

  function take(text) {
    pending += text.replace(/\r/g, '');
    let at = pending.indexOf('\n');
    while (at >= 0) {
      say(pending.slice(0, at));
      pending = pending.slice(at + 1);
      at = pending.indexOf('\n');
    }
    if (pending.endsWith(ASKS)) {
      ask(pending.slice(0, -ASKS.length));
      pending = '';
    }
    transcript.scrollTop = transcript.scrollHeight;
  }

  function reset() {
    transcript.textContent = '';
    pending = '';
    asking = false;
    form.hidden = true;
  }

  form.addEventListener('submit', async (e) => {
    e.preventDefault();
    if (!asking) return;
    const answer = line.value;
    let sent = false;
    try { sent = await invoke('installer_send', { line: answer }); } catch (err) { sent = false; }
    if (!sent) return;
    // A resposta fica no registo ao lado da pergunta, como num terminal.
    say(prompt.textContent + ASKS + answer);
    asking = false;
    form.hidden = true;
    transcript.scrollTop = transcript.scrollHeight;
  });
  document.addEventListener('contextmenu', (e) => e.preventDefault());

  async function poll() {
    let got = null;
    try { got = await invoke('installer_poll'); } catch (err) { got = null; }
    if (got && got.text) take(got.text);
    if (!got || got.state === 'WAITING') status.textContent = '…';
    else if (got.state === 'CLOSED') { status.textContent = ''; setTimeout(reset, 1500); }
    else status.textContent = '';
    setTimeout(poll, 250);
  }

  form.hidden = true;
  poll();
})();
