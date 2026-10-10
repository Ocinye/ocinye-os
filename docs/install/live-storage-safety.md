# Prova de Segurança de Armazenamento do Live (D013, L0-S)

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

Estas condutas são do ambiente de arranque **do instalador**, tal como
verificado. Repetem-se no artefacto novo em `ocinye.mode=install`
(`P2-install-idle-overlay`: A2 alterado, os mesmos 1 245 184 bytes). **Não foram
corrigidas**: o modo de instalação mantém o contrato do OIE, e alterá-lo exige
decisão e cobertura próprias.

| Id | Achado | Condição | Gravidade proposta |
|---|---|---|---|
| D013-SF-01 | Um disco **não escolhido** com um ext4 por recuperar é **escrito** (diário reproduzido) antes de o instalador mostrar o primeiro ecrã | o disco fica antes do suporte em `/sys/block` (interno `nvme*`/`sd*` com CD ou pen) | Alta |
| D013-SF-02 | Partições de swap de discos não escolhidos são activadas | discos `sd*`, `hd*`, `vd*` | Média |
| D013-SF-03 | RAID e LVM de discos não escolhidos são montados/activados e os seus sistemas de ficheiros montados em leitura | membros presentes | Média |
| D013-SF-04 | Um disco interno com uma pasta `casper/` desvia o arranque e deixa uma consola de root do initramfs | disco preparado, antes do suporte | Média |

O veredicto `D013_AMD64_ISO_E2E_VALID` continua verdadeiro para o que mediu: os
discos ao lado do destino ficaram iguais **porque estavam depois do suporte e
desmontados em ordem**. O que não mediu foi o caso acima.

## Parte B — a guarda

Só nos modos `live`, `hardware-check`, `recovery` e em modo em falta ou
desconhecido; **nunca** em `install`. Vive em
[`infra/image/oie-rootfs`](../../infra/image/oie-rootfs), abaixo de qualquer
interface:

1. **Antes do udev** (`init-top/ocinye-blockguard`): decide pelo `ocinye.mode=`
   da linha de comandos; arma, e mascara as regras de montagem de RAID e de
   activação de LVM.
2. **Em cada dispositivo** (`01-ocinye-blockguard.rules`): `blockdev --setro`
   quando o dispositivo aparece, antes das regras que o sondam.
3. **Antes de o casper procurar** (`casper-premount/05ocinye_blockguard`):
   varredura de tudo o que já existir.
4. **O casper contido** (alteração feita na construção do initramfs, que falha
   se o guião já não for o esperado): só examina um ISO 9660 com a etiqueta
   `OCINYE_OS`, e o passo de swap não faz nada.

O OIE verifica a política pelo que o núcleo reporta e, se não se cumprir, fica
em **estado restrito**: só metadados de dispositivos, sem sondar assinaturas.

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

## Não-regressão do caminho de instalação

A infra-estrutura comum mudou (menu, initramfs, binário do OIE), pelo que a prova
de instalação existente correu por inteiro no artefacto novo:
`scripts/image-e2e.sh iso ocinye-os-f3fc6ba0863b-amd64-20261010T040118Z amd64`
→ **`RESULT iso-amd64 PASS`, 26 de 26 verificações**. São as 25 da fase A —
suporte protegido, disco pequeno recusado, escolha do disco, confirmação errada
não escreve, instalação offline, os dois discos ao lado iguais byte a byte,
identidade da raiz instalada, arranque do sistema instalado, primeiro arranque,
`NÃO RECLAMADO`, ciclo de reclamação completo, `RECLAMADO`, ESP e GRUB, sem swap —
mais uma nova: o menu esperou 40 s sem tecla e nada arrancou. O único passo que
mudou na prova foi escolher «Instalar» no menu (antes, a ISO entrava sozinha no
instalador ao fim de 10 s).

O artefacto congelado da fase A não foi alterado: a soma da ISO é a mesma antes e
depois de todos os ensaios.

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
- Resíduo: um disco interno com **ISO 9660** e a etiqueta `OCINYE_OS` (uma
  cópia do próprio suporte) continua a poder ser adoptado como suporte.

## Estado da frase forte

`StorageSafetyEvidence = INTENDED`. A imagem diz «Os discos internos devem
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

Cada ensaio termina numa linha `RESULT <nome> PASS|FAIL|INVALID` e deixa
`result.json` e o registo da consola série. `INVALID` é falha da bancada, nunca
um `PASS`. A evidência deste registo está em
`~/.cache/ocinye-image-builder/evidence/l0s/l0s-evidence-20261010.tgz`
(`f7b33d5a40dd259d3359f2a66bfe2220aac86428ebf908301b0aea48f68b28fa`), fora do
repositório.
