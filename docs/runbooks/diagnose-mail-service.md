# Runbook — Diagnosticar o serviço de correio

**Quando:** o correio não aparece, não envia, ou um membro reporta erro.
**Quem:** quem detiver `mail.administer`, para os passos na aplicação. Os
passos de ambiente exigem acesso ao Core.

## Primeiro: separe as três perguntas

Ler, enviar e assistir falham em separado. Comece por saber **qual**.

**Workspace → Correio → Definições → Estado do serviço.**

| Leitura | Envio | Assistência | Situação |
|---|---|---|---|
| Indisponível | Indisponível | Indisponível | Correio não configurado |
| Disponível | Indisponível | — | SMTP em baixo ou credencial recusada |
| Indisponível | Disponível | — | IMAP em baixo |
| Disponível | Disponível | Indisponível | **Normal** sem nó de IA |

A última linha não é uma avaria. O correio não depende de IA.

## «Não vejo mensagens nenhumas»

Confirme primeiro se o adaptador em uso é `unconfigured`. Se for, o correio não
está configurado — ver [configure-mail-service.md](configure-mail-service.md).

Se estiver configurado e a caixa continuar vazia: **a ingestão IMAP não está
implementada** (`mail.sync` reporta `planned`). O índice só contém o que lá foi
posto, e nada o põe automaticamente. É uma limitação declarada, não uma falha.

## «Não consigo enviar»

**1. A recusa foi da política ou do serviço?**

Uma recusa que fale em `RESTRICTED` ou em destinatários externos é a política de
classificação a funcionar ([ADR-0403](../adrs/0403-mail-send-policy.md)). Não é
para contornar.

**2. A credencial foi recusada?**

Procure `AuthenticationFailed` nos logs. Se aparecer, o token expirou ou foi
revogado. Emita um novo e reinicie.

**3. O membro pode enviar?**

Precisa de `mail.send`, e — numa caixa partilhada — de papel `Sender` ou
`Manager`.

## «Um membro diz que não vê imagens»

Comportamento correcto. Conteúdo remoto é bloqueado por omissão porque carregá-lo
informa quem enviou de que a mensagem foi aberta. O botão «Carregar mesmo
assim» existe na própria mensagem.

## O que os logs contêm

Anfitriões, portos, códigos de erro traduzidos, identificadores de correlação.

**Não contêm**, e não devem passar a conter: palavras-passe, corpos de mensagem,
anexos, ou prompts com mensagens inteiras. Se precisar de investigar uma
mensagem concreta, use o identificador de correlação e o audit trail — não
aumente o que é registado.

## Escalar

Se o serviço de correio do fornecedor estiver em baixo, o Ocinye OS não tem nada
a corrigir. Confirme com o estado do fornecedor antes de mexer em configuração:
uma credencial rodada durante uma avaria externa cria dois problemas.
