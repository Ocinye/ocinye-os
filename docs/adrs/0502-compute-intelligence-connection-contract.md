# ADR-0502 — Contrato de ligação do nó Compute/Intelligence

- **Estado:** Accepted
- **Domínio:** Compute
- **Impacto:** HIGH
- **Data:** 2026-09-09
- **Depende de:** ADR-0500 (Compute Registry e Node Agent), ADR-0300 (AI Gateway),
  ADR-0304 (contrato canónico de inferência), ADR-0305 (Provider Conformance)

## Context

O primeiro nó de IA/Compute da Ocinye ainda não existe (`CLAUDE.md` §7). O
Compute Registry, a identidade de máquina e o protocolo do nó já estão decididos
(ADR-0500, `docs/node-protocol/` v1): registo, enrolamento por token de uso
único, credencial de agente própria, heartbeat, liveness derivada, e a topologia
`VPS → WireGuard → CAM-01` com o agente a ligar **para fora**. A inferência tem o
seu contrato canónico (ADR-0304) e a fronteira de fornecedor a sua suite de
conformidade (ADR-0305).

O que **não** estava decidido, e o `docs/node-protocol/` listava como «não
implementado»: a confiança do transporte para lá do WireGuard, a rotação da
credencial do agente, e como uma capacidade reportada por um nó passa a
**disponível** no AI Gateway. Sem estas decisões, a milestone seguinte começaria
por **decidir como o Core fala com o nó** — e não por ligá-lo.

Esta ADR fecha essas decisões. **Não integra nenhum modelo nem liga nenhum
fornecedor** — o nó não existe. Fecha o *contrato*, para que a milestone
`CONNECT OCINYE AI / COMPUTE NODE` comece já na ligação.

## Decision

### 1. Transporte e confiança

A ligação nó↔Core assenta em **duas camadas, e nenhuma sozinha basta**:

1. **WireGuard** — o nó não tem endereço público e o Core não abre ligações para
   dentro dele (ADR-0500). O túnel é a fronteira de rede; fora dele o nó é
   inalcançável.
2. **Credencial de agente por pedido** — cada chamada do agente leva o
   `x-ocinye-node-token` (node-protocol v1). O token identifica a máquina; a
   rede identifica o caminho. Uma credencial válida vinda de fora do túnel não
   entra, e um pacote dentro do túnel sem credencial não autentica.

**mTLS não é exigido para o troço dentro do WireGuard**, e a razão é a mesma do
plaintext Workspace↔Core dentro da rede de serviço (ADR-0103): a propriedade que
autoriza é a topologia, confrontada no arranque. Um túnel WireGuard autenticado
por chave é o análogo da rede de serviço do Compose. mTLS **é** exigido se algum
dia o transporte deixar de ser WireGuard — e o Core recusa uma origem de nó que
não seja nem o túnel nem mTLS. A decisão é registada aqui para não ser
redecidida por inércia no dia do primeiro nó.

### 2. Rotação e revogação da credencial de agente

A credencial de agente **expira** e **rotaciona sem downtime**: o agente pede uma
nova antes de a corrente caducar, apresentando a corrente ainda válida
(re-enrolamento suave). O Core aceita a nova e invalida a anterior num passo
atómico; só o digest é persistido, como no enrolamento inicial. **Revogar** é uma
operação de administração governada — como a revogação de sessão de um membro
(ADR-0107) — com auditoria, e um agente revogado deixa de fazer heartbeat com
efeito imediato: a liveness derivada trata-o como `offline` sem ninguém pôr uma
flag.

### 3. Activação de capacidade

Uma capacidade reportada por um nó é **dado não confiável** até o Core a activar
(ADR-0500: o nó pode mentir). O fluxo:

```text
nó reporta {compute, ai.general, …} no heartbeat
  → Core regista a capacidade contra o nó, estado `reported`
  → Core prova-a: para Intelligence, a Provider Conformance Suite (ADR-0305);
    para Compute, um job de verificação controlado
  → só ao passar é que a capacidade fica `available` no AI Gateway / Compute
  → a ausência de nó mantém-na `EXPECTED_PENDING_AI`, que não é avaria
```

