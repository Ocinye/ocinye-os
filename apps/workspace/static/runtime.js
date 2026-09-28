/*
 * Ocinye OS — a declaração do runtime (ADR-0611).
 *
 * O ÚNICO ficheiro do Workspace que pergunta ao ambiente em que corre. Todo o
 * resto lê `window.ocinyeRuntime`:
 *
 *   ocinyeRuntime.mode                    'web' | 'desktop' | 'dedicated'
 *   ocinyeRuntime.availability(nome)      'yes' | 'limited' | 'no'
 *   ocinyeRuntime.has(nome)               true para 'yes' e 'limited'
 *   ocinyeRuntime.capabilities()          [[nome, disponibilidade], …]
 *
 * Um teste do Workspace falha se `__TAURI__`, o agente de utilizador ou a ponte
 * nativa aparecerem noutro ficheiro, e se a tabela abaixo divergir de
 * `ocinye_contracts::runtime`.
 *
 * O que isto NÃO faz: decidir o que a pessoa pode. Uma capacidade de runtime
 * diz se o cliente consegue; a autoridade é do Core, igual em todos os
 * runtimes.
 *
 * Nesta fase (R1) só existe a Web. A casca Desktop declara-se por aperto de mão
 * na ponte tipada (R3/R4), nunca por um global que uma página pudesse imitar.
 */

(() => {
  'use strict';

  const CAPABILITY_VERSION = 1;

  /* O alvo da Web, igual a `RuntimeCapability::target(RuntimeMode::Web)`. */
  const WEB_TARGET = Object.freeze({
    native_filesystem: 'no',
    native_open_dialog: 'yes',
    native_save_dialog: 'limited',
    external_webview: 'limited',
    native_notifications: 'limited',
    protocol_handler: 'limited',
    native_clipboard: 'limited',
    background_execution: 'no',
    native_updates: 'no',
    fullscreen_workspace: 'limited',
  });

  /* O navegador pode dar menos do que o alvo, nunca mais: a detecção por
     normas só baixa uma disponibilidade. */
  function webActual() {
    const actual = Object.assign({}, WEB_TARGET);
    const baixar = (nome, quando) => {
      if (quando) actual[nome] = 'no';
    };
    baixar('native_notifications', !('Notification' in window));
    baixar('native_clipboard', !(navigator.clipboard && navigator.clipboard.writeText));
    baixar('fullscreen_workspace', !document.fullscreenEnabled);
    baixar('native_open_dialog', !('FileReader' in window));
    return Object.freeze(actual);
  }

  const mode = 'web';
  const table = webActual();

  const api = Object.freeze({
    CAPABILITY_VERSION,
    mode,
    /* Instalada como PWA: é a mesma Web noutra janela (ADR-0018). */
    standalone: !!(window.matchMedia && window.matchMedia('(display-mode: standalone)').matches),
    availability(nome) {
      return Object.prototype.hasOwnProperty.call(table, nome) ? table[nome] : 'no';
    },
    has(nome) {
      return this.availability(nome) !== 'no';
    },
    capabilities() {
      return Object.keys(table).map((nome) => [nome, table[nome]]);
    },
  });

  Object.defineProperty(window, 'ocinyeRuntime', {
    value: api,
    writable: false,
    configurable: false,
    enumerable: false,
  });
})();
