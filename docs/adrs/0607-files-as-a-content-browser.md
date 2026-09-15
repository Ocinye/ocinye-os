# ADR-0607 — Ficheiros como um explorador de conteúdo

- **Estado:** Accepted
- **Domínio:** Workspace
- **Impacto:** HIGH
- **Data:** 2026-09-15
- **Relaciona-se com:** [ADR-0204](0204-institutional-files-and-folders.md) ·
  [ADR-0207](0207-personal-files-and-storage.md) ·
  [ADR-0200](0200-object-storage.md) ·
  [ADR-0602](0602-workspace-ssr-progressive-enhancement.md)

## Context

O espaço pessoal de ficheiros existia e funcionava — carregar, listar,
descarregar, pastas, quota —, mas a superfície estava **abaixo do nível
esperado**: uma tabela de administração, com formulários de gestão sempre
abertos, o `<input type="file">` nativo do browser como carregamento principal,
uma pasta reduzida a uma etiqueta, e nenhuma miniatura, grelha, pré-visualização
ou selecção. Não parecia o gestor de ficheiros de um sistema operativo; parecia
um formulário.

O trabalho não era construir armazenamento novo — o domínio pessoal já existia
(`storage_objects → file_versions → files`, diferenciado por `owner_id`,
ADR-0207) e é governado (quota admitida, ADR-0108). Era transformar a
**experiência** sem criar um segundo backend, e sem quebrar as fronteiras que já
lá estavam: a posse como autoridade, a classificação, a auditoria.

Havia ainda uma restrição do terreno: o Object Storage **não tem endpoint
público**. Uma URL assinada — o mecanismo normal de descarga (ADR-0200) — é
assinada para o host interno (`object-store:9000`), que nenhum browser alcança.
A pré-visualização de imagens de nota já resolvia isto servindo os bytes
same-origin pelo Core; a descarga pessoal, não, e por isso estava partida.

## Decision

Os Ficheiros pessoais passam a ser um **explorador de conteúdo**, entregue em
fases sobre o mesmo backend governado.

1. **Casca nativa.** Grelha e lista alternáveis, fichas com ícone, nome e tipo
   legível; barra com trilho, quota, «Nova pasta», «Lixo» e um «Carregar»
   próprio que esconde o `<input>` nativo; acções por ficha num menu `⋯` que
   abre à pedido, em vez de formulários permanentes. As condições de FAIL visual
   do briefing (§111) são o contrato desta casca.

2. **Servir same-origin.** Como o armazenamento não tem endpoint público, o Core
   serve os bytes pessoais pela origem do Workspace — pré-visualização de imagem
   (`/preview`), leitura de texto/código (`/text`), visualização inline de
   imagem e PDF (`/inline`), miniatura (`/thumbnail`) e descarga (`/raw`) —, com
   a posse reavaliada a cada pedido. A localização do armazenamento nunca chega à
   página (§26, §40). É uma **excepção deliberada** ao princípio «o Core não
   transporta bytes»: institucionalmente mantém-se a URL assinada; pessoalmente,
   o terreno obriga.

3. **Quick Look.** Abrir um ficheiro pré-visualiza-o numa camada: imagem e PDF
   inline, texto e código escapados num `<pre>` (nunca interpretados), e uma
   ficha com descarregar para o resto. O PDF é desenhado pelo visualizador do
   browser, fora do processo da página.

4. **Miniaturas.** Um trabalhador gera um derivado visual de cada imagem e da
   primeira página de cada PDF, ligado à versão (`file_thumbnails`), servido
   same-origin. A imagem é redimensionada e re-codificada em WebP; o PDF é
   rasterizado por um subprocesso isolado (`pdftoppm`, um renderizador que **não
   executa o JavaScript do documento**), com prazo. É a fronteira que separa o
   conteúdo não confiável do processo do worker.

5. **Gestão.** Selecção múltipla com acções em lote (mover/eliminar), arrastar
   para mover para uma pasta ou para a raiz, e Favoritos e Recentes como vistas
   que atravessam pastas. Tudo reautoriza pela posse no Core, ficheiro a
   ficheiro; a marca de favorito é preferência do membro, e viaja na
   continuidade.

Nenhuma decisão de autorização se move para o cliente. A Experience esconde o
que não se pode ver; nunca decide se se pode (ADR-0602).

## Alternatives

- **Endpoint de armazenamento público + URL assinada no browser.** Exigiria
  expor o MinIO à Internet e assiná-lo para um host público — mais superfície de
  ataque (§24, §40) por uma configuração de rede que não existe. Servir
  same-origin esconde a localização e não abre nada.
- **Um silo de armazenamento paralelo para o pessoal.** Rejeitado por ADR-0207:
  reutiliza-se o domínio canónico, diferenciado por `owner_id`.
- **`<input type="file">` nativo como carregamento principal.** É precisamente
  uma das condições de FAIL do §111. O «Carregar» próprio esconde-o.
- **Rasterizar PDF em processo, com uma biblioteca embutida.** Um PDF hostil é
  conteúdo não confiável; corrê-lo dentro do worker sem isolamento é o que a
  §76 proíbe. O subprocesso com prazo é o mínimo defensável.

## Consequences

- Para ficheiros pessoais, o **Core transporta bytes** — pré-visualização,
  miniatura, texto e descarga saem pela origem do Workspace. É uma excepção
  documentada, forçada pela ausência de endpoint público, e contida ao caminho
  pessoal.
- A **descarga institucional** tem o mesmo defeito latente (URL assinada para o
  host interno) e continua por resolver — está registada como trabalho à parte.
- Ficam **explicitamente adiadas**, como trabalho futuro e não como lacuna deste
  marco:
  - miniaturas de **Office e vídeo**, que exigem um conversor mais pesado
    (LibreOffice, ffmpeg) num trabalhador de conversão dedicado;
  - **isolamento mais forte** da rasterização — seccomp, ou um contentor à parte
    — acima do subprocesso com prazo e utilizador não privilegiado de hoje.
- A CSP do Workspace ganhou `frame-src 'self'` (para o Quick Look de PDF numa
  `iframe` da própria origem); a resposta de `/inline` abre o enquadramento só
  same-origin. A CSP das páginas mantém-se fechada.
