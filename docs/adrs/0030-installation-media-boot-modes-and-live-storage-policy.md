# ADR-0030 — Modos de arranque do suporte de instalação e política de armazenamento da sessão Live

- **Estado:** Proposed
- **Domínio:** Foundation
- **Impacto:** HIGH
- **Depende de:** ADR-0027 · ADR-0028 · ADR-0029
- **Data:** 2026-10-10

## Context

O suporte de instalação do Ocinye OS (D013) arrancava para uma só coisa: o
ambiente de instalação (OIE). Passa a oferecer também «Experimentar o Ocinye OS»,
uma verificação de hardware e um diagnóstico em modo de texto. Uma sessão que diz
«não instala nada» tem de o cumprir abaixo de qualquer interface, e o suporte não
pode entrar no instalador por omissão, por tempo ou por engano.

A auditoria do ambiente de arranque existente
([registo L0-S](../install/live-storage-safety.md)) mostrou que essa promessa não
era verdadeira por construção: antes de qualquer código do Ocinye correr, o casper
monta em leitura os sistemas de ficheiros que encontra à procura do seu suporte —
e uma montagem em leitura de um ext4 com diário por recuperar **escreve** no
disco —, activa as partições de swap que encontra, e o udev monta RAID por
software.

## Decision

1. **Quatro modos, um parâmetro, fixos durante o arranque.** `ocinye.mode=` com
   `live`, `install`, `hardware-check` e `recovery`. O contrato tipado vive em
   `ocinye_image_contracts::bootmode` e é a única fonte: o menu de arranque, a
   guarda do initramfs e o OIE derivam dele. O modo lê-se uma vez da linha de
   comandos do núcleo e não tem forma de mudar.
2. **Modo em falta, desconhecido ou contraditório resolve para `live`** — a
   política não destrutiva —, nunca para o instalador.
3. **Só `install` alcança o código que escreve.** `flow::run` exige um
   `InstallAuthority`, que só se obtém de um arranque cujo modo é `install`,
   nomeado explicitamente; não se clona, não se desserializa, não se constrói
   noutro sítio. As acções do caminho destrutivo (escolher disco, confirmar,
   instalar) são recusadas com `NOT_INSTALL_MODE` nos outros modos.
4. **Menu de arranque sem temporizador e sem arranque por omissão.**
   `timeout=-1`; o foco inicial está em «Experimentar o Ocinye OS», e foco não é
   execução. Sete escolhas: Experimentar, Instalar, Verificar o hardware,
   Avançado / Recuperação, Idioma, Reiniciar, Desligar.
5. **Política de armazenamento por modo.** Em `live`, `hardware-check` e
   `recovery`: nenhum dispositivo de blocos interno é escrito; nenhum sistema de
   ficheiros interno é montado, nem em leitura; sem swap, sem montagem de RAID,
   sem activação de LVM. Em `install` (L0-H): o mesmo até à confirmação escrita
   do OIE; depois dela, só o disco confirmado fica gravável.
6. **A guarda fica abaixo da interface, em três partes**, só na raiz do OIE:
   um guião `init-top` que decide antes do udev e arma em todos os arranques
   do suporte (`boot=casper`), o de instalação incluído (L0-H);
   uma regra udev que põe cada dispositivo de blocos em só-leitura à medida que
   aparece, antes das regras que o sondam, montam ou activam; e uma varredura
   antes de o casper procurar o suporte. Com a guarda armada, o casper só
   examina o suporte deste arranque — etiqueta `OCINYE_OS`, identidade da
   construção e ligação à entrada de arranque do firmware —: a procura
   é contida por uma alteração feita ao guião do casper na construção do
   initramfs, que falha a construção se o guião já não for o esperado.
7. **A guarda nunca chega a um sistema instalado.** Os ficheiros vivem em
   `infra/image/oie-rootfs`, só a fase `oie` os instala, e a inspecção offline
   recusa uma raiz `virt` ou `metal` que os traga. O guião não arma fora do
   suporte.
8. **Estado restrito.** O OIE verifica a política a partir do que o núcleo
   reporta (marca da guarda, `ro` de cada dispositivo, swap, RAID, mapper,
   montagens). Se não se cumprir, a sessão fica restrita: só metadados de
   dispositivos de blocos, sem sondar assinaturas, nunca um recurso a escrita.
   «Instalar» continua disponível porque só reinicia, com o aviso próprio.
9. **Live → Instalar é um reinício normal.** A sessão explica, reinicia pelo
   firmware, o menu aparece e espera, e o operador escolhe Instalar. Sem kexec,
   sem `BootNext`, sem marca no suporte, sem autorização transportada.
10. **A frase forte é evidência, não intenção.** `StorageSafetyEvidence` tem
    dois valores; a construção escreve sempre `INTENDED`, e `CERTIFIED` só pode
    vir de um registo de certificação da Prova de Segurança de Armazenamento.
    Mesmo certificado, um arranque cuja política não se verifique não a mostra.

## Alternatives

- **Só esconder os discos na interface** — não é protecção; o casper escreve
  antes de haver interface.
- **Parâmetros do núcleo apenas** (`nopersistent`, `systemd.swap=0`, …) —
  úteis e mantidos, mas não cobrem a montagem feita pelo casper nem um modo em
  falta, que arrancaria sem eles.
- **Converter uma sessão Live numa de instalação, libertando um disco** —
  retirado por decisão do operador: não se converte uma sessão de leitura numa
  de escrita. O que o L0-H adoptou é diferente: a guarda arma também no
  arranque `install`, e só o OIE, com a sua confirmação escrita, liberta o
  alvo.
- **kexec ou `BootNext` para chegar ao instalador sem menu** — fora de âmbito na
  v1: um segundo mecanismo de transição, e uma escrita no firmware que o Live
  não pode fazer.
- **Um initramfs próprio em vez do casper** — uma segunda implementação, por
  provar, do que o casper já faz.

## Consequences

Com o L0-H, o modo de instalação deixa de ter as condutas do casper observadas
na auditoria: os achados D013-SF-01 a 04 e o resíduo da cópia ISO 9660 estão
resolvidos em bancada virtual
([registo](../install/live-storage-safety.md)). A decisão continua `Proposed`
porque a evidência é virtual, o hardware físico não correu, e a guarda ainda
tem decisões em guiões de shell do initramfs que devem passar para um único
ponto de entrada em Rust. A sessão de texto dos modos não
destrutivos é uma superfície provisória de bancada, não a interface do produto.
A alteração ao guião do casper acompanha a versão do casper da base fixada: uma
actualização que o mude falha a construção, por desenho.
