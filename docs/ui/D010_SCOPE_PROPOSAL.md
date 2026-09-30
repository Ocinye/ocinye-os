# D010 — ACCESS, PROVISIONING & SYSTEM SURFACE COMPLETION · proposta de âmbito

> Não é implementação. Não há `ocinye-design-complete-D010.zip` até o âmbito ser aprovado.
> DEPLOY = NOT_PERFORMED · DEPLOY_AUTHORIZATION = NOT_GIVEN

## Objectivo
Fechar o percurso do sistema operativo antes da primeira instalação de produção: instalação / ponto de acesso → autenticação → entrada na Distribuição → Desktop → estados de sessão e sistema → administração. Não redesenha as 28 aplicações.

## Modelo (congelado na D009 R2)
- Disponíveis (4) ⊇ activadas na Instância [1..4] ⊇ acessíveis ao membro. A escolha usa activadas ∩ acessíveis.
- Ponto de acesso → uma Instância; pode fixar uma Distribuição. Nunca concede autoridade. Anfitrião desconhecido falha fechado.
- Genérico: 0 → sem acesso · 1 → Desktop directo · >1 → escolher Distribuição. Fixo: verificar acesso; sem ele, estado honesto; nunca outra Distribuição.
- Antes da entrada, o genérico mostra só a Instância; o fixo pode mostrar a sua Distribuição.
- Sessão: membro · Instância · ponto de acesso · Distribuição activa · contexto activo (conceitos; nomes de campos depois da descoberta).
- Contextos dentro da Distribuição activa, escolhidos no Desktop; nunca na entrada.
- Mudar de Distribuição (só no genérico) fecha as janelas e abre o Desktop da outra; revalida activação, acesso e contexto; nada é transportado.
- Âmbitos: disposição, fixações e fundo por **membro + Instância + Distribuição**; predefinição da Instância futura por **Instância + Distribuição**; «Repor» regressa à predefinição da Distribuição activa.
- «Última Distribuição/contexto»: só preferência revalidada; fora do âmbito até haver contrato.

## Ecrãs (36 propostos · 33 obrigatórios · 29 sensíveis)

| SCREEN_ID | SCREEN_NAME | JOURNEY | CURRENT_REPO_STATUS | CURRENT_DESIGN_STATUS | DOMAIN/CONTRACT | D010_REQUIRED | SECURITY_SENSITIVE | PT | EN | FR | DESKTOP | TABLET | MOBILE | NOTES |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| S01 | Instalação · boas-vindas (web) | instalação | CLI apenas | DESIGN_ONLY | CONFIG | DECIDIR |  | req | req | req | req | req | req |  |
| S02 | Instalação · Distribuições activadas (várias) | instalação | perfil único | SUPERSEDED | CORE+CONFIG | SIM | SEC | req | req | req | req | req | req |  |
| S03 | Instalação · aplicações por Distribuição | instalação | activação real | DESIGN_ONLY | CORE | SIM |  | req | req | req | req | req | req |  |
| S04 | Instalação · primeiro administrador | instalação | CLI | — | CORE | SIM | SEC | req | req | req | req | req | req |  |
| S05 | Instalação · ponto de acesso canónico | instalação | PUBLIC_URL | — | RUNTIME+CONFIG | SIM | SEC | req | req | req | req | req | req |  |
| S06 | Instalação · pronto | instalação | — | DESIGN_ONLY | — | SIM |  | req | req | req | req | req | req |  |
| S07 | Entrada · ponto genérico | entrada | login.rs | R2 ref | RUNTIME | SIM | SEC | req | req | req | req | req | req | sem Distribuição antes da entrada |
| S08 | Entrada · ponto fixo | entrada | — | R2 ref | RUNTIME+CONFIG | SIM | SEC | req | req | req | req | req | req | www.empresa.com → Business |
| S09 | Escolher Distribuição | entrada | — | R2 ref | CORE+WORKSPACE | SIM | SEC | req | req | req | req | req | req | activadas ∩ acessíveis |
| S10 | Zero Distribuições | entrada | — | R2 ref | CORE | SIM | SEC | req | req | req | req | req | req |  |
| S11 | Ponto fixo · sem acesso | entrada | — | R2 ref | CORE | SIM | SEC | req | req | req | req | req | req | sem queda noutra Distribuição |
| S12 | Ponto fixo · Distribuição desactivada | entrada | — | R2 ref | CORE+CONFIG | SIM | SEC | req | req | req | req | req | req | sem redireccionamento |
| S13 | Anfitrião desconhecido | entrada | — | R2 ref | RUNTIME | SIM | SEC | req | req | req | req | req | req | falha fechada |
| S14 | Ponto de acesso mal configurado / TLS inválido | entrada | — | — | RUNTIME | SIM | SEC | req | req | req | req | req | req | estado servido fora da Instância |
| S15 | Mudar de Distribuição | sessão | — | R2 ref | WORKSPACE+CORE | SIM | SEC | req | req | req | req | req | req | só no genérico |
| S16 | Confirmar mudança (janelas fecham) | sessão | — | R2 ref | WORKSPACE | SIM | SEC | req | req | req | req | req | req |  |
| S17 | Distribuição destino indisponível | sessão | — | — | CORE | SIM | SEC | req | req | req | req | req | req |  |
| S18 | Acesso à Distribuição revogado a meio | sessão | — | — | CORE | SIM | SEC | req | req | req | req | req | req |  |
| S19 | Mudar de contexto no Desktop | Desktop | sem contexto activo | R2 ref | CORE | SIM | SEC | req | req | req | req | req | req |  |
| S20 | Sem contexto disponível | Desktop | — | — | CORE | SIM |  | req | req | req | req | req | req |  |
| S21 | Contexto revogado/apagado enquanto activo | Desktop | — | — | CORE | SIM | SEC | req | req | req | req | req | req |  |
| S22 | Bloquear / desbloquear | sessão | desactivado | D15 protótipo | WORKSPACE | SIM | SEC | req | req | req | req | req | req |  |
| S23 | Sessão expirada por anfitrião | sessão | real | D001 | RUNTIME | SIM | SEC | req | req | req | req | req | req | cookie host-only |
| S24 | Recuperar palavra-passe (real) | entrada | ecrã sem endpoint | D001 | CORE | SIM | SEC | req | req | req | req | req | req |  |
| S25 | Convite por ligação | entrada | primeiro acesso | — | CORE | DEFERRED | SEC | req | req | req | req | req | req |  |
| S26 | Administração › Distribuições activadas | administração | — | D19 superseded | CORE | SIM | SEC | req | req | req | req | req | req | sem desactivação destrutiva |
| S27 | Administração › Pontos de acesso · lista | administração | — | — | CONFIG | SIM | SEC | req | req | req | req | req | req | canónico marcado |
| S28 | Pontos de acesso · adicionar domínio | administração | — | — | CONFIG+RUNTIME | SIM | SEC | req | req | req | req | req | req |  |
| S29 | Pontos de acesso · verificação DNS pendente | administração | — | — | RUNTIME | SIM | SEC | req | req | req | req | req | req | instruções, sem automação falsa |
| S30 | Pontos de acesso · TLS pendente/activo/inválido | administração | — | — | RUNTIME | SIM | SEC | req | req | req | req | req | req |  |
| S31 | Pontos de acesso · associar Distribuição / genérico | administração | — | — | CONFIG | SIM | SEC | req | req | req | req | req | req |  |
| S32 | Pontos de acesso · desactivar (protege o último/canónico) | administração | — | — | CONFIG | SIM | SEC | req | req | req | req | req | req |  |
| S33 | Administração › Proveniência das predefinições por Distribuição | administração | — | D009 | WORKSPACE | SIM |  | req | req | req | req | req | req |  |
| S34 | Ligação profunda para outra Distribuição | sessão | — | — | CORE+WORKSPACE | SIM | SEC | req | req | req | req | req | req | pedir mudança ou recusar |
| S35 | Manutenção | sistema | — | D35 design only | CORE | DEFERRED |  | req | req | req | req | req | req |  |
| S36 | Core indisponível por ponto de acesso | sistema | real (/boot) | D001 | RUNTIME | SIM |  | req | req | req | req | req | req |  |

