# Política de palavras-passe

Documento canónico. Decisão: [ADR-0104](../adrs/0104-password-policy-and-hashing.md).
Contexto: [ADR-0103](../adrs/0103-core-owned-authentication.md) tornou a
palavra-passe o **único factor** de autenticação.

Tudo o que se segue é imposto **no Ocinye Core**. O front-end ajuda; não decide
(`CLAUDE.md` §31).

## Resumo

| Regra | Valor |
|---|---|
| Comprimento mínimo | **15 caracteres** |
| Comprimento máximo | **256 caracteres** |
| Limite de bytes aceite no pedido | 4096 |
| Maiúsculas / dígitos / símbolos obrigatórios | **Nenhum** |
| Espaços | Aceites, e significativos |
| Unicode | Aceite; normalização **NFC**, e nada mais |
| Passphrases | Encorajadas |
| Blocklist | Local, versionada, embutida no binário |
| Rotação periódica | **Não existe** |
| Hashing | Argon2id, formato PHC |
| Pepper | Não, nesta fase — ver ADR-0104 |
| MFA | **`NOT IMPLEMENTED`** |

## Porque 15

Sem segundo factor, a palavra-passe é a totalidade da defesa contra credential
stuffing, password spraying e ataque offline a um dump da base de dados. Quinze
caracteres com Argon2id põem uma frase curta fora do alcance realista.

Se alguma vez existir MFA, este número é revisitável — **por ADR**, e para baixo
só com razão escrita.

## Porque não há regras de composição

Exigir «pelo menos uma maiúscula, um dígito e um símbolo» produz `Password1!` e
afasta as pessoas das frases longas, que são o que resiste de facto. Números e
símbolos são bem-vindos; obrigatórios, não.

## Normalização: exactamente uma transformação

Antes de calcular o verificador aplica-se **Unicode NFC**. Nada mais.

Existe porque a mesma frase escrita em teclados diferentes produz bytes
diferentes — `é` como um ponto de código, ou `e` seguido de acento combinante — e
sem isto o segundo início de sessão falharia sem explicação possível.

**O que não acontece, nunca:**

- não há `trim` — espaços à frente e atrás fazem parte da palavra-passe;
- não há mudança de caixa;
- não há truncagem.

Rejeitou-se **NFKC**: é uma dobragem de compatibilidade que colapsa caracteres
distintos e reduz entropia sem o titular saber.

## Blocklist

`crates/ocinye-core/src/password/blocklist.txt`, embutida no binário e revista
periodicamente (data no cabeçalho do ficheiro).

Antes de comparar, o candidato é canonicalizado — **por esta ordem**:

1. minúsculas;
2. remoção de dígitos e pontuação **finais**;
3. dobragem de substituições (`0`→`o`, `1`→`i`, `3`→`e`, `4`/`@`→`a`, `5`/`$`→`s`, `7`→`t`).

A ordem importa e falhar nela é silencioso: dobrar antes de cortar transforma o
`123` de `Password123` em letras, que o cortador já não remove, e a entrada
deixa de casar.

Para lá da lista, são recusados por código — porque são infinitos:

| Padrão | Exemplo |
|---|---|
| Unidade repetida | `abcabcabcabcabc` |
| Carácter repetido | `aaaaaaaaaaaaaaaa` |
| Percurso de teclado | `qwertyuiopasdfgh` |
| Nome da instituição | `ocinye`, `0c1nye`, `ocinye-os` |

**Nenhuma palavra-passe sai da instituição**, em nenhuma forma. Consultar uma base
externa de credenciais comprometidas — por k-anonimato, único formato aceitável —
é `PLANNED` e exige ADR próprio.

## Sem rotação periódica

Forçar troca a cada 90 dias produz `Verao2026`, `Verao2027`. Só se força mudança
quando há razão:

- a credencial ainda é temporária;
- um administrador fez reset;
- há evidência de compromisso;
- política excepcional, escrita e justificada.

## Credenciais temporárias

