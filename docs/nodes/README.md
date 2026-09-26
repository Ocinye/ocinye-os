# Nós

Um **Nó** é uma máquina ou ambiente de computação que contribui recursos a uma
[Instância](../instance/README.md): um servidor físico, uma VM, uma máquina de
cloud, uma estação de trabalho, um host de GPU. `Instância ≠ Nó`
([ADR-0013](../adrs/0013-general-purpose-os-instance-and-node.md)).

Uma instalação pequena é uma Instância com um nó implícito — o host onde o Core
corre — e zero nós registados. Uma maior regista nós de computação e de IA, que
entram e saem sem o Core deixar de responder.

## O que pertence aqui

- Registo, enrolamento, heartbeat, vivacidade, e capacidade de um nó.

## O que não pertence aqui

- O protocolo do agente, campo a campo: [`docs/node-protocol`](../node-protocol/README.md).
- O registo de computação e os estados: [`docs/compute`](../compute/README.md).

## O ciclo de vida

```text
administrador regista  → token de enrolamento (uso único, com prazo)
agente enrola          → token de agente (o Core guarda só o digest)
agente bate            → recursos, consumo, capacidades, modelos
silêncio > limiar      → offline: os seus modelos deixam de servir; o Core continua
agente volta a bater   → online: a capacidade regressa sozinha, sem reinício
token falso ou gasto   → 401
```

## A capacidade

Cinco quantidades por recurso ([ADR-0504](../adrs/0504-node-capacity-model.md)):
**física** (o nó reporta), **reservada** (o operador guarda para o host),
**alocável** (física − reservada), **alocada** (zero até haver despacho de
trabalhos) e **consumida** (o nó reporta). `GET /api/v1/compute/nodes` dá-as por
nó; `GET /api/v1/compute/capacity` soma os nós online e mostra ao lado o
armazenamento pessoal que os membros usam.

## Provas

`services/core-server/tests/node_fabric_http.rs` percorre as oito etapas do
programa — registo, heartbeat, descoberta, nó que cai, Core saudável, nó que
volta, capacidade que regressa, nó falso recusado — e a reserva.
