# ADR-0205 — Extracção de conteúdo e pesquisa lexical do corpo

- **Estado:** Accepted
- **Domínio:** Knowledge
- **Impacto:** FOUNDATIONAL
- **Data:** 2026-08-29
- **Relaciona-se com:** [ADR-0204](0204-institutional-files-and-folders.md) ·
  [ADR-0202](0202-search-fts-pgvector.md) ·
  [ADR-0010](0010-events-outbox.md) ·
  [ADR-0412](0412-scientific-lifecycle-and-provenance.md) ·
  [ADR-0700](0700-institutional-continuity-and-portability.md)

## Context

Depois de [ADR-0204](0204-institutional-files-and-folders.md), os ficheiros
institucionais têm identidade, versões e governação. O que ainda não tinham era
conteúdo alcançável: a pesquisa via títulos e descrições, e o corpo de um PDF
era opaco. Uma frase escrita na página quatro de um relatório existia na
instituição e não se encontrava.

## Decision

> **Um `FileVersion` guardado pode produzir uma representação textual derivada,
> reconstruível e ligada à versão exacta; essa representação torna o corpo
> pesquisável sem transformar o índice em autoridade e sem alterar a validade do
> ficheiro se o processamento falhar.**

### O trabalho usa o outbox que já existe

Durável, idempotente, `FOR UPDATE SKIP LOCKED`, com tentativas e recuo
exponencial ([ADR-0010](0010-events-outbox.md)). A única coisa nova é um
tipo de trabalho.

A identidade do trabalho é a **versão**, nunca o ficheiro. Uma versão nova não
reinterpreta a anterior, e a extracção da v1 continua a descrever a v1.

O pedido nasce dentro de `create_with_first_version` e `add_version`, e não em
quem as chama: como responsabilidade do chamador seria uma coisa de que alguém
se teria de lembrar, e um caminho novo que criasse uma versão sem pedir
extracção produziria um ficheiro silenciosamente não pesquisável.

### Três estados, e não um

| Camada | Estados |
|---|---|
| Armazenamento | `STORED` |
| Extracção | `QUEUED` · `PROCESSING` · `AVAILABLE` · `UNSUPPORTED` · `FAILED` |
| Índice lexical | derivado da extracção |

O que isto compra é uma frase que a interface pode dizer com verdade:

> **Ficheiro guardado. Não foi possível tornar o conteúdo pesquisável.**

E não «o carregamento falhou», que mandaria alguém carregar outra vez um
ficheiro que já lá está.

`UNSUPPORTED` é estado normal: um PNG não tem extractor e nunca terá. Um PDF de
páginas digitalizadas lê-se sem erro e não produz texto nenhum — também é
`UNSUPPORTED`, porque dizer `AVAILABLE` com zero pedaços afirmaria que o corpo
está pesquisável quando não está.

O armazenamento não responder é a única coisa tratada como **erro**: o outbox
volta a tentar. Marcar `FAILED` aí afirmaria que o conteúdo não se consegue ler,
quando o que aconteceu foi um disco não atender.

### A proveniência da leitura

Cada extracção guarda o nome do extractor, a sua versão e a soma dos bytes de
que saiu. É o que permite responder, daqui a dois anos, «porque é que este
pedaço existe desta forma?» sem arqueologia.

### Extrair não é afirmar

Ler «a temperatura foi 82 °C» de um PDF produz texto pesquisável. **Não** produz
um `Result`, uma observação, uma evidência nem uma afirmação científica. Afirmar
conhecimento continua a ser um acto de uma pessoa.

### O índice descobre; a autoridade decide

> **A pesquisa pode usar um índice para descobrir candidatos, mas a visibilidade
> decide-se contra o estado autoritativo corrente. Um índice nunca é autoridade
> de autorização.**

`file_chunks` **não guarda classificação nenhuma**. A composição
`most_restrictive(file, workspace)` é feita na consulta, contra o estado
corrente — pelo que restringir um Research Workspace esconde imediatamente o
corpo dos seus ficheiros, sem reindexar coisa alguma.

### Versões, na pesquisa

A pesquisa institucional normal prefere a **versão corrente**: se a v1 e a v2
contêm a mesma frase, dois resultados aparentemente iguais não ajudam ninguém.
Os pedaços da v1 **não** são substituídos pelos da v2 — continuam guardados, e é
isso que permite que uma recuperação histórica exacta continue a resolver.

## Consequences

O corpo passa a ser encontrável sem nenhum modelo de IA. A pesquisa lexical
funciona hoje, e continuará a funcionar quando houver embeddings — que serão
outra coisa ao lado desta, e não em vez dela.

`file_extractions` e `file_chunks` são **derivados reconstruíveis** no manifesto
de continuidade: saem de `FileVersion` + bytes + definição do extractor, e as
três coisas são duráveis. A reconstrução é provada e não afirmada — um teste
apaga a extracção, confirma que a frase desapareceu, reprocessa, e exige a mesma
frase na mesma página da mesma versão.

### Dependência

`pdf-extract` 0.12 (MIT), com dezasseis crates transitivas, todas MIT ou
Apache-2.0 e todas Rust puro. O leitor corre dentro de `catch_unwind`: é Rust
seguro em memória, mas entra em pânico com documentos estranhos, e um pânico no
worker levaria consigo o lote inteiro de eventos.

Uma dessas transitivas — `ttf-parser`, via `lopdf` — está marcada como **não
mantida** (RUSTSEC-2024-0355). É o único aviso desta árvore que é alcançado por
entrada não confiável: lê as fontes embutidas num PDF que alguém carregou. O
risco fica escrito em `.cargo/audit.toml` com o que existe em vez de uma
correcção a montante — segurança de memória, `catch_unwind`, execução no worker
e fora do caminho de um pedido, e uma falha que não invalida o ficheiro. Sai
dessa lista quando `lopdf` deixar de o arrastar, ou quando a extracção de PDF
passar a correr dentro do Capability Runtime.

### O que fica por fazer

OCR não entra aqui: um PDF digitalizado fica `UNSUPPORTED` e diz porquê.
Embeddings, recuperação híbrida e conteúdo autorizado para agentes são a decisão
seguinte.
