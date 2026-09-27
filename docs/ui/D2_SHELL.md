# D2 · Casca (barra de topo e painéis)

CSS: `static/ods-d2-shell.css`. Substitui a apresentação de `ui/shell.rs`; **a filtragem por permissões, o registo de aplicações e as fixações ficam** (inventário: `MIXED_UI_AND_LOGIC`).

## Estrutura (ordem exacta, da esquerda para a direita)

```html
<div class="ods-shell" data-oc="shell">
  <header class="ods-topbar">
    <div class="ods-slot" data-oc="account">
      <button class="ods-topbar__logo" data-oc="account-toggle" aria-haspopup="menu" aria-expanded="false" aria-label="{nome}"><img src="/static/ocinye_logo.png" alt=""></button>
      <div class="ods-popover ods-popover--left ods-glass ods-account" data-oc="account-menu" role="menu" hidden>
        <div class="ods-account__head"><span class="ods-avatar">FM</span><div><p class="ods-account__name">{nome}</p><p class="ods-account__mail">{email}</p><p class="ods-account__inst">OCINYE OS · {PERFIL}</p></div></div>
        <a class="ods-menu__item" href="/settings" role="menuitem">…Conta</a>
        <a class="ods-menu__item" href="/settings" role="menuitem">…Definições</a>
        <a class="ods-menu__item" href="/help" role="menuitem">…Ajuda</a>
        <button class="ods-menu__item" data-oc="lock-open" role="menuitem" aria-disabled="true">…Bloquear ecrã <kbd class="ods-menu__kbd">⌘ L</kbd></button>
        <div class="ods-menu__sep"></div>
        <form method="post" action="/logout"><button class="ods-menu__item ods-menu__item--danger" role="menuitem">…Terminar sessão</button></form>
      </div>
    </div>
    <div class="ods-slot" data-oc="profile">
      <button class="ods-topbar__profile ods-gold-badge" data-oc="profile-toggle" aria-expanded="false" aria-label="Perfil: Research">Re</button>
      <div class="ods-popover ods-popover--left ods-glass ods-profile-card" data-oc="profile-card" hidden>…</div>
    </div>
    <div class="ods-slot"><button class="ods-topbar__ctx" data-oc="ctx-toggle">…UENR-001</button></div>
    <nav class="ods-crumbs">…</nav>
    <form class="ods-topbar__ask" method="get" action="/ask" role="search"><svg class="ods-icon"><use href="/static/ods-icons.svg#ods-nye"/></svg><input name="q" placeholder="…"><kbd class="ods-kbd" data-oc="palette-open">⌘K</kbd></form>
    <div class="ods-slot" data-oc="create">
      <button class="ods-topbar__create" data-oc="create-toggle" aria-haspopup="menu" aria-expanded="false">+ Criar</button>
      <div class="ods-popover ods-popover--right ods-glass ods-create-menu" data-part="create__menu" data-oc="create-menu" role="menu" hidden>…</div>
    </div>
    <div class="ods-slot"><button class="ods-status" data-oc="status-toggle">…CORE … IA</button><div class="ods-popover ods-popover--right ods-glass ods-status-card" data-oc="status-card" hidden>…</div></div>
    <div class="ods-slot" data-oc="sino">
      <button class="ods-iconbtn" data-oc="abrir-notificacoes" aria-label="Notificações · 4 por ler" aria-expanded="false"><svg class="ods-icon"><use href="/static/ods-icons.svg#ods-bell"/></svg><span class="ods-count" data-oc="notificacoes-contagem" aria-hidden="true">4</span></button>
      <div class="ods-popover ods-popover--right ods-glass ods-notif" data-oc="notificacoes" hidden><div class="ods-notif__list" data-oc="notificacoes-lista">…</div><a class="ods-btn ods-btn--ghost ods-btn--block" href="/notifications">Ver todas</a></div>
    </div>
    <div class="ods-slot"><button class="ods-topbar__clock" data-oc="clock" aria-expanded="false"><span class="ods-topbar__date">Dom 27 set</span><span class="ods-topbar__time">03:11</span></button><div class="ods-popover ods-popover--right ods-glass ods-cal" hidden>…</div></div>
  </header>
  …desktop / conteúdo…
</div>
```

## Painéis e dados

| Painel | Dados (de onde) | Estados |
|---|---|---|
| Conta | `GET /me` (nome, email, avatar) · perfil da instância | — |
| Perfil (Re/Bu/Pe/Ed) | `InstanceProfile` (`ocinye-contracts/application.rs`), apps activas, instância (`0052_instance_identity`). Iniciais: Research **Re**, Business **Bu**, Personal **Pe**, Education **Ed** | «Versão da predefinição do Desktop» → **indisponível** até G-04 (`ods.state.pending_contract`) |
| Criar | itens → `/notes/new` (POST, form existente), `/tasks/new`, `/calendar` (novo evento), `/mail/compose`, `/files` (carregar), `/ai/agents/new`; cada item só aparece se o Core devolver a permissão | atalhos de teclado: `app.js` |
| Estado CORE · IA | G-09. Até lá: ponto do Core a partir de `/health`; a linha IA mostra `ods-state--unavailable` | nunca «OK» sem resposta do Core |
| Notificações | `/notifications/recent` (existe), «ver todas» → `/notifications`; marcar lida = rota POST existente em `routes.rs` l.321 | vazio: «Sem notificações» · erro: `ods.state.error` |
| Relógio | local; calendário mensal; «Abrir Calendário» → `/calendar` | — |
| Bloquear ecrã | **G-01** → o item mostra `aria-disabled="true"` e o tooltip `ods.state.pending_contract`; o ecrã `.ods-lock` só entra com `POST /session/lock` | ⌘L desactivado até G-01 |

## Comportamento (`app.js`)
- Um só painel aberto de cada vez; Esc ou clique fora fecham e devolvem o foco ao botão.
- Os botões alternam `aria-expanded`; os painéis alternam `hidden`.
