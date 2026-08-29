# ADR-0204 — O ficheiro institucional é a autoridade sobre os bytes

- **Estado:** Accepted
- **Domínio:** Data
- **Impacto:** FOUNDATIONAL
- **Data:** 2026-08-29
- **Relaciona-se com:** [ADR-0200](0200-object-storage.md) ·
  [ADR-0202](0202-search-fts-pgvector.md) ·
  [ADR-0412](0412-scientific-lifecycle-and-provenance.md) ·
  [ADR-0700](0700-institutional-continuity-and-portability.md) ·
  [ADR-0602](0602-workspace-ssr-progressive-enhancement.md)

## Context

Antes desta decisão, quem guardava bytes na Ocinye tinha de os declarar
`Document` — uma afirmação de conhecimento institucional, com título, tipo
documental e data. Um PNG de uma montagem experimental, uma folha de cálculo de
trabalho, um PDF de uma factura: todos entravam pela mesma porta, e todos saíam
de lá a dizer que eram conhecimento.

Havia dois problemas, e são diferentes.

O primeiro é semântico. **Carregar um ficheiro não é o mesmo que afirmar
conhecimento institucional.** Obrigar as duas coisas a coincidir enche o acervo
documental de material que ninguém quis declarar, e ensina as pessoas a mentir
ao formulário para poderem guardar um ficheiro.

O segundo é estrutural. `documents.storage_object_id` apontava directamente aos
bytes, pelo que um documento tinha exactamente uma versão para sempre. Corrigir
o gráfico da página 4 obrigava a criar outro documento, e as duas versões
ficavam a existir como se fossem dois trabalhos.

## Decision

Introduzem-se **quatro conceitos**, com uma autoridade só.

`File` é a identidade institucional dos bytes. Tem organização, unidade,
ambiente, nome e **classificação**. É ele quem decide quem lê, quem descarrega e
quem escreve.

`FileVersion` é uma versão numerada e imutável de um `File`, apontando a um
`storage_object`. A sequência começa em 1 e a maior é a corrente. **Uma versão
não tem classificação própria** e não abre nada que o ficheiro feche.

`Folder` é uma estrutura de navegação dentro de um ambiente. **Uma pasta é uma
estrutura de navegação dentro de um contentor de autoridade; mover um `File`
entre contentores de autoridade não é uma operação de pasta.** Uma pasta não
tem classificação e não a empresta ao que está dentro dela.

`Document` deixa de deter bytes e passa a resolver através de um `File`.
Continua a ser o que sempre foi — uma afirmação de conhecimento — mas agora é
uma afirmação **sobre** um ficheiro, e não a única forma de ter um.

### A composição da classificação

A classificação efectiva é `most_restrictive(workspace, file)`, calculada contra
o **estado corrente** do ambiente. Restringir um Research Workspace fecha
imediatamente os ficheiros lá dentro, sem reescrever linha nenhuma.

### O que a pesquisa pode e não pode

> **A pesquisa pode usar um índice para descobrir candidatos, mas a
> visibilidade decide-se contra o estado autoritativo corrente. Um índice nunca
> é autoridade de autorização.**

`search_documents.classification` é uma cópia. Compõe-se com a classificação
viva do ambiente no momento da consulta, e é essa composição que filtra.

## Consequences

Ganha-se versionamento real, com citação de versões exactas: `/files/{id}` é
«o corrente» e muda; `/file-versions/{id}` é a versão 3 e continua a sê-lo
amanhã.

Ganha-se um sítio para arrumar sem classificar. Uma pessoa cria pastas, arrasta
ficheiros e não afirma nada sobre eles.

Perde-se a simplicidade de um documento apontar aos seus bytes. A leitura passa
a atravessar um `LATERAL` até à versão corrente, e quem escrever consultas novas
tem de o saber.

`storage_objects` mantém-se como está. Não ganhou estados novos nem ciclo de
vida próprio nesta decisão: continua a ser onde os bytes estão, e `ON DELETE
RESTRICT` impede que desapareçam por baixo de uma versão que alguém cita.

### A pré-visualização é same-origin, pelo Core

A `Content-Security-Policy` do Workspace continua `img-src 'self' data:`, e
continua assim por decisão. Uma imagem institucional aparece através de
`GET /files/{id}/preview`: o Core resolve o `File`, autoriza o actor corrente,
resolve a `FileVersion` corrente, lê os bytes e serve-os inline.

> **A Experience não precisa de conhecer nem confiar no endpoint físico onde os
> bytes institucionais estão guardados.**

A alternativa — acrescentar hosts de object storage a `img-src` — faria a camada
de experiência conhecer topologia de armazenamento, tornaria a CSP dependente do
deployment e acrescentaria uma origem externa à página.

Inline serve-se apenas uma **lista fechada** de formatos raster: `image/png`,
`image/jpeg`, `image/webp`. Não é `image/*`, porque um SVG é um documento com
script e servi-lo inline na origem do Workspace seria executá-lo lá. A resposta
leva o tipo validado, `Content-Disposition: inline`, `nosniff`, `private` e um
`ETag` derivado da soma dos bytes guardados.

Isto **não é a descarga**. Pré-visualizar é uma representação inline autorizada;
descarregar continua a sair por ligação assinada, e as duas coisas aparecem
separadas na auditoria — `preview` e `download` — para que quem pergunta «quem
tirou isto da instituição» não tenha de filtrar toda a gente que apenas olhou
para o ecrã.

O custo é o Core transportar os bytes. É a troca certa nesta fase; se um dia o
débito justificar outra arquitectura, introduz-se um mecanismo dedicado e
prova-se a fronteira outra vez.

### O que fica por decidir

A pré-visualização de PDF e de outros formatos paginados. OCR não entra nesta
decisão.
