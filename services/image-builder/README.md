# ocinye-image-builder

Constrói as imagens do Ocinye OS — ISO, QCOW2, RAW — a partir de um pacote de
release D011 e da base Ubuntu 24.04 Minimal verificada (D013,
[ADR-0027](../../docs/adrs/0027-image-builder-and-formats.md);
[guia](../../docs/install/images.md)). Fase A: só **desenvolvimento**; o canal
estável falha fechado (`stable-gate`). `PROVISIONAL_PENDING_D011_CERTIFICATION`.

Corre como root numa VM Linux descartável (`scripts/image-build.sh` cria-a a
partir de `infra/image/builder`), nunca no computador do operador. Passos
B01–B17, cada um com erro tipado: fonte, pacote, base (`gpgv` + digests fixados
em `infra/image/base.json`), VM de construção em camadas qcow2 com chave
efémera, `infra/image/build/provision.sh` por fases, limpeza e inspecção offline
(`qemu-nbd`), manifestos embutidos, artefactos, inventário, SBOM (syft),
proveniência, `IMAGE_MANIFEST.json`, `SHA256SUMS`, assinatura de
desenvolvimento e re-verificação com o mesmo código que o Installer usará.

Ferramentas na VM: QEMU, qemu-img, cloud-image-utils, gpgv, xorriso,
squashfs-tools, mtools, dosfstools, zstd, minisign, parted, syft (fixado).

**Arquitectura da ferramenta ≠ arquitectura do alvo.** O construtor e o syft
correm na VM de construção e a sua arquitectura é a dela (`BUILDER_PLATFORM`,
registado no arranque), independente de `--arch` (a arquitectura da imagem,
`TARGET_ARCH`). O syft lê a raiz montada como dados (`syft scan dir:<rootfs>`) —
sem `chroot`, sem executar binários do alvo —, por isso um syft `arm64` numa VM
`arm64` produz o SBOM de uma raiz `amd64`. `infra/image/builder/install-syft.sh`
escolhe o binário por `uname -m` da VM e verifica-o contra `syft.sha256`; uma
arquitectura de VM desconhecida falha fechado. Testes do mapeamento:
`infra/image/builder/install-syft.test.sh`.

Testes: `cargo test -p ocinye-image-builder`; a construção e as provas reais
com `scripts/image-build.sh` e `scripts/image-e2e.sh`.
