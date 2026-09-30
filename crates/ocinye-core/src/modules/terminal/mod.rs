//! O Terminal no Core: executar uma linha do ocsh (ADR-0312).
//!
//! # O que este módulo é
//!
//! A terceira entrada para as mesmas operações (ADR-0307): a linha é lida pelo
//! parser determinístico do ocsh, validada contra o registo de comandos, e cada
//! comando invoca **uma capability** pelo executor agentic — autoridade
//! restabelecida à fonte (ADR-0411), política, esquema, risco, auditoria.
//!
//! # O que não é
//!
//! Um executor próprio. Nada aqui toca num repositório, num serviço de domínio
//! ou num processo: tudo o que tem efeito ou lê dados do Core passa pelo
//! [`executor`](crate::modules::agentic::executor). O que é deste módulo é a
//! tradução — da invocação tipada para o pedido da capability, e do resultado
//! para blocos do Terminal — e o contexto do separador.

use std::cmp::Ordering;
use std::time::Instant;

use ocinye_contracts::agentic::{
    CapabilityId, CapabilityRequest, CapabilityResult, ExecutionStatus,
    ResourceKind as AgenticKind, ResourceRef,
};
use ocinye_contracts::ocsh::registry::{Binding, CommandSpec, OutputShape, COMMANDS};
use ocinye_contracts::ocsh::wire::{Block, ContextView, ExecRequest, ExecResponse, Tone};
use ocinye_contracts::ocsh::{parse, ExitCode, Invocation, ParseError, Parsed, Stage};
use ocinye_domain::{Principal, ResourceContext, ResourceKind};
use serde_json::{json, Value};
use sqlx::PgPool;
use uuid::Uuid;

use crate::capabilities::Capabilities;
use crate::error::CoreResult;
use crate::modules::agentic::{executor, registry, runtime};
use crate::realtime::Realtime;
use ocinye_observability::CorrelationIds;

/// Palavras que querem dizer «o contexto pessoal», nas três línguas.
const PERSONAL: &[&str] = &["personal", "pessoal", "personnel", "~"];

/// Tudo o que uma execução precisa do Core.
pub struct Deps<'a> {
    /// A base.
    pub pool: &'a PgPool,
    /// As capacidades do sistema (IA, correio, …).
    pub capabilities: &'a Capabilities,
    /// O plano de tempo real.
    pub realtime: &'a Realtime,
    /// A correlação do pedido.
    pub ids: &'a CorrelationIds,
}

/// Executa uma linha do ocsh em nome de `principal`.
///
/// # Errors
///
/// Só quando o executor não consegue concluir (por exemplo, a auditoria de uma
/// mutação não se escreve). Recusas, erros de uso e comandos desconhecidos são
/// respostas com código de saída, não erros.
pub async fn execute(
    deps: &Deps<'_>,
    principal: &Principal,
    request: &ExecRequest,
) -> CoreResult<ExecResponse> {
    let start = Instant::now();
    let context = resolve_context(deps.pool, principal, request.context.as_deref()).await;

    let (exit, blocks, capability, context) = match context {
        Err(()) => (
            ExitCode::Denied,
            vec![note(
                Tone::Deny,
                "ocsh.err.context_unreachable",
                vec![],
                vec!["context use personal".into()],
            )],
            None,
            ContextView::personal(),
        ),
        Ok(context) => match parse(&request.line) {
            Ok(Parsed::Empty) => (ExitCode::Ok, vec![], None, context),
            Ok(Parsed::Help(topic)) => (ExitCode::Ok, vec![help(principal, &topic)], None, context),
            Err(error) => (
                error.exit(),
                vec![parse_error(principal, &error)],
                None,
                context,
            ),
            Ok(Parsed::Run(inv)) => run(deps, principal, &inv, context).await?,
        },
    };

    Ok(ExecResponse {
        exit: exit.code(),
        blocks,
        capability,
        context,
        ms: u64::try_from(start.elapsed().as_millis()).unwrap_or(u64::MAX),
    })
}

type Outcome = (ExitCode, Vec<Block>, Option<String>, ContextView);

