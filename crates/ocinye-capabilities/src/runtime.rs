//! The Wasmtime host.
//!
//! Loads a component, grants exactly what its manifest declares and policy
//! allows, and runs it under fuel, memory and wall-time limits.
//!
//! # The invocation contract
//!
//! Input arrives on stdin, output leaves on stdout, diagnostics on stderr. It
//! is deliberately the plainest contract that works across languages: a
//! capability can be written in Rust today and in something else tomorrow
//! without the host changing.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use std::num::NonZeroUsize;
use wasmtime::{Config, Engine, Linker, Module, Store, StoreLimits, StoreLimitsBuilder, Trap};
use wasmtime_wasi::p2::pipe::{MemoryInputPipe, MemoryOutputPipe};

// O módulo chamava-se `preview1` até à `wasmtime-wasi` 47; na 48 é `p1`.
// A API é a mesma — mudou o nome, não o contrato.
use wasmtime_wasi::p1::{self, WasiP1Ctx};
use wasmtime_wasi::WasiCtxBuilder;

use crate::error::{CapabilityError, CapabilityResult};
use crate::manifest::Manifest;

/// One request to run a capability.
pub struct Invocation<'a> {
    /// The capability's declaration.
    pub manifest: &'a Manifest,
    /// The component bytes.
    pub component: &'a [u8],
    /// Input delivered on stdin.
    pub input: Vec<u8>,
}

/// What running a capability produced.
#[derive(Debug, Clone)]
pub struct InvocationOutcome {
    /// Bytes written to stdout.
    pub output: Vec<u8>,
    /// Diagnostics written to stderr, truncated.
    pub diagnostics: String,
    /// Wall time consumed.
    pub duration: Duration,
    /// Fuel consumed, when the engine reports it.
    pub fuel_used: Option<u64>,
}

/// Host state for one invocation.
struct HostState {
    wasi: WasiP1Ctx,
    limits: StoreLimits,
}

/// How often the epoch advances.
///
/// The wall-time limit is enforced to this resolution. A hundred milliseconds
/// is far finer than any limit worth setting and coarse enough that the ticker
/// costs nothing.
/// Fundos de pilha guardados quando uma capacidade faz trap.
///
/// Vinte é o valor que a `wasmtime` usa por omissão. Fica escrito para
/// que continue a ser uma decisão nossa e não uma herança silenciosa.
const BACKTRACE_FRAMES: usize = 20;

const EPOCH_TICK: Duration = Duration::from_millis(100);

/// The Ocinye SystemCapability Runtime.
pub struct CapabilityRuntime {
    engine: Engine,
    /// Cleared on drop, which is what stops the ticker thread.
    ticking: Arc<AtomicBool>,
}

impl Drop for CapabilityRuntime {
    fn drop(&mut self) {
        self.ticking.store(false, Ordering::Relaxed);
    }
}

impl CapabilityRuntime {
    /// Build the runtime.
    ///
    /// # Errors
    ///
    /// Returns an error when the engine cannot be created.
    pub fn new() -> CapabilityResult<Self> {
        let mut config = Config::new();
        // Fuel bounds computation, independently of how loaded the machine is.
        config.consume_fuel(true);
        // Epoch interruption is what makes the wall-time limit enforceable even
        // for a capability that never yields.
        config.epoch_interruption(true);

        // Em macOS, sinais em vez das portas de excepção Mach.
        //
        // Por omissão, o `wasmtime` instala um *exception port* do Mach para o
        // processo inteiro, e é assim que apanha um trap dentro de WebAssembly.
        // Um exception port é do **processo**, e não do motor: quem o instala
        // passa a arbitrar as excepções de tudo o que corre ali dentro.
        //
        // Isso chega para partir um processo que também conduz outros
        // programas. A suite de viagens de browser do Ocinye OS abortava com
        // `SIGABRT` — sem uma linha de pânico, cerca de uma vez em cada duas
        // execuções — assim que passou a existir um motor no mesmo processo que
        // os Chromes. Medi-o pela negativa: sem motor, três execuções limpas;
        // com motor e **sem executar WebAssembly nenhum**, aborta na mesma. O
        // que causava não era a execução; era a presença.
        //
        // Com sinais, o `wasmtime` instala um `SIGSEGV`/`SIGBUS` que encadeia
        // com quem lá estava, em vez de tomar conta do processo. O isolamento é
        // o mesmo — o que muda é o mecanismo por onde um trap é apanhado.
        #[cfg(target_os = "macos")]
        config.macos_use_mach_ports(false);
        // `wasm_backtrace(true)` foi substituído por um limite de fundo de
        // pilha. Continua a recolher-se o rasto — é o que torna um trap
        // diagnosticável — e o limite fica escrito em vez de implícito no
        // valor por omissão da biblioteca.
        config.wasm_backtrace_max_frames(Some(
            NonZeroUsize::new(BACKTRACE_FRAMES).expect("um limite positivo"),
        ));

        let engine =
            Engine::new(&config).map_err(|error| CapabilityError::Load(error.to_string()))?;

        // One ticker for the engine, not one watchdog per invocation.
        //
        // The epoch is a property of the *engine*: incrementing it advances the
        // clock for every store running on it. A thread per invocation that
        // incremented once at its own deadline therefore interrupted whichever
        // capabilities happened to be running at that moment, so a ten-second
        // capability finishing cut short a two-minute one beside it. It also
        // held a sleeping thread for the whole wall-time budget even when the
        // capability returned immediately.
        //
        // A steady ticker turns the epoch into what it is meant to be — a
        // shared clock — and each store expresses its own limit as a number of
        // ticks. See `run`.
        let ticking = Arc::new(AtomicBool::new(true));
        {
            let engine = engine.clone();
            let ticking = Arc::clone(&ticking);
            std::thread::spawn(move || {
                while ticking.load(Ordering::Relaxed) {
                    std::thread::sleep(EPOCH_TICK);
                    engine.increment_epoch();
                }
            });
        }

        Ok(Self { engine, ticking })
    }

