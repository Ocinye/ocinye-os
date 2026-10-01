# ADR-0019 — Uma Instância, uma ou mais Distribuições

- **Estado:** Accepted
- **Domínio:** Foundation
- **Impacto:** HIGH
- **Depende de:** [ADR-0013](0013-general-purpose-os-instance-and-node.md) · [ADR-0014](0014-instance-profiles-and-application-activation.md) · [ADR-0624](0624-distribution-defaults.md)
- **Data:** 2026-09-30

## Context

ADR-0014 fez do perfil um atributo único da Instância (`organisations.profile`, um valor). A
ADR-0624 R2 fixou o alvo: uma Instância activa [1..4] Distribuições e o estado do membro é por
Distribuição. Hoje activar uma segunda é estruturalmente impossível e `PUT /instance/profile`
**substitui** a Distribuição.

## Decision

1. **Há um Ocinye OS e quatro Distribuições fechadas** (`ocinye_contracts::distribution::Distribution`):
   Research, Business, Personal, Education. Não são produtos, binários, Cores nem catálogos diferentes.
2. **Disponíveis ≠ activadas ≠ acessíveis.** Disponíveis = as quatro. Activadas = conjunto não vazio
   por Instância (`instance_distributions`). Acessíveis = activadas ∩ acesso do membro
   (`member_distribution_access`), decidido pelo Core.
3. **A Distribuição activa da sessão é uma de cada vez** e decide a experiência (predefinição do
   Desktop, fixações, fundo, primeiros passos, recomendações, activação de aplicações por
   `Distribution::activates`). **Não concede autoridade.**
4. **Acesso a uma Distribuição não é RBAC.** É uma porta de entrada tipada (`EntryDecision`). Não há
   `ROLE_RESEARCH` nem equivalentes. Dentro da Distribuição, papéis e permissões continuam a decidir.
5. **Invariantes:** pelo menos uma Distribuição activada (desactivar a última é **recusado**; não há
   estado de manutenção implícito); pelo menos um membro com `organisation.manage` tem acesso a uma
   Distribuição activada (retirar o último é recusado).
6. **Activar** acrescenta ao conjunto; não altera disposições nem fixações; dá acesso só a quem activou.
   **Desactivar** guarda tudo (linha, acessos, disposições, fixações), recusa entrada nova, invalida
   sessões activas nessa Distribuição no pedido seguinte, faz os pontos fixos nela recusarem (S12) e
   as ligações profundas para ela recusarem. Não há apagamento de Distribuição.
7. **Estado do membro por membro + Instância + Distribuição**: disposição (com o fundo lá dentro) e
   fixações. Chave no repositório: `(person_id, distribution)`, porque um membro pertence a uma Instância.
8. **Hierarquia de predefinições:** sistema → Distribuição[D] → Instância[I,D] → membro[M,I,D].
   A camada Instância[I,D] **continua por implementar** (FG-014, DEFERRED). «Repor» apaga a linha
   [M,D] da Distribuição activa e volta à predefinição efectiva **dessa** Distribuição.
9. **Aplicações:** a activação continua por Instância (`instance_applications`); sem decisão explícita,
   uma aplicação opcional está activa se **alguma** Distribuição activada a traz, e é **visível** numa
   sessão se a Distribuição activa a traz ∧ está activa ∧ o membro a vê. Nenhuma aplicação nova.
10. **Migração determinística:** `profile = X` → activadas = {X}; todos os membros com acesso a X;
    disposições e fixações existentes → Distribuição X. Nada é copiado para as outras três.

## Alternatives

| Alternativa | Porque não |
|---|---|
| Distribuição como papel | Funde entrada e autoridade; um administrador «Business» pareceria ter poderes. |
| Filtrar Distribuições só no Workspace | A URL directa passaria; o Core tem de decidir. |
| Copiar a disposição do membro para as quatro | Cria personalizações que o membro nunca fez e que depois divergem. |
| Permitir zero Distribuições como manutenção | Instância sem Desktop; confundível com falha; manutenção é outra capacidade (S35, DEFERRED). |

## Consequences

- `InstanceProfile` passa a alias de `Distribution` durante uma versão; `PUT /instance/profile` é
  substituído por `POST /instance/distributions/{d}/enable|disable` (auditados).
- `organisations.profile` fica só leitura uma versão (compatibilidade de rolagem) e sai depois.
- Portões de teste em `D010_CONTRACT_MATRIX.md`.
