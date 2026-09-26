# ADR-0701 — O pacote de release e o instalador de anfitrião

- **Estado:** Accepted
- **Domínio:** Operations
- **Impacto:** HIGH
- **Depende de:** [ADR-0013](0013-general-purpose-os-instance-and-node.md) · [ADR-0700](0700-institutional-continuity-and-portability.md)
- **Data:** 2026-09-26

## Context

O Ocinye OS só estava instalado num sítio: a produção da Ocinye, montada à mão e
mantida por `scripts/deploy-production.sh`. Um sistema de uso geral tem de se
instalar num anfitrião Linux que ninguém da Ocinye preparou — sem GPU, sem chave
de IA, sem fornecedor externo — e de ficar igual à produção que se conhece, e não
a uma variante que só existe no dia da instalação.

## Decision

**1. O que se instala é um pacote de release**, produzido por
`scripts/release-bundle.sh` a partir de um commit exacto: a árvore (`git
archive`), as cinco imagens do release para uma arquitectura, o instalador, e
`SHA256SUMS`. O instalador confere as somas **antes** de escrever no anfitrião.
Não há `curl | sh`: o pacote viaja inteiro, e as somas publicam-se à parte.

**2. O instalador é `install/ocinye`**, com `install`, `status` e
`uninstall --purge`. Produz **o mesmo layout da produção** — `/etc/ocinye/*.env`
(0600), `/srv/ocinye/releases/<sha>`, `/srv/ocinye/current`, o mesmo Compose, o
mesmo proxy, o `ocinye.service` do systemd. Uma Instância instalada e a produção
da Ocinye são o mesmo produto.

**3. Os segredos nascem no anfitrião**: palavra-passe da base de dados, credencial
do armazenamento e a raiz de selagem. Nenhum valor por omissão, nenhum segredo no
pacote. A credencial temporária do primeiro administrador aparece uma vez no
terminal, ou num ficheiro 0600 quando a instalação é automatizada.

**4. A ordem é a do bootstrap**: anfitrião → pacote → release → configuração →
imagens → persistência → Instância e primeiro administrador (com perfil) →
serviços → saúde vista de fora (`https://<domínio>/` → `303`). A Instância é
criada antes de o Core arrancar, porque o Core recusa arrancar sem ela
([ADR-0013](0013-general-purpose-os-instance-and-node.md)).

**5. Só o Workspace é público.** O proxy gerado serve um anfitrião; o Core não tem
porta nem nome. TLS com certificado fornecido ou auto-assinado (para laboratório).
Sem `real_ip_header`: uma Instância genérica não tem edge conhecido, e confiar num
cabeçalho de IP de qualquer cliente seria deixá-lo escolher o seu endereço.

**6. O instalador é Bash, e não Rust.** Corre antes de existir qualquer binário do
Ocinye no anfitrião, e o que faz é orquestrar Docker, Compose e ficheiros — o
mesmo que os scripts de operação que já existem (`deploy-production.sh`,
`institutional-backup.sh`). Um binário Rust exigiria um pacote por arquitectura
só para correr `docker compose`. É a excepção documentada ao Rust-first
([ADR-0004](0004-rust-first.md)).

## Alternatives

- **Kubernetes/Helm.** Uma Instância é um anfitrião; nada o exige (§18).
- **Imagens num registry público.** Adiado para a Parte 17: exige assinatura e
  política de retenção. O pacote carrega as imagens por `docker load`.
- **Construir as imagens no anfitrião.** É o que a produção faz, e custa uma
  toolchain Rust e dezenas de minutos por instalação. O pacote traz-as feitas.

## Consequences

- `scripts/install-e2e.sh` prova-o: instala num anfitrião Linux limpo e
  descartável (`docker:dind`), entra por um browser por HTTPS — credencial
  temporária, palavra-passe, segundo factor, Aplicações do perfil, Ficheiros, uma
  Nota guardada, o Prompt sem fornecedor —, destrói, e repete de raiz com outro
  perfil.
- As imagens de terceiros (PostgreSQL, Redis, nginx, o espelho do MinIO)
  descarregam-se na instalação; uma instalação sem rede é trabalho futuro.
- O pacote não é assinado; as somas provam integridade, não origem (Parte 17).