    /// Run a capability.
    ///
    /// # Errors
    ///
    /// Returns [`CapabilityError::ResourceExhausted`] when a limit is reached,
    /// [`CapabilityError::Execution`] when the capability fails, and
    /// [`CapabilityError::Load`] when the component cannot be instantiated.
    pub fn run(&self, invocation: Invocation<'_>) -> CapabilityResult<InvocationOutcome> {
        invocation.manifest.validate()?;
        let limits = invocation.manifest.limits;

        let module = Module::new(&self.engine, invocation.component)
            .map_err(|error| CapabilityError::Load(error.to_string()))?;

        let stdout = MemoryOutputPipe::new(4 * 1024 * 1024);
        let stderr = MemoryOutputPipe::new(64 * 1024);

        // Deny by default: no environment, no arguments, no preopened
        // directory, no network. The capability sees its input and nothing else.
        let mut wasi = WasiCtxBuilder::new();
        wasi.stdin(MemoryInputPipe::new(invocation.input))
            .stdout(stdout.clone())
            .stderr(stderr.clone());

        let store_limits = StoreLimitsBuilder::new()
            .memory_size(usize::try_from(limits.memory_bytes).unwrap_or(usize::MAX))
            .instances(1)
            .tables(4)
            .build();

        let mut store = Store::new(
            &self.engine,
            HostState {
                wasi: wasi.build_p1(),
                limits: store_limits,
            },
        );
        store.limiter(|state| &mut state.limits);
        store
            .set_fuel(limits.fuel)
            .map_err(|error| CapabilityError::Load(error.to_string()))?;

        // This store's own budget, in ticks of the shared clock. Rounded up and
        // never zero: a limit below one tick is still a limit, and a deadline
        // of zero would trap before the first instruction.
        let ticks = limits
            .wall_time_ms
            .div_ceil(u64::try_from(EPOCH_TICK.as_millis()).unwrap_or(100))
            .max(1);
        store.set_epoch_deadline(ticks);

        let mut linker: Linker<HostState> = Linker::new(&self.engine);
        p1::add_to_linker_sync(&mut linker, |state: &mut HostState| &mut state.wasi)
            .map_err(|error| CapabilityError::Load(error.to_string()))?;

        // A instanciação **corre código** — a função de arranque do módulo — e
        // por isso pode gastar combustível, exceder a época ou pedir memória a
        // mais. Um limite atingido aqui é um limite atingido, e não uma falha
        // de carregamento: quem chama tem de conseguir distinguir «esta
        // capacidade precisa de mais orçamento» de «esta capacidade não abre».
        //
        // Passa pelo mesmo classificador da execução, que decide pelo tipo do
        // trap e nunca pelo texto da mensagem.
        let comeco_da_instanciacao = Instant::now();
        let instance = linker
            .instantiate(&mut store, &module)
            .map_err(|error| classify(&error, comeco_da_instanciacao.elapsed(), ""))?;

        let entry = instance
            .get_typed_func::<(), ()>(&mut store, "_start")
            .map_err(|_| {
                CapabilityError::Contract(
                    "the component does not export `_start`; it must be a WASI command".to_owned(),
                )
            })?;

        let started = Instant::now();
        let result = entry.call(&mut store, ());
        let duration = started.elapsed();

        let fuel_used = store
            .get_fuel()
            .ok()
            .map(|remaining| limits.fuel.saturating_sub(remaining));

        drop(store);
        let diagnostics = String::from_utf8_lossy(&stderr.contents())
            .chars()
            .take(4_000)
            .collect();

        match result {
            Ok(()) => Ok(InvocationOutcome {
                output: stdout.contents().to_vec(),
                diagnostics,
                duration,
                fuel_used,
            }),
            Err(error) => Err(classify(&error, duration, &diagnostics)),
        }
    }
}

