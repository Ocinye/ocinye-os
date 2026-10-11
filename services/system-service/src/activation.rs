//! Local activation: this machine creates the Instance it will run.
//!
//! An Ocinye OS image carries a release — the tree, the images, and that
//! release's own installer. Activation runs exactly that installer, on this
//! machine, with the answers a person gave at the machine's own screen. It is
//! the same program, creating the same Instance, as when the Ocinye OS
//! Installer drives it from another computer: this module is one more host of
//! the installation, not another way of installing.
//!
//! What a caller chooses are the seven values of the request, each validated
//! before it gets here. The program, its sub-command, its option names, where
//! the credential is written and the environment are fixed below. Nothing is
//! passed through a shell.

use std::fs;
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use ocinye_system_contracts::reason::{AdapterFailure, Reason};
use ocinye_system_contracts::request::{Request, Secret};
use ocinye_system_contracts::state::{ActivationPhase, ActivationState, InitialCredential};

use crate::tool::{self, Tool};

/// An installation that has not finished in this long has not worked.
const DEADLINE: Duration = Duration::from_secs(60 * 60);

/// What starts an installation and waits for it. One implementation runs the
/// release's installer; tests use another.
pub trait Installer: Send + Sync {
    /// Run to the end and say whether it succeeded. `log` receives everything
    /// the installation prints.
    fn install(&self, program: &Path, args: &[String], log: &Path) -> bool;
}

/// The release's installer, as a subprocess.
pub struct ReleaseInstaller;

impl Installer for ReleaseInstaller {
    fn install(&self, program: &Path, args: &[String], log: &Path) -> bool {
        // The container runtime is held and disabled on an image until the
        // machine is given something to run.
        for unit in ["containerd.service", "docker.socket", "docker.service"] {
            if tool::run(Tool::Systemctl, &["enable", "--now", unit]).is_err() {
                return false;
            }
        }
        let args: Vec<&str> = args.iter().map(String::as_str).collect();
        tool::run_logged(program, &args, log, DEADLINE) == Ok(())
    }
}

/// Local activation on one machine.
pub struct Activation {
    /// Where images keep releases (`/usr/lib/ocinye/release`).
    releases: PathBuf,
    /// What an installed Instance leaves (`/etc/ocinye/release.env`).
    installed: PathBuf,
    /// This module's own files.
    dir: PathBuf,
    running: Arc<AtomicBool>,
    installer: Arc<dyn Installer>,
}

impl Activation {
    /// Activation over these paths.
    #[must_use]
    pub fn new(
        releases: PathBuf,
        installed: PathBuf,
        dir: PathBuf,
        installer: Arc<dyn Installer>,
    ) -> Self {
        Self {
            releases,
            installed,
            dir,
            running: Arc::new(AtomicBool::new(false)),
            installer,
        }
    }

