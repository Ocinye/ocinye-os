//! Definições do membro.
//!
//! # O que esta superfície é
//!
//! A relação do próprio membro com o Ocinye OS: a sua conta e as suas
//! credenciais. **Não** é administração.
//!
//! A fronteira é deliberada e vale a pena escrevê-la:
//!
//! > O self-service altera as credenciais e as preferências do próprio membro.
//! > Nunca se torna uma porta lateral para a autorização institucional.
//!
//! Papéis, filiações, concessões e estado da conta pertencem a Administração,
//! onde são concedidos por alguém com autoridade para isso — e ficam no trilho
//! de auditoria com autor. Um campo aqui que os alterasse seria uma escalada de
//! privilégio com aspecto de preferência.
//!
//! # As secções
//!
//! `Conta`, `Segurança`, `Idioma e região` e `Aplicações`. A última nasceu com a
//! primeira preferência do membro com persistência e consumidor reais: as
//! **aplicações fixadas** na barra lateral, guardadas no Core (`member_app_pins`)
//! e resolvidas pela sessão. Aqui gere-se o **conjunto** — a ordem muda-se
//! arrastando na própria barra. Continua a valer a fronteira: o self-service
//! altera preferências e credenciais do próprio, nunca a autorização — e as
//! aplicações que aparecem para fixar são as que o membro já pode abrir.

use leptos::prelude::*;
use ocinye_contracts::AvatarChoice;
use serde_json::Value;

use crate::ui::components::{button, card, section_head, text_field, Button, Variant};
use crate::ui::components::{pill_tabs, Tab};
use ocinye_contracts::Locale;

/// Os três separadores das definições, com o rótulo no idioma corrente.
///
/// `activo` é o caminho do separador em que se está. Reunir os três num só sítio
/// impede que uma secção nova acrescente um separador e esqueça outra — a barra
/// é a mesma em toda a página de definições.
fn seccoes_das_definicoes(activo: &str) -> Vec<Tab> {
    vec![
        Tab::link(
            crate::i18n::t("settings.tab.account"),
            "/settings",
            activo == "/settings",
        ),
        Tab::link(
            crate::i18n::t("settings.tab.security"),
            "/settings/security",
            activo == "/settings/security",
        ),
        Tab::link(
            crate::i18n::t("settings.tab.language"),
            "/settings/language",
            activo == "/settings/language",
        ),
        Tab::link(
            crate::i18n::t("settings.tab.apps"),
            "/settings/apps",
            activo == "/settings/apps",
        ),
    ]
}

fn text(payload: &Value, key: &str) -> String {
    payload
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or("—")
        .to_owned()
}

/// Uma linha de facto, em leitura.
fn facto(rotulo: &'static str, valor: String) -> impl IntoView {
    view! {
        <div class="oc-row--between oc-gap-5 oc-list__row">
            <span class="oc-t-meta">{rotulo}</span>
            <span class="oc-t-cell">{valor}</span>
        </div>
    }
}

/// `Definições → Conta`.
///
/// Em leitura. Nome e correio institucional não têm hoje um fluxo
/// de alteração seguro — mudar um endereço institucional exige verificação, e
/// inventar aqui um campo editável seria prometer um processo que não existe.
pub fn account(
    me: &Value,
    organisation: &Value,
    escolha: &AvatarChoice,
    error: Option<String>,
    done: bool,
) -> impl IntoView {
    let nome = text(me, "display_name");
    let correio = text(me, "email");
    let estado = text(me, "status");
    let instituicao = text(organisation, "name");

    view! {
        <div class="oc-page oc-page--narrow">
            <div class="oc-head">
                <div class="oc-head__text">
                    <h1>{crate::i18n::t("settings.title")}</h1>
                    <p>{crate::i18n::t("settings.subtitle")}</p>
                </div>
            </div>

            <div class="oc-tabs oc-tabs--under oc-card__head--flush">
                {pill_tabs(
                    seccoes_das_definicoes("/settings"),
                    crate::i18n::t("settings.tabs.aria"),
                )}
            </div>

            {imagem_de_perfil(escolha, &nome, error, done)}

            {card(
                section_head(crate::i18n::t("settings.account.section"), None, None),
                view! {
                    {facto(crate::i18n::t("settings.field.name"), nome)}
                    {facto(crate::i18n::t("settings.field.email"), correio)}
                    {facto(crate::i18n::t("settings.field.status"), estado)}
                    {facto(crate::i18n::t("settings.field.institution"), instituicao)}
                    <p class="oc-muted oc-t-caption--muted oc-mt-5">
                        {crate::i18n::t("settings.account.managed_note")}
                    </p>
                },
            )}
        </div>
    }
}

