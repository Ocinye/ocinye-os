# ADR-0300 — AI Gateway orientado a capacidades

- **Estado:** Accepted
- **Domínio:** AI
- **Impacto:** HIGH
- **Data:** 2026-08-22

## Context

A IA é uma capacidade transversal da Ocinye (`CLAUDE.md` §8), mas nenhum nó de IA
existe. O sistema tem de ser AI-native sem ser AI-dependent, e a ausência de IA
tem de ser um estado legítimo e visível, não uma falha disfarçada.

## Decision

Todo o acesso a IA passa pelo **Ocinye AI Gateway**.

### A aplicação pede capacidades, nunca modelos

`GENERAL`, `CODING`, `REASONING`, `EMBEDDING`. O mapeamento
capacidade → modelo é **configuração**, nunca código:

```
OCINYE_AI_CAPABILITY_MAP="GENERAL=qwen2.5,CODING=qwen2.5-coder,REASONING=deepseek-r1"
```

Vazio por omissão. Nenhum nome de modelo aparece no código.

### Estado actual: indisponível

Sem nó Ocinye enrolado, o gateway devolve `capability_unavailable` e o Workspace
mostra que nenhum nó de IA da Ocinye está disponível. **Este é o comportamento
correcto, não uma avaria.** A indisponibilidade nunca quebra a plataforma:
as funcionalidades degradam de forma explícita e informada.

### Nenhum fornecedor externo automático

O Core **não** contacta OpenAI, Anthropic, Google ou qualquer outro fornecedor.
O tipo `ProviderKind::External` existe no modelo para representar uma decisão
institucional futura, mas nunca é seleccionado implicitamente: exige
`OCINYE_AI_ALLOW_EXTERNAL_PROVIDERS`, registo explícito do provider e um ADR
próprio que analise confidencialidade e residência de dados.

### RAG é permission-aware por construção

O context assembly aplica identidade, membership, classificação e fronteiras de
unidade/workspace **antes** da recuperação. Filtrar a resposta depois da geração
não corrige um contexto indevidamente montado. Índices e embeddings herdam a
classificação da fonte, e material `RESTRICTED` nunca entra num índice
institucional sem autorização separada.

### Prompt injection

Conteúdo recuperado é **dados**, potencialmente hostis — nunca instrução. As
quatro camadas são estruturalmente distintas: system policy, application policy,
user input, retrieved content. Conteúdo recuperado não pode alterar permissões,
escalar privilégios nem desencadear acções com efeitos.

### Rastreabilidade

Cada `AiJob` regista capacidade, modelo, versão, âmbito, momento e requerente,
mais as **referências** dos artefactos recuperados. Prompts e respostas não são
persistidos pelo gateway.

## Alternatives

| Alternativa | Porque foi rejeitada |
|---|---|
| **Chamar modelos directamente dos módulos** | Espalharia nomes de modelos e política de retrieval pelo código, tornando impossível trocar de nó por configuração. |
| **Usar um fornecedor externo entretanto** | Explicitamente proibido (`CLAUDE.md` §41): mascararia a ausência do nó local e enviaria dados institucionais para fora sem decisão institucional. |
| **Esconder a secção de IA até existir nó** | Contraria a verdade operacional: a arquitectura existe e o estado real deve ser visível. |
| **Biblioteca de abstracção de LLMs de terceiros** | Traria acoplamento a nomes de modelos e a suposições de fornecedores que este ADR existe para evitar. |

## Consequences

**Positivas** — o sistema funciona plenamente com zero modelos; ligar CAM-01 é
configuração e enrolamento, não uma reescrita; a fronteira de segurança do RAG
está definida antes de existir RAG.

**Negativas, aceites** — a camada de indirecção existe antes de ter um único
consumidor real, o que é deliberado: é a diferença entre acrescentar um nó e
reescrever a aplicação.

## Referências

`CLAUDE.md` §8, §41, §42, §43 · briefing §49–§53 · ADR-0500
