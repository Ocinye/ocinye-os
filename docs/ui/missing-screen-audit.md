# Auditoria de ecrãs em falta · D009 R2 (base para D010)

Três verdades: **A** repositório `main @ ece29e98d688f366006833b3fe7267e7e1663cc0` · **B** cobertura D001–D009 · **C** `Ocinye OS Proposta.dc.html` (v3, corrigida). A proposta não é verdade sozinha.

## Factos do repositório que decidem o âmbito

- Uma organização por Core (`state.organisation_id`); `organisations.profile` é **um** valor (`CHECK research|business|education|personal`).
- Um só endereço público: `OCINYE_WORKSPACE_PUBLIC_URL` (https em produção); a verificação de origem compara com ele (`routes::origin_is_ours`). Não há mapeamento anfitrião → Instância/Distribuição.
- Cookie de sessão sem `Domain` (host-only), `HttpOnly`, `SameSite=Lax`: cada anfitrião tem a sua sessão.
- `member_desktop_layouts` e `member_app_pins`: chave `person_id` — uma disposição (com o fundo dentro) e uma lista de fixações por membro, para todas as Distribuições.
- Sem contexto activo global (shell: «não há unidade activa global»); sem bloqueio de ecrã; recuperação sem endpoint.
- Não existem Equipa, Departamento, Turma, Tarefas (aplicação), Histórico.

## Quadros da proposta

| Quadro | Nome | Classe | Porquê |
|---|---|---|---|
| D1 | Instalação · Boas-vindas | DESIGN_ONLY | Hoje: install/ocinye + bootstrap-admin (CLI). Não há assistente web. |
| D2 | Instalação · Distribuição (escolha única) | SUPERSEDED | Passa a «activar uma ou mais». organisations.profile guarda um só valor → CORE/CONFIG contract (D010). |
| D3 | Instalação · Aplicações | DESIGN_ONLY | A activação por Instância é real (instance_applications); o passo de instalação não existe. |
| D4 | Instalação · IA opcional | DESIGN_ONLY | Fornecedores de IA configuram-se depois (ADR-0310). |
| D5 | Instalação · Inicialização | DESIGN_ONLY | O arranque real é /boot (BootVm). |
| D6 | Instalação · Pronto | DESIGN_ONLY |  |
| D7 | Entrada (antiga, «Research») | SUPERSEDED | Dividida em D7 genérico e D7b fixo. |
| D7 (R2) | Entrada · ponto genérico | REAL_DESIGNED_NOT_IMPLEMENTED | login.rs existe; a identidade da Instância sem Distribuição e o anfitrião precisam de pontos de acesso (D010). |
| D7b | Entrada · ponto fixo (www.empresa.com → Business) | MISSING_CONTRACT | Sem mapeamento anfitrião → Instância/Distribuição; um só OCINYE_WORKSPACE_PUBLIC_URL. |
| D8 | MFA · desafio | REAL_AND_IMPLEMENTED | mfa.rs |
| D8a | MFA · configurar | REAL_AND_IMPLEMENTED | mfa.rs |
| D8b | Códigos de recuperação | REAL_AND_IMPLEMENTED | mfa.rs |
| D9 (antigo) | Escolher espaço de trabalho | SUPERSEDED | Contexto não se escolhe na entrada. |
| D9 | Escolher Distribuição | MISSING_CONTRACT | Precisa de Distribuições activadas ∩ acessíveis (Core) e Distribuição activa na sessão. |
| D9b | Zero Distribuições acessíveis | MISSING_CONTRACT |  |
| D9c | Ponto fixo · sem acesso | MISSING_CONTRACT |  |
| D9d | Anfitrião desconhecido | MISSING_CONTRACT | Hoje qualquer Host chega à mesma Instância (sem mapeamento). |
| D10 | Recuperar palavra-passe | REAL_DESIGNED_NOT_IMPLEMENTED | RecoverVm.available = false (G-26). |
| D11 | Convite / primeiro acesso | REAL_AND_IMPLEMENTED | first_access.rs (palavra-passe definitiva). Convite por correio: DEFERRED. |
| D12 | Sessão expirada | REAL_AND_IMPLEMENTED | SessionEndReason::Expired |
| D13 | Acesso revogado | REAL_AND_IMPLEMENTED | Sem motivo do Core (G-27). |
| D14 | Core degradado | REAL_AND_IMPLEMENTED | BootState / Health |
| D15 | Ecrã de bloqueio | MISSING_CONTRACT | «Bloquear» aria-disabled (shell.user.lock_pending). |
| D16 | Desktop | REAL_AND_IMPLEMENTED | D001 + predefinições D009. |
| D17 (antigo) | Mudar de espaço | SUPERSEDED | Substituído por D17 (contexto dentro do Desktop). |
| D17 | Mudar de contexto (dentro da Distribuição) | MISSING_CONTRACT | Sem contexto activo global (shell: «não há unidade activa global», CLAUDE.md §34.3). |
| D17b | Mudar de Distribuição | MISSING_CONTRACT | Distribuição activa na sessão não existe. |
| D17c | Confirmar mudança de Distribuição | MISSING_CONTRACT | Decisão: fecha as janelas (reset do espaço de trabalho). |
| D18 | Administração › Instância | REAL_AND_IMPLEMENTED | D006 |
| D19 | Definições › Distribuição | SUPERSEDED | Passa a Administração › Distribuições activadas (D010). |
| D20 | Definições › Sistema | DESIGN_ONLY | Controlos de operador sem contrato. |
| D21 | Gestor de Aplicações | REAL_AND_IMPLEMENTED | shell::launcher; ícones D009. |
| D22 | Ficheiros | REAL_AND_IMPLEMENTED | D004 |
| D23 | Nye · conversa | REAL_AND_IMPLEMENTED | D003 |
| D24 | Definições › Idioma | REAL_AND_IMPLEMENTED | D007 |
| D25 | Notas | REAL_AND_IMPLEMENTED | D004 |
| D26 | Correio | REAL_AND_IMPLEMENTED | D004 |
| D27 | Calendário | REAL_AND_IMPLEMENTED | D004 |
| D28 | Tarefas (aplicação) | INVALID_DOMAIN | Não há ApplicationId::Tasks; tarefas em O Meu Trabalho/Projectos. |
| D29 | Actividade | REAL_AND_IMPLEMENTED | D007 |
| D30 | Administração · membros | REAL_AND_IMPLEMENTED | D006 |
| D31 | Modo escuro · Notas | DESIGN_ONLY | Sem modo escuro no produto. |
| D32 | Modo escuro · Correio | DESIGN_ONLY |  |
| D33 | Estado · sem permissão | REAL_AND_IMPLEMENTED | Load::Denied |
| D34 | Estado · não encontrado | REAL_AND_IMPLEMENTED |  |
| D35 | Estado · manutenção | DESIGN_ONLY | Sem estado de manutenção no Core. |
| D36 | Estado · quota cheia | REAL_DESIGNED_NOT_IMPLEMENTED |  |
| D37 | Todas as janelas | REAL_AND_IMPLEMENTED | D002 |
| D38 | Menu do utilizador | REAL_AND_IMPLEMENTED | Bloquear desactivado. |
| D39 | Monitor de Actividade | REAL_AND_IMPLEMENTED | D007.1 |
| D40 | Lixo | OUTDATED | «30 dias» não é regra do produto. |
| D41 | Terminal | OUTDATED | «session elevate» não existe; confirmação por plano (D008). |
| D42 | Browser | OUTDATED | «lista permitida» não existe (D008). |
| D43 | Nye · aplicação | REAL_AND_IMPLEMENTED |  |
| P1–P8 (antigos) | Presets com contextos na entrada, Equipas, Turmas | SUPERSEDED | Equipa/Departamento/Turma não existem no domínio. |
| P1–P8 (R2) | Presets de ponto de acesso e Distribuição | REAL_DESIGNED_NOT_IMPLEMENTED | P7/P8 (Desktops D009) implementáveis já. |
| E1–E5 | Idiomas | REAL_AND_IMPLEMENTED | pt/en/fr no catálogo. |
| E6 | Instalação · fr | DESIGN_ONLY |  |

