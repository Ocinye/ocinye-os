# ADR-0415 — Correio: HTML de autoria com fronteira de saída própria

- **Estado:** Accepted
- **Domínio:** Mail
- **Impacto:** HIGH
- **Data:** 2026-09-12
- **Complementa:** [ADR-0402](0402-mail-html-sanitisation.md) · [ADR-0414](0414-mail-html-projection-and-signature.md)

## Context

O [ADR-0414](0414-mail-html-projection-and-signature.md) decidiu que o HTML de
saída **não é escrito pelo membro**: era uma projecção determinística e escapada
do texto simples, mais a assinatura institucional. O raciocínio — «um compositor
de HTML arbitrário é uma superfície de ataque e um convite ao correio-marketing»
— continua verdadeiro para **HTML arbitrário**.

Mas um compositor de correio de produção precisa de **formatação**: negrito,
itálico, listas, ligações, citação. Um membro que escreve à instituição espera
poder enfatizar uma frase ou enumerar três pontos, como em qualquer cliente
moderno. Negar-lho não é uma postura de segurança — é uma lacuna de produto que
empurra as pessoas para fora do Ocinye Mail.

A questão que o ADR-0414 formulou — **quem produz o HTML** — mantém-se a certa. O
que muda é a resposta: o HTML pode ser de autoria **se atravessar uma fronteira
de confiança de saída que o reduza a um subconjunto seguro e institucional**.
«HTML arbitrário» e «formatação de autoria higienizada» não são a mesma coisa, e
tratá-las como se fossem é que seria o erro.

Há duas fronteiras distintas, e confundi-las é perigoso nos dois sentidos:

- **Entrada** ([ADR-0402](0402-mail-html-sanitisation.md)): HTML externo, não
  autenticado, potencialmente hostil. Higienizado por lista de permissões
  (`ammonia`), com política de conteúdo remoto, `cid:`, imagens, tabelas — um
  conjunto largo, porque o objectivo é **mostrar** correio recebido tal como foi
  enviado, sem executar nada.
- **Saída** (este ADR): HTML composto por um membro autenticado, para sair em
  nome da instituição. O objectivo é **produzir** uma mensagem sóbria, e o
  subconjunto é mais **estreito** do que o de entrada — sem imagens remotas, sem
  tabelas de maquetagem, sem estilos inline, sem `cid` de autoria. Reutilizar o
  higienizador de entrada aqui seria errado: as suas suposições de confiança são
  outras, e o seu conjunto largo deixaria passar o que uma mensagem de saída não
  deve conter.

## Decision

O correio passa a aceitar **HTML de autoria do membro**, sujeito a uma
**fronteira de saída própria e estrita**, distinta da de entrada:

1. **Sanitização de saída obrigatória.** Todo o HTML composto pelo membro passa
   por `mail::outbound::sanitize_outbound` antes de se tornar MIME. É uma lista
   de permissões `ammonia` independente da de entrada, com um subconjunto
   **semântico**: `p`, `br`, `b`/`strong`, `i`/`em`, `u`, `s`, `ul`/`ol`/`li`,
   `blockquote`, `a[href]` (só `http`/`https`/`mailto`), e cabeçalhos leves.
   **Retira** `script`, `style`, `iframe`, `object`, `embed`, `svg`, `form`,
   atributos `style`, `class`, `id`, `on*`, e qualquer `src` remoto. A formatação
   sobrevive por **tags semânticas**, não por estilo inline — que os clientes de
   email descartam de qualquer forma.

2. **O servidor é a autoridade.** A higienização do cliente é conveniência de
   UX; a decisão é do Core. O que o cliente enviar é re-higienizado no servidor,
   e é o resultado disso que vai para o fio.

3. **Alternativa `text/plain` credível.** Toda a mensagem HTML leva uma parte
   `text/plain` — a projecção de texto do próprio membro (o `innerText` do
   editor) —, e a assinatura entra nas **duas** partes. Uma mensagem nunca é
   só-HTML.

4. **A assinatura continua projecção do Core.** O ADR-0414 mantém-se para a
   assinatura: o membro nunca edita o HTML institucional. O HTML de autoria é o
   **corpo**; a assinatura é acrescentada por baixo, gerada de dados estruturados.

5. **Texto simples continua a ser um caminho de primeira classe.** Uma mensagem
   sem formatação não produz HTML de autoria: segue o caminho do ADR-0414
   (projecção determinística do texto). O HTML só existe quando o membro formatou.

Isto **revê** o ponto do ADR-0414 que dizia «o HTML nunca é de autoria»: passa a
ser, através desta fronteira. Tudo o resto do ADR-0414 permanece.

## Alternatives

- **Manter só texto simples (ADR-0414 como estava).** Rejeitado: é a lacuna de
  produto que este milestone existe para fechar.
- **Reutilizar o higienizador de entrada.** Rejeitado: as suposições de confiança
  diferem, e o conjunto largo de entrada (imagens, tabelas, `cid`) não pertence a
  uma mensagem de saída. Uma fronteira que serve os dois sentidos serve mal os
  dois.
- **Confiar na higienização do cliente.** Rejeitado: nenhum controlo do cliente
  é uma fronteira de segurança (briefing §49). O cliente higieniza para dar
  resposta imediata; o servidor decide.
- **Guardar estilos inline para maquetagem rica.** Rejeitado: estilo inline é a
  porta de mais problemas de segurança e de renderização, e os clientes de email
  descartam-no. A formatação semântica é suficiente e sóbria.

## Consequences

- Existe `crates/ocinye-core/src/modules/mail/outbound.rs`, com
  `sanitize_outbound`, provado por testes de que retira `script`/`style`/`on*`/
  esquemas perigosos e preserva a formatação semântica.
- `OutgoingMessage.html_body` passa a poder trazer HTML de autoria já
  higienizado; o caminho de envio deriva a parte `text/plain` do texto do membro
  e acrescenta a assinatura às duas partes.
- O rascunho ganha `body_html` (migração 0042): a formatação sobrevive a um
  recarregamento e à retoma.
- A distinção entrada/saída fica documentada e testada, para que uma fusão futura
  das duas não aconteça por descuido.
- Zero dependência de IA: nada aqui precisa de inferência.
