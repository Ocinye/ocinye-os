# ADR-0311 — Política e roteamento de IA entre vários fornecedores

- **Estado:** Accepted
- **Domínio:** AI
- **Impacto:** HIGH
- **Depende de:** [ADR-0310](0310-ai-fabric-provider-registry.md) · [ADR-0304](0304-canonical-inference-contract.md)
- **Data:** 2026-09-26

## Context

Com a [ADR-0310](0310-ai-fabric-provider-registry.md) uma Instância liga vários
fornecedores ao mesmo tempo — um Qwen local, a Anthropic, a OpenAI. O Model
Router escolhia o primeiro modelo que servisse a capacidade, e a única política
era o interruptor da instalação para externos. Faltavam três coisas: dizer quem
responde primeiro, dizer até onde os dados podem viajar, e continuar a responder
quando o preferido cai.

## Decision

**1. A política vem antes da preferência, e não se negoceia.** Um modelo só é
candidato se:

- o **tecto do modelo** (`max_classification`) cobre a classificação do pedido;
- sendo **externo**, a instalação admite externos
  (`OCINYE_AI_ALLOW_EXTERNAL_PROVIDERS`) **e** a classificação do pedido cabe no
  **tecto externo da Instância** (`instance_ai_policy.external_max_classification`,
  por omissão `INTERNAL`; `NONE` fecha a IA externa por completo).

Os dois tectos acumulam-se: ganha o mais apertado.

**2. «Tudo excluído pela política» é uma resposta tipada**, `AI_POLICY_BLOCKED`,
distinta de `AI_NO_COMPATIBLE_MODEL`. Uma diz que os dados são sensíveis demais
para o que está ligado; a outra, que nada está ligado. Nunca se degrada para um
externo para «conseguir responder».

**3. A preferência ordena o que a política deixou.** Por capacidade
(`ai_routing_preferences`): o fornecedor preferido primeiro; depois o que corre em
infraestrutura da Instância; depois o resto. O `OCINYE_AI_CAPABILITY_MAP`, quando
fixa um modelo, continua a mandar.

**4. Recurso a outro candidato, quando permitido.** Se o preferido não responde,
o Prompt tenta o seguinte — só com `allow_fallback` (por omissão, sim). Sem ele, a
falha do preferido é a resposta: `AI_PROVIDER_UNHEALTHY`. Recorrer é seguro no
Prompt porque um pedido de inferência não tem efeitos; um efeito externo nunca é
repetido por esta via ([ADR-0304](0304-canonical-inference-contract.md)).

**5. O pedido declara a classificação dos dados.** `classification` no
`POST /ai/prompt`, por omissão `INTERNAL`: o texto de um membro é conteúdo
institucional até ele dizer o contrário. Um modelo externo nasce com o tecto
`PUBLIC` (ADR-0310), pelo que, por omissão, nenhum pedido chega a um externo sem
a administração o decidir duas vezes — no modelo e na política.

**6. Gere-se por `ai.infrastructure.manage`**: `GET/PUT /api/v1/ai/policy` e
`PUT /api/v1/ai/routing/{capability}`. A preferência é preferência, nunca
permissão: não abre nada que a política feche.

## Alternatives

- **Roteamento por custo ou latência medidos.** Precisa de telemetria que ainda
  não existe e de uma política de custo por Instância. Adiado; a ordem por
  preferência é determinística e explicável.
- **Preferência por modelo, e não por fornecedor.** Mais fina, e mais frágil: um
  fornecedor muda nomes de modelo; a Instância escolhe em quem confia.
- **Classificação inferida do texto.** Um classificador que erra para baixo
  manda dados confidenciais para fora. A classificação é declarada, e o omisso é
  conservador.

## Consequences

- Uma Instância tem Claude para `GENERAL`, Qwen Coder local para `CODING` e só
  modelos locais para dados confidenciais, por configuração.
- Um pedido confidencial nunca chega a um externo — provado por HTTP com um
  externo que aceitaria tudo e cuja saúde fica `unknown`.
- A política de retrieval (RAG) continua a sua: o contexto recuperado respeita a
  ACL de quem pergunta antes de qualquer destas decisões
  ([ADR-0300](0300-ai-gateway.md)).
- Falta a superfície de administração final, que aguarda o Claude Design.

Prova: `services/core-server/tests/ai_routing_http.rs` (A–H do programa) e os
testes de `intelligence::routing`.
