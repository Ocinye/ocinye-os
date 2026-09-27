# D7 · Nye

CSS: `static/ods-d7-nye.css`. Contratos existentes: `GET /ask?q=`, `POST /ask/plans/{plan_id}/execute`, `POST /ask/plans/{plan_id}/reject`, `/ai/prompt` (GET/POST). **O Nye funciona sem fornecedor de IA**: comandos determinísticos (abrir, procurar, organizar) vêm do plano do Core.

## Superfícies
| Superfície | Onde | Contrato |
|---|---|---|
| Campo na barra de topo | D2, `<form action="/ask" method="get">` `name="q"` | existe |
| Nye flutuante | Desktop, canto inferior direito, alinhado com o botão das apps | arrastável durante a sessão; **a posição não se guarda** até G-02 (volta ao canto em cada carregamento) |
| Popup circular | abre ao clicar no flutuante (não ao arrastar) | texto: `GET /ask?q=` devolvido como fragmento; a resposta aparece no círculo |
| Nye completo | `/ask` | existe; «Ver conversa completa» leva lá |
| Voz | botão do microfone | **G-07** → `aria-disabled="true"`, tooltip `ods.state.pending_contract`; nunca escuta sem clique |

## Estrutura
```html
<div class="ods-nye-float" data-oc="nye-float" data-ods-float>
  <button class="ods-nye-btn" data-oc="nye-open" aria-label="Falar com o Nye · arraste para mover">
    <span class="ods-nye-btn__glow"></span><span class="ods-nye-btn__ring"></span><span class="ods-nye-btn__track"></span><span class="ods-nye-btn__arc"></span>
    <span class="ods-nye-btn__core"><svg class="ods-icon"><use href="/static/ods-icons.svg#ods-nye"/></svg></span>
  </button>
</div>

<div class="ods-nye-orb" data-oc="nye-orb" role="dialog" aria-modal="true" aria-label="Nye" hidden>
  <div class="ods-scrim" data-oc="nye-close"></div>
  <div class="ods-nye-orb__circle">
    <span class="ods-nye-orb__ring ods-nye-orb__ring--track"></span><span class="ods-nye-orb__ring ods-nye-orb__ring--arc"></span><span class="ods-nye-orb__ring ods-nye-orb__ring--outer"></span>
    <button class="ods-iconbtn ods-iconbtn--round ods-nye-orb__close" data-oc="nye-close" aria-label="Fechar">…</button>
    <!-- o mesmo núcleo animado do botão, a 64px -->
    <div data-state="idle"><p class="ods-nye-orb__hello">Bom dia, Fidel</p><p class="ods-nye-orb__sub">Como posso ajudar?</p></div>
    <div data-state="listening" hidden><div class="ods-nye-wave"><span></span><span></span><span></span><span></span><span></span></div><p class="ods-label">A ouvir…</p></div>
    <div data-state="answer" hidden aria-live="polite"><p class="ods-nye-orb__q">«…»</p><p class="ods-nye-orb__a" data-tone="ok|warn|bad">…</p><a class="ods-btn ods-btn--sm ods-btn--ghost" href="/ask?q=…">Ver conversa completa</a></div>
    <form class="ods-nye-orb__field" method="get" action="/ask" data-oc="nye-form">
      <input name="q" placeholder="Escreva ou fale com o Nye…" autocomplete="off">
      <button type="button" class="ods-nye-orb__mic" data-oc="nye-voice" aria-disabled="true" aria-label="Falar">…mic…</button>
      <button type="submit" class="ods-nye-orb__send" aria-label="Enviar">…arrow-r…</button>
    </form>
    <p class="ods-nye-orb__hint">ENTER · ESC</p>
  </div>
</div>
```
- Sem JS o formulário navega para `/ask` (degrada bem). Com JS, o `app.js` faz `fetch` same-origin e mostra o estado `answer`.
- Plano que exige confirmação: a resposta mostra «Confirmar» / «Recusar» com os `POST` existentes de execute/reject.
- Tom: `bad` para política/recusa do Core, `warn` para «sem modelo compatível».
- O Nye não pode ser removido do Desktop. Movimento desliga-se com `prefers-reduced-motion`.
- Nye é masculino: «o Nye».
