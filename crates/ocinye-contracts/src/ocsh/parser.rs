//! O parser do ocsh: dos símbolos a uma invocação tipada, validada contra o
//! registo, ou a um erro — sempre antes de qualquer execução.
//!
//! Determinístico. Nenhum modelo lê a linha, e um comando desconhecido é
//! desconhecido: nunca é entregue ao Nye (só `nye …` e `? …` o são).

use std::collections::BTreeMap;

use super::lexer::{lex, LexError, Token};
use super::registry::{
    command, distance, family, CommandSpec, ValueKind, FAMILIES, HOST_SHELL_WORDS,
    POSIX_HINTS,
};
use super::ExitCode;

/// Um valor já tipado.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Value {
    /// Texto (também caminhos e identificadores, validados pelo executor).
    Text(String),
    /// Inteiro não negativo.
    Integer(u64),
}

impl Value {
    /// O texto, se for texto.
    #[must_use]
    pub fn as_text(&self) -> Option<&str> {
        match self {
            Self::Text(t) => Some(t),
            Self::Integer(_) => None,
        }
    }
}

/// Uma operação de pipeline tipado.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Stage {
    /// Mantém as linhas com este texto em algum valor.
    Filter(String),
    /// Ordena por uma coluna.
    Sort {
        /// A coluna.
        column: String,
        /// Descendente.
        desc: bool,
    },
    /// As primeiras n.
    Head(u64),
    /// Quantas.
    Count,
    /// Saída em JSON.
    ExportJson,
}

/// Uma invocação validada.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Invocation {
    /// A definição do comando.
    pub spec: &'static CommandSpec,
    /// Argumentos, pelo nome da definição.
    pub args: BTreeMap<&'static str, Value>,
    /// Opções com valor.
    pub options: BTreeMap<&'static str, Value>,
    /// Interruptores presentes.
    pub flags: Vec<&'static str>,
    /// `--json` (ou `| export json`).
    pub json: bool,
    /// As operações de pipeline, por ordem.
    pub stages: Vec<Stage>,
}

/// O resultado do parse.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Parsed {
    /// Linha vazia.
    Empty,
    /// Um comando para executar.
    Run(Invocation),
    /// `família --help`: ajuda desta família (ou comando).
    Help(String),
}

