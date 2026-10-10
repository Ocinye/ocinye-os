//! Os subcomandos que o executor da instalação (`ocinye-bootstrap`) chama no
//! servidor (D011, ADR-0022): `endpoint-seed` e as quatro verificações.
//!
//! Como o `bootstrap-admin`, exigem poder correr um processo no anfitrião com a
//! configuração do Core no ambiente — a mesma autoridade de quem escrevesse na
//! base à mão — e não acrescentam superfície nenhuma à rede.
//!
//! Cada um escreve **uma** linha JSON no stdout e nada mais: é um contrato
//! lido por uma máquina, e uma frase à volta seria uma frase a analisar.
//! Recusas saem com código 2 e `{"refused":"<CÓDIGO>"}`; erros de execução
//! (base inacessível, configuração) com código 1 no stderr.

use anyhow::{bail, Context};
use ocinye_core::config::CoreConfig;
use ocinye_core::db;
use ocinye_core::modules::organisation::installation;
use ocinye_observability::CorrelationIds;
use serde::Serialize;

fn line<T: Serialize>(value: &T) -> anyhow::Result<()> {
    println!("{}", serde_json::to_string(value).context("json")?);
    Ok(())
}

/// Lê `--nome valor` de um argv fechado: um argumento desconhecido é recusado,
/// e nunca ignorado.
fn flags(argv: &[String], allowed: &[&str]) -> anyhow::Result<Vec<(String, String)>> {
    let mut out = Vec::new();
    let mut iter = argv.iter();
    while let Some(flag) = iter.next() {
        if !allowed.contains(&flag.as_str()) {
            bail!("argumento desconhecido: {flag}");
        }
        let value = iter
            .next()
            .with_context(|| format!("{flag} precisa de um valor"))?;
        if out.iter().any(|(f, _)| f == flag) {
            bail!("{flag} repetido");
        }
        out.push((flag.clone(), value.clone()));
    }
    Ok(out)
}

fn get<'a>(flags: &'a [(String, String)], name: &str) -> Option<&'a str> {
    flags
        .iter()
        .find(|(f, _)| f == name)
        .map(|(_, v)| v.as_str())
}

async fn pool() -> anyhow::Result<(CoreConfig, sqlx::PgPool)> {
    let config = CoreConfig::from_env().context("configuração")?;
    let pool = db::connect(&config)
        .await
        .context("ligação à base de dados")?;
    Ok((config, pool))
}

/// `endpoint-seed --installation-id ID --host H --distribution D`.
///
/// # Errors
///
/// Argument, configuration and database errors. Refusals exit with code 2.
pub async fn endpoint_seed(argv: &[String]) -> anyhow::Result<()> {
    let flags = flags(argv, &["--installation-id", "--host", "--distribution"])?;
    let (Some(installation_id), Some(host), Some(distribution)) = (
        get(&flags, "--installation-id"),
        get(&flags, "--host"),
        get(&flags, "--distribution"),
    ) else {
        bail!("--installation-id, --host e --distribution são obrigatórios");
    };
    let (config, pool) = pool().await?;
    let ids = CorrelationIds::generate();
    match installation::seed_bound_endpoint(
        &pool,
        installation_id,
        host,
        distribution,
        config.endpoint_seed_url.as_deref(),
        &ids,
    )
    .await
    .context("endpoint-seed")?
    {
        Ok(seeded) => line(&seeded),
        Err(refusal) => {
            line(&serde_json::json!({ "refused": refusal.code() }))?;
            std::process::exit(2);
        }
    }
}

fn require_json(argv: &[String]) -> anyhow::Result<()> {
    if argv != ["--json"] {
        bail!("use --json: a saída é um contrato lido por uma máquina");
    }
    Ok(())
}

/// `verify-schema --json`.
///
/// # Errors
///
/// Configuration and database errors.
pub async fn verify_schema(argv: &[String]) -> anyhow::Result<()> {
    require_json(argv)?;
    let (_, pool) = pool().await?;
    line(&installation::verify_schema(&pool).await?)
}

/// `verify-instance --json`. Sem Instância, `null` e código 2.
///
/// # Errors
///
/// Configuration and database errors.
pub async fn verify_instance(argv: &[String]) -> anyhow::Result<()> {
    require_json(argv)?;
    let (_, pool) = pool().await?;
    match installation::verify_instance(&pool).await? {
        Some(state) => line(&state),
        None => {
            line(&serde_json::json!({ "refused": "NO_INSTANCE" }))?;
            std::process::exit(2);
        }
    }
}