## Contratos em falta
**Core (9):** C1 conjunto de Distribuições activadas por Instância (substitui `organisations.profile`) · C2 Distribuições acessíveis por membro (não é um papel; não substitui RBAC) · C3 Distribuição activa na sessão/principal · C4 disposição, fixações e fundo por Distribuição (`member_desktop_layouts`/`member_app_pins` com Distribuição na chave) · C5 predefinição da Instância por Distribuição (FG-014) · C6 contexto activo (hoje inexistente) · C7 resolução de ligação profunda com Distribuição de destino · C8 bloqueio/desbloqueio · C9 recuperação de palavra-passe.
**Runtime (6):** R1 mapeamento anfitrião → ponto de acesso tipado e confiável (não o Host arbitrário) · R2 várias origens públicas (`origin_is_ours`, CSRF, redirecionamentos) · R3 fronteira de sessão por anfitrião documentada · R4 certificado: emissão, renovação, inválido · R5 proxy (nginx `server_name`) gerado da configuração · R6 URIs de redirecionamento SSO exactos por ponto de acesso (se houver SSO).
**Configuração (4):** K1 esquema de ponto de acesso (anfitrião, Instância, Distribuição opcional, canónico, estado) · K2 `bootstrap-admin` com várias Distribuições · K3 registo de verificação de domínio · K4 invariantes (último/canónico não se apaga; Distribuição desactivada falha honesto).

## Portões de segurança
anfitrião desconhecido · Host forjado · ponto fixo com Distribuição errada · Distribuição desactivada · membro sem acesso · enumeração de Distribuições antes da entrada · janela obsoleta entre Distribuições · ligação profunda entre Distribuições · fronteira de cookie · CSRF/origem com vários anfitriões · redirecionamento de domínio próprio · open redirect · URIs SSO exactos · contexto depois de mudar de Distribuição.

## Design-only vs Code
Design: S01–S36 em 1440/924/390 × pt/en/fr, estados de administração de pontos de acesso, instruções DNS (sem automação falsa). Code: C1–C9, R1–R6, K1–K4, migração de âmbito por Distribuição, testes dos portões.

## Dependências
D009 integrado primeiro (sem aprofundar «uma Distribuição por Instância»). C1–C3 antes de S07–S18. C4 antes de activar uma segunda Distribuição em produção.

## Aceitação
Cada ecrã nos 3 tamanhos e 3 idiomas; portões de segurança provados no browser real e em testes; nenhum anfitrião desconhecido abre uma Instância; nenhum ponto fixo cai noutra Distribuição; mudança de Distribuição não deixa janelas da anterior; «Repor» nunca cruza Distribuições.
