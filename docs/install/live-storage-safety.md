# Prova de Segurança de Armazenamento do Live e da Instalação (D013, L0-S e L0-H)

> Registo de evidência, não de intenção. Tudo o que aqui se afirma foi medido
> numa bancada virtual (QEMU/OVMF, `amd64` emulado em TCG, dentro da VM do
> construtor) a 2026-10-10. **Hardware físico: `NOT_RUN`.** A decisão está na
> [ADR-0030](../adrs/0030-installation-media-boot-modes-and-live-storage-policy.md).

A pergunta: **arrancar e usar o «Experimentar o Ocinye OS» ou a verificação de
hardware escreve em algum disco que ninguém escolheu?**

## Como se mede

[`infra/image/e2e/storage_proof.py`](../../infra/image/e2e/storage_proof.py)
constrói discos-sentinela, arranca o suporte com eles ligados e compara.

Os sentinelas ligam-se como discos SCSI (`sda`, `sdb`, …) e ficam **antes** do
suporte (`sr0`) em `/sys/block`. É a ordem que uma máquina real apresenta (um
`nvme0n1` ou `sda` interno antes da pen ou do CD) e a ordem em que o casper
procura o seu suporte. A prova de instalação existente liga os discos como
`vd*`, que ficam **depois** de `sr0`: por isso nunca viu o que se regista abaixo.

| | Sentinela | O que representa |
|---|---|---|
| A1 | GPT + ext4, desmontado em ordem | disco de dados |
| A2 | GPT + ext4 com diário por recuperar (`needs_recovery`) | disco de um sistema que não desligou em ordem |
| B | partição EFI, FAT32 | ESP |
| C1, C2 | dois volumes físicos LVM, grupo `sentvg`, volume com ext4 | LVM |
| D1, D2 | dois membros de um RAID1 por software, sincronizado | mdraid |
| E | ESP + raiz ext4 com `fstab` + swap (tipos GPT de raiz e de swap) | instalação Linux existente |
| F | 64 MiB aleatórios, sem tabela de partições | dados arbitrários |
| G | GPT + NTFS | Windows |
| H | contentor LUKS2 | disco cifrado |
| I | GPT + partição de swap | swap |
| J | partições com as etiquetas `writable`, `casper-rw`, `OCINYE_OS` (ext4) | etiquetas a que o casper dá significado |
| K | ext4 com etiqueta `OCINYE_OS` e uma pasta `casper/` | falso suporte |
| L | ext4 directamente no disco, sem tabela | o «caso feio» do casper |

Nenhum sentinela é destino de instalação em nenhum ensaio.

Três testemunhas por sentinela:

1. **Soma do conteúdo** — SHA-256 do que o convidado podia ver, antes e depois.
   Como o sentinela é um ficheiro lido por inteiro, isto prova igualdade byte a
   byte.
2. **Registo de escritas** — o sentinela fica por baixo de uma camada qcow2
   vazia; qualquer escrita, mesmo de bytes iguais, ocupa lá um bloco. Zero bytes
   na camada = nenhum pedido de escrita chegou ao disco.
3. **Só-leitura no hipervisor** — o mesmo arranque com o sentinela protegido
   contra escrita pelo QEMU; uma tentativa de escrita aparece como recusa no
   registo do núcleo do convidado.

Mais: as variáveis UEFI depois de cada ensaio, comparadas com um ensaio de
controlo que só chega ao menu de arranque (firmware, shim e GRUB, sem núcleo),
com o mesmo hardware.

## Parte A — o ambiente de arranque existente

Suporte: o artefacto congelado da fase A,
`ocinye-os-f3fc6ba0863b-amd64-20261009T084117Z` (ISO
`0f05a955684ab2618313e1d536f8391d31a1c247194e495e89bf0940bbdc9b8c`), ligado em
só-leitura; a soma é a mesma depois de todos os ensaios. Arranca a sua única
entrada (o instalador) e fica parado no primeiro ecrã, sem tecla nenhuma.

