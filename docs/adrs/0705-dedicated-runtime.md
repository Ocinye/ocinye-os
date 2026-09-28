# ADR-0705 — Ocinye Dedicated: uma configuração de implantação, não um sistema operativo

- **Estado:** Proposed
- **Domínio:** Operations
- **Impacto:** MEDIUM
- **Depende de:** [ADR-0018](0018-universal-web-access-and-runtime-classes.md) · [ADR-0702](0702-desktop-shell-technology.md) · [ADR-0704](0704-desktop-instance-connection-and-updates.md)
- **Data:** 2026-09-27

## Context

Algumas organizações querem postos onde o Ocinye é o ambiente principal:
liga-se a máquina, aparece o ecrã de bloqueio do Ocinye. A tentação é fazer uma
distribuição Linux. Não é isso que falta.

## Decision

```text
anfitrião mínimo suportado (uma distribuição Linux fixada, WebKitGTK conhecido)
   └ sessão gráfica de um utilizador sem privilégios
        └ Ocinye Desktop em arranque automático, reiniciado se falhar
             └ ecrã de bloqueio da Instância → Desktop em Full Workspace
```

- **O anfitrião governa hardware, kernel, controladores e processos.** O Ocinye
  não se apresenta como sistema operativo do hardware.
- **Sem root.** A casca corre como utilizador comum; arranque e reinício por
  unidade de utilizador (`systemd --user`) ou pelo gestor de sessão.
- **Política:** `desktop.autostart_enforced`, `desktop.full_workspace_enforced`
  e `browser.download_destination_enforced` vêm da configuração da Instância
  (D15 G-24); a preferência da pessoa aparece bloqueada com o selo
  ORGANIZAÇÃO.
- **Saída:** Full Workspace não é quiosque por omissão; a recuperação do posto
  pelo administrador do anfitrião fica documentada.
- **Nesta fundação:** documentação e scripts de referência para uma
  distribuição suportada; nenhuma distribuição nova.

## Consequences

- `OCINYE_DEDICATED_RUNTIME_READY` só com a distribuição suportada provada numa
  máquina real (arranque, sem root, reinício, bloqueio), e nunca «qualquer
  Linux».
