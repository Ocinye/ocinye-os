# ADR-0620 — As aplicações de produtividade: corpos de janela, envio pelo Workspace e conteúdo como texto

- **Estado:** Accepted
- **Domínio:** Workspace
- **Impacto:** HIGH
- **Depende de:** [ADR-0204](0204-institutional-files-and-folders.md) · [ADR-0413](0413-notes-as-institutional-knowledge.md) · [ADR-0402](0402-mail-html-sanitisation.md) · [ADR-0618](0618-window-manager.md) · [ADR-0619](0619-nye-universal-surface.md)
- **Substitui parcialmente:** [ADR-0618](0618-window-manager.md), na parte da política `MultiWindow` («um recurso novo abre outra janela»)
- **Data:** 2026-09-29

## Context

O Claude Design entregou na D004 os ecrãs de Ficheiros, Notas, Calendário e
Correio (`ui/apps/*`, `oc-apps.css`, `oc-apps.js`, `i18n/ui_apps.rs`), como
corpos de janelas geridas da D002. O Core já tinha os contratos de cada
aplicação: ficheiros pessoais com envio por partes, notas com documento
estruturado e revisão, agenda com fuso, correio com rascunhos e política de
envio.

Ligar os ecrãs a esses contratos obrigou a decidir cinco coisas que nenhum dos
dois lados decidia sozinho.

1. **As outras janelas.** O Design diz que só a janela do pedido traz o corpo e
   que as outras «chegam por `?frame=1`» — mas na D002 todas as aplicações
   estavam `app_pending` e nada as carregava. Com ecrãs reais, o corpo de uma
   janela aparecia noutra.
2. **Navegar dentro de uma aplicação.** A ADR-0618 dizia que, numa aplicação
   `MultiWindow`, «um recurso novo abre outra janela». Com a lista das Notas e
   as pastas dos Ficheiros dentro da janela, cada clique abria uma janela nova,
   até à mesa encher.
3. **Quem envia os bytes.** O Design emite `oc:files-upload` e desenha a fila;
   o envio (partes, somas, progresso, cancelar) ficou para a Code.
4. **O conteúdo que não é nosso.** Uma nota, uma mensagem recebida e a
   descrição de um evento são escritas por pessoas — ou por remetentes externos.
   O Leptos 0.8 não escapa os filhos de `<textarea>`.
5. **As acções sem estado no Design.** Eliminar definitivamente, mover para uma
   pasta escolhida e ler uma mensagem sem transporte não têm ecrã.

## Decision

**Corpos de janela.** Só a janela do pedido é `Ready` e recebe o corpo; as
janelas de aplicações com ecrã (Ficheiros, Notas, Calendário, Correio, Nye) são
`Loading`, e o `wm-engine.js` pede o seu corpo por `?frame=1` — o endereço que o
servidor guarda para a janela. Um corpo que não chega não entra: a janela passa
a uma ligação para o seu endereço. As aplicações sem ecrã continuam `Pending`.

**Navegação `MultiWindow`** (substitui parcialmente a ADR-0618). Continua a
valer que uma janela que já mostra exactamente o endereço é focada, e que
«Nova janela» abre sempre outra. Muda o resto:

- a **pergunta** é estado da vista (`?item=`, `?folder=`, `?sort=`, `?q=`): o
  mesmo caminho com outra pergunta fica na janela que o mostra;
- navegar a partir da **janela activa da mesma aplicação** — a lista das Notas,
  uma pasta, o redireccionamento depois de gravar — fica nela;
- uma ligação vinda **de fora** da aplicação (o Desktop, a pesquisa, outra
  aplicação) para um recurso que nenhuma janela mostra abre uma janela nova.

**Envio de ficheiros.** O motor é da Code (`static/files-engine.js`) e ouve o
evento do Design. Pergunta primeiro ao Core se cabe, abre uma sessão pelo BFF,
envia partes do tamanho que o Core indica, cada uma com a sua soma, e fecha com
a soma do todo, calculada incrementalmente — nunca o ficheiro inteiro em
memória, nunca um limite fixo no cliente. O progresso é o das partes aceites;
cancelar aborta a sessão no Core; uma falha fica na fila com a razão e, quando
tentar de novo pode mudar alguma coisa, com «tentar de novo».

**Conteúdo como texto.** Tudo o que um membro ou um remetente escreveu chega à
página como texto escapado: os `<textarea>` passam por `text::rcdata`; as Notas
são Markdown restrito que se converte, sem perdas, para o `NoteDocument` do Core
e de volta — o que não cabe no documento fica texto literal; o corpo de uma
mensagem, já higienizado pelo Core, passa a parágrafos de texto simples. Nada
disto é instrução para a Nye: a referência que uma aplicação lhe passa
(`/ai/prompt?ref=note:<id>`) é relida com a sessão do membro, e o que ele não
pode ver não aparece — nem o nome.

**O que o Design não desenhou não se inventa.** Eliminar definitivamente é
recusado até haver uma confirmação governada; mover pela barra de selecção é
recusado enquanto o formulário não tiver destino; ler uma mensagem sem
transporte mostra a indisponibilidade. Cada falta vai ao Design
(`docs/ui/CODE_FEEDBACK.md`).

**O fuso é o do membro.** O Calendário calcula dias, colunas e minutos no fuso
da Instância que o `/me` devolve, e envia ao Core o instante UTC desse fuso.

## Alternatives

| Alternativa | Porque não |
|---|---|
| Desenhar no servidor o corpo de todas as janelas | Uma página pagaria os pedidos ao Core de todas as aplicações abertas, e uma que falhasse falhava a página. |
| Manter «um recurso novo abre outra janela» | Com listas dentro da janela, é uma janela por clique e um `409` à décima. |
| Uma janela por aplicação para todas | Tira a «Nova janela» que o registo concede às Notas e aos Ficheiros. |
| Enviar o ficheiro num só pedido | Um limite fixo e o ficheiro inteiro em memória — o contrário do que o Core já oferece. |
| Mostrar o HTML higienizado de uma mensagem | A higienização é do Core; a página não precisa de confiar nela para mostrar texto. |

## Consequences

- Uma janela de fundo custa um pedido depois de a página carregar; a página
  não espera por ela.
- O `oc-apps.js` do Design liga todas as aplicações da página de uma vez; para
  ligar só a que chegou por `?frame=1`, o motor esconde as outras durante a
  chamada. Pediu-se ao Design um `init(root)`.
- Duas janelas da mesma aplicação só se abrem por «Nova janela», ou de fora
  dela; os testes da D002 que as criavam para outros fins passaram a usá-la.
- A leitura de mensagens, as versões de ficheiros pessoais, os participantes de
  eventos e os ficheiros partilhados continuam a depender de contratos do Core
  ou de transporte (`design-integration.json`, lacunas FG-D4).
- O Core ganhou uma leitura owner-scoped, só com o nome, de um ficheiro pessoal
  pela versão (`GET /api/v1/me/files/{version_id}`), para a Nye dizer de que
  ficheiro se fala.
