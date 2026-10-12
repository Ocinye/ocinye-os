//! The only way this service starts another program.
//!
//! A subprocess is an implementation detail of a typed adapter, never the
//! contract. Every one goes through [`run`], which fixes what a caller cannot
//! be trusted to get right each time:
//!
//! - the executable is one of [`Tool`]: an absolute path from a closed table,
//!   never resolved through `PATH`, never supplied by a client;
//! - arguments are built by the adapter in Rust; there is no shell, no
//!   `sh -c`, no interpolation;
//! - the environment is cleared and replaced by [`ENVIRONMENT`];
//! - standard input is closed, output is read up to [`OUTPUT_LIMIT`] and the
//!   rest is discarded;
//! - the process is killed at the deadline;
//! - the exit status is checked, and the tool's text never becomes a message:
//!   a failure is an [`AdapterFailure`] class.

use std::io::Read;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use ocinye_system_contracts::reason::AdapterFailure;

/// A program this service may start. Closed: adding one is a reviewed change.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tool {
    /// `systemctl`: power actions and unit state.
    Systemctl,
    /// `timedatectl`: time zone and clock source.
    Timedatectl,
    /// `localectl`: system locale and keyboard layout.
    Localectl,
    /// `hostnamectl`: only read.
    Hostnamectl,
    /// `rfkill`: radio switches, read and set.
    Rfkill,
    /// `ip`: only read, and only as JSON.
    Ip,
    /// `amixer`: ALSA mixer levels and switches of a card, read and set.
    Amixer,
    /// `netplan`: only `apply`, after the service wrote its own file.
    Netplan,
    /// `udevadm`: only `trigger` of one subsystem, after a rule changed.
    Udevadm,
}

impl Tool {
    /// Its absolute path on an Ocinye OS image (Ubuntu Server 24.04).
    #[must_use]
    pub const fn path(self) -> &'static str {
        match self {
            Self::Systemctl => "/usr/bin/systemctl",
            Self::Timedatectl => "/usr/bin/timedatectl",
            Self::Localectl => "/usr/bin/localectl",
            Self::Hostnamectl => "/usr/bin/hostnamectl",
            Self::Rfkill => "/usr/sbin/rfkill",
            Self::Ip => "/usr/sbin/ip",
            Self::Amixer => "/usr/bin/amixer",
            Self::Netplan => "/usr/sbin/netplan",
            Self::Udevadm => "/usr/bin/udevadm",
        }
    }

    /// How long it may take. `netplan apply` restarts the network daemon and
    /// waits for it; a mixer answers at once.
    #[must_use]
    pub const fn deadline(self) -> Duration {
        match self {
            Self::Netplan => Duration::from_secs(90),
            Self::Udevadm => Duration::from_secs(30),
            Self::Amixer => Duration::from_secs(10),
            _ => DEADLINE,
        }
    }
}

/// Who runs the tools: the system, or, in tests, a recording that answers
/// from a script. Adapters take one of these and never call [`run`] behind
/// its back, so what they would ask the system is observable.
pub trait Runner: Send + Sync {
    /// Run `tool` with `args`, as [`run`] does.
    ///
    /// # Errors
    ///
    /// As [`run`].
    fn run(&self, tool: Tool, args: &[&str]) -> Result<String, AdapterFailure>;
}

/// The real system.
#[derive(Debug, Default, Clone, Copy)]
pub struct System;

impl Runner for System {
    fn run(&self, tool: Tool, args: &[&str]) -> Result<String, AdapterFailure> {
        run(tool, args)
    }
}

/// The whole environment of a subprocess.
pub const ENVIRONMENT: &[(&str, &str)] = &[
    ("PATH", "/usr/sbin:/usr/bin:/sbin:/bin"),
    ("LC_ALL", "C.UTF-8"),
    ("LANG", "C.UTF-8"),
];
/// Bytes of standard output kept.
pub const OUTPUT_LIMIT: usize = 64 * 1024;
/// Default deadline.
pub const DEADLINE: Duration = Duration::from_secs(20);

/// Whether an argument is safe to hand to a tool: no NUL, not empty, and not
/// something the tool would read as an option when the adapter meant a value.
/// Adapters pass options as literals and values through [`value`].
#[must_use]
pub fn value(arg: &str) -> Option<&str> {
    if arg.is_empty()
        || arg.starts_with('-')
        || arg.contains('\0')
        || arg.chars().any(char::is_control)
    {
        None
    } else {
        Some(arg)
    }
}

