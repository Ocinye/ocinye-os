# ocinye-update-engine

O motor de actualizações do Ocinye OS ([ADR-0032](../../docs/adrs/0032-ocinye-update-engine.md),
[docs/update](../../docs/update/README.md)): root, unidade
`ocinye-update.service`, socket `/run/ocinye/update.sock` só para root. O
System Service é o único cliente; a interface, a CLI e o Nye passam por ele.

**Pertence aqui:** verificar (TUF), descarregar com retoma, pré-verificar,
preparar a camada nativa, activar (Instância pelo `install/ocinye upgrade` do
release novo, ponteiros `current`/`previous`, reinício por ordem), portão de
saúde, compromisso ou reversão automática, validação depois de um reinício
(três tentativas), reversão manual, importação offline, manutenção da base
Ubuntu, a entrada de recuperação do menu de arranque (`ocinye.update=rollback`,
uma vez por arranque), a reconciliação do diário ao arrancar, a retenção.

**Não pertence aqui:** autorizar quem pede (System Service); assinar (o
publicador); desenhar (a interface).

| Módulo | |
|---|---|
| `engine` | a máquina de estados, o diário, as operações |
| `source` | repositório ou pacote offline, descarga retomável |
| `native` | camada nativa: extracção, ponteiros, unidades, retenção |
| `host` | o que o motor pede ao sistema (`systemctl`, `docker`, `apt-get`, `dpkg`, o instalador do release), sem shell |
| `base` | fontes aprovadas e pacotes pendentes da base |
| `store` | escrita atómica, tranca de manutenção |
| `server` | o socket |

Testes: `cargo test -p ocinye-update-engine` — contra um repositório TUF real
servido por HTTP e um anfitrião em memória: N → N+1, recusas, retoma, offline,
reversões, reinício, entrada de recuperação e um corte de energia em cada passo
(`Fault`). Na imagem: `scripts/image-e2e.sh update`.
