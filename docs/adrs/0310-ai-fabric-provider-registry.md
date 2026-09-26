# ADR-0310 — O tecido de IA: fornecedores registados pela Instância

- **Estado:** Accepted
- **Domínio:** AI
- **Impacto:** HIGH
- **Depende de:** [ADR-0304](0304-canonical-inference-contract.md) · [ADR-0110](0110-instance-secrets-authority.md) · [ADR-0013](0013-general-purpose-os-instance-and-node.md)
- **Data:** 2026-09-26

## Context

Até aqui um modelo só chegava a uma Instância reportado por um nó. O contrato
canónico de inferência ([ADR-0304](0304-canonical-inference-contract.md)) e o
Model Router existiam, provados contra um fornecedor de teste, mas em produção o
único fornecedor ligado ao processo era o `NoProvider`. Uma Instância de uso geral
precisa de mais: ligar o Ollama ou o vLLM que tem no próprio host, e — quando o
decidir — a OpenAI, a Anthropic, a Google ou a Mistral, sem recompilar nada e sem
que a credencial passe por outro sítio que não a Autoridade de Segredos
([ADR-0110](0110-instance-secrets-authority.md)).

## Decision

**1. Um fornecedor é um registo da Instância** (`ai_providers`, migração 0056):
tipo, nome, endereço, residência e uma **referência** ao segredo. Nunca a
credencial. Gere-se por `ai.infrastructure.manage`, em `/api/v1/ai/providers`
(listar, registar, activar/desactivar, remover, registar modelos), rota do Core e
não de nenhuma aplicação: desactivar o Prompt não esconde a infraestrutura.

**2. Os modelos de um fornecedor vivem no mesmo registo que os dos nós**
(`ai_models`, com `provider_id`). O Model Router escolhe entre todos por
capacidade, lendo o registo a cada pedido; «um modelo serve» e «quem o corre»
continuam separados. Desactivar um fornecedor tira-lhe os modelos do
roteamento no pedido seguinte, sem reinício.

**3. Residência explícita, e externo só por decisão.** `local` — os dados ficam
em infraestrutura que a Instância controla — ou `external`. Um modelo `external`
só é candidato com `OCINYE_AI_ALLOW_EXTERNAL_PROVIDERS=true`, e nasce com o tecto
`PUBLIC` salvo indicação contrária. Um endereço externo exige `https`; nenhum
endereço leva credenciais, parâmetros ou fragmento.

**4. Três protocolos, cinco tipos.** `openai`, `mistral` e `openai_compatible`
falam *chat completions*; `anthropic` fala a Messages API; `google` fala a
Gemini API. Os adaptadores (`intelligence::adapters`) traduzem o contrato
canónico e de volta: os três blocos — instrução do sistema, dados, instrução do
membro — ficam três mensagens, e os dados vão marcados como dados. Um erro do
fornecedor torna-se uma das razões fechadas de `InferenceError`, sem o texto
dele. Sem redirecionamentos: o endereço registado é o único destino da credencial.

**5. A credencial abre-se por pedido.** O Gateway constrói o adaptador para o
pedido que o precisa, abre o segredo no âmbito `ai_gateway` e larga-o no fim.
Um segredo revogado degrada o Prompt com `AI_PROVIDER_UNHEALTHY` — e nenhuma
chamada sai.

**6. A saúde é observação.** O resultado da última chamada fica no fornecedor
(`healthy`, `unreachable`, `refused`) para a administração ver; nunca decide o
roteamento.

## Alternatives

- **Um fornecedor por variável de ambiente.** Uma credencial num ficheiro de
  ambiente, um fornecedor por processo, e reiniciar para mudar. Rejeitada:
  contradiz a Autoridade de Segredos e o hot-plug.
- **Uma tabela de modelos por fornecedor.** Duas fontes para o Router. Rejeitada:
  a escolha por capacidade tem de ver tudo o que serve.
- **SDKs de cada fornecedor.** Cinco dependências grandes para três protocolos
  HTTP simples. Rejeitada; o `reqwest` já existia.

## Consequences

- Uma Instância liga um modelo local ou de cloud por configuração, sem fork nem
  reinício; o Prompt passa a responder por ele (`origin=MODEL`).
- O estado por omissão não muda: sem fornecedor registado, tudo continua
  `SYSTEM`/`DEGRADED`, e externo continua desligado.
- Embeddings não passam por estes adaptadores: têm contrato próprio
  ([ADR-0206](0206-embeddings-and-hybrid-retrieval.md)).
- Os fornecedores registados viajam na continuidade (`ai_providers` comparada
  por identidade); os modelos registados viajam no mesmo despejo.
- Falta, e fica para a Parte 8, a política de roteamento por custo, latência e
  classificação, e a superfície de administração final, que aguarda o Claude
  Design (`docs/ui/CLAUDE_DESIGN_HANDOFF.md`).

Prova: `services/core-server/tests/ai_providers_http.rs` (registar → responder →
desactivar → reactivar → revogar; externo nunca chamado; valor em claro ausente
da base) e os testes de `intelligence::adapters` (os três protocolos contra um
fornecedor em porta efémera).