async fn run(
    deps: &Deps<'_>,
    principal: &Principal,
    inv: &Invocation,
    context: ContextView,
) -> CoreResult<Outcome> {
    let spec = inv.spec;
    match spec.binding {
        Binding::Local => {
            if spec.family == "help" {
                let topic = inv
                    .args
                    .get("topic")
                    .and_then(|v| v.as_text())
                    .unwrap_or("")
                    .to_owned();
                return Ok((ExitCode::Ok, vec![help(principal, &topic)], None, context));
            }
            Ok((
                ExitCode::Ok,
                vec![Block::Client {
                    action: spec.family.to_owned(),
                }],
                None,
                context,
            ))
        }
        // A ponte explícita: o Core só diz que isto é uma pergunta à Nye. A
        // resposta vem do caminho canónico (`/ai/prompt`), que decide de novo.
        Binding::Nye => Ok((
            ExitCode::Ok,
            vec![Block::Ask {
                question: inv
                    .args
                    .get("question")
                    .and_then(|v| v.as_text())
                    .unwrap_or("")
                    .to_owned(),
            }],
            None,
            context,
        )),
        Binding::Capability(id) => {
            let Some((input, resources)) = request_for(inv, &context) else {
                return Ok((
                    ExitCode::Usage,
                    vec![note(
                        Tone::Warn,
                        "ocsh.err.needs_workspace",
                        vec![],
                        vec!["context list".into()],
                    )],
                    None,
                    context,
                ));
            };
            let request = CapabilityRequest {
                capability: CapabilityId::new(id),
                input,
                resources,
                dry_run: false,
            };
            let institution =
                ResourceContext::organisation(ResourceKind::Person, principal.organisation_id);
            let result = executor::execute(
                deps.pool,
                deps.capabilities,
                deps.realtime,
                principal,
                &runtime::main_agent_boundary(),
                None,
                &request,
                &institution,
                false,
                deps.ids,
            )
            .await?;
            let (exit, blocks, context) = render(inv, &result, context);
            Ok((exit, blocks, Some(id.to_owned()), context))
        }
    }
}

/// O pedido da capability para esta invocação, e os recursos a que se aplica.
/// `None` quando o comando precisa de um ambiente e o contexto é pessoal.
fn request_for(inv: &Invocation, context: &ContextView) -> Option<(Value, Vec<ResourceRef>)> {
    let flag = |name: &str| inv.flags.contains(&name);
    let workspace = || {
        context.workspace_id.map(|id| ResourceRef {
            kind: AgenticKind::Workspace,
            id,
            label: None,
        })
    };
    Some(match (inv.spec.family, inv.spec.sub) {
        ("tasks", "list") => (json!({ "open_only": flag("open") }), vec![workspace()?]),
        ("context", "list") => (json!({ "mine": flag("mine") }), vec![]),
        _ => (json!({}), vec![]),
    })
}

