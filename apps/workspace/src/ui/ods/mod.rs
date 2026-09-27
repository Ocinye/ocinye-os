//! As primitivas da UI nova (Claude Design, D1).
//!
//! Traduzem `docs/ui/D1_PRIMITIVES.md` para Leptos: a estrutura HTML, as
//! classes `ods-` e os estados por atributo. Nenhuma define cor, medida ou
//! comportamento: a apresentação é de `static/ods-d1-primitives.css`, e o
//! comportamento (abrir, fechar, teclado) do `static/app.js`, ligado por
//! `data-oc`.
//!
//! Nada aqui decide autorização: uma primitiva mostra o que lhe dão.

use leptos::prelude::*;

/// Um ícone do sprite do Claude Design (`static/ods-icons.svg`).
///
/// `id` é o nome sem o prefixo: `icone("bell", "")` desenha `#ods-bell`. O
/// sprite tem os símbolos que o desenho pediu, e só esses: um ícone que não
/// esteja lá não se inventa aqui.
pub fn icone(id: &'static str, extra: &'static str) -> impl IntoView {
    let classe = if extra.is_empty() {
        "ods-icon".to_owned()
    } else {
        format!("ods-icon {extra}")
    };
    view! {
        <svg class=classe aria-hidden="true" focusable="false">
            <use href=format!("/static/ods-icons.svg#ods-{id}")></use>
        </svg>
    }
}

/// O tom de um distintivo ou de um ponto de estado.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tom {
    /// Sem tom: neutro.
    Neutro,
    /// Operacional, concluído.
    Sucesso,
    /// Atenção.
    Aviso,
    /// Erro, recusa.
    Erro,
    /// Informação.
    Info,
}

impl Tom {
    const fn sufixo(self) -> &'static str {
        match self {
            Self::Neutro => "",
            Self::Sucesso => "success",
            Self::Aviso => "warning",
            Self::Erro => "error",
            Self::Info => "info",
        }
    }
}

/// Um distintivo de estado. O texto diz sempre o estado: a cor nunca é o único
/// sinal.
pub fn distintivo(texto: String, tom: Tom) -> impl IntoView {
    let classe = match tom.sufixo() {
        "" => "ods-badge".to_owned(),
        s => format!("ods-badge ods-badge--{s}"),
    };
    view! { <span class=classe>{texto}</span> }
}

/// Um ponto de estado. Decorativo: vai sempre ao lado de um texto.
pub fn ponto(tom: Tom) -> impl IntoView {
    let classe = match tom.sufixo() {
        "" => "ods-dot".to_owned(),
        s => format!("ods-dot ods-dot--{s}"),
    };
    view! { <span class=classe aria-hidden="true"></span> }
}

/// Os estados honestos (D1): vazio, a carregar, indisponível, recusado e erro
/// são coisas diferentes, e cada um diz o que é. Uma falha do Core nunca
/// aparece como zero nem como lista vazia.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Estado {
    /// A pessoa não tem acesso.
    Recusado,
    /// A capacidade não existe nesta Instância, ou espera por um contrato.
    Indisponivel,
    /// O Core falhou.
    Erro,
}

/// Um estado honesto, com a sua mensagem.
pub fn estado(estado: Estado, mensagem: String) -> impl IntoView {
    let (modificador, icone_id) = match estado {
        Estado::Recusado => ("denied", "lock"),
        Estado::Indisponivel => ("unavailable", "status"),
        Estado::Erro => ("error", "status"),
    };
    view! {
        <div class=format!("ods-state ods-state--{modificador}") role="status">
            {icone(icone_id, "")}
            <span>{mensagem}</span>
        </div>
    }
}

/// Uma recusa do Core a um pedido que a pessoa acabou de fazer.
///
/// O mesmo estado `denied`, mas anunciado de imediato (`role="alert"`): quem
/// submeteu um formulário tem de saber já que ele não passou, e porquê.
pub fn recusa(mensagem: String) -> impl IntoView {
    view! {
        <div class="ods-state ods-state--denied" role="alert">
            {icone("lock", "")}
            <span>{mensagem}</span>
        </div>
    }
}

