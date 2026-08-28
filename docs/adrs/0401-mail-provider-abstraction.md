# ADR-0401 — Abstracção de fornecedor de correio

- **Estado:** Accepted
- **Domínio:** Mail
- **Impacto:** MEDIUM
- **Data:** 2026-08-22
- **Relaciona-se com:** [ADR-0200](0200-object-storage.md) · [ADR-0300](0300-ai-gateway.md)

## Context

O correio institucional da Ocinye vai correr, nesta fase, sobre um serviço de
terceiros. Mais tarde poderá correr sobre infraestrutura própria. Entre os dois
momentos há anos, e o `CLAUDE.md` §71 é claro: não fechar portas.

O padrão já existe no repositório em duas outras camadas — o object storage
abstrai o fornecedor S3 ([ADR-0200](0200-object-storage.md)) e o AI Gateway
abstrai o modelo ([ADR-0300](0300-ai-gateway.md)). O correio segue o mesmo.

## Decision

Um `trait MailProvider` em
`crates/ocinye-core/src/modules/mail/provider.rs`, com sete operações:
`list_messages`, `fetch_message`, `fetch_attachment`, `send_message`,
`move_message`, `set_read`, `set_starred`.

**O domínio nunca vê IMAP nem SMTP.** Vê `FetchedMessage`, `OutgoingMessage`,
`ProviderError`. Um adaptador traduz; a tradução é a fronteira.

Duas implementações:

| Adaptador | Quando | O que faz |
|---|---|---|
| `UnconfiguredProvider` | Sem correio configurado | Responde a tudo com `NotConfigured` e uma frase institucional |
| `ImapSmtpProvider` | Com `OCINYE_MAIL_*` definido | SMTP via `lettre`; leitura IMAP **`PLANNED`** |

`UnconfiguredProvider` **não é um mock**. É o comportamento correcto de uma
instalação sem correio, e é o que o `CLAUDE.md` §69 exige: um estado declarado
em vez de uma página em branco. O `AppState` guarda `Arc<dyn MailProvider>` e
nunca `Option`, precisamente para que nenhum manipulador possa esquecer-se de
tratar o caso ausente.

### `ProviderError` é fechado

```
NotConfigured · Unavailable · AuthenticationFailed · NotFound · Rejected · TooLarge
```

Seis situações, cada uma com uma resposta institucional distinta. Um erro de
protocolo nunca chega ao membro: `Rejected` carrega texto já traduzido, e os
restantes não carregam texto nenhum.

### Credenciais não são dados institucionais

A tabela `mail_provider_settings` **não tem colunas de credenciais**, por
desenho. Anfitrião, porto e modo de segurança são configuração operacional e
vivem lá; a password vive apenas em `OCINYE_MAIL_PASSWORD`, lida no arranque.

`ImapSmtpConfig` e `MailConfig` implementam `Debug` à mão para redigir a
password: `CoreConfig` deriva `Debug` e é registado no arranque, pelo que sem
isso a credencial estaria na primeira linha de cada log (briefing §57, §58).

## Alternatives

**Falar IMAP/SMTP directamente no serviço.** Mais curto por um dia. Torna
impossível testar sem uma caixa real e cimenta o protocolo em código de domínio.

**Um `Option<Arc<dyn MailProvider>>`.** Empurra a decisão «e se não houver
correio?» para cada manipulador. Um deles acabaria por errar.

## Consequences

- Trocar de fornecedor é escrever um adaptador; o domínio não muda.
- A saúde reportada distingue leitura de envio, porque IMAP e SMTP são serviços
  diferentes e caem separadamente.
- `ProviderHealth::endpoints` mostra anfitriões e portos ao ecrã de
  administração. Nunca credenciais — não por convenção, mas porque a estrutura
  não as tem.
