# ADR-0107 — MFA obrigatório para identidades privilegiadas, sessões e recuperação

- **Estado:** Accepted
- **Domínio:** Identity
- **Impacto:** FOUNDATIONAL
- **Data:** 2026-09-08
- **Substitui parcialmente:** [ADR-0103](0103-core-owned-authentication.md), na parte que dizia MFA `NOT REQUIRED`
- **Complementa:** [ADR-0104](0104-password-policy-and-hashing.md) · [ADR-0100](0100-authorization-model.md) · [ADR-0700](0700-institutional-continuity-and-portability.md) · [ADR-0401](0401-mail-provider-abstraction.md)

## Context

O [ADR-0103](0103-core-owned-authentication.md) trouxe a autenticação para o
Core com um único factor — palavra-passe — e declarou MFA `NOT REQUIRED` *nesta
fase*, deixando escrito que exigi-lo seria uma decisão com ADR próprio. Esta é
essa decisão.

Entretanto o [ADR-0108-equivalente da identidade privilegiada](../../migrations/0028_linked_privileged_identity.sql)
ligou cada conta administrativa a uma pessoa humana e deu-lhe `PlatformAdmin`.
Ou seja: hoje, **uma palavra-passe sozinha estabelece autoridade privilegiada**.
Uma palavra-passe comprometida de uma conta `PlatformAdmin` é a instituição
comprometida — sem nós, sem IA, sem correio configurado, o que a Ocinye já tem a
proteger é exactamente o acesso administrativo.

Ao mesmo tempo, a gestão de sessões está incompleta: uma sessão só se revoga
como **consequência** (suspender, repor palavra-passe revogam todas), e não há
forma de um administrador matar **uma** sessão de um membro — o caso «perdi um
portátil» é diferente de «invalidar tudo», e o produto final precisa dos dois.

Autenticação forte, códigos de recuperação e revogação de sessões são a mesma
fronteira de segurança. Tratam-se aqui juntos.

## Decision

### 1. MFA é obrigatório para identidades privilegiadas

A política é por **classe**, resolvida da autoridade do próprio principal:

```
identity_kind = privileged   OU   PlatformAdmin efectivo
→ MFA obrigatório
```

Uma palavra-passe, por si só, **não pode** estabelecer uma sessão que trabalhe
com autoridade privilegiada. Depois de a palavra-passe ser aceite, a sessão
nasce num estado que não permite trabalho ordinário até o segundo factor ser
satisfeito. As duas dimensões continuam independentes ([ADR-0100](0100-authorization-model.md)):
dar `PlatformAdmin` a um humano não o torna privilegiado, e revogar-lho não o
torna normal — mas **qualquer** das duas condições activa a exigência de MFA.

Membros sem autoridade privilegiada **podem** enrolar MFA (é-lhes permitido,
não exigido). A obrigatoriedade alarga-se com a classe, não com a vontade.

### 2. TOTP: o seed é selado, não resumido

O segundo factor é **TOTP** (RFC 6238, HMAC-SHA1, 30 s, 6 dígitos, janela de
±1 passo para tolerância de relógio). O *seed* tem de ser recuperável para
validar códigos futuros — logo **sela-se**, não se resume. Guarda-se cifrado
(criptograma), nunca em claro, nunca em log, nunca exportável em claro.

Uma conta privilegiada com MFA obrigatório mas **sem** seed enrolado entra num
estado de enrolamento: a primeira coisa que faz é enrolar. O enrolamento
apresenta o segredo em base32 e um `otpauth://` para QR, **uma única vez**, e só
se conclui quando um código válido o confirma — provar que o autenticador do
membro já gera o código certo antes de fechar a porta atrás dele.

### 3. Códigos de recuperação: verificadores, uso único, consumo atómico

