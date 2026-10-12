//! Running the few system tools the engine needs.
//!
//! A closed set of absolute executables, explicit arguments, no shell, the
//! environment cleared and replaced, stdin closed, output bounded, a deadline
//! after which the process is killed. Arguments come from validated values
//! (paths the engine built, unit names from a list), never from a request.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// The tools.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tool {
    /// `/usr/bin/systemctl`.
    Systemctl,
    /// `/usr/bin/docker`.
    Docker,
    /// `/usr/bin/apt-get`.
    AptGet,
    /// `/usr/bin/dpkg`.
    Dpkg,
}

impl Tool {
    /// Its absolute path.
    #[must_use]
    pub const fn path(self) -> &'static str {
        match self {
            Self::Systemctl => "/usr/bin/systemctl",
            Self::Docker => "/usr/bin/docker",
            Self::AptGet => "/usr/bin/apt-get",
            Self::Dpkg => "/usr/bin/dpkg",
        }
    }
}

/// What a run produced.
#[derive(Debug, Clone)]
pub struct Ran {
    /// Exit code; `None` when killed.
    pub code: Option<i32>,
    /// Standard output, at most [`OUTPUT_LIMIT`] bytes.
    pub stdout: String,
}

/// Output kept from a tool.
pub const OUTPUT_LIMIT: u64 = 256 * 1024;

/// Run a tool.
pub fn run(tool: Tool, args: &[&str], deadline: Duration) -> std::io::Result<Ran> {
    run_program(Path::new(tool.path()), args, None, &[], deadline)
}

/// Run an installer script the engine verified (its path is inside a
/// verified release), from `cwd`.
pub fn run_installer(
    program: &Path,
    args: &[&str],
    cwd: &Path,
    deadline: Duration,
) -> std::io::Result<Ran> {
    run_program(program, args, Some(cwd), &[("HOME", "/root")], deadline)
}

fn run_program(
    program: &Path,
    args: &[&str],
    cwd: Option<&Path>,
    extra_env: &[(&str, &str)],
    deadline: Duration,
) -> std::io::Result<Ran> {
    if !program.is_absolute() {
        return Err(std::io::Error::other("a tool path is absolute"));
    }
    if args.iter().any(|a| a.contains('\0')) {
        return Err(std::io::Error::other("an argument holds NUL"));
    }
    let mut cmd = Command::new(program);
    cmd.args(args)
        .env_clear()
        .env("PATH", "/usr/sbin:/usr/bin:/sbin:/bin")
        .env("LC_ALL", "C.UTF-8")
        .env("LANG", "C.UTF-8")
        .env("DEBIAN_FRONTEND", "noninteractive")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .current_dir(cwd.map_or_else(|| PathBuf::from("/"), Path::to_path_buf));
    for (k, v) in extra_env {
        cmd.env(k, v);
    }
    let mut child = cmd.spawn()?;
    let stdout = child.stdout.take();
    let reader = std::thread::spawn(move || {
        let mut out = Vec::new();
        if let Some(s) = stdout {
            let _ = s.take(OUTPUT_LIMIT).read_to_end(&mut out);
        }
        out
    });
    let start = Instant::now();
    let code = loop {
        if let Some(status) = child.try_wait()? {
            break status.code();
        }
        if start.elapsed() > deadline {
            let _ = child.kill();
            let _ = child.wait();
            break None;
        }
        std::thread::sleep(Duration::from_millis(200));
    };
    let out = reader.join().unwrap_or_default();
    Ok(Ran {
        code,
        stdout: String::from_utf8_lossy(&out).into_owned(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn as_ferramentas_sao_caminhos_absolutos() {
        for t in [Tool::Systemctl, Tool::Docker, Tool::AptGet, Tool::Dpkg] {
            assert!(t.path().starts_with("/usr/bin/"), "{t:?}");
        }
        assert!(run_program(
            Path::new("sh"),
            &["-c", "true"],
            None,
            &[],
            Duration::from_secs(1)
        )
        .is_err());
    }
}
