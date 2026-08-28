# ADR-0012 — O plano realtime: uma ligação que dura, sobre uma autoridade que não

- **Estado:** Accepted
- **Domínio:** Foundation
- **Impacto:** HIGH
- **Data:** 2026-08-26
- **Assenta sobre:** [ADR-0011](0011-redis.md) · [ADR-0010](0010-events-outbox.md) · [ADR-0009](0009-postgresql-sqlx.md)
- **Complementa:** [ADR-0002](0002-deterministic-core-and-agentic-control-plane.md)

## Context

Todo o Ocinye foi construído sobre pedidos que começam e acabam. Um pedido HTTP
traz um portador, o Core resolve o principal, decide, responde, esquece. A
autorização é resolvida **por pedido**, e é essa brevidade que a torna correcta:
entre dois pedidos, uma revogação tem sempre onde acontecer.

Uma superfície de comunicação em tempo real quebra isso. A ligação abre uma vez
e fica aberta horas. Se a autoridade for resolvida quando o socket abre, uma
pessoa removida de uma conversa às 10h continua a recebê-la às 18h — não por um
defeito, mas porque nada voltou a perguntar.

Não existia plano realtime nenhum. Havia `OCINYE_REDIS_URL` na configuração, uma
string que nada lia, e o [ADR-0011](0011-redis.md) a dizer para que serve o
Redis quando existir. Não havia WebSocket, não havia SSE, e o sino de
notificações era renderizado no servidor quando a página carregava.

## Decision

**O plano realtime existe, é bidireccional, e não é autoridade sobre nada.**

### 1. Três camadas, com papéis que não se misturam

    PostgreSQL   verdade durável        o que aconteceu
    Redis        coordenação efémera    quem está, quem escreve, e para onde propagar
    WebSocket    transporte             como chega ao browser

Nenhuma das duas últimas decide o que é verdade. O Redis pode ser reiniciado e
não se perde uma mensagem; o socket pode cair e não se perde uma mensagem.

### 2. Persistir primeiro, publicar depois

Uma operação só é propagada **depois** de a Core Operation canónica a ter
persistido com sucesso. Nunca ao contrário, e nunca em paralelo.

Se o `publish` falhar depois do `commit`, a operação **continua válida**: o
cliente recupera-a por reconciliação no `reload` ou no `reconnect`, a partir do
PostgreSQL. Não há retrocesso, não há compensação, e não se inventa uma
transacção entre dois sistemas que não a sabem fazer.

    commit  →  publish  →  entregue
    commit  →  publish falha  →  ainda verdade, chega no reconnect
    commit falha  →  nada publicado

### 3. `Pub/Sub` é sinalização; PostgreSQL é recuperação

O Redis `Pub/Sub` não garante entrega — não guarda, não repete, não confirma.
Tratá-lo como garantia seria construir sobre uma promessa que ele nunca fez.
O que ele dá é latência baixa para quem está a ouvir naquele instante. Quem não
estava, lê a base.

### 4. A autoridade é reestabelecida, não recordada

É a decisão central deste ADR, e a razão de ele existir.

> **Identity may persist. Authority must be re-established.**

Concretamente, numa ligação que dura:

- **subscrever** um canal exige autorização verificada nesse momento — conhecer
  um identificador nunca é suficiente para o receber;
- **cada envio** por esse canal reverifica antes de propagar;
- a autorização obtida quando o socket abriu **não é reutilizada** para decidir
  o que se entrega meia hora depois.

Uma remoção de participação retira acesso realtime, e retira-o sem esperar que
o cliente se desligue.

### 5. Comandos duráveis convergem nas mesmas operações

Se um comando durável entrar pelo socket, atravessa **exactamente** a mesma
Core Operation que a entrada HTTP atravessaria — a mesma autorização, a mesma
auditoria, a mesma persistência. O socket é uma porta, não um atalho.

### 6. Durável e efémero são coisas declaradas, não coisas parecidas

