# Viagens de browser: duas fontes de estado implícito

- **Escrito em:** 2026-08-31, depois de duas falhas de CI
- **Relaciona-se com:** [`apps/workspace/tests/browser.rs`](../../apps/workspace/tests/browser.rs)

Uma viagem de browser é a única prova que atravessa o produto inteiro, e por isso
é também onde o não-determinismo se esconde melhor: um teste que falha uma vez em
cinquenta parece ruído, e o instinto é repeti-lo.

> **Uma viagem não pode depender implicitamente nem do relógio da máquina nem de
> um estado transitório do browser.**

As duas classes abaixo custaram, cada uma, várias corridas vermelhas antes de
serem nomeadas.

## Tempo implícito

O sintoma é um teste que falha a certas horas do dia, ou em certas máquinas, e
passa nas outras.

A causa é haver **dois relógios**. O Calendário calcula os seus intervalos na
zona de quem observa — que vem do cookie `oc_tz`, escrito pelo browser. Um
fixture que criasse o evento noutra referência estaria a medir a diferença entre
as duas, e o resultado passava a depender do fuso da máquina onde a suite corre.

A resposta errada é escolher uma hora «longe da meia-noite». Meio-dia em Lisboa é
01:00 do dia seguinte em UTC+14 e 23:00 do anterior em UTC−12: reduz a
probabilidade, não remove a dependência.

A resposta certa é **declarar**:

```rust
harness.declarar_fuso("Europe/Lisbon");
```

Todas as páginas da viagem herdam esse relógio, e a propriedade passa de «espera-se
que criação e observação usem a mesma referência» para «esta viagem diz qual é a
referência». Os testes correm em `UTC`, `Europe/Lisbon`, `Pacific/Kiritimati`
(UTC+14) e `Etc/GMT+12` (UTC−12) — os extremos existem para impedir que uma
correcção futura volte a esconder a dependência.

O fuso vive no `Harness`, e por isso não escapa da viagem. Isso é estrutural, e
estrutural não é observado: há um teste que o observa.

## Estado transitório do browser

O sintoma é uma asserção aparentemente impossível — a URL é a do destino e o
conteúdo é o da origem.

A causa é que **durante uma navegação o DOM da página anterior ainda existe e
ainda é não-vazio**. Um helper que devolvesse «o primeiro conteúdo não-vazio»
devolve a página de onde se veio, com a URL para onde se vai.

Estável significa duas coisas ao mesmo tempo: o documento acabou de carregar
(`readyState === "complete"`) **e** duas leituras seguidas dizem o mesmo. A
segunda apanha o que a primeira não vê — um documento completo que ainda está a
ser substituído.

## Como se prova uma destas correcções

Não com repetições. Vinte passagens verdes com o defeito no sítio provam que a
corrida não aconteceu naquelas vinte vezes.

Prova-se tornando o fenómeno **determinístico** e verificando os dois sentidos:

```
defeito presente   → reproduz
correcção presente → não reproduz
```

Para o tempo, isso é correr em fusos extremos. Para o DOM, é impor latência à
rede pelo CDP até a janela da navegação ser larga e igual em qualquer máquina.

É a mesma disciplina de um controlo positivo: primeiro provar que o instrumento
detecta, e só depois confiar no que ele diz.

## E as duas metades de uma asserção

Quando se verifica que uma página é a certa, verificam-se duas coisas:

```
o destino  está
a origem   não está
```

Só a primeira deixa passar um falso verde — o conteúdo antigo que por acaso
contém parte do que se procurava.
