# D15 · Runtimes

CSS: `static/ods-d15-runtime.css`. Referência: `Ocinye Browser.dc.html` (A–E, U, Z, AA, AB, AC; percursos 75, 76).

## Modelo (fixo)
| Runtime | O que é | Mostra-se ao membro? |
|---|---|---|
| **Ocinye Web** | a instância num navegador moderno; zero instalação; experiência completa | só em Definições › Runtime / Acerca |
| **Ocinye Web instalada (PWA)** | a mesma, em janela própria sem barra do navegador | idem |
| **Ocinye Desktop** | casca nativa fina (Windows/macOS/Linux) à volta do mesmo sistema | idem |
| **Ocinye Dedicated** | máquina cujo anfitrião mínimo arranca a casca como ambiente principal | idem |
- **Nunca** um selo «Web Mode/Desktop Mode» permanente. O runtime só se revela onde muda algo (Browser a degradar, transferências, definições).
- O anfitrião é responsável por kernel, controladores, hardware e processos. Nunca desenhar o Ocinye como sistema operativo do hardware.

## Web
- A interface é a mesma (Desktop, janelas, Lançador, apps, Terminal, Nye, widgets). Não existe «edição web» reduzida.
- **Sugestão de instalação** (`.ods-install-hint`, uma vez, dispensável): «Instalar o Ocinye — abre numa janela própria, sem a barra do navegador. É opcional: este separador continua a funcionar igual.» [Agora não] [Instalar] + ligação «Ou transferir o Ocinye Desktop para Windows, macOS ou Linux →». Instalar usa o `beforeinstallprompt` do navegador onde exista; sem suporte, a sugestão não aparece.
- Manifesto PWA: `display: standalone`, `start_url: /`, `scope: /`, ícones do sprite da marca, `theme_color` `--ods-navy-900`.
- **Nova versão** (`.ods-update-banner`, G-17): faixa informativa por baixo da barra de topo: «Há uma nova versão do Ocinye. Recarregue quando quiser — nada do que não guardou é perdido até lá.» [Recarregar]. Nunca recarregar sozinho.

## Desktop
- Janela nativa com a moldura do anfitrião (aceite diferenças entre plataformas); **dentro**, só o Ocinye Design System. Título da janela: «Ocinye Desktop · {host da instância}».
- Capacidades a mais: notificações nativas com ligação ao recurso exacto, área de transferência, diálogos nativos abrir/guardar, associações de ficheiros (opt-in), `ocinye://` registado, microfone/câmara, webviews do Browser, bandeja/barra de menus, arranque com a sessão (opt-in).
- **Ligar a uma instância** (`.ods-instance-connect`, Z):
  1. Campo «Endereço da instância» (`https://…`), recentes deste computador (nome, host, última vez). Rodapé: «O Ocinye Desktop não usa um directório central. … Se esta aplicação falhar, o mesmo endereço funciona em qualquer navegador.»
  2. Verificação visível: Ligação segura (TLS) · Identidade da instância (`GET /instance/branding`) · Versão compatível.
  3. **Confiar**: cartão com nome, host, certificado, impressão digital, versão. [Voltar] [Confiar e continuar]. Mudar de instância mostra: «Abre uma sessão separada. Nada é reutilizado: identidade, ficheiros e cookies ficam em cada instância.»
  4. Iniciar sessão — o **mesmo** ecrã do D3; nota «Não há conta nova nem identidade própria do Desktop.»
- **Actualização da casca** (`.ods-update-chip` + popover, AA): chip âmbar «Actualização» na barra de topo; popover «Ocinye Desktop 1.4.2 está pronto. Reinicie para actualizar; as janelas e o trabalho são retomados.» Aplicação 1.4.1 → 1.4.2 · Instância 0.9.4 · sem alteração. [Mais tarde] [Reiniciar agora]. A versão da instância e a da casca nunca se confundem.
- **Bloquear ≠ Terminar sessão ≠ Fechar a aplicação** (Definições › Aplicação Desktop › Sessão neste computador): bloquear esconde o ecrã e mantém tudo; terminar sessão termina na instância; fechar a aplicação mantém a sessão salvo política.
- **Bandeja**: abrir o Ocinye · notificações · estado · sair. Não é segunda navegação.
- **Arranque com a sessão**: opt-in explícito ou política da organização (então aparece bloqueado com o selo ORGANIZAÇÃO).

