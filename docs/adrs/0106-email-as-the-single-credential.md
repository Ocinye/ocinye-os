# ADR-0106 — O endereço institucional é a credencial única

- **Estado:** Accepted
- **Domínio:** Identity
- **Impacto:** HIGH
- **Data:** 2026-08-27
- **Substitui parcialmente:** [ADR-0103](0103-core-owned-authentication.md), na parte do identificador
- **Complementa:** [ADR-0104](0104-password-policy-and-hashing.md)

## Context

O [ADR-0103](0103-core-owned-authentication.md) trouxe a autenticação para
dentro do Core e escolheu **username + palavra-passe**. A escolha do
identificador nunca foi discutida nesse documento: as alternativas que ele pesou
eram sobre o *mecanismo* — Keycloak, passkeys, magic links, MFA — e nenhuma
sobre *o que uma pessoa escreve na primeira caixa*.

O username ficou por omissão. E, na prática, produziu duas identidades para a
mesma pessoa: `fidel` para entrar, `fidel.monteiro@ocinye.com` para tudo o
resto. Duas coisas que uma pessoa tem de saber sobre si própria, quando uma
chegava.

## Decision

**O endereço institucional é o identificador de autenticação, e o único.**

O Ocinye deixa de ter username. Não como campo escondido, não como alternativa
aceite em silêncio, não como coluna reservada para o caso de: sai da base, sai
dos contratos, sai dos ecrãs e sai da linha de comandos.

### 1. Porque um identificador e não dois

Porque um sistema que aceita dois identificadores tem duas superfícies de
autenticação, dois sítios onde uma conta se pode duplicar, e duas respostas
para «quem é esta pessoa». O endereço já é o que a instituição usa para tudo o
resto — assinar, receber, atribuir, citar. Ter um segundo nome só para entrar é
guardar uma coisa a mais sobre cada pessoa sem que ela sirva para nada.

### 2. Porque o endereço, e não o username

Porque o endereço já existe, já é único na instituição, já é conhecido de quem
o tem, e já é o que uma pessoa escreve quando lhe perguntam quem é. O username
era um segundo nome que só existia dentro do Ocinye.

### 3. O que isto **não** decide

Não decide o mecanismo. A palavra-passe continua a ser a palavra-passe, com o
Argon2id do [ADR-0104](0104-password-policy-and-hashing.md). Passkeys continuam
`PLANNED`, e MFA continua adiado. O que muda é o que se escreve na primeira
caixa.

E não toca no correio. A conta com que o adaptador de IMAP se autentica —
`OCINYE_MAIL_USERNAME` — e o nome de utilizador que cada membro guarda ao ligar
a sua caixa ([ADR-0409](0409-mailbox-credentials-per-member.md)) são
credenciais de **outro sistema**, que o Ocinye guarda e apresenta. Continuam
onde estão, com o nome que têm, porque quem as define não é o Ocinye.

### 4. A limitação de tentativas passa a contar por endereço

O `authentication_attempts` guardava o username tal como foi apresentado, para
travar tentativas repetidas contra uma conta. Passa a guardar o endereço, pela
mesma razão e com a mesma forma.

### 5. Um endereço que muda é a mesma pessoa

O identificador da pessoa continua a ser o seu `id`, e não o endereço. Alterar
um endereço institucional não cria uma conta nova, não perde sessões por si, e
não é uma operação de identidade nova — é a mesma que já existia.

## Alternatives

**Manter os dois, e aceitar qualquer um.** É o que a maioria dos sistemas faz.
Rejeitada porque é precisamente o estado que produziu o problema: duas coisas a
identificar a mesma pessoa, e nenhuma delas claramente *a* certa. Aceitar os
dois não simplifica — duplica.

**Manter o username e esconder o endereço.** O inverso, e pior: obriga a
inventar um nome para cada pessoa que entra, e esse nome não serve para mais
nada.

**Manter a coluna, vazia, para o caso de.** Rejeitada. Uma coluna que ninguém
escreve é uma coluna que alguém acaba por escrever — e a partir daí há duas
maneiras de entrar outra vez, sem que nada o tenha decidido.

## Consequences

**O que melhora.** Uma pessoa tem uma credencial. O ecrã de entrada tem uma
caixa com um nome que se entende. O gestor de palavras-passe do browser guarda
a conta com o endereço, que é o que ele espera. E deixa de existir a pergunta
«qual era o meu utilizador?».

**O que se perde.** A possibilidade de entrar sem revelar o endereço. Num
sistema institucional fechado isso não é privacidade: quem já está lá dentro
conhece os endereços dos colegas, porque lhes escreve.

**O que fica por decidir.** Se um dia houver identidades federadas de fora da
Ocinye, o endereço pode deixar de ser único no universo. Nessa altura o
identificador volta a ser uma decisão — e é o `id` que a torna possível sem
migrar nada.

## Referências

ADR-0103 · ADR-0104 · ADR-0409 · `CLAUDE.md` §22
