# ADR-0504 — A capacidade de um nó: física, reservada, alocável, alocada, consumida

- **Estado:** Accepted
- **Domínio:** Compute
- **Impacto:** MEDIUM
- **Depende de:** [ADR-0500](0500-compute-registry-node-agent.md) · [ADR-0013](0013-general-purpose-os-instance-and-node.md) · [ADR-0108](0108-resource-governance-and-compute-control-plane.md)
- **Data:** 2026-09-26

## Context

Um nó já reportava o que tinha — núcleos, memória, armazenamento, GPUs — e o Core
derivava a sua vivacidade do último heartbeat
([ADR-0500](0500-compute-registry-node-agent.md)). Para uma Instância governar os
nós que a servem ([ADR-0013](0013-general-purpose-os-instance-and-node.md)),
«o que o nó tem» não chega: parte da máquina é do sistema operativo do host, parte
será de trabalhos, e parte está em uso agora. Misturar os três é o erro clássico
de quem escalona.

## Decision

Cinco quantidades por recurso (CPU, memória, armazenamento), e mais nenhuma:

| Quantidade | Quem a diz | Como |
|---|---|---|
| **física** | o nó | reportada no heartbeat — dado não confiável, nunca base de autorização |
| **reservada** | o operador | `PUT /compute/nodes/{id}/reservation`, auditado, só com administração da plataforma |
| **alocável** | o Core | `física − reservada`, nunca negativa; sem relatório, desconhecida |
| **alocada** | o Core | **zero por construção** enquanto não houver despacho de trabalhos — o número diz isso, e não inventa um |
| **consumida** | o nó | reportada (memória em uso); campo opcional, para agentes antigos continuarem a funcionar |

As GPUs contam-se e somam-se em VRAM. A Instância soma a capacidade dos nós
**online** (`GET /compute/capacity`), e põe-lhe ao lado o armazenamento pessoal que
os membros usam, medido como a quota de cada um o mede — é aí que a capacidade
dos nós e as quotas se encontram.

**A identidade de um nó continua a ser uma credencial por nó**: um token de
enrolamento de uso único, trocado por um token de agente de longa duração, de que
o Core guarda só o digest. Não há palavra-passe partilhada entre nós. Uma
identidade criptográfica — par de chaves no nó, pedidos assinados, mTLS — é mais
forte e está prevista ([ADR-0502](0502-compute-intelligence-connection-contract.md));
fica para a revisão das fronteiras de confiança (Parte 12), com o modelo de
ameaças inteiro à frente.

## Alternatives

**Deixar o nó dizer o que é alocável.** Seria confiar num dado de uma máquina que
pode estar comprometida para uma decisão de governação.

**Um escalonador agora.** O programa proíbe complexidade de cluster sem evidência;
`alocada = 0` diz a verdade até ao primeiro despacho.

## Consequences

- O operador vê, por nó e pela Instância, o que existe, o que está reservado, o
  que se pode dar, o que foi dado e o que se usa.
- O primeiro despacho de trabalhos terá de escrever `alocada`; o contrato já tem o
  sítio.
