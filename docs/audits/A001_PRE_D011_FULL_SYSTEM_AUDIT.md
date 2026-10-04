# A001 — Auditoria e correcção do sistema inteiro antes da D011

**Data:** 2026-10-04. **Base:** `main @ 32ba19a` (D010 fundida, PR #193).
**Ramo:** `audit/pre-d011-full-system`. **Âmbito:** D001 a D010 — Core,
core-server, Workspace, migrações, contratos, documentação.
**Modo:** auditoria com correcção autónoma dos defeitos verificados; sem
funcionalidades novas, sem D011, sem deploy.

**DEPLOY = NOT_PERFORMED · DEPLOY_AUTHORIZATION = NOT_GIVEN · PRODUCTION_INSTALL = NOT_PERFORMED.**

## 1. Método

1. **A verdade do repositório primeiro:** contagens re-derivadas da árvore
   (`scripts/repository-facts.sh`), registo de aplicações, migrações e ADRs lidos
   dos ficheiros — nenhum número herdado de relatórios anteriores.
2. **Descoberta em paralelo, por camada:** seis auditores só de leitura, cada um
   com um âmbito (D010 multi-Distribuição e pontos de acesso; segurança HTTP do
   Workspace; autorização e isolamento do Core; Terminal, Browser e Nye; Gestor
   de Janelas, i18n, acessibilidade e documentação; esquema, migrações e
   pânicos). Cada um devolveu candidatos com `ficheiro:linha` e uma ideia de
   reprodução — **candidatos, não factos**.
3. **Cada candidato verificado no código** antes de qualquer alteração; os que
   se confirmaram corrigiram-se **na causa**, com prova de regressão; os que não
   se confirmaram ficaram classificados.
4. **Prova de regressão por reversão** para os críticos: a guarda retirada, o
   teste certo a falhar com a assinatura esperada, a guarda reposta (§5).
5. **Certificação final** sobre a árvore exacta do commit candidato:
   `./scripts/verify.sh` numa worktree limpa.

## 2. Estado de partida, re-derivado

| Facto | Valor (árvore em `32ba19a`) |
|---|---|
| Aplicações registadas | 28 (`MANIFESTS`), ids e rotas únicas |
| Migrações | 63 (→ 64 com a desta auditoria) |
| Tabelas | 96 |
| ADRs | 107 |
| Permissões | 76 |
| Funções de teste | 1824 (721 com base de dados) |
| Caminhos / operações do Core | 254 / 302 |

## 3. Achados

Severidade pelo modelo da instrução (CRITICAL · HIGH · MEDIUM · LOW), sem
inflação. **Nenhum CRITICAL** se confirmou: as duas fugas entre membros
(H003, H004) exigem conhecer um identificador opaco ou deter já um papel de
administração.

### HIGH — todos corrigidos

| Id | Defeito | Correcção | Prova |
|---|---|---|---|
| A001-H001 | Nenhum cliente dizia ao servidor que uma janela tinha trabalho por gravar: mudar de Distribuição (S16) fechava-a **em silêncio**; e o diálogo do D002, com JavaScript, perdia a continuação `after=switch:…` | `oc-apps.js` reporta o estado sujo (`POST /wm/{id}/state`); `wm-engine.js` deixa o formulário com `after` seguir sem interceptar | `f_mudar_…` (D010) + browser |
| A001-H002 | Uma sonda `/ready` perdida tornava invisíveis as aplicações com módulo, e `view()` **apagava** essas janelas (com trabalho por gravar) | Sem Core operacional, as janelas escondem-se e voltam; só se fecham de vez quando o Core diz que deixaram de ser do membro | revisão + browser |
| A001-H003 | `POST /mail/send` com `draft_id` lia os anexos de **qualquer** rascunho (IDOR) | `attachments_for_send` resolve o rascunho pela mesma regra de posse em SQL, e exige a caixa de onde se envia | `h003_…` (reversão: PASS) |
| A001-H004 | Um `OrganisationAdmin` repunha a palavra-passe (recebendo a credencial), suspendia e terminava sessões de um `PlatformAdmin` | `ensure_actor_may_govern`: agir sobre quem detém `PlatformAdmin` exige `platform.administer` — reposição, estado, sessão, eliminação | `h004_…` (reversão: PASS) |
| A001-H005 | O ecrã de bloqueio (S22) só era imposto por uma macro: Notas, Ficheiros, Terminal, janelas e uploads respondiam com a sessão bloqueada | `lock_gate`, uma camada do router: bloqueado, só `/lock`, `/unlock`, `/logout`, entrada e estáticos; `GET` → `/lock`, resto `423` | `h005_…` (reversão: PASS) |
| A001-H006 | `/avatar/me/{version}` colava o segmento descodificado num pedido ao Core: `..%2F` alcançava qualquer `GET` do Core com o token do membro | A versão é um segmento `[A-Za-z0-9_-]{1,64}`; os outros identificadores encaminhados ao Core (rascunhos, anexos, conversas, caixas) passaram a `Uuid` | `h006_…` (reversão: PASS) |
| A001-H007 | Suspender, desactivar, tirar o papel ou apagar o único administrador que entra numa Distribuição deixava a Instância sem administração pelo Workspace; as duas guardas tomavam trancas diferentes | As operações de conta e de papel verificam a via de administração quando mexem num administrador que entra; as duas guardas partilham a tranca | `h007_…` (reversão: PASS) |
| A001-H008 | O plano agentic executava capacidades de aplicações **desactivadas** na Instância (`mail.send` com o Correio desligado) | O executor recusa (`CapabilityUnavailable`) pelo mesmo mapa de prefixos que a porta HTTP | `h008_…` |

### MEDIUM

| Id | Defeito | Estado |
|---|---|---|
| A001-M001 | `local_path` deixava passar TAB/CR/LF: `/\t/mal` era `//mal` no browser (open redirect) | FIXED · `m001_…` (reversão: PASS) |
| A001-M002 | Perder a Distribuição activa com duas ou mais restantes prendia o membro na recusa | FIXED · `m002_…` |
| A001-M003 | `POST /distribution` fechava as janelas sem o diálogo do D002 (e voltar à mesma também) | FIXED — uma activa viva leva à mudança S16; a mesma não muda nada · `m003_…` |
| A001-M004 | Um grant explícito de uma permissão exclusiva da plataforma escapava à exigência de MFA | FIXED — `is_platform_privileged`, derivado da política de papéis · `m004_…` |
| A001-M005 | As listas de hipóteses, metodologias e estudos ignoravam a classificação de cada artefacto | FIXED (o mesmo que RES-07) |
| A001-M006 | Comentários aceitavam qualquer assunto, de qualquer ambiente, e herdavam só a classificação do ambiente | FIXED — o assunto resolve-se e a classificação é a mais restritiva |
| A001-M007 | A guarda da última liderança de um ambiente corria sem tranca, contava pertenças revogadas, e despromover o último passava | FIXED — serializada na linha do ambiente · `m007_…` |
| A001-M008 | O analisador Markdown das notas era quadrático: uma linha de 2 MB prendia um trabalhador | FIXED — linear · `uma_linha_hostil_…` |
| A001-M009 | Argon2id corre nos trabalhadores do runtime | DEFERRED_WITH_REASON — mudar a API de hashing em 15 chamadas; o login já tem limite por IP e por endereço |
| A001-M010 | O avatar (até 64 MP, Lanczos3) descodificava-se num trabalhador do runtime | FIXED — `spawn_blocking` |
| A001-M011 | Várias janelas no mesmo documento repetem `id`s (`oc-res-title`, `oc-notes-title`) | DESIGN_DECISION_REQUIRED — a marcação é do Design |
| A001-M012 | O desbloqueio falhado mostrava a chave crua `auth.login.refused`; nenhum portão via chaves usadas inexistentes | FIXED — chave Code `lock.failed` e o portão `cada_chave_literal_usada_existe` (reversão: PASS) |
| A001-M013 | Documentação desactualizada: §1 (D010 «no ramo»; «nem todas as aplicações têm ecrã»), `docs/ui/README`, `README`, catálogo de ADRs sem 8 entradas, ADR-0014 sem emenda | FIXED |
| A001-M014 | A redacção do ocsh deixava passar valores entre aspas e nomes em maiúsculas ou compostos | FIXED · `o_segredo_nao_escapa_…` |
| A001-M015 | O limite de 5 tentativas de desbloqueio era corrível em paralelo | FIXED — contado antes do Core, sob a tranca do registo |
| A001-M016 | Fixar o ponto canónico numa Distribuição (e desactivá-la) deixava a administração sem entrada | FIXED — o canónico é genérico (ADR-0020 §10) · `m016_…` (reversão: PASS) |
| A001-M017 | Mensagens: voltar a um grupo devolvia o papel antigo; um administrador retirava o dono; o registo não dizia quem | FIXED; o último que governa poder sair → DESIGN_DECISION_REQUIRED |
| A001-M018 | Uma corrida perdida contra uma restrição única respondia 500 | FIXED — 409, sem nomear a restrição · `m018_…` |
| A001-M019 | A lista de relações de um ambiente mostrava ligações cujas pontas o leitor não alcança | FIXED — as duas pontas resolvem-se por quem lê |
| A001-M020 | `wasmtime`/`wasmtime-wasi` 48.0.3 com sete avisos publicados a 2026-10-02 (RUSTSEC-2026-0321 a 0327, um crítico); o portão de dependências do `verify.sh` falhou na corrida desta auditoria | FIXED — 48.0.5, só no `Cargo.lock`, mesma versão maior. Severidade MEDIUM e não CRITICAL: o crítico (callbacks async de componentes), o wasip3 e o GC não são usados pelo runtime `wasm32-wasip1`, e o convidado é uma capacidade verificada por soma |

### LOW

| Id | Defeito | Estado |
|---|---|---|
| A001-L001 | `GET /api/v1/access/resolve` é público e confirma nomes configurados | ACCEPTED_CURRENT_BEHAVIOR — o Workspace precisa dele antes da sessão; não revela mais que o DNS |
| A001-L002 | `observe` sonda o anfitrião configurado a partir do Core | ACCEPTED_CURRENT_BEHAVIOR — só `organisation.manage`, resultado de três estados |
| A001-L003 | O trigger «uma activada» tem *write skew* para escritas fora do produto | ACCEPTED_CURRENT_BEHAVIOR — o caminho do produto serializa (`FOR UPDATE`) |
| A001-L004 | O cookie de sessão não usa o prefixo `__Host-` | DEFERRED_WITH_REASON — mudar o nome termina todas as sessões vivas e exige `Secure` (o desenvolvimento local é HTTP); a fazer com o próximo deploy |
| A001-L005 | Abrir uma aplicação por `GET` abre uma janela | ACCEPTED_CURRENT_BEHAVIOR — navegação, limitada a 24 janelas |
| A001-L006 | O `CHECK` do par de contexto da 0063 passava com o tipo nulo | FIXED — migração 0064 · `l006_…` |
| A001-L007 | Escolher contexto podia sobreviver a uma mudança de Distribuição concorrente; repor não era auditado | FIXED |
| A001-L008 | Uma enxurrada de `Host` inventados esvaziava a cache de resolução | FIXED |
| A001-L009 | Lembretes de tarefa não autorizavam a tarefa (oráculo 200/500) | FIXED |
| A001-L010 | Um leitor de uma caixa partilhada muda o lido/estrela no fornecedor | DESIGN_DECISION_REQUIRED |
| A001-L011 | Repetir «fixar», «desactivar» ou «escolher contexto» re-auditava (e terminava sessões) | FIXED |
| A001-L012 | Dar acesso / fixar um ponto podia correr contra uma desactivação | FIXED — `FOR SHARE` |
| A001-L013 | Leituras de nomes sem filtro de organização | FIXED |
| A001-L014 | A isenção de revalidação era por sufixo do caminho | FIXED — caminhos exactos |
| A001-L015 | O atalho `?` do ocsh saltava o tecto da linha e os caracteres de controlo | FIXED |
| A001-L016 | Entrar de novo não terminava a sessão que o browser trazia | FIXED |
| A001-L017 | «Voltar» (notificações) só reconhecia o endereço público | FIXED |
| A001-L018 | Abrir uma janela pelo motor perdia a pergunta do endereço | FIXED |
| A001-L019 | A actualização do correio reencaminhava duas vezes e perdia a pasta | FIXED; mostrar o resultado → pedido ao Design |
| A001-L020 | Uma mudança de ponto/Distribuição na Administração servia-se da cache até 5 s no mesmo processo | FIXED |
| A001-L021 | A lista de tipos da auditoria lia todas as organizações | FIXED |
| A001-L022 | Um ficheiro pessoal pedido como institucional respondia 500 | FIXED |
| A001-L023 | Largar uma sessão local deixava a do Core viva | FIXED |
| A001-L024 | `forbid(unsafe_code)` em falta em três raízes de crate | FIXED (0 blocos `unsafe` em produção) |
| A001-L025 | O executor não audita recusas de validação/aprovação; o Terminal não cria plano para «aprovação exigida» | DEFERRED_WITH_REASON — nenhum comando v1 exige aprovação |
| A001-L026 | `may_invoke` não confere o âmbito declarado da capacidade | DEFERRED_WITH_REASON — defesa em profundidade; os serviços re-autorizam |
| A001-L027 | O Browser classifica e enquadra um endereço por duas análises diferentes | DEFERRED_WITH_REASON — não explorável (moldura vazia) |
| A001-L028 | Uma quebra de linha no ocsh sai 2 (controlo) e não 126 | ACCEPTED_CURRENT_BEHAVIOR — recusada antes de qualquer análise |
| A001-L029 | Erros do avatar em português fixo e nunca mostrados | DESIGN_DECISION_REQUIRED |
| A001-L030 | O acesso dado pelo trigger de membro novo não é auditado | ACCEPTED_CURRENT_BEHAVIOR — documentado na D010 |
| A001-L031 | O espelho `organisations.profile` desempata por ordem alfabética | ACCEPTED_CURRENT_BEHAVIOR — espelho de uma versão |
| A001-L032 | `access_endpoints` aceita `''` e «um canónico» é «no máximo um» na base | DEFERRED_WITH_REASON — o Core valida e semeia exactamente um |
| A001-L033 | `ai_providers.secret_id` sem chave composta com a organização | DEFERRED_WITH_REASON — o Core verifica; migração a planear |
| A001-L034 | «1 janelas abertas vão fechar» | DESIGN_DECISION_REQUIRED (chave plural) |
| A001-L035 | ADR-0312 continua `Proposed` com o Terminal em `main` | DESIGN_DECISION_REQUIRED (aceitação é decisão humana) |
| A001-L036 | Retirar o acesso a todas as Distribuições não restringe a API | ACCEPTED_CURRENT_BEHAVIOR — Distribuição não é autoridade (ADR-0019) |
| A001-L037 | A confirmação de um plano mostra o resumo do modelo | ACCEPTED_CURRENT_BEHAVIOR — mostra também a capacidade, o risco e os campos ligados ao digest |

### NOT_A_BUG

| Id | Candidato | Porquê |
|---|---|---|
| A001-N001 | As duas guardas de administração contariam «administrador» de forma diferente | As duas usam `invited` ou `active` |

## 4. Totais

| | Encontrados | Corrigidos | Outros |
|---|---|---|---|
| CRITICAL | 0 | 0 | — |
| HIGH | 8 | 8 | — |
| MEDIUM | 20 | 18 | 1 DEFERRED · 1 DESIGN |
| LOW | 37 | 18 | 9 ACCEPTED · 6 DEFERRED · 4 DESIGN |
| NOT_A_BUG | 1 | — | — |

**Total: 66.** Corrigidos 44 · ACCEPTED_CURRENT_BEHAVIOR 9 · DEFERRED_WITH_REASON 7 ·
DESIGN_DECISION_REQUIRED 5 · NOT_A_BUG 1.

## 5. Provas

- **Testes novos:** `crates/ocinye-core/tests/a001_regressions.rs`,
  `crates/ocinye-core/tests/d010_upgrade.rs` (0001–0059 → corrente, sem perda nem
  duplicação), `apps/workspace/tests/a001_journeys.rs`, o portão de i18n
  `cada_chave_literal_usada_existe`, e os testes do Markdown e do ocsh.
- **Reversões (13, todas `PASS`).** Em cada uma, a guarda foi retirada, o
  teste certo correu e falhou com a assinatura esperada, e o ficheiro foi
  reposto (confirmado depois, ficheiro a ficheiro):

  | Id | Injecção | Teste | Assinatura |
  |---|---|---|---|
  | H003 | sem `accessible_draft` | `h003` | «o rascunho de outra pessoa foi alcançado» |
  | H004 | `ensure_actor_may_govern` desligada | `h004` | «um OrganisationAdmin recebeu a credencial» |
  | H005 | `lock_gate` nunca vê o bloqueio | `h005` | «as Notas abriram com o ecrã bloqueado» |
  | H006 | sem validação da versão | `h006` | «chegou ao Core» |
  | H007 | sem `ensure_an_administrator_can_enter` ao suspender | `h007` | «a Instância ficou sem administrador que entre» |
  | H008 | sem a porta da aplicação no executor | `a_capability_of_an_inactive_application_does_not_run` | «o Correio desactivado enviou pelo plano agentic» |
  | M001 | sem a recusa de caracteres de controlo | `m001` | o regresso com `evil` foi seguido (fraca: o texto está sempre na mensagem; a prova é a falha) |
  | M007 | sem a guarda da última liderança | `m007` | «o ambiente ficou sem liderança» |
  | M012 | `t("auth.login.refused")` de volta | `cada_chave_literal_usada_existe` | a chave em falta nomeada |
  | M014 | `if !quoted` de volta | `o_segredo_nao_escapa_por_aspas_nem_por_grafia` | o segredo no eco |
  | M016 | sem a recusa do canónico | `m016` | «o canónico foi fixado» |
  | M018 | sem o mapeamento 23505 → `Conflict` | `m018` | `left: InternalError, right: Conflict` |
  | L006 | a restrição da 0063 reposta na base | `l006` | «uma sessão guardou um contexto órfão» |

  A primeira injecção do H006 estava errada (`false && a || b` só desligava o
  primeiro termo) e o teste passou: foi registada como inválida, refeita a
  desligar a condição inteira, e falhou como devia.
- **Browser real:** a fatia D010 foi certificada na D010; esta auditoria não
  alterou ecrãs (só JavaScript de integração: estado sujo, `after`, pergunta do
  endereço) — provado pelos percursos HTTP e pelo
  código. **`BROWSER_REAL_PASS = NOT_RUN`** para essas três mudanças de
  JavaScript: ficam por observar num browser real.
- **`SCREEN_READER_TEST = NOT_RUN`.**

## 6. O que fica para decidir (DESIGN_DECISION_REQUIRED)

1. Ids únicos por janela quando várias aplicações partilham o documento (M011).
2. Quem governa um grupo quando o último dono ou administrador sai (M017).
3. Se um leitor de uma caixa partilhada marca lido/estrela no fornecedor (L010).
4. Onde mostrar o resultado da actualização do correio e os erros do avatar (L019, L029).
5. A forma plural de «janelas abertas» (L034) e a aceitação da ADR-0312 (L035).
