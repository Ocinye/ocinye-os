//! Campos de formulário.
//!
//! Todos os campos têm rótulo associado — sem excepção. Um campo cujo único
//! rótulo é o placeholder deixa de ter rótulo assim que se começa a escrever.

use leptos::prelude::*;

/// Um campo de texto com rótulo.
pub fn field(
    id: &'static str,
    label: &'static str,
    name: &'static str,
    placeholder: &'static str,
    input_type: &'static str,
) -> impl IntoView {
    view! {
        <div class="oc-field">
            <label class="oc-field__label" for=id>{label}</label>
            <input
                class="oc-input"
                id=id
                name=name
                type=input_type
                placeholder=placeholder
            />
        </div>
    }
}

/// Um campo de texto com rótulo e valor inicial.
///
/// Existe para os formulários que são re-renderizados com o que já lá estava:
/// um composer que volta com uma sugestão, ou com um pedido de confirmação,
/// não pode devolver os campos em branco. Sem isto, quem escreveu uma mensagem
/// longa perdia-a ao carregar num botão (briefing §51).
pub fn field_with_value(
    id: &'static str,
    label: &'static str,
    name: &'static str,
    placeholder: &'static str,
    input_type: &'static str,
    value: impl Into<String>,
) -> impl IntoView {
    let value = value.into();
    view! {
        <div class="oc-field">
            <label class="oc-field__label" for=id>{label}</label>
            <input
                class="oc-input"
                id=id
                name=name
                type=input_type
                placeholder=placeholder
                value=value
            />
        </div>
    }
}

/// A classe de altura de uma área de texto.
///
/// A altura era um atributo `style`, que a CSP do Workspace descarta. São três
/// tamanhos, e são os três que a aplicação usa; qualquer outro valor cai no do
/// meio em vez de ficar sem altura nenhuma.
fn height_class(height: u16) -> &'static str {
    match height {
        0..=70 => "oc-textarea--sm",
        71..=100 => "oc-textarea--md",
        _ => "oc-textarea--lg",
    }
}

/// Uma área de texto com rótulo.
pub fn textarea(
    id: &'static str,
    label: &'static str,
    name: &'static str,
    placeholder: &'static str,
    height: u16,
) -> impl IntoView {
    view! {
        <div class="oc-field">
            <label class="oc-field__label" for=id>{label}</label>
            <textarea
                class=format!("oc-textarea {}", height_class(height))
                id=id
                name=name
                placeholder=placeholder
            ></textarea>
        </div>
    }
}

/// Uma área de texto com rótulo e conteúdo inicial.
///
/// O conteúdo vai no corpo do elemento, não num atributo `value`: um
/// `<textarea value="...">` é ignorado pelo browser, e o campo apareceria
/// vazio.
pub fn textarea_with_value(
    id: &'static str,
    label: &'static str,
    name: &'static str,
    placeholder: &'static str,
    height: u16,
    value: impl Into<String>,
) -> impl IntoView {
    let value = value.into();
    view! {
        <div class="oc-field">
            <label class="oc-field__label" for=id>{label}</label>
            <textarea
                class=format!("oc-textarea {}", height_class(height))
                id=id
                name=name
                placeholder=placeholder
            >{value}</textarea>
        </div>
    }
}

/// Um selector com rótulo.
///
/// Cada opção traz um sinalizador de disponibilidade: uma opção indisponível é
/// renderizada desactivada, em vez de aceitar uma escolha que falharia depois.
pub fn select(
    id: &'static str,
    label: &'static str,
    name: &'static str,
    options: Vec<(String, bool)>,
) -> impl IntoView {
    // Se nenhuma opção está disponível, o desactivado pertence ao `<select>`,
    // não a cada `<option>`: um browser não mostra uma opção desactivada, e o
    // campo ficaria em branco em vez de dizer porque está vazio.
    let none_available = options.iter().all(|(_, available)| !*available);

    view! {
        <div class="oc-field">
            <label class="oc-field__label" for=id>{label}</label>
            <select class="oc-select" id=id name=name disabled=none_available>
                {options
                    .into_iter()
                    .map(|(text, available)| {
                        // Uma opção indisponível nunca é um valor submissível.
                        let value = if available { text.clone() } else { String::new() };
                        view! {
                            <option value=value disabled=!available && !none_available>
                                {text}
                            </option>
                        }
                    })
                    .collect_view()}
            </select>
        </div>
    }
}

