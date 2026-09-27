# D3 · Arranque, login, MFA e primeiro acesso

CSS: `static/ods-d3-auth.css`. **Rotas, `action` e `name` são os actuais** (medidos em `screens/login.rs`, `mfa.rs`, `first_access.rs`, `boot.rs`).

| Ecrã | Rota | Formulários (imutáveis) | Marcadores a manter |
|---|---|---|---|
| Arranque | `GET /boot` | `<form method="get" action="/boot" class="ods-boot__actions" data-part="boot__actions">` + `<input type="hidden" name="return_to">` | — |
| Login | `GET/POST /login` | `action="/login"`: `name="email"`, `name="password"` | `data-oc="clock"`; **sem** `data-oc="reveal"` no login (o teste exige-o) |
| Primeiro acesso | `GET/POST /first-access` | `action="/first-access"`: `name="_username"` (oculto), `name="password"`, `name="confirmation"`; `action="/logout"` | `data-oc="reveal"` ×2 com `data-oc-target="new-pass"` e `"confirm-pass"`; `data-oc="clock"` |
| MFA · configurar | `GET /mfa` | `action="/mfa/confirm"` `name="code"`; `action="/mfa/acknowledge"` `name="acknowledged" value="1" required`; `action="/logout"` | `data-oc="secret"` + `data-oc-value`, `secret-copy`, `recovery-codes`, `recovery-copy`, `recovery-download` |
| MFA · desafio | — | `action="/mfa/challenge"` `name="code"`; `action="/mfa/recovery"` `name="code"` | — |

## Estrutura comum
```html
<main class="ods-auth">
  <span class="ods-auth__clock" data-oc="clock"></span>
  <span class="ods-auth__mark"><img src="/static/ocinye_logo.png" alt=""></span>
  <p class="ods-auth__inst">{nome da instância}</p>
  <section class="ods-auth__card">
    <h1 class="ods-auth__title">Iniciar sessão</h1>
    <form method="post" action="/login">
      <label class="ods-field"><span class="ods-field__label">Endereço de correio ou utilizador</span><input class="ods-input" name="email" autocomplete="username" required></label>
      <label class="ods-field"><span class="ods-field__label">Palavra-passe</span><input class="ods-input" type="password" name="password" autocomplete="current-password" required></label>
      <button class="ods-btn ods-btn--primary ods-btn--block" type="submit">Iniciar sessão</button>
    </form>
  </section>
</main>
```

## Regras de produto
- O perfil **nunca** se escolhe no login (pertence à instância).
- Um contexto → Desktop directo. Vários → selector de contexto (reutiliza `/workspaces/{id}`).
- Reiniciar/desligar não aparece a membros.
- Erros: `<p class="ods-field__error" role="alert">` com o texto que o Core devolveu, nunca genérico.
- Instalação (primeira execução) **não** se mistura com login: é o fluxo `install/` e fica fora deste pacote (FUNCTIONAL GAP de apresentação — sem rota no Workspace).
