# Prompt de implementação — Ocinye Workspace

Copie o texto abaixo (a partir da linha `---`) para o seu agente de implementação (Claude Code ou
equivalente), com esta pasta de handoff disponível no repositório.

---

Vais implementar o **Ocinye Workspace**, a interface humana do Ocinye OS (sistema operacional
institucional da Ocinye). Tens um dossier de design completo nesta pasta:

- `README.md` — especificação de todos os ecrãs, componentes, estados e comportamentos
- `DESIGN_TOKENS.md` — cores, tipografia, espaçamento, radius, sombras, animações
- `icons/icons.svg` + `icons/ICONS.md` — os 37 ícones do design e onde cada um é usado
- `assets/ocinye_logo.png` — logótipo oficial
- `prototype/Ocinye Workspace.dc.html` — protótipo navegável de alta fidelidade (20 ecrãs)

**Lê o README.md e o DESIGN_TOKENS.md por inteiro antes de escrever código, e abre o protótipo no
browser para ver o comportamento real** (navegação pela sidebar, hover das tabelas, `⌘K`, menu
`+ Criar`, colapsar sidebar, empty states).

## O que construir

Recria estes ecrãs no ambiente do codebase de destino, usando os seus padrões, biblioteca de
componentes e convenções de routing/estado já existentes. Se o projecto ainda não tiver frontend,
usa React + TypeScript + Vite (Next.js se houver necessidade de SSR), CSS Modules ou Tailwind com os
tokens do dossier mapeados 1:1.

O protótipo HTML é **referência de design, não código de produção** — não o copies nem o embutas.

Fidelidade: **alta**. Cores, tipografia, espaçamento, densidade e estados são finais e devem ser
reproduzidos com exactidão. Se algo não estiver especificado, segue o padrão equivalente mais próximo
do dossier em vez de inventar.

## Ordem de trabalho

1. **Fundações** — tokens (CSS custom properties de `DESIGN_TOKENS.md`), IBM Plex Sans + IBM Plex Mono,
   reset, focus ring dourado (`outline:2px solid #E0A731; outline-offset:1px`), componente de ícone
   que consome `icons/icons.svg`.
2. **Shell** — `AppShell` com sidebar colapsável (224px ↔ 58px), topbar (breadcrumb, pesquisa global,
   `+ Criar`, notificações, estado do Core, avatar), área de conteúdo com scroll e routing.
3. **Command palette** (`⌘K` / `Ctrl+K`, `Esc` fecha) com grupos `NAVEGAR` e `ACÇÕES`.
4. **Componentes base** — `DataTable` (tabs, pesquisa, filtro, header de colunas, linhas densas,
   paginação), `Badge` (7 tons, sempre ponto + texto), `Card`/`SectionHeader`, `Button` (4 variantes),
   `Field`/`Textarea`/`Select`/`Toggle`/`Checkbox`, `Tabs` (pill e contextual), `EmptyState`,
   `ProgressBar` e `ProgressDonut`.
5. **Login** — ecrã escuro estilo workstation, sem MFA, sem signup, sem login social.
6. **Ecrãs de lista** com o mesmo `DataTable`: Unidades, Ideias, Projectos, Bibliografia, Dados,
   Agentes, Membros (Administração), Audit Log.
7. **Home / Dashboard**, **O Meu Trabalho**, **Conhecimento**, **Actividade**.
8. **Research Workspaces** — Ideia (13 tabs) e Projecto (13 tabs), com cabeçalho contextual.
9. **Detalhe da Unidade** (9 tabs).
10. **Ocinye AI** — hub com empty state, **Criar Agente IA**, **Prompt Ocinye**, **Computação**.

Entrega em fatias verificáveis: fundações + shell primeiro, depois um ecrã de cada tipo, depois os
restantes. Após cada fatia, confirma visualmente contra o protótipo.

## Regras não negociáveis

