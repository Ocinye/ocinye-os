# Findings — Pré-IA (registo vivo)

Severidade: **P0** perda de dados/segurança/outage · **P1** workflow central ou
privilégio partido · **P2** defeito funcional/UX significativo · **P3**
polimento/não-bloqueante.

Estado: **OPEN** · **FIXED** (com PR) · **DEFERRED** (adiado com razão) ·
**WONTFIX** (por desenho).

## Contagem — encontrados vs. abertos

Um defeito **encontrado** e corrigido não é um defeito **aberto**. O portão final
exige `P0 abertos = 0`, `P1 abertos = 0`, `P2 abertos = 0` (salvo deferimento
autorizado). Os P0/P2 desta tabela foram encontrados **durante** a auditoria (ou
imediatamente antes) e corrigidos.

| Sev | Encontrados | **Abertos** |
|---|---|---|
| P0 | 1 (F-02, outage por *stage drift*) | **0** |
| P1 | 1 (F-08, backup partido no esquema actual) | **0** |
| P2 | 1 (F-01, descarga institucional) | **0** |
| P3 | 5 (F-03..F-07) | 2 (F-05, F-06 — em avaliação) |

| ID | Sev | Domínio | Descrição | Estado |
|---|---|---|---|---|
| F-01 | P2 | Files | Descarga institucional redireccionava para o host interno do armazenamento (`object-store:9000`), inalcançável — descarga partida | **FIXED** (#107, ADR-0608) |
| F-02 | P0 | Deploy | Um stage novo no Dockerfile mudou o alvo *default*; core/worker/workspace saíram do stage errado → workspace em baixo (502) | **FIXED** (#109) + guardado (#111) |
| F-03 | P3 | Ops | Sem rollback rápido: um deploy mau exigia corrigir-para-a-frente (~30 min com produção em baixo) | **FIXED** (#111, `rollback-production.sh` + runbook) |
| F-04 | ~~P2~~ **P3** | IA | `/mail/assist` e `/messaging/assist` devolvem **503** sem provider, enquanto `/ai/prompt` e `/agentic/invoke` devolvem **200 tipado** (ADR-0308). Superfícies distintas (helper inline de texto vs. conversa Prompt); a UI degrada com elegância (mostra «não conseguiu ajudar», sem erro cru); permissão verificada primeiro; sem fallback externo. Não é fuga nem torna o Core não-saudável | **WONTFIX** (por desenho) |
| F-05 | P3 | Observab. | `api.ocinye.com/api/v1/health` → 404; o health vive em `/health` (interno, usado pelo healthcheck do contentor), sem sonda pública equivalente | **OPEN** (a avaliar) |
| F-06 | P3 | Data | `collaboration::add_comment` autoriza `Create` no ambiente mas não verifica que `subject_id` pertence a esse ambiente — nit de integridade, **não** é fuga de acesso (leituras ficam seguras pela visibilidade do ambiente) | **OPEN** (a avaliar) |
| F-07 | P3 | UX | «Criar» global (e a hero/acesso-rápido do Home) mostravam Projecto/Dataset/Referência/Tarefa como «Ainda não disponível» — mas são criáveis em contexto. Rótulo falso para feature implementada | **FIXED** (#113: rotam ao contexto; a Tarefa diz onde se cria) |
| F-08 | **P1** | Data/Continuidade | O `snapshot` — espinha do `institutional-backup` e do `verify-snapshot` — falhava no esquema actual (`column "id" does not exist`): `file_favourites` viaja mas nasceu sem `id`. **Produção não conseguia produzir uma cópia verificável.** Encontrado ao **provar** o backup (directiva 7) | **FIXED** (#115: migração 0048 + guarda `tests/continuity.rs`; ciclo backup→restauro provado) |

## Notas de disposição

- **F-04** foi reavaliado e é **por desenho**: o `assist` de Mail/Messaging é um
  helper inline que devolve texto, uma superfície distinta da conversa do Prompt
  (que usa o envelope tipado do ADR-0308). Sem provider, devolve 503 com mensagem
  institucional, a UI degrada com elegância, a permissão é verificada primeiro, e
  não há fallback externo — honesto e não-bloqueante. Alinhá-lo ao envelope do
  Prompt seria sobre-engenharia (§71) sem benefício para quem usa. O contrato
  tipado central (`/ai/prompt`, `/agentic/invoke`) está correcto.
- **F-07** está **corrigido** (#113): as acções implementadas do «Criar» e do Home
  passaram a levar ao ecrã onde se cria, ou a dizer onde (a Tarefa); só a falta de
  autorização desactiva de facto. `not_yet_available()` fica para features
  genuinamente futuras (adicionar nó de compute), que são declaradas-futuro.
- **F-08** foi o achado da **prova de backup** (§63/directiva 7): tentar provar
  revelou que não funcionava. Corrigido, com um guarda que corre o manifesto de
  facto — a lacuna era que o portão via a *decisão* mas não a *execução*. Ver
  [09-backup-restore.md](09-backup-restore.md).
- **F-05/F-06** ficam P3 em avaliação — nenhum bloqueia a operação determinista.
  Nem os P3 abertos, nem os fixed, deixam `P0/P1/P2 abertos > 0`.

## O que a auditoria **não** encontrou (evidência positiva)

- **Sem broken access control:** toda a mutação de estado passa por
  `authorize()`/`can()`/`require()` ou posse validada em SQL (`owner_id`). Duas
  excepções por desenho: `/compute/{enroll,heartbeat}` (credencial de máquina,
  ADR-0500) e `/invitations/accept`,`/auth/login` (o token/credencial é a prova).
- **Sem IDOR:** toda a leitura por-id resolve por um portão de autoridade antes de
  devolver; versões resolvem pelo recurso-pai; recusas são `NotFound` (não revelam
  existência).
- **Sem rota-página inalcançável** e **sem entrada de navegação/Criar para rota
  inexistente**.
- **Sem dados de demonstração fixos** no caminho de render; onde o Core não tem
  endpoint, a UI mostra um estado «ainda não» declarado, não dados falsos.
