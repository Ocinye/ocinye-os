# Handoff — Ocinye Workspace (interface humana do Ocinye OS)

## 1. Visão geral

O **Ocinye Workspace** é a interface através da qual os membros da Ocinye trabalham diariamente
sobre o **Ocinye OS** (sistema operacional institucional) e o **Ocinye Core** (núcleo tecnológico).
Não é um website nem um dashboard SaaS: é um ambiente operacional de investigação e engenharia,
desktop-first, denso em informação, com sidebar persistente, topbar, pesquisa global e command palette.

Este pacote contém o protótipo navegável de alta fidelidade com 20 ecrãs, o sistema visual completo
(cores, tipografia, espaçamento, componentes, estados, badges), o conjunto de ícones e um prompt de
implementação pronto a usar.

## 2. Sobre os ficheiros de design

Os ficheiros em `prototype/` são **referências de design criadas em HTML** — protótipos que mostram o
aspecto e o comportamento pretendidos, **não código de produção para copiar directamente**.

A tarefa é **recriar estes ecrãs no ambiente do codebase de destino** (React, Vue, Svelte, Blazor,
SwiftUI, etc.), usando os padrões, a biblioteca de componentes e a arquitectura já existentes nesse
projecto. Se ainda não existir ambiente, escolher a stack mais apropriada (recomendação: React +
TypeScript + Vite, ou Next.js se houver SSR/rotas de servidor) e implementar aí.

## 3. Fidelidade

**Alta fidelidade (hi-fi).** Cores, tipografia, espaçamento, densidade, estados e microinterações são
finais. A UI deve ser recriada com fidelidade ao pixel, usando as bibliotecas do codebase de destino.
Todos os valores exactos estão em `DESIGN_TOKENS.md` e nas secções abaixo.

### Fidelidade ao pixel e escala de apresentação

As duas coisas não são a mesma, e o codebase separa-as.

**As medidas deste dossier são as medidas do desenho.** É contra elas que
`apps/workspace/tests/design_fidelity.rs` compara o CSS, assinatura a
assinatura, contra o protótipo em `prototype/`. Um componente que passe a ter
outra altura é uma alteração ao desenho, e implica mudar o protótipo.

**A escala a que esse desenho é apresentado é uma decisão separada.**
`--oc-interface-scale` está em `1.15` e é aplicada uma vez, na raiz. Nenhuma
proporção muda com ela — é o desenho inteiro, à mesma, apresentado maior porque
a instituição o quis assim.

A distinção existe para que aumentar a interface nunca obrigue a reescrever a
expectativa dos testes. Reescrevê-la seria afirmar que o protótipo mudou quando
não mudou, e a partir daí a fidelidade deixaria de ser verificável.

## 4. Arquitectura de ecrãs (mapa de navegação)

```
/login                        Ecrã de início de sessão (estilo workstation, sem MFA)
└── Shell autenticada (sidebar + topbar + main + command palette)
    ├── /                     Home / Dashboard
    ├── /my-work              O Meu Trabalho
    ├── /units                Unidades (tabela)
    │   └── /units/:id        Detalhe da Unidade (9 tabs)
    ├── /ideas                Ideias (tabela)
    │   └── /ideas/:id        Research Workspace da Ideia (13 tabs)
    ├── /projects             Projectos (tabela)
    │   └── /projects/:id     Research Workspace do Projecto (13 tabs)
    ├── /knowledge            Conhecimento (Knowledge Hub)
    ├── /bibliography         Bibliografia (tabela)
    ├── /datasets             Dados / Datasets (tabela)
    ├── /ai                   Ocinye AI (hub, estado vazio)
    │   ├── /ai/agents        Agentes (tabela)
    │   ├── /ai/agents/new    Criar Agente IA (formulário)
    │   └── /ai/prompt        Prompt Ocinye (interacção directa)
    ├── /compute              Computação (0 nós registados)
    ├── /activity             Actividade
    ├── /admin                Administração › Membros (tabela, 5 tabs)
    └── /audit                Audit Log (tabela)
```

Command palette (`⌘K` / `Ctrl+K`) é global na shell autenticada e substitui, nesta fase, a página
dedicada de pesquisa global.

## 5. Layout global

### 5.1 Shell autenticada
- Root: `position:fixed; inset:0; display:flex; background:#F6F8FA`.
- **Sidebar**: largura `224px` expandida / `58px` colapsada, transição `width .18s ease`,
  fundo `#0B2D4A`, coluna flex com header (52px), lista de navegação (scroll) e rodapé.
