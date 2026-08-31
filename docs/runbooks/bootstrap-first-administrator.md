# Runbook — Bootstrap do primeiro administrador

**Quando:** instalação nova, sem nenhum `platform_admin` utilizável.
**O que cria:** a organização, a pessoa institucional e a identidade privilegiada ligada a ela.
**Quem:** quem tiver acesso ao host do Ocinye Core.
**Duração:** minutos.

## Antes

- [ ] PostgreSQL acessível e migrations aplicadas.
- [ ] `OCINYE_DATABASE_URL` definida no ambiente do comando.
- [ ] Canal seguro combinado para entregar a credencial (ver «Entrega»).

## Passos

```bash
ocinye-core-server bootstrap-admin \
  --name        "Fidel Monteiro"        --email       fidel@ocinye.com \
  --admin-name  "Fidel Admin"           --admin-email fidel.admin@ocinye.com
```

Saída, **uma única vez**:

```
  Instituição e administrador criados.

  Organização          ocinye
  Pessoa institucional Fidel Monteiro · fidel@ocinye.com
    (sem acesso — dê-lho pelo Ocinye OS, em Administração)

  Identidade privilegiada
  Nome                 Fidel Admin
  Utilizador           fidel.admin@ocinye.com
  Palavra-passe        XXXX-XXXX-XXXX-XXXX-XXXX-XXXX
  Válida até           AAAA-MM-DD HH:MM UTC
```

## Duas identidades, e porquê

O comando cria **duas** linhas ligadas entre si:

| | Pessoa institucional | Identidade privilegiada |
|---|---|---|
| Quem é | `Fidel Monteiro` | `Fidel Admin` |
| Para que serve | responder pelo trabalho | executar administração |
| Autoridade | nenhuma, à partida | `platform_admin` |
| Credencial no arranque | **nenhuma** | temporária, 24 h |

> **Uma identidade privilegiada ligada estabelece responsabilidade, e não
> herança de autoridade.**

A pessoa institucional nascer sem credencial não é um passo em falta: é a
regra. O servidor arranca o primeiro administrador; o administrador arranca a
instituição, pelo produto. Quem tentar entrar com `fidel@ocinye.com` logo a
seguir ao bootstrap **não consegue**, e está correcto.

Para lhe dar entrada, depois do primeiro acesso: **Administração › o membro ›
Segurança › Dar acesso**. Isso emite-lhe uma credencial temporária e não lhe
altera papéis, unidades nem autoridade. Não crie uma segunda pessoa com o mesmo
nome — dois registos repartem autoria, pertenças e histórico por dois sítios
que ninguém volta a juntar.

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
  SELECT p.endereço, p.status, r.role
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
  "UPDATE people SET status = 'suspended' WHERE endereço = 'nome.utilizador'"
```

O guard conta apenas administradores **utilizáveis**, pelo que o bootstrap volta
a aceitar. Registe porque o fez.
