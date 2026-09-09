# Operação do Ocinye Mail

Runbooks: [configurar](../runbooks/configure-mail-service.md) ·
[diagnosticar](../runbooks/diagnose-mail-service.md) ·
[caixa partilhada](../runbooks/create-shared-mailbox.md).

## Configuração

Todas as variáveis são obrigatórias em conjunto. **Todas ou nenhuma** — o Core
recusa arrancar com configuração parcial.

| Variável | Exemplo | Nota |
|---|---|---|
| `OCINYE_MAIL_INSTITUTIONAL_DOMAINS` | `ocinye.com` | Separado por vírgulas. Sem isto, tudo é externo |
| `OCINYE_MAIL_IMAP_HOST` | `mail.ocinye.com` | |
| `OCINYE_MAIL_IMAP_PORT` | `993` | |
| `OCINYE_MAIL_IMAP_TLS` | `tls` | `tls` ou `starttls`. **Nunca desligável** |
| `OCINYE_MAIL_SMTP_HOST` | `mail.ocinye.com` | |
| `OCINYE_MAIL_SMTP_PORT` | `465` | |
| `OCINYE_MAIL_SMTP_TLS` | `tls` | `tls` (465) ou `starttls` (587) |
| `OCINYE_MAIL_USERNAME` | `mail-test@ocinye.com` | Endereço **completo** |
| `OCINYE_MAIL_PASSWORD` | — | **Nunca no Git, nunca em `.env.example`** |
| `OCINYE_MAIL_MAX_MESSAGE_BYTES` | `26214400` | 25 MB por omissão |

### A cifra não tem interruptor

`*_TLS` aceita `tls` — cifra desde o primeiro byte, portos 993 e 465 — ou
`starttls`, que promove um socket em claro nos portos 143 e 587.

**`false`, `none`, `off` e `plain` fazem o Core recusar arrancar**, com a razão.
Não são ignorados nem interpretados como `starttls`. Uma ligação de correio sem
cifra envia a password da caixa em claro, e não existe ambiente em que isso seja
aceitável ([ADR-0408](../adrs/0408-imap-transport.md)).

Ausente significa a convenção do porto, e ambas cifram.

Nenhuma tem valor por omissão útil. Uma credencial ausente significa **correio
não configurado**, que é um estado verdadeiro; inventar um valor substituí-lo-ia
por um estado avariado.

## Verificar antes de arrancar

```bash
set -a && source .env.local && set +a
ocinye-core-server mail-check
```

Liga por IMAP e por SMTP, lista as pastas que o servidor tem, e conta
mensagens.

**Não imprime** a password, a resposta de autenticação do servidor, assuntos,
remetentes ou corpos. **Não envia nada** — diagnosticar um caminho de saída
enviando correio a alguém é como mensagens de teste chegam a pessoas reais.

A saída é segura para colar num ticket.

## Três validações que impedem o arranque

**Configuração parcial.** Anfitrião sem password, ou password sem anfitrião. Um
correio meio configurado parece disponível e falha no momento em que alguém
carrega em «Enviar» — o pior momento possível para o descobrir.

**Correio configurado sem domínios institucionais.** Sem a lista, todos os
destinatários contam como externos, e a política de classificação passa a barrar
correio interno perfeitamente normal.

**Uma ligação sem cifra.** Ver acima.

## Estados possíveis

| `mail` | `mail.send` | `mail.sync` | `mail.ai_assist` | Situação |
|---|---|---|---|---|
| `not_configured` | `not_configured` | `not_configured` | `not_configured` | Sem correio configurado |
| `available` | `available` | `available` | `no_resource` | Configurado, worker a sincronizar, sem nó de IA |
| `available` | `available` | `available` | `available` | Configurado, worker a sincronizar, com nó de IA |
| `available` | `available` | `degraded` | — | Configurado, mas o worker parou ou uma caixa falha |
| `unavailable` | `unavailable` | `unavailable` | — | Configurado, IMAP/SMTP sem responder |

`mail.sync` reporta `available` quando a ingestão automática está de facto a
correr, e `degraded` quando não está — nunca `available` só por o correio estar
configurado. O worker percorre as caixas ligadas a cada cinco minutos e, no fim
de cada passagem, deixa uma batida em `mail_ingestion_heartbeat`; a capacidade
lê essa batida. Sem uma batida recente — worker parado, ou instalação acabada de
restaurar — degrada; uma passagem que não conseguiu actualizar alguma caixa
também degrada, com a razão a ficar na caixa (`mailboxes.last_sync_error`).

Verificar a disponibilidade sem verificar a batida seria reportar saudável o que
não se verificou (`CLAUDE.md` §62). Quem precisa de agora tem na mesma o botão
«Actualizar».

## Diagnóstico

`GET /api/v1/mail/status` e o cartão «Estado do serviço» em
`/mail/settings` mostram:

- se se consegue ler e se se consegue enviar — **separadamente**, porque IMAP e
  SMTP são serviços distintos e caem em separado;
- se a assistência pode servir um pedido;
- que adaptador está em uso;
- que anfitriões e portos. **Nunca credenciais.**

## Uma credencial hoje, muitas depois

Esta instalação usa **uma** credencial para toda a plataforma. Chega para
validar a integração e para o Ocinye Mail funcionar.

Não chega para a versão multiutilizador. Quando cada membro tiver endereço
próprio, cada caixa terá credenciais próprias — e essas não podem ficar em claro
na base de dados, nem podem ser apenas um digest, porque o Ocinye precisa da
credencial original para se autenticar no IMAP.

```
Membro → Conta de correio → credencial cifrada → camada de secrets → IMAP/SMTP
```

`PLANNED`. Exige ADR próprio, e uma decisão sobre onde a chave de cifra vive.
Autenticação delegada, se o fornecedor vier a oferecê-la, elimina o problema em
vez de o gerir.

## O que não existe

- **Ingestão automática.** Não há worker a sincronizar correio recebido; a
  actualização é pedida por quem está a ler.
- **Ecrã de administração de correio.** O modelo e as consultas de caixas
  partilhadas existem; o ecrã que as gere é `PLANNED`.
- **Descarga de anexos.** Depende de object storage, que não está configurado.
- **Retenção e arquivo legal.** Fora de âmbito. Ver
  [ADR-0407](../adrs/0407-mail-index-not-archive.md).
