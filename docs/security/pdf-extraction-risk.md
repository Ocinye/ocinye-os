# `ttf-parser`: um advisory alcançável por input não confiável

- **Estado:** aceite com mitigações, para a primeira produção
- **Revisto em:** 2026-08-30
- **Relaciona-se com:** [ADR-0205](../adrs/0205-content-extraction-and-lexical-body-search.md)

## O que é

```
ttf-parser v0.25.1
└── lopdf v0.42.0
    └── pdf-extract v0.12.0
        └── ocinye-core
```

`ttf-parser` está marcado como **não mantido**. Chega ao Ocinye OS por
`pdf-extract`, que é o leitor de PDF da extracção de conteúdo.

## Porque isto importa mais do que um advisory normal

Porque é alcançável por **input não confiável**. Qualquer pessoa autorizada a
carregar um ficheiro pode carregar um PDF, e esse PDF é lido por este código. A
distância entre um ficheiro que alguém envia e o parser é curta.

Não escondemos isto atrás de «não há CVE conhecido». Não mantido significa que
se houver um, ninguém o corrige.

## O que está feito

**A leitura não corre no caminho do pedido.** Corre no worker, a partir do
outbox. Um leitor que demore ou que rebente não segura ninguém à espera de uma
página.

**`catch_unwind`.** Um PDF hostil que faça o parser entrar em pânico é apanhado
e registado como `Leitura::Falhou`. O ficheiro continua guardado e legível para
download; o que falha é torná-lo pesquisável. Um pânico sem isto levaria o worker
consigo — e com ele o escoamento do outbox da instituição inteira.

**Limite de tamanho.** `MAX_SOURCE_BYTES = 128 MiB`. Acima disso não se tenta,
e o estado é registado em vez de se deixar a memória da máquina decidir por
acidente.

**Um formato sem leitor não é um erro.** É um estado da extracção, registado, e
o evento dá-se por entregue: repetir não mudaria nada.

## O que **não** está feito

**Não há sandbox.** O leitor corre no processo do worker, com os privilégios do
worker. `catch_unwind` apanha pânicos de Rust; não apanharia corrupção de
memória se alguma existisse.

**Não há timeout.** Um PDF construído para fazer o parser trabalhar durante
horas ocupa um worker durante horas. O limite de tamanho reduz a superfície, não
a elimina.

## Porque não se migrou já para o Capability Runtime

Porque a migração é grande e a razão seria «fazer o advisory desaparecer», que é
a razão errada. O Capability Runtime (ADR-0501) existe e corre convidados WASM;
mover a extracção de PDF para lá é o destino certo, e é trabalho que merece a sua
própria milestone com as suas próprias provas — não um desvio no meio da
primeira instalação de produção.

## Condição de saída

Esta aceitação deixa de valer quando **qualquer** destas se tornar verdade:

1. É publicado um CVE contra `ttf-parser` ou `lopdf` explorável por um documento.
2. `pdf-extract` passa a depender de um parser mantido, e basta actualizar.
3. A extracção de PDF passa a correr dentro do Capability Runtime.
4. Um PDF real faz o worker entrar em pânico em produção mais do que uma vez —
   o que indicaria exploração e não malformação.

Até lá, o advisory fica visível: o `cargo audit` da CI continua a reportá-lo, e
esta página existe para que ninguém o confunda com um problema resolvido.
