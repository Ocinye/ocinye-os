# Knowledge Plane

Implementação: [`ocinye-core::modules::knowledge`](../../crates/ocinye-core/README.md).

## Bibliografia é domínio científico

Uma referência não é um marcador. Transporta autores, ano, DOI, ISBN, publicação,
resumo, palavras-chave, licença, origem, chave de citação, unidade, workspace,
classificação — e o **registo importado tal como chegou**, em `raw_metadata`,
para proveniência.

Interoperabilidade com BibTeX através de uma
[capacidade WASM](../capabilities/README.md). **Não é um Zotero completo**, e não
é objectivo que seja: o Ocinye OS relaciona sistemas maduros em vez de os
recriar.

## Direitos de autor: a posição da instituição

A Ocinye **não** armazena indiscriminadamente artigos e livros completos.

Conteúdo integral é retido apenas quando existe uma **base legal registada na
própria fonte**:

| Base | Permite conteúdo integral |
|---|---|
| `metadata_only` | **Não** — o valor por omissão |
| `open_licence` | Sim, e a licença tem de ser nomeada |
| `institutional_licence` | Sim |
| `authored_by_ocinye` | Sim |
| `public_domain` | Sim |
| `permission_granted` | Sim |

Sem base registada, a instituição guarda **metadata, citação, notas e um link
autorizado** — o que torna a bibliografia útil sem a tornar ilícita.

Isto é imposto em dois sítios: no serviço, e por uma **constraint na base de
dados**, para que nenhum outro caminho o contorne.

## Notas são versionadas

Cada edição fotografa a revisão anterior em `note_revisions`. Uma nota
conceptual é o registo de como um raciocínio evoluiu; sobrescrevê-la apagaria
precisamente isso.

## Ficheiros institucionais

> **Carregar um ficheiro não é o mesmo que afirmar conhecimento institucional.**

Um `File` é a identidade dos bytes: organização, unidade, ambiente, nome e
**classificação**. É ele que decide quem lê, quem descarrega e quem escreve.
Cada `FileVersion` é numerada e imutável — a sequência começa em 1, a maior é a
corrente, e uma versão **não tem classificação própria**.

As pastas arrumam e não classificam. **Uma pasta é uma estrutura de navegação
dentro de um contentor de autoridade; mover um `File` entre contentores de
autoridade não é uma operação de pasta.** Mover um ficheiro RESTRICTED para uma
pasta chamada «Público» muda onde ele aparece e mais nada.

A classificação efectiva é `most_restrictive(workspace, file)`, calculada contra
o estado corrente do ambiente: restringir um Research Workspace fecha os
ficheiros lá dentro sem reescrever linha nenhuma.

### O corpo, e o que ele não é

Uma versão guardada produz uma **representação textual derivada**, feita pelo
worker através do outbox. Ela torna o corpo pesquisável — e não é o ficheiro,
não é autoridade, e não é conhecimento.

Ler «a temperatura foi 82 °C» de um PDF produz texto encontrável. Não produz um
`Result`, uma observação nem uma evidência: afirmar conhecimento continua a ser
um acto de uma pessoa.

`file_chunks` não guarda classificação. A visibilidade compõe-se na consulta,
contra o ficheiro e o ambiente como estão **agora** — restringir um Research
Workspace esconde o corpo dos seus ficheiros sem reindexar coisa nenhuma.

A extracção é reconstruível a partir de `FileVersion` + bytes + extractor, e é
por isso que a continuidade a classifica como derivada.

Decisões: [ADR-0204](../adrs/0204-institutional-files-and-folders.md) ·
[ADR-0205](../adrs/0205-content-extraction-and-lexical-body-search.md).

## Documentos

Um documento é uma **afirmação** de conhecimento sobre um ficheiro. Não detém
bytes: resolve através do seu `File` e vê sempre a versão corrente dele. A
classificação que o governa é a do ficheiro.

Metadata na base, bytes em object storage. Cada documento transporta tipo,
checksum SHA-256, dimensão e tipo de conteúdo.

**Apenas título e descrição são indexados para pesquisa.** Extrair corpos de
documentos para o índice é uma decisão separada e explicitamente autorizada — o
índice é um instrumento de descoberta, não uma segunda cópia do corpus.

## Fonte, entrada bibliográfica e documento

Três palavras que se confundem facilmente, e que este domínio mantém separadas
de propósito.

| | O que é | Onde vive |
|---|---|---|
| **Entrada bibliográfica** | O registo: autores, ano, DOI, publicação. O que se cita. | `sources` |
| **Fonte** | A mesma linha, vista como origem de conhecimento — o que uma Nota apoia ou refuta. | `sources`, através de `research_links` |
| **Documento** | Bytes que a instituição guarda: um PDF, um relatório, um anexo. | `documents` + object storage |

A relação entre eles é explícita e condicional: uma entrada bibliográfica **pode**
apontar para um documento com o seu texto integral, através de
`full_text_document_id`, **apenas quando existe base legal registada**. Fundir os
conceitos para simplificar a recuperação de conteúdo apagaria exactamente a
distinção que torna a posição sobre direitos de autor aplicável.

## Proveniência

Cada artefacto transporta quem o criou, quando, em que unidade e com que
classificação, desde a primeira migration. Além disso:

- uma entrada importada guarda o **registo original tal como chegou**, em
  `raw_metadata`;
