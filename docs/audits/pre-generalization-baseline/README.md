# Linha de base anterior à generalização — Parte 0

**Data:** 2026-09-26. **Âmbito:** provar o sistema actual antes de o generalizar,
sem mudar comportamento. **Documentos irmãos:**
[sistema actual](../../architecture/CURRENT_SYSTEM.md) e
[arquitectura-alvo](../../architecture/TARGET_OCINYE_OS.md).

Uma linha de base desconhecida não serve de fundação. Esta regista o que estava
verde, o que estava vermelho sem que ninguém o visse, e o que foi reparado para
que a próxima parte comece de uma árvore provada.

## 1. Identificação

| | Valor | Como se obteve |
|---|---|---|
| `BASELINE_SHA` | `7c8441b` | `origin/main` no início da Parte 0 |
| `PRODUCTION_SHA` | `b4c95b6cdfe8` | `/etc/ocinye/release.env` e etiquetas das imagens em execução, lidos no host |
| `DB_SCHEMA_VERSION` (árvore) | `0051_member_app_pins` | `migrations/` |
| `DB_SCHEMA_VERSION` (produção) | `0051`, **derivado** | o release `b4c95b6` contém as 51 migrations e o Core aplica-as no arranque; o Core está *healthy*. A leitura directa de `_sqlx_migrations` em produção **não foi feita** — a consulta à base de produção foi recusada pelo classificador de permissões, e não se contornou |
| `main` ↔ produção | só a #168 (testes) | `git log b4c95b6..7c8441b` |

## 2. Regressão exigida pela Parte 0

A regressão mínima do programa, e onde cada área é provada. As quatro marcadas
como **nova** não tinham viagem própria e foram escritas nesta parte; fixam o
comportamento **de hoje**.

| Área | Viagem de browser | Estado |
|---|---|---|
| Login | `sem_sessao_o_arranque_entrega_ao_login`, `com_sessao_o_arranque_entrega_ao_workspace`, `o_primeiro_acesso_troca_a_temporaria_pela_definitiva` e o arranque bloqueado/degradado | PASS |
| Home | `a_home_abre_com_saudacao_e_indicadores` — **nova** | PASS |
| Gestor de Aplicações | as seis do lançador e da fixação | PASS |
| Ficheiros | doze viagens, incluindo bytes reais até ao object store | PASS |
| Notas | nove viagens | PASS |
| Correio | seis viagens | PASS |
| Ideias | `uma_pessoa_cria_uma_ideia_e_nasce_o_workspace`, `clicar_numa_ideia_na_lista_leva_ao_ambiente` | PASS |
| Ideia → Projecto | `idea_to_project_e2e` | PASS |
| Tarefas | `task_lifecycle_e2e` | PASS |
| Datasets | `uma_pessoa_cria_um_dataset_no_seu_ambiente`, `dataset_detail_e2e` | PASS |
| Recursos | `os_meus_recursos_mostram_o_armazenamento_pessoal` — **nova** | PASS |
| Administração | onze viagens de membros, unidades e ambientes | PASS |
| Troca de idioma | `trocar_de_idioma_muda_a_interface_e_volta_ao_canonico` — **nova**, pt→en→fr→pt | PASS |
| Prompt degradado | `sem_fornecedor_o_prompt_responde_com_o_estado_degradado` — **nova** | PASS |

**As novas não são placebo.** A do Prompt e a da Home falharam primeiro por
razões reais antes de passarem (ver F-09 e F-10). A do idioma foi provada por
reversão: com a escrita do cookie de idioma desligada em `set_language`, falha
com `«My Resources» não apareceu`; reposto o código, passa.

**Corridas.** `./scripts/verify.sh` completo sobre `9b14311` (a árvore da
`main` com F-05 corrigido), contra uma base PostgreSQL criada de vazio e o
fixture S3 local:

| Portão | Resultado |
|---|---|
| Integridade do sistema de verificação | PASS — 7 propriedades |
| Fronteiras, dependências, ligações, esquema, autoridade de escrita, configuração | PASS |
| `cargo fmt`, `clippy -D warnings`, capacidades WASM | PASS |
| Testes (todas as suites) | PASS — 0 falhas; browser **119 passed, 0 failed, 4 ignored** (as capturas) |
| Capacidades, builds de release, isolamento do fornecedor de teste, catálogo, paridade, matriz, ADRs, alvos do Compose, autoria, factos da documentação | PASS |
| Contrato da Secção 1 | **FAIL** — F-07 |