/// `verify-endpoints --json`.
///
/// # Errors
///
/// Configuration and database errors.
pub async fn verify_endpoints(argv: &[String]) -> anyhow::Result<()> {
    require_json(argv)?;
    let (_, pool) = pool().await?;
    line(&installation::verify_endpoints(&pool).await?)
}

/// `verify-admin-bootstrap --json`.
///
/// # Errors
///
/// Configuration and database errors.
pub async fn verify_admin_bootstrap(argv: &[String]) -> anyhow::Result<()> {
    require_json(argv)?;
    let (_, pool) = pool().await?;
    line(&installation::verify_admin_bootstrap(&pool).await?)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn argv(values: &[&str]) -> Vec<String> {
        values.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn um_argumento_desconhecido_e_recusado() {
        assert!(flags(&argv(&["--sql", "x"]), &["--host"]).is_err());
        assert!(flags(&argv(&["--host"]), &["--host"]).is_err());
        assert!(flags(&argv(&["--host", "a", "--host", "b"]), &["--host"]).is_err());
        let ok = flags(&argv(&["--host", "a.b"]), &["--host"]).unwrap();
        assert_eq!(get(&ok, "--host"), Some("a.b"));
    }

    /// O que o Core escreve é o que o executor da instalação lê: cada tipo do
    /// Core, serializado, lê-se no tipo do contrato (`deny_unknown_fields`).
    #[test]
    fn a_saida_do_core_le_se_no_contrato_do_installer() {
        use ocinye_core::modules::organisation::installation as i;
        use ocinye_installer_contracts::core_output as c;
        let seeded = i::Seeded {
            result: "created",
            endpoint_id: uuid::Uuid::nil(),
            host: "r.instalacao.test".into(),
            distribution: "research",
            state: "active",
        };
        let back: c::EndpointSeedOutput =
            serde_json::from_str(&serde_json::to_string(&seeded).unwrap()).unwrap();
        assert!(matches!(back, c::EndpointSeedOutput::Seeded { .. }));
        for r in [
            i::SeedRefusal::NotANewInstance,
            i::SeedRefusal::SessionsExist,
            i::SeedRefusal::HostInvalid,
            i::SeedRefusal::HostIsCanonical,
            i::SeedRefusal::HostTaken,
            i::SeedRefusal::DistributionNotEnabled,
            i::SeedRefusal::DistributionUnknown,
        ] {
            let line = serde_json::json!({ "refused": r.code() }).to_string();
            assert!(
                matches!(
                    serde_json::from_str::<c::EndpointSeedOutput>(&line).unwrap(),
                    c::EndpointSeedOutput::Refused { .. }
                ),
                "{line}"
            );
        }
        let schema = i::SchemaState {
            latest: "0064".into(),
            count: 64,
            pending: 0,
        };
        let _: c::VerifySchema =
            serde_json::from_value(serde_json::to_value(&schema).unwrap()).unwrap();
        let inst = i::InstanceState {
            name: "N".into(),
            slug: "n".into(),
            distributions: vec!["business".into()],
            applications: i::ApplicationsState {
                registered: 28,
                active: 20,
                essential_inactive: 0,
            },
        };
        let _: c::VerifyInstance =
            serde_json::from_value(serde_json::to_value(&inst).unwrap()).unwrap();
        let ep = i::EndpointState {
            host: "os.instalacao.test".into(),
            canonical: true,
            distribution: None,
            state: "active".into(),
        };
        let _: Vec<c::VerifyEndpoint> =
            serde_json::from_value(serde_json::to_value(vec![ep]).unwrap()).unwrap();
        let adm = i::AdminBootstrapState {
            privileged_identity_exists: true,
            temporary_credential_pending: true,
        };
        let _: c::VerifyAdminBootstrap =
            serde_json::from_value(serde_json::to_value(adm).unwrap()).unwrap();
    }

    #[test]
    fn a_verificacao_so_fala_json() {
        assert!(require_json(&argv(&["--json"])).is_ok());
        assert!(require_json(&argv(&[])).is_err());
        assert!(require_json(&argv(&["--json", "--fix"])).is_err());
    }
}