/// Do resultado da capability aos blocos do Terminal.
fn render(
    inv: &Invocation,
    result: &CapabilityResult,
    context: ContextView,
) -> (ExitCode, Vec<Block>, ContextView) {
    let refusal =
        |exit, tone, key: &str| (exit, vec![note(tone, key, vec![], vec![])], context.clone());
    match result.status {
        ExecutionStatus::Succeeded => {}
        ExecutionStatus::PermissionDenied => {
            return refusal(ExitCode::Denied, Tone::Deny, "ocsh.denied")
        }
        ExecutionStatus::CapabilityUnavailable => {
            return refusal(ExitCode::Unavailable, Tone::Warn, "ocsh.err.unavailable")
        }
        ExecutionStatus::ValidationFailed => {
            return refusal(ExitCode::Usage, Tone::Err, "ocsh.err.invalid")
        }
        ExecutionStatus::ResourceNotFound => {
            return refusal(ExitCode::Failure, Tone::Err, "ocsh.err.resource_missing")
        }
        ExecutionStatus::ApprovalRequired => {
            return refusal(ExitCode::Denied, Tone::Warn, "ocsh.err.needs_confirmation")
        }
        ExecutionStatus::DryRun | ExecutionStatus::Failed | ExecutionStatus::NotAttempted => {
            return refusal(ExitCode::Failure, Tone::Err, "ocsh.err.failed")
        }
    }
    let output = result.output.clone().unwrap_or(Value::Null);

    // O contexto: `show` e `use` escolhem da lista que o Core acabou de
    // autorizar — nunca de um identificador que o cliente afirme.
    if inv.spec.family == "context" && inv.spec.sub != "list" {
        return context_command(inv, &output, context);
    }

    match inv.spec.output {
        OutputShape::Table(columns) => {
            let rows = table_rows(&output, columns);
            match pipeline(rows, &inv.stages, columns) {
                Ok(Piped::Rows(rows, route)) => {
                    let block = if inv.json {
                        Block::Json {
                            value: rows_as_json(columns, &rows),
                        }
                    } else {
                        Block::Table {
                            columns: columns.iter().map(|c| (*c).to_owned()).collect(),
                            rows,
                            pipeline: route
                                .map(|r| format!("{}.{} → {r}", inv.spec.family, inv.spec.sub)),
                        }
                    };
                    (ExitCode::Ok, vec![block], context)
                }
                Ok(Piped::Count(n)) => (
                    ExitCode::Ok,
                    vec![if inv.json {
                        Block::Json {
                            value: json!({ "count": n }),
                        }
                    } else {
                        Block::Facts {
                            rows: vec![("count".into(), n.to_string())],
                        }
                    }],
                    context,
                ),
                Err(column) => (
                    ExitCode::Usage,
                    vec![note(
                        Tone::Err,
                        "ocsh.err.unknown_column",
                        vec![("column".into(), column)],
                        vec![],
                    )],
                    context,
                ),
            }
        }
        OutputShape::Facts(keys) => {
            let rows: Vec<(String, String)> = keys
                .iter()
                .map(|k| {
                    let value = if *k == "context" {
                        context.code.clone().unwrap_or_else(|| "personal".into())
                    } else {
                        cell(output.get(*k))
                    };
                    ((*k).to_owned(), value)
                })
                .collect();
            let block = if inv.json {
                Block::Json {
                    value: Value::Object(
                        rows.iter()
                            .map(|(k, v)| (k.clone(), Value::String(v.clone())))
                            .collect(),
                    ),
                }
            } else {
                Block::Facts { rows }
            };
            (ExitCode::Ok, vec![block], context)
        }
        OutputShape::Lines | OutputShape::Note => (ExitCode::Ok, vec![], context),
    }
}

fn context_command(
    inv: &Invocation,
    output: &Value,
    context: ContextView,
) -> (ExitCode, Vec<Block>, ContextView) {
    let items = output
        .get("items")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let view_of = |item: &Value| ContextView {
        workspace_id: item
            .get("id")
            .and_then(Value::as_str)
            .and_then(|s| Uuid::parse_str(s).ok()),
        code: item.get("code").and_then(Value::as_str).map(str::to_owned),
        title: item.get("title").and_then(Value::as_str).map(str::to_owned),
    };

    if inv.spec.sub == "use" {
        let target = inv
            .args
            .get("target")
            .and_then(|v| v.as_text())
            .unwrap_or("")
            .trim()
            .to_owned();
        if PERSONAL.contains(&target.to_lowercase().as_str()) {
            return (
                ExitCode::Ok,
                vec![note(Tone::Ok, "ocsh.ok.context_personal", vec![], vec![])],
                ContextView::personal(),
            );
        }
        let wanted = target.to_lowercase();
        let found = items.iter().find(|item| {
            ["id", "code", "title"].iter().any(|k| {
                item.get(*k)
                    .and_then(Value::as_str)
                    .is_some_and(|v| v.to_lowercase() == wanted)
            })
        });
        return match found {
            Some(item) => {
                let view = view_of(item);
                (
                    ExitCode::Ok,
                    vec![note(
                        Tone::Ok,
                        "ocsh.ok.context_workspace",
                        vec![("code".into(), view.code.clone().unwrap_or_default())],
                        vec![],
                    )],
                    view,
                )
            }
            // Não encontrado e não alcançável dizem-se da mesma maneira: não
            // se revela que existe um ambiente onde a pessoa não entra.
            None => (
                ExitCode::Failure,
                vec![note(
                    Tone::Err,
                    "ocsh.err.context_not_found",
                    vec![("target".into(), target)],
                    vec!["context list".into()],
                )],
                context,
            ),
        };
    }

    // `context` / `context show`.
    let rows = vec![
        (
            "context".to_owned(),
            context.code.clone().unwrap_or_else(|| "personal".into()),
        ),
        (
            "title".to_owned(),
            context.title.clone().unwrap_or_else(|| "—".into()),
        ),
        (
            "kind".to_owned(),
            context
                .workspace_id
                .and_then(|id| {
                    items
                        .iter()
                        .find(|i| i.get("id").and_then(Value::as_str) == Some(&id.to_string()))
                })
                .map_or_else(|| "personal".to_owned(), |i| cell(i.get("kind"))),
        ),
    ];
    let block = if inv.json {
        Block::Json {
            value: Value::Object(
                rows.iter()
                    .map(|(k, v)| (k.clone(), Value::String(v.clone())))
                    .collect(),
            ),
        }
    } else {
        Block::Facts { rows }
    };
    (ExitCode::Ok, vec![block], context)
}

