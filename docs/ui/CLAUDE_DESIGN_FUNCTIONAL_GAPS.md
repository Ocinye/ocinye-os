# Lacunas funcionais encontradas pelo desenho

> Cada lacuna é uma coisa que o desenho aprovado mostra e que o repositório não
> sabe fazer. A UI **não** as simula: até haver contrato, o controlo aparece
> indisponível e diz porquê (§45).

| ID | Ecrã | Comportamento pedido | Contrato em falta | Forma proposta |
|---|---|---|---|---|
| G-01 | Casca › Bloquear ecrã | bloquear a sessão sem a terminar; desbloquear com palavra-passe; ⌘L | estado «sessão bloqueada» no BFF | `POST /session/lock`, `POST /session/unlock {password}`; o `session.rs` recusa rotas enquanto bloqueada, excepto `/lock` |
| G-02 | Desktop | widgets persistentes por membro: presença, ordem, tamanho, minimizado, fundo, densidade | `Desktop` e `Widget Registry` no Core | `GET/PUT /me/desktop` → `{version, base_default_version, wallpaper, density, widgets:[{id, kind, w, h, minimized}]}`; `WidgetKind` tipado em `ocinye-contracts` com tamanhos permitidos e se é obrigatório |
| G-03 | Desktop › indicadores Research | contagens de unidades activas, ideias em investigação, projectos em execução, datasets | resumo tipado | `GET /me/summary?profile=research` → `{units_active, ideas_investigating, projects_running, datasets}`; falha do Core nunca devolve 0 (contrato §5) |
| G-04 | Desktop › predefinição | admin compõe, pré-visualiza e publica (só novos / disponível / actualizar quem está na versão antiga / forçar todos); membro repõe com anular | modelo de predefinição versionada por perfil e instância | `GET/PUT /admin/desktop-default` (rascunho), `POST /admin/desktop-default/publish {mode}`, `POST /me/desktop/restore` |
| G-05 | Gestor de janelas | abrir apps em janelas internas, focar, mover, redimensionar, minimizar, maximizar, alternar, «todas as janelas» | estado de janelas; relação com rotas reais (§21) | estado **no cliente** (`app.js`) com a URL da janela activa em `history`; cada janela carrega a rota real num `<iframe>` same-origin (`frame-src 'self'` já é permitido). Decidir antes: iframe vs. fragmento SSR |
| G-06 | Lançador › Recentes | últimas apps abertas pelo membro | histórico de aberturas | acrescentar `recent_apps` a `GET /me` ou derivar de `activity` com `owner` |
| G-07 | Nye › voz | push-to-talk, estados ouvir / transcrever / agir / falar / falha | contrato de voz | `POST /ask/voice` (áudio → transcrição) ou Web Speech só no cliente, com consentimento explícito; sem escuta permanente |
| G-08 | Monitor de Actividade | processos/serviços com CPU, memória, disco, rede, GPU; terminar processo não-sistema | métricas de serviços e nós | `GET /admin/monitor?metric=cpu|mem|disk|net|gpu` → `{services:[{id, name, kind, owner, protected, value, value2}], summary}`; `POST /admin/monitor/{id}/stop` só para serviços não protegidos, autorizado no Core |
| G-09 | Casca › estado CORE · IA | resumo curto do Core e do fornecedor de IA activo | agregado tipado | `GET /me/status` → `{core:{ok, version, services}, ai:{mode:none|local|external, model, node}}` |

# Acrescento a docs/ui/CLAUDE_DESIGN_FUNCTIONAL_GAPS.md (fatia 1 · autenticação)

| ID | Ecrã | Comportamento | Contrato | Estado |
|---|---|---|---|---|
| G-26 | Recuperar palavra-passe (D10) | pedir instruções; resposta sempre neutra | `GET /password/recover` → `recover(false, disponivel, …)`; `POST /password/recover {email}` → 202 sempre → `recover(true, …)` | vista pronta; `disponivel=false` até existir o POST |
| G-27 | Sessão expirada / acesso revogado (D12, D13) | cartão por motivo | `/login?reason=expired` (cookie de sessão desconhecido) · `/login?reason=revoked` (motivo do Core) → `fim_de_sessao(…)` | vista pronta |
| G-30 | Idioma antes da sessão (D7) | pt · en · fr no rodapé do cartão | `POST /login/language {lang, return_to}` grava `oc_locale` e redirige | **CONNECT** |
| G-31 | Perfil e endereço à porta (D7, P1–P4) | etiqueta do perfil + anfitrião | `GET /api/v1/instance/branding` → `profile`; anfitrião do pedido → `Porta { nome, perfil, host }` → `login_na_porta(…)` | **CONNECT** |
| — | Chave de acesso · SSO (D7) | botões desenhados | sem ADR: `aria-disabled="true"` + `ods.state.pending_contract`; SSO só no perfil `business` | à espera de ADR |
| — | Lembrar espaço (D9) | preferência por dispositivo | sem contrato: caixa desligada com o estado | à espera de contrato |
