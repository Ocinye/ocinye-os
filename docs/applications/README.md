# Aplicações do Ocinye OS

> Estado: **`CURRENT` (fatia A)** — o registo autoritativo e o Gestor de
> Aplicações (lançador) existem e estão em produção. A **fixação na barra
> lateral** (persistência por membro) é a fatia seguinte e ainda **não** existe;
> a barra lateral continua, nesta fatia, a mostrar a navegação completa.

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

## O que ainda não existe (fatias seguintes)

- **Fixação de aplicações na barra lateral**, persistida por membro no Core
  (tabela + módulo + rotas), com ordem e conjunto por omissão, e o
  refactor da barra lateral para «navegação essencial + fixadas».
- Integração em Definições (repor fixações), E2E de browser, matriz de browsers e
  capturas de ecrã comparadas com a direcção visual aprovada.
