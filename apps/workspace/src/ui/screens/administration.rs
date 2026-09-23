//! Administração de membros: criar, ver acesso, ver segurança.
//!
//! # O que estes ecrãs nunca mostram
//!
//! Uma palavra-passe, um verificador, ou o comprimento de qualquer um dos dois.
//! A única excepção é a credencial temporária acabada de emitir, apresentada
//! **uma única vez** por [`issued_credential`] — e mesmo essa não é recuperável
//! depois de a página ser fechada (briefing §18, §19, §73).

use leptos::prelude::*;
use serde_json::Value;

use crate::ui::components::{badge, button, card, section_head, Button, Tone, Variant};
use crate::ui::icon::{icon, Icon};
use crate::ui::roles;

fn text<'a>(value: &'a Value, key: &str) -> &'a str {
    value.get(key).and_then(Value::as_str).unwrap_or("—")
}

fn day(value: &Value, key: &str) -> String {
    value.get(key).and_then(Value::as_str).map_or_else(
        || "—".to_owned(),
        |stamp| stamp[..stamp.len().min(10)].to_owned(),
    )
}

// Os papéis técnicos e os seus rótulos vivem em [`crate::ui::roles`] — uma só
// lista, que o seletor de criação, o de atribuição e os crachás de acesso
// partilham. Aqui usam-se `roles::OFERECIDOS` e `roles::label_do_codigo`.

/// Posições institucionais: código do domínio → chave do rótulo no catálogo.
/// **Não concedem acesso** (ADR-0100). A chave resolve-se pela via i18n no
/// idioma corrente — uma `const` não pode chamar `t()`, mas pode guardar a chave.
const POSITIONS: [(&str, &str); 9] = [
    ("researcher", "admin.position.researcher"),
    ("engineer", "admin.position.engineer"),
    (
        "principal_investigator",
        "admin.position.principal_investigator",
    ),
    ("unit_lead", "admin.position.unit_lead"),
    ("fellow", "admin.position.fellow"),
    ("student", "admin.position.student"),
    ("director", "admin.position.director"),
    ("founder", "admin.position.founder"),
    (
        "external_collaborator",
        "admin.position.external_collaborator",
    ),
];

/// O rótulo de uma posição institucional, no idioma corrente, pelo código do
/// domínio.
///
/// Uma só tradução, partilhada pelo formulário de criação, pelo detalhe do
/// membro e pela lista — para que «founder» nunca apareça cru num sítio e
/// «Fundador» noutro. Um código que este build não conhece devolve-se como está,
/// em vez de desaparecer: uma posição nova é visível, e não engolida.
#[must_use]
pub fn position_label(code: &str) -> String {
    if code.is_empty() {
        return "—".to_owned();
    }
    POSITIONS.iter().find(|(c, _)| *c == code).map_or_else(
        || code.to_owned(),
        |(_, key)| crate::i18n::t(key).to_owned(),
    )
}

/// Ecrã «Adicionar utilizador».
///
/// Um formulário e não um assistente de cinco passos: os campos cabem num ecrã,
/// e dividi-los esconderia que a posição institucional e o papel técnico são
/// decisões independentes que se tomam ao mesmo tempo.
pub fn new_member(units: &Value, message: Option<String>) -> impl IntoView {
    // `/api/v1/units` responde com um array; aceita-se também `{ "items": [...] }`.
    // Sem isto, o picker de unidade nascia sempre vazio e desactivado — uma
    // unidade recém-criada não aparecia aqui.
    let unit_rows: Vec<(String, String)> = units
        .as_array()
        .or_else(|| units.get("items").and_then(Value::as_array))
        .map(|items| {
            items
                .iter()
                .map(|unit| (text(unit, "id").to_owned(), text(unit, "name").to_owned()))
                .collect()
        })
        .unwrap_or_default();
    let has_units = !unit_rows.is_empty();

    view! {
        <div class="oc-page">
            <div class="oc-head">
                <div class="oc-head__text">
                    <h1>{crate::i18n::t("admin.new.title")}</h1>
                    <p>{crate::i18n::t("admin.new.subtitle")}</p>
                </div>
            </div>

            {message
                .map(|text| {
                    view! { <div class="oc-callout oc-callout--error" role="alert">{text}</div> }
                })}

            <form method="post" action="/admin/members/new" class="oc-split oc-split--2">
                <section class="oc-card">
                    <div class="oc-card__head"><h2>{crate::i18n::t("admin.new.identity")}</h2></div>
                    <div class="oc-card__body">
                        <div class="oc-field">
                            <label class="oc-field__label" for="m-name">{crate::i18n::t("admin.new.full_name")}</label>
                            <input class="oc-input" id="m-name" name="full_name" required
                                   placeholder=crate::i18n::t("admin.new.full_name_ph") />
                        </div>
                        // Um campo, e não dois.
                        //
                        // Havia aqui o antigo campo de nome de utilizador,
                        // renomeado para `email` quando o username saiu
                        // (ADR-0106) e deixado ao lado do verdadeiro. Ficaram
                        // dois `input` com o mesmo `id` e o mesmo `name` — e o
                        // primeiro trazia ainda o `pattern` do username, que
                        // **não admite `@`**.
                        //
                        // O efeito era pior do que desarrumação: nenhum
                        // endereço válido passava a validação do browser, e
                        // ninguém conseguia criar um membro por este ecrã.
                        <div class="oc-field">
                            <label class="oc-field__label" for="m-email">
                                {crate::i18n::t("login.institutional_address")}
                            </label>
                            <input class="oc-input" id="m-email" name="email" type="email" required
                                   autocapitalize="none" spellcheck="false"
                                   placeholder=crate::i18n::t("admin.new.email_ph") />
                            <p class="oc-field__hint">
                                {crate::i18n::t("admin.new.email_hint")}
                            </p>
                        </div>
                    </div>
                </section>

                <div>
                    <section class="oc-card oc-mb-5">
                        <div class="oc-card__head"><h2>{crate::i18n::t("admin.new.organisation")}</h2></div>
                        <div class="oc-card__body">
                            <div class="oc-field">
                                <label class="oc-field__label" for="m-position">
                                    {crate::i18n::t("admin.position.label")}
                                </label>
                                <select class="oc-select" id="m-position" name="position">
                                    <option value="">"—"</option>
                                    {POSITIONS
                                        .iter()
                                        .map(|(value, key)| {
                                            view! { <option value=*value>{crate::i18n::t(key)}</option> }
                                        })
                                        .collect_view()}
                                </select>
                                <p class="oc-field__hint">
                                    {crate::i18n::t("admin.new.position_truth")}
                                    <strong>{crate::i18n::t("admin.new.position_no_access")}</strong>
                                </p>
                            </div>

                            <div class="oc-field">
                                <label class="oc-field__label" for="m-unit">{crate::i18n::t("admin.new.initial_unit")}</label>
                                <select
                                    class="oc-select"
                                    id="m-unit"
                                    name="unit_id"
                                    disabled=!has_units
                                >
                                    {if !has_units {
                                        view! {
                                            <option value="">{crate::i18n::t("admin.new.no_units")}</option>
                                        }
                                            .into_any()
                                    } else {
                                        view! {
                                            <option value="">{crate::i18n::t("admin.new.no_unit_option")}</option>
                                            {unit_rows
                                                .into_iter()
                                                .map(|(id, name)| {
                                                    view! { <option value=id>{name}</option> }
                                                })
                                                .collect_view()}
                                        }
                                            .into_any()
                                    }}
                                </select>
                            </div>
                        </div>
                    </section>

                    <section class="oc-card oc-mb-5">
                        <div class="oc-card__head"><h2>{crate::i18n::t("admin.new.access")}</h2></div>
                        <div class="oc-card__body">
                            <div class="oc-field">
                                <label class="oc-field__label" for="m-role">{crate::i18n::t("admin.role.technical")}</label>
                                <select class="oc-select" id="m-role" name="role" required>
                                    {roles::OFERECIDOS
                                        .into_iter()
                                        .map(|role| {
                                            view! {
                                                <option value=role.as_str()>
                                                    {roles::label_com_descricao(role)}
                                                </option>
                                            }
                                        })
                                        .collect_view()}
                                </select>
                                <p class="oc-field__hint">
                                    {crate::i18n::t("admin.new.role_hint")}
                                </p>
                            </div>
                        </div>
                    </section>

                    <section class="oc-card oc-mb-5">
                        <div class="oc-card__body oc-card__body--subtle">
                            <div class="oc-row oc-gap-5">
                                {icon(Icon::Shield, 14)}
                                <strong>{crate::i18n::t("admin.new.whats_next")}</strong>
                            </div>
                            <p class="oc-muted">
                                {crate::i18n::t("admin.new.whats_next_body")}
                            </p>
                        </div>
                    </section>

                    <div class="oc-row oc-gap-5 oc-justify-end">
                        {button(Button::new(crate::i18n::t("action.cancel"), Variant::Secondary).href("/admin"))}
                        <button type="submit" class="oc-btn oc-btn--primary">{crate::i18n::t("admin.new.submit")}</button>
                    </div>
                </div>
            </form>
        </div>
    }
}

/// Ecrã que apresenta a credencial temporária, **uma única vez**.
///
/// Depois de sair desta página não há forma de a recuperar. Não existe endpoint
/// que a leia de volta, nem para o administrador principal.
pub fn issued_credential(email: &str, password: &str, expires_at: &str) -> impl IntoView {
    let email = email.to_owned();
    let password = password.to_owned();
    let expires = expires_at.get(..16).unwrap_or(expires_at).replace('T', " ");

    view! {
        <div class="oc-page">
            <div class="oc-head">
                <div class="oc-head__text">
                    <h1>{crate::i18n::t("admin.issued.title")}</h1>
                    <p>{crate::i18n::t("admin.issued.subtitle")}</p>
                </div>
            </div>

            <section class="oc-card oc-credential">
                <div class="oc-card__body">
                    <div class="oc-field">
                        <span class="oc-field__label">{crate::i18n::t("login.institutional_address")}</span>
                        <div class="oc-credential__value oc-mono">{email}</div>
                    </div>

                    <div class="oc-field">
                        <span class="oc-field__label">{crate::i18n::t("admin.issued.temp_password")}</span>
                        <div class="oc-credential__value oc-mono">
                            // Coberta por omissão: uma credencial não deve ficar
                            // visível num ecrã que alguém pode estar a partilhar.
                            <span
                                class="oc-credential__secret"
                                data-oc="secret"
                                data-oc-value=password.clone()
                            >
                                "••••••••••••••••••••••••••••"
                            </span>
                            <span class="oc-row oc-gap-5">
                                <button
                                    type="button"
                                    class="oc-btn oc-btn--secondary"
                                    data-oc="secret-toggle"
                                    aria-pressed="false"
                                >
                                    {crate::i18n::t("first_access.show")}
                                </button>
                                <button
                                    type="button"
                                    class="oc-btn oc-btn--secondary"
                                    data-oc="secret-copy"
                                >
                                    {crate::i18n::t("mfa.copy_short")}
                                </button>
                            </span>
                        </div>
                    </div>

                    <div class="oc-field">
                        <span class="oc-field__label">{crate::i18n::t("admin.issued.valid_until")}</span>
                        <div class="oc-credential__value oc-mono">{expires}" UTC"</div>
                    </div>

                    <div class="oc-callout oc-callout--warning" role="alert">
                        <strong>{crate::i18n::t("admin.issued.shown_once")}</strong>
                        {crate::i18n::t("admin.issued.transmit_note")}
                    </div>
                </div>
            </section>

            <div class="oc-row oc-gap-5">
                {button(Button::new(crate::i18n::t("admin.issued.done"), Variant::Primary).href("/admin"))}
            </div>
        </div>
    }
}