| Conduta | Componente | Classe | Evidência |
|---|---|---|---|
| Sistemas de ficheiros internos montados em leitura à procura do suporte | casper, `check_dev` em `/scripts/casper` | `POTENTIAL_WRITE` | `EXT4-fs (sda1): mounted … ro` para A1, E, J |
| Diário ext4 recuperado numa montagem «em leitura» | núcleo, por causa da montagem acima | **`WRITE`** | A2: `recovery required on readonly filesystem` → `recovery complete`; 1 245 184 bytes escritos; `needs_recovery` desaparece; soma diferente. Reproduzido em 5 de 5 ensaios sem guarda |
| Partições de swap activadas | casper, `casper-bottom/13swap` (`USERNAME=ubuntu`) | `POTENTIAL_WRITE` | `Adding 162792k swap on /dev/sdh3`, `Adding 130024k swap on /dev/sdl1`; zero bytes escritos nos ensaios (sem pressão de memória) |
| RAID1 montado automaticamente | udev `64-md-raid-assembly.rules` no initramfs | `POTENTIAL_WRITE` | `md/raid1:md127: active with 2 out of 2 mirrors`; zero bytes escritos (conjunto limpo) |
| Volume LVM activado e o seu ext4 montado em leitura | udev `69-lvm.rules` + casper | `POTENTIAL_WRITE` | `EXT4-fs (dm-0): mounted … ro`; zero bytes escritos |
| Arranque desviado por um falso suporte | casper, `find_livefs` | segurança | com K ligado, o casper adopta `casper/fake.squashfs`, falha e deixa uma consola `(initramfs)` do BusyBox |
| Sondagem `blkid`, leitura de tabelas de partições | udev, núcleo | `READ` | sem escritas |
| NTFS, FAT, LUKS, dados arbitrários | — | `READ` | G, B, H, F sem alterações |
| `systemd-fstab-generator` | systemd | `READ` | o casper escreve um `fstab` só com `overlay` e `tmpfs` (`casper-bottom/12fstab`) |
| `systemd-gpt-auto-generator` | systemd | `NOT_PRESENT` (mascarado em `/etc/systemd/system-generators`) | sem unidades geradas |
| udisks, automontadores | — | `NOT_PRESENT` | ausentes da raiz |
| fsck | — | `NOT_PRESENT` | `fsck.mode=skip` |
| Procura da chave do operador (`key_roots`) | `ocinye-oie` | `UNKNOWN` neste ensaio | só corre depois da confirmação; não exercitada (monta `vfat`/`exfat`/`iso9660` em leitura) |
| Partição `writable` criada na pen (`find_or_create_persistent_partition`) | casper | `UNKNOWN` | existe no código quando não há `nopersistent`; **não reproduzida**: com a ISO numa pen de 1 GiB livre, zero bytes escritos no suporte |
| Variáveis UEFI | firmware, shim (`SbatLevel`) | fora do sistema | as mesmas variáveis que o ensaio de controlo sem núcleo |

Com os sentinelas em só-leitura no hipervisor, o mesmo arranque regista
`EXT4-fs (sdb1): write access unavailable, cannot proceed`, não monta o RAID e
não activa swap: a tentativa de escrita existe e foi recusada.

### Achados de segurança da D013 (modo de instalação)

Estas condutas eram do ambiente de arranque **do instalador**, tal como
verificado na fase A, e repetiam-se no artefacto do L0-S em
`ocinye.mode=install` (`P2-install-idle-overlay`: A2 alterado, 1 245 184 bytes).
O **L0-H** corrigiu-as. O registo original mantém-se; cada achado ganha a sua
disposição.

| Id | Achado | Condição | Gravidade | Estado (L0-H) |
|---|---|---|---|---|
| D013-SF-01 | Um disco **não escolhido** com um ext4 por recuperar é **escrito** (diário reproduzido) antes de o instalador mostrar o primeiro ecrã | o disco fica antes do suporte em `/sys/block` | Alta | `RESOLVED` (bancada virtual) |
| D013-SF-02 | Partições de swap de discos não escolhidos são activadas | discos `sd*`, `hd*`, `vd*` | Média | `RESOLVED` (bancada virtual) |
| D013-SF-03 | RAID e LVM de discos não escolhidos são montados/activados e os seus sistemas de ficheiros montados em leitura | membros presentes | Média | `RESOLVED` (bancada virtual) |
| D013-SF-04 | Um disco interno com uma pasta `casper/` desvia o arranque e deixa uma consola de root do initramfs | disco preparado, antes do suporte | Média | `RESOLVED` (bancada virtual) |
| resíduo | Um disco interno com uma cópia ISO 9660 do próprio suporte pode ser adoptado como suporte | cópia antes do suporte | — | `CLOSED` (bancada virtual) |

