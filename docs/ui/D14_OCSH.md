# D14 · ocsh — linguagem

## Princípios
- Gramática: `família subcomando [argumentos] [--opções] [| operação …]`. IDs em inglês técnico, estáveis nas três línguas. Sem aliases localizados nesta versão.
- O registo de comandos é **o mesmo** que alimenta a Superfície Universal de Comandos e o Nye (G-10). Não há taxonomia duplicada.
- Descoberta por função: famílias acima da função do membro **não aparecem** no autocompletar nem no `help`; se escritas, devolvem «Permissão negada» genérico.
- `?` no início da linha = linguagem natural para o Nye (explícito; nunca se adivinha).

## Registo (v1)
| Grupo | Família | Subcomandos | Função | Notas |
|---|---|---|---|---|
| Espaço de trabalho | `apps` | list · open · pin · unpin | membro | fonte: Registo de Aplicações |
| | `open` | `open <app> [recurso]` | membro | directo |
| | `window` | list · focus · tile · minimize | membro | via Gestor de Janelas |
| | `desktop` | widgets · add · reset (médio) | membro | |
| | `context` | show (omissão) · list · use | membro | `context personal` ≡ `context use personal` |
| | `session` | status (omissão) · elevate (admin) · drop | membro | |
| Ficheiros | `files` | pwd · ls · cd · tree · mkdir · open · mv · rename · trash · restore · upload | membro | `trash` é reversível: sem confirmação, mostra `files restore` |
| | `storage` | status (omissão) | membro | |
| Trabalho | `projects` | list (--json, --status) · open · show | membro | |
| | `tasks` | list (--mine, --today, --json) · done | membro | |
| | `notes` | list · new | membro | |
| | `mail` | unread (omissão) | membro | |
| | `calendar` | today (omissão) | membro | |
| IA | `nye` | ask · run · explain | membro | `help --nye` ≡ `nye explain` |
| | `ai` | status (omissão) | membro | |
| Administração | `members` | list · invite (médio) · revoke (alto) | admin + elevação | |
| | `resources` | status (omissão) | admin + elevação | |
| | `policies` | list (omissão) | admin + elevação | |
| Sistema | `system` | status (omissão) · version · logs (operador) | membro | |
| | `nodes` | list (omissão) | operador | |
| | `backup` | run · status | operador | |
| Shell | `help` `clear` `history` `whoami` `exit` | | todos | locais ao cliente |

## Espaço de nomes
- Raiz virtual `~` = contexto activo. `~/files`, `~/projects`, `~/notes`. Forma canónica de ligação: `ocinye://<contexto>/files/…`.
- Comandos `files` resolvem relativos a `~/files` quando a pasta actual está fora dele.
- Caminhos do anfitrião (`/etc`, `/usr`, `/home`) → erro «Caminho fora do espaço Ocinye».

## Pipelines tipados
A saída de cada comando de listagem é um conjunto tipado (`{cap, cols, rows}`), não texto.
| Operação | Efeito |
|---|---|
| `filter <texto>` | mantém linhas cujo valor ou rótulo contém o texto |
| `sort <coluna> [--desc]` | ordena (numérico-consciente) |
| `head <n>` | primeiras n |
| `count` | número de linhas |
| `export json` | igual a `--json` |
Operação desconhecida → erro com a lista. O cabeçalho mostra o percurso: `pipeline: projects.list → filter(active) → sort(name)`.

## Códigos de saída
| Código | Significado |
|---|---|
| 0 | sucesso |
| 1 | erro de execução / recurso não encontrado / confirmação recusada |
| 2 | uso inválido (subcomando, opção, argumento em falta) |
| 69 | capacidade indisponível (ex.: IA sem recurso) |
| 77 | permissão negada ou requer elevação |
| 126 | bloqueado (shell do anfitrião, sudo) |
| 127 | comando não encontrado |
| 130 | cancelado (Ctrl+C / Esc) |

## Risco (decidido pelo Core — G-13)
| Nível | Padrão | Exemplo |
|---|---|---|
| baixo | executa | `files ls`, `files trash` |
| médio | pré-visualização + `[s/N]` (en `[y/N]`, fr `[o/N]`) | `desktop reset`, `members invite`, `history clear` |
| alto | pré-visualização determinística + palavra escrita (**REVOGAR / REVOKE / RÉVOQUER**) + recibo | `members revoke`, `shutdown`, `remove provider`, `bulk send` |
A pré-visualização vem do Core (não do cliente): Acção, Alvo, Efeitos, Preserva, Reversível, Capacidade, Auditoria. Texto errado → «Confirmação não corresponde. Cancelado — nada foi alterado.» Nunca uma tecla só.

## Elevação (G-14)
- `session elevate` → pré-visualização (capacidades, 15 min, só este separador, MFA TOTP, Audit Log) → código de 6 dígitos mascarado → faixa ouro «SESSÃO DE ADMINISTRAÇÃO · expira em mm:ss · Terminar».
- A elevação nunca se duplica com o separador, nunca passa ao Nye, expira sozinha. `session drop` termina.
- Não existe `sudo`. `sudo`, `su`, `bash`, `sh`, `zsh`, `ssh`, `host` → nota explicativa (exit 126). A **Consola do Anfitrião** é outra superfície, só operadores, fora deste pacote.

## Nye no Terminal
- `nye ask "…"` / `? …`: resposta em bloco `nye` com a rota de inferência e as fontes (capacidades usadas).
- `nye run "…"`: o Nye **propõe** um comando determinístico; a confirmação mostra que a autoridade é a do membro. Nunca executa em silêncio.
- `nye explain` / `help --nye`: explica o último erro.
- Sem IA: `nyeUnavail` + equivalente determinístico. Todo o resto funciona.

## Histórico
- Local ao membro (sessão) até G-15. ↑/↓, `history`, Ctrl+R.
- Valores de `--password`, `--token`, `--secret`, `--key` substituídos por `••••` antes de guardar. `history clear` pede `[s/N]`.
