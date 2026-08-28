# ADR-0700 — Continuidade institucional e portabilidade entre servidores

- **Estado:** Accepted
- **Domínio:** Operations
- **Impacto:** FOUNDATIONAL
- **Data:** 2026-08-28
- **Relaciona-se com:** [ADR-0009](0009-postgresql-sqlx.md) ·
  [ADR-0200](0200-object-storage.md) · [ADR-0201](0201-data-residency.md) ·
  [ADR-0011](0011-redis.md) ·
  [ADR-0012](0012-realtime-plane.md) · [ADR-0407](0407-mail-index-not-archive.md) ·
  [ADR-0411](0411-execution-time-principal-freshness.md) ·
  [ADR-0412](0412-scientific-lifecycle-and-provenance.md)

## Context

Um servidor pode desaparecer. Pode arder, pode ser desligado por quem o aluga,
pode ficar obsoleto, pode ser substituído por outro melhor. Nenhuma destas
coisas deve poder apagar aquilo que a Ocinye sabe.

A pergunta parece de administração de sistemas — «temos backups?» — e não é. A
pergunta é: **o que é que constitui a instituição, e o que é que é apenas o
sítio onde ela estava a correr?** Essa distinção não se descobre a olhar para o
servidor. Descobre-se a olhar para o que o domínio considera estado
autoritativo, e essa é uma decisão do Core (`CLAUDE.md` §3).

O estado anterior era pior do que «sem backups». Era **sem resposta**:

- `docs/backups/README.md` dizia, correctamente, que nada existia. Mas listava
  quatro activos numa tabela escrita à mão, e uma tabela escrita à mão envelhece
  em silêncio.
- Nada no código sabia que existe uma chave — `OCINYE_MAIL_KEY` — sem a qual
  `mailbox_credentials` chega íntegra e completamente ilegível. Um `pg_dump`
  perfeito e inútil, e isso só se descobre no dia do desastre.
- Nada dizia que o Redis deve arrancar **vazio**, nem porquê.
- Nada distinguia restaurar de recriar. Uma instalação nova com as mesmas
  migrations tem as mesmas sessenta e duas tabelas e não tem uma única coisa em
  comum com a instituição.

## Decision

**A continuidade é uma propriedade arquitectural do Ocinye Core, e não um
procedimento de quem opera a máquina.**

Concretamente, quatro decisões.

### 1. Todo o estado é classificado, e a classificação vive em código

`ocinye_core::continuity::classification` classifica cada activo do sistema numa
de sete classes, e cada entrada diz **porquê** está onde está:

| Classe | Viaja | O que significa |
|---|---|---|
| `AUTHORITATIVE` | sim | Não existe noutro sítio. Perdê-lo é perder a instituição. |
| `INTERPRETIVE` | sim | Sem ele, o autoritativo chega íntegro e ilegível. |
| `DURABLE_DERIVED` | não | Reconstruível a partir do autoritativo; perdê-lo custa tempo. |
| `REBUILDABLE` | não | Determinista a partir do código. |
| `EPHEMERAL` | não | Tem prazo. Não autoriza e não persiste. |
| `EXTERNAL` | não | Vive noutro sistema, e é de lá que volta. |
| `OPERATIONAL_CREDENTIAL` | não | Roda-se numa migração; copiá-la alarga a exposição. |

A classe `INTERPRETIVE` existe porque a `AUTHORITATIVE` não chegava. A chave de
selagem não é estado institucional — não guarda conhecimento nenhum — e sem ela
metade do estado institucional é ruído. Chamar-lhe apenas «segredo» punha-a na
mesma caixa que as credenciais de fornecedor, que **não** devem viajar.

Um teste percorre as migrations e falha se uma tabela do esquema não estiver
coberta por uma decisão.

### 2. O PostgreSQL, o Object Storage e a chave de selagem são uma só memória

Não são três backups. São três metades da mesma coisa, e qualquer uma sozinha é
inútil:

```
PostgreSQL   sem Object Storage  → referências para o nada
Object Storage sem PostgreSQL    → bytes sem significado
qualquer um sem a chave          → `mailbox_credentials` ilegível
```

Um procedimento de continuidade que salve um e esqueça outro não está
incompleto: está errado, e parece completo.

### 3. As identidades sobrevivem, e isso é verificado

> **Server migration moves institutional state; it does not recreate
> institutional history.**

Um `Result` criado em 2026 continua a ser o mesmo `Result` depois de dez
migrações de servidor. Se os identificadores mudassem, teríamos importado uma
instituição parecida em vez de mudado a nossa de sítio — e a proveniência
ficaria a apontar para o nada.