/// Run a tool and return its standard output.
///
/// # Errors
///
/// [`AdapterFailure::Unreachable`] when the tool cannot be started,
/// [`AdapterFailure::Timeout`] at the deadline, [`AdapterFailure::Refused`] on
/// a non-zero exit, [`AdapterFailure::UnexpectedReply`] when its output is not
/// UTF-8.
pub fn run(tool: Tool, args: &[&str]) -> Result<String, AdapterFailure> {
    run_program(tool.path(), args, tool.deadline())
}

/// [`run`] with the program given: exists so the runner itself can be tested
/// against programs every machine has. Not exported.
pub(crate) fn run_program(
    program: &str,
    args: &[&str],
    deadline: Duration,
) -> Result<String, AdapterFailure> {
    debug_assert!(program.starts_with('/'), "tools are absolute paths");
    let mut child = Command::new(program)
        .args(args)
        .env_clear()
        .envs(ENVIRONMENT.iter().copied())
        .current_dir("/")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| AdapterFailure::Unreachable)?;
    let mut stdout = child.stdout.take().ok_or(AdapterFailure::Unreachable)?;
    // Read on another thread so a tool that fills its pipe cannot outlive the
    // deadline by blocking this one.
    let reader = std::thread::spawn(move || {
        let mut kept = Vec::new();
        let mut buf = [0u8; 8192];
        loop {
            match stdout.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    let room = OUTPUT_LIMIT.saturating_sub(kept.len());
                    kept.extend_from_slice(&buf[..n.min(room)]);
                }
            }
        }
        kept
    });
    let started = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if started.elapsed() >= deadline => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(AdapterFailure::Timeout);
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(10)),
            Err(_) => return Err(AdapterFailure::Unreachable),
        }
    };
    let out = reader.join().map_err(|_| AdapterFailure::UnexpectedReply)?;
    if !status.success() {
        return Err(AdapterFailure::Refused);
    }
    String::from_utf8(out).map_err(|_| AdapterFailure::UnexpectedReply)
}

/// Run a program that is not in the table — the installer of the release an
/// image carries, whose path contains the release's id — to its end, with
/// everything it prints going to `log`. The same rules as [`run`]: an absolute
/// path, no shell, a fixed environment, a deadline. Its home is its own
/// directory, never an account's.
///
/// # Errors
///
/// As [`run`], without [`AdapterFailure::UnexpectedReply`]: the output is not
/// read here.
pub fn run_logged(
    program: &std::path::Path,
    args: &[&str],
    log: &std::path::Path,
    deadline: Duration,
) -> Result<(), AdapterFailure> {
    if !program.is_absolute() {
        return Err(AdapterFailure::Unreachable);
    }
    let out = std::fs::OpenOptions::new()
        .append(true)
        .open(log)
        .map_err(|_| AdapterFailure::Unreachable)?;
    let err = out.try_clone().map_err(|_| AdapterFailure::Unreachable)?;
    let home = log.parent().unwrap_or(std::path::Path::new("/"));
    let mut child = Command::new(program)
        .args(args)
        .env_clear()
        .envs(ENVIRONMENT.iter().copied())
        .env("HOME", home)
        .current_dir("/")
        .stdin(Stdio::null())
        .stdout(out)
        .stderr(err)
        .spawn()
        .map_err(|_| AdapterFailure::Unreachable)?;
    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) if status.success() => return Ok(()),
            Ok(Some(_)) => return Err(AdapterFailure::Refused),
            Ok(None) if started.elapsed() >= deadline => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(AdapterFailure::Timeout);
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(200)),
            Err(_) => return Err(AdapterFailure::Unreachable),
        }
    }
}

#[cfg(test)]
pub mod fake {
    //! A runner that answers from a script and records what it was asked.

    use super::{AdapterFailure, Runner, Tool};
    use std::sync::Mutex;

    type Answer = (Tool, Vec<String>, Result<String, AdapterFailure>);

    /// A scripted runner. An exact `(tool, args)` gets its scripted answer.
    /// Anything else that only reads (`get`, `show`) fails as a missing
    /// control would; anything that changes succeeds with no output.
    #[derive(Default)]
    pub struct Script {
        answers: Mutex<Vec<Answer>>,
        /// Every call, in order.
        pub calls: Mutex<Vec<(Tool, Vec<String>)>>,
    }

    impl Script {
        /// `tool args` prints `out`.
        pub fn answer(&self, tool: Tool, args: &[&str], out: &str) {
            self.answers.lock().unwrap().push((
                tool,
                args.iter().map(|a| (*a).to_owned()).collect(),
                Ok(out.to_owned()),
            ));
        }

