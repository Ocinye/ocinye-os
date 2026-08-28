# Matriz de operações e exposição agentic

> **Gerado a partir do catálogo tipado de operações. Não editar contagens nem entradas à mão.**

Reproduzir com:

```
cargo test -p ocinye-core --lib despeja_a_matriz -- --ignored --nocapture
```

Cada operação aparece na sua linha. Duas operações distintas nunca são agrupadas numa só para a tabela ficar mais curta: foi assim que uma contagem de treze passou por oito.

| Operação | Módulo | Exposição | Capability | Fronteira | Razão |
|---|---|---|---|---|---|
| `research::create_idea` | research | Addressable | `research.idea.create` | — | — |
| `research::update_idea` | research | Addressable | `research.idea.revise` | — | — |
| `research::transition_idea` | research | Addressable | `research.idea.transition` | — | — |
| `research::promote_idea` | research | Addressable | `research.idea.promote` | — | — |
| `research::get_idea` | research | Addressable | `research.idea.read` | — | — |
| `research::get_project` | research | Addressable | `research.project.read` | — | — |
| `research::transition_project` | research | Addressable | `research.project.transition` | — | — |
| `research::get_workspace_overview` | research | Addressable | `research.workspace.overview` | — | — |
| `knowledge::create_source` | knowledge | Addressable | `knowledge.source.create` | — | — |
| `knowledge::review_bibliography` | knowledge | Addressable | `knowledge.bibliography.review` | — | — |
| `knowledge::get_source` | knowledge | Addressable | `knowledge.source.read` | — | — |
| `knowledge::create_note` | knowledge | Addressable | `knowledge.note.create` | — | — |
| `knowledge::update_note` | knowledge | Addressable | `knowledge.note.revise` | — | — |
| `knowledge::get_note` | knowledge | Addressable | `knowledge.note.read` | — | — |
| `knowledge::get_document` | knowledge | Addressable | `knowledge.document.read` | — | — |
| `knowledge::link_objects` | knowledge | Addressable | `knowledge.link.create` | — | — |
| `knowledge::list_links` | knowledge | Addressable | `knowledge.links.list` | — | — |
| `search::search` | knowledge | Addressable | `knowledge.search` | — | — |
| `collaboration::create_task` | collaboration | Addressable | `collaboration.task.create` | — | — |
| `collaboration::assign_task` | collaboration | Addressable | `collaboration.task.assign` | — | — |
| `collaboration::transition_task` | collaboration | Addressable | `collaboration.task.transition` | — | — |
| `collaboration::list_tasks` | collaboration | Addressable | `collaboration.task.list` | — | — |
| `mail::draft` | mail | Addressable | `mail.draft` | — | — |
| `mail::draft_reply` | mail | Addressable | `mail.draft_reply` | — | — |
| `mail::draft_transform` | mail | Addressable | `mail.draft_transform` | — | — |
| `mail::evaluate_send` | mail | Addressable | `mail.evaluate_send` | — | — |
| `mail::read_message` | mail | Addressable | `mail.read` | — | — |
| `mail::search_messages` | mail | Addressable | `mail.search` | — | — |
| `mail::send_message` | mail | Addressable | `mail.send` | — | — |
| `compute::list_nodes` | compute | Addressable | `compute.node.list` | — | — |
| `organisation::create_unit` | organisation | Addressable | `organisation.unit.create` | — | — |
| `data::create_dataset` | data | Addressable | `data.dataset.create` | — | — |
| `calendar::create_event` | calendar | Addressable | `calendar.event.create` | — | — |
| `calendar::update_event` | calendar | Addressable | `calendar.event.update` | — | — |
| `calendar::cancel_event` | calendar | Addressable | `calendar.event.cancel` | — | — |
| `calendar::create_reminder` | calendar | Addressable | `calendar.reminder.create` | — | — |
| `data::add_version_file` | data | NonDelegable | — | `USER_MEDIATED_BINARY_BOUNDARY` | A execução segura exige entrada binária mediada pela pessoa através da fronteira autenticada de carregamento. Bytes de ficheiro, caminhos locais, URLs arbitrários e credenciais de armazenamento não são entradas agentic. |
| `identity::grant_role` | identity | NonDelegable | — | `AUTHORITY_BOUNDARY` | O efeito principal é mudar a fronteira de autorização ou a capacidade de outra pessoa aceder ao sistema. Uma operação assim não deve tornar-se executável só porque conteúdo recuperado não confiável pode influenciar uma proposta agentic. |
| `identity::revoke_role` | identity | NonDelegable | — | `AUTHORITY_BOUNDARY` | O efeito principal é mudar a fronteira de autorização ou a capacidade de outra pessoa aceder ao sistema. Uma operação assim não deve tornar-se executável só porque conteúdo recuperado não confiável pode influenciar uma proposta agentic. |
| `identity::set_account_status` | identity | NonDelegable | — | `AUTHORITY_BOUNDARY` | O efeito principal é mudar a fronteira de autorização ou a capacidade de outra pessoa aceder ao sistema. Uma operação assim não deve tornar-se executável só porque conteúdo recuperado não confiável pode influenciar uma proposta agentic. |
| `governance::create_grant` | governance | NonDelegable | — | `AUTHORITY_BOUNDARY` | O efeito principal é mudar a fronteira de autorização ou a capacidade de outra pessoa aceder ao sistema. Uma operação assim não deve tornar-se executável só porque conteúdo recuperado não confiável pode influenciar uma proposta agentic. |
| `governance::revoke_grant` | governance | NonDelegable | — | `AUTHORITY_BOUNDARY` | O efeito principal é mudar a fronteira de autorização ou a capacidade de outra pessoa aceder ao sistema. Uma operação assim não deve tornar-se executável só porque conteúdo recuperado não confiável pode influenciar uma proposta agentic. |
| `organisation::add_unit_member` | organisation | NonDelegable | — | `AUTHORITY_BOUNDARY` | O efeito principal é mudar a fronteira de autorização ou a capacidade de outra pessoa aceder ao sistema. Uma operação assim não deve tornar-se executável só porque conteúdo recuperado não confiável pode influenciar uma proposta agentic. |
| `identity::revoke_own_session` | identity | Addressable | `identity.session.revoke` | — | — |
| `identity::choose_preset` | identity | Addressable | `identity.avatar.choose_preset` | — | — |
| `identity::change_own_password` | identity | NonDelegable | — | `SECRET_BOUNDARY` | A execução segura exige a palavra-passe actual, e uma palavra-passe nunca pode entrar no contexto de um modelo. O agente pode abrir Definições → Segurança e explicar o que se segue. |
| `identity::reset_password` | identity | NonDelegable | — | `SECRET_BOUNDARY` | A operação produz uma credencial temporária. Delegá-la faria o material secreto passar pelo plano agentic para chegar a quem o pediu. |
| `identity::create_member` | identity | NonDelegable | — | `SECRET_BOUNDARY` | Tal como está modelada, a operação devolve a credencial de primeiro acesso. Se um dia a criação e a emissão forem operações separadas, a primeira volta a ser candidata a endereçável. |
| `identity::create_invitation` | identity | NonDelegable | — | `SECRET_BOUNDARY` | O convite produz um segredo que autentica quem o apresenta, e esse material não atravessa o plano agentic. |
| `identity::bootstrap_platform_admin` | identity | NonDelegable | — | `SECRET_BOUNDARY` | Emite a credencial inicial da instalação, e acontece quando ainda não há ninguém para autorizar seja o que for. É um acto de arranque, não uma operação institucional. |
| `identity::set_photograph` | identity | NonDelegable | — | `USER_MEDIATED_BINARY_BOUNDARY` | A execução segura exige entrada binária mediada pela pessoa através da fronteira autenticada de carregamento. Bytes de ficheiro, caminhos locais, URLs arbitrários e credenciais de armazenamento não são entradas agentic. |
| `compute::submit_job` | compute | NotImplemented | — | — | Não há trabalhos de computação no Core: o módulo regista nós e mais nada. |
| `knowledge::create_result` | knowledge | NotImplemented | — | — | A entidade Resultado ainda não existe no domínio. |

## Contagens

| | |
|---|---|
| Operações institucionais significativas | **53** |
| `Addressable` | **38** |
| `NonDelegable` | **13** |
| `NotImplemented` | **2** |
| Sem classificação | **0** |
| Capabilities no registry | **38** |

## Fronteiras de confiança

> **Non-delegability is determined by the nature of the trust boundary crossed, not by risk level alone.**

**`SECRET_BOUNDARY`** — 5 operações: `identity::change_own_password`, `identity::reset_password`, `identity::create_member`, `identity::create_invitation`, `identity::bootstrap_platform_admin`

**`AUTHORITY_BOUNDARY`** — 6 operações: `identity::grant_role`, `identity::revoke_role`, `identity::set_account_status`, `governance::create_grant`, `governance::revoke_grant`, `organisation::add_unit_member`

**`USER_MEDIATED_BINARY_BOUNDARY`** — 2 operações: `data::add_version_file`, `identity::set_photograph`

.
