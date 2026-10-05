# Redis: a dependência de execução, e a revisão que está por fazer

Estado: **`REDIS_RUNTIME_AND_LICENSE_DECISION_REQUIRED`** — registado a
2026-10-05, no endurecimento da D011, a partir do retorno da D013 (fase A).

Este documento diz o que o Ocinye OS pede ao Redis, para que a decisão de versão
e de licença se tome sobre factos. **Não contém conclusão jurídica nenhuma**, e
não escolhe versão nem substituto.

## Porque está aqui

O Compose usava `redis:7-alpine`, uma etiqueta flutuante. Em 2026-10-05 ela
servia `7.4.11` (digest `sha256:858f009f…3499`). O fabricante anunciou que, a
partir da série 7.4, as condições de licença deixaram de ser as da série 7.2 e
anteriores; quais se aplicam à redistribuição numa imagem pública do Ocinye OS
(D013) é a pergunta que a revisão responde.

Até lá, a D011 **congela** a imagem no que a etiqueta já servia —
`redis:7.4.11-alpine@sha256:858f009f9709ce576febc734aa78b8f6d624b82571f9ddb6bda4377c833b3499` —
para que a certificação funcional seja reprodutível. Isto **não** é uma escolha
de versão nem uma aprovação:

```
D011 functional certification  ≠  D013 public redistribution approval
REDIS_PUBLIC_REDISTRIBUTION_APPROVED          = FALSE
D013_PUBLIC_IMAGE_RELEASE_BLOCKED_BY_REDIS_REVIEW = TRUE
```

## O que o Ocinye pede ao Redis

| | |
|---|---|
| **REDIS_CURRENT_RUNTIME** | `redis:7.4.11-alpine`, por digest; um contentor, sem porto no anfitrião, só na rede interna do Compose; sem palavra-passe (`redis://redis:6379`) |
| **REDIS_CURRENT_USAGE** | só o **plano realtime** do Core (`crates/ocinye-core/src/realtime/`, [ADR-0011](../adrs/0011-redis.md) e [ADR-0012](../adrs/0012-realtime-plane.md)): propagação de eventos já guardados no PostgreSQL, presença (`oc:rt:vivo:*`, TTL 45 s), estado declarado (`oc:rt:declarado:*`) e «a escrever» (TTL 6 s). Quem liga: `ocinye-core-server`. O `worker` declara `depends_on` só para a ordem de arranque — não liga |
| **REDIS_REQUIRED_FEATURES** | `PUBLISH`/`SUBSCRIBE` (um canal por conversa e por pessoa), `SET` com `EX`, `SET`, `GET`, `DEL`, `KEYS` com padrão; RESP2 (cliente `redis` 0.32.7 com `connection-manager`); `PING` no healthcheck |
| Persistência | nenhuma exigida. `--save 60 1` só para o arranque não começar frio; `--appendonly no`. Não entra no conjunto de continuidade: o que se perde reconstrói-se do PostgreSQL |
| Cluster / réplicas | nenhum. Um nó |
| Durabilidade esperada | nenhuma. Não é fonte de verdade ([ADR-0011](../adrs/0011-redis.md)) |
| Falha | **não fatal**: sem Redis o Core arranca, o plano realtime fica degradado (ligação limitada a 3 s), e o que se perde é a chegada instantânea, a presença e o «a escrever» — a mensagem está guardada e aparece ao recarregar |
| **REDIS_MIN_COMPATIBILITY** | qualquer servidor que fale RESP2 e implemente os comandos acima com a semântica do Redis; nenhuma funcionalidade da série 7.4 é usada |
| **REDIS_REPLACEMENT_COMPLEXITY** | **baixa, tecnicamente**: um cliente, um módulo, sete comandos, sem dados a migrar e sem persistência a preservar. Trocar o servidor é mudar uma imagem e provar o plano realtime (os testes de `realtime` e a viagem das Mensagens). Qualquer troca é decisão de produto |
| **REDIS_LICENSE_REVIEW_REQUIRED** | **TRUE** |

## O que a revisão decide

1. Que série do Redis, ou que servidor compatível, o Ocinye OS executa e
   redistribui — com as condições de licença confirmadas por quem pode
   confirmá-las.
2. Se a resposta for outra imagem, a troca segue a regra das imagens de
   execução ([artefactos de terceiros](../deployment/third-party-artifacts.md)):
   versão escolhida com razão, digest fixado, plano realtime provado.

Até essa decisão, a D013 não publica uma imagem pública com o Redis dentro.