/// Separador «Segurança» do detalhe de um membro.
///
/// Só metadados. Nunca um hash, nunca uma palavra-passe (briefing §73).
pub fn security_tab(person_id: &str, overview: &Value, recusa: Option<&str>) -> impl IntoView {
    let status = text(overview, "account_status").to_owned();
    // Quem decide é o Core. Ausente a resposta — porque a consulta falhou — o
    // ecrã não oferece a operação: oferecê-la por omissão mostraria um botão que
    // o Core vai recusar, e faria quem administra julgar-se sem autoridade.
    let pode_provisionar = overview
        .get("may_be_provisioned")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let accao = format!("/admin/members/{person_id}/provision");
    let recusa = recusa.map(str::to_owned);
    let has_permanent = overview
        .get("has_permanent_password")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let failures = overview
        .get("recent_failed_attempts")
        .and_then(Value::as_i64)
        .unwrap_or(0);
    let temporary_expiry = day(overview, "temporary_credential_expires_at");
    let temporary_expired = overview
        .get("temporary_credential_expired")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let changed = day(overview, "password_changed_at");
    let last_sign_in = day(overview, "last_successful_sign_in");

    // Uma credencial expirada continua a existir na base, mas dizer «expira» de
    // uma data passada é apresentá-la como se ainda estivesse por vir. Quando já
    // passou, diz-se «Expirada em», com o seu próprio tom.
    let tem_temporaria = temporary_expiry != "—";
    // Reemitir, e não «dar acesso», quando já houve uma credencial que expirou:
    // a acção é a mesma no Core, mas o nome tem de dizer o que aconteceu.
    let reemitir = pode_provisionar && temporary_expired;
    let rotulo_acesso = if reemitir {
        crate::i18n::t("admin.access.reissue")
    } else {
        crate::i18n::t("admin.access.give")
    };

    let sessions: Vec<Value> = overview
        .get("live_sessions")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let session_count = sessions.len();
    // A autoridade para revogar uma sessão é a mesma que gere a conta, resolvida
    // no actor pelo Core. Sem o sinal, não se oferece o botão.
    let pode_gerir = overview
        .get("may_manage_account")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let pid_sessao = person_id.to_owned();

    view! {
        <div class="oc-split oc-split--2">
            {card(
                section_head(crate::i18n::t("admin.security.credential"), None, None),
                view! {
                    <dl class="oc-facts">
                        <dt>{crate::i18n::t("admin.account.status")}</dt>
                        <dd>{badge(status.clone(), Tone::of(&status))}</dd>

                        <dt>{crate::i18n::t("admin.password.permanent")}</dt>
                        <dd>
                            {if has_permanent {
                                crate::i18n::t("admin.password.self_set")
                            } else {
                                crate::i18n::t("admin.password.not_yet_set")
                            }}
                        </dd>

                        <dt>{crate::i18n::t("admin.password.set_at")}</dt>
                        <dd class="oc-mono">{changed}</dd>

                        <dt>{crate::i18n::t("admin.credential.temporary")}</dt>
                        <dd>
                            {if !tem_temporaria {
                                view! { <span class="oc-mono">"—"</span> }.into_any()
                            } else if temporary_expired {
                                badge(
                                    crate::i18n::tf(
                                        "admin.credential.expired_on",
                                        &[("date", &temporary_expiry)],
                                    ),
                                    Tone::Err,
                                )
                                    .into_any()
                            } else {
                                view! {
                                    <span class="oc-mono">
                                        {crate::i18n::t("admin.credential.expires_prefix")}
                                        {temporary_expiry.clone()}
                                    </span>
                                }
                                    .into_any()
                            }}
                        </dd>

                        <dt>{crate::i18n::t("admin.last_sign_in")}</dt>
                        <dd class="oc-mono">{last_sign_in}</dd>

                        <dt>{crate::i18n::t("admin.recent_failures")}</dt>
                        <dd class="oc-mono">{failures.to_string()}</dd>
                    </dl>
                },
            )}

            {card(
                section_head(
                    crate::i18n::t("admin.sessions.active"),
                    None,
                    Some(session_count.to_string()),
                ),
                if sessions.is_empty() {
                    view! { <p class="oc-muted">{crate::i18n::t("admin.sessions.none")}</p> }.into_any()
                } else {
                    view! {
                        <div>
                            {sessions
                                .iter()
                                .map(|session| {
                                    let state = text(session, "state").to_owned();
                                    let id = text(session, "id").to_owned();
                                    let accao_revogar = format!(
                                        "/admin/members/{pid_sessao}/sessions/{id}/revoke"
                                    );
                                    let revogavel = pode_gerir && id != "—";
                                    view! {
                                        <div class="oc-list__row">
                                            <span class="oc-fill oc-truncate">
                                                {text(session, "user_agent").to_owned()}
                                            </span>
                                            {badge(state.clone(), Tone::of(&state))}
                                            <span class="oc-mono oc-list__meta">
                                                {text(session, "ip_prefix").to_owned()}
                                            </span>
                                            {revogavel.then(|| view! {
                                                <form method="post" action=accao_revogar>
                                                    <button
                                                        class="oc-btn oc-btn--sm oc-btn--danger"
                                                        type="submit"
                                                    >
                                                        {crate::i18n::t("admin.action.revoke")}
                                                    </button>
                                                </form>
                                            })}
                                        </div>
                                    }
                                })
                                .collect_view()}
                        </div>
                    }
                        .into_any()
                },
            )}
        </div>

        {(pode_provisionar || recusa.is_some())
            .then(|| {
                view! {
                    <div class="oc-mt-6">
                        {card(
                            section_head(rotulo_acesso, None, None),
                            view! {
                                <div>
                                    <p class="oc-muted">
                                        {if reemitir {
                                            crate::i18n::t("admin.access.reissue_note")
                                        } else {
                                            crate::i18n::t("admin.access.give_note")
                                        }}
                                    </p>
                                    {recusa
                                        .clone()
                                        .map(|texto| view! {
                                                <div
                                                    class="oc-callout oc-callout--error oc-mt-3"
                                                    role="alert"
                                                >
                                                    {texto}
                                                </div>
                                            })}
                                    {pode_provisionar
                                        .then(|| {
                                            view! {
                                                <form method="post" action=accao.clone() class="oc-mt-3">
                                                    <button class="oc-btn oc-btn--primary" type="submit">
                                                        {rotulo_acesso}
                                                    </button>
                                                </form>
                                            }
                                        })}
                                </div>
                            },
                        )}
                    </div>
                }
            })}
    }
}

/// Separador «Acesso»: porque é que este membro consegue o que consegue.
pub fn access_tab(access: &Value) -> impl IntoView {
    let roles: Vec<String> = access
        .get("roles")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default();

    let permissions: Vec<(String, String)> = access
        .get("institution_permissions")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .map(|entry| {
                    (
                        text(entry, "permission").to_owned(),
                        text(entry, "source").to_owned(),
                    )
                })
                .collect()
        })
        .unwrap_or_default();

    let grants: Vec<Value> = access
        .get("grants")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    let permission_count = permissions.len();
    let grant_count = grants.len();

    view! {
        <div class="oc-split oc-split--2">
            {card(
                section_head(crate::i18n::t("admin.roles.technical"), None, None),
                if roles.is_empty() {
                    view! { <p class="oc-muted">{crate::i18n::t("admin.roles.none_assigned")}</p> }.into_any()
                } else {
                    view! {
                        <div class="oc-row oc-gap-5 oc-wrap">
                            {roles
                                .into_iter()
                                .map(|role| {
                                    // O crachá mostra o rótulo canónico; o tom continua
                                    // a resolver-se pelo código estável, que não muda.
                                    // Caminho completo: a variável local `roles` acima
                                    // sombreia o módulo dentro deste fecho.
                                    badge(crate::ui::roles::label_do_codigo(&role), Tone::of(&role))
                                })
                                .collect_view()}
                        </div>
                    }
                        .into_any()
                },
            )}

            {card(
                section_head(crate::i18n::t("admin.grants.explicit"), None, Some(grant_count.to_string())),
                if grants.is_empty() {
                    view! {
                        <p class="oc-muted">
                            {crate::i18n::t("admin.grants.none")}
                        </p>
                    }
                        .into_any()
                } else {
                    view! {
                        <div>
                            {grants
                                .iter()
                                .map(|grant| {
                                    let revoked = grant.get("revoked_at").is_some_and(|v| !v.is_null());
                                    view! {
                                        <div class="oc-list__row">
                                            <span class="oc-fill oc-mono oc-truncate">
                                                {text(grant, "permission").to_owned()}
                                            </span>
                                            <span class="oc-mono oc-list__meta">
                                                {text(grant, "scope").to_owned()}
                                            </span>
                                            {badge(
                                                if revoked {
                                                    crate::i18n::t("admin.state.revoked")
                                                } else {
                                                    crate::i18n::t("admin.state.active")
                                                }
                                                .to_owned(),
                                                if revoked { Tone::Gray } else { Tone::Ok },
                                            )}
                                        </div>
                                    }
                                })
                                .collect_view()}
                        </div>
                    }
                        .into_any()
                },
            )}

            <section class="oc-card oc-span-2">
                {section_head(
                    crate::i18n::t("admin.permissions.institutional"),
                    None,
                    Some(permission_count.to_string()),
                )}
                <div class="oc-card__body">
                    {if permissions.is_empty() {
                        view! {
                            <p class="oc-muted">
                                {crate::i18n::t("admin.permissions.none")}
                            </p>
                        }
                            .into_any()
                    } else {
                        view! {
                            <div class="oc-facts oc-facts--dense">
                                {permissions
                                    .into_iter()
                                    .map(|(permission, source)| {
                                        view! {
                                            <span class="oc-mono">{permission}</span>
                                            <span class="oc-muted">{source_label(&source)}</span>
                                        }
                                    })
                                    .collect_view()}
                            </div>
                        }
                            .into_any()
                    }}
                </div>
            </section>
        </div>
    }
}