/// `Definições → Idioma e região`.
///
/// O idioma é uma preferência de apresentação: cada língua diz-se pelo seu
/// próprio nome, sem bandeiras (i18n §57, §58), e escolher uma não muda o
/// significado de nada — só a língua em que o Ocinye se mostra. Funciona sem
/// JavaScript: é um formulário que submete e volta com a interface na língua
/// escolhida.
pub fn language(saved: bool) -> impl IntoView {
    let actual = crate::i18n::current();
    view! {
        <div class="oc-page oc-page--narrow">
            <div class="oc-head">
                <div class="oc-head__text">
                    <h1>{crate::i18n::t("settings.title")}</h1>
                    <p>{crate::i18n::t("settings.subtitle")}</p>
                </div>
            </div>

            <div class="oc-tabs oc-tabs--under oc-card__head--flush">
                {pill_tabs(
                    seccoes_das_definicoes("/settings/language"),
                    crate::i18n::t("settings.tabs.aria"),
                )}
            </div>

            {saved.then(|| view! {
                <div class="oc-callout" role="status">
                    {crate::i18n::t("settings.language.saved")}
                </div>
            })}

            {card(
                section_head(crate::i18n::t("settings.language_region.title"), None, None),
                view! {
                    <form method="post" action="/settings/language" class="oc-lang">
                        <p class="oc-t-caption--muted oc-mb-5">
                            {crate::i18n::t("settings.language.help")}
                        </p>
                        <fieldset class="oc-lang__set">
                            <legend class="oc-sr">{crate::i18n::t("settings.language.label")}</legend>
                            {Locale::ALL
                                .into_iter()
                                .map(|loc| {
                                    let escolhido = loc == actual;
                                    view! {
                                        <label class="oc-lang__opt">
                                            <input
                                                type="radio"
                                                name="locale"
                                                value=loc.as_str()
                                                checked=escolhido
                                            />
                                            <span class="oc-lang__name">{loc.native_name()}</span>
                                        </label>
                                    }
                                })
                                .collect_view()}
                        </fieldset>
                        <div class="oc-mt-5">
                            <button class="oc-btn oc-btn--primary" type="submit">
                                {crate::i18n::t("settings.language.save")}
                            </button>
                        </div>
                    </form>
                },
            )}
        </div>
    }
}

