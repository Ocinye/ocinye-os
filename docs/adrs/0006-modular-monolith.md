# ADR-0006 — Ocinye Core como modular monolith

- **Estado:** Accepted
- **Domínio:** Foundation
- **Impacto:** HIGH
- **Refinado por:** [ADR-0007](0007-domain-boundaries-in-modules.md)
- **Data:** 2026-08-22

## Context

O Ocinye OS cobre muitos domínios institucionais. A tentação é dividi-los em
serviços desde o início. Contra isso pesam factos concretos: a instituição tem
hoje poucos membros, uma única base de dados canónica, e nenhuma necessidade
demonstrada de escalar componentes independentemente.

Ao mesmo tempo, o `CLAUDE.md` §17 exige que as fronteiras permitam extracção
futura de AI, Compute, Search, Storage e workers.

## Decision

O Ocinye Core é um **modular monolith**: um único binário (`core-server`) com
módulos verticais por domínio dentro da crate `ocinye-core`.

Cada módulo tem a mesma forma:

```
modules/<domínio>/
    mod.rs        API pública do módulo — a única superfície importável
    model.rs      linhas de persistência (privadas ao módulo)
    repository.rs SQL explícito
    service.rs    camada de aplicação: autorização, invariantes, eventos, auditoria
    README.md     finalidade, limites, o que não pertence ali
```

Regras de fronteira:

1. Um módulo importa outro **apenas através do seu `mod.rs`**. Tipos de
   persistência (`model.rs`, `repository.rs`) são privados ao módulo.
2. Toda a autorização acontece na `service.rs`, nunca na `api.rs` nem no cliente.
3. Comunicação assíncrona entre módulos usa **eventos de domínio** (ADR-0010),
   não chamadas directas de serviço, sempre que a operação puder ser diferida.

> **Refinamento.** Esta decisão previa um `api.rs` dentro de cada módulo. Ao
> implementar, verificou-se que isso torna o transporte parte do núcleo e
> contraria `CLAUDE.md` §3. O transporte foi movido para
> `services/core-server/src/routes/`; ver [ADR-0007](0007-domain-boundaries-in-modules.md).

Microserviços só quando existir necessidade concreta e documentada num ADR que
supersede este.

## Alternatives

| Alternativa | Porque foi rejeitada |
|---|---|
| **Microserviços desde o início** | Introduz consistência eventual, descoberta de serviços, tracing distribuído e failure modes de rede sem qualquer benefício actual. Proibido pelo `CLAUDE.md` §17 sem necessidade demonstrada. |
| **Monolito sem fronteiras internas** | Mais rápido de escrever, mas torna a extracção futura um rewrite. Contraria directamente o princípio de evolução (`CLAUDE.md` §71). |
| **Modular monolith com crates por módulo** | Fronteiras seriam garantidas pelo compilador em vez de por convenção — atraente, mas desproporcionado agora (ADR-0005). Reavaliável quando um módulo se tornar candidato real a extracção. |

## Consequences

**Positivas**

- Uma transacção de base de dados abrange a mudança de estado, o evento de
  domínio e o registo de auditoria — a garantia mais valiosa do sistema.
- Deployment e operação triviais na fase actual.
- Fronteiras documentadas permitem extrair AI, Compute ou Search depois.

**Negativas, aceites**

- As fronteiras são mantidas por convenção e revisão, não pelo compilador. Cada
  `README.md` de módulo declara explicitamente o que não pertence ali, e a
  revisão de código verifica-o.
- Componentes escalam juntos. Aceitável enquanto a carga for a de uma
  instituição com dezenas de membros.

## Referências

`CLAUDE.md` §17, §20, §71 · ADR-0005 · ADR-0010