- **Coluna principal**: `flex:1; min-width:0; display:flex; flex-direction:column`.
  - **Topbar**: altura `52px`, fundo `#FFFFFF`, `border-bottom:1px solid #E4E9F0`, `padding:0 16px`, `gap:16px`.
  - **Content**: `flex:1; min-height:0; overflow-y:auto`.
- Padding padrão das páginas: `22px 24px 40px`. Páginas com cabeçalho contextual usam
  `18px 24px 0` no bloco branco do cabeçalho e `20px 24px 40px` no corpo.
- Larguras máximas: Home `1560px`, Criar Agente `1060px`, Actividade `900px`, prompt input `880px`.

### 5.2 Sidebar — estrutura
Header: tile branco `26×26`, `border-radius:7px` com o logótipo (`20×20`, `object-fit:contain`);
título `OCINYE OS` (600/11.5px, `letter-spacing:.14em`, branco) e subtítulo `WORKSPACE`
(mono 9.5px, `rgba(255,255,255,.42)`, `letter-spacing:.1em`); botão de colapsar `22×22`.

Grupos (label mono 9.5px `rgba(255,255,255,.32)`, `letter-spacing:.16em`, `padding:12px 8px 5px`):
- **PESSOAL** — Home, O Meu Trabalho
- **INVESTIGAÇÃO** — Unidades, Ideias, Projectos
- **CONHECIMENTO** — Conhecimento, Bibliografia, Dados
- **INTELIGÊNCIA** — Ocinye AI, Agentes, Computação
- **INSTITUCIONAL** — Actividade, Administração, Audit Log

Item de navegação: altura `32px`, `padding:0 9px`, `border-radius:8px`, `gap:10px`,
ícone `15×15` (`stroke-width:1.4`), texto 500/12.5px.
- repouso `color:rgba(255,255,255,.68)`, fundo transparente
- hover `background:rgba(255,255,255,.07)`
- activo `background:rgba(255,255,255,.10)`, `color:#FFFFFF`
- quando colapsada, os labels desaparecem (`display:none`) e o `title` do elemento serve de tooltip

Rodapé: `border-top:1px solid rgba(255,255,255,.08)`; Definições e Ajuda (altura 30px, 400/12px,
`rgba(255,255,255,.62)`); cartão de perfil com avatar `24px` (iniciais `JM`), nome 500/11.5px e
estado do sistema (ponto `#4FA97B` 5px + `SISTEMA OK` mono 9.5px). Clicar termina a sessão.

### 5.3 Topbar — estrutura
1. Breadcrumb: `OCINYE / <ecrã actual>` (mono 11.5px; `OCINYE` `#7C8B9A`, separador `#C3CDD8`,
   ecrã actual `#0B2D4A` peso 500, `white-space:nowrap`), `min-width:150px`.
2. Pesquisa global: `flex:0 1 420px; min-width:180px`, altura `33px`, fundo `#F3F6F9`,
   `border:1px solid #E4E9F0`, `radius:8px`, ícone search 14px, placeholder
   `Pesquisar no Ocinye…` (400/12.5px `#8A98A6`), atalho `⌘K` em pill mono 10px.
   Hover: `background:#FFFFFF; border-color:#C3CDD8`. Clique abre a command palette.
3. Espaçador `flex:1`.
4. Botão **+ Criar** (`#E0A731`, texto `#0B2D4A` 600/12px, altura 31px, radius 8px) que abre um
   menu de 212px (`radius:11px`, `box-shadow:0 16px 40px rgba(11,45,74,.14)`, `animation:ocFade .12s`)
   com: Nova Ideia `I`, Novo Projecto `P`, Nova Nota `N`, Nova Referência `R`, Novo Dataset `D`,
   Nova Tarefa `T`, Novo Agente IA `A`.
5. Divisor `1×22px #E4E9F0`.
6. Notificações: ícone 16px `#5F7183` + ponto `#E0A731` 6px com anel branco de 1.5px.
7. Estado do Core: pill `#F0F7F3` / `border #D8EBE0`, ponto `#3E8F66` com `animation:ocPulse 2.6s infinite`,
   texto `CORE OK` mono 500/10px `#2E6B4C`.
8. Avatar `27px` circular `#0B2D4A`, iniciais brancas 600/10.5px.

## 6. Ecrãs

### 6.1 Login (`/login`)
**Objectivo:** entrar no ambiente Ocinye, com a sensação de arranque de uma workstation.

