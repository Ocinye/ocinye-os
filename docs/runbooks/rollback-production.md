# Runbook — Reverter produção para um release anterior

**Quando:** um deploy deixou produção degradada ou em baixo (a workspace não
arranca, o Core não fica saudável, um serviço falha depois do swap) e é preciso
repor o estado anterior **depressa**, em vez de corrigir-para-a-frente (que
reconstrói tudo e custa dezenas de minutos).
**O que faz:** troca o symlink `current` para um release anterior — cujas imagens
**já estão** no servidor — e levanta os contentores. Segundos, não minutos.
**Quem:** quem tem a chave de deploy (`~/.ssh/id_ed25519_fm65`).
**Duração:** o RTO observado é da ordem de segundos (não há reconstrução).

## Antes

- [ ] Confirmar que é mesmo um problema do release novo, e não uma dependência
      externa (Postgres, Redis, object-store) — esses não se resolvem revertendo.
- [ ] Saber para onde reverter. Por omissão é o **release anterior**; um release
      específico dá-se pelo seu SHA.

## Passos

Ver os releases que existem no servidor (só leitura):

```bash
scripts/rollback-production.sh --list
```

Reverter para o release anterior:

```bash
scripts/rollback-production.sh
```

Ou para um release específico (SHA curto ou longo):

```bash
scripts/rollback-production.sh 99cb02a99a58
```

O script:

1. resolve o release alvo e **recusa** se o seu compose ou as suas imagens não
   estiverem no servidor (não reconstrói nada);
2. troca o symlink `current` e o `OCINYE_RELEASE_SHA` em `/etc/ocinye/release.env`;
3. levanta os contentores desse release (`docker compose up -d --remove-orphans`,
   com o compose **do próprio release** — um release antigo pode ter menos
   serviços);
4. espera pela saúde do Core, com prazo, e imprime o **RTO observado**.

## Autoridade

O script toca em três coisas e mais nada: o symlink `current`, o `release.env`, e
`docker compose up` sobre um release e imagens que já existem. Não reconstrói, não
corre `git`, não abre uma consola remota. É a operação estreita de reverter.

## Depois

- [ ] Confirmar a saúde de fora:

```bash
curl -s -o /dev/null -w "%{http_code}\n" https://os.ocinye.com/    # espera 303
```

- [ ] Corrigir a causa do deploy falhado **numa branch**, pelo pipeline normal
      (`branch → PR → CI → merge → deploy`). A reversão é para restaurar serviço,
      não para ficar; produção deve voltar a correr `origin/main` assim que a
      correcção estiver mergeada.
- [ ] Registar o incidente: que release falhou, porquê, e o RTO.

## Nota sobre a próxima migração

> **Corrigido em 2026-09-26** (linha de base da generalização). Esta nota dizia
> que reverter o código era seguro com migrações aditivas. **O código diz o
> contrário**, e quem o seguisse punha o Core a reiniciar em ciclo.

Um release anterior tem um esquema anterior, e as migrações da Ocinye são só de
avanço (não há *down*). O Core aplica as migrações no arranque com
`sqlx::migrate!`, que por omissão **recusa** arrancar contra uma base onde está
aplicada uma migração que ele não conhece (`VersionMissing`); o repositório nunca
activa `ignore_missing`. Por isso:

- **Se o release falhado não trouxe migrações novas**, a reversão por symlink
  deste runbook é segura.
- **Se trouxe uma migração nova — aditiva ou não —, a reversão por symlink
  falha**: o Core anterior não arranca. O caminho é o restauro da base a partir
  do conjunto de continuidade anterior ao deploy
  ([migrar para outro servidor](migrate-to-another-server.md)), ou avançar com
  uma correcção.

Antes de reverter, compare o `migrations/` dos dois releases:

```bash
diff <(ls /srv/ocinye/releases/<anterior>/migrations) <(ls /srv/ocinye/releases/<falhado>/migrations)
```

Uma diferença é a resposta. Resolver isto por compatibilidade de esquema é
trabalho da Parte 10 do programa de generalização
([arquitectura-alvo](../architecture/TARGET_OCINYE_OS.md#6-itens-obrigatórios-que-nasceram-da-parte-0)).
