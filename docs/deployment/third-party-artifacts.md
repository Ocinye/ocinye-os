# Artefactos de terceiros espelhados

Alguns artefactos de que o Ocinye OS depende deixaram de ser distribuídos por
quem os fazia. Quando isso acontece, o sistema continua a funcionar só enquanto
as cópias em cache sobrevivem — e uma CI, uma instalação nova ou uma
reconstrução de imagem descobrem-no da pior maneira.

Este documento regista os artefactos que a Ocinye passou a servir a partir de um
espaço que controla, porquê, e como se prova que são os mesmos.

## Classificação

> **`LEGACY_COMPATIBILITY_DEPENDENCY`** — um artefacto espelhado para manter o
> sistema actual reprodutível, **sem** ser a escolha de futuro. Tem sempre uma
> saída planeada, com parte do programa e portão.

Um espelho resolve **disponibilidade e reprodutibilidade**. Não resolve
**manutenção**: não traz correcções de segurança, e o código que contém não volta
a ser actualizado por ninguém.

## MinIO — servidor e cliente `mc`

**Porque.** O MinIO Community Edition foi arquivado pelo fabricante. Em
2026-09-26 verificou-se que as imagens `quay.io/minio/minio`,
`quay.io/minio/mc`, `minio/minio` e `minio/mc` já não se descarregam — nem por
etiqueta, nem por digest — e que `https://dl.min.io/client/mc/release/linux-amd64/mc`
responde `410 Gone`. Três coisas partiram com isso:

- a CI, que levanta um fixture S3-compatible antes dos testes;
- o backup nocturno de produção, cuja imagem descarrega o `mc`;
- qualquer instalação num host que não tenha as imagens em cache.

**O que se espelha.** Exactamente o que produção corre, e não o que o Compose
dizia que corria: no host de produção a etiqueta
`RELEASE.2025-04-22T22-12-26Z` estava reatribuída à imagem `latest` de
2025-09-07. O espelho reproduz essa imagem.

| | Servidor | Cliente |
|---|---|---|
| Versão | `RELEASE.2025-09-07T16-13-09Z` (commit `07c3a429bfed`) | `RELEASE.2025-08-13T08-35-41Z` (commit `7394ce0dd2a8`) |
| Índice de origem (quay.io/Docker Hub) | `sha256:14cea493d9a34af32f524e538b8346cf79f3321eff8e708c1e2960462bd8936e` | `sha256:a7fe349ef4bd8521fb8497f55c6042871b2ae640607cf99d9bede5e9bdf11727` |
| Manifesto `linux/amd64` — idêntico à origem | `sha256:a1a8bd4ac40ad7881a245bab97323e18f971e4d4cba2c2007ec1bedd21cbaba2` | `sha256:eb4ea9884b77704230e2423e9004d2fa738dc272876b9cc41a297d29443b8780` |
| Manifesto `linux/arm64` — idêntico à origem | `sha256:9966a92a734f9411e32f4f41d7d9d826fcdc0f68c4e20b70295bd4e7c11f8a2f` | `sha256:37d109dddbbb2c95873f5fc81ac93f37023264770fc580a7564148892087b1b7` |
| Índice espelhado (amd64 + arm64) | `sha256:55f2ff7d2834fd5f6c76c1c9f54855a325549b1960e7fa3d1f220c62af447a09` | `sha256:d00c9868670a5e56a1710322cd98f7e4faa7291b23cdf31f59e90d44ec2a0156` |
| Destino | `ghcr.io/ocinye/third-party/minio-server` | `ghcr.io/ocinye/third-party/minio-mc` |
| Licença | GNU AGPL v3 — `/licenses/LICENSE` e `/licenses/CREDITS` dentro da imagem, intactos | idem |
| Base | Red Hat UBI 9 Micro (redistribuível sob os termos UBI) | idem |

**Porque o índice espelhado tem outro digest.** O índice de origem lista também
`linux/ppc64le`, e esse manifesto já não existe em lado nenhum: não se pode
republicar um índice que aponta para o que não há. O índice espelhado contém as
**mesmas entradas, copiadas byte a byte**, para `amd64` e `arm64`. Os manifestos
por plataforma — que são o que um host efectivamente corre — têm o digest da
origem.

**Como foi produzido, e como se re-verifica.**

1. `amd64`: `docker save` das imagens em execução no host de produção.
   `arm64`: `docker save` das mesmas etiquetas numa estação de trabalho, com o
   mesmo índice de origem.
2. Cada blob foi verificado contra o seu nome (`sha256` do conteúdo = nome do
   ficheiro): 40 blobs, zero divergências.
3. Os blobs foram publicados sem recompressão (`crane`, layout OCI), pelo que os
   digests dos manifestos por plataforma se mantêm.
