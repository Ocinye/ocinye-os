# Internacionalização do Ocinye OS

O Ocinye fala três línguas — **Português de Portugal (`pt`)**, **English (`en`)**
e **Français de France (`fr`)** — e o português é a língua **canónica**: define o
significado, e o inglês e o francês são projecções fiéis dele.

Este directório é a documentação permanente da arquitectura de idioma. Não é um
registo de traduções feitas página a página; é o contrato que qualquer
funcionalidade futura segue.

## Documentos

- [`CANONICAL_LANGUAGE.md`](CANONICAL_LANGUAGE.md) — o contrato de língua canónica
  e a invariante que nunca muda.
- [`GLOSSARY.md`](GLOSSARY.md) — a terminologia institucional nas três línguas.
- [`locale-inventory.json`](locale-inventory.json) — o inventário legível por
  máquina do catálogo (cresce a cada fatia).

## A arquitectura, em três peças

1. **O identificador** — [`ocinye_contracts::Locale`](../../crates/ocinye-contracts/src/locale.rs).
   Exactamente `pt`, `en`, `fr`. Variantes externas (`pt-PT`, `en-US`, `fr-FR`)
   normalizam-se à entrada; a persistência recusa-as. É um tipo partilhado pelo
   Core e pelo Workspace, sem I/O, compilável para `wasm32`.

2. **O idioma corrente** — um `tokio::task_local` no Workspace
   ([`i18n`](../../apps/workspace/src/i18n/mod.rs)). Um middleware resolve-o no
   início de cada pedido (cookie `oc_locale` → `Accept-Language` → canónico) e
   corre o resto dentro do escopo. Assim `t("chave")` lê a língua certa em toda a
   renderização SSR **sem fiar um parâmetro por centenas de assinaturas**.

3. **O catálogo** — [`i18n/catalog.rs`](../../apps/workspace/src/i18n/catalog.rs).
   Uma linha por chave, com o `pt` obrigatório e o `en`/`fr` a segui-lo. A chave
   descreve a **semântica** (`nav.home`), nunca a palavra. O macro `catalogo!`
   constrói as entradas; o portão de completude
   ([`completeness.rs`](../../apps/workspace/src/i18n/completeness.rs)) garante,
   em CI, que cada chave de produção tem as três línguas e que os marcadores de
   interpolação casam.

## A via, e só a via

Todo o texto de produto passa por `t(chave)` (uma mensagem), `tf(chave, args)`
(com interpolação) ou `tp(chave, n)` (plural). **Não há `if locale == "fr"`
espalhado pelos ecrãs, nem catálogos por página.**

## Conteúdo não é interface

O Ocinye traduz a **interface**. Não traduz o **conteúdo** que os membros
escrevem: títulos de projectos, notas, nomes de ficheiros, correio, nomes de
unidades. O idioma é apresentação — nunca muda autorização, estado de domínio,
identidade de recurso, nem conteúdo autoral.

## Fluxo de uma funcionalidade nova

1. Escrever a cópia canónica em português.
2. Dar-lhe uma chave semântica estável.
3. `pt` → `en` → `fr` no catálogo.
4. O CI verifica a paridade.
5. Os testes de ecrã verificam a disposição.

Uma funcionalidade com texto só numa língua está incompleta.
