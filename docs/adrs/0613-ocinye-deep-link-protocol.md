# ADR-0613 — `ocinye://`: um protocolo de ligações internas, nunca um canal de comandos

- **Estado:** Proposed
- **Domínio:** Workspace
- **Impacto:** HIGH
- **Depende de:** [ADR-0018](0018-universal-web-access-and-runtime-classes.md) · [ADR-0612](0612-browser-manager.md) · [ADR-0703](0703-desktop-trust-boundary-and-native-bridge.md)
- **Data:** 2026-09-27

## Context

O Browser, o Terminal, o Nye e as notificações precisam de apontar para
recursos do Ocinye (`um ficheiro`, `um projecto`, `o Terminal`). No Desktop, o
sistema anfitrião pode entregar ligações `ocinye://` à casca. Um protocolo
personalizado é um ponto de entrada vindo de fora — de um email, de uma página
— e muitos produtos tornaram-no num executor remoto.

## Decision

### 1. A gramática é pequena e fechada

```text
ocinye://<tipo>/<id>[#<âncora>]
tipo ∈ { apps, files, notes, projects, tasks, workspaces, settings, browser }
```

- `apps/<ApplicationId>` — só ids do manifesto (ADR-0016).
- `files|notes|projects|tasks|workspaces/<uuid>` — UUID validado.
- `settings/<secção>` e `browser/<página>` — listas fechadas.
- Sem query string, sem parâmetros de comando, sem `..`, sem separadores
  codificados (`%2F`), sem `@`, sem porta, sem utilizador; limite de 256 bytes.

O parser é um só, puro, em `ocinye-contracts`, usado pelo Workspace e pela
casca; tem testes negativos e *fuzz* determinístico.

### 2. Uma ligação abre, não executa

`ocinye://apps/terminal` → «abrir a aplicação Terminal» pelo router do
Workspace. Nunca `execute("terminal")`. Abrir um recurso é uma **navegação**:
a autoridade é a da rota HTTPS equivalente, e o Core decide como sempre.

### 3. Toda a ligação tem equivalente HTTPS

Cada forma mapeia para uma rota do Workspace (`ocinye://files/<id>` →
`https://<instância>/files/<id>`). Sem Desktop, a ligação abre a rota Web. A
instalação nunca é precisa para chegar a um recurso.

### 4. Várias Instâncias

`ocinye://` não nomeia a Instância. Com mais de uma confiada, a casca pergunta
a qual abrir e nunca escolhe pela ligação; uma ligação nunca troca de Instância
sozinha.

## Alternatives

- **`ocinye://run?cmd=…`.** Recusado: é um canal de comandos.
- **Só HTTPS.** Não chega no Desktop (o anfitrião entrega ligações por esquema),
  mas é a base: cada `ocinye://` é açúcar sobre uma rota HTTPS.

## Consequences

- Ligações malformadas (`ocinye://system/../..`, `ocinye://exec/…`, tipos
  desconhecidos, payloads enormes) são recusadas antes de qualquer efeito, com
  testes que o provam.
