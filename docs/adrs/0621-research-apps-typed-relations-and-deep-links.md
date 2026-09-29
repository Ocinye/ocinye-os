# ADR-0621 — As aplicações de investigação: relações pela linhagem, ligações profundas canónicas e transições só do Core

- **Estado:** Accepted
- **Domínio:** Workspace
- **Impacto:** HIGH
- **Depende de:** [ADR-0306](0306-resource-resolution-as-authorization-boundary.md) · [ADR-0412](0412-scientific-lifecycle-and-provenance.md) · [ADR-0618](0618-window-manager.md) · [ADR-0619](0619-nye-universal-surface.md) · [ADR-0620](0620-productivity-apps-in-managed-windows.md)
- **Data:** 2026-09-29

## Context

O Claude Design entregou na D005 os ecrãs de Projectos, O Meu Trabalho, Ideias,
Dados e Conhecimento (`ui/apps/{res,projects,work,ideas,datasets,knowledge}.rs`,
`i18n/ui_research.rs`). Os cinco mostram recursos que se ligam uns aos outros —
a ideia que originou um projecto, a fonte que uma ideia cita, a versão de um
dataset que um resultado usou — e cada ligação é um sítio por onde um título
escondido pode vazar.

Ligar os ecrãs ao Core obrigou a decidir cinco coisas.

1. **De onde vêm as relações.** O Core tem duas leituras: a lista
   `/workspaces/{id}/links` (as linhas de `research_links`) e a linhagem
   (`/lineage/{kind}/{id}`). A lista não filtra as pontas nem diz a origem.
2. **Que endereço tem cada recurso.** Um recurso aparece numa lista, numa
   relação, na Nye e na pesquisa; se cada superfície inventar o seu endereço,
   um deles escapa à reautorização.
3. **Que transições se oferecem.** Projectos, tarefas e ideias têm máquinas de
   estado no domínio; o Design desenha os botões, mas não sabe o grafo.
4. **O que falta no Core.** A lista de ambientes não dizia o estado da ideia nem
   o do projecto; não havia leitura de uma fonte nem de um documento por id.
5. **A atribuição de uma tarefa.** O Core valida que a pessoa atribuída pode ler
   o ambiente — não que pertence a ele.

## Decision

**As relações lêem-se pela linhagem, à profundidade 1, nos dois sentidos.** A
linhagem resolve cada ponta com a política de quem pergunta
(`resources::resolve`, ADR-0306) e devolve a origem (`declared` ou
`operation`). Uma relação cuja outra ponta o membro não vê **não aparece** —
nem o título, nem o tipo, nem uma contagem. «Registada pela operação» só se diz
quando o Core diz `operation`. Uma relação não dá autoridade: abrir o destino
reautoriza-o. A lista `research_links` não se usa para desenhar.

**Cada recurso tem um endereço canónico, e só um:** `/projects/{id}`,
`/my-work/{id}`, `/ideas/{id}`, `/datasets/{id}?v=<rótulo>`,
`/knowledge/sources/{id}`, `/knowledge/documents/{id}`. As listas, as relações,
a Nye e a pesquisa usam o mesmo mapa (`controllers::research::kind_and_href`).
Um tipo sem ecrã (versão de dataset isolada, nota, ficheiro) é **texto sem
elo**. Cada endereço é reautorizado ao carregar; os filtros da lista viajam na
pergunta e voltam com «Voltar». `/bibliography` é a secção de fontes do
Conhecimento; `/tasks/{id}` redirecciona para `/my-work/{id}`; O Meu Trabalho é
`ApplicationId::Work`, uma janela por aplicação.

**As transições são só as que o Core devolve.** A tarefa traz
`available_transitions`; o projecto passou a trazê-las, calculadas pelo grafo
do domínio (`workflow::project_targets_from`), e só se mostram a quem pode
transitar. Não há matriz no Workspace: o rótulo depende do par (origem,
destino), e o destino é do Core. Rejeitar e arquivar uma ideia exigem motivo,
porque o Core o exige.

**O Core ganhou leituras estreitas, e não mais do que isso.** Um resumo por
ambiente (estado da ideia e do projecto, código, responsável, unidade), só para
os ambientes que o filtro de visibilidade já autorizou; o filtro
`?idea_state=`; `started_at`, `completed_at` e `available_transitions` no
projecto; `GET /sources/{id}` e `GET /documents/{id}`. Um documento é metadata,
SHA-256 e descarga same-origin (ADR-0608) — nunca o conteúdo na página.

**A BFF recusa uma atribuição a quem não pertence ao ambiente.** Os candidatos
são as pessoas do ambiente (responsável e membros), e nunca o directório da
Instância; uma pessoa forjada no formulário é recusada antes do Core. A regra
certa é do Core, e fica pedida.

**Texto externo é dado.** O resumo, a URL e os campos de uma fonte chegam
escapados e rotulados; só `http(s)` vira elo; a Nye recebe a referência tipada
(`ref=source:<id>`) e relê-a com a sessão do membro. Tudo funciona sem IA.

## Alternatives

| Alternativa | Porque não |
|---|---|
| Desenhar a partir de `research_links` e filtrar na BFF | Uma segunda política de autorização, no cliente, e sem a origem. |
| Uma matriz de transições no Workspace | Diverge do domínio na primeira mudança, e oferece o que o Core recusa. |
| Endereço por superfície (`?item=` na lista, `/x/{id}` na Nye) | Dois caminhos para o mesmo recurso, e só um reautorizado. |
| Pedir o estado de cada ideia/projecto linha a linha | N pedidos por página; o resumo sai numa só consulta sobre ids já autorizados. |
| Confiar na validação de atribuição do Core | Aceita qualquer pessoa que leia o ambiente, incluindo quem não trabalha nele. |

## Consequences

- Uma relação a um **dataset** (não a uma versão) não aparece: o dataset não se
  resolve em `resources::resolve`. Fica pedido no Core.
- Editar projecto, tarefa, ideia e dataset, retirar uma versão, o contexto
  activo e a pesquisa no catálogo de dados não têm contrato; nenhum tem controlo
  no ecrã (`design-integration.json`, lacunas FG-D5).
- A promoção de uma ideia não é idempotente no Core — a segunda devolve `409` —,
  e por isso não duplica o projecto; a ideia fica, promovida, com a ligação.
- Os ficheiros de dataset continuam objectos guardados, sem elo para Ficheiros;
  o carregamento pela BFF lê o ficheiro inteiro (limite do corpo).
