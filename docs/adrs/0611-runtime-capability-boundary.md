# ADR-0611 — A fronteira tipada de capacidades de runtime

- **Estado:** Proposed
- **Domínio:** Workspace
- **Impacto:** HIGH
- **Depende de:** [ADR-0018](0018-universal-web-access-and-runtime-classes.md) · [ADR-0600](0600-leptos-workspace-runtime.md)
- **Data:** 2026-09-27

## Context

O mesmo Workspace vai correr num navegador, numa PWA e dentro de uma casca
nativa (ADR-0018). Algumas coisas mudam: há ou não diálogo nativo de guardar,
webviews externos, `ocinye://` registado, notificações do anfitrião.

A forma habitual de lidar com isso é espalhar perguntas pelo código —
`if (window.__TAURI__)`, `if (navigator.userAgent…)`, `if (desktop)`. Ao fim de
um ano, ninguém sabe o que a Web faz de diferente, a detecção por agente de
utilizador mente, e uma página externa que imite o objecto global ganha o
comportamento privilegiado.

## Decision

### 1. Uma declaração, num sítio

As capacidades de runtime são um tipo fechado, em
`ocinye_contracts::runtime` (partilhado por Workspace e casca):

```text
RuntimeCapabilities {
    mode: web | desktop | dedicated,
    shell_version: Option<Version>,          // só DESKTOP/DEDICATED
    capability_version: u32,
    native_filesystem, native_open_dialog, native_save_dialog,
    external_webview, native_notifications, protocol_handler,
    native_clipboard, background_execution, native_updates,
    fullscreen_workspace: Availability,      // yes | limited | no
}
```

`Availability` tem três valores, e `limited` é um valor de primeira classe: a
Web tem notificações **se** a pessoa as permitir, e isso não é `yes`.

### 2. Um módulo de cliente

No browser, um único módulo (`static/runtime.js`) expõe
`ocinyeRuntime.capabilities()` e `ocinyeRuntime.has(nome)`. É o **único**
ficheiro que pode perguntar ao ambiente — detecção de funcionalidades por
normas (`'Notification' in window`, `matchMedia('(display-mode: standalone)')`)
na Web, e o aperto de mão com a casca no Desktop. Um teste percorre o JavaScript
do Workspace e falha se `__TAURI__`, `userAgent` ou a ponte nativa aparecerem
fora dele.

### 3. A casca declara-se por aperto de mão, não por um global

No `DESKTOP`, a casca não injecta um objecto global que uma página pudesse
imitar. O módulo pede à ponte nativa **tipada** (ADR-0703) o seu `describe()`,
que só existe no webview de confiança; a resposta tem `mode`, `shell_version`,
`capability_version` e as capacidades — e nada do anfitrião além de plataforma e
arquitectura. Sem resposta, o runtime é `WEB`. Um webview externo nunca obtém
resposta, e por isso nunca passa por `DESKTOP`.

### 4. O servidor não decide o runtime

O runtime é do cliente. O Core não confia nele para autorizar nada: uma
capacidade de runtime diz **se o cliente consegue**, nunca **se a pessoa pode**.
O que a pessoa pode continua a ser o Core a decidir, igual em todos os
runtimes.

### 5. Compatibilidade

A casca envia `shell_version` e `capability_version` no aperto de mão com a
Instância; a Instância publica `minimum_desktop_version` e o intervalo de
`capability_version` suportado (ADR-0704). Fora do intervalo, a casca diz porquê
e oferece a Web.

## Alternatives

- **Detecção por agente de utilizador.** Recusado: mente, e muda sem aviso.
- **Um global injectado pela casca (`window.__OCINYE_DESKTOP__`).** Recusado: é
  imitável por conteúdo que chegue ao mesmo contexto, e convida a perguntas
  espalhadas.
- **Builds diferentes do Workspace por runtime.** Recusado: dois produtos
  (ADR-0018).

## Consequences

- Qualquer comportamento dependente do runtime lê `ocinyeRuntime.has(…)`.
- `Definições › Runtime` mostra a mesma declaração que o código usa: o que se
  diz à pessoa é o que o código faz.
- Uma capacidade nova entra no tipo, na matriz e no módulo, na mesma alteração.