No fim do enrolamento emitem-se **códigos de recuperação**, mostrados **uma
única vez**. Guarda-se apenas o **verificador** (Argon2id, a infra do
[ADR-0104](0104-password-policy-and-hashing.md)) — nunca o código, nunca cifrado
para leitura posterior. Cada código é de **uso único**, consumido
**atomicamente** (um `UPDATE ... WHERE state='active'` condicional, para que dois
pedidos concorrentes não gastem o mesmo). Regenerar códigos invalida os
anteriores. Um código de recuperação satisfaz o desafio quando o autenticador
não está à mão; não substitui o enrolamento.

### 4. Fail closed

MFA desconhecido, indisponível ou por verificar → **negar** o trabalho
privilegiado. Se o material de selagem não estiver disponível (§7), uma conta
privilegiada **não consegue** enrolar nem validar, e por isso **não** obtém
sessão privilegiada — a instituição prefere um administrador temporariamente
fechado de fora a um administrador autenticado por um factor só.

Isto não enfraquece a salvaguarda do último administrador
([docs/authorization](../authorization/README.md)): continua a ser impossível
suspender, desactivar ou despojar do papel o último `PlatformAdmin` capaz de
entrar. MFA fail-closed e essa salvaguarda são ortogonais.

### 5. Estado de sessão para o portão de MFA

`SessionState` ganha um estado antes de `Active`: uma sessão de uma identidade
com MFA obrigatório, cuja palavra-passe foi aceite mas cujo segundo factor ainda
não, **não permite trabalho ordinário**. O portão vive na fronteira central
(`CurrentPrincipal`), ao lado do que já recusa `PasswordChangeRequired` — nunca
no cliente. Como em toda a parte, uma sessão não se transiciona no lugar:
satisfazer o factor revoga a sessão do portão e cria uma `Active` no seu lugar,
o mesmo padrão que `set_password` já usa.

### 6. Revogação individual de sessão é uma operação de domínio governada

Revogar **uma** sessão de um membro passa a ser uma operação explícita
(`RevokeMemberSession`), não um efeito colateral e não um endpoint silencioso:

- **Autoridade fresca:** reautoriza o actor como `PlatformAdmin` no momento.
- **Posse do alvo:** a sessão indicada **tem de** pertencer ao membro-alvo;
  caso contrário `NotFound` — indistinguível de não existir (anti-IDOR), o mesmo
  princípio de `revoke_own_session`.
- **Audit próprio:** regista `member_session_revoked`. Isto abre uma excepção
  deliberada à nota em `audit.rs` que dizia não haver acção de «sessão
  revogada»: essa nota falava da revogação como *consequência* (a causa levava a
  contagem); um administrador a matar a sessão de **outra** pessoa é um acto por
  direito próprio, com actor, alvo e sessão, e é isso que a acção regista.
- **Só metadata segura** sai para a interface: id/identificador de exibição,
  `created_at`, `last_seen`, cliente/dispositivo quando já existir, estado.
  Nunca token, nunca cookie, nunca credencial.

«Revogar todas» continua a existir pelas operações de lifecycle (suspender,
repor palavra-passe) quando apropriado. Uma sessão revogada não executa a
operação seguinte — reautorização em tempo de execução ([ADR-0411](0411-execution-time-principal-freshness.md)).

### 7. Uma raiz institucional de selagem; uma subchave por domínio (HKDF)

O TOTP precisa de material de selagem. O sistema tinha **uma** chave
(`OCINYE_MAIL_KEY`, ChaCha20-Poly1305), por decisão deliberada e com teste que a
fixa. Selar o seed TOTP com uma chave chamada «mail» faria o nome mentir; uma
segunda chave-raiz duplicaria a história de continuidade ([ADR-0700](0700-institutional-continuity-and-portability.md)).
A decisão:

> **Uma chave-raiz institucional de selagem; uma subchave criptograficamente
> separada por domínio para cada classe de segredo.**

- A raiz passa a chamar-se **`OCINYE_SEALING_KEY`** — renomeação de
  `OCINYE_MAIL_KEY`, **mesmo valor de segredo**. É generalização semântica, não
  rotação.
