# Matriz de capacidades por runtime

> Duas colunas por runtime: o **alvo** (o que a arquitectura permite, ADRs
> 0018/0611/0612/0702) e o **estado** (o que existe hoje e está provado). Um
> `limitado` nunca se escreve `sim`.
>
> Desde a R1, as capacidades **de cliente** (diálogos, webviews, notificações,
> protocolo, área de transferência…) são código:
> `ocinye_contracts::runtime::RuntimeCapability::target`, e o `static/runtime.js`
> tem de dizer exactamente o mesmo para a Web
> (`apps/workspace/tests/runtime_boundary.rs`). Esta tabela é mais larga: junta
> as capacidades de produto (aplicações, Nye, Browser) e o estado de cada uma.

Legenda do estado: `CURRENT` provado · `PLANNED` decidido, por fazer · `—` não se aplica.

| Capacidade | Web — alvo | Web — estado | Desktop — alvo | Desktop — estado | Dedicated — alvo | Dedicated — estado |
|---|---|---|---|---|---|---|
| Aplicações do Core | sim | CURRENT | sim | PLANNED (R3) | sim | PLANNED (R11) |
| Desktop (D4) | sim | CURRENT (layout fixo) | sim | PLANNED | sim | PLANNED |
| Gestor de Janelas (D5) | sim | PLANNED (G-05) | sim | PLANNED | sim | PLANNED |
| Nye (determinístico) | sim | CURRENT | sim | PLANNED | sim | PLANNED |
| ocsh / Terminal | sim | ramo `feat/ocsh-terminal` | sim | PLANNED | sim | PLANNED |
| Browser integrado (sites dentro) | **limitado** — só sites que aceitam incorporação; os outros abrem num separador novo | PLANNED (R5) | sim (webview isolado) | PLANNED (R5) | sim | PLANNED |
| Texto da página para o Nye | **não** (origem cruzada) | — | sim, explícito | PLANNED (R9) | sim | PLANNED |
| Janela privada do Browser | **limitado** — o `iframe` usa o armazenamento do navegador anfitrião | PLANNED | sim (partição efémera) | PLANNED (R6) | sim | PLANNED |
| Transferência para Ocinye Files | **limitado** — transferir e carregar depois | CURRENT (carregar) | sim (stream governado) | PLANNED (R7) | sim | PLANNED |
| Transferência para o computador | sim (do navegador) | CURRENT | sim (diálogo nativo) | PLANNED (R7) | política | PLANNED |
| Seletor de ficheiros do computador | sim | CURRENT | sim | PLANNED | sim | PLANNED |
| Notificações nativas | **limitado** — Web Notifications, com permissão | PLANNED | sim | PLANNED (R10) | sim | PLANNED |
| Ligações `ocinye://` | **limitado** — resolvidas dentro do Ocinye; de fora, a rota HTTPS | PLANNED (R8) | sim (protocolo registado) | PLANNED (R10) | sim | PLANNED |
| Área de transferência | sim (API do navegador, gesto da pessoa) | CURRENT | sim | PLANNED | sim | PLANNED |
| Full Workspace | **limitado** — ecrã inteiro do navegador | — | sim | PLANNED | sim (política) | PLANNED |
| Instalação (PWA) | sim, opcional | PLANNED (R2) | — | — | — | — |
| Actualização | recarregar quando a pessoa quiser | PLANNED (G-17) | actualizador assinado | PLANNED | actualizador + política | PLANNED |
| Tempo real (WebSocket) | sim | **defeituoso** — ver `CURRENT_STATE.md` | sim | PLANNED | sim | PLANNED |
