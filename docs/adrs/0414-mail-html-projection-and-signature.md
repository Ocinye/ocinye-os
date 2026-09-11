# ADR-0414 — Correio: projecção HTML determinística e assinatura institucional

- **Estado:** Accepted
- **Domínio:** Mail
- **Impacto:** MEDIUM
- **Data:** 2026-09-11
- **Complementa:** [ADR-0400](0400-mail-as-institutional-surface.md) · [ADR-0402](0402-mail-html-sanitisation.md)

## Context

Até aqui, o Ocinye Mail **compunha apenas texto simples**. A decisão vinha
escrita no próprio contrato de saída (`OutgoingMessage`): «uma composição que
produzisse HTML tornaria a instituição fonte do mesmo conteúdo que o leitor tem
de desconfiar, e não traz nada que uma mensagem institucional precise». O
raciocínio continua válido para **HTML de autoria** — um compositor de HTML
arbitrário é uma superfície de ataque e um convite ao correio-marketing.

Mas há uma coisa que uma mensagem institucional precisa e que o texto simples não
dá: uma **assinatura institucional com o logótipo da Ocinye**, na própria
mensagem que o destinatário recebe. Uma assinatura de identidade é HTML por
natureza, e uma que degrade para um texto diferente no envio não é a mesma
assinatura.

A questão não é «texto simples ou HTML». É **quem produz o HTML**.

## Decision

O correio passa a enviar **`multipart/alternative`** — `text/plain` **e**
`text/html` — mantendo a autoria em texto simples. O HTML **não é escrito pelo
membro**: é uma **projecção determinística, gerada pelo Ocinye**, do texto que o
membro escreveu.

1. **A autoria continua a ser texto simples.** Não se introduz um compositor de
   HTML. O membro escreve texto; o `text_body` carrega essa escrita, tal e qual.

2. **O `text/plain` é canónico e completo.** É a representação de referência e a
   alternativa de recurso: um cliente que recuse HTML recebe a mensagem inteira,
   assinatura incluída, no texto simples.

3. **O `text/html` é uma projecção determinística.** O texto do membro é
   **escapado** (nada do que ele escreve vira marcação), os parágrafos são
   preservados, e a **assinatura institucional** é acrescentada a partir de dados
   estruturados — nome, cargo (quando factual), instituição, endereço — e do
   logótipo.

4. **HTML arbitrário continua proibido.** Não há caminho para o membro injectar
   marcação na mensagem. A única marcação que sai é a que o Ocinye gera.

5. **O logótipo viaja embutido por `cid:`**, numa parte `multipart/related`, e
   não por URL remoto: a assinatura não faz um pedido a um servidor ao ser aberta
   — o que seria, na prática, um pixel de rastreio ao contrário.

6. **A fronteira de saída não é a de entrada.** O [ADR-0402](0402-mail-html-sanitisation.md)
   limpa HTML **de entrada**, que é conteúdo externo não confiável, e por isso
   descarta estilo em linha e quase tudo. A projecção de **saída** é conteúdo da
   **própria instituição**, determinístico, e usa estilo em linha porque o
   email exige-o. São duas fronteiras de confiança diferentes, e reutilizar o
   sanitizador de entrada na saída seria descartar precisamente a assinatura que
   se quer produzir.

## O que muda no contrato

`OutgoingMessage` deixa de ter um `body` único e passa a distinguir as duas
representações, com as imagens embutidas explícitas:

```
text_body: String            // canónico e completo
html_body: Option<String>    // projecção determinística; None = só texto
inline_images: Vec<InlineImage>   // partes `cid:` (o logótipo)
```

A assinatura só produz `html_body` quando há algo a mostrar (a oficial ligada, ou
uma linha pessoal). Sem nada a acrescentar, a mensagem viaja só como texto — não
se paga uma parte HTML que nada mostra.

## A assinatura

Só factos, e degrada com elegância: um cargo em falta não deixa linha vazia,
desaparece. Não inventa telefone, cargo nem um website que ainda não existe. É
sóbria — sem faixa, sem rodapé de marketing, sem ícones sociais, sem gradientes.

```
[logótipo]  Nome do membro
            Cargo institucional (se existir)
            Ocinye
            email@ocinye.com
```

A **preferência** do membro (`mail_preferences.official_signature`, por omissão
ligada) decide se a assinatura oficial é acrescentada. O campo de assinatura que
já existia (`mail_preferences.signature`) **não se perde**: passa a ser uma
**linha pessoal opcional**, escapada na projecção HTML, acrescentada acima da
assinatura oficial.

## Alternatives

- **Manter só texto simples.** Seguro e honesto, mas não põe o logótipo na
  mensagem que o destinatário recebe — que é o requisito.
- **HTML de autoria (um compositor rico).** Rejeitado: torna a instituição fonte
  de HTML arbitrário, alarga a superfície de ataque, e é o correio-marketing que
  o Ocinye recusa. A projecção determinística dá a assinatura sem nada disto.
- **Logótipo por URL remoto.** Rejeitado: um pedido remoto ao abrir a mensagem é
  um sinal de leitura, o inverso do que o bloqueio de conteúdo remoto protege.
  `cid:` embutido não faz pedido nenhum.

## Consequences

- O adaptador SMTP passa a emitir `multipart/alternative` (+ `related` para o
  logótipo, + `mixed` quando há anexos), gerado de forma determinística.
- Existe uma fronteira de segurança de **saída**, separada da de entrada, com o
  seu próprio teste: nada do texto do membro atravessa como marcação.
- O `mail_preferences.signature`, que era configuração morta — guardado e nunca
  usado no envio —, passa a ser consumido.
- A assinatura tem duas representações coerentes, e o `text/plain` continua
  completo para quem recusa HTML.
