# ocinye-system-ui

A **interface nativa do sistema** do Ocinye OS: entrar, bloqueio, a barra, as
definições rápidas e a aplicação Sistema
([ADR-0031](../../docs/adrs/0031-rust-system-authority-and-native-system-surfaces.md)).
Tauri sobre o webview do sistema; a interface é TypeScript-sem-compilação
(JavaScript simples), HTML e CSS, com as formas do Design (System Experience).

**Apresenta o sistema; não controla nada.** A janela tem cinco comandos, e a
capacidade (`capabilities/default.json`) concede exactamente esses:

| Comando | O que faz |
| --- | --- |
| `session` | Um pedido tipado ao Session Service (entrar, bloquear, desbloquear, sair, prova fresca). Nunca `ConsumeGrant`. |
| `system` | Uma capacidade tipada do System Service, com a concessão quando há uma. |
| `binding` | A forma canónica de um pedido, pelo mesmo serializador que o serviço usa: é para ela que a prova fresca é emitida. |
| `surface` | Quanto ecrã a interface precisa: todo, ou só a barra. O compositor lê o estado da janela. |
| `open_workspace` | Abre a janela do Workspace. |

Os dois serviços recebem pedidos fechados e decidem sozinhos — por quem está
ligado ao socket e pelo estado da sessão — se respondem. Nada do que este
processo diga conta como autoridade. O webview não lê ficheiros, não corre
comandos e não chega à rede (CSP `default-src 'none'`, sem `style` em linha).

## A janela do Workspace

`ocinye-system-ui --workspace` é o mesmo programa noutro papel: um **processo
separado**, com uma janela chamada `workspace` que **nenhuma capacidade nomeia**
— não tem comando nenhum — e que fica na origem do Workspace. O endereço vem de
`/var/lib/ocinye/seat/workspace-url`, escrito pela instalação; não é um argumento. Por
ser outro processo, o compositor não o trata como superfície de confiança.

## Primeiro arranque

Sem Instância, o ecrã de entrada dá lugar a **Configurar este computador**
(`activation.start`), ao progresso, e à entrega — única — da credencial
temporária do primeiro administrador (`activation.credential`).

## Primeiro acesso

No ecrã de entrada, sem sair dele: quem traz uma credencial temporária escolhe
a palavra-passe; quem tem de ter segundo factor e ainda não tem inscreve-o (a
chave do autenticador é mostrada para ser escrita à mão — este ecrã não tem
câmara nem desenha um código QR) e recebe os códigos de recuperação, uma vez.

## Textos

Português, inglês e francês em `ui/strings.js`. O idioma é o do sistema
(`locale.status`); antes de haver sessão pode ser trocado no ecrã de entrada.

## Bancada

`ui-tests/` serve a interface num browser com um duplo dos dois serviços
(`fake.js`, cenários `#signed-out`, `#locked`, `#active`, `#offline`, `#mfa`,
`#down`). Serve para ver e medir a interface; não prova nada sobre os serviços.

```bash
python3 apps/system-ui/ui-tests/serve.py 18766
```

A prova no ecrã verdadeiro — compositor, serviços reais, teclado e ponteiro —
faz-se com `infra/system-ui/bench/` numa VM com placa gráfica virtual.

## Estado

`IMPLEMENTED`, provado numa VM `arm64`: entrar (com recusa e nova tentativa), a
barra com o Workspace por baixo, definições rápidas e Sistema com dados reais
do System Service. **Não** provado em `amd64` nem em hardware físico. O leitor
de ecrã está `NOT_YET_CERTIFIED`. Wi-Fi, Bluetooth e som aparecem como
indisponíveis porque o System Service ainda responde `NOT_IMPLEMENTED`.

## Construir

Fora da workspace do host, como a janela do Installer.

```bash
docker run --rm --platform linux/arm64 -v "$PWD":/src -w /src/apps/system-ui ocinye-system-ui-build:arm64 cargo build --release --features tauri/custom-protocol
```
