//! Provisiona a caixa pessoal de um membro.
//!
//! Corre como `ocinye-core-server provision-mailbox --owner … --address …`.
//!
//! # Porque um subcomando e não um ecrã
//!
//! Pela mesma razão que o `bootstrap-admin`: quem pode correr um processo nesta
//! máquina, com as credenciais da base já no ambiente, tem a mesma autoridade
//! que teria para escrever a linha à mão. Não acrescenta superfície nenhuma.
//!
//! Criar a caixa **no serviço de correio** é outro acto, feito no painel do
//! fornecedor. Este comando regista no Ocinye uma caixa que já existe lá fora —
//! e não a inventa. É por isso que imprime o que fez, para se poder confrontar.

use anyhow::{bail, Context};
use ocinye_core::config::CoreConfig;
use ocinye_core::db;
use ocinye_observability::CorrelationIds;

#[derive(Debug, Default)]
struct Args {
    owner: Option<String>,
    address: Option<String>,
    name: Option<String>,
}

fn parse_args(argv: &[String]) -> anyhow::Result<Args> {
    let mut args = Args::default();
    let mut iter = argv.iter();

    while let Some(flag) = iter.next() {
        let value = || {
            iter.clone()
                .next()
                .cloned()
                .with_context(|| format!("{flag} needs a value"))
        };
        match flag.as_str() {
            "--owner" => {
                args.owner = Some(value()?);
                iter.next();
            }
            "--address" => {
                args.address = Some(value()?);
                iter.next();
            }
            "--name" => {
                args.name = Some(value()?);
                iter.next();
            }
            "--help" | "-h" => {
                print_usage();
                std::process::exit(0);
            }
            other => bail!("unknown argument: {other}"),
        }
    }
    Ok(args)
}

fn print_usage() {
    eprintln!(
        "Uso: ocinye-core-server provision-mailbox \\
  --owner pessoa@ocinye.com \\
  --address pessoa@ocinye.com \\
  [--name \"Nome a mostrar\"]

Regista no Ocinye a caixa pessoal de um membro. A caixa tem de existir no
serviço de correio: este comando não a cria lá.

Recusa um endereço fora dos domínios da instituição, uma pessoa desactivada,
e uma segunda caixa pessoal para quem já tem uma.

A senha da caixa não se passa aqui. Quem a detém liga-a no Workspace, e ela
fica cifrada com a chave da instalação (ADR-0409)."
    );
}

/// Run the provisioning subcommand.
///
/// # Errors
///
/// Returns an error when arguments are missing, the database is unreachable, or
/// the Core refuses to provision the mailbox.
pub async fn run(argv: &[String]) -> anyhow::Result<()> {
    let args = parse_args(argv)?;

    let (Some(owner), Some(address)) = (&args.owner, &args.address) else {
        print_usage();
        bail!("--owner e --address são obrigatórios");
    };

    let config = CoreConfig::from_env().context("configuração")?;
    let pool = db::connect(&config)
        .await
        .context("ligação à base de dados")?;
    db::migrate(&pool).await.context("migrations")?;

    let id = ocinye_core::modules::mail::provision_personal_mailbox(
        &pool,
        &config.mail.institutional_domains,
        owner,
        address,
        args.name.as_deref(),
        &CorrelationIds::generate(),
    )
    .await
    .context("provisionar a caixa")?;

    println!("Caixa registada: {address} → {owner}");
    println!("  identificador: {id}");
    println!();
    println!("Falta a senha. Quem detém a caixa entra no Workspace, vai a");
    println!("Correio → Definições, e liga-a com as credenciais do serviço.");

    Ok(())
}
