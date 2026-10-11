//! Side effects on the machine, behind one trait so the claim rules can be
//! exercised without a machine. Every command is a fixed program with fixed
//! arguments; nothing here takes text from the network.

use std::io::Read as _;
use std::path::Path;
use std::process::Command;
use std::time::Duration;

pub trait System {
    /// Unix seconds.
    fn now(&self) -> i64;
    /// Random bytes from the kernel CSPRNG (only after [`System::wait_rng`]).
    fn random(&self, buf: &mut [u8]) -> std::io::Result<()>;
    /// Block until the kernel RNG is initialised, at most `limit`.
    fn wait_rng(&self, limit: Duration) -> bool;
    /// `ssh-keygen -q -t <kind> -N "" -C "" -f <path>`.
    fn ssh_keygen(&self, kind: &str, path: &Path) -> std::io::Result<()>;
    /// `systemctl <args>`.
    fn systemctl(&self, args: &[&str]) -> std::io::Result<()>;
    /// `ufw <args>`.
    fn ufw(&self, args: &[&str]) -> std::io::Result<()>;
    /// Arm the confirmation-timeout timer `secs` from now.
    fn arm_timeout(&self, secs: i64) -> std::io::Result<()>;
    /// Hand the offered-keys directory to the AKC user.
    fn chown_offered(&self, dir: &Path) -> std::io::Result<()>;
    /// Enable or expire the `ocinye-claim` account (defence in depth: the
    /// `AuthorizedKeysCommand` already answers nothing once claimed).
    fn claim_account(&self, enabled: bool) -> std::io::Result<()>;
    /// `chown ocinye:ocinye <path>` (sshd reads `authorized_keys` as the user).
    fn chown_ocinye(&self, path: &Path) -> std::io::Result<()>;
}

fn run(program: &str, args: &[&str]) -> std::io::Result<()> {
    // Never on stdout: stdout carries the claim protocol and the JSON the
    // Installer reads.
    let st = Command::new(program)
        .args(args)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()?;
    if st.success() {
        Ok(())
    } else {
        Err(std::io::Error::other(format!(
            "{program} {}: {st}",
            args.first().unwrap_or(&"")
        )))
    }
}

/// The real machine.
pub struct Real;

impl System for Real {
    fn now(&self) -> i64 {
        chrono::Utc::now().timestamp()
    }

    fn random(&self, buf: &mut [u8]) -> std::io::Result<()> {
        std::fs::File::open("/dev/urandom")?.read_exact(buf)
    }

    fn wait_rng(&self, limit: Duration) -> bool {
        // /dev/random blocks only until the CRNG is initialised (Linux ≥ 5.6),
        // then never again: one byte read is the readiness probe.
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let mut b = [0u8; 1];
            let ok = std::fs::File::open("/dev/random")
                .and_then(|mut f| f.read_exact(&mut b))
                .is_ok();
            let _ = tx.send(ok);
        });
        rx.recv_timeout(limit).unwrap_or(false)
    }

    fn ssh_keygen(&self, kind: &str, path: &Path) -> std::io::Result<()> {
        let p = path.to_str().ok_or_else(|| std::io::Error::other("path"))?;
        let mut args = vec!["-q", "-t", kind, "-N", "", "-C", "", "-f", p];
        if kind == "ecdsa" {
            args.extend(["-b", "256"]);
        }
        run("/usr/bin/ssh-keygen", &args)
    }

    fn systemctl(&self, args: &[&str]) -> std::io::Result<()> {
        run("/usr/bin/systemctl", args)
    }

    fn ufw(&self, args: &[&str]) -> std::io::Result<()> {
        run("/usr/sbin/ufw", args)
    }

    fn arm_timeout(&self, secs: i64) -> std::io::Result<()> {
        // A transient timer, replaced if one is armed already; the command it
        // runs re-checks the deadline under the lock, so an early or late
        // firing is harmless.
        let _ = run(
            "/usr/bin/systemctl",
            &["stop", "ocinye-claim-timeout.timer"],
        );
        let on = format!("--on-active={}s", secs.max(1) + 1);
        run(
            "/usr/bin/systemd-run",
            &[
                "--unit=ocinye-claim-timeout",
                &on,
                "--timer-property=AccuracySec=1s",
                ocinye_image_contracts::paths::FIRSTBOOT_BIN,
                "claim-timeout",
            ],
        )
    }

    fn chown_offered(&self, dir: &Path) -> std::io::Result<()> {
        let p = dir.to_str().ok_or_else(|| std::io::Error::other("path"))?;
        run("/usr/bin/chown", &["ocinye-claim-akc:ocinye-claim-akc", p])
    }

    fn claim_account(&self, enabled: bool) -> std::io::Result<()> {
        run(
            "/usr/sbin/usermod",
            &[
                "--expiredate",
                if enabled { "" } else { "1" },
                "ocinye-claim",
            ],
        )
    }

    fn chown_ocinye(&self, path: &Path) -> std::io::Result<()> {
        let p = path.to_str().ok_or_else(|| std::io::Error::other("path"))?;
        run("/usr/bin/chown", &["ocinye:ocinye", p])
    }
}

