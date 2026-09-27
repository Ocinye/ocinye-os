# ADR-0208 — Um armazenamento de objectos mantido: Garage

- **Estado:** Accepted
- **Domínio:** Data
- **Impacto:** HIGH
- **Depende de:** [ADR-0200](0200-object-storage.md)
- **Data:** 2026-09-26

## Context

O Ocinye OS guarda os bytes num armazenamento S3-compatível
([ADR-0200](0200-object-storage.md)), e a implementação era o MinIO. Em Setembro
de 2026 o MinIO Community Edition foi arquivado: as imagens deixaram de ser
publicadas em qualquer registry, incluindo por digest, e o cliente `mc` deixou de
ser servido. A Parte 0 da generalização espelhou as imagens para a Ocinye, fixadas
por digest, como **ponte de compatibilidade**
([artefactos de terceiros](../deployment/third-party-artifacts.md)), e decidiu
que escolher um armazenamento mantido é condição de
`OCINYE_GENERAL_OS_BASELINE_READY`.

O que o Core pede ao armazenamento é pouco e está no código
(`crates/ocinye-core/src/storage.rs`): `PutObject`, `GetObject` em fluxo,
`DeleteObject`, e o carregamento em partes (`CreateMultipartUpload`,
`UploadPart`, `CompleteMultipartUpload`, `AbortMultipartUpload`), com a soma
calculada pelo Core. Nada de versões de objecto, políticas públicas nem eventos.

## Decision

**Garage** ([garagehq.deuxfleurs.fr](https://garagehq.deuxfleurs.fr)), servidor
S3-compatível escrito em Rust, mantido pela Deuxfleurs, pensado para correr num
anfitrião pequeno e crescer para vários — o mesmo desenho de Instância e Nó do
Ocinye OS.

**Evidência, antes de decidir.** A 2026-09-26, as cinco suites do Core que
exercitam o armazenamento correram contra o Garage `v2.1.0` num contentor local,
sem alterar uma linha de código:

| Suite | Testes |
|---|---|
| `segmented_upload` (multipart, retoma, somas) | 11 |
| `institutional_files` | 29 |
| `resource_storage` (quotas, posse) | 16 |
| `content_extraction` | 14 |
| `personal_notes` (imagens) | 33 |

103 testes, 0 falhas; 35 objectos e 196 MiB ficaram no bucket, com multipart
exercido — a prova de que os testes tocaram no Garage e não saltaram.

## Alternatives

- **Manter o MinIO espelhado.** Funciona hoje, sem correcções de segurança
  futuras. É a ponte, não o destino.
- **SeaweedFS** (Apache-2.0). Maduro e mantido; mais peças móveis (master,
  volume, filer) para o que o Ocinye pede. Fica como alternativa se o Garage
  falhar a prova completa.
- **Ceph RGW.** Resolve um problema de outra escala.
- **Um fornecedor cloud.** Contraria a operação sem dependências de cloud
  externa (§1-A).

## Consequences

- **Licença.** O Garage é AGPL-3.0. O Ocinye OS usa-o como serviço separado,
  sem modificações, a partir da imagem publicada, e fala com ele só por S3 e pela
  API de administração, pela rede. A leitura da Ocinye é que isto não torna o
  código do Ocinye OS uma obra derivada; **não é um parecer jurídico**, e quem
  distribua ou modifique o Garage deve ler a AGPL-3.0 com o seu próprio
  aconselhamento — uma Instância que o modifique e o sirva a terceiros tem, pela
  secção 13, de oferecer o código dessas modificações. Regista-se no inventário de
  terceiros.
- **Feito, e provado de raiz com o release `e62a1e5dc795`**, cada prova num
  anfitrião Linux descartável:
  - o serviço no Compose e no instalador: a chave e o bucket pela API de
    administração do Garage (`infra/garage/init.sh`), idempotente, a cada arranque;
  - o backup e o restauro com o `rclone` (MIT) em vez do `mc`;
  - a passagem MinIO → Garage (`scripts/object-store-cutover.sh`): PRE-CHECK,
    MIGRATE com o Core parado, VERIFY por `rclone check`, contagem e bytes e
    `verify-objects` — o Core recalcula a soma de cada objecto registado, lido do
    Garage —, CUTOVER, e ROLLBACK que devolve a Instância ao MinIO intacto;
  - instalação dos quatro perfis, actualização N → N+1 com a passagem, release
    falhado revertido, reversão manual de volta ao MinIO, backup e restauro noutro
    anfitrião com recusa sem a raiz de selagem e com uma errada, e a certificação
    de hardware nas duas classes.
- **Aceite a 2026-09-27**, com as duas condições que faltavam cumpridas:
  - o `verify.sh` completo passou contra o Garage no commit `01001f7` — as 125
    viagens de browser, as suites com base de dados e todos os portões;
  - a produção da Ocinye passou para o Garage no deploy do release
    `18eb8248db1b` (antes `f0fef6d72071`), pelo mesmo procedimento: 5 objectos,
    1 218 251 bytes, contagem e bytes iguais nos dois lados, e `verify-objects` a
    recalcular a soma de cada um, lido do Garage. O MinIO ficou parado com o
    volume `object-data` intacto, e `core.env.pre-garage` guarda a configuração
    para um rollback manual.
- **O que a passagem da produção ensinou.** O script corre como root e reescrevia
  a configuração com `install -m 600`; o Compose da produção corre como o
  utilizador de serviço, que lê `/etc/ocinye` pelo grupo, e não conseguiu ler o
  `object-store.env` novo. A passagem estava verificada e a troca feita, mas os
  serviços não levantaram até se repor `root:ocinye 0640`. As provas de anfitrião
  não o apanharam porque aí o Compose corre como root. O script passou a manter
  o dono, o grupo e o modo de cada ficheiro que reescreve.
- Os carregamentos em partes abandonados ficam como partes por terminar; o Core já
  os aborta quando a sessão expira, e o Garage tem limpeza própria.
