# ADR-0623 — Terminal e Browser: duas fronteiras, nenhuma ponte de execução comum

- **Estado:** Accepted
- **Domínio:** Workspace
- **Impacto:** HIGH
- **Depende de:** [ADR-0312](0312-ocsh-governed-command-shell.md) · [ADR-0611](0611-runtime-capability-boundary.md) · [ADR-0612](0612-browser-manager.md) · [ADR-0616](0616-browser-page-context-for-nye.md) · [ADR-0703](0703-desktop-trust-boundary-and-native-bridge.md)
- **Data:** 2026-09-30

## Contexto

A D008 desenha as duas superfícies de sistema do Ocinye OS: o Ocinye Terminal
(ocsh) e o Ocinye Browser. As duas têm um «motor» e uma camada de runtime, e a
tentação é juntá-las num `SystemRuntime.execute(...)`: uma ponte nativa, um
executor de scripts, um corredor de comandos. As fronteiras de confiança são
opostas. O ocsh recebe texto de um membro autenticado e converte-o em
capabilities tipadas do Core. O Browser recebe conteúdo de terceiros hostis por
omissão e não lhe dá nada do Ocinye.

As ADRs 0312 e 0612 decidem cada lado. Esta ADR decide o que não existe entre
os dois.

## Decisão

1. **`OcshCapabilityRuntime` ≠ `BrowserRuntime`.** São contratos distintos,
   em módulos distintos, com testes distintos. Nenhum tipo, trait, evento,
   rota ou comando nativo serve os dois.
2. **Nada genérico.** Não existem `Runtime.execute`, `Runtime.shell`,
   `Runtime.webview_exec`, `Runtime.native_call`, `invoke(nome)`, nem um
   comando nativo que receba o nome de outro comando (ADR-0703 §2).
3. **Sem ponte Browser → ocsh.** Nenhuma capability, intent ou evento leva
   texto de uma página para o Terminal, para um comando ou para qualquer
   entrada executável. Copiar e colar é um gesto da pessoa; a colagem de
   várias linhas no Terminal não executa nada sozinha.
4. **Sem ponte ocsh → Browser.** O ocsh não abre, lê, navega nem controla
   abas. Um futuro comando `browser open <url>` exigiria capability própria,
   ADR própria e continuaria a passar pelo Browser Manager como qualquer outro
   pedido de navegação.
5. **Ficheiros de cliente separados.** `static/oc-terminal.js` só em
   `/terminal`; `static/oc-browser.js` só em `/browser`. Um teste de
   arquitectura falha se um importar, chamar ou referir o outro.
6. **A Nye liga-se aos dois pelo caminho canónico.** `nye ask` no Terminal e
   «Perguntar à Nye sobre esta página» no Browser chamam a Superfície Universal
   pelas mesmas regras (ADR-0307, 0616). A Nye não ganha autoridade em nenhum
   dos lados e o texto de uma página nunca é instrução.

## Alternativas

- **Um runtime de sistema partilhado.** Recusado: uma única fronteira
  enfraquecida para o caso pior (conteúdo web hostil com acesso a execução).
- **O Browser expõe `readText` ao ocsh para compor pipelines.** Recusado nesta
  fase: transforma conteúdo não confiável em entrada de comandos.

## Consequências

- `scripts/architecture_boundaries.py` ganha duas regras: nenhum símbolo
  `Runtime.execute|shell|native_call|webview_exec`; nenhuma referência cruzada
  entre `oc-terminal.js` e `oc-browser.js`, nem entre `ui::apps::terminal` e
  `ui::apps::browser` (excepto a função pura `terminal::visible`, que se move
  para um módulo comum de texto se o Code preferir).
- Automação de páginas, se algum dia existir, é um domínio de segurança novo
  com ADR própria e não reaproveita nada do ocsh.

## Aceitação (D008, 2026-09-30)

Aprovada pelo utilizador como a arquitectura canónica da D008. O que ficou
imposto, e onde:

- **Nenhuma camada de execução partilhada.** São recusados em toda a árvore de
  produção `SystemRuntime`, `GenericExecutionRuntime`, `OcshBrowserBridge` e
  `Runtime.execute|shell|native_call|webview_exec`
  (`scripts/architecture_boundaries.py`, provado por reversão).
- **Nenhuma referência cruzada** entre os ficheiros do Terminal
  (`oc-terminal.js`, `ui/apps/terminal.rs`, `terminal.rs`, o módulo `terminal`
  do Core) e os do Browser (`oc-browser.js`, `ui/apps/browser.rs`).
- **Nenhum processo no caminho do ocsh**: `std::process`, `tokio::process`,
  `Command::new` e `/bin/sh` fora dos testes, no Terminal e em
  `ocinye-contracts::ocsh`.
- **Cada cliente só na sua rota**: o literal `/static/oc-terminal.js` e o
  `/static/oc-browser.js` existem só em `routes/sys.rs`, que os junta à página
  de `/terminal` e de `/browser`. Nenhuma das duas se desenha como corpo de uma
  janela de fundo (`?frame=1` responde sem corpo, e o Gestor de Janelas mostra
  a ligação para o endereço).
- **CSP por rota.** Só a resposta de `/browser` aceita molduras `https:`
  (`frame-src 'self' https:`); todas as outras continuam `frame-src 'self'`.

**O runtime Desktop não está provado.** O repositório não tem casca nativa: o
isolamento de webviews, as partições de sessão, o foco nativo, as transferências
e as permissões de sites são especificação do produto e ficam
`DESKTOP_RUNTIME_REQUIRED` · `DESKTOP_WEBVIEW_RUNTIME_VALIDATION = NOT_CERTIFIED`
até uma casca existir e esses caminhos correrem. O mesmo vale para o Dedicado.
