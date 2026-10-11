# infra/image

O que entra numa imagem do Ocinye OS e como se constrói (D013).

| Caminho | O que é |
|---|---|
| `base.json` | a base Ubuntu fixada: série, digests por arquitectura, soma do `SHA256SUMS`, impressão digital e soma do anel |
| `keys/` | chaves **públicas** de confiança ([README](keys/README.md)) |
| `rootfs/` | fragmentos copiados para a raiz: sshd (bloco `Match User ocinye-claim`), sudoers, cloud-init (lista de permissões), netplan por omissão, unidades systemd, incluindo as do posto (ver abaixo) |
| `oie-rootfs/` | fragmentos **só** da raiz do OIE: a guarda de blocos do suporte (initramfs, udev, contenção do casper). Nunca numa raiz instalável: a inspecção offline recusa-a |
| `e2e/storage_proof.py` | a bancada da [Prova de Segurança de Armazenamento do Live](../../docs/install/live-storage-safety.md) |
| `build/provision.sh` | o que corre dentro da VM de construção, por fases (`common`, `virt`, `metal`, `oie`, `finalize`) |
| `builder/` | a VM Lima do construtor, o instalador do syft (`install-syft.sh`, escolhe o binário pela arquitectura **da VM de construção**, não a do alvo) e a soma fixada (`syft.sha256`) |
| `e2e/image_e2e.py` | o banco de ensaio das imagens (`scripts/image-e2e.sh`) |
| `tools/gen-console-strings.js` | gera as tabelas de texto das consolas a partir do `strings.js` do Design |

## O posto (ADR-0031)

Uma imagem instalável traz o System Service, o Session Service, o compositor e
a interface nativa do sistema, e as unidades que os ligam:

| Unidade | Papel |
|---|---|
| `ocinye-system.service` | a autoridade sobre a máquina (root), em `/run/ocinye/system.sock` |
| `ocinye-session.service` | a sessão local; root **sem capacidade nenhuma**, sistema só de leitura, rede só de loopback |
| `ocinye-seat-link.path` / `.service` | quando existe `/etc/ocinye/release.env` (há uma Instância instalada), aprende a chave pública de sessão pela porta de loopback do proxy, regista o endereço do Workspace e arranca o ecrã |
| `ocinye-display.service` | o compositor e a interface, como `ocinye-ui`, numa sessão do logind em `tty1`. **Nunca activada no arranque**: é a ligação que a arranca |

Enquanto a máquina não tem uma Instância ligada, `tty1` é da consola de texto
(`ocinye-console@tty1`): o código de emparelhamento aparece lá, e só lá. Uma
máquina sem dispositivo de ecrã fica com a consola de texto.

O Session Service fala com o Core por `127.0.0.1:8480`, a porta que o proxy
publica **só no loopback** (`infra/nginx/instance/20-machine.conf`) e por onde
só passa o que entrar numa máquina precisa.

Estado: `IMPLEMENTED`, **sem prova numa imagem construída** — o posto foi
provado numa VM com um Core de substituição (`infra/system-ui/bench/`), não
ainda com uma Instância instalada pela imagem.
