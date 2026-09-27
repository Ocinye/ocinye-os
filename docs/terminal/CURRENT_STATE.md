# Ocinye Terminal e ocsh — estado actual (M0)

> Discovery de 2026-09-27, no ramo `feat/ocsh-terminal` (a partir de
> `ui/claude-design`). Descreve o que **existe** no repositório e que o Terminal
> tem de reutilizar, e o que **não existe**. Não é uma lista de intenções.
> Contagens não se escrevem aqui: saem do catálogo tipado
> (`docs/agentic/operation-capability-matrix.md`).

## O que existe e o Terminal reutiliza

| Peça | Onde | O que dá ao ocsh |
|---|---|---|
| Catálogo de operações | `crates/ocinye-core/src/operations.rs` (`catalogue()`, `AgenticExposure`) | cada operação do Core é *Addressable*, *NonDelegable* (com fronteira e razão) ou *NotImplemented* (ADR-0307) |
| Registo de capabilities | `modules/agentic/registry.rs` (`CapabilityRegistry`, `registry()`), descritor em `ocinye-contracts/src/agentic.rs` (`CapabilityDescriptor`) | id estável, permissão, âmbito, risco (5 níveis), aprovação, reversibilidade, `supports_dry_run`, tecto de classificação, esquema de entrada |
| Executor | `modules/agentic/executor.rs::execute` | autoridade restabelecida à fonte (ADR-0411, `authority::resolve`) → registo → resolução de recursos → política → validação de esquema → aprovação → handler → auditoria. Recusas são `ExecutionStatus`, não excepções |
| Planos e aprovações | `modules/agentic/{lifecycle,repository,planner}.rs`, tabelas `action_plans`/`action_approvals` (migração 0011) | confirmação ligada à pessoa, ao digest canónico do plano e a 15 minutos; execução reclamada por `UPDATE` condicional (uso único); reverificação do risco no momento da execução |
| Superfície Universal | Workspace `GET /ask` → Core `POST /api/v1/agentic/invoke` | intenção Search/Ask/Act; **Act só com modelo** (o texto vira plano por inferência) |
| Registo de aplicações | `ocinye-contracts/src/application.rs` (`ApplicationId`, manifesto, categorias), fixações `GET/PUT /api/v1/me/apps/pins` | o Terminal regista-se aqui; `apps` lê daqui |
| Autenticação e MFA | `modules/identity/mfa.rs` (`verify_challenge`), sessão com `mfa_satisfied` | base para a elevação de sessão (G-14) |
| Auditoria | `core/audit.rs` (`AuditEntry`, `record`, `record_standalone`), tabela `audit_events` | acções com efeito auditam como na UI e no Nye |
| Tempo real | `GET /api/v1/realtime` (WebSocket, ADR-0012), canais `Conversation`/`Person` | transporte para eventos de comandos longos, se vierem a existir |
| Outbox e worker | `core/outbox.rs`, `services/worker` | trabalho assíncrono do Core |
| BFF do Workspace | `apps/workspace/src/routes.rs` (JSON same-origin, sessão no servidor, `origin_is_ours`) | o Terminal fala com o Core através do Workspace, como as Notas e o Correio |
| Design | pacote D13 em `~/Downloads/handoff-d13-terminal` (a aplicar em M9) | especificação visual, gramática ocsh proposta, códigos de saída, G-10…G-15 |

## O que não existe

| Ausência | Consequência para o Terminal |
|---|---|
| Parser, tokenizer ou gramática de comandos | o ocsh é código novo; o único leitor de texto é `Intent::detect` (primeira palavra) |
| Esquema de **saída** nas capabilities (`output: Option<Value>`) | a forma tabular de cada comando vive na definição do comando, não da capability |
| Capabilities para: ficheiros pessoais (listar, criar pasta, mudar nome, mover, lixo, restaurar), fixar aplicações, recursos, estado do sistema | têm de nascer como capabilities (e ficam também ao alcance do Nye) antes de o comando existir |
| Lista de projectos (`GET /projects`) | só `GET /projects/{id}` e `/workspaces`; `projects list` lê os ambientes |
| Rota «hoje» no calendário, contador de não lidos no correio | `/calendar/agenda?from&to` e `/mail/mailboxes` servem |
| SSE | só o WebSocket do plano de tempo real |
| Cancelamento de trabalhos | `ai_jobs` admite `cancelled` mas nada o escreve; o estado `Cancelled` dos planos nunca é posto |
| Elevação de sessão | só MFA na entrada e um passo pontual na regeneração de códigos |
| Rate limiting geral | só o do login (`Throttle`) e `CoreError::RateLimited` |
| Gestor de Janelas | o D5 está parado (G-05); `window`/`desktop` não têm onde bater |
| Estado de backup por HTTP | a continuidade é CLI do `core-server` e o timer do anfitrião |

## Execução de processos no repositório

`std::process::Command` em código de produção só existe no
`services/conversion-runner` (poppler/LibreOffice/ffmpeg e `docker run`, com
perfis e argumentos fixos, nunca texto de um pedido HTTP). Nada disto é
alcançável a partir do Terminal, e o ocsh não acrescenta nenhum.
