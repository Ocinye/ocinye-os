# Runbook — Configurar o serviço de correio institucional

**Quando:** a Ocinye passa a dispor de um serviço de correio e o Ocinye OS deve
usá-lo.
**Quem:** quem opera o Ocinye OS. Exige acesso ao ambiente do Core, não uma
permissão da aplicação.

## O fornecedor actual

O correio de `ocinye.com` é fornecido pela **LWS**. Os valores confirmados são:

| | |
|---|---|
| IMAP | `mail.ocinye.com:993`, SSL/TLS implícito |
| SMTP | `mail.ocinye.com:465`, SSL/TLS implícito |
| Utilizador | o **endereço de email completo** |
| POP3 | **não é usado** — o Ocinye Mail precisa de sincronizar uma caixa, não de a esvaziar |

O webmail da LWS continua disponível como recurso administrativo. **Não é
integrado por iframe** e não é a interface do Ocinye OS: o Ocinye Mail é um
módulo nativo que fala IMAP e SMTP directamente
([ADR-0400](../adrs/0400-mail-as-institutional-surface.md)).

## Antes de começar

Precisa de seis valores, obtidos junto do fornecedor de correio:

- anfitrião e porto de IMAP;
- anfitrião e porto de SMTP;
- o nome de utilizador da conta de serviço;
- a sua palavra-passe ou *application token*.

**Prefira um token de aplicação à palavra-passe da conta**, quando o fornecedor
o suportar: pode ser revogado sozinho, sem afectar mais nada.

Precisa também da lista de domínios institucionais — hoje, `ocinye.com`.

## O que não fazer

- **Não escreva a palavra-passe em `.env.example`.** Esse ficheiro está no Git.
- **Não a escreva em `mail_provider_settings`.** Essa tabela não tem colunas de
  credenciais, por desenho ([ADR-0401](../adrs/0401-mail-provider-abstraction.md)).
- **Não use a conta pessoal de ninguém.** Uma conta de serviço, com identidade
  própria.
- **Não reutilize a credencial noutro ambiente.** Desenvolvimento, staging e
  produção nunca partilham credenciais (`CLAUDE.md` §56).

## Passos

**1.** Crie no fornecedor uma caixa **dedicada ao Ocinye OS** — por exemplo
`mail-test@ocinye.com` — com password própria. Não use a caixa pessoal de
ninguém nem uma conta administrativa.

**2.** Escreva as variáveis num ficheiro local, fora do Git. O `.gitignore` já
cobre `.env.*` com excepção de `.env.example`, pelo que `.env.local` está
protegido:

```
OCINYE_MAIL_INSTITUTIONAL_DOMAINS=ocinye.com

OCINYE_MAIL_IMAP_HOST=mail.ocinye.com
OCINYE_MAIL_IMAP_PORT=993
OCINYE_MAIL_IMAP_TLS=tls

OCINYE_MAIL_SMTP_HOST=mail.ocinye.com
OCINYE_MAIL_SMTP_PORT=465
OCINYE_MAIL_SMTP_TLS=tls

OCINYE_MAIL_USERNAME=mail-test@ocinye.com
OCINYE_MAIL_PASSWORD=<a password da caixa>
```

**Todas ou nenhuma.** O Core recusa arrancar com configuração parcial, com
correio configurado e lista de domínios vazia, e com `*_TLS` a pedir uma
ligação sem cifra.

**3.** Verifique **antes** de arrancar o Core:

```bash
set -a && source .env.local && set +a
ocinye-core-server mail-check
```

O comando liga por IMAP e por SMTP, lista as pastas que o servidor tem, e conta
mensagens. **Não imprime a password, nem assuntos, nem remetentes, nem corpos**,
e **não envia nada** — a saída é segura para colar num ticket.

Esperado:

```
  ✓ SMTP    ligação e autenticação aceites
  ✓ IMAP    ligação, autenticação e INBOX acessíveis
```

**4.** Se ambos passarem, arranque o Ocinye Core e confirme:

```
INFO Ocinye Mail adapter ready imap=… imap_security=tls smtp=… smtp_security=tls
```

**5.** Verifique em **Workspace → Correio → Definições → Estado do serviço**:

| Linha | Esperado |
|---|---|
| Leitura | Disponível |
| Envio | Disponível |
| Adaptador | `imap_smtp` |

**6.** Em **Correio**, carregue em **Actualizar**. A caixa de entrada é
sincronizada a partir do servidor. **Não há sincronização automática**: ver
[operations.md](../mail/operations.md).

**7.** Envie uma mensagem de teste **para a própria caixa de teste**, e mais
nada. Enviar para um endereço externo durante a validação exige autorização
explícita de quem responde pela instituição.

## Se falhar

`mail-check` diz qual dos dois falhou e o passo seguinte. Os dois erros mais
comuns:

| Sintoma | Causa habitual |
|---|---|
| `credenciais recusadas` | O utilizador não é o endereço de email completo, ou a conta não permite IMAP |
| `sem resposta` | Porto errado — 993/465 e não 143/587 — ou a rede não deixa sair |

Mais: [diagnose-mail-service.md](diagnose-mail-service.md).

## Depois

- Registe **onde** a credencial está guardada e **quem** lhe tem acesso.
- Marque a data de expiração do token, se tiver.
- Nada mais a fazer: o Ocinye OS não guarda a credencial em lado nenhum além do
  processo em execução.

## Uma caixa hoje, muitas depois

Esta configuração usa **uma** credencial para toda a instalação. Serve para
validar a integração e para pôr o Ocinye Mail a funcionar.

Não serve para a versão multiutilizador: quando cada membro tiver o seu
endereço, cada caixa terá credenciais próprias. Essas **não podem** ficar em
claro na base de dados, e também não podem ser apenas um digest — o Ocinye
precisa da credencial original para se autenticar no IMAP.

A evolução prevista é uma camada de secrets com cifra em repouso, ou
autenticação delegada se o fornecedor vier a oferecê-la. Nenhuma das duas está
construída, e qualquer uma exige ADR próprio.