/// `Definições → Aplicações`.
///
/// Gerir o **conjunto** de aplicações fixadas na barra lateral: uma caixa por
/// aplicação fixável que o membro pode abrir, marcada se está fixada. Guardar
/// substitui a lista pela escolha; «Repor» devolve o conjunto por omissão. A
/// **ordem** muda-se arrastando na própria barra — aqui decide-se o que entra,
/// não a ordem. Funciona sem JavaScript: é um formulário que submete.
///
/// Só aparecem aplicações que o membro pode abrir: fixar não é autorização, e
/// oferecer aqui um ecrã que o Core recusaria seria prometer o que não se cumpre.
pub fn apps(viewer: &crate::ui::shell::Viewer, saved: bool) -> impl IntoView {
    use crate::ui::apps as registo;

    let fixadas: std::collections::BTreeSet<&str> =
        viewer.pinned.iter().map(String::as_str).collect();
    let fixaveis: Vec<&'static registo::Application> =
        registo::visible_to(viewer, viewer.core_status)
            .into_iter()
            .filter(|app| app.can_pin)
            .collect();
    let vazio = fixaveis.is_empty();

    view! {
        <div class="oc-page oc-page--narrow">
            <div class="oc-head">
                <div class="oc-head__text">
                    <h1>{crate::i18n::t("settings.title")}</h1>
                    <p>{crate::i18n::t("settings.subtitle")}</p>
                </div>
            </div>

            <div class="oc-tabs oc-tabs--under oc-card__head--flush">
                {pill_tabs(
                    seccoes_das_definicoes("/settings/apps"),
                    crate::i18n::t("settings.tabs.aria"),
                )}
            </div>

            {saved.then(|| view! {
                <div class="oc-callout" role="status">
                    {crate::i18n::t("settings.apps.saved")}
                </div>
            })}

            {card(
                section_head(crate::i18n::t("settings.apps.title"), None, None),
                view! {
                    <p class="oc-t-caption--muted oc-mb-5">
                        {crate::i18n::t("settings.apps.help")}
                    </p>
                    {if vazio {
                        view! {
                            <p class="oc-muted oc-t-caption--muted">
                                {crate::i18n::t("settings.apps.empty")}
                            </p>
                        }
                        .into_any()
                    } else {
                        view! {
                            <form method="post" action="/settings/apps" class="oc-applist">
                                <fieldset class="oc-applist__set">
                                    <legend class="oc-sr">{crate::i18n::t("settings.apps.title")}</legend>
                                    {fixaveis
                                        .into_iter()
                                        .map(|app| {
                                            let marcada = fixadas.contains(app.id());
                                            view! {
                                                <label class="oc-applist__opt">
                                                    <input
                                                        type="checkbox"
                                                        name="pinned"
                                                        value=app.id()
                                                        checked=marcada
                                                    />
                                                    <span class="oc-applist__nome">{app.label()}</span>
                                                    <span class="oc-applist__desc">{app.description()}</span>
                                                </label>
                                            }
                                        })
                                        .collect_view()}
                                </fieldset>
                                <div class="oc-row oc-gap-5 oc-mt-5">
                                    <button class="oc-btn oc-btn--primary" type="submit">
                                        {crate::i18n::t("settings.apps.save")}
                                    </button>
                                    <button
                                        class="oc-btn oc-btn--secondary"
                                        type="submit"
                                        name="action"
                                        value="reset"
                                    >
                                        {crate::i18n::t("settings.apps.reset")}
                                    </button>
                                </div>
                                <p class="oc-t-caption--muted oc-mt-5">
                                    {crate::i18n::t("settings.apps.reset_note")}
                                </p>
                            </form>
                        }
                        .into_any()
                    }}
                },
            )}
        </div>
    }
}

