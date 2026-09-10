# Ocinye Notes Editor

O editor estruturado de notas do Ocinye Workspace, compilado num único ficheiro
servido **same-origin**. É a única peça de JavaScript vendorizada do Workspace, e
existe porque a Experience de uma nota pede um editor de texto rico moderno,
enquanto o Core exige que o corpo canónico seja um **documento estruturado**
versionado (ADR-0413).

## O que é, e o que não é

- **É** uma camada fina sobre o [ProseMirror](https://prosemirror.net) — um motor
  de edição maduro, não um motor escrito de raiz (decisão da Slice A). O esquema
  em [`src/schema.js`](src/schema.js) é um espelho fiel do modelo do Core em
  `crates/ocinye-core/src/modules/knowledge/document.rs`.
- **Não** fala com o Core. As gravações vão para o BFF do Workspace, na mesma
  origem, que detém o token da sessão (ADR-0601). O editor detém o documento; só
  o Core o torna verdadeiro.

## Porque um ficheiro compilado no repositório

A CSP do Workspace é `script-src 'self'`, sem *nonce* e sem CDN. Um `<script>`
inline não executa, e um script externo só carrega da própria origem. Por isso o
editor e as suas dependências são compilados por [esbuild](https://esbuild.github.io)
num único *bundle* IIFE — sem `eval`, sem `import()` dinâmico, sem injecção de
`<style>` — e o resultado, [`../static/notes-editor.js`](../static/notes-editor.js),
é **versionado como um artefacto vendorizado**. A CI não o recompila; é tratado
como se fosse uma biblioteca de terceiros trazida para dentro.

## A fronteira de confiança

O conteúdo colado é **dado hostil** (CLAUDE.md §43, ADR-0413 §6). Duas defesas,
uma a seguir à outra:

1. **Aqui**, na colagem: o *parser* do ProseMirror só conhece os nós e marcas do
   esquema. Um `<script>`, um atributo `onclick`, um SVG ou uma ligação
   `javascript:` não têm representação no esquema e são descartados — a ligação
   perigosa é recusada por `getAttrs` e o texto sobrevive como texto simples.
2. **No Core**, na gravação: `NoteDocument::from_value` volta a validar tudo. O
   editor nunca é a autoridade sobre o que se guarda.

## Reconstruir o bundle

Depois de alterar qualquer ficheiro em `src/`:

```bash
cd apps/workspace/editor
npm ci
npm run build
```

Isto reescreve `../static/notes-editor.js`. Faz o *commit* do bundle na mesma
alteração que mexe na fonte — a fonte e o artefacto viajam juntos.

As versões das dependências estão fixadas em [`package.json`](package.json) e
travadas em `package-lock.json`. `node_modules/` não é versionado.

## Estrutura

| Ficheiro | Papel |
|---|---|
| `src/schema.js` | O esquema do editor — espelho do modelo do Core. A fronteira de colagem. |
| `src/serialize.js` | A tradução ProseMirror ⇄ `NoteDocument`. O contrato de ida e volta. |
| `src/editor.js` | A *view*, a barra de ferramentas, o teclado, as regras de entrada, a *checklist* clicável e a máquina de estados do autosave. |
| `src/index.js` | Ponto de entrada: monta cada `[data-oc-notes-editor]` da página. |
| `build.mjs` | A configuração do esbuild. |

## O autosave, em regra

`sujo → espera → A guardar… → (200 do Core) → Guardado`. Nunca «Guardado» antes
do 200. Uma falha mantém o conteúdo e oferece «Tentar de novo». Uma revisão base
obsoleta responde 409 e é uma recusa de sobreposição — o conteúdo fica, e é
preciso recarregar (ADR-0413 §5, §7).
