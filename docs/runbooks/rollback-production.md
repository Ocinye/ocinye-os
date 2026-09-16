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

Um release anterior tem um esquema anterior. Se o release falhado **aplicou uma
migração** que o anterior não conhece, reverter o código não reverte o esquema —
e as migrações da Ocinye são de avanço (não há *down*). Nesse caso, reverter o
código é seguro enquanto o esquema novo for **retro-compatível** com o código
anterior (é o caso das migrações aditivas). Uma migração destrutiva exige o plano
de restauro da base (ver o runbook de restauro), não uma simples reversão de
symlink.
