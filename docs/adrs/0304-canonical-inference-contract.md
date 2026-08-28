# ADR-0304 — O contrato canónico de inferência

- **Estado:** Accepted
- **Domínio:** AI
- **Impacto:** HIGH
- **Data:** 2026-08-22
- **Complementa:** [ADR-0300](0300-ai-gateway.md) · [ADR-0301](0301-agentic-control-plane.md)

## Context

O [ADR-0300](0300-ai-gateway.md) decidiu que a aplicação pede **capacidades**,
nunca modelos. O [ADR-0301](0301-agentic-control-plane.md) separou o Agent
Runtime do AI Gateway pela mesma razão.

Faltava a peça no meio: **o que é que um provider devolve?**

Enquanto essa pergunta não tivesse resposta própria do Ocinye OS, a resposta
implícita era «o que quer que o modelo devolva» — e nessa altura o Runtime
depende do formato de um fornecedor, exactamente o que os dois ADRs anteriores
proibiram.

O sintoma foi concreto: ao fechar a milestone anterior, afirmei que o E2E
agentic precisava de esperar por um modelo real, porque um fixture teria de
imitar o formato de um. Isso estava errado, e estava errado por esta lacuna.

## Decision

O Gateway define o contrato. Os adapters traduzem para ele.

### `InferenceRequest`

```
capability          GENERAL · CODING · REASONING · EMBEDDING — nunca um modelo
system              a instrução do Ocinye OS. Escrita pelo Core, só pelo Core
data: Vec<DataBlock>  material a processar. DADOS, nunca autoridade
instruction         o que o membro pediu, nas palavras dele
schema              a forma que a resposta tem de ter, ou nada para prosa
max_output_tokens
```

**Os três blocos são campos distintos**, e isso é a decisão e não um detalhe.
Um contrato que aceitasse uma só string opaca teria já misturado política de
sistema com conteúdo recuperado antes de chegar ao adapter — e a defesa contra
injecção assenta precisamente nessa separação sobreviver até ao modelo
([ADR-0405](0405-mail-prompt-injection.md), briefing §43, §79).

Um adapter é livre de renderizar os blocos como o seu modelo espera. Não é
livre de os receber já fundidos.

### `InferenceResponse`

```
text        prosa, quando não se pediu forma
value       o valor estruturado, quando se pediu
model       provider · modelo · versão — para PROVENIÊNCIA, não para routing
usage       tokens, quando o fornecedor os reporta
```

`model` existe porque *output* institucional tem de ser atribuível a um modelo
e a uma versão (`CLAUDE.md` §41). **Nada a montante ramifica sobre ele.**

### Saída estruturada faz parte do contrato

O Runtime precisa de um **plano**, não de um parágrafo. Portanto o pedido leva
um esquema e a resposta traz um valor.

Arrancar uma resposta com forma a um modelo concreto — *function calling*, modo
JSON, gramáticas, repetições — é trabalho de adapter, e fica trabalho de
adapter.

O Core valida **na mesma**, sempre. Um provider a afirmar conformidade não é
conformidade (briefing §174).

### `InferenceError` é fechado e mudo

Cinco variantes, e nenhuma carrega palavras do fornecedor. O texto de erro de um
modelo pode citar o prompt de volta, e o prompt pode conter a correspondência de
um membro.

`MalformedResponse` é distinta de `Refused` de propósito: o provider respondeu, e
o que respondeu não serve. É o caso que o Core nunca deve tapar adivinhando o
que se queria dizer (briefing §108).

### Um fixture é um provider de primeira classe

`FixtureProvider` implementa este contrato e mais nada. Não imita nenhum
formato, porque imitar um testaria o adapter em vez da arquitectura.

Com ele, o caminho inteiro corre **hoje, sem GPU**:

```
linguagem natural → Main Agent → ActionPlan → Capability → aprovação → Core → resultado
```

Quatro comportamentos: cooperativo, **hostil** (o que um modelo devolve depois
de ler «ignora as instruções anteriores»), malformado, e indisponível.

O fixture está atrás de `#[cfg(feature = "test-fixtures")]`. Um binário de
release **não contém este código** — não inalcançável, ausente. Verificado:
`strings` sobre o binário de release não encontra os seus identificadores
(briefing §164, §204).

## Alternatives

**Esperar por um modelo real para definir a resposta.** O que eu tinha proposto,
e o que este ADR corrige. Teria feito o contrato herdar a forma do primeiro
fornecedor que aparecesse.

**Um contrato genérico de «texto entra, texto sai».** Simples, e empurra o
parsing do plano para cada chamador. A saída estruturada tem de estar no
contrato ou está em todo o lado.

## Consequences

- **A L40S é um adapter.** Quando chegar, aparece uma implementação deste trait
  e nada acima dela muda: nem o Runtime, nem o planner, nem o executor, nem a
  interface. Validar isso é o passo 6 do ciclo, e é um passo de integração e
  não de arquitectura.
- Qwen, Qwen Coder e DeepSeek são configuração no Model Registry, como sempre
  foram ([ADR-0300](0300-ai-gateway.md)).
- `NoProvider` é o adapter desta instalação: recusa tudo com razão declarada.
  **Não é um mock** — é o comportamento correcto de uma instalação sem
  inferência.
