# Iconografia canónica · D009

Um mapa: `experience::iconography::APP_ICONS` (`ApplicationId → símbolo`). `ui::components::app_icon(href)` delega nele; barra, lançador, paleta, janela e fichas lêem o mesmo símbolo (ICON-01).

**Norma (ICON-04/05):** `viewBox 0 0 16 16`, traço `currentColor` 1.4, terminações redondas, sem cor fixa, sem texto, sem raster, sem recurso remoto. Excepção: `nye` (dourado da marca, D003). Activo: fundo azul, ícone branco (sem mudança). Ícone ao lado de texto é decorativo (`aria-hidden`); controlo só de ícone tem `aria-label` (ICON-07).

**Auditoria:** KEEP 18 · REFINE 3 (files, settings, browser) · REPLACE 7 (units, ai, agents, resources, activity, administration, audit). MISSING 0 · TEMPORARY 0.

| app | D009 | antes | estado |
|---|---|---|---|
| `home` | `home` | `home` | KEEP |
| `work` | `work` | `work` | KEEP |
| `notes` | `notes` | `notes` | KEEP |
| `calendar` | `calendar` | `calendar` | KEEP |
| `trash` | `trash` | `trash` | KEEP |
| `mail` | `mail` | `mail` | KEEP |
| `messages` | `messages` | `messages` | KEEP |
| `files` | `files-app` | `files` | REFINE · DUPLICATE_SEMANTICS (= folha de Notas) |
| `knowledge` | `knowledge` | `knowledge` | KEEP |
| `bibliography` | `bibliography` | `bibliography` | KEEP |
| `units` | `org-tree` | `units` | REPLACE · DUPLICATE_SEMANTICS (= grelha Indicadores) |
| `ideas` | `idea` | `idea` | KEEP |
| `projects` | `project` | `project` | KEEP |
| `datasets` | `data` | `data` | KEEP |
| `results` | `results` | `results` | KEEP |
| `prompt` | `nye` | `nye` | KEEP (excepção de cor da marca) |
| `ai` | `ai-fabric` | `ai` | REPLACE · DUPLICATE_SEMANTICS (hexágono = Nye) |
| `agents` | `agents` | `agent` | REPLACE · WRONG_SEMANTICS (robô = chatbot) |
| `compute` | `compute` | `compute` | KEEP |
| `resources` | `resources` | `workspace` | REPLACE · WRONG_SEMANTICS (camadas ≠ quota) |
| `activity` | `activity-feed` | `activity` | REPLACE · WRONG_SEMANTICS (pulso = telemetria) |
| `administration` | `administration` | `admin` | REPLACE · DUPLICATE_SEMANTICS (raios = sol/definições) |
| `audit` | `audit-record` | `shield` | REPLACE · WRONG_SEMANTICS (escudo = segurança) |
| `monitor` | `gauge` | `gauge` | KEEP |
| `settings` | `gear` | `settings` | REFINE · INCONSISTENT_WEIGHT (dois círculos) |
| `help` | `help` | `help` | KEEP |
| `terminal` | `terminal` | `terminal` | KEEP |
| `browser` | `browser-window` | `browser` | REFINE · DUPLICATE_SEMANTICS (globo = Idioma) |

## Distribuições (ICON-02) — não são aplicações

Família: hexágono Ocinye (o mesmo contorno) + um glifo interior. `dist-research` três nós ligados (investigação estruturada) · `dist-business` módulos ordenados · `dist-personal` um ponto central (uma pessoa) · `dist-education` dois degraus (progressão). Sem letras (o distintivo «Re/Bu/Pe/Ed» passa a ícone; o nome continua no `aria-label` e no texto).

## Migração (ICON-09)

`MIGRATION`: /files files→files-app · /units units→org-tree · /ai ai→ai-fabric · /ai/agents agent→agents · /resources workspace→resources · /activity activity→activity-feed · /admin admin→administration · /audit shield→audit-record · /settings settings→gear · /browser browser→browser-window. Os antigos **ficam** no sprite (widgets, listas e ecrãs D001–D008 ainda os usam). Teste `a_migracao_nao_parte_referencias`.

## Validação

Pranchas 16/20/24/32/48 em claro, escuro e activo: `screenshots/d009/d009-55…61`. Testes: `cada_aplicacao_tem_exactamente_um_simbolo_que_existe`, `nao_ha_colisao_semantica`, `quatro_icones_de_distribuicao_que_existem`, `os_simbolos_canonicos_sao_seguros_e_seguem_o_tema` (lêem `static/icons.svg` com `include_str!`: um símbolo em falta falha o build de testes — ICON-03, §190).

## Lacuna

`experience::icon::Icon::id()` devolve ids `oc-*` que não existem no sprite (e cita `ods-icons.svg`, que também não existe). Não é desenhado hoje; deve apagar-se ou passar a `iconography` (G9-11).
