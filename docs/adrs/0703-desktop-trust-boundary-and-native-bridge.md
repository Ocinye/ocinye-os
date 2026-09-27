# ADR-0703 — A fronteira de confiança da casca: webview de confiança, webviews externos, ponte tipada

- **Estado:** Proposed
- **Domínio:** Operations
- **Impacto:** FOUNDATIONAL
- **Depende de:** [ADR-0702](0702-desktop-shell-technology.md) · [ADR-0601](0601-workspace-bff-session.md) · [ADR-0611](0611-runtime-capability-boundary.md)
- **Data:** 2026-09-27

## Context

A casca Desktop põe no mesmo processo nativo duas coisas de confiança oposta: o
Workspace da Instância, que a pessoa escolheu e que fala com o Core, e páginas
da Internet abertas no Browser, que são hostis por omissão. Se as duas
partilharem cookies, armazenamento ou ponte nativa, um site passa a ter a
sessão do Ocinye ou o sistema de ficheiros da pessoa.

## Decision

### A frase

> **A casca hospeda; o Core governa. Conteúdo web externo não é de confiança,
> nem dentro do Ocinye Browser.**

### 1. Três zonas

| Zona | O que corre | Recebe |
|---|---|---|
| **Webview de confiança** | o Workspace da Instância ligada, e só a sua origem exacta (`https://host[:porta]` validado na ligação) | a ponte nativa tipada; a sessão BFF da Instância (cookie `HttpOnly`) |
| **Webviews externos** | páginas do Browser, uma por aba, numa partição do Browser | nada da ponte; nenhum cookie, armazenamento ou cabeçalho da origem do Ocinye |
| **Ponte nativa** | código Rust da casca | pedidos tipados só do webview de confiança |

### 2. A ponte é uma lista fechada e tipada

Comandos com nome e esquema, cada um uma *capability* do Tauri concedida só à
janela de confiança e só para a origem remota da Instância:
`runtime.describe`, `dialog.save_to_host`, `dialog.open_from_host`,
`notify.show`, `clipboard.write_text`, `browser.*` (o Browser Manager nativo,
ADR-0612), `deep_link.subscribe`, `updater.check/apply`, `workspace.set_mode`.

Não existem — e um teste sobre o registo de comandos falha se aparecerem —
`exec`, `shell`, `invoke(nome)`, leitura ou escrita de caminho arbitrário,
`eval` em webview, nem qualquer comando que receba um nome de comando.

### 3. A origem de confiança é exacta

- A casca só carrega no webview de confiança a origem da Instância confiada
  (ADR-0704 §Ligação). Uma navegação para outra origem é interceptada: se for
  `http(s)`, abre no Browser (webview externo); `ocinye://` segue o router
  (ADR-0613); o resto é recusado.
- Janelas novas pedidas pelo Workspace (`window.open`) seguem a mesma regra.
- A CSP do Workspace não é afrouxada para a casca: a casca adapta-se à Web, não
  o contrário.

### 4. Partições

| Partição | Onde | Duração |
|---|---|---|
| `ocinye:<instância>` | webview de confiança | persistente, uma por Instância |
| `browser:<instância>` | webviews externos normais | persistente, local ao computador |
| `private:<janela>` | webviews de uma janela privada | efémera; apagada ao fechar a última janela privada |

Cookies de sites pertencem à partição do Browser: não são contas Ocinye, não
passam pelo Core, não se sincronizam. Onde a plataforma não isola (ADR-0702),
a casca **não abre** o Browser integrado nessa partição e di-lo.

### 5. Segredos no computador

A sessão é o cookie `HttpOnly` da Instância, guardado pelo motor do webview na
partição da Instância — como num navegador. A casca não lê nem copia tokens
para ficheiros. O que tiver de persistir (lista de Instâncias confiadas,
impressões digitais, preferências locais) fica no directório de dados da
aplicação com permissões do utilizador; um segredo, se algum dia for preciso,
vai para o cofre do sistema (Keychain, Credential Manager, Secret Service).

### 6. Esquemas

Webviews externos só navegam `http` e `https`. `mailto:` e `tel:` passam por
uma lista permitida e pedem confirmação; qualquer outro esquema é recusado,
nunca entregue cegamente ao anfitrião.

## Alternatives

- **Um só webview com iframes para o Browser.** Recusado: `iframe` herda o
  processo e o motor do Workspace, e a maioria dos sites recusa ser incorporada.
- **Ponte genérica `invoke(nome, args)` com verificação no nativo.** Recusado:
  uma ponte genérica é uma superfície de ataque que cresce sozinha.
- **Guardar o token de sessão na casca e injectá-lo.** Recusado: tira a sessão
  do cookie `HttpOnly` e põe-a num sítio que o disco e os processos alcançam.

## Consequences

- Testes negativos obrigatórios (R4/R5/R13): página externa sem acesso a
  cookies, `localStorage`, IPC, `window.parent` ou protocolo privilegiado.
- Um XSS no Workspace alcançaria a ponte; por isso a ponte é estreita e cada
  comando que toque no anfitrião pede mediação da pessoa (diálogo nativo).
