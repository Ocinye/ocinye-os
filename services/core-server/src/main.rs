//! Ocinye Core Runtime — the HTTP surface of the institutional kernel.
//!
//! This binary is transport, not domain. It parses requests, resolves the
//! acting principal, calls a service in [`ocinye_core`], and renders the result.
//! **No authorization decision is taken here**: handlers stay thin precisely so
//! that a route cannot forget to make one (ADR-0006).

#![forbid(unsafe_code)]

use ocinye_core_server::{bootstrap, mail_check, routes};

use std::sync::Arc;
use std::time::Duration;

use anyhow::Context;
use ocinye_core::config::CoreConfig;
use ocinye_core::modules::identity::{Authenticator, Throttle};
use ocinye_core::modules::mail::imap_smtp::{ImapSmtpConfig, ImapSmtpProvider};
use ocinye_core::modules::mail::provider::UnconfiguredProvider;
use ocinye_core::modules::mail::MailProvider;
use ocinye_core::modules::organisation;
use ocinye_core::password::Secret;
use ocinye_core::password::{Hasher, HashingParams};
use ocinye_core::storage::ObjectStore;
use ocinye_core::{authn::TokenVerifier, db};
use ocinye_observability::{CorrelationIds, LogFormat};
use tokio::net::TcpListener;
use tokio::signal;

use ocinye_core_server::state::AppState;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // One binary, two entry points. The bootstrap path deliberately shares this
    // process rather than being a second crate: it must load exactly the same
    // configuration and hashing parameters the server will use, or the
    // credential it writes would be hashed differently from the one verified.
    let argv: Vec<String> = std::env::args().skip(1).collect();
    if argv.first().map(String::as_str) == Some("bootstrap-admin") {
        return bootstrap::run(&argv[1..]).await;
    }
    // Answers "do these credentials work?" without starting the Core, and
    // without printing anything that would be unsafe to paste into a ticket.
    if argv.first().map(String::as_str) == Some("mail-check") {
        return mail_check::run().await;
    }

    let config = CoreConfig::from_env().context("configuration")?;
    ocinye_observability::init(
        "ocinye-core",
        &config.log_level,
        LogFormat::parse(&config.log_format),
    );

    tracing::info!(
        environment = config.environment.as_str(),
        organisation = config.organisation_slug,
        "starting Ocinye Core"
    );

    let pool = db::connect(&config).await.context("database connection")?;
    db::migrate(&pool).await.context("migrations")?;

    let ids = CorrelationIds::generate();
    let organisation = organisation::bootstrap_organisation(
        &pool,
        &config.organisation_slug,
        &config.organisation_slug,
        &ids,
    )
    .await
    .context("organisation bootstrap")?;

    // Object storage is optional at startup. Its absence is reported through
    // the health endpoint rather than preventing the Core from running.
    let store = ObjectStore::new(config.storage.clone());
    if store.is_none() {
        tracing::warn!("object storage is not configured; uploads and downloads are unavailable");
    } else {
        ensure_default_backend(&pool, &config)
            .await
            .context("storage backend registration")?;
    }

    let verifier = TokenVerifier::new(config.oidc.clone()).context("token verifier")?;
    if !verifier.is_configured() {
        tracing::warn!(
            "no identity provider is configured; every authenticated request will be refused"
        );
    }

    let bind_address = config.bind_address.clone();
    // Built once: the Argon2 parameters were already validated at
    // configuration time, so a Core that reaches this point can hash safely.
    let authenticator = Arc::new(Authenticator::new(
        Hasher::new(HashingParams {
            memory_kib: config.auth.argon2_memory_kib,
            iterations: config.auth.argon2_iterations,
            parallelism: config.auth.argon2_parallelism,
        }),
        Throttle {
            per_ip: config.auth.throttle_per_ip,
            per_username: config.auth.throttle_per_username,
            window_minutes: config.auth.throttle_window_minutes,
        },
        config.auth.temporary_credential_hours,
    ));

    // The adapter is chosen once, at startup, from configuration. When mail is
    // not configured the unconfigured adapter takes its place and says so on
    // every call — an interface that shows an empty inbox instead of a reason
    // is the failure mode this avoids (briefing §60).
    let mail_provider = build_mail_provider(&config);

    // Os componentes lêem-se uma vez, no arranque. Um que não esteja construído
    // não impede o Core de subir: a operação que precisar dele recusa com uma
    // razão, que é a diferença entre uma capacidade indisponível e uma
    // instalação partida.
    let capabilities = Arc::new(
        ocinye_core::capabilities::Capabilities::load(&config.capability_components_dir)
            .context("capability runtime")?,
    );

    let state = AppState {
        pool,
        config: Arc::new(config),
        verifier,
        authenticator,
        store: store.map(Arc::new),
        // No Ocinye node is enrolled, so nothing serves inference. When one is,
        // an adapter replaces this and **nothing above it changes**
        // (ADR-0002, ADR-0301).
        inference: Arc::new(ocinye_core::modules::intelligence::NoProvider),
        mail_provider,
        capabilities,
        organisation_id: organisation.id,
    };

    let app = routes::router(state);
    let listener = TcpListener::bind(&bind_address).await.context("bind")?;
    tracing::info!(address = %bind_address, "Ocinye Core listening");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .context("server")?;

    tracing::info!("Ocinye Core stopped");
    Ok(())
}

