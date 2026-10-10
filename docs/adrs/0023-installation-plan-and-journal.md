# ADR-0023 — Plano de Instalação imutável e diário de instalação no servidor

- **Estado:** Proposed
- **Domínio:** Foundation
- **Impacto:** MEDIUM
- **Depende de:** [ADR-0022](0022-graphical-remote-installer.md)
- **Data:** 2026-10-04

Importada do pacote D011 do Claude Design, no formato desta biblioteca.

## Context

Uma instalação remota pode ser interrompida — rede, portátil fechado, falha de uma fase.
Sem um registo do que foi feito, retomar é adivinhar, e apagar restos pode apagar o que não
é do Installer.

## Decision

1. Antes de qualquer mutação o controlador constrói um `InstallationPlan` (identidade do
   release, identidade do alvo com o hash da chave do anfitrião, configuração canónica,
   factos de hardware sem segredos, lista fixa de fases) e o seu `plan_sha256` sobre JSON
   canónico. Nada muda no alvo antes de o operador confirmar; o bootstrap só aceita
   `Execute` para esse hash. Qualquer mudança cria um plano novo.
2. O bootstrap escreve um diário por instalação (0600, root) com tempos de fase, códigos
   de falha, caminhos criados e `instance_created` — nunca segredos, credenciais ou
   valores de ambiente.
3. Retomar continua a partir do diário, pela classe de segurança da fase (A–E); a criação
   da Instância (P11) nunca se retoma às cegas — o estado do Core lê-se primeiro. Parar
   só acontece em pontos seguros.
4. `RemoveIncomplete` apaga só caminhos do Installer registados no diário, e só enquanto
   não existir Instância. Não é desinstalação.

## Alternatives

- **Retomar pelo estado observado do anfitrião** — ambíguo; não distingue o que é do
  Installer.
- **Plano mutável** — a confirmação do operador deixaria de significar alguma coisa.

## Consequences

Uma instalação interrompida descobre-se a partir de qualquer sessão posterior; o recibo
refere o `plan_sha256`; actualizações futuras podem referir o plano instalado.
