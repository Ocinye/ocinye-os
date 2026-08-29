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

### O que fica por decidir

A pré-visualização de imagens. A `Content-Security-Policy` do Workspace é
`img-src 'self' data:`, e mostrar uma imagem do armazenamento exigiria ou
alargá-la a um host configurável — possivelmente externo — ou fazer o Workspace
transportar os bytes. É uma decisão de segurança, e não uma consequência de
alguém querer ver uma miniatura. Hoje pré-visualiza-se texto, e diz-se
claramente o que não se mostra.
