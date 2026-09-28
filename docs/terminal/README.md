# Ocinye Terminal e ocsh

O Terminal é a aplicação; o ocsh é a shell do Ocinye — gramática, parser,
registo de comandos e execução governada pelo Core. **Não é uma shell do
anfitrião.**

| Documento | Conteúdo |
|---|---|
| [CURRENT_STATE.md](CURRENT_STATE.md) | o que o repositório já tem e o Terminal reutiliza, e o que falta |
| [ARCHITECTURE.md](ARCHITECTURE.md) | fluxo, onde vive o código, decisões e fases M0–M10 |
| [COMMAND_MODEL.md](COMMAND_MODEL.md) | gramática, AST, definição de comando, risco, códigos de saída, contexto, inventário v1 |

Estado: **M0 (discovery e arquitectura)**. Nenhum comando existe ainda.
