# ADR-0104 — Política de palavras-passe e armazenamento de verificadores

- **Estado:** Accepted
- **Domínio:** Identity
- **Impacto:** MEDIUM
- **Data:** 2026-08-22
- **Depende de:** [ADR-0103](0103-core-owned-authentication.md)

## Context

O [ADR-0103](0103-core-owned-authentication.md) tornou a palavra-passe o **único
factor** de autenticação do Ocinye OS. Isso muda o cálculo: sem segundo factor,
a resistência da palavra-passe é a totalidade da defesa contra credential
stuffing, password spraying e ataque offline a um dump da base de dados.

Faltava decidir o que é uma palavra-passe aceitável e como é guardada.

## Decision

### Política

| Regra | Valor | Porquê |
|---|---|---|
| Comprimento mínimo | **15 caracteres** | Único factor. Quinze põe uma frase curta fora do alcance de ataque offline realista com Argon2id. |
| Comprimento máximo | **256 caracteres** | Muito acima dos 64 exigidos. O limite existe só para que um chamador não autenticado não faça o servidor correr Argon2 sobre um megabyte. |
| Composição | **nenhuma regra** | Exigir maiúscula/dígito/símbolo produz `Password1!` e afasta das frases longas que resistem de facto. |
| Espaços e Unicode | **aceites** | Uma passphrase com espaços e acentos é exactamente o que se quer encorajar. |
| Rotação periódica | **não** | Forçar troca a cada 90 dias produz `Verao2026`, `Verao2027`. Só se força mudança por credencial temporária, reset administrativo ou evidência de compromisso. |
| Blocklist | **local, versionada** | Ver abaixo. |

### Normalização

Aplica-se **exactamente uma** transformação antes de calcular o verificador:
**Unicode NFC**.

Existe porque a mesma passphrase escrita em teclados diferentes produz sequências
de bytes diferentes — `é` como um ponto de código, ou `e` seguido de acento
combinante — e sem normalização a segunda tentativa de início de sessão falharia
sem explicação possível.

**Nada mais é alterado.** Não há `trim`, não há mudança de caixa, não há
truncagem. Espaços à frente e atrás fazem parte da palavra-passe. Isto está
documentado, e não é silencioso (`CLAUDE.md` §34, briefing §34).

Rejeitou-se **NFKC**, que a NIST SP 800-63B também admite: NFKC é uma dobragem
de compatibilidade e colapsa caracteres distintos (`ﬁ` → `fi`, dígitos de largura
total → ASCII), reduzindo entropia sem que o utilizador o saiba. NFC é
canonicamente equivalente e não perde nada.

### Blocklist

Uma lista local, versionada em `crates/ocinye-core/src/password/blocklist.txt`,
embutida no binário. A comparação **canonicaliza** o candidato: minúsculas,
remoção de dígitos e pontuação finais, e dobragem das substituições habituais
(`0`→`o`, `1`→`i`, `4`→`a`…), por esta ordem. `Password123` e `p4ssw0rd`
colapsam ambos em `password`.

Para lá da lista, são recusados por código: unidades repetidas
(`abcabcabcabcabc`), percursos de teclado (`qwertyuiopasdfgh`) e o nome da
instituição, porque são infinitos e não se listam.

**Nenhuma palavra-passe é enviada para fora**, em nenhuma forma. Consultar uma
base externa de credenciais comprometidas — por k-anonimato, único formato
aceitável — fica `PLANNED`, e exigirá ADR próprio: faria a definição de
palavra-passe depender de um terceiro estar acessível.

### Hashing

**Argon2id**, através da crate `argon2`, guardado em **formato PHC**:

```
$argon2id$v=19$m=19456,t=2,p=1$<salt>$<hash>
```

- salt único por hash, do CSPRNG do sistema operativo;
- parâmetros por omissão segundo a recomendação OWASP — m=19 MiB, t=2, p=1;
- parâmetros **configuráveis** (`OCINYE_ARGON2_*`), com pisos validados **no
  arranque, em todos os ambientes**: um Core que não consegue calcular
  verificadores em condições recusa-se a arrancar;
