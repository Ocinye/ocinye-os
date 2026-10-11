# infra/image

O que entra numa imagem do Ocinye OS e como se constrói (D013).

| Caminho | O que é |
|---|---|
| `base.json` | a base Ubuntu fixada: série, digests por arquitectura, soma do `SHA256SUMS`, impressão digital e soma do anel |
| `keys/` | chaves **públicas** de confiança ([README](keys/README.md)) |
| `rootfs/` | fragmentos copiados para a raiz: sshd (bloco `Match User ocinye-claim`), sudoers, cloud-init (lista de permissões), netplan por omissão, unidades systemd |
| `oie-rootfs/` | fragmentos **só** da raiz do OIE: a guarda de blocos do suporte (initramfs, udev, contenção do casper). Nunca numa raiz instalável: a inspecção offline recusa-a |
| `e2e/storage_proof.py` | a bancada da [Prova de Segurança de Armazenamento do Live](../../docs/install/live-storage-safety.md) |
| `build/provision.sh` | o que corre dentro da VM de construção, por fases (`common`, `virt`, `metal`, `oie`, `finalize`) |
| `builder/` | a VM Lima do construtor, o instalador do syft (`install-syft.sh`, escolhe o binário pela arquitectura **da VM de construção**, não a do alvo) e a soma fixada (`syft.sha256`) |
| `e2e/image_e2e.py` | o banco de ensaio das imagens (`scripts/image-e2e.sh`) |
| `tools/gen-console-strings.js` | gera as tabelas de texto das consolas a partir do `strings.js` do Design |
