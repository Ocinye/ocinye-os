# Apagamento da UI — 2026-09-28

Ramo `chore/ui-wipe`, a partir de `feat/design-ui-canonical @ 2923bae` (o último
commit com interface). **Sem deploy**: a produção continua na UI que tinha.

## Porquê

A interface existente misturava pacotes D0–D14 transcritos a partir de
protótipos com a primeira fatia escrita pelo próprio Claude Design, e a
transcrição era a origem das divergências visuais. Decidiu-se apagar **toda** a
UI, **sem ponte**, incluindo a fatia 1, e recomeçar só com código do Claude
Design.

## O que saiu

| Onde | O quê |
|---|---|
| `apps/workspace/src/ui/` | Os ecrãs, a shell, os componentes, `ods`, o documento HTML |
| `apps/workspace/static/` | Os 17 CSS, `app.js`, `terminal.js`, `notes-editor.js`, o sprite `ods-icons.svg`, `ocinye_logo.png` |
| `apps/workspace/editor/` | A fonte do editor de notas (ProseMirror) |
| `apps/workspace/tests/` | `browser.rs`, `ui_contract.rs`, `experience_boundary.rs`, `installed_instance.rs` |
| `design/` | O dossier legado, os protótipos, os tokens, os ícones e as referências do Claude Design |
| `docs/ui/` | Os documentos D0–D14, o `DESIGN_LOCK`, as perguntas e o inventário |
| `scripts/` | `i18n-chrome-guard.py`, `ui-legacy-inventory.sh` |
| Dependências | `leptos`, `qrcode`, `pulldown-cmark`; em teste, `chromiumoxide` e as que só o harness de browser usava |
| i18n | 1 876 chaves de ecrã; ficaram 331 (`nav.*`, `apps.*`, `ocsh.*`, `terminal.*`) |

## O que ficou

- **As rotas.** O `router` tem os mesmos caminhos.
  - **Páginas `GET`** respondem `503` com o corpo `interface_pending` (texto,
    sem HTML).
  - **Acções `POST`** continuam a falar com o Core e redireccionam como antes.
    Uma recusa responde com o estado HTTP e um código estável (`not_found`,
    `forbidden`, `rejected`, …), sem desenhar o formulário de novo.
  - **O que serve bytes ou JSON** continua: descargas, pré-visualizações,
    rascunhos, carregamentos, *autosave*, fixações.
- **A sessão BFF**, o *same-origin*, os cabeçalhos de segurança, o portão de
  arranque e o idioma do pedido.
- **`apps/workspace/src/experience/`**: o registo de aplicações e o modelo de
  navegação (`Screen`, `Viewer`), que validam as fixações.
- **`static/runtime.js`** (ADR-0611), os avatares predefinidos e o logótipo do
  correio (o Core refere-os).
- **O Terminal no servidor** (`terminal.rs`) e as suas chaves de i18n.

## Acções desactivadas de propósito

Produzem um segredo que se mostra **uma única vez**; sem ecrã, executá-las
perdia-o (uma credencial que ninguém vê é uma conta trancada). Respondem
`interface_pending` sem chamar o Core:

`create_member`, `member_reset_password`, `provision_member`,
`settings_mfa_regenerate`, `mfa_confirm`.

Também `assist` (a escrita assistida devolvia o texto no compositor) e
`review_bibliography` (a revisão devolvia o resultado num ecrã).

## Consequência operacional

- **Não é possível usar o Ocinye OS por um browser neste ramo.** O login é um
  `POST /login` sem página que o envie.
- `scripts/install-e2e.sh`, `upgrade-e2e.sh`, `restore-e2e.sh` e
  `hardware-certification.sh` terminam em **`NOT_RUN`** (saída 2): a prova delas
  cria dados por um browser.
- `architecture-gates.sh` passa de 3 para 2 portões (`Runtime Boundary` entra;
  `Experience Structural Boundary` e `UI Contract` saem). O contrato de
  enumeração perde a linha `viagens-de-browser`.