- uma Nota guarda **cada revisão anterior**, não apenas a actual;
- uma relação guarda **quem a criou e porquê**, no seu campo de nota;
- um documento guarda o **checksum SHA-256**, que é o que torna verificável a
  afirmação «estes são os mesmos bytes».

Quando uma operação passa pelo plano agentic, a proveniência **da operação** —
que agente, que capability, que plano — fica na auditoria, separada da autoria
do artefacto. Um modelo que prepara texto não se torna autor institucional
(`CLAUDE.md` §72, briefing §87).

### Isto não é a proveniência científica

O que está acima é **proveniência de artefacto**: quem criou isto, quando, a
partir de que registo original, com que checksum. Responde à pergunta da autoria.

A **proveniência científica** responde a outra: de que dados, versões, métodos,
estudos e execuções **deriva** um resultado. Vive em `research_links`, é tipada, e
guarda se a relação foi observada pela operação ou declarada por alguém.

E nenhuma das duas é auditoria. A auditoria responde ao que aconteceu no sistema;
um registo de auditoria completo não responde à pergunta científica, e uma
proveniência completa não responde à operacional.

Detalhe: [ciclo de vida científico, proveniência e
linhagem](../architecture/scientific-lifecycle.md).

## Ler não é processar com IA

Duas coisas diferentes, e o tecto da segunda é mais baixo:

| Classificação | Um membro autorizado pode ler | Pode ser enviado para inferência |
|---|---|---|
| `PUBLIC` | Sim | Sim |
| `INTERNAL` | Sim | Sim |
| `CONFIDENTIAL` | Com pertença | **Só com nó local** |
| `RESTRICTED` | Muito estreito | **Nunca**, nem com nó local |

O Context Engine aplica os dois tectos, por esta ordem: primeiro a política de
leitura de quem pergunta, depois a de processamento. Material retido pelo segundo
é **contado e declarado**, não escondido — «encontrei coisas que não posso enviar
a um modelo» é diferente de «não encontrei nada», e quem decide se a resposta
está completa precisa de saber qual dos dois.

Esta instalação não tem nó local. Na prática, hoje, nada acima de `INTERNAL`
chega a um modelo.

## Memória institucional

O objectivo não é guardar ficheiros. É conseguir responder, daqui a anos: o que
foi estudado, porquê, com que dados, com que método, quem participou, o que
falhou, que ideias foram abandonadas e porquê, e que trabalhos se relacionam.

As três peças que tornam isso possível já existem: os artefactos, as **relações
tipadas** entre eles, e o **motivo obrigatório** ao encerrar uma Ideia. Nenhuma
delas é recuperável retroactivamente, e é por isso que estão desde o princípio.

## Relações

`research_links` guarda relações tipadas entre objectos de investigação:
`cites`, `supports`, `refutes`, `derived_from`, `uses`, `produces`,
`relates_to`.

Um conjunto fechado, por desenho: uma relação arbitrária tornaria o futuro
Knowledge Graph inconsultável.

Criar uma relação exige direito sobre **os dois extremos**. Poder escrever num
ambiente não é autoridade para nomear um recurso noutro — e uma aresta cujo
extremo distante quem lê não alcança seria um canal lateral: ler o extremo
próximo, aprender que o outro existe
([ADR-0306](../adrs/0306-resource-resolution-as-authorization-boundary.md)).

## Endereçável por agentes

`Note`, `Source` e `Document` são endereçáveis pelo Agentic Control Plane, e as
operações publicadas sobre eles estão em [`docs/agentic/`](../agentic/README.md).

Duas fronteiras que não se movem por serem pedidas por um agente:

- **O conteúdo de um documento não viaja.** A capability de leitura devolve
  metadata, e di-lo explicitamente com `content_included: false`. Obter os bytes
  é um acto separado e autorizado, dependente de object storage estar disponível.
- **Nenhum agente regista base legal.** `knowledge.source.create` cria sempre
  com `metadata_only`. Elevar a base é uma decisão jurídica de uma pessoa, não
  um campo que um modelo preenche.

## Ferramentas bibliográficas

Em **Bibliografia → Ferramentas**, um membro cola BibTeX e recebe três coisas: as
referências que o sistema conseguiu ler, as que não conseguiu, e uma versão
normalizada do que leu.

**O que a operação afirma.** Que a estrutura foi lida — tipo de entrada, chave de
citação e campos — e que o que se leu tem uma forma canónica: tipo em
minúsculas, um campo por linha, chaves ordenadas.

**O que não afirma.** Que a referência existe, que o DOI resolve, que o autor
escreveu aquilo ou que o ano está certo. Nada disso se sabe sem consultar fontes
externas, e esta operação não consulta nenhuma: é a mesma offline.

**Não guarda nada.** Rever é o passo anterior a acrescentar; guardar uma
referência é uma decisão separada, em «Nova Referência». Autoriza-se contra a
mesma permissão, porque rever bibliografia só faz sentido onde se pode
acrescentá-la.

A leitura acontece dentro do Capability Runtime, em isolamento WebAssembly/WASI
— sem rede, sem sistema de ficheiros, com combustível e tempo contados. Quem usa
a ferramenta não precisa de saber isso; quem mantém o sistema precisa de saber
que um analisador de texto vindo de fora corre onde não alcança nada.
