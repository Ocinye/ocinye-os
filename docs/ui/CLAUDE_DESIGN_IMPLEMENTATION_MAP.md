# Mapa de implementação do Claude Design

> Auditoria **só de leitura** ao repositório `ocinye-os` (pasta local), feita a
> 2026-09-27. O Claude Design não tem permissão de escrita no repositório: este
> ficheiro e o pacote em `handoff/` são para o Claude Code aplicar no ramo
> `ui/claude-design`.

## 0. Estado do reset — **não concluído**

| Verificação | Resultado |
|---|---|
| `OCINYE_LEGACY_UI_REMOVED = TRUE` | **ausente** em todo o repositório |
| `OCINYE_READY_FOR_CLAUDE_DESIGN_HANDOFF = TRUE` | **ausente** |
| `docs/ui/CLAUDE_DESIGN_HANDOFF.md` | diz `UI_STATUS = DESIGN_IN_PROGRESS` |
| `static/ocinye.css`, `static/icons.svg`, `ui/components/` | **ainda presentes** (7 191 linhas de CSS legado, 50 ícones) |
| `UI_RESET_REPORT.md`, `CLAUDE_DESIGN_HANDOFF_MANIFEST.md`, `BEHAVIOURAL_CONTRACT_MATRIX.md` | **em falta**; existem `CLAUDE_DESIGN_CONTRACT.md`, `CLAUDE_DESIGN_HANDOFF.md`, `LEGACY_UI_INVENTORY.md` |

Consequência: a implementação de ecrãs **não começa**. O pacote D0 (tokens e
fundação) pode entrar já, porque é aditivo e usa um prefixo novo (`ods-`) que não
colide com as classes `oc-*` legadas.

## 1. Restrições técnicas que mudam a forma do desenho

| Restrição (`CLAUDE_DESIGN_CONTRACT.md`) | Efeito na implementação |
|---|---|
| Vistas em Rust/Leptos SSR (`apps/workspace/src/ui/`) | os protótipos `.dc.html` são **especificação**, não código a copiar |
| CSP: sem `style=""`, sem `<style>` e sem `<script>` inline | todo o estilo inline dos protótipos passa a **classes** em `static/`; valores dinâmicos (posição de janela, % de barra) passam por atributos `data-*` lidos pelo `app.js`, que usa `element.style.setProperty` (a CSP permite estilo definido por script same-origin via CSSOM) |
| JS só em `static/app.js`, sem framework | animações dos protótipos (anéis do Nye, gráfico do Monitor) passam a CSS `@keyframes` + SVG estático |
| Sem recursos externos além das Google Fonts | ícones num **sprite novo** servido pelo Workspace; o logótipo já está em `static/ocinye_logo.png` |
| Rotas, `action`/`name` e `data-oc` imutáveis | cada ecrã novo mantém os marcadores; o mapa abaixo diz onde |
| Textos só por `catalog.rs` (pt canónico, en, fr) | o pacote traz as chaves novas prontas em `catalogo!` |

## 2. Mapa superfície a superfície

Legenda: **CONNECT** existe contrato, só falta apresentação · **IMPLEMENT NEW**
componente visual novo sobre contrato existente · **ADAPT CONTRACT** o contrato
existe mas precisa de um campo ou rota · **FUNCTIONAL GAP** não existe backend ·
**BLOCKED** depende de outra fatia.

