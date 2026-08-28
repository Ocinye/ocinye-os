# ADR-0103 — Autenticação no Ocinye Core (username + password)

- **Estado:** Accepted
- **Domínio:** Identity
- **Impacto:** HIGH
- **Data:** 2026-08-22
- **Substitui:** [ADR-0102](0102-identity-provider.md)

## Context

O [ADR-0102](0102-identity-provider.md) delegou a autenticação a um Identity
Provider dedicado (Keycloak, por OIDC), em cumprimento do `CLAUDE.md` §33
(«Nunca implementes autenticação caseira»). O Ocinye Core verificava um access
token contra o JWKS publicado e nunca via uma palavra-passe.

Essa decisão continua tecnicamente sólida. O que mudou não foi a análise
técnica: foi o que a Ocinye precisa de conseguir fazer nesta fase.

O ADR-0102 implica, para autenticar o primeiro membro:

1. operar um Keycloak com estado, backup e actualizações;
2. configurar um realm, um cliente, mapeadores e política de MFA;
3. manter uma segunda fonte de identidade sincronizada com `people`;
4. resolver, na prática, *dois* problemas de bootstrap em vez de um.

Nenhum ambiente da Ocinye está deployado (`CLAUDE.md` §1). O custo de operar um
IdP antes de existir um único utilizador real é desproporcionado, e o fluxo que
a instituição precisa — um administrador cria contas, cada pessoa define a sua
palavra-passe no primeiro acesso — é exactamente o que o IdP tornaria mais
indirecto, não mais simples.

Foi ponderado manter o ADR-0102 e adiar a criação de membros. Rejeitado: a
camada de identidade é fundacional, e adiá-la adiaria tudo o que depende de
saber quem está a agir.

## Decision

**O Ocinye Core passa a ser a autoridade de autenticação**, com **nome de
utilizador e palavra-passe** como único factor nesta fase.

Concretamente:

- o Core armazena **verificadores Argon2id**, nunca palavras-passe
  ([ADR-0104](0104-password-policy-and-hashing.md));
- as sessões são **server-side, com identificador opaco**, guardadas em
  `sessions` e representadas por um digest SHA-256 — o token nunca é persistido;
- uma credencial temporária gerada por CSPRNG é o **único** modo de um membro
  receber acesso inicial, e expira;
- autenticar com uma credencial temporária produz uma sessão em
  `SessionState::PasswordChangeRequired`, sobre a qual o Core **recusa qualquer
  trabalho normal**;
- o primeiro administrador é criado por um subcomando
  (`ocinye-core-server bootstrap-admin`) que corre uma única vez, e que também
  começa com credencial temporária;
- o Ocinye Workspace deixa de fazer o fluxo OIDC: recolhe credenciais e
  encaminha-as para o Core, que decide.

**MFA passa a `NOT IMPLEMENTED` e `NOT REQUIRED` nesta fase.** É uma redução
real de segurança face ao ADR-0102, assumida explicitamente e compensada por uma
política de palavra-passe mais exigente ([ADR-0104](0104-password-policy-and-hashing.md)).

### O que fica em aberto

A coluna `people.oidc_subject` mantém-se, e o `Principal` continua a ter um campo
`subject`. Federar um IdP no futuro — para SSO com JupyterHub, Forgejo ou outras
ferramentas — não exigirá migração de esquema. O módulo `oidc` do Workspace foi
**removido** por ser código morto (`CLAUDE.md` §53); reintroduzi-lo é trabalho
localizado, e a sua ausência é preferível a mantê-lo a apodrecer sem ser
exercitado.

### Relação com o `CLAUDE.md` §33

O §33 diz «nunca implementes autenticação caseira» e exige um IdP dedicado. Esta
decisão **contraria** essa norma. Não se finge que a cumpre.

O que a mitiga:

- nada de criptografia própria: Argon2id vem de biblioteca madura, e o formato
  PHC é padrão ([ADR-0104](0104-password-policy-and-hashing.md));
- nada de esquemas de sessão inventados: identificador opaco, digest em base de
  dados, cookie `HttpOnly`/`Secure`/`SameSite=Strict`;
- a superfície de credenciais está contida num único módulo
  (`crates/ocinye-core/src/password/`) e num único serviço
  (`modules/identity/authentication.rs`), para poder ser revista como uma peça.

O §33 do `CLAUDE.md` foi actualizado na mesma alteração, com referência a este
ADR, para que a constituição e o repositório não se contradigam
(`CLAUDE.md` §69, §83).

## Alternatives

| Alternativa | Porque foi rejeitada |
|---|---|
| **Manter o ADR-0102 (Keycloak)** | Correcto a prazo; desproporcionado hoje. Exige operar um componente com estado, backup e actualizações antes de existir um único utilizador, e resolve o bootstrap duas vezes. Revisitável quando houver SSO a fazer para várias ferramentas. |
| **Keycloak apenas como store de credenciais, com o Core a orquestrar** | O pior dos dois: continua a exigir operá-lo, e acrescenta um salto de rede a cada verificação, sem retirar do Core a responsabilidade do fluxo. |
| **Passkeys/WebAuthn como factor único agora** | Elimina a palavra-passe, o que é atraente. Rejeitado porque exige recuperação para dispositivo perdido — e a recuperação seria, na prática, uma palavra-passe. Fica `PLANNED`. |
| **Magic links por email** | Move o segredo para o email, cuja infraestrutura a Ocinye também não opera. Troca um problema por outro, com menos controlo. |
| **Autenticação no Core com MFA imediato** | TOTP acrescentaria um segundo factor real, mas também enrolamento, códigos de recuperação e uma segunda via de bootstrap. Adiado deliberadamente para que a fundação seja pequena o suficiente para ser revista por inteiro. |

## Consequences

**Positivas** — um único bootstrap; nenhum componente com estado adicional a
operar; o fluxo institucional real (administrador cria, pessoa define) fica
representado directamente no domínio; `can(actor, permission, contexto)` passa a
ter um `Principal` que o Core montou de ponta a ponta.

**Negativas, aceites** — o Core passa a ter superfície de credenciais, que antes
não tinha: hashing, throttling, expiração, rotação de sessão e um caminho de
recuperação administrativa. **Não há MFA.** Uma palavra-passe comprometida é
acesso comprometido, e é por isso que o mínimo é de 15 caracteres e existe
blocklist. O SSO para ferramentas científicas fica adiado.

**Verificação** — `crates/ocinye-core/tests/identity.rs` corre contra PostgreSQL
real e prova, entre outros, que uma credencial temporária nunca abre uma sessão
normal, que expira, que é consumida ao ser usada, e que nenhuma palavra-passe
aparece em `credentials`, em `audit_events` ou em `authentication_attempts`.
