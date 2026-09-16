# 02 — Feature Inventory (Pré-IA)

Estado por domínio. Vocabulário: **PROVEN** (exercitado e verificado), **PARTIAL**
(funciona; cobertura E2E ou variante por fechar), **AI_RUNTIME_BLOCKED** (só a
inferência real falta), **DEFERRED** (futuro declarado). O detalhe de rotas,
autorização e persistência está nas varreduras da superfície Workspace e do Core
API desta auditoria; o resumo abaixo é a leitura de auditoria.

| Domínio | Superfície | IA? | Autoridade | Estado |
|---|---|---|---|---|
| Autenticação / sessão | `/entrar`,`/auth/*`, MFA | não | credencial + posse | **PROVEN** (browser J1/J21) |
| Autorização / RBAC | transversal | não | RBAC+ABAC servidor | **PROVEN** ([05](05-security-review.md)) |
| Administração de membros | `/admin`, `/admin/members/*` | não | `Needs*` extractor | **PROVEN** (J2); abas «actividade/auditoria por-membro» são DEFERRED declaradas |
| Unidades | `/units*` | não | `ManageMembers`/`Read` | **PROVEN** (J2) |
| Research Workspaces | `/workspaces/{id}` | não | visibilidade + `ManageMembers` | **PROVEN** (J3) |
| Ideias | `/ideas*` | não | `Create`/`Transition` | **PROVEN** criação; promoção→projecto **PARTIAL** (J4) |
| Projectos | `/projects*` | não | `Create`/`Transition` | **PROVEN** |
| Tarefas / O Meu Trabalho | `/my-work`,`/tasks` | não | visibilidade | **PROVEN** |
| Calendário | `/calendar*` | não | visibilidade + posse | **PROVEN** (J12, ~20 viagens) |
| Mensagens | `/messages*` | não | participação | **PROVEN** (J13); `assist` é IA opcional |
| Mail | `/mail*` | não | `Mail*` + posse de caixa | **PROVEN** (J10/J11); anexo **PARTIAL** no browser; `assist` é IA opcional |
| Notas | `/notes*` | não | posse + partilha | **PROVEN** (J5) |
| Ficheiros pessoais | `/files`,`/me/files/*` | não | posse em SQL | **PROVEN** (J6/J7); conversão provada no host (11) |
| Ficheiros institucionais | `/files/*`,`/workspaces/{id}/files` | não | classificação efectiva | **PROVEN** (J8) |
| Conhecimento | `/knowledge` | não | visibilidade | **PROVEN** (J16); «Resultados»/sub-acervos são DEFERRED declarados |
| Bibliografia | `/bibliography*` | não | `Create`/visibilidade (+WASM) | **PROVEN** (J14) |
| Datasets | `/datasets*` | não | `Create`/visibilidade | **PROVEN** criação; versões **PARTIAL** no browser (J15) |
| Ciência (hipótese→resultado) | `/workspaces/{id}/science`,… | não | visibilidade; validar é não-delegável | **PROVEN** (J16) |
| Recursos / governança | `/resources` | não | próprio | **PROVEN** (Core); UI **PARTIAL** no browser (J9) |
| Pesquisa | `/search` | não | visibilidade | **PROVEN** (lexical); semântica degrada tipada |
| Actividade / Auditoria | `/activity`,`/audit` | não | visibilidade / `ReadAudit` | **PROVEN** |
| Notificações | `/notifications` | não | posse | **PROVEN** |
| Criar global | menu «Criar» | não | por-item | **PROVEN** (2 live); 5 disabled por criação-em-contexto (F-07, rótulo a rever) |
| Definições / Ajuda | `/settings*`,`/help` | não | próprio | **PROVEN** |
| Prompt Ocinye | `/ai/prompt` | **sim** | `AiUse` + admissão | **PROVEN até à fronteira**; conclusão real é **AI_RUNTIME_BLOCKED** (200 tipado degradado hoje) |
| Ask/Act (superfície universal) | `/ask` | **sim** | actor∩agente∩recurso | **PROVEN até à fronteira**; **AI_RUNTIME_BLOCKED** |
| Agentes | `/ai/agents*` | não (definição) | `AgentsView` | **PROVEN** (definição); execução é **AI_RUNTIME_BLOCKED** |
| Compute | `/compute` | não | `Read`/`Administer` | **PROVEN** (0 nós, estado verdadeiro) |
| IA hub | `/ai` | não | `Read` | **PROVEN** (indisponível, honesto) |

Nenhum domínio determinista está bloqueado por ausência de GPU. As únicas
capacidades `AI_RUNTIME_BLOCKED` exigem inferência de modelo real, e degradam de
forma tipada/honesta hoje.
