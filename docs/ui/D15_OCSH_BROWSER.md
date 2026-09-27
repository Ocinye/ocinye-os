# D15 · Adenda ao D14 — família `browser` do ocsh

| Comando | Capacidade | Resultado |
|---|---|---|
| `browser open <url>` | browser.open | «✓ Aberto na aba n · {título}» + janela |
| `browser tabs [--json]` | browser.tabs | tabela # · Título · Endereço · activa (# clicável → `browser focus n`) |
| `browser focus <n>` | browser.focus | «Aba n · {título}» |
| `browser close <n>` | browser.close | «Aba fechada: {título}» |
| `browser history` | browser.history | últimas 8 páginas (hora · título · URL) |
- Tudo pelo Browser Manager (G-18, evento `ocinye-browser`). Nunca controlo directo de webviews.
- Sem janela Browser: `browser open` cria uma; os outros devolvem «Não há Browser disponível» (exit 69) com a sugestão `open browser`.
- `open browser` e `window tile browser notes` funcionam pelo Gestor de Janelas como qualquer app.
- Histórico do Browser **não** é o histórico do ocsh nem o Audit Log.
- Implementado no protótipo `Ocinye Terminal.dc.html` (grupo Espaço de trabalho).
