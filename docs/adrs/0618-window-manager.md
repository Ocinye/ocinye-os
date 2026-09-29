# ADR-0618 — O Gestor de Janelas: estado por sessão no Workspace, autoridade no Core

- **Estado:** Accepted
- **Domínio:** Workspace
- **Impacto:** HIGH
- **Depende de:** [ADR-0016](0016-application-manifest-contract.md) · [ADR-0601](0601-workspace-bff-session.md) · [ADR-0602](0602-workspace-ssr-progressive-enhancement.md) · [ADR-0611](0611-runtime-capability-boundary.md)
- **Data:** 2026-09-29

## Context

O Claude Design entregou na D002 a apresentação das janelas geridas pelo
Ocinye: a janela e os seus controlos, a camada, a prateleira, o alternador, a
escolha entre janelas de uma aplicação, o encaixe, o diálogo de fechar com
trabalho por guardar, o menu do Desktop e os painéis da barra de cima
(`ui/wm/mod.rs`, `oc-wm.css`, `oc-wm.js`). O motor — ordem, foco, geometria,
encaixe, persistência, política de lançamento e ciclo de vida — ficou a Code.

Não havia motor nenhum no repositório (`docs/runtime/CURRENT_STATE.md`: «Gestor
de Janelas ausente»). As sessões do Workspace vivem na memória do processo
(ADR-0601), e o reinício já termina todas.

## Decision

**Papéis.** O Design apresenta; o Gestor de Janelas guarda o estado e o ciclo
de vida das janelas; o registo de aplicações decide a identidade e a política
de lançamento; o Core decide a autoridade. O Gestor de Janelas não é uma camada
de autoridade: abrir uma janela não autoriza nada, e o identificador de uma
janela não é um token.

**Estado.** Uma `Desk` por sessão do Workspace, guardada no `SessionStore`
ao lado da sessão e com a mesma vida: sai com o fim de sessão, com a varredura
das expiradas e com o reinício. Não há estado no browser que conte, e não há
tabela nova no Core. Cada janela tem identificador (`w1`, `w2`… válido só nesta
sessão), aplicação, rota, estado (`normal`, `maximized`, `minimized`,
`snap-left`, `snap-right`), o estado a que volta ao sair de minimizada,
geometria, ordem, e o que a aplicação disse sobre trabalho por guardar. A
janela activa é **calculada** — a de ordem mais alta que não está minimizada —
e nunca guardada. A mesa tem limite (24 janelas).

**Transições.** Funções puras e deterministas sobre a mesa: abrir-ou-focar,
focar, minimizar, restaurar, maximizar, encaixar à esquerda, à direita ou
maximizar (sem quartos), mover e redimensionar, fechar. A ordem é um relógio
lógico da mesa. Geometria validada no servidor: números inválidos recusados;
válidos mas fora da área aproximados, com o mínimo de 360 × 240 e sempre com a
pega (120 × 40) ao alcance; a área que o cliente declara é limitada a valores
plausíveis.

**Política de lançamento.** `ApplicationManifest.launch`: `SingleInstance` ou
`MultiWindow`, no registo e nunca na vista. Ficheiros e Notas aceitam várias
janelas; as outras aplicações têm uma. Lançar de novo uma de uma janela foca a
que existe; «Nova janela» só vale onde o registo deixa, e é uma acção, não um
endereço (redirige para a rota da janela).

**Autoridade.** Uma rota de aplicação abre a sua janela **depois** do portão da
aplicação (`screen_open`, o mesmo filtro do lançador). `POST /wm` passa pelo
mesmo portão. A rota de uma janela tem de ser da sua aplicação e do Workspace
(só caminho e pergunta; nada de esquema, anfitrião, `//`, `\`, `..`). Em cada
página, antes de desenhar, fecham-se as janelas de aplicações que o membro
deixou de poder abrir: uma permissão retirada nunca reabre uma janela. O
conteúdo de cada janela continua a ser pedido ao Core com a autoridade dele.

**Contrato HTTP** (FG-010, FG-026): `GET /wm` (as janelas), `POST /wm`
(abrir ou focar), `POST /wm/{id}` com `op=focus|minimize|maximize|restore|close|snap|move|resize`,
`POST /wm/{id}/close` com `decision=save|discard|cancel`, e
`POST /wm/{id}/state` para a aplicação dizer se tem trabalho por guardar e se o
pode guardar. Com `Accept: application/json` a resposta é o estado inteiro;
sem JavaScript, cada controlo é um formulário que volta à rota da janela
activa. `GET {rota}?frame=1` é só o corpo da janela; `?close={id}` desenha o
diálogo de fechar.

**Fechar.** Uma janela limpa fecha já. Com trabalho por guardar, só com uma
decisão, executada exactamente como foi confirmada: «Cancelar» não muda nada;
«Não guardar» fecha; «Guardar» só se a aplicação disse que consegue, e a janela
fecha quando a aplicação disser que guardou. O motor nunca adivinha o «por
guardar».

**Conteúdo.** Sem `iframe`: o corpo da janela da rota pedida é desenhado pelo
servidor, e os das outras chegariam por `?frame=1` (mesma origem, mesma sessão,
sem segunda casca). Enquanto nenhuma aplicação tem ecrã do Design (FG-021), todas
as janelas mostram o `app_pending` D001 dentro da janela.

**JavaScript.** O `oc-wm.js` (Design) transforma gestos e teclas em intenções
`oc:wm`; o `wm-engine.js` (Code) valida a forma de cada intenção, pergunta ao
servidor e aplica o estado que ele devolve (foco, ordem e geometria no lugar;
uma mudança estrutural volta a desenhar a página). Durante um arrastar mostra a
posição provisória; o que fica é o que o servidor responde. O atalho do
alternador é `Alt` + `W`, igual em todas as plataformas: só o `runtime.js`
pergunta ao ambiente (ADR-0611).

**Apresentação por largura.** O mesmo estado, três apresentações, decididas
pelo CSS do Design: a partir de 1100px, janelas livres; entre 641 e 1099px
(incluindo 924×540), sempre maximizadas, sem arrastar, redimensionar, encaixar
nem o controlo de maximizar; até 640px, uma aplicação activa em ecrã cheio com
«Voltar ao Desktop» e o alternador.

## Alternatives

- **Persistir as janelas no Core** (uma tabela por membro). Recusado nesta
  fatia: as janelas são apresentação de uma sessão, não um facto institucional;
  uma tabela nova traria migração, continuidade e um contrato do Core para um
  estado que acaba com a sessão. Fica possível mais tarde, atrás do mesmo
  `Desk`, se a restauração entre sessões for pedida.
- **Estado no browser** (`localStorage`). Recusado: seria a fonte da verdade
  errada, sem isolamento por sessão nem validação.
- **`iframe` por janela.** Recusado: duplicaria a casca e a sessão em cada
  janela e enfraqueceria as fronteiras de CSP e de navegação.
- **Deduzir a política da vista.** Recusado: a política é do registo.

## Consequences

- Um reinício do Workspace fecha as janelas, como já fecha as sessões.
- Cada página com janelas pede ao Core a identidade e as permissões antes de
  desenhar, e fecha o que deixou de ser visível.
- O `document.rs` do Design não tem lugar para um script de Code; o
  `wm-engine.js` entra no fim do `<head>` só nas páginas com janelas (pedido ao
  Design em `docs/ui/CODE_FEEDBACK.md`).
- As viagens `apps/workspace/tests/d002_journeys.rs` provam o contrato contra
  um Core real; os testes do motor provam as transições.