/// O contexto do separador, reautorizado a cada comando.
///
/// `Err(())` quando o separador aponta para um ambiente que a pessoa já não
/// alcança (ou nunca alcançou): o Terminal volta ao pessoal e di-lo.
async fn resolve_context(
    pool: &PgPool,
    principal: &Principal,
    raw: Option<&str>,
) -> Result<ContextView, ()> {
    let Some(raw) = raw.map(str::trim).filter(|r| !r.is_empty()) else {
        return Ok(ContextView::personal());
    };
    if PERSONAL.contains(&raw.to_lowercase().as_str()) {
        return Ok(ContextView::personal());
    }
    let id = Uuid::parse_str(raw).map_err(|_| ())?;
    let workspace = crate::modules::research::get_workspace(pool, principal, id)
        .await
        .map_err(|_| ())?;
    Ok(ContextView {
        workspace_id: Some(workspace.id),
        code: Some(workspace.code),
        title: Some(workspace.title),
    })
}

/// Os comandos que esta pessoa vê: os locais, a ponte da Nye, e os que invocam
/// uma capability que ela pode usar. **Só descoberta**: escrever um comando
/// escondido chega ao executor, que o recusa na mesma.
fn visible_to(principal: &Principal) -> Vec<&'static CommandSpec> {
    let available: Vec<String> = registry()
        .available_to(principal, None)
        .into_iter()
        .map(|d| d.id.as_str().to_owned())
        .collect();
    COMMANDS
        .iter()
        .filter(|c| match c.binding {
            Binding::Capability(id) => available.iter().any(|a| a == id),
            Binding::Local | Binding::Nye => true,
        })
        .collect()
}

/// A ajuda, filtrada pelo que esta pessoa pode usar.
///
/// Um comando cuja capability a pessoa não tem não aparece — e se for escrito,
/// o Core recusa-o na mesma. Esconder é cortesia; a recusa é que protege.
fn help(principal: &Principal, topic: &str) -> Block {
    let topic = topic.trim();
    let entries = visible_to(principal)
        .into_iter()
        .filter(|c| {
            topic.is_empty() || topic == c.family || topic == format!("{} {}", c.family, c.sub)
        })
        .map(|c| (usage(c), c.help_key.to_owned()))
        .collect();
    Block::Help {
        topic: topic.to_owned(),
        entries,
    }
}

fn usage(c: &ocinye_contracts::ocsh::registry::CommandSpec) -> String {
    let mut out = c.family.to_owned();
    if !c.sub.is_empty() {
        out.push(' ');
        out.push_str(c.sub);
    }
    for a in c.args {
        out.push_str(&if a.required {
            format!(" <{}>", a.name)
        } else {
            format!(" [{}]", a.name)
        });
    }
    for o in c.options {
        out.push_str(&match o.value {
            None => format!(" [--{}]", o.name),
            Some(_) => format!(" [--{} <v>]", o.name),
        });
    }
    if c.json {
        out.push_str(" [--json]");
    }
    out
}

