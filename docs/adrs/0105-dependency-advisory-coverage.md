# ADR-0105 — Nenhuma base de advisories é tratada como exaustiva

- **Estado:** Accepted
- **Domínio:** Security
- **Impacto:** HIGH
- **Data:** 2026-08-24
- **Relaciona-se com:** [ADR-0004](0004-rust-first.md) ·
  [ADR-0005](0005-monorepo-cargo-workspace.md) ·
  [ADR-0103](0103-core-owned-authentication.md)

## Context

Em 2026-08-24 o repositório estava em dois estados ao mesmo tempo.

O Dependabot do GitHub tinha um alerta aberto: `GHSA-h395-gr6q-cpjc`
(`CVE-2026-25537`), severidade Moderate, sobre o `jsonwebtoken 9.3.1` — uma
dependência directa de runtime, compilada no `core-server` e no `worker`.

O `cargo audit` reportava **zero vulnerabilidades**.

Nenhum dos dois estava a mentir. O `cargo audit` consulta a base RustSec; o
Dependabot consulta a GitHub Advisory Database. Este advisory existe numa e não
na outra. O que estava errado não era nenhuma das ferramentas: era a conclusão
tirada de um único tique verde chamado «segurança».

A divergência também não é num sentido só, e é isso que fecha o argumento. O
`RUSTSEC-2023-0071` — o ataque Marvin sobre o crate `rsa` — marca a versão
0.9.10 que esta árvore resolve, e por isso vive em `.cargo/audit.toml` com a
razão escrita. O GitHub tem o mesmo defeito como `GHSA-c38w-74pg-36hr` mas
publica-o com intervalo `<= 0.9.6`, ou seja, dá-o por corrigido desde a 0.9.7.

Uma base vê o que a outra não vê, **nos dois sentidos**. Não existe aqui um
scanner melhor a escolher.

Havia ainda um segundo problema, do mesmo feitio. A CI morreu duas vezes com
`No space left on device` a meio de um `rustc`. O sintoma aparece longe da
causa: lê-se como avaria do código, e é o runner a ficar sem espaço. Não havia
qualquer medição de disco, e por isso a falha só se manifestava vinte minutos
depois do início, no ficheiro errado.

## Decision

**Nenhuma base de advisories é tratada como exaustiva, e cada garantia de
segurança na CI diz o nome do universo que consultou.**

Concretamente:

1. **Portões com nome próprio.** Deixa de existir um tique agregado de
   segurança. Passam a existir *Advisories RustSec*, *Advisories do GitHub*,
   *Postura do branch canónico*, *Guardas de versões vulneráveis conhecidas* e
   *Segredos*. Cada verde prova exactamente o que o seu nome diz.

2. **A base do GitHub é consultada directamente.** A Dependency Review Action
   do GitHub faria a comparação para o diff de um pull request, mas foi medida
   e devolve 404 neste repositório: é privado e a organização está no plano
   gratuito. Em vez de assumir disponibilidade ou de manter um job
   permanentemente vermelho, o `Cargo.lock` inteiro é confrontado com a GitHub
   Advisory Database através da API pública de GraphQL, que responde em
   qualquer plano.

   Confrontar o lockfile inteiro é mais forte do que rever o diff: apanha
   também uma versão que já cá estava e sobre a qual saiu um advisory ontem.

3. **O pull request e o branch canónico respondem a perguntas diferentes.** Um
   pull request é julgado pelo que traz: contém alguma versão vulnerável
   conhecida? O `main` é julgado pelo que ainda carrega: existe algum alerta
   Dependabot aberto?

   Os dois ciclos de vida ficam separados de propósito. Uma regra da forma
   «falha enquanto houver um alerta aberto» aplicada a pull requests reprovaria
   justamente o pull request que vem fechar o alerta, porque o alerta só fecha
   quando a correcção chega ao branch por omissão.

4. **Falhar a perguntar nunca é uma resposta.** Uma consulta que devolve erro,
   403 ou algo que não é uma lista é recusada e reportada como
   `NÃO VERIFICADO`, com código de saída próprio. Uma falha de telemetria de
   segurança convertida em lista vazia é como um repositório passa a acreditar
   que está limpo.

