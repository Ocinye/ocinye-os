# ADR-0018 — Acesso Web universal e as três classes de runtime

- **Estado:** Proposed
- **Domínio:** Foundation
- **Impacto:** FOUNDATIONAL
- **Depende de:** [ADR-0013](0013-general-purpose-os-instance-and-node.md) · [ADR-0600](0600-leptos-workspace-runtime.md) · [ADR-0601](0601-workspace-bff-session.md)
- **Data:** 2026-09-27

## Context

O Ocinye OS chega hoje às pessoas por um único caminho: um navegador moderno
aponta ao Workspace de uma Instância (`os.ocinye.com`), que é servido por SSR e
fala com o Core pela sessão BFF. O programa de runtimes acrescenta uma casca
nativa (Ocinye Desktop) e um posto dedicado (Ocinye Dedicated), e um navegador
integrado.

O risco é conhecido de outros produtos: a aplicação instalada ganha
funcionalidades que a Web não tem, a Web passa a «versão reduzida», e ao fim de
dois anos há dois produtos com lógica duplicada — e uma Instância que só se
alcança bem com um instalador. Para uma instituição soberana, que pode ter de
abrir o seu sistema num computador emprestado, isso é inaceitável.

## Decision

### A frase

> **A instalação acrescenta ao Ocinye. Nunca é precisa para chegar ao Ocinye.**

### 1. Três classes de runtime, fechadas

| Runtime | O que é |
|---|---|
| `WEB` | a Instância num navegador moderno suportado; zero instalação. Inclui a mesma Web instalada como PWA — é a mesma classe, noutra janela |
| `DESKTOP` | uma casca nativa fina que hospeda o mesmo Workspace |
| `DEDICATED` | uma configuração de implantação do `DESKTOP`: anfitrião mínimo que arranca a casca como ambiente principal |

São **runtimes**, não perfis de Instância (ADR-0014): `research`, `business`,
`education` e `personal` dizem que aplicações a Instância tem; o runtime diz por
onde uma pessoa a alcança. Os dois eixos são ortogonais, e nenhum código decide
um a partir do outro.

`DEDICATED` não é outro frontend: é o `DESKTOP` com política de arranque e de
modo (ADR-0705).

### 2. A Web é a linha de base, e é completa

Tudo o que o Core governa — identidade, MFA, contextos, aplicações, Desktop,
Nye, Terminal, Ficheiros, Definições — funciona na Web, sem instalação. Uma
funcionalidade que a Web não pode dar (webviews externos, diálogos nativos,
`ocinye://` registado) **degrada, explica e oferece alternativa**; nunca parte a
aplicação nem a esconde.

### 3. Uma lógica, adaptadores por runtime

O Core, os contratos, o Workspace (rotas, ecrãs, i18n, RBAC apresentado) e os
módulos de cliente (Browser Manager, ocsh, Nye) são **um só código**. O que
muda por runtime passa por uma fronteira tipada de capacidades de runtime
(ADR-0611). Não existem `web-files`, `desktop-notes` nem ramos de negócio por
runtime.

### 4. A mesma autoridade

`WEB`, `DESKTOP` e `DEDICATED` usam a mesma autenticação da Instância, a mesma
sessão BFF, o mesmo MFA e o mesmo Core. Não há conta Desktop, nem conta global
Ocinye, nem directório central de Instâncias.

### 5. Prova constitucional

Uma viagem de browser, numa máquina sem nada instalado, abre o endereço da
Instância, entra, escolhe contexto e usa Desktop, Ficheiros, Notas, Nye
determinístico, Terminal e Definições. Essa viagem é condição de qualquer
release; um release que a parta não é candidato.

## Alternatives

- **Desktop como runtime principal, Web como «lite».** Recusado: faz da
  instalação uma condição de acesso e duplica lógica.
- **Runtimes como perfis de Instância.** Recusado: confunde onde se trabalha com
  o que a organização usa; uma Instância `business` pode ser aberta na Web e num
  posto dedicado ao mesmo tempo.
- **Aplicações nativas por plataforma.** Recusado: três bases de código de
  interface para uma equipa pequena, e a divergência é garantida.

## Consequences

- Cada funcionalidade nova declara o comportamento Web antes do Desktop.
- A matriz de capacidades (`docs/runtime/CAPABILITY_MATRIX.md`) diz `limitado`
  onde a Web é limitada; não se converte `limitado` em `sim`.
- A casca nativa é substituível: se desaparecer, o endereço da Instância
  continua a servir tudo.