| | |
|---|---|
| Geração | CSPRNG do sistema operativo |
| Formato | 6 grupos de 4, separados por hífen — `ZhAS-sXJz-CDux-QgHq-BuFL-37Hn` |
| Alfabeto | 55 símbolos, **sem** `0`/`O` nem `1`/`l`/`I` |
| Entropia | ≈ **139 bits** |
| Validade | **24 horas** por omissão (`OCINYE_TEMPORARY_CREDENTIAL_HOURS`) |
| Apresentação | **Uma única vez**, ao administrador que a emitiu |
| Reutilização | Impossível: é consumida ao definir a palavra-passe |

O alfabeto exclui caracteres ambíguos porque estas credenciais são lidas de um
ecrã e ditadas ao telefone. O administrador **não** as inventa: uma «temporária»
escolhida por uma pessoa é uma permanente à espera.

A expiração é avaliada na verificação, não confiada a varredura.

## Primeiro acesso

```
login com credencial temporária
  ↓
sessão `password_change_required`   ← nenhuma API normal responde
  ↓
POST /auth/password
  ↓
valida · consome a temporária · revoga TODAS as sessões · emite sessão nova
  ↓
Ocinye Workspace
```

A nova palavra-passe **não pode** ser a credencial temporária, nem a palavra-passe
actual. Ambas as comparações são feitas contra o **verificador**: o Core nunca
recebe uma palavra-passe para comparar com outra.

## Hashing

```
$argon2id$v=19$m=19456,t=2,p=1$<salt>$<hash>
```

| Parâmetro | Omissão | Variável | Piso |
|---|---|---|---|
| Memória | 19 MiB | `OCINYE_ARGON2_MEMORY_KIB` | 8 MiB |
| Iterações | 2 | `OCINYE_ARGON2_ITERATIONS` | 2 |
| Paralelismo | 1 | `OCINYE_ARGON2_PARALLELISM` | 1 |

Os pisos são validados **no arranque, em todos os ambientes**. Um Core que não
consegue calcular verificadores em condições recusa-se a arrancar — um
verificador fraco escrito em desenvolvimento sobrevive até à primeira produção.

### Calibração

O custo certo depende da máquina. Alvo: **≈ 500 ms** por verificação no hardware
de produção, medido com a concorrência esperada.

```bash
cargo test -p ocinye-core --release password::hashing -- --nocapture
```

Subir os parâmetros **não** invalida palavras-passe: o formato PHC transporta os
parâmetros com o hash, e `needs_rehash` substitui os antigos em silêncio no
início de sessão seguinte.

## Throttling

| Sinal | Omissão | Variável |
|---|---|---|
| Falhas por origem de rede | 20 | `OCINYE_THROTTLE_PER_IP` |
| Falhas por conta | 10 | `OCINYE_THROTTLE_PER_USERNAME` |
| Janela | 15 minutos | `OCINYE_THROTTLE_WINDOW_MINUTES` |

**Não é bloqueio de conta.** Bloquear ao fim de N falhas entrega a quem souber um
nome de utilizador uma negação de serviço contra essa pessoa. A recusa expira
sozinha.

Falhas são registadas em `authentication_attempts` com o nome de utilizador, a
origem e o desfecho. **Nunca** com a palavra-passe, o seu hash ou o seu
comprimento.

## Recuperação

Nesta fase, **administrativa**. Não há auto-serviço.

O membro pede ajuda por processo institucional; um administrador autorizado emite
nova credencial temporária, o que revoga a palavra-passe anterior e todas as
sessões.

**Não existem, e não devem passar a existir, perguntas de segurança.** «Nome da
mãe» é um segredo partilhado com toda a gente que conhece a pessoa.

Runbook: [Repor a palavra-passe de um membro](../runbooks/reset-member-password.md).

## O que nunca acontece

- Uma palavra-passe é armazenada em claro. Em lado nenhum, em momento nenhum.
- Um administrador consulta a palavra-passe de outro membro.
- Uma palavra-passe viaja em query string, URL ou log.
- Colar é bloqueado num campo de palavra-passe.
- Um campo de palavra-passe tem `maxlength` que trunque em silêncio.
- A resposta de início de sessão distingue as suas causas de falha.