/// Traduz a origem de um acesso, tal como o Core a nomeou.
fn source_label(source: &str) -> &'static str {
    match source {
        "technical_role" => crate::i18n::t("admin.source.technical_role"),
        "unit_membership" => crate::i18n::t("admin.source.unit_membership"),
        "workspace_membership" => crate::i18n::t("admin.source.workspace_membership"),
        "explicit_grant" => crate::i18n::t("admin.source.explicit_grant"),
        // Vocabulário que este build não conhece: dizê-lo é melhor do que
        // inventar uma tradução.
        _ => crate::i18n::t("admin.source.unknown"),
    }
}

/// O «Overview» do membro: o dossier de relance, sem segredos.
///
/// Consolida o que já existe espalhado pelas secções abaixo — posição, estado,
/// segundo factor, palavra-passe, papéis, sessões —, para que quem administra
/// veja o essencial sem percorrer a página inteira. O estado do segundo factor
/// vem do Core (`mfa_required`/`mfa_enrolled`): é lá que vive a regra que o
/// torna obrigatório, e não no ecrã.
fn overview_tab(position: &str, status: &str, security: &Value, access: &Value) -> impl IntoView {
    let mfa_required = security
        .get("mfa_required")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let mfa_enrolled = security
        .get("mfa_enrolled")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let (mfa_texto, mfa_tone) = if !mfa_required {
        (crate::i18n::t("admin.mfa.not_required"), Tone::Gray)
    } else if mfa_enrolled {
        (crate::i18n::t("admin.mfa.required_enrolled"), Tone::Ok)
    } else {
        (crate::i18n::t("admin.mfa.required_pending"), Tone::Warn)
    };

    let has_permanent = security
        .get("has_permanent_password")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let sessoes = security
        .get("live_sessions")
        .and_then(Value::as_array)
        .map_or(0, Vec::len);

    let papeis: Vec<String> = access
        .get("roles")
        .and_then(Value::as_array)
        .map(|rs| {
            rs.iter()
                .filter_map(Value::as_str)
                .map(|code| roles::label_do_codigo(code).to_owned())
                .collect()
        })
        .unwrap_or_default();
    let papeis_texto = if papeis.is_empty() {
        crate::i18n::t("admin.roles.none_short").to_owned()
    } else {
        papeis.join(", ")
    };

    let status = status.to_owned();
    let position = position.to_owned();

    card(
        section_head(crate::i18n::t("admin.overview.summary"), None, None),
        view! {
            <dl class="oc-facts">
                <dt>{crate::i18n::t("admin.position.label")}</dt>
                <dd>{position}{crate::i18n::t("admin.position.no_access_suffix")}</dd>

                <dt>{crate::i18n::t("admin.account.status")}</dt>
                <dd>{badge(status.clone(), Tone::of(&status))}</dd>

                <dt>{crate::i18n::t("settings.recovery.section")}</dt>
                <dd>{badge(mfa_texto.to_owned(), mfa_tone)}</dd>

                <dt>{crate::i18n::t("admin.password.permanent")}</dt>
                <dd>
                    {if has_permanent {
                        crate::i18n::t("admin.password.self_set")
                    } else {
                        crate::i18n::t("admin.password.not_yet_set")
                    }}
                </dd>

                <dt>{crate::i18n::t("admin.roles.technical")}</dt>
                <dd>{papeis_texto}</dd>

                <dt>{crate::i18n::t("admin.sessions.active")}</dt>
                <dd class="oc-mono">{sessoes.to_string()}</dd>
            </dl>
        },
    )
}

/// Detalhe de um membro: quem é, o que pode, e o estado da sua credencial.
///
/// # A barra do topo não finge separadores
///
/// As quatro secções que existem — Acesso, Segurança, Unidades, Research
/// Workspaces — são desenhadas em pilha nesta página, e a barra do topo leva a
/// cada uma por âncora (`href="#membro-…"`): funciona sem JavaScript, e não
/// promete uma troca de painel que não acontece. Havia aqui uma `role="tablist"`
/// de `<span>` sem destino nenhum — tinha o aspecto de separadores e não fazia
/// nada, que é precisamente a categoria que a auditoria «Zero Dead UI» proíbe.
///
/// «Overview» é a primeira secção — o dossier de relance. «Actividade» e «Audit»
/// não têm secção neste dossier, e ficam declarados indisponíveis **com a razão
/// de cada um** — não «ainda não disponível», que serviria para tudo. Existem
/// como ecrãs próprios; o que não existe é o recorte por membro.
pub fn member_detail(
    person: &Value,
    security: &Value,
    access: &Value,
    units_catalog: &Value,
    workspaces_catalog: &Value,
    permissions_catalog: &Value,
    recusa: Option<&str>,
) -> impl IntoView {
    let person_id = text(person, "id").to_owned();
    let units_catalog = units_catalog.clone();
    let workspaces_catalog = workspaces_catalog.clone();
    let permissions_catalog = permissions_catalog.clone();
    let recusa = recusa.map(str::to_owned);
    let name = text(person, "full_name").to_owned();
    // O endereço, uma vez. Havia aqui um `username` ao lado dele, e a linha
    // mostrava a mesma pessoa duas vezes: `afernandes · afernandes@ocinye.com`.
    let email = text(person, "email").to_owned();
    let status = text(person, "status").to_owned();
    // A posição em português. Vinha crua — «founder» em vez de «Fundador» — por
    // não passar pela mesma tradução que o formulário de criação usa.
    let position = position_label(
        person
            .get("institutional_position")
            .and_then(Value::as_str)
            .unwrap_or(""),
    );

    let security = security.clone();
    let access = access.clone();

    view! {
        // Mesmas classes do cabeçalho do Research Workspace: um segundo padrão
        // de cabeçalho contextual seria um segundo sítio para os alinhar.
        <div class="oc-band">
            <div class="oc-row--top oc-gap-11 oc-mb-3">
                <div class="oc-fill">
                    <div class="oc-row oc-row--wrap oc-gap-6">
                        <span class="oc-pill">{crate::i18n::t("admin.member.pill")}</span>
                        <h1 class="oc-t-screen">{name}</h1>
                        {badge(status.clone(), Tone::of(&status))}
                    </div>
                    <div class="oc-mono oc-mt-3">{email}</div>
                    <div class="oc-muted oc-mt-3">
                        {crate::i18n::t("admin.member.position_prefix")}{position.clone()}
                        {crate::i18n::t("admin.position.no_access_suffix")}
                    </div>
                </div>
            </div>

            // `data-oc-section-nav`: o `app.js` acompanha a secção activa pela
            // âncora e pelo scroll. O servidor marca «Overview» activa por
            // omissão (`aria-current="location"`) — funciona sem JavaScript, e
            // um deep-link para outra secção é corrigido no carregamento.
            <nav class="oc-tabs oc-tabs--ctx" aria-label=crate::i18n::t("admin.member.sections_aria") data-oc-section-nav="">
                <a class="oc-tab" href="#membro-overview" aria-current="location">{crate::i18n::t("admin.tab.overview")}</a>
                <a class="oc-tab" href="#membro-acesso">{crate::i18n::t("admin.tab.access")}</a>
                <a class="oc-tab" href="#membro-seguranca">{crate::i18n::t("settings.tab.security")}</a>
                <a class="oc-tab" href="#membro-unidades">{crate::i18n::t("nav.units")}</a>
                <a class="oc-tab" href="#membro-research-workspaces">{crate::i18n::t("home.continue.aside")}</a>
                {[
                    ("nav.activity", "admin.tab.activity_unavailable"),
                    ("admin.tab.audit", "admin.tab.audit_unavailable"),
                ]
                    .iter()
                    .map(|(label_key, porque_key)| {
                        view! {
                            <span class="oc-tab oc-unavailable" aria-disabled="true" title=crate::i18n::t(porque_key)>
                                {crate::i18n::t(label_key)}
                            </span>
                        }
                    })
                    .collect_view()}
            </nav>
        </div>

        <div class="oc-page">
            <section id="membro-overview">
                {section_head(crate::i18n::t("admin.tab.overview"), None, None)}
                {overview_tab(&position, &status, &security, &access)}
            </section>
            <div class="oc-vspace"></div>
            <section id="membro-acesso">
                {section_head(crate::i18n::t("admin.tab.access"), None, None)}
                {access_tab(&access)}
                {roles_admin(&person_id, &access)}
                {grants_admin(&person_id, &access, &permissions_catalog)}
            </section>
            <div class="oc-vspace"></div>
            <section id="membro-seguranca">
                {section_head(crate::i18n::t("settings.tab.security"), None, None)}
                {security_tab(&person_id, &security, recusa.as_deref())}
                {account_admin(&person_id, &security)}
            </section>
            <div class="oc-vspace"></div>
            <section id="membro-unidades">
                {section_head(crate::i18n::t("nav.units"), None, None)}
                {units_admin(&person_id, &access, &units_catalog)}
            </section>
            <div class="oc-vspace"></div>
            <section id="membro-research-workspaces">
                {section_head(crate::i18n::t("home.continue.aside"), None, None)}
                {workspaces_admin(&person_id, &access, &workspaces_catalog)}
            </section>
        </div>
    }
}