Fundo `#071E33` em `position:fixed; inset:0`, coluna flex de três zonas (barra de estado, centro
com scroll, rodapé). Camadas decorativas, todas absolutas:
- `radial-gradient(120% 90% at 78% 12%, rgba(21,73,116,.85), transparent 60%)` +
  `radial-gradient(90% 70% at 12% 96%, rgba(224,167,49,.13), transparent 62%)`
- três círculos com apenas borda: `rgba(255,255,255,.055)` (78vh, top-left), `rgba(224,167,49,.14)`
  (64vh, top-left), `rgba(255,255,255,.05)` (96vh, bottom-right)
- grelha técnica: linhas de 1px `rgba(255,255,255,.028)`, `background-size:56px 56px`, `opacity:.5`

Barra superior: `OCINYE CORE · OPERACIONAL` (mono 500/11px `rgba(255,255,255,.5)`,
`letter-spacing:.1em`) com ponto `#4FA97B` e halo `0 0 0 3px rgba(79,169,123,.15)`; à direita
data/hora mono 11px `rgba(255,255,255,.4)`.

Centro (`flex:1; min-height:0; overflow-y:auto; justify-content:center; gap:34px; padding:24px`):
- Tile branco `78×78`, `radius:20px`, `box-shadow:0 18px 50px rgba(0,0,0,.35), 0 0 0 1px rgba(255,255,255,.14)`,
  logótipo `60×60`.
- `OCINYE OS` 600/25px branco `letter-spacing:.14em`; `OCINYE WORKSPACE` mono 12px `#E0A731`
  `letter-spacing:.22em`.
- Cartão de sessão: largura `352px`, `background:rgba(255,255,255,.055)`,
  `border:1px solid rgba(255,255,255,.11)`, `radius:16px`, `padding:26px 24px 22px`,
  `backdrop-filter:blur(14px)`.
  - Avatar `52px` circular, `linear-gradient(180deg,#1C4B74,#0E3454)`, borda `rgba(255,255,255,.18)`,
    iniciais 600/18px; nome 500/14px branco; email mono 11px `rgba(255,255,255,.45)`.
  - Campos: altura `40px`, `background:rgba(7,30,51,.5)`, `border:1px solid rgba(255,255,255,.13)`,
    `radius:9px`, ícone 13px + input 13px branco. Focus: `border-color:#E0A731`.
    Password com `letter-spacing:.14em`.
  - Botão **Iniciar sessão**: largura total, altura `42px`, `#E0A731`, texto `#0B2D4A` 600/13px,
    radius 9px, ícone seta; hover `filter:brightness(1.07)`, active `translateY(1px)`.
  - Linha inferior: `Utilizar outro utilizador` (11px `rgba(255,255,255,.5)`, hover `#E0A731`) e
    `PT · pt-PT` (mono 11px).

Rodapé: `border-top:1px solid rgba(255,255,255,.07)`, três acções centradas com `gap:26px`
(Desligar, Reiniciar, Estado do Sistema), 11.5px `rgba(255,255,255,.55)`, hover branco.

**Não implementar:** MFA, códigos de 6 dígitos, signup público, login social, banners de marketing.

### 6.2 Home / Dashboard (`/`)
Responde a “o que precisa da minha atenção?”.

- Cabeçalho: `Bom dia, Eng. João Manuel` (600/21px, `letter-spacing:-.01em`) + subtítulo
  `Tem 6 tarefas atribuídas e 3 itens a aguardar a sua revisão.` (12.5px `#5F7183`).
  À direita: `Nova Ideia`, `Novo Projecto` (brancos, borda `#E4E9F0`, hover `border-color:#0B2D4A`)
  e `Prompt Ocinye` (`#0B2D4A`, texto branco, ponto dourado 6px).
- **4 KPI cards** (`grid-template-columns:repeat(4,1fr); gap:12px`): cartão branco, borda `#E4E9F0`,
  radius 11px, `padding:14px 15px`; label mono 11px `#7C8B9A` `letter-spacing:.1em`; delta mono 10.5px
  (verde `#3E8F66` ou `#A9B5C1`); valor 600/27px; hint 11.5px `#8A98A6`.
  Dados: UNIDADES 12 (+1, activas) · IDEIAS 86 (+7, 14 em revisão) · PROJECTOS 24 (+2, 18 em execução) ·
  DATASETS 16 (—, 4 restritos).
