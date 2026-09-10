# ADR-0503 — Controlo institucional e residência física de um nó de compute

- **Estado:** Accepted
- **Domínio:** Compute
- **Impacto:** MEDIUM
- **Data:** 2026-09-10
- **Depende de:** [ADR-0500](0500-compute-registry-node-agent.md) (Compute Registry e Node Agent)
- **Relaciona-se com:** [ADR-0201](0201-data-residency.md) (residência de dados),
  [ADR-0502](0502-compute-intelligence-connection-contract.md) (contrato de ligação)

## Context

O primeiro nó GPU real da Ocinye será uma instância numa cloud pública de
terceiros (OVHcloud, uma NVIDIA L40S) a correr software da Ocinye. Não é ainda o
servidor físico próprio da Ocinye (`CLAUDE.md` §7); **não é o CAM-01**.

Hoje um `compute_node` descreve onde está apenas com `location_label` — texto
livre (`migrations/0007_intelligence_and_compute.sql`). Isso não distingue duas
coisas que são diferentes e que este caso obriga a separar:

- **quem controla o software e os dados** que correm na máquina;
- **onde o hardware está fisicamente**, e de quem é.

A residência já é modelada — mas só para o armazenamento (`ADR-0201`,
`storage_backends`, enum `Residency`). O plano de compute não herdou esse
vocabulário, e por isso uma máquina «controlada pela Ocinye, alugada a
terceiros» não tem representação fiel: seria registada com um `location_label`
livre e nada mais. Alugar hardware **não** cede controlo, e o modelo tem de o
poder dizer.

## Decision

Acrescentam-se a `compute_nodes` **dois eixos tipados, aditivos e
independentes**, reutilizando o vocabulário que já existe — sem criar uma
taxonomia paralela, um catálogo de regiões, nem um motor de política novo:

- **`institutional_control`** — `OCINYE | EXTERNAL` (enum
  `InstitutionalControl`). Quem controla o software e os dados. Um nó enrolado
  corre o Node Agent da Ocinye sob a nossa credencial, por isso o default é
  `OCINYE`. `EXTERNAL` fica reservado para um fornecedor que a instituição **não**
  controla (uma API de inferência externa), que se representa ao nível do
  fornecedor (`ai_models.provider_kind = external`), e **não** como um nó
  enrolado.
- **`physical_residency`** — reutiliza o enum `Residency` de `ADR-0201`
  (`UNDECLARED | THIRD_PARTY_CLOUD | OCINYE_CAMAMA | OCINYE_COLOCATION`). Onde o
  hardware está, e de quem é. O default é `UNDECLARED` — não se afirma uma
  residência que não foi declarada.

O `location_label` mantém-se como a etiqueta legível («OVHcloud Europe»); os dois
campos são a representação **tipada** ao lado dele.

O nó da L40S fica então, sem ambiguidade:

```text
institutional_control = OCINYE
physical_residency    = THIRD_PARTY_CLOUD
location_label        = "OVHcloud Europe"
```

e um futuro servidor próprio seria:

```text
institutional_control = OCINYE
physical_residency    = OCINYE_CAMAMA
location_label        = "Ocinye — Camama, Angola"
```

**Nenhum relaxamento de política nasce daqui.** Os dois campos são descritivos:
descrevem o nó, não concedem autoridade nem alteram a classificação de nada. A
decisão de como cada classificação (`PUBLIC`/`INTERNAL`/`CONFIDENTIAL`/`RESTRICTED`)
se comporta perante inferência neste nó é separada, e será tomada quando a
inferência real existir — esta ADR só garante que a informação necessária para a
tomar passa a estar representada com fidelidade.

## Alternatives

- **Manter só `location_label`.** Rejeitada: texto livre não distingue controlo
  de residência, e confundir «quem controla» com «quem possui o servidor» é
  exactamente o erro que a chegada de uma máquina alugada torna caro.
- **Um enum único combinando controlo e residência.** Rejeitada: colapsa dois
  eixos que são independentes por natureza — uma máquina da Ocinye em terceiros e
  uma máquina de terceiros em instalações da Ocinye são casos diferentes, e um
  enum plano ou os mistura ou multiplica-se combinatoriamente.
- **Uma moldura de residência nova e ampla para compute.** Rejeitada por
  desproporção (`CLAUDE.md` §71): reutiliza-se o `Residency` que já existe.
- **Inferir o controlo de `ai_models.provider_kind`.** Rejeitada: `ai_models` é
  um inventário transitório reportado pelo nó (apagado e reinserido a cada
  heartbeat); o controlo de um nó é uma propriedade durável do nó, e vive nele.

## Consequences

- `compute_nodes` ganha `institutional_control` e `physical_residency`
  (migração `0036`), ambos com default honesto e `CHECK` de domínio. Mudança
  puramente aditiva; um nó já existente assume `OCINYE` / `UNDECLARED`.
- O registo de um nó passa a aceitar os dois eixos (opcionais; omitir cai nos
  defaults), e o `NodeView` da API expõe-os. O `Core` continua a tratar tudo o
  que um nó reporta como entrada não confiável — estes dois campos são definidos
  **no registo, por um `PlatformAdmin`**, não reportados pelo agente.
- Fica representável, pela primeira vez, a distinção entre controlo institucional
  e residência física no plano de compute. É o que o caso OVHcloud exige, e o que
  permitirá, mais tarde, uma política de classificação que trate correctamente
  inferência numa máquina controlada pela Ocinye mas alojada em terceiros.
- **O que esta ADR não faz:** não liga nenhum nó, não instala nenhum modelo, não
  torna a IA disponível, não decide o runtime de inferência nem o primeiro
  modelo, e não altera nenhuma política de autorização ou de classificação. A IA
  continua indisponível e `compute_nodes = 0`.