## Full Workspace
- Opção em Definições › Aplicação Desktop › Modo: Janela · Full Workspace. O Ocinye ocupa o ecrã; ao entrar, aviso de 5 s: «Full Workspace · o Ocinye ocupa o ecrã. Ctrl+Shift+F para sair; os atalhos do sistema anfitrião continuam a funcionar.»
- Sem modo quiosque por omissão: Alt+Tab/⌘Tab/Win continuam.

## Dedicated
- Percurso E: anfitrião mínimo (texto técnico discreto, «Kernel, controladores e hardware: sistema anfitrião») → ecrã da casca (marca + progresso + «a ligar a {host}») → **ecrã de bloqueio do Ocinye** (hora, data, avatar, palavra-passe, instância) → Desktop em Full Workspace.
- Arranque automático e Full Workspace podem ser impostos por política (G-24): as linhas aparecem bloqueadas com o selo ORGANIZAÇÃO.

## `ocinye://`
- Canal de sistema; o membro não precisa de o conhecer. Formas: `ocinye://files/<id>`, `ocinye://projects/<id>`, `ocinye://notes/<id>`, `ocinye://apps/<app>`, `ocinye://settings/<secção>`, `ocinye://browser/<página>`.
- **Dentro do Ocinye** (Browser, Terminal, Nye, notificações): abre a aplicação pela rota do Ocinye; a aba do Browser **não** navega. Toast «Ligação interna do Ocinye · aberta em {app}».
- **Fora do Ocinye** (AC): com Desktop instalado → cartão «A abrir no Ocinye Desktop…» → janela do recurso. Sem Desktop → URL web equivalente (`https://{instância}/{rota}`) num separador. Nunca exigir instalação para chegar a um recurso.
- Sem ligação ao Ocinye → toast de erro, nada abre.

## Offline parcial (AB)
- Barra de topo: chip vermelho «Ocinye indisponível · a religar».
- Browser (Desktop): a navegação externa continua; faixa âmbar na janela «Sem ligação ao Ocinye. A navegação continua. Guardar no Ocinye Files, Knowledge e Nye ficam indisponíveis até voltar.» Acções Ocinye desactivadas com motivo; «Este computador» continua disponível nas transferências.
- Nunca mostrar sucesso falso. O estado visual existente mantém-se.

## Definições › Runtime / Aplicação Desktop (U)
Cartão **RUNTIME**: nome do runtime + plataforma + capacidades (✓ disponível · – limitado · ✕ não disponível):
Instância (versão) · Integração nativa · Browser integrado («Limitado pela segurança do navegador» no Web) · Notificações · Ficheiros do computador · Ligações `ocinye://`. No Web, botões [Instalar o Ocinye] [Transferir Ocinye Desktop].
Secções (Desktop/Dedicated): Arranque e modo · Integração com o sistema · Sessão neste computador · Instâncias · Acerca. Cada linha leva o selo de onde vive:
| Selo | Classe | Significado |
|---|---|---|
| NESTE COMPUTADOR | `.ods-scope[data-scope="local"]` | posição de janelas, cookies do Browser, associações, arranque |
| NA INSTÂNCIA | `.ods-scope[data-scope="instance"]` | tudo o resto (Desktop, preferências, marcadores) |
| ORGANIZAÇÃO | `.ods-scope[data-scope="org"]` | imposto por política |
**Acerca**: Ocinye OS (instância) · Ocinye Desktop Shell (se houver) · Plataforma. Nada de detalhes do anfitrião além do nome e arquitectura para membros normais.

## Várias instâncias
Preparado, não obrigatório: lista em Instâncias; mudar é sempre explícito (passo de confiança + nova sessão). Nunca reutilizar sessão entre instâncias.

## Garantia permanente
Se o Ocinye Desktop falhar, o endereço da instância funciona em qualquer navegador. Isto aparece no ecrã de ligação, na ajuda e na documentação.
