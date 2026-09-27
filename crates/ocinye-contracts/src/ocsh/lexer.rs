//! O lexer do ocsh: de uma linha de texto a palavras e barras de pipeline.
//!
//! # O que recusa, e porquê aqui
//!
//! O ocsh não é uma shell do anfitrião e não expande nada. Uma linha com `;`,
//! `&`, `>`, `<`, `` ` `` ou `$(`/`${`/`$NOME` fora de aspas é recusada **antes**
//! de haver comando: não porque o ocsh a fosse executar (não tem com quê), mas
//! para que ninguém tenha dúvidas sobre o que escreveu. Dentro de aspas, estes
//! caracteres são texto — um nome de ficheiro pode ter um `$`.

use std::fmt;

/// Tamanho máximo de uma linha, em bytes.
pub const MAX_LINE_BYTES: usize = 4096;
/// Número máximo de símbolos numa linha.
pub const MAX_TOKENS: usize = 256;

/// Um símbolo da linha.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Token {
    /// Uma palavra, já sem aspas nem escapes.
    Word {
        /// O texto.
        text: String,
        /// Se veio (ao menos em parte) entre aspas: `"--json"` é texto, não uma opção.
        quoted: bool,
    },
    /// A barra de pipeline tipado.
    Pipe,
}

/// Porque é que a linha não se lê.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LexError {
    /// Maior do que [`MAX_LINE_BYTES`].
    TooLong,
    /// Mais símbolos do que [`MAX_TOKENS`].
    TooManyTokens,
    /// Aspas por fechar.
    UnterminatedQuote,
    /// Um caracter de controlo (além de espaço e tabulação).
    ControlCharacter,
    /// Sintaxe de shell do anfitrião, fora de aspas: o texto do operador.
    HostSyntax(&'static str),
    /// Uma barra invertida no fim da linha (a continuação é do cliente).
    DanglingEscape,
}

impl fmt::Display for LexError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooLong => write!(f, "line too long"),
            Self::TooManyTokens => write!(f, "too many tokens"),
            Self::UnterminatedQuote => write!(f, "unterminated quote"),
            Self::ControlCharacter => write!(f, "control character"),
            Self::HostSyntax(op) => write!(f, "host shell syntax not supported: {op}"),
            Self::DanglingEscape => write!(f, "dangling escape"),
        }
    }
}