- Nunca se usa a raiz directamente. Derivam-se subchaves por **HKDF-SHA256**,
  com labels versionadas e explícitas:

  ```
  ocinye/sealing/mail/v1        → subchave efectiva do Correio
  ocinye/sealing/mfa-totp/v1    → subchave efectiva do TOTP
  ```

- Correio e MFA **nunca** partilham subchave efectiva. As subchaves são
  runtime-only: nunca persistidas, nunca em log, nunca exportadas.
- O criptograma passa a ser **auto-descritivo**: leva uma versão de esquema, para
  distinguir o formato antigo (chave directa) do novo (subchave derivada) **sem
  adivinhar**. A raiz continua a ser **um** item durável na continuidade.
- **Não** se introduz `OCINYE_MFA_KEY`. Quando existir KMS/HSM ou separação real
  de serviços, cada domínio poderá ter raiz própria; hoje ambas viveriam no
  mesmo Core/VPS e o ganho de isolamento não paga a complexidade.

**Segurança da migração.** Produção tem **zero** criptogramas de correio (o
correio está `NOT CONFIGURED`), pelo que passar o correio à subchave derivada
não torna nada ilegível. O formato versionado garante que, se algum criptograma
antigo existir num dia, é lido pelo caminho legado e re-selado explicitamente —
nunca um bulk-reseal sem salvaguarda. Na configuração, `OCINYE_SEALING_KEY` tem
precedência; um fallback a `OCINYE_MAIL_KEY` existe **apenas** para a release
coordenada de migração; ambos presentes com valores diferentes → **FAIL
CLOSED**; o fallback é removido depois de produção migrada e verificada.

## Alternatives

- **MFA opcional / recomendado.** Rejeitada: uma recomendação não protege o
  acesso privilegiado; a palavra-passe continuaria a bastar.
- **WebAuthn/passkeys em vez de TOTP.** Desejável, e continua `PLANNED`. TOTP é o
  mínimo que não exige hardware nem registo de dispositivo e funciona hoje; a
  arquitectura de sessão e de recuperação aqui decidida serve os dois.
- **Guardar o seed TOTP resumido (hash).** Impossível: TOTP exige o segredo para
  recalcular o código. Selar é a única via correcta.
- **`OCINYE_MFA_KEY` dedicada (segunda raiz).** Rejeitada agora (§7): duplica
  continuidade e rotação sem isolamento operacional real enquanto tudo vive no
  mesmo host.
- **Revogação de sessão como endpoint sem audit.** Rejeitada explicitamente: o
  CLAUDE.md proíbe o atalho; a operação tem de ser um conceito legítimo do
  domínio, com audit, posse e autoridade.

## Consequences

- Uma migração (`0029`) acrescenta: o seed TOTP selado, os verificadores de
  recuperação, e o estado de enrolamento/desafio. O `SessionState` ganha um
  estado e o CHECK de `sessions` acompanha.
- O módulo `password::sealed` passa a raiz+HKDF+versão; o correio deriva a sua
  subchave. A continuidade (`continuity/keys.rs`, `manifest.rs`) declara a nova
  tabela selada e mantém **uma** raiz durável.
- Uma dependência nova de TOTP entra ao abrigo do [ADR-0004](0004-rust-first.md)
  e do §54; primitivas HKDF/HMAC vêm de bibliotecas maduras (RustCrypto) — nunca
  KDF nem cripto caseira ([§38](../../CLAUDE.md)).
- **Operação:** o deploy desta fatia exige `OCINYE_SEALING_KEY` no VPS (mesmo
  valor do actual `OCINYE_MAIL_KEY`). Sem ele, contas privilegiadas ficam
  fail-closed. `Fidel Admin` real faz o enrolamento **pelo browser** depois do
  deploy; nenhum seed ou código de recuperação passa por chat ou por quem opera
  a ferramenta.
- CLAUDE.md §33 e §1, o ADR-0103 e `docs/feature-status/` deixam de poder dizer
  «MFA = NOT IMPLEMENTED / NOT REQUIRED» — actualizam-se na mesma alteração.