- Corpo: `grid-template-columns:1.35fr 1fr; gap:12px; align-items:start`.
  - **Continuar trabalho** — 3 cartões de Research Workspace em `grid` de 1px sobre `#EEF2F6`
    (min-height 118px): pill de tipo (IDEIA/PROJECTO/UNIDADE, mono 9.5px em `#F1F4F8`), id mono 9.5px,
    título 500/13px, tempo relativo e badge de estado.
  - **Tarefas pendentes** — linhas de 44px, checkbox 14px (borda `#C3CDD8`, radius 4px), título
    500/12.5px, contexto mono 11px `#8A98A6`, badge de tipo (REVISÃO/ACESSO/RELATÓRIO/FINANÇAS/REUNIÃO)
    e prazo alinhado à direita (hoje `#B3261E`, amanhã `#C87A22`, restantes `#5F7183`).
  - **Actividade recente** — ponto 6px (navy/gold/verde/vermelho por tipo), texto 12px/1.45 `#28394A`,
    meta mono 10.5px `#A9B5C1`.
  - **Cartão Ocinye AI** (`#0B2D4A`, radius 11px): círculo decorativo com borda `rgba(224,167,49,.22)`,
    label `OCINYE AI`, título 500/14.5px branco, explicação do estado vazio, botões
    `Abrir Prompt` (gold) e `Hub de IA` (`rgba(255,255,255,.09)` + borda `rgba(255,255,255,.18)`).
  - **Acesso rápido** — grelha 2×2 de botões 36px com ponto dourado: Nova Ideia, Novo Projecto,
    Novo Dataset, Prompt IA.

### 6.3 O Meu Trabalho (`/my-work`)
Título + subtítulo, barra de tabs (`Tarefas`, `Actividade`, `Ideias`, `Projectos`, `Documentos`,
`Datasets`, `Favoritos`, `Notas`) com `border-bottom:1px solid #E4E9F0`, e grelha `1.5fr 1fr`:
lista de tarefas atribuídas (linhas 46px) à esquerda; `Documentos recentes` (pill de extensão
PDF/PPTX/CSV/DOCX) e `Unidades seguidas` à direita.

### 6.4 Ecrãs de lista / tabela
Sete ecrãs partilham exactamente o mesmo componente de tabela: Unidades, Ideias, Projectos,
Bibliografia, Dados, Agentes, Membros, Audit Log.

Estrutura: cabeçalho da página (título 600/19px + subtítulo 12.5px + botão de acção primária navy) →
cartão branco (`border:1px solid #E4E9F0; radius:11px; overflow:hidden`) contendo:
1. **Barra de controlo** (44px, `border-bottom:1px solid #EEF2F6`): tabs (pill 27px, activa
   `#0B2D4A`/branco, inactiva `#5F7183`, hover `#F1F4F8`), espaçador, campo de pesquisa (250px,
   altura 29px, `#F3F6F9`) e botão `Filtrar` (borda `#E4E9F0`, hover navy).
2. **Header de colunas**: `background:#FAFCFD`, altura 34px, labels mono 500/10px `#7C8B9A`
   `letter-spacing:.1em`; colunas numéricas alinhadas à direita.
3. **Linhas**: `display:grid` com `grid-template-columns` por tabela, `gap:14px`, `padding:0 16px`,
   `min-height:38px` (modo denso 30px), `border-bottom:1px solid #F3F6F9`,
   hover `background:#F8FAFC`, cursor pointer. Primeira célula 500/12.5px `#0F1A24`; células de
   texto 400/12px `#5F7183`; códigos, datas, DOIs, versões e ids em mono 400/11.5px;
   badges conforme §7.3; progresso = barra 5px (`#EEF2F6` / preenchimento `#0B2D4A`, radius 3px) +
   percentagem mono 500/11px.
4. **Rodapé** (42px): contagem mono 11.5px `#8A98A6` + paginação `‹ ›` (27×27, borda `#E4E9F0`).

Colunas por ecrã:
- **Unidades**: Unidade · Código · Responsável · Membros → · Ideias → · Projectos → · Estado
- **Ideias**: Título · Unidade · Responsável · Estado · Prioridade · Classificação · Actualizada →
- **Projectos**: Código · Projecto · Unidade · Responsável · Estado · Progresso · Início · Fim
- **Bibliografia**: Título · Autores · Ano · Origem · Tipo · DOI · Citações →
- **Dados**: Nome · Responsável · Registo · Versão · Tamanho → · Tipo · Classificação · Acesso
- **Agentes**: Agente · Propósito · Estado · Âmbito · Capacidade · Utilização →
- **Membros**: Nome · E-mail · Unidade · Função · Registo · Estado · Actividade →
- **Audit Log**: Data · Utilizador · Acção · Recurso · Contexto · Resultado · Correlation ID

