# ADR-0606 — Identidade privilegiada ligada

- **Estado:** Accepted
- **Domínio:** Identity
- **Impacto:** FOUNDATIONAL
- **Data:** 2026-08-31
- **Relaciona-se com:** [ADR-0103](0103-core-owned-authentication.md) ·
  [ADR-0604](0604-workspace-access-presentation.md) ·
  [ADR-0605](0605-first-production-deployment.md)

## Context

A instituição precisa de um primeiro administrador. A maneira óbvia é dar
`platform_admin` à pessoa que a dirige e seguir em frente. É o que a versão
anterior do `bootstrap-admin` fazia: uma linha em `people`, com o papel colado.

Isso junta duas coisas que respondem a perguntas diferentes. **Quem responde**
pelo que aconteceu é uma pessoa, com nome, histórico e pertenças. **O que
executou** uma operação administrativa é uma sessão com autoridade. Numa única
linha, as duas confundem-se: a auditoria diz «Fidel Monteiro apagou», e não
distingue o Fidel a trabalhar do Fidel a administrar. Pior — a autoridade fica
sempre ligada, e todo o trabalho normal daquela pessoa passa a correr com ela.

A saída errada seria um `switch-to-admin`: um botão que eleva a sessão. Isso põe
a fronteira dentro da interface, e uma fronteira que a interface desenha é uma
fronteira que a interface pode enganar-se a desenhar.

## Decision

Duas linhas em `people`, ligadas.

```text
Fidel Monteiro   identity_kind = 'human'       belongs_to = NULL
      ▲
      └───────── Fidel Admin   identity_kind = 'privileged'   belongs_to = Fidel Monteiro
                               papéis: platform_admin
```

A propriedade que isto estabelece:

> **Uma identidade privilegiada ligada estabelece responsabilidade, e não
> herança de autoridade.**

A ligação diz **quem responde** por aquela identidade. Não transporta um único
papel, permissão ou pertença em nenhuma das direcções. Um teste fixa cada
sentido: revogar a autoridade não torna a identidade normal, e dar autoridade a
uma pessoa não a torna privilegiada.

### O que a base garante, e não o código

`identity_kind` é `'human'` ou `'privileged'`, por `CHECK`. Uma identidade
privilegiada **tem** de ter dono; uma humana **não pode** ter. Ninguém é dono de
si próprio. E um gatilho recusa que o dono seja outra identidade privilegiada:
a cadeia tem exactamente um degrau, porque uma cadeia com dois já não responde à
pergunta «quem é a pessoa».

Isto vive na base porque é uma invariante de estado, e não uma regra de fluxo. O
código que a esquecesse continuaria a compilar; a base recusa a linha.

### O bootstrap cria a instituição, não a povoa

`bootstrap-admin` passa a exigir quatro argumentos: a pessoa e a identidade. A
organização é adoptada ou criada de forma idempotente, e um slug de exemplo
(`default`, `demo`, `test`, …) é **recusado** em vez de corrigido — um valor
desses não é um engano de escrita, é a configuração a não ter sido posta.

A pessoa institucional nasce **sem credencial**. Não é um passo em falta:

> **O servidor arranca o primeiro administrador. O administrador arranca a
> instituição pelo Ocinye OS.**

### Dar acesso a quem já existe

Daí decorre uma operação que faltava ao produto: `provision_existing_person`.
Sem ela, o primeiro administrador teria de criar um segundo `Fidel Monteiro`
para poder entrar como pessoa — e dois registos com o mesmo nome repartem
autoria, pertenças e histórico por dois sítios que ninguém volta a juntar.

A operação recusa três coisas, e cada recusa tem a sua razão:

| recusa | porquê |
|---|---|
| uma identidade privilegiada | nasce do bootstrap, não da administração |
| quem já tem credencial viva | isso é `reset_password`, e fica registado como reposição |
| alguém de outra organização | e responde `NotFound`, para o identificador não ser um oráculo de existência |

Fica auditada como `account_provisioned` — distinta de `member_created`, porque
uma pessoa que já existia não foi criada agora.

### A faixa apresenta; não autoriza

A sessão privilegiada é assinalada por uma faixa permanente, encarnada, presente
em todos os ecrãs. Ela é desenhada a partir do que o Core responde em `/me`.

> **A faixa representa a sessão. A faixa não autoriza a sessão.**

Removê-la do HTML não concede nada a ninguém: a autoridade é decidida no Core, a
cada pedido. A faixa condensa-se em ecrãs estreitos, e não desaparece — uma
faixa que se some quando o ecrã aperta falha exactamente quando é mais fácil
esquecer com que autoridade se está a agir.

O mesmo princípio governa o botão «Dar acesso»: quem decide se ele aparece é o
Core, no campo `may_be_provisioned`. Ausente a resposta, o ecrã **não** oferece
a operação. Um `unwrap_or(true)` mostraria o botão sempre que a consulta
falhasse, e quem administra carregaria nele para receber uma recusa que se lê
como falta de autoridade sua.

## Consequences

**Ganha-se** uma auditoria que responde às duas perguntas: `Fidel Admin`
executou, e `Fidel Monteiro` responde. Ganha-se que o trabalho normal da pessoa
deixe de correr com autoridade de plataforma. Ganha-se uma instalação nova que
não obriga a duplicar ninguém.

**Custa** duas linhas onde havia uma, e um bootstrap com quatro argumentos em vez
de dois. Custa também que quem administra tenha duas credenciais para gerir.

**Não se decide aqui** nenhum mecanismo de elevação de sessão. Continua a não
haver `switch-to-admin`, e quem quiser administrar entra com a identidade que
administra.

**Fica por fazer**: a auditoria resolve a pessoa por trás da identidade por
consulta (`LEFT JOIN people ON belongs_to_person_id`), e ainda não a apresenta
nesse formato no ecrã de Audit. O dado está lá e é exercido por teste; falta o
ecrã lê-lo.
