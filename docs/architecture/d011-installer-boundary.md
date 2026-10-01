# D011 installer boundary (D010)

D011 = instalador gráfico remoto: corre na máquina do operador e instala num servidor dedicado.
A D010 não desenha nem implementa esse instalador. Deixa contratos tipados para que a D011 **nunca**
escreva na base de dados directamente.

## O que a D010 fornece (Core / runtime, tipado, auditado)

| Operação | Forma | Invariante que fica no Core |
|---|---|---|
| Distribuições suportadas | `Distribution::ALL` · `ocinye distributions list --json` | quatro, fechadas |
| Configurar activadas | `ocinye instance distributions set research,business` · `POST /instance/distributions/{d}/enable` | ≥ 1 activada; activar não mexe em membros |
| Primeiro administrador | `bootstrap-admin --distribution <d>…` (substitui `--profile`) | recebe acesso a todas as activadas |
| Pontos de acesso | `ocinye endpoint add --host … [--distribution d] [--canonical]` · `verify` · `list --json` | nome válido, único, um canónico activo |
| Validar a Instância | `ocinye instance check --json` | activadas ≠ ∅; canónico existe; fixos apontam para activadas |
| Saúde / verificação | `/health`, `/boot` (BootVm), `ocinye health --json` | só leitura |

## O que a D011 consome e faz (fora da D010)

Ligação SSH, preflight do servidor, instalação de pacotes, emissão e renovação de certificados,
geração/escrita da configuração do proxy (`server_name` a partir de `endpoint list`), instruções de
DNS, progresso e recibo da instalação.

## Específico do servidor / runtime

`OCINYE_TRUSTED_PROXIES` (CIDR), caminhos de certificados, unidades systemd, compose.

## Não é seguro manipular directamente

`instance_distributions`, `member_distribution_access`, `access_endpoints`, `member_desktop_layouts`,
`member_app_pins`, `audit_*`. Toda a escrita passa pelo Core (auditoria + invariantes).
