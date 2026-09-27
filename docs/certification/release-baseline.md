# Linha de base do Ocinye OS de uso geral (Parte 17)

> **O Ocinye OS deixou de estar arquitecturalmente preso à organização de
> investigação Ocinye.** A Ocinye é uma Instância, com o perfil `research`, de um
> produto que se instala de raiz numa máquina Linux qualquer.

Esta página é a evidência canónica do release que o afirma. Cada linha liga uma
capacidade que a Parte 17 exige à prova que a exercita, e a prova à corrida onde
passou. Uma capacidade que não tenha linha aqui não está provada.

| | |
|---|---|
| Release em produção | `4f8d0489f54e` (`main @ 4f8d048`, PR #179), deployado a 2026-09-27 |
| Releases anteriores | `18eb8248db1b` (a passagem para o Garage), `f0fef6d72071` |
| Esquema | 58 migrações |
| Provas de anfitrião | pacote de prova de `e62a1e5dc795`, anfitriões Linux descartáveis |
| Verificação canónica | `./scripts/verify.sh` em `01001f7`, contra o Garage: PASS |
| CI | PR #178, todos os trabalhos verdes |

## O que o release suporta, e onde está provado

| Capacidade | Prova |
|---|---|
| Instalação auto-alojada de raiz | `scripts/install-e2e.sh`: quatro anfitriões novos, instalação, entrada com segundo factor, trabalho pelo browser — PASS |
| Instância genérica | ADR-0013; a viagem de uma instância que não é a Ocinye, criada de raiz pelo bootstrap |
| Perfil seleccionável | os quatro perfis na prova de instalação; a viagem de empresa sem módulos científicos |
| Gestor de Aplicações e registo | ADR-0016, §45-A; viagens do lançador, pesquisa e fixação |
| Governança de recursos | ADR-0108/0109; «Meus Recursos» e quotas de armazenamento nas viagens e nas suites do Core |
| Ficheiros | viagens de carregamento, Quick Look e descarga; «Meus ficheiros» na Instância instalada |
| Aplicações base deterministas | Notas, Tarefas, Ideias → Projectos, Datasets, Calendário, Correio: as 126 viagens de browser |
| Gestão de membros | viagens de acesso, primeiro acesso e revogação; a varredura de autorização |
| Arquitectura de Nós | ADR-0504; registo, heartbeat, offline e regresso por HTTP (`node_fabric_http.rs`) |
| Fornecedores de IA | ADR-0310; registo com segredo selado e o Prompt a responder por ele (`ai_providers_http.rs`) |
| Operação sem IA | produção com 0 fornecedores e 0 nós; o Prompt responde `SYSTEM`/`DEGRADED`, e na Instância instalada também |
| Ligação a fornecedor externo | protocolos OpenAI, Anthropic, Gemini e compatível, contra fornecedores simulados; nenhum real contactado |
| Encaminhamento entre fornecedores | ADR-0311; os cenários de `ai_routing_http.rs` |
| Compatibilidade com nó local | o encaminhamento pelo inventário reportado pelo nó (ADR-0304) |
| Governança de segredos | ADR-0110; nunca devolvidos, procurados em claro em toda a base e não encontrados |
| Backup e restauro | `scripts/restore-e2e.sh` — PASS; na produção, o agendador dispara e a cópia externa chega ao cofre, confirmada por leitura de volta (abaixo) |
| Actualização | `scripts/upgrade-e2e.sh` — PASS, com a passagem MinIO → Garage e um release falhado revertido |
| `pt`, `en`, `fr` | a viagem de troca de língua; o portão de chaves em falta = 0 |
| Hardware | `MINIMUM_SUPPORTED` 2 vCPU · 4 GB, `RECOMMENDED` 4 vCPU · 8 GB — [medido](../install/hardware-results.md) |
| Armazenamento mantido | Garage (ADR-0208, `Accepted`), em produção desde este release |

A jornada de 33 passos está em [`general-os.md`](general-os.md), com os 33 provados.

## Em produção, depois do deploy

- `/ready` com os componentes críticos disponíveis e o `storage` disponível, já
  sobre o Garage; o Workspace a encaminhar para `/boot`; o Core a recusar sem
  sessão (`401`).
- `verify-objects` na produção: 5 objectos lidos do Garage, somas recalculadas.
- Backup agendado: ver [Backup agendado](#backup-agendado).

## Backup agendado

- **O agendador dispara:** a execução de 2026-09-27 03:00 UTC foi disparada pelo
  `ocinye-backup.timer`. O conjunto local ficou completo (manifesto, base, os 5
  objectos já lidos do Garage, somas, cifra `age`), mas a cópia externa foi
  recusada: o `rclone` pedia `CreateBucket`, e a chave do R2, só de objectos,
  recebia 403. O `mc` que ele substituiu não fazia esse pedido.
- **A cópia externa chega ao cofre:** com `no_check_bucket` no release
  `4f8d0489f54e`, a execução de 2026-09-27 07:46 UTC — lançada pelo operador com
  `systemctl start ocinye-backup.service`, a mesma unidade que o agendador dispara —
  terminou com sucesso: 7 objectos, cópia externa **confirmada por leitura de
  volta**, e retenção aplicada nas duas pontas.
- A próxima execução agendada (2026-09-28 03:00 UTC) é a primeira em que as duas
  coisas acontecem juntas; é confirmação, não condição.

## Escritas novas no Garage

Depois do deploy, um ficheiro carregado pelo browser na produção passou o bucket
de 5 para 7 objectos (o ficheiro e a sua miniatura), e `verify-objects` recalculou
a soma dos 7, lidos do Garage.

## Declaração

```
OCINYE_GENERAL_OS_BASELINE_READY = TRUE   (2026-09-27, release 4f8d0489f54e)
```

## O que este release **não** é

- **Não é um pacote assinado.** As somas provam integridade, não origem; as
  imagens não estão num registry público.
- **Não tem IA real ligada.** 0 fornecedores, 0 nós, 0 GPU:
  `OCINYE_AI_RUNTIME_READY` continua falso, e isso é o estado certo deste release.
- **`amd64` não foi medido** na certificação de hardware (foi `arm64`).
- **Os espelhos da Ocinye no GHCR** para o Garage e o `rclone` existem mas são
  privados; o Compose usa as imagens de origem, fixadas por digest.
