# ADR-0303 — Capabilities tipadas: registry, executor, risco e aprovação

- **Estado:** Accepted
- **Domínio:** Agentic
- **Impacto:** HIGH
- **Data:** 2026-08-22
- **Complementa:** [ADR-0301](0301-agentic-control-plane.md) · [ADR-0302](0302-agent-access-intersection.md)

## Context

A pergunta operacional desta arquitectura é: **o que é que um agente pode
fazer?** Se a resposta for «chamar ferramentas», a resposta real é «o que quer
que essas ferramentas alcancem», e o sistema não tem fronteira.

## Decision

### A capability é a unidade de acção

Tudo o que um agente pode causar existe como uma entrada num registry fechado,
definido em código. Não é uma tabela: um conjunto de capabilities editável em
tempo de execução é um conjunto que nenhum teste consegue fixar, e esta é a
camada onde um teste exaustivo vale mais.

Acrescentar uma capability é uma alteração de código que passa por revisão — que
é exactamente a cerimónia que dar um poder novo a um agente merece.

### Não existe `execute_shell`

Nem `run_command`, nem `execute_sql`, nem `arbitrary_http_request`. Existe um
teste, `no_capability_reaches_infrastructure`, que percorre o registry e falha
se algum identificador contiver `shell`, `exec`, `sql`, `http`, `file`,
`secret`, `token`, `credential` ou `env`.

Um handler é fino: tipa a sua entrada, chama o **serviço de domínio que detém a
invariante**, e devolve o resultado. Nunca escreve SQL e nunca reimplementa uma
regra — um handler que passasse ao lado do seu serviço seria uma regra que só se
aplica quando um agente pergunta.

### `SystemCapability` e `Capability` são coisas diferentes

Antes desta alteração, `Capability` significava «estado de capacidade do
sistema» — o correio está configurado, há nó de IA. O briefing usa a palavra
para «acção tipada que um agente executa».

Duas coisas com o mesmo nome, uma a responder «o correio está configurado?» e
outra «criar uma pasta». Em documentação e em mensagens de erro, «capability
unavailable» deixaria de ter significado único.

O existente passou a **`SystemCapability`**, que é o que sempre foi.

### A ordem do executor

```
resolver  →  autorizar  →  validar  →  aprovação  →  executar  →  auditar
```

**Autorizar antes de validar**, e é deliberado: um erro de validação descreve a
forma da entrada de uma capability, e devolvê-lo a quem não a pode usar entrega
o mapa de uma interface que essa pessoa não tem que ver. É o mesmo defeito que
um formulário que diz «campo em falta» antes de verificar quem pergunta.

### O risco vem do registry, nunca da proposta

Cinco níveis: `ReadOnly`, `LowImpact`, `MaterialMutation`, `ExternalEffect`,
`Privileged`.

Um modelo a quem se peça que classifique o risco do que propõe classificará uma
acção destrutiva como inofensiva — às vezes por engano, às vezes porque um
documento lho disse. **A proposta não tem campo de risco.** O planner preenche-o
a partir do descriptor.

`ExternalEffect` e `Privileged` exigem sempre confirmação humana, e uma
capability pode ser **mais** cautelosa do que o seu nível, nunca menos.

### A aprovação liga-se ao plano

Um digest cobre o que o plano *faz*: capabilities, entradas, recursos, ordem.
Não cobre a redacção do resumo — reescrever a frase mostrada não deve obrigar a
confirmar outra vez; mudar o destinatário deve.

Uma confirmação é válida para **uma pessoa, um digest e quinze minutos**. As
três. Uma confirmação não é um vale que outra pessoa possa gastar, e não é boa
para sempre.

## Consequences

- Onze capabilities, em cinco domínios. O Mail tem sete, porque é o módulo que
  exercita todas as invariantes de uma vez — pesquisa, leitura, composição,
  transformação, classificação, efeito externo e aprovação humana. O registry
  cresce **à medida que cada domínio é auditado**; converter cada endpoint
  automaticamente produziria cem portas por testar.
- `mail.send` está no registry, é `ExternalEffect` + `Always`, e o handler
  devolve `CapabilityUnavailable`: o caminho agentic **não duplica** o pipeline
  de envio que `POST /mail/send` detém, porque duas formas de enviar são dois
  sítios onde a política de classificação pode divergir.
- Adicionar uma capability: ver
  [docs/agentic/capability-authoring.md](../agentic/capability-authoring.md).