/// Um estado indisponível à espera de um contrato que ainda não existe
/// (`docs/ui/CLAUDE_DESIGN_FUNCTIONAL_GAPS.md`).
pub fn a_espera_de_contrato() -> impl IntoView {
    estado(
        Estado::Indisponivel,
        crate::i18n::t("ods.state.pending_contract").to_owned(),
    )
}

/// Um estado vazio: não há nada, e isso diz-se.
pub fn vazio(icone_id: &'static str, titulo: String, corpo: Option<String>) -> impl IntoView {
    view! {
        <div class="ods-empty">
            <span class="ods-empty__icon">{icone(icone_id, "ods-icon--lg")}</span>
            <p class="ods-empty__title">{titulo}</p>
            {corpo.map(|c| view! { <p class="ods-empty__body">{c}</p> })}
        </div>
    }
}

/// Um aviso em linha.
pub fn aviso(tom: Tom, texto: String) -> impl IntoView {
    let classe = match tom {
        Tom::Aviso => "ods-notice ods-notice--warning",
        Tom::Erro => "ods-notice ods-notice--error",
        _ => "ods-notice",
    };
    view! { <div class=classe role="status">{texto}</div> }
}

/// Uma barra de progresso. A largura vem de `data-ods-value`, posta pelo
/// `app.js` por CSSOM: um `style=""` seria descartado pela CSP.
pub fn progresso(percentagem: u8, rotulo: String) -> impl IntoView {
    let valor = percentagem.min(100);
    view! {
        <div
            class="ods-progress"
            role="progressbar"
            aria-valuemin="0"
            aria-valuemax="100"
            aria-valuenow=valor.to_string()
            aria-label=rotulo
            data-ods-value=valor.to_string()
        >
            <div class="ods-progress__bar"></div>
        </div>
    }
}

/// O tamanho de um avatar.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TamanhoAvatar {
    /// 26px.
    Pequeno,
    /// 38px.
    Medio,
    /// 72px.
    Grande,
}

/// Um avatar: a imagem do membro, se houver, senão as iniciais.
pub fn avatar(
    iniciais: String,
    imagem: Option<String>,
    nome: String,
    tamanho: TamanhoAvatar,
) -> impl IntoView {
    let classe = match tamanho {
        TamanhoAvatar::Pequeno => "ods-avatar ods-avatar--sm",
        TamanhoAvatar::Medio => "ods-avatar",
        TamanhoAvatar::Grande => "ods-avatar ods-avatar--lg",
    };
    match imagem {
        Some(src) => view! { <span class=classe><img src=src alt=nome /></span> }.into_any(),
        None => view! { <span class=classe aria-label=nome>{iniciais}</span> }.into_any(),
    }
}

/// O avatar de um membro, a partir da escolha que o Core guardou: a fotografia
/// (endereçada pela versão), um avatar do catálogo, ou as iniciais.
pub fn avatar_do_membro(
    escolha: &ocinye_contracts::AvatarChoice,
    iniciais: &str,
    nome: &str,
    tamanho: TamanhoAvatar,
) -> impl IntoView {
    use ocinye_contracts::AvatarChoice;
    let imagem = match escolha {
        AvatarChoice::Initials => None,
        AvatarChoice::Preset { preset } => {
            AvatarChoice::preset_file(preset).map(|file| format!("/static/avatars/{file}"))
        }
        AvatarChoice::Custom { version } => Some(format!("/avatar/me/{version}")),
    };
    avatar(iniciais.to_owned(), imagem, nome.to_owned(), tamanho)
}

/// O ícone do sprite de cada aplicação.
///
/// Um símbolo do sprite por aplicação (D1, completado no D12 com mensagens,
/// conhecimento, bibliografia, IA, computação e administração).
pub fn icone_da_aplicacao(ecra: crate::ui::shell::Screen) -> &'static str {
    use crate::ui::shell::Screen;
    match ecra {
        Screen::Home => "home",
        Screen::MyWork => "work",
        Screen::Notes => "notes",
        Screen::Calendar => "calendar",
        Screen::Mail => "mail",
        Screen::Resources => "storage",
        Screen::Units => "units",
        Screen::Ideas => "idea",
        Screen::Projects => "project",
        Screen::Files => "files",
        Screen::Datasets => "data",
        Screen::Agents => "agent",
        Screen::Activity => "activity",
        Screen::Audit => "audit",
        Screen::Prompt | Screen::Ask => "nye",
        Screen::Search => "search",
        Screen::Settings => "settings",
        Screen::Help => "help",
        Screen::Terminal => "terminal",
        Screen::Messaging => "messages",
        Screen::Knowledge => "knowledge",
        Screen::Bibliography => "bibliography",
        Screen::Ai => "ai",
        Screen::Compute => "compute",
        Screen::Admin => "admin",
    }
}

