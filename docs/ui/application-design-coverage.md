# Application design coverage — D008

Estado depois da integração da D008 (Ocinye Terminal e Ocinye Browser), no ramo `feat/design-d008`, sobre `main @ 7a23011` (D007 + D007.1). Origem: o registo (`ocinye_contracts::ApplicationId::ALL` = `experience/apps.rs` `APPLICATIONS`), não o protótipo.

## Realidade

- **O registo tem 28 aplicações**: as 24 de antes da D007.1, as três que ela registou (Monitor de Actividade, Resultados, Lixo) e o Browser da D008. O teste `o_registo_tem_28_e_nenhuma_esta_pendente` (`apps/workspace/tests/d007_1_journeys.rs`) enumera-as, confirma ids e rotas únicos, nomes e descrições em pt/en/fr, e abre cada uma.
- **Nenhuma aplicação registada fica provisória.** O Terminal deixou o `app_pending` (D008-A); o Browser entrou com uma entrada (D008-B).
- **O Browser é real no runtime Web** (moldura isolada, só `https:`, recurso honesto). No Desktop e no Dedicado é `DESKTOP_RUNTIME_REQUIRED` (`DESKTOP_WEBVIEW_RUNTIME_VALIDATION = NOT_CERTIFIED`): não há casca nativa.
- O protótipo de 30 fichas (`reference/Ocinye OS.dc.html`, `MODS`) é evidência histórica do Design, não verdade do produto: o Gestor de Aplicações deriva do registo e da autorização.
- Tarefas não é uma aplicação: `/tasks/new` e `/tasks/{id}` redireccionam para O Meu Trabalho (o mesmo domínio). Histórico não existe: o Ocinye não regista o que cada membro abre, e a Actividade não o substitui.
- Janelas: MultiWindow = Notas, Ficheiros; todas as outras SingleInstance. O Terminal e o Browser só se desenham na sua rota (o cliente de cada um e, no Browser, a política de molduras); numa janela de fundo mostram a ligação para o endereço.

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
| 27 | terminal | Terminal (ocsh) | /terminal | System | — (each command by the Core) | D008 | REGISTERED | real screen (D008-A) |
| 28 | browser | Browser · Browser · Browser | /browser | System | — (Instance may deactivate) | D008 | REGISTERED_BY_D008 | real screen, Web runtime (Desktop: DESKTOP_RUNTIME_REQUIRED) |
| 29 | — | Tarefas (prototype) | /tasks/* → /my-work/* | — | — | — | PROTOTYPE_ONLY · CANONICAL_ALIAS / REDIRECT | redirect to O Meu Trabalho |
| 30 | — | Histórico (prototype) | — | — | — | — | PROTOTYPE_ONLY · DOMAIN_NOT_READY | no route |

Legenda: REGISTERED — no registo, com ecrã; REGISTERED_BY_D007.1 / REGISTERED_BY_D008 — registado nessa revisão; PROTOTYPE_ONLY — só no protótipo; CANONICAL_ALIAS / REDIRECT — um endereço que leva à aplicação canónica; DOMAIN_NOT_READY — o domínio não existe.
