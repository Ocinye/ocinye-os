//! O registo de comandos do ocsh: uma só lista, que serve o parse, a
//! validação, a ajuda, o autocompletar, a documentação e a execução.
//!
//! # O que não está aqui
//!
//! O **risco** e a **autoridade**. São da capability que o comando invoca, e o
//! Core lê-os do registo agentic no momento da execução: um comando não pode
//! declarar-se mais seguro do que a acção que faz, e ver um comando no registo
//! não dá direito a executá-lo.

/// O tipo de um argumento ou de um valor de opção.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValueKind {
    /// Texto livre.
    Text,
    /// Um inteiro não negativo.
    Integer,
    /// Um caminho no espaço de nomes do Ocinye (nunca do anfitrião).
    Path,
    /// Um identificador de recurso (UUID ou código).
    Id,
}

/// Um argumento posicional.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArgSpec {
    /// Nome, para a ajuda e para o executor.
    pub name: &'static str,
    /// Tipo.
    pub kind: ValueKind,
    /// Se é obrigatório.
    pub required: bool,
    /// Se é o último e absorve o resto da linha (`nye ask o que quiser`).
    pub rest: bool,
    /// Se nunca pode ir para histórico, logs ou auditoria.
    pub sensitive: bool,
}

/// Uma opção `--nome`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OptSpec {
    /// Nome, sem os dois traços.
    pub name: &'static str,
    /// `None` = interruptor (`--mine`); `Some` = leva um valor.
    pub value: Option<ValueKind>,
    /// Se o valor nunca pode ir para histórico, logs ou auditoria.
    pub sensitive: bool,
}

/// O que executa o comando.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Binding {
    /// Resolve-se no cliente, sem o Core (`help`, `clear`, `history`, `exit`).
    Local,
    /// Uma capability do registo agentic do Core, pelo seu id estável.
    Capability(&'static str),
    /// A ponte explícita para o Nye (Superfície Universal, intenção «perguntar»).
    Nye,
}

/// A forma da saída.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputShape {
    /// Uma tabela com estas colunas (ids estáveis; os rótulos são i18n).
    Table(&'static [&'static str]),
    /// Linhas de texto.
    Lines,
    /// Pares chave/valor, por esta ordem (ids estáveis; os rótulos são i18n).
    Facts(&'static [&'static str]),
    /// Uma nota (sucesso, aviso, erro).
    Note,
}

/// Quem vê o comando na ajuda e no autocompletar. **Só descoberta**: quem
/// decide se executa é o Core.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Audience {
    /// Qualquer membro.
    Member,
    /// Administração.
    Admin,
    /// Operação da instalação.
    Operator,
}

/// De onde vêm as sugestões de um argumento.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Completion {
    /// Nenhuma.
    None,
    /// Famílias de comandos.
    Families,
    /// Aplicações.
    Apps,
    /// Pastas em `~/files`.
    Folders,
    /// Contextos alcançáveis.
    Contexts,
    /// Projectos alcançáveis.
    Projects,
}

/// Um subcomando (ou a acção única de uma família sem subcomandos).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommandSpec {
    /// Família (`files`).
    pub family: &'static str,
    /// Subcomando (`mkdir`), ou `""` numa família sem subcomandos.
    pub sub: &'static str,
    /// Argumentos posicionais, por ordem.
    pub args: &'static [ArgSpec],
    /// Opções próprias (as globais `--json` e `--help` são do parser).
    pub options: &'static [OptSpec],
    /// O que executa.
    pub binding: Binding,
    /// A forma da saída.
    pub output: OutputShape,
    /// Se aceita `--json`.
    pub json: bool,
    /// Chave i18n da descrição.
    pub help_key: &'static str,
    /// Sugestões para o primeiro argumento.
    pub complete: Completion,
    /// Quem o vê na descoberta.
    pub audience: Audience,
}

/// Uma família de comandos.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FamilySpec {
    /// Nome (`files`).
    pub name: &'static str,
    /// Subcomando usado quando nenhum é escrito (`storage` ≡ `storage status`).
    pub default_sub: Option<&'static str>,
    /// Chave i18n da descrição da família.
    pub help_key: &'static str,
}

const NO_ARGS: &[ArgSpec] = &[];
const NO_OPTS: &[OptSpec] = &[];

