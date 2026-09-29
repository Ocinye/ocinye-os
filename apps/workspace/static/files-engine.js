/* Ocinye OS · motor de envio e de mover de Ficheiros (Code, D004).
 *
 * O Design (oc-apps.js) emite as intenções; este motor executa-as contra o
 * Workspace, que as leva ao Core — o Core é a autoridade de tudo:
 *   oc:files-upload {files, folder}   → preflight, sessão por partes, partes,
 *                                       fecho com a soma do ficheiro inteiro;
 *   oc:files-up-cancel {id}           → pára e aborta a sessão no Core;
 *   oc:files-up-retry {id}            → recomeça esse ficheiro;
 *   oc:files-move {id, target}        → mover um ficheiro para uma pasta.
 *
 * Nada de limite fixo de tamanho: quem diz o que cabe é o preflight do Core.
 * O ficheiro nunca está todo em memória: cada parte lê-se com Blob.slice,
 * entra na soma do todo (SHA-256 incremental) e é largada. O progresso é o que
 * foi medido: partes concluídas sobre o total. Os textos vêm traduzidos do
 * servidor (<template data-part="files-up-strings">). Sem localStorage. */
(() => {
  'use strict';
  const T = () => { const t = document.querySelector('template[data-part="files-up-strings"]'); return t ? t.dataset : {}; };
  const app = () => document.querySelector('[data-oc="app"][data-app="files"]');
  const jobs = new Map();
  let seq = 0;

  function hexDe(buffer) {
    const bytes = new Uint8Array(buffer);
    let s = '';
    for (let i = 0; i < bytes.length; i += 1) s += bytes[i].toString(16).padStart(2, '0');
    return s;
  }

  function criarSha256() {
    const K = new Uint32Array([
      0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
      0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
      0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
      0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
      0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
      0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
      0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
      0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
      0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
      0xc67178f2,
    ]);
    const H = new Uint32Array([
      0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
      0x5be0cd19,
    ]);
    const W = new Uint32Array(64);
    let resto = new Uint8Array(0);
    let total = 0;
    const rotr = (x, n) => (x >>> n) | (x << (32 - n));

    function bloco(b, off) {
      for (let i = 0; i < 16; i += 1) {
        W[i] = (b[off + 4 * i] << 24) | (b[off + 4 * i + 1] << 16)
          | (b[off + 4 * i + 2] << 8) | b[off + 4 * i + 3];
      }
      for (let i = 16; i < 64; i += 1) {
        const s0 = rotr(W[i - 15], 7) ^ rotr(W[i - 15], 18) ^ (W[i - 15] >>> 3);
        const s1 = rotr(W[i - 2], 17) ^ rotr(W[i - 2], 19) ^ (W[i - 2] >>> 10);
        W[i] = (W[i - 16] + s0 + W[i - 7] + s1) | 0;
      }
      let a = H[0], b2 = H[1], c = H[2], d = H[3], e = H[4], f = H[5], g = H[6], h = H[7];
      for (let i = 0; i < 64; i += 1) {
        const s1 = rotr(e, 6) ^ rotr(e, 11) ^ rotr(e, 25);
        const ch = (e & f) ^ (~e & g);
        const t1 = (h + s1 + ch + K[i] + W[i]) | 0;
        const s0 = rotr(a, 2) ^ rotr(a, 13) ^ rotr(a, 22);
        const maj = (a & b2) ^ (a & c) ^ (b2 & c);
        const t2 = (s0 + maj) | 0;
        h = g; g = f; f = e; e = (d + t1) | 0; d = c; c = b2; b2 = a; a = (t1 + t2) | 0;
      }
      H[0] = (H[0] + a) | 0; H[1] = (H[1] + b2) | 0; H[2] = (H[2] + c) | 0; H[3] = (H[3] + d) | 0;
      H[4] = (H[4] + e) | 0; H[5] = (H[5] + f) | 0; H[6] = (H[6] + g) | 0; H[7] = (H[7] + h) | 0;
    }

    function update(bytes) {
      total += bytes.length;
      let dados;
      if (resto.length) {
        dados = new Uint8Array(resto.length + bytes.length);
        dados.set(resto);
        dados.set(bytes, resto.length);
      } else {
        dados = bytes;
      }
      let off = 0;
      while (dados.length - off >= 64) {
        bloco(dados, off);
        off += 64;
      }
      resto = dados.slice(off);
    }

    function hex() {
      const tamBits = total * 8;
      const padLen = (resto.length < 56 ? 56 : 120) - resto.length;
      const fim = new Uint8Array(resto.length + padLen + 8);
      fim.set(resto);
      fim[resto.length] = 0x80;
      const dv = new DataView(fim.buffer);
      dv.setUint32(fim.length - 4, tamBits >>> 0);
      dv.setUint32(fim.length - 8, Math.floor(tamBits / 0x100000000));
      for (let off = 0; off < fim.length; off += 64) bloco(fim, off);
      let s = '';
      for (let i = 0; i < 8; i += 1) s += ('00000000' + (H[i] >>> 0).toString(16)).slice(-8);
      return s;
    }

    return { update, hex };
  }

  function bytes(n) {
    const u = ['B', 'KB', 'MB', 'GB', 'TB'];
    let v = Number(n) || 0;
    let i = 0;
    while (v >= 1024 && i < u.length - 1) { v /= 1024; i += 1; }
    let s = i === 0 || v >= 100 ? String(Math.round(v)) : v.toFixed(1);
    if ((document.documentElement.lang || 'pt').slice(0, 2) !== 'en') s = s.replace('.', ',');
    return s + ' ' + u[i];
  }

  /* A bandeja: a mesma marcação que o Design desenha (ui::apps::files::upload). */
  function icon(name) {
    const ns = 'http://www.w3.org/2000/svg';
    const svg = document.createElementNS(ns, 'svg');
    svg.setAttribute('aria-hidden', 'true');
    svg.setAttribute('focusable', 'false');
    svg.setAttribute('class', 'oc-icon');
    const use = document.createElementNS(ns, 'use');
    use.setAttribute('href', '/static/icons.svg#' + name);
    svg.appendChild(use);
    return svg;
  }
  function tray() {
    const a = app();
    if (!a) return null;
    let ul = a.querySelector('.oc-files-tray ul');
    if (ul) return ul;
    const side = a.querySelector('[data-part="app-side"]');
    if (!side) return null;
    const sec = document.createElement('section');
    sec.className = 'oc-files-tray';
    sec.setAttribute('aria-labelledby', 'oc-files-tray-t');
    const h = document.createElement('h2');
    h.className = 'oc-app-side__title';
    h.id = 'oc-files-tray-t';
    h.textContent = T().uploads || '';
    ul = document.createElement('ul');
    sec.appendChild(h);
    sec.appendChild(ul);
    side.appendChild(sec);
    return ul;
  }
  function render(job) {
    const ul = tray();
    if (!ul) return;
    const s = T();
    let li = ul.querySelector('[data-part="files-upload"][data-id="' + CSS.escape(job.id) + '"]');
    if (!li) {
      li = document.createElement('li');
      li.className = 'oc-files-up';
      li.setAttribute('data-part', 'files-upload');
      li.setAttribute('data-id', job.id);
      ul.appendChild(li);
    }
    li.setAttribute('data-state', job.state);
    li.textContent = '';
    const name = document.createElement('span');
    name.className = 'oc-files-up__name';
    name.textContent = job.file.name;
    const size = document.createElement('span');
    size.className = 'oc-files-up__size';
    size.textContent = bytes(job.file.size);
    name.appendChild(size);
    li.appendChild(name);
    const busy = job.state === 'queued' || job.state === 'checking' || job.state === 'sending';
    if (job.state === 'sending' && job.total) {
      const p = document.createElement('progress');
      p.max = job.total;
      p.value = job.done;
      p.setAttribute('aria-label', job.file.name);
      li.appendChild(p);
    } else if (busy) {
      const p = document.createElement('progress');
      p.setAttribute('aria-label', job.file.name);
      li.appendChild(p);
    }
    const st = document.createElement('span');
    st.className = 'oc-files-up__state';
    st.setAttribute('role', 'status');
    st.textContent = job.state === 'sending'
      ? bytes(Math.min(job.sent, job.file.size)) + ' / ' + bytes(job.file.size)
      : job.message || s[job.state] || '';
    li.appendChild(st);
    const btn = (oc, ic, label) => {
      const b = document.createElement('button');
      b.type = 'button';
      b.className = 'oc-app__icon';
      b.setAttribute('data-oc', oc);
      b.setAttribute('data-id', job.id);
      b.setAttribute('aria-label', (label || '').replace('{name}', job.file.name));
      b.appendChild(icon(ic));
      li.appendChild(b);
    };
    if (busy) btn('files-up-cancel', 'close', s.cancel);
    /* Uma recusa de tipo não muda por tentar de novo: sem «tentar de novo». */
    if ((job.state === 'failed' && !job.final) || job.state === 'cancelled') btn('files-up-retry', 'refresh', s.retry);
  }

  function settle() {
    const all = Array.from(jobs.values());
    if (all.some((j) => j.state === 'queued' || j.state === 'checking' || j.state === 'sending')) return;
    /* Recarregar mostra os ficheiros novos — mas apagaria a linha de um envio
     * que falhou, e com ela a razão e o «tentar de novo». Com uma falha ou um
     * cancelamento na fila, a fila fica; a lista actualiza-se na próxima vista. */
    if (all.some((j) => j.state === 'failed' || j.state === 'cancelled')) return;
    if (all.some((j) => j.state === 'done')) setTimeout(() => location.reload(), 600);
  }

  async function abortSession(job) {
    if (!job.session) return;
    try { await fetch('/files/uploads/' + job.session, { method: 'DELETE' }); } catch (_) { /* a sessão expira no Core */ }
    job.session = null;
  }

  async function run(job) {
    job.state = 'checking'; job.final = false; job.done = 0; job.sent = 0; job.message = ''; job.cancelled = false;
    render(job);
    const json = { 'Content-Type': 'application/json', Accept: 'application/json' };
    try {
      const pre = await fetch('/files/upload-preflight', { method: 'POST', headers: json, body: JSON.stringify({ size_bytes: job.file.size }) });
      if (pre.ok) {
        const cap = await pre.json();
        if (cap && cap.allowed === false) { job.state = 'failed'; job.message = T().full; render(job); return settle(); }
      }
      if (job.cancelled) { job.state = 'cancelled'; render(job); return settle(); }
      const body = { filename: job.file.name, content_type: job.file.type || 'application/octet-stream', size_bytes: job.file.size };
      if (job.folder && job.folder !== 'root') body.folder_id = job.folder;
      const open = await fetch('/files/personal-upload', { method: 'POST', headers: json, body: JSON.stringify(body) });
      if (!open.ok) {
        job.state = 'failed';
        if (open.status === 422) { job.final = true; job.message = T().type; }
        render(job);
        return settle();
      }
      const sess = await open.json();
      job.session = sess.session_id;
      job.total = sess.total_parts;
      const chunk = sess.chunk_size_bytes;
      const whole = criarSha256();
      job.state = 'sending';
      render(job);
      for (let n = 1; n <= job.total; n += 1) {
        if (job.cancelled) { await abortSession(job); job.state = 'cancelled'; render(job); return settle(); }
        const from = (n - 1) * chunk;
        const to = Math.min(from + chunk, job.file.size);
        const buf = await job.file.slice(from, to).arrayBuffer();
        whole.update(new Uint8Array(buf));
        const sum = hexDe(await crypto.subtle.digest('SHA-256', buf));
        const put = await fetch('/files/uploads/' + job.session + '/parts/' + n + '?sha256=' + sum, {
          method: 'PUT', headers: { 'Content-Type': 'application/octet-stream' }, body: buf,
        });
        if (!put.ok) { await abortSession(job); job.state = 'failed'; render(job); return settle(); }
        job.done = n; job.sent = to;
        render(job);
      }
      const fin = await fetch('/files/uploads/' + job.session + '/complete', { method: 'POST', headers: json, body: JSON.stringify({ sha256: whole.hex() }) });
      job.session = null;
      job.state = fin.ok ? 'done' : 'failed';
      render(job);
    } catch (_) {
      await abortSession(job);
      job.state = 'failed';
      render(job);
    }
    return settle();
  }

  document.addEventListener('oc:files-upload', (e) => {
    const d = e.detail || {};
    const files = Array.from(d.files || []);
    if (!files.length) return;
    e.preventDefault();
    files.forEach((file) => {
      seq += 1;
      const job = { id: 'u' + seq, file, folder: d.folder || null, state: 'queued', done: 0, total: 0, sent: 0 };
      jobs.set(job.id, job);
      render(job);
    });
    /* Um de cada vez: o segundo não compete pela largura de banda do primeiro. */
    (async () => { for (const job of Array.from(jobs.values())) if (job.state === 'queued') await run(job); })();
  });
  document.addEventListener('oc:files-up-cancel', (e) => {
    const job = jobs.get(e.detail && e.detail.id);
    if (!job) return;
    e.preventDefault();
    job.cancelled = true;
    if (job.state === 'queued') { job.state = 'cancelled'; render(job); settle(); }
  });
  document.addEventListener('oc:files-up-retry', (e) => {
    const job = jobs.get(e.detail && e.detail.id);
    if (!job) return;
    e.preventDefault();
    run(job);
  });
  document.addEventListener('oc:files-move', async (e) => {
    const d = e.detail || {};
    const file = /^f\.([0-9a-f-]{36})\./.exec(d.id || '');
    const folder = /^d\.([0-9a-f-]{36})$/.exec(d.target || '');
    if (!file || !folder) return;
    e.preventDefault();
    const form = new URLSearchParams({ file_id: file[1], folder_id: folder[1] });
    try { await fetch('/me/files/move', { method: 'POST', body: form, redirect: 'manual' }); } catch (_) { /* recarregar mostra o estado real */ }
    location.reload();
  });
})();
