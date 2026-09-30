# ADR-0624 — Predefinições de Distribuição: configuração de produto tipada, versionada e abaixo da Instância

- **Estado:** Proposed (D009)
- **Depende de:** ADR-0014, ADR-0016
- **Data:** 2026-09-30

## Decision

1. As predefinições de cada Distribuição (fixações, disposição do Desktop, fundo, primeiros passos, recomendadas) são **configuração de produto**, tipada em Rust (`experience::distribution::DEFAULTS`), versionada por `DISTRIBUTION_DEFAULTS_VERSION`, e nunca lida de ficheiro, caminho ou JSON.
2. Hierarquia efectiva: **membro ?? Instância ?? Distribuição ?? sistema**. A predefinição da Instância, quando existir (FG-014), ganha sempre à da Distribuição; «Repor» regressa à predefinição efectiva, nunca força a da Distribuição.
3. Uma actualização do produto nunca reescreve uma disposição gravada nem uma predefinição publicada pela Instância. Quem nunca personalizou segue a versão nova.
4. Uma Distribuição desconhecida cai na predefinição do sistema (sem widgets), nunca em Research.
5. Nenhum widget é obrigatório enquanto não houver fonte de dados para ele (`notice`, FG-013) — um obrigatório sem dados seria um cartão morto em todos os Desktops.
6. Nada disto é autorização. Fixações e widgets passam pelo mesmo filtro de visibilidade; um widget recusado pelo Core não se desenha.

## Consequences

- `ocinye_contracts::desktop::WIDGET_KINDS`: `notice` deixa de ser `mandatory`; `WALLPAPERS` ganha `field`, `module`, `calm`, `lattice`.
- `apps::default_pins_for` delega em `distribution::default_pins`; o caso especial «research + work» desaparece (passa a dado).
