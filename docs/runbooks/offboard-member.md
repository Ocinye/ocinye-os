# Runbook — Offboarding de um membro

**Quando:** alguém deixa a instituição.
**Quem:** quem detiver `members.manage`, com quem lidera a unidade.

Um offboarding não é apagar uma conta. É transferir responsabilidades e depois
fechar o acesso, **preservando** tudo o que a pessoa produziu.

## Princípio

> A autoria histórica nunca muda. Que uma pessoa criou uma ideia, escreveu uma
> nota ou catalogou um dataset é facto institucional, e permanece depois de sair
> (`CLAUDE.md` §14, §41).

O que se transfere é **responsabilidade futura**, não crédito passado.

## Antes

Abra o detalhe do membro e percorra o que está a seu cargo.

| Artefacto | Acção |
|---|---|
| Ideias de que é responsável | Transferir para outro membro da unidade |
| Projectos que lidera | Transferir a liderança |
| Tarefas abertas | Reatribuir ou fechar com motivo |
| Agentes de IA que criou | Transferir, ou retirar se eram pessoais |
| Grants explícitos | Revogar os que só se justificavam pelo seu trabalho |
| Membership de unidades | Revogar |
| Membership de research workspaces | Revogar |

Cada revogação de grant exige razão escrita, e fica na auditoria.

## Passos

1. Transferir tudo o que consta acima.
2. Confirmar que nada ficou por transferir.
3. **Desactivar** a conta — ver
   [Suspender ou desactivar](suspend-or-disable-member.md).
4. Registar a data de saída na razão.

Se houver período de aviso e a pessoa continuar a trabalhar, **suspenda apenas no
último dia**. Desactivar cedo produz trabalho perdido e um pedido de reinstatement.

## Verificação

```bash
psql "$OCINYE_DATABASE_URL" -c "
  SELECT status, deactivated_at FROM people WHERE endereço = '<nome>'"

psql "$OCINYE_DATABASE_URL" -c "
  SELECT count(*) FROM sessions WHERE person_id = '<uuid>' AND state <> 'revoked'"

psql "$OCINYE_DATABASE_URL" -c "
  SELECT permission, scope FROM explicit_access_grants
   WHERE subject_id = '<uuid>' AND revoked_at IS NULL"
```

Esperado: `disabled`, `0` sessões, e nenhum grant vivo que já não se justifique.

A conta continua a existir, e é isso que se pretende: os artefactos que a pessoa
criou continuam a poder nomear quem os criou.