## Percursos

| Percurso | Hoje (A) | Alvo | Classe |
|---|---|---|---|
| Instalação/bootstrap | CLI (bootstrap-admin --profile, um perfil) | Assistente web + várias Distribuições | CONFIG_CONTRACT_REQUIRED |
| Distribuições activadas | Uma (organisations.profile) | [1..4] por Instância | CORE_CONTRACT_REQUIRED · SEC |
| Ponto de acesso / domínio | Um OCINYE_WORKSPACE_PUBLIC_URL; origin_is_ours compara com ele | Mapeamento tipado anfitrião → Instância (+ Distribuição) | RUNTIME + CONFIG · SEC |
| Domínio próprio / TLS | https obrigatório em produção; sem gestão de domínios | Verificação DNS, certificado, renovação, estados | RUNTIME · DEFERRED parcial |
| Entrada | Email + palavra-passe, MFA privilegiado | Variante genérica e fixa | ADAPTER + CONTRACT |
| Passkey | Não existe | — | DEFERRED |
| Recuperação | Ecrã honesto, sem endpoint (G-26) | Endpoint no Core | CORE_CONTRACT_REQUIRED · SEC |
| Convite | Primeiro acesso por palavra-passe provisória | Convite por ligação | DEFERRED |
| Sessão expirada / revogada | Real (sem motivo de revogação) | Por anfitrião; motivo | ADAPTER |
| Zero / uma / várias Distribuições | Não existe (uma implícita) | D9/D9b + entrada directa | CORE_CONTRACT_REQUIRED · SEC |
| Mudar de Distribuição | Não existe | D17b/D17c, janelas fecham | WORKSPACE + CORE · SEC |
| Mudar de contexto | Não existe contexto activo global | D17 dentro do Desktop | CORE_CONTRACT_REQUIRED · SEC |
| Bloqueio | Desactivado no menu | D15 + desbloqueio | WORKSPACE_CONTRACT_REQUIRED · SEC |
| Core degradado / indisponível | Real | por ponto de acesso | ALREADY_IMPLEMENTED |
| Instância nova / primeiro membro | D009 predefinições | por Distribuição activada | ADAPTER |
| Administração da Instância | D006 (Instância, Membros, Papéis) | + Distribuições activadas, pontos de acesso, proveniência | MISSING_SCREEN |
| Gestor de Aplicações | Real | — | ALREADY_IMPLEMENTED |
| Terminar sessão | Real | por anfitrião (cookie host-only) | ALREADY_IMPLEMENTED |

## Responsivo e idiomas

Todos os ecrãs D010 precisam de 1440 · 924 · 390 e pt · en · fr. As referências R2 já cobrem: entrada fixa (1440/924/390), genérica (1440/390), escolha (1440/924/390, fr), zero (1440/390), anfitrião desconhecido (1440, en 390), contexto (1440/390).