5. **Guardas locais para o que já nos mordeu.** Uma lista curta e explícita de
   versões mínimas — hoje, `jsonwebtoken >= 10.3.0` — corre offline e em
   milissegundos. Não é um scanner e não cresce por varrimento: cresce por
   incidente. Existe porque um `Cargo.lock` pode regredir num merge antes de
   qualquer serviço externo ter oportunidade de reparar.

6. **Buscar e julgar são separados.** Cada avaliador de política é uma função
   pura sobre o que a rede devolveu, testada contra fixtures: alerta aberto,
   corrigido, dispensado, severidade abaixo do piso, dependência de
   desenvolvimento, âmbito omisso, payload de erro. Um portão cuja única prova
   é «hoje ficou verde» não prova nada sobre o dia em que aparecer um achado.

7. **A capacidade do runner é medida, não suposta.** Antes de qualquer
   compilação: medir, libertar o que a imagem do GitHub traz e este projecto
   não usa, medir outra vez, e exigir um mínimo. Falhar em quinze segundos com
   uma mensagem que nomeia a causa vale mais do que falhar em vinte minutos
   dentro do compilador.

## Consequences

**O que melhora.**

Um advisory que exista apenas numa das bases passa a ser visto. O caso concreto
que originou esta decisão — `cargo audit` verde com um alerta Dependabot aberto
— deixa de poder repetir-se sem que um portão com nome próprio o diga.

Uma falha de disco na CI passa a ser diagnosticável a partir do log, porque o
consumo de cada fase fica registado.

**O que custa.**

O portão do GitHub consulta a rede e demora cerca de um minuto e meio para as
662 crates da árvore. É tempo real, e o preço de perguntar à base que a outra
ferramenta não consulta.

Uma lista de versões conhecidas é, por construção, incompleta: cobre só
incidentes passados. Não substitui os scanners, e o ADR não a apresenta como
tal — é defesa de última linha para regressão, não descoberta.

**O que ficou por resolver, e é preciso dizê-lo.**

A CI **não consegue ler os alertas Dependabot deste repositório.** Foi medido, e
não deduzido: a API responde `403 Forbidden` ao `GITHUB_TOKEN` das Actions mesmo
com `security-events: read` declarado. Não é uma permissão em falta que se possa
acrescentar — o token de Actions não tem âmbito para esta API, e aumentar
permissões não cria o que não existe.

A medição foi feita de propósito **antes** do merge, com um passo que não
reprovava nada. O job de postura só corre no `main`, e descobrir isto depois
seria descobri-lo com o branch canónico vermelho por uma razão que nada tem que
ver com o código que lá entrou.

O que a substitui: o portão do branch canónico confronta o `Cargo.lock` do
`main` com a mesma base de advisories que o Dependabot lê, e essa consulta não
precisa de permissão nenhuma. Para um projecto Cargo, um alerta Dependabot
aberto existe precisamente porque uma versão do lockfile cai no intervalo de um
advisory do GitHub — que é a pergunta que o portão faz.

A diferença que resta é real e fica escrita: o portão não vê o **estado** que o
GitHub atribui a um alerta. Um alerta dispensado por decisão humana continuaria
a ser encontrado no lockfile, e um advisory sobre um ecossistema que não o Cargo
— as próprias acções da CI, por exemplo — não seria visto de todo.

Sai deste estado no dia em que existir um token institucional com âmbito de
leitura de alertas. O passo já está preparado para o usar, e até lá diz o que
não consultou em vez de ficar verde a fingir.

## Alternatives considered

**Escolher um scanner e confiar nele.** Rejeitado pela medição: as duas bases
divergem nos dois sentidos, e qualquer escolha única perderia advisories reais.

**Dispensar o alerta com «o OIDC não está ligado».** Rejeitado. A configuração
reduz alcance; não retira código vulnerável de dentro do binário. E, neste caso
concreto, a não-explorabilidade dependia de uma linha — `exp` em
`required_spec_claims` — que ninguém sabia estar a carregar peso.

**Manter um job de Dependency Review permanentemente vermelho** à espera de uma
mudança de plano. Rejeitado: um portão que falha sempre deixa de ser lido, e
leva os outros consigo.
