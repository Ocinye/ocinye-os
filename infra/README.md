# Infraestrutura

Infraestrutura local de desenvolvimento. **Nada aqui está deployado.**

## Conteúdo

| Caminho | O quê |
|---|---|
| [`compose/`](compose/docker-compose.yml) | Stack local: PostgreSQL, Redis, MinIO |

## Levantar

```bash
docker compose -f infra/compose/docker-compose.yml up -d
```

| Serviço | Porta | Nota |
|---|---|---|
| PostgreSQL 17 + pgvector | 5442 | pgvector é exigido pela migration 0006 |
| Redis 7 | 6380 | Coordenação efémera, nunca fonte de verdade |
| MinIO | 9000 (consola 9001) | Bucket criado **privado** pelo `minio-init` |

Portas fora do habitual de propósito: colidir com outro PostgreSQL local é a
primeira coisa que acontece a quem já desenvolve noutro projecto.

## Isto é desenvolvimento

Todas as credenciais aqui são placeholders e **nunca** podem chegar a staging ou
produção. Os valores `CHANGE_ME` devem ser alterados mesmo localmente, para que
nunca se tornem hábito.

O bucket é criado com `mc anonymous set none`: privado, e verificado pela CI, que
confirma a mensagem `bucket ready and private` nos logs.

## O que falta para deployar

Nenhuma imagem de container dos serviços da Ocinye existe. Não há Dockerfile de
produção, terminação TLS, realm de produção, backups nem runbooks.

Ver [docs/deployment/](../docs/deployment/README.md), que descreve o que está
decidido e o que falta — e não descreve nada como estando a correr.