/// `Definições → Segurança`.
///
/// `sessions` é `None` quando a lista não pôde ser lida. Não é o mesmo que uma
/// lista vazia, e o ecrã não pode dizer a mesma coisa das duas: «não há sessões
/// activas para além desta» é uma afirmação sobre a conta do membro, e fazê-la
/// porque o Core não respondeu é afirmar sobre a segurança de alguém aquilo que
/// não se sabe.
pub fn security(
    sessions: Option<&Value>,
    error: Option<String>,
    done: Option<String>,
) -> impl IntoView {
    let carregou = sessions.is_some();
    let linhas = sessions
        .and_then(|value| value.as_array().cloned())
        .unwrap_or_default();

    view! {
        <div class="oc-page oc-page--narrow">
            <div class="oc-head">
                <div class="oc-head__text">
                    <h1>{crate::i18n::t("settings.title")}</h1>
                    <p>{crate::i18n::t("settings.subtitle")}</p>
                </div>
            </div>

            <div class="oc-tabs oc-tabs--under oc-card__head--flush">
                {pill_tabs(
                    seccoes_das_definicoes("/settings/security"),
                    crate::i18n::t("settings.tabs.aria"),
                )}
            </div>

            {error.map(|m| view! { <div class="oc-card oc-alert" role="alert">{m}</div> })}
            {done.map(|m| view! { <div class="oc-callout" role="status">{m}</div> })}

            {card(
                section_head(crate::i18n::t("settings.password.section"), None, None),
                view! {
                    <form method="post" action="/settings/password">
                        // A palavra-passe actual é obrigatória: uma sessão
                        // aberta não é prova suficiente de quem está a escrever.
                        {text_field(
                            "pw-current",
                            crate::i18n::t("settings.password.current"),
                            "current",
                            crate::i18n::t("settings.password.current_hint"),
                            "password",
                        )}
                        {text_field(
                            "pw-new",
                            crate::i18n::t("settings.password.new"),
                            "password",
                            crate::i18n::t("settings.password.new_hint"),
                            "password",
                        )}
                        {text_field(
                            "pw-confirm",
                            crate::i18n::t("settings.password.confirm"),
                            "confirmation",
                            crate::i18n::t("settings.password.confirm_hint"),
                            "password",
                        )}
                        <p class="oc-field__hint">
                            {crate::i18n::t("settings.password.note")}
                        </p>
                        <div class="oc-row--end oc-gap-5 oc-mt-5">
                            {button(Button::new(crate::i18n::t("settings.password.change"), Variant::Primary))}
                        </div>
                    </form>
                },
            )}

            <div class="oc-mt-5"></div>

            {card(
                section_head(crate::i18n::t("settings.sessions.section"), None, None),
                view! {
                    {if !carregou {
                        view! {
                            <div class="oc-card oc-alert" role="alert">
                                {crate::i18n::t("settings.sessions.unreadable")}
                            </div>
                        }
                            .into_any()
                    } else if linhas.is_empty() {
                        view! {
                            <p class="oc-muted">
                                {crate::i18n::t("settings.sessions.none")}
                            </p>
                        }
                            .into_any()
                    } else {
                        view! {
                            <div>
                                {linhas
                                    .iter()
                                    .map(|s| {
                                        let id = text(s, "id");
                                        let actual = s
                                            .get("is_current")
                                            .and_then(Value::as_bool)
                                            .unwrap_or(false);
                                        // Indícios de sessão, não identidade de
                                        // dispositivo: o `User-Agent` é escrito
                                        // pelo cliente e o prefixo de rede não
                                        // decide autorização nenhuma.
                                        let origem = format!(
                                            "{} · {}",
                                            text(s, "user_agent"),
                                            text(s, "ip_prefix"),
                                        );
                                        view! {
                                            <div class="oc-list__row">
                                                <span class="oc-fill oc-truncate oc-t-cell">
                                                    {origem}
                                                </span>
                                                <span class="oc-mono oc-list__meta">
                                                    {text(s, "last_seen_at")
                                                        .chars()
                                                        .take(16)
                                                        .collect::<String>()}
                                                </span>
                                                {if actual {
                                                    view! {
                                                        <span class="oc-badge oc-badge--ok">
                                                            {crate::i18n::t("settings.sessions.current")}
                                                        </span>
                                                    }
                                                        .into_any()
                                                } else {
                                                    view! { <span></span> }.into_any()
                                                }}
                                                <form
                                                    method="post"
                                                    action=format!("/settings/sessions/{id}/revoke")
                                                >
                                                    {button(
                                                        Button::new(
                                                            if actual {
                                                                crate::i18n::t("settings.sessions.end_this")
                                                            } else {
                                                                crate::i18n::t("settings.sessions.end")
                                                            },
                                                            Variant::Secondary,
                                                        ),
                                                    )}
                                                </form>
                                            </div>
                                        }
                                    })
                                    .collect_view()}
                            </div>
                        }
                            .into_any()
                    }}
                    <p class="oc-muted oc-t-caption--muted oc-mt-5">
                        {crate::i18n::t("settings.sessions.note")}
                    </p>
                },
            )}
        </div>
    }
}

