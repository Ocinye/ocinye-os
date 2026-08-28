# ADR-0408 — O transporte IMAP: cifra obrigatória, pastas descobertas, sessão por operação

- **Estado:** Accepted
- **Domínio:** Mail
- **Impacto:** LOCAL
- **Data:** 2026-08-22
- **Complementa:** [ADR-0401](0401-mail-provider-abstraction.md) · [ADR-0407](0407-mail-index-not-archive.md)

## Context

O [ADR-0401](0401-mail-provider-abstraction.md) decidiu a abstracção de
fornecedor e deixou a leitura IMAP como `PLANNED`. Este ADR regista as decisões
tomadas ao construí-la, para a primeira integração real — um serviço IMAP+SMTP
comercial sobre `ocinye.com`.

Três delas não são detalhes de implementação: mudam o que o sistema garante.

## Decision

### 1. Não existe forma de desligar a cifra

`OCINYE_MAIL_IMAP_TLS` e `OCINYE_MAIL_SMTP_TLS` aceitam `tls` (cifra desde o
primeiro byte, portos 993/465) ou `starttls` (socket em claro promovido, portos
143/587).

**`false`, `none`, `off` e `plain` fazem o Ocinye Core recusar arrancar**, com a
razão escrita. Não são ignorados nem tratados como `starttls`: são rejeitados.

Uma ligação de correio sem cifra envia a password da caixa em claro. Não existe
ambiente — nem desenvolvimento — em que isso seja aceitável, e `CLAUDE.md` §56
proíbe precisamente a configuração permissiva que sobrevive até produção.

Ausente significa *a convenção do porto*, e ambas as convenções cifram.

Um booleano foi recusado: `TLS=true` é ambíguo entre as duas formas de cifrar, e
escolher a errada não falha de forma limpa — ou bloqueia, ou liga em claro.

### 2. O provedor criptográfico é nomeado, não descoberto

A árvore de dependências traz dois — `ring`, via `lettre`, e `aws-lc-rs`, via o
SDK da AWS. O `rustls` recusa-se a adivinhar entre eles e entra em pânico.

O `ClientConfig` é construído com `builder_with_provider(ring)`, explicitamente.
Isto evita também `install_default()`, que é um global de uma só chamada e
competiria com qualquer outra parte do processo a tentar fazer o mesmo.

**Isto foi encontrado por correr o diagnóstico, não por o ler.** Sem
`mail-check`, o primeiro sintoma teria sido um pânico no primeiro pedido de
correio de um membro.

### 3. Os nomes das pastas são perguntados ao servidor

`Sent`, `Sent Items`, `INBOX.Sent`, `Enviados` — todos existem em servidores
reais. `resolve_folder` faz `LIST`, procura por correspondência exacta, depois
por segmento final (para hierarquias como `INBOX.Sent`), e só então cai no nome
convencional.

Fixar `Sent` no código funciona até encontrar um servidor que discorde, e nessa
altura o envio parece funcionar enquanto nada é arquivado — uma falha que
ninguém nota durante semanas.

A caixa de entrada é a excepção: `INBOX` é o único nome em que todos os
servidores concordam, e salta a viagem extra.

### 4. Uma sessão por operação

Uma ligação IMAP longa e reutilizada é mais rápida e consideravelmente mais
difícil de acertar: as sessões carregam estado de pasta seleccionada, os
servidores fecham-nas sem aviso, e uma sessão morta falha de formas que parecem
correio em falta.

Correcção primeiro. Pooling é uma alteração a fazer com uma medição na mão
(`CLAUDE.md` §71), não por antecipação.

### 5. `BODY.PEEK`, nunca `BODY`

Ler uma mensagem no Ocinye Workspace **não** a marca como lida no servidor.
Marcar como lida é uma operação própria, tomada deliberadamente.

O contrário faria a interface alterar estado alheio como efeito secundário de
mostrar alguma coisa.

### 6. A pasta faz parte do endereço de uma mensagem

Um UID de IMAP só é único dentro de uma pasta. `fetch_message`,
`fetch_attachment`, `set_read`, `set_starred` e `move_message` passaram a
receber a `MailFolder`, que o índice já registava.

Antes disto, assumir `INBOX` teria devolvido a mensagem errada — ou nenhuma — ao
abrir algo em `Enviados`.

## Alternatives

**`STARTTLS` para IMAP.** Não implementado, e recusado com uma mensagem clara em
vez de cair para uma sessão sem cifra. Todos os serviços que valem a pena
oferecem 993.

**Descobrir pastas por `SPECIAL-USE` (RFC 6154).** Melhor quando existe, e não
existe em todo o lado. A correspondência por nome cobre os dois casos; ler
`SPECIAL-USE` quando anunciado é uma melhoria futura, não uma condição.

## Consequences

- `mail.sync` deixa de ser `PLANNED` e passa a **`DEGRADED`**: um membro pode
  actualizar uma pasta, e nada a actualiza por ele. Reportá-la como
  `AVAILABLE` descreveria mal o que o sistema faz — correio novo não aparece
  sozinho (`CLAUDE.md` §69).
- Existe `ocinye-core-server mail-check`: prova credenciais sem arrancar o Core
  e sem imprimir nada que não se possa colar num ticket. **Não envia nada** —
  diagnosticar um caminho de saída enviando correio a alguém é como mensagens
  de teste chegam a pessoas reais.
- Uma credencial única por instalação serve para validar a integração. A versão
  multiutilizador exige credenciais por caixa, cifradas em repouso — decisão
  registada como `PLANNED` em [docs/mail/operations.md](../mail/operations.md),
  e que exigirá ADR próprio.
