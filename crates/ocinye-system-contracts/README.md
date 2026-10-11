# ocinye-system-contracts

Os contratos tipados do **Ocinye System Service**
([ADR-0031](../../docs/adrs/0031-rust-system-authority-and-native-system-surfaces.md)):
a única definição do que se pode pedir a uma máquina com o Ocinye OS instalado.

> Rust controla o sistema; as interfaces nativas do sistema usam TypeScript,
> HTML e CSS para apresentar o sistema.

## O que pertence aqui

- `capability` — o catálogo fechado: cada capacidade com a sua família,
  autorização (`PRESENCE`, `MEMBER`, `OPERATOR`), prova fresca, classe de
  auditoria, dependência de hardware, e se é oferecida com a sessão bloqueada,
  pelo cliente sem ecrã e como proposta do Nye.
- `request` — um pedido por capacidade, com argumentos limitados e
  `Request::validate`; `Payload` e `Outcome`.
- `state` — o que o serviço reporta, sempre lido do sistema.
- `reason` — os códigos de recusa. A interface é dona da frase em `pt`, `en` e
  `fr`; o texto de uma ferramenta nunca é a mensagem.
- `event` — mudanças de estado empurradas pelo serviço.
- `wire` — tramas de comprimento limitado e o aperto de mão por versão.
- `bindings/` — as ligações TypeScript, **geradas** destes tipos.

## O que não pertence aqui

Nada que execute: nem adaptadores, nem sockets, nem política. E nenhuma
capacidade que receba um comando, um executável ou um caminho a abrir — há um
teste que mede isso (`nenhuma_capacidade_e_um_executor_generico`).

## Ligações TypeScript

Não se editam à mão. Depois de uma mudança deliberada ao contrato:

```bash
OCINYE_UPDATE_BINDINGS=1 cargo test -p ocinye-system-contracts --test bindings
```

O mesmo teste, sem a variável, falha quando as ligações versionadas diferem das
geradas — é o portão que corre na CI.

## Testes

```bash
cargo test -p ocinye-system-contracts
```
