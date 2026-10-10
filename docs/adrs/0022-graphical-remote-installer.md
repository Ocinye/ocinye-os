# ADR-0022 — O Ocinye OS Installer: controlador gráfico local, bootstrap remoto tipado

- **Estado:** Proposed
- **Domínio:** Foundation
- **Impacto:** HIGH
- **Depende de:** [ADR-0701](0701-release-bundle-and-host-installer.md) · [ADR-0021](0021-installer-consumes-typed-contracts.md)
- **Data:** 2026-10-04

Importada do pacote D011 do Claude Design (`docs/adrs/0022-graphical-remote-installer.md`),
no formato desta biblioteca. Fica `Proposed` até à certificação final da D011.

## Context

`install/ocinye` instala um release num anfitrião Linux, mas exige que alguém entre no
servidor, copie o pacote e corra comandos. Quem instala uma Instância é um operador, não
necessariamente um engenheiro de sistemas.

## Decision

1. O Installer corre no computador do operador: uma janela Tauri 2 (UI só por classes, CSP
   igual à do Workspace) e um **Installer Controller** em Rust que detém a máquina de
   estados, o transporte SSH, a verificação do release e o recibo. Plataforma certificada
   primeiro: macOS; Windows/Linux portáveis, não certificados. Electron rejeitado.
2. O SSH é só transporte. A UI nunca vê uma shell. O controlador envia um binário
   temporário `ocinye-bootstrap` (do pacote, com a soma em `MANIFEST.json`) e fala um
   protocolo JSON por linhas com um **conjunto fechado de comandos**. Não existe
   `execute_shell`/`run_remote`.
3. O bootstrap não é um daemon permanente: vive numa pasta temporária 0700 durante a
   sessão, é removido no fim, e deixa só um diário sem segredos em
   `/var/lib/ocinye-installer/`.
4. A identidade do anfitrião fica fixada por servidor no armazenamento próprio do
   Installer; uma chave mudada é paragem dura; esquecê-la é uma acção explícita.
5. A instalação reutiliza a sequência de `install/ocinye`, chamada fase a fase, e
   subcomandos do Core; o Installer acrescenta transporte, diário, pontos de acesso e
   verificação.

## Alternatives

- **Script remoto (`curl | sh`)** — rejeitado: execução não tipada, sem verificação.
- **Electron** — segundo runtime, maior superfície, não Rust-first.
- **Agente permanente no servidor** — superfície de rede nova sem necessidade.

## Consequences

- O pacote de release ganha `MANIFEST.json` e `ocinye-bootstrap` (binário estático).
- Entram subcomandos do Core no anfitrião — `endpoint-seed` e quatro verificações
  só-leitura `verify-schema`, `verify-instance`, `verify-endpoints`,
  `verify-admin-bootstrap` —, com a mesma razão do `bootstrap-admin`: sem superfície de
  rede nova. As quatro verificações substituem o `instance-check` do Design.
- Alvo suportado na v1 (decisão de produto de 2026-10-04): **só Ubuntu Server 24.04 LTS**
  (Minimal, sem GUI); qualquer outro sistema é `BLOCKED` no preflight.
