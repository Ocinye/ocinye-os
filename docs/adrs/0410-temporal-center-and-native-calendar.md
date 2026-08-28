# ADR-0410 — Ocinye Temporal Center e Calendário Nativo

- **Estado:** Accepted
- **Domínio:** Calendar
- **Impacto:** HIGH
- **Data:** 2026-08-24
- **Relaciona-se com:** [ADR-0006](0006-modular-monolith.md) ·
  [ADR-0007](0007-domain-boundaries-in-modules.md) ·
  [ADR-0307](0307-dual-entry-single-authority.md) ·
  [ADR-0400](0400-mail-as-institutional-surface.md)

## Context

O Ocinye OS tem um relógio na barra superior desde a auditoria de navegação. É
honesto: mostra a hora do computador de quem está a ver, não persiste nada e não
decide nada. Também não faz mais nada — não se pode clicar nele, porque não há
para onde ir.

Isso não é um esquecimento. A regra que a auditoria de controlos estabeleceu é
que todo o controlo visível muda estado real ou diz que não está disponível, e um
relógio que parecesse clicável e abrisse nada seria a afordância falsa que aquele
trabalho existiu para eliminar.

Falta então a coisa a que o relógio deveria dar entrada. E a pergunta que este
ADR responde não é *como desenhar um calendário* — é **o que é o tempo dentro de
uma instituição científica**, e porque é que isso não cabe num widget.

Três factos que o repositório já tinha, e que tornam a pergunta concreta:

- `Task.due_on` existe. Uma tarefa com prazo **já é** um compromisso temporal, e
  duplicá-la como evento de calendário criaria duas verdades para a mesma data.
- `crates/ocinye-contracts/src/temporal.rs` existe, com `TimeZoneName`,
  `Occurrence` e `LocalTimeProblem` — e **zero consumidores**. Foi escrito como
  fundação e ficou por estrear.
- O sino de notificações existe na barra superior, declarado indisponível com a
  razão à vista: «As notificações ainda não existem no Ocinye Core». Um lembrete
  sem destino visível seria um lembrete que ninguém recebe.

## Decision

### A frase

> **Time in Ocinye has both a canonical instant and, when relevant, a human
> temporal context.**

E as que dela decorrem:

> **UTC defines when an instant occurs; an IANA timezone preserves how that time
> was intended.**

> **Browser time is presentation and input context. Core time is institutional
> authority.**

> **A model may interpret a temporal expression; only the deterministic Core
> converts it into canonical institutional time.**

> **One temporal truth, multiple views.**

### Um instante não chega

«Reunião às 14:00 em Paris» são duas coisas ao mesmo tempo: um ponto na linha do
tempo — o mesmo visto de Luanda ou de Camama — e uma **intenção humana**, que é
*catorze horas, na cidade onde as pessoas estão*.

Guardar só o instante chega para mostrar a hora certa a cada pessoa. Não chega
para **editar**: quem abrir a reunião para a mudar para as 15:00 tem de saber
15:00 de onde. E não chega para a recorrência: «todas as terças às 14:00»
atravessa a mudança de hora de Verão, e sem a zona não há maneira de saber se a
terça seguinte é uma hora antes ou depois.

Por isso um evento com hora guarda **o instante e a zona da intenção**, e não um
`offset` fixo. Um `+01:00` é o resultado de uma regra, não a regra.

### Um evento de dia inteiro não tem hora

Um aniversário, um feriado, um prazo de submissão: são **datas civis**, e não
instantes. Representá-los como «00:00 UTC» seria inventar uma hora que ninguém
marcou, e depois vê-la mudar de dia para quem está noutro fuso.

Evento com hora e evento de dia inteiro são, portanto, **formas distintas** no
modelo, e não a mesma forma com campos a zero.

### Um lembrete não é um evento

Um `Reminder` refere um recurso; não o copia. Referencia um `CalendarEvent`, uma
`Task`, ou outro recurso que o modelo suporte, e guarda apenas o que é seu: quem
recebe, quando dispara, em que estado está.

Duplicar o conteúdo faria com que mudar o evento deixasse o lembrete a dizer
outra coisa — e o lembrete é precisamente o que a pessoa lê primeiro.

### Um prazo de tarefa não é um evento de calendário

Uma `Task` com `due_on` **aparece** no calendário. Não é copiada para lá.

A `Task` continua a pertencer a Collaboration, e o Calendário devolve uma
**projecção temporal**: uma vista que junta eventos, prazos e lembretes numa
linha do tempo, sem que nenhum deles mude de dono.

E a consequência, que é a parte que costuma correr mal:

> **Mudar a data de uma tarefa a partir do calendário termina na operação real
> de actualização da tarefa.**

Dual Entry, Single Authority ([ADR-0307](0307-dual-entry-single-authority.md))
aplica-se também **entre módulos**, e não apenas entre a interface e o agente.

### O calendário é um módulo nativo

Não é um `iframe`, não é uma aplicação à parte, não sincroniza com o Google nem
com o Outlook. É um módulo do Ocinye OS como o Correio é: com as suas entidades,
as suas operações determinísticas, a sua autorização e o seu lugar no catálogo de
operações.

Um evento respeita os âmbitos institucionais que já existem — pessoal, unidade,
Research Workspace, instituição — e a política de classificação que já existe.
Não se inventa uma árvore de autorização temporal.