Linhas de Unidades/Ideias/Projectos navegam para o respectivo detalhe; a acção primária de Agentes
abre `Criar Agente IA`. Audit Log usa acções em notação técnica (`auth.session.open`,
`dataset.access.read`, `idea.state.change`, `permission.grant`, `agent.create`) e resultados
`OK` / `NEGADO` / `AVISO` — nunca tratar como feed de actividade.

### 6.5 Detalhe da Unidade (`/units/:id`)
Cabeçalho branco: nome 600/19px + código em pill mono (`#F1F4F8`) + badge `ACTIVA`; linha de meta
(`Responsável: … · 24 membros · criada em …`); 9 tabs (`Visão geral`, `Membros`, `Ideias`,
`Projectos`, `Bibliografia`, `Dados`, `Documentos`, `Actividade`, `Configuração`) com
`border-radius:8px 8px 0 0` e activa em navy.
Corpo `1.5fr 1fr`: cartão `Sobre a unidade` (texto 12.5px/1.7 `#42546A` + grelha 2×2 de métricas
separada por linhas de 1px `#EEF2F6`) e cartão `Actividade recente`.

### 6.6 Research Workspace da Ideia (`/ideas/:id`)
Um dos ecrãs mais importantes: deve transmitir “estou dentro desta investigação”.

Cabeçalho contextual: pill `IDEIA` + título 600/19px + badge de estado (`EXPLORATION`) + badge de
classificação (`INTERNAL`); meta mono `IDE-0142 · UENR-001 · Maria Santos, Carlos Lima · criada em …`;
acções `Partilhar`, `Promover a Projecto` e `IA neste workspace` (navy com ponto dourado).
13 tabs: Visão geral, Bibliografia, Fontes, Notas, Documentos, Datasets, Código, Experiências,
Resultados, Tarefas, IA, Actividade, Histórico (scroll horizontal quando necessário).

Corpo `1.4fr 1fr 1fr`:
1. `Descrição` (12.5px/1.7) + `PALAVRAS-CHAVE` em chips (`#F3F6F9`, borda `#E4E9F0`, radius 6px,
   `padding:4px 9px`) + grelha de 3 métricas (Referências 42, Datasets 5, Experiências 11).
2. `Actividade recente`.
3. `Tarefas`: por linha, título + badge de estado, barra de progresso 5px e percentagem.

Estados de uma Idea: `Discovery`, `Exploration`, `Concept`, `Review`, `Project Candidate`, `Archived`.
Uma Idea **não** é ainda um Project.

### 6.7 Research Workspace do Projecto (`/projects/:id`)
Mesma linguagem contextual, com código do projecto como título, badges `ACTIVE`/`INTERNAL` e meta
`nome · unidade · responsável · janela temporal`. 13 tabs: Visão geral, Membros, Planeamento,
Bibliografia, Documentos, Dados, Código, Experiências, Resultados, Tarefas, Financiamento, IA,
Histórico.
Corpo `1.4fr 1fr 1fr`: `Descrição` + donut de progresso
(`conic-gradient(#E0A731 0 65%, #EEF2F6 65% 100%)`, 88px com miolo branco de 66px e `65%` a 600/17px)
com estado textual e ponto verde; `Planeamento` (marcos M1–M4 com badge e barra 4px);
`Equipa` (avatar 24px placeholder + nome + badge de função).
Estados de um Project: `Draft`, `Active`, `On Hold`, `Completed`, `Archived`.

### 6.8 Conhecimento (`/knowledge`)
Knowledge Hub = memória institucional. Tabs `Tudo`, `Bibliografia`, `Fontes`, `Notas`, `Documentos`,
`Resultados`, `Publicações`; 4 cartões de contagem (Bibliografia 1.248, Documentos 3.417,
Datasets 116, Resultados 284 — valor 600/25px, hover `border-color:#0B2D4A`); lista
`Adicionado recentemente` com pill de extensão.

### 6.9 Ocinye AI (`/ai`)
Hub de IA com cabeçalho branco, acções `Criar Agente` e `Abrir Prompt`, e tabs `Visão geral`,
`Arquitectura`, `Capacidades`, `Modelos`.
Estado vazio institucional (cartão branco, `padding:56px 24px`): tile 78px `radius:20px` com hexágono
navy e núcleo dourado; título **“Nenhum nó de IA Ocinye está actualmente disponível”** (600/15.5px);
explicação 12.5px/1.65 `#5F7183`; botões `Configurar IA` (gold) e `Ver computação`.
Abaixo, 4 cartões: AGENTES IA 0, MODELOS 0, CONVERSAS 0, RECURSOS 0, cada um com link de navegação.

