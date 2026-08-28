# `ocinye-contracts`

Tipos canónicos institucionais do Ocinye OS.

## Finalidade

Uma definição por conceito institucional, partilhada por Core, Workspace, Worker
e Node Agent. `Classification`, `IdeaState`, `TechnicalRole`, `AiCapability`,
`Residency` — existem aqui, e apenas aqui.

Esta é a vantagem concreta do Rust-first ([ADR-0004](../../docs/adrs/0004-rust-first.md)):
a divergência entre runtimes passa a ser um erro de compilação, não um bug em
produção.

## Responsabilidades

- Enumerações e tipos de valor institucionais.
- DTOs de fronteira: envelope de erro, paginação.
- Representação estável em texto (`as_str` / `parse`) para persistência e wire.

## Limites

**O que pertence aqui:** tipos de valor, enumerações, identificadores, DTOs.

**O que não pertence aqui:**

- Decisões de política — vivem em [`ocinye-domain`](../ocinye-domain/README.md).
  Partilhar um *tipo* com o Workspace é seguro; partilhar uma *decisão de
  autorização* não é.
- Tipos de persistência, SQL, I/O de qualquer espécie.
- Segredos, configuração, lógica de servidor.

## Dependências

`serde`, `serde_json`, `uuid`, `chrono`, `thiserror`. Deliberadamente leve: esta
crate compila para `wasm32` e é a única que o Workspace precisa de partilhar com
o Core.

## Interfaces

Consumida por todas as outras crates do workspace. Não consome nenhuma.

## Configuração

Nenhuma.

## Execução e testes

```bash
cargo test -p ocinye-contracts
```

13 testes. Cobrem, entre outros: a ordenação de `Classification`, que o valor por
omissão **não** é `PUBLIC`, que a derivação nunca abre um artefacto, e que todas
as enumerações fazem round-trip pela sua representação estável.

## Segurança relevante

- `Classification::DEFAULT` é `INTERNAL`, nunca `PUBLIC`: o valor seguro ganha
  quando ninguém escolheu.
- `Classification::most_restrictive` garante que um artefacto derivado nunca fica
  mais aberto do que a sua origem.
- `Residency::default()` é `UNDECLARED`: o sistema nunca afirma residência que
  não foi declarada.
- `PageRequest::normalised` limita o tamanho de página em vez de rejeitar, para
  que um parâmetro malformado não seja um vector de negação de serviço.
