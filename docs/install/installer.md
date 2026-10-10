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

## Privilégio no servidor

O Installer liga como um utilizador normal e decide, pela sondagem, como o
bootstrap corre — um estado tipado (`Elevation`), nunca uma suposição:

| Estado | Quando | O que acontece |
|---|---|---|
| `SudoPassword` (`SUDO_PASSWORD_REQUIRED`) | o utilizador está no grupo `sudo`/`admin`/`wheel` e `sudo -n true` falha | a janela pede a palavra-passe (I05); ela vai pelo stdin, na primeira linha, e nunca para disco, diário, recibo ou argumentos |
| `SudoNoPassword` (`SUDO_PASSWORDLESS_AVAILABLE`) | `sudo -n true` passa | não se pede nada, e não se guarda segredo nenhum — nem vazio; o bootstrap corre com `sudo -n`, que **falha** em vez de esperar se a regra não valer |
| `Root` | `uid=0` | corre directamente |
| nenhum (`SUDO_UNAVAILABLE`) | nada disto | recusa (I27) |

Sudo sem palavra-passe **não** alarga a confiança: a chave do anfitrião continua
fixada, o protocolo continua fechado, e nada corre fora das fases tipadas.

## Retorno da D013 (fase A)

Três achados da fase A da D013 foram tratados na D011, antes da certificação
final (o pacote de Design da D013 não foi alterado):

- **D013_FEEDBACK_F01 — identidade imutável das imagens de execução.** Todas as
  imagens de terceiros de produção passaram a `etiqueta@sha256`, e o
  `MANIFEST.json` regista repositório, etiqueta e digest
  ([artefactos de terceiros](../deployment/third-party-artifacts.md)).
- **D013_FEEDBACK_REDIS — revisão de execução e de licença.** O Redis ficou
  congelado no que já corria, e a revisão está aberta
  ([dependência do Redis](../architecture/redis-runtime-dependency.md)):
  `REDIS_PUBLIC_REDISTRIBUTION_APPROVED = FALSE`,
  `D013_PUBLIC_IMAGE_RELEASE_BLOCKED_BY_REDIS_REVIEW = TRUE`.
- **D013_FEEDBACK_F07 — sudo sem palavra-passe.** Provado numa VM local com um
  operador sintético em `NOPASSWD` e palavra-passe bloqueada
  (`scripts/installer-vm.sh create … --sudo-nopasswd`), ao lado do caminho com
  palavra-passe, que continua provado.

## Fora da D011

ACME (`DEFERRED`); assinatura do release (`RELEASE_SIGNING = NOT_IMPLEMENTED`);
modo fornecedor de computação (`PROVIDER_MODE_IMPLEMENTED = FALSE`); outros
sistemas além do Ubuntu 24.04; desinstalar.

## Provas

Em VMs descartáveis, nunca em servidores reais: `scripts/installer-vm.sh`
(Lima, Ubuntu 24.04 Minimal) e `scripts/installer-e2e.sh` (o controlador real
contra o bootstrap real e um pacote real), com
`infra/installer-test/secret-audit.py` a procurar cada segredo em tudo o que fica
ou se imprime.

### Local `arm64` — 2026-10-05

VM Lima (`vz`, Ubuntu Server 24.04.5 LTS Minimal `arm64`, imagem
`release-20261001`), **alocação de teste** 2 vCPU · 4 GiB · 40 GiB, sem swap —
isto não é o mínimo do produto ([hardware](hardware-results.md)). Pacote de prova
`848a9249ac50`; o lado do servidor é igual ao do commit final.

- **Ciclo DEV** (quatro VMs): Docker em falta → instalado com a chave conferida;
  Docker compatível já presente → não reinstalado; `docker.io` → recusado sem
  mutação; resto de um `docker.io` removido → instalado; `ufw` activo → só
  80/443 `comment "ocinye"`, idempotente; `firewalld` e uma política `nftables`
  própria → acção externa, intocados; TLS do operador (válido, nomes em falta,
  SHA-1, curva EC por extenso) → os inválidos recusados localmente; interromper
  (`--kill-after-phase`) → reatar; parar → retomar; Ocinye já instalado →
  bloqueado; chave de anfitrião mudada → paragem antes de autenticar.
- **Viagem CLEAN** pela janela real (o `ui_bridge` sobre o mesmo dispatcher), de
  I01 a I18: 2 Distribuições, 3 pontos de acesso, auto-assinado; instalação em
  83 s; `ACTIVATION_PENDING [DNS]` → DNS controlado → «Verificar novamente» →
  `INSTALLED_TEST_MODE`; V01–V13, V12b, V16 e V-FW a passar; a credencial
  temporária entra uma vez, obriga a mudar a palavra-passe e o segundo factor, e
  depois é recusada; o serviço volta depois de um reboot.
- **Segredos**: 10 segredos, 1078 alvos (incluindo 3950 amostras dos argumentos
  de todos os processos durante a instalação), 0 ocorrências.

| Medida (VM CLEAN) | Antes | Depois | Diferença |
|---|---|---|---|
| Disco usado | 0,93 GB | 3,70 GB | **+2,77 GB** (imagens 2,32 GB) |
| RAM usada em repouso | 288 MB | 631 MB | **+344 MB** |
| RAM dos contentores Ocinye | — | ≈ 156 MB | Core 84 MB, PostgreSQL 50 MB, o resto ≤ 4 MB cada |
| Swap | 0 | 0 | — |
| Serviços a correr | 13 | 15 | `docker`, `containerd` |
| Pacotes | 276 | 289 | `docker-ce`, `docker-ce-cli`, `containerd.io`, `docker-compose-plugin` e as dependências `iptables`/`nftables` |

Serviços persistentes que o Installer acrescenta: `docker.service` e
`containerd.service` (o runtime), `ocinye.service` (`oneshot`, levanta o Compose
no arranque) e oito contentores (proxy, workspace, core, worker,
conversion-runner, redis, postgres, object-store). Nada mais fica a correr: o
bootstrap e a sua pasta temporária desaparecem no fim.

### Cloud `amd64`

Por fazer: é o portão final da certificação D011, e o único. Estado:
**`D011_READY_FOR_FINAL_AMD64_CERTIFICATION`** — o código está em `main`, os
portões do repositório passam, e a certificação é um comando, descrito no
[runbook](../runbooks/certify-installer-on-external-amd64-vm.md). O D011 **não**
está certificado: nenhuma VM `amd64` externa correu ainda.
