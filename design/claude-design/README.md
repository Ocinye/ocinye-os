# Pacote Ocinye OS · Claude Design → Claude Code

Ramo de destino: **`ui/claude-design`** (D0 já aplicado). Nunca `main`.
Tudo cumpre `docs/ui/CLAUDE_DESIGN_CONTRACT.md`: classes e tokens `ods-` só, sem `style=""`, sem `<style>`/`<script>` inline, sem recursos externos além das Google Fonts, rotas / `action` / `name` / `data-oc` mantidos.

## Pastas
| Pasta | O que tem |
|---|---|
| `static/` | CSS por fatia e o sprite de ícones. Ordem de carregamento: `ocinye-ds.css` → `ods-d1-primitives.css` → `ods-d2-shell.css` → `ods-d3-auth.css` → `ods-d4-desktop.css` → `ods-d5-windows.css` → `ods-d6-launcher.css` → `ods-d7-nye.css` → `ods-d8-apps-core.css` → `ods-d9-apps.css` → `ods-d10-settings.css` → `ods-d11-adaptive.css`. `ods-icons.svg`: sprite com ids `ods-*` |
| `docs/ui/` | `CLAUDE_DESIGN_IMPLEMENTATION_MAP.md` (auditoria), `CLAUDE_DESIGN_FUNCTIONAL_GAPS.md` (G-01…G-09), e uma especificação por fatia: `D0_DESIGN_TOKENS.md`, `D1_PRIMITIVES.md`, `D2_SHELL.md`, `D3_AUTH.md`, `D4_DESKTOP.md`, `D5_WINDOWS.md`, `D6_LAUNCHER.md`, `D7_NYE.md`, `D8_FILES_NOTES_MAIL.md`, `D9_APPS.md`, `D10_SETTINGS_ADMIN.md`, `D11_ADAPTIVE.md` |
| `i18n/` | `d0…d11_catalog_entries.rs`: grupos `catalogo!` em pt/en/fr para juntar a `catalog.rs` e a `GROUPS` |
| `design-reference/` | protótipos aprovados (`Ocinye OS.dc.html`, `Ocinye OS Apps.dc.html`, `Ocinye OS Proposta.dc.html` + `support.js`, logótipo e ícones). Especificação visual; **não é código a copiar** (usa estilo inline) |

## Fatias e o que depende de contrato novo
| Fatia | Pode entrar já | Espera por |
|---|---|---|
| D1 primitivas | sim | — |
| D2 casca | sim | bloquear ecrã G-01; estado IA G-09 (indisponível até lá) |
| D3 login / MFA / primeiro acesso / arranque | sim | — |
| D4 Desktop | disposição fixa servida pelo servidor | personalizar G-02; indicadores G-03 (contagem por listagem até lá); predefinição G-04 |
| D5 janelas | não | G-05 |
| D6 lançador + barra | sim | recentes G-06 |
| D7 Nye | texto sim | voz G-07; posição guardada G-02 |
| D8 Ficheiros / Notas / Correio | sim | — |
| D9 restantes apps | sim | Monitor G-08 |
| D10 Definições / Admin | sim | tema por membro; predefinição G-04 |
| D11 adaptação / escuro / idiomas | com cada fatia | — |

Nenhuma fatia guarda estado de produto no navegador. Onde o contrato falta, a UI mostra o estado indisponível (`ods-state--unavailable` + `ods.state.pending_contract`).

## Limites deste pacote
- Escrito a partir de leitura do repositório: os marcadores listados são os medidos em `shell.rs`, `login.rs`, `mfa.rs`, `first_access.rs`, `boot.rs`, `home.rs`, `files.rs`, `mail.rs`, `settings.rs`, `administration.rs`. Para os restantes ecrãs (D9), mantém-se o marcador de cada elemento com o mesmo papel no ficheiro actual.
- Não foi compilado nem passado pelo `verify.sh`.
- As vistas Leptos são para o Claude Code escrever a partir destas especificações.
