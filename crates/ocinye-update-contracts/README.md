# ocinye-update-contracts

Os tipos das actualizações do Ocinye OS ([ADR-0032](../../docs/adrs/0032-ocinye-update-engine.md),
[docs/update](../../docs/update/README.md)). Sem I/O, sem criptografia: o que o
motor, o publicador, o System Service e a interface dizem uns aos outros.

**Pertence aqui:** a versão `<id>-r<n>` e o número monotónico; o manifesto de
actualização (analisador estrito: campos desconhecidos e chaves repetidas
recusados, nomes de alvos seguros); as fases e as transições permitidas
(`Phase::may_become`); o diário e o histórico (50 entradas); a configuração
(`/etc/ocinye/update.json`) e a validação do endereço do repositório; as razões
tipadas; o protocolo do socket do motor; o estado da manutenção da base.

**Não pertence aqui:** verificar assinaturas (`ocinye-update-repository`),
aplicar um release (`services/update-engine`), autorizar (`services/system-service`).

Testes: `cargo test -p ocinye-update-contracts`. Ligações TypeScript geradas
com as do System Service (`OCINYE_TEST_UPDATE_BINDINGS=1`).
