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
    admin_name: Option<String>,
    admin_email: Option<String>,
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
            "--admin-name" => {
                args.admin_name = Some(value()?);
                iter.next();
            }
            "--admin-email" => {
                args.admin_email = Some(value()?);
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
  --name        \"Nome Completo\" \\
  --email       pessoa@ocinye.com \\
  --admin-name  \"Nome Completo (Admin)\" \\
  --admin-email pessoa.admin@ocinye.com

Cria duas coisas ligadas entre si:

  · a **pessoa institucional** (--name/--email), que é quem responde. Nasce
    sem credencial: quem provisiona a instituição é o administrador, pelo
    Ocinye OS, e não o servidor.

  · a **identidade privilegiada** (--admin-name/--admin-email), que é o que
    executa, com autoridade de plataforma, ligada àquela pessoa.

Imprime uma palavra-passe temporária da identidade privilegiada, uma única
vez. Terá de a substituir no primeiro acesso: não existe palavra-passe de
bootstrap permanente.

Corre uma única vez. Se já existir um administrador utilizável, recusa."
    );
}

/// Slugs que não podem nomear uma instituição a sério.
///
/// # Porque isto recusa em vez de corrigir
///
/// Porque um valor destes não é um engano de escrita: é a configuração a não
/// ter sido posta. Substituí-lo em silêncio por `ocinye` daria uma instalação
/// que arranca bem e cuja variável de ambiente continua errada — e o erro só
/// aparece na segunda instalação, quando as duas escrevem para a mesma
/// organização.
const SLUGS_RECUSADOS: [&str; 6] = ["default", "demo", "test", "example", "sample", "changeme"];

/// Normaliza o slug configurado, ou explica porque não serve.
///
/// Separada de [`run`] para poder ser exercida sem base de dados. A versão
/// anterior tinha a decisão dentro da função assíncrona, e o teste ao lado
/// verificava apenas que a constante continha o que continha — uma tautologia
/// que ficava verde com a guarda inteiramente removida.
///
/// # Errors
///
/// Quando o slug está vazio ou é um valor de exemplo.
fn slug_utilizavel(configurado: &str) -> anyhow::Result<String> {
    let slug = configurado.trim().to_lowercase();
    if slug.is_empty() || SLUGS_RECUSADOS.contains(&slug.as_str()) {
        bail!(
            "OCINYE_ORGANISATION_SLUG está a «{configurado}»: isso não nomeia \
             uma instituição. Defina-o antes de arrancar."
        );
    }
    Ok(slug)
}

/// Run the bootstrap subcommand.
///
/// # Errors
///
/// Returns an error when arguments are missing, the database is unreachable, or
/// a platform administrator already exists.
pub async fn run(argv: &[String]) -> anyhow::Result<()> {
    let args = parse_args(argv)?;

    let (Some(name), Some(email), Some(admin_name), Some(admin_email)) = (
        &args.name,
        &args.email,
        &args.admin_name,
        &args.admin_email,
    ) else {
        print_usage();
        bail!("--name, --email, --admin-name e --admin-email são obrigatórios");
    };

    // Dois endereços distintos, ou não são duas identidades.
    //
    // O mesmo endereço nos dois lados daria uma pessoa e uma identidade
    // privilegiada com a mesma caixa de correio: a reposição de uma chegaria à
    // outra, e a separação que justifica todo este modelo deixaria de existir
    // onde ela mais conta.
    if name.trim().eq_ignore_ascii_case(admin_name.trim())
        || email.trim().eq_ignore_ascii_case(admin_email.trim())
    {
        bail!(
            "a pessoa institucional e a identidade privilegiada têm de ser \
             distinguíveis: use nomes e endereços diferentes"
        );
    }

    let config = CoreConfig::from_env().context("configuração")?;

    // Fail-closed antes de escrever seja o que for.
    let slug = slug_utilizavel(&config.organisation_slug)?;

    let pool = db::connect(&config)
        .await
        .context("ligação à base de dados")?;
    db::migrate(&pool).await.context("migrations")?;

    let ids = CorrelationIds::generate();
    // Idempotente: a organização é adoptada se já existir, e criada se não.
    // Correr o bootstrap outra vez não pode dar uma segunda instituição com o
    // mesmo nome — seria a mesma repartição de autoria, pertenças e histórico
    // que a duplicação de uma pessoa provoca, só que ao nível de tudo.
    let organisation = organisation::bootstrap_organisation(&pool, &slug, &slug, &ids)
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

    let (person, credential) = match identity::bootstrap_privileged_identity(
        &pool,
        &authenticator,
        organisation.id,
        identity::HumanOwner {
            full_name: name.clone(),
            email: email.clone(),
        },
        admin_name,
        admin_email,
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
    println!("  Instituição e administrador criados.");
    println!();
    println!("  Organização          {slug}");
    println!("  Pessoa institucional {name} · {email}");
    println!("    (sem acesso — dê-lho pelo Ocinye OS, em Administração)");
    println!();
    println!("  Identidade privilegiada");
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
    fn as_duas_identidades_sao_lidas_separadamente() {
        let args = parse_args(&argv(&[
            "--name",
            "Fidel Monteiro",
            "--email",
            "fidel@ocinye.com",
            "--admin-name",
            "Fidel Admin",
            "--admin-email",
            "fidel.admin@ocinye.com",
        ]))
        .unwrap();

        assert_eq!(args.name.as_deref(), Some("Fidel Monteiro"));
        assert_eq!(args.email.as_deref(), Some("fidel@ocinye.com"));
        assert_eq!(args.admin_name.as_deref(), Some("Fidel Admin"));
        assert_eq!(args.admin_email.as_deref(), Some("fidel.admin@ocinye.com"));
    }

    /// O uso diz que a pessoa institucional nasce sem acesso.
    ///
    /// Um operador que não leia isto arranca o servidor, tenta entrar com o
    /// endereço da pessoa, falha, e conclui que o bootstrap correu mal. Correu
    /// bem: o servidor arranca o administrador, e é o administrador que arranca
    /// a instituição.
    #[test]
    fn o_uso_explica_que_a_pessoa_nasce_sem_credencial() {
        // `print_usage` escreve para stderr; o texto vive aqui para poder ser
        // exercido. Uma cópia escrita ao lado ficaria verde enquanto o original
        // mudava — foi o que já aconteceu noutro sítio deste repositório.
        let fonte = include_str!("bootstrap.rs");
        let uso = &fonte[fonte.find("fn print_usage").expect("uso")..];
        let uso = &uso[..uso.find("\n}").expect("fim")];
        assert!(uso.contains("Nasce"), "o uso não diz que a pessoa nasce sem credencial");
        assert!(uso.contains("--admin-email"), "o uso não anuncia a identidade privilegiada");
    }

    /// Slugs de configuração por pôr são recusados, e não corrigidos.
    #[test]
    fn um_slug_de_exemplo_nao_nomeia_uma_instituicao() {
        for recusado in ["default", "DEMO", " test ", "", "   ", "changeme"] {
            assert!(
                slug_utilizavel(recusado).is_err(),
                "«{recusado}» arrancou uma instituição"
            );
        }
    }

    #[test]
    fn um_slug_a_serio_passa_e_e_normalizado() {
        assert_eq!(slug_utilizavel(" Ocinye ").unwrap(), "ocinye");
        assert_eq!(slug_utilizavel("banza").unwrap(), "banza");
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
