# ADR-0305 — Conformidade de fornecedor como fronteira obrigatória

- **Estado:** Accepted
- **Domínio:** AI
- **Impacto:** HIGH
- **Data:** 2026-08-22
- **Complementa:** [ADR-0304](0304-canonical-inference-contract.md)

## Context

O [ADR-0304](0304-canonical-inference-contract.md) definiu o contrato canónico.
Faltava a pergunta operacional que se faz quando alguém escreve um adapter:

> Este adapter pode ser usado?

Sem resposta própria, a resposta por omissão é «parece que sim» — e a
arquitectura passa a depender de cada autor de adapter ter lido os ADRs e
percebido as suas implicações.

## Decision

Existe uma **Ocinye Inference Provider Conformance Suite**, e:

> **Um fornecedor não é suportado pelo Ocinye OS enquanto não a passar.**

É um requisito de engenharia, não uma preferência. Um adapter que não passou não
entra no Model Registry.

### A suite testa o contrato, não o modelo

Formas, versões, prazos, limites, canonicalização de erros, e que o adapter não
contrabandeia semântica própria através da fronteira.

**Não** testa qualidade linguística. Não precisa de GPU, rede ou base de dados.

### A suite tem duas metades, e a divisão é deliberada

`intelligence::conformance::certify` certifica um **adapter em isolamento**:
declara o que serve, honra o prazo, responde no contrato, não devolve texto do
pedido nos erros, identidade limitada, respostas dentro do limite, resposta
estruturada quando foi pedida forma, e nenhum campo reservado ao Core.

O resto — «um provider hostil não escala», «um `ResourceRef` alucinado não
resolve», «o risco não pode ser baixado», «uma aprovação não pode ser
afirmada» — **não é propriedade do provider**. É propriedade da **reacção do
Core** a um, e testá-la precisa do registry, do executor, de um principal e de
uma base de dados.

Essa metade vive em `crates/ocinye-core/tests/agentic.rs`, movida pelos mesmos
comportamentos de fixture.

A divisão é honesta: um módulo pode certificar um adapter, e **nenhum adapter
pode certificar o Core**.

### Um provider hostil é conformante, e isso é o ponto

O `FixtureProvider::hostile()` passa a suite. Não é contradição: conformidade é
sobre a **fronteira**, não sobre as intenções do modelo. Um provider que devolve
um plano cheio de capabilities inventadas honrou o contrato perfeitamente — e o
Core recusa o plano, que é onde a contenção vive.

Ler ao contrário é o erro a evitar: **passar a suite não torna um provider
confiável**. Torna-o utilizável.

### O que a suite não pode dizer

Que o modelo por trás é seguro, correcto ou alinhado. Nada aqui poderia
estabelecer isso, e a arquitectura não depende disso
([ADR-0302](0302-agent-access-intersection.md)).

## Alternatives

**Documentar as expectativas e confiar.** É o que existia. Funciona até ao
primeiro adapter escrito à pressa.

**Uma suite por fornecedor.** Duplicação, e cada cópia diverge. A suite é do
Ocinye porque o contrato é do Ocinye.

## Consequences

- Adicionar um fornecedor tem passos:
  [docs/agentic/provider-contract.md](../agentic/provider-contract.md).
- A suite corre em segundos e sem infraestrutura, pelo que não há razão para
  não a correr.
- `NoProvider` — o adapter correcto de uma instalação sem inferência — **passa**.
  Uma suite que reprovasse a ausência honesta de IA estaria a medir a coisa
  errada.
- Quando a L40S chegar, a integração é: escrever o adapter, correr a suite,
  registar no Model Registry. Nessa ordem.
