# Actualizações do Ocinye OS

> **A ISO instala o Ocinye. O Update Engine fá-lo evoluir.** Uma actualização
> normal de uma máquina instalada nunca pede a ISO outra vez.

Decisão: [ADR-0032](../adrs/0032-ocinye-update-engine.md). Esta pasta descreve
o sistema como está construído, e o que falta para o pôr em produção.

| Documento | Para quê |
|---|---|
| [Publicar releases](publishing.md) | `ocinye-release`: chaves, canais, preparar, publicar, pacotes offline |
| [Migrações](migrations.md) | expandir → migrar → contrair, e o que a reversão pode e não pode |
| [Cerimónia de chaves](key-ceremony.md) | como nascem as chaves de produção — ainda não aconteceu |
| [Servidor de actualizações](update-server-deployment.md) | runbook da máquina que servirá o repositório — ainda não existe |

## Estado

| | |
|---|---|
| Motor, contratos, repositório, publicador | `IMPLEMENTED`, testados em integração contra um repositório TUF real servido por HTTP |
| Imagens | toda a imagem instalável traz o motor, a camada nativa versionada e a raiz pública do seu canal |
| Definições › Actualizações, CLI `ocinye-system update …` | `IMPLEMENTED` |
| Assinatura | **`RELEASE_SIGNING = DEV_ONLY`**: chaves de desenvolvimento geradas localmente; nenhuma chave de produção existe |
| Repositório público | não existe; o endereço é configuração da máquina, vazio por omissão |
| Base Ubuntu | manutenção de pacotes das fontes aprovadas, **sem reversão atómica**; mudança de versão maior `DEFERRED` |

## As peças

```text
Definições › Actualizações ─┐
ocinye-system update …  ────┼─▶ System Service ─▶ /run/ocinye/update.sock ─▶ ocinye-update-engine
Nye (proposta tipada)  ─────┘   (autoriza: classe        (root; diário; TUF;     │
                                 de cada capacidade)      tranca de manutenção)   │
                                                                                  ▼
                       repositório TUF estático (HTTP/HTTPS/espelho) ou pacote offline .ocu
```

- **`crates/ocinye-update-contracts`** — os tipos: versão `<id>-r<n>`, número
  monotónico, manifesto estrito, fases e transições, diário, configuração,
  razões tipadas, protocolo do motor.
- **`crates/ocinye-update-repository`** — TUF pelo `tough` (sem criptografia
  nossa): chaves, raiz e rotação, preparação, publicação, verificação, pacotes
  offline, transporte HTTP.
- **`services/update-engine`** — o motor. Dedicado, e não dentro do System
  Service, porque a activação reinicia o System Service.
- **`services/release-tool`** — `ocinye-release`, o publicador.
- **`services/system-service`** — a única porta: `update.*` e `base.*` chegam
  tipados, são autorizados e passam ao motor.

## Na máquina

| Caminho | O quê |
|---|---|
| `/usr/share/ocinye/update/trust/<canal>/root.json` | a raiz TUF confiada (só chaves públicas), da imagem |
| `/etc/ocinye/update.json` | canal, repositório, verificação e descarga automáticas |
| `/var/lib/ocinye/update/` | diário, histórico (50), auditoria, metadados TUF confiados, descargas |
| `/usr/lib/ocinye/releases/<versão>/` | camada nativa de cada release guardado |
| `/usr/lib/ocinye/current`, `previous` | ponteiros trocados por `rename(2)` |
| `/run/lock/ocinye-maintenance.lock` | uma manutenção de cada vez: actualização, base, instalador |

Retenção: o release activo e o anterior (alvo de reversão); o resto recolhe-se.

## Uma actualização

```text
IDLE ─check─▶ UPDATE_AVAILABLE ─download─▶ DOWNLOADING ▶ VERIFYING ▶ STAGED ▶ AWAITING_INSTALL
AWAITING_INSTALL ─install─▶ INSTALLING ▶ ACTIVATING ─saúde─▶ COMPLETED
                                                    └falha▶ ROLLING_BACK ▶ ROLLED_BACK
                                         └reinício pedido▶ AWAITING_REBOOT ▶ BOOT_VALIDATING ▶ COMPLETED
qualquer estado ─não é seguro decidir─▶ RECOVERY_REQUIRED
```

