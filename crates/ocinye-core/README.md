# `ocinye-core`

**Ocinye Core** — o núcleo institucional do Ocinye OS.

## Finalidade

Detém o estado institucional: persiste-o, aplica-lhe a política de
[`ocinye-domain`](../ocinye-domain/README.md), regista o que aconteceu e emite
eventos de domínio.

**Não é o backend de um website.** É o núcleo sobre o qual outros runtimes
assentam — hoje o Workspace; depois CLI, notebooks, agentes e integrações.

## Forma de um módulo

Todos os módulos de domínio têm a mesma forma
([ADR-0006](../../docs/adrs/0006-modular-monolith.md)):

| Ficheiro | Responsabilidade |
|---|---|
| `mod.rs` | A API pública do módulo. A única superfície importável. |
| `model.rs` | Linhas de persistência. Privadas ao módulo. |
| `repository.rs` | SQL explícito. |
| `service.rs` | Camada de aplicação: autorização, invariantes, eventos, auditoria. |

**Toda a mudança de estado passa por um serviço.** Um repositório nunca é chamado
de fora do seu módulo, e nenhum handler HTTP chama um directamente — é isso que
impede uma rota de esquecer a autorização.

## A regra da transacção

Uma mudança de estado, o seu evento de domínio e o seu registo de auditoria
fazem commit juntos ou não fazem commit nenhum. Por isso os serviços recebem uma
transacção, não um pool: a fronteira é decidida por quem chama.

## Módulos

| Módulo | Domínio |
|---|---|
| `identity` | Pessoas, ligação ao IdP, papéis técnicos, convites. |
| `organisation` | A instituição e as suas unidades científicas. |
| `research` | Research Workspaces, ideias, projectos, promoção. |
| `knowledge` | Bibliografia, notas, documentos, relações. |
| `data` | Datasets, versões imutáveis, ficheiros. |
| `collaboration` | Tarefas, comentários, feed de actividade. |
| `governance` | Leitura do registo de auditoria. |
| `search` | Índice institucional e pesquisa permission-aware. |
| `intelligence` | AI Gateway, registo de modelos, agentes, montagem de contexto. |
| `compute` | Compute Registry, enrolamento e heartbeat de nós. |
| `mail` | Ocinye Mail: caixas, fornecedor, higienização, política de saída. |
| `platform` | Capacidades do sistema, derivadas do estado real. |

O `mail` desvia-se da forma acima, e deliberadamente: acrescenta `provider.rs`
(a abstracção de fornecedor), `imap_smtp.rs` (o adaptador), `sanitize.rs` (a
higienização do HTML recebido) e `policy.rs` (o que pode sair da instituição).

A razão é que é o único módulo cuja entrada principal vem de **fora da
instituição, sem autenticação**. Ver
[docs/mail/](../../docs/mail/README.md) e
[ADR-0400](../../docs/adrs/0400-mail-as-institutional-surface.md) a
[ADR-0407](../../docs/adrs/0407-mail-index-not-archive.md).

## Camada de plataforma

| Módulo | Responsabilidade |
|---|---|
| `config` | Configuração por ambiente. Recusa arrancar mal configurado em produção. |
| `db` | Pool e migrations. Uma migration falhada impede o arranque. |
| `authn` | Verificação de token. Vestigial sob [ADR-0103](../../docs/adrs/0103-core-owned-authentication.md); mantido para federação futura. |
| `password` | Argon2id, política, blocklist, credenciais temporárias. |
| `audit` | Registo de auditoria, escrito na transacção da acção auditada. |
| `outbox` | Outbox transaccional. |
| `visibility` | Tradução do filtro de leitura para SQL. |
| `storage` | Acesso a object storage S3-compatible. |
| `continuity` | O que constitui a instituição e o que é só o sítio onde ela corre ([ADR-0700](../../docs/adrs/0700-institutional-continuity-and-portability.md)). Classifica todo o estado, descreve esta instalação, e compara duas descrições. **Não guarda estado próprio**: lê, descreve, compara. |
| `error` | Erros e o seu mapeamento para o envelope da API. |

## Limites

**O que não pertence aqui:** HTTP, rotas, renderização. O transporte vive em
[`services/core-server`](../../services/core-server/README.md).

**O que nunca é guardado aqui:** palavras-passe em claro, e credenciais de
serviços externos.

Sob [ADR-0103](../../docs/adrs/0103-core-owned-authentication.md) o Core é a
autoridade de autenticação e guarda **verificadores** Argon2id em formato PHC —
nunca a palavra-passe. A credencial do serviço de correio não é guardada de todo:
vive apenas em `OCINYE_MAIL_PASSWORD`, e `mail_provider_settings` não tem coluna
onde a escrever ([ADR-0401](../../docs/adrs/0401-mail-provider-abstraction.md)).

## Dependências

`sqlx` (PostgreSQL, SQL explícito), `aws-sdk-s3`, `jsonwebtoken`, `reqwest`,
`tokio`. Ver [ADR-0009](../../docs/adrs/0009-postgresql-sqlx.md) para a decisão
sobre SQLx e a dívida consciente que traz.

## Configuração

Toda por ambiente, prefixo `OCINYE_`. Ver [`.env.example`](../../.env.example).
Um valor obrigatório em falta causa falha de arranque, nunca um default
silencioso.

## Execução e testes

```bash
cargo test -p ocinye-core

# Testes de autorização contra base de dados real
OCINYE_TEST_DATABASE_URL="postgres://…/ocinye_test" \
  cargo test -p ocinye-core --test authorization
```

Os testes de autorização testam **negação**: membro de outra unidade, membro
removido, referência directa a objecto, `CONFIDENTIAL`, `RESTRICTED`, fuga por
pesquisa, fuga por contagem, acesso entre organizações.

## Segurança relevante

- Autorização em `service.rs`, nunca em `api.rs` nem no cliente.
- Leitura negada devolve `not_found`, para não revelar existência.
- O predicado de autorização faz parte da query: `LIMIT`, `OFFSET` e `COUNT`
  operam apenas sobre o conjunto autorizado.
- `audit_events` é append-only, imposto por trigger na base de dados.
- Payloads de eventos e metadata de auditoria são filtrados contra chaves que
  possam transportar conteúdo.
