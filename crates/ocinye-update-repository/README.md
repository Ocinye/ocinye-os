# ocinye-update-repository

O repositório TUF das actualizações ([ADR-0032](../../docs/adrs/0032-ocinye-update-engine.md)),
sobre a biblioteca `tough` — nenhuma criptografia nossa.

**Pertence aqui:** chaves por papel e a raiz (`keys`), com rotação; a preparação
de um release a partir de um pacote da D011 e de uma camada nativa (`stage`); a
publicação num canal com `consistent_snapshot` (`publish`), e a renovação de
`snapshot`/`timestamp`; a verificação contra uma raiz confiada, local ou
remota, com expiração, versões que nunca recuam e alvos verificados por tamanho
e soma (`verify`); o pacote offline `.ocu` (`offline`); a extracção segura de
arquivos, com limites (`archive`); o transporte HTTP (`transport`).

**Não pertence aqui:** a máquina de estados nem o diário (o motor); a política
de que release oferecer (o motor); guardar chaves (a máquina de quem publica).

Limite: as raízes criadas têm limiar 1 (ver
[cerimónia de chaves](../../docs/update/key-ceremony.md)).

Testes: `cargo test -p ocinye-update-repository` — publicação, rotação,
assinatura errada, alvo alterado, metadados antigos, pacote offline.
