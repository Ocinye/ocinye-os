//! Ocinye Worker Runtime.
//!
//! Drains the transactional outbox and runs work that has no business blocking
//! a request: propagating domain events, refreshing derived state, keeping the
//! Intelligence Plane's view of availability honest, and delivering the
//! reminders somebody set for themselves.
//!
//! # Why polling, and why that is enough
//!
//! Events are durable in PostgreSQL (ADR-0010), so the only cost of polling is
//! latency, not loss. `FOR UPDATE SKIP LOCKED` lets several workers run
//! concurrently without any coordination beyond the database.

#![forbid(unsafe_code)]

mod handlers;
mod outbox;
mod reminders;

use std::time::Duration;

use anyhow::Context;
use ocinye_core::config::CoreConfig;
use ocinye_core::db;
use ocinye_observability::LogFormat;
use tokio::signal;

/// How often the outbox is polled when it was empty last time.
const IDLE_POLL_INTERVAL: Duration = Duration::from_secs(2);
/// How often derived state is refreshed.
const MAINTENANCE_INTERVAL: Duration = Duration::from_secs(30);
/// Events claimed per pass.
const BATCH_SIZE: i64 = 50;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = CoreConfig::from_env().context("configuration")?;
    ocinye_observability::init(
        "ocinye-worker",
        &config.log_level,
        LogFormat::parse(&config.log_format),
    );

    let pool = db::connect(&config).await.context("database connection")?;
    tracing::info!("Ocinye Worker started");

    // O armazenamento, quando a instalação o tem. `None` é um estado legítimo
    // — uma instalação sem object storage corre —, e o handler que precisa de
    // bytes falha o evento em vez de o dar por feito: o evento sobrevive, e é
    // processado no dia em que houver armazenamento.
    let store = ocinye_core::storage::ObjectStore::new(config.storage.clone());
    if store.is_none() {
        tracing::warn!(
            "no object store configured; content extraction will retry until one exists"
        );
    }

    let offline_after = i64::try_from(config.compute.node_offline_after.as_secs()).unwrap_or(120);

    let mut maintenance = tokio::time::interval(MAINTENANCE_INTERVAL);
    maintenance.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    // `Skip` e não `Burst`: se o processo esteve parado uma hora, o que interessa
    // é entregar o que está por entregar **agora**, e não correr cento e vinte
    // passagens seguidas a recuperar batidas perdidas.
    let mut lembretes = tokio::time::interval(reminders::POLL_INTERVAL);
    lembretes.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    // O correio novo deixa de esperar que alguém carregue em sincronizar.
    //
    // O adaptador vem do mesmo construtor que o Core usa: uma instalação sem
    // correio configurado recebe o fornecedor que recusa tudo, e a passagem
    // regista a razão em cada caixa em vez de as deixar vazias sem explicação.
    let correio = ocinye_core::modules::mail::from_config(&config);
    let mut ingestao =
        tokio::time::interval(ocinye_core::modules::mail::service::INGESTION_INTERVAL);
    ingestao.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    // O sinal de paragem é armado **uma vez**, fora do ciclo.
    //
    // # Porque isto era um defeito e não um detalhe
    //
    // Estava dentro do `select!`, e um `select!` dentro de um ciclo constrói os
    // seus futuros de novo a cada iteração: o handler de `SIGTERM` era instalado,
    // descartado e reinstalado a cada passagem. Um sinal que chegasse enquanto
    // outro ramo corria não encontrava ninguém à escuta e perdia-se — o worker
    // ignorava `SIGTERM` e só morria com `SIGKILL`, que é o que aconteceu todas
    // as vezes que reiniciei a stack.
    //
    // Um sinal segurado numa variável fica armado entre iterações. `&mut` no
    // `select!` porque o futuro é consumido a cada tentativa e tem de sobreviver
    // à seguinte.
    let mut paragem = std::pin::pin!(shutdown_signal());

    loop {
        tokio::select! {
            () = &mut paragem => {
                tracing::info!("Ocinye Worker stopping");
                break;
            }
            _ = maintenance.tick() => {
                if let Err(error) = handlers::refresh_derived_state(&pool, offline_after).await {
                    tracing::error!(error = %error, "maintenance pass failed");
                }
            }
            _ = lembretes.tick() => {
                // Uma passagem que falha regista-se e não derruba o worker: a
                // entrega de lembretes não pode levar consigo o escoamento do
                // outbox, que serve o resto da instituição.
                match ocinye_core::modules::calendar::delivery::deliver_due(&pool).await {
                    Ok(0) => {}
                    Ok(count) => tracing::info!(count, "reminders delivered"),
                    Err(error) => tracing::error!(error = %error, "reminder pass failed"),
                }
            }
            _ = ingestao.tick() => {
                // Uma passagem do worker não vem de um pedido HTTP: não há cabeçalhos de
                // onde herdar correlação, e por isso gera-se uma nova. É o que liga
                // as linhas desta passagem umas às outras nos registos.
                let ids = ocinye_observability::CorrelationIds::from_headers(None, None);
                match ocinye_core::modules::mail::service::ingest_all(
                    &pool,
                    correio.as_ref(),
                    &ids,
                )
                .await
                {
                    Ok(passagem) if passagem.mailboxes == 0 => {}
                    Ok(passagem) => tracing::info!(
                        mailboxes = passagem.mailboxes,
                        indexed = passagem.indexed,
                        failed = passagem.failed,
                        "mail index refreshed"
                    ),
                    // Uma passagem que falha regista-se e não derruba o worker:
                    // o correio não pode levar consigo o escoamento do outbox,
                    // que serve o resto da instituição.
                    Err(error) => tracing::error!(error = %error, "mail ingestion pass failed"),
                }
            }
            drained = outbox::drain(&pool, BATCH_SIZE, store.as_ref(), None) => {
                match drained {
                    // An empty pass means idle: back off rather than spin.
                    Ok(0) => tokio::time::sleep(IDLE_POLL_INTERVAL).await,
                    Ok(count) => tracing::debug!(count, "events processed"),
                    Err(error) => {
                        tracing::error!(error = %error, "outbox drain failed");
                        tokio::time::sleep(IDLE_POLL_INTERVAL).await;
                    }
                }
            }
        }
    }

    Ok(())
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
        () = ctrl_c => {},
        () = terminate => {},
    }
}
