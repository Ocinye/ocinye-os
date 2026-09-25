# Aplicações do Ocinye OS

> Estado: **`CURRENT` (fatias A+B)** — o registo autoritativo, o Gestor de
> Aplicações (lançador) e a **fixação na barra lateral** (persistida por membro
> no Core) existem. A barra lateral é agora **navegação essencial + aplicações
> fixadas**; deixou de ser o catálogo completo. Falta a fatia C (E2E de browser,
> matriz, capturas, integração em Definições e a invariante no CLAUDE.md).

O Ocinye OS trata as aplicações como **entidades de primeira classe**: são
descobertas, pesquisadas e lançadas através de uma camada de aplicações
coerente, em vez de viverem todas numa barra lateral cada vez mais cheia.

## O modelo

Três superfícies, com responsabilidades distintas:

| Superfície | Papel |
|---|---|
| **Gestor de Aplicações** (o lançador) | A superfície **autoritativa de descoberta**: todas as aplicações que o membro pode abrir. |
| **Barra lateral** | Navegação essencial (e, na fatia seguinte, as aplicações que o membro **fixou**). Não é o catálogo completo. |
| **Criar** (`+ Criar`) | Cria um **objecto de domínio** novo (uma ideia, um projecto). É outra coisa que abrir uma aplicação (§35 do briefing). |

## O registo é a fonte única

A identidade e os metadados de cada aplicação existem **uma vez**, no registo
[`apps/workspace/src/ui/apps.rs`](../../apps/workspace/src/ui/apps.rs). Cada
`Application` apoia-se num [`Screen`] tipado — de onde herda `id`, rota, rótulo,
ícone e o direito que a revela — e acrescenta o que é próprio da camada de
aplicações: a **categoria**, a descrição, as palavras de pesquisa e a política de
fixação. O lançador, a pesquisa e (na fatia seguinte) a barra lateral lêem todos
deste registo; **não há uma segunda tabela** de rótulos, rotas ou ícones.

- **A identidade é técnica e estável.** `files`, nunca `Ficheiros`. O idioma só
  muda a apresentação (`pt` → Ficheiros, `en` → Files, `fr` → Fichiers).
- **A categoria é semântica.** `RESEARCH`, apresentada por chave i18n.
- Ver [`APPLICATION_REGISTRY.md`](APPLICATION_REGISTRY.md) para a lista derivada.

## A descoberta nunca contorna a autorização

A visibilidade de uma aplicação segue **exactamente** o mesmo filtro da barra
lateral — permissão institucional ou relevância de módulo, resolvidas pelo Core
em `GET /api/v1/me`. O lançador esconde o que o membro não pode alcançar, mas
esconder é cortesia: a autoridade continua no Core, que recusa quem escrever a
rota à mão (`CLAUDE.md` §4, §59). Uma aplicação de administração nunca fica
utilizável só porque a sua ficha existe.

## Disponibilidade ≠ fornecedor

A **disponibilidade de uma aplicação** e a **disponibilidade de um fornecedor de
inferência** são eixos distintos. O **Prompt Ocinye** é uma aplicação para quem
tem `ai.use`, e **lança mesmo sem GPU** — abre a superfície determinista, que
conclui num envelope tipado `SYSTEM`/`DEGRADED`. O lançador nunca desactiva a
ficha do Prompt por não haver fornecedor (briefing §54).

## O lançador

- Abre-se pelo botão **Aplicações** na barra lateral, ou pelo atalho
  **Cmd/Ctrl + Shift + A** (não colide com o `⌘K` da superfície de comando).
- Tem **pesquisa imediata** (sobre o rótulo e a descrição já traduzidos, mais
  palavras-chave estáveis entre línguas — «file», «fichier» e «ficheiro»
  encontram todas os Ficheiros) e **filtros por categoria**. Ambos no cliente,
  sem um pedido ao Core só para mostrar nomes e ícones.
- Lançar é **navegar para a rota canónica**; abrir a aplicação já aberta fecha o
  lançador sem recarregar.
- `ESC` fecha; o foco entra na pesquisa ao abrir e regressa ao gatilho ao fechar;
  o `Tab` fica preso no painel enquanto está aberto; as setas percorrem a grelha.
- A categoria activa usa o **estado activo canónico do Ocinye** (superfície azul,
  texto branco), o mesmo da navegação — e não um sublinhado dourado
  (`design/README.md` §7.7).

## Prova

- Guardas do registo em [`apps.rs`](../../apps/workspace/src/ui/apps.rs): cada
  aplicação tem rota registada, rótulo e descrição no catálogo, identificador
  único, categoria com aplicações, e política de fixação coerente; a superfície
  de comando (`Search`/`Ask`) não é uma aplicação do lançador.
- Testes de renderização em [`shell.rs`](../../apps/workspace/src/ui/shell.rs): o
  lançador está presente com pesquisa, filtros e grelha; esconde o que o membro
  não pode abrir; e o Prompt aparece a quem tem `ai.use` mesmo sem GPU.
- Completude `pt`/`en`/`fr` de todas as chaves pelo portão de completude do
  catálogo.

## Fixação na barra lateral (fatia B)

- A barra lateral é **navegação essencial** (Home, O Meu Trabalho) mais as
  **aplicações fixadas** pelo membro, pela ordem dele. Deixou de ser o catálogo.
- A fixação é uma **preferência do membro, persistida no Core** —
  `member_app_pins` (migração 0051), lida/escrita por `GET`/`PUT
  /api/v1/me/apps/pins`, resolvida pelo dono da sessão. Segue o membro entre
  browsers e máquinas. **Não altera autorização** (`CLAUDE.md` §4).
- **Sem escolha, o conjunto por omissão** do registo (`notes`, `files`,
  `projects`); uma lista vazia é uma escolha explícita, distinta de nunca ter
  escolhido. Um membro existente sem preferência recebe o padrão sem perder
  acesso — tudo continua descobrível no lançador.
- **`Desafixar ≠ desinstalar`**: tirar uma aplicação da barra tira só o atalho; a
  aplicação continua no lançador e a rota continua a funcionar.
- Fixar/desafixar acontece no lançador (o botão de cada ficha), persiste no Core
  e **actualiza a barra ao vivo**, sem recarregar. A barra nunca oferece uma
  ficha para um ecrã sem autorização — as fixadas são filtradas pela mesma
  visibilidade do lançador.

## O que ainda não existe (fatia C)

- Reordenar por arrastar; integração em Definições (repor fixações); E2E de
  browser, matriz de browsers e capturas comparadas com a direcção visual
  aprovada; a invariante permanente no `CLAUDE.md`.
