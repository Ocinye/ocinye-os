# ADR-0032 — O Ocinye Update Engine: releases assinados por TUF, aplicados sem reinstalar

- **Estado:** Proposed
- **Domínio:** Foundation
- **Impacto:** FOUNDATIONAL
- **Depende de:** [ADR-0026](0026-release-and-image-signing.md) · [ADR-0031](0031-rust-system-authority-and-native-system-surfaces.md) · [ADR-0701](0701-release-bundle-and-host-installer.md)
- **Data:** 2026-10-12

## Context

Uma máquina instalada a partir da ISO tem de evoluir sem voltar à ISO. Até aqui
havia duas peças soltas:

- o **pacote de release** da D011 (`MANIFEST.json`, `SHA256SUMS`, imagens,
  `install/ocinye`), cuja identidade é o SHA curto do commit, verificado só por
  somas, e o `ocinye upgrade` em bash que troca o release de uma Instância com
  checkpoint da base e portão de saúde;
- as **peças nativas** do posto (System Service, Session Service, compositor,
  interface do sistema, firstboot), fixas em `/usr/lib/ocinye/`, sem versão nem
  maneira de ser trocadas.

Nenhuma das duas sabe quem publicou um release, nem resiste a um espelho
comprometido, a uma reposição de metadados antigos ou a um corte de energia a
meio. As somas sobre HTTPS provam que os bytes chegaram como saíram do
servidor, não que o Ocinye os publicou.

## Decision

> **A ISO instala o Ocinye. O Update Engine fá-lo evoluir.** Uma actualização
> normal nunca pede reinstalação.

1. **Um serviço Rust dedicado, `ocinye-update-engine`** (root, unidade
   `ocinye-update.service`, socket `/run/ocinye/update.sock` só para root), é a
   única autoridade sobre releases instalados. Dedicado e não dentro do System
   Service porque a activação reinicia o System Service; um motor que vivesse
   nele matar-se-ia a meio da transacção. O System Service continua a ser a
   única porta: `update.*` chega-lhe tipado, é autorizado pela classe de cada
   capacidade (ADR-0031) e só então é passado ao motor. A interface, a CLI e o
   Nye pedem; nenhum decide.

2. **Identidade do release — não se cria outra.** O release continua a ser o da
   D011 (`release.id`, 12 hex, e `commit`). O manifesto de actualização
   (`ocinye-update-contracts::UpdateManifest`) acrescenta-lhe o que a D011 não
   tem: um **número de release monotónico** atribuído por quem publica, a
   **revisão** (`<id>-r<N>`, a mesma forma das imagens da D013), o **canal**, a
   versão mínima de origem, os componentes, a política de migração, o espaço
   necessário, se pede reinício, e a soma do `MANIFEST.json` da D011 que
   acompanha. Analisador estrito (`deny_unknown_fields`); falha fechado.

3. **Confiança: TUF, com a biblioteca `tough`** (AWS, MIT/Apache, usada pelo
   Bottlerocket) em vez de protocolo criptográfico próprio. Quatro papéis TUF:
   `root` (limiar configurável; rotação pela cadeia de `root.json`), `targets`,
   `snapshot`, `timestamp`. Daí vêm, sem código nosso: expiração, protecção
   contra reposição e congelamento, monotonia das versões, verificação de
   tamanho e de soma, rotação de chaves. Os papéis da ADR-0026 mantêm-se: a
   **raiz** é a raiz TUF; a chave de **release** assina `targets`;
   `snapshot`/`timestamp` podem viver online porque não autorizam código novo.
   Em desenvolvimento há uma chave por papel, gerada localmente, nunca no Git
   nem numa imagem. **`RELEASE_SIGNING = DEV_ONLY`** até haver cerimónia de
   produção.

4. **Canais: `development`, `candidate`, `stable`** — os da ADR-0026, não
   outros. Um repositório TUF por canal; a raiz confiada de cada canal viaja na
   imagem (`/usr/share/ocinye/update/trust/<canal>/root.json`, só chaves
   públicas). Uma imagem de desenvolvimento segue `development`; uma de
   produção, `stable`. Mudar de canal é capacidade de operador, auditada, e
   nunca aceita um número de release menor do que o instalado.

5. **O repositório é estático e não é autoridade.** Metadados TUF assinados e
   alvos imutáveis endereçados pelo conteúdo (`consistent_snapshot`), servidos
   por qualquer HTTP(S), armazenamento de objectos ou espelho. O servidor não
   tem chaves. O endereço é configuração da máquina
   (`/etc/ocinye/update.json`), nunca compilado. Um pacote **offline** (`.ocu`)
   é o mesmo repositório, de um só release, num arquivo: importado do suporte,
   passa pela mesma verificação contra a raiz instalada.

6. **Um release é um estado coerente**: o pacote da D011 (imagens, árvore,
   instalador, migrações) e a **camada nativa** (`native-<arch>.tar`: binários,
   unidades, interface). As peças nativas passam a viver em
   `/usr/lib/ocinye/releases/<versão>/`, com `/usr/lib/ocinye/current` e
   `previous` como ponteiros trocados por `rename(2)`; os caminhos estáveis
   (`/usr/lib/ocinye/ocinye-system-service`, …) são ligações para `current`.

7. **Máquina de estados duradoura.** O diário (`/var/lib/ocinye/update/journal.json`,
   escrito por ficheiro temporário + `fsync` + `rename` + `fsync` do
   directório) é a verdade; a interface nunca é. Cada transição é tipada e
   gravada antes do efeito que a segue. Ao arrancar, o motor **reconcilia** o
   diário com o que encontra no disco e decide uma de: nada a fazer, retomar,
   descartar, completar, reverter, ou `RECOVERY_REQUIRED` com diagnóstico.
   Nunca adivinha.