/// Porque é que a linha não é um comando.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    /// O lexer recusou.
    Lex(LexError),
    /// Palavra de uma shell do anfitrião (`sudo`, `bash`, …).
    HostShell(String),
    /// Família desconhecida, com uma sugestão.
    UnknownCommand {
        /// O que foi escrito.
        word: String,
        /// «Quis dizer …?»
        suggestion: Option<String>,
    },
    /// Subcomando desconhecido (ou em falta numa família sem omissão).
    UnknownSubcommand {
        /// A família.
        family: String,
        /// O que foi escrito (`""` se nada).
        word: String,
    },
    /// Falta um argumento obrigatório.
    MissingArgument(&'static str),
    /// Argumentos a mais.
    UnexpectedArgument(String),
    /// Opção desconhecida.
    UnknownOption(String),
    /// Opção sem o valor que precisa.
    MissingOptionValue(&'static str),
    /// Valor com o tipo errado.
    BadValue {
        /// Onde.
        name: &'static str,
        /// O que foi escrito.
        value: String,
    },
    /// Operação de pipeline desconhecida ou mal formada.
    BadStage(String),
    /// `--json` num comando que não o aceita.
    JsonNotSupported,
    /// Pipeline sobre um comando que não produz uma tabela.
    NotPipeable,
}

impl ParseError {
    /// O código de saída desta falha.
    #[must_use]
    pub const fn exit(&self) -> ExitCode {
        match self {
            Self::Lex(LexError::HostSyntax(_)) | Self::HostShell(_) => ExitCode::Blocked,
            Self::UnknownCommand { .. } => ExitCode::NotFound,
            _ => ExitCode::Usage,
        }
    }
}

/// Lê e valida uma linha.
///
/// # Errors
///
/// [`ParseError`] quando a linha não é um comando válido do ocsh.
pub fn parse(line: &str) -> Result<Parsed, ParseError> {
    // `? pergunta` é o atalho explícito para `nye ask`.
    let trimmed = line.trim_start();
    if let Some(rest) = trimmed.strip_prefix('?') {
        let question = rest.trim();
        if question.is_empty() {
            return Err(ParseError::MissingArgument("question"));
        }
        let spec = command("nye", "ask").expect("nye ask está no registo");
        let mut args = BTreeMap::new();
        args.insert("question", Value::Text(question.to_owned()));
        return Ok(Parsed::Run(Invocation {
            spec,
            args,
            options: BTreeMap::new(),
            flags: vec![],
            json: false,
            stages: vec![],
        }));
    }

    let tokens = lex(line).map_err(ParseError::Lex)?;
    let mut segments: Vec<Vec<(String, bool)>> = vec![vec![]];
    for t in tokens {
        match t {
            Token::Word { text, quoted } => segments.last_mut().expect("há sempre um").push((text, quoted)),
            Token::Pipe => segments.push(vec![]),
        }
    }
    let head = segments.remove(0);
    if head.is_empty() {
        if segments.is_empty() {
            return Ok(Parsed::Empty);
        }
        return Err(ParseError::BadStage("|".into()));
    }

    let (first, _) = &head[0];
    let lower = first.to_lowercase();
    if HOST_SHELL_WORDS.contains(&lower.as_str()) {
        return Err(ParseError::HostShell(first.clone()));
    }
    let Some(fam) = family(first) else {
        return Err(ParseError::UnknownCommand {
            word: first.clone(),
            suggestion: suggest(first),
        });
    };

    let mut rest: &[(String, bool)] = &head[1..];

    // `família --help` / `família sub --help`.
    if rest.iter().any(|(w, q)| !q && (w == "--help" || w == "-h")) {
        let topic = rest
            .iter()
            .find(|(w, q)| *q || !w.starts_with('-'))
            .map_or_else(|| fam.name.to_owned(), |(w, _)| format!("{} {w}", fam.name));
        return Ok(Parsed::Help(topic));
    }

    // O subcomando: o próximo símbolo, se a família o declarar.
    let has_subs = super::registry::commands_of(fam.name).any(|c| !c.sub.is_empty());
    let spec = if has_subs {
        match rest.first() {
            Some((w, false)) if command(fam.name, w).is_some() => {
                let s = command(fam.name, w).expect("verificado");
                rest = &rest[1..];
                s
            }
            Some((w, false)) if !w.starts_with('-') && fam.default_sub.is_none() => {
                return Err(ParseError::UnknownSubcommand { family: fam.name.into(), word: w.clone() })
            }
            Some((w, false)) if !w.starts_with('-') && command(fam.name, fam.default_sub.unwrap_or("")).is_some_and(|c| c.args.is_empty()) => {
                return Err(ParseError::UnknownSubcommand { family: fam.name.into(), word: w.clone() })
            }
            _ => match fam.default_sub {
                Some(d) => command(fam.name, d).expect("omissão verificada no registo"),
                None => {
                    return Err(ParseError::UnknownSubcommand {
                        family: fam.name.into(),
                        word: rest.first().map(|(w, _)| w.clone()).unwrap_or_default(),
                    })
                }
            },
        }
    } else {
        command(fam.name, "").expect("família sem subcomandos tem a entrada vazia")
    };

    let mut inv = bind(spec, rest)?;

    for seg in segments {
        let stage = stage(&seg)?;
        if !matches!(spec.output, super::registry::OutputShape::Table(_)) {
            return Err(ParseError::NotPipeable);
        }
        if stage == Stage::ExportJson {
            inv.json = true;
        }
        inv.stages.push(stage);
    }
    if inv.json && !spec.json {
        return Err(ParseError::JsonNotSupported);
    }
    Ok(Parsed::Run(inv))
}

fn bind(spec: &'static CommandSpec, words: &[(String, bool)]) -> Result<Invocation, ParseError> {
    let mut args = BTreeMap::new();
    let mut options = BTreeMap::new();
    let mut flags = Vec::new();
    let mut json = false;
    let mut positional: Vec<String> = Vec::new();

    let mut i = 0;
    while i < words.len() {
        let (w, quoted) = &words[i];
        // Depois de um argumento `rest`, tudo é texto dele.
        let absorbing = spec.args.last().is_some_and(|a| a.rest) && positional.len() + 1 >= spec.args.len() && !positional.is_empty();
        if !quoted && !absorbing && w.starts_with("--") && w.len() > 2 {
            let body = &w[2..];
            let (name, inline) = match body.split_once('=') {
                Some((n, v)) => (n, Some(v.to_owned())),
                None => (body, None),
            };
            if name == "json" && inline.is_none() {
                json = true;
                i += 1;
                continue;
            }
            let Some(opt) = spec.options.iter().find(|o| o.name == name) else {
                return Err(ParseError::UnknownOption(format!("--{name}")));
            };
            match opt.value {
                None => {
                    if inline.is_some() {
                        return Err(ParseError::UnknownOption(format!("--{name}")));
                    }
                    flags.push(opt.name);
                }
                Some(kind) => {
                    let raw = match inline {
                        Some(v) => v,
                        None => {
                            i += 1;
                            words.get(i).map(|(v, _)| v.clone()).ok_or(ParseError::MissingOptionValue(opt.name))?
                        }
                    };
                    options.insert(opt.name, typed(opt.name, kind, raw)?);
                }
            }
        } else if !quoted && !absorbing && w.starts_with('-') && w.len() > 1 && !w[1..].starts_with(|c: char| c.is_ascii_digit()) {
            return Err(ParseError::UnknownOption(w.clone()));
        } else {
            positional.push(w.clone());
        }
        i += 1;
    }

    let mut pos = positional.into_iter();
    for (idx, a) in spec.args.iter().enumerate() {
        if a.rest {
            let joined: Vec<String> = pos.by_ref().collect();
            if joined.is_empty() {
                if a.required {
                    return Err(ParseError::MissingArgument(a.name));
                }
            } else {
                args.insert(a.name, Value::Text(joined.join(" ")));
            }
            debug_assert_eq!(idx + 1, spec.args.len());
            break;
        }
        match pos.next() {
            Some(raw) => {
                args.insert(a.name, typed(a.name, a.kind, raw)?);
            }
            None if a.required => return Err(ParseError::MissingArgument(a.name)),
            None => {}
        }
    }
    if let Some(extra) = pos.next() {
        return Err(ParseError::UnexpectedArgument(extra));
    }
    Ok(Invocation { spec, args, options, flags, json, stages: vec![] })
}

fn typed(name: &'static str, kind: ValueKind, raw: String) -> Result<Value, ParseError> {
    match kind {
        ValueKind::Integer => raw
            .parse::<u64>()
            .map(Value::Integer)
            .map_err(|_| ParseError::BadValue { name, value: raw }),
        ValueKind::Text | ValueKind::Path | ValueKind::Id => Ok(Value::Text(raw)),
    }
}

fn stage(words: &[(String, bool)]) -> Result<Stage, ParseError> {
    let Some((op, _)) = words.first() else {
        return Err(ParseError::BadStage("|".into()));
    };
    let rest = &words[1..];
    let one = |r: &[(String, bool)]| -> Result<String, ParseError> {
        match r {
            [(v, _)] => Ok(v.clone()),
            _ => Err(ParseError::BadStage(op.clone())),
        }
    };
    match op.as_str() {
        "filter" => {
            if rest.is_empty() {
                return Err(ParseError::BadStage(op.clone()));
            }
            Ok(Stage::Filter(rest.iter().map(|(w, _)| w.as_str()).collect::<Vec<_>>().join(" ")))
        }
        "sort" => match rest {
            [(c, _)] => Ok(Stage::Sort { column: c.clone(), desc: false }),
            [(c, _), (d, false)] if d == "--desc" => Ok(Stage::Sort { column: c.clone(), desc: true }),
            _ => Err(ParseError::BadStage(op.clone())),
        },
        "head" => one(rest)?
            .parse::<u64>()
            .map(Stage::Head)
            .map_err(|_| ParseError::BadStage(op.clone())),
        "count" if rest.is_empty() => Ok(Stage::Count),
        "export" if one(rest)? == "json" => Ok(Stage::ExportJson),
        _ => Err(ParseError::BadStage(op.clone())),
    }
}

/// «Quis dizer …?»: a palavra POSIX equivalente, ou a família mais próxima
/// (distância ≤ 2).
#[must_use]
pub fn suggest(word: &str) -> Option<String> {
    if let Some((_, hint)) = POSIX_HINTS.iter().find(|(w, _)| *w == word) {
        return Some((*hint).to_owned());
    }
    FAMILIES
        .iter()
        .map(|f| (distance(word, f.name), f.name))
        .filter(|(d, _)| *d <= 2)
        .min()
        .map(|(_, n)| n.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(line: &str) -> Invocation {
        match parse(line) {
            Ok(Parsed::Run(i)) => i,
            other => panic!("{line}: {other:?}"),
        }
    }

    #[test]
    fn familia_sub_opcoes_e_omissao() {
        let i = run("tasks list --mine");
        assert_eq!((i.spec.family, i.spec.sub), ("tasks", "list"));
        assert_eq!(i.flags, ["mine"]);
        let i = run("tasks");
        assert_eq!(i.spec.sub, "list", "a omissão");
        let i = run("tasks --json");
        assert!(i.json);
    }

    #[test]
    fn nye_ask_absorve_o_resto_e_o_atalho() {
        let i = run("nye ask resume o projecto --json");
        assert_eq!(i.args["question"], Value::Text("resume o projecto --json".into()));
        let i = run("?  o que tenho para hoje");
        assert_eq!((i.spec.family, i.spec.sub), ("nye", "ask"));
        assert_eq!(i.args["question"], Value::Text("o que tenho para hoje".into()));
        assert_eq!(parse("?"), Err(ParseError::MissingArgument("question")));
        assert!(matches!(parse("nye"), Err(ParseError::UnknownSubcommand { .. })));
    }

    #[test]
    fn desconhecido_e_127_com_sugestao_e_nunca_vai_ao_nye() {
        let e = parse("taks list").unwrap_err();
        assert_eq!(e, ParseError::UnknownCommand { word: "taks".into(), suggestion: Some("tasks".into()) });
        assert_eq!(e.exit(), ExitCode::NotFound);
        let e = parse("ls -la").unwrap_err();
        assert_eq!(e, ParseError::UnknownCommand { word: "ls".into(), suggestion: Some("files ls".into()) });
        let e = parse("resume este projecto por favor").unwrap_err();
        assert!(matches!(e, ParseError::UnknownCommand { .. }), "linguagem natural não é um comando");
    }

    #[test]
    fn shells_do_anfitriao_sao_126() {
        for linha in ["sudo tasks list", "bash -c 'rm -rf /'", "SH", "ssh root@host", "eval x"] {
            let e = parse(linha).unwrap_err();
            assert!(matches!(e, ParseError::HostShell(_)), "{linha}: {e:?}");
            assert_eq!(e.exit(), ExitCode::Blocked);
        }
        for linha in ["tasks; rm -rf /", "tasks && id", "tasks | /bin/sh", "tasks $(id)", "tasks `id`"] {
            let e = parse(linha).unwrap_err();
            assert!(matches!(e.exit(), ExitCode::Blocked | ExitCode::Usage), "{linha}: {e:?}");
        }
        // `| /bin/sh` não é uma operação de pipeline: erro de uso, nunca execução.
        assert_eq!(parse("tasks | /bin/sh"), Err(ParseError::BadStage("/bin/sh".into())));
    }

    #[test]
    fn erros_de_uso() {
        assert_eq!(parse("tasks list --everything"), Err(ParseError::UnknownOption("--everything".into())));
        assert_eq!(parse("tasks list extra"), Err(ParseError::UnexpectedArgument("extra".into())));
        assert_eq!(parse("tasks frobnicate"), Err(ParseError::UnknownSubcommand { family: "tasks".into(), word: "frobnicate".into() }));
        assert_eq!(parse("clear --json"), Err(ParseError::JsonNotSupported));
        assert_eq!(parse("clear | count"), Err(ParseError::NotPipeable));
        assert_eq!(parse("tasks list --mine=yes"), Err(ParseError::UnknownOption("--mine".into())));
        assert_eq!(parse("| count"), Err(ParseError::BadStage("|".into())));
        assert_eq!(parse("   "), Ok(Parsed::Empty));
    }

    #[test]
    fn opcao_entre_aspas_e_texto() {
        // `"--help"` entre aspas não pede ajuda.
        let i = run(r#"nye ask "--help""#);
        assert_eq!(i.args["question"], Value::Text("--help".into()));
    }

    #[test]
    fn ajuda_por_opcao() {
        assert_eq!(parse("tasks --help"), Ok(Parsed::Help("tasks".into())));
        assert_eq!(parse("tasks list -h"), Ok(Parsed::Help("tasks list".into())));
    }

    #[test]
    fn pipelines_tipados() {
        let i = run("tasks list | filter active | sort due --desc | head 5");
        assert_eq!(
            i.stages,
            vec![
                Stage::Filter("active".into()),
                Stage::Sort { column: "due".into(), desc: true },
                Stage::Head(5)
            ]
        );
        let i = run("tasks | export json");
        assert!(i.json);
        assert_eq!(parse("tasks | head x"), Err(ParseError::BadStage("head".into())));
        assert_eq!(parse("tasks | grep x"), Err(ParseError::BadStage("grep".into())));
        assert_eq!(parse("tasks | count extra"), Err(ParseError::BadStage("count".into())));
    }

    /// Fuzz determinístico: linhas geradas de um alfabeto hostil nunca fazem o
    /// parser entrar em pânico, e nenhuma com sintaxe de anfitrião fora de
    /// aspas vira um comando.
    #[test]
    fn fuzz_deterministico_nao_rebenta() {
        const ALFABETO: &[&str] = &[
            "tasks", "list", "nye", "ask", "help", "--mine", "--json", "--help", "-h", "|", "filter",
            "sort", "head", "count", "export", "json", "\"", "'", "\\", ";", "&", "&&", ">", "<",
            "`", "$(", "${", "$X", "$", " ", "\t", "?", "../", "é", "🙂", "\u{202e}", "=", "--x=",
            "5", "-1", "sudo", "ls",
        ];
        let mut estado: u64 = 0x9E37_79B9_7F4A_7C15;
        for _ in 0..20_000 {
            let mut linha = String::new();
            estado ^= estado << 13;
            estado ^= estado >> 7;
            estado ^= estado << 17;
            let n = (estado % 9) as usize;
            for _ in 0..n {
                estado ^= estado << 13;
                estado ^= estado >> 7;
                estado ^= estado << 17;
                linha.push_str(ALFABETO[(estado % ALFABETO.len() as u64) as usize]);
                if estado & 1 == 0 {
                    linha.push(' ');
                }
            }
            if let Ok(Parsed::Run(inv)) = parse(&linha) {
                // Se é um comando, é um comando do registo — nada mais.
                assert_eq!(command(inv.spec.family, inv.spec.sub), Some(inv.spec), "{linha}");
            }
        }
    }

    #[test]
    fn entradas_estranhas_nao_rebentam() {
        for linha in [
            "../../etc/passwd",
            "%2e%2e%2f",
            "tasks\u{202e}list",
            "ｔａｓｋｓ",
            "\"\"",
            "''",
            "|||",
            "tasks | | count",
            "-",
            "--",
            "nye ask \u{1F600}",
        ] {
            let _ = parse(linha);
        }
    }
}
