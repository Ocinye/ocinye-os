# CODE_FEEDBACK — integração da D001.2

De: Claude Code (integração) · Para: Claude Design · Revisão: **D001.2**
(sobre a D001.1) · Ramo `feat/design-d001-2`.

A D001.2 foi aplicada sem alterações (dois CSS e o HANDOFF). Nenhum ViewModel,
registo, predefinição ou contrato do Core mudou. Verificado no browser contra o
sistema real (Core + Workspace + PostgreSQL), Research, pt, tema claro, com a
predefinição do registo (sem disposição do membro; «Repor» devolve
`layout: null`).

## Correcção ao que este ficheiro dizia

A integração da D001.1 registou «4 colunas a 924×540» como paridade atingida.
**Estava errado:** a referência a 924×540 é uma composição de 2 colunas
(Calendário e Tarefas 1×2 ocupam meia largura cada). A D001.2 corrigiu a regra
no código; a entrada antiga fica só como história em `design-integration.json`.

## Fechado pela D001.2 (confirmado em execução)

| D001.1 | Resultado medido |
|---|---|
| Pastilha CORE · IA transparente | `#F3F6F9` sem passar o rato; hover/aberto, premido e foco dourado presentes |
| Grelha em x=18 com a barra escondida | x=36; Indicadores em x 37/254/470/687 contra 36/253/470/687 da referência, y=71 nos dois |
| Títulos a 1 coluna cortados | nenhum título nem subtítulo cortado em nenhuma largura testada; «Calendário» e «Armazenamento» numa linha a 924 |
| Indicadores 3 + 1 a 639px | 2 + 2 a 639px de área — mas ver o defeito abaixo, logo acima da fronteira |
| Login 28px mais alto que a captura | captura arquivada; o código é canónico: logótipo 52px em x=436, cartão 360px em x=282, sem transbordo |

## D001_2_VISUAL_PARITY_DEFECT

1. **Indicadores 3 + 1 numa faixa de 4px.** `.oc-kpis` é
   `repeat(auto-fit, minmax(150px, 1fr))` com 14px de intervalo: quatro
   indicadores precisam de 4 × 150 + 3 × 14 = **642px** de largura interior.
   O 2 + 2 só entra com a área de trabalho `max-width: 639px`. Com a área entre
   **640 e 643px** (janela de ~770–773px com a barra de aplicações visível) a
   grelha faz **3 + 1** e o quarto indicador fica cortado dentro do widget de
   107px. Medido: área 639 → 2 + 2; 640, 641, 642, 643 → 3 + 1; 644 → 4.
   A D001.2 diz «nunca 3 + 1». Uma correcção possível é o limiar do container
   subir para 643px, ou o mínimo dos indicadores descer; a escolha é vossa.

## Fora do âmbito, registado para não se confundir

- **O quadrado do canto inferior esquerdo com a barra escondida.** Na
  referência ocupa x 22–73, y 486–537, a 3px do fundo da janela; é o botão de
  mostrar da ocultação automática (FG-007, só referência). O que está
  implementado é o botão de mostrar da ocultação manual, em y 466–518. Não o
  tratei como defeito da D001.2; quando a FG-007 chegar, a posição é vossa.
- A bolha da Nye e os dados de exemplo da captura (12 ideias, lista de
  tarefas) são D002 e conteúdo, não composição.

## CORE_STATUS_CONTRACT_FOLLOWUP

Sem mudança: `/ready` `degraded` continua **OPERACIONAL** à porta (FG-024) até o
Core separar capacidades obrigatórias de opcionais.

## MISSING_DESIGN_STATE

1. Estados `503` (dependência em falta), `422`, `409` e «aplicação inactiva»
   em acções continuam com o código estável em texto.
2. Recuperar palavra-passe disponível (FG-002).

## D002_REQUIRED

Aplicações, gestor de janelas G-05 e pré-visualização, Nye, ecrã bloqueado,
painéis da barra de cima, menu de contexto do Desktop, ocultação automática da
barra de aplicações (FG-007), lançador com categorias/favoritos/recentes,
assistente de instalação, Monitor, tema escuro do Desktop e tema «sistema».
