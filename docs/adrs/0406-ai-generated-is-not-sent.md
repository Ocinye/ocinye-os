# ADR-0406 — Texto gerado não é mensagem enviada

- **Estado:** Accepted
- **Domínio:** Mail
- **Impacto:** MEDIUM
- **Data:** 2026-08-22
- **Relaciona-se com:** [ADR-0405](0405-mail-prompt-injection.md) · [ADR-0300](0300-ai-gateway.md)

## Context

O `CLAUDE.md` §8 diz que a IA não substitui responsabilidade humana. No correio
isso tem um significado concreto e verificável: uma mensagem enviada em nome de
um membro da Ocinye foi decidida por essa pessoa.

O risco não é teórico. Um composer com assistência onde «gerar» e «enviar» são
dois botões lado a lado, ambos primários, ambos a um clique, acaba por enviar
texto que ninguém leu.

## Decision

**A separação é estrutural, não visual.**

O composer é um único formulário HTML com dois botões de submissão e
`formaction` diferentes:

| Botão | Destino | O que acontece |
|---|---|---|
| Gerar sugestão | `POST /mail/assist` | Devolve texto; volta a desenhar o composer |
| Enviar | `POST /mail/send` | Única rota que fala com o `MailProvider` |

`assist` **não chama** `/mail/send`. Não é uma convenção nem uma verificação: é
a ausência de uma chamada. Para que a assistência enviasse, alguém teria de
acrescentar código novo, e este ADR é o que essa pessoa encontraria.

Do lado do Core, a mesma forma: `mail::assist` devolve `AssistResult`, cujo
único conteúdo é texto e metadados de proveniência. `mail::send` recebe uma
`OutgoingMessage` e um identificador de caixa.

### O que o membro vê

O texto gerado aterra no campo «Mensagem», editável, com um aviso por cima:

> Texto sugerido pela assistência do Ocinye OS. **Ainda não foi enviado.**
> Reveja-o e edite-o antes de enviar.

O aviso é dourado — a marca de IA do design — mas a frase carrega o sentido
sozinha, porque estado nunca é comunicado só por cor (`CLAUDE.md` §51).

### Sem auto-envio por agente

Nesta fase não existe agente que envie correio, e o ADR-0406 é a razão. A
capacidade de um agente actuar sobre correio exige decisão institucional
própria, com o seu ADR.

## Alternatives

**Um botão «gerar e enviar» com confirmação.** Uma caixa de confirmação torna-se
invisível ao terceiro uso.

**Marcar a mensagem enviada como gerada por IA.** `DraftOrigin` existe e regista
como o rascunho nasceu, porque a instituição vai querer a resposta mais tarde.
Mas não aparece como aviso ao destinatário: a mensagem é do membro que a enviou,
qualquer que tenha sido a ferramenta usada para a escrever.

## Consequences

- Sem nó de IA, o painel de assistência declara-se indisponível e **todo o resto
  do correio funciona**: ler, escrever, responder, enviar, pesquisar.
- `mail.ai_assist` é uma `Capability` separada de `mail` e `mail.send`
  precisamente para que essa distinção seja visível na administração.
