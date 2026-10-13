# ocinye-system-service

O **Ocinye System Service**: a autoridade privilegiada, e única, sobre uma
máquina com o Ocinye OS instalado
([ADR-0031](../../docs/adrs/0031-rust-system-authority-and-native-system-surfaces.md)).

Dois binários:

- `ocinye-system-service serve [SOCKET]` — o daemon. Corre como root sob o
  systemd, escuta em `/run/ocinye/system.sock` e serve o contrato de
  [`ocinye-system-contracts`](../../crates/ocinye-system-contracts/README.md).
- `ocinye-system` — o cliente sem ecrã: um subcomando por capacidade, os mesmos
  códigos de recusa, a mesma autorização. Não é uma shell.

## O percurso de um pedido

```text
pedido ─▶ posição ─▶ autorização ─▶ prova fresca ─▶ validação ─▶ adaptador ─▶ auditoria ─▶ resultado
```

- **Posição** (`authority`): quem liga é o que o núcleo diz (credenciais do par
  do socket) — operador de recuperação (root), a conta da interface, a do Core,
  ou ninguém.
- **Autorização antes da validação**: quem não pode pedir não recebe a descrição
  de como pedir bem.
- **Adaptadores** (`linux`, `display`, `audio`, `wifi`, `bluetooth`,
  `netplan`): `/proc` e `/sys` primeiro; depois D-Bus (`dbus`); uma ferramenta
  do sistema só através de `tool`, que fixa o caminho absoluto, limpa o
  ambiente, não usa shell, limita a saída e mata o processo no prazo. Um
  ficheiro escrito para outro programa é escrito por `files`: temporário ao
  lado, modo final desde o primeiro byte, `fsync`, `rename`.
- **Auditoria** (`audit`): linhas JSON em `/var/log/ocinye/system-audit.jsonl`,
  construídas de um resumo sem segredos.

## Estado

`IMPLEMENTED`, provado em imagens `arm64` e `amd64` construídas (`scripts/image-e2e.sh seat`) para o
primeiro bloco: estado do sistema, inventário, rede (leitura), energia, brilho,
hora, idioma, teclado, acessibilidade, armazenamento (leitura), estado de
actualizações, diagnóstico, acesso remoto e recuperação (leitura).

O segundo bloco está **implementado e testado contra duplos, sem prova numa
máquina** (nem VM, nem hardware): o que se pede ao sistema está medido; que o
sistema o faça, não.

| Família | Capacidades | Como |
|---|---|---|
| Ecrãs | `display.status` | Conectores DRM de `/sys/class/drm/card*-*`: ligado (`status`), activo (`enabled`), modo preferido (primeira linha de `modes`), interno (eDP, LVDS, DSI). Frequência e escala vêm `null`: são do compositor. |
| Som | `audio.status`, `audio.set_volume`, `audio.set_mute`, `audio.set_output` | ALSA, sem servidor de som: placas de `/proc/asound/cards`; nível e interruptor com `amixer -c N -M get/sset` (controlos `Master`, `PCM`, `Speaker`, `Headphone`; `Capture`, `Mic`), lidos estritamente — uma linha mal formada é resposta ilegível, não um número. A saída predefinida é `defaults.pcm.card`/`defaults.ctl.card` em `/etc/asound.conf`, escrito atomicamente, e só se o ficheiro não existir ou tiver sido escrito por este serviço. Sem placa: `HARDWARE_ABSENT`. |
| Wi-Fi | `wifi.scan`, `wifi.connect`, `wifi.disconnect`, `wifi.forget`, `wifi.set_radio`; `network.status` diz a rede em uso | iwd por D-Bus (`net.connman.iwd`): `Station.Scan` e `GetOrderedNetworks`, `Network.Connect`, `Station.Disconnect`, `KnownNetwork.Forget`, `Device.Powered`. Uma rede com palavra-passe é provisionada no formato do próprio iwd — `/var/lib/iwd/<nome>.psk` com `[Security]` `Passphrase=`, modo 0600, escrita atómica, nome em claro só com letras, dígitos, `-` e `_`, senão `=` e o nome em hexadecimal —, o buffer é zerado, e se a rede a recusar o ficheiro volta ao que era. A palavra-passe nunca é registada nem devolvida. O endereço vem do `systemd-networkd` (DHCP), não do iwd. |
| Bluetooth | `bluetooth.status`, `set_power`, `scan`, `pair`, `connect`, `disconnect`, `remove` | BlueZ por D-Bus (`org.bluez`): `Adapter1.Powered`, `StartDiscovery`/`StopDiscovery` (oito segundos), `Device1.Pair`/`Connect`/`Disconnect`, `Adapter1.RemoveDevice`; um dispositivo emparelhado fica `Trusted`. **Nenhum agente é registado**: emparelha o que não pede código; o que pede PIN ou confirmação é recusado pelo BlueZ e chega como `ADAPTER_FAILED { REFUSED }`. |
| Rede | `network.configure` | Só interfaces com fios: `/etc/netplan/90-ocinye-<iface>.yaml` (0600, atómico), DHCP ou fixo (endereço/prefixo, porta de saída na mesma rede, até quatro servidores de nomes), depois `netplan apply`; se falhar, o ficheiro anterior é reposto e aplicado outra vez. O identificador da definição começa por `00-` para que o `systemd-networkd` a prefira às `all-en`/`all-eth` da imagem. Wi-Fi: `NOT_IMPLEMENTED` (é DHCP pelo ficheiro da imagem). |
| Privacidade | `privacy.set_microphone`, `privacy.set_camera`; `privacy.status` | Microfone: captação desligada (`amixer … nocap`) em todas as placas com entrada, lembrado e reposto quando o serviço arranca; `audio.set_mute` não reabre uma entrada bloqueada. Câmara: a regra `/etc/udev/rules.d/70-ocinye-camera.rules` (`SUBSYSTEM=="video4linux", MODE="0000"`), `udevadm control --reload` e `udevadm trigger --subsystem-match=video4linux --action=change`; retirada quando volta a ser permitida. «Microfone em uso» é um `state: RUNNING` em `/proc/asound/card*/pcm*c/sub*/status`. |

