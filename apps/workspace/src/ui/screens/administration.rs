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

fn text<'a>(value: &'a Value, key: &str) -> &'a str {
    value.get(key).and_then(Value::as_str).unwrap_or("—")
}

fn day(value: &Value, key: &str) -> String {
    value.get(key).and_then(Value::as_str).map_or_else(
        || "—".to_owned(),
        |stamp| stamp[..stamp.len().min(10)].to_owned(),
    )
}

/// Os papéis técnicos oferecidos ao criar um membro.
///
/// Ordenados do mais estreito para o mais amplo, de propósito: a primeira opção
/// deve ser a que se escolhe por omissão, e a última a que exige pensar.
const ROLES: [(&str, &str); 8] = [
    ("research_member", "Investigador — acesso científico comum"),
    ("research_lead", "Research Lead — lidera ideias e projectos"),
    ("collaborator", "Colaborador — âmbito estreito"),
    (
        "external_collaborator",
        "Colaborador externo — só o que for atribuído",
    ),
    ("unit_manager", "Gestor de unidade"),
    ("auditor", "Auditor — evidência, sem conteúdo"),
    ("organisation_admin", "Administrador da organização"),
    ("platform_admin", "Administrador da plataforma"),
];

/// Posições institucionais. **Não concedem acesso** (ADR-0100).
const POSITIONS: [(&str, &str); 9] = [
    ("researcher", "Investigador"),
    ("engineer", "Engenheiro"),
    ("principal_investigator", "Investigador principal"),
    ("unit_lead", "Responsável de unidade"),
    ("fellow", "Bolseiro"),
    ("student", "Estudante"),
    ("director", "Director"),
    ("founder", "Fundador"),
    ("external_collaborator", "Colaborador externo"),
];