8. **Ordem de activação** (só depois de descarregado, verificado, preparado e
   aprovado na pré-verificação):
   1. tranca de manutenção (exclui outra actualização, a manutenção da base e o
      instalador);
   2. a Instância, se existir: o `install/ocinye upgrade` **do release novo**,
      com o seu checkpoint da base, troca e portão de saúde (o mecanismo já
      provado da D011, chamado sem shell, com argumentos fixos);
   3. a camada nativa: `previous ← current`, `current ← novo`, reinício das
      unidades por ordem (sessão, ecrã, System Service; o motor por último, só
      depois do compromisso);
   4. portão de saúde do sistema inteiro;
   5. se o release pede reinício: `AWAITING_REBOOT`, e o compromisso só vem
      depois de um arranque saudável — validado pelo próprio motor quando
      arranca (`ocinye-update.service`, depois do System e do Session Service),
      com três tentativas antes de reverter;
   6. `COMMITTED`, histórico, retenção.

9. **Reversão automática.** Se o portão falha depois da activação: ponteiros
   nativos de volta, Instância devolvida ao release anterior pelo próprio
   instalador (com o checkpoint da base quando o esquema mudou). Se não for
   seguro reverter: `RECOVERY_REQUIRED`, nunca meio release.

10. **Migrações: expandir → migrar → contrair.** Um release de funcionalidade
    só pode **acrescentar** esquema compatível com o release anterior durante a
    janela de reversão. Uma migração destrutiva declara-se (`contract`), só é
    aceite quando a origem já é um release que não depende do que ela retira, e
    a reversão por cima dela é a reposição do checkpoint, dita como tal. O
    manifesto diz a política; a pré-verificação recusa o que a viole.

11. **A base Ubuntu é outra coisa e diz-se.** Manutenção de pacotes da base
    (`base.*`): só das fontes aprovadas, sob a mesma tranca, com diário,
    recuperação de um `dpkg` interrompido e detecção de reinício pedido. **Não
    tem reversão atómica** e não se finge que tem. A mudança de versão maior do
    Ubuntu é `BASE_OS_MAJOR_UPGRADE = DEFERRED`.

12. **Retenção.** Ficam sempre o release activo, o anterior (alvo de reversão)
    e o pendente. O resto recolhe-se por regra tipada, nunca por pedido da
    interface.

13. **Recuperação a partir do menu de arranque.** O sistema instalado tem uma
    entrada `Ocinye OS - release anterior` (`/etc/grub.d/42_ocinye_previous`),
    que arranca o mesmo núcleo com `ocinye.update=rollback` — com o initrd quando
    existe, e sem ele (a imagem cloud `amd64` arranca assim) com a raiz por
    `PARTUUID`, como o `10_linux`; sem forma de nomear a raiz, não há entrada. O motor age sobre
    ela uma vez por arranque (marca o `boot_id` antes de agir): é a mesma
    reversão diarizada das Definições, registada com o menu de arranque como
    actor. Quem escolhe a entrada está à frente da máquina — a autoridade que o
    menu já exige —, e a entrada não dá mais nada: nem shell, nem outro
    parâmetro. Sem release anterior, recusa e não muda nada. O suporte de
    instalação, em modo Recuperação, não abre o disco instalado na v1 e
    encaminha para esta entrada e para as Definições.

14. **Motor do release anterior como recurso.** Se o motor de um release
    acabado de activar não consegue correr, `ocinye-update-fallback.service`
    (`OnFailure=`) arranca o do release anterior sobre o mesmo diário, que
    reconcilia e reverte a transacção em curso.

## Alternatives

- **`apt`/pacotes `.deb` próprios num repositório APT assinado**: resolveria a
  camada nativa, não a Instância (imagens, base de dados), e a assinatura do
  APT não protege contra congelamento nem dá limiares de raiz.
- **Imagens A/B do sistema inteiro (OSTree, RAUC, systemd-sysupdate)**: a
  resposta certa para um sistema imutável; o Ocinye OS v1 assenta numa base
  Ubuntu mutável com uma Instância em contentores e uma base de dados que não
  cabe numa partição A/B. Fica como direcção para uma v2.
- **minisign sobre `SHA256SUMS` (ADR-0026) também para actualizações**: prova a
  origem, não a frescura; um espelho podia servir para sempre um release antigo
  e vulnerável.
- **Implementar TUF à mão**: recusado; há uma implementação auditada e usada em
  produção.

## Consequences

- A primeira ISO com o motor é a base de todas as actualizações seguintes; uma
  máquina instalada antes dela não se actualiza sozinha.
- O motor liga-se ao `aws-lc-rs` pelo `tough`: constrói-se, como o compositor,
  no contentor da base Ubuntu para a arquitectura do alvo.
- O repositório público e as chaves de produção ficam por montar: código,
  formato e runbook estão prontos (`docs/update/`); falta a máquina e a
  cerimónia.
- Sem número de release no pacote da D011, quem publica atribui-o e o TUF
  garante que nunca recua.
- O publicador de hoje cria raízes de limiar 1, uma chave por papel; o quórum
  de raiz da produção exige estendê-lo antes da
  [cerimónia](../update/key-ceremony.md). O motor já verifica limiares.
- Uma reversão pedida depois do compromisso repõe a base do checkpoint: o que a
  Instância escreveu desde a actualização perde-se, e quem a pede é avisado
  ([migrações](../update/migrations.md)).
