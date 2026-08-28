# Runbook — Recuperar acesso administrativo

**Quando:** não existe nenhum `platform_admin` capaz de iniciar sessão.
**Quem:** quem tiver acesso ao host e à base de dados.
**Risco:** alto. Cada passo aqui contorna a interface, e todos ficam na
auditoria.

> Não existe flag de override no `bootstrap-admin`, e não deve passar a existir.
> Este runbook é o caminho, precisamente porque exige acesso ao host — a mesma
> autoridade que já permitiria escrever a linha à mão.

## Primeiro: confirme que é mesmo isso

```bash
psql "$OCINYE_DATABASE_URL" -c "
  SELECT p.endereço, p.status
    FROM people p JOIN person_roles r ON r.person_id = p.id
   WHERE r.role = 'platform_admin' AND r.revoked_at IS NULL"
```

| O que vê | O que fazer |
|---|---|
| Uma linha `active` | Não perdeu o acesso. É palavra-passe esquecida: **Caso A**. |
| Uma linha `suspended`/`disabled` | **Caso B**. |
| Nenhuma linha | **Caso C**. |

## Caso A — administrador existe, palavra-passe perdida

Se houver **outro** administrador, peça-lhe um
[reset](reset-member-password.md). Fim.

Se for o único, emita-lhe uma credencial temporária pela base de dados. Precisa
de um verificador Argon2id: **não escreva texto em claro na coluna** — a
constraint recusa, e com razão.

```bash
# Gera credencial e verificador, sem tocar na base de dados.
cargo run -q -p ocinye-core-server -- bootstrap-admin --help  # confirma o binário
```

Como o `bootstrap-admin` recusa quando já há administrador, o caminho mais curto
e mais auditável é **Caso B**: suspenda a conta actual e volte a correr o
bootstrap para uma conta nova.

## Caso B — administrador existe mas não pode autenticar

O guard do bootstrap conta apenas administradores **utilizáveis**
(`invited` ou `active`). Uma conta `suspended` ou `disabled` não bloqueia.

```bash
# 1. Confirme que a conta antiga está mesmo fora de uso.
psql "$OCINYE_DATABASE_URL" -c "
  UPDATE people SET status = 'disabled', deactivated_at = now()
   WHERE endereço = '<antigo>'"

# 2. Crie um novo administrador pelo caminho normal.
ocinye-core-server bootstrap-admin \
  --name "Nome Completo" --endereço novo.admin --email pessoa@ocinye.com
```

Segue-se o [bootstrap normal](bootstrap-first-administrator.md): credencial
temporária, entrega segura, mudança obrigatória no primeiro acesso.

## Caso C — não há nenhum administrador

Corra o [bootstrap](bootstrap-first-administrator.md). O guard não encontra nada
e aceita.

## Depois, sempre

- [ ] Registe o incidente: o que aconteceu, quem executou, quando.
- [ ] Confirme na auditoria:

```bash
psql "$OCINYE_DATABASE_URL" -c "
  SELECT action, occurred_at, metadata FROM audit_events
   WHERE action IN ('bootstrap_admin','account_disabled')
   ORDER BY occurred_at DESC LIMIT 10"
```

- [ ] **Crie um segundo administrador.** Ter apenas um é como se chegou aqui.
- [ ] Reveja porque a situação surgiu, e o que a evitaria.

## O que nunca fazer

- **Nunca** escrever uma palavra-passe em claro em `credentials.verifier`. A
  constraint recusa, e contorná-la significaria remover a constraint.
- **Nunca** conceder `platform_admin` por `UPDATE` directo sem registar porquê:
  fica um papel sem história e sem responsável.
- **Nunca** desactivar os triggers de append-only de `audit_events` para
  «limpar». O rasto de uma recuperação de emergência é exactamente o que se quer
  guardar.
