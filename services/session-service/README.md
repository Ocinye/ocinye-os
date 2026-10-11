# ocinye-session-service

O **Ocinye Session Service**: a autoridade sobre a sessão local de uma máquina
com o Ocinye OS — entrar, bloquear, desbloquear (também sem o Core), sair e a
prova fresca que o System Service exige
([ADR-0031](../../docs/adrs/0031-rust-system-authority-and-native-system-surfaces.md)).

Não é um fornecedor de identidade. Quem é membro, e o que pode, decide o Core;
este serviço guarda apenas o que a máquina precisa para reconhecer, enquanto a
política da Instância o permitir, a pessoa que já entrou.

`ocinye-session-service serve` — o daemon. Escuta em `/run/ocinye/session.sock`
com o mesmo fio e o mesmo aperto de mão do
[`ocinye-system-contracts`](../../crates/ocinye-system-contracts/README.md).

## Regras

- **Entrar precisa do Core.** O Core responde com uma `SessionPolicy` assinada
  (Ed25519) para esta máquina; uma política mal assinada, de outra máquina ou
  de uma época de revogação anterior é recusada.
- **Desbloquear sem o Core** só se a política o permitir e dentro do prazo que
  ela fixa. O verificador local é Argon2id com custo calibrado na máquina, nunca
  abaixo do piso; a palavra-passe não é guardada.
- **Tentativas** têm espera crescente e um máximo; esgotado, a sessão termina.
- **Reinício do serviço** deixa a sessão bloqueada; **outro arranque** da máquina
  termina-a.
- **Prova fresca**: uma concessão vale para um actor, um pedido canónico exacto,
  uma vez, durante 120 s. Só o System Service (root) a pode consumir.
- **Quem liga** é o que o núcleo diz: root, a conta da interface do sistema, ou
  ninguém — e a ninguém não se responde nem o estado.

## Estado

`IMPLEMENTED`, sem prova numa imagem. A ligação ao Core (`core_link`) usa o
login e o segundo factor do próprio Core e pede a política assinada em
`POST /auth/machine-session`; o token da sessão do Core fica em
`/var/lib/ocinye/session/core-link.json` (0600) e nunca chega à interface. A
chave pública da Instância lê-se de `/etc/ocinye/instance-session.pub`
(hexadecimal); sem ela nenhuma política verifica e ninguém entra.
