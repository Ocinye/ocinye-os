# ADR-0500 — Compute Registry e Node Agent

- **Estado:** Accepted
- **Domínio:** Compute
- **Impacto:** HIGH
- **Data:** 2026-08-22

## Context

A Ocinye planeia adquirir um servidor GPU em Camama. Não existe. A arquitectura
tem de suportar zero, um e N nós sem reconstrução, e **sem nunca hardcodar**
`CAM-01`.

## Decision

### Compute Registry

Um nó é uma **linha em base de dados**, não uma constante. Identificador,
localização, estado, CPU, RAM, GPUs, capacidades, modelos, saúde, `last_seen_at`
e versão do agente são todos **dados reportados**, nunca assumidos.

`compute_nodes = 0` é o estado corrente, verdadeiro e reportado como tal. Nenhum
`CAM-01` fictício é criado; fixtures de nós existem apenas em testes claramente
identificados.

### Liveness derivada, nunca declarada

Um nó está online se e só se o seu último heartbeat estiver dentro da janela de
liveness. Não existe flag `is_online` que alguém possa pôr a `true`.

### Identidade de máquina

O Node Agent **não é um utilizador**. Tem identidade e credenciais próprias.
Nunca reutiliza credenciais humanas. O enrolamento usa um token de utilização
única e curta duração, do qual só o digest é persistido; o agente recebe depois
uma credencial própria, rotacionável e revogável.

### Protocolo do nó

`docs/node-protocol/` define enrollment, heartbeat, relato de recursos, relato de
modelos e ciclo de vida de jobs. Versionado, como qualquer contrato.

### Segurança de rede

O nó GPU **nunca** aceita tráfego público de aplicação. A topologia futura é
`VPS → WireGuard → CAM-01`. O agente estabelece a ligação **para fora**; o Core
não abre ligações para dentro do nó.

Um nó comprometido é tratado como hostil no threat model: pode mentir sobre os
seus recursos, pelo que os relatos são dados não confiáveis, e a autorização de
jobs é sempre decidida pelo Core.

## Alternatives

| Alternativa | Porque foi rejeitada |
|---|---|
| **Assumir um nó e configurá-lo por variáveis de ambiente** | Torna o segundo nó um rewrite; contraria `CLAUDE.md` §29. |
| **Kubernetes / SLURM já** | Proibido nesta fase e desproporcionado para zero nós. Integrar um scheduler maduro é a via correcta quando houver HPC (briefing §102). |
| **Agente com pull de jobs por SSH** | Reutilizaria credenciais e canais humanos para identidade de máquina. |
| **Expor o nó com API pública** | Rejeitado por segurança (briefing §58). |

## Consequences

**Positivas** — zero, um ou N nós sem mudança estrutural; a UI mostra o estado
verdadeiro; identidade de máquina separada desde o início.

**Negativas, aceites** — o registry existe antes de ter nós; o Node Agent é hoje
um esqueleto que enrola, faz heartbeat e reporta recursos, sem execução de jobs.
Declarado como esqueleto, não como runtime pronto para produção.

## Referências

`CLAUDE.md` §29, §30 · briefing §54–§58 · ADR-0300