«Bancada virtual» é literal: nenhum destes estados foi medido em hardware
físico.

#### D013-SF-01 — `RESOLVED`

- **Causa.** À procura do suporte, o casper monta em leitura todos os sistemas
  de ficheiros que conhece, pela ordem de `/sys/block`; montar um ext4 com o
  diário por recuperar reprodu-lo, e isso é uma escrita.
- **Correcção.** A guarda de blocos arma em **todos** os arranques do suporte,
  instalação incluída (`bootmode::guard_must_arm`): cada dispositivo fica em
  só-leitura antes do udev e antes de o casper procurar, e a procura do casper
  fica contida ao suporte ligado a este arranque.
- **Regressão.** O teste que corre o guião `init-top` contra
  `guard_must_arm` linha a linha de comandos; na bancada, o sentinela
  `A2-ext4-dirty` em `H1`, `H2`, `H4` e `H5`.
- **Evidência.** Zero bytes e soma igual em `A2-ext4-dirty` nos cinco ensaios
  de instalação, e nenhuma linha `EXT4-fs … recovery complete` no registo do
  núcleo; em `H2`, com o disco protegido pelo hipervisor, o arranque chegou ao
  mesmo ecrã. Controlo `C0` (ISO da fase A, sem guarda): `A2-ext4-dirty`
  alterado e `EXT4-fs (sdb1): recovery complete`.

#### D013-SF-02 — `RESOLVED`

- **Causa.** O passo `casper-bottom/13swap` activa swap em `[hsv]d*`, e o
  núcleo aceita `swapon` num dispositivo marcado só-leitura.
- **Correcção.** O passo de swap do casper não faz nada com a guarda armada,
  agora também em `install`; e o OIE recusa mostrar a lista de discos se
  `/proc/swaps` tiver alguma entrada (`STORAGE_PROTECTION_UNVERIFIED`).
- **Regressão.** `policy::swap_activa_restringe`; o teste do fluxo que exige a
  recusa antes da lista de discos.
- **Evidência.** Directa, pelo registo do núcleo: nenhuma linha `Adding …k swap`
  em nenhum ensaio do L0-H, e o controlo `C0` mostra duas
  (`/dev/sdl1`, `/dev/sdh3`) — a bancada vê a activação quando ela existe.
  Indirecta, pelo portão do OIE: `H4` e `H5` chegaram à lista de discos, o que
  só acontece com a política verificada. O modo de instalação não imprime a
  linha `OCINYE-POLICY`, pelo que não há leitura de `/proc/swaps` registada
  nesse modo. `I-swap` e `E-linux-install` sem alteração.

#### D013-SF-03 — `RESOLVED`

- **Causa.** As regras udev de montagem de RAID e de activação de LVM actuam
  sobre qualquer membro que apareça.
- **Correcção.** Com a guarda armada as duas regras ficam mascaradas e os
  membros em só-leitura; o OIE recusa com um conjunto RAID ou um dispositivo
  `dm-*` presente. Depois da confirmação, um dispositivo empilhado só fica
  gravável se **todos** os seus membros pertencerem ao disco libertado.
- **Regressão.** `policy::depois_da_confirmacao_so_o_alvo_pode_ser_gravavel`;
  na bancada, o alvo `T-stale-target` traz um RAID e um LVM antigos.
- **Evidência.** `C1`, `C2`, `D1`, `D2` sem alteração em todos os ensaios, e
  nenhuma linha `md/raid1` nem de `device-mapper` sobre eles no registo do
  núcleo; o controlo `C0` mostra `md/raid1:md127: active with 2 out of 2
  mirrors`. `H4` e `H5` instalaram por cima do alvo com RAID e LVM antigos e só
  o alvo foi escrito.

