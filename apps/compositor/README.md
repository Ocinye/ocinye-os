# ocinye-compositor

O compositor do Ocinye OS: um compositor Wayland mínimo, feito de propósito para
as superfícies nativas do sistema
([ADR-0031](../../docs/adrs/0031-rust-system-authority-and-native-system-surfaces.md)).
Não é um ambiente de trabalho: não tem painel, lançador nem lista de janelas.

```text
ocinye-compositor -- PROGRAMA [ARGUMENTOS…]
```

Toma o ecrã e os dispositivos de entrada de um posto e arranca o programa que
lhe é dado — a interface nativa do sistema. Quando esse programa termina, o
compositor termina. O programa é fixado pela unidade que arranca o compositor;
nada em execução lhe pode pedir outro.

## A superfície de confiança

A janela do processo que o próprio compositor arrancou é a **superfície de
confiança**. É reconhecida pelo que o núcleo diz do cliente (o PID do processo
filho), nunca pelo que o cliente diz de si — um título ou um identificador de
aplicação são texto de quem os escreve.

- **Sem sessão activa**, a superfície de confiança tem o ecrã inteiro e mais
  nenhuma janela é mostrada nem recebe entrada. O compositor pergunta ao
  Session Service, uma vez por segundo, se há sessão; **não obter resposta não
  é um sim**.
- **Com sessão activa**, a superfície de confiança é a barra no topo (44 px) e a
  janela de aplicação mais recente — o Workspace — tem o resto. Quando a
  interface pede o ecrã inteiro (painel rápido, Sistema, um diálogo), volta a
  cobrir tudo.
- **Sem superfície de confiança, nada é mostrado**: uma aplicação nunca fica
  sozinha com o ecrã.
- **O ponteiro é desenhado pelo compositor**: o que a pessoa vê debaixo da mão
  não é uma imagem que uma aplicação forneceu.
- **`Ctrl+Alt+F<n>`** muda de terminal virtual, como em qualquer Linux: a
  consola de texto, com o código de emparelhamento, vive num deles. Nenhum
  cliente vê essas teclas.
- **Bloqueio por inactividade**: este é o processo que vê o teclado e o
  ponteiro. Quando passa o tempo que a política da Instância fixa (o Session
  Service diz o número), pede o bloqueio ao Session Service e trata o ecrã como
  bloqueado desde logo.

## Fronteira

O compositor é dono das janelas **do sistema operativo**. As janelas das
aplicações do Workspace são do Gestor de Janelas do Workspace (ADR-0618), dentro
da superfície que este compositor mostra. Não há dois gestores de janelas a
competir: aqui, nenhuma janela se move nem se redimensiona — cada uma tem a área
que a política lhe dá.

## Como funciona

- Sessão pelo gestor de postos (`libseat`), sem correr como root quando há
  `seatd` ou `logind`.
- Ecrã pelo modo de vídeo do núcleo (DRM/KMS): a primeira saída ligada, no modo
  que o ecrã prefere. **Uma saída na v1.**
- Desenho em OpenGL ES sobre GBM, que o Mesa também dá em software: uma máquina
  sem GPU acelerada continua a ter ecrã.
- Ao cliente dá memória partilhada (`wl_shm`) e nenhum `linux-dmabuf`; arranca-o
  com `WEBKIT_DISABLE_DMABUF_RENDERER=1` e `WEBKIT_DISABLE_COMPOSITING_MODE=1`,
  para que a interface nunca dependa de GL no processo web. Numa imagem `amd64`
  construída, sem estas variáveis, o processo web do WebKitGTK 2.52 morria no
  `llvmpipe` e a janela ficava em branco com o compositor levantado; qualquer
  uma das duas, sozinha, fazia a interface aparecer na mesma imagem.
- Entrada pelo `libinput`: teclado para a superfície com foco, ponteiro para a
  superfície por baixo dele.
- Sobre a biblioteca [Smithay](https://github.com/Smithay/smithay) 0.7 (MIT).

## Estado

`IMPLEMENTED`, provado numa VM `arm64` com placa gráfica virtual
(`infra/system-ui/bench/`): toma o ecrã, mostra a interface do sistema (Tauri
sobre WebKitGTK), recebe teclado e ponteiro, dá a barra à superfície de
confiança e o resto à janela do Workspace, e esconde-a quando a interface pede o
ecrã. **Não** provado: `amd64`, hardware físico, segunda saída, mudança de
terminal virtual. Ainda **não** tem: cursores por contexto (há uma seta só),
toque, e escala de saída diferente de 1.

O desenho é concluído (`glFinish`) antes de cada troca de página: um
renderizador por software não tem uma barreira que o núcleo possa esperar, e sem
isso o ecrã mostrava a imagem a meio de ser feita.

## Construir

Fora da workspace do host, como a janela do Installer: liga a pilha gráfica do
sistema de base e constrói-se nessa base, para a arquitectura de destino.

```bash
docker build --platform linux/arm64 -t ocinye-system-ui-build:arm64 infra/system-ui/build
```

```bash
docker run --rm --platform linux/arm64 -v "$PWD":/src -w /src/apps/compositor ocinye-system-ui-build:arm64 cargo build --release
```