/// `Definições → Segurança → Códigos de recuperação`.
///
/// Regenerar invalida os anteriores, e por isso pede a palavra-passe **e** o
/// código do autenticador — a mesma sessão aberta não chega para uma acção
/// deste peso (ADR-0107). `codes` só vem preenchido depois de uma regeneração
/// bem-sucedida, e são mostrados uma única vez.
pub fn mfa_recovery(
    mfa_active: bool,
    codes: Option<&[String]>,
    error: Option<String>,
) -> impl IntoView {
    let corpo = if let Some(codigos) = codes {
        let linhas = codigos.join("\n");
        view! {
            <p class="oc-muted">
                {crate::i18n::t("settings.recovery.new_saved")}
            </p>
            <pre class="oc-mfa__codes oc-mono" data-oc="recovery-codes">{linhas}</pre>
            <div class="oc-row oc-gap-3 oc-mt-3">
                <button type="button" class="oc-btn oc-btn--sm" data-oc="recovery-copy">
                    {crate::i18n::t("settings.recovery.copy")}
                </button>
                <button type="button" class="oc-btn oc-btn--sm" data-oc="recovery-download">
                    {crate::i18n::t("settings.recovery.download")}
                </button>
            </div>
        }
        .into_any()
    } else if mfa_active {
        view! {
            <p class="oc-muted">
                {crate::i18n::t("settings.recovery.regen_intro")}
            </p>
            {error.map(|text| view! {
                <div class="oc-callout oc-callout--error oc-mt-3" role="alert">{text}</div>
            })}
            <form method="post" action="/settings/mfa/regenerate" class="oc-mt-3">
                <input
                    class="oc-input oc-mt-3"
                    type="password"
                    name="password"
                    autocomplete="current-password"
                    required
                    placeholder=crate::i18n::t("settings.password.current")
                />
                <input
                    class="oc-input oc-mt-3"
                    type="text"
                    name="code"
                    inputmode="numeric"
                    autocomplete="one-time-code"
                    required
                    placeholder=crate::i18n::t("settings.recovery.code_ph")
                />
                <button class="oc-btn oc-btn--danger oc-mt-3" type="submit">
                    {crate::i18n::t("settings.recovery.regen_button")}
                </button>
            </form>
        }
        .into_any()
    } else {
        view! {
            <p class="oc-muted">
                {crate::i18n::t("settings.recovery.no_mfa")}
            </p>
        }
        .into_any()
    };

    view! {
        <div class="oc-page oc-page--narrow">
            <div class="oc-head">
                <h1 class="oc-t-screen">{crate::i18n::t("settings.recovery.title")}</h1>
            </div>
            {card(section_head(crate::i18n::t("settings.recovery.section"), None, None), corpo)}
        </div>
    }
}

