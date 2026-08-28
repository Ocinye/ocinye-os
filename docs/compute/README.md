# Compute Plane

**Estado: 0 nós registados. `CAM-01` não existe.**

Zero é o estado corrente, verdadeiro e válido. A plataforma funciona
integralmente sem qualquer nó.

## O registo

Um nó é uma **linha em base de dados**, não uma constante. Identificador,
localização, estado, CPU, memória, GPUs, capacidades, modelos, saúde,
`last_seen_at` e versão do agente são **dados reportados**, nunca assumidos.

Nenhum identificador de nó aparece no código. `CAM-01`, `HPC-01`, `DC-01` são
valores que um registo futuro fornecerá.

## Liveness derivada

Um nó está online se e só se o seu último heartbeat estiver dentro da janela.
Não existe flag `is_online`. Ver
[`ComputeNode::effective_status`](../../crates/ocinye-core/src/modules/compute/model.rs).

## Identidade de máquina

O agente tem credenciais próprias, revogáveis, obtidas trocando um token de
enrolamento de utilização única. Nunca usa credenciais de uma pessoa.

## Segurança de rede

O nó GPU **nunca** aceita tráfego público de aplicação. A topologia futura é
`VPS → WireGuard → CAM-01`, com o agente a estabelecer a ligação para fora.

Um nó comprometido é tratado como hostil: pode mentir sobre os seus recursos,
pelo que os relatos são dados não confiáveis e a autorização de jobs é sempre
decidida pelo Core.

## Workloads científicos

O Ocinye OS é **agnóstico à linguagem** dos workloads. Um Experiment pode usar
Python, Fortran, C/C++, Julia, Rust, OpenFOAM ou MPI.

O que o Ocinye OS precisa de saber é: input, versão do código, ambiente, job, nó,
parâmetros, output e proveniência. **Não reimplementa solvers científicos.**

## Trajectória

```
Hoje     0 nós
Depois   CAM-01 — AMD EPYC · 256 GB ECC · RTX 4090 24 GB
Depois   CAM-02 — mais storage, mais GPUs
Depois   HPC-01
Depois   colocation, microdatacenter
```

Cada passo é uma linha no registo, não uma reescrita. O modelo já é 0..N.

## Não implementado

- Despacho e agendamento de jobs.
- Contabilização de recursos.
- Integração com um scheduler HPC maduro — quando houver HPC, integra-se um que
  exista, não se escreve um.

## Verificado: o que acontece quando um nó chega

Durante a auditoria de 2026-08-22 registou-se **uma fixture** de um nó e um
modelo, para confirmar que a arquitectura reconhece o novo recurso sem alterações
de código.

Resultado, sem tocar em código, migration ou interface:

| Antes | Depois |
|---|---|
| `compute` → `no_resource` | `compute` → `available`, «1 de 1 nós activos» |
| `ai.general` → `no_resource` | `ai.general` → `available` |
| Agentes → `configured` | Agentes → `ready` |

A fixture foi removida em seguida. **Nunca existe seed em produção.**

Isto é a invariante que o registo existe para garantir: quando o CAM-01 for
instalado, o Ocinye OS integra-o pela arquitectura existente.