## Testes retirados — 141

Os nomes ficam aqui porque são o inventário do comportamento que a UI nova tem
de voltar a provar. O código está em `2923bae`.

### `apps/workspace/tests/browser.rs` — 127

- `o_workspace_serve_um_browser_a_serio`
- `uma_pessoa_marca_um_compromisso_de_ponta_a_ponta`
- `as_vistas_partilham_o_universo_autorizado`
- `nenhuma_observacao_procura_um_elemento_sem_tolerar_a_transicao`
- `uma_hora_inexistente_de_dst_e_recusada_com_frase_a_vista`
- `nenhuma_vista_do_calendario_consulta_por_si`
- `alterar_um_evento_pelo_browser_persiste_sem_mexer_na_autoridade`
- `cancelar_pelo_browser_transita_sem_apagar`
- `um_evento_de_dia_inteiro_esconde_o_intervalo_meio_aberto`
- `uma_hora_que_nao_existe_e_explicada_a_pessoa`
- `um_prazo_de_tarefa_aparece_no_calendario_sem_duplicar`
- `um_lembrete_percorre_o_worker_ate_ao_sino`
- `uma_notificacao_antiga_nao_contorna_a_autorizacao_actual`
- `perder_o_acesso_esconde_o_evento_tambem_pelo_identificador`
- `nem_o_administrador_ve_a_agenda_pessoal_alheia`
- `uma_agenda_que_falha_nao_diz_que_esta_vazia`
- `uma_abertura_a_frio_encontra_o_arranque_primeiro`
- `sem_sessao_o_arranque_entrega_ao_login`
- `com_sessao_o_arranque_entrega_ao_workspace`
- `um_destino_profundo_sobrevive_ao_arranque`
- `um_marcador_forjado_nao_autentica_ninguem`
- `com_marcador_e_sessao_o_workspace_abre`
- `a_navegacao_interna_nao_repete_o_arranque`
- `nenhum_destino_de_regresso_sai_do_ocinye`
- `os_modulos_no_arranque_vem_do_core`
- `o_arranque_e_utilizavel_por_teclado`
- `o_arranque_cabe_num_ecra_pequeno`
- `um_core_bloqueado_nao_mostra_o_login`
- `um_core_sem_resposta_nao_se_confunde_com_bloqueado`
- `um_core_degradado_deixa_seguir_e_diz_o_que_falta`
- `um_arranque_bloqueado_nao_grava_o_marcador`
- `um_marcador_forjado_nao_contorna_o_bloqueio`
- `o_arranque_nunca_e_guardado`
- `uma_sessao_a_meio_nao_e_libertada_pelo_arranque`
- `o_marcador_de_arranque_nao_vale_por_sessao`
- `voltar_atras_depois_do_arranque_nao_prende_a_pessoa`
- `a_topbar_acompanha_o_core_ao_longo_da_sessao`
- `um_core_que_recupera_deixa_passar_quem_estava_preso`
- `uma_pessoa_valida_bibliografia_e_o_wasm_corre_por_baixo`
- `uma_bibliografia_partida_e_explicada_a_pessoa`
- `bibtex_hostil_aparece_como_texto`
- `sem_ambiente_a_ferramenta_diz_que_nao_ha_onde_trabalhar`
- `o_calendario_da_barra_nao_le_a_agenda`
- `o_calendario_da_barra_abre_e_fecha`
- `uma_pessoa_marca_uma_actividade_e_ela_aparece_onde_devia`
- `uma_pessoa_marca_com_participantes`
- `duplo_clique_num_dia_abre_uma_actividade_nesse_dia`
- `o_lancador_abre_da_barra_e_fecha_com_escape`
- `lancar_uma_aplicacao_navega_para_a_rota`
- `a_pesquisa_do_lancador_filtra`
- `o_filtro_de_categoria_mostra_so_a_categoria`
- `fixar_persiste_e_desafixar_nao_desinstala`
- `a_descoberta_respeita_a_autorizacao`
- `a_home_abre_com_saudacao_e_indicadores`
- `o_nye_responde_no_circulo`
- `os_meus_recursos_mostram_o_armazenamento_pessoal`
- `trocar_de_idioma_muda_a_interface_e_volta_ao_canonico`
- `uma_instancia_nova_abre_com_o_seu_nome_e_as_suas_aplicacoes`
- `desactivar_uma_aplicacao_esconde_a_e_reactivar_devolve_a_intacta`
- `sem_fornecedor_o_prompt_responde_com_o_estado_degradado`
- `o_ano_inteiro_nao_e_um_pedido_impossivel`
- `uma_pessoa_liga_a_sua_caixa_de_correio`
- `uma_pessoa_sem_caixa_liga_a_sua_do_estado_vazio`
- `um_compromisso_a_meia_noite_e_meia_aparece_no_dia_de_quem_olha`
- `uma_pessoa_comeca_uma_conversa_e_envia_a_primeira_mensagem`
- `uma_pessoa_cria_um_grupo_com_duas_pessoas`
- `o_sino_abre_um_painel_com_o_que_chegou`
- `a_pessoa_arruma_o_correio_e_nao_o_parte`
- `o_compositor_obedece_e_guarda_o_que_se_escreveu`
- `o_destinatario_por_confirmar_conta_no_envio`
- `o_correio_rola_por_dentro_e_nao_por_fora`
- `a_cadeia_cientifica_percorre_se_do_resultado_ate_a_origem`
- `uma_pessoa_constroi_a_cadeia_cientifica_pelo_workspace`
- `uma_pessoa_organiza_e_percorre_os_ficheiros_no_browser`
- `um_estranho_com_o_identificador_nao_alcanca_o_ficheiro_no_browser`
- `uma_pessoa_larga_um_ficheiro_e_ele_fica`
- `uma_imagem_institucional_carrega_na_origem_do_workspace`
- `uma_frase_do_corpo_de_um_pdf_encontra_se_pelo_workspace`
- `um_formato_sem_leitor_diz_que_o_ficheiro_esta_guardado`
- `a_previsualizacao_mostra_o_mesmo_texto_que_a_pesquisa_encontra`
- `uma_parafrase_encontra_o_documento_pelo_workspace`
- `a_pesquisa_semantica_indisponivel_nao_e_um_erro`
- `uma_citacao_continua_a_abrir_a_versao_que_citou`
- `quem_pertence_a_um_ambiente_alcanca_conhecimento_pela_navegacao`
- `ver_a_entrada_de_ficheiros_nao_da_acesso_a_ficheiro_nenhum`
- `uma_conta_de_investigacao_sem_pertencas_ve_os_modulos_de_investigacao`
- `um_colaborador_externo_nao_ganha_os_modulos_de_investigacao`
- `uma_unidade_nasce_governavel_e_a_pertenca_concede_se_pelo_produto`
- `quem_nao_gere_a_unidade_nao_recebe_os_controlos_nem_a_operacao`
- `a_vista_agregada_de_ficheiros_atravessa_ambientes_e_conta_o_que_mostra`
- `uma_conta_suspensa_perde_autoridade_a_meio_da_sessao`
- `conceder_e_revogar_uma_pertenca_veem_se_na_mesma_sessao`
- `quem_lidera_um_ambiente_gere_quem_participa_pelo_produto`
- `quem_nao_lidera_o_ambiente_nao_recebe_os_controlos_nem_a_operacao`
- `nenhum_ecra_da_navegacao_esta_morto`
- `um_carregamento_em_partes_retoma_se_noutro_contexto`
- `o_conteudo_estavel_nunca_devolve_a_pagina_anterior`
- `o_fuso_declarado_vale_para_a_viagem_e_nao_escapa_dela`
- `a_faixa_privilegiada_atravessa_a_navegacao_e_o_recarregar`
- `dar_acesso_a_quem_ja_existe_nao_cria_uma_segunda_pessoa`
- `o_detalhe_do_membro_marca_a_seccao_e_atribui_uma_unidade`
- `o_enrolamento_do_segundo_factor_prova_se_de_ponta_a_ponta`
- `o_desafio_de_mfa_precede_a_autoridade_privilegiada`
- `um_codigo_de_recuperacao_entra_uma_vez_e_nao_a_segunda`
- `revogar_uma_sessao_de_membro_pelo_produto`
- `uma_pessoa_cria_uma_ideia_e_nasce_o_workspace`
- `criar_uma_ideia_aparece_na_actividade`
- `idea_to_project_e2e`
- `clicar_numa_ideia_na_lista_leva_ao_ambiente`
- `o_separador_de_navegacao_activo_e_azul_branco_sem_dourado`
- `task_lifecycle_e2e`
- `uma_pessoa_cria_um_dataset_no_seu_ambiente`
- `dataset_detail_e2e`
- `agent_detail_e2e`
- `uma_pessoa_cria_uma_referencia_no_seu_ambiente`
- `o_criar_global_abre_cada_criacao_deterministica`
- `o_primeiro_acesso_troca_a_temporaria_pela_definitiva`
- `uma_pessoa_escreve_uma_nota_e_ela_fica`
- `uma_gravacao_obsoleta_nao_sobrepoe_nem_perde_o_texto`
- `uma_imagem_largada_numa_nota_carrega_e_fica`
- `uma_nota_encontra_se_pela_pesquisa`
- `uma_nota_ganha_etiquetas_e_filtra_se_por_elas`
- `uma_nota_arruma_se_numa_pasta_pelo_editor`
- `uma_nota_partilhada_le_se_e_so_se_le`
- `uma_versao_antiga_de_uma_nota_restaura_se`
- `uma_nota_apagada_vai_ao_lixo_e_restaura_se`
- `o_terminal_executa_pelo_core_e_desenha_so_texto`