4. Para re-verificar: `crane manifest ghcr.io/ocinye/third-party/minio-server@sha256:55f2…`
   devolve as duas entradas acima, e `crane digest --platform linux/amd64 …`
   devolve `sha256:a1a8bd4a…`.

**Como se consome.** Sempre por digest, nunca por etiqueta:

```text
ghcr.io/ocinye/third-party/minio-server@sha256:55f2ff7d2834fd5f6c76c1c9f54855a325549b1960e7fa3d1f220c62af447a09
ghcr.io/ocinye/third-party/minio-mc@sha256:d00c9868670a5e56a1710322cd98f7e4faa7291b23cdf31f59e90d44ec2a0156
```

**Saída — feita (ADR-0208, secção seguinte).** Antes de `OCINYE_GENERAL_OS_BASELINE_READY = TRUE`, a fase de
Host/Storage do programa de generalização escolhe um store S3-compatible
mantido, com ADR, testes de compatibilidade, migração dos dados, rollback e E2E
([arquitectura-alvo §6](../architecture/TARGET_OCINYE_OS.md#6-itens-obrigatórios-que-nasceram-da-parte-0)).
Até lá, este espelho **não** recebe actualizações, e nenhuma etiqueta nova se
acrescenta a ele.

## A saída: Garage, rclone e curl (ADR-0208)

O MinIO deixou de ser o armazenamento do Ocinye OS: o **Garage** substitui-o no
Compose de produção, no de desenvolvimento, na CI e no instalador, e o **rclone**
substitui o `mc` no backup, no restauro e na passagem dos dados. O MinIO acima
fica só como `object-store-legacy`, por perfil, para a transição de Instâncias
que já o usavam ([`object-store-cutover.sh`](../../scripts/object-store-cutover.sh)),
e sai do Compose depois de todas as conhecidas terem passado.

Estes três **não** são dependências de compatibilidade: são mantidos por quem os
faz. Consomem-se da origem, sempre por digest; o espelho na Ocinye é uma reserva,
com os mesmos digests, para o dia em que a origem deixar de os servir.

| | Garage | rclone | curl |
|---|---|---|---|
| Papel | armazenamento S3 da Instância | cliente S3 do backup, do restauro e da transição | imagem mínima do `garage-init` em desenvolvimento e na CI |
| Versão | `v2.1.0` | `1.75.1` | `8.16.0` |
| Origem, por digest | `dxflrs/garage@sha256:4c9b34c113e61358466e83fd6e7d66e6d18657ede14b776eb78a93ee8da7cf6a` | `rclone/rclone@sha256:45401ad7410db1d67ffdb58e19059ad20b0d8e0285a60e38bbec55cc1019c7a5` | `curlimages/curl@sha256:463eaf6072688fe96ac64fa623fe73e1dbe25d8ad6c34404a669ad3ce1f104b6` |
| Espelho (reserva) | `ghcr.io/ocinye/third-party/garage:v2.1.0`, mesmo digest | `ghcr.io/ocinye/third-party/rclone:1.75.1`, mesmo digest | — |
| Licença | GNU AGPL v3 | MIT | curl (MIT/X derivada) |
| Como se usa | serviço separado, sem modificações; configuração em `infra/garage/garage.toml` | binário copiado para a imagem de backup | só corre o `infra/garage/init.sh` |

**Licença do Garage.** O Garage corre como um componente separado, num
contentor próprio, a partir da imagem publicada pelos seus autores, e esta
integração **não modifica** o seu código-fonte. As obrigações de distribuição que
a AGPL v3 impõe a quem distribui ou modifica o Garage documentam-se aqui e
avaliam-se à parte; este documento regista factos técnicos, e não é parecer
jurídico. Quem publicar uma imagem do Garage modificada tem de o reavaliar.

**Espelhos privados.** Os pacotes `third-party/garage` e `third-party/rclone`
nasceram privados no GHCR, e a API do GitHub não muda a visibilidade de um
pacote. Por isso o Compose, a CI e a imagem de backup usam a origem por digest;
tornar os espelhos públicos é um gesto de quem administra a organização, e só
então passam a ser consumíveis sem autenticação.

## IBM Plex — fontes do Workspace

Servidas pelo próprio Workspace (`apps/workspace/static/fonts/`), e não por uma
CDN: a CSP é `font-src 'self'`, e nenhum terceiro vê quem abre o Ocinye OS.

| | Plex Sans | Plex Mono |
|---|---|---|
| Origem | pacote npm `@ibm/plex-sans@1.1.0`, `fonts/complete/woff2` | `@ibm/plex-mono@2.5.0`, idem |
| Pesos | 400, 400 itálico, 500, 600, 700 | 400, 500, 600 |
| Licença | SIL Open Font License 1.1 — `static/fonts/LICENSE.txt`, intacta | idem |
| Modificações | nenhuma | nenhuma |
