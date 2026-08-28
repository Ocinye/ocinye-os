# ADR-0403 — Enviar é exportar: política de classificação no envio

- **Estado:** Accepted
- **Domínio:** Mail
- **Impacto:** HIGH
- **Data:** 2026-08-22
- **Relaciona-se com:** [ADR-0101](0101-permissions-scopes-and-grants.md) · [ADR-0201](0201-data-residency.md)

## Context

O `CLAUDE.md` §36 estabelece que a classificação acompanha o artefacto ao longo
de todo o ciclo de vida, «incluindo cópias, versões, derivados e exportações».

Um email com um anexo para um endereço externo é uma exportação. E é uma
exportação irreversível: depois de entregue, nenhuma ACL da Ocinye alcança a
caixa de correio de outra pessoa.

## Decision

`SendPolicy::evaluate(recipients, attachment_classifications, confirmed)`, em
`crates/ocinye-core/src/modules/mail/policy.rs`.

| Classificação | Destinatários internos | Algum destinatário externo |
|---|---|---|
| `PUBLIC` | permitido | permitido |
| `INTERNAL` | permitido | permitido, **com confirmação** |
| `CONFIDENTIAL` | permitido | permitido, **com confirmação**, auditado |
| `RESTRICTED` | permitido | **recusado** |

Três decisões dentro desta tabela merecem justificação.

**A classificação mais alta governa a mensagem inteira.** Um anexo `RESTRICTED`
entre dez `PUBLIC` torna a mensagem `RESTRICTED`. O contrário permitiria
contornar a política por diluição.

**A confirmação nunca transforma uma recusa em envio.** Confirmar é consentir
num acto permitido, não autoridade para realizar um proibido. Existe um teste
cujo nome é exactamente isto, porque é a garantia que mais importa e a que mais
facilmente se perderia numa refactorização.

**`PUBLIC` nunca pergunta, e correio interno nunca pergunta.** Perguntar em cada
mensagem treina as pessoas a clicar sem ler, o que destrói o valor da pergunta
nos casos em que ela importa.

### O que conta como externo

`MailAddress::new` decide por correspondência **exacta** de domínio contra
`OCINYE_MAIL_INSTITUTIONAL_DOMAINS`. `ocinye.com.atacante.net` termina no
domínio institucional e **não é** o domínio institucional: é externo.

Uma lista vazia torna todos os destinatários externos — falha fechada. Por isso
o Core recusa arrancar com correio configurado e lista de domínios vazia: seria
uma configuração que barra correio interno perfeitamente normal, e o membro não
teria como perceber porquê.

## Alternatives

**Recusar `CONFIDENTIAL` para o exterior também.** Considerado. Recusado porque
tornaria o correio inutilizável para trabalho real com parceiros, e o resultado
previsível seria as pessoas usarem correio pessoal — pior em todos os eixos.

**Deixar a decisão ao membro sem política.** Contraria o `CLAUDE.md` §36.

## Consequences

- Anexos institucionais são **`PLANNED`**: sem object storage configurado não há
  o que anexar, pelo que hoje a lista de classificações chega vazia e a política
  devolve `Allowed`. A tabela acima entra em vigor no mesmo commit que ligar os
  anexos, sem alteração à política.
- Cada decisão tem rótulo estável (`allowed`, `needs_confirmation`, `refused`)
  para o audit trail.
- Onze testes cobrem a política, incluindo domínios semelhantes e a tentativa de
  contornar uma recusa com confirmação.