Contagens não provam isto. Cento e vinte datasets antes e cento e vinte depois
podem ser outros cento e vinte. Por isso `ocinye-core-server snapshot` recolhe
identidades, e `verify-snapshot` exige que coincidam elemento a elemento —
incluindo as somas dos objectos e as arestas de proveniência, com a origem
(`declared` ou `operation`) preservada.

A comparação é exaustiva por construção: cada tabela do esquema tem uma decisão
escrita — comparada por identidade, comparada por outro mecanismo, ou fora com
a razão. A versão anterior comparava vinte e quatro tabelas de sessenta e duas
e nada dizia sobre as outras trinta e oito, entre elas `person_roles`,
`unit_memberships` e `credentials`.

### 4. Restaurar não é criar o domínio outra vez

São operações diferentes e **têm de ser distinguíveis**:

| | O que produz |
|---|---|
| `sqlx migrate run` numa base vazia | uma instalação nova, sem instituição nenhuma |
| `pg_restore` de um dump | a instituição, com a sua história |

`verify-snapshot` recusa a primeira contra um manifesto da segunda, e diz
quantas identidades faltam por família. Foi verificado: contra uma base criada
de novo com as mesmas dezanove migrations, sai não-zero e enumera as ausências.

**Restaurar primeiro, evoluir depois.** Um restore para um esquema mais recente
confunde uma falha de transporte com uma falha de evolução, e o manifesto regista
o nível de migrations para que a diferença seja dita antes de doer.

### 5. O Redis arranca vazio, e isso é a prova de que não é fonte de verdade

O servidor novo arranca com o Redis limpo. Se alguma coisa deixar de funcionar
por causa disso, o defeito é a coisa que passou a depender dele para persistir,
e não o Redis vazio. O Redis é coordenação efémera e nunca fonte de verdade
([ADR-0011](0011-redis.md)), e o que atravessa o plano de tempo real persiste
primeiro e publica depois ([ADR-0012](0012-realtime-plane.md)) — pelo que uma
instalação nova não perde nada por ele chegar limpo.

## Alternatives

**Um script `backup.sh` e um runbook.** É o que a maioria dos projectos faz, e é
o que produz a frase `Backup completed successfully` porque o `pg_dump`
terminou com zero. Rejeitado: essa frase não é evidência de nada. O que o Core
sabe fazer, e mais ninguém, é dizer *o que tem de ir* e *se o que chegou é o
mesmo*.

**Replicação contínua para um segundo servidor.** Resolve o RPO e não resolve a
pergunta: continua sem dizer o que constitui a instituição, e replica um erro
lógico tão depressa como replica um dado bom. Fica em aberto para quando houver
um segundo servidor.

**Kubernetes, alta disponibilidade, failover automático.** Rejeitado nesta fase
(`CLAUDE.md` §18). Resolvem *uptime*; esta ADR é sobre **sobrevivência**, que é
outra coisa. Um sistema com três réplicas e sem restore testado perde tudo com
a mesma facilidade.

**Guardar tudo, incluindo pesos de modelos e mensagens de correio.** Rejeitado:
os pesos são runtime substituível — o que a instituição preserva é a
*identidade* do modelo quando uma operação científica dependeu dela — e
`mail_messages` é um índice, não um arquivo ([ADR-0407](0407-mail-index-not-archive.md)).

## Consequences

**O que passa a ser possível.** Perguntar ao sistema o que é preciso levar
(`continuity-inventory`), descrever o que esta instalação contém (`snapshot`),
e provar que o que chegou é o mesmo (`verify-snapshot`, `verify-objects`). A
resposta deixa de estar na documentação que alguém procura no dia em que o
servidor já ardeu.

**O que passa a ser obrigatório.** Uma tabela nova numa migration obriga a uma
decisão de continuidade, ou o portão fecha. É deliberado: é a única forma de a
cobertura não envelhecer sozinha.

**O que continua a não existir.** Nenhuma cópia fora do servidor. Nenhum
agendamento. Nenhuma política de retenção. O 3-2-1 **não existe** e não deve ser
declarado (`CLAUDE.md` §63). O que existe é a verificação, e um ensaio de
restore executado.

**O custo assumido.** `verify-objects` lê todos os bytes do Object Storage. Não
há amostragem, e isso torna-o caro à escala. É o preço de a resposta significar
alguma coisa: o objecto que ninguém leu é o que costuma faltar.

**O limite conhecido.** O manifesto enumera identidades, e cresce linearmente
com a instituição — cerca de oito megabytes para cento e sessenta mil recursos.
Se algum dia isso pesar, a alternativa é uma soma sobre as identidades
ordenadas, e perde-se a capacidade de dizer *qual* faltou. A troca não se faz
antes de doer.
