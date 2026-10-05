# ocinye-image-contracts

Os contratos tipados das imagens do Ocinye OS (D013,
[ADR-0026](../../docs/adrs/0026-release-and-image-signing.md) a
[ADR-0029](../../docs/adrs/0029-image-machine-instance-identity.md)).
`PROVISIONAL_PENDING_D011_CERTIFICATION`: assenta na forma canónica e no
manifesto do release da D011 (`ocinye-installer-contracts`; PD-01/PD-02).

**O que pertence aqui:** tipos fechados (`deny_unknown_fields`) e as regras que
decidem — validação dos manifestos (`manifest`), inventário e portões do
construtor (`build`), identidade da máquina e factos do primeiro arranque
(`firstboot`), as transições da reclamação como funções puras (`claim`), a
elegibilidade dos discos e a confirmação escrita (`oie`), a gramática do
comentário assinado e a verificação de um release (`signing`), e o recibo da
origem (`receipt`). Os programas que correm numa máquina só as executam.

**O que não pertence:** E/S, processos, rede, o Core, o domínio, persistência.

Testes: `cargo test -p ocinye-image-contracts`. O feature `test-support` expõe
fixtures aos testes dos consumidores; nunca entra num binário.