fn parse_error(principal: &Principal, error: &ParseError) -> Block {
    match error {
        // «Quis dizer» só nomeia um comando que existe e que esta pessoa vê
        // (D008 · T-08): nunca um comando inexistente, nem uma família que a
        // ajuda lhe esconde.
        ParseError::UnknownCommand { word, suggestion } => {
            let visible = visible_to(principal);
            let suggestion = suggestion.iter().filter(|s| {
                let mut w = s.split_whitespace();
                let fam = w.next().unwrap_or_default();
                let sub = w.next();
                visible
                    .iter()
                    .any(|c| c.family == fam && sub.is_none_or(|sub| c.sub == sub))
            });
            note(
                Tone::Err,
                "ocsh.err.not_found",
                vec![("cmd".into(), word.clone())],
                suggestion.cloned().collect(),
            )
        }
        // `sudo` tem uma resposta própria: a autoridade vem das capabilities,
        // e não de um prefixo.
        ParseError::HostShell(word) => {
            let key = if matches!(word.to_lowercase().as_str(), "sudo" | "su" | "doas") {
                "ocsh.sudo.title"
            } else {
                "ocsh.host.title"
            };
            note(
                Tone::Deny,
                key,
                vec![("cmd".into(), word.clone())],
                vec!["help".into()],
            )
        }
        ParseError::Lex(ocinye_contracts::ocsh::lexer::LexError::HostSyntax(op)) => note(
            Tone::Deny,
            "ocsh.err.host_syntax",
            vec![("op".into(), (*op).to_owned())],
            vec![],
        ),
        ParseError::Lex(e) => note(
            Tone::Err,
            "ocsh.err.syntax",
            vec![("detail".into(), e.to_string())],
            vec![],
        ),
        ParseError::UnknownSubcommand { family, word } => note(
            Tone::Err,
            if word.is_empty() {
                "ocsh.err.missing_sub"
            } else {
                "ocsh.err.bad_sub"
            },
            vec![
                ("family".into(), family.clone()),
                ("cmd".into(), word.clone()),
            ],
            vec![format!("help {family}")],
        ),
        ParseError::MissingArgument(name) => note(
            Tone::Err,
            "ocsh.err.missing_arg",
            vec![("name".into(), (*name).to_owned())],
            vec![],
        ),
        ParseError::UnexpectedArgument(w) => note(
            Tone::Err,
            "ocsh.err.unexpected_arg",
            vec![("cmd".into(), w.clone())],
            vec![],
        ),
        ParseError::UnknownOption(o) => note(
            Tone::Err,
            "ocsh.err.bad_opt",
            vec![("opt".into(), o.clone())],
            vec![],
        ),
        ParseError::MissingOptionValue(o) => note(
            Tone::Err,
            "ocsh.err.missing_value",
            vec![("opt".into(), format!("--{o}"))],
            vec![],
        ),
        ParseError::BadValue { name, value } => note(
            Tone::Err,
            "ocsh.err.bad_value",
            vec![
                ("name".into(), (*name).to_owned()),
                ("value".into(), value.clone()),
            ],
            vec![],
        ),
        ParseError::BadStage(s) => note(
            Tone::Err,
            "ocsh.err.pipe",
            vec![("op".into(), s.clone())],
            vec![],
        ),
        ParseError::JsonNotSupported => note(Tone::Err, "ocsh.err.no_json", vec![], vec![]),
        ParseError::NotPipeable => note(Tone::Err, "ocsh.err.not_pipeable", vec![], vec![]),
    }
}

fn note(tone: Tone, key: &str, params: Vec<(String, String)>, suggestions: Vec<String>) -> Block {
    Block::Note {
        tone,
        key: key.to_owned(),
        params,
        suggestions,
    }
}

fn cell(value: Option<&Value>) -> String {
    match value {
        None | Some(Value::Null) => "—".to_owned(),
        Some(Value::String(s)) => s.clone(),
        Some(Value::Array(items)) => items
            .iter()
            .map(|v| cell(Some(v)))
            .collect::<Vec<_>>()
            .join(", "),
        Some(other) => other.to_string(),
    }
}

fn table_rows(output: &Value, columns: &[&str]) -> Vec<Vec<String>> {
    output
        .get("items")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .map(|item| columns.iter().map(|c| cell(item.get(*c))).collect())
                .collect()
        })
        .unwrap_or_default()
}

