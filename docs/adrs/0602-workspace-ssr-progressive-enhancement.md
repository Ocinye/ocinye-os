# ADR-0602 — Workspace em SSR com progressive enhancement, hidratação adiada

- **Estado:** Accepted
- **Domínio:** Workspace
- **Impacto:** MEDIUM
- **Data:** 2026-08-22
- **Refina:** [ADR-0600](0600-leptos-workspace-runtime.md)

## Context

O dossier de design do Ocinye Workspace (em [`design/`](../../design/README.md))
especifica 20 ecrãs de alta fidelidade com interactividade real: command palette
`⌘K`, sidebar colapsável com persistência, menu `+ Criar`, tabs contextuais,
alternância de densidade das tabelas e estados de hover.

O ADR-0600 escolheu Leptos em SSR puro e adiou a hidratação com uma justificação
explícita: não existia interactividade que a justificasse. **Essa condição
deixou de se verificar.** O design tem agora interacções concretas, e a decisão
tem de ser revisitada em vez de mantida por inércia.

O `CLAUDE.md` §16-A é directo sobre onde o WASM entra: *"WebAssembly complementa
Rust… utilizado estrategicamente para interface, isolamento, portabilidade e
extensibilidade"*, listando "frontend interactivo" como caso legítimo.

Havia ainda um facto operacional relevante: a máquina de desenvolvimento estava
sem espaço em disco, e uma cadeia de build com `cargo-leptos` e
`wasm32-unknown-unknown` acrescenta vários GB. Isso condicionou o momento, não a
direcção.

## Decision

A implementação do design é feita em **Leptos SSR**, mantendo o servidor como
autoridade, com uma **camada de progressive enhancement** para as interacções de
DOM.

### O que fica no servidor

Todo o conteúdo, todos os dados, toda a navegação entre ecrãs e todos os
formulários. Cada ecrã tem um URL próprio e é renderizado por inteiro pelo
servidor, com a sessão dele.

### O que fica no cliente

Um único ficheiro (`static/app.js`, ~300 linhas) que trata exclusivamente de
comportamento de DOM:

| Interacção | Porquê é comportamento de DOM |
|---|---|
| Command palette `⌘K` | Abrir/fechar overlay e filtrar itens já renderizados |
| Sidebar colapsável | Alternar uma classe; a preferência é do browser |
| Menu `+ Criar` | Abrir/fechar um menu já renderizado |
| Tabs locais | Mostrar um painel já enviado pelo servidor |
| Densidade das tabelas | Preferência visual do utilizador |

Regras que este ficheiro respeita, e que a revisão de código verifica:

1. **Não decide autorização.** Nenhuma decisão de acesso passa pelo cliente.
2. **Não guarda tokens nem dados institucionais.** O `localStorage` guarda duas
   preferências visuais e nada mais.
3. **Não obtém dados.** Todos os dados vêm do servidor, com a sessão dele.
4. **É enhancement, não requisito.** Sem JavaScript, a navegação, os
   formulários, as tabelas e todos os ecrãs continuam a funcionar; perdem-se a
   palette e o colapso da sidebar.

### Hidratação continua a ser o destino

Quando o Workspace precisar de estado partilhado no cliente — resultados de
pesquisa incrementais, streaming das respostas do Prompt Ocinye, actualizações
em tempo real — a hidratação Leptos é a resposta, e os componentes já estão
escritos em Leptos. Passar a hidratar é uma mudança de cadeia de build sobre os
mesmos componentes, **não uma reescrita**.

O Prompt Ocinye é o candidato natural a forçar essa passagem, e está desenhado
como o ecrã mais bem resolvido precisamente por isso.

## Alternatives

| Alternativa | Porque foi rejeitada |
|---|---|
| **Leptos SSR + hidratação já** | É o destino declarado. Exige `cargo-leptos`, o alvo `wasm32-unknown-unknown` e uma separação `ssr`/`hydrate` em toda a aplicação — plumbing substancial antes do primeiro ecrã, para interacções que hoje são toggles de DOM. Adiado com condição explícita de reavaliação, não indefinidamente. |
| **React + TypeScript + Vite** | É o que o prompt do dossier sugere *quando o projecto não tem frontend*. Este tem, e é Rust-first por princípio institucional ([ADR-0004](0004-rust-first.md)). Obrigaria ainda a duplicar em TypeScript os tipos que `ocinye-contracts` já define. |
| **Leptos CSR puro** | Atrasa o primeiro render, complica a sessão do lado do servidor e afastaria o Workspace da forma BFF do [ADR-0601](0601-workspace-bff-session.md). |
| **SSR sem qualquer JavaScript** | Deixaria por implementar a command palette, o colapso da sidebar e o menu `+ Criar`, que o design classifica como não negociáveis. Entregar 17 dos 20 ecrãs e chamar-lhe completo seria falso. |

## Consequences

**Positivas**

- Os 20 ecrãs são entregues com fidelidade ao design, sem cadeia de build WASM.
- O servidor continua a ser autoridade: nenhuma decisão institucional passa pelo
  browser.
- Sem JavaScript, o Workspace continua utilizável — o que é uma propriedade de
  acessibilidade, não só de robustez.
- Os componentes Leptos são exactamente os que a hidratação futura usará.

**Negativas, aceites e declaradas**

- Existe JavaScript num repositório Rust-first. É uma excepção delimitada, com
  regras explícitas e revisível: ~300 linhas, sem dados, sem autorização.
- Cada navegação é um round-trip ao servidor. Aceitável para um ambiente
  institucional interno com dados que vêm do Core de qualquer forma.
- O Prompt Ocinye não terá streaming de respostas até haver hidratação — o que
  hoje é irrelevante, porque **não existe nenhum nó de IA** para responder.

## Referências

`CLAUDE.md` §16-A, §4 · `design/README.md` · ADR-0004 · ADR-0600 · ADR-0601