Passar a conformidade **não torna o nó confiável** — torna-o utilizável
(ADR-0305). A autorização de cada job continua a ser decidida pelo Core, nunca
pelo relato do nó.

### 4. Inferência: forma, streaming, falha

O que atravessa a fronteira de inferência é o **contrato canónico do Gateway**
(ADR-0304), e não o formato de nenhum fornecedor: a tradução vive no adapter e
nunca alcança o Agent Runtime nem o Core determinístico. Acrescenta-se a esta
ADR, por serem fronteira de ligação e não de formato:

- **Streaming** é opcional e negociado; um nó que não o suporte serve a resposta
  inteira, e o Gateway não promete ao chamador o que o nó não dá.
- **Timeouts** são do Core, não do nó: uma inferência sem resposta dentro do
  prazo é falha, e o nó não pode estendê-lo. Um nó que emudece a meio é
  `offline` pela liveness, não «a pensar para sempre».
- **Semântica de falha**: uma falha de inferência é um `CapabilityResult` de erro
  tipado, nunca estado do sistema (ADR-0002). Indisponibilidade degrada com
  razão; nunca inventa uma resposta.

### 5. Segredos e observabilidade

Os segredos do nó (credencial de agente, chave WireGuard) vivem no mecanismo de
segredos da instalação, **nunca no repositório, nunca no Workspace, nunca em
log**. Não se geram credenciais definitivas de um servidor que não existe: o
enrolamento emite-as no dia da ligação. A observabilidade do nó é a do resto do
sistema (§62): logs estruturados, `last_seen_at`, estado derivado, sem segredos e
sem conteúdo de inferência.

### 6. O ponto de partida da próxima milestone

O procedimento passo-a-passo vive em
[`docs/runbooks/connect-compute-node.md`](../runbooks/connect-compute-node.md). A
milestone `CONNECT OCINYE AI / COMPUTE NODE` começa nesse runbook — provisionar,
estabelecer confiança, enrolar, heartbeat, activar capacidade, correr um teste
controlado — e não em decidir contrato nenhum.

## Alternatives

| Alternativa | Porque foi rejeitada |
|---|---|
| **mTLS obrigatório já, dentro do WireGuard** | Duas camadas criptográficas a fazer o mesmo trabalho dentro de um túnel autenticado; a topologia já é a fronteira (ADR-0103). Fica exigido se o transporte deixar de ser WireGuard. |
| **Credencial de agente eterna** | Uma credencial que nunca roda é uma que nunca se pode retirar sem downtime; contraria a identidade de máquina rotacionável do ADR-0500. |
| **Confiar na capacidade reportada pelo nó** | O nó é hostil no threat model (ADR-0500); relato não é prova. A capacidade prova-se antes de servir. |
| **Definir tudo só quando o nó chegar** | Faria a milestone seguinte começar por decidir arquitectura em vez de ligar — exactamente o que esta ADR evita. |

## Consequences

**Positivas** — a próxima milestone começa na ligação, não na decisão. O contrato
de confiança, rotação e activação está fechado e é o mesmo para zero, um ou N
nós. A ausência de IA/Compute é uma classificação (`EXPECTED_PENDING_AI`), não uma
avaria.

**Negativas, aceites** — a rotação suave, a activação por conformidade e o
despacho de jobs continuam **não implementados** (ADR-0500 já os declarava
esqueleto); esta ADR decide-lhes a *forma*, não os constrói. Constroem-se na
milestone da ligação, contra um nó real.

## Referências

`CLAUDE.md` §7, §8, §29, §30, §41 · ADR-0500 · ADR-0300 · ADR-0304 · ADR-0305 ·
ADR-0107 · `docs/node-protocol/` · `docs/runbooks/connect-compute-node.md`