1. **Verificar** — os metadados do canal contra a raiz confiada: assinaturas,
   limiares, expiração, versões que nunca recuam. Só se oferece um release do
   canal, da arquitectura da máquina, com número maior que o instalado e cuja
   origem mínima a máquina cumpre.
2. **Descarregar** — retomável; cada alvo verificado por tamanho e soma antes
   de ser usado.
3. **Pré-verificar** — espaço, política de migração, protocolo do sistema.
4. **Instalar** — com a palavra-passe local do operador (`LOCAL_REAUTH`):
   a Instância pelo `install/ocinye upgrade` **do release novo** (checkpoint da
   base, troca, portão de saúde), depois a camada nativa, depois o reinício das
   unidades por ordem.
5. **Portão de saúde** — System, Session e ecrã activos, e a Instância saudável
   quando existe. Falha: reversão automática — ponteiros de volta e a Instância
   devolvida ao release anterior pelo instalador, com a base reposta se o
   esquema mudou.
6. **Compromisso** — histórico, retenção, e o motor reinicia-se no release novo.

Cada transição é gravada no diário (temporário + `fsync` + `rename` + `fsync`
do directório) **antes** do efeito que a segue. Ao arrancar, o motor reconcilia
o diário com o disco: nada a fazer, retomar, descartar, completar, reverter, ou
`RECOVERY_REQUIRED` com a razão. Um corte de energia em qualquer passo deixa um
release inteiro — o anterior ou o novo —, nunca metade; a matriz está nos
testes do motor (`services/update-engine/tests/engine.rs`).

## Recuperação

- **Definições › Actualizações › Voltar ao anterior** (operador, palavra-passe
  local).
- **Menu de arranque: `Ocinye OS - release anterior`** — arranca com
  `ocinye.update=rollback`; o motor reverte uma vez nesse arranque, com o menu
  como actor na auditoria. O menu do sistema instalado aparece três segundos, e
  sempre depois de um arranque falhado.
- **Suporte de instalação em modo Recuperação** — mostra o diagnóstico só de
  leitura; na v1 não abre o disco instalado, e diz onde está a reversão.
- **`ocinye-system update rollback`** — como root, na consola.

## Offline

Um `.ocu` é o repositório de um só release num arquivo, produzido por
`ocinye-release bundle`. Copia-se para `/var/lib/ocinye/update/inbox/` ou para
um suporte montado em `/media/…`, e importa-se (`update.import`): passa pela
mesma verificação contra a raiz instalada, e segue o mesmo caminho daí em
diante.

## Base Ubuntu

`base.status`, `base.check`, `base.apply`: `apt-get` só das fontes aprovadas
(as da imagem, com `signed-by`), sob a mesma tranca, com recuperação de um
`dpkg` interrompido e detecção de reinício pedido. **Não há reversão atómica de
pacotes da base**, e as Definições dizem-no. A versão maior do Ubuntu não muda
pelo motor (`BASE_OS_MAJOR_UPGRADE = DEFERRED`).

## Provas

- `cargo test -p ocinye-update-engine` — N → N+1, assinatura errada, alvo
  alterado no espelho, metadados antigos e congelados, retoma, offline,
  reversão automática e manual, reinício e validação de arranque, corte de
  energia em cada passo, entrada de recuperação.
- `cargo test -p ocinye-update-repository` — publicação, rotação, verificação.
- `scripts/image-e2e.sh update NOME ARCH` — numa máquina instalada a partir da
  ISO (o disco do cenário `window`), contra repositórios de
  `scripts/update-e2e-repos.sh`: raiz estranha recusada, release sintético
  doente revertido sozinho, N+1 pelas Definições, pacote offline com corte de
  energia a meio.
