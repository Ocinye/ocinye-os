# ADR-0400 — Ocinye Mail como superfície institucional, não como cliente de email

- **Estado:** Accepted
- **Domínio:** Mail
- **Impacto:** HIGH
- **Data:** 2026-08-22
- **Relaciona-se com:** [ADR-0006](0006-modular-monolith.md) · [ADR-0007](0007-domain-boundaries-in-modules.md)

## Context

A Ocinye precisa de correio institucional. A pergunta que este ADR responde não
é *qual cliente de email usar* — é **o que é o correio dentro de um sistema
operacional institucional**.

Um cliente de email é uma aplicação que mostra mensagens. O `CLAUDE.md` §2 é
explícito sobre o que não estamos a construir, e «mais uma aba com uma caixa de
entrada» cabe nessa lista tão bem como um CMS ou um dashboard.

Ao mesmo tempo, o correio é onde a instituição fala com o mundo. É por lá que
sai uma proposta, entra um convite para colaboração, chega um dataset de um
parceiro. Tratá-lo como um anexo cosmético ao Workspace desperdiça isso.

## Decision

**Ocinye Mail é um módulo do Ocinye Core**, ao lado de `research`, `knowledge` e
`governance` — não um serviço à parte, não uma integração, não um iframe para um
webmail de terceiros.

Consequências directas dessa escolha:

- **A autorização é a mesma.** As sete permissões de correio (`mail.use`,
  `mail.send`, `mail.ai_use`, `mail.shared.view`, `mail.shared.send`,
  `mail.shared.manage`, `mail.administer`) vivem no mesmo catálogo `Permission`
  que todas as outras e passam pelo mesmo `can()`.
- **A classificação é a mesma.** `PUBLIC`/`INTERNAL`/`CONFIDENTIAL`/`RESTRICTED`
  governam o que pode sair por correio ([ADR-0403](0403-mail-send-policy.md)).
- **A auditoria é a mesma.** Um envio é um `audit_event` como qualquer outra
  operação institucional.
- **As capacidades são as mesmas.** `mail`, `mail.send`, `mail.sync` e
  `mail.ai_assist` são `Capability` com `CapabilityState`, e a interface lê-as
  como lê o estado da IA ou da computação.

## Alternatives

**Integrar um webmail existente por iframe ou SSO.** Mais rápido. Perde tudo o
que motiva o módulo: a classificação não atravessa a fronteira, a auditoria fica
noutro sistema, e o `CLAUDE.md` §16 obriga a tratar essa fronteira como não
confiável — o que significaria não poder afirmar nada sobre o que lá acontece.

**Um serviço separado `services/mail`.** Contraria o [ADR-0006](0006-modular-monolith.md)
sem necessidade real. O correio partilha pessoas, permissões e auditoria com o
resto do Core; extraí-lo hoje criaria três chamadas de rede para responder a
«esta pessoa pode ler esta caixa».

**Não fazer correio.** Legítimo, e foi considerado. Recusado porque o correio é
onde a proveniência institucional se perde hoje: uma decisão tomada por email
não fica em lado nenhum que o Ocinye OS conheça.

## Consequences

- O Core ganha um módulo cuja fronteira externa (IMAP/SMTP) é a mais hostil de
  todo o sistema. Ver [ADR-0402](0402-mail-html-sanitisation.md) e
  [ADR-0405](0405-mail-prompt-injection.md).
- O correio **não** entra no índice institucional de pesquisa. A pesquisa de
  correio é por caixa, dentro do que o membro já pode ler. Um índice partilhado
  seria um caminho para correspondência alheia.
- `mail.sync` fica **`PLANNED`**: a ingestão IMAP não está implementada, e o
  estado é reportado como tal em vez de simulado.
