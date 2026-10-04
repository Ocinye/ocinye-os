# Ocinye OS Installer (D011)

O Installer instala um release do Ocinye OS num servidor **Ubuntu Server 24.04
LTS** novo, a partir do computador do operador. Decisões:
[ADR-0022](../adrs/0022-graphical-remote-installer.md) (controlador local,
bootstrap tipado), [ADR-0023](../adrs/0023-installation-plan-and-journal.md)
(plano selado, diário), [ADR-0024](../adrs/0024-installer-tls-and-endpoints.md)
(TLS e pontos de acesso),
[ADR-0025](../adrs/0025-hardware-discovery-and-compute-boundary.md) (hardware).

## As peças

| Peça | Onde | O que faz |
|---|---|---|
| Janela | `apps/installer` (Tauri 2, fora da workspace) | os ecrãs do Design; a webview só pode chamar os 40 comandos do Installer |
| Controlador | `crates/ocinye-installer-controller` | máquina de estados, SSH (só Ed25519/ECDSA, chave fixada), verificação do pacote e do TLS, plano, recibo |
| Contratos | `crates/ocinye-installer-contracts` | tipos partilhados: identificadores validados, `MANIFEST.json` canónico, preflight, plano, protocolo, diário, recibo |
| Bootstrap | `services/installer-bootstrap` (`ocinye-bootstrap`, musl estático) | corre no servidor, temporário; fala o protocolo fechado, executa as fases P01–P16 destacado (`setsid`) e escreve o diário |
| Manifesto | `services/release-tool` | escreve o `MANIFEST.json` do pacote (`scripts/release-bundle.sh`) |
| Core | `ocinye endpoint-seed`, `verify-schema`, `verify-instance`, `verify-endpoints`, `verify-admin-bootstrap` | semear pontos ligados e verificar só por leitura, dentro do contentor do Core |

Não existe operação genérica de execução remota: o bootstrap aceita só os
comandos do enum `Command` (`ocinye_installer_contracts::protocol`), e cada fase
chama programas fixos com argumentos tipados.

## O caminho

1. **Release** — o pacote verifica-se localmente (`MANIFEST.json`, somas, o hash
   do bootstrap).
2. **Servidor** — a chave do anfitrião mostra-se e o operador confia
   explicitamente; uma chave mudada é paragem.
3. **Privilégio** — `sudo` com palavra-passe, enviada só pelo stdin.
4. **Preflight** — só leitura: Ubuntu 24.04 positivo, arquitectura, recursos,
   portas, firewall, runtime de contentores, instalação anterior, GPU (descoberta,
   nunca exigida).
5. **Configuração** — Instância, Distribuições, pontos de acesso (DNS verificado
   da máquina do operador, nunca alterado), TLS (fornecido e validado, ou
   auto-assinado de teste), primeiro administrador.
6. **Plano** — selado com `plan_sha256`; o bootstrap só executa esse plano.
7. **Instalação** — Docker do repositório oficial com a impressão digital da
   chave conferida contra o manifesto (nunca `curl | sh`); `ufw` só abre 80/443
   com `comment "ocinye"` e nunca é desligado; as fases de `install/ocinye`
   chamadas uma a uma.
8. **Credencial** — o Core emite a credencial temporária do primeiro
   administrador; mostra-se uma vez, à ordem do operador, e nunca vai para o
   diário, o recibo ou a linha de comandos.
9. **Verificação** — V01–V13; estado do ciclo de vida:
   `INSTALLATION_INCOMPLETE` · `INSTALLATION_COMPLETE` · `ACTIVATION_PENDING` ·
   `INSTALLED_TEST_MODE` · `OPERATIONAL`.

Interromper não perde a instalação: reabrir o Installer encontra o diário, e
retomar continua pela classe de segurança de cada fase; a criação da Instância
nunca se repete às cegas.

## Fora da D011

ACME (`DEFERRED`); assinatura do release (`RELEASE_SIGNING = NOT_IMPLEMENTED`);
modo fornecedor de computação (`PROVIDER_MODE_IMPLEMENTED = FALSE`); outros
sistemas além do Ubuntu 24.04; desinstalar.

## Provas

Em VMs descartáveis, nunca em servidores reais: `scripts/installer-vm.sh`
(Lima, Ubuntu 24.04 Minimal) e `scripts/installer-e2e.sh` (o controlador real
contra o bootstrap real e um pacote real). Os resultados ficam no relatório da
D011.
