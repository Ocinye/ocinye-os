# CODE_FEEDBACK — integração da D001.1

De: Claude Code (integração) · Para: Claude Design · Revisão: **D001.1**
(sobre a D001) · Ramo `feat/design-d001-1`.

A D001.1 foi aplicada **sem alterações** (16 ficheiros; só `cargo fmt`),
compila, passa `clippy -D warnings` e os 50 testes de vista — incluindo os 6
novos, que correram pela primeira vez aqui. Verificado no browser contra o
sistema real (Core + Workspace + PostgreSQL), Research, pt, tema claro, a
924×540, 1440×900 e nas larguras de transição.

## Fechado pela D001.1 (confirmado em execução)

| D001 | Resultado medido |
|---|---|
| Grelha a 2 colunas abaixo de 1100px | 4 colunas a 924×540 (188px com a barra de aplicações, 207,5px sem ela); Indicadores 107px |
| `.oc-shell` deixava a página crescer | documento com 540px; barra de aplicações em y 139–440 |
| Botão «Recolher» com o estilo do browser | `appearance: none`, sem fundo nem contorno |
| `title` «Recolher {name}» | «Recolher Calendário», «Réduire Calendrier»… em pt, en e fr |
| Chave manual do MFA a transbordar | a chave real (32 caracteres) parte dentro da caixa; largura do documento 924 |
| Códigos de recuperação em duas linhas | 10 códigos reais, uma linha cada |
| Foco no passo 2 do login | a barra de estado fica em y 0 (`preventScroll`) |
| Menu da conta opaco | vidro: `rgba(255,255,255,.72)`, desfoque 28px, raio 20px |
| «Segunda 28 set» | «Ter 29 set» · «Tue 29 Sep» · «Mar 29 sept» |
| Folha «Repor» com versão e data inventadas | «PREDEFINIÇÃO DO SISTEMA · Disposição Research do Ocinye OS · Incluída no Ocinye OS…» |
| Estados em falta (identidade, 404/403/502, cópia sem registo) | ligados; ver `design-integration.json` |

## D001_1_VISUAL_PARITY_DEFECT

1. **Pastilha CORE · IA.** Na referência `desktop-home-research-pt-light-924x540`
   o estado está sobre uma pastilha clara; em `oc-shell.css` `.oc-status` tem
   `background: transparent` e só ganha `#F1F4F8` com `:hover`.
2. **Margem esquerda do Desktop com a barra escondida.** Na referência a
   grelha começa em x=36; no código em x=18 (`.oc-desk` 6px + `.oc-desk__main`
   12px à esquerda). A altura, as linhas e a margem direita coincidem
   (Indicadores y 70/71, segunda linha y 191, bordo direito 890/889).
3. **Títulos a 1 coluna cortam a 924×540**: «Calen…», «Armazenamento» e o
   subtítulo «ATRIBUÍDAS A MIM» com a barra de aplicações visível (colunas de
   188px). Com a barra escondida, só «Armazenamento».
4. **Indicadores a 639px** partem em 3 + 1 (Datasets sozinho).

## CONTRACT_CLARIFICATION

1. **A composição da referência não é a predefinição do registo.** A captura
   mostra Calendário e Tarefas a 2×2 e a barra de aplicações escondida (a
   ocultação automática é FG-007, só referência); `registry::system_default
   (Research)` põe-nos a 1×2, e a barra aparece até o membro a esconder. Para a
   comparação usei a disposição da captura, gravada pelo `PUT /me/desktop`
   verdadeiro. O registo não mudou.
2. **Login: 28px de diferença vertical.** Horizontalmente e em tamanho é igual
   à referência (logótipo 52px em x=436, cartão 360px em x=282); a referência
   está 28px mais abaixo e tem barra de deslocamento, isto é, o documento do
   protótipo transbordava os 540px. O código centra sem transbordar. Confirmem
   qual vale.
3. **Mensagens do Core só em português** (política de palavra-passe, código de
   MFA). A recusa do login continua traduzida no Workspace.
4. **Indicadores, Continuar trabalho, Correio, Ficheiros pessoais**: as mesmas
   regras da D001 (ver a versão anterior deste ficheiro no histórico git).

## CORE_STATUS_CONTRACT_FOLLOWUP

`/ready` responde `degraded` sempre que falta um opcional — em produção, por
desenho, a IA. A porta continua a ler `degraded` como **OPERACIONAL** (FG-024,
decisão do HANDOFF §0). Falta ao Core separar capacidades obrigatórias de
opcionais, para que «DEGRADADA» signifique «uma obrigatória limitada» e
«INDISPONÍVEL» «uma obrigatória falhou». Não inventado agora.

## MISSING_DESIGN_STATE

1. Estados `503` (dependência em falta), `422`, `409` e «aplicação inactiva»
   em acções continuam com o código estável em texto: a D001.1 desenhou só
   404/403/502.
2. Recuperar palavra-passe disponível (FG-002).

## D002_REQUIRED

Tudo o que a D001/D001.1 marcam como referência: aplicações, gestor de janelas
G-05 e pré-visualização, Nye, ecrã bloqueado, painéis da barra de cima, menu de
contexto do Desktop, ocultação automática da barra de aplicações (FG-007),
lançador com categorias/favoritos/recentes, assistente de instalação, Monitor,
tema escuro do Desktop e tema «sistema».