    /// The one release this image carries, with its installer: its id and the
    /// installer's path. An image with none, or with more than one, offers no
    /// activation — which release to install is not this module's to guess.
    fn release(&self) -> Option<(String, PathBuf)> {
        let mut found = fs::read_dir(&self.releases)
            .ok()?
            .filter_map(Result::ok)
            .filter_map(|e| {
                let id = e.file_name().into_string().ok()?;
                let installer = e.path().join("install/ocinye");
                let plain = !id.is_empty()
                    && id.len() <= 64
                    && id
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'.');
                (plain && installer.is_file()).then_some((id, installer))
            });
        let only = found.next()?;
        found.next().is_none().then_some(only)
    }

    fn credential_file(&self) -> PathBuf {
        self.dir.join("credential")
    }
    fn admin_file(&self) -> PathBuf {
        self.dir.join("admin")
    }
    /// Present from the moment an installation starts until it succeeds.
    fn started_file(&self) -> PathBuf {
        self.dir.join("started")
    }
    fn log_file(&self) -> PathBuf {
        self.dir.join("install.log")
    }

    /// Where activation stands.
    #[must_use]
    pub fn state(&self) -> ActivationState {
        let release = self.release();
        // Running comes first: the installer writes the Instance's
        // configuration long before it is done, and a machine whose services
        // are still coming up is not an activated machine.
        let phase = if self.running.load(Ordering::SeqCst) {
            ActivationPhase::Running
        } else if self.installed.exists() {
            ActivationPhase::Activated
        } else if self.started_file().exists() {
            // Started and neither running nor finished: it failed, or the
            // service was stopped under it.
            ActivationPhase::Failed
        } else if release.is_some() {
            ActivationPhase::Ready
        } else {
            ActivationPhase::NotOffered
        };
        let steps_begun = if phase == ActivationPhase::Running {
            // The installer announces each step on a line of its own.
            fs::read_to_string(self.log_file())
                .map(|log| log.lines().filter(|l| l.starts_with("== ")).count())
                .ok()
                .and_then(|n| u32::try_from(n).ok())
                .unwrap_or(0)
        } else {
            0
        };
        ActivationState {
            phase,
            release_id: release.map(|(id, _)| id),
            steps_begun,
            credential_waiting: phase == ActivationPhase::Activated
                && self.credential_file().is_file(),
        }
    }

    /// Start creating the Instance.
    ///
    /// # Errors
    ///
    /// [`Reason::OperationInProgress`] while one runs,
    /// [`Reason::NotOfferedHere`] when there is an Instance already or no
    /// release to create one from.
    pub fn start(&self, request: &Request) -> Result<ActivationState, Reason> {
        let Request::ActivationStart {
            domain,
            instance_name,
            distributions,
            person_name,
            person_email,
            admin_name,
            admin_email,
        } = request
        else {
            return Err(Reason::MalformedRequest);
        };
        let Some((_, program)) = self.release() else {
            return Err(Reason::NotOfferedHere);
        };
        // One at a time: whoever sets the flag is the one that runs. Asked
        // before whether an Instance exists, because one that is still being
        // created already has its configuration on disk.
        if self.running.swap(true, Ordering::SeqCst) {
            return Err(Reason::OperationInProgress);
        }
        if self.installed.exists() {
            self.running.store(false, Ordering::SeqCst);
            return Err(Reason::NotOfferedHere);
        }
        let prepared = (|| -> std::io::Result<()> {
            fs::DirBuilder::new()
                .recursive(true)
                .mode(0o700)
                .create(&self.dir)?;
            let _ = fs::remove_file(self.credential_file());
            fs::write(self.started_file(), b"")?;
            fs::write(self.admin_file(), admin_email.as_bytes())?;
            fs::OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .mode(0o600)
                .open(self.log_file())?;
            Ok(())
        })();
        if prepared.is_err() {
            self.running.store(false, Ordering::SeqCst);
            return Err(Reason::AdapterFailed {
                cause: AdapterFailure::Refused,
            });
        }
        let mut args: Vec<String> = vec![
            "install".into(),
            "--domain".into(),
            domain.clone(),
            "--instance-name".into(),
            instance_name.clone(),
        ];
        for d in distributions {
            args.extend(["--distribution".into(), d.as_str().to_owned()]);
        }
        args.extend([
            "--name".into(),
            person_name.clone(),
            "--email".into(),
            person_email.clone(),
            "--admin-name".into(),
            admin_name.clone(),
            "--admin-email".into(),
            admin_email.clone(),
            "--tls".into(),
            "self-signed".into(),
            "--credential-file".into(),
            self.credential_file().to_string_lossy().into_owned(),
        ]);
        let (installer, running) = (Arc::clone(&self.installer), Arc::clone(&self.running));
        let (log, started, installed) =
            (self.log_file(), self.started_file(), self.installed.clone());
        std::thread::spawn(move || {
            let done = installer.install(&program, &args, &log);
            // Success is the installer saying so *and* the Instance being
            // there: one without the other is not an activated machine.
            if done && installed.exists() {
                let _ = fs::remove_file(&started);
            }
            running.store(false, Ordering::SeqCst);
        });
        Ok(self.state())
    }

    /// Hand over the first administrator's temporary credential, once.
    ///
    /// # Errors
    ///
    /// [`Reason::NotFound`] when there is none to hand over.
    pub fn credential(&self) -> Result<InitialCredential, Reason> {
        if !self.installed.exists() {
            return Err(Reason::NotFound);
        }
        let read = |p: PathBuf| {
            fs::read_to_string(p)
                .ok()
                .map(|s| s.trim().to_owned())
                .filter(|s| !s.is_empty())
        };
        let (Some(password), Some(username)) =
            (read(self.credential_file()), read(self.admin_file()))
        else {
            return Err(Reason::NotFound);
        };
        // Gone from the machine before it is on the screen: a second request
        // finds nothing, whatever became of the first answer.
        fs::remove_file(self.credential_file()).map_err(|_| Reason::AdapterFailed {
            cause: AdapterFailure::Refused,
        })?;
        let _ = fs::remove_file(self.admin_file());
        Ok(InitialCredential {
            username,
            password: Secret::new(password),
        })
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::AtomicU32;
    use std::sync::Mutex;

    use ocinye_system_contracts::state::Distribution;

    use super::*;

    /// An installer that records what it was asked and does what it is told.
    struct Scripted {
        succeed: bool,
        /// Whether it leaves an Instance behind (the marker and a credential).
        installs: bool,
        installed: PathBuf,
        asked: Mutex<Vec<(PathBuf, Vec<String>)>>,
        hold: Arc<AtomicBool>,
    }

    impl Installer for Scripted {
        fn install(&self, program: &Path, args: &[String], log: &Path) -> bool {
            self.asked
                .lock()
                .unwrap()
                .push((program.to_owned(), args.to_vec()));
            fs::write(log, "== primeiro ==\n  nota\n== segundo ==\n").unwrap();
            // Like the real one: the configuration is on disk well before the
            // installation ends.
            if self.installs {
                fs::write(&self.installed, "OCINYE_RELEASE_SHA=abc\n").unwrap();
            }
            while self.hold.load(Ordering::SeqCst) {
                std::thread::sleep(Duration::from_millis(5));
            }
            if self.installs {
                let at = args.iter().position(|a| a == "--credential-file").unwrap();
                fs::write(&args[at + 1], "temporaria-de-prova\n").unwrap();
            }
            self.succeed
        }
    }

    struct Bench {
        activation: Activation,
        installer: Arc<Scripted>,
        hold: Arc<AtomicBool>,
        root: PathBuf,
    }

    fn bench(releases: &[&str], succeed: bool, installs: bool) -> Bench {
        static N: AtomicU32 = AtomicU32::new(0);
        let root = std::env::temp_dir().join(format!(
            "ocinye-activation-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::SeqCst)
        ));
        let _ = fs::remove_dir_all(&root);
        for id in releases {
            fs::create_dir_all(root.join("release").join(id).join("install")).unwrap();
            fs::write(
                root.join("release").join(id).join("install/ocinye"),
                "#!/bin/sh\n",
            )
            .unwrap();
        }
        fs::create_dir_all(root.join("etc")).unwrap();
        let hold = Arc::new(AtomicBool::new(false));
        let installer = Arc::new(Scripted {
            succeed,
            installs,
            installed: root.join("etc/release.env"),
            asked: Mutex::new(Vec::new()),
            hold: Arc::clone(&hold),
        });
        let activation = Activation::new(
            root.join("release"),
            root.join("etc/release.env"),
            root.join("state"),
            installer.clone(),
        );
        Bench {
            activation,
            installer,
            hold,
            root,
        }
    }

    fn request() -> Request {
        Request::ActivationStart {
            domain: "ocinye.empresa.example".into(),
            instance_name: "Instituto de Prova".into(),
            distributions: vec![Distribution::Research, Distribution::Education],
            person_name: "Pessoa de Prova".into(),
            person_email: "pessoa@empresa.example".into(),
            admin_name: "Administração".into(),
            admin_email: "admin@empresa.example".into(),
        }
    }

    fn settle(b: &Bench) -> ActivationState {
        for _ in 0..400 {
            let s = b.activation.state();
            if s.phase != ActivationPhase::Running {
                return s;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        panic!("the installation never ended");
    }

    #[test]
    fn uma_imagem_sem_release_ou_com_mais_de_um_nao_oferece_activacao() {
        assert_eq!(
            bench(&[], true, true).activation.state().phase,
            ActivationPhase::NotOffered
        );
        let two = bench(&["aaa", "bbb"], true, true);
        assert_eq!(two.activation.state().phase, ActivationPhase::NotOffered);
        assert_eq!(
            two.activation.start(&request()),
            Err(Reason::NotOfferedHere)
        );
        assert!(two.installer.asked.lock().unwrap().is_empty());
    }

    #[test]
    fn activar_corre_o_instalador_do_release_com_os_valores_pedidos_e_mais_nenhum() {
        let b = bench(&["f3fc6ba0863b"], true, true);
        let ready = b.activation.state();
        assert_eq!(
            (ready.phase, ready.release_id.as_deref()),
            (ActivationPhase::Ready, Some("f3fc6ba0863b"))
        );
        b.activation.start(&request()).unwrap();
        let done = settle(&b);
        assert_eq!(done.phase, ActivationPhase::Activated);
        assert!(done.credential_waiting);
        let asked = b.installer.asked.lock().unwrap();
        let (program, args) = &asked[0];
        assert_eq!(program, &b.root.join("release/f3fc6ba0863b/install/ocinye"));
        let credential = b
            .root
            .join("state/credential")
            .to_string_lossy()
            .into_owned();
        let expected = [
            "install",
            "--domain",
            "ocinye.empresa.example",
            "--instance-name",
            "Instituto de Prova",
            "--distribution",
            "research",
            "--distribution",
            "education",
            "--name",
            "Pessoa de Prova",
            "--email",
            "pessoa@empresa.example",
            "--admin-name",
            "Administração",
            "--admin-email",
            "admin@empresa.example",
            "--tls",
            "self-signed",
            "--credential-file",
            &credential,
        ];
        assert_eq!(args, &expected.map(str::to_owned));
    }

    #[test]
    fn a_credencial_do_primeiro_administrador_entrega_se_uma_vez() {
        let b = bench(&["rel"], true, true);
        assert_eq!(b.activation.credential(), Err(Reason::NotFound));
        b.activation.start(&request()).unwrap();
        settle(&b);
        let c = b.activation.credential().unwrap();
        assert_eq!(
            (c.username.as_str(), c.password.expose()),
            ("admin@empresa.example", "temporaria-de-prova")
        );
        assert!(
            !b.root.join("state/credential").exists(),
            "ficou na máquina depois de entregue"
        );
        assert_eq!(b.activation.credential(), Err(Reason::NotFound));
        assert!(!b.activation.state().credential_waiting);
    }

    #[test]
    fn enquanto_corre_nao_comeca_outra_e_diz_quantos_passos_ja_comecaram() {
        let b = bench(&["rel"], true, true);
        b.hold.store(true, Ordering::SeqCst);
        b.activation.start(&request()).unwrap();
        let mut running = b.activation.state();
        for _ in 0..200 {
            if running.steps_begun == 2 {
                break;
            }
            std::thread::sleep(Duration::from_millis(5));
            running = b.activation.state();
        }
        assert_eq!(
            (running.phase, running.steps_begun),
            (ActivationPhase::Running, 2)
        );
        assert_eq!(
            b.activation.start(&request()),
            Err(Reason::OperationInProgress)
        );
        b.hold.store(false, Ordering::SeqCst);
        assert_eq!(settle(&b).phase, ActivationPhase::Activated);
        // With an Instance there, there is nothing left to activate.
        assert_eq!(b.activation.start(&request()), Err(Reason::NotOfferedHere));
        assert_eq!(b.installer.asked.lock().unwrap().len(), 1);
    }

    #[test]
    fn um_instalador_que_falha_ou_que_diz_que_sim_sem_deixar_instancia_e_uma_falha() {
        for (succeed, installs) in [(false, false), (true, false)] {
            let b = bench(&["rel"], succeed, installs);
            b.activation.start(&request()).unwrap();
            let s = settle(&b);
            assert_eq!(s.phase, ActivationPhase::Failed, "succeed={succeed}");
            assert!(!s.credential_waiting);
            assert_eq!(b.activation.credential(), Err(Reason::NotFound));
            // A failed activation can be tried again.
            b.activation.start(&request()).unwrap();
            settle(&b);
            assert_eq!(b.installer.asked.lock().unwrap().len(), 2);
        }
    }
}
