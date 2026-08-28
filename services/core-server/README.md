# `core-server` — Core Runtime

A superfície HTTP do Ocinye Core.

## Finalidade

Transporte, não domínio. Interpreta pedidos, resolve o principal que age, chama
um serviço em [`ocinye-core`](../../crates/ocinye-core/README.md) e devolve o
resultado.

**Nenhuma decisão de autorização é tomada aqui.** Os handlers são deliberadamente
finos, precisamente para que uma rota não possa esquecer-se de a tomar
([ADR-0006](../../docs/adrs/0006-modular-monolith.md)).

## Responsabilidades

- Router Axum, agrupado por domínio institucional.
- Extractores: identificadores de correlação e o principal autenticado.
- Envelope de erro único.
- Cabeçalhos de segurança, CORS, limite de corpo.
- Bootstrap: migrations, organização, backend de storage.

## Limites

**O que não pertence aqui:** regras de domínio, SQL, decisões de política.

## API

Versionada em `/api/v1`. A versão é explícita: uma mudança incompatível é uma
versão nova, com ADR e entrada no CHANGELOG.

| Grupo | Rotas |
|---|---|
| Saúde | `GET /health`, `GET /ready` |
| Identidade | `GET /me`, `GET /people`, `POST /invitations`, `POST /invitations/accept`, `POST\|DELETE /people/{id}/roles` |
| Organização | `GET /organisation`, `GET\|POST /units`, `GET\|DELETE /units/{id}`, membros |
| Investigação | `GET /workspaces`, `GET /workspaces/{id}`, `POST /ideas`, transições, promoção, projectos |
| Conhecimento | fontes, texto integral, notas, documentos, download, relações |
| Dados | datasets, versões, ficheiros, publicação |
| Colaboração | tarefas, comentários, actividade |
| Pesquisa | `GET /search`, `GET /search/semantic-availability` |
| Inteligência | `GET /ai/status`, `GET /ai/models`, `GET /ai/context-preview` |
| Computação | `GET /compute/status`, `GET\|POST /compute/nodes`, `POST /compute/enroll`, `POST /compute/heartbeat` |
| Correio | `GET /mail/status`, `GET /mail/mailboxes`, `GET /mail/mailboxes/{id}/messages`, `GET /mail/messages/{id}`, `POST /mail/messages/{id}/flags`, `POST /mail/send`, `POST /mail/assist`, `GET\|POST /mail/preferences` |
| Governação | `GET /audit` |

### `/ready` diz a verdade

Sonda a base de dados com uma query real. Reporta o storage, o Identity Provider
e o estado do Intelligence Plane. **A prontidão depende apenas da base de dados**:
storage e IA podem legitimamente estar ausentes, e um deployment sem eles ainda
serve a instituição.

### `POST /mail/send` é a única rota que envia

`POST /mail/assist` devolve texto e não chama `send`. Não é uma verificação — é
a ausência de uma chamada, e é o que torna
[ADR-0406](../../docs/adrs/0406-ai-generated-is-not-sent.md) verificável em vez
de prometido.

Nenhuma rota de correio consulta um papel administrativo. A pertença à caixa é
decidida em SQL, dentro do repositório
([ADR-0404](../../docs/adrs/0404-mail-privacy-boundary.md)).

### `/ai/context-preview`

Mostra exactamente que artefactos uma recuperação colocaria no contexto de um
modelo. Existe para que a fronteira de recuperação seja **inspeccionável antes de
existir qualquer modelo** que a consuma.

## Configuração

Ver [`.env.example`](../../.env.example).

O servidor recusa arrancar quando:

- os parâmetros de hashing são fracos — **em qualquer ambiente**, porque um
  verificador fraco escrito em desenvolvimento é um verificador fraco que
  sobrevive ao primeiro deployment;
- o correio está **parcialmente** configurado, ou configurado sem domínios
  institucionais;
- em produção: a origem CORS é um wildcard, ou um issuer OIDC configurado não é
  HTTPS.

Um serviço de correio inalcançável **não** impede o arranque: investigação,
conhecimento, identidade e governação nada têm que ver com email, e desligar a
instituição inteira porque um anfitrião de correio caiu seria uma avaria
auto-infligida.

## Execução

```bash
set -a && source .env && set +a
cargo run --bin ocinye-core-server
```

Os subcomandos partilham o binário, e deliberadamente: têm de carregar
exactamente a mesma configuração que o servidor carregará.

```bash
# Cria o primeiro administrador. Corre uma única vez.
ocinye-core-server bootstrap-admin --name … --username … --email …

# Liga uma caixa de correio a um membro, a partir da linha de comandos.
ocinye-core-server provision-mailbox …

# Prova as credenciais de correio sem arrancar o Core.
# Não imprime a password nem conteúdo, e não envia nada.
ocinye-core-server mail-check

# Gera a chave que sela as credenciais de caixa. 32 bytes, base64.
ocinye-core-server mail-key
```

### Continuidade

Quatro comandos respondem a «um servidor pode desaparecer; o que é preciso
levar, e como se sabe que chegou?»
([ADR-0700](../../docs/adrs/0700-institutional-continuity-and-portability.md)).

```bash
# O que tem de viajar, e porquê. Lê só a classificação, que é código.
ocinye-core-server continuity-inventory

# Descreve o que esta instalação contém. Manifesto em stdout, resumo em stderr.
ocinye-core-server snapshot > manifesto.json

# Lê a base local e compara. Sai não-zero e diz onde, quando diverge.
ocinye-core-server verify-snapshot < manifesto.json

# Lê cada objecto do bucket e recalcula a soma. Caro de propósito.
ocinye-core-server verify-objects
```

`verify-snapshot` compara o **registo** dos objectos guardados; passa num
servidor cujo bucket está vazio. `verify-objects` compara os **bytes**, e
recusa-se a concluir seja o que for quando o armazenamento não responde — não
ter conseguido observar não é a mesma coisa que os objectos faltarem.

O procedimento completo está em
[migrate-to-another-server](../../docs/runbooks/migrate-to-another-server.md).

## Segurança relevante

- Cabeçalhos em todas as respostas: `nosniff`, `DENY`, `no-referrer`, CSP
  `default-src 'none'`, `Cache-Control: no-store`, HSTS em produção.
- CORS fecha por omissão: sem origem configurada, nenhuma origem de browser é
  permitida — o Core é normalmente alcançado pelo servidor do Workspace.
- Um membro inactivo é recusado no extractor, antes de chegar a um serviço.
- Falhas internas são registadas com detalhe e devolvidas sem nenhum.
