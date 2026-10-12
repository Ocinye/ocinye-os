//! The engine's socket: `0600 root`, one JSON request per connection.
//!
//! Short operations answer directly. Long ones (download, install, rollback,
//! base maintenance) reserve the engine and run on their own thread; the
//! answer is the state at once, and the interface follows `status`.

use crate::base;
use crate::engine::{Engine, Stop};
use crate::host::Host;
use crate::paths::Paths;
use ocinye_update_contracts::wire::{EngineReply, EngineRequest, Operation, MAX_FRAME};
use ocinye_update_contracts::UpdateReason;
use std::io::{Read, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::sync::Arc;

/// A long operation run on its own thread.
type Job<H> = Box<dyn FnOnce(&Engine<H>) + Send>;

fn refused(reason: UpdateReason) -> EngineReply {
    EngineReply::Refused { reason }
}

fn state_or_refusal<H: Host>(
    engine: &Engine<H>,
    r: Result<ocinye_update_contracts::UpdateState, Stop>,
) -> EngineReply {
    match r {
        Ok(state) => EngineReply::State {
            state: Box::new(state),
        },
        Err(Stop::Reason(reason)) => refused(reason),
        Err(Stop::Crashed(_)) => EngineReply::State {
            state: Box::new(engine.status()),
        },
    }
}

/// Answer one request.
pub fn handle<H: Host + 'static>(
    engine: &Arc<Engine<H>>,
    paths: &Paths,
    host: &Arc<H>,
    request: &EngineRequest,
) -> EngineReply {
    if request.validate().is_err() {
        return refused(UpdateReason::ManifestInvalid);
    }
    let actor = request.actor.clone();
    let via = request.via.clone();
    let spawn = |what: &'static str, f: Job<H>| -> EngineReply {
        if let Err(r) = engine.reserve(what) {
            return refused(r);
        }
        let e = Arc::clone(engine);
        std::thread::spawn(move || {
            f(&e);
            e.release();
        });
        // Give the operation the moment it needs to record its first phase.
        std::thread::sleep(std::time::Duration::from_millis(200));
        EngineReply::State {
            state: Box::new(engine.status()),
        }
    };
    match &request.operation {
        Operation::Status => EngineReply::State {
            state: Box::new(engine.status()),
        },
        Operation::History => EngineReply::History {
            history: engine.history(),
        },
        Operation::Check => {
            if engine.busy() {
                return refused(UpdateReason::MaintenanceBusy);
            }
            let r = engine.check();
            let auto = r
                .as_ref()
                .is_ok_and(|s| s.phase == ocinye_update_contracts::Phase::UpdateAvailable)
                && engine.auto_download();
            let reply = state_or_refusal(engine, r);
            if auto {
                let _ = spawn(
                    "download",
                    Box::new(move |e| {
                        let _ = e.download(&actor, &via);
                    }),
                );
            }
            reply
        }
        Operation::Download => spawn(
            "download",
            Box::new(move |e| {
                let _ = e.download(&actor, &via);
            }),
        ),
        Operation::Install => spawn(
            "install",
            Box::new(move |e| {
                let _ = e.install(&actor, &via);
            }),
        ),
        Operation::Rollback => spawn(
            "rollback",
            Box::new(move |e| {
                let _ = e.rollback(&actor, &via);
            }),
        ),
        Operation::Cancel => state_or_refusal(engine, engine.cancel()),
        Operation::Configure { change } => {
            if engine.busy() && change.channel.is_some() {
                return refused(UpdateReason::MaintenanceBusy);
            }
            state_or_refusal(engine, engine.configure(change))
        }
        Operation::Import { bundle } => {
            if engine.busy() {
                return refused(UpdateReason::MaintenanceBusy);
            }
            state_or_refusal(engine, engine.import(bundle))
        }
        Operation::BaseStatus => EngineReply::Base {
            state: base::status(paths),
        },
        Operation::BaseCheck | Operation::BaseApply => {
            if let Err(r) = engine.reserve("base") {
                return refused(r);
            }
            let result = if matches!(request.operation, Operation::BaseCheck) {
                base::check(paths, host.as_ref())
            } else {
                base::apply(paths, host.as_ref())
            };
            engine.release();
            match result {
                Ok(state) => EngineReply::Base { state },
                Err(reason) => refused(reason),
            }
        }
    }
}

fn peer_is_root(stream: &UnixStream) -> bool {
    #[cfg(target_os = "linux")]
    {
        rustix::net::sockopt::socket_peercred(stream).is_ok_and(|c| c.uid.is_root())
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = stream;
        true
    }
}

fn serve_one<H: Host + 'static>(
    engine: &Arc<Engine<H>>,
    paths: &Paths,
    host: &Arc<H>,
    mut stream: UnixStream,
) {
    if !peer_is_root(&stream) {
        return;
    }
    let mut buf = Vec::new();
    if (&mut stream)
        .take(MAX_FRAME as u64 + 1)
        .read_to_end(&mut buf)
        .is_err()
        || buf.len() > MAX_FRAME
    {
        return;
    }
    let reply = match serde_json::from_slice::<EngineRequest>(&buf) {
        Ok(request) => handle(engine, paths, host, &request),
        Err(_) => refused(UpdateReason::ManifestInvalid),
    };
    if let Ok(mut bytes) = serde_json::to_vec(&reply) {
        bytes.push(b'\n');
        let _ = stream.write_all(&bytes);
    }
}

/// Listen until the process ends.
pub fn serve<H: Host + 'static>(
    engine: Arc<Engine<H>>,
    paths: Paths,
    host: Arc<H>,
) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    if let Some(dir) = paths.socket.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let _ = std::fs::remove_file(&paths.socket);
    let listener = UnixListener::bind(&paths.socket)?;
    std::fs::set_permissions(&paths.socket, std::fs::Permissions::from_mode(0o600))?;
    println!("OCINYE-UPDATE-ENGINE-READY");
    for stream in listener.incoming().flatten() {
        let e = Arc::clone(&engine);
        let p = paths.clone();
        let h = Arc::clone(&host);
        std::thread::spawn(move || serve_one(&e, &p, &h, stream));
    }
    Ok(())
}
