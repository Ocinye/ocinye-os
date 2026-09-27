//! ocsh — a Ocinye Shell: gramática, lexer, parser e registo de comandos.
//!
//! Dados e funções puras, sem IO: partilhados pelo Core (que decide e
//! executa), pelo Workspace (ajuda e autocompletar imediatos) e por um futuro
//! CLI `ocinye`. **O parse autoritativo é o do Core**; o do cliente é só
//! conveniência.
//!
//! # O que o ocsh não é
//!
//! Uma shell do anfitrião. Não há `sh -c`, nem `exec`, nem expansão, nem
//! redirecção, nem variáveis do processo. Um comando invoca uma capability do
//! Core, e é o Core que autoriza (ADR-0312).

pub mod lexer;
pub mod parser;
pub mod registry;
pub mod wire;

pub use parser::{parse, Invocation, ParseError, Parsed, Stage, Value};

/// Os códigos de saída do ocsh. Estáveis e documentados
/// (`docs/terminal/COMMAND_MODEL.md`); não espelham o HTTP.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExitCode {
    /// Sucesso.
    Ok,
    /// Falha de execução, não encontrado, confirmação recusada.
    Failure,
    /// Uso inválido.
    Usage,
    /// Capacidade indisponível (ex.: IA sem recurso).
    Unavailable,
    /// Permissão negada, política bloqueou, requer elevação.
    Denied,
    /// Bloqueado por desenho (shell do anfitrião).
    Blocked,
    /// Comando não encontrado.
    NotFound,
    /// Cancelado.
    Cancelled,
}

impl ExitCode {
    /// O número.
    #[must_use]
    pub const fn code(self) -> u8 {
        match self {
            Self::Ok => 0,
            Self::Failure => 1,
            Self::Usage => 2,
            Self::Unavailable => 69,
            Self::Denied => 77,
            Self::Blocked => 126,
            Self::NotFound => 127,
            Self::Cancelled => 130,
        }
    }
}

/// Nomes de opção que são sempre sensíveis, declarados ou não.
pub const SENSITIVE_OPTION_NAMES: &[&str] = &[
    "password", "token", "secret", "key", "api-key", "mfa", "code",
];

/// A linha pronta para histórico, logs ou auditoria: valores de argumentos e
/// opções sensíveis substituídos por `••••`.
///
/// Trabalha sobre a linha **escrita**, não sobre o parse, para também cobrir
/// linhas que não chegaram a ser comandos válidos.
#[must_use]
pub fn redact(line: &str) -> String {
    let Ok(tokens) = lexer::lex(line) else {
        // Linha ilegível: guarda-se só a primeira palavra, sem nada que possa
        // ser um segredo.
        return line
            .split_whitespace()
            .next()
            .map_or_else(String::new, |w| format!("{w} ••••"));
    };
    let mut out: Vec<String> = Vec::new();
    let mut hide_next = false;
    for t in tokens {
        match t {
            lexer::Token::Pipe => out.push("|".into()),
            lexer::Token::Word { text, quoted } => {
                if hide_next {
                    out.push("••••".into());
                    hide_next = false;
                    continue;
                }
                if !quoted {
                    if let Some(body) = text.strip_prefix("--") {
                        let (name, inline) = match body.split_once('=') {
                            Some((n, _)) => (n, true),
                            None => (body, false),
                        };
                        if is_sensitive_option(name) {
                            if inline {
                                out.push(format!("--{name}=••••"));
                            } else {
                                out.push(text.clone());
                                hide_next = true;
                            }
                            continue;
                        }
                    }
                }
                out.push(if quoted || text.contains(' ') {
                    quote(&text)
                } else {
                    text
                });
            }
        }
    }
    // Argumentos posicionais declarados sensíveis.
    if let Ok(Parsed::Run(inv)) = parse(line) {
        for (name, value) in &inv.args {
            let secret = inv.spec.args.iter().any(|a| a.name == *name && a.sensitive);
            if let (true, Some(text)) = (secret, value.as_text()) {
                for w in &mut out {
                    if w == text || *w == quote(text) {
                        *w = "••••".into();
                    }
                }
            }
        }
    }
    out.join(" ")
}

fn is_sensitive_option(name: &str) -> bool {
    SENSITIVE_OPTION_NAMES.contains(&name)
        || registry::COMMANDS
            .iter()
            .flat_map(|c| c.options.iter())
            .any(|o| o.name == name && o.sensitive)
}

fn quote(text: &str) -> String {
    format!("\"{}\"", text.replace('\\', "\\\\").replace('"', "\\\""))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codigos_de_saida_estaveis() {
        let todos = [
            (ExitCode::Ok, 0),
            (ExitCode::Failure, 1),
            (ExitCode::Usage, 2),
            (ExitCode::Unavailable, 69),
            (ExitCode::Denied, 77),
            (ExitCode::Blocked, 126),
            (ExitCode::NotFound, 127),
            (ExitCode::Cancelled, 130),
        ];
        for (c, n) in todos {
            assert_eq!(c.code(), n);
        }
    }

    #[test]
    fn segredos_nunca_ficam_na_linha_redigida() {
        for (linha, segredo) in [
            ("provider add --key sk-live-123", "sk-live-123"),
            ("provider add --key=sk-live-123", "sk-live-123"),
            ("x --password 'hunter2 com espaço'", "hunter2"),
            ("x --token=abc.def", "abc.def"),
            ("x --mfa 123456", "123456"),
            ("x --password \"unterminated", "unterminated"),
        ] {
            let r = redact(linha);
            assert!(!r.contains(segredo), "{linha} → {r}");
        }
        assert_eq!(redact("tasks list --open"), "tasks list --open");
        assert_eq!(
            redact(r#"nye ask "o que há hoje""#),
            r#"nye ask "o que há hoje""#
        );
    }
}
