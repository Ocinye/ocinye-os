# ocinye-oie

O ambiente de instalação do ISO do Ocinye OS (D013, [ADR-0027](../../docs/adrs/0027-image-builder-and-formats.md);
[guia](../../docs/install/bare-metal.md)). Corre no sistema vivo (casper) em
`tty1` e na consola série, um de cada vez. `PROVISIONAL_PENDING_D011_CERTIFICATION`.

Escreve num só disco: o que o operador escolheu pelo caminho estável e cujo
token (fim do número de série) escreveu — e só depois disso. Classifica os
discos pelas regras de `ocinye_image_contracts::oie` (suporte de arranque,
montado, pequeno, sem caminho estável, ambíguo), confere o suporte contra os
factos embutidos na sua própria raiz, entrega ao curtin um esquema fixo (GPT,
ESP 1 GiB, ext4, sem swap, sem rede), confere o que ficou escrito e deixa o
diário em `/var/log/ocinye/oie-install.json`.

O instalador só corre num arranque `ocinye.mode=install`
([ADR-0030](../../docs/adrs/0030-installation-media-boot-modes-and-live-storage-policy.md)):
`flow::run` exige um `InstallAuthority`, que os outros modos não conseguem obter.
Em `live`, `hardware-check`, `recovery` e em modo em falta ou desconhecido,
`ocinye-oie run` é uma sessão de texto **provisória** (`session.rs`) que
inventaria, relata e reinicia, e que não tem como escolher um disco. A política
de armazenamento desses modos é verificada em `policy.rs` a partir do que o
núcleo reporta; se não se cumprir, a sessão fica restrita a metadados de
dispositivos.

`ocinye-oie probe` mostra a classificação dos discos, só de leitura;
`ocinye-oie policy` o veredicto da política de armazenamento deste arranque.

Testes: `cargo test -p ocinye-oie` (o fluxo com entrada guionada: uma
confirmação errada nunca chega a escrever).
