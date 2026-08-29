# Segurança do Ocinye OS

Segurança é requisito fundacional, não uma milestone final. Uma funcionalidade
que contorna segurança não é uma funcionalidade concluída.

> **Baseline v1 — 2026-08-23.**
> O repositório foi auditado adversarialmente de ponta a ponta, com correcção em
> linha e regressão. 12 findings — um `HIGH`, cinco `MEDIUM`, seis `LOW` —
> todos corrigidos; um risco residual de dependência aceite e escrito.
> Registo completo: [2026-08-23-security-baseline-v1.md](2026-08-23-security-baseline-v1.md).
>
> **Follow-ups posteriores** ficam na §11 desse mesmo documento, com identificador próprio (`SB1-FU-…`). Não renumeram os doze findings da auditoria original: aquele total é o registo do que foi encontrado então, e ajustá-lo ao presente tira-lhe o valor de registo.

## Princípios

| Princípio | Como se manifesta no código |
|---|---|
| **Fail closed** | Todo o caminho de decisão em `ocinye-domain::policy` termina numa autorização explícita. Não existe ramo `_ => allow`. |
| **Deny by default** | O default de qualquer recurso novo é negar. `Classification::DEFAULT` é `INTERNAL`, nunca `PUBLIC`. Uma capacidade WASM sem declaração recebe nada. |
| **Autorização server-side** | Avaliada em `service.rs`, nunca em `api.rs`, nunca no cliente. O Workspace esconde; o Core decide. |
| **Least privilege** | Cada componente recebe apenas o necessário: o Core tem credenciais próprias de DB e de storage; um nó tem identidade de máquina; uma capacidade tem o seu manifesto. |
| **Trust boundaries explícitas** | Nenhuma rede é confiável por ser interna. Ver [arquitectura](../architecture/README.md#trust-boundaries). |
| **A existência não é revelada** | Uma leitura negada devolve `not_found`. |

## Autenticação

**Estado actual: `endereço + palavra-passe`, no Ocinye Core**
([ADR-0103](../adrs/0103-core-owned-authentication.md), que substitui o
[ADR-0102](../adrs/0102-identity-provider.md)).

> **`MFA = NOT IMPLEMENTED`. Não é exigido nesta fase.**
>
> Isto é uma redução real de segurança face à decisão anterior, e não se esconde.
> Uma palavra-passe comprometida é acesso comprometido. O que a compensa: mínimo
> de **15 caracteres**, blocklist, Argon2id, throttling, e sessões curtas com
> rotação obrigatória. MFA, passkeys e SSO federado são `PLANNED` e exigem ADR.

O Core:

- **nunca** armazena uma palavra-passe — apenas verificadores **Argon2id** em
  formato PHC ([ADR-0104](../adrs/0104-password-policy-and-hashing.md));
- deriva o principal do identificador de sessão **mais o estado institucional em
  base de dados** — papéis, memberships e grants nunca vêm do cliente;
- devolve **a mesma mensagem** para utilizador inexistente, palavra-passe errada,
  credencial expirada e conta suspensa, e gasta trabalho equivalente em todos os
  casos, para que o endpoint não seja um oráculo;
- recusa qualquer trabalho normal numa sessão em `password_change_required`, ao
  nível do extractor — antes de qualquer handler correr;
- valida os parâmetros de Argon2id **no arranque, em todos os ambientes**: um
  Core que não consegue calcular verificadores em condições recusa-se a arrancar.

**Não existe via de desenvolvimento que contorne a autenticação.** Um ramo
enfraquecido no binário distribuído é um ramo enfraquecido em produção no dia em
que alguém erra uma variável de ambiente.

### Sessões

Identificador opaco de 256 bits do CSPRNG; só o digest SHA-256 é persistido.
Cookie `HttpOnly` · `SameSite=Strict` · `Secure` fora de desenvolvimento.
Rotação obrigatória depois de início de sessão, mudança de palavra-passe e reset.
**Não há promoção de sessão**: uma sessão restrita é revogada e substituída.

Suspensão, desactivação e reset revogam **todas** as sessões de imediato.
Alterações de papel não precisam de o fazer: papéis e grants são lidos da base de
dados a cada pedido, pelo que não há autorização em cache para ficar stale.

Detalhe: [docs/identity/](../identity/README.md) e
[docs/password-policy/](../password-policy/README.md).

## Autorização

RBAC + regras contextuais ([ADR-0100](../adrs/0100-authorization-model.md)),
com permissões nomeadas, âmbitos e grants explícitos
([ADR-0101](../adrs/0101-permissions-scopes-and-grants.md)).

Toda a pergunta de autorização é feita em termos de uma `Permission` nomeada —
`documents.download`, `compute.manage_nodes` — avaliada por
`can(principal, permission, contexto)`. **`if role == admin` não aparece em lado
nenhum.**

Duas portas independentes, e ambas têm de permitir:

- a **permissão** responde *pode este actor fazer esta espécie de operação aqui*;
- a **classificação** responde *pode ver este material em concreto*.

É essa separação que permite ao `PlatformAdmin` administrar a plataforma sem com
isso ganhar acesso a ciência `RESTRICTED`.

Duas dimensões separadas:

- **Posição institucional** (`Founder`, `Director`, …) — concede **zero**
  permissões. Nem sequer aparece no tipo `Principal`.
- **Papel técnico** (`platform_admin`, `auditor`, …) — concede capacidade.

Mais **memberships contextuais**: papel na unidade e no Research Workspace.

| Classificação | Quem lê |
|---|---|
| `PUBLIC`, `INTERNAL` | Qualquer membro activo |
| `CONFIDENTIAL` | Membro da unidade, membro do workspace, ou admin |
| `RESTRICTED` | **Só** membro explícito do workspace, ou gestor da unidade |

`RESTRICTED` ignora deliberadamente papéis administrativos. É esta linha que
impede "Fundador" de significar "lê tudo", e está coberta por um teste que
percorre **todos** os papéis técnicos.

## Classificação

`PUBLIC` · `INTERNAL` · `CONFIDENTIAL` · `RESTRICTED`.

Acompanha o artefacto por cópias, versões, derivados e exportações. Um artefacto
nunca fica mais aberto do que o workspace que o contém: cada caminho de criação
passa por `most_restrictive`.

Afecta: leitura, escrita, download, exportação, pesquisa, indexação, IA, logging.

Exportar `RESTRICTED` é um direito **mais estreito** do que o ler: exige lead do
workspace ou gestor da unidade.

## Auditoria

Componente fundacional. `audit_events` é **append-only, imposto por trigger na
base de dados** — a aplicação não consegue reescrever a sua própria história nem
por engano. Verificado por teste contra PostgreSQL real.

Cada registo é escrito na **mesma transacção** que a acção auditada: uma acção
não auditável não é executada.

Metadata de auditoria é filtrada contra chaves que possam transportar conteúdo
(`content`, `body`, `password`, `token`, `secret`, `prompt`, `file`, `payload`).

## Uploads

Trust boundary. Antes de qualquer byte chegar ao storage: autorização, dimensão
máxima, tipo de conteúdo contra **allow-list**, nome de ficheiro normalizado,
checksum SHA-256 calculado e persistido.

A chave do objecto é **gerada pelo sistema e opaca**. Conhecê-la não concede
nada: o bucket é privado e cada download é autorizado e auditado, servido por URL
assinada de 5 minutos.

`scanned_at` a `NULL` significa "não analisado", **nunca** "limpo".

## Pesquisa e IA

O predicado de autorização faz parte da query. `LIMIT`, `OFFSET` e `COUNT`
operam apenas sobre o conjunto autorizado — um total que incluísse linhas
escondidas revelaria a sua existência. Coberto por teste.

O RAG aplica a política **antes** da recuperação, mais o tecto de classificação
do próprio modelo. Filtrar a resposta depois da geração não corrige um contexto
mal montado.

Conteúdo recuperado é **dados**, nunca instrução: system policy, application
policy, user input e retrieved content são estruturalmente distintos.

## Correio

A superfície mais exposta do Ocinye OS: a única entrada que **qualquer pessoa no
mundo** pode usar sem conta, com HTML e anexos arbitrários, e cujo conteúdo será
renderizado a um membro autenticado.

| Controlo | Como |
|---|---|
| HTML recebido | Higienização por **lista de permissões** (`ammonia`) antes de qualquer renderização. Um único `inner_html` no Workspace, documentado. |
| Conteúdo remoto | **Bloqueado.** É rastreio. O Core sabe servir o corpo com ele a pedido, e regista-o na auditoria; a CSP do Workspace (`img-src 'self' data:`) não o carrega, e a interface diz isso em vez de oferecer um botão inerte. |
| Saída de material classificado | `RESTRICTED` não sai para destinatário externo, e **confirmar não desfaz a recusa**. |
| Privacidade entre membros | Nenhum papel administrativo lê uma caixa pessoal alheia. A garantia está na cláusula `WHERE`, não numa verificação. |
| Prompt injection | Conjunto fechado de acções, blocos delimitados, e — a garantia que não depende do modelo — **a assistência não tem acções com efeito**. |
| Envio por IA | Impossível por construção: `assist` e `send` são rotas distintas, e `assist` não chama `send`. |
| Credenciais do serviço | Fora da base de dados. `mail_provider_settings` não tem colunas de credenciais. |

Detalhe: [docs/mail/security.md](../mail/security.md).

## Segredos

Nenhum no Git. `.env.example` sem valores reais. Tokens de convite, de
enrolamento e de agente existem em plaintext **uma só vez**, no momento em que
são emitidos; só o digest SHA-256 é persistido, pelo que não são recuperáveis de
uma base de dados nem de um backup.

## Escrita entre origens

Uma escrita autenticada tem de vir desta origem, e o cookie não chega para o
garantir.

`SameSite` compara o **domínio registável**, não a origem. Uma página em
`ocinye.com` — que o `CLAUDE.md` §5 reserva para o futuro website público — é
*same-site* com `workspace.ocinye.com`, e o browser envia o cookie da sessão com
os pedidos dela. O mesmo vale para um XSS em qualquer subdomínio irmão. Um
subdomínio não é uma fronteira de confiança (`CLAUDE.md` §16).

Por isso, em métodos que alteram estado:

| Serviço | Regra |
|---|---|
| **Workspace** | O `Origin` tem de existir e tem de ser esta origem. Fora de produção aceita-se também um `Origin` `http://` cujo host coincida com o `Host` do pedido, para que `localhost` e `127.0.0.1` sirvam o mesmo processo — e **só** fora de produção, porque comparar apenas o host aceitaria uma despromoção de esquema. |
| **Core** | Um `Origin` presente e não reconhecido é recusado. Um `Origin` ausente passa: não veio do caminho cross-origin de um browser, veio de uma CLI, de um notebook, de um agente ou do servidor do Workspace, e nenhum deles é conduzível por uma página hostil (`CLAUDE.md` §3). |

## Cabeçalhos

Core: `nosniff`, `x-frame-options: DENY`, `no-referrer`,
CSP `default-src 'none'; frame-ancestors 'none'`, `no-store`,
`cross-origin-opener-policy` e `cross-origin-resource-policy` a `same-origin`.

Workspace: `default-src 'none'; script-src 'self'; style-src 'self' + Google
Fonts; font-src Google Fonts; img-src 'self' data:; connect-src 'self';
form-action 'self'; base-uri 'none'; frame-ancestors 'none'`.

Sem `unsafe-inline` e sem `unsafe-eval`. O único script é `static/app.js`, que
faz comportamento de DOM — palette, sidebar, menu de criação — e nunca dados nem
decisões de autorização (ADR-0602).

`img-src 'self' data:` é deliberado e tem uma consequência que se declara: o
Workspace **não vai buscar conteúdo a servidores de terceiros**, por isso não
oferece carregar imagens remotas de um email. O Core continua a saber servir o
corpo com o conteúdo remoto a pedido — e a auditá-lo — para um cliente que o
queira; a interface humana não o faz.

## Dependências

**Nenhuma base de advisories é tratada como exaustiva**
([ADR-0105](../adrs/0105-dependency-advisory-coverage.md)).

Em 2026-08-24 o repositório esteve em dois estados ao mesmo tempo: o Dependabot
com um alerta aberto sobre o `jsonwebtoken`, e o `cargo audit` a reportar zero
vulnerabilidades. Nenhum dos dois estava errado. Consultam colecções
diferentes.

| Fonte | Quem a lê | O que cobre |
|---|---|---|
| **RustSec** | `cargo audit`, na CI e no `./scripts/verify.sh` | Advisories publicados pela RustSec Advisory Database |
| **GitHub Advisory Database** | `scripts/advisory_gate.py`, contra o `Cargo.lock` inteiro | Advisories publicados pelo GitHub, que é também o que o Dependabot lê |
| **Alertas do repositório** | `scripts/dependabot_posture.py` — preparado, **inactivo** | A API responde 403 ao `GITHUB_TOKEN`; medido, não suposto. Coberto pela linha acima, que corre no `main` e no relógio diário |
| **Versões já conhecidas** | `scripts/known-vulnerable-versions.sh` | A lista curta do que já mordeu este repositório |

As duas bases divergem **nos dois sentidos**. O `GHSA-h395-gr6q-cpjc` existe no
GitHub e não no RustSec. O `RUSTSEC-2023-0071` marca o `rsa 0.9.10` que esta
árvore resolve, enquanto o GitHub publica o mesmo defeito como
`GHSA-c38w-74pg-36hr` com intervalo `<= 0.9.6` — dando-o por corrigido. Não há
aqui um scanner melhor a escolher.

**Um verde diz o nome do universo que consultou.** Não existe um tique agregado
de «segurança»; existem portões nomeados, e cada um prova o que o seu nome diz.

O mesmo princípio vale para as suites de teste, e por uma razão descoberta da
maneira difícil: **um verde só é prova se os testes esperados foram descobertos
e correram**. `cargo test` devolve zero quando nada falhou, e não diz nada sobre
o que não correu. Ver [CLAUDE.md §59](../../CLAUDE.md) e
`scripts/test-enumeration.sh`.

**Falhar a perguntar não é uma resposta.** Uma consulta que devolva erro, 403 ou
algo que não seja uma lista é reportada como `NÃO VERIFICADO`, com código de
saída próprio. Uma falha de telemetria convertida em lista vazia é como um
repositório passa a acreditar que está limpo.

**A política cobre o grafo inteiro, e não uma lista de crates escolhidas.** Os
portões lêem o `Cargo.lock` completo — mais de seiscentos pares nome/versão — e
não uma lista de dependências consideradas sensíveis. Uma lista
assim envelheceria sem ninguém dar por isso, e a vulnerabilidade seguinte estaria
justamente na crate que não estava lá. As áreas que mais importam quando um
achado aparece são, ainda assim, previsíveis:

| Área | Dependências directas |
|---|---|
| Autenticação | `jsonwebtoken`, `argon2`, `password-hash` |
| TLS | `rustls`, `aws-lc-rs`, `rustls-webpki` |
| Web | `axum`, `tower-http`, `hyper`, `reqwest` |
| Base de dados | `sqlx` (só o driver `postgres`) |
| Sandbox de capacidades | `wasmtime` |
| Correio | `lettre` |

**As excepções vivem em `.cargo/audit.toml`**, cada uma com a razão escrita e
com a condição em que sai da lista. Uma excepção é risco aceite e explicado,
nunca ruído silenciado — e nunca é a resposta a um advisory que tem correcção
disponível.

**Actualizações.** As de segurança são conduzidas pelos alertas do GitHub e
estão activas. As de versão são semanais e agrupadas
([`.github/dependabot.yml`](../../.github/dependabot.yml)). Nenhuma entra sem
passar a CI inteira e por revisão humana.

## O que ainda não existe

Declarado, não escondido:

| Controlo | Estado |
|---|---|
| Rate limiting geral | **Não implementado.** A autenticação **tem** throttling — por prefixo de rede e por endereço institucional, com janela que expira e sem bloqueio de conta. As restantes rotas não têm. |
| Antimalware em uploads | **Não implementado.** O hook existe; o scan não. |
| Antimalware em anexos de correio | **Não implementado.** A descarga de anexos está declarada indisponível, o que hoje remove a via. Reabrir quando os anexos forem ligados. |
| Ingestão IMAP | **Não implementada.** `mail.sync` reporta `planned`. |
| Verificação de assinatura de capacidades WASM | **Não implementado.** O campo existe no manifesto. |
| Rede para capacidades WASM | **Não implementado — e pedi-la é recusado**, não silenciosamente concedido. |
| Backups | **Não configurados.** O restore **foi** exercitado uma vez, a 2026-08-28; não há agendamento, cópia fora do servidor, retenção nem cifra dos artefactos. Um `pg_dump` não cifrado é uma cópia de tudo o que a instituição classificou. |
| WebAuthn/passkeys | `PLANNED` no IdP. |
| Sessões do Workspace em Redis | `PLANNED`. Hoje em memória do processo. |
| Scanning de dependências | **Corre, em duas bases distintas.** Ver [Dependências](#dependências) abaixo e [ADR-0105](../adrs/0105-dependency-advisory-coverage.md). |

Modelo de ameaças completo: [docs/threat-model/](../threat-model/README.md).