1. **É um ambiente operacional, não um website.** Sem homepage pública, marketing, pricing, blog ou
   landing page. Densidade útil, tabelas compactas, navegação institucional persistente.
2. **Cor:** Deep Navy `#0B2D4A` como estrutura, Sunrise Gold `#E0A731` só como acento/CTA/selecção.
   Máximo dois fundos por ecrã. Sem gradientes fortes, glassmorphism (excepto cartão do login),
   sombras pesadas, cards gigantes ou elementos decorativos sem função.
3. **Ícones:** usa exclusivamente o conjunto de `icons/icons.svg` (traço fino, geometria simples,
   `currentColor`). Não introduzir ícones coloridos nem misturar bibliotecas com pesos diferentes.
   Se faltar um ícone, desenha-o no mesmo estilo (`stroke-width` 1.3–1.8, viewBox 14/16).
4. **Estado nunca só por cor:** todos os badges levam ponto + texto. As quatro classificações
   (`PUBLIC`, `INTERNAL`, `CONFIDENTIAL`, `RESTRICTED`) são visíveis em Dataset, Document, Idea,
   Project e no contexto de IA.
5. **Ideia ≠ Projecto.** Estados de Idea: Discovery, Exploration, Concept, Review, Project Candidate,
   Archived. Estados de Project: Draft, Active, On Hold, Completed, Archived.
6. **Audit Log não é feed de actividade:** notação técnica de acções, recurso, contexto, resultado e
   correlation ID.
7. **Infraestrutura ainda não existe:** Ocinye AI e Computação mostram estados vazios elegantes
   (“Nenhum nó de IA Ocinye está actualmente disponível”, “0 nós registados”). Não inventar nós,
   modelos ou métricas online. A arquitectura visual fica pronta para quando existirem.
8. **A IA é capacidade transversal**, não um menu secundário: hub, agentes, criação de agentes e
   Prompt Ocinye são funções de primeiro nível, e cada Research Workspace tem tab `IA` que abre o
   prompt já vinculado ao seu contexto.
9. **Prompt Ocinye tem de ser a peça mais bem resolvida da interface** e não um clone de chat
   genérico: barra de contexto (agente + Research Workspace + capacidade), área de conversa preparada
   para texto, fontes, referências, datasets, documentos, código, resultados e tabelas, e o input
   grande com acções discretas e botão circular dourado de envio.
10. **Desktop-first** (referência 1440×900; funcional a partir de ~1180px com scroll horizontal nas
    tabelas). Adapta a laptop e tablet sem sacrificar densidade. Sem dark mode nesta fase — só o
    login é escuro, mas os tokens devem permitir uma variante escura futura.
11. **Acessibilidade:** contraste AA, focus visível, navegação completa por teclado (nav, tabs,
    tabelas, palette), labels em todos os campos, corpo ≥ 12px.
12. **Conteúdo é exemplo.** Todos os nomes, unidades, projectos, datasets e agentes do protótipo são
    dados de demonstração: liga a dados reais e não fixes nada em código. Mantém a língua
    **pt-PT** exactamente como está nos ecrãs (rótulos, placeholders, estados e mensagens).

## Dados / API que a UI espera

Unidades, Ideias, Projectos, Tarefas, Actividade, Documentos, Bibliografia, Datasets, Membros,
Audit Log, Agentes, Modelos, Nós de computação e contadores de KPI. Paginação, ordenação, filtro e
pesquisa devem ser server-side. Cada acesso a dados classificados deve gerar entrada no Audit Log.

## Definição de pronto

- Todos os ecrãs do §4 do README implementados e navegáveis.
- Tokens centralizados; zero valores de cor/tamanho soltos nos componentes.
- Tabelas, badges, tabs, empty states e botões vêm de componentes partilhados (nenhuma duplicação
  por ecrã).
- `⌘K`, menu `+ Criar`, colapsar sidebar, hovers e focus states a funcionar.
- Comparação lado a lado com o protótipo sem desvios visíveis de cor, tipografia ou espaçamento.
