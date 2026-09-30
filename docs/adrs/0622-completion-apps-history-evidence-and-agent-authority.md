# ADR-0622 — As aplicações de conclusão: a história relê o alvo, a prova mostra-se por lista branca, e um agente não empresta a sua autoridade

- **Estado:** Accepted
- **Domínio:** Workspace
- **Impacto:** HIGH
- **Depende de:** [ADR-0100](0100-authorization-model.md) · [ADR-0306](0306-resource-resolution-as-authorization-boundary.md) · [ADR-0301](0301-agentic-control-plane.md) · [ADR-0310](0310-ai-fabric-provider-registry.md) · [ADR-0618](0618-window-manager.md) · [ADR-0619](0619-nye-universal-surface.md) · [ADR-0621](0621-research-apps-typed-relations-and-deep-links.md)
- **Data:** 2026-09-30

## Context

O Claude Design entregou na D007 os ecrãs das nove aplicações que ainda eram
`app_pending`: Mensagens, IA, Agentes, Computação, Meus Recursos, Actividade,
Auditoria, Definições e Ajuda (`ui/apps/{messages,fabric,ledger,member,ops}.rs`).
Ao ligá-las ao Core verificou-se o que ele devolvia de facto:

1. **A Actividade é uma projecção gravada.** Cada entrada guarda a
   classificação e o ambiente do momento em que foi escrita, e um resumo em
   prosa numa só língua que cita o título de então. Nada a actualiza: uma ideia
   reclassificada, ou um ambiente de onde o membro saiu, continuava a aparecer
   com o título antigo.
2. **A metadata de auditoria é o que cada escritor pôs lá.** O Core retira
   algumas chaves (`password`, `token`, `secret`, …), mas grava endereços,
   nomes, identificadores de sessão e motivos em texto livre sem limite.
3. **As instruções de um agente saíam para quem o via.** Um agente
   institucional é visível a toda a instituição; as instruções do seu autor iam
   com ele.
4. **Nada dizia onde um membro pode criar um agente.** A permissão depende do
   âmbito *e* do contexto (a unidade, o ambiente), e o `/me` só diz se ela
   existe algures.
5. **Mensagens.** Os papéis dos participantes não se liam; uma resposta citava
   o texto de uma mensagem depois retirada; dois envios paralelos com a mesma
   chave davam um erro interno; e começar uma conversa procurava no directório
   inteiro da Instância.
6. **A IA dizia «indisponível» sem motivo** por capacidade, e «fornecedores
   saudáveis» contava modelos.

## Decision

**A história relê o alvo.** A Actividade é consciência, não prova: cada evento
mostra o alvo **relido agora** com a sessão de quem vê (a mesma leitura
autorizada das ligações e da Nye). O que não se lê fica **redigido** — sem
título, sem classificação, sem ambiente, sem ligação. O resumo gravado pelo
Core nunca se mostra: o que aconteceu diz-se pelo tipo tipado do evento, na
língua do membro, e o título é o do alvo relido.

**A prova mostra-se por lista branca.** A Auditoria é só leitura (sem editar,
apagar ou exportar). O detalhe mostra a metadata **só pelas chaves de uma lista
positiva** (`AUDIT_ALLOWED_KEYS`), só com valores escalares e curtos; tudo o
resto conta-se e não se mostra, incluindo as chaves que o Core venha a escrever
amanhã. A lista não contém nada que diga *quem* (endereços, nomes,
identificadores) nem *o quê* (títulos, rótulos, códigos, motivos livres). Os
tipos do filtro vêm do próprio registo (`GET /audit/resource-types`, com a
autoridade de o ler), nunca de uma lista escrita no Workspace.

**Um agente não empresta a sua autoridade — nem as instruções do autor.** O
Core só devolve as instruções a quem criou o agente. Onde se pode criar um
agente pergunta-se ao Core (`GET /ai/agents/capabilities`), pela mesma política
da criação; o formulário oferece só isso, e a criação decide outra vez. Um
agente «pronto» abre-se na Nye pela referência tipada (`agent:<id>`), relida
com a sessão do membro. A IA é o estado do *AI Fabric* — nunca uma segunda
conversa; perguntar é na Nye. Um fornecedor mostra rótulo, tipo, residência,
estado e *se* tem credencial; nunca o endereço, a referência do segredo ou o
segredo.

**As Mensagens fecham o directório.** Os papéis na conversa vêm do Core; uma
resposta a uma mensagem retirada não a cita; envios com a mesma chave
serializam-se (tranca consultiva) e são um só; o texto de um envio recusado
volta intacto, com a mesma chave. Começar uma conversa fica indisponível e
explicado até existir um contrato de candidatos elegíveis: a procura no
directório foi retirada.

**A Computação é só leitura** (nós e capacidade reportada, sem despacho);
**Meus Recursos é o quadro do próprio**, sem parâmetro de pessoa; **as
Definições só mudam a camada do membro** (o fuso é da Instância); **a Ajuda é
de primeira parte**: um tópico por aplicação do registo e os atalhos de uma
única lista declarada (`experience::shortcuts`), que o alternador e a Nye
também lêem, e que um teste liga ao código que os trata.

## Alternatives

| Alternativa | Porque não |
|---|---|
| Mostrar o resumo do Core e só redigir o que falha a releitura | O resumo cita o título de então: uma ideia reclassificada continuava a dizer o que era. E é prosa numa só língua. |
| Refazer a projecção da Actividade no Core, por evento | É o destino certo (filtrar pela autoridade actual no próprio Core), mas pede uma releitura por tipo de alvo dentro do módulo de colaboração; fica registado. Até lá, a releitura é na fronteira, com a leitura autorizada de cada tipo. |
| Lista negra de chaves de auditoria no Workspace | Uma chave nova, escrita amanhã por outro módulo, passaria. Uma lista positiva falha fechada. |
| Esconder as instruções só no Workspace | Qualquer cliente do Core (CLI, agente) as leria. A resposta do Core é a fronteira. |
| Oferecer os âmbitos pelo `/me.capabilities` | Diz que a permissão existe algures, não em que unidade ou ambiente; o formulário ofereceria o que o Core recusa. |
| Pesquisar o directório para começar conversas | Enumera a Instância a quem só quer escrever a alguém; sem um contrato de elegibilidade, é uma fuga. |

## Consequences

- O Core ganhou, aditivamente: `role` por participante, excerto vazio para
  respostas a mensagens retiradas, tranca de idempotência no envio,
  instruções só para o autor, `GET /ai/agents/capabilities`, `reason` por
  capacidade no estado da IA (e `providers` conta fornecedores), e
  `GET /audit/resource-types`.
- Faltam no Core, sem controlo morto na interface: candidatos elegíveis para
  conversas, editar/retirar mensagens, anexos, activar/desactivar/arquivar
  agentes e o seu histórico de execução, despacho de trabalhos, outros
  recursos além do armazenamento, filtros de auditoria por acção e resultado,
  fuso por membro, e a filtragem da Actividade pela autoridade actual no
  próprio Core.
- As provas: `apps/workspace/tests/d007_journeys.rs` (16 viagens contra um Core
  real), `apps/workspace/tests/d007_contracts.rs` (guardas estáticas) e
  `crates/ocinye-core/tests/d007_contracts.rs`, cada guarda provada por
  reversão.
