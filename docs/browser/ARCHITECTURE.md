# Ocinye Browser — arquitectura

> Decisões: [ADR-0612](../adrs/0612-browser-manager.md) (Browser Manager),
> [0613](../adrs/0613-ocinye-deep-link-protocol.md) (`ocinye://`),
> [0614](../adrs/0614-browser-downloads-and-uploads.md) (transferências),
> [0615](../adrs/0615-browser-privacy-model.md) (privacidade),
> [0616](../adrs/0616-browser-page-context-for-nye.md) (Nye),
> [0703](../adrs/0703-desktop-trust-boundary-and-native-bridge.md) (fronteira de confiança).
> Visual: pacote D15 do Claude Design (`docs/ui/D15_*.md`). Estado: `PLANNED`.

## Invariantes

1. **Conteúdo web externo não é de confiança**, mesmo dentro do Ocinye Browser.
2. **Webviews externos não recebem** a sessão, os cookies, o armazenamento, os
   cabeçalhos da origem do Ocinye nem a ponte nativa.
3. **O Browser Manager é dono da navegação externa; o Gestor de Janelas é dono
   das janelas do Ocinye. Abas não são janelas.**
4. **Nye e ocsh só usam a API tipada do Browser Manager**; ninguém injecta
   JavaScript numa página, nem o modelo.
5. **Nunca um proxy universal pelo Core.**
6. **A Web degrada com honestidade**: o que não se pode incorporar abre num
   separador novo, e a aba do Ocinye nunca navega para fora.

## Modelo

```text
BrowserWindow { id, window_id (Gestor de Janelas), partition: standard|private, tabs: [BrowserTab], active }
BrowserTab    { id, url (normalizada), title (texto), loading, can_back, can_forward,
                origin, security: https|http|cert|internal|private|none, crashed }
Download      { id, tab, file_name (saneado), size?, destination: ocinye_files|host, state, progress }
SitePermission{ origin, kind: mic|cam|geo|notifications|clipboard|popups|downloads, decision }
```

A origem mostrada na barra vem **do Browser Manager**, nunca da página: uma
página não escreve a barra de endereço.

## API (evento `ocinye-browser`, D15 G-18)

| Acção | Web | Desktop |
|---|---|---|
| `open(url)` | nova aba; `iframe` com *sandbox* ou recurso honesto | webview externo |
| `window.new({private})` | janela; privado = aviso de limitação | partição efémera |
| `tab.new/close/focus`, `tabs.list` | estado no cliente | estado na casca |
| `navigate/reload/back/forward` | no `iframe` quando possível | no webview |
| `current.read_metadata` | URL e título conhecido | URL, título, origem, segurança |
| `current.read_text` | — | extracção controlada (ADR-0616) |

## Análise de URL

Parser da norma WHATWG (`url` em Rust; `URL` no browser), nunca expressões
regulares próprias:

1. `ocinye://…` → parser de ligações internas (ADR-0613).
2. Esquema explícito: `https`/`http` → navegar; `mailto`/`tel` → lista
   permitida com confirmação; o resto → recusado.
3. Sem esquema, com ponto e sem espaços → `https://` + texto.
4. O resto → pesquisa pelo modelo configurado (URL validado depois de gerado).

## Web: `iframe` e recurso honesto

- `sandbox="allow-scripts allow-forms allow-popups allow-popups-to-escape-sandbox"`
  — sem `allow-same-origin` (o site corre numa origem opaca e não alcança nada do
  Ocinye) e sem `allow-top-navigation` (não navega a aba do Ocinye).
- `referrerpolicy="no-referrer"`; `allow` vazio (sem microfone/câmara).
- A CSP do Workspace ganha `frame-src https:` **só** na página `/browser`.
- Sem `load` utilizável em 3 s → «Este site não pode ser mostrado dentro do
  Ocinye Web» + «Abrir num novo separador do navegador» (`noopener,noreferrer`).

## Desktop: webviews

Um webview externo por aba, criado ao abrir e suspenso em segundo plano
conforme a plataforma (medir em R6: memória por aba, 5 e 10 abas). Pop-ups →
nova aba controlada ou recusa. Falha de um webview → «A página falhou ·
Recarregar», sem afectar o Workspace. Certificados inválidos → estado de erro;
continuar só por decisão explícita, só nessa aba e sessão.

## Fora de âmbito (fundação)

Extensões; gestor de palavras-passe; automação de páginas; injecção de
JavaScript; proxy de rede; sincronização de histórico; perfis de browser;
clone do Chromium.
