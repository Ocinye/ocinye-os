# Cerimónia de chaves de produção

> **Estado: `NOT_RUN`.** Nenhuma chave de produção existe. Todas as imagens e
> releases até hoje são de desenvolvimento (`RELEASE_SIGNING = DEV_ONLY`), com
> chaves geradas numa máquina de quem desenvolve, nunca no Git nem numa imagem.
> O canal `stable` falha fechado sem esta cerimónia.

> **Pré-requisito por cumprir.** O `ocinye-release` de hoje cria raízes com
> uma chave por papel e limiar 1 (`keys::create_root`). O quórum de raiz 2-de-3
> abaixo exige estender `root create` e `root rotate` para várias chaves e um
> limiar — trabalho a fazer, e a testar, antes da cerimónia. O formato TUF e o
> motor (`tough`) já verificam limiares; falta o publicador.

Este procedimento cria as chaves TUF dos canais `candidate` e `stable`
([ADR-0026](../adrs/0026-release-and-image-signing.md),
[ADR-0032](../adrs/0032-ocinye-update-engine.md)). É uma decisão humana e um
acto humano: nenhum agente a executa.

## O que se produz

| Papel | Chaves | Onde ficam |
|---|---|---|
| `root` | três pares, limiar 2 | três suportes offline cifrados, guardados por três pessoas diferentes, em dois locais |
| `targets` (release) | um par por canal | máquina de publicação dedicada, cifrada, sem outro uso |
| `snapshot`, `timestamp` | um par cada | podem ficar na máquina de publicação ou num serviço de assinatura online: não autorizam código novo |

Só as **chaves públicas** (o `root.json` de cada canal) entram no repositório e
nas imagens. O servidor de actualizações não recebe chave nenhuma.

## Antes

- Três pessoas presentes, identificadas, com a sua função escrita na acta.
- Uma máquina limpa e desligada da rede para a raiz (arranque de um suporte
  conhecido; sem disco persistente).
- Três suportes novos para a raiz; um suporte para levar as chaves públicas.
- O commit exacto do `ocinye-release` a usar, compilado e com a soma na acta.

## Durante

1. Na máquina offline, para cada guardião, gerar um par `root` com
   `ocinye-release keys init` num directório temporário cifrado; copiar só o
   par `root` desse guardião para o seu suporte.
2. Gerar, na mesma sessão, os pares `targets`, `snapshot` e `timestamp` de cada
   canal; destinam-se à máquina de publicação.
3. `ocinye-release root create` com as chaves públicas dos três guardiões e o
   limiar 2, expiração de um ano.
4. Ler em voz alta os identificadores de chave e a soma de cada `root.json`;
   escrevê-los na acta; as três pessoas assinam.
5. Apagar o directório temporário; desligar a máquina; os suportes ficam
   selados.

## Depois

- `ocinye-release channel init` na máquina de publicação, com o `root.json`.
- As imagens de produção levam esse `root.json` em
  `/usr/share/ocinye/update/trust/<canal>/root.json`; a construção recusa um
  canal `stable` sem ele.
- Rotação anual da raiz (`ocinye-release root rotate`), com o mesmo quórum.

## Compromisso de uma chave

| Chave | Resposta |
|---|---|
| `timestamp`/`snapshot` | rodar na raiz (quórum); as máquinas seguem a cadeia |
| `targets` | rodar na raiz; republicar os releases válidos; publicar a lista do que foi assinado pela chave comprometida |
| uma `root` abaixo do limiar | rodar a raiz com as duas restantes |
| `root` no limiar | não há recuperação remota: as máquinas precisam de um `root.json` novo por um suporte de confiança (ISO ou pacote offline assinado pela raiz antiga **antes** do compromisso) |