fn rows_as_json(columns: &[&str], rows: &[Vec<String>]) -> Value {
    Value::Array(
        rows.iter()
            .map(|r| {
                Value::Object(
                    columns
                        .iter()
                        .zip(r)
                        .map(|(c, v)| ((*c).to_owned(), Value::String(v.clone())))
                        .collect(),
                )
            })
            .collect(),
    )
}

enum Piped {
    Rows(Vec<Vec<String>>, Option<String>),
    Count(usize),
}

/// Aplica as operações do pipeline tipado sobre as linhas. `Err(coluna)` quando
/// `sort` nomeia uma coluna que o comando não tem.
fn pipeline(
    mut rows: Vec<Vec<String>>,
    stages: &[Stage],
    columns: &[&str],
) -> Result<Piped, String> {
    let mut route: Vec<String> = Vec::new();
    for stage in stages {
        match stage {
            Stage::Filter(text) => {
                let wanted = text.to_lowercase();
                rows.retain(|r| r.iter().any(|c| c.to_lowercase().contains(&wanted)));
                route.push(format!("filter({text})"));
            }
            Stage::Sort { column, desc } => {
                let idx = columns
                    .iter()
                    .position(|c| c == column)
                    .ok_or_else(|| column.clone())?;
                rows.sort_by(|a, b| {
                    let o = compare(&a[idx], &b[idx]);
                    if *desc {
                        o.reverse()
                    } else {
                        o
                    }
                });
                route.push(format!(
                    "sort({column}{})",
                    if *desc { ", desc" } else { "" }
                ));
            }
            Stage::Head(n) => {
                rows.truncate(usize::try_from(*n).unwrap_or(usize::MAX));
                route.push(format!("head({n})"));
            }
            Stage::Count => return Ok(Piped::Count(rows.len())),
            Stage::ExportJson => route.push("export(json)".into()),
        }
    }
    Ok(Piped::Rows(
        rows,
        (!route.is_empty()).then(|| route.join(" → ")),
    ))
}

/// Ordena números como números e o resto como texto.
fn compare(a: &str, b: &str) -> Ordering {
    match (a.parse::<f64>(), b.parse::<f64>()) {
        (Ok(x), Ok(y)) => x.partial_cmp(&y).unwrap_or(Ordering::Equal),
        _ => a.to_lowercase().cmp(&b.to_lowercase()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Nenhum comando morto: cada comando ligado a uma capability aponta para
    /// uma que existe no registo agentic.
    #[test]
    fn cada_comando_liga_a_uma_capability_que_existe() {
        for c in COMMANDS {
            if let Binding::Capability(id) = c.binding {
                assert!(
                    registry().get(&CapabilityId::new(id)).is_some(),
                    "o comando `{} {}` aponta para `{id}`, que não existe",
                    c.family,
                    c.sub
                );
            }
        }
    }

    #[test]
    fn pipeline_filtra_ordena_corta_e_conta() {
        let rows = vec![
            vec!["b".to_owned(), "10".to_owned()],
            vec!["a".to_owned(), "9".to_owned()],
            vec!["c".to_owned(), "100".to_owned()],
        ];
        let cols = ["name", "n"];
        let Ok(Piped::Rows(out, route)) = pipeline(
            rows.clone(),
            &[
                Stage::Sort {
                    column: "n".into(),
                    desc: false,
                },
                Stage::Head(2),
            ],
            &cols,
        ) else {
            panic!()
        };
        assert_eq!(
            out,
            vec![
                vec!["a".to_owned(), "9".to_owned()],
                vec!["b".to_owned(), "10".to_owned()]
            ],
            "numérico"
        );
        assert_eq!(route.as_deref(), Some("sort(n) → head(2)"));
        assert!(matches!(
            pipeline(
                rows.clone(),
                &[Stage::Filter("A".into()), Stage::Count],
                &cols
            ),
            Ok(Piped::Count(1))
        ));
        assert!(
            matches!(pipeline(rows, &[Stage::Sort { column: "x".into(), desc: true }], &cols), Err(c) if c == "x")
        );
    }

    #[test]
    fn celulas_de_dados_sao_texto() {
        assert_eq!(
            cell(Some(&json!("<script>x</script>"))),
            "<script>x</script>"
        );
        assert_eq!(cell(Some(&json!(null))), "—");
        assert_eq!(cell(Some(&json!(["a", "b"]))), "a, b");
    }
}
