# ADR-0027 — Ocinye OS Image Builder: base Ubuntu, formatos e reprodutibilidade

- **Estado:** Proposed
- **Domínio:** Foundation
- **Impacto:** HIGH
- **Depende de:** ADR-0026 · [ADR-0004](0004-rust-first.md) · ADR-0022 (Proposed)
- **Data:** 2026-10-05

Importada do pacote D013 do Claude Design (fase A, provisória sobre a D011), no formato desta biblioteca.

## Context

Hoje o operador instala Ubuntu Server 24.04 à mão e depois corre o Installer. Para servidores físicos, virtualização e cloud, o Ocinye OS precisa de imagens oficiais que já tragam a base, o runtime e um release verificado — sem criar uma distribuição nova nem um núcleo próprio.

## Decision

1. Base: a imagem cloud oficial **Ubuntu Server 24.04 LTS Minimal**, fixada por série e SHA-256, verificada pelo `SHA256SUMS.gpg` da Canonical. `/etc/os-release` continua o do Ubuntu. Sem interface gráfica.
2. Construtor `ocinye-image-builder` (Rust) que orquestra QEMU (montagem numa VM de construção), libguestfs (limpeza e inspecção offline), curtin (instalação em disco no ISO), casper/xorriso (ISO), qemu-img (formatos), syft (SBOM). Packer, debootstrap para a raiz de destino e o instalador interactivo do Ubuntu rejeitados.
3. Formatos v1: **ISO** (UEFI, com o ambiente de instalação OIE em texto), **QCOW2**, **RAW** (`.raw.zst` para transporte). Perfis `virt` e `metal` (este com `linux-firmware`). VMDK, VHDX, OVA e imagens de fornecedores cloud ficam FUTURE.
4. Conteúdo: Docker com versões exactas e chave conferida contra o manifesto do release, desligado até CLAIMED; exactamente um release Ocinye em `/usr/lib/ocinye/release/<id>/`; nove imagens OCI pré-carregadas por digest.
5. Disco: GPT, ESP 1 GiB, ext4, sem swap, sem cifra (FUTURE).
6. Entradas fixadas (série Ubuntu, snapshot apt, versões Docker, digests OCI, toolchain); manifestos canónicos com `SOURCE_DATE_EPOCH`; reprodutibilidade byte a byte é alvo, não afirmação.
7. Manifestos: `ImageContentManifest` embutido e `OcinyeImageManifest` publicado; SBOM SPDX 2.3; proveniência própria (sem afirmar SLSA).

## Consequences

Instalação offline da base; primeiro arranque rápido; uma imagem por release e revisão; tamanho maior do que uma imagem Ubuntu pura (imagens de contentores incluídas). Depende do pedido F-01 à D011 (digests das imagens de terceiros) para publicação.

## Implementação — D013 fase A de código (2026-10-05)

`PROVISIONAL_PENDING_D011_CERTIFICATION`, só construções `development` (o canal
estável falha fechado nos oito portões, `ocinye-image-builder stable-gate`).
`services/image-builder` corre como root numa VM Lima descartável
(`infra/image/builder`), nunca no computador do operador. Diferenças em relação
ao Design, registadas:

- **B11/B12 com `qemu-nbd` + `mount`**, não libguestfs: o disco desligado é
  anexado e limpo/inspeccionado sem arrancar, que é a propriedade que importava;
  a VM descartável já tem root, e evita-se o aparelho do libguestfs.
- **Rede de construção sem lista de permissões imposta** (`NETWORK_POLICY_VIOLATION`
  existe como tipo; o filtro de saída da VM de construção é `NOT_IMPLEMENTED`).
- **Versões Docker** resolvidas no momento da construção a partir do repositório
  com a chave conferida, `apt-mark hold` e registadas em `RuntimePins` — fixadas
  por registo, não escolhidas de antemão.
- **Contagem de imagens OCI** é a do manifesto do release, não um número fixo
  (o pacote `f3fc6ba0863b` tem 5 de release e 5 de terceiros, incluindo o MinIO
  ainda referido pelo Compose de produção).
- **ISO**: `casper` + `curtin` + `xorriso`, El Torito só UEFI com ESP anexada em
  GPT (arranca de CD virtual, media virtual BMC e pen USB); a carga é a raiz
  `metal` em squashfs (fonte `fsimage` do curtin); `IMAGE_MANIFEST.json` não
  entra no ISO (só `IMAGE_CONTENT.json`).
- A base Ubuntu fica fixada em `infra/image/base.json` (série, digests, soma do
  `SHA256SUMS`, impressão digital e soma do anel de chaves em `infra/image/keys/`).
- **Arquitectura da ferramenta ≠ do alvo.** O syft corre na VM de construção e
  lê a raiz montada como dados (`syft scan dir:<rootfs>`), sem `chroot` nem
  execução de binários do alvo: a sua arquitectura segue a da VM, não a da
  imagem. `infra/image/builder/install-syft.sh` escolhe-a por `uname -m`
  (`x86_64`→`amd64`, `aarch64`/`arm64`→`arm64`; desconhecida falha fechado) e
  verifica contra `infra/image/builder/syft.sha256`, a única fonte das somas. O
  SBOM continua por perfil (`metal`→ISO, `virt`→QCOW2/RAW), e a sua arquitectura
  é a da raiz do alvo. Em Apple Silicon: VM `arm64`, syft `arm64`, TCG para os
  passos `amd64`, e o syft lê a raiz `amd64` offline.
