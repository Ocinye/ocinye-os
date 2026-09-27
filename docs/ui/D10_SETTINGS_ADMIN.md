# D10 · Definições e Administração

CSS: `static/ods-d10-settings.css`. Membro e administração **nunca na mesma navegação**: `/settings/*` é do membro; `/admin/*` só aparece se o Core devolver a permissão.

## Definições do membro
| Secção | Rota | Formulários (mantêm) |
|---|---|---|
| Conta | `/settings` | avatar: `/settings/avatar/preset`, `/settings/avatar/initials`, upload (rota l.404) |
| Segurança | `/settings/security`, `/settings/password` | `action="/settings/password"` com os `name` actuais |
| MFA | `/settings/mfa`, `/settings/mfa/regenerate` | `data-oc="recovery-codes"`, `recovery-copy`, `recovery-download` |
| Aplicações | `/settings/apps` (GET/POST) | fixações |
| Aparência (claro/escuro/sistema) | — | **lacuna de preferência**: até haver `theme` em `GET /me`, segue o sistema (`data-theme="system"`) e o controlo mostra `ods-state--unavailable` |
| Desktop (repor predefinição) | — | **G-04** → indisponível |
| Idioma | existente em i18n | pt · en · fr |

## Administração
| Secção | Rota | Marcadores (mantêm) |
|---|---|---|
| Consola | `/admin` | |
| Instância (perfil + apps) | `/admin/instance` | `data-oc="instance-admin"`, `instance-app` (+`data-app-id`), `instance-save` |
| Membros | `/admin/members/*` | formulários POST existentes |
| Segredos | (administration.rs) | `data-oc="secret"`, `secret-toggle`, `secret-copy` — nunca mostrar em claro por defeito |
| Audit Log | `/audit` | |
| Predefinição do Desktop | — | **G-04**; o editor dos protótipos fica em `design-reference` como especificação |

Layout: `<div class="ods-settings"><nav class="ods-settings__nav" aria-label="…">…<a class="ods-app__side-item" aria-current="page">…</a></nav><main class="ods-settings__main">…<section class="ods-settings__section">…</section></main></div>`.
