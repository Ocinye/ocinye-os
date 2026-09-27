# D15 · Contratos

Continua a numeração (G-01…G-09 no D0–D12, G-10…G-15 no D14).

| ID | Contrato | Proposta | Até existir |
|---|---|---|---|
| **G-16** | Runtime/Acerca | `GET /me/runtime` não é preciso: o runtime é do cliente. Precisa-se de `GET /instance/version` → `{ os_version, api }` (público ou autenticado) | versão lida do HTML servido |
| **G-17** | Nova versão do cliente web | cabeçalho `X-Ocinye-Build` em cada resposta (ou `GET /instance/version` periódico); o cliente compara com o seu e mostra a faixa | sem faixa |
| **G-18** | Browser Manager | módulo do **cliente** (não do Core) com API `open(url)`, `tabs()`, `focus(n)`, `close(n)`, `history()`; exposto a UI, ocsh e Nye pelo evento `ocinye-browser` (`detail.act/a` → `detail.result`). Opcional: `GET /browser/probe?url=` no Core devolve `{ embeddable }` lendo `X-Frame-Options`/CSP `frame-ancestors` sem cookies | detecção por tempo limite do iframe |
| **G-19** | Preferências do Browser | `GET/PUT /me/browser/preferences` (motor, destino de transferências, retenção, privacidade), `GET/POST/DELETE /me/browser/bookmarks`, `GET/DELETE /me/browser/history?range=`, `GET/PUT/DELETE /me/browser/site-permissions`. Histórico é **privado do membro**: sem acesso de administrador; retenção máxima por política da instância | tudo local ao cliente |
| **G-20** | Transferir para o Ocinye Files | Desktop: a casca faz o download para um ficheiro temporário e envia por sessão de carregamento existente (0027/0050) para `Meus ficheiros/Downloads`; `POST /files/personal/uploads` com `source_url` para proveniência | só «Este computador» |
| **G-21** | Carregar de Ocinye Files para um site | Desktop: `GET /files/{id}/content` com o token do membro, entregue pela casca ao input do webview; evento de actividade `file.shared_external {host}` | só o seletor nativo |
| **G-22** | Actualização da casca | canal de actualização da aplicação (fora do Core); a casca mostra o chip | — |
| **G-23** | Nye com contexto de página | `POST /ask` aceita `context: [{ kind: "browser_page", url, title, text_hash, text, private }]`; a política de IA decide (`private: true` exige consentimento explícito no pedido); a resposta devolve `sources` | Nye sem página: responde só com acções determinísticas |
| **G-24** | Política de posto dedicado | `GET /instance/settings` ganha `desktop.autostart_enforced`, `desktop.full_workspace_enforced`, `browser.download_destination_enforced` | sem imposição |
| **G-25** | Ligação nas notificações | cada notificação do Core leva `link` (`ocinye://…` exacto, com âncora quando existir); a casca e o `Notification` do Web abrem esse recurso | o clique abre a app, não o recurso |

## Fronteira de confiança (obrigatória)
- Webviews externos **sem** cookies, armazenamento nem cabeçalhos da origem do Ocinye. Partição própria por janela privada, apagada ao fechar.
- Nenhum webview externo recebe `ipc`/ponte nativa. Pop-ups externos → abas controladas.
- Páginas `ocinye://browser/*` são da origem do Ocinye; nunca se renderizam dentro de um webview externo.
- CSP do Ocinye Web intacta: o `iframe` de sites é `sandbox="allow-scripts allow-forms allow-popups allow-popups-to-escape-sandbox"` **sem** `allow-same-origin` em relação ao Ocinye; nunca `allow-top-navigation`.
- `verify.sh`: sem `eval`, sem `innerHTML` com dados de sites; textos de sites (títulos) por `textContent`.