#### D013-SF-04 — `RESOLVED`

- **Causa.** O casper adopta como suporte o primeiro sistema de ficheiros com
  uma pasta `casper/`.
- **Correcção.** O suporte é escolhido por `ocinye-oie select-medium` dentro do
  initramfs, e o casper só examina esse dispositivo (ver «Confiança no suporte»).
- **Regressão.** Os testes de `medium` e de `origin`; na bancada, os discos
  `K` e `L` (`--traps`).
- **Evidência.** Com `K` e `L` ligados antes do suporte, todos os ensaios
  chegaram ao instalador ou à sessão; nenhum caiu na consola do initramfs. O
  controlo `C1` (ISO da fase A com as mesmas armadilhas) acabou em
  `(initramfs)`, na consola do BusyBox, e nunca chegou ao instalador.

#### Resíduo da cópia ISO 9660 — `CLOSED`

- **Causa.** A etiqueta `OCINYE_OS` não distingue o suporte de uma cópia sua.
- **Correcção.** Ver «Confiança no suporte»: identidade por construção e
  ligação ao dispositivo de que o firmware arrancou.
- **Regressão.** `medium::a_copia_num_disco_interno_nunca_e_o_suporte`,
  `a_ordem_de_enumeracao_nao_decide`,
  `duas_copias_removiveis_sem_firmware_e_recusa`,
  `o_firmware_a_apontar_para_um_nao_suporte_nao_autoriza_outro_interno`.
- **Evidência.** Com uma cópia byte a byte da ISO num disco interno `sda`
  (antes do suporte), o suporte adoptado foi `sr0` (CD) e `sdq`/`sdr` (pen),
  sempre por `FIRMWARE_BOOT_ENTRY`; zero bytes escritos na cópia.

O veredicto `D013_AMD64_ISO_E2E_VALID` continua verdadeiro para o que mediu.

## Parte B — a guarda

Arma em **todos** os arranques do suporte (`boot=casper`), o de instalação
incluído, e nunca fora dele. Vive em
[`infra/image/oie-rootfs`](../../infra/image/oie-rootfs), abaixo de qualquer
interface:

1. **Antes do udev** (`init-top/ocinye-blockguard`): arma, e mascara as regras
   de montagem de RAID e de activação de LVM.
2. **Em cada dispositivo** (`01-ocinye-blockguard.rules`): `blockdev --setro`
   quando o dispositivo aparece, antes das regras que o sondam.
3. **Antes de o casper procurar** (`casper-premount/05ocinye_blockguard`):
   varredura de tudo o que já existir, e a escolha do suporte.
4. **O casper contido** (alteração feita na construção do initramfs, que falha
   se o guião já não for o esperado): só examina o suporte escolhido, e o passo
   de swap não faz nada.

O OIE verifica a política pelo que o núcleo reporta. Nos modos não destrutivos,
se não se cumprir, a sessão fica em **estado restrito**. No modo de instalação,
se não se cumprir, **a lista de discos não chega a ser mostrada**.

### Autoridade de escrita na instalação (L0-H)

Antes da confirmação: nenhum disco interno é gravável, o alvo incluído. Depois:
só o alvo confirmado.

- A autoridade nasce da confirmação escrita do OIE, não de um nome de
  dispositivo: `authorize_installation` devolve um `ConfirmedTarget`, que só
  existe num arranque `install` e para o disco cuja confirmação foi escrita.
- `release_target` volta a sondar o disco e exige que seja o mesmo
  (`same_target`), verifica a política antes e depois, regista a libertação em
  `/run/ocinye/released/<disco>` e só então o põe gravável, com as suas
  partições.
- Não há uma segunda máquina de estados: é o fluxo do OIE que já existia.
- A procura de chaves do operador só olha para discos removíveis.

### Confiança no suporte (L0-H)

O suporte não se reconhece por nome (`sdb`, `sr0`, `nvme…`) nem pela ordem de
enumeração.

