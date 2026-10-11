# ocinye-compositor

O compositor do Ocinye OS: um compositor Wayland mínimo, feito de propósito para
as superfícies nativas do sistema
([ADR-0031](../../docs/adrs/0031-rust-system-authority-and-native-system-surfaces.md)).
Não é um ambiente de trabalho: não tem painel, lançador nem lista de janelas.

```text
ocinye-compositor -- PROGRAMA [ARGUMENTOS…]
```

Toma o ecrã e os dispositivos de entrada de um posto, arranca o programa que lhe
é dado — a interface nativa do sistema — e mostra as janelas dele a ocupar o
ecrã inteiro. Quando esse programa termina, o compositor termina. O programa é
fixado pela unidade que arranca o compositor; nada em execução lhe pode pedir
outro.

## Fronteira

O compositor é dono das janelas **do sistema operativo**. As janelas das
aplicações do Workspace são do Gestor de Janelas do Workspace (ADR-0618), dentro
da superfície que este compositor mostra. Não há dois gestores de janelas a
competir: aqui, toda a janela de topo ocupa a saída e não se move nem
redimensiona.

## Como funciona

- Sessão pelo gestor de postos (`libseat`), sem correr como root quando há
  `seatd` ou `logind`.
- Ecrã pelo modo de vídeo do núcleo (DRM/KMS): a primeira saída ligada, no modo
  que o ecrã prefere. **Uma saída na v1.**
- Desenho em OpenGL ES sobre GBM, que o Mesa também dá em software: uma máquina
  sem GPU acelerada continua a ter ecrã.
- Entrada pelo `libinput`: teclado para a superfície com foco, ponteiro para a
  superfície por baixo dele.
- Sobre a biblioteca [Smithay](https://github.com/Smithay/smithay) 0.7 (MIT).

## Estado

`SPIKE_PASSED`, não integrado. Provado numa VM `arm64` com placa gráfica
virtual: toma o ecrã e mostra um cliente Wayland (captura pelo monitor do QEMU,
`infra/system-ui/bench/graphics_bench.py`). Ainda **não** tem: cursor desenhado,
superfície de bloqueio imposta pelo Session Service, camadas de confiança,
segunda saída, mudança de terminal virtual testada. Não correu em `amd64` nem em
hardware físico, e ainda não mostrou uma janela Tauri.

## Construir

Fora da workspace do host, como a janela do Installer: liga a pilha gráfica do
sistema de base e constrói-se nessa base, para a arquitectura de destino.

```bash
docker build --platform linux/arm64 -t ocinye-system-ui-build:arm64 infra/system-ui/build
```

```bash
docker run --rm --platform linux/arm64 -v "$PWD":/src -w /src/apps/compositor ocinye-system-ui-build:arm64 cargo build --release
```
