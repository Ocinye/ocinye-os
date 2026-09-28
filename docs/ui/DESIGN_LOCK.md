# DESIGN_LOCK

Implementação visual canónica do Ocinye OS, escrita pelo Claude Design. O que está `LOCKED` não se reestrutura, não se re-estiliza e não se «simplifica»; a integração faz-se à volta, pelos ViewModels.

- Base: `chore/ui-wipe` @ `c99cbda`
- Referência de aceitação: `design/claude-design/reference/` («Ocinye OS», «Ocinye OS Apps», «Ocinye OS Proposta»)

## Linguagem visual
- Controlos em cápsula (raio = metade da altura); botões de ícone quadrados em círculo; cartões 20px; cartão de autenticação 24px.
- Navegação activa: um só estado, fundo azul Ocinye e texto branco. Sem sublinhado dourado.
- Um só dourado por ecrã (acção principal).
- Tudo o que está no Desktop é vidro: fundo translúcido, contorno fino claro, desfoque, texto branco. Qualquer widget novo herda-o de `.oc-dw` (variáveis `--dw-*` na `.oc-desk`); em fundos claros o texto passa a escuro automaticamente.
- «Instância» sempre com maiúscula em pt e en.

## LOCKED
| Componente | Implementação | Parte |
|---|---|---|
| Documento base | `ui/document.rs` | P0 |
| Tokens, fontes, reposição, foco, estados | `static/oc-base.css` | P0 |
| Relógio, copiar | `static/oc-base.js` | P0 |
| Sprite de ícones | `static/icons.svg` | P0 |
| Peças partilhadas | `ui/components/mod.rs` | P0 |
| Moldura de autenticação (barra, identidade, rodapé) | `ui/screens/auth/mod.rs` · `static/oc-auth.css` | P1 |
| Início de sessão (dois passos) · Recuperar (G-26) · Fim de sessão | `ui/screens/auth/login.rs` · `static/oc-auth.js` | P1 |
| Primeiro acesso | `ui/screens/auth/first_access.rs` | P1 |
| MFA · configurar (duas colunas) · códigos (duas colunas) · desafio | `ui/screens/auth/mfa.rs` | P1 |
| Arranque | `ui/screens/auth/boot.rs` | P1 |
| Casca: barra de cima, barra de aplicações, lançador, paleta, «+ Criar», menus, estado | `ui/shell/mod.rs` · `static/oc-shell.css` · `static/oc-shell.js` | P2 |
| Janela de aplicação (página inteira, G-05) | `ui/shell/mod.rs::app_window` | P2 |
| Desktop: grelha, widgets, edição, biblioteca, fundo, escurecimento, repor | `ui/screens/home/mod.rs` · `static/oc-desk.css` · `static/oc-desk.js` | P2.3 |
| Registo de widgets e predefinições do sistema | `ui/screens/home/registry.rs` | P2.3 |
| Ícone por aplicação | `ui/components::app_icon` | P2 |

## Decisões aprovadas (valem para as partes seguintes)
- Login em dois passos só no browser; um único `POST /login`.
- Sem chave de acesso nem SSO à porta; sem «escolher espaço».
- D8a/D8b em duas colunas; D10 indisponível (G-26), com a confirmação neutra desenhada.
- À porta: logótipo, «OCINYE OS» e a distribuição (código + nome). Sem nome da Instância nem endereço (decisão do Fidel).
