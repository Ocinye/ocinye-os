# Storage e residência de dados

Decisões: [ADR-0200](../adrs/0200-object-storage.md) (object storage) e
[ADR-0201](../adrs/0201-data-residency.md) (residência).

## Metadata e blobs

PostgreSQL guarda metadata; object storage S3-compatible guarda os bytes.
Ficheiros grandes **nunca** entram no PostgreSQL: degradaria backups, replicação
e memória sem ganho nenhum.

O domínio não está acoplado a nenhum fornecedor. Um backend é uma linha.

## A chave é opaca

Gerada pelo sistema, sem relação com o nome de ficheiro do utilizador — que é
normalizado e guardado apenas como metadata.

**Conhecer a chave não concede nada.** O bucket é privado; cada download é
autorizado pelo Core, auditado, e servido por URL assinada de 5 minutos.

## Controlo institucional ≠ residência física

Duas coisas diferentes, e é fácil confundi-las em comunicação:

- **Controlo institucional** — a Ocinye governa, classifica e controla o acesso.
- **Residência física** — onde os bytes estão.

Cada backend declara `location_label`, `residency` e `migration_state`.

| Residência | Significado |
|---|---|
| `UNDECLARED` | **O valor por omissão.** Nada é afirmado. |
| `THIRD_PARTY_CLOUD` | Infraestrutura de terceiros |
| `OCINYE_CAMAMA` | Infraestrutura da Ocinye em Camama — só quando existir |
| `OCINYE_COLOCATION` | Equipamento da Ocinye em colocation |

`Residency::is_ocinye_owned()` é o único ponto que decide se a instituição pode
dizer que os dados residem em infraestrutura sua. **Hoje devolve `false` em todos
os backends existentes**, e nenhuma documentação ou interface afirma o contrário.

## Migrar para infraestrutura própria

1. Registar o novo backend.
2. Marcar `migration_planned`.
3. Copiar objectos com verificação de checksum.
4. Repontar as referências.
5. Marcar `stable`.

**Sem alterações no domínio.**

## Uploads

Trust boundary. Antes de qualquer byte chegar ao storage: autorização, dimensão,
tipo contra allow-list, nome normalizado contra traversal, checksum calculado e
persistido.

`scanned_at` a `NULL` significa **"não analisado"**, nunca "limpo". A análise
antimalware não está implementada, e isso está declarado.
