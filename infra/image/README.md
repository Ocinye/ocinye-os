# infra/image

O que entra numa imagem do Ocinye OS e como se constrói (D013).

| Caminho | O que é |
|---|---|
| `base.json` | a base Ubuntu fixada: série, digests por arquitectura, soma do `SHA256SUMS`, impressão digital e soma do anel |
| `keys/` | chaves **públicas** de confiança ([README](keys/README.md)) |
| `rootfs/` | fragmentos copiados para a raiz: sshd (bloco `Match User ocinye-claim`), sudoers, cloud-init (lista de permissões), netplan por omissão, unidades systemd, incluindo as do posto (ver abaixo) |
| `oie-rootfs/` | fragmentos **só** da raiz do OIE: a guarda de blocos do suporte (initramfs, udev, contenção do casper). Nunca numa raiz instalável: a inspecção offline recusa-a |
| `e2e/storage_proof.py` | a bancada da [Prova de Segurança de Armazenamento do Live](../../docs/install/live-storage-safety.md) |
| `build/provision.sh` | o que corre dentro da VM de construção, por fases (`common`, `virt`, `metal`, `oie`, `finalize`) |
| `builder/` | a VM Lima do construtor, o instalador do syft (`install-syft.sh`, escolhe o binário pela arquitectura **da VM de construção**, não a do alvo) e a soma fixada (`syft.sha256`) |
| `e2e/image_e2e.py` | o banco de ensaio das imagens (`scripts/image-e2e.sh`) |
| `tools/gen-console-strings.js` | gera as tabelas de texto das consolas a partir do `strings.js` do Design |

## O posto (ADR-0031)

Uma imagem instalável traz o System Service, o Session Service, o compositor e
a interface nativa do sistema, e as unidades que os ligam:

| Unidade | Papel |
|---|---|
| `ocinye-system.service` | a autoridade sobre a máquina (root), em `/run/ocinye/system.sock` |
| `seatd.service` | o gestor de postos: entrega ao compositor o ecrã e os dispositivos de entrada |
| `ocinye-session.service` | a sessão local; root **sem capacidade nenhuma**, sistema só de leitura, rede só de loopback |
| `ocinye-seat-start.service` | numa máquina com dispositivo de ecrã, arranca o ecrã depois de os dispositivos assentarem |
| `ocinye-display.service` | o compositor e a interface, como `ocinye-ui`, em `tty1`; o ecrã e a entrada vêm do `seatd` (o logind, sem polkit, recusa activar a sessão de quem não é root). **Nunca activada no arranque**: é a `ocinye-seat-start` que a arranca. Se o ecrã não levantar, `tty1` volta à consola de texto |
| `ocinye-seat-link.path` / `.service` | quando existe `/etc/ocinye/release.env` (há uma Instância instalada), aprende a chave pública de sessão pela porta de loopback do proxy e regista o endereço do Workspace |
| `ocinye-console@tty2.service` | a consola de texto e o código de emparelhamento, a `Ctrl+Alt+F2` do ecrã gráfico (e na linha série, como antes) |
| `iwd.service` | o Wi-Fi, que o System Service pede por D-Bus; só associa e autentica (`/etc/iwd/main.conf`: `EnableNetworkConfiguration=false`) |
| `bluetooth.service` | o BlueZ, que o System Service pede por D-Bus; sem agente de emparelhamento |

Uma máquina sem dispositivo de ecrã fica com a consola de texto em `tty1`.

O endereço de uma ligação Wi-Fi vem do `systemd-networkd`, por DHCP
(`/etc/systemd/network/80-ocinye-wifi.network`, que casa com `Type=wlan`), como
o das ligações com fios descritas pelo netplan. O som é ALSA (`alsa-utils`), sem
servidor de som. Não há NetworkManager. O suporte de instalação (OIE) traz os
pacotes, mas o `iwd` e o `bluetooth` ficam **mascarados** nele, e a inspecção
offline verifica-o (`no_radio_daemons`); numa imagem instalável verifica que
estão activados (`radio_daemons_enabled`).

### Activação local

Uma máquina sem Instância mostra, no seu próprio ecrã, **Configurar este
computador**: o System Service corre o instalador do release que a imagem traz
(`/usr/lib/ocinye/release/<id>/install/ocinye install …`) com os sete valores
pedidos — domínio, nome da Instância, distribuições, a pessoa e o primeiro
administrador —, sem rede e sem outro computador. É o mesmo instalador, a criar
a mesma Instância, que o Ocinye OS Installer conduz à distância: mais um
anfitrião da instalação, não outra maneira de instalar. A credencial temporária
do primeiro administrador é entregue **uma vez**, nesse ecrã, e sai da máquina
antes de aparecer.

O emparelhamento à distância continua disponível (consola de texto): é também o
caminho para administrar a máquina por SSH.

O Session Service fala com o Core por `127.0.0.1:8480`, a porta que o proxy
publica **só no loopback** (`infra/nginx/instance/20-machine.conf`) e por onde
só passa o que entrar numa máquina precisa.

Estado: `IMPLEMENTED`, **sem prova numa imagem construída** — o posto foi
provado numa VM com um Core de substituição (`infra/system-ui/bench/`), não
ainda com uma Instância instalada pela imagem.

### No suporte de instalação

O suporte (OIE) traz as mesmas peças gráficas, com outro programa no ecrã:
`ocinye-oie-display.service` arranca o compositor com
`ocinye-system-ui --installer`, e `ocinye-oie-window.service` serve-lhe a
conversa deste arranque (`ocinye-oie run --window`). Em `ocinye.mode=install` é a
conversa que os terminais têm, e a primeira consola a continuar fica com a
instalação, como entre o ecrã e a linha série; nos outros modos é a sessão só
de leitura desse modo, desenhada (passo `MEDIA`). A janela não tem capacidade nenhuma do sistema:
lê o que o motor diz e devolve uma linha.