/// Separador «Unidades» do membro: as unidades a que pertence, e a
/// administração das suas pertenças.
///
/// # Autoridade
///
/// O que este ecrã oferece nunca é o que decide. As mutações batem no Core
/// (`/api/v1/units/{id}/members`), que reautoriza o **actor** — não o membro
/// aqui aberto — a cada operação. A pertença do membro não governa o que o
/// administrador pode fazer: um membro sem unidade nenhuma continua
/// administrável por quem tem autoridade.
///
/// # Fronteira
///
/// Administrar a pertença a uma unidade não concede leitura do conteúdo
/// científico dessa unidade. Aqui trata-se de estrutura, não de conteúdo.
pub fn units_admin(person_id: &str, access: &Value, catalog: &Value) -> impl IntoView {
    // Catálogo de unidades da instituição: id → nome.
    //
    // `/api/v1/units` responde com um **array** de unidades. Aceita-se também a
    // forma `{ "items": [...] }` para não depender de qual delas o chamador traz.
    let catalogo: Vec<(String, String)> = catalog
        .as_array()
        .or_else(|| catalog.get("items").and_then(Value::as_array))
        .map(|itens| {
            itens
                .iter()
                .filter_map(|u| {
                    let id = u.get("id").and_then(Value::as_str)?;
                    let nome = u
                        .get("name")
                        .and_then(Value::as_str)
                        .or_else(|| u.get("code").and_then(Value::as_str))
                        .unwrap_or(id);
                    Some((id.to_owned(), nome.to_owned()))
                })
                .collect()
        })
        .unwrap_or_default();

    // Pertenças actuais do membro: (id, papel).
    let pertencas: Vec<(String, String)> = access
        .get("units")
        .and_then(Value::as_array)
        .map(|itens| {
            itens
                .iter()
                .filter_map(|u| {
                    let id = u.get("id").and_then(Value::as_str)?;
                    let papel = u.get("role").and_then(Value::as_str).unwrap_or("member");
                    Some((id.to_owned(), papel.to_owned()))
                })
                .collect()
        })
        .unwrap_or_default();

    let nome_de = |id: &str| -> String {
        catalogo
            .iter()
            .find(|(uid, _)| uid == id)
            .map(|(_, n)| n.clone())
            .unwrap_or_else(|| id.to_owned())
    };

    let ja_membro: std::collections::HashSet<String> =
        pertencas.iter().map(|(id, _)| id.clone()).collect();

    // Unidades que ainda pode receber (não repetir as que já tem).
    let elegiveis: Vec<(String, String)> = catalogo
        .iter()
        .filter(|(id, _)| !ja_membro.contains(id))
        .cloned()
        .collect();

    let sem_unidades_na_org = catalogo.is_empty();
    let sem_pertencas = pertencas.is_empty();

    // Linhas das pertenças actuais, cada uma com alterar-papel e remover.
    let linhas: Vec<_> = pertencas
        .iter()
        .map(|(id, papel)| {
            let nome = nome_de(id);
            let papel_actual = papel.clone();
            let accao_papel = format!("/admin/members/{person_id}/units/{id}/role");
            let accao_remover = format!("/admin/members/{person_id}/units/{id}/remove");
            let is_manager = papel_actual == "manager";
            view! {
                <tr>
                    <td>{nome}</td>
                    <td>
                        <form method="post" action=accao_papel class="oc-row oc-gap-3">
                            <select class="oc-select oc-select--sm" name="role">
                                <option value="member" selected=!is_manager>{crate::i18n::t("admin.role.member")}</option>
                                <option value="manager" selected=is_manager>{crate::i18n::t("admin.role.manager")}</option>
                            </select>
                            <button class="oc-btn oc-btn--sm" type="submit">{crate::i18n::t("action.save")}</button>
                        </form>
                    </td>
                    <td class="oc-td--actions">
                        <form method="post" action=accao_remover>
                            <button
                                class="oc-btn oc-btn--sm oc-btn--danger"
                                type="submit"
                            >
                                {crate::i18n::t("action.remove")}
                            </button>
                        </form>
                    </td>
                </tr>
            }
        })
        .collect();

    let accao_atribuir = format!("/admin/members/{person_id}/units");

    view! {
        {card(
            section_head(crate::i18n::t("admin.units.memberships"), None, None),
            view! {
                {if sem_pertencas {
                    view! {
                        <p class="oc-muted">{crate::i18n::t("admin.units.none_assigned")}</p>
                    }
                    .into_any()
                } else {
                    view! {
                        <table class="oc-table oc-table--dense">
                            <thead>
                                <tr>
                                    <th>{crate::i18n::t("entity.unit")}</th>
                                    <th>{crate::i18n::t("admin.col.role")}</th>
                                    <th class="oc-td--actions">{crate::i18n::t("admin.col.actions")}</th>
                                </tr>
                            </thead>
                            <tbody>{linhas}</tbody>
                        </table>
                    }
                    .into_any()
                }}
            },
        )}

        <div class="oc-mt-6">
            {card(
                section_head(crate::i18n::t("admin.units.assign"), None, None),
                if sem_unidades_na_org {
                    view! {
                        <p class="oc-muted">
                            {crate::i18n::t("admin.units.none_org")}
                            <a class="oc-link" href="/units/new">{crate::i18n::t("nav.units")}</a>
                            {crate::i18n::t("admin.units.before_assign")}
                        </p>
                    }
                    .into_any()
                } else if elegiveis.is_empty() {
                    view! {
                        <p class="oc-muted">
                            {crate::i18n::t("admin.units.all_assigned")}
                        </p>
                    }
                    .into_any()
                } else {
                    view! {
                        <form method="post" action=accao_atribuir class="oc-row oc-row--wrap oc-gap-3">
                            <select class="oc-select" name="unit_id" required>
                                <option value="">{crate::i18n::t("admin.units.choose")}</option>
                                {elegiveis
                                    .into_iter()
                                    .map(|(id, nome)| view! {
                                        <option value=id>{nome}</option>
                                    })
                                    .collect_view()}
                            </select>
                            <select class="oc-select" name="role">
                                <option value="member">{crate::i18n::t("admin.role.member")}</option>
                                <option value="manager">{crate::i18n::t("admin.role.manager")}</option>
                            </select>
                            <button class="oc-btn oc-btn--primary" type="submit">
                                {crate::i18n::t("admin.action.assign")}
                            </button>
                        </form>
                    }
                    .into_any()
                },
            )}
        </div>
    }
}

/// Separador «Research Workspaces» do membro: os ambientes de investigação a
/// que pertence, e a administração dessas pertenças.
///
/// # Autoridade e fronteira
///
/// Como nas unidades, as mutações batem no Core
/// (`/api/v1/workspaces/{id}/members`), que reautoriza o **actor** a cada
/// operação — a pertença do membro aberto nunca governa o que o administrador
/// pode fazer. E administrar a pertença a um workspace **não** concede leitura
/// do seu conteúdo científico, mesmo `RESTRICTED`: é estrutura, não conteúdo.
pub fn workspaces_admin(person_id: &str, access: &Value, catalog: &Value) -> impl IntoView {
    // Catálogo de workspaces: id → título. `/api/v1/workspaces` pagina em
    // `{ "items": [...] }`; aceita-se também um array directo.
    let catalogo: Vec<(String, String)> = catalog
        .get("items")
        .and_then(Value::as_array)
        .or_else(|| catalog.as_array())
        .map(|itens| {
            itens
                .iter()
                .filter_map(|w| {
                    let id = w.get("id").and_then(Value::as_str)?;
                    let nome = w
                        .get("title")
                        .and_then(Value::as_str)
                        .or_else(|| w.get("code").and_then(Value::as_str))
                        .unwrap_or(id);
                    Some((id.to_owned(), nome.to_owned()))
                })
                .collect()
        })
        .unwrap_or_default();

    let pertencas: Vec<(String, String)> = access
        .get("workspaces")
        .and_then(Value::as_array)
        .map(|itens| {
            itens
                .iter()
                .filter_map(|w| {
                    let id = w.get("id").and_then(Value::as_str)?;
                    let papel = w.get("role").and_then(Value::as_str).unwrap_or("viewer");
                    Some((id.to_owned(), papel.to_owned()))
                })
                .collect()
        })
        .unwrap_or_default();

    let nome_de = |id: &str| -> String {
        catalogo
            .iter()
            .find(|(wid, _)| wid == id)
            .map(|(_, n)| n.clone())
            .unwrap_or_else(|| id.to_owned())
    };

    let ja_membro: std::collections::HashSet<String> =
        pertencas.iter().map(|(id, _)| id.clone()).collect();

    let elegiveis: Vec<(String, String)> = catalogo
        .iter()
        .filter(|(id, _)| !ja_membro.contains(id))
        .cloned()
        .collect();

    let sem_workspaces = catalogo.is_empty();
    let sem_pertencas = pertencas.is_empty();

    // Um seletor de papel reutilizado, com o papel actual pré-seleccionado.
    let papel_options = |actual: &str| {
        let is_viewer = actual == "viewer";
        let is_member = actual == "member";
        let is_lead = actual == "lead";
        view! {
            <option value="viewer" selected=is_viewer>{crate::i18n::t("admin.ws_role.viewer")}</option>
            <option value="member" selected=is_member>{crate::i18n::t("admin.role.member")}</option>
            <option value="lead" selected=is_lead>{crate::i18n::t("admin.ws_role.lead")}</option>
        }
    };

    let linhas: Vec<_> = pertencas
        .iter()
        .map(|(id, papel)| {
            let nome = nome_de(id);
            let accao_papel = format!("/admin/members/{person_id}/workspaces/{id}/role");
            let accao_remover = format!("/admin/members/{person_id}/workspaces/{id}/remove");
            let opts = papel_options(papel);
            view! {
                <tr>
                    <td>{nome}</td>
                    <td>
                        <form method="post" action=accao_papel class="oc-row oc-gap-3">
                            <select class="oc-select oc-select--sm" name="role">{opts}</select>
                            <button class="oc-btn oc-btn--sm" type="submit">{crate::i18n::t("action.save")}</button>
                        </form>
                    </td>
                    <td class="oc-td--actions">
                        <form method="post" action=accao_remover>
                            <button class="oc-btn oc-btn--sm oc-btn--danger" type="submit">
                                {crate::i18n::t("action.remove")}
                            </button>
                        </form>
                    </td>
                </tr>
            }
        })
        .collect();

    let accao_atribuir = format!("/admin/members/{person_id}/workspaces");

    view! {
        {card(
            section_head(crate::i18n::t("admin.ws.memberships"), None, None),
            view! {
                {if sem_pertencas {
                    view! { <p class="oc-muted">{crate::i18n::t("admin.ws.none_assigned")}</p> }
                        .into_any()
                } else {
                    view! {
                        <table class="oc-table oc-table--dense">
                            <thead>
                                <tr>
                                    <th>{crate::i18n::t("admin.col.workspace")}</th>
                                    <th>{crate::i18n::t("admin.col.role")}</th>
                                    <th class="oc-td--actions">{crate::i18n::t("admin.col.actions")}</th>
                                </tr>
                            </thead>
                            <tbody>{linhas}</tbody>
                        </table>
                    }
                    .into_any()
                }}
            },
        )}

        <div class="oc-mt-6">
            {card(
                section_head(crate::i18n::t("admin.ws.assign"), None, None),
                if sem_workspaces {
                    view! {
                        <p class="oc-muted">
                            {crate::i18n::t("admin.ws.none_org")}
                        </p>
                    }
                    .into_any()
                } else if elegiveis.is_empty() {
                    view! {
                        <p class="oc-muted">
                            {crate::i18n::t("admin.ws.all_assigned")}
                        </p>
                    }
                    .into_any()
                } else {
                    view! {
                        <form
                            method="post"
                            action=accao_atribuir
                            class="oc-row oc-row--wrap oc-gap-3"
                        >
                            <select class="oc-select" name="workspace_id" required>
                                <option value="">{crate::i18n::t("admin.ws.choose")}</option>
                                {elegiveis
                                    .into_iter()
                                    .map(|(id, nome)| view! { <option value=id>{nome}</option> })
                                    .collect_view()}
                            </select>
                            <select class="oc-select" name="role">{papel_options("member")}</select>
                            <button class="oc-btn oc-btn--primary" type="submit">{crate::i18n::t("admin.action.assign")}</button>
                        </form>
                    }
                    .into_any()
                },
            )}
        </div>
    }
}

