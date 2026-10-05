//! Running a system tool: a fixed program, a typed argv, no shell.
//!
//! The bootstrap may call `apt-get`, `systemctl`, `ufw`, `docker`, `ss` and
//! the release's own `install/ocinye`, and nothing else, and always the same
//! way: an absolute program path that is a **constant** of this crate (or
//! derived from constants and a validated release id), arguments as separate
//! argv elements, a cleared environment, and a time limit. There is no
//! `sh -c` anywhere in this crate, and there is no function that takes a
//! command line.

use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// The programs the bootstrap may run. Absolute paths on Ubuntu 24.04.
pub mod program {
    /// apt-get.
    pub const APT_GET: &str = "/usr/bin/apt-get";
    /// dpkg-query.
    pub const DPKG_QUERY: &str = "/usr/bin/dpkg-query";
    /// systemctl.
    pub const SYSTEMCTL: &str = "/usr/bin/systemctl";
    /// ufw.
    pub const UFW: &str = "/usr/sbin/ufw";
    /// docker.
    pub const DOCKER: &str = "/usr/bin/docker";
    /// ss.
    pub const SS: &str = "/usr/bin/ss";
    /// curl (installed by P04 when missing).
    pub const CURL: &str = "/usr/bin/curl";
    /// setsid (util-linux).
    pub const SETSID: &str = "/usr/bin/setsid";
    /// snap.
    pub const SNAP: &str = "/usr/bin/snap";
    /// nft.
    pub const NFT: &str = "/usr/sbin/nft";
}

/// What a run produced.
#[derive(Debug)]
pub struct Output {
    /// Exit code (`None`: killed by a signal or by the time limit).
    pub code: Option<i32>,
    /// Standard output (kept in memory only).
    pub stdout: String,
    /// Standard error.
    pub stderr: String,
    /// The time limit was reached.
    pub timed_out: bool,
}

impl Output {
    /// Exit code 0.
    #[must_use]
    pub fn ok(&self) -> bool {
        self.code == Some(0) && !self.timed_out
    }
}

/// Is the program present?
#[must_use]
pub fn exists(program: &str) -> bool {
    Path::new(program).is_file()
}

fn base(program: &Path, args: &[&str]) -> Command {
    let mut cmd = Command::new(program);
    cmd.args(args)
        .env_clear()
        .env(
            "PATH",
            "/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin",
        )
        .env("LANG", "C.UTF-8")
        .env("LC_ALL", "C.UTF-8")
        .env("DEBIAN_FRONTEND", "noninteractive")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    cmd
}

/// Run a constant program with a typed argv.
#[must_use]
pub fn run(program: &'static str, args: &[&str], limit: Duration) -> Output {
    run_path(Path::new(program), args, limit)
}

/// Run a program at a path derived from constants (`install/ocinye` in the
/// staged release). The caller is responsible for the path being one of
/// those; it never comes from the operator.
#[must_use]
pub fn run_path(program: &Path, args: &[&str], limit: Duration) -> Output {
    let child = base(program, args).spawn();
    let Ok(mut child) = child else {
        return Output {
            code: None,
            stdout: String::new(),
            stderr: format!("cannot start {}", program.display()),
            timed_out: false,
        };
    };
    // Drain both pipes on threads so a chatty child never blocks on a full pipe.
    let mut out_pipe = child.stdout.take();
    let mut err_pipe = child.stderr.take();
    let out = std::thread::spawn(move || {
        let mut s = String::new();
        if let Some(p) = out_pipe.as_mut() {
            let _ = p.read_to_string(&mut s);
        }
        s
    });
    let err = std::thread::spawn(move || {
        let mut s = String::new();
        if let Some(p) = err_pipe.as_mut() {
            let _ = p.read_to_string(&mut s);
        }
        s
    });
    let started = Instant::now();
    let mut timed_out = false;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Some(status),
            Ok(None) if started.elapsed() > limit => {
                timed_out = true;
                let _ = child.kill();
                break child.wait().ok();
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(100)),
            Err(_) => break None,
        }
    };
    Output {
        code: status.and_then(|s| s.code()),
        stdout: out.join().unwrap_or_default(),
        stderr: err.join().unwrap_or_default(),
        timed_out,
    }
}

/// Seconds → Duration.
#[must_use]
pub const fn secs(s: u64) -> Duration {
    Duration::from_secs(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn os_argumentos_nao_passam_por_nenhuma_shell() {
        // Um argumento com metacaracteres chega inteiro, como um só argv.
        let o = run_path(Path::new("/bin/echo"), &["a; id", "$(id)"], secs(5));
        assert!(o.ok());
        assert_eq!(o.stdout, "a; id $(id)\n");
    }

    #[test]
    fn o_limite_de_tempo_mata_o_processo() {
        let o = run_path(Path::new("/bin/sleep"), &["5"], Duration::from_millis(300));
        assert!(o.timed_out && !o.ok());
    }

    #[test]
    fn o_ambiente_e_limpo() {
        std::env::set_var("OCINYE_TEST_SEGREDO", "x");
        let o = run_path(Path::new("/usr/bin/env"), &[], secs(5));
        assert!(!o.stdout.contains("OCINYE_TEST_SEGREDO"));
        assert!(o.stdout.contains("LANG=C.UTF-8"));
    }
}