/// O símbolo do sprite novo para cada ícone do enum legado (`ui::icon::Icon`),
/// para os componentes partilhados desenharem com o sprite do Claude Design.
///
/// Onze não têm correspondente directo no sprite (Restart, SidebarCollapse,
/// ChevronUp, Filter, Science, Attach, Tools, Send, Reply, Archive, EmptyState):
/// levam o mais próximo, e estão em `docs/ui/CLAUDE_DESIGN_QUESTIONS.md` (Q-18).
pub fn icone_do_legado(icone: crate::ui::icon::Icon) -> &'static str {
    use crate::ui::icon::Icon;
    match icone {
        Icon::User => "user",
        Icon::Lock => "lock",
        Icon::ArrowRight | Icon::Send | Icon::Reply => "arrow-r",
        Icon::Power => "logout",
        Icon::Restart => "auto",
        Icon::SystemStatus => "status",
        Icon::SidebarCollapse | Icon::EmptyState => "grid",
        Icon::ChevronUp => "chev-d",
        Icon::Search => "search",
        Icon::Plus => "plus",
        Icon::Close => "close",
        Icon::Bell => "bell",
        Icon::Calendar => "calendar",
        Icon::Filter => "list",
        Icon::Settings | Icon::Tools => "settings",
        Icon::Help => "help",
        Icon::Home => "home",
        Icon::MyWork => "work",
        Icon::Apps => "apps-brand",
        Icon::Units => "units",
        Icon::Idea => "idea",
        Icon::Project => "project",
        Icon::Knowledge => "knowledge",
        Icon::Science | Icon::Data | Icon::Dataset => "data",
        Icon::Bibliography => "bibliography",
        Icon::Ai => "ai",
        Icon::Agent => "agent",
        Icon::Compute | Icon::ComputeLg => "compute",
        Icon::Activity => "activity",
        Icon::Admin => "admin",
        Icon::Audit => "audit",
        Icon::AiHexLg | Icon::AiHexMd => "nye",
        Icon::Shield => "shield",
        Icon::Attach | Icon::Files | Icon::Folder => "files",
        Icon::Document => "notes",
        Icon::Mail => "mail",
        Icon::Messaging => "messages",
        Icon::Star => "star-fill",
        Icon::Archive => "storage",
        Icon::Trash => "trash",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn o_icone_aponta_para_o_sprite_novo() {
        let html = icone("bell", "").to_html();
        assert!(html.contains("/static/ods-icons.svg#ods-bell"));
        assert!(html.contains(r#"aria-hidden="true""#));
    }

    #[test]
    fn a_barra_de_progresso_nao_escreve_estilo_e_limita_a_cem() {
        let html = progresso(180, "Quota".to_owned()).to_html();
        // O nome do atributo montado aos pedaços: o guarda da CSP lê a fonte e
        // acusaria a própria verificação.
        let atributo = ["sty", "le="].concat();
        assert!(!html.contains(&atributo));
        assert!(html.contains(r#"data-ods-value="100""#));
        assert!(html.contains(r#"aria-valuenow="100""#));
    }

    #[test]
    fn cada_estado_honesto_tem_a_sua_classe() {
        for (e, classe) in [
            (Estado::Recusado, "ods-state--denied"),
            (Estado::Indisponivel, "ods-state--unavailable"),
            (Estado::Erro, "ods-state--error"),
        ] {
            let html = estado(e, "x".to_owned()).to_html();
            assert!(html.contains(classe), "{classe}");
            assert!(html.contains(r#"role="status""#));
        }
    }

    #[test]
    fn o_distintivo_leva_sempre_texto() {
        let html = distintivo("OPERACIONAL".to_owned(), Tom::Sucesso).to_html();
        assert!(html.contains("ods-badge--success"));
        assert!(html.contains("OPERACIONAL"));
    }
}
