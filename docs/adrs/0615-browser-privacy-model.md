# ADR-0615 — Privacidade do Browser: histórico, marcadores, privado, telemetria

- **Estado:** Proposed
- **Domínio:** Workspace
- **Impacto:** MEDIUM
- **Depende de:** [ADR-0612](0612-browser-manager.md) · [ADR-0703](0703-desktop-trust-boundary-and-native-bridge.md)
- **Data:** 2026-09-27

## Context

O histórico de navegação diz muito de uma pessoa. Numa plataforma institucional
com auditoria, a tentação é juntá-lo ao que já se regista. Não é o mesmo
objecto: a auditoria prova acções governadas; o histórico é memória pessoal.

## Decision

- **Cookies e dados de sites:** locais ao runtime, na partição do Browser.
  Nunca passam pelo Core.
- **Histórico e marcadores:** locais ao computador (Desktop) ou ao navegador
  (Web, `localStorage` do Workspace, sem conteúdo, só URL e título) nesta
  fundação. Sincronizá-los com a Instância exige decisão de produto e ADR
  própria (D15 G-19), com dados privados do membro, sem acesso de
  administrador por omissão e retenção limitada pela Instância.
- **Privado:** partição efémera; nada entra no histórico normal; o Nye nunca lê
  uma página privada sem pedido explícito, só para esse pedido.
- **Auditoria:** nenhuma visita é auditada. Auditam-se acções governadas:
  guardar ficheiro externo no Ocinye, enviar página ao Knowledge, mudar política
  do Browser.
- **Telemetria:** contagens (falhas de webview, número de abas agregado), nunca
  URLs nem conteúdo, salvo política explícita.
- **Motor de pesquisa:** modelo de URL configurável e validado; independente dos
  fornecedores de IA.

## Alternatives

- **Histórico no Core desde o início.** Recusado: dados pessoais sensíveis sem
  modelo de privacidade escrito.
- **Histórico como auditoria.** Recusado: tornaria os administradores
  omniscientes sobre a navegação das pessoas.

## Consequences

- «Limpar histórico» é local e imediato; prova-se por E2E.
- A janela privada prova-se por E2E: cookie ausente após fechar e reabrir.
