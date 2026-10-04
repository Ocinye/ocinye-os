# ADR-0025 — Descoberta de capacidades de hardware e a fronteira do futuro Nó de Computação

- **Estado:** Proposed
- **Domínio:** Compute
- **Impacto:** MEDIUM
- **Depende de:** [ADR-0013](0013-general-purpose-os-instance-and-node.md) · [ADR-0503](0503-compute-node-control-and-residency.md) · [ADR-0504](0504-node-capacity-model.md)
- **Data:** 2026-10-04

Importada do pacote D011 do Claude Design, no formato desta biblioteca. O número fica na
faixa 0001–0099 por ser a numeração atribuída pelo Design; o domínio é Computação.

## Context

O servidor onde se instala pode ter GPU. Saber o que ele tem é útil ao operador e à
futura rede de computação, mas instalar o Ocinye OS não pode tornar uma máquina num nó.

## Decision

1. O Installer descobre o hardware só por leitura (CPU, RAM, armazenamento, rede, GPU de
   qualquer fabricante, VRAM, driver, runtime de aceleração, capacidade de computação,
   presença de computação confidencial e TPM) e regista-o como `HardwareCapabilities`.
   GPU nunca é exigida; erros de descoberta são `UNKNOWN` e não bloqueiam.
2. A evidência na D011 é só `DETECTED`. `VERIFIED` e `ATTESTED` ficam reservadas.
3. `ComputeReadiness` é informativa e não tem variante que signifique registado,
   disponível, a ganhar ou ligado.
4. Um Nó de Computação Ocinye é uma instalação com Modo Fornecedor explicitamente ligado
   — opt-in, reversível, desligado por omissão — e distinto do registo institucional de
   nós. Instalar o Ocinye OS nunca enrola uma máquina. `PROVIDER_MODE_IMPLEMENTED = FALSE`.
5. Nós comunitários não são confiáveis por omissão; fragmentar ou cifrar-e-dividir não é
   garantia de confidencialidade.

## Alternatives

- **Exigir GPU** — contradiz o §1-A (opera com zero GPU).
- **Enrolar na instalação** — mistura instalar com aderir a uma rede.

## Consequences

A D012 constrói enrolamento, identidade, runtime, agendamento, níveis, atestação,
medição e economia sobre os tipos da D011, sem mudar o Installer. Nenhuma telemetria.
