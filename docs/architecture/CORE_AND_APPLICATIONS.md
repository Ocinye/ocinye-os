# O Core e as aplicações

O que pertence ao Ocinye Core, o que pertence às aplicações, e o que acontece a
cada um quando uma aplicação é desactivada ou falha. Decisão:
[ADR-0015](../adrs/0015-core-and-applications-boundary.md); contexto:
[sistema actual](CURRENT_SYSTEM.md) e [arquitectura-alvo](TARGET_OCINYE_OS.md).

> **O Core governa. As aplicações implementam.**

## O Core

Nunca se desactiva, e nenhuma aplicação o substitui.

| Autoridade | Onde vive |
|---|---|
| Identidade, autenticação, MFA, sessões | `modules::identity`, `authn`, `password` |
| Autorização, permissões, papéis, grants | `ocinye-domain::policy`, `modules::governance` |
| Políticas e invariantes | `ocinye-domain`, `operations`, `authority` |
| Registo de aplicações e activação por Instância | `ocinye-contracts::application`, `modules::organisation::applications` |
| Governação de recursos e quotas | `modules::resource` |
| Segredos (selagem) | `password::sealed` |
| Auditoria | `audit`, `modules::governance` |
| Configuração da Instância | `modules::organisation`, `instance_identity` |
| Autoridade sobre nós | `modules::compute` (enrolamento, heartbeat) |
| Coordenação do estado persistente | `db`, `outbox`, `continuity` |

## As aplicações

Implementam domínio, e governam-se pelo Core: Notas, Ficheiros (essencial),
Correio, Mensagens, Calendário, Unidades, Ideias, Projectos, Datasets,
Conhecimento, Bibliografia, o Prompt, os Agentes, a Computação (o ecrã), e as
superfícies de administração. O registo de cada uma está em
[`docs/applications`](../applications/README.md).

Hoje partilham o binário do Core ([ADR-0006](../adrs/0006-modular-monolith.md)).
A fronteira é de autoridade e de comportamento, e não de processo.

## Desactivar uma aplicação

```text
pedido /api/v1/<caminho>
  → o caminho pertence a uma aplicação?  (application_of_api_path)
       não → segue (identidade, saúde, Instância, contentores partilhados, nós)
       sim → a aplicação está activa nesta Instância?
               sim → segue para a autorização de sempre
               não → 503 application_inactive
```

O que continua, com uma aplicação opcional desactivada — e é provado por
`services/core-server/tests/application_boundary_http.rs`:

- autenticação e `GET /api/v1/me` (que diz o que está inactivo);
- `/ready` e a saúde do Core;
- a configuração da Instância, e portanto o caminho para a reactivar;
- as outras aplicações;
- os dados da aplicação desactivada, intactos para quando voltar.

No Workspace, a aplicação sai do lançador, da barra, da paleta e do «+ Criar», e
a sua rota diz que não está activa
([ADR-0014](../adrs/0014-instance-profiles-and-application-activation.md)).

## Quando uma aplicação falha

Um `panic` num handler é um `500` com o envelope de erro, registado no log com a
causa, que nunca chega ao cliente; as outras rotas continuam a responder
(`routes::panic_boundary`). Uma dependência de uma aplicação em baixo — o
servidor de correio, o armazenamento — já era um estado tipado da própria
aplicação (`503`), e não do Core.

## O que ainda não existe

- **Isolamento de processo.** Um erro de memória ou um ciclo infinito numa
  aplicação continua a partilhar o processo do Core. O manifesto de aplicação
  (Parte 4) é onde se decide se alguma aplicação precisa de outro processo.
- **Rotas declaradas pela aplicação.** O mapa caminho → aplicação vive no Core e
  é mantido à mão, com um teste que lista também os caminhos que **não** devem
  ser recusados; o manifesto passará a declará-las.
