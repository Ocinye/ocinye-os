# ocinye-firstboot

O primeiro arranque, a reclamação e a consola de uma máquina instalada a partir
de uma imagem do Ocinye OS (D013, [ADR-0028](../../docs/adrs/0028-first-boot-and-claim.md)).
Binário estático (musl) em `/usr/lib/ocinye/ocinye-firstboot`.
`PROVISIONAL_PENDING_D011_CERTIFICATION`.

| Subcomando | Quem o corre |
|---|---|
| `init` | `ocinye-firstboot.service` (F1–F6), antes do SSH |
| `authorized-keys %u %f %t %k` | o sshd, como `ocinye-claim-akc`, para a conta `ocinye-claim` |
| `claim-serve --offered SHA256:…` | o comando forçado dessa conta, como root por uma regra sudoers |
| `confirm --claim ID` · `claim --provisioned --key FP` | `ocinye`, por `sudo -n` |
| `claim-timeout` | o temporizador transitório do prazo de confirmação |
| `console --vt` · `--serial` | `ocinye-console@tty1` e `@ttyS0`/`@ttyAMA0` |
| `status` | quem quiser: o estado sem segredos |

Estado: `/var/lib/ocinye-firstboot` (root `0700`, escrita atómica, um `flock`);
o código de emparelhamento só em `/run/ocinye-firstboot` (tmpfs, `0600`), nunca
no diário. Nunca cria uma Instância, contacta um serviço, instala pacotes nem
executa um comando recebido.

Testes: `cargo test -p ocinye-firstboot` (uma máquina simulada sob uma raiz
temporária); as provas na máquina real estão em `scripts/image-e2e.sh`.
