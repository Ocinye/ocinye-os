# Certificação do Ocinye OS de uso geral (Parte 16)

A certificação não é um teste gigante escrito à parte: é a **composição** das provas
que já correm, cada uma no sítio onde a sua fronteira vive, orquestradas por
`scripts/certify-general-os.sh`. Esta página liga cada passo da jornada do
programa à prova que o exercita. Onde a prova é parcial, diz-se.

O portão `OCINYE_GENERAL_OS_BASELINE_READY` só se declara com **todas** estas
corridas verdes no mesmo release, e com a pendência do armazenamento de objectos
resolvida (abaixo).

## A jornada

| # | Passo | Prova | Onde |
|---|---|---|---|
| 1 | Instalar o Ocinye OS | instalação de raiz num anfitrião descartável | `scripts/install-e2e.sh` |
| 2 | Criar a Instância | bootstrap com nome e perfil | idem |
| 3 | Perfil de investigação | corrida `research` | idem |
| 4 | Criar o administrador | credencial temporária entregue pelo instalador | idem |
| 5 | Entrar | palavra-passe definitiva + segundo factor pela chave manual | `installed_instance.rs` |
| 6 | Mudar de língua | `trocar_de_idioma_muda_a_interface_e_volta_ao_canonico` | viagens de browser |
| 7 | Gestor de Aplicações | `o_lancador_abre_da_barra_e_fecha_com_escape`, `a_pesquisa_do_lancador_filtra` | viagens de browser |
| 8 | Fixar aplicações | `fixar_persiste_e_desafixar_nao_desinstala`; fixações por Instância em `instance_branding_http.rs` | browser + HTTP |
| 9 | Criar um membro | `dar_acesso_a_quem_ja_existe_nao_cria_uma_segunda_pessoa`, `o_primeiro_acesso_troca_a_temporaria_pela_definitiva` | viagens de browser |
| 10 | Atribuir recursos | `os_meus_recursos_mostram_o_armazenamento_pessoal`; entitlements em `crates/ocinye-core/tests/resource.rs` | browser + Core |
| 11 | Carregar um ficheiro | `uma_pessoa_larga_um_ficheiro_e_ele_fica`; «Meus ficheiros» na Instância instalada | browser + instalação |
| 12 | Criar uma nota | `uma_pessoa_escreve_uma_nota_e_ela_fica`; nota guardada na Instância instalada | browser + instalação |
| 13 | Correio (com backend de teste) | `uma_pessoa_liga_a_sua_caixa_de_correio`, `a_pessoa_arruma_o_correio_e_nao_o_parte` | viagens de browser |
| 14 | Criar uma ideia | `uma_pessoa_cria_uma_ideia_e_nasce_o_workspace` | viagens de browser |
| 15 | Promover ideia → projecto | `idea_to_project_e2e`; `promoting_the_same_idea_twice_produces_one_project` | browser + Core |
| 16 | Criar uma tarefa | `task_lifecycle_e2e` | viagens de browser |
| 17 | Criar um dataset | `uma_pessoa_cria_um_dataset_no_seu_ambiente` | viagens de browser |
| 18 | Actividade | actividade de notas e do calendário nas viagens; **parcial**: o feed institucional de Actividade não tem viagem própria | viagens de browser |
| 19 | Fornecedor de IA de teste | fornecedor simulado registado com segredo | `ai_providers_http.rs` |
| 20 | O Prompt usa-o | `origin=MODEL`, credencial só no cabeçalho | idem |
| 21 | Segundo fornecedor | cenário C | `ai_routing_http.rs` |
| 22 | Outra capacidade, outro fornecedor | cenário D | idem |
| 23 | Desactivar fornecedores | cenário H | idem |
| 24 | Resposta determinística sem fornecedor | cenários G e H; `sem_fornecedor_o_prompt_responde_com_o_estado_degradado` | HTTP + browser |
| 25 | Registar um nó simulado | enrolamento e heartbeat | `node_fabric_http.rs` |
| 26 | A capacidade do nó aparece | `GET /compute/capacity` | idem |
| 27 | O nó cai | offline derivado; vista do operador | `node_fabric_http.rs`, `operations_http.rs` |
| 28 | O Core continua saudável | idem, e `/ready` com os componentes críticos disponíveis | idem |
| 29 | Backup | `ocinye backup` | `scripts/restore-e2e.sh` |
| 30 | Actualizar a Instância | N → N+1, dados intactos; release falhado revertido | `scripts/upgrade-e2e.sh` |
| 31 | Restaurar noutro anfitrião | `ocinye install --restore` | `scripts/restore-e2e.sh` |
| 32 | Voltar a entrar | mesma palavra-passe, mesmo segundo factor | `installed_instance.rs` (restaurada) |
| 33 | Estado intacto | nota e ficheiro presentes; verificações de continuidade | `scripts/restore-e2e.sh` |

E depois, cada perfil de raiz: **`business`, `personal` e `education`** —
`scripts/install-e2e.sh` corre os quatro, cada um num anfitrião novo, e a viagem
confirma as aplicações que o contrato de perfis diz que cada um traz.

## Correr

```bash
scripts/certify-general-os.sh
```

Constrói o pacote de prova, corre o `verify.sh` completo (as 125 viagens de
browser, os testes HTTP e todos os portões), as instalações dos quatro perfis, a
actualização, o restauro e a certificação de hardware, e escreve o resultado com
o SHA do release.

## Estado das provas de anfitrião

Release `e62a1e5dc795`, pacote de prova, anfitriões Linux descartáveis:

| Prova | Resultado |
|---|---|
| Instalação de raiz, quatro perfis (`research`, `business`, `personal`, `education`) | PASS |
| Actualização N → N+1 com MinIO → Garage, release falhado revertido, reversão manual | PASS |
| Backup, restauro noutro anfitrião, recusa sem raiz de selagem e com uma errada | PASS |
| Hardware 2 vCPU · 4 GB e 4 vCPU · 8 GB | PASS ([resultados](../install/hardware-results.md)) |

## O que falta para o portão

- **Passo 18 — `PARTIAL`.** O feed institucional de Actividade não tem viagem
  própria; `PARTIAL` não conta como `PASS`.
- **Passagem da produção para o Garage** e o `verify.sh` completo contra ele
  ([ADR-0208](../adrs/0208-maintained-object-store.md)).