        /// `tool args` fails with `failure`, before any answer scripted for it.
        pub fn fail(&self, tool: Tool, args: &[&str], failure: AdapterFailure) {
            self.answers.lock().unwrap().insert(
                0,
                (
                    tool,
                    args.iter().map(|a| (*a).to_owned()).collect(),
                    Err(failure),
                ),
            );
        }

        /// `tool args` no longer fails.
        pub fn heal(&self, tool: Tool, args: &[&str]) {
            self.answers.lock().unwrap().retain(|(t, a, r)| {
                !(*t == tool && r.is_err() && a.iter().map(String::as_str).eq(args.iter().copied()))
            });
        }

        /// Whether some tool ran with exactly `args`.
        pub fn ran(&self, args: &[&str]) -> bool {
            self.calls
                .lock()
                .unwrap()
                .iter()
                .any(|(_, a)| a.iter().map(String::as_str).eq(args.iter().copied()))
        }

        /// The calls that changed something (not `get`/`show`), in order.
        pub fn changes(&self) -> Vec<(Tool, Vec<String>)> {
            self.calls
                .lock()
                .unwrap()
                .iter()
                .filter(|(_, a)| !a.iter().any(|w| w == "get" || w == "show"))
                .cloned()
                .collect()
        }
    }

    impl Runner for Script {
        fn run(&self, tool: Tool, args: &[&str]) -> Result<String, AdapterFailure> {
            let args: Vec<String> = args.iter().map(|a| (*a).to_owned()).collect();
            self.calls.lock().unwrap().push((tool, args.clone()));
            if let Some((_, _, answer)) = self
                .answers
                .lock()
                .unwrap()
                .iter()
                .find(|(t, a, _)| *t == tool && *a == args)
            {
                return answer.clone();
            }
            if args.iter().any(|w| w == "get" || w == "show") {
                Err(AdapterFailure::Refused)
            } else {
                Ok(String::new())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn todas_as_ferramentas_sao_caminhos_absolutos_sem_shell() {
        for t in [
            Tool::Systemctl,
            Tool::Timedatectl,
            Tool::Localectl,
            Tool::Hostnamectl,
            Tool::Rfkill,
            Tool::Ip,
            Tool::Amixer,
            Tool::Netplan,
            Tool::Udevadm,
        ] {
            let p = t.path();
            assert!(p.starts_with("/usr/"), "{p}");
            let name = p.rsplit('/').next().unwrap();
            assert!(
                !["sh", "bash", "dash", "env", "sudo", "su"].contains(&name),
                "{p}"
            );
        }
    }

    #[test]
    fn o_ambiente_e_limpo_e_substituido() {
        // SAFETY of intent: this only proves the child does not inherit it.
        let out = run_program("/usr/bin/env", &[], Duration::from_secs(5)).unwrap();
        let mut names: Vec<&str> = out.lines().filter_map(|l| l.split('=').next()).collect();
        names.sort_unstable();
        assert_eq!(names, ["LANG", "LC_ALL", "PATH"]);
    }

    #[test]
    fn um_argumento_nao_e_interpretado_por_nenhuma_shell() {
        let out = run_program(
            "/bin/echo",
            &["$(id)", ";", "`id`", "|", "id"],
            Duration::from_secs(5),
        )
        .unwrap();
        assert_eq!(out.trim(), "$(id) ; `id` | id");
    }

    #[test]
    fn saida_nao_zero_e_recusa_e_a_ferramenta_ausente_e_inalcancavel() {
        assert_eq!(
            run_program("/usr/bin/false", &[], Duration::from_secs(5)),
            Err(AdapterFailure::Refused)
        );
        assert_eq!(
            run_program("/nonexistent/ocinye-tool", &[], Duration::from_secs(5)),
            Err(AdapterFailure::Unreachable)
        );
    }

    #[test]
    fn o_prazo_mata_o_processo() {
        let started = Instant::now();
        assert_eq!(
            run_program("/bin/sleep", &["30"], Duration::from_millis(200)),
            Err(AdapterFailure::Timeout)
        );
        assert!(started.elapsed() < Duration::from_secs(5));
    }

    #[test]
    fn a_saida_e_limitada() {
        let out = run_program("/usr/bin/yes", &[], Duration::from_millis(300));
        // `yes` never ends: the deadline wins, and nothing grew without bound.
        assert_eq!(out, Err(AdapterFailure::Timeout));
    }

    #[test]
    fn um_valor_que_pareceria_uma_opcao_nao_passa() {
        assert_eq!(value("Africa/Luanda"), Some("Africa/Luanda"));
        assert_eq!(value("--root=/"), None);
        assert_eq!(value(""), None);
        assert_eq!(value("a\nb"), None);
    }
}
