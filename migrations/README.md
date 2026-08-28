# Migrations

Toda a alteração de schema é uma migration versionada. **Nunca alteração manual
em produção** (`CLAUDE.md` §58).

## Aplicar

```bash
export DATABASE_URL="postgres://ocinye:ocinye_dev_only@localhost:5442/ocinye"
sqlx migrate run --source migrations
```

O Core também as aplica ao arrancar. **Uma migration falhada impede o arranque**:
o serviço recusa correr contra um schema que não compreende.

## Ordem

| # | O quê |
|---|---|
| 0001 | Organização, pessoas, papéis, convites, unidades, auditoria, outbox |
| 0002 | Research workspaces, ideias, projectos |
| 0003 | Storage, documentos, bibliografia, notas, relações |
| 0004 | Datasets, versões, ficheiros |
| 0005 | Tarefas, comentários, actividade |
| 0006 | Índice de pesquisa, extensão pgvector |
| 0007 | Compute Registry, credenciais de nó, registo de modelos, jobs de IA |
| 0008 | Identidade: credenciais, sessões, tentativas de autenticação, grants |
| 0009 | Agentes de IA |
| 0010 | Ocinye Mail: caixas, partilhas, mensagens, rascunhos, preferências |
| 0011 | Plano agentic: planos de acção e aprovações |
| 0012 | Guarda de `TRUNCATE` na trilha de auditoria |

## Regras

- **Uma migration aplicada nunca é editada.** Corrige-se com uma nova.
- **A base impõe invariantes, não apenas a aplicação.** Ver
  [docs/data-model/](../docs/data-model/README.md) para os nove `CHECK`,
  triggers e índices que impedem estados institucionalmente impossíveis.
- **Dados não são apagados sem necessidade explícita.** Memberships são
  revogadas, comentários retirados, ideias arquivadas com motivo.
- **Migrations destrutivas exigem revisão explícita e plano de rollback.**

## Os triggers de auditoria

A migration 0001 instala triggers que rejeitam `UPDATE` e `DELETE` em
`audit_events`. A migration 0012 acrescenta o que faltava: `TRUNCATE`.

**Porque foi preciso uma segunda migration.** Os dois primeiros são
`FOR EACH ROW`. `TRUNCATE` não percorre linhas — não chama triggers de linha —
por isso executava sem objecção e esvaziava a tabela. Quem pudesse escrever na
base podia apagar a prova de o ter feito. O terceiro trigger é
`FOR EACH STATEMENT`, que é a única forma de recusar o comando.

A aplicação não consegue reescrever a sua própria história nem por engano, nem
linha a linha nem de uma vez.

Trabalho legítimo de retenção exige uma migration privilegiada que remova e
reponha os triggers — deliberadamente difícil, e visível no histórico de
migrations.

## Extensões

`pgcrypto` (0001) para `gen_random_uuid()`, e `vector` (0006) para pgvector. A
imagem `pgvector/pgvector:pg17` traz ambas.