- **Identidade.** Cada construção gera um identificador de 128 bits, gravado no
  initramfs e no descritor do volume ISO 9660 (`OCINYE-MEDIA-<id>`). Um
  candidato tem a etiqueta `OCINYE_OS` **e** essa identidade.
- **Ligação.** `BootCurrent` e a entrada `Boot####` do firmware dão o caminho
  físico do dispositivo de que a máquina arrancou; é esse o suporte.
- **Sem a palavra do firmware**, só se aceita um candidato removível único.
- **Uma cópia num disco fixo interno nunca é adoptada.** Ambiguidade recusa.

### O que a bancada desmentiu do desenho

- **A marca de só-leitura não impede `swapon`.** No primeiro artefacto de teste
  (`…-20261010T022341Z`, commit `1e73291`) a guarda estava armada, todos os
  dispositivos tinham `ro=1`, e o casper activou as duas partições de swap. Nada
  foi escrito, e o verificador pôs a sessão em estado restrito sozinho
  (`SWAP_ACTIVE /dev/sdh3`, `/dev/sdl1`) — mas swap activa é escrita possível. A
  correcção (commit `bff46ae`) contém o passo de swap do casper pelo nome.
- A marca **impede** a recuperação do diário ext4 e a montagem do RAID, como se
  esperava.

### Resultados — artefacto `ocinye-os-f3fc6ba0863b-amd64-20261010T040118Z`

ISO `3b144ecc91df44daa4bbb7a3fb3b55a49cd8d2e9534026b8d7e01e8c86a371a3`, commit
`bff46ae`, assinatura de desenvolvimento. Os quinze sentinelas (A–L) ligados em
todos os ensaios de sessão. Em cada um: o menu ficou 45 s sem tecla e nada
arrancou; depois a escolha explícita; inventário, relatório de hardware, rede,
pedido de instalação (recusado e explicado), inventário outra vez; desligar pela
sessão.

| Ensaio | Modo | Ligação | Política | Sentinelas alterados | Bytes escritos | Variáveis UEFI vs. controlo |
|---|---|---|---|---|---|---|
| `P2-live-overlay` | Experimentar | camada | `VERIFIED` (28 dispositivos) | 0 de 15 | 0 | iguais |
| `P2-live-readonly` | Experimentar | só-leitura | `VERIFIED` (28) | 0 de 15 | — | iguais |
| `P2-hwcheck-overlay` | Verificar o hardware | camada | `VERIFIED` (28) | 0 de 15 | 0 | iguais |
| `P2-hwcheck-readonly` | Verificar o hardware | só-leitura | `VERIFIED` (28) | 0 de 15 | — | iguais |
| `P2-recovery-overlay` | Recuperação (texto) | camada | `VERIFIED` (28) | 0 de 15 | 0 | iguais |
| `P2-restricted-overlay` | Experimentar, verificador com falha injectada | camada | `RESTRICTED` (`VERIFIER_FAULT_INJECTED`) | 0 de 15 | 0 | iguais |
| `P2-live-usb` | Experimentar, suporte numa pen com 1 GiB livre | camada | `VERIFIED` (31) | 0 de 15 | 0; **0 no suporte** | sem controlo com o mesmo hardware |
| `P2-menu-control` | só o menu | camada | — | 0 de 15 | 0 | (é o controlo) |

`VERIFIED` quer dizer, medido dentro do convidado: guarda armada, todos os
dispositivos e partições com `ro=1`, `/proc/swaps` vazio, nenhum conjunto em
`/proc/mdstat`, nenhum dispositivo `dm-*`, nenhuma montagem a partir de um
dispositivo de blocos que não seja o suporte. Nos registos do núcleo destes
ensaios não há nenhuma linha `EXT4-fs … mounted`, `Adding … swap` nem `md/raid1`.
Os discos K e L deixaram de desviar o arranque.

O estado restrito ficou provado duas vezes: por causa real (swap, no primeiro
artefacto) e por falha injectada no verificador (`ocinye.selftest=policy-fail`,
que só pode restringir: a guarda não o lê). Em ambos a sessão não sondou
nenhum disco («não inspeccionado») e o pedido de instalação mostrou o aviso.