### O relógio só ganha clique quando houver sistema

O bloco de data e hora passa a ser a entrada para o Centro Temporal. Mas a ordem
importa, e é esta: domínio, Core, autorização, consultas, e **só então** o
relógio deixa de ser `<time>` e passa a controlo.

A hora que ele mostra continua a ser a do browser, e continua a não decidir nada:
carimbos de auditoria, expiração de sessões, disparo de lembretes e prazos vêm do
Core. A hora do browser é escolhida por quem o usa.

## Alternatives

**Guardar apenas UTC.** Simples, e perde a intenção. Editar «14:00 em Paris»
passaria a exigir adivinhar a zona a partir de quem está a editar, que é a pessoa
errada para decidir isso.

**Guardar apenas hora local com `offset`.** Um `offset` não representa uma zona:
`+01:00` é Paris no Inverno e é outra coisa no Verão. A recorrência partiria.

**Copiar tarefas para o calendário.** Traria uma vista única de graça, e duas
verdades para a mesma data. Uma delas ficaria desactualizada, e não há forma de
saber qual.

**Integrar um calendário externo.** Resolveria o desenho e entregaria os
compromissos da instituição a um terceiro, com a classificação que a instituição
define e o terceiro não conhece.

## O que ficou construído

A decisão acima é de Agosto de 2026. Isto é o que dela resultou, e serve de
inventário para quem voltar aqui:

| camada | o que existe |
|---|---|
| Domínio | `CalendarEvent`, `Reminder`, `Notification`, `ReminderDelivery`, com nove restrições na base |
| Core | catorze operações, uma fórmula de sobreposição, um predicado de visibilidade |
| HTTP | onze rotas, com o tecto de intervalo e a tradução dos erros temporais |
| Worker | entrega com posse atómica, sem depender de um browser aberto |
| Workspace | `/calendar` com quatro vistas, Centro Temporal, formulários, sino |
| Agentic | quatro capabilities sobre as mesmas operações |
| Provas | 29 testes de domínio, 6 de HTTP, 13 viagens de browser e 1 guarda estrutural, paridade UI↔Agente |

> **Correcção posterior, 2026-08-25.** Quando esta ADR foi aceite, a linha das
> provas dizia catorze viagens de browser e corria uma. Os testes partilhavam o
> directório de perfil do Chrome, que se recusa — com razão — a ser usado por
> dois processos; e um `.ok()?` no arranque transformava a falha num `return`
> silencioso, contado como sucesso. Treze viagens não corriam e a suite dizia-se
> verde.
>
> Corrigido na milestone de cadeia de fornecimento: perfil por harness, e um
> Chrome que não arranca passa a ser falha e nunca salto. A suite passou de
> catorze «ok» em 4 s para catorze em 47 s, e passam todas — o que estava errado
> era a **evidência** com que a cobertura foi declarada, não o Calendar.
>
> Ao medir a execução de facto apurou-se também a contagem: dos catorze testes,
> **treze levantam browser** e o décimo quarto é estrutural — lê o ficheiro do
> ecrã e verifica que nenhuma vista consulta por si. A linha acima passa a
> ler-se assim.
>
> A classe ficou fechada em `scripts/test-enumeration.sh` e em CLAUDE.md §59:
> uma suite crítica tem de provar que os testes esperados foram descobertos e
> correram, e não apenas que nada falhou.

E três coisas que só apareceram ao construir:

- **A projecção de prazos passou por verde sem medir nada, três vezes.** Uma
  tarefa herda a classificação do seu workspace, portanto nascia `RESTRICTED`, e
  a cláusula do artefacto negava o prazo ao forasteiro sozinha — a contenção
  nunca era exercitada. Só com a tarefa `INTERNAL` dentro de um workspace
  `RESTRICTED` é que a metade do contentor passou a ser medida.
- **A fronteira de autoridade estava um nível acima do efeito.** Vivia no ciclo
  de vida do plano, que é o único chamador em produção; chamar o executor
  directamente com um retrato antigo marcava o evento. Desceu para
  `executor::execute`, e o Calendário herda-a em vez de a reimplementar
  ([ADR-0411](0411-execution-time-principal-freshness.md)).
- **O servidor dependia do directório de trabalho.** `ServeDir` usava um caminho
  relativo: fora da raiz do repositório o HTML chegava e o JavaScript não, o que
  faz uma interface parecer partida sem dizer porquê. Foi o harness de browser
  que o encontrou.

## Consequences

O Ocinye OS passa a ter modelo temporal próprio, e com ele a obrigação de o
manter correcto: horário de Verão, horas locais que não existem no dia da
mudança, horas locais ambíguas no regresso. `LocalTimeProblem` deixa de ser
fundação por estrear e passa a ser o tipo que a interface tem de saber mostrar.

O `Reminder` obriga a execução fora do browser, portanto a um worker
determinista, com posse atómica para que dois workers não entreguem o mesmo
lembrete duas vezes. O padrão já existe no `outbox` — `FOR UPDATE SKIP LOCKED` —
e reutiliza-se em vez de se reinventar.

E o sino deixa de poder continuar indisponível: um lembrete que dispara sem sítio
onde aparecer é um lembrete que não foi entregue.
