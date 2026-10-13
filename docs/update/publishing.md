# Publicar releases

`ocinye-release` (`services/release-tool`) é o único publicador. Corre na
máquina de quem publica, com as chaves dessa máquina; o servidor que serve o
repositório não tem chave nenhuma.

```bash
cargo build --release --locked -p ocinye-release-tool --bin ocinye-release
```

## O repositório

Um repositório TUF por canal (`development`, `candidate`, `stable`), estático:

```text
<repo>/<canal>/metadata/   <n>.root.json, timestamp.json, <n>.snapshot.json, <n>.targets.json
<repo>/<canal>/targets/    <sha256>.<nome> — alvos imutáveis, endereçados pelo conteúdo
```

Os alvos de um release são `releases/<versão>/manifest.json`,
`releases/<versão>/bundle/…` (o pacote da D011, ficheiro a ficheiro) e
`releases/<versão>/native/native-<arch>.tar` (a camada nativa).

| Papel TUF | Assina | Onde vive a chave |
|---|---|---|
| `root` | as chaves de todos os papéis, e a rotação | offline; limiar configurável |
| `targets` | o que é um release | máquina de publicação |
| `snapshot`, `timestamp` | frescura e coerência | podem viver online: não autorizam código novo |

## Uma vez por canal

```bash
ocinye-release keys init KEYS_DIR                       # 0700; um par por papel
ocinye-release root create --keys KEYS_DIR --out root.json --days 365
ocinye-release channel init --repo REPO --channel candidate --root root.json
```

O `root.json` (só chaves públicas) é o que as imagens desse canal levam em
`/usr/share/ocinye/update/trust/<canal>/root.json`.

## Cada release

```bash
scripts/update-native.sh target/<triplo>/release VERSAO NUMERO CANAL ARCH native.tar
ocinye-release stage --bundle PACOTE_D011 --native native.tar --out STAGED \
  --channel candidate --number NUMERO --minimum 0 --revision N \
  --migrations none|expand|contract:N \
  --notes-pt "…" --notes-en "…" --notes-fr "…" [--reboot] [--security]
ocinye-release publish --repo REPO --channel candidate --keys KEYS_DIR --staged STAGED
ocinye-release verify  --repo REPO --channel candidate --root root.json
```

- **Versão**: `<id do pacote>-r<revisão>`; o id é o do pacote da D011 (12 hex
  do commit). Uma nova revisão do mesmo pacote é outro release.
- **Número**: atribuído por quem publica, estritamente crescente no canal. O
  motor nunca oferece um número menor ou igual ao instalado, e o TUF garante que
  os metadados nunca recuam.
- **`--minimum`**: o número mínimo de origem; uma máquina mais antiga não vê o
  release.
- **`--migrations`**: ver [migrações](migrations.md).
- **`--synthetic PROPÓSITO`**: marca um release de teste; as Definições
  mostram-no como tal. Nunca num canal que não seja `development`.

`ocinye-release refresh` volta a assinar `snapshot`/`timestamp` antes de
expirarem; num repositório público, corre agendado.

## Pacote offline

```bash
ocinye-release bundle --repo REPO --channel candidate --version VERSAO --out release.ocu
```

O `.ocu` traz os metadados do canal e os alvos de um release. A máquina
verifica-o contra a raiz que já tem, como verificaria o repositório.

## Rotação de chaves

```bash
ocinye-release root rotate --repo REPO --channel C --old-keys VELHAS --new-keys NOVAS [--days N]
```

Publica `N+1.root.json` assinado pelas chaves antigas **e** pelas novas; as
máquinas seguem a cadeia de raízes. Uma raiz comprometida exige o procedimento
da [cerimónia de chaves](key-ceremony.md).

## Desenvolvimento

`scripts/image-build.sh build` publica o release de cada imagem no canal
`development` de `~/.cache/ocinye-update/repo`, com chaves em
`~/.cache/ocinye-update/dev-keys` (geradas uma vez, nunca no Git nem numa
imagem). `scripts/update-e2e-repos.sh` deriva daí os repositórios do cenário
`update` do E2E das imagens.
