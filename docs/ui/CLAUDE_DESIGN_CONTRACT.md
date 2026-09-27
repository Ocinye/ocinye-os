# O contrato técnico da UI nova

> Para quem escreve a UI nova no repositório — o Claude Design incluído. O desenho
> é teu; estas são as fronteiras que o produto já provou e que a UI nova não pode
> partir. Uma alteração que as parta não entra, por mais bonita que seja.

## Onde se escreve

| | |
|---|---|
| Ramo | `ui/claude-design`, criado a partir de `chore/ui-reset-foundation`. **Nunca** a `main`, nunca produção |
| Vistas | `apps/workspace/src/ui/` — Rust, [Leptos](https://leptos.dev) em SSR, macro `view! { … }`. Um ecrã é uma função que recebe o que o Core devolveu e produz HTML no servidor |
| Estilo | `apps/workspace/static/` — CSS servido pelo próprio Workspace |
| Comportamento no browser | `apps/workspace/static/app.js` — JavaScript sem framework, carregado same-origin |
| Textos | `apps/workspace/src/i18n/catalog.rs` — cada texto em `pt`, `en` e `fr` |

A UI legada **não se reaproveita**: nada de restilizar as classes `oc-*` antigas nem
de partir dos componentes de `ui/components/`. O [inventário](LEGACY_UI_INVENTORY.md)
diz o que é legado.

## O que não pode mudar

1. **Rotas.** Cada caminho que o Workspace serve hoje continua a existir, com o
   mesmo método. Um ecrã novo pode mudar a composição, não o endereço.
2. **Formulários.** O `action`, o `method` e o `name` de cada campo são o contrato
   com o servidor. Um formulário que envie outro nome de campo deixa de funcionar
   sem erro visível.
3. **Marcadores `data-oc`.** São os pontos por onde o `app.js` e as 126 viagens de
   browser encontram as coisas. Mantêm-se no elemento que tem o mesmo papel.
4. **Autorização.** O Workspace **nunca decide** o que alguém pode fazer: mostra o
   que o Core devolveu. Esconder um botão não é autorização, e mostrar um que o
   Core recusaria é um defeito.
5. **Estados honestos.** Vazio, a carregar, indisponível, recusado e erro são
   estados diferentes, e cada um diz o que é. Uma falha do Core nunca aparece
   como zero nem como lista vazia.
6. **Textos pela via i18n.** Nenhuma frase escrita directamente numa vista; uma
   chave em falta em `en` ou `fr` fecha a CI.
7. **Conteúdo de membros** (títulos, corpos de notas e de correio) é conteúdo, não
   texto da interface: não se traduz, e o HTML que vem do Core entra já
   higienizado, nunca como HTML cru novo.

## A política de segurança do conteúdo (CSP)

O Workspace envia, e a UI nova tem de caber nela:

```
default-src 'none'; script-src 'self';
style-src 'self' https://fonts.googleapis.com; font-src https://fonts.gstatic.com;
img-src 'self' data:; connect-src 'self'; frame-src 'self';
form-action 'self'; base-uri 'none'; frame-ancestors 'none'
```

Na prática:

- **Nenhum `style="…"`** num elemento, e nenhum `<style>` inline: o browser
  descarta-os. Posicionamentos e variantes fazem-se com classes.
- **Nenhum `<script>` inline** nem `onclick="…"`: o comportamento vive em
  `static/app.js`.
- **Nenhum recurso de outro domínio**, excepto as fontes do Google: ícones,
  imagens e bibliotecas são servidos pelo próprio Workspace. Um SVG decorativo
  entra como ficheiro em `static/` ou como `data:`.

## Acessibilidade

Navegação por teclado, foco visível, contraste, HTML semântico, `label` em cada
campo, e estado nunca comunicado só por cor. Os separadores mantêm
`aria-selected`, e a navegação `aria-current`.

## Como se sabe que está feito

Um ecrã novo está feito quando `./scripts/verify.sh` passa: as viagens de browser
entram, carregam, escrevem e confirmam pelo produto. Uma viagem que falhe porque
um marcador mudou de sítio corrige-se no ecrã, não no teste, excepto quando a
viagem estava a medir apresentação — e aí regista-se em
[`LEGACY_UI_INVENTORY.md`](LEGACY_UI_INVENTORY.md).
