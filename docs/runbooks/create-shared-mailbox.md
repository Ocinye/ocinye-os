# Runbook — Criar e gerir uma caixa de correio partilhada

**Quando:** uma unidade, um projecto ou uma função precisa de um endereço comum
— `investigacao@`, `geral@`, `parcerias@`.
**Quem:** quem detiver `mail.administer` para criar; `mail.shared.manage` para
gerir a pertença de uma caixa existente.

## Estado

A criação e a gestão de pertenças ainda **não têm ecrã** no Ocinye Workspace
(`PLANNED`). O modelo, as constraints e as consultas existem e estão testados; a
operação faz-se hoje por migration ou por SQL revisto.

Este runbook descreve o que a operação tem de garantir, para que o ecrã — quando
existir — não a torne mais permissiva.

## Duas regras que não se contornam

**Uma caixa partilhada não tem dono.** `ck_mailboxes_ownership_agrees` obriga:
`personal` tem `owner_id`, `shared` não tem. Uma caixa partilhada com dono seria
alcançável pelos dois ramos da verificação de acesso.

**Converter uma caixa pessoal em partilhada dá acesso à correspondência que lá
está.** É a única forma legítima de a instituição alcançar o correio de alguém
que saiu — e é um acto visível, auditado e reversível, nunca um privilégio
silencioso ([ADR-0404](../adrs/0404-mail-privacy-boundary.md)).

Se converter uma caixa pessoal:

1. informe a pessoa, salvo se houver razão institucional documentada para não o
   fazer;
2. registe quem autorizou;
3. dê pertença apenas a quem precisa, com o papel mínimo.

## Os quatro papéis

| Papel | Ler | Responder | Enviar como | Gerir pertenças |
|---|---|---|---|---|
| `Reader` | ✓ | | | |
| `Responder` | ✓ | ✓ | | |
| `Sender` | ✓ | ✓ | ✓ | |
| `Manager` | ✓ | ✓ | ✓ | ✓ |

**Comece pelo mínimo.** `Reader` para quem só precisa de acompanhar. `Sender`
apenas para quem representa a instituição naquele endereço.

## Remover alguém

Marque `revoked_at`. **Não apague a linha.** O acesso cessa na consulta seguinte
— não há cache de pertenças — e fica registo de que existiu, que é o que permite
responder mais tarde a «quem teve acesso a esta caixa».

## Quando alguém sai da instituição

Ver [offboard-member.md](offboard-member.md). Em relação ao correio:

1. revogue todas as pertenças a caixas partilhadas;
2. decida explicitamente o que fazer à caixa pessoal — manter fechada, ou
   converter em partilhada com autorização registada;
3. **não** reatribua a caixa pessoal a outra pessoa sem essa decisão.
