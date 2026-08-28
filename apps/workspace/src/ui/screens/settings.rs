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
//! # Porque só há duas secções
//!
//! `Conta` e `Segurança`. Não há `Aparência` nem `Preferências` porque hoje não
//! existe preferência do membro com persistência e consumidor reais: a
//! densidade das tabelas é estado do browser, e `mail_preferences` pertence ao
//! Correio, não à pessoa.
//!
//! Criar uma tabela de preferências para a página ter mais um separador seria
//! inventar infraestrutura para preencher espaço. Uma terceira secção nasce
//! quando existir a primeira preferência que a justifique.

use leptos::prelude::*;
use ocinye_contracts::AvatarChoice;
use serde_json::Value;

use crate::ui::components::{button, card, section_head, text_field, Button, Variant};
use crate::ui::components::{pill_tabs, Tab};

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
/// Em leitura. Nome, utilizador e correio institucional não têm hoje um fluxo
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
    let utilizador = text(me, "username");
    let correio = text(me, "email");
    let estado = text(me, "status");
    let instituicao = text(organisation, "name");

    view! {
        <div class="oc-page oc-page--narrow">
            <div class="oc-head">
                <div class="oc-head__text">
                    <h1>"Definições"</h1>
                    <p>"A sua conta e as suas credenciais no Ocinye OS."</p>
                </div>
            </div>

            <div class="oc-tabs oc-tabs--under oc-card__head--flush">
                {pill_tabs(
                    vec![
                        Tab::link("Conta", "/settings", true),
                        Tab::link("Segurança", "/settings/security", false),
                    ],
                    "Secções das definições",
                )}
            </div>

            {imagem_de_perfil(escolha, &nome, error, done)}

            {card(
                section_head("A SUA CONTA", None, None),
                view! {
                    {facto("NOME", nome)}
                    {facto("UTILIZADOR", utilizador)}
                    {facto("CORREIO INSTITUCIONAL", correio)}
                    {facto("ESTADO", estado)}
                    {facto("INSTITUIÇÃO", instituicao)}
                    <p class="oc-muted oc-t-caption--muted oc-mt-5">
                        "Estes dados são geridos pela Administração da Ocinye. Papéis, filiações
                         e acessos não se alteram aqui — são concedidos por quem tem autoridade
                         para isso, e ficam registados."
                    </p>
                },
            )}
        </div>
    }
}

/// `Definições → Segurança`.
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
                    <h1>"Definições"</h1>
                    <p>"A sua conta e as suas credenciais no Ocinye OS."</p>
                </div>
            </div>

            <div class="oc-tabs oc-tabs--under oc-card__head--flush">
                {pill_tabs(
                    vec![
                        Tab::link("Conta", "/settings", false),
                        Tab::link("Segurança", "/settings/security", true),
                    ],
                    "Secções das definições",
                )}
            </div>

            {error.map(|m| view! { <div class="oc-card oc-alert" role="alert">{m}</div> })}
            {done.map(|m| view! { <div class="oc-callout" role="status">{m}</div> })}

            {card(
                section_head("PALAVRA-PASSE", None, None),
                view! {
                    <form method="post" action="/settings/password">
                        // A palavra-passe actual é obrigatória: uma sessão
                        // aberta não é prova suficiente de quem está a escrever.
                        {text_field(
                            "pw-current",
                            "Palavra-passe actual",
                            "current",
                            "A que usa hoje",
                            "password",
                        )}
                        {text_field(
                            "pw-new",
                            "Nova palavra-passe",
                            "password",
                            "Mínimo de 15 caracteres",
                            "password",
                        )}
                        {text_field(
                            "pw-confirm",
                            "Confirmar",
                            "confirmation",
                            "Repita a nova palavra-passe",
                            "password",
                        )}
                        <p class="oc-field__hint">
                            "Ao mudar a palavra-passe, todas as suas sessões terminam e esta é
                             substituída por uma nova. Continua a trabalhar sem voltar a entrar."
                        </p>
                        <div class="oc-row--end oc-gap-5 oc-mt-5">
                            {button(Button::new("Mudar palavra-passe", Variant::Primary))}
                        </div>
                    </form>
                },
            )}

            <div class="oc-mt-5"></div>

            {card(
                section_head("AS MINHAS SESSÕES", None, None),
                view! {
                    {if !carregou {
                        view! {
                            <div class="oc-card oc-alert" role="alert">
                                "A lista de sessões não pôde ser lida. Isto não quer dizer
                                 que não existam outras sessões — quer dizer que não
                                 sabemos quais são."
                            </div>
                        }
                            .into_any()
                    } else if linhas.is_empty() {
                        view! {
                            <p class="oc-muted">
                                "Não há sessões activas para além desta."
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
                                                            "SESSÃO ACTUAL"
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
                                                            if actual { "Terminar esta" } else { "Terminar" },
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
                        "Terminar a sessão actual encerra este acesso e volta ao início de sessão."
                    </p>
                },
            )}
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
                        aria-label=format!("Avatar Ocinye {id}")
                    >
                        <img src=format!("/static/avatars/{ficheiro}") alt="" />
                    </button>
                </form>
            }
        })
        .collect();

    card(
        section_head("IMAGEM DE PERFIL", None, None),
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
                            "Imagem de perfil actualizada."
                        </div>
                    }
                })}

            <div class="oc-avatar-edit">
                {avatar(&actual, &iniciais, AvatarSize::Large)}
                <p class="oc-muted oc-t-caption--muted">
                    "As iniciais são sempre o recurso: se a imagem não carregar, é
                     o seu nome que aparece."
                </p>
            </div>

            <p class="oc-field__label oc-mt-8">"AVATARES OCINYE"</p>
            <div class="oc-avatars">{presets}</div>

            <div class="oc-row--end oc-gap-5 oc-mt-8">
                <form method="post" action="/settings/avatar/initials">
                    {button(Button::new("Usar iniciais", Variant::Secondary))}
                </form>
            </div>

            <p class="oc-field__label oc-mt-8">"FOTOGRAFIA"</p>
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
                        if tem_fotografia { "Substituir fotografia" } else { "Carregar fotografia" },
                        Variant::Primary,
                    ),
                )}
            </form>
            <p class="oc-muted oc-t-caption--muted oc-mt-5">
                "JPEG, PNG ou WebP, até 8 MiB. A fotografia é recortada num quadrado
                 ao centro e guardada pela Ocinye — não é enviada para nenhum serviço
                 externo, e a informação de câmara e localização que a acompanhe não é
                 conservada."
            </p>
        },
    )
}
