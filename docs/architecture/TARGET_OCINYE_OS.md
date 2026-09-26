# Ocinye OS de uso geral — arquitectura-alvo e delta

> **Estado de todo este documento: `PLANNED`.** Nada aqui descreve
> funcionalidade existente. O que existe está em
> [CURRENT_SYSTEM.md](CURRENT_SYSTEM.md). Cada linha do delta passa a `CURRENT`
> só quando a parte do programa que a entrega tiver o seu portão E2E verde.

O Ocinye OS está a deixar de ser o ambiente operativo interno da Ocinye para ser
um **ambiente operativo auto-alojado, de uso geral**, para pessoas e
organizações — empresas, universidades, institutos de investigação, instituições
públicas, pessoas individuais. A instalação actual da Ocinye passa a ser a
**primeira instância real** desse sistema.

Não é uma reescrita, nem um repositório novo, nem um redesenho por gosto. É
**generalizar** a arquitectura que já funciona, de modo que a instalação da
Ocinye se torne uma configuração dela.

**A definição canónica mudou com a Parte 1**
([ADR-0013](../adrs/0013-general-purpose-os-instance-and-node.md)), no mesmo PR que
tornou a Instância real; vive em [`README.md`](README.md#o-que-é-o-ocinye-os), e o
guarda `scripts/documentation-facts.py` protege-a.

---

## 1. O produto

A definição é a de [`README.md`](README.md#o-que-é-o-ocinye-os).

Corre sobre um Linux mínimo. O Linux continua a ser responsável pelo kernel, os
drivers, o escalonamento, as primitivas de sistema de ficheiros e de rede. O
Ocinye OS é o ambiente que as pessoas e as organizações efectivamente operam.

**Fora de âmbito, sempre:** kernel, drivers, substituto do Linux, compositor de
desktop, browser, sistema de ficheiros novo.

## 2. Princípios

1. **Auto-alojado.** Uma organização corre o Ocinye OS em infraestrutura que
   controla.
2. **Independente de fornecedor.** Nenhuma dependência constitucional de uma
   cloud, um fornecedor de IA, um object store, um fabricante de GPU ou um SaaS.
   Distingue-se a *dependência estratégica* (a evitar) da *dependência técnica
   deliberada* (PostgreSQL, Rust, Linux — a manter).
3. **Baseado em aplicações.** A funcionalidade pertence às aplicações. O Core
   governa-as e não as absorve.
4. **Consciente de recursos.** Armazenamento, CPU, RAM, GPU, rede e quotas são
   recursos governados de primeira classe.
5. **Nativo em IA, independente de IA.** Completo com zero GPU, zero
   fornecedores, zero modelos.
6. **Governado pelo Core.** *Operado com IA, governado pelo Core.* A IA propõe,
   raciocina, orquestra e pede; nunca é a autoridade final sobre identidade,
   autorização, permissões, políticas, invariantes, segredos, persistência ou
   transições destrutivas.
7. **Capaz de funcionar localmente.** Uma instalação mínima opera sem nenhum
   serviço de cloud obrigatório.
8. **Configurado, não bifurcado.** Investigação, empresa, educação e uso pessoal
   são configurações do mesmo sistema, nunca bases de código divergentes.

E quatro distinções que atravessam tudo:

| | não é | porque |
|---|---|---|
| **Instância** | Nó | uma é o ambiente governado; o outro é uma máquina que lhe contribui recursos |
| **Perfil** | fork, nem autorização | o perfil escolhe predefinições; o RBAC decide autoridade |
| **Desafixar** | desinstalar | já é invariante (§45-A) |
| **Configuração** | código novo | «a empresa A não quer Bibliografia» resolve-se desactivando a aplicação |

## 3. A pilha conceptual

```text
┌──────────────────────────────────────────────┐
│ OCINYE WORKSPACE   interface · lançador · definições
├──────────────────────────────────────────────┤
│ APLICAÇÕES         Notas · Ficheiros · Projectos · Correio · Investigação …
├──────────────────────────────────────────────┤
│ AI FABRIC          Gateway · Router · Políticas · Fornecedores · Modelos
├──────────────────────────────────────────────┤
│ OCINYE CORE        Identidade · RBAC · Políticas · Recursos · Segredos
│                    Autoridade de aplicações e de nós · Auditoria · Invariantes
├──────────────────────────────────────────────┤
│ RESOURCE PLANE     Armazenamento · CPU · RAM · GPU · Nós
├──────────────────────────────────────────────┤
│ RUNTIME DE CONTENTORES / SERVIÇOS
├──────────────────────────────────────────────┤
│ LINUX · HARDWARE
└──────────────────────────────────────────────┘
```

É conceptual. A estrutura do repositório não tem de a imitar se as fronteiras
actuais — crates com arestas permitidas, portões arquitecturais — a exprimirem.

## 4. Os conceitos-alvo

**Instância.** Um ambiente Ocinye OS governado de forma independente: dono do
reino de identidade, membros, configuração, aplicações, dados, políticas,
segredos, recursos, nós e fornecedores de IA. O modelo por omissão é **uma
instância por instalação** — não multi-tenancy SaaS. O ponto de partida é a
tabela `organisations`, que já existe e já delimita todo o domínio por
`organisation_id`; a decisão de a promover a Instância ou de a pôr debaixo de uma
é da Parte 1, por ADR.

**Perfil.** Predefinições de uma instância: aplicações activas, fixações por
omissão, configuração inicial, recursos por omissão. `Research`, `Business`,
`Personal`, `Education`. Muda-se depois da instalação; nunca concede autoridade.
A instalação actual mapeia para `Research`.

**Aplicação.** Uma entidade com manifesto — identidade, versão, chaves i18n,
ícone, categoria, rota, capacidades pedidas, recursos, rede, capacidades de IA,
disponibilidade, ciclo de vida, saúde, política de fixação — e **activação por
instância**. Pede capacidades (`storage.read`, `mail.send`, `ai.reasoning`…) e não
herda base, sistema de ficheiros, segredos, rede nem credenciais. As nativas são
confiáveis; o manifesto não pode assumir que todas as futuras são compiladas.

**Nó.** Uma máquina ou ambiente de computação que contribui recursos a uma
instância, com identidade criptográfica, enrolamento, heartbeat, capacidades
declaradas e capacidade alocável. O Core escala e governa; o nó nunca recebe
autoridade de base de dados.

**Autoridade de Segredos.** O Core guarda credenciais de fornecedores e de
integrações cifradas, entrega o **uso** e não o valor, audita, roda e revoga. Uma
aplicação recebe o resultado, não o segredo.

**AI Fabric.** `aplicação/agente → Gateway → política + router → adapter de
fornecedor → endpoint de modelo`. Vários fornecedores em simultâneo; um registo
de modelos separado da identidade do fornecedor; roteamento por capacidade,
classificação, política, disponibilidade e entitlement; `NO_EXTERNAL_AI` imposto
pelo router, não pelo texto do pedido.

## 5. O delta, área a área

Cada linha diz o que existe, o que falta, e a parte do programa que o entrega.

| Área | Hoje | Alvo | Parte |
|---|---|---|---|
| **Linha de base** | CI vermelha desde 24/Set; backup nocturno a falhar; *branch protection* sem *required checks*; MinIO retirado pelo fabricante | CI verde e observada, backup a correr, dependências mortas substituídas por artefactos controlados | **0** |
| **Instância** | **`CURRENT` desde a Parte 1** ([ADR-0013](../adrs/0013-general-purpose-os-instance-and-node.md), [docs/instance](../instance/README.md)): singleton `instance_identity`, resolução sem slug por omissão, nome da Instância no TOTP, na assinatura e na IA, inventário de IA por Instância, instalação existente mapeada sem perdas | — | 1 ✓ |
| **Perfis** | estrutura de investigação no núcleo: unidades semeadas sempre, tarefas e datasets presos a ambientes de investigação, Home e «+ Criar» centrados em investigação, e **Ficheiros escondido a quem não tem papel de investigação** (a relevância de módulo decide a sua visibilidade) | quatro perfis sobre o mesmo sistema; `Research` = comportamento actual | 2 |
| **Fronteira Core/aplicações** | um crate, um binário, um router incondicional | aplicações desactiváveis sem afectar autenticação, pertença, autorização, contas de recursos, lançador, definições ou saúde do Core | 3 |
| **Manifesto de aplicação** | registo estático no Workspace, sem disponibilidade nem activação | manifesto versionado, activação por instância, capacidades pedidas, portão de consistência | 4 |
| **Nós** | registo, enrolamento por token, heartbeat; nó sem trabalho; token de portador sem mTLS | identidade criptográfica, capacidade física/alocável/reservada/alocada/consumida, saída e regresso automáticos | 5 |
| **Segredos** | raiz de selagem com HKDF por domínio; nenhum armazém genérico; segredos de plataforma em env | armazém de segredos com metadados, âmbito, uso sem exposição, rotação, revogação e auditoria | 6 |
| **AI Fabric** | `NoProvider` fixo no binário; `ai_models` escrita só por nós; sem registo de fornecedores | fornecedores configuráveis em runtime, adapters isolados, registo de modelos, estado degradado tipado por fornecedor | 7 |
| **Roteamento e política** | primeiro candidato; flag global de externos; tecto por classificação | router por política com `NO_EXTERNAL_AI`, fallback permitido, alterações sem reinício | 8 |
| **Instalação** | não existe; deploy assume um host preparado à mão | `ocinye install` versionado; host limpo → workspace aberto, sem GPU nem chave de IA | 9 |
| **Upgrade e rollback** | `git archive` + build no host; migrations irreversíveis; rollback falha através de uma migration | preflight, checkpoint, portão de saúde, recuperação provada de um release falhado | 10 |
| **Backup e restauro** | mecanismo completo e provado em ensaio; agendador instalado mas partido | backup agendado verde, restauro para um host limpo, incluindo configuração e segredos de fornecedor | 11 |
| **Segurança** | Core auditado; serviços sem `cap_drop`/`read_only`; runner com `docker.sock` | modelo de ameaças por fronteira do sistema geral; negativos E2E | 12 |
| **Observabilidade** | logs estruturados; sem métricas nem alertas | saúde de serviços, nós, fornecedores, modelos e aplicações, sem conteúdo de membros | 13 |
| **Configuração e marca** | marca da Ocinye no código | nome, logótipo, idioma, fuso, perfil e aplicações configuráveis por instância, sem mexer em semântica de segurança | 14 |
| **Hardware mínimo** | um host de 8 vCPU/15 GiB, nunca medido | mínimo medido, sem GPU | 15 |
| **Certificação** | 119 viagens de browser em Chromium | jornada completa por perfil; Chromium, WebKit, Firefox; 1440/1280/768/390; `pt`/`en`/`fr` | 16 |
| **Release** | — | `OCINYE_GENERAL_OS_BASELINE_READY = TRUE`, só com evidência | 17 |

## 6. Itens obrigatórios que nasceram da Parte 0

Estes não estavam na lista do programa. A descoberta fê-los aparecer, e ficam
aqui para não se perderem:

- **Object store mantido.** O MinIO Community Edition foi arquivado pelo
  fabricante e deixou de ser distribuído. A Parte 0 repõe a reprodutibilidade
  com um espelho controlado pela Ocinye, fixado por digest e classificado como
  `LEGACY_COMPATIBILITY_DEPENDENCY` — uma ponte, não a decisão de armazenamento.
  **Antes de `OCINYE_GENERAL_OS_BASELINE_READY = TRUE`**, a fase de Host/Storage
  (Partes 9–11) escolhe um store S3-compatible mantido, com ADR, testes de
  compatibilidade, migração dos dados, rollback e E2E.
- **Rollback através de migrations.** Hoje é impossível por construção. A Parte
  10 tem de o resolver por compatibilidade de esquema ou por restauro, e dizer
  qual.
- **O CI tem de ser portão, não aviso.** A Parte 0 mostrou treze merges para
  `main` sem uma única execução de testes. Enquanto os merges forem forçados, a
  única prova de uma parte é a corrida local do `verify.sh`, e o relatório de
  cada parte tem de o dizer.
- **i18n da shell e do Core.** Os literais em português fora do catálogo e a
  preferência de idioma só em cookie são incompatíveis com a Parte 14 e com a
  matriz de idiomas da Parte 16.

## 7. O que este programa não constrói

Suite de escritório, editor de vídeo, browser, CRM, ERP, contabilidade,
alojamento de código, loja de aplicações, plataforma Kubernetes, sistema de
ficheiros distribuído, kernel, hipervisor. Nem malha de serviços, consenso
distribuído ou escalonador de cluster sem evidência de necessidade. **Um único
nó continua a ser a instalação mais simples e mais fiável**, e a distribuída é
aditiva.