A corrida final, depois das correcções desta parte, está em
[§5](#5-corrida-final).

**Capturas.** `./scripts/capturas.sh` produz as capturas para revisão humana
(Calendário, painéis da barra, Correio, cadeia científica) — 55 nesta corrida. Ficam fora do
repositório, por serem imagens de uma execução e não documentação.

## 3. Achados

Severidade pela escala do programa: **P0** bloqueia tudo, **P1** bloqueia a parte
dependente, **P2** mina o contrato da parte, **P3** menor. `INVALID` não é um
achado do código.

| ID | Sev | Achado | Estado |
|---|---|---|---|
| F-01 | **P1** | **O backup nocturno de produção falha.** O `ocinye-backup.timer` está instalado e dispara; a imagem de backup reconstrói-se a cada release e descarregava o `mc` de `dl.min.io`, que responde `410`. Última execução observada: 2026-09-25 03:00, falhada. `CLAUDE.md` §1 e `docs/backups` diziam que o agendador não estava instalado. | Corrigido no código (o `mc` vem do espelho); **fecha com a primeira execução agendada verde depois do deploy** |
| F-02 | **P1** | **O MinIO foi retirado pelo fabricante.** Imagens `quay.io/minio/*` e `minio/*` já não se descarregam, nem por digest; `dl.min.io` responde `410`. A CI não levanta o fixture S3, e uma instalação nova é impossível. Produção funciona só por cache. | Corrigido por espelho `LEGACY_COMPATIBILITY_DEPENDENCY` ([registo](../../deployment/third-party-artifacts.md)); substituição do store é item obrigatório antes da release geral |
| F-03 | **P1** | **A CI da `main` está vermelha desde 2026-09-24 — treze pushes seguidos — e os testes não correram em nenhum.** Primeiro o fixture S3 (F-02), depois também a integridade (F-05). As PRs #158–#168 entraram com `gh pr merge --admin` e foram para produção sem uma execução de testes na CI. | Fecha com a CI desta PR verde **sem** merge forçado |
| F-04 | **P1** | **A *branch protection* da `main` já não tem *required checks* e tem `enforce_admins: false`**, enquanto `CLAUDE.md` §1 e §73 dizem cinco *required checks* e `enforce_admins` activo. | §1 corrigido para dizer a verdade. **Repor a protecção é decisão humana** (§73: alterar *branch protection* exige autorização explícita) |
| F-05 | P2 | `design_fidelity` falha em duas propriedades desde a #164: o ícone `oc-apps` no sprite e não no dossier, e um anel de foco escrito à mão em `.oc-apps__card`. | Corrigido — `9b14311`; o render é idêntico (os tokens resolvem para os mesmos 2px dourados com 2px de afastamento) |
| F-06 | P2 | **O rollback não atravessa migrações.** O sqlx recusa arrancar um Core mais antigo contra uma base com migração desconhecida (`VersionMissing`); o runbook dizia que era seguro com migrações aditivas. O rollback também não repõe as confs do nginx do release anterior. | Runbook corrigido para dizer a verdade; a solução é da Parte 10 |
| F-07 | P2 | `CLAUDE.md` §1 desactualizado: contagens, backup, *branch protection*. | Corrigido nesta parte |
| F-08 | P2 | **O Model Router lê todo o inventário `ai_models`, sem filtrar por organização.** `compute_nodes` tem `organisation_id`; a consulta dos candidatos não o usa. Com uma organização por base (hoje) é inofensivo; com duas, o nó de uma serve os pedidos da outra. | **Portão da Parte 1** (isolamento entre instâncias) |
| F-09 | P2 | O Prompt mostra a razão tipada só dentro de «Detalhes», e a instrução de sistema do Core fixa a organização e a língua («ao serviço do sistema operacional institucional da Ocinye… português europeu»). Há literais portugueses fora do catálogo no turno: o selo «ESTADO» e a mensagem de `AI_PERMISSION_DENIED`. | Registado para as Partes 7 e 14 |
| F-10 | P2 | A `Home` marca a localização com `aria-current="page"` na barra, e as abas de secção com `aria-current="location"`; são convenções diferentes para a mesma pergunta. | Registado; a viagem nova usa a da barra |
| F-11 | P2 | O deploy não tem portão de saúde nem backup prévio, e o host acumula 237 GB de cache de build (227 GB recuperáveis) num disco a 53%. | Registado para a Parte 10 |
| F-12 | P2 | Em produção, a etiqueta `quay.io/minio/minio:RELEASE.2025-04-22T22-12-26Z` aponta para a imagem `latest` de 2025-09-07, reetiquetada no host: o Compose dizia uma versão e corria outra. | Resolvido pela fixação por digest (F-02) |
| F-13 | P2 | O guarda de i18n não observa `ui/shell.rs` nem `routes.rs`: ~22 literais portugueses na shell e ~66 títulos de página ficam fora da tradução. O idioma vive só num cookie. | Registado para as Partes 14 e 16 |
| F-14 | P3 | A retenção local do backup (`ls -1dt ocinye-*`) conta também os ficheiros `.sha256`, e pode manter menos conjuntos do que `OCINYE_BACKUP_KEEP` diz. **Plausível, não verificado.** | Verificar na Parte 11 |
| F-15 | P3 | O cabeçalho de `infra/compose/docker-compose.yml` diz que o stack traz Core, Worker e Workspace; só traz os serviços de dados. | Registado |
| I-01 | `INVALID` | Na primeira corrida, nove viagens de upload falharam com os bytes a não chegarem ao PostgreSQL: o MinIO local reiniciou a meio (tempo de actividade reposto). Na corrida seguinte, com o fixture estável, as nove passaram. | Não é achado do código |

## 4. Reparações feitas nesta parte

Nenhuma muda comportamento de produto.

- **F-05** — dossier de ícones e anel de foco por tokens.
- **F-01, F-02** — o Compose (dev e produção), a CI e a imagem de backup passam
  a consumir o MinIO do espelho `ghcr.io/ocinye/third-party/*`, por digest. A
  imagem de backup copia o `mc` num stage próprio em vez de o descarregar. O
  índice espelhado difere do de origem (o `ppc64le` já não existe em lado
  nenhum), mas os manifestos por plataforma — os que um host corre — são
  byte a byte os de origem.
- **F-06, F-07** — runbook de rollback, tabela de deploy e `CLAUDE.md` §1 a
  dizer a verdade.
- Quatro viagens de browser novas, e o contrato de enumeração de 119 para 123.

## 5. Corrida final

Preenchida com a corrida de `./scripts/verify.sh` sobre o commit final desta
parte, e com a CI da PR.

## 6. Prontidão para a Parte 1

A Parte 1 só começa com a linha de base **provada**: `verify.sh` verde, a CI
desta PR verde sem merge forçado, e o backup agendado a correr depois do deploy.
O F-04 (repor a *branch protection*) e o F-08 (isolamento do inventário de
modelos) são pré-condições que a Parte 1 tem de resolver ou justificar.
