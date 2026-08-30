# ADR-0605 — Primeira instalação de produção e fronteiras públicas de serviço

- **Estado:** Accepted
- **Domínio:** Workspace
- **Impacto:** FOUNDATIONAL
- **Data:** 2026-08-30
- **Relaciona-se com:** [ADR-0604](0604-workspace-access-presentation.md) ·
  [ADR-0601](0601-workspace-bff-session.md) ·
  [ADR-0200](0200-object-storage.md) ·
  [ADR-0700](0700-institutional-continuity-and-portability.md)

## Context

Até aqui o Ocinye OS corria em máquinas de desenvolvimento. Esta ADR regista a
primeira instalação real, e sobretudo **as fronteiras que ela torna públicas** —
porque um endereço publicado é uma promessa, e mudá-lo depois custa mais do que
escolhê-lo bem.

## Decision

### As três fronteiras

| endereço | o que é |
|---|---|
| `ocinye.com` | a presença institucional pública da Ocinye |
| `www.ocinye.com` | redirecciona, com `308` |
| `os.ocinye.com` | o Ocinye Workspace — a Experience humana |
| `api.ocinye.com` | o Ocinye Core — a autoridade institucional |

E o princípio que as governa:

> **O encaminhamento público não redefine fronteiras de autoridade.**

O Core exposto num hostname próprio continua a ser o mesmo Core, com o mesmo
prefixo `/api/v1`. Publicá-lo não o transforma num segundo produto, e o hostname
não lhe acrescenta nem lhe retira autoridade.

O Workspace **não** vive na raiz do domínio. A raiz é onde uma instituição se
apresenta ao mundo; a Experience tem o seu endereço para que a fronteira seja
legível de fora, sem ter de se explicar.

### O que a infraestrutura não é

> **A Cloudflare é infraestrutura de edge e de segurança, não autoridade
> institucional.**

> **O VPS de produção é infraestrutura operacional, não a fronteira da memória
> institucional.**

O servidor pode desaparecer. A instituição não pode desaparecer com ele, e é por
isso que a continuidade é uma propriedade separada desta ADR — e um portão
separado.

### Os serviços de estado não são públicos

PostgreSQL, Redis e o object storage não declaram porto no host. Falam-se pela
rede interna, e o único processo com portos publicados é o proxy. A consola do
object storage não é alcançável de fora.

O browser nunca recebe credenciais do armazenamento. Nem chaves, nem URLs
assinados de escrita: quem fala com o armazenamento é o Core.

### O modelo de release

Produção corre um **commit exacto** da `main` canónica, entregue como pacote
fechado produzido por `git archive` e verificado por soma **no destino**. As
imagens são construídas no próprio servidor e etiquetadas com esse SHA.

Não há registry, e não há Git no servidor. Um registry resolve a distribuição da
mesma imagem por vários servidores — um problema que a instituição ainda não tem
— e criaria dependências que ela passaria a ter: autenticação no GitHub,
disponibilidade do GHCR, um token guardado no VPS. Com um servidor, o que
interessa é que produção corra um SHA identificável, e isso consegue-se sem nada
disso.

O compilador de Rust vive apenas dentro da etapa de construção do contentor.

### O limite do edge não redefine o produto

O Ocinye Files aceita 512 MiB. O plano de Cloudflare em que a instituição está
recusa pedidos proxied acima de ~100 MB. Nenhuma destas duas afirmações apaga a
outra: o ficheiro atravessa em pedaços de 32 MiB, montados pelo armazenamento.

Reduzir o limite institucional para caber no limite do fornecedor seria deixar a
infraestrutura decidir o produto. Tirar a API de trás do edge publicaria o
origin. Entregar credenciais ao browser tiraria a autorização do Core.

## Consequences

- O correio continua no fornecedor actual. `mail`, `imap`, `smtp` e `pop` ficam
  DNS-only e não passam pelo edge; MX, SPF, DKIM e DMARC ficam intactos.
- O origin não tem IPv6. Enquanto assim for, não há AAAA de origin — a Cloudflare
  responde por IPv6 no edge, e isso não torna o servidor dual-stack.
- Um pedido por IP ou com `Host` inesperado recebe `444` e fecha. O Workspace não
  é alcançável fora do edge por acidente.
- A instalação declara honestamente o que não tem: sem fornecedor de embeddings,
  a recuperação semântica é `NOT_CONFIGURED` e a lexical continua a funcionar.

## Alternativas recusadas

**Workspace na raiz do domínio.** Poupa um hostname e gasta a fronteira: o dia
em que a Ocinye quisesse um site institucional teria de mudar o endereço por
onde toda a gente entra.

**`uploads.ocinye.com` DNS-only.** Resolveria o limite do edge publicando o
origin — sem WAF, sem rate limiting, e com o endereço do servidor a circular.

**Browser a falar directamente com o object storage.** Resolveria o mesmo
problema entregando ao browser aquilo que a autorização existe para não lhe dar.