/// Register the configured storage backend if it is not already known.
///
/// Residency comes from configuration and defaults to `UNDECLARED`: the system
/// never claims data resides in Ocinye infrastructure that does not exist
/// (ADR-0201).
async fn ensure_default_backend(
    pool: &sqlx::PgPool,
    config: &CoreConfig,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO storage_backends
             (code, kind, display_name, location_label, region, bucket, residency, is_default)
         VALUES ($1, 's3_compatible', $1, $2, $3, $4, $5, TRUE)
         ON CONFLICT (code) DO UPDATE
            SET location_label = EXCLUDED.location_label,
                region = EXCLUDED.region,
                bucket = EXCLUDED.bucket,
                residency = EXCLUDED.residency,
                updated_at = now()",
    )
    .bind(&config.storage.backend_code)
    .bind(&config.storage.location_label)
    .bind(&config.storage.region)
    .bind(&config.storage.bucket)
    .bind(config.storage.residency.as_str())
    .execute(pool)
    .await?;
    Ok(())
}

/// Wait for a termination signal so in-flight requests can finish.
/// Choose the mail adapter from configuration.
///
/// Never fails the boot. A mail service that cannot be reached must not stop
/// the Ocinye Core from starting: research, knowledge, identity and governance
/// have nothing to do with email, and taking the whole institution offline
/// because a mail host is down would be a self-inflicted outage.
fn build_mail_provider(config: &CoreConfig) -> Arc<dyn MailProvider> {
    if !config.mail.is_configured() {
        tracing::info!("Ocinye Mail is not configured on this deployment");
        return Arc::new(UnconfiguredProvider);
    }

    let settings = ImapSmtpConfig {
        imap_host: config.mail.imap_host.clone(),
        imap_port: config.mail.imap_port,
        imap_security: config.mail.imap_security,
        smtp_host: config.mail.smtp_host.clone(),
        smtp_port: config.mail.smtp_port,
        smtp_security: config.mail.smtp_security,
        username: config.mail.username.clone(),
        password: Secret::new(config.mail.password.clone()),
    };

    match ImapSmtpProvider::new(settings) {
        Ok(provider) => {
            // Hosts and ports only. The username is an address and the password
            // is never anywhere near a log line (briefing §57).
            tracing::info!(
                imap = %config.mail.imap_host,
                imap_port = config.mail.imap_port,
                imap_security = config.mail.imap_security.as_str(),
                smtp = %config.mail.smtp_host,
                smtp_port = config.mail.smtp_port,
                smtp_security = config.mail.smtp_security.as_str(),
                "Ocinye Mail adapter ready"
            );
            Arc::new(provider)
        }
        Err(error) => {
            tracing::error!(
                cause = %error,
                "Ocinye Mail adapter could not be built; mail will report as unavailable"
            );
            Arc::new(UnconfiguredProvider)
        }
    }
}

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c().await.expect("install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => tracing::info!("received Ctrl+C"),
        () = terminate => tracing::info!("received SIGTERM"),
    }

    // A short grace period: long enough for in-flight requests, short enough
    // that a deploy is not held up by a stuck connection.
    tokio::time::sleep(Duration::from_millis(250)).await;
}