- o formato PHC transporta os parâmetros **com** o hash, o que torna possível
  `needs_rehash`: depois de uma verificação bem-sucedida, um verificador
  guardado com parâmetros mais fracos é substituído em silêncio, sem obrigar
  ninguém a repor a palavra-passe.

Uma constraint da base de dados recusa qualquer verificador que não comece por
`$argon2id$`.

### Pepper: não, por agora

Um pepper do lado do servidor defende contra fuga **apenas** da base de dados —
uma ameaça real. Não é adoptado nesta fase por uma razão concreta: só compensa
enquanto o pepper vive fora do mesmo backup e do mesmo host que a base, e a
Ocinye tem hoje um host, um procedimento de backup e nenhuma estratégia de
secrets em produção (`CLAUDE.md` §1). Introduzi-lo agora acrescentaria um
problema de rotação sem a separação que o justifica.

A porta fica aberta ao custo de nada: com PHC e `needs_rehash`, adoptar um
pepper mais tarde é mudar um parâmetro e deixar o rehash acontecer sozinho nos
inícios de sessão seguintes.

### Credenciais temporárias

Geradas por CSPRNG, 24 caracteres de um alfabeto de 55 símbolos — cerca de
**139 bits** de entropia, muito acima dos «20–24 caracteres aleatórios» pedidos.
O alfabeto exclui `0`/`O` e `1`/`l`/`I`, porque estas credenciais são lidas de um
ecrã e ditadas ao telefone.

Validade por omissão de **24 horas**, configurável
(`OCINYE_TEMPORARY_CREDENTIAL_HOURS`). A expiração é avaliada **na verificação**,
não confiada a uma varredura: uma linha pode estar `active` para lá da validade e
tem de ser recusada na mesma.

## Alternatives

| Alternativa | Porque foi rejeitada |
|---|---|
| **bcrypt** | Maduro e bem compreendido, mas trunca aos 72 bytes — incompatível com passphrases longas — e não é resistente a memória. |
| **scrypt** | Aceitável; Argon2id é o recomendado actual e tem melhor resistência a ataque por canais laterais no modo híbrido. |
| **PBKDF2** | Só se justifica onde há requisito de conformidade que o exija. Não é o caso. |
| **Mínimo de 12 caracteres** | Seria defensável **com** segundo factor. Sem MFA, quinze é o mínimo honesto. |
| **Regras de composição** | Produzem palavras-passe piores e mais difíceis de memorizar. Explicitamente rejeitadas pelo briefing §6. |
| **Corpus completo de fugas (10⁷ entradas)** | Com mínimo de 15 caracteres, a cauda longa do corpus é praticamente inalcançável. Aumentaria o binário em dezenas de MB para apanhar quase nada mais do que a lista curta e as regras estruturais já apanham. |

## Consequences

**Positivas** — os parâmetros de custo podem subir sem repor palavras-passe;
uma palavra-passe nunca é truncada nem alterada de forma irreprodutível; a
política é aplicada **server-side** e o front-end apenas ajuda.

**Negativas, aceites** — sem pepper, um dump da base de dados permite ataque
offline, limitado apenas pelo custo do Argon2id e pelo mínimo de 15 caracteres.
A blocklist é pequena e envelhece: precisa de revisão periódica, registada em
runbook.

**Verificação** — `crates/ocinye-core/src/password/` tem testes que provam,
entre outros: que 14 caracteres são recusados e 15 aceites; que uma passphrase de
64 caracteres passa; que espaços à frente e atrás sobrevivem; que as formas
composta e decomposta de uma palavra normalizam para os mesmos bytes; que o mesmo
segredo produz hashes diferentes; que um verificador malformado se comporta como
palavra-passe errada em vez de rebentar; e que subir o custo marca hashes
antigos para rehash sem os invalidar.
