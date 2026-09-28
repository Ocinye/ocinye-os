# `apps/workspace` — Ocinye Workspace

A principal interface humana do Ocinye OS: um cliente do Ocinye Core, nunca uma
segunda autoridade (`CLAUDE.md` §4).

## Estado: sem interface

A UI foi apagada por inteiro a 2026-09-28, à espera do código do Claude Design.
O que saiu, o que ficou e porquê está em
[`docs/ui/UI_WIPE_REPORT.md`](../../docs/ui/UI_WIPE_REPORT.md).

Enquanto não houver ecrãs:

- cada página (`GET`) responde `503` com o corpo `interface_pending`;
- as acções (`POST`, `PUT`, `DELETE`) continuam a falar com o Core e
  redireccionam como antes; uma recusa responde com o estado HTTP e um código
  estável (`not_found`, `forbidden`, `rejected`, `conflict`, …);
- o que serve bytes ou JSON continua: descargas, pré-visualizações, rascunhos,
  carregamentos por partes, gravação de notas, fixações de aplicações;
- as acções que produzem um segredo mostrado uma única vez (credenciais
  temporárias, códigos de recuperação) estão desactivadas, porque sem ecrã o
  segredo perdia-se.

## O que pertence aqui

- **Backend-for-Frontend** ([ADR-0601](../../docs/adrs/0601-workspace-bff-session.md)):
  sessão do lado do servidor, os tokens do Core nunca chegam ao browser, cookie
  opaco `HttpOnly` · `SameSite` · `Secure`.
- **Fronteira same-origin** para toda a escrita, cabeçalhos de segurança e CSP.
- **O portão de arranque** (`/boot`) e o idioma do pedido (`oc_locale`).
- **O registo de aplicações e o modelo de navegação**
  (`src/experience/`, §45-A), que validam as fixações do membro.
- **O Terminal no servidor** (`src/terminal.rs`): desenha as respostas tipadas
  do Core em texto, com as suas chaves de i18n.
- **A declaração de runtime** (`static/runtime.js`, ADR-0611).

## O que não pertence aqui

Autorização (é do Core), estado institucional (é do Core) e desenho (é do
código do Claude Design, que ainda não chegou).

## Estrutura

```
src/
  api.rs         cliente do Ocinye Core
  boot.rs        o portão de arranque e a sonda ao /ready
  config.rs      configuração por ambiente
  experience/    registo de aplicações e modelo de navegação (sem apresentação)
  i18n/          catálogo pt/en/fr das chaves que o servidor ainda usa
  routes.rs      as rotas: acções, bytes, JSON e as páginas pendentes
  session.rs     sessões do lado do servidor
  terminal.rs    o Terminal em texto
static/
  runtime.js     a declaração de runtime (ADR-0611)
  avatars/       os avatares predefinidos que o Core oferece
  ocinye-logo-email.png   o logótipo da assinatura de correio
tests/
  runtime_boundary.rs     só o runtime.js pergunta ao ambiente
  security_headers.rs     cabeçalhos e transporte
```

## Configuração

Ver [`.env.example`](../../.env.example). Em produção recusa arrancar sem HTTPS
e sem cookies seguros.

## Execução e testes

```bash
set -a && source .env && set +a
cargo run --bin ocinye-workspace     # http://localhost:8090
cargo test -p ocinye-workspace
```

## Limitações declaradas

- **Não se usa por um browser.** Sem ecrãs, o login é um `POST /login` sem
  página que o envie.
- **Sessões em memória**: um reinício termina-as.
- **As provas de instalação, actualização, restauro e hardware terminam em
  `NOT_RUN`**, porque criavam dados por um browser.
