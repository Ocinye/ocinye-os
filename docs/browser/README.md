# Ocinye Browser

A aplicação que navega a Internet dentro do Ocinye, sem nunca lhe dar a sessão,
os cookies, os ficheiros ou a ponte nativa do Ocinye.

| Documento | O quê |
|---|---|
| [`ARCHITECTURE.md`](ARCHITECTURE.md) | invariantes, modelo, API do Browser Manager, Web vs Desktop |

**Pertence aqui:** o Browser Manager, abas, navegação, transferências,
privacidade, a leitura governada de páginas pelo Nye. **Não pertence:** as
janelas do Ocinye (Gestor de Janelas), a casca nativa (em
[`../runtime/`](../runtime/README.md)).

Decisões: [ADR-0612](../adrs/0612-browser-manager.md) a
[ADR-0616](../adrs/0616-browser-page-context-for-nye.md) e
[ADR-0703](../adrs/0703-desktop-trust-boundary-and-native-bridge.md). Estado:
`PLANNED`.
