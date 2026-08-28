# Runbook — Suspender ou desactivar um membro

**Quem:** quem detiver `members.manage`.

## Qual dos dois

| | Suspender | Desactivar |
|---|---|---|
| Intenção | Temporário | Permanente |
| Casos | Licença longa; suspeita a investigar; disputa em curso | Saída da instituição; fim de contrato |
| Reversível | Sim, sem cerimónia | Sim, mas é uma decisão |
| Sessões | Revogadas de imediato | Revogadas de imediato |
| Autoria | Preservada | Preservada |
| Dados | Intactos | Intactos |

**Nenhum dos dois apaga o que quer que seja.** Que uma pessoa participou é
memória institucional e não se remove porque saiu (`CLAUDE.md` §58, §41).

Perante suspeita de compromisso da conta, e não da pessoa, prefira
[repor a palavra-passe](reset-member-password.md): expulsa quem lá esteja sem
marcar a pessoa.

## Antes de desactivar

Um membro que sai leva responsabilidades consigo. Antes:

- [ ] Ideias e projectos de que é responsável — transferir.
- [ ] Tarefas abertas — reatribuir.
- [ ] Agentes de IA que criou — transferir ou retirar.
- [ ] Grants explícitos que detém — revogar os que já não se justificam.
- [ ] Datasets e documentos de que é titular — confirmar a titularidade.

O ecrã de detalhe do membro lista o que está por transferir.

## Passos

**Workspace →** Administração → Membros → *(o membro)* → **Estado**.

A razão é **obrigatória** e fica na auditoria. Escreva-a para quem a vier ler
daqui a dois anos: «licença sabática até 2027-03» diz alguma coisa; «pedido»
não diz nada.

## Não se pode fazer a si próprio

O Core recusa suspender ou desactivar a conta de quem faz o pedido. É assim que
uma instituição fica sem administrador e sem caminho de volta.

## Verificação

```bash
psql "$OCINYE_DATABASE_URL" -c "
  SELECT status, deactivated_at FROM people WHERE username = '<nome>'"

psql "$OCINYE_DATABASE_URL" -c "
  SELECT count(*) FROM sessions
   WHERE person_id = '<uuid>' AND state <> 'revoked'"
```

Esperado: o estado pedido, `deactivated_at` preenchido, e `0` sessões vivas.

## Reinstatement

Repor o estado para `active` devolve o acesso. A pessoa mantém a palavra-passe
que tinha, salvo se tiver havido reset entretanto.

Se a suspensão foi por suspeita de compromisso,
[reponha a palavra-passe](reset-member-password.md) **antes** de reinstatar.
