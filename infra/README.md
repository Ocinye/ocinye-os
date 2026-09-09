# Infraestrutura

Duas coisas vivem aqui: a stack **local de desenvolvimento** (a secção
[Levantar](#levantar)), e os artefactos que descrevem a **produção a correr**. A
produção é servida a partir destes ficheiros — ver
[docs/deployment/](../docs/deployment/README.md).

## Conteúdo

| Caminho | O quê |
|---|---|
| [`compose/docker-compose.yml`](compose/docker-compose.yml) | Stack **local**: PostgreSQL, Redis, MinIO |
| [`compose/docker-compose.production.yml`](compose/docker-compose.production.yml) | Topologia de **produção**: proxy, workspace, core, worker, postgres, redis, object-store |
| [`docker/Dockerfile`](docker/Dockerfile) | Imagem dos serviços, construída no host e etiquetada pelo SHA do release |
| [`systemd/ocinye.service`](systemd/ocinye.service) | Arranque ordenado da stack de produção após reboot |
| [`nginx/`](nginx/) | Configuração do proxy (o único processo com porta pública) |
| [`scheduling/`](scheduling/) | Unidades de agendamento de backup (**não instaladas** em nenhum servidor) |

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

## Produção

Produção corre a partir de `compose/docker-compose.production.yml`, governada por
`systemd/ocinye.service`, com as imagens construídas no host pelo
[`scripts/deploy-production.sh`](../scripts/deploy-production.sh) a partir de um
SHA exacto de `origin/main`. Os segredos vivem em `/etc/ocinye/*.env`, fora do Git.

Ver [docs/deployment/](../docs/deployment/README.md) para a arquitectura, o
mecanismo de release e o estado de cada componente de operação (o que está a
correr e o que ainda falta — backups off-host agendados, métricas, runbook de
rollback).
