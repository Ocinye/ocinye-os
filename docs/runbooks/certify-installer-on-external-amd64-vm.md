# Runbook — Certificar o Installer (D011) numa VM `amd64` externa

**Quando:** existir uma VM Ubuntu Server 24.04 LTS Minimal `amd64` num fornecedor
de cloud, acabada de criar e descartável. É o único passo que falta à
certificação do D011.
**Quem:** quem detém a conta do fornecedor e este repositório.
**Estado deste procedimento:** `NOT_RUN`. O guião está escrito e revisto, mas
nunca correu contra uma VM externa; a primeira execução é também a sua prova. Se
o guião falhar por defeito próprio, o resultado é `INVALID`, não `FAIL`.

## Antes

- [ ] A VM é **nova** e **descartável**: Ubuntu Server 24.04 LTS Minimal,
      `x86_64`, no mínimo 2 vCPU · 4 GiB · 40 GiB. Nunca um servidor com dados.
- [ ] Uma conta com `sudo` sem palavra-passe (a que a imagem de cloud cria) e a
      sua chave SSH privada nesta máquina.
- [ ] As portas 22, 80 e 443 da VM alcançáveis a partir desta máquina.
- [ ] A impressão digital da chave `ed25519` do servidor, **lida na consola do
      fornecedor** (registo de arranque ou consola série), no formato
      `SHA256:…`. Nunca a que a própria ligação SSH apresenta.
- [ ] Esta árvore no commit de `main` que se quer certificar, limpa.

## Passos

1. Construir o pacote de release `amd64` desse commit:

   ```bash
   scripts/release-bundle.sh --platform linux/amd64
   ```

   O comando diz onde deixou o pacote.

2. Correr a certificação. Os cinco argumentos são o endereço da VM, a conta, a
   chave, a pasta do pacote e a impressão digital lida na consola:

   ```bash
   OCINYE_D011_EXTERNAL_VM_IS_DISPOSABLE=yes scripts/installer-certify-external.sh HOST UTILIZADOR CHAVE PACOTE IMPRESSAO
   ```

   O guião recusa uma VM que não seja Ubuntu 24.04 `x86_64`, que já tenha Ocinye
   ou que tenha contentores a correr; confere a chave do servidor com a da
   consola; corre o `preflight` e a instalação com o controlador real
   (`ocinye-installer-cli`), TLS auto-assinado e nomes `.test` resolvidos para a
   VM; amostra os argumentos de todos os processos durante a instalação; e corre
   a auditoria de segredos.

## Resultado

Uma linha: `RESULT d011-external-amd64 PASS|FAIL|INVALID`.

- `PASS` exige as três: o instalador sai com zero, o estado final é
  `INSTALLED_TEST_MODE`, e a auditoria não encontra nenhum segredo.
- A evidência fica em `~/.cache/ocinye-installer-test/external-<host>/`
  (`certification.log`, `preflight.jsonl`, `install.jsonl`,
  `measure-before.txt`, `measure-after.txt`, `secret-audit.txt`).

## Depois

- [ ] Com `PASS`: registar em [`docs/install/installer.md`](../install/installer.md),
      na secção «Cloud `amd64`», a data, o fornecedor, a imagem, o commit e as
      medidas; só então o D011 passa a `D011_CERTIFIED`.
- [ ] Destruir a VM no fornecedor.
- [ ] Com `FAIL` ou `INVALID`: não registar certificação; guardar a pasta de
      evidência e abrir a correcção.

## O que este procedimento não cobre

A viagem pela janela gráfica do Installer nessa VM (provada localmente em
`arm64`), TLS do operador com um certificado real, e ACME.
