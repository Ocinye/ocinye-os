# Runbook — Bootstrap do primeiro administrador

**Quando:** instalação nova, sem nenhum `platform_admin` utilizável.
**Quem:** quem tiver acesso ao host do Ocinye Core.
**Duração:** minutos.

## Antes

- [ ] PostgreSQL acessível e migrations aplicadas.
- [ ] `OCINYE_DATABASE_URL` definida no ambiente do comando.
- [ ] Canal seguro combinado para entregar a credencial (ver «Entrega»).

## Passos

```bash
ocinye-core-server bootstrap-admin \
  --name "Nome Completo" \
  --username nome.utilizador \
  --email pessoa@ocinye.com
```

Saída, **uma única vez**:

```
  Administrador principal criado.

  Nome                 Nome Completo
  Utilizador           nome.utilizador
  Palavra-passe        XXXX-XXXX-XXXX-XXXX-XXXX-XXXX
  Válida até           AAAA-MM-DD HH:MM UTC
```

## Entrega

A credencial é temporária e vale 24 horas. Entregue-a por um canal que não fique
registado de forma durável:

- **Aceitável:** presencialmente; chamada de voz; mensagem efémera cifrada.
- **Não aceitável:** email, SMS, ticket, chat institucional, captura de ecrã.

Não a escreva em papel que fique. Não a cole num gestor de palavras-passe: vai ser
substituída dentro de minutos.

## Primeiro acesso

O administrador vai a `https://workspace.ocinye.com/login`, autentica-se com a
credencial, e é levado directamente para **Defina a sua palavra-passe**. Não
consegue abrir mais nada até o fazer — o Core recusa, e escrever um endereço à
mão não contorna nada.

## Verificação

```bash
psql "$OCINYE_DATABASE_URL" -c "
  SELECT p.username, p.status, r.role
    FROM people p JOIN person_roles r ON r.person_id = p.id
   WHERE r.role = 'platform_admin' AND r.revoked_at IS NULL"
```

Esperado logo após o bootstrap: `status = invited`. Depois do primeiro acesso:
`status = active`.

```bash
psql "$OCINYE_DATABASE_URL" -c "
  SELECT action, occurred_at FROM audit_events
   WHERE action = 'bootstrap_admin'"
```

## Se recusar

```
Recusado: A plataforma já tem um administrador. O bootstrap corre uma única vez.
```

É o comportamento correcto. Não existe flag de override. Se perdeu o acesso, siga
[Recuperar acesso administrativo](recover-administrative-access.md).

## Se a credencial expirar antes de ser usada

Não há como prolongá-la. Suspenda a conta criada e volte a correr o bootstrap:

```bash
psql "$OCINYE_DATABASE_URL" -c \
  "UPDATE people SET status = 'suspended' WHERE username = 'nome.utilizador'"
```

O guard conta apenas administradores **utilizáveis**, pelo que o bootstrap volta
a aceitar. Registe porque o fez.