| Superfície do desenho | Rota / ficheiro actual | Contrato funcional | Classe |
|---|---|---|---|
| Tokens, tipografia, foco, movimento, claro/escuro | `static/ocinye.css` (legado) | — | **IMPLEMENT NEW** (D0, entregue) |
| Primitivas (botão, campo, separador, menu, popover, toast, tabela) | `ui/components/` (legado) | contratos a11y dos separadores ficam | **IMPLEMENT NEW** (D1) |
| Barra de topo: logótipo, ícone de perfil «Re/Bu/Pe/Ed» | `ui/shell.rs` | `InstanceProfile` em `ocinye-contracts/application.rs`; `/admin/instance` | **CONNECT** |
| Contexto «UENR-001 · Home» | `ui/shell.rs`, `/workspaces/{id}` | `GET /me` (unidades do membro) | **CONNECT** |
| Campo «Pergunte ou peça ao Nye…» | `/ask`, `/search` | planos `/ask/plans/{id}/execute|reject` | **CONNECT** |
| «+ Criar» com atalhos | vários `…/new` | `/notes/new`, `/tasks/new`, `/ideas/new`, `/units/new`, `/datasets/new`, `/ai/agents/new`, `/mail/compose` | **CONNECT** (atalhos de teclado: `app.js`) |
| Indicador CORE · IA + cartão de estado | `/health`, `/ai`, `/compute` | saúde do Core existe; estado do fornecedor de IA em `0056_ai_providers` | **ADAPT CONTRACT** — falta um resumo tipado «Core ok / IA: nenhum, local, externo» consumível pela casca |
| Relógio + calendário mensal | `/calendar` | eventos existem | **CONNECT** |
| Menu do utilizador (Conta, Definições, Ajuda, Terminar sessão) | `/settings`, `/help`, `POST /logout` | existe | **CONNECT** |
| Bloquear ecrã (⌘L) | — | não há rota nem estado de sessão bloqueada | **FUNCTIONAL GAP** G-01 |
| Desktop (Home) com widgets persistentes | `/` → `screens/home.rs` (painel fixo) | não há `Desktop`, `Widget Registry` nem persistência | **FUNCTIONAL GAP** G-02 |
| Minimizar / redimensionar / mover widgets | — | idem | **FUNCTIONAL GAP** G-02 |
| Cartões de indicadores Research (Unidades, Ideias, Projectos, Datasets) | `/units`, `/ideas`, `/projects`, `/datasets` | contagens por listagem; sem endpoint de resumo | **ADAPT CONTRACT** G-03 |
| Predefinição do Desktop (membro repõe, admin publica) | — | nada | **FUNCTIONAL GAP** G-04 |
| Aviso «nova predefinição disponível» | — | depende de G-04 | **BLOCKED** |
| Gestor de janelas (abrir, focar, mover, minimizar, maximizar, alternar) | — (cada app é uma página) | não há estado de janelas; §21 exige URLs reais | **FUNCTIONAL GAP** G-05 |
| Todas as janelas (visão geral) | — | depende de G-05 | **BLOCKED** |
| Botão das aplicações + barra vertical (desaparece em repouso) | `ui/shell.rs` (lançador) | `ui/apps.rs` (registo), `PUT /apps/pins` | **CONNECT** |
| Lançador de aplicações (pesquisa, categorias, favoritos, recentes) | `ui/shell.rs` | registo + pins; «recentes» não existe | **CONNECT** + G-06 |
| Nye flutuante arrastável (posição guardada) | — | posição é preferência local (`app.js`, `PREFS`) | **IMPLEMENT NEW** |
| Popup circular do Nye (texto, voz, resposta no círculo) | `/ask`, `/ai/prompt` | planos determinísticos existem; voz não | **CONNECT** (texto) + **FUNCTIONAL GAP** G-07 (voz) |
| Nye completo | `/ask` | existe | **CONNECT** |
| Monitor de Actividade | `/compute`, `/activity` | não há métricas de processos/serviços | **FUNCTIONAL GAP** G-08 |
| Painel do perfil (instância, predefinição, módulos) | `/admin/instance` | perfil e apps activas existem; «versão da predefinição» depende de G-04 | **CONNECT** + **BLOCKED** |
| Ficheiros, Notas, Correio, Mensagens, Calendário | rotas `/files`, `/me/files/*`, `/notes/*`, `/mail/*`, `/messages/*`, `/calendar` | completos | **CONNECT** (D8) |
| Login, MFA, primeiro acesso, arranque | `/login`, `/mfa/*`, `/first-access`, `/boot` | completos | **CONNECT** (D3) |
| Definições | `/settings/*` | conta, segurança, MFA, apps, avatar | **CONNECT** (D10) |
| Administração / Audit | `/admin/*`, `/audit` | completos | **CONNECT** (D10) |

## 3. Ordem proposta

1. **D0 — este pacote.** `handoff/static/ocinye-ds.css`, `handoff/docs/ui/D0_DESIGN_TOKENS.md`, `handoff/i18n/d0_catalog_entries.rs`.
2. O Claude Code conclui o reset e fixa as duas flags.
3. D1 primitivas → D2 casca (tudo **CONNECT**) → D3 login.
4. D4/D5 (Desktop, janelas) **só depois** de G-02 e G-05 terem contrato: não se simula persistência no cliente (§45).

## 4. Ficheiros a rever pelo Claude Code

- `apps/workspace/static/ocinye-ds.css` (novo; ligar no `<head>` **depois** de `ocinye.css` durante a transição, e sozinho depois do reset)
- `apps/workspace/src/i18n/catalog.rs` (acrescentar o grupo `DS_SHELL` de `d0_catalog_entries.rs` a `GROUPS`)
- `docs/ui/CLAUDE_DESIGN_FUNCTIONAL_GAPS.md` (novo)
