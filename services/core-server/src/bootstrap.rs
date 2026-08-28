//! Bootstrap of the first platform administrator.
//!
//! Run as `ocinye-core-server bootstrap-admin --name … --username … --email …`.
//!
//! # Why a subcommand and not an endpoint
//!
//! An HTTP bootstrap endpoint is reachable by anyone who can reach the service,
//! and is only closed by application logic. A subcommand requires the ability to
//! execute a process on the host with the database credentials already in the
//! environment — which is the same authority needed to write the row by hand.
//! It adds no attack surface at all (briefing §12).
//!
//! # One shot
//!
//! Refuses when a usable platform administrator already exists, checked twice:
//! once before the transaction and once inside it, so two concurrent runs
//! cannot both succeed. There is no override flag. Recovering from the loss of
//! every administrator is a runbook, not a command-line switch — see
//! `docs/runbooks/recover-administrative-access.md`.

use std::sync::Arc;

use anyhow::{bail, Context};
use ocinye_core::config::CoreConfig;
use ocinye_core::modules::identity::{self, Authenticator, Throttle};
use ocinye_core::modules::organisation;
use ocinye_core::password::{Hasher, HashingParams};
use ocinye_core::{db, CoreError};
use ocinye_observability::CorrelationIds;

/// Arguments accepted by the subcommand.
#[derive(Debug, Default)]
struct Args {
    name: Option<String>,
    email: Option<String>,
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
            "--name" => {
                args.name = Some(value()?);
                iter.next();
            }
            "--email" => {
                args.email = Some(value()?);
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
        "Uso: ocinye-core-server bootstrap-admin \\
  --name \"Nome Completo\" \\
  --email pessoa@ocinye.com

Cria o primeiro PlatformAdmin da instalação e imprime uma palavra-passe
temporária, uma única vez. O administrador terá de a substituir no primeiro
acesso: não existe palavra-passe de bootstrap permanente.

Corre uma única vez. Se já existir um administrador utilizável, recusa."
    );
}

/// Run the bootstrap subcommand.
///
/// # Errors
///
/// Returns an error when arguments are missing, the database is unreachable, or
/// a platform administrator already exists.
pub async fn run(argv: &[String]) -> anyhow::Result<()> {
    let args = parse_args(argv)?;

    let (Some(name), Some(email)) = (&args.name, &args.email) else {
        print_usage();
        bail!("--name e --email são obrigatórios");
    };

    let config = CoreConfig::from_env().context("configuração")?;
    let pool = db::connect(&config)
        .await
        .context("ligação à base de dados")?;
    db::migrate(&pool).await.context("migrations")?;

    let ids = CorrelationIds::generate();
    let organisation = organisation::bootstrap_organisation(
        &pool,
        &config.organisation_slug,
        &config.organisation_slug,
        &ids,
    )
    .await
    .context("organização")?;

    let authenticator = Arc::new(Authenticator::new(
        Hasher::new(HashingParams {
            memory_kib: config.auth.argon2_memory_kib,
            iterations: config.auth.argon2_iterations,
            parallelism: config.auth.argon2_parallelism,
        }),
        Throttle {
            per_ip: config.auth.throttle_per_ip,
            per_email: config.auth.throttle_per_email,
            window_minutes: config.auth.throttle_window_minutes,
        },
        config.auth.temporary_credential_hours,
    ));

    let (person, credential) = match identity::bootstrap_platform_admin(
        &pool,
        &authenticator,
        organisation.id,
        name,
        email,
        &ids,
    )
    .await
    {
        Ok(result) => result,
        Err(CoreError::Conflict(message)) => {
            eprintln!("Recusado: {message}");
            eprintln!(
                "\nSe perdeu o acesso administrativo, siga \
                 docs/runbooks/recover-administrative-access.md."
            );
            std::process::exit(1);
        }
        Err(error) => return Err(error.into()),
    };

    // Printed to stdout, once. Nothing else on this path writes it anywhere:
    // not the log, not the database, not the audit trail.
    println!();
    println!("  Administrador principal criado.");
    println!();
    println!("  Nome                 {}", person.full_name);
    println!("  Utilizador           {}", credential.email);
    println!("  Palavra-passe        {}", credential.secret.expose());
    println!(
        "  Válida até           {}",
        credential.expires_at.format("%Y-%m-%d %H:%M UTC")
    );
    println!();
    println!("  Esta palavra-passe é apresentada uma única vez e é temporária.");
    println!("  No primeiro acesso terá de definir a sua palavra-passe definitiva");
    println!("  antes de poder utilizar o Ocinye Workspace.");
    println!();

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn argv(values: &[&str]) -> Vec<String> {
        values.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn the_three_required_arguments_are_parsed() {
        let args = parse_args(&argv(&[
            "--name",
            "Filipe Monteiro",
            "--email",
            "filipe@ocinye.com",
        ]))
        .unwrap();

        assert_eq!(args.name.as_deref(), Some("Filipe Monteiro"));
        assert_eq!(args.email.as_deref(), Some("filipe@ocinye.com"));
    }

    #[test]
    fn an_unknown_argument_is_refused_rather_than_ignored() {
        // Silently ignoring `--password` would be the worst possible outcome:
        // the operator would believe they had set one.
        assert!(parse_args(&argv(&["--password", "hunter2"])).is_err());
        assert!(parse_args(&argv(&["--force"])).is_err());
    }

    #[test]
    fn a_flag_without_a_value_is_refused() {
        assert!(parse_args(&argv(&["--name"])).is_err());
    }

    #[test]
    fn there_is_no_way_to_supply_a_password() {
        // The whole point: the operator cannot choose the credential, so there
        // is no permanent bootstrap password to leak or to forget to change.
        let usage_accepts_password = ["--password", "--pass", "--secret", "--credential"]
            .iter()
            .any(|flag| parse_args(&argv(&[flag, "x"])).is_ok());
        assert!(!usage_accepts_password);
    }
}
