# ocinye-system-client

O cliente do **Ocinye System Service**
([ADR-0031](../../docs/adrs/0031-rust-system-authority-and-native-system-surfaces.md)).
É por aqui que a interface nativa (o lado Rust de uma janela Tauri), o cliente
sem ecrã `ocinye-system` e os testes chegam ao serviço — e, por isso, pelo mesmo
contrato.

Não decide nada: faz o aperto de mão, envia um pedido tipado e devolve o
resultado tipado. Uma recusa do serviço não é um erro; é um `Outcome::Refused`
com o código da razão. Um serviço que fale outro contrato é recusado em
`Client::connect`, antes de qualquer capacidade poder ser chamada.

Não pertence aqui: política, estado, ou qualquer forma de alcançar o sistema que
não seja o socket do serviço.

```bash
cargo test -p ocinye-system-client
```