/// Administração dos **papéis técnicos** de um membro: conceder e revogar.
///
/// # Autoridade
///
/// Renderiza-se apenas quando o Core diz que o **actor** pode administrar papéis
/// (`access.may_manage_roles`) — resolvido a partir da autoridade de quem
/// consulta, nunca do que a conta-alvo tem. Sem esse sinal, a secção não
/// aparece: um botão que o Core recusaria faria o administrador julgar-se sem
/// autoridade que tem, ou o contrário. Cada operação é, ainda assim,
/// reautorizada no Core no momento em que corre.
pub fn roles_admin(person_id: &str, access: &Value) -> impl IntoView {
    let pode_gerir = access
        .get("may_manage_roles")
        .and_then(Value::as_bool)
        .unwrap_or(false);

    let actuais: Vec<String> = access
        .get("roles")
        .and_then(Value::as_array)
        .map(|itens| {
            itens
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default();

    let ja_tem: std::collections::HashSet<String> = actuais.iter().cloned().collect();
    let elegiveis: Vec<(&str, String)> = roles::OFERECIDOS
        .into_iter()
        .filter(|role| !ja_tem.contains(role.as_str()))
        .map(|role| (role.as_str(), roles::label_com_descricao(role)))
        .collect();

    // A revogação de um papel batido no Core reautoriza o actor e pode ser
    // recusada — retirar o último Platform Admin, por exemplo. A recusa volta ao
    // ecrã em vez de ser engolida.
    let linhas: Vec<_> = actuais
        .iter()
        .map(|role| {
            let etiqueta = roles::label_do_codigo(role);
            let accao = format!("/admin/members/{person_id}/roles/{role}/revoke");
            view! {
                <tr>
                    <td>{etiqueta}</td>
                    <td class="oc-td--actions">
                        <form method="post" action=accao>
                            <button class="oc-btn oc-btn--sm oc-btn--danger" type="submit">
                                {crate::i18n::t("admin.action.revoke")}
                            </button>
                        </form>
                    </td>
                </tr>
            }
        })
        .collect();

    let sem_papeis = actuais.is_empty();
    let accao_conceder = format!("/admin/members/{person_id}/roles");

    view! {
        <div class="oc-mt-6">
            {if pode_gerir {
                view! {
                {card(
                    section_head(crate::i18n::t("admin.roles.manage"), None, None),
                    view! {
                        {if sem_papeis {
                            view! { <p class="oc-muted">{crate::i18n::t("admin.roles.none_assigned_technical")}</p> }
                                .into_any()
                        } else {
                            view! {
                                <table class="oc-table oc-table--dense">
                                    <thead>
                                        <tr>
                                            <th>{crate::i18n::t("admin.col.role")}</th>
                                            <th class="oc-td--actions">{crate::i18n::t("admin.col.actions")}</th>
                                        </tr>
                                    </thead>
                                    <tbody>{linhas}</tbody>
                                </table>
                            }
                                .into_any()
                        }}

                        {if elegiveis.is_empty() {
                            view! {
                                <p class="oc-muted oc-mt-3">
                                    {crate::i18n::t("admin.roles.all_assigned")}
                                </p>
                            }
                                .into_any()
                        } else {
                            view! {
                                <form
                                    method="post"
                                    action=accao_conceder.clone()
                                    class="oc-row oc-row--wrap oc-gap-3 oc-mt-3"
                                >
                                    <select class="oc-select" name="role" required>
                                        <option value="">{crate::i18n::t("admin.roles.choose")}</option>
                                        {elegiveis
                                            .clone()
                                            .into_iter()
                                            .map(|(id, label)| view! {
                                                <option value=id>{label}</option>
                                            })
                                            .collect_view()}
                                    </select>
                                    <input
                                        class="oc-input oc-fill"
                                        type="text"
                                        name="reason"
                                        required
                                        minlength="4"
                                        placeholder=crate::i18n::t("admin.reason.audit_ph")
                                    />
                                    <button class="oc-btn oc-btn--primary" type="submit">
                                        {crate::i18n::t("admin.action.grant")}
                                    </button>
                                </form>
                            }
                                .into_any()
                        }}
                    },
                )}
                }
                .into_any()
            } else {
                // Não renderizar nada deixava o defeito que o utilizador viu: a
                // opção de mudar o acesso «desaparecia» sem explicação. Quem pode
                // ver o membro mas não gerir papéis vê agora a condição, em vez do
                // vazio — o mesmo princípio do «Criar» (autoridade explicada, §3).
                view! {
                {card(
                    section_head(crate::i18n::t("admin.roles.technical"), None, None),
                    view! {
                        <p class="oc-muted">
                            {crate::i18n::t("admin.roles.manage_requires")}
                        </p>
                    },
                )}
                }
                .into_any()
            }}
        </div>
    }
}

/// Administração dos **grants explícitos** de âmbito institucional de um membro.
///
/// # Fronteira do que se administra aqui
///
/// Só grants de âmbito **instituição**. O acesso dentro de unidades e de
/// research workspaces administra-se nos separadores próprios (Unidades,
/// Research Workspaces), onde o âmbito tem um alvo concreto. Um grant
/// institucional é o que amplia o acesso de alguém para além do que os papéis
/// lhe dão, e é essa a decisão que esta secção torna explícita e revogável.
///
/// # Autoridade
///
/// Renderiza-se apenas quando o Core diz que o actor pode gerir grants
/// (`access.may_manage_grants`). O Core recusa conceder o que o próprio actor
/// não possui — esta secção não repete essa regra, confia nela.
pub fn grants_admin(person_id: &str, access: &Value, permissions_catalog: &Value) -> impl IntoView {
    let pode_gerir = access
        .get("may_manage_grants")
        .and_then(Value::as_bool)
        .unwrap_or(false);

    let catalogo: Vec<String> = permissions_catalog
        .as_array()
        .or_else(|| permissions_catalog.get("items").and_then(Value::as_array))
        .map(|itens| {
            itens
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default();

    // Só os grants vivos podem ser revogados; um já revogado mostra-se no
    // separador Acesso, mas aqui não oferece botão.
    let vivos: Vec<Value> = access
        .get("grants")
        .and_then(Value::as_array)
        .map(|itens| {
            itens
                .iter()
                .filter(|g| g.get("revoked_at").is_none_or(Value::is_null))
                .cloned()
                .collect()
        })
        .unwrap_or_default();

    let linhas: Vec<_> = vivos
        .iter()
        .filter_map(|grant| {
            let id = grant.get("id").and_then(Value::as_str)?.to_owned();
            let permissao = text(grant, "permission").to_owned();
            let ambito = text(grant, "scope").to_owned();
            let accao = format!("/admin/members/{person_id}/grants/{id}/revoke");
            Some(view! {
                <tr>
                    <td class="oc-mono">{permissao}</td>
                    <td class="oc-mono oc-muted">{ambito}</td>
                    <td class="oc-td--actions">
                        <form method="post" action=accao class="oc-row oc-gap-3">
                            <input
                                class="oc-input"
                                type="text"
                                name="reason"
                                required
                                minlength="4"
                                placeholder=crate::i18n::t("admin.reason.ph")
                            />
                            <button class="oc-btn oc-btn--sm oc-btn--danger" type="submit">
                                {crate::i18n::t("admin.action.revoke")}
                            </button>
                        </form>
                    </td>
                </tr>
            })
        })
        .collect();

    let sem_grants = linhas.is_empty();
    let sem_catalogo = catalogo.is_empty();
    let accao_conceder = format!("/admin/members/{person_id}/grants");

    view! {
        {pode_gerir.then(|| view! {
            <div class="oc-mt-6">
                {card(
                    section_head(crate::i18n::t("admin.grants.manage"), None, None),
                    view! {
                        {if sem_grants {
                            view! {
                                <p class="oc-muted">
                                    {crate::i18n::t("admin.grants.none_active")}
                                </p>
                            }
                                .into_any()
                        } else {
                            view! {
                                <table class="oc-table oc-table--dense">
                                    <thead>
                                        <tr>
                                            <th>{crate::i18n::t("admin.col.permission")}</th>
                                            <th>{crate::i18n::t("admin.col.scope")}</th>
                                            <th class="oc-td--actions">{crate::i18n::t("admin.col.actions")}</th>
                                        </tr>
                                    </thead>
                                    <tbody>{linhas}</tbody>
                                </table>
                            }
                                .into_any()
                        }}

                        {if sem_catalogo {
                            view! {
                                <p class="oc-muted oc-mt-3">
                                    {crate::i18n::t("admin.grants.catalog_unavailable")}
                                </p>
                            }
                                .into_any()
                        } else {
                            view! {
                                <form
                                    method="post"
                                    action=accao_conceder.clone()
                                    class="oc-row oc-row--wrap oc-gap-3 oc-mt-3"
                                >
                                    // Âmbito instituição: o acesso dentro de unidades e
                                    // workspaces vive nos separadores próprios.
                                    <input type="hidden" name="scope" value="institution" />
                                    <select class="oc-select" name="permission" required>
                                        <option value="">{crate::i18n::t("admin.grants.choose")}</option>
                                        {catalogo
                                            .clone()
                                            .into_iter()
                                            .map(|p| {
                                                let rotulo = p.clone();
                                                view! { <option value=p>{rotulo}</option> }
                                            })
                                            .collect_view()}
                                    </select>
                                    <input
                                        class="oc-input oc-fill"
                                        type="text"
                                        name="reason"
                                        required
                                        minlength="4"
                                        placeholder=crate::i18n::t("admin.reason.audit_ph")
                                    />
                                    <button class="oc-btn oc-btn--primary" type="submit">
                                        {crate::i18n::t("admin.grants.grant_submit")}
                                    </button>
                                </form>
                            }
                                .into_any()
                        }}
                    },
                )}
            </div>
        })}
    }
}

/// Estados de conta para os quais faz sentido transitar, dado o estado actual.
///
/// Nunca se oferece o estado corrente, nem `invited` como destino — para esse
/// volta-se pela emissão de credencial, não por uma mudança de estado.
fn account_transitions(current: &str) -> Vec<(&'static str, &'static str)> {
    match current {
        "active" | "invited" => vec![
            ("suspended", crate::i18n::t("admin.transition.suspend")),
            ("disabled", crate::i18n::t("admin.transition.disable")),
        ],
        "suspended" => vec![
            ("active", crate::i18n::t("admin.transition.reactivate")),
            ("disabled", crate::i18n::t("admin.transition.disable_short")),
        ],
        "disabled" => vec![("active", crate::i18n::t("admin.transition.reactivate"))],
        _ => Vec::new(),
    }
}

/// Administração da **credencial e do estado** de uma conta: repor
/// palavra-passe e transitar o estado.
///
/// # Autoridade
///
/// Renderiza-se apenas quando o Core diz que o actor pode gerir a conta
/// (`overview.may_manage_account`). A proibição de o administrador se
/// auto-bloquear, e a de deixar a instituição sem um administrador capaz de
/// entrar, vivem no Core, no momento da operação — não neste ecrã. Se o Core
/// recusar, a razão volta ao detalhe.
pub fn account_admin(person_id: &str, overview: &Value) -> impl IntoView {
    let pode_gerir = overview
        .get("may_manage_account")
        .and_then(Value::as_bool)
        .unwrap_or(false);

    let status = text(overview, "account_status").to_owned();
    let transicoes = account_transitions(&status);
    let sem_transicoes = transicoes.is_empty();
    let accao_estado = format!("/admin/members/{person_id}/status");
    let accao_reset = format!("/admin/members/{person_id}/reset-password");

    view! {
        {pode_gerir.then(|| view! {
            <div class="oc-mt-6">
                {card(
                    section_head(crate::i18n::t("admin.account.manage"), None, None),
                    view! {
                        <div>
                            <div>
                                <p class="oc-muted">
                                    {crate::i18n::t("admin.account.reset_note")}
                                </p>
                                <form method="post" action=accao_reset.clone() class="oc-mt-3">
                                    <button class="oc-btn oc-btn--danger" type="submit">
                                        {crate::i18n::t("admin.account.reset_submit")}
                                    </button>
                                </form>
                            </div>

                            <div class="oc-mt-6">
                            {if sem_transicoes {
                                view! {
                                    <p class="oc-muted">
                                        {crate::i18n::t("admin.account.no_transitions")}
                                    </p>
                                }
                                    .into_any()
                            } else {
                                view! {
                                    <form
                                        method="post"
                                        action=accao_estado.clone()
                                        class="oc-row oc-row--wrap oc-gap-3"
                                    >
                                        <select class="oc-select" name="status" required>
                                            <option value="">{crate::i18n::t("admin.account.change_state")}</option>
                                            {transicoes
                                                .clone()
                                                .into_iter()
                                                .map(|(value, label)| view! {
                                                    <option value=value>{label}</option>
                                                })
                                                .collect_view()}
                                        </select>
                                        <input
                                            class="oc-input oc-fill"
                                            type="text"
                                            name="reason"
                                            required
                                            minlength="4"
                                            placeholder=crate::i18n::t("admin.reason.audit_ph")
                                        />
                                        <button class="oc-btn oc-btn--primary" type="submit">
                                            {crate::i18n::t("admin.action.apply")}
                                        </button>
                                    </form>
                                }
                                    .into_any()
                            }}
                            </div>
                        </div>
                    },
                )}
            </div>
        })}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const PID: &str = "11111111-1111-1111-1111-111111111111";
    const UID: &str = "33333333-3333-3333-3333-333333333333";

    /// A posição institucional lê-se em português, e um código desconhecido
    /// aparece como está em vez de desaparecer.
    #[test]
    fn a_posicao_institucional_le_se_em_portugues() {
        assert_eq!(position_label("founder"), "Fundador");
        assert_eq!(position_label("director"), "Director");
        assert_eq!(position_label(""), "—");
        // Um código que este build não conhece é visível, não engolido.
        assert_eq!(position_label("chair_of_the_board"), "chair_of_the_board");
        // «founder» cru — o que aparecia na coluna — não sobrevive à tradução.
        assert_ne!(position_label("founder"), "founder");
    }

    /// A barra do topo do membro não finge separadores.
    ///
    /// As quatro secções que existem levam a conteúdo real por âncora, e o
    /// conteúdo tem o alvo onde aterrar; as três que não têm secção declaram-se
    /// indisponíveis com a razão de cada uma. A categoria proibida é o `<span>`
    /// com aspecto de separador e sem destino nem razão — que era o que estava
    /// aqui, escondido de `nenhuma_tab_e_decorativa` por usar `role="tablist"` em
    /// vez de `role="tab"`.
    #[test]
    fn os_separadores_do_membro_levam_a_seccoes_ou_dizem_porque_nao() {
        let person = json!({
            "id": PID,
            "full_name": "Ana Fernandes",
            "email": "ana@ocinye.com",
            "status": "active",
            "institutional_position": "Investigadora",
        });
        let html = member_detail(
            &person,
            &json!({ "account_status": "active" }),
            &json!({}),
            &json!([]),
            &json!({ "items": [] }),
            &json!([]),
            None,
        )
        .to_html();

        for (href, id) in [
            ("#membro-overview", "id=\"membro-overview\""),
            ("#membro-acesso", "id=\"membro-acesso\""),
            ("#membro-seguranca", "id=\"membro-seguranca\""),
            ("#membro-unidades", "id=\"membro-unidades\""),
            (
                "#membro-research-workspaces",
                "id=\"membro-research-workspaces\"",
            ),
        ] {
            assert!(
                html.contains(&format!("href=\"{href}\"")),
                "o separador {href} deixou de ser uma âncora para a sua secção"
            );
            assert!(
                html.contains(id),
                "a secção {id} não existe para a âncora do separador aterrar"
            );
        }
        assert!(
            !html.contains("role=\"tablist\""),
            "voltou o tablist decorativo: spans com aspecto de separador e sem destino"
        );

        for razao in [
            "A actividade por membro ainda não é uma consulta do Core.",
            "A auditoria por membro ainda não é uma consulta do Core.",
        ] {
            assert!(
                html.contains(razao),
                "um separador indisponível perdeu a sua razão: {razao}"
            );
        }
        assert!(
            !html.contains("title=\"Ainda não disponível\""),
            "um separador do membro ainda usa a razão genérica em vez de dizer qual"
        );
    }

    /// O separador activo está marcado, e o inactivo não se confunde com o
    /// indisponível.
    ///
    /// Por omissão — sem JavaScript, sem âncora — «Overview» é a secção activa,
    /// com `aria-current="location"` (a mesma convenção do resto da navegação, e
    /// o que o leitor de ecrã anuncia). O `app.js` mantém-no conforme a âncora e
    /// o scroll; o servidor garante o ponto de partida. Os separadores
    /// indisponíveis (`Actividade`, `Audit`) continuam `aria-disabled` e **nunca**
    /// activos: inactivo é uma coisa, indisponível é outra.
    #[test]
    fn o_separador_activo_do_membro_esta_marcado() {
        let person = json!({
            "id": PID,
            "full_name": "Ana Fernandes",
            "email": "ana@ocinye.com",
            "status": "invited",
            "institutional_position": "Investigadora",
        });
        let html = member_detail(
            &person,
            &json!({ "account_status": "invited" }),
            &json!({}),
            &json!([]),
            &json!({ "items": [] }),
            &json!([]),
            None,
        )
        .to_html();

        // A barra é o gancho que o `app.js` procura.
        assert!(
            html.contains("data-oc-section-nav"),
            "a barra de secções perdeu o gancho que o app.js segue"
        );
        // «Overview» nasce activa, e é a única.
        assert!(
            html.contains("href=\"#membro-overview\" aria-current=\"location\""),
            "o Overview devia nascer activo, com aria-current=location"
        );
        assert_eq!(
            html.matches("aria-current=\"location\"").count(),
            1,
            "só um separador de secção pode estar activo de partida"
        );
        // Inactivo não é desactivado: os separadores navegáveis não são
        // aria-disabled.
        assert!(
            html.contains("href=\"#membro-acesso\"")
                && !html.contains("#membro-acesso\" aria-disabled"),
            "um separador inactivo não pode parecer desactivado"
        );
        // Indisponível continua indisponível, e nunca activo.
        assert!(
            html.contains("aria-disabled=\"true\""),
            "os separadores indisponíveis perderam o seu estado"
        );
    }

    /// O Overview do membro diz o estado do segundo factor, e lê os papéis em
    /// português. A regra do MFA vem do Core; o ecrã só a apresenta.
    #[test]
    fn o_overview_mostra_o_segundo_factor_e_os_papeis() {
        let enrolado = overview_tab(
            "Fundador",
            "active",
            &json!({
                "mfa_required": true,
                "mfa_enrolled": true,
                "has_permanent_password": true,
                "live_sessions": [{}]
            }),
            &json!({ "roles": ["research_lead"] }),
        )
        .to_html();
        assert!(enrolado.contains("Exigido e enrolado"));
        assert!(
            enrolado.contains("Líder de investigação"),
            "os papéis técnicos lêem-se em português"
        );

        let por_enrolar = overview_tab(
            "Investigador",
            "invited",
            &json!({ "mfa_required": true, "mfa_enrolled": false, "live_sessions": [] }),
            &json!({ "roles": [] }),
        )
        .to_html();
        assert!(por_enrolar.contains("Exigido, por enrolar"));
        assert!(
            por_enrolar.contains("Nenhum"),
            "sem papéis técnicos diz-se «Nenhum», não um espaço vazio"
        );

        let nao_exigido = overview_tab(
            "Investigador",
            "active",
            &json!({ "mfa_required": false, "mfa_enrolled": false, "live_sessions": [] }),
            &json!({}),
        )
        .to_html();
        assert!(nao_exigido.contains("Não exigido"));
    }

    /// Sem unidades na instituição, não se oferece atribuir: encaminha-se para
    /// as criar. Zero Dead UI — o controlo diz porque não está disponível.
    #[test]
    fn sem_unidades_na_org_encaminha_para_criar() {
        let html = units_admin(PID, &json!({ "units": [] }), &json!([])).to_html();
        assert!(html.contains("Ainda não existem unidades"));
        assert!(html.contains("href=\"/units/new\""));
        assert!(html.contains("Nenhuma unidade atribuída"));
    }

    /// Com unidades e o membro sem nenhuma: estado vazio + formulário de
    /// atribuição com as unidades elegíveis e a acção correcta.
    #[test]
    fn oferece_atribuir_quando_o_membro_nao_tem_unidades() {
        let html = units_admin(
            PID,
            &json!({ "units": [] }),
            &json!([{ "id": UID, "name": "Inteligência Artificial", "code": "AI" }]),
        )
        .to_html();
        assert!(html.contains("Nenhuma unidade atribuída"));
        assert!(html.contains(&format!("action=\"/admin/members/{PID}/units\"")));
        assert!(html.contains("Inteligência Artificial"));
        assert!(html.contains(">Atribuir<"));
    }

    /// Uma pertença actual mostra-se com nome, papel, e as acções de alterar
    /// papel e remover — apontando às rotas certas.
    #[test]
    fn mostra_a_pertenca_e_as_accoes() {
        let html = units_admin(
            PID,
            &json!({ "units": [{ "id": UID, "role": "manager" }] }),
            &json!([{ "id": UID, "name": "Inteligência Artificial", "code": "AI" }]),
        )
        .to_html();
        assert!(html.contains("Inteligência Artificial"));
        assert!(html.contains(&format!("action=\"/admin/members/{PID}/units/{UID}/role\"")));
        assert!(html.contains(&format!(
            "action=\"/admin/members/{PID}/units/{UID}/remove\""
        )));
        // O papel actual vem seleccionado.
        assert!(html.contains("value=\"manager\" selected"));
        // Já membro dessa unidade: não reaparece na lista de atribuir.
        assert!(html.contains("já pertence a todas as unidades"));
    }

    const WID: &str = "77777777-7777-7777-7777-777777777777";

    /// Research workspaces: sem nenhum visível, não se inventa um picker.
    #[test]
    fn workspaces_sem_catalogo_diz_o() {
        let html =
            workspaces_admin(PID, &json!({ "workspaces": [] }), &json!({ "items": [] })).to_html();
        assert!(html.contains("Nenhum research workspace atribuído"));
        assert!(html.contains("Ainda não existem research workspaces"));
    }

    /// Com workspaces e o membro sem nenhum: oferece atribuir, com o título
    /// (não o código) e a acção certa.
    #[test]
    fn workspaces_oferece_atribuir() {
        let html = workspaces_admin(
            PID,
            &json!({ "workspaces": [] }),
            &json!({ "items": [{ "id": WID, "title": "Modelos de linguagem", "code": "LLM", "kind": "project" }] }),
        )
        .to_html();
        assert!(html.contains("Nenhum research workspace atribuído"));
        assert!(html.contains(&format!("action=\"/admin/members/{PID}/workspaces\"")));
        assert!(html.contains("Modelos de linguagem"));
    }

    /// Uma pertença mostra o papel (Lead/Membro/Leitor) e as acções às rotas
    /// certas; o papel actual vem seleccionado.
    #[test]
    fn workspaces_mostra_pertenca_e_papel() {
        let html = workspaces_admin(
            PID,
            &json!({ "workspaces": [{ "id": WID, "role": "lead" }] }),
            &json!({ "items": [{ "id": WID, "title": "Modelos de linguagem", "code": "LLM" }] }),
        )
        .to_html();
        assert!(html.contains("Modelos de linguagem"));
        assert!(html.contains(&format!(
            "action=\"/admin/members/{PID}/workspaces/{WID}/role\""
        )));
        assert!(html.contains(&format!(
            "action=\"/admin/members/{PID}/workspaces/{WID}/remove\""
        )));
        assert!(html.contains("value=\"lead\" selected"));
    }

    /// A oferta vem do Core, e o ecrã não a reinventa.
    #[test]
    fn dar_acesso_aparece_quando_o_core_diz_que_pode() {
        let html = security_tab(
            "11111111-1111-1111-1111-111111111111",
            &json!({"account_status": "active", "may_be_provisioned": true}),
            None,
        )
        .to_html();
        assert!(
            html.contains("Dar acesso"),
            "a operação não chega a quem administra"
        );
        assert!(
            html.contains(
                r#"action="/admin/members/11111111-1111-1111-1111-111111111111/provision""#
            ),
            "o formulário não aponta para a pessoa que está a ser vista"
        );
    }

    /// Revogar uma sessão individual: o botão só aparece com autoridade do
    /// actor, e aponta para a sessão certa daquele membro.
    #[test]
    fn revogar_sessao_so_com_autoridade_do_actor() {
        const PID: &str = "11111111-1111-1111-1111-111111111111";
        const SID: &str = "aaaaaaaa-1111-2222-3333-444444444444";
        let sessao = json!({
            "account_status": "active",
            "may_manage_account": true,
            "live_sessions": [{"id": SID, "state": "active", "user_agent": "Firefox", "ip_prefix": "10.0.0.0/24"}]
        });
        let com = security_tab(PID, &sessao, None).to_html();
        assert!(com.contains(&format!(
            "action=\"/admin/members/{PID}/sessions/{SID}/revoke\""
        )));

        // Sem o sinal do actor, nenhuma sessão ganha botão.
        let sem = json!({
            "account_status": "active",
            "may_manage_account": false,
            "live_sessions": [{"id": SID, "state": "active", "user_agent": "Firefox", "ip_prefix": "10.0.0.0/24"}]
        });
        let html = security_tab(PID, &sem, None).to_html();
        assert!(
            !html.contains("/sessions/"),
            "revogar apareceu sem autoridade"
        );
    }

    /// Com a resposta do Core em falta, o ecrã cala-se.
    ///
    /// Um `unwrap_or(true)` mostraria o botão sempre que a consulta falhasse, e
    /// quem administra carregaria nele para receber uma recusa que parece falta
    /// de autoridade sua.
    #[test]
    fn sem_resposta_do_core_a_operacao_nao_e_oferecida() {
        for resposta in [json!({"account_status": "active"}), json!(null)] {
            let html = security_tab("abc", &resposta, None).to_html();
            assert!(
                !html.contains("Dar acesso"),
                "ofereceu a operação sem o Core a ter autorizado: {resposta}"
            );
        }
    }

    /// Quem já tem acesso não vê o botão — vê a razão, se tentou.
    #[test]
    fn a_recusa_do_core_e_mostrada_e_o_botao_desaparece() {
        let html = security_tab(
            "abc",
            &json!({"account_status": "active", "may_be_provisioned": false}),
            Some("Esta pessoa já tem acesso. Use a reposição de palavra-passe."),
        )
        .to_html();
        assert!(
            html.contains("reposição de palavra-passe"),
            "a razão do Core foi engolida pelo caminho"
        );
        assert!(
            html.contains("oc-callout--error"),
            "a razão não está marcada como recusa"
        );
        assert!(
            !html.contains(r#"type="submit""#),
            "voltou a oferecer a operação que o Core acabou de recusar"
        );
    }

    /// O texto diz o que a operação faz e o que **não** faz.
    #[test]
    fn dar_acesso_nao_se_confunde_com_dar_autoridade() {
        let html = security_tab(
            "abc",
            &json!({"account_status": "active", "may_be_provisioned": true}),
            None,
        )
        .to_html();
        assert!(
            html.contains("não lhe altera papéis, unidades nem autoridade"),
            "nada distingue dar entrada de dar poder"
        );
    }

    #[test]
    fn o_formulario_separa_posicao_institucional_de_papel_tecnico() {
        let html = new_member(&json!({"items": []}), None).to_html();
        assert!(html.contains("Posição institucional"));
        assert!(html.contains("Papel técnico"));
        assert!(
            html.contains("Não concede acesso a nada"),
            "o formulário tem de dizer que a posição não concede acesso"
        );
    }

    #[test]
    fn sem_unidades_o_selector_diz_o_e_fica_desactivado() {
        let html = new_member(&json!({"items": []}), None).to_html();
        assert!(html.contains("Ainda não existem unidades"));
        let select = &html[html.find(r#"id="m-unit""#).expect("selector")..];
        let select = &select[..select.find("</select>").expect("fim")];
        assert!(select.contains("disabled"));
    }

    #[test]
    fn o_formulario_nao_oferece_escolher_a_palavra_passe() {
        // O administrador nunca escolhe a credencial (briefing §16, §43).
        let html = new_member(&json!({"items": []}), None).to_html();
        assert!(!html.contains(r#"type="password""#));
        assert!(!html.contains(r#"name="password""#));
        assert!(html.contains("gera uma palavra-passe temporária"));
    }

    #[test]
    fn o_formulario_explica_o_que_acontece_a_seguir() {
        let html = new_member(&json!({"items": []}), None).to_html();
        assert!(html.contains("apresentada uma única vez"));
        assert!(html.contains("canal seguro"));
    }

    #[test]
    fn a_credencial_e_declarada_como_apresentada_uma_unica_vez() {
        let html = issued_credential(
            "afernandes@ocinye.com",
            "AAAA-BBBB-CCCC",
            "2026-08-23T10:00:00Z",
        )
        .to_html();
        assert!(html.contains("só é apresentada uma vez"));
        assert!(html.contains("afernandes@ocinye.com"));
        assert!(html.contains("2026-08-23 10:00"));
    }

    #[test]
    fn a_credencial_esta_coberta_por_omissao() {
        let html = issued_credential(
            "afernandes@ocinye.com",
            "AAAA-BBBB-CCCC",
            "2026-08-23T10:00:00Z",
        )
        .to_html();
        // O valor está no atributo para o botão «Mostrar», mas o texto visível
        // é a máscara: um ecrã partilhado não a revela sozinho.
        assert!(html.contains("••••"));
        assert!(html.contains(r#"data-oc="secret-toggle""#));
        assert!(html.contains(r#"data-oc="secret-copy""#));
    }

    #[test]
    fn o_separador_de_seguranca_nunca_mostra_material_de_credencial() {
        let html = security_tab(
            "abc",
            &json!({
                "account_status": "active",
                "has_permanent_password": true,
                "password_changed_at": "2026-08-22T10:00:00Z",
                "recent_failed_attempts": 3,
                "live_sessions": [
                    {"state": "active", "user_agent": "Firefox", "ip_prefix": "10.0.0.0/24"}
                ]
            }),
            None,
        )
        .to_html();

        assert!(html.contains("Definida pelo próprio"));
        assert!(html.contains("Firefox"));
        for proibido in ["argon2", "$argon2id$", "verifier", "hash", "token_digest"] {
            assert!(
                !html.to_lowercase().contains(proibido),
                "expõe «{proibido}»"
            );
        }
    }

    #[test]
    fn o_separador_de_seguranca_declara_quando_nao_ha_palavra_passe_definitiva() {
        let html = security_tab(
            "abc",
            &json!({
                "account_status": "invited",
                "has_permanent_password": false,
                "temporary_credential_expires_at": "2026-08-23T10:00:00Z",
                "live_sessions": []
            }),
            None,
        )
        .to_html();
        assert!(html.contains("Ainda não definida"));
        assert!(html.contains("Sem sessões activas."));
    }

    /// Um convite com a credencial temporária expirada oferece **reemitir**, e
    /// apresenta a data passada como expirada — não como uma expiração futura.
    ///
    /// É o estado exacto que dava «An unexpected error occurred» ao carregar em
    /// «Dar acesso»: agora a acção diz o que faz, e a data diz a verdade.
    #[test]
    fn convite_expirado_oferece_reemitir_e_diz_expirada() {
        let html = security_tab(
            "abc",
            &json!({
                "account_status": "invited",
                "has_permanent_password": false,
                "temporary_credential_expires_at": "2026-09-08T10:00:00Z",
                "temporary_credential_expired": true,
                "may_be_provisioned": true,
                "live_sessions": []
            }),
            None,
        )
        .to_html();
        assert!(
            html.contains("Reemitir acesso"),
            "um convite expirado devia oferecer reemitir o acesso"
        );
        assert!(
            !html.contains("Dar acesso"),
            "não é a primeira entrega: não se diz «dar acesso» a quem já foi provisionado"
        );
        assert!(
            html.contains("Expirada em"),
            "uma data já passada aparece como «Expirada em», e não como expiração futura"
        );
        assert!(
            !html.contains("Credencial temporária expira"),
            "a rotulagem antiga apresentava uma data passada como se ainda fosse futura"
        );
    }

    #[test]
    fn o_separador_de_acesso_diz_a_origem_de_cada_permissao() {
        let html = access_tab(&json!({
            "roles": ["research_member"],
            "grants": [],
            "institution_permissions": [
                {"permission": "ideas.view", "source": "technical_role"},
                {"permission": "documents.download", "source": "explicit_grant"}
            ]
        }))
        .to_html();

        assert!(html.contains("ideas.view"));
        assert!(html.contains("papel técnico"));
        assert!(html.contains("grant explícito"));
    }

    #[test]
    fn sem_permissoes_institucionais_o_ecra_nao_conclui_ausencia_de_acesso() {
        // Alguém pode não ter nada à escala institucional e ter tudo dentro de
        // um research workspace. Dizer «sem acesso» seria falso.
        let html = access_tab(&json!({
            "roles": [],
            "grants": [],
            "institution_permissions": []
        }))
        .to_html();
        assert!(html.contains("Não significa nenhum"));
    }

    #[test]
    fn o_detalhe_declara_que_a_posicao_nao_concede_acesso() {
        let html = member_detail(
            &json!({
                "full_name": "Ana Fernandes",
                "email": "afernandes@ocinye.com",
                "status": "active",
                "institutional_position": "founder"
            }),
            &json!({"account_status": "active", "has_permanent_password": true, "live_sessions": []}),
            &json!({"roles": ["research_member"], "grants": [], "institution_permissions": []}),
            &json!({"items": []}),
            &json!({"items": []}),
            &json!([]),
            None,
        )
        .to_html();

        assert!(html.contains("Ana Fernandes"));
        assert!(html.contains("afernandes@ocinye.com"));
        assert!(
            html.contains("não concede acesso"),
            "o detalhe tem de dizer que «Fundador» não é uma permissão"
        );
    }

    #[test]
    fn o_detalhe_nunca_expoe_material_de_credencial() {
        let html = member_detail(
            &json!({"full_name": "A", "email": "a@b.c", "status": "active"}),
            &json!({"account_status": "active", "has_permanent_password": true, "live_sessions": []}),
            &json!({"roles": [], "grants": [], "institution_permissions": []}),
            &json!({"items": []}),
            &json!({"items": []}),
            &json!([]),
            None,
        )
        .to_html()
        .to_lowercase();

        for proibido in ["argon2", "verifier", "token_digest", "password\":"] {
            assert!(!html.contains(proibido), "expõe «{proibido}»");
        }
    }

    #[test]
    fn uma_origem_desconhecida_e_declarada_e_nao_inventada() {
        assert_eq!(source_label("something_new"), "origem desconhecida");
    }

    // ── Fatia 3: Acesso / Segurança ──────────────────────────────────────

    /// A autoridade é do actor. Sem o sinal do Core, não há como **gerir**
    /// papéis — nem revogar, nem conceder —, mas a condição é **explicada** em
    /// vez de a opção desaparecer sem rasto.
    #[test]
    fn gerir_papeis_so_aparece_quando_o_actor_pode() {
        let sem = roles_admin(
            PID,
            &json!({ "roles": ["research_member"], "may_manage_roles": false }),
        )
        .to_html();
        assert!(!sem.contains("Gerir papéis técnicos"));
        assert!(!sem.contains("/roles/research_member/revoke"));
        // Mas a razão pela qual não pode gerir é dita, não escondida.
        assert!(sem.contains("Papéis técnicos"));
        assert!(sem.contains("segundo factor"));

        let com = roles_admin(
            PID,
            &json!({ "roles": ["research_member"], "may_manage_roles": true }),
        )
        .to_html();
        assert!(com.contains("Gerir papéis técnicos"));
        // Revoga o papel que tem…
        assert!(com.contains(&format!(
            "action=\"/admin/members/{PID}/roles/research_member/revoke\""
        )));
        // …e oferece conceder um que não tem, pela acção certa.
        assert!(com.contains(&format!("action=\"/admin/members/{PID}/roles\"")));
        assert!(com.contains("value=\"platform_admin\""));
        // Não reoferece o que já tem.
        assert!(!com.contains("value=\"research_member\""));
    }

    /// Só grants vivos podem ser revogados; um já revogado não ganha botão.
    #[test]
    fn gerir_grants_revoga_so_os_vivos_e_gated_pelo_actor() {
        let access = json!({
            "may_manage_grants": true,
            "grants": [
                { "id": "aaaaaaaa-0000-0000-0000-000000000001", "permission": "datasets.manage", "scope": "institution", "revoked_at": null },
                { "id": "aaaaaaaa-0000-0000-0000-000000000002", "permission": "ai.use", "scope": "institution", "revoked_at": "2026-09-01T00:00:00Z" }
            ]
        });
        let html = grants_admin(PID, &access, &json!(["datasets.manage", "ai.use"])).to_html();
        assert!(html.contains("Gerir grants institucionais"));
        // O vivo tem botão de revogar…
        assert!(html.contains("action=\"/admin/members/11111111-1111-1111-1111-111111111111/grants/aaaaaaaa-0000-0000-0000-000000000001/revoke\""));
        // …o já revogado, não.
        assert!(!html.contains("grants/aaaaaaaa-0000-0000-0000-000000000002/revoke"));
        // O formulário de conceder fixa o âmbito instituição.
        assert!(html.contains("name=\"scope\" value=\"institution\""));

        let sem = grants_admin(
            PID,
            &json!({ "may_manage_grants": false, "grants": [] }),
            &json!([]),
        )
        .to_html();
        assert!(!sem.contains("Gerir grants institucionais"));
    }

    /// As transições oferecidas dependem do estado actual, e nunca oferecem o
    /// estado corrente nem `invited`.
    #[test]
    fn transicoes_de_conta_dependem_do_estado() {
        assert_eq!(
            account_transitions("active"),
            vec![
                (
                    "suspended",
                    "Suspender — barra o acesso, preserva a autoria"
                ),
                (
                    "disabled",
                    "Desactivar — barra permanentemente, mantém o histórico"
                ),
            ]
        );
        assert_eq!(
            account_transitions("disabled"),
            vec![("active", "Reactivar — devolve o acesso")]
        );
        // Estado desconhecido não inventa transições.
        assert!(account_transitions("qualquer").is_empty());
    }

    /// A gestão de credencial e estado é gated pelo sinal do actor, e oferece
    /// repor palavra-passe e as transições certas.
    #[test]
    fn gerir_conta_gated_pelo_actor_e_com_as_accoes_certas() {
        let com = account_admin(
            PID,
            &json!({ "account_status": "active", "may_manage_account": true }),
        )
        .to_html();
        assert!(com.contains("Gerir credencial e estado"));
        assert!(com.contains(&format!("action=\"/admin/members/{PID}/reset-password\"")));
        assert!(com.contains(&format!("action=\"/admin/members/{PID}/status\"")));
        assert!(com.contains("value=\"suspended\""));

        let sem = account_admin(
            PID,
            &json!({ "account_status": "active", "may_manage_account": false }),
        )
        .to_html();
        assert!(!sem.contains("Gerir credencial e estado"));
        assert!(!sem.contains("reset-password"));
    }
}

#[cfg(test)]
mod pureza_i18n {
    use super::*;
    use serde_json::json;

    /// Um ecrã, um idioma: a Administração de membros em francês, sem chrome
    /// português.
    ///
    /// Cobre a criação (`new_member`), a credencial emitida (`issued_credential`)
    /// e o detalhe do membro (`member_detail`) — que reúne o resumo, o acesso, a
    /// segurança, as unidades e os research workspaces, incluindo as secções de
    /// gestão. A prosa sensível à segurança tem de sair em francês: se sobrevive
    /// em português, o membro que escolheu francês lê a garantia na língua errada.
    #[tokio::test]
    async fn a_administracao_nao_mistura_linguas() {
        use crate::i18n::{with_locale, Locale};

        let person = json!({
            "id": "11111111-1111-1111-1111-111111111111",
            "full_name": "Ana Fernandes",
            "email": "ana@ocinye.com",
            "status": "active",
            "institutional_position": "founder"
        });
        let security = json!({
            "account_status": "active",
            "has_permanent_password": true,
            "mfa_required": true,
            "mfa_enrolled": false,
            "password_changed_at": "2026-08-22T09:14:00Z",
            "last_successful_sign_in": "2026-08-22T10:31:00Z",
            "recent_failed_attempts": 0,
            "may_manage_account": true,
            "live_sessions": [{
                "id": "aaaaaaaa-1111-2222-3333-444444444444",
                "state": "active",
                "user_agent": "Firefox",
                "ip_prefix": "10.0.0.0/24"
            }]
        });
        let access = json!({
            "roles": ["research_member"],
            "grants": [],
            "may_manage_roles": true,
            "may_manage_grants": true,
            "institution_permissions": [
                {"permission": "ideas.view", "source": "technical_role"}
            ],
            "units": [],
            "workspaces": []
        });

        let fr = with_locale(Locale::Fr, async {
            let criar = new_member(&json!({ "items": [] }), None).to_html();
            let credencial =
                issued_credential("ana@ocinye.com", "AAAA-BBBB-CCCC", "2026-08-23T10:00:00Z")
                    .to_html();
            let detalhe = member_detail(
                &person,
                &security,
                &access,
                &json!({ "items": [] }),
                &json!({ "items": [] }),
                &json!([]),
                None,
            )
            .to_html();
            format!("{criar}{credencial}{detalhe}")
        })
        .await;

        for francesa in [
            "Ajouter un membre",
            "Créer le membre",
            "Ne donne accès à rien.",
            "Rôle technique",
            "Utilisateur créé",
            "Ce mot de passe n’est affiché qu’une seule fois.",
            "MEMBRE",
            "État du compte",
            "Rôles techniques",
            "Sessions actives",
            "Sections du membre",
            "Appartenances aux unités",
            "Gérer les rôles techniques",
            "Réinitialiser le mot de passe",
        ] {
            assert!(fr.contains(francesa), "fr: falta «{francesa}»");
        }

        for portuguesa in [
            "Adicionar membro",
            "Criar membro",
            "Não concede acesso a nada.",
            "Papel técnico",
            "Utilizador criado",
            "só é apresentada uma vez",
            "Estado da conta",
            "Papéis técnicos",
            "Sessões activas",
            "Secções do membro",
            "Gerir papéis técnicos",
            "Repor palavra-passe",
            "Em resumo",
            "Posição institucional",
        ] {
            assert!(
                !fr.contains(portuguesa),
                "fr: chrome português «{portuguesa}» sobreviveu"
            );
        }
    }
}
