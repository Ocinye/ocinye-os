# ADR-0407 — `mail_messages` é um índice, não um arquivo

- **Estado:** Accepted
- **Domínio:** Mail
- **Impacto:** MEDIUM
- **Data:** 2026-08-22
- **Relaciona-se com:** [ADR-0009](0009-postgresql-sqlx.md) · [ADR-0401](0401-mail-provider-abstraction.md)

## Context

Para mostrar uma lista de correio é preciso ter alguma coisa localmente:
consultar o servidor IMAP a cada scroll é lento e frágil. A pergunta é **quanto**
guardar.

A resposta fácil é «tudo»: corpos, anexos, cabeçalhos. Dá pesquisa rápida e
funciona offline. Cria também uma segunda cópia de toda a correspondência da
instituição dentro do PostgreSQL, sob backup, sob replicação, e sujeita a
qualquer falha futura de autorização nessa base.

O `CLAUDE.md` §26 já diz que ficheiros grandes não ficam no PostgreSQL. O
briefing §72 acrescenta que não se persiste conteúdo sensível sem razão.

## Decision

`mail_messages` guarda **metadados suficientes para desenhar uma lista**:
remetente, assunto, excerto, data, estado de leitura, presença de anexos,
identidade da conversa, e o `provider_id` para ir buscar o resto.

Não guarda: corpo HTML, corpo de texto integral, bytes de anexos, cabeçalhos
completos.

O corpo é obtido ao abrir a mensagem, higienizado, mostrado, e não é escrito. A
fonte canónica do correio é o serviço de correio; o Ocinye OS mantém um índice
sobre ela.

### Consequências assumidas

**A pesquisa é sobre metadados e excerto**, não sobre o corpo integral. É uma
limitação real e está documentada. Preferível a manter uma cópia integral da
correspondência institucional para tornar a pesquisa melhor.

**Sem serviço de correio não há leitura**, nem sequer do que já foi indexado com
corpo — porque nunca houve corpo indexado. A interface diz isso em vez de mostrar
uma lista que não abre.

### Identidade de conversa

`thread_key` deriva de `References`/`In-Reply-To`, nunca do assunto. Agrupar por
assunto junta mensagens não relacionadas com o mesmo texto — «Reunião», «Fwd:
Fwd: proposta» — e no correio institucional isso significa mostrar a alguém uma
conversa em que não participou.

## Alternatives

**Arquivo integral.** Melhor pesquisa, offline completo. Recusado pelas razões
acima. Se a instituição vier a precisar de arquivo legal de correio, é um
sistema com o seu próprio ciclo de vida, retenção e ADR — não uma tabela que
cresce por omissão.

**Não indexar nada.** Cada listagem seria uma viagem ao IMAP. Inutilizável.

## Consequences

- `mail.sync` é uma `Capability` própria e está **`PLANNED`**: a ingestão que
  preenche este índice não está implementada.
- Apagar uma mensagem no serviço de correio deixa uma linha órfã no índice até à
  sincronização seguinte. A ingestão terá de reconciliar, não apenas inserir.
- A tabela tem `search_vector` sobre o que guarda, e a pesquisa é sempre dentro
  de uma caixa que o actor já pode ler.
