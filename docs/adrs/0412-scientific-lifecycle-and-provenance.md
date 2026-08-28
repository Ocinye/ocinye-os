# ADR-0412 — Ciclo de vida científico e proveniência de primeira classe

- **Estado:** Accepted
- **Domínio:** Science
- **Impacto:** HIGH
- **Data:** 2026-08-28
- **Relaciona-se com:** [ADR-0100](0100-authorization-model.md) ·
  [ADR-0101](0101-permissions-scopes-and-grants.md) ·
  [ADR-0307](0307-dual-entry-single-authority.md)

## Context

O Ocinye OS tinha ideias, projectos, bibliografia, notas, documentos e datasets.
Não tinha o que fica **entre** uma ideia e um dado: o que se quis testar, com
que método, em que corrida, e o que daí saiu.

Sem isso, a pergunta que o `CLAUDE.md` §10 põe como razão de ser do sistema de
registo — *que experiência produziu determinado resultado?* — não tinha resposta.
Havia uma tabela de relações, `research_links`, e uma pessoa podia declarar que
um documento se relacionava com um dataset. Uma declaração não é proveniência:
qualquer pessoa a podia escrever sobre quaisquer dois identificadores.

Duas coisas estavam realmente partidas, e a primeira é um achado de segurança:

**`knowledge::link_objects` aceitava qualquer `UUID` e qualquer nome de tipo.**
Não resolvia nenhuma das pontas. Uma relação podia nomear um recurso que quem a
escrevia não alcançava — e a listagem devolvia-a depois, com o tipo e o
identificador lá dentro. Um identificador nomeia âmbito; não o concede
(`CLAUDE.md` §34.2), e aqui nem sequer nomeava: bastava adivinhar.

**Metodologia não existia.** A proposta inicial era representá-la por uma
relação. Uma aresta para um texto solto não responde «que metodologia produziu
isto?» daqui a cinco anos, e uma metodologia melhorada faria a linhagem passar a
descrever outra coisa sem que ninguém a tivesse alterado.

## Decision

**Sete objectos, cada um uma entidade, com identidade e ciclo próprios:**
`Hypothesis`, `Methodology`, `MethodologyVersion`, `Study`, `StudyExecution`,
`Result`, `ResultValidation`.

**`MethodologyVersion` e `DatasetVersion` são recursos, não campos.** É o que
torna a proveniência honesta: um resultado produzido com a versão 2 continua a
dizer «versão 2» depois de a 5 existir. Uma aresta para a metodologia mudaria de
significado no dia em que alguém a melhorasse.

**`research_links` evolui; não é duplicada.** O vocabulário de relações cresce de
7 para 15 verbos, e cada verbo declara que pares de tipos aceita. Uma segunda
tabela de proveniência daria dois sítios onde procurar a mesma coisa, e dois
sítios acabam por discordar.

**A origem de uma aresta é guardada.** `origin` distingue `declared` — alguém
afirmou esta relação — de `operation` — o Core observou-a acontecer, na mesma
transacção que a produziu. É a fronteira entre o que se sugere e o que a
instituição registou:

> **AI may suggest provenance. AI may not invent institutional provenance.**

**A linhagem é uma projecção, não uma tabela.** Cada travessia lê
`research_links` e resolve cada nó pelo serviço que o detém, com a política de
quem percorre. Guardar o grafo para acelerar a interface criaria uma segunda
fonte de verdade.

**Um nó que a política recuse termina a travessia, e não aparece.** Nem
identificador, nem tipo, nem título, nem contagem. A forma do grafo é ela própria
informação: «depende de mais três coisas que não podes ver» já diz que há três.

> **Uma fronteira de autorização escondida tem de ser indistinguível de uma
> folha visível.**

Disto decorre o que `truncated` pode significar, e só isso: *entre os recursos
que esta pessoa está autorizada a observar, a consulta atingiu o limite técnico
de profundidade.* Nunca «há mais para lá desta fronteira».

**Três permissões novas**, concedidas por pertença a unidade e a ambiente, e não
por papel técnico institucional: `science.view`, `science.create`,
`results.validate`.

`results.validate` é separada de `science.create` porque são actos diferentes.
Descrever trabalho é registar o que se fez; validar é afirmar o que a instituição
sabe. Quem pode escrever um resultado não fica por isso habilitado a declarar que
ele está certo, e só a liderança de ambiente e a gestão de unidade a recebem.

**`science::record_validation` é não-delegável**, atrás de uma classe de
fronteira nova — `INSTITUTIONAL_CLAIM_BOUNDARY` (ver ADR-0307). As outras sete
operações científicas são endereçáveis, com capability tipada.

**Uma reprodução exige a execução que a reproduziu.** Reprodutibilidade é
evidência, e não um rótulo.

## Alternatives

**Representar a metodologia por uma relação.** Rejeitada: sem identidade, a
pergunta «que metodologia produziu isto?» não tem resposta estável no tempo, que
é a única altura em que alguém a faz.

**Uma tabela de proveniência separada de `research_links`.** Rejeitada: duas
tabelas para o mesmo conceito dão duas respostas à mesma pergunta, e a divergência
aparece no dia em que alguém precisa da certa.

**Materializar o grafo de linhagem.** Rejeitada nesta fase: uma cache do grafo é
uma segunda fonte de verdade científica, e o limite de profundidade já mantém a
consulta previsível. Reconsiderável quando houver medição que o justifique.

**Marcar a linhagem como truncada quando um nó é recusado.** Rejeitada: seria o
canal lateral que a fronteira existe para fechar. `truncated` fala da consulta,
nunca da pessoa.

**Três entidades para experimento, simulação e análise.** Rejeitada: partilham
tudo o que importa a esta camada, e triplicariam cada consulta de linhagem. O que
as distingue é detalhe de disciplina, e vive no campo `kind`.

**Resolver a não-delegabilidade da validação com aprovação humana.** Rejeitada:
uma confirmação deixaria a afirmação escrita como se tivesse sido *feita*, e não
*assumida*. A questão não é risco; é autoria.

## Consequences

**Ganha-se** a resposta à pergunta que sustenta a memória institucional, uma
fronteira de autorização que a topologia do grafo não contorna, e a distinção
entre o que um agente sugere e o que a instituição observou.

**Fecha-se** a fuga em `link_objects`: as duas pontas passam a ser resolvidas
com a política de quem escreve, e a matriz de compatibilidade recusa pares que
não fazem sentido.

**Paga-se** em superfície: sete tabelas, catorze caminhos HTTP, sete
capabilities, três permissões e uma classe de fronteira nova. O `research_links`
passa a ter `workspace_id` anulável, para relações que atravessam ambientes — e
`NULL` **nunca** significa «qualquer pessoa pode ver isto»: significa que a
visibilidade se decide pelas duas pontas, cada uma pela sua política.

**Fica por fazer:** não há ecrãs de criação para a cadeia — hipóteses, estudos e
execuções entram pela API ou por um agente. A leitura, a navegação e a validação
têm ecrã. A linhagem tem tecto de cinco saltos, e a interface diz quando lá
chega.

**Não é um caderno de laboratório electrónico.** Um ELN precisa de protocolos
passo a passo, inventário de reagentes, assinatura por etapa e calibração de
equipamento. Nada disso é preciso para responder à pergunta que esta camada
existe para responder, e acrescentá-lo por antecipação seria construir hoje a
complexidade de daqui a vários anos (`CLAUDE.md` §71).
