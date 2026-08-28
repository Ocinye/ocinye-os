# Runbook — Repor a palavra-passe de um membro

**Quando:** um membro perdeu o acesso, a credencial temporária expirou, ou há
suspeita de compromisso.
**Quem:** quem detiver `members.manage`.

## Confirme quem está a pedir

Um pedido de reset é o vector de engenharia social mais barato que existe.
**Confirme a identidade por um canal que não seja aquele por onde o pedido
chegou.**

Se o pedido chegou por email, telefone. Se chegou por chat, telefone. Se não
consegue confirmar, não reponha.

## O que o reset faz

1. Emite nova credencial temporária, válida 24 horas.
2. Invalida a palavra-passe anterior — **deixa de funcionar imediatamente**.
3. Revoga **todas** as sessões activas do membro.
4. Marca a conta como devendo mudança de palavra-passe.
5. Regista `password_reset` na auditoria, com quem o fez.

Se a suspeita for de compromisso, os pontos 2 e 3 são o objectivo: expulsam quem
lá esteja.

## Passos

**Workspace →** Administração → Membros → *(o membro)* → Segurança →
**Redefinir palavra-passe**.

Confirmação:

> Isto terminará as sessões activas e criará uma nova palavra-passe temporária.
> O utilizador terá de definir uma nova palavra-passe no próximo acesso.

A credencial é apresentada **uma única vez**.

## Entrega

Como em [Criar um membro](create-member.md#entrega).

## O que nunca fazer

- **Nunca** escolher a palavra-passe definitiva de outra pessoa. Não existe tal
  funcionalidade, e não deve passar a existir.
- **Nunca** pedir a palavra-passe actual de alguém. Não é recuperável, e pedi-la
  ensina a entregá-la a quem a peça.
- **Nunca** repor sem confirmar quem pede.

## Verificação

```bash
psql "$OCINYE_DATABASE_URL" -c "
  SELECT kind, state, expires_at FROM credentials
   WHERE person_id = '<uuid>' ORDER BY created_at DESC LIMIT 3"
```

Esperado: uma `temporary` `active` com expiração no futuro; a `permanent`
anterior `revoked`.

```bash
psql "$OCINYE_DATABASE_URL" -c "
  SELECT count(*) FROM sessions
   WHERE person_id = '<uuid>' AND state <> 'revoked'"
```

Esperado: `0`.
