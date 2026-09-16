# 05 — Security Review (Pré-IA)

Revisão de autorização, IDOR e fronteiras, sobre `main @ 99cb02a`. A autorização é
verificada **no servidor** (camada de serviço), não pela UI.

## Autorização / controlo de acesso — **sem defeitos**

Toda a mutação de estado passa por um dos dois modelos, verificado handler a
handler:

- **RBAC + ABAC** (`authorize()` / `can()` / `require()` / o extractor
  `Authorised<Needs…>`) — organisation, research, collaboration, data, knowledge,
  files institucionais, science, calendar, mail, messaging, intelligence, compute
  (register), governance, administration. A Administração verifica **antes de o
  corpo ser lido** (o extractor tipado), pelo que um erro de validação nunca
  entrega a forma da entrada a quem não pode usar a operação.
- **Posse validada em SQL** (`owner_id`/`person_id`) para recursos privados ao
  dono, onde RBAC não se aplica — lembretes e notificações do calendário, notas e
  pastas pessoais, imagens de nota, ficheiros pessoais (cada consulta fechada
  sobre `owner_id`, reavaliada por operação), rascunhos e preferências de correio,
  mensagens (participação), avatar.

Duas excepções **por desenho**, nomeadas: `POST /compute/{enroll,heartbeat}`
(credencial de máquina, não `Principal`; o heartbeat trata o payload como não
confiável — ADR-0500) e `POST /invitations/accept`, `POST /auth/login`, MFA (o
token/credencial é a prova).

## IDOR — **sem defeitos**

Toda a leitura por identificador resolve por um portão de autoridade antes de
devolver:

- **Versões resolvem pelo recurso-pai, e é o pai que decide** — `files::get_version`
  ("a versão não tem autoridade nenhuma … é o ficheiro que decide"),
  content/excerpts de versão, versões de metodologia e execuções (via
  `readable_artefact_workspace`), versões de dataset.
- **Leituras por-id owner-scoped** — `/ai/conversations/{id}` (id de outro ⇒ 404),
  `/agentic/plans/{id}`, `/messaging/conversations/{id}`, `/me/avatar/{version}`,
  `/me/files/{version}/*` (`owns_personal_file_version`), `/me/notes/{id}`.
- **Leituras por-id com visibilidade/authorize** — science get-by-id, files
  `show_file`, research get_idea/get_project, calendar get_event,
  `/administration/members/{id}/*` (`scoped_person`; outra organização ⇒ NotFound).

Recusas são uniformemente **`NotFound`** — a existência de um artefacto alheio não
se revela (ADR-0100). O caminho inline (`/files/{id}/preview`,
`preview_personal_file`) reautoriza a cada chamada (dono ou partilha viva para
aquela versão exacta), fechando o vector «conhecer a chave do objecto».

## Fronteira de conteúdo não confiável (conversão)

Re-certificada nesta auditoria (ADR-0609, ver
[11-production-verification.md](11-production-verification.md)): o worker **não**
tem o socket do Docker nem corre parsers hostis; a conversão corre num contentor
descartável com `--network=none`, `--read-only`, `--cap-drop=ALL`,
`--security-opt no-new-privileges`, `--user 65534`, tectos de CPU/memória/PID/tempo,
destruído após. Provado no host de produção: PDF, RTF (LibreOffice) e mp4 (ffmpeg)
convertem sob o endurecimento; rede ausente; rootfs só-leitura; sem órfãos.

## Sem mock de IA em produção (§84) — **verificado**

O `FixtureProvider` (o fornecedor determinístico de teste) está atrás de
`#[cfg(feature = "test-fixtures")]`, e essa *feature* está **só** em
`[dev-dependencies]` do core-server — com o resolver 2 do Cargo, as features de
uma dev-dependency não atravessam para os alvos de produção, pelo que **um binário
de release não contém o código de todo**. Produção fixa `NoProvider`
(`core-server/src/main.rs`). O guarda `scripts/test_supply_chain.py` confirma-o com
`cargo tree` — não é o comentário que decide, é o portão. Zero-IA em produção é uma
resposta `SYSTEM`/`DEGRADED` honesta, nunca inteligência falsa.

## Soberania

O Model Router só aceita fornecedores `ocinye_node`, a menos que
`OCINYE_AI_ALLOW_EXTERNAL_PROVIDERS` esteja ligado (falso em produção). Nenhum
fornecedor externo é usado em substituição do estado sem-provider.

## Itens a fechar (não-bloqueantes)

- **F-06** (P3): `collaboration::add_comment` não confirma que `subject_id`
  pertence ao ambiente autorizado — nit de integridade, não fuga (as leituras
  ficam fechadas pela visibilidade do ambiente). Ver [findings.md](findings.md).
- Cabeçalhos HTTP, CSRF, XSS/sanitização, SSRF e limites de abuso: cobertos pelos
  testes existentes (`security_headers`, sanitização de Mail/Notas, refusão de
  injecção de cabeçalho no envio); uma revisão dedicada por fixture hostil fica
  para uma fatia de segurança própria desta auditoria.