### `apps/workspace/tests/ui_contract.rs` — 5

- `nenhum_ecra_depende_de_um_atributo_style_que_a_csp_descarta`
- `a_csp_do_workspace_nao_admite_estilos_inline`
- `cada_avatar_do_catalogo_tem_ficheiro`
- `tudo_o_que_o_arranque_chama_existe`
- `nenhum_ecra_pede_um_nome_de_utilizador`

### `apps/workspace/tests/experience_boundary.rs` — 7

- `os_modulos_de_teste_sao_o_ultimo_item`
- `nenhum_ecra_escreve_o_caminho_de_um_endpoint_do_core`
- `nenhum_ecra_constroi_o_seu_proprio_cliente`
- `nenhum_ecra_importa_um_avaliador_de_politicas`
- `a_prontidao_nao_e_inferida_de_um_pedido_de_dominio`
- `conteudo_do_dominio_nunca_vira_marcacao`
- `nenhuma_vista_decide_um_dia_civil_em_greenwich`

### `apps/workspace/tests/installed_instance.rs` — 2

- `uma_instancia_instalada_abre_entra_e_trabalha`
- `uma_instancia_restaurada_reconhece_quem_la_estava`

## Lacunas funcionais que continuam abertas

Não são visuais: são contratos do Core ou do BFF que o desenho pedia e que não
existem.

