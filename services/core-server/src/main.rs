//! Ocinye Core Runtime — the HTTP surface of the institutional kernel.
//!
//! This binary is transport, not domain. It parses requests, resolves the
//! acting principal, calls a service in [`ocinye_core`], and renders the result.
//! **No authorization decision is taken here**: handlers stay thin precisely so
//! that a route cannot forget to make one (ADR-0006).

#![forbid(unsafe_code)]

use ocinye_core_server::{bootstrap, continuity, mail_check, provision, routes};

use std::sync::Arc;
use std::time::Duration;

use anyhow::Context;
use ocinye_core::config::CoreConfig;
use ocinye_core::modules::identity::{Authenticator, Throttle};
use ocinye_core::modules::organisation;
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
    // Regista no Ocinye uma caixa que já existe no serviço de correio. Sem
    // isto, ligar uma caixa pressupunha uma caixa que nada criava.
    if argv.first().map(String::as_str) == Some("provision-mailbox") {
        return provision::run(&argv[1..]).await;
    }
    if argv.first().map(String::as_str) == Some("mail-check") {
        return mail_check::run().await;
    }
    // ── Continuidade institucional ──────────────────────────────────────
    //
    // Um servidor é uma instância de execução. Estes três respondem às
    // perguntas que uma migração faz: o que é preciso levar, o que esta
    // instalação contém, e se o que chegou é o mesmo que saiu.
    if argv.first().map(String::as_str) == Some("continuity-inventory") {
        return continuity::inventory();
    }
    if argv.first().map(String::as_str) == Some("snapshot") {
        return continuity::snapshot().await;
    }
    if argv.first().map(String::as_str) == Some("verify-keys") {
        return continuity::verify_keys().await;
    }
    if argv.first().map(String::as_str) == Some("verify-objects") {
        return continuity::verify_objects().await;
    }
    if argv.first().map(String::as_str) == Some("verify-snapshot") {
        return continuity::verify_snapshot().await;
    }
    // Uma chave nova para `OCINYE_MAIL_KEY`.
    //
    // Escreve-a e mais nada: a saída deste comando destina-se a ir directa para
    // o cofre de segredos da instalação, e uma frase à volta seria uma frase que
    // alguém colava com ela.
    if argv.first().map(String::as_str) == Some("mail-key") {
        println!("{}", ocinye_core::password::sealed::SealingKey::generate());
        return Ok(());
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
            per_email: config.auth.throttle_per_email,
            window_minutes: config.auth.throttle_window_minutes,
        },
        config.auth.temporary_credential_hours,
    ));

    // The adapter is chosen once, at startup, from configuration. When mail is
    // not configured the unconfigured adapter takes its place and says so on
    // every call — an interface that shows an empty inbox instead of a reason
    // is the failure mode this avoids (briefing §60).
    let mail_provider = ocinye_core::modules::mail::from_config(&config);

    // Os componentes lêem-se uma vez, no arranque. Um que não esteja construído
    // não impede o Core de subir: a operação que precisar dele recusa com uma
    // razão, que é a diferença entre uma capacidade indisponível e uma
    // instalação partida.
    let capabilities = Arc::new(
        ocinye_core::capabilities::Capabilities::load(&config.capability_components_dir)
            .context("capability runtime")?,
    );

    // Construído antes de a configuração ir para dentro do `Arc`: o registo
    // guarda o transporte e a chave, e não a configuração inteira.
    let mail_registry = Arc::new(ocinye_core::modules::mail::ProviderRegistry::new(
        mail_provider,
        config.mail.clone(),
        config.mail.sealing_key.clone(),
    ));

    // O plano realtime. Nunca falha o arranque: sem Redis, o Ocinye continua
    // inteiro e o que se perde é propagação instantânea, presença e `typing`
    // (ADR-0012 §9).
    let realtime = Arc::new(ocinye_core::realtime::Realtime::connect(&config.redis_url).await);

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
        mail_probe: Arc::clone(&mail_registry)
            as Arc<dyn ocinye_core::modules::mail::provider::CredentialProbe>,
        mail_registry,
        realtime,
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
