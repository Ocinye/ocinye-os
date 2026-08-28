# ADR-0402 — Higienização do HTML recebido por correio

- **Estado:** Accepted
- **Domínio:** Mail
- **Impacto:** MEDIUM
- **Data:** 2026-08-22
- **Relaciona-se com:** [ADR-0602](0602-workspace-ssr-progressive-enhancement.md)

## Context

O corpo de uma mensagem de correio é HTML escrito por quem a enviou. Não é
conteúdo da instituição, não passou por revisão, e quem o escreveu pode ser
hostil. É a entrada mais perigosa de todo o Ocinye OS: chega sem autenticação,
em volume, e o seu propósito natural é ser renderizado.

O `CLAUDE.md` §32 lista XSS e *poisoned documents* no modelo de ameaças. Este é
o sítio onde ambos se materializam.

## Decision

**Lista de permissões, nunca lista de proibições.** Higienização com `ammonia`
em `crates/ocinye-core/src/modules/mail/sanitize.rs`, contra um conjunto
explícito de etiquetas e atributos. Tudo o resto desaparece.

Removidos sem excepção: `<script>`, `<iframe>`, `<object>`, `<embed>`,
`<form>`, `<input>`, `<style>`, atributos `on*`, e qualquer URL cujo esquema não
esteja em `http`, `https`, `mailto`, `tel`, `cid`.

Uma lista de proibições foi recusada porque falha em silêncio: o vector que
ninguém previu passa, e só se descobre depois.

### Conteúdo remoto é bloqueado por omissão

`block_remote()` reescreve `src` para `data-oc-remote` e `cid:` para
`data-oc-inline`, e conta o que bloqueou. Uma imagem remota é um pedido HTTP
para um servidor de terceiros no momento em que a mensagem é aberta: diz a quem
enviou que foi lida, quando, e de que rede. É rastreio, e o padrão é não o
permitir.

O membro pode carregá-lo — explicitamente, por mensagem, com a consequência
escrita ao lado do botão. Fazer isso fica no audit trail.

### Um único `inner_html` em todo o Workspace

`apps/workspace/src/ui/screens/mail.rs` é o único ficheiro da interface que
injecta HTML que não construiu, e está documentado como tal. O CSS
(`.oc-mail__body *`) neutraliza `position` e `float` para que uma mensagem não
possa sobrepor-se aos controlos à volta.

## Alternatives

**Renderizar apenas texto simples.** Seguro e inutilizável: metade do correio
institucional real é HTML, e mostrar tags cruas não é uma alternativa.

**Renderizar num `<iframe sandbox>`.** Isolamento melhor. Recusado por agora:
exige uma origem separada para ser real, e sem isso dá uma falsa sensação de
segurança. Reavaliar quando existir domínio dedicado.

**Escrever o higienizador.** Contraria o `CLAUDE.md` §16-A: não se reinventa
infraestrutura madura, e um higienizador de HTML próprio é uma superfície de
ataque que ninguém audita.

## Consequences

- Doze testes de segurança cobrem a higienização, incluindo `javascript:`,
  `onerror=`, `<script>` aninhado, `data:` e domínios semelhantes.
- Um deles verifica **construções executáveis**, não substrings: `alert(` dentro
  de texto já escapado é inofensivo, e rejeitá-lo tornaria o teste ruidoso sem o
  tornar mais forte.
- `SanitizedBody` devolve `linked_domains` e `blocked_remote_count`, que a
  interface mostra. O membro vê para onde a mensagem liga antes de clicar.
