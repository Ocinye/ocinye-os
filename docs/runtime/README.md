# Runtimes do Ocinye OS

Por onde uma pessoa chega a uma Instância: **Web** (navegador, e a mesma Web
instalada como PWA), **Desktop** (casca nativa fina) e **Dedicated** (posto
cujo anfitrião arranca a casca). São runtimes, não perfis de Instância.

> **A instalação acrescenta ao Ocinye. Nunca é precisa para chegar ao Ocinye.**

| Documento | O quê |
|---|---|
| [`CURRENT_STATE.md`](CURRENT_STATE.md) | o que existe hoje, com evidência (discovery R0) |
| [`TARGET_ARCHITECTURE.md`](TARGET_ARCHITECTURE.md) | a arquitectura decidida e as fases R0–R13 |
| [`CAPABILITY_MATRIX.md`](CAPABILITY_MATRIX.md) | o que cada runtime pode, alvo e estado |

**Pertence aqui:** o modelo de runtimes, a fronteira de capacidades, a casca
Desktop, a PWA, o posto dedicado. **Não pertence:** o Browser (em
[`../browser/`](../browser/README.md)) nem o visual (pacote D15 em
[`../ui/`](../ui/)).

Decisões: [ADR-0018](../adrs/0018-universal-web-access-and-runtime-classes.md),
[ADR-0611](../adrs/0611-runtime-capability-boundary.md),
[ADR-0617](../adrs/0617-pwa-and-service-worker-policy.md),
[ADR-0702](../adrs/0702-desktop-shell-technology.md) a
[ADR-0705](../adrs/0705-dedicated-runtime.md). Estado: `PLANNED` — só a
discovery e as ADRs existem.