### 6.10 Criar Agente IA (`/ai/agents/new`)
Grelha `1.25fr 1fr`, largura máxima 1060px.
Coluna esquerda — secção `IDENTIDADE`: Nome do agente (input 36px, placeholder
`Ex.: Assistente de Pesquisa`), Descrição (textarea 64px), Instruções gerais (textarea 92px),
e par de selects `Capacidade principal` (General / Coding / Reasoning / Data) e `Modelo base`
(`Nenhum modelo disponível` enquanto não houver nós). Labels de campo 500/11.5px `#42546A`;
inputs com borda `#E4E9F0`, radius 8px, focus `border-color:#0B2D4A`.
Coluna direita:
- `ÂMBITO DE ACESSO`: três botões segmentados (Pessoal activo / Unidade / Institucional) —
  prever também âmbito Research Workspace.
- `CONHECIMENTO`: checkboxes de bibliografia da unidade, documentos institucionais, datasets INTERNAL.
- Painel de **Segurança** (`#FAFCFD`): ícone escudo + explicação de que o agente lê apenas até
  `INTERNAL`, que CONFIDENTIAL/RESTRICTED ficam inacessíveis e que cada acesso é registado no Audit Log.
- `Estado`: toggle 38×21 navy (activo por omissão).
- Acções: `Cancelar` (branco) e `Criar Agente` (gold 600/12px).

### 6.11 Prompt Ocinye (`/ai/prompt`)
Coluna de altura total sobre fundo branco — experiência própria, não um clone de chat genérico.
- **Barra de contexto** (52px): selector de agente (`Assistente de Pesquisa` com ponto dourado),
  pill `CONTEXTO IDE-0142 · UENR-001`, e selector de capacidades à direita:
  `Geral` (activo navy), `Raciocínio`, `Código`, `Dados` (indisponível — texto `#A9B5C1`,
  borda `#EEF2F6`, sufixo mono `indisponível`).
- **Área da conversa**: estado vazio centrado com tile 56px (hexágono), título
  `Interagir com Ocinye` (600/18px), explicação do âmbito e do estado da infraestrutura, e 4
  sugestões em chips de 32px (`Resumir investigação sobre hidrogénio verde`, `Comparar bibliografia
  de armazenamento`, `Analisar dataset climático de 2010–2024`, `Criar estrutura de relatório`).
  As respostas devem prever texto, fontes, referências, datasets, documentos, código, resultados e tabelas.
- **Prompt input** (peça central, máx. 880px): cartão `radius:14px`, borda `#E4E9F0`,
  `box-shadow:0 4px 18px rgba(11,45,74,.06)`, `padding:14px 15px 11px`; textarea 62px com placeholder
  `Escreva o seu pedido…` (13.5px/1.6); barra de acções `Anexar`, `Dataset`, `Documento`,
  `Ferramentas` (chips 29px), dica mono `⏎ enviar · ⇧⏎ nova linha` e botão circular de envio
  `34px` em `#E0A731`.
- Nota final centrada 10.5px `#A9B5C1`: “O Ocinye AI pode cometer erros. Verifique informação crítica
  e consulte as fontes citadas.”
Dentro de um Research Workspace, a tab `IA` abre esta mesma interface já vinculada ao contexto —
visualmente distinta de uma conversa institucional geral (pill de contexto preenchido).

### 6.12 Computação (`/compute`)
Cabeçalho com acção `Adicionar Nó` (gold) e tabs `Nós`, `Trabalhos`, `Recursos`, `Ambientes`.
O cartão mostra **o header de colunas do estado futuro** (NÓ, ESTADO, LOCALIZAÇÃO, CPU, RAM, GPU,
ARMAZENAMENTO, SAÚDE) seguido do estado vazio: tile 58px, `0 nós registados` (600/15px) e
“Nenhum nó de computação Ocinye está actualmente disponível…”. Abaixo, 4 métricas a zero
(Trabalhos activos, GPU disponível, CPU disponível, Armazenamento 0 B).
**Nunca inventar nós online.**

### 6.13 Actividade (`/activity`)
Feed institucional, largura máxima 900px, linhas de 12px com ponto 7px, texto 12.5px/1.5 e meta mono.

### 6.14 Command palette (`⌘K`)
Overlay `rgba(7,30,51,.42)` + `backdrop-filter:blur(2px)`, painel 600px a 14vh do topo,
`radius:14px`, `box-shadow:0 30px 80px rgba(7,30,51,.32)`, `animation:ocFade .14s ease`.
Campo de 50px com placeholder `Pesquisar ou executar um comando…` e pill `ESC`.
Grupos `NAVEGAR` (ponto `#C3CDD8`) e `ACÇÕES` (ponto `#E0A731`), linhas de 34px com atalho mono à
direita. Fecha com `Esc` ou clique no overlay. Preparado para pesquisar Units, Ideas, Projects,
People, Documents, Bibliography, Datasets e Results.

