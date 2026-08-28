# Ocinye Mail

Correio institucional dentro do Ocinye Workspace.

Decisões: [ADR-0400](../adrs/0400-mail-as-institutional-surface.md) ·
[ADR-0401](../adrs/0401-mail-provider-abstraction.md) ·
[ADR-0402](../adrs/0402-mail-html-sanitisation.md) ·
[ADR-0403](../adrs/0403-mail-send-policy.md) ·
[ADR-0404](../adrs/0404-mail-privacy-boundary.md) ·
[ADR-0405](../adrs/0405-mail-prompt-injection.md) ·
[ADR-0406](../adrs/0406-ai-generated-is-not-sent.md) ·
[ADR-0407](../adrs/0407-mail-index-not-archive.md) ·
[ADR-0408](../adrs/0408-imap-transport.md)

Segurança: [security.md](security.md). Operação: [operations.md](operations.md).
Assistência: [ai.md](ai.md).

## Estado

| Capacidade | Estado | Nota |
|---|---|---|
| Modelo de domínio, permissões, migrations | **`CURRENT`** | 8 tabelas, 7 permissões |
| Abstracção de fornecedor | **`CURRENT`** | `MailProvider`, dois adaptadores |
| Higienização de HTML | **`CURRENT`** | Lista de permissões, 12 testes |
| Política de envio por classificação | **`CURRENT`** | 11 testes |
| Fronteira de privacidade | **`CURRENT`** | Em SQL, em cada consulta |
| Interface (caixa, leitura, composer, definições) | **`CURRENT`** | 6 ecrãs |
| Envio SMTP | **`CURRENT`** | Via `lettre`, quando configurado |
| Leitura IMAP | **`CURRENT`** | Pastas, listagem, corpo, anexos, flags, mover |
| Descoberta de pastas | **`CURRENT`** | Perguntada ao servidor, não fixada no código |
| **Sincronização** | **`DEGRADED`** | Manual. Não existe worker de ingestão periódica |
| Assistência de escrita | **`CURRENT`** | Depende de um nó de IA |
| Anexos institucionais no envio | **`PLANNED`** | Depende de object storage |
| Descarga de anexos recebidos | **`PLANNED`** | O adaptador lê-os; falta a rota e o ecrã |
| Caixas partilhadas — administração | **`PLANNED`** | Modelo e consultas existem; ecrã não |
| Agentes que actuam sobre correio | **`NOT IMPLEMENTED`** | Exige ADR próprio |

**Nesta instalação o correio não está configurado.** `OCINYE_MAIL_*` está por
definir, o adaptador em uso é `UnconfiguredProvider`, e a interface diz isso em
vez de mostrar uma caixa vazia.

## O que o correio é aqui

Um módulo do Ocinye Core, ao lado de `research` e `governance` — não uma
integração e não um webmail embebido. Consequência prática: as permissões, a
classificação, a auditoria e o modelo de capacidades são os mesmos do resto do
Ocinye OS.

## Anatomia

```
crates/ocinye-contracts/src/mail.rs        vocabulário: pastas, papéis, acções, endereços
crates/ocinye-core/src/modules/mail/
  ├── provider.rs      o trait, os erros, o adaptador não configurado
  ├── imap_smtp.rs     o adaptador IMAP+SMTP
  ├── sanitize.rs      higienização do HTML recebido
  ├── policy.rs        o que pode sair da instituição
  ├── repository.rs    consultas — a fronteira de privacidade vive aqui
  └── service.rs       o domínio: ler, assistir, avaliar, enviar
services/core-server/src/routes/mail.rs    a API
apps/workspace/src/ui/screens/mail.rs      a interface
migrations/0010_mail.sql                   8 tabelas
```

## As três fronteiras

**Privacidade.** Nenhum papel administrativo lê uma caixa pessoal alheia. A
garantia está na cláusula `WHERE` de cada consulta, não numa verificação que
alguém possa esquecer ([ADR-0404](../adrs/0404-mail-privacy-boundary.md)).

**Conteúdo não confiável.** Tudo o que chega — HTML, nomes de ficheiro, nomes de
remetente — foi escrito por quem enviou. É higienizado antes de ser mostrado
([ADR-0402](../adrs/0402-mail-html-sanitisation.md)).

**Saída da instituição.** Enviar é exportar, e a classificação governa
([ADR-0403](../adrs/0403-mail-send-policy.md)).

## O correio não depende de IA

Ler, escrever, responder, enviar e pesquisar funcionam com **zero** nós de IA
registados. Só a assistência de escrita fica indisponível, e diz-se
indisponível.

## Permissões

| Permissão | O que autoriza |
|---|---|
| `mail.use` | Aceder ao correio institucional |
| `mail.send` | Enviar mensagens |
| `mail.ai_use` | Usar a assistência de escrita |
| `mail.shared.view` | Ver caixas partilhadas de que faz parte |
| `mail.shared.send` | Enviar a partir de uma caixa partilhada |
| `mail.shared.manage` | Gerir a pertença a uma caixa partilhada |
| `mail.administer` | Configurar o serviço e diagnosticá-lo |

`mail.administer` **não** dá acesso a correspondência. Ver
[ADR-0404](../adrs/0404-mail-privacy-boundary.md).

## API

| Método | Caminho | O que faz |
|---|---|---|
| `GET` | `/api/v1/mail/status` | Estado do serviço: ler, enviar, assistência |
| `GET` | `/api/v1/mail/mailboxes` | Caixas alcançáveis, com contagens reais |
| `GET` | `/api/v1/mail/mailboxes/{id}/messages` | Uma pasta, ou uma pesquisa |
| `POST` | `/api/v1/mail/mailboxes/{id}/sync` | Actualiza o índice a partir do serviço |
| `GET` | `/api/v1/mail/messages/{id}` | Uma mensagem, higienizada |
| `POST` | `/api/v1/mail/messages/{id}/flags` | Lida / assinalada |
| `POST` | `/api/v1/mail/send` | **A única rota que envia** |
| `POST` | `/api/v1/mail/assist` | Devolve texto. Nunca envia |
| `GET`/`POST` | `/api/v1/mail/preferences` | Preferências próprias |

## Verificar uma configuração

```bash
ocinye-core-server mail-check
```

Liga por IMAP e SMTP, lista as pastas que o servidor tem, conta mensagens. Não
imprime credenciais nem conteúdo, e **não envia nada**. Ver
[operations.md](operations.md).

## Limitações declaradas

- **A sincronização é manual.** Um membro actualiza uma pasta; nada o faz por
  ele. `mail.sync` reporta `degraded`, não `available`.
- **A pesquisa é sobre metadados e excerto**, não sobre o corpo integral —
  consequência de [ADR-0407](../adrs/0407-mail-index-not-archive.md).
- **Anexos não podem ser descarregados.** Aparecem descritos, com a acção
  declarada indisponível.
- **Não há calendário nem contactos.** Fora de âmbito nesta fase.
