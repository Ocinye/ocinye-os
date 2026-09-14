# ADR-0207 — Ficheiros pessoais: todo o membro tem um espaço próprio

- **Estado:** Accepted
- **Domínio:** Knowledge
- **Impacto:** HIGH
- **Data:** 2026-09-14
- **Relaciona-se com:** [ADR-0200](0200-object-storage.md) ·
  [ADR-0204](0204-institutional-files-and-folders.md) ·
  [ADR-0108](0108-resource-governance-and-compute-control-plane.md) ·
  [ADR-0413](0413-notes-as-institutional-knowledge.md)

## Context

Desde [ADR-0204](0204-institutional-files-and-folders.md), o ficheiro é a
autoridade sobre os bytes, e essa autoridade compõe-se com um ambiente de
investigação: um ficheiro institucional vive num Research Workspace, e quem lá
escreve é quem lá tem papel. A migração `0032` abriu uma segunda forma — um
ficheiro **do dono** (`owner_id`, sem ambiente) —, e as Notas e os anexos do
Correio já a usam para guardar imagens e ficheiros pessoais na mesma primitiva
(`storage_objects → file_versions → files`). O armazenamento pessoal já é
**governado**: a quota de bytes por membro existe ([ADR-0108](0108-resource-governance-and-compute-control-plane.md)),
com um limite institucional por omissão, e «Meus Recursos» já a mostra.

Mas o espaço pessoal era só canalização: escrevia-se (notas, anexos) e nunca se
**listava**. O ecrã de Ficheiros só oferecia destinos institucionais — os
ambientes onde o membro pode escrever —, pelo que um membro sem ambiente nenhum
via «Não tem onde carregar ficheiros». Isso é conceptualmente errado: o membro
tem um espaço pessoal, e a interface escondia-o.

Há ainda um princípio maior em jogo: **a ausência de capacidade de IA não pode
desactivar uma capacidade determinística**. Ficheiros não precisa de GPU, modelo
ou nó de inferência; exigir um ambiente de investigação para carregar um
ficheiro acopla, sem razão, uma capacidade básica a estruturas que ela não
requer.

## Decision

**Todo o membro activo tem um espaço de ficheiros pessoal — «Meus ficheiros» —,
que existe sem unidade, projecto, ambiente de investigação nem IA.**

- O espaço pessoal **reutiliza a primitiva canónica** (`storage_objects →
  file_versions → files`), diferenciada por `owner_id`. Não há silo novo, nem
  tabela nova: a decisão de não duplicar o domínio de ficheiros (`0031`, `0032`)
  mantém-se.
- A **autoridade é a posse**, não ABAC: um ficheiro pessoal é do dono, e cada
  leitura, listagem e escrita fecha-se sobre `person_id`. Conhecer um
  identificador não abre o ficheiro de outra pessoa; um administrador não ganha
  leitura por ser administrador.
- A **quota é a do armazenamento pessoal já governado** ([ADR-0108](0108-resource-governance-and-compute-control-plane.md)):
  a admissão de bytes corre no carregamento, com o limite institucional por
  omissão, e uma admissão recusada aborta a transacção sem consumir espaço.
- O provisionamento é **preguiçoso e sem migração**: o espaço materializa-se no
  primeiro carregamento (o prefixo de chave `persons/{owner_id}/…` e a coluna
  `owner_id`), pelo que os membros existentes ganham-no sem serem recriados nem
  exigirem acção de administração.
- O Core expõe o espaço por rotas próprias — listar (`GET /me/files`), carregar
  (`POST /me/files/uploads`) e descarregar por ligação assinada
  (`GET /me/files/{version_id}/download`) —, e o Core continua a **não
  transportar bytes**: assina e encaminha ([ADR-0200](0200-object-storage.md)).
- Os destinos institucionais permanecem **adicionais**: um membro com papel num
  ambiente vê-o como destino a mais, autorizado no momento da escrita.

## Alternatives

- **Manter Ficheiros só institucional.** Rejeitado: contradiz o modelo de dados
  (o ficheiro do dono já existe) e a experiência (um membro sem ambiente fica
  sem espaço), e acopla uma capacidade determinística a estruturas que ela não
  precisa.
- **Um bucket ou uma tabela por membro.** Rejeitado: a primitiva partilhada já
  distingue o pessoal pelo `owner_id` e pela chave; um contentor por pessoa
  seria um segundo domínio de armazenamento sem ganho.
- **Uma permissão `files.personal.*` nova.** Rejeitada por agora: a autoridade
  de um ficheiro pessoal é a posse, e uma permissão a conceder seria uma segunda
  fonte de verdade a discordar do dono. Uma permissão só nasce quando houver uma
  decisão que a posse não exprima (por exemplo, partilha).

## Consequences

- «Meus ficheiros» aparece a todo o membro activo, com carregar, listar e
  descarregar — e a mensagem «Não tem onde carregar ficheiros» desaparece para
  um membro comum.
- O espaço pessoal conta para a quota que «Meus Recursos» já mostra: uma só
  contabilidade, não um contador paralelo.
- Pastas pessoais, mudança de nome, mover, lixo e restauro, e a partilha, ficam
  para fatias seguintes; a identidade do objecto (imutável, opaca) já as
  suporta sem reescrever chaves.
- O princípio geral — **capacidades determinísticas não dependem de IA** —
  fica registado aqui como caso, e vale para Correio, Notas, Calendário e o
  resto.
