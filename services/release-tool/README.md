# ocinye-release-tool

Ferramentas de quem publica o Ocinye OS. Correm na máquina de construção, nunca
numa máquina instalada.

| Binário | Faz |
|---|---|
| `ocinye-release-manifest` | escreve o `MANIFEST.json` de um pacote de release da D011 (`scripts/release-bundle.sh`): árvore, imagens por digest, instalador, somas ([ADR-0701](../../docs/adrs/0701-release-bundle-and-host-installer.md)) |
| `ocinye-release` | publica releases em repositórios TUF de actualização: chaves, raiz e rotação, canais, preparar, publicar, renovar, verificar, pacotes offline ([publicar](../../docs/update/publishing.md), [ADR-0032](../../docs/adrs/0032-ocinye-update-engine.md)) |

**Não pertence aqui:** aplicar releases (o motor), servir o repositório (um
servidor estático sem chaves), guardar chaves (o directório 0700 de quem
publica — nunca o Git, nunca uma imagem).

Testes: a lógica vive em `ocinye-update-repository` e é lá testada; este crate
é a linha de comandos por cima.