/// Distinguish a capability that hit a limit from one that failed on its own.
///
/// Classified from the trap type, not from the error message. Matching on
/// message text is fragile: a Wasmtime release that reworded a trap would
/// silently reclassify every resource exhaustion as an ordinary failure, and an
/// operator would stop being able to tell "this capability needs more fuel"
/// from "this capability is broken".
fn classify(error: &wasmtime::Error, duration: Duration, diagnostics: &str) -> CapabilityError {
    if let Some(trap) = error.downcast_ref::<Trap>() {
        return match trap {
            Trap::OutOfFuel => CapabilityError::ResourceExhausted(format!(
                "the capability exhausted its computation budget after {duration:?}"
            )),
            Trap::Interrupt => CapabilityError::ResourceExhausted(format!(
                "the capability exceeded its wall-time limit after {duration:?}"
            )),
            other => CapabilityError::Execution(format!("{other}\n{diagnostics}")),
        };
    }

    // A memory limit surfaces as an instantiation or growth failure rather than
    // a trap, so it is recognised separately.
    let message = error.to_string();
    if message.contains("exceeds memory limits") || message.contains("memory minimum size") {
        return CapabilityError::ResourceExhausted(message);
    }

    CapabilityError::Execution(format!("{message}\n{diagnostics}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_runtime_builds_with_limits_enabled() {
        assert!(CapabilityRuntime::new().is_ok());
    }

    /// Um componente que nunca devolve o controlo.
    ///
    /// Sem importações: o que se está a medir é o limite de tempo do host, não
    /// o WASI.
    const SPINS_FOREVER: &str = r#"(module (func (export "_start") (loop br 0)))"#;

    fn manifest_with_wall_time(ms: u64) -> Manifest {
        Manifest::parse(&format!(
            r#"{{"identifier":"ocinye.spin","name":"s","version":"0.1.0","description":"d",
                "inputs":[],"outputs":[],
                "limits":{{"fuel":5000000000,"memory_bytes":16777216,"wall_time_ms":{ms}}}}}"#
        ))
        .expect("manifest")
    }

    #[test]
    fn a_capability_that_never_yields_is_stopped_at_its_wall_time() {
        let runtime = CapabilityRuntime::new().unwrap();
        let manifest = manifest_with_wall_time(300);

        let outcome = runtime.run(Invocation {
            manifest: &manifest,
            component: SPINS_FOREVER.as_bytes(),
            input: Vec::new(),
        });

        assert!(
            matches!(outcome, Err(CapabilityError::ResourceExhausted(_))),
            "um componente em ciclo infinito não foi interrompido: {outcome:?}"
        );
    }

    /// Uma capacidade curta a terminar não encurta a que corre ao lado.
    ///
    /// # Porque este teste existe
    ///
    /// O relógio de época pertence ao `Engine`, não ao `Store`. A versão
    /// anterior armava um fio por invocação que incrementava a época **uma
    /// vez** ao seu próprio prazo — o que interrompia todas as outras
    /// invocações a correr nesse instante. Uma capacidade de dois minutos
    /// morria porque uma de meio segundo ao seu lado tinha acabado, e a razão
    /// devolvida dizia «excedeu o limite de tempo», que era falso.
    #[test]
    fn one_invocation_does_not_cut_another_short() {
        let runtime = std::sync::Arc::new(CapabilityRuntime::new().unwrap());

        let long = {
            let runtime = std::sync::Arc::clone(&runtime);
            std::thread::spawn(move || {
                let manifest = manifest_with_wall_time(2_000);
                let started = Instant::now();
                let outcome = runtime.run(Invocation {
                    manifest: &manifest,
                    component: SPINS_FOREVER.as_bytes(),
                    input: Vec::new(),
                });
                (outcome, started.elapsed())
            })
        };

        // Uma invocação curta ao lado, que expira muito antes.
        let short = manifest_with_wall_time(200);
        let _ = runtime.run(Invocation {
            manifest: &short,
            component: SPINS_FOREVER.as_bytes(),
            input: Vec::new(),
        });

        let (outcome, elapsed) = long.join().expect("thread");

        assert!(
            matches!(outcome, Err(CapabilityError::ResourceExhausted(_))),
            "a invocação longa devia acabar por esgotar o seu próprio limite: {outcome:?}"
        );
        assert!(
            elapsed >= Duration::from_millis(1_500),
            "a invocação de 2 s foi interrompida ao fim de {elapsed:?}: o prazo de \
             outra invocação continua a alcançá-la"
        );
    }

    #[test]
    fn a_component_that_is_not_wasm_is_refused_at_load() {
        let runtime = CapabilityRuntime::new().unwrap();
        let manifest = Manifest::parse(
            r#"{"identifier":"ocinye.test","name":"t","version":"0.1.0","description":"d",
                "inputs":[],"outputs":[]}"#,
        )
        .unwrap();

        let result = runtime.run(Invocation {
            manifest: &manifest,
            component: b"this is not a wasm module",
            input: Vec::new(),
        });
        assert!(matches!(result, Err(CapabilityError::Load(_))));
    }
}