| Sinal | Onde vive | O que significa |
|---|---|---|
| `MessageCreated` | a mensagem está no PostgreSQL | notificação de algo que já é verdade |
| `ReadStateChanged` | a leitura está no PostgreSQL | idem; o sinal transporta, a operação decide |
| `PresenceChanged` | Redis, com TTL | ninguém precisa de saber quem esteve online ontem |
| `TypingChanged` | Redis, com TTL curto | não é um facto institucional; é um gesto |

`typing` **nunca** entra no PostgreSQL nem no registo de auditoria. Guardar quem
estava a escrever e desistiu seria guardar uma hesitação.

### 7. O que expira, expira sozinho

Presença e `typing` desaparecem por TTL, e não por um adeus educado do cliente.
Um browser que fecha, uma rede que cai ou um portátil que adormece não mandam
aviso nenhum — desenhar para o adeus é desenhar para o caso que não acontece.

### 8. Uma pessoa não são três presenças

Várias ligações da mesma identidade agregam-se numa presença só. Três separadores
abertos são três sockets e uma pessoa.

### 9. O Redis em baixo degrada, não derruba

Sem Redis: o Core continua de pé, o histórico continua legível, as operações
duráveis continuam a acontecer pelo caminho canónico. O que se perde é
propagação instantânea, presença e `typing` — e a interface **diz isso**, em vez
de mostrar uma lista vazia com ar de normalidade.

A prontidão institucional passa a ter o realtime como componente factual, e um
componente em baixo não é o Core em baixo.

### 10. O plano agentic não fala com nada disto

    Agent → typed capability → Core Operation → persistência → fanout

O plano agentic não abre sockets, não publica no Redis, e não sabe que eles
existem. O Workspace também não escreve no Redis.

### 11. As sessões continuam onde estão

`session.rs` diz que mover a sessão para Redis é `PLANNED`. Continua a ser.
O Redis passar a existir operacionalmente não é razão para mudar onde vivem as
sessões — são duas decisões, e juntá-las seria aproveitar uma obra para fazer
outra que ninguém pediu.

## Alternatives

**SSE com fanout em processo.** Simples, sem serviços novos, e com `reconnect`
dado pelo browser. Rejeitada por uma limitação estrutural e não por gosto: com
dois processos do Core, uma mensagem enviada num não chega ao outro. Serviria
hoje e teria de ser substituída à primeira instância a mais — incluindo em
testes que levantem dois Cores.

**SSE com Redis.** Resolve o fanout distribuído e deixa por resolver a direcção
de volta: `typing`, batimentos de presença e confirmações de leitura sobem do
cliente. Acabaríamos com `SSE` para descer e `POST`s para subir — dois
transportes a simular um.

**WebSocket sem Redis.** O mesmo problema do primeiro caso, com mais código.

**Só PostgreSQL, com `LISTEN/NOTIFY`.** Considerada a sério: existe, é
transaccional, e dispensaria o Redis. Rejeitada porque `NOTIFY` prende uma
ligação da base por cada subscritor e porque presença e `typing` — que são
escrita constante e descartável — não pertencem à base canónica.

## Consequences

**O que melhora.** O Ocinye passa a ter um plano de tempo real com contratos
tipados, e a autoridade sobre ele é resolvida quando é exercida, e não quando a
ligação abriu. Duas instâncias do Core comunicam.

**O que fica mais caro.** Mais um serviço a correr, a monitorizar e a falhar. E
uma superfície nova onde a autorização pode ser esquecida — cada canal novo tem
de declarar quem o pode ouvir, e a fronteira recusa por omissão.

**O que fica por decidir.** A política de retenção de mensagens, a federação
entre instituições, e se a presença deve ser visível fora da conversa. Nenhuma
destas é decidida aqui.

## Referências

`CLAUDE.md` §6, §25 · ADR-0009 · ADR-0010 · ADR-0011 · ADR-0002
