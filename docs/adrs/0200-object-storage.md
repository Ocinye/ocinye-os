# ADR-0200 — Object Storage S3-compatible

- **Estado:** Accepted
- **Domínio:** Data
- **Impacto:** HIGH
- **Data:** 2026-08-22

## Context

O `CLAUDE.md` §26 exige que ficheiros grandes fiquem fora do PostgreSQL e que o
domínio não fique acoplado a nenhum fornecedor. A instituição não possui hoje
infraestrutura própria; possuí-la é um objectivo declarado.

## Decision

**Object storage compatível com S3**, acedido através do `aws-sdk-s3` apontado a
um endpoint configurável. Em desenvolvimento, **MinIO** via Docker Compose.

O domínio conhece três conceitos, todos linhas em base de dados:

- `StorageBackend` — endpoint, bucket, região, `location_label`, `residency`,
  `migration_state`;
- `StorageObject` — chave opaca, checksum SHA-256, dimensão, MIME,
  classificação, estado de scan;
- referências a partir de `Document` e `DatasetFile`.

Regras:

- **A chave do objecto é gerada pelo sistema** e opaca. O nome de ficheiro
  enviado pelo utilizador é normalizado e guardado apenas como metadado.
- **Conhecer a chave não concede nada.** Cada download é autorizado pelo Core e
  servido por URL assinada de curta duração. O bucket nunca é público.
- **O tipo de conteúdo é validado contra uma allow-list**, e o `Content-Type`
  declarado pelo cliente nunca é confiável.
- Checksum SHA-256 calculado no momento do armazenamento e persistido.

Trocar de fornecedor, ou migrar para infraestrutura Ocinye, é acrescentar um
backend e migrar objectos — não reescrever o domínio.

## Alternatives

| Alternativa | Porque foi rejeitada |
|---|---|
| **Blobs no PostgreSQL** | Proibido pelo `CLAUDE.md` §26: degrada backups, replicação e memória para ganho nenhum. |
| **Filesystem local** | Não sobrevive a mais do que um nó nem a colocation futura; obrigaria a reescrever o acesso a ficheiros na primeira migração. |
| **SDK específico de um fornecedor** | Acoplaria o domínio, contra `CLAUDE.md` §26. |
| **`rust-s3`** | Mais leve que o `aws-sdk-s3`; SDK oficial preferido por cobertura de assinatura, retries e manutenção. |

## Consequences

**Positivas** — localização física substituível; metadados e blobs separados
com fronteiras claras; residência declarada explicitamente (ADR-0201).

**Negativas, aceites** — o `aws-sdk-s3` é pesado em dependências; MinIO é mais um
serviço em desenvolvimento; URLs assinadas exigem relógios sincronizados.

## Referências

`CLAUDE.md` §26, §40 · briefing §33, §78, §79 · ADR-0201