/// As famílias, pela ordem da ajuda.
pub const FAMILIES: &[FamilySpec] = &[
    FamilySpec { name: "help", default_sub: None, help_key: "ocsh.family.help" },
    FamilySpec { name: "clear", default_sub: None, help_key: "ocsh.family.clear" },
    FamilySpec { name: "history", default_sub: None, help_key: "ocsh.family.history" },
    FamilySpec { name: "exit", default_sub: None, help_key: "ocsh.family.exit" },
    FamilySpec { name: "whoami", default_sub: None, help_key: "ocsh.family.whoami" },
    FamilySpec { name: "context", default_sub: Some("show"), help_key: "ocsh.family.context" },
    FamilySpec { name: "tasks", default_sub: Some("list"), help_key: "ocsh.family.tasks" },
    FamilySpec { name: "nodes", default_sub: Some("list"), help_key: "ocsh.family.nodes" },
    FamilySpec { name: "nye", default_sub: None, help_key: "ocsh.family.nye" },
];

/// Os comandos. Cada entrada com [`Binding::Capability`] tem de existir no
/// registo agentic do Core — há um teste no Core que falha se não existir.
pub const COMMANDS: &[CommandSpec] = &[
    CommandSpec {
        family: "help",
        sub: "",
        args: &[ArgSpec { name: "topic", kind: ValueKind::Text, required: false, rest: false, sensitive: false }],
        options: NO_OPTS,
        binding: Binding::Local,
        output: OutputShape::Lines,
        json: false,
        help_key: "ocsh.cmd.help",
        complete: Completion::Families,
        audience: Audience::Member,
    },
    CommandSpec {
        family: "clear",
        sub: "",
        args: NO_ARGS,
        options: NO_OPTS,
        binding: Binding::Local,
        output: OutputShape::Note,
        json: false,
        help_key: "ocsh.cmd.clear",
        complete: Completion::None,
        audience: Audience::Member,
    },
    CommandSpec {
        family: "history",
        sub: "",
        args: NO_ARGS,
        options: NO_OPTS,
        binding: Binding::Local,
        output: OutputShape::Lines,
        json: false,
        help_key: "ocsh.cmd.history",
        complete: Completion::None,
        audience: Audience::Member,
    },
    CommandSpec {
        family: "exit",
        sub: "",
        args: NO_ARGS,
        options: NO_OPTS,
        binding: Binding::Local,
        output: OutputShape::Note,
        json: false,
        help_key: "ocsh.cmd.exit",
        complete: Completion::None,
        audience: Audience::Member,
    },
    CommandSpec {
        family: "whoami",
        sub: "",
        args: NO_ARGS,
        options: NO_OPTS,
        binding: Binding::Capability("identity.self.read"),
        output: OutputShape::Facts(&["display_name", "instance", "roles", "context"]),
        json: true,
        help_key: "ocsh.cmd.whoami",
        complete: Completion::None,
        audience: Audience::Member,
    },
    CommandSpec {
        family: "context",
        sub: "show",
        args: NO_ARGS,
        options: NO_OPTS,
        binding: Binding::Capability("research.workspace.list"),
        output: OutputShape::Facts(&["context", "title", "kind"]),
        json: true,
        help_key: "ocsh.cmd.context.show",
        complete: Completion::None,
        audience: Audience::Member,
    },
    CommandSpec {
        family: "context",
        sub: "list",
        args: NO_ARGS,
        options: &[OptSpec { name: "mine", value: None, sensitive: false }],
        binding: Binding::Capability("research.workspace.list"),
        output: OutputShape::Table(&["code", "title", "kind"]),
        json: true,
        help_key: "ocsh.cmd.context.list",
        complete: Completion::None,
        audience: Audience::Member,
    },
    CommandSpec {
        family: "context",
        sub: "use",
        args: &[ArgSpec { name: "target", kind: ValueKind::Id, required: true, rest: false, sensitive: false }],
        options: NO_OPTS,
        binding: Binding::Capability("research.workspace.list"),
        output: OutputShape::Note,
        json: false,
        help_key: "ocsh.cmd.context.use",
        complete: Completion::Contexts,
        audience: Audience::Member,
    },
    CommandSpec {
        family: "tasks",
        sub: "list",
        args: NO_ARGS,
        options: &[OptSpec { name: "open", value: None, sensitive: false }],
        binding: Binding::Capability("collaboration.task.list"),
        output: OutputShape::Table(&["title", "state", "due_on"]),
        json: true,
        help_key: "ocsh.cmd.tasks.list",
        complete: Completion::None,
        audience: Audience::Member,
    },
    CommandSpec {
        family: "nodes",
        sub: "list",
        args: NO_ARGS,
        options: NO_OPTS,
        binding: Binding::Capability("compute.node.list"),
        output: OutputShape::Facts(&["registered", "online"]),
        json: true,
        help_key: "ocsh.cmd.nodes.list",
        complete: Completion::None,
        audience: Audience::Operator,
    },
    CommandSpec {
        family: "nye",
        sub: "ask",
        args: &[ArgSpec { name: "question", kind: ValueKind::Text, required: true, rest: true, sensitive: false }],
        options: NO_OPTS,
        binding: Binding::Nye,
        output: OutputShape::Lines,
        json: false,
        help_key: "ocsh.cmd.nye.ask",
        complete: Completion::None,
        audience: Audience::Member,
    },
];