/// Uma opção de [`select_labelled`].
pub struct SelectOption {
    /// O que é submetido.
    pub value: String,
    /// O que é lido.
    pub label: String,
    /// Se pode ser escolhida.
    pub available: bool,
    /// Se está escolhida.
    pub selected: bool,
}

impl SelectOption {
    /// Uma opção disponível e não escolhida.
    #[must_use]
    pub fn new(value: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            label: label.into(),
            available: true,
            selected: false,
        }
    }

    /// Marca-a como escolhida.
    #[must_use]
    pub const fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }
}

/// Um selector cujo valor submetido difere do rótulo lido.
///
/// [`select`] usa o mesmo texto para ambos, o que serve onde o valor *é* o
/// rótulo — um endereço de correio, por exemplo. Não serve onde o Core espera
/// um código estável (`more_formal`) e o membro deve ler português
/// (`Mais formal`): usar o rótulo como valor faz o Core recusar, e usar o
/// código como rótulo mostra jargão interno na interface.
pub fn select_labelled(
    id: &'static str,
    label: &'static str,
    name: &'static str,
    options: Vec<SelectOption>,
) -> impl IntoView {
    let none_available = options.iter().all(|option| !option.available);

    view! {
        <div class="oc-field">
            <label class="oc-field__label" for=id>{label}</label>
            <select class="oc-select" id=id name=name disabled=none_available>
                {options
                    .into_iter()
                    .map(|option| {
                        let SelectOption { value, label, available, selected } = option;
                        view! {
                            <option
                                value=value
                                disabled=!available && !none_available
                                selected=selected
                            >
                                {label}
                            </option>
                        }
                    })
                    .collect_view()}
            </select>
        </div>
    }
}

/// Uma caixa de selecção cujo `name` difere do `id`.
///
/// Necessário quando o nome do campo pertence ao contrato do Core e o `id`
/// pertence ao ecrã. Usar o `id` como `name` — como acontecia — faz o campo
/// chegar ao Core com um nome que ele não conhece, e a opção é silenciosamente
/// perdida.
pub fn named_checkbox(
    id: &'static str,
    name: &'static str,
    label: &'static str,
    checked: bool,
) -> impl IntoView {
    view! {
        <label class="oc-check" for=id>
            <input type="checkbox" id=id name=name checked=checked />
            <span>{label}</span>
        </label>
    }
}

/// Um grupo de opções mutuamente exclusivas.
///
/// Radios e não botões: um grupo de `<button type="button">` sem `name` tem
/// aparência de escolha e não submete nada — era o que existia aqui antes desta
/// auditoria. Radios funcionam sem JavaScript, com teclado e com leitor de ecrã.
///
/// Cada opção traz o motivo pelo qual está indisponível, quando está: um
/// controlo desactivado sem explicação é tão opaco como um que não faz nada
/// (briefing §53).
pub fn radio_group(
    name: &'static str,
    label: &'static str,
    options: Vec<RadioOption>,
) -> impl IntoView {
    view! {
        <fieldset class="oc-seg-group">
            <legend class="oc-field__label">{label}</legend>
            <div class="oc-seg" role="radiogroup" aria-label=label>
                {options
                    .into_iter()
                    .map(|option| {
                        let id = format!("{name}-{}", option.value);
                        let label_for = id.clone();
                        let disabled = option.unavailable_reason.is_some();
                        view! {
                            <label
                                class="oc-seg__option"
                                for=label_for
                                title=option.unavailable_reason.clone().unwrap_or_default()
                            >
                                <input
                                    type="radio"
                                    id=id
                                    name=name
                                    value=option.value
                                    checked=option.selected
                                    disabled=disabled
                                />
                                <span>{option.label}</span>
                            </label>
                        }
                    })
                    .collect_view()}
            </div>
        </fieldset>
    }
}

/// Uma opção de [`radio_group`].
pub struct RadioOption {
    /// Valor enviado ao Core. Vocabulário do contrato, não texto de ecrã.
    pub value: &'static str,
    /// Texto mostrado.
    pub label: &'static str,
    /// Se está seleccionada por omissão.
    pub selected: bool,
    /// Porque está indisponível, quando está.
    pub unavailable_reason: Option<String>,
}

impl RadioOption {
    /// Uma opção disponível.
    #[must_use]
    pub const fn new(value: &'static str, label: &'static str, selected: bool) -> Self {
        Self {
            value,
            label,
            selected,
            unavailable_reason: None,
        }
    }
}