## Parte C — instalação (L0-H), artefacto `ocinye-os-f3fc6ba0863b-amd64-20261010T071820Z`

ISO `2e4eb0c04fa3b5d2088d554092798ab4e21b5d9eeb40eee7594013ae896bb496`, commit
`237629b`, assinatura de desenvolvimento. Em todos os ensaios com «armadilhas»
estão ligados os discos `K` e `L`; com «cópia», um disco interno `sda` com a
ISO inteira.

| Ensaio | Modo | Suporte | Extras | Sentinelas alterados | Bytes na cópia | Suporte adoptado |
|---|---|---|---|---|---|---|
| `H1-install-idle-traps` | instalar, parado no 1.º ecrã | CD | armadilhas, cópia | 0 de 15 | 0 | `sr0`, `FIRMWARE_BOOT_ENTRY` |
| `H2-install-idle-readonly` | idem, sentinelas protegidos pelo hipervisor | CD | armadilhas, cópia | 0 de 15 | 0 | `sr0`, `FIRMWARE_BOOT_ENTRY` |
| `H3-install-idle-usb` | instalar, parado | pen | armadilhas, cópia | 0 de 15 | 0; 0 no suporte | `sdq`, `FIRMWARE_BOOT_ENTRY` |
| `H4-install-full-cdrom` | instalação completa | CD | armadilhas, cópia, alvo com RAID e LVM antigos | 0 de 15 | 0 | `sr0`, `FIRMWARE_BOOT_ENTRY` |
| `H5-install-full-usb` | instalação completa | pen | idem | 0 de 15 | 0; 0 no suporte | `sdr`, `FIRMWARE_BOOT_ENTRY` |
| `H6-live-traps` | Experimentar | CD | armadilhas, cópia | 0 de 15 | 0 | `sr0`; `VERIFIED` (32) |
| `H7-hwcheck` | Verificar o hardware | CD | armadilhas, cópia | 0 de 15 | 0 | `sr0`; `VERIFIED` (32) |
| `H8-recovery` | Recuperação | CD | — | 0 de 13 | — | `sr0`; `VERIFIED` (25) |
| `H9-restricted` | verificador com falha injectada | CD | — | 0 de 13 | — | `RESTRICTED` |
| `H10-menu-control` | só o menu | CD | armadilhas | 0 de 15 | — | (controlo) |
| `H11-live-usb` | Experimentar | pen | armadilhas, cópia | 0 de 15 | 0; 0 no suporte | `sdq`; `VERIFIED` (35) |
| `C0-legacy-control` | controlo: ISO da fase A | CD | — | **1 de 13** (`A2-ext4-dirty`) | — | `FAIL`, como se esperava |
| `C1-legacy-traps-control` | controlo: ISO da fase A | CD | armadilhas | **1 de 15** | — | `INVALID`: consola do initramfs, nunca chegou ao instalador |

Os dois controlos correm a ISO congelada da fase A (`…084117Z`), que não tem
guarda: existem para provar que a bancada vê as condutas dos achados quando
elas acontecem. A soma dessa ISO é a mesma antes e depois.

Esta tabela é da **segunda** execução da matriz. A primeira deu os mesmos
resultados, mas a sua evidência bruta perdeu-se com a VM do construtor, quando
o disco do anfitrião encheu; a que conta é esta, guardada em
`~/.cache/ocinye-image-builder/evidence/l0h/runs-v2/` (um `result.json` e um
registo série por ensaio), fora do repositório. Na primeira execução o ensaio
`H6` falhou por defeito da bancada — o percurso de sessão não recolhia a linha
do suporte —, corrigido no commit `6660a62`.

Nas duas instalações completas a bancada mediu os bytes escritos em cinco
momentos:

| Momento | Sentinelas | Cópia da ISO | Alvo |
|---|---|---|---|
| 1.º ecrã, nada escolhido | 0 | 0 | 0 |
| lista de discos mostrada | 0 | 0 | 0 |
| alvo escolhido, por confirmar | 0 | 0 | 0 |
| confirmação errada | 0 | 0 | 0 |
| instalação terminada | 0 | 0 | 4 865 261 568 (`H4`), 4 872 404 992 (`H5`) |

