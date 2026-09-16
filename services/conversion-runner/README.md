# `conversion-runner` — a fronteira de conversão

Converte conteúdo **não confiável** em derivados (hoje, a miniatura da primeira
página de um PDF) sem que os parsers hostis toquem no worker da Ocinye. É a
implementação da [ADR-0609](../../docs/adrs/0609-disposable-conversion-isolation.md).

## O que pertence aqui

- **O runner** (`ocinye-conversion-runner`) — um sidecar HTTP na rede interna. É
  o **único** componente com o socket do Docker montado, e a única autoridade
  para criar contentores. Recebe bytes, corre um contentor descartável e
  endurecido, devolve o derivado, e apaga tudo. **Não faz parsing de conteúdo
  nenhum.**
- **O `ocinye-convert`** — o que corre **dentro** de cada contentor descartável.
  Lê `/in/input` (só-leitura), executa a ferramenta do perfil, escreve
  `/out/output`. Quase vazio de propósito.
- **O catálogo de perfis** (`lib.rs`) — a lista **fechada** de conversões que
  existem, com o tecto de recursos de cada uma. Partilhado pelos dois binários,
  para que um perfil que o runner aceite e o conversor não reconheça seja
  impossível.

## O que não pertence aqui

- **Autorização.** A posse do ficheiro já foi decidida no Core antes de o worker
  enfileirar a miniatura. O runner não conhece pessoas, sessões nem segredos.
- **Estado.** Cada conversão é um directório efémero, destruído a seguir. Nada
  persiste entre conversões.
- **Comandos arbitrários.** O runner aceita um **nome de perfil** de uma lista
  fechada, nunca um comando. Todos os argumentos do `docker run` são fixos; os
  bytes hostis viajam por um ficheiro montado só-leitura, nunca pela linha de
  comando.

## Forma de segurança (ADR-0609)

- **O worker não tem o socket do Docker.** Comprometer o worker não dá root sobre
  o host. O socket vive só aqui, contido nesta caixa mínima e auditada.
- **Cada conversão é descartável e endurecida:** `--network=none`,
  `--read-only`, input montado só-leitura, output temporário dedicado, sem
  segredos, `--user 65534`, `--security-opt no-new-privileges`, `--cap-drop=ALL`,
  tectos de CPU, memória, processos e tempo, destruída após.
- **O derivado é não confiável até validação.** A geração da miniatura
  re-codifica-o de raiz.

## Configuração

| Variável | Serviço | O que é |
|---|---|---|
| `OCINYE_CONVERSION_RUNNER_URL` | worker | onde encontrar o runner |
| `OCINYE_CONVERTER_IMAGE` | runner | a imagem do conversor descartável (obrigatória) |
| `OCINYE_CONVERSION_RUNNER_ADDR` | runner | onde escutar na rede interna |
| `OCINYE_CONVERSION_SPOOL` | runner | o spool, no mesmo caminho no host e no runner |
| `OCINYE_CONVERSION_MAX_BYTES` | runner | o tecto de bytes de entrada |

## Desenvolvimento

O isolamento **só existe no host Linux de produção**, onde o Docker corre com o
perfil que o suporta. Numa máquina de desenvolvimento sem o runner, uma miniatura
de PDF fica por gerar (o outbox volta a tentar) e a grelha cai no ícone; a
miniatura de imagem, que se descodifica em processo, não depende disto. Para
exercer o caminho completo localmente é preciso Docker e o runner a correr.

## Acrescentar um formato

Acrescentar Office ou vídeo é acrescentar uma linha a `PROFILES` (`lib.rs`), um
ramo em `ocinye-convert`, e a ferramenta à imagem do conversor. O worker continua
a pedir um nome de uma lista fechada; nada ganha autoridade nova.
