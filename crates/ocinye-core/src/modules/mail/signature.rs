//! A assinatura institucional do correio, e as projecções do corpo enviado.
//!
//! # A fronteira que isto é
//!
//! Ao contrário de [`super::sanitize`], que limpa HTML **de entrada** escrito por
//! quem enviou a mensagem, aqui o HTML é **de saída** e é a Ocinye que o produz.
//! Não há HTML do membro a atravessar: o membro escreve texto simples, esse
//! texto é **escapado** e projectado em HTML de forma determinística, e a
//! assinatura institucional é acrescentada a partir de dados estruturados. Nada
//! do que o membro escreve chega ao email como marcação (ADR-0414).
//!
//! # O que a assinatura mostra
//!
//! Só factos: o nome do membro, o cargo institucional **quando existe**, a
//! instituição, e o endereço de envio. Não inventa telefone, cargo nem um
//! website que ainda não existe — um campo em falta desaparece, não deixa uma
//! linha vazia. É sóbria de propósito: sem faixa, sem rodapé de marketing, sem
//! ícones sociais.

/// Os factos do membro que a assinatura usa. Só o que é verdadeiro.
#[derive(Debug, Clone)]
pub struct SignatureFacts {
    /// Nome do membro.
    pub full_name: String,
    /// Cargo institucional, quando existe e é factual. `None` omite a linha.
    pub role: Option<String>,
    /// O endereço de envio — o que o destinatário vê e a que responde.
    pub email: String,
    /// A linha pessoal opcional do membro (`mail_preferences.signature`),
    /// acrescentada acima da assinatura oficial. Texto simples; escapado no HTML.
    pub personal_line: Option<String>,
}

/// Como o logótipo é referido na projecção HTML.
#[derive(Debug, Clone, Copy)]
pub enum LogoRef<'a> {
    /// `cid:<id>` — para o email real, onde o logótipo viaja embutido.
    Cid(&'a str),
    /// Um URL — para a pré-visualização no Workspace, onde `cid:` não resolve.
    Url(&'a str),
}

/// O identificador da parte embutida do logótipo (o `Content-ID` do email).
pub const LOGO_CONTENT_ID: &str = "ocinye-logo";

/// O tipo de conteúdo do logótipo embutido.
pub const LOGO_CONTENT_TYPE: &str = "image/png";

/// Os bytes do logótipo institucional, na versão optimizada para email
/// (~180 px, PNG). Embutido no binário para o envio não depender do sistema de
/// ficheiros nem da rede — a assinatura não pinga um servidor ao ser aberta.
pub const LOGO_PNG: &[u8] = include_bytes!("../../../assets/ocinye-logo-email.png");

/// As bandas visuais da Ocinye, em literais — o email exige estilo em linha, e
/// não resolve tokens CSS.
const NAVY: &str = "#0B2D4A";
const NAVY_MID: &str = "#1C4B74";
const TEXT_SECONDARY: &str = "#42546A";
const TEXT_MUTED: &str = "#5F7183";
const BORDER: &str = "#E4E9F0";
const FONT: &str = "Arial, Helvetica, sans-serif";

/// As duas projecções de um corpo de saída.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Projections {
    /// `text/plain` — canónico e sempre completo.
    pub text: String,
    /// `text/html` — a projecção determinística, quando há assinatura a mostrar.
    pub html: Option<String>,
}