## 7. Componentes e estados

### 7.1 Botões
| Tipo | Fundo | Texto | Altura | Radius | Hover |
| --- | --- | --- | --- | --- | --- |
| Primário (acção institucional) | `#0B2D4A` | `#FFFFFF` 500/12px | 31px | 8px | `#123C60` |
| Primário de destaque / CTA | `#E0A731` | `#0B2D4A` 600/12px | 31–42px | 8–9px | `brightness(1.06)` |
| Secundário | `#FFFFFF` + borda `#E4E9F0` | `#0B2D4A` 500/12px | 31px | 8px | `border-color:#0B2D4A` |
| Terciário / em navy | `rgba(255,255,255,.09)` + borda `rgba(255,255,255,.18)` | `#FFFFFF` | 30px | 7px | — |
| Ícone | transparente | `#5F7183` | 29px | 8px | `background:#F3F6F9` |

### 7.2 Campos
Input/select/textarea: fundo branco, borda `#E4E9F0`, radius 8px, texto 12.5px, placeholder `#98A6B4`,
altura 36px (input) / 64–92px (textarea), focus `border-color:#0B2D4A`.
Foco visível global: `outline:2px solid #E0A731; outline-offset:1px`.

### 7.3 Badges (sempre ponto + texto, nunca só cor)
Formato: mono 500/10px, `letter-spacing:.04em`, `padding:2.5px 6px`, `radius:4px`, borda de 1px,
ponto de 5px. Paleta de tons:

| Tom | Texto | Fundo | Borda | Ponto | Uso |
| --- | --- | --- | --- | --- | --- |
| ok | `#2E6B4C` | `#F0F7F3` | `#D8EBE0` | `#3E8F66` | Activa, Active, Concluída, PUBLIC, OK, Aberto, Project Candidate |
| gold | `#8A6110` | `#FDF6E7` | `#F2E3BE` | `#E0A731` | Review, Revisão, Workspace |
| navy | `#0B2D4A` | `#F1F5F9` | `#DCE4EC` | `#0B2D4A` | Concept, Completed, INTERNAL, Institucional |
| blue | `#20537F` | `#EFF5FB` | `#D5E4F0` | `#2B6CB0` | Exploration, Em curso, Unidade |
| gray | `#5F7183` | `#F5F7F9` | `#E4E9F0` | `#A9B5C1` | Discovery, Draft, Archived, Suspensa, Desactivado, Pessoal, Baixa |
| warn | `#8A4B10` | `#FDF2E7` | `#F2DBBE` | `#C87A22` | On Hold, Pausado, CONFIDENTIAL, Restrito, AVISO |
| err | `#8C2019` | `#FCF0EF` | `#F1D6D3` | `#B3261E` | RESTRICTED, Negado, NEGADO, Alta prioridade |

Classificação de dados obrigatória e visível em Dataset, Document, Idea, Project e contexto de IA:
`PUBLIC` · `INTERNAL` · `CONFIDENTIAL` · `RESTRICTED`.

### 7.4 Cartões
Branco, borda `1px #E4E9F0`, radius 11px. Cabeçalho de secção: `padding:13px 15px`,
`border-bottom:1px solid #EEF2F6`, título 600/13px, acção/legenda à direita (500/11.5px `#0B2D4A`
ou mono 11px `#8A98A6`). Hover de cartão clicável: `border-color:#C3CDD8` +
`box-shadow:0 2px 10px rgba(11,45,74,.06)`.

### 7.5 Empty states
Tile quadrado (44–78px) com borda `#E4E9F0`, radius 12–20px, fundo `#FAFCFD` e ícone técnico simples;
título 600/15–15.5px; explicação 12.5px/1.65 `#5F7183` com largura máxima ~430px; 1–2 acções.
Sem ilustrações decorativas.

### 7.6 Tabs
Pill (listas): altura 27px, `padding:0 11px`, radius 7px, activa `#0B2D4A`/branco.
Contextual (cabeçalhos de detalhe): altura 32px, `padding:0 12px`,
`border-radius:8px 8px 0 0`, activa navy, hover `#F1F4F8`, scroll horizontal quando excedem.

## 8. Interacções e comportamento

