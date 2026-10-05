# Chaves públicas de confiança do construtor de imagens

Só chaves **públicas**. Nenhuma chave privada vive no repositório — a de
desenvolvimento fica em `~/.cache/ocinye-image-builder/dev-signing/` de quem
constrói, e a de produção não existe ainda (D013, ADR-0026).

| Ficheiro | O que é | Impressão digital | Como foi obtida |
|---|---|---|---|
| `ubuntu-cloudimage-keyring.gpg` | UEC Image Automatic Signing Key `<cdimage@ubuntu.com>`, que assina os `SHA256SUMS` de `cloud-images.ubuntu.com` | `D2EB 4462 6FDD C30B 513D 5BB7 1A5D 6C4C 7DB8 7C81` | `keyserver.ubuntu.com`, importada e conferida pela impressão digital a 2026-10-05; `gpgv` aceitou com ela o `SHA256SUMS.gpg` de `release-20261001` |

A soma do ficheiro está em `../base.json` (`keyring_sha256`): o construtor
recusa um anel que não seja este byte a byte, e recusa um `SHA256SUMS` que não
seja o fixado (`sums_sha256`) — mudar a série da base é uma alteração revista,
nunca uma descarga do que lá estiver hoje.
