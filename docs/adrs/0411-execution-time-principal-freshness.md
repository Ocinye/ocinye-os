# ADR-0411 — Autoridade estabelece-se na execução, não no planeamento

- **Estado:** Accepted
- **Domínio:** Security
- **Impacto:** HIGH
- **Data:** 2026-08-24
- **Relaciona-se com:** [ADR-0100](0100-authorization-model.md) ·
  [ADR-0301](0301-agentic-control-plane.md) ·
  [ADR-0307](0307-dual-entry-single-authority.md)

## Context

O Ocinye OS afirmava, em código e em comentários, que o executor agentic
reautorizava antes de agir. O teste que devia prová-lo,
`revoking_access_after_approval_stops_the_execution`, tinha esta linha:

```rust
// The principal is rebuilt, exactly as the next request would build it.
let actor = reload(&pool, actor.person_id).await;
```

Era o **teste** a recarregar, não o executor. A frase no comentário — «o
executor volta a fazer a pergunta contra o actor tal como ele está nesse
momento» — descrevia algo que o código não fazia.

Medido, o estado era este:

- `lifecycle::execute` autorizava contra o `Principal` que lhe entregassem;
- tinha **um só** chamador: a rota HTTP, que carrega o principal a cada pedido;
- o worker não executava planos, e nenhum sítio guardava um `Principal`.

Não havia, portanto, caminho vivo de exploração. Havia uma garantia inteira
assente na disciplina de quem chama, com nada no Core a impô-la. Um teste
entregando um retrato tirado antes da revogação fazia o plano correr até ao fim
e escrever a nota.

A diferença importa porque é a diferença entre uma propriedade e um hábito. Um
hábito não sobrevive ao segundo chamador.

## Decision

### As frases

> **Identity may persist. Authority must be re-established.**

> **A plan can remember what was intended. It cannot remember permission.**

> **A `Principal` is a snapshot of authority, not authority itself.**

> **Planning-time authorization controls exposure; execution-time authorization
> controls effect.**

### Identidade e autoridade separam-se

`ActorRef` guarda quem: pessoa e organização. É durável, porque a pessoa
continua a ser a mesma.

`CurrentAuthority` guarda o que essa pessoa pode **agora**. Só o resolvedor o
constrói — o campo é privado e não há outro caminho —, e por isso uma operação
sensível pode exigir que a autoridade tenha sido estabelecida, em vez de esperar
que quem chama se tenha lembrado.

Deitar fora as permissões ao construir um `ActorRef` é o objectivo, e não uma
economia: o que sobra é suficiente para voltar a perguntar, e insuficiente para
responder.

### A fronteira é uma, e é central

`crates/ocinye-core/src/authority.rs`. Não um `reload` dentro de cada mutação:
uma convenção por módulo é uma convenção que o módulo seguinte não herda, e
vinte chamadas espalhadas são vinte oportunidades de alguém escrever a vigésima
primeira sem ela.

O executor reclama o plano com a identidade de quem pede, e a partir daí o
principal recebido **não autoriza nada**. A autoridade volta a estabelecer-se à
fonte canónica, a partir do `requested_by` guardado com o plano.

### Fecha em caso de dúvida

Se a pessoa não se resolve, se a conta não está activa, ou se deixou de
pertencer à organização, o plano assenta como `Failed` e nada acontece. «Não
consegui saber» nunca se traduz em «então deixa passar».

### A confirmação continua válida, e continua a não ser autoridade

Quando a execução é recusada por autorização, a confirmação **não** é
invalidada. São factos diferentes: o consentimento dado continua a ser um
consentimento dado; o que mudou foi o que a pessoa pode causar. O registo
sobrevive, e o teste verifica-o.

### Não é um segundo motor de política

O resolvedor estabelece factos. Quem decide continua a ser
`ocinye_domain::policy`, e o domínio continua a autorizar sobre o estado actual
do recurso dentro da transacção. São duas camadas, e nenhuma substitui a outra.

## Alternatives

**Recarregar o principal em cada operação.** Espalha a mesma consulta por todo o
Core e cria tantas convenções quantos os módulos. A operação que faltasse não
daria erro nenhum.

**Uma época de autorização global.** Um contador que invalidasse planos a cada
alteração resolveria a invalidação e invalidaria também por alterações
irrelevantes. Não foi preciso: resolver na execução chega, e mede-se.

**Confiar na fronteira HTTP.** É o que já acontecia. Funciona enquanto houver
exactamente um chamador que se lembre.

## Consequences

Uma resolução por execução lógica, e não uma por operação de domínio. O custo é
uma consulta a mais no caminho do executor agentic; a rota HTTP mantém a sua,
que agora deixa de ser a única defesa.

`ExplicitAccessGrant`, papéis, pertenças e estado de conta passam todos a ser
lidos no momento da execução, porque todos entram na construção do principal —
não foi preciso enumerá-los, e é por isso que a fronteira é onde é.
