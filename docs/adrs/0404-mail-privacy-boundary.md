# ADR-0404 — Uma caixa de correio pessoal não é alcançável por privilégio

- **Estado:** Accepted
- **Domínio:** Mail
- **Impacto:** HIGH
- **Data:** 2026-08-22
- **Relaciona-se com:** [ADR-0101](0101-permissions-scopes-and-grants.md) · [ADR-0100](0100-authorization-model.md)

## Context

O `CLAUDE.md` §34 separa título institucional de capacidade técnica. O
[ADR-0101](0101-permissions-scopes-and-grants.md) concretizou-o: o
`PlatformAdmin` administra a plataforma sem por isso ganhar acesso a ciência
`RESTRICTED`.

O correio leva a mesma questão ao extremo. Administrar o serviço de correio —
configurar anfitriões, ver se o SMTP responde, criar caixas partilhadas — e ler
a correspondência de um colega são poderes completamente diferentes. Em quase
todos os sistemas de correio institucional, o segundo vem de graça com o
primeiro.

## Decision

**Nenhum papel administrativo lê uma caixa pessoal alheia.** Nem
`OrganisationAdmin`, nem `PlatformAdmin`, nem o `Fundador`.

A garantia não vive numa verificação que alguém possa esquecer-se de escrever.
Vive na **cláusula `WHERE` de cada consulta** em
`crates/ocinye-core/src/modules/mail/repository.rs`:

```sql
(b.kind = 'personal' AND b.owner_id = $1)
OR (b.kind = 'shared'  AND s.id IS NOT NULL)
```

Onde `$1` é sempre `principal.person_id`. Não existe nenhum caminho no
repositório que aceite um `person_id` diferente do actor, e não existe nenhuma
consulta de correio que consulte um papel.

Uma caixa que não é do actor lê-se como **inexistente**, não como recusada:
saber que existe uma caixa fechada já é informação (ADR-0100).

### Caixas partilhadas são explícitas e graduadas

`shared_mailbox_memberships` com quatro papéis:

| Papel | Ler | Responder | Enviar como | Gerir |
|---|---|---|---|---|
| `Reader` | ✓ | | | |
| `Responder` | ✓ | ✓ | | |
| `Sender` | ✓ | ✓ | ✓ | |
| `Manager` | ✓ | ✓ | ✓ | ✓ |

Uma pertença revogada (`revoked_at IS NOT NULL`) deixa de dar acesso na consulta
seguinte — não há cache de pertenças.

### A constraint que segura o modelo

`ck_mailboxes_ownership_agrees`: uma caixa `personal` tem `owner_id`, uma caixa
`shared` não tem. Verificada contra PostgreSQL real. Sem ela, uma caixa
partilhada com dono seria alcançável pelos dois ramos da condição acima.

## Alternatives

**Uma permissão `mail.read_any` para investigações internas.** Recusada. Se a
instituição alguma vez precisar disso, precisa de um processo com autorização
nomeada, prazo e registo — não de uma permissão que fica ligada num papel.

## Consequences

- Não existe ecrã de administração que liste mensagens de outra pessoa, e não
  existe rota que o permita.
- A administração de correio (`mail.administer`) cobre configuração,
  diagnóstico e caixas partilhadas. Nada mais.
- Recuperar correio de alguém que saiu da instituição exige converter a caixa em
  partilhada — um acto visível, auditado e reversível — e não um privilégio
  silencioso.