/// Lê uma linha.
///
/// # Errors
///
/// [`LexError`] quando a linha tem sintaxe que o ocsh não aceita.
pub fn lex(line: &str) -> Result<Vec<Token>, LexError> {
    if line.len() > MAX_LINE_BYTES {
        return Err(LexError::TooLong);
    }
    if line
        .chars()
        .any(|c| c.is_control() && c != ' ' && c != '\t')
    {
        return Err(LexError::ControlCharacter);
    }

    let mut tokens: Vec<Token> = Vec::new();
    let mut current = String::new();
    let mut in_word = false;
    let mut quoted = false;
    let mut chars = line.chars().peekable();

    let push = |tokens: &mut Vec<Token>, text: &mut String, quoted: &mut bool| {
        tokens.push(Token::Word {
            text: std::mem::take(text),
            quoted: *quoted,
        });
        *quoted = false;
    };

    while let Some(c) = chars.next() {
        match c {
            ' ' | '\t' => {
                if in_word {
                    push(&mut tokens, &mut current, &mut quoted);
                    in_word = false;
                }
            }
            '"' => {
                in_word = true;
                quoted = true;
                let mut closed = false;
                while let Some(d) = chars.next() {
                    match d {
                        '"' => {
                            closed = true;
                            break;
                        }
                        '\\' => match chars.next() {
                            Some(e @ ('"' | '\\')) => current.push(e),
                            Some(e) => {
                                current.push('\\');
                                current.push(e);
                            }
                            None => return Err(LexError::UnterminatedQuote),
                        },
                        other => current.push(other),
                    }
                }
                if !closed {
                    return Err(LexError::UnterminatedQuote);
                }
            }
            '\'' => {
                in_word = true;
                quoted = true;
                let mut closed = false;
                for d in chars.by_ref() {
                    if d == '\'' {
                        closed = true;
                        break;
                    }
                    current.push(d);
                }
                if !closed {
                    return Err(LexError::UnterminatedQuote);
                }
            }
            '\\' => match chars.next() {
                Some(e) => {
                    in_word = true;
                    current.push(e);
                }
                None => return Err(LexError::DanglingEscape),
            },
            '|' => {
                if chars.peek() == Some(&'|') {
                    return Err(LexError::HostSyntax("||"));
                }
                if in_word {
                    push(&mut tokens, &mut current, &mut quoted);
                    in_word = false;
                }
                tokens.push(Token::Pipe);
            }
            ';' => return Err(LexError::HostSyntax(";")),
            '&' => {
                return Err(LexError::HostSyntax(if chars.peek() == Some(&'&') {
                    "&&"
                } else {
                    "&"
                }))
            }
            '>' => return Err(LexError::HostSyntax(">")),
            '<' => return Err(LexError::HostSyntax("<")),
            '`' => return Err(LexError::HostSyntax("`")),
            '$' => match chars.peek() {
                Some('(') => return Err(LexError::HostSyntax("$(")),
                Some('{') => return Err(LexError::HostSyntax("${")),
                Some(n) if n.is_ascii_alphabetic() || *n == '_' => {
                    return Err(LexError::HostSyntax("$VAR"))
                }
                _ => {
                    in_word = true;
                    current.push('$');
                }
            },
            other => {
                in_word = true;
                current.push(other);
            }
        }
        if tokens.len() > MAX_TOKENS {
            return Err(LexError::TooManyTokens);
        }
    }
    if in_word {
        push(&mut tokens, &mut current, &mut quoted);
    }
    if tokens.len() > MAX_TOKENS {
        return Err(LexError::TooManyTokens);
    }
    Ok(tokens)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(line: &str) -> Vec<String> {
        lex(line)
            .expect("lê")
            .into_iter()
            .map(|t| match t {
                Token::Word { text, .. } => text,
                Token::Pipe => "|".to_owned(),
            })
            .collect()
    }

    #[test]
    fn palavras_aspas_e_escapes() {
        assert_eq!(
            words(r#"files mkdir "Annual Reports""#),
            ["files", "mkdir", "Annual Reports"]
        );
        assert_eq!(
            words("files mkdir 'a \"b\" c'"),
            ["files", "mkdir", "a \"b\" c"]
        );
        assert_eq!(words(r#"x "a\"b""#), ["x", "a\"b"]);
        assert_eq!(
            words(r"files mkdir Annual\ Reports"),
            ["files", "mkdir", "Annual Reports"]
        );
        assert_eq!(words("  projects   list  "), ["projects", "list"]);
        assert_eq!(
            words("projects list | filter active"),
            ["projects", "list", "|", "filter", "active"]
        );
        assert_eq!(words("a|b"), ["a", "|", "b"]);
    }

    #[test]
    fn dentro_de_aspas_a_sintaxe_do_anfitriao_e_texto() {
        assert_eq!(
            words(r#"files mkdir "$(rm -rf /); `x` > y && z""#)
                .last()
                .unwrap(),
            "$(rm -rf /); `x` > y && z"
        );
        assert_eq!(
            words("files mkdir 'custo $5'"),
            ["files", "mkdir", "custo $5"]
        );
        assert_eq!(words("preco $5"), ["preco", "$5"]);
    }

    #[test]
    fn a_sintaxe_do_anfitriao_fora_de_aspas_e_recusada() {
        for (linha, op) in [
            ("; rm -rf /", ";"),
            ("files ls; rm -rf /", ";"),
            ("files ls && rm -rf /", "&&"),
            ("files ls & ", "&"),
            ("files ls || true", "||"),
            ("files ls > /etc/passwd", ">"),
            ("files ls < x", "<"),
            ("files ls `id`", "`"),
            ("files ls $(id)", "$("),
            ("files ls ${HOME}", "${"),
            ("files ls $HOME", "$VAR"),
        ] {
            assert_eq!(lex(linha), Err(LexError::HostSyntax(op)), "{linha}");
        }
    }

    #[test]
    fn linhas_malformadas_nao_rebentam() {
        assert_eq!(
            lex(r#"files mkdir "aberta"#),
            Err(LexError::UnterminatedQuote)
        );
        assert_eq!(lex("files mkdir 'aberta"), Err(LexError::UnterminatedQuote));
        assert_eq!(lex("files ls \\"), Err(LexError::DanglingEscape));
        assert_eq!(lex("files\u{0}ls"), Err(LexError::ControlCharacter));
        assert_eq!(lex("files\u{1b}[31mls"), Err(LexError::ControlCharacter));
        assert_eq!(lex(&"a".repeat(MAX_LINE_BYTES + 1)), Err(LexError::TooLong));
        assert_eq!(
            lex(&"a ".repeat(MAX_TOKENS + 1)),
            Err(LexError::TooManyTokens)
        );
        assert_eq!(lex(""), Ok(vec![]));
        assert_eq!(
            words("files mkdir Relatórios 🙂 ﬁ"),
            ["files", "mkdir", "Relatórios", "🙂", "ﬁ"]
        );
    }

    #[test]
    fn aspas_marcam_a_palavra() {
        let t = lex(r#"x "--json""#).unwrap();
        assert_eq!(
            t[1],
            Token::Word {
                text: "--json".into(),
                quoted: true
            }
        );
    }
}
