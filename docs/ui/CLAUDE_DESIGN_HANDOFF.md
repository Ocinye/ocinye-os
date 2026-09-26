# Coordenação com o Claude Design

**Desde 2026-09-26, durante o programa de generalização.**

```text
UI_STATUS              = DESIGN_IN_PROGRESS
VISUAL_SOURCE_OF_TRUTH = CLAUDE_DESIGN_PENDING
CURRENT_UI             = PRODUCTION_COMPATIBILITY_SURFACE
CLAUDE_CODE_SCOPE      = funcionalidade · arquitectura · APIs · Core · segurança ·
                         testes · infraestrutura
```

O redesenho completo da interface do Ocinye OS está a ser produzido à parte, no
Claude Design, e passará a ser a especificação visual e de interacção do
Workspace. Até essa entrega ser aprovada:

- **Não se redesenha** a barra lateral, a Home, o Login, os Ficheiros, o Prompt,
  as Definições, nem o lançador; não se cria sistema de design, tokens, ícones,
  tipografia ou biblioteca de componentes paralelos; não se faz refactor de CSS
  por estética.
- **O Workspace actual continua a ser o produto em produção**, e o harness dos
  testes E2E. Não se degrada, não se removem ecrãs que funcionam, e não se deixam
  marcadores de posição.
- **Correcções funcionais continuam** — botões, rotas, 404/500, estado,
  localização, acessibilidade, fugas de permissão, formulários, carregamentos,
  segurança —, mínimas e compatíveis com a interface actual.
- **Capacidades novas expõem contratos tipados** (aplicações, Instância e perfil,
  nós e capacidade, fornecedores de IA, segredos, saúde, actualizações) que a
  interface nova possa consumir sem reescrever o backend.
- Quando uma capacidade nova precisa de interface para ser provada, usa-se a
  **mais pequena** superfície compatível, com os componentes existentes, marcada
  como **transitória**.

## Superfícies transitórias

| Superfície | Porquê existe | Estado |
|---|---|---|
| Administração › Instância (`/admin/instance`) | activar/desactivar aplicações e mudar o perfil tinham de ser provados de ponta a ponta (ADR-0014) | transitória; componentes existentes; sem CSS novo |
| Aviso «aplicação não activa nesta instância» | a rota de uma aplicação inactiva tinha de dizer porquê | reutiliza o padrão `oc-notice` e um ícone existente |

## Quando o pacote do Claude Design chegar

Não se reescreve o frontend de imediato. Primeiro, uma **auditoria de integração**
contra rotas, componentes, registo de aplicações, contratos da API, autorização,
comportamento E2E, localização, acessibilidade e máquinas de estado, que produz
`UI_INTEGRATION_PLAN.md` com cada componente classificado como `REUSE`,
`RESTYLE`, `REFACTOR`, `REPLACE` ou `NEW`. Só depois se implementa — fielmente;
um desvio por acessibilidade, segurança ou impossibilidade técnica escreve-se
antes de se fazer.

## Os testes E2E

Continuam a correr contra a interface actual. Distinguem-se:

- **comportamento de produto** — têm de sobreviver ao redesenho;
- **apresentação actual** — poderão ser actualizados na integração.

Preferem-se selectores semânticos (`data-oc`) a selectores visuais.
