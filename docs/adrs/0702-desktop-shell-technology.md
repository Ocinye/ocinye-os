# ADR-0702 — A casca Ocinye Desktop: Tauri 2 sobre o WebView do sistema

- **Estado:** Proposed
- **Domínio:** Operations
- **Impacto:** HIGH
- **Depende de:** [ADR-0004](0004-rust-first.md) · [ADR-0018](0018-universal-web-access-and-runtime-classes.md) · [ADR-0611](0611-runtime-capability-boundary.md)
- **Data:** 2026-09-27

## Context

> Proposta a confirmar pela prova de arquitectura da fase R3: a escolha só
> passa a `Accepted` quando as partições e o isolamento forem medidos em cada
> plataforma.

O runtime `DESKTOP` precisa de uma casca nativa em Windows, macOS e Linux que:
hospede o Workspace de uma Instância remota num webview de confiança; crie
**vários** webviews externos isolados para o Browser, com partições de sessão
separadas e uma partição privada efémera; registe `ocinye://`; ofereça diálogos
nativos, notificações e bandeja; e se actualize com pacotes assinados. A casca
**hospeda**; não governa (ADR-0703).

O Ocinye é Rust-first (ADR-0004), mas isso não autoriza reinventar um motor de
navegação.

## Decision

**Tauri 2** (wry + tao), com o **WebView do sistema**: WebView2 (Chromium
Edge) no Windows, WKWebView no macOS, WebKitGTK no Linux. A casca vive no
monorepo em `apps/desktop/`, **fora da workspace do host** (como
`wasm/capabilities`), para que as dependências nativas não entrem em cada
`cargo build` do servidor nem na CI do Core.

| Critério | Tauri 2 |
|---|---|
| Windows / macOS / Linux | sim / sim / sim (WebKitGTK 4.1) |
| Isolamento de webview | webviews separados por janela; ACL de *capabilities* por janela/webview e por origem remota; padrão *isolation* para IPC |
| Vários webviews | sim (`WebviewBuilder`, várias por janela — *feature* `unstable` a confirmar em R3) |
| Conteúdo externo | `WebviewUrl::External`; sem IPC se nenhuma *capability* o permitir |
| Partições | `data_directory` (Windows/Linux), `data_store_identifier` (macOS ≥ 14), `incognito` para privado — **a provar empiricamente em R3** |
| Actualizador | `tauri-plugin-updater`, pacotes assinados (minisign), endpoint fixo na build |
| Protocolos | `tauri-plugin-deep-link` (`ocinye://`) |
| Diálogos / notificações / bandeja | plugins oficiais |
| Segurança | *capabilities* e *permissions* declarativas; CSP própria; sem Node no processo |
| Tamanho | ordem de 5–15 MB (sem motor próprio) |
| Manutenção / licença | Tauri Programme (Commons Conservancy); MIT/Apache-2.0 |
| Actualização do motor | do sistema: WebView2 evergreen; WKWebView com o macOS; WebKitGTK com a distribuição |

### O que isto obriga a declarar

- **O motor não é igual nos três sistemas.** O Browser no macOS e no Linux é
  WebKit; no Windows é Chromium. Diferenças de compatibilidade de sites são do
  motor do anfitrião, e documentam-se; não se prometem iguais.
- **O motor é actualizado pelo anfitrião.** No Linux, a versão do WebKitGTK é a
  da distribuição suportada; o Dedicated fixa uma distribuição (ADR-0705).
- **Partição e privado dependem da plataforma.** macOS < 14 não tem
  `data_store_identifier`; aí o Browser externo corre com `incognito` por janela
  ou recusa-se a abrir uma partição persistente, e di-lo. Tudo o que a matriz
  diz sobre partições é medido em R3, não suposto.

## Alternatives

- **Electron.** Chromium e Node embebidos: partições de sessão maduras
  (`session.fromPartition`), `WebContentsView` para vários webviews, motor igual
  nos três sistemas. Recusado como omissão: não é Rust-first, a casca teria
  ~100 MB e um motor que a Ocinye passa a ter de actualizar, e o processo
  principal Node alarga a superfície nativa. Fica como plano B **se** R3 provar
  que as partições do WebKit não cumprem a ADR-0703.
- **wry + tao directamente.** Mais fino, mas reimplementaria actualizador,
  protocolos, ACL de IPC e empacotamento que o Tauri já mantém.
- **CEF (Chromium Embedded) por ligações Rust.** Motor uniforme, mas pesado,
  com ligações menos mantidas e actualização do motor a cargo da Ocinye.
- **Casca fora do repositório** (como o D15 sugere). Recusado: o contrato entre
  casca e Instância (aperto de mão, `ocinye://`, ponte tipada) é código
  partilhado com `ocinye-contracts`; separá-lo convida à divergência. O
  isolamento de build faz-se excluindo `apps/desktop` da workspace do host.

## Consequences

- R3 prova, em cada sistema separadamente, arranque, ligação à Instância, login,
  webview externo isolado e partição privada. Um sistema não se infere de outro.
- A CI do Core não compila a casca; a casca tem a sua própria pipeline.
- Assinatura (Authenticode, Developer ID + notarização, pacotes Linux
  verificados) é pré-condição de distribuição geral, não da prova (ADR-0704).