| ID | Ecrã | Comportamento pedido | Contrato em falta | Forma proposta |
| ID | Ecrã | Comportamento | Contrato | Estado |
| G-01 | Casca › Bloquear ecrã | bloquear a sessão sem a terminar; desbloquear com palavra-passe; ⌘L | estado «sessão bloqueada» no BFF | `POST /session/lock`, `POST /session/unlock {password}`; o `session.rs` recusa rotas enquanto bloqueada, excepto `/lock` |
| G-02 | Desktop | widgets persistentes por membro: presença, ordem, tamanho, minimizado, fundo, densidade | `Desktop` e `Widget Registry` no Core | `GET/PUT /me/desktop` → `{version, base_default_version, wallpaper, density, widgets:[{id, kind, w, h, minimized}]}`; `WidgetKind` tipado em `ocinye-contracts` com tamanhos permitidos e se é obrigatório |
| G-03 | Desktop › indicadores Research | contagens de unidades activas, ideias em investigação, projectos em execução, datasets | resumo tipado | `GET /me/summary?profile=research` → `{units_active, ideas_investigating, projects_running, datasets}`; falha do Core nunca devolve 0 (contrato §5) |
| G-04 | Desktop › predefinição | admin compõe, pré-visualiza e publica (só novos / disponível / actualizar quem está na versão antiga / forçar todos); membro repõe com anular | modelo de predefinição versionada por perfil e instância | `GET/PUT /admin/desktop-default` (rascunho), `POST /admin/desktop-default/publish {mode}`, `POST /me/desktop/restore` |
| G-05 | Gestor de janelas | abrir apps em janelas internas, focar, mover, redimensionar, minimizar, maximizar, alternar, «todas as janelas» | estado de janelas; relação com rotas reais (§21) | estado **no cliente** (`app.js`) com a URL da janela activa em `history`; cada janela carrega a rota real num `<iframe>` same-origin (`frame-src 'self'` já é permitido). Decidir antes: iframe vs. fragmento SSR |
| G-06 | Lançador › Recentes | últimas apps abertas pelo membro | histórico de aberturas | acrescentar `recent_apps` a `GET /me` ou derivar de `activity` com `owner` |
| G-07 | Nye › voz | push-to-talk, estados ouvir / transcrever / agir / falar / falha | contrato de voz | `POST /ask/voice` (áudio → transcrição) ou Web Speech só no cliente, com consentimento explícito; sem escuta permanente |
| G-08 | Monitor de Actividade | processos/serviços com CPU, memória, disco, rede, GPU; terminar processo não-sistema | métricas de serviços e nós | `GET /admin/monitor?metric=cpu|mem|disk|net|gpu` → `{services:[{id, name, kind, owner, protected, value, value2}], summary}`; `POST /admin/monitor/{id}/stop` só para serviços não protegidos, autorizado no Core |
| G-09 | Casca › estado CORE · IA | resumo curto do Core e do fornecedor de IA activo | agregado tipado | `GET /me/status` → `{core:{ok, version, services}, ai:{mode:none|local|external, model, node}}` |
| G-26 | Recuperar palavra-passe (D10) | pedir instruções; resposta sempre neutra | `GET /password/recover` → `recover(false, disponivel, …)`; `POST /password/recover {email}` → 202 sempre → `recover(true, …)` | vista pronta; `disponivel=false` até existir o POST |
| G-27 | Sessão expirada / acesso revogado (D12, D13) | cartão por motivo | `/login?reason=expired` (cookie de sessão desconhecido) · `/login?reason=revoked` (motivo do Core) → `fim_de_sessao(…)` | vista pronta |
| G-30 | Idioma antes da sessão (D7) | pt · en · fr no rodapé do cartão | `POST /login/language {lang, return_to}` grava `oc_locale` e redirige | **CONNECT** |
| G-31 | Perfil e endereço à porta (D7, P1–P4) | etiqueta do perfil + anfitrião | `GET /api/v1/instance/branding` → `profile`; anfitrião do pedido → `Porta { nome, perfil, host }` → `login_na_porta(…)` | **CONNECT** |

## Como voltar

O código do Claude Design entra por pacote, sobre este ramo. Cada ecrã novo
substitui o `interface_pending` da sua rota, traz os seus testes de browser, e
reabre o portão correspondente (`test-enumeration.sh`,
`architecture-gates.sh`, os scripts de e2e).
