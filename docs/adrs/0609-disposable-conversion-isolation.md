# ADR-0609 — Conversão de conteúdo não confiável em contentores descartáveis

- **Estado:** Accepted
- **Domínio:** Workspace
- **Impacto:** HIGH
- **Data:** 2026-09-16
- **Relaciona-se com:** [ADR-0607](0607-files-as-a-content-browser.md) ·
  [ADR-0608](0608-same-origin-institutional-downloads.md) ·
  [ADR-0200](0200-object-storage.md) ·
  [ADR-0500](0500-compute-registry-node-agent.md)

## Context

As miniaturas de Ficheiros ([ADR-0607](0607-files-as-a-content-browser.md))
mostram a primeira página de um PDF. Gerá-la é correr um **parser grande**
(`pdftoppm`) sobre bytes de um utilizador real — bytes potencialmente **hostis**.
Na primeira versão, isso corria como subprocesso **dentro do worker**, com um
prazo e `kill_on_drop`. Foi assumido como o mínimo defensável, e a própria
ADR-0607 registou o endurecimento como trabalho por fazer (§76).

O caminho a seguir amplia o problema, não o reduz: a seguir vêm **LibreOffice** e
**ffmpeg**, para miniaturas de Office e vídeo. São parsers enormes, com histórico
de vulnerabilidades de parsing e de execução de código a partir de documentos
manipulados. Corrê-los dentro do worker — o processo que escoa a fila inteira da
instituição — é fazer de cada PDF ou DOCX carregado uma aposta na segurança do
worker. Se um parser cair, o atacante encontra o worker.

Havia ainda uma restrição do terreno, medida no host de produção: o
`bubblewrap`, a escolha óbvia de sandbox, **não funciona** no contentor do worker.
O kernel do host permite user namespaces não-privilegiados
(`kernel.unprivileged_userns_clone=1`), mas o perfil default do Docker
(AppArmor/seccomp) recusa criá-los — `unshare --user` dá «Operation not
permitted». Habilitá-lo exigiria **enfraquecer** o AppArmor do worker, trocando o
confinamento global do contentor por uma jaula interna. Não é a direcção certa.

## Decision

Conteúdo não confiável converte-se num **contentor descartável e endurecido**,
criado por um **Conversion Runner** — um serviço à parte que é a única fronteira
com autoridade para criar contentores.

```
Core → preview job → Worker (orquestra, SEM socket do Docker)
    → Conversion Runner (sidecar; detém a autoridade de criar contentores)
        → contentor descartável, por conversão:
             --network=none · --read-only · input montado só-leitura
             output temporário dedicado · sem segredos · --user 65534
             --security-opt no-new-privileges · --cap-drop=ALL
             --pids-limit · --memory · --cpus · prazo · destruído após
    → Worker valida o derivado (ainda não confiável) → guarda a miniatura
```

1. **O worker nunca corre parsers hostis nem contentores.** Deixou de rasterizar
   em processo. Pede ao runner, por HTTP na rede interna, um **perfil de uma
   lista fechada** (`pdf-thumbnail` hoje), e recebe o derivado. No domínio, a
   rasterização passou a ser uma trait injectada
   (`thumbnail::ConversionBoundary`): o `ocinye-core` não conhece Docker nem
   subprocessos — conhece-os o worker, que constrói o cliente.

2. **O socket do Docker vive só no runner.** Montar `/var/run/docker.sock` é dar
   autoridade equivalente a root sobre o host. A decisão é **conter** essa
   autoridade numa caixa mínima e auditada — que não faz parsing de conteúdo
   nenhum e só sabe correr uma lista fechada de perfis — em vez de a espalhar
   pelo worker. Comprometer o worker **não** dá root sobre o host, porque o
   worker nunca teve o socket. O runner nunca aceita «corre este comando
   Docker»: aceita um nome de perfil, e todos os argumentos do `docker run` são
   fixos; os bytes hostis viajam por um ficheiro montado só-leitura, nunca pela
   linha de comando.

3. **O contentor de conversão é descartável e quase vazio.** Uma imagem separada
   com o `ocinye-convert` e a ferramenta do formato, e nada mais — sem rede, sem
   segredos, sem o resto do sistema. Corre com todos os tectos acima e é
   destruído após cada conversão. É a caixa que uma vulnerabilidade de parsing
   encontra, em vez do worker.

4. **O original é imutável e o derivado é não confiável até validação.** O
   conversor lê uma cópia só-leitura; o que produz é tratado como ainda não
   confiável — a geração de miniatura re-codifica-o de raiz numa WebP
   ([ADR-0607](0607-files-as-a-content-browser.md)), que é a validação.

A viabilidade foi **medida no host de produção** antes de se construir: o
contentor endurecido corre o `pdftoppm` (rede ausente confirmada, rootfs
só-leitura a bloquear escrita fora de `/tmp`), e o padrão de montagem irmã
funciona (input só-leitura, output por um spool no mesmo caminho no host e no
runner).

## Alternatives

- **`bubblewrap` no worker.** Bloqueado pelo perfil do contentor sem enfraquecer
  o AppArmor global — trocar confinamento a mais por confinamento a menos.
  Rejeitado.
- **seccomp + rlimits só no processo do worker.** Melhor do que nada e sem user
  namespaces, mas o parser continua no mesmo contexto operacional do worker, e um
  LibreOffice comprometido continua dentro do processo que serve a instituição.
  Rejeitado como solução final.
- **Montar o socket do Docker no worker.** É dar-lhe root sobre o host. É
  exactamente o que esta decisão evita.
- **Um contentor persistente de conversão.** Um contentor de longa duração
  acumula estado entre conversões, e um comprometimento persiste. Descartável por
  conversão não deixa nada para trás.

## Consequences

- Há um **serviço novo e privilegiado** — o Conversion Runner —, o único com o
  socket do Docker. Passa a ser o componente sensível a auditar: é pequeno, não
  abre bytes hostis, e só corre perfis de uma lista fechada. Um serviço a mais na
  topologia (`core-server`, `worker`, `node-agent`, `conversion-runner`).
- O worker ganha uma **dependência de runtime**: sem o runner, uma miniatura de
  PDF fica por gerar e o outbox volta a tentar; a de imagem, que se descodifica
  em processo, não depende disto. Uma miniatura em falta é o ícone — não uma
  avaria.
- O deploy passa a construir uma **imagem de conversor** (perfil `build`, não
  corre como serviço) e a garantir o **spool** no host. O runner precisa do
  socket e do spool no mesmo caminho; mais nada, e nenhum segredo.
- O isolamento **só existe no host Linux de produção**, e não se prova numa
  máquina de desenvolvimento sem Docker com este perfil. Cada peça verifica-se no
  host; a lógica pura, por teste.
- Fica preparado o caminho para **Office e vídeo**: acrescentar um formato é
  acrescentar um perfil à lista fechada e a ferramenta à imagem do conversor —
  sem que o worker ganhe autoridade nova, e sem que um parser novo alcance o
  resto do sistema.
