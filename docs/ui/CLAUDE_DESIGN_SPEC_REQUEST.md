# Pedido de especificações ao Claude Design

> Os pacotes D0–D11 especificam as molduras (D8 `.ods-app`, D9 `.ods-page`, D10
> `.ods-settings`) e algumas peças. Estes ecrãs têm painéis cujo **interior** não
> está especificado. O Claude Code não os desenha (decisão do Fidel, 2026-09-27):
> aplica o que vier. Para cada painel: a estrutura HTML, as classes `ods-` e os
> estados — e mantêm-se os marcadores `data-oc` listados, que são o contrato com
> o `app.js` e as viagens de browser.

Pedido: um pacote **D12** com uma especificação por ecrã desta lista, no formato
dos anteriores (`docs/ui/D12_*.md` + CSS em `static/ods-d12-*.css` + textos em
`i18n/`), contra o ramo `ui/claude-design`.

| Ecrã (`apps/workspace/src/ui/screens/`) | Painéis (funções Rust) | Marcadores `data-oc` a manter | Classes legadas a substituir |
|---|---|---|---|
| `administration.rs` | `day`, `position_label`, `new_member`, `issued_credential`, `security_tab`, `access_tab`, `source_label`, `overview_tab`, `member_detail`, `units_admin`, `workspaces_admin`, `roles_admin`, `grants_admin`, `account_transitions` … | `instance-admin`, `instance-app`, `instance-save`, `secret`, `secret-copy`, `secret-toggle` | 41 |
| `ai.rs` | `items`, `hub`, `counter`, `new_agent`, `campo`, `fonte`, `agent_detail`, `metric` |  | 31 |
| `ask.rs` | `ask`, `command_form`, `result`, `results`, `planned`, `executed`, `unavailable` | `ask-result` | 28 |
| `calendar.rs` | `items_from`, `calendar`, `toolbar`, `period_text`, `step`, `week_start`, `month_grid_start`, `faixa_do_dia`, `dispor`, `agora_no_dia`, `eixo_das_horas`, `linhas_das_horas`, `coluna_do_dia`, `faixa_de_dia_inteiro` … | `all-day`, `allday-fields`, `editor`, `escolhidos`, `fim`, `inicio`, `linha-do-tempo`, `lista-pessoas`, `participantes`, `pessoa`, `procura-pessoa`, `scope`, `sem-pessoas`, `submeter`, `temporal-centre`, `timed-fields`, `timezone`, `timezone-label` … | 86 |
| `compute.rs` | `columns`, `items`, `bytes`, `compute`, `metric` |  | 13 |
| `files.rs` | `number`, `tamanho`, `files`, `all_files`, `vista_do_lixo`, `barra_de_seleccao`, `ficha_de_pasta`, `painel_de_pasta`, `ficha_de_ficheiro`, `tipo_legivel`, `quota_texto`, `destino_de_carregamento`, `escolher_ambiente`, `seletor_de_ambiente` … | `fs`, `fs-abrir`, `fs-carregar`, `fs-carregar-form`, `fs-grelha`, `fs-item`, `fs-lote`, `fs-lote-conta`, `fs-lote-ids`, `fs-lote-limpar`, `fs-ql-corpo`, `fs-ql-descarregar`, `fs-ql-fechar`, `fs-ql-nome`, `fs-quicklook`, `fs-sel`, `fs-thumb`, `fs-vista` … | 89 |
| `help.rs` | `seccao`, `p`, `estado`, `help` |  | 11 |
| `knowledge.rs` | `count`, `knowledge`, `counter`, `counter_not_implemented` |  | 15 |
| `lists.rs` | `sem_autorizacao`, `list_screen`, `actor_da_auditoria`, `number`, `string_list`, `items`, `footer`, `unit_selector`, `ideas_tabs`, `projects_tabs`, `pager`, `footer_paginado`, `truncated`, `unit_cell` … | `normalizado`, `revisao` | 27 |
| `mail.rs` | `mail`, `separador`, `comandos_de_disposicao`, `sync_action`, `compose_action`, `service_notice`, `unavailable_screen`, `rail`, `list`, `row`, `reading_placeholder`, `reading`, `addresses`, `flag_form` … | `alternar-pastas`, `anexo`, `anexos-lista`, `assinatura`, `assistencia`, `compositor`, `compositor-corpo`, `compositor-descartar`, `compositor-draft-id`, `compositor-editor`, `compositor-enviar`, `compositor-estado`, `compositor-expandir`, `compositor-fechar`, `compositor-ferramentas`, `compositor-ficheiro`, `compositor-html`, `compositor-pega` … | 120 |
| `messaging.rs` | `conversation_path`, `iniciais`, `inteiro`, `instante`, `hora`, `quando_curto`, `separador_do_dia`, `presenca`, `linha_da_conversa`, `messaging`, `nova_conversa`, `aviso`, `sem_conversa`, `aberta_view` … | `a-escrever`, `a-responder`, `abrir-assist`, `abrir-emoji`, `abrir-reaccoes`, `accoes-da-mensagem`, `acrescentar-membro`, `assist`, `assist-menu`, `campo-nome`, `cancelar-resposta`, `citada`, `composer`, `conversa`, `conversa-aberta`, `copiar`, `criar-conversa`, `detalhes` … | 101 |
| `my_work.rs` | `items`, `my_work` |  | 15 |
| `notes.rs` | `notes_list`, `note_card`, `shared_notes_list`, `notes_trash`, `note_editor`, `activity_panel`, `history_panel`, `revision_preview`, `shared_note_reader`, `share_panel` |  | 66 |
| `notice.rs` | `not_found`, `failure`, `access_denied`, `application_inactive`, `unavailable`, `rejected`, `conflict` | `app-inactive` | 5 |
| `prompt.rs` | `context_from`, `prompt`, `member_turn`, `ocinye_turn`, `action_chip` | `copiar-resposta`, `prompt-form`, `prompt-scroll`, `prompt-send`, `prompt-textarea`, `resposta` | 37 |
| `resources.rs` | `human_bytes`, `human_bytes_com`, `state_badge`, `source_label`, `i64_at`, `resources`, `metric` |  | 12 |
| `science.rs` | `maybe`, `items`, `scientific_chain`, `etapa`, `result_detail`, `passos`, `rotulo_do_recurso`, `validate_result`, `cabecalho`, `contexto_do_ambiente`, `recusa`, `classificacoes`, `nova_hipotese`, `accoes` … |  | 19 |
| `search.rs` | `entity_label`, `destination`, `search`, `resultados_do_corpo` |  | 19 |
| `settings.rs` | `seccoes_das_definicoes`, `facto`, `account`, `language`, `apps`, `security`, `mfa_recovery`, `imagem_de_perfil` | `recovery-codes`, `recovery-copy`, `recovery-download` | 40 |
| `workspaces.rs` | `items`, `tab_destination`, `tabs`, `idea_state_label`, `idea_transition_verb`, `idea_lifecycle_actions`, `research_workspace`, `idea_overview`, `project_overview`, `metric`, `metric_text`, `activity_list`, `task_list`, `task_state_label` … |  | 49 |

## O que já está especificado e entra sem este pedido

- D8: moldura `.ods-app`; Notas — lista (`.ods-notes__list-item`) e editor (`.ods-editor`, `.ods-editor__state`); Ficheiros — grelha, ficha, lote, Quick Look; Correio — linha (`.ods-mail__row`) e compositor (`.ods-composer`).
- D9: `.ods-page` como moldura de listas e detalhes.
- D10: `.ods-settings` com a navegação e as secções.
- D11: adaptação e modo escuro.
