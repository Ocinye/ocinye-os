# `ocinye-observability`

Logging estruturado e correlação de pedidos.

## Finalidade

Todos os runtimes do Ocinye OS — Core, Worker, Node Agent, Workspace —
inicializam o logging por aqui, para que uma operação possa ser seguida através
de fronteiras de processo por um único identificador de correlação.

## Conceitos

- **Request id** — identifica um pedido HTTP.
- **Correlation id** — segue uma operação lógica através do Workspace, do Core e
  do Worker, e futuramente de um nó computacional.

Ambos são devolvidos ao chamador, para que um membro que reporte um problema
possa citar um identificador que localiza as linhas de log correspondentes.

## O que nunca é registado

Passwords, tokens, cookies, documentos completos, conteúdos de datasets e
prompts. `redact` e `SENSITIVE_FIELDS` existem como rede de segurança para
valores que cheguem a um sítio de log apesar dessa regra — **não** como licença
para os registar.

## Segurança relevante

`CorrelationIds::from_headers` só adopta um identificador vindo do cliente se
este parecer um identificador emitido por nós. Ecoar entrada arbitrária para os
logs tornaria trivial a injecção em logs; um valor hostil é substituído por um
novo, não sanitizado.

## Execução e testes

```bash
cargo test -p ocinye-observability
```

6 testes, incluindo a rejeição de valores hostis com quebras de linha, sequências
de escape ANSI e comprimento excessivo.