Uma operação de Wi-Fi, de Bluetooth ou de endereçamento de cada vez: uma
segunda, enquanto a primeira corre, é `OPERATION_IN_PROGRESS`.

**Primeiro arranque.** `activation.configure` (presença, auditada como
segurança) leva a língua, o fuso horário e, opcionalmente, o endereçamento de
uma interface com fios, e corre o mesmo código de `locale.configure`,
`time.configure` (com o relógio da rede) e `network.configure`. Só é aceite
enquanto a máquina não tem Instância e se oferece para a criar (`READY` ou
`FAILED`); depois, `NOT_OFFERED_HERE` — essas mudanças passam a ser do
operador, pelas suas próprias capacidades.

### O que continua `NOT_IMPLEMENTED`, e porquê

- **`audio.set_input`**: em ALSA a placa predefinida é uma só para saída e
  entrada (`defaults.pcm.card`). Escolher uma entrada noutra placa pede um
  encaminhamento por direcção, que é trabalho de um servidor de som; a entrada
  segue a placa da saída.
- **`display.configure`**: quem põe modos e escala é o compositor. O serviço
  lê os conectores do núcleo e não escreve neles.
- **`update.prepare`, `update.apply`**: fora deste bloco.

### Limites conhecidos

- **Câmara em uso** vem sempre `false`: saber que processo tem um
  `/dev/video*` aberto não está em nenhum ficheiro barato do núcleo. Uma câmara
  já aberta quando se bloqueia continua aberta até ser fechada; o root ignora o
  modo 0000.
- **Microfone ligado depois do bloqueio**: uma placa que apareça depois de se
  bloquear só fica sem captação no próximo arranque do serviço ou no próximo
  pedido.
- **Wi-Fi empresarial** (802.1X): listado, recusado com `NOT_IMPLEMENTED`.
  WEP não é listado.
- **Recusas do iwd**: uma recusa de uma rede com palavra-passe
  (`net.connman.iwd.Failed`, `InvalidFormat`, `NoAgent`) é
  `WIFI_AUTHENTICATION_FAILED`; o iwd não distingue, no nome do erro, uma
  palavra-passe errada de outra recusa da rede.
- `network.configure` pede `LOCAL_REAUTH`: o operador volta a escrever a
  palavra-passe local no diálogo de confiança. Pedia `CORE_STEP_UP`, que o ecrã
  ainda não oferece, e o botão ficava sem efeito; a mudança é a mesma das
  actualizações (ADR-0032). Continua só de operador e auditada como segurança.

### Transporte D-Bus

`zbus` (API bloqueante): a ADR-0031 põe D-Bus antes de um subprocesso, e os
nomes dos erros do iwd e do BlueZ são tipados em vez de texto de uma
ferramenta. É Rust puro: o serviço continua um binário estático `musl`
(`x86_64` e `aarch64`, ligados com `rust-lld`). Tudo passa pelo traço `Bus`
(`dbus.rs`), com um duplo nos testes.

## Testes

```bash
cargo test -p ocinye-system-service
```

A matriz de recusas percorre o catálogo inteiro; os leitores correm contra uma
árvore `/sys` e `/proc` construída numa pasta temporária; o protocolo é exercido
por um socket Unix real. As ferramentas correm por um `Runner` e o D-Bus por um
`Bus`: os testes trocam-nos por um guião que responde com saídas gravadas
(`amixer`, `ip`) e objectos D-Bus enlatados (iwd, BlueZ), e verificam o que o
adaptador pediria ao sistema e o que decide com a resposta. Se o sistema o faz
prova-se numa imagem a correr — e para o segundo bloco isso ainda não foi
feito.
