# Instalar o Ocinye OS num servidor físico (ISO)

> **D013 fase A de código — `PROVISIONAL_PENDING_D011_CERTIFICATION`.**
> Hardware físico real: **`NOT_RUN`**. Tudo o que aqui se descreve foi provado
> em máquinas virtuais UEFI (QEMU/OVMF) com o ISO como CD virtual; o caminho
> USB foi verificado só tecnicamente (estrutura do ISO híbrido), nunca gravado
> numa pen a partir deste repositório.

## Requisitos

- Firmware **UEFI** em `x86-64` (BIOS legado não suportado; Secure Boot não
  testado — se o firmware o exigir e o arranque falhar, desligue-o).
- **≥ 2 GB** de memória para o ambiente de instalação; o Ocinye OS mede-se em
  [hardware](hardware-results.md).
- Um disco de sistema de **≥ 20 GB**, que vai ser **apagado por inteiro**.
- Rede: **não é precisa para instalar**. É precisa depois, para reclamar e para
  o Installer.

## 1. Verificar a imagem

Antes de gravar seja o que for: [verificar](images.md#verificar) o `SHA256SUMS`
e a assinatura. Um ISO que não verifica não se usa.

## 2. Levar o ISO até à máquina

### Pen USB

O ISO é **híbrido**: tem uma tabela GPT com a partição EFI anexada, e o mesmo
ficheiro arranca de CD e de disco. Grava-se byte a byte, sem «extrair»:

```bash
sudo dd if=ocinye-os-<id>-r<N>-amd64.iso of=/dev/<a-pen> bs=4M conv=fsync status=progress
```

`/dev/<a-pen>` é **a pen inteira** (não uma partição), confirmada com `lsblk`
antes — o `dd` apaga o que lá estiver sem perguntar. No macOS, `diskutil list`
e `/dev/rdiskN`; no Windows, Rufus em modo «DD». Este repositório **nunca**
escreve numa pen: o comando é seu.

### Media virtual de um BMC (iDRAC, iLO, XClarity, IPMI)

Monte o ISO como CD/DVD virtual na consola do BMC, escolha o arranque único
pelo CD virtual (UEFI) e abra a consola remota (KVM) ou a consola série do BMC
(SOL). O ambiente de instalação fala nas duas: ecrã e porta série a 115200.

### CD virtual de um hipervisor

Anexe o ISO como CD e um disco em branco, firmware UEFI (OVMF).

## 3. O ambiente de instalação (OIE)

O menu de arranque mostra «Instalar Ocinye OS · <imagem>» (e
«DESENVOLVIMENTO» numa imagem de desenvolvimento). Depois, na consola:

1. **Boas-vindas** — nada é escrito em nenhum disco antes da confirmação.
   **L** muda a língua.
2. **Compatibilidade** — UEFI, arquitectura, memória. GPU não é exigida.
3. **Disco** — a lista de todos os discos, com modelo, número de série e
   conteúdo. **Nenhum vem escolhido.** O suporte de onde arrancou aparece como
   «ESTE SUPORTE · protegido», um disco montado como «montado · protegido», um
   disco pequeno demais e um sem caminho estável não se podem escolher. Dois
   discos iguais sem número de série aparecem marcados com `?`: escolha pelo
   caminho físico (porta) que a lista mostra.
4. **Confirmação** — escreva os **últimos 4 caracteres do número de série**
   do disco escolhido (ou o nome do dispositivo, se ele não tiver série). Outra
   coisa qualquer — «sim», vazio, a série de outro disco — pára sem escrever.
5. **Chave do operador** (opcional) — se ligar uma pen com o ficheiro
   `ocinye-operator.pub` (uma chave **pública** Ed25519 ou ECDSA, uma só), ela
   fica autorizada e o servidor pode ser reclamado sem código.
6. **Instalação** — confere o conteúdo do suporte contra os somatórios
   embutidos, particiona (GPT · ESP 1 GiB · ext4 · sem swap), escreve o sistema,
   confere o que escreveu, instala o carregador de arranque UEFI e escreve o
   diário em `/var/log/ocinye/oie-install.json`. Não desligue a máquina.
7. **Fim** — retire o suporte e reinicie.

## 4. Primeiro arranque e reclamação

A consola passa a mostrar «Por reclamar», o endereço, a chave de anfitrião em
grupos e o identificador de arranque. Para reclamar, ver
[imagens](images.md#o-ciclo-de-vida-de-uma-máquina); a seguir, a configuração
continua no [Installer](installer.md).

## Erros

| Ecrã | O que fazer |
|---|---|
| Nenhum disco serve | ligue um disco de pelo menos 20 GB e «Tentar de novo» |
| Suporte corrompido ou alterado | verifique a imagem e grave de novo |
| Confirmação não coincide | o disco não foi alterado; recomece |
| Falha ao instalar o carregador de arranque | confirme que o arranque UEFI está permitido; tente de novo |
| Falha de escrita | escolha outro disco ou substitua este |

## Recuperação e reinstalação

Reinstalar é arrancar o ISO de novo e escolher o disco (apaga-o). Um modo de
recuperação no menu de arranque está desenhado (`c13.rec.*`) e **não**
implementado na fase A.