/// A superfície onde o membro escolhe como aparece.
///
/// # Três caminhos, e nenhum obrigatório
///
/// Não é preciso carregar uma fotografia para deixar de ser `FM`. Os avatares
/// Ocinye estão ali, e escolher um é um clique — não um upload, não um
/// ficheiro, não uma decisão sobre uma imagem pessoal que nem toda a gente quer
/// pôr num sistema institucional.
///
/// As iniciais continuam a ser uma escolha, e não apenas o que sobra: quem tem
/// um preset e prefere voltar ao nome carrega em «Usar iniciais», e é isso que
/// fica guardado. Remover uma fotografia não presume o que vem a seguir.
///
/// # Sem estado local
///
/// A grelha não pinta a escolha antes de o Core a confirmar. Cada opção é um
/// formulário que submete, e o que se vê depois é o que ficou guardado — não
/// uma antecipação que pode não se cumprir.
fn imagem_de_perfil(
    escolha: &AvatarChoice,
    nome: &str,
    error: Option<String>,
    done: bool,
) -> impl IntoView {
    use crate::ui::components::{avatar, AvatarSize};

    let iniciais = crate::ui::initials(nome);
    let tem_fotografia = matches!(escolha, AvatarChoice::Custom { .. });
    let actual = escolha.clone();

    let presets: Vec<_> = ocinye_contracts::AVATAR_PRESETS
        .iter()
        .map(|(preset, file)| {
            let escolhido = matches!(
                escolha,
                AvatarChoice::Preset { preset: atual } if atual == preset
            );
            let id = (*preset).to_owned();
            let ficheiro = (*file).to_owned();
            view! {
                <form method="post" action="/settings/avatar/preset" class="oc-avatars__cell">
                    <input type="hidden" name="preset" value=id.clone() />
                    <button
                        type="submit"
                        class="oc-avatars__pick"
                        class:oc-avatars__pick--on=escolhido
                        aria-pressed=if escolhido { "true" } else { "false" }
                        title=id.clone()
                        aria-label=crate::i18n::tf("settings.avatar.preset_alt", &[("name", &id)])
                    >
                        <img src=format!("/static/avatars/{ficheiro}") alt="" />
                    </button>
                </form>
            }
        })
        .collect();

    card(
        section_head(crate::i18n::t("settings.avatar.section"), None, None),
        view! {
            // `oc-alert`, e não `oc-notice`: a segunda é a classe dos ecrãs de
            // excepção — 404, recusa, falha — que vivem sozinhos numa página,
            // centrados, com 96px de margem em cima e em baixo. Aplicada a uma
            // linha dentro de um cartão, abria um vazio da altura de um ecrã
            // com a frase suspensa ao meio.
            {error
                .map(|razao| {
                    view! { <div class="oc-card oc-alert" role="alert">{razao}</div> }
                })}
            {done
                .then(|| {
                    view! {
                        <div class="oc-card oc-alert oc-alert--ok" role="status">
                            {crate::i18n::t("settings.avatar.updated")}
                        </div>
                    }
                })}

            <div class="oc-avatar-edit">
                {avatar(&actual, &iniciais, AvatarSize::Large)}
                <p class="oc-muted oc-t-caption--muted">
                    {crate::i18n::t("settings.avatar.initials_note")}
                </p>
            </div>

            <p class="oc-field__label oc-mt-8">{crate::i18n::t("settings.avatar.presets_label")}</p>
            <div class="oc-avatars">{presets}</div>

            <div class="oc-row--end oc-gap-5 oc-mt-8">
                <form method="post" action="/settings/avatar/initials">
                    {button(Button::new(crate::i18n::t("settings.avatar.use_initials"), Variant::Secondary))}
                </form>
            </div>

            <p class="oc-field__label oc-mt-8">{crate::i18n::t("settings.avatar.photo_label")}</p>
            <form
                method="post"
                action="/settings/avatar/photo"
                enctype="multipart/form-data"
                class="oc-avatar-upload"
            >
                <input
                    type="file"
                    name="file"
                    id="fotografia"
                    class="oc-input"
                    accept="image/jpeg,image/png,image/webp"
                    required
                />
                {button(
                    Button::new(
                        if tem_fotografia {
                            crate::i18n::t("settings.avatar.replace")
                        } else {
                            crate::i18n::t("settings.avatar.upload")
                        },
                        Variant::Primary,
                    ),
                )}
            </form>
            <p class="oc-muted oc-t-caption--muted oc-mt-5">
                {crate::i18n::t("settings.avatar.photo_note")}
            </p>
        },
    )
}

#[cfg(test)]
mod pureza_i18n {
    use super::*;
    use serde_json::json;

