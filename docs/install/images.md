# Imagens do Ocinye OS (D013) — ISO, QCOW2 e RAW

> **D013 fase A de código — `PROVISIONAL_PENDING_D011_CERTIFICATION`.** Só
> existem construções de **desenvolvimento**: cada imagem diz
> «COMPILAÇÃO DE DESENVOLVIMENTO · nunca para produção» na consola e no menu de
> arranque, é assinada só com uma chave de desenvolvimento, e o canal estável
> recusa construir (`ocinye-image-builder stable-gate`). Nenhuma imagem foi
> publicada. Decisões: [ADR-0026](../adrs/0026-release-and-image-signing.md) a
> [ADR-0029](../adrs/0029-image-machine-instance-identity.md) (`Proposed`).

Uma imagem do Ocinye OS é o **Ubuntu Server 24.04 LTS Minimal** oficial, fixado
por série e verificado pela assinatura da Canonical, com o runtime Docker
pré-instalado (seguro e desligado), **um** release Ocinye verificado em
`/usr/lib/ocinye/release/<id>/` e as imagens de contentores desse release
pré-carregadas por digest. Não traz identidade de máquina, palavra-passe, chave
nem Instância: tudo isso nasce depois.

| Formato | Para quê | Perfil |
|---|---|---|
| `…-amd64.iso` | servidor físico (pen USB, media virtual de BMC) ou CD virtual | `metal` (com `linux-firmware`) |
| `…-amd64.qcow2` | KVM/QEMU, Proxmox, OpenStack | `virt` |
| `…-amd64.raw.zst` | disco genérico (a soma do `.raw` descomprimido está no manifesto) | `virt` |

Arranque: **só UEFI** (BIOS legado `NOT_SUPPORTED_V1`); Secure Boot
`NOT_TESTED` (shim e GRUB assinados pela Ubuntu, mas não provado). Disco: GPT,
ESP 1 GiB, ext4 no resto, **sem swap**, sem cifra.

## O ciclo de vida de uma máquina

```text
ISO ─ OIE ─ disco escolhido e confirmado ─ instalado ─┐
QCOW2 / RAW ──────────────────────────────────────────┤
                                                      ▼
                      primeiro arranque (F1–F6) ─ POR RECLAMAR ─ reclamação ─ RECLAMADO ─ Installer (D011)
```

- **Primeiro arranque** (`ocinye-firstboot`): espera pela entropia do núcleo,
  gera as chaves de anfitrião Ed25519 e ECDSA e o identificador de arranque
  `ocb-…`, confere o release e as imagens pré-carregadas contra o
  `IMAGE_CONTENT.json` embutido, fecha a firewall (`ufw` só com `limit 22/tcp`) e
  gera um código de emparelhamento em memória. Sem identidade, ou com o release
  alterado, a máquina **não** fica «Por reclamar» e a consola diz porquê.
- **Reclamação** por SSH, sem serviço novo: a conta restrita `ocinye-claim`
  aceita qualquer chave **só enquanto** a máquina está por reclamar, com comando
  forçado e um protocolo fechado (`hello` · `enroll` · `abort`). O código —
  25 símbolos, 125 bits, uso único, 15 minutos, 5 tentativas — aparece só na
  consola da máquina, ao carregar em **P**. Depois de `enroll`, a confirmação
  faz-se como `ocinye` com a chave inscrita
  (`sudo -n /usr/lib/ocinye/ocinye-firstboot confirm --claim <id>`), em 10
  minutos; sem ela, a chave sai e volta um código novo. Numa cloud, a chave que
  a plataforma pôs em `ocinye` reclama directamente
  (`… claim --provisioned --key SHA256:…`).
- **Depois de reclamada**, o Docker é activado e a D011 continua como em
  qualquer servidor. A extensão da D011 que faz o Installer falar este protocolo
  e recusar executar antes de RECLAMADO é da fase B.

A consola (`tty1` e série) mostra o estado, o endereço, a chave de anfitrião em
grupos de quatro e o identificador de arranque, em português, inglês ou francês
(**L**). Bloqueio por tentativas: **U** na consola desbloqueia (presença física).

## Construir (desenvolvimento)

Numa máquina macOS Apple Silicon (M3 ou posterior, para a virtualização
encaixada) com o Lima:

```bash
scripts/image-build.sh build ocinye-os-f3fc6ba0863b amd64
```

O primeiro argumento é um pacote de release D011 em
`~/.cache/ocinye-installer-test/bundles`. O script compila os binários
estáticos, cria a VM descartável `ocinye-image-builder`
([`infra/image/builder`](../../infra/image/builder/lima-image-builder.yaml)) e
corre lá o [`ocinye-image-builder`](../../services/image-builder), que faz os
passos B01–B17: verifica a fonte, o pacote e a base Ubuntu (`gpgv` com o anel
fixado em [`infra/image/keys`](../../infra/image/keys/README.md)), monta numa VM
de construção a partir de camadas qcow2, limpa e inspecciona offline o que um
clone não pode partilhar, embute os manifestos, produz os artefactos, o
inventário de pacotes, o SBOM SPDX 2.3 (syft), a proveniência, o
`IMAGE_MANIFEST.json`, o `SHA256SUMS` e a assinatura de desenvolvimento — e
volta a verificar tudo. Imagens `amd64` são montadas com TCG (lento); `arm64`
com KVM.

A chave de desenvolvimento fica em `~/.cache/ocinye-image-builder/dev-signing/`
(gerada uma vez, `0700`), nunca no repositório nem numa imagem. A saída fica em
`~/.cache/ocinye-image-builder/dist/<nome>/`.

## Verificar

```bash
scripts/image-build.sh verify NOME amd64
```

ou, sem o construtor, com as ferramentas de qualquer operador:

```bash
minisign -Vm SHA256SUMS -x signatures/SHA256SUMS.minisig -p ocinye-dev.pub
```

```bash
sha256sum -c SHA256SUMS
```

Uma imagem de desenvolvimento verifica com a chave de desenvolvimento e
**só** no canal `development`; uma chave de desenvolvimento nunca torna uma
imagem «estável» (o verificador recusa, mesmo que a lista de chaves o permita).

## Provas

[`scripts/image-e2e.sh`](../../scripts/image-e2e.sh) arranca os artefactos em
QEMU dentro da VM do construtor e conduz-os como um operador: pela consola série
real (o código lê-se do ecrã depois de carregar em P) e pelo canal SSH de
reclamação. Ver [Resultados](#resultados).

## Resultados

`NOT_RUN` até a primeira construção e as provas correrem; preenchido só com o
que correu.

## O que não existe

Assinatura de produção, chaves de raiz e de release, listas assinadas
(`NOT_IMPLEMENTED`); canal estável (bloqueado pelos oito portões); PXE
(`FUTURE`); Secure Boot (`NOT_TESTED`); BIOS legado (`NOT_SUPPORTED_V1`);
cifra de disco (`FUTURE`); hardware físico real (`NOT_RUN`); VMDK/VHDX/OVA e
imagens de fornecedores cloud (`FUTURE`); a integração no Installer (descritor,
«Verificar imagem», reclamação pela janela, `image_source` no recibo) — fase B;
lista de permissões de rede na VM de construção (`NOT_IMPLEMENTED`); GPU, IA e
D012 (fora de âmbito); telemetria (não existe).