Na lista de discos só o alvo (`sdq`) era seleccionável. Os discos que trazem a
etiqueta do suporte (a cópia, `J`, `K`) aparecem como «este suporte, protegido»:
é conservador, e a redacção fica para a interface.

## Não-regressão do caminho de instalação

A prova de instalação existente correu por inteiro no artefacto do L0-H:
`scripts/image-e2e.sh iso ocinye-os-f3fc6ba0863b-amd64-20261010T071820Z amd64`
→ **`RESULT iso-amd64 PASS`, 26 de 26 verificações**: o menu espera sem tecla,
suporte protegido, disco pequeno recusado, escolha do disco, confirmação errada
não escreve, instalação offline, os dois discos ao lado iguais byte a byte,
identidade da raiz instalada, arranque do sistema instalado, primeiro arranque,
ciclo de reclamação completo, `RECLAMADO`, ESP e GRUB, sem swap. Uma primeira
tentativa nesse dia ficou `INVALID` — o disco do anfitrião encheu a meio — e não
conta.

A prova equivalente do artefacto do L0-S (`…040118Z`, commit `bff46ae`) deu
`RESULT iso-amd64 PASS`, 26 de 26. O artefacto congelado da fase A não foi
alterado.

## O que isto prova e o que não prova

- Prova, nesta bancada: os modos não destrutivos não escreveram um byte nos
  quinze sentinelas, não montaram nenhum sistema de ficheiros interno, não
  activaram swap, RAID nem LVM, não escreveram no suporte nem acrescentaram
  variáveis UEFI.
- **Não prova** hardware físico (`NOT_RUN`): NVMe, controladores reais,
  contadores de escrita do disco. É da fase de validação física.
- **Não correu**: corte de energia abrupto; XFS e btrfs com diário sujo; NTFS
  hibernado; a partição `writable` do casper numa pen com a ISO num esquema que a
  provoque.
- Os comandos de passagem directa (SMART) não consultam a marca de só-leitura;
  a sessão actual não os emite.
- **Não correu no L0-H**: um arranque em que o firmware não dá entrada de
  arranque (o recurso ao candidato removível único só tem testes unitários);
  duas pens com o mesmo suporte; a libertação do alvo em NVMe físico.
- A guarda continua a ter decisões em guiões de shell do initramfs; a decisão
  do suporte e a da política já são do binário em Rust.

## Estado da frase forte

`StorageSafetyEvidence = INTENDED`, sem alteração no L0-H: a evidência é de bancada virtual e está completa nesse âmbito; a validação física (Lenovo) continua por fazer. A imagem diz «Os discos internos devem
permanecer intactos.» A frase «Nada é escrito nos discos internos.» só passa a
poder ser mostrada quando um registo de certificação marcar o artefacto como
`CERTIFIED` — o que depende de decisão humana sobre esta evidência e da
validação física —, e mesmo então só num arranque cuja política se verifique.

## Reproduzir

Dentro da VM do construtor (`scripts/image-build.sh vm-create`):

```bash
sudo python3 infra/image/e2e/storage_proof.py sentinels --work /var/lib/ocinye-image-build/l0s
```

```bash
sudo python3 infra/image/e2e/storage_proof.py run --iso CAMINHO.iso --arch amd64 --work /var/lib/ocinye-image-build/l0s --name NOME --mode live --attach overlay --traps
```

```bash
sudo python3 infra/image/e2e/storage_proof.py run --iso CAMINHO.iso --arch amd64 --work /var/lib/ocinye-image-build/l0s --name NOME --mode install-full --attach overlay --traps --iso-copy --expect-binding FIRMWARE_BOOT_ENTRY
```

Cada ensaio termina numa linha `RESULT <nome> PASS|FAIL|INVALID` e deixa
`result.json` e o registo da consola série. `INVALID` é falha da bancada, nunca
um `PASS`. A evidência deste registo está em
`~/.cache/ocinye-image-builder/evidence/l0s/l0s-evidence-20261010.tgz`
(`f7b33d5a40dd259d3359f2a66bfe2220aac86428ebf908301b0aea48f68b28fa`), fora do
repositório.
