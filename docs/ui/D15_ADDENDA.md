# D15 · Adenda — o que faltava do programa

Fecha as secções 31, 46, 49, 52, 59, 72 e 84 do «Web, Desktop Shell & Integrated Browser — Complete Experience Design Program». Ecrãs novos no protótipo: **AD–AI**.

## 31 · Câmara e localização (AG)
- O pedido é o mesmo `.ods-browser-perm`, com o tipo em `data-kind="mic|cam|geo|notifications|clipboard"` e o ícone correspondente (`ods-mic`, `ods-camera`, `ods-location`, `ods-bell`).
- Câmara pede sempre **câmara e microfone** juntos quando o site pede os dois; nunca dois pedidos seguidos.
- Localização diz «Localização aproximada». Precisa só quando o site a pede; o Ocinye nunca envia a localização da instância ou do membro.
- Em uso: chip na barra (vermelho para câmara/microfone; neutro para localização) e ícone na aba.
- Respostas: Bloquear · Permitir uma vez · Permitir. Excepções por site em Definições › Permissões de sites, com «Repor».

## 46 · Notificações nativas (AD)
- **Desktop**: a casca envia a notificação pelo sistema anfitrião (aspecto do anfitrião, não do Ocinye). Conteúdo: app «Ocinye» · quando · título curto · 1 linha de contexto · acções [Abrir] [Marcar como lida].
- Cada notificação leva um `ocinye://` exacto (`ocinye://projects/p-0142#s3`). Clique no corpo ou em «Abrir» = foco na janela desse recurso (abre-a se não existir) no ponto certo. Nunca abre o Desktop genérico.
- Sem ligação à instância: a casca não inventa notificações; mostra as que o Core entregou antes.
- Preferência: Definições › Aplicação Desktop › Notificações nativas (NESTE COMPUTADOR). Desligadas → ficam só no sino do Ocinye.
- **Web**: `Notification` do navegador, só depois de o membro permitir; o clique foca o separador do Ocinye e abre o recurso. Sem permissão, só o sino.
- Contrato: o evento de notificação do Core ganha `link` (`ocinye://…`) — **G-25**.

## 49 · Bandeja / barra de menus (AE)
- Menu **nativo** do anfitrião (não é HTML): cabeçalho «Ocinye Desktop · {host} · Ligado/Sem ligação · IA local/sem IA» · Abrir o Ocinye · Notificações (n) · Bloquear o Ocinye (⌘L) · Sair do Ocinye Desktop (⌘Q). Nota: «Sair fecha a aplicação. A sessão na instância mantém-se, salvo política.»
- Não é uma segunda navegação: sem aplicações, sem pesquisa.
- Opt-in em Definições › Ícone na barra do sistema (NESTE COMPUTADOR).

## 52 · Ocinye Web sem ligação (AH)
- Diferente do Desktop: no Web, o próprio Ocinye deixa de falar com a instância.
- Faixa `.ods-offline-banner` por baixo da barra de topo: «Sem ligação à instância · A religar · tentativa n. O que está aberto continua visível; nada é guardado até voltar. Sites abertos noutros separadores do navegador não são afectados.» + chip vermelho na barra.
- O que está aberto mantém-se (sem ecrãs brancos). Acções que escrevem ficam desactivadas com o motivo. Nada de sucesso falso.
- No Browser (Web): as páginas internas continuam; abrir um site num separador novo continua a funcionar (é o navegador anfitrião).
- Ao voltar: «Ligação restabelecida» e a faixa desaparece. Se a sessão expirou entretanto → ecrã de bloqueio, não login novo.

## 59 · Ferramentas de programador (AI)
- **Desligadas** nas builds de produção; a linha em Definições do Browser › Programador diz «Desactivadas nesta build» (bloqueado).
- Em builds de desenvolvimento (ou com modo de programador por política): item «Ferramentas de programador ⌥⌘I» no menu da página, só em páginas externas.
- Abrem numa **janela à parte**, só para essa aba. Nunca sobre páginas do Ocinye, nunca com acesso à origem do Ocinye. Não são o Terminal nem o ocsh.

## 72 · Web responsivo (AF)
| Largura | Browser |
|---|---|
| ≥ 1024px | janela com barra completa, painel do Nye ao lado |
| 640–1023px | janela ocupa a área; painel do Nye por cima (já no CSS) |
| < 640px | **uma aplicação de cada vez**, sem janela nem trilho de aplicações; a barra esconde Avançar e Transferências (vão para o menu); abas roláveis; favoritos em 2 colunas |
- No mobile, o Browser usa o espaço todo; a barra de topo do Ocinye mantém marca, contexto, pesquisa, relógio e avatar.
- O Desktop Shell nunca é preciso para usar em mobile.

## 84 · Componentes pedidos → classes
| Componente do programa | Classe / origem |
|---|---|
| RuntimeIndicator | `.ods-runtime-card` + `.ods-runtime-cap` (Definições › Runtime); nunca permanente |
| DesktopShellFrame | moldura nativa do anfitrião (fora do DS) + `.ods-root` dentro |
| BrowserWindow | `.ods-window` (D5) + `.ods-browser` |
| BrowserTabs | `.ods-browser__tabs` / `.ods-browser__tablist` |
| BrowserTab | `.ods-browser-tab` |
| BrowserToolbar | `.ods-browser__toolbar` |
| BrowserAddressBar | `.ods-omnibox` |
| BrowserSecurityIndicator | `.ods-omnibox__sec[data-sec]` + popover `.ods-browser-pop` |
| BrowserPermissionPrompt | `.ods-browser-perm` (`.ods-browser-pop`) |
| BrowserDownload | `.ods-browser-dl` + diálogo `.ods-browser-dialog` |
| BrowserDownloadsPanel | `.ods-browser-downloads` + `ocinye://browser/downloads` |
| BrowserHistory | `.ods-browser-int` (`ocinye://browser/history`) |
| BrowserBookmarks | `.ods-browser-int__grid[data-side]` (`ocinye://browser/bookmarks`) |
| BrowserPrivateState | `.ods-browser[data-private]` + `.ods-browser__private` + `.ods-browser-start__private` |
| BrowserPageActions | `.ods-browser-menu` (`ods-menu`) |
| ExternalOpenFallback | `.ods-browser-err[data-kind="frame-blocked"]` |
| NativeIntegrationSetting | linha `.ods-settings__row` + `.ods-scope[data-scope]` |
| InstanceConnector | `.ods-instance-connect` |
| DesktopUpdateBanner | `.ods-update-chip` + `.ods-update-pop` (Desktop) · `.ods-update-banner` (Web) |
| OfflineRuntimeState | `.ods-offline-chip` + `.ods-browser__offline` (Desktop) · `.ods-offline-banner` (Web) |

## 84 · Claro/escuro — revisão
Todas as superfícies do Ocinye usam tokens, e as correcções do D11 cobrem-nas em `dark` e `system` (Q-34). Excepções deliberadas, iguais nos dois temas:
- **páginas externas** (viewport `data-page="site"`) mantêm o aspecto do site: fundo branco;
- **diálogos nativos** e **notificações nativas** são do anfitrião;
- **faixa privada** `--ods-browser-private` #2B3A4A com texto #DCE3F0 (contraste 9,4:1) nos dois temas;
- `.ods-browser-dialog` usa `--ods-surface-window`: fica escuro com o tema.
Verificar em `verify.sh` com os dois temas: contraste ≥ 4,5:1 no texto de `.ods-omnibox__host`, `.ods-browser-err__body`, `.ods-browser-nye__step`.
