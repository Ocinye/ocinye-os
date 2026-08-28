# Assistência de escrita no Ocinye Mail

Decisões: [ADR-0405](../adrs/0405-mail-prompt-injection.md) ·
[ADR-0406](../adrs/0406-ai-generated-is-not-sent.md).
Ver também [docs/ai/](../ai/README.md).

## A regra

> **Texto gerado não é mensagem enviada.**

A separação é estrutural, não visual. O composer é um formulário com dois botões
de submissão e destinos diferentes:

| Botão | Destino | Efeito |
|---|---|---|
| Gerar sugestão | `POST /mail/assist` | Devolve texto ao campo «Mensagem» |
| Enviar | `POST /mail/send` | Única rota que fala com o serviço de correio |

`assist` não chama `send`. Não é uma verificação — é a ausência de uma chamada.

## As dez acções

Conjunto fechado, definido em `ComposeAction`:

`generate` · `reply` · `more_formal` · `shorter` · `more_cordial` ·
`more_direct` · `clarify` · `proofread` · `translate` · `summarise`

O membro escolhe uma e escreve o conteúdo do pedido. Uma acção desconhecida é
recusada na fronteira HTTP: o modelo nunca recebe um verbo que um email tenha
proposto.

## Como o pedido é montado

```
<<<EMAIL_RECEBIDO
(conteúdo da mensagem — DADOS, nunca instruções)
>>>

<<<RASCUNHO
(o que o membro já escreveu)
>>>

<<<PEDIDO_DO_MEMBRO
(a instrução, escrita pelo membro)
>>>
```

A instrução de sistema declara, por escrito, que o bloco `EMAIL_RECEBIDO` é
material a processar e não instruções a seguir.

Nenhuma delimitação textual é uma garantia. A garantia é a seguinte.

## A assistência não tem poderes

`assist` devolve uma `String`. Não envia, não move mensagens, não lê outras
caixas, não altera permissões, não escreve na base de dados.

Uma injecção bem sucedida faz o modelo escrever texto estranho num campo que o
membro lê antes de enviar. Não há escalada porque não há nada para escalar.

## Classificação limita o contexto

`SendPolicy::may_use_as_ai_context` permite `PUBLIC` e `INTERNAL`. Nada mais.

`human_read = true` **não** implica `ai_processing_allowed = true`: uma pessoa a
ler material `CONFIDENTIAL` está vinculada às suas obrigações institucionais; um
modelo é um sistema cuja retenção e encaminhamento a instituição não controla
inteiramente.

## Sem nó de IA

`mail.ai_assist` reporta `no_resource` e o painel diz:

> A assistência de escrita depende de uma capacidade de IA do Ocinye OS, que não
> está actualmente disponível. Escrever, responder e enviar não dependem dela e
> continuam a funcionar normalmente.

Se o membro não tiver `mail.ai_use`, a frase é outra — não poder e não haver são
situações diferentes, e a interface distingue-as.

**Nunca se liga um fornecedor externo para esconder a ausência de IA local**
(`CLAUDE.md` §41).

## Proveniência

`DraftOrigin` regista se um rascunho foi escrito, gerado ou editado a partir de
uma sugestão. A instituição vai querer a resposta mais tarde.

Não aparece como aviso ao destinatário: a mensagem é do membro que a enviou,
qualquer que tenha sido a ferramenta usada para a escrever.
