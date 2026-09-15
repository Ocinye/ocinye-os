# ADR-0608 — Descarga institucional servida same-origin

- **Estado:** Accepted
- **Domínio:** Workspace
- **Impacto:** MEDIUM
- **Data:** 2026-09-15
- **Relaciona-se com:** [ADR-0607](0607-files-as-a-content-browser.md) ·
  [ADR-0200](0200-object-storage.md) ·
  [ADR-0204](0204-institutional-files-and-folders.md) ·
  [ADR-0207](0207-personal-files-and-storage.md)

## Context

A descarga de um ficheiro institucional — a versão corrente por
`/files/{id}/download`, uma versão exacta por `/file-versions/{vid}/download` —
respondia com uma **ligação assinada** de curta duração (ADR-0200), e a
experiência redireccionava o browser para lá. É o mecanismo correcto **quando o
armazenamento tem um endpoint público**. O da Ocinye não tem: a ligação é
assinada para o host interno (`object-store:9000`), que nenhum browser alcança.
A descarga institucional estava, por isso, **partida** — um clique terminava num
«não foi possível encontrar o servidor», e não num ficheiro.

Isto não era novidade. A [ADR-0607](0607-files-as-a-content-browser.md) já tinha
encontrado o mesmo terreno no espaço pessoal e resolvido-o servindo os bytes
same-origin pelo Core; e, na sua secção de consequências, registou de forma
explícita que **a descarga institucional tinha o mesmo defeito latente e ficava
por resolver, como trabalho à parte**. A decisão 2 dessa ADR chegou a escrever
que «institucionalmente mantém-se a URL assinada». Esta ADR executa esse trabalho
adiado, e **revê essa frase**: mantê-la seria manter uma descarga que não
descarrega.

A pré-visualização institucional de imagens já servia same-origin (ADR-0204), o
que prova que o caminho existe e é aceite; o que faltava era estendê-lo à
descarga, que é o mesmo problema com outro cabeçalho (`attachment` em vez de
`inline`).

## Decision

A descarga de ficheiros institucionais passa a ser **servida same-origin pelo
Core**, exactamente como a pessoal (ADR-0607) e a pré-visualização (ADR-0204).

1. **Bytes pela origem, não uma ligação assinada.** O Core ganha
   `GET /files/{id}/raw` (versão corrente) e `GET /file-versions/{vid}/raw`
   (versão exacta), que reavaliam a autorização e devolvem os bytes com
   `Content-Disposition: attachment`. A experiência liga a `/download`, e o BFF
   transporta esses bytes — a localização do armazenamento nunca chega à página
   (§26, §40).

2. **A autoridade é a mesma, e não menos.** `raw` corre a **mesma** composição de
   classificação e a **mesma** `Action::Download` que a ligação assinada corria —
   `get`/`get_version` resolvem o ficheiro que governa, e a decisão regista-se sob
   a acção que aconteceu. Servir os bytes em vez de uma URL não relaxa nenhuma
   fronteira: quem a leitura recusa continua a não obter os bytes, pela versão
   corrente e pela exacta, e a recusa é indistinguível de o ficheiro não existir.

3. **A ligação assinada permanece como contrato de API, não como caminho da
   experiência.** Os endpoints `/download` (que devolvem `{ url }`) ficam: são o
   mecanismo certo no dia em que exista um endpoint público de armazenamento, e
   retirá-los é uma alteração de contrato com ADR própria. O que muda é **quem os
   usa** — a experiência deixa de os usar, como já deixara no caminho pessoal.

Isto revê a decisão 2 e a consequência «defeito latente por resolver» da
ADR-0607. A ADR-0607 mantém-se `Accepted`: a sua decisão de fundo — Ficheiros como
explorador de conteúdo, servido same-origin porque o armazenamento não tem
endpoint público — não muda; estende-se ao institucional o que ela já decidira
para o pessoal.

## Alternatives

- **Expor o armazenamento à Internet e assinar para um host público.** Rejeitada
  pela ADR-0607 pela mesma razão: mais superfície de ataque (§24, §40) por uma
  configuração de rede que não existe, e a localização deixaria de estar
  escondida. Servir same-origin não abre nada.
- **Deixar como estava e documentar o defeito.** Era o estado registado pela
  ADR-0607, e é honesto, mas uma descarga que nunca descarrega não é uma
  limitação declarada — é uma função partida à espera de quem lhe clique.
- **Reescrever a ADR-0607 em vez de escrever esta.** Proibido pelo §68: uma
  decisão que muda é registada numa ADR nova, não emendada na antiga para deixar
  a pasta arrumada.

## Consequences

- Para ficheiros institucionais, **o Core transporta os bytes da descarga** —
  como já transportava os da pré-visualização e os da descarga pessoal. A
  excepção ao princípio «o Core não transporta bytes» (ADR-0607) deixa de estar
  contida ao caminho pessoal e passa a cobrir toda a descarga de ficheiros,
  institucional e pessoal, forçada pela mesma ausência de endpoint público.
- O **defeito latente** que a ADR-0607 registou está **resolvido**; deixa de
  existir descarga que aponta para um host inalcançável.
- Os endpoints `/download` presigned mantêm-se implementados e testados, sem
  consumidor na experiência. Não são código morto — são um contrato de API que
  volta a ser o caminho certo quando existir armazenamento com endpoint público.
- A superfície de bytes same-origin do Core cresce por dois endpoints; a
  disciplina é a de sempre — nome de ficheiro higienizado à porta, `nosniff`,
  `attachment`, autoridade reavaliada a cada pedido.
