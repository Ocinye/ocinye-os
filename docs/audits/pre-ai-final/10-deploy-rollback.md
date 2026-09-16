# 10 — Deploy / Rollback (Pré-IA)

## Deploy

`scripts/deploy-production.sh` empacota **`origin/main`** (não o HEAD local) por
`git archive`, recusa uma árvore suja, envia o pacote com verificação de soma,
constrói as imagens no host, troca o symlink `current` só depois de as imagens
existirem, e imprime o release SHA. Verificado nesta auditoria a correr de facto
(os deploys desta sessão) e nos seus princípios.

**Reforço desta auditoria (F-02/F-03):**

- **Guarda contra *stage drift* do Dockerfile** (`scripts/compose_build_targets.py`,
  na CI e no `verify.sh`): cada serviço do Compose que constrói de um Dockerfile
  multi-stage tem de nomear o `target`. Um build sem `target` escolhe o **último**
  stage — e foi assim que um stage novo acrescentado ao fim tirou o `curl` das
  imagens de core/worker/workspace e deitou produção abaixo. A CI não constrói
  imagens; este guarda é onde o *drift* se apanha. Provado por reversão.

## Rollback

`scripts/rollback-production.sh` + [runbook](../../runbooks/rollback-production.md).
Dado um release conhecido-bom **já no host**, repõe o symlink `current` e levanta
os contentores desse release — **segundos, sem reconstruir**. Autoridade mínima:
toca só no symlink `current`, no `release.env` e num `docker compose up` sobre
releases e imagens que já existem. `--list` é só-leitura.

**Estado da prova:**

- **`--list` e a resolução do alvo** — verificados só-leitura contra o host (lista
  o `current` e os releases anteriores correctamente).
- **O flip ao vivo (com RTO medido)** — **por exercer**. O classificador de
  auto-mode bloqueia SSH que muta produção fora do deploy autorizado, e não há
  ambiente de staging. Fica para uma **autorização controlada única**, no fim da
  auditoria, exclusiva a `./scripts/rollback-production.sh` segundo o runbook — não
  uma permissão genérica de shell. É o critério 9 do [gate](gate.md).

## Segurança de deploy com migração (§38)

Um deploy que aplica migração de esquema devia tirar uma cópia da base **antes**.
Hoje o mecanismo de backup existe (e foi corrigido nesta auditoria, ver
[09](09-backup-restore.md)), mas o deploy **não** o dispara automaticamente. A
automação do pré-backup de migração fica declarada como trabalho de operação — com
a nota de que a primeira migração que a beneficiaria (a 0048, que **corrige** o
backup) é ela própria aditiva e segura, pelo que a sua aplicação não esperou por um
backup que só ela torna possível.