/// Escapa texto para HTML. O mesmo que a saída faz em toda a parte: nada do que
/// o membro escreve pode virar marcação.
fn escape(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Projecta texto simples em HTML, preservando parágrafos e quebras de linha.
///
/// Linhas em branco separam parágrafos; uma quebra de linha simples vira `<br>`.
/// Determinístico e sem surpresas: a mesma entrada dá sempre a mesma saída.
#[must_use]
pub fn authored_text_to_html(text: &str) -> String {
    text.split("\n\n")
        .map(|paragrafo| paragrafo.trim_matches('\n'))
        .filter(|paragrafo| !paragrafo.is_empty())
        .map(|paragrafo| {
            let corpo = escape(paragrafo).replace('\n', "<br>");
            format!("<p style=\"margin:0 0 1em;color:{TEXT_SECONDARY};\">{corpo}</p>")
        })
        .collect()
}

/// A assinatura institucional em texto simples.
///
/// ```text
/// Nome do membro
/// <cargo, se existir>
/// Ocinye
/// email@ocinye.com
/// ```
#[must_use]
pub fn signature_text(facts: &SignatureFacts) -> String {
    let mut linhas: Vec<String> = Vec::new();
    if let Some(linha) = facts.personal_line.as_deref().map(str::trim) {
        if !linha.is_empty() {
            linhas.push(linha.to_owned());
            linhas.push(String::new());
        }
    }
    linhas.push(facts.full_name.trim().to_owned());
    if let Some(role) = facts.role.as_deref().map(str::trim) {
        if !role.is_empty() {
            linhas.push(role.to_owned());
        }
    }
    linhas.push("Ocinye".to_owned());
    linhas.push(facts.email.trim().to_owned());
    linhas.join("\n")
}

/// A assinatura institucional em HTML — sóbria, com estilo em linha e uma tabela
/// para aguentar os clientes de email. O logótipo é uma imagem pequena; o resto
/// é o nome, o cargo (se existir), a instituição e o endereço.
#[must_use]
pub fn signature_html(facts: &SignatureFacts, logo: LogoRef) -> String {
    let logo_src = match logo {
        LogoRef::Cid(id) => format!("cid:{id}"),
        LogoRef::Url(url) => url.to_owned(),
    };
    let nome = escape(facts.full_name.trim());
    let email = escape(facts.email.trim());

    let linha_pessoal = facts
        .personal_line
        .as_deref()
        .map(str::trim)
        .filter(|linha| !linha.is_empty())
        .map(|linha| {
            let corpo = escape(linha).replace('\n', "<br>");
            format!(
                "<div style=\"margin:0 0 12px;color:{TEXT_SECONDARY};\
                 font-family:{FONT};font-size:13px;line-height:1.5;\">{corpo}</div>"
            )
        })
        .unwrap_or_default();

    let linha_cargo = facts
        .role
        .as_deref()
        .map(str::trim)
        .filter(|role| !role.is_empty())
        .map(|role| format!("<div style=\"color:{TEXT_MUTED};\">{}</div>", escape(role)))
        .unwrap_or_default();

    format!(
        "<div style=\"font-family:{FONT};\">\
           {linha_pessoal}\
           <table role=\"presentation\" cellpadding=\"0\" cellspacing=\"0\" border=\"0\" \
             style=\"border-collapse:collapse;border-top:1px solid {BORDER};padding-top:12px;\">\
             <tr>\
               <td style=\"vertical-align:top;padding:12px 14px 0 0;\">\
                 <img src=\"{logo_src}\" width=\"44\" height=\"44\" alt=\"Ocinye\" \
                   style=\"display:block;border:0;width:44px;height:44px;\">\
               </td>\
               <td style=\"vertical-align:top;padding-top:12px;border-left:2px solid {NAVY_MID};\
                 padding-left:14px;font-size:13px;line-height:1.55;color:{TEXT_SECONDARY};\">\
                 <div style=\"font-weight:700;font-size:14px;color:{NAVY};\">{nome}</div>\
                 {linha_cargo}\
                 <div style=\"color:{TEXT_SECONDARY};\">Ocinye</div>\
                 <div><a href=\"mailto:{email}\" \
                   style=\"color:{NAVY_MID};text-decoration:none;\">{email}</a></div>\
               </td>\
             </tr>\
           </table>\
         </div>"
    )
}

/// Compõe as projecções de um corpo de saída a partir do texto que o membro
/// escreveu e dos seus factos.
///
/// - `text` é sempre completo: o texto do membro, e — quando há assinatura — o
///   delimitador `-- ` e a assinatura em texto.
/// - `html` só existe quando há assinatura a mostrar (a oficial ligada, ou uma
///   linha pessoal). Sem nada a acrescentar, a mensagem viaja só em texto, e não
///   se paga uma parte HTML que nada mostra.
#[must_use]
pub fn compose(
    authored: &str,
    facts: &SignatureFacts,
    official: bool,
    logo: LogoRef,
) -> Projections {
    let tem_pessoal = facts
        .personal_line
        .as_deref()
        .map(str::trim)
        .is_some_and(|linha| !linha.is_empty());
    let ha_assinatura = official || tem_pessoal;

    if !ha_assinatura {
        return Projections {
            text: authored.to_owned(),
            html: None,
        };
    }

    // `-- ` numa linha só é o delimitador de assinatura de facto, e os clientes
    // de email reconhecem-no para a separarem do corpo.
    let assinatura_texto = if official {
        signature_text(facts)
    } else {
        // Sem a oficial, mas com linha pessoal: só a linha pessoal.
        facts
            .personal_line
            .as_deref()
            .map(str::trim)
            .unwrap_or_default()
            .to_owned()
    };
    let text = format!("{authored}\n\n-- \n{assinatura_texto}");

    let corpo_html = authored_text_to_html(authored);
    let assinatura_html = if official {
        signature_html(facts, logo)
    } else {
        // Só a linha pessoal, escapada.
        let linha = facts
            .personal_line
            .as_deref()
            .map(str::trim)
            .unwrap_or_default();
        format!(
            "<div style=\"margin-top:12px;color:{TEXT_SECONDARY};font-family:{FONT};\
             font-size:13px;line-height:1.5;\">{}</div>",
            escape(linha).replace('\n', "<br>")
        )
    };

    let html = format!(
        "<div style=\"font-family:{FONT};font-size:14px;color:{TEXT_SECONDARY};\">\
           {corpo_html}\
           <div style=\"margin-top:16px;\">{assinatura_html}</div>\
         </div>"
    );

    Projections {
        text,
        html: Some(html),
    }
}

/// Compose the projections for a message whose body the member authored as rich
/// text (ADR-0415).
///
/// Unlike [`compose`], the HTML body comes in already — sanitised member HTML,
/// not a projection of plain text — and is used verbatim. `authored_text` is the
/// plain-text alternative (the editor's own text), used for `text/plain`. The
/// signature is appended to both parts, exactly as in [`compose`]. Because the
/// member formatted the message, this always yields an HTML part, signature or
/// not.
#[must_use]
pub fn compose_rich(
    authored_html: &str,
    authored_text: &str,
    facts: &SignatureFacts,
    official: bool,
    logo: LogoRef,
) -> Projections {
    let tem_pessoal = facts
        .personal_line
        .as_deref()
        .map(str::trim)
        .is_some_and(|linha| !linha.is_empty());
    let ha_assinatura = official || tem_pessoal;

    let assinatura_texto = if official {
        signature_text(facts)
    } else if tem_pessoal {
        facts
            .personal_line
            .as_deref()
            .map(str::trim)
            .unwrap_or_default()
            .to_owned()
    } else {
        String::new()
    };
    let text = if ha_assinatura {
        format!("{authored_text}\n\n-- \n{assinatura_texto}")
    } else {
        authored_text.to_owned()
    };

    let assinatura_html = if !ha_assinatura {
        String::new()
    } else if official {
        format!(
            "<div style=\"margin-top:16px;\">{}</div>",
            signature_html(facts, logo)
        )
    } else {
        let linha = facts
            .personal_line
            .as_deref()
            .map(str::trim)
            .unwrap_or_default();
        format!(
            "<div style=\"margin-top:16px;color:{TEXT_SECONDARY};font-family:{FONT};\
             font-size:13px;line-height:1.5;\">{}</div>",
            escape(linha).replace('\n', "<br>")
        )
    };

    let html = format!(
        "<div style=\"font-family:{FONT};font-size:14px;color:{TEXT_SECONDARY};\">\
           {authored_html}{assinatura_html}\
         </div>"
    );

    Projections {
        text,
        html: Some(html),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn factos() -> SignatureFacts {
        SignatureFacts {
            full_name: "Fidel Monteiro".to_owned(),
            role: Some("Fundador".to_owned()),
            email: "fidel.monteiro@ocinye.com".to_owned(),
            personal_line: None,
        }
    }

    #[test]
    fn o_texto_tem_a_assinatura_com_factos() {
        let p = compose("Olá", &factos(), true, LogoRef::Cid(LOGO_CONTENT_ID));
        assert!(p.text.contains("Olá"));
        assert!(
            p.text.contains("\n-- \n"),
            "falta o delimitador de assinatura"
        );
        assert!(p.text.contains("Fidel Monteiro"));
        assert!(p.text.contains("Fundador"));
        assert!(p.text.contains("Ocinye"));
        assert!(p.text.contains("fidel.monteiro@ocinye.com"));
    }

    #[test]
    fn o_html_tem_a_assinatura_oficial_e_o_logo_por_cid() {
        let p = compose("Olá", &factos(), true, LogoRef::Cid(LOGO_CONTENT_ID));
        let html = p.html.expect("há projecção HTML");
        assert!(html.contains("Fidel Monteiro"));
        assert!(html.contains("src=\"cid:ocinye-logo\""));
        assert!(html.contains("mailto:fidel.monteiro@ocinye.com"));
    }

    #[test]
    fn a_pre_visualizacao_usa_um_url_e_nao_cid() {
        let html = signature_html(&factos(), LogoRef::Url("/static/ocinye-logo-email.png"));
        assert!(html.contains("src=\"/static/ocinye-logo-email.png\""));
        assert!(!html.contains("cid:"));
    }

    #[test]
    fn um_cargo_em_falta_nao_deixa_linha_vazia() {
        let mut f = factos();
        f.role = None;
        let texto = signature_text(&f);
        // Nome, Ocinye, email — três linhas, sem uma linha vazia do cargo.
        assert_eq!(texto, "Fidel Monteiro\nOcinye\nfidel.monteiro@ocinye.com");
        let html = signature_html(&f, LogoRef::Cid(LOGO_CONTENT_ID));
        assert!(html.contains("Fidel Monteiro"));
        assert!(!html.contains("<div style=\"color:#5F7183;\"></div>"));
    }

    #[test]
    fn o_texto_do_membro_e_escapado_no_html() {
        let p = compose(
            "<script>alert(1)</script> & \"aspas\"",
            &factos(),
            true,
            LogoRef::Cid(LOGO_CONTENT_ID),
        );
        let html = p.html.expect("html");
        assert!(
            !html.contains("<script>"),
            "HTML do membro atravessou: {html}"
        );
        assert!(html.contains("&lt;script&gt;"));
        assert!(html.contains("&amp;"));
        // O texto simples fica como foi escrito — é texto, não marcação.
        assert!(p.text.contains("<script>alert(1)</script>"));
    }

    #[test]
    fn sem_assinatura_nem_linha_pessoal_viaja_so_texto() {
        let mut f = factos();
        f.role = None;
        let p = compose("Corpo", &f, false, LogoRef::Cid(LOGO_CONTENT_ID));
        assert_eq!(p.text, "Corpo");
        assert!(p.html.is_none(), "não devia haver HTML sem nada a mostrar");
    }

    #[test]
    fn a_linha_pessoal_sozinha_ainda_projecta_html_escapado() {
        let mut f = factos();
        f.personal_line = Some("Enviado do <meu> telemóvel".to_owned());
        let p = compose("Corpo", &f, false, LogoRef::Cid(LOGO_CONTENT_ID));
        assert!(p.text.contains("Enviado do <meu> telemóvel"));
        let html = p.html.expect("html com linha pessoal");
        assert!(html.contains("Enviado do &lt;meu&gt; telemóvel"));
        assert!(!html.contains("<meu>"));
    }

    #[test]
    fn os_paragrafos_do_texto_sao_preservados() {
        let html = authored_text_to_html("Primeiro\n\nSegundo\ncom quebra");
        assert_eq!(html.matches("<p ").count(), 2);
        assert!(html.contains("Segundo<br>com quebra"));
    }
}
