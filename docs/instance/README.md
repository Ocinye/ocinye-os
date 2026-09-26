# Instância

Uma **Instância** é um ambiente Ocinye OS governado de forma independente: dona
do seu reino de identidade, membros, configuração, aplicações, dados, políticas,
segredos, recursos, nós e fornecedores de IA. A Ocinye é a primeira; uma
universidade, uma empresa ou uma pessoa com o Ocinye OS instalado tem a sua.

Decisão: [ADR-0013](../adrs/0013-general-purpose-os-instance-and-node.md).

## O que pertence aqui

- O modelo de Instância, e como uma instalação sabe qual serve.
- O que um operador define para criar uma Instância, e o que não pode mudar
  depois.

## O que não pertence aqui

- Os **Nós** que contribuem recursos a uma Instância: [`docs/compute/`](../compute/README.md).
- Os **Perfis** (investigação, empresa, pessoal, educação) e a marca: partes
  seguintes do programa ([arquitectura-alvo](../architecture/TARGET_OCINYE_OS.md)).

## Uma instalação, uma Instância

Este é o modelo por omissão e o único suportado. **Não é multi-tenancy.** Na base
de dados, a Instância é a organização que a governa — a linha de `organisations`
que já delimita todo o domínio por `organisation_id` —, e a instalação regista
qual é na tabela singleton `instance_identity`. O `id` dessa linha é a
identidade durável da Instância: a mesma quando ela muda de servidor.

| | É | Muda? |
|---|---|---|
| **Slug** | identidade técnica; prefixo das chaves de objecto | não — renomear órfã os objectos |
| **Nome** | o que as pessoas lêem: emissor TOTP, assinatura de correio, trilho do Workspace, instrução de sistema da IA | sim, é da Instância |
| **`instance_identity.id`** | a identidade durável da Instância | nunca |

## Como o Core decide qual Instância serve

```text
OCINYE_INSTANCE_SLUG definido (ou o nome antigo, OCINYE_ORGANISATION_SLUG)
  → adopta essa organização, ou cria-a se a base ainda não tem Instância
senão, a Instância registada em instance_identity
senão, a única organização da base, se houver exactamente uma
senão → o Core recusa arrancar, e diz o que definir
```

A primeira resolução fica registada, e daí em diante o arranque é
determinístico. Uma configuração que peça **outra** Instância numa instalação que
já tem uma é recusada: uma instalação não muda de dono por uma variável de
ambiente.

## Criar uma Instância

Numa base vazia, o [bootstrap do primeiro administrador](../runbooks/bootstrap-first-administrator.md)
cria a Instância e o seu primeiro administrador de uma vez:

```bash
ocinye-core-server bootstrap-admin --instance-name "Cooperativa Exemplo" …
```

Sem `OCINYE_INSTANCE_SLUG`, o slug deriva do nome (`cooperativa-exemplo`).

## A primeira Instância

A instalação que existia antes deste modelo passou a ser a primeira Instância
pela migração `0052_instance_identity`: com exactamente uma organização na base,
ela fica registada; o nome das organizações cujo nome era o próprio slug —
defeito do bootstrap antigo — passa a um nome legível (`ocinye` → `Ocinye`), e um
nome que alguém escolheu não é tocado. Nenhuma linha muda de dono.

## Provas

| Propriedade | Onde |
|---|---|
| As cinco vias de resolução, e a recusa de mudar de Instância | `crates/ocinye-core/tests/instance_model.rs` |
| A instalação existente passa a primeira Instância sem perdas | idem, `a_instalacao_existente_passa_a_primeira_instancia_sem_perdas` |
| Um nó de uma Instância não serve outra | `crates/ocinye-core/tests/instance_isolation.rs` |
| Uma Instância nova abre com o seu nome e as suas aplicações | `apps/workspace/tests/browser.rs`, `uma_instancia_nova_abre_com_o_seu_nome_e_as_suas_aplicacoes` |