/// Ecrã «Adicionar utilizador».
///
/// Um formulário e não um assistente de cinco passos: os campos cabem num ecrã,
/// e dividi-los esconderia que a posição institucional e o papel técnico são
/// decisões independentes que se tomam ao mesmo tempo.
pub fn new_member(units: &Value, message: Option<String>) -> impl IntoView {
    let unit_rows: Vec<(String, String)> = units
        .get("items")
        .and_then(Value::as_array)
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
                    <h1>"Adicionar membro"</h1>
                    <p>
                        "O Ocinye Core gera uma palavra-passe temporária. O membro terá de
                         definir a sua no primeiro acesso."
                    </p>
                </div>
            </div>

            {message
                .map(|text| {
                    view! { <div class="oc-callout oc-callout--error" role="alert">{text}</div> }
                })}

            <form method="post" action="/admin/members/new" class="oc-split oc-split--2">
                <section class="oc-card">
                    <div class="oc-card__head"><h2>"IDENTIDADE"</h2></div>
                    <div class="oc-card__body">
                        <div class="oc-field">
                            <label class="oc-field__label" for="m-name">"Nome completo"</label>
                            <input class="oc-input" id="m-name" name="full_name" required
                                   placeholder="Ex.: Ana Maria Fernandes" />
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
                                "Endereço institucional"
                            </label>
                            <input class="oc-input" id="m-email" name="email" type="email" required
                                   autocapitalize="none" spellcheck="false"
                                   placeholder="ana.fernandes@ocinye.com" />
                            <p class="oc-field__hint">
                                "É a identidade e a credencial de entrada. A convenção da
                                 instituição é primeiro.ultimo@ocinye.com, em minúsculas."
                            </p>
                        </div>
                    </div>
                </section>

                <div>
                    <section class="oc-card oc-mb-5">
                        <div class="oc-card__head"><h2>"ORGANIZAÇÃO"</h2></div>
                        <div class="oc-card__body">
                            <div class="oc-field">
                                <label class="oc-field__label" for="m-position">
                                    "Posição institucional"
                                </label>
                                <select class="oc-select" id="m-position" name="position">
                                    <option value="">"—"</option>
                                    {POSITIONS
                                        .iter()
                                        .map(|(value, label)| {
                                            view! { <option value=*value>{*label}</option> }
                                        })
                                        .collect_view()}
                                </select>
                                <p class="oc-field__hint">
                                    "Verdade organizacional. "
                                    <strong>"Não concede acesso a nada."</strong>
                                </p>
                            </div>

                            <div class="oc-field">
                                <label class="oc-field__label" for="m-unit">"Unidade inicial"</label>
                                <select
                                    class="oc-select"
                                    id="m-unit"
                                    name="unit_id"
                                    disabled=!has_units
                                >
                                    {if !has_units {
                                        view! {
                                            <option value="">"Ainda não existem unidades"</option>
                                        }
                                            .into_any()
                                    } else {
                                        view! {
                                            <option value="">"Sem unidade"</option>
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
                        <div class="oc-card__head"><h2>"ACESSO"</h2></div>
                        <div class="oc-card__body">
                            <div class="oc-field">
                                <label class="oc-field__label" for="m-role">"Papel técnico"</label>
                                <select class="oc-select" id="m-role" name="role" required>
                                    {ROLES
                                        .iter()
                                        .map(|(value, label)| {
                                            view! { <option value=*value>{*label}</option> }
                                        })
                                        .collect_view()}
                                </select>
                                <p class="oc-field__hint">
                                    "Na dúvida, escolha o mais estreito. Alargar depois é um
                                     pedido; estreitar é uma conversa."
                                </p>
                            </div>
                        </div>
                    </section>

                    <section class="oc-card oc-mb-5">
                        <div class="oc-card__body oc-card__body--subtle">
                            <div class="oc-row oc-gap-5">
                                {icon(Icon::Shield, 14)}
                                <strong>"O que acontece a seguir"</strong>
                            </div>
                            <p class="oc-muted">
                                "É gerada uma palavra-passe temporária, válida 24 horas e
                                 apresentada uma única vez. Entregue-a por canal seguro. O membro
                                 não entra no Workspace com ela: serve só para definir a sua."
                            </p>
                        </div>
                    </section>

                    <div class="oc-row oc-gap-5 oc-justify-end">
                        {button(Button::new("Cancelar", Variant::Secondary).href("/admin"))}
                        <button type="submit" class="oc-btn oc-btn--primary">"Criar membro"</button>
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
                    <h1>"Utilizador criado"</h1>
                    <p>"A conta existe. Falta entregar o acesso."</p>
                </div>
            </div>

            <section class="oc-card oc-credential">
                <div class="oc-card__body">
                    <div class="oc-field">
                        <span class="oc-field__label">"Endereço institucional"</span>
                        <div class="oc-credential__value oc-mono">{email}</div>
                    </div>

                    <div class="oc-field">
                        <span class="oc-field__label">"Palavra-passe temporária"</span>
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
                                    "Mostrar"
                                </button>
                                <button
                                    type="button"
                                    class="oc-btn oc-btn--secondary"
                                    data-oc="secret-copy"
                                >
                                    "Copiar"
                                </button>
                            </span>
                        </div>
                    </div>

                    <div class="oc-field">
                        <span class="oc-field__label">"Válida até"</span>
                        <div class="oc-credential__value oc-mono">{expires}" UTC"</div>
                    </div>

                    <div class="oc-callout oc-callout--warning" role="alert">
                        <strong>"Esta palavra-passe só é apresentada uma vez."</strong>
                        " Transmita-a ao membro através de um canal seguro — presencialmente,
                         por voz, ou por mensagem efémera cifrada. Nunca por email, SMS ou chat.
                         Depois de fechar esta página, ninguém a consegue recuperar."
                    </div>
                </div>
            </section>

            <div class="oc-row oc-gap-5">
                {button(Button::new("Concluído", Variant::Primary).href("/admin"))}
            </div>
        </div>
    }
}

/// Separador «Segurança» do detalhe de um membro.
///
/// Só metadados. Nunca um hash, nunca uma palavra-passe (briefing §73).
pub fn security_tab(
    person_id: &str,
    overview: &Value,
    recusa: Option<&str>,
) -> impl IntoView {
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
    let changed = day(overview, "password_changed_at");
    let last_sign_in = day(overview, "last_successful_sign_in");

    let sessions: Vec<Value> = overview
        .get("live_sessions")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let session_count = sessions.len();

    view! {
        <div class="oc-split oc-split--2">
            {card(
                section_head("Credencial", None, None),
                view! {
                    <dl class="oc-facts">
                        <dt>"Estado da conta"</dt>
                        <dd>{badge(status.clone(), Tone::of(&status))}</dd>

                        <dt>"Palavra-passe definitiva"</dt>
                        <dd>
                            {if has_permanent {
                                "Definida pelo próprio"
                            } else {
                                "Ainda não definida"
                            }}
                        </dd>

                        <dt>"Definida em"</dt>
                        <dd class="oc-mono">{changed}</dd>

                        <dt>"Credencial temporária expira"</dt>
                        <dd class="oc-mono">{temporary_expiry}</dd>

                        <dt>"Último acesso"</dt>
                        <dd class="oc-mono">{last_sign_in}</dd>

                        <dt>"Falhas recentes (7 dias)"</dt>
                        <dd class="oc-mono">{failures.to_string()}</dd>
                    </dl>
                },
            )}

            {card(
                section_head(
                    "Sessões activas",
                    None,
                    Some(session_count.to_string()),
                ),
                if sessions.is_empty() {
                    view! { <p class="oc-muted">"Sem sessões activas."</p> }.into_any()
                } else {
                    view! {
                        <div>
                            {sessions
                                .iter()
                                .map(|session| {
                                    let state = text(session, "state").to_owned();
                                    view! {
                                        <div class="oc-list__row">
                                            <span class="oc-fill oc-truncate">
                                                {text(session, "user_agent").to_owned()}
                                            </span>
                                            {badge(state.clone(), Tone::of(&state))}
                                            <span class="oc-mono oc-list__meta">
                                                {text(session, "ip_prefix").to_owned()}
                                            </span>
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
                            section_head("Dar acesso", None, None),
                            view! {
                                <div>
                                    <p class="oc-muted">
                                        "Esta pessoa existe na instituição e ainda não tem como entrar. Dar-lhe acesso emite uma credencial temporária e não lhe altera papéis, unidades nem autoridade."
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
                                                        "Dar acesso"
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
                section_head("Papéis técnicos", None, None),
                if roles.is_empty() {
                    view! { <p class="oc-muted">"Sem papéis atribuídos."</p> }.into_any()
                } else {
                    view! {
                        <div class="oc-row oc-gap-5 oc-wrap">
                            {roles
                                .into_iter()
                                .map(|role| badge(role.clone(), Tone::of(&role)))
                                .collect_view()}
                        </div>
                    }
                        .into_any()
                },
            )}

            {card(
                section_head("Grants explícitos", None, Some(grant_count.to_string())),
                if grants.is_empty() {
                    view! {
                        <p class="oc-muted">
                            "Nenhum. O acesso deste membro vem apenas de papéis e memberships."
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
                                                if revoked { "revogado" } else { "activo" }.to_owned(),
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
                    "Permissões institucionais",
                    None,
                    Some(permission_count.to_string()),
                )}
                <div class="oc-card__body">
                    {if permissions.is_empty() {
                        view! {
                            <p class="oc-muted">
                                "Nenhuma permissão de âmbito institucional. Não significa nenhum
                                 acesso: pode ter permissões dentro de unidades ou de research
                                 workspaces."
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
        "technical_role" => "papel técnico",
        "unit_membership" => "membership de unidade",
        "workspace_membership" => "membership de research workspace",
        "explicit_grant" => "grant explícito",
        // Vocabulário que este build não conhece: dizê-lo é melhor do que
        // inventar uma tradução.
        _ => "origem desconhecida",
    }
}

/// Detalhe de um membro: quem é, o que pode, e o estado da sua credencial.
///
/// Os separadores «Overview», «Units», «Research Workspaces», «Activity» e
/// «Audit» do dossier ficam declarados como indisponíveis em vez de levarem a
/// um ecrã vazio: dois deles existem, e dizê-lo é mais honesto do que sugerir
/// sete que não existem.
pub fn member_detail(
    person: &Value,
    security: &Value,
    access: &Value,
    recusa: Option<&str>,
) -> impl IntoView {
    let person_id = text(person, "id").to_owned();
    let recusa = recusa.map(str::to_owned);
    let name = text(person, "full_name").to_owned();
    // O endereço, uma vez. Havia aqui um `username` ao lado dele, e a linha
    // mostrava a mesma pessoa duas vezes: `afernandes · afernandes@ocinye.com`.
    let email = text(person, "email").to_owned();
    let status = text(person, "status").to_owned();
    let position = person
        .get("institutional_position")
        .and_then(Value::as_str)
        .unwrap_or("—")
        .to_owned();

    let security = security.clone();
    let access = access.clone();

    view! {
        // Mesmas classes do cabeçalho do Research Workspace: um segundo padrão
        // de cabeçalho contextual seria um segundo sítio para os alinhar.
        <div class="oc-band">
            <div class="oc-row--top oc-gap-11 oc-mb-3">
                <div class="oc-fill">
                    <div class="oc-row oc-row--wrap oc-gap-6">
                        <span class="oc-pill">"MEMBRO"</span>
                        <h1 class="oc-t-screen">{name}</h1>
                        {badge(status.clone(), Tone::of(&status))}
                    </div>
                    <div class="oc-mono oc-mt-3">{email}</div>
                    <div class="oc-muted oc-mt-3">
                        "Posição institucional: "{position}
                        " — não concede acesso."
                    </div>
                </div>
            </div>

            <div class="oc-tabs oc-tabs--ctx" role="tablist" aria-label="Separadores do membro">
                <span class="oc-tab" aria-selected="true">"Acesso"</span>
                <span class="oc-tab" aria-selected="false">"Segurança"</span>
                {["Overview", "Unidades", "Research Workspaces", "Actividade", "Audit"]
                    .iter()
                    .map(|label| {
                        view! {
                            <span
                                class="oc-tab oc-unavailable"
                                aria-disabled="true"
                                title="Ainda não disponível"
                            >
                                {*label}
                            </span>
                        }
                    })
                    .collect_view()}
            </div>
        </div>

        <div class="oc-page">
            {section_head("Acesso", None, None)}
            {access_tab(&access)}
            <div class="oc-vspace"></div>
            {section_head("Segurança", None, None)}
            {security_tab(&person_id, &security, recusa.as_deref())}
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// A oferta vem do Core, e o ecrã não a reinventa.
    #[test]
    fn dar_acesso_aparece_quando_o_core_diz_que_pode() {
        let html = security_tab(
            "11111111-1111-1111-1111-111111111111",
            &json!({"account_status": "active", "may_be_provisioned": true}),
            None,
        )
        .to_html();
        assert!(html.contains("Dar acesso"), "a operação não chega a quem administra");
        assert!(
            html.contains(
                r#"action="/admin/members/11111111-1111-1111-1111-111111111111/provision""#
            ),
            "o formulário não aponta para a pessoa que está a ser vista"
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
        let html = security_tab("abc", &json!({
            "account_status": "active",
            "has_permanent_password": true,
            "password_changed_at": "2026-08-22T10:00:00Z",
            "recent_failed_attempts": 3,
            "live_sessions": [
                {"state": "active", "user_agent": "Firefox", "ip_prefix": "10.0.0.0/24"}
            ]
        }), None)
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
        let html = security_tab("abc", &json!({
            "account_status": "invited",
            "has_permanent_password": false,
            "temporary_credential_expires_at": "2026-08-23T10:00:00Z",
            "live_sessions": []
        }), None)
        .to_html();
        assert!(html.contains("Ainda não definida"));
        assert!(html.contains("Sem sessões activas."));
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
}