- **Navegação**: sidebar, breadcrumb, command palette, linhas de tabela (Unidades/Ideias/Projectos),
  cartões de `Continuar trabalho`, acessos rápidos, cartão de IA e menu `+ Criar`.
- **Atalhos**: `⌘K`/`Ctrl+K` abre a palette; `Esc` fecha palette e menu Criar. Previstos:
  `⌘⇧I` nova ideia, `⌘⇧A` novo agente, `⌘⇧P` prompt, `⌘⇧C` computação.
- **Sidebar colapsável**: alterna 224px/58px com transição de 180ms; labels escondidos, tooltips via `title`.
- **Hover discreto** em todas as linhas e cartões (`#F8FAFC` em tabela, `#F1F4F8` em navy).
- **Animações**: `ocFade` (opacity + translateY 4px, 120–140ms) em overlays e menus;
  `ocPulse` (2.6s infinito) no ponto de estado do Core. Sem animações decorativas longas.
- **Estados de sistema** a suportar: Online, Offline, Unavailable, Active, Draft, Archived, Warning,
  Error, Pending, Restricted.
- **Acessibilidade**: contraste AA, focus ring dourado, navegação por teclado em nav/tabs/tabelas,
  labels em todos os campos, estado nunca comunicado só por cor, texto mínimo 10px apenas para
  metadados mono (corpo ≥ 12px).
- **Responsividade**: desktop-first (1440px de referência, funcional a partir de ~1180px com scroll
  horizontal nas tabelas); laptop 1280–1440 e tablet ≥ 1024 (sidebar colapsada por omissão).
  Não sacrificar densidade para parecer mobile-first.
- **Dark mode**: fora de âmbito nesta fase — apenas o login é escuro. A estrutura de tokens deve
  permitir uma variante escura futura.

## 9. Estado da aplicação

| Estado | Tipo | Notas |
| --- | --- | --- |
| `route` | string | ecrã actual (ver mapa do §4); `login` quando não autenticado |
| `sidebarCollapsed` | boolean | persiste por utilizador |
| `paletteOpen` | boolean | `⌘K` / `Esc` |
| `createMenuOpen` | boolean | fecha ao navegar ou com `Esc` |
| `activeTab` | string por ecrã de detalhe | primeira tab por omissão |
| `tableFilters` | { tab, query, sort, page } por tabela | server-side em produção |
| `denseRows` | boolean | preferência de densidade (38px ↔ 30px) |

Dados a obter do backend: unidades, ideias, projectos, tarefas, actividade, documentos, bibliografia,
datasets, membros, audit log, agentes, modelos, nós de computação e contadores de KPI. Todos os
conteúdos do protótipo são exemplos de visualização — não fixar em código (em particular, os nomes de
agentes são apenas exemplos, não agentes oficiais).

## 10. Design tokens

Ver `DESIGN_TOKENS.md` (paleta, tipografia, espaçamento, radius, sombras, animações) — versão
canónica dos valores.

## 11. Assets

- `assets/ocinye_logo.png` — logótipo oficial fornecido pelo cliente (1254×1254, PNG **sem** canal
  alfa; usar sempre sobre tile branco ou pedir uma versão SVG/transparente para produção).
- `icons/icons.svg` — sprite com os 37 ícones do protótipo (`<symbol>` com `viewBox` original,
  `currentColor`, `stroke-width` 1.4). Índice e mapa de utilização em `icons/ICONS.md`.
  Não substituir por biblioteca de terceiros sem verificar o peso de traço: a coerência do conjunto
  depende de traço fino e geometria simples.
- Tipografia: **IBM Plex Sans** (400/500/600/700) e **IBM Plex Mono** (400/500/600), Google Fonts.
  Mono é usado para metadados, códigos, ids, datas, DOIs, badges e labels de coluna.

## 12. Ficheiros deste pacote

```
design_handoff_ocinye_workspace/
├── README.md                     este documento
├── IMPLEMENTATION_PROMPT.md      prompt pronto para o agente de implementação
├── DESIGN_TOKENS.md              tokens (CSS custom properties + tabela)
├── icons/
│   ├── icons.svg                 sprite com 37 símbolos
│   └── ICONS.md                  índice e utilização de cada ícone
├── assets/
│   └── ocinye_logo.png           logótipo oficial
└── prototype/
    └── Ocinye Workspace.dc.html  protótipo navegável (20 ecrãs) — referência de design
```

O protótipo é um único ficheiro HTML com um runtime de componentes: abre no browser e navega-se com
rato e teclado. Serve de fonte de verdade visual; a implementação deve seguir os padrões do codebase
de destino.