    /// Um ecrã, um idioma: as Definições em francês, sem chrome português.
    ///
    /// Cobre a Segurança (palavra-passe e sessões) e o Idioma — as superfícies
    /// que uma migração parcial deixara meio em português.
    #[tokio::test]
    async fn as_definicoes_nao_misturam_linguas() {
        use crate::i18n::{with_locale, Locale};
        let fr = with_locale(Locale::Fr, async {
            let seguranca = security(Some(&json!([])), None, None).to_html();
            let idioma = language(false).to_html();
            format!("{seguranca}{idioma}")
        })
        .await;
        for francesa in [
            "Mot de passe",
            "Changer le mot de passe",
            "Mes sessions",
            "Langue et région",
        ] {
            assert!(fr.contains(francesa), "fr: falta «{francesa}»");
        }
        for portuguesa in ["Palavra-passe", "As minhas sessões", "Mudar palavra-passe"] {
            assert!(
                !fr.contains(portuguesa),
                "fr: chrome português «{portuguesa}» sobreviveu"
            );
        }
    }
}

#[cfg(test)]
mod tests_apps {
    use super::*;
    use crate::ui::shell::{CoreStatus, ResolucaoSessao, Viewer};

    fn viewer(pinned: &[&str]) -> Viewer {
        Viewer {
            resolucao: ResolucaoSessao::Resolvida,
            zona: "UTC".to_owned().try_into().expect("fuso"),
            name: "Ana".to_owned(),
            sessao_privilegiada: false,
            administra: false,
            organisation: "Ocinye".to_owned(),
            email: Some("ana@ocinye.com".to_owned()),
            session_expires_in: None,
            avatar: AvatarChoice::Initials,
            core_status: CoreStatus::Ok,
            temporal: Vec::new(),
            temporal_failure: None,
            unread: 0,
            capabilities: ocinye_contracts::Permission::all()
                .into_iter()
                .map(|p| p.as_str().to_owned())
                .collect(),
            modules: [
                "units",
                "ideas",
                "projects",
                "knowledge",
                "files",
                "bibliography",
                "datasets",
            ]
            .into_iter()
            .map(ToOwned::to_owned)
            .collect(),
            pinned: pinned.iter().map(|s| (*s).to_owned()).collect(),
            inactive_apps: Vec::new(),
        }
    }

    /// A página lista as aplicações fixáveis, marca as fixadas, e oferece guardar
    /// e repor. As Notas estão fixadas; os Ficheiros não.
    #[test]
    fn a_pagina_de_aplicacoes_gere_o_conjunto_fixado() {
        let html = apps(&viewer(&["notes"]), false).to_html();
        // O separador novo existe.
        assert!(
            html.contains(r#"href="/settings/apps""#),
            "falta o separador Aplicações"
        );
        // Uma caixa por aplicação fixável, com o id no valor.
        assert!(
            html.contains(r#"name="pinned" value="notes""#),
            "falta a caixa das Notas"
        );
        assert!(
            html.contains(r#"name="pinned" value="files""#),
            "falta a caixa dos Ficheiros"
        );
        // A das Notas está marcada; a dos Ficheiros não.
        let notes = html.find(r#"value="notes""#).expect("notes");
        let tag_notes = &html[html[..notes].rfind("<input").expect("input")..notes + 40];
        assert!(
            tag_notes.contains("checked"),
            "as Notas deviam estar marcadas"
        );
        let files = html.find(r#"value="files""#).expect("files");
        let tag_files = &html[html[..files].rfind("<input").expect("input")..files + 40];
        assert!(
            !tag_files.contains("checked"),
            "os Ficheiros não deviam estar marcados"
        );
        // Guardar e repor.
        assert!(html.contains(r#"value="reset""#), "falta o botão de repor");
        assert!(
            html.contains(r#"action="/settings/apps""#),
            "falta o destino do formulário"
        );
        // Home não é fixável: não tem caixa.
        assert!(
            !html.contains(r#"name="pinned" value="home""#),
            "Home não é fixável"
        );
    }
}