#[cfg(test)]
pub mod fake {
    use std::cell::{Cell, RefCell};
    use std::path::Path;
    use std::time::Duration;

    /// A recording, deterministic machine.
    pub struct Fake {
        pub clock: Cell<i64>,
        pub seed: Cell<u8>,
        pub rng_ready: bool,
        pub calls: RefCell<Vec<String>>,
    }

    impl Fake {
        pub fn new(now: i64) -> Self {
            Self {
                clock: Cell::new(now),
                seed: Cell::new(1),
                rng_ready: true,
                calls: RefCell::new(vec![]),
            }
        }
        pub fn called(&self, prefix: &str) -> bool {
            self.calls.borrow().iter().any(|c| c.starts_with(prefix))
        }
    }

    impl super::System for Fake {
        fn now(&self) -> i64 {
            self.clock.get()
        }
        fn random(&self, buf: &mut [u8]) -> std::io::Result<()> {
            let s = self.seed.get();
            for (i, b) in buf.iter_mut().enumerate() {
                *b = s.wrapping_mul(31).wrapping_add(i as u8);
            }
            self.seed.set(s.wrapping_add(1));
            Ok(())
        }
        fn wait_rng(&self, _: Duration) -> bool {
            self.rng_ready
        }
        fn ssh_keygen(&self, kind: &str, path: &Path) -> std::io::Result<()> {
            // A real-shaped public key so fingerprints parse.
            use base64::Engine as _;
            let alg: &[u8] = if kind == "ed25519" {
                b"ssh-ed25519"
            } else {
                b"ecdsa-sha2-nistp256"
            };
            let mut blob = (alg.len() as u32).to_be_bytes().to_vec();
            blob.extend_from_slice(alg);
            let mut r = [0u8; 32];
            self.random(&mut r)?;
            blob.extend_from_slice(&r);
            let line = format!(
                "{} {}\n",
                std::str::from_utf8(alg).unwrap(),
                base64::engine::general_purpose::STANDARD.encode(blob)
            );
            std::fs::write(path, "PRIVATE")?;
            std::fs::write(path.with_extension("pub"), line)?;
            self.calls.borrow_mut().push(format!("ssh-keygen {kind}"));
            Ok(())
        }
        fn systemctl(&self, args: &[&str]) -> std::io::Result<()> {
            self.calls
                .borrow_mut()
                .push(format!("systemctl {}", args.join(" ")));
            Ok(())
        }
        fn ufw(&self, args: &[&str]) -> std::io::Result<()> {
            self.calls
                .borrow_mut()
                .push(format!("ufw {}", args.join(" ")));
            Ok(())
        }
        fn arm_timeout(&self, secs: i64) -> std::io::Result<()> {
            self.calls.borrow_mut().push(format!("arm-timeout {secs}"));
            Ok(())
        }
        fn chown_offered(&self, _: &Path) -> std::io::Result<()> {
            self.calls.borrow_mut().push("chown-offered".into());
            Ok(())
        }
        fn claim_account(&self, enabled: bool) -> std::io::Result<()> {
            self.calls
                .borrow_mut()
                .push(format!("claim-account {enabled}"));
            Ok(())
        }
        fn chown_ocinye(&self, _: &Path) -> std::io::Result<()> {
            self.calls.borrow_mut().push("chown-ocinye".into());
            Ok(())
        }
    }
}
