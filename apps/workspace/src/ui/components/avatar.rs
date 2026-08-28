//! Como uma pessoa aparece.
//!
//! # Uma só resolução
//!
//! A barra lateral, o menu de conta e as Definições mostram a mesma pessoa, e
//! têm de mostrar a mesma coisa. Se cada uma decidisse por si, bastaria uma
//! esquecer-se de um dos três estados para a mesma pessoa aparecer de duas
//! maneiras no mesmo ecrã.
//!
//! A ordem é sempre esta, e vive só aqui:
//!
//! ```text
//! Custom  → a fotografia do membro
//! Preset  → um avatar do produto
//! Initials → o nome
//! ```
//!
//! # As iniciais não são um erro
//!
//! São o estado de origem, e o refúgio de todos os outros. Não dependem de
//! storage, de rede, de escolha nem de sorte — e por isso, quando a fotografia
//! não carrega, é para elas que se volta, sem ícone de imagem partida e sem
//! nada que sugira que se perdeu alguma coisa. Perdeu-se uma imagem; a pessoa
//! continua ali.

use leptos::prelude::*;
use ocinye_contracts::AvatarChoice;

/// A dimensão a que o avatar aparece.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AvatarSize {
    /// Barra lateral e listas.
    Small,
    /// Cabeçalho do menu de conta.
    Medium,
    /// Definições.
    Large,
}

impl AvatarSize {
    const fn class(self) -> &'static str {
        match self {
            Self::Small => "oc-avatar oc-avatar--sm",
            Self::Medium => "oc-avatar oc-avatar--md",
            Self::Large => "oc-avatar oc-avatar--lg",
        }
    }
}

/// Renderiza a representação visual de uma pessoa.
///
/// `initials` é obrigatório mesmo quando há imagem: é o que fica quando a
/// imagem falha, e passá-lo sempre significa que esse caminho nunca precisa de
/// ir buscar o nome a lado nenhum.
pub fn avatar(choice: &AvatarChoice, initials: &str, size: AvatarSize) -> impl IntoView {
    let initials = initials.to_owned();
    let class = size.class();

    match choice {
        AvatarChoice::Initials => view! {
            <span class=class>{initials}</span>
        }
        .into_any(),

        // Um asset do produto, servido pelo Workspace. Não passa por storage
        // nem por autorização: é um ficheiro que acompanha o código, igual para
        // toda a gente, e escolher um não copia nada para lado nenhum.
        AvatarChoice::Preset { preset } => {
            // O ficheiro vem da tabela do catálogo, e não do identificador: o
            // logótipo é PNG e os motivos são SVG, e nenhum deles é nomeado por
            // quem envia o pedido.
            let Some(file) = AvatarChoice::preset_file(preset) else {
                return view! { <span class=class>{initials}</span> }.into_any();
            };
            let src = format!("/static/avatars/{file}");
            view! {
                <span class=class>
                    <img src=src alt="" />
                    <i>{initials}</i>
                </span>
            }
            .into_any()
        }

        // A fotografia do membro, endereçada pela versão do conteúdo. O
        // endereço só muda quando a fotografia muda, e por isso o browser pode
        // guardá-la sem nunca mostrar a anterior.
        AvatarChoice::Custom { version } => {
            let src = format!("/avatar/me/{version}");
            view! {
                <span class=class>
                    <img src=src alt="" />
                    <i>{initials}</i>
                </span>
            }
            .into_any()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn html(choice: &AvatarChoice) -> String {
        avatar(choice, "FM", AvatarSize::Small).to_html()
    }

    /// Cada estado produz a sua representação, e todos guardam as iniciais.
    ///
    /// A segunda metade é a que importa: mesmo com imagem, as iniciais viajam
    /// no documento. É delas que o CSS se serve quando o `<img>` falha, e é por
    /// isso que uma fotografia que não carrega não deixa um buraco.
    #[test]
    fn cada_estado_tem_a_sua_representacao_e_guarda_as_iniciais() {
        let iniciais = html(&AvatarChoice::Initials);
        assert!(iniciais.contains("FM"));
        assert!(
            !iniciais.contains("<img"),
            "as iniciais não carregam imagem"
        );

        let preset = html(&AvatarChoice::Preset {
            preset: "compute-01".to_owned(),
        });
        assert!(preset.contains(r#"src="/static/avatars/compute-01.svg""#));
        assert!(
            preset.contains("FM"),
            "o preset perdeu o refúgio das iniciais"
        );

        let custom = html(&AvatarChoice::Custom {
            version: "abc123".to_owned(),
        });
        assert!(custom.contains(r#"src="/avatar/me/abc123""#));
        assert!(
            custom.contains("FM"),
            "a fotografia perdeu o refúgio das iniciais"
        );
    }

    /// O avatar nunca expõe o armazenamento.
    ///
    /// Nem bucket, nem endpoint, nem chave de objecto, nem URL assinado. O que
    /// aparece no documento é um endereço do próprio Ocinye OS.
    #[test]
    fn o_avatar_nao_expoe_o_armazenamento() {
        let custom = html(&AvatarChoice::Custom {
            version: "abc123".to_owned(),
        });
        for vestígio in [
            "X-Amz",
            "amazonaws",
            "minio",
            "9000",
            "bucket",
            "ocinye-artifacts",
            "object_key",
        ] {
            assert!(
                !custom.contains(vestígio),
                "o avatar deixou escapar «{vestígio}» para o documento"
            );
        }
        assert!(
            !custom.contains("http://") && !custom.contains("https://"),
            "o avatar aponta para fora do Ocinye OS: {custom}"
        );
    }
}