/// Palavras de shells do anfitrião. Reconhecidas para dizer, com clareza, que o
/// ocsh não é uma delas (código 126) — nunca executadas.
pub const HOST_SHELL_WORDS: &[&str] = &[
    "sudo", "su", "bash", "sh", "zsh", "fish", "ssh", "host", "exec", "eval", "doas",
];

/// Palavras POSIX com um equivalente no Ocinye, para a sugestão «quis dizer».
/// Não são aliases: `ls` continua a ser comando desconhecido.
pub const POSIX_HINTS: &[(&str, &str)] = &[
    ("ls", "files ls"),
    ("cd", "files cd"),
    ("pwd", "files pwd"),
    ("mkdir", "files mkdir"),
    ("mv", "files mv"),
    ("rm", "files trash"),
    ("cat", "files open"),
    ("cls", "clear"),
    ("man", "help"),
];

/// A família com este nome.
#[must_use]
pub fn family(name: &str) -> Option<&'static FamilySpec> {
    FAMILIES.iter().find(|f| f.name == name)
}

/// Os comandos de uma família.
pub fn commands_of(family: &str) -> impl Iterator<Item = &'static CommandSpec> + '_ {
    COMMANDS.iter().filter(move |c| c.family == family)
}

/// O comando `família sub` (sub `""` numa família sem subcomandos).
#[must_use]
pub fn command(family: &str, sub: &str) -> Option<&'static CommandSpec> {
    COMMANDS.iter().find(|c| c.family == family && c.sub == sub)
}

/// Distância de edição, para «quis dizer». Pequena e sem dependências.
#[must_use]
pub fn distance(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.iter().enumerate() {
        let mut cur = vec![i + 1; b.len() + 1];
        for (j, cb) in b.iter().enumerate() {
            let cost = usize::from(ca != cb);
            cur[j + 1] = (prev[j + 1] + 1).min(cur[j] + 1).min(prev[j] + cost);
        }
        prev = cur;
    }
    prev[b.len()]
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn cada_comando_pertence_a_uma_familia_declarada() {
        for c in COMMANDS {
            assert!(family(c.family).is_some(), "{} sem família", c.family);
        }
        for f in FAMILIES {
            assert!(commands_of(f.name).next().is_some(), "família {} sem comandos", f.name);
            if let Some(d) = f.default_sub {
                assert!(command(f.name, d).is_some(), "{}: omissão {d} não existe", f.name);
            }
        }
    }

    #[test]
    fn nao_ha_comandos_repetidos_nem_chaves_de_ajuda_repetidas() {
        let mut vistos = BTreeSet::new();
        let mut chaves = BTreeSet::new();
        for c in COMMANDS {
            assert!(vistos.insert((c.family, c.sub)), "{} {} repetido", c.family, c.sub);
            assert!(chaves.insert(c.help_key), "{} repetida", c.help_key);
        }
    }

    #[test]
    fn so_o_ultimo_argumento_absorve_o_resto() {
        for c in COMMANDS {
            for (i, a) in c.args.iter().enumerate() {
                assert!(!a.rest || i + 1 == c.args.len(), "{} {}: rest não é o último", c.family, c.sub);
            }
        }
    }

    #[test]
    fn nenhuma_palavra_do_anfitriao_e_familia() {
        for w in HOST_SHELL_WORDS {
            assert!(family(w).is_none(), "{w} é família");
        }
        for (w, _) in POSIX_HINTS {
            assert!(family(w).is_none(), "{w} virou alias");
        }
    }

    #[test]
    fn distancia_de_edicao() {
        assert_eq!(distance("taks", "tasks"), 1);
        assert_eq!(distance("", "abc"), 3);
        assert_eq!(distance("nodes", "nodes"), 0);
    }
}
