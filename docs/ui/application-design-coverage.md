# Application design coverage — D007 + D007.1

Estado depois da integração da D007 e da revisão correctiva D007.1, no ramo `feat/design-d007`. Origem: o registo (`ocinye_contracts::ApplicationId::ALL` = `experience/apps.rs` `APPLICATIONS`), não o protótipo.

## Realidade

- **O registo tem 27 aplicações**: as 24 de antes e as três que a D007.1 registou (Monitor de Actividade, Resultados, Lixo). O teste `o_registo_tem_27_e_so_o_terminal_esta_pendente` (`apps/workspace/tests/d007_1_journeys.rs`) enumera-as, confirma ids e rotas únicos, nomes e descrições em pt/en/fr, e abre cada uma.
- **Só o Terminal continua `app_pending`** (D008). O Browser não está registado (D008 regista-o).
- O protótipo de 30 fichas (`reference/Ocinye OS.dc.html`, `MODS`) é evidência histórica do Design, não verdade do produto: o Gestor de Aplicações deriva do registo e da autorização.
- Tarefas não é uma aplicação: `/tasks/new` e `/tasks/{id}` redireccionam para O Meu Trabalho (o mesmo domínio). Histórico não existe: o Ocinye não regista o que cada membro abre, e a Actividade não o substitui.
- Janelas: MultiWindow = Notas, Ficheiros; todas as outras SingleInstance.

## Matriz

| # | App id | Rótulo pt · en · fr | Rota | Categoria | Portão | Design | Estado | Hoje |
|---|---|---|---|---|---|---|---|---|
| 01 | home | Home · Home · Accueil | / | Productivity | — | D001/D002 | REGISTERED | Desktop |
| 02 | work | O Meu Trabalho · My work · Mon travail | /my-work | Productivity | — | D005 | REGISTERED | real screen |
| 03 | notes | Notas · Notes · Notes | /notes | Productivity | — | D004 | REGISTERED | real screen (MultiWindow) |
| 04 | calendar | Calendário · Calendar · Calendrier | /calendar | Productivity | calendar.view | D004 | REGISTERED | real screen |
| 05 | trash | Lixo · Trash · Corbeille | /trash | Productivity | — (personal) | D007.1 | REGISTERED_BY_D007.1 | real screen |
| 06 | mail | Correio · Mail · Courrier | /mail | Communication | mail.use | D004/D004.1 | REGISTERED | real screen |
| 07 | messages | Mensagens · Messages · Messages | /messages | Communication | messaging.use | D007 | REGISTERED | real screen |
| 08 | files | Ficheiros · Files · Fichiers | /files | Knowledge | documents.view + module | D004/D004.1 | REGISTERED | real screen (MultiWindow) |
| 09 | knowledge | Conhecimento · Knowledge · Connaissance | /knowledge | Knowledge | bibliography.view + module | D005 | REGISTERED | real screen |
| 10 | bibliography | Bibliografia · Bibliography · Bibliographie | /bibliography | Knowledge | bibliography.view + module | D005 | REGISTERED | real screen |
| 11 | units | Unidades · Units · Unités | /units | Research | units.view | D006 | REGISTERED | real screen |
| 12 | ideas | Ideias · Ideas · Idées | /ideas | Research | ideas.view | D005 | REGISTERED | real screen |
| 13 | projects | Projectos · Projects · Projets | /projects | Research | projects.view | D005 | REGISTERED | real screen |
| 14 | datasets | Dados · Data · Données | /datasets | Research | datasets.view + module | D005 | REGISTERED | real screen |
| 15 | results | Resultados · Results · Résultats | /results | Research | projects.view (each result by the Core) | D007.1 | REGISTERED_BY_D007.1 | real screen |
| 16 | prompt | Nye | /ai/prompt | Research | ai.use | D003/D003.1 | REGISTERED | real screen |
| 17 | ai | Ocinye AI | /ai | Research | ai.use | D007 | REGISTERED | real screen |
| 18 | agents | Agentes · Agents · Agents | /ai/agents | Research | agents.view | D007 | REGISTERED | real screen |
| 19 | compute | Computação · Compute · Calcul | /compute | Research | compute.view | D007 | REGISTERED | real screen |
| 20 | resources | Meus Recursos · My Resources · Mes ressources | /resources | Administration | — | D007 | REGISTERED | real screen |
| 21 | activity | Actividade · Activity · Activité | /activity | Administration | organisation.view | D007 | REGISTERED | real screen |
| 22 | monitor | Monitor de Actividade · Activity Monitor · Moniteur d’activité | /admin/monitor | Administration | platform.administer | D007.1 | REGISTERED_BY_D007.1 | real screen |
| 23 | administration | Administração · Administration · Administration | /admin | Administration | members.manage | D006 | REGISTERED | real screen |
| 24 | audit | Registo de auditoria · Audit log · Journal d’audit | /audit | Administration | audit.view | D007 | REGISTERED | real screen |
| 25 | settings | Definições · Settings · Paramètres | /settings | Administration | — | D007 | REGISTERED | real screen |
| 26 | help | Ajuda · Help · Aide | /help | Administration | — | D007 | REGISTERED | real screen |
| 27 | terminal | Terminal (ocsh) | /terminal | System | — | D008 | REGISTERED · RESERVED_FOR_D008 | app_pending (the only one) |
| 28 | — | Tarefas (prototype) | /tasks/* → /my-work/* | — | — | — | PROTOTYPE_ONLY · CANONICAL_ALIAS / REDIRECT | redirect to O Meu Trabalho |
| 29 | — | Histórico (prototype) | — | — | — | — | PROTOTYPE_ONLY · DOMAIN_NOT_READY | no route |
| 30 | — | Browser (prototype) | — | — | — | D008 | PROTOTYPE_ONLY · RESERVED_FOR_D008 | not registered |

Legenda: REGISTERED — no registo, com ecrã; REGISTERED_BY_D007.1 — registado nesta revisão; PROTOTYPE_ONLY — só no protótipo; RESERVED_FOR_D008 — fica para a D008; CANONICAL_ALIAS / REDIRECT — um endereço que leva à aplicação canónica; DOMAIN_NOT_READY — o domínio não existe.
