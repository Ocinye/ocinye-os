//! Linking this machine's seat to the Instance installed on it.
//!
//! An Ocinye OS machine has no screen session until an Instance runs on it.
//! Once one does, three facts have to be on disk before anyone can sit down:
//! the public key that Instance signs session policies with, the address of
//! its Workspace, and a way for that address to resolve to this machine even
//! with no network. `ocinye-session-service link-instance` writes them, from
//! what the installation recorded and what the Core answers on the loopback
//! door. It is run by `ocinye-seat-link.service`, as root, at every boot.

use std::fs;
use std::io::Write;
use std::net::IpAddr;
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};

/// The files a link reads and writes.
pub struct Paths {
    /// What the installation recorded (`/etc/ocinye/release.env`).
    pub release_env: PathBuf,
    /// The Instance's public session key (`/etc/ocinye/instance-session.pub`).
    pub key: PathBuf,
    /// The Workspace's address (`/etc/ocinye/workspace-url`).
    pub workspace_url: PathBuf,
    /// The host table (`/etc/hosts`).
    pub hosts: PathBuf,
}

impl Paths {
    /// The paths on an installed machine.
    #[must_use]
    pub fn system() -> Self {
        Self {
            release_env: "/etc/ocinye/release.env".into(),
            key: "/etc/ocinye/instance-session.pub".into(),
            workspace_url: "/etc/ocinye/workspace-url".into(),
            hosts: "/etc/hosts".into(),
        }
    }
}

/// How a link ended well.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Linked {
    /// The Core answered: the key on disk is the one it publishes now.
    Fresh,
    /// The Core did not answer, and the key learned at an earlier boot stays.
    Kept,
}

/// Why there is no link.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkError {
    /// The installation recorded no usable name for the Instance.
    NoInstance,
    /// The Core did not answer and no key was learned before.
    CoreUnreachable,
    /// A file could not be written.
    Write,
}

/// The line this module owns in the host table ends with this.
const MARK: &str = "# ocinye-seat";

/// A DNS name as the installation validates it: labels of letters, digits and
/// hyphens. Nothing else is ever written into the host table or an address.
fn is_name(host: &str) -> bool {
    !host.is_empty()
        && host.len() <= 253
        && host.split('.').all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        })
}

fn is_key(text: &str) -> bool {
    text.len() == 64 && text.bytes().all(|b| b.is_ascii_hexdigit())
}

fn write(path: &Path, content: &str) -> Result<(), LinkError> {
    let temporary = path.with_extension("ocinye-new");
    fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o644)
        .open(&temporary)
        .and_then(|mut f| f.write_all(content.as_bytes()).and_then(|()| f.sync_all()))
        .and_then(|()| fs::rename(&temporary, path))
        .map_err(|_| LinkError::Write)
}

/// The host table with this machine's Instance name pointing at the loopback
/// interface: every line this module wrote before is replaced, every other
/// line is kept as it is.
fn hosts_with(current: &str, host: &str) -> String {
    let mut out: Vec<&str> = current
        .lines()
        .filter(|l| !l.trim_end().ends_with(MARK))
        .collect();
    let line = format!("127.0.0.1 {host} {MARK}");
    out.push(&line);
    let mut text = out.join("\n");
    text.push('\n');
    text
}

/// Link the seat. `instance_key` asks the Core for its public session key.
///
/// # Errors
///
/// See [`LinkError`].
pub fn link(
    paths: &Paths,
    instance_key: impl FnOnce() -> Option<String>,
) -> Result<Linked, LinkError> {
    let recorded = fs::read_to_string(&paths.release_env).map_err(|_| LinkError::NoInstance)?;
    let host = recorded
        .lines()
        .find_map(|l| l.strip_prefix("OCINYE_PROXY_HEALTH_HOST="))
        .map(|h| h.trim().to_ascii_lowercase())
        .ok_or(LinkError::NoInstance)?;
    let address = host.parse::<IpAddr>().is_ok();
    if !address && !is_name(&host) {
        return Err(LinkError::NoInstance);
    }

    let outcome = match instance_key().filter(|k| is_key(k)) {
        Some(key) => {
            write(&paths.key, &format!("{}\n", key.to_ascii_lowercase()))?;
            Linked::Fresh
        }
        None => {
            let kept = fs::read_to_string(&paths.key).is_ok_and(|k| is_key(k.trim()));
            if !kept {
                return Err(LinkError::CoreUnreachable);
            }
            Linked::Kept
        }
    };
    // A name resolves to this machine without asking anyone; an address is
    // already where it is.
    if !address {
        let current = fs::read_to_string(&paths.hosts).unwrap_or_default();
        let wanted = hosts_with(&current, &host);
        if wanted != current {
            write(&paths.hosts, &wanted)?;
        }
    }
    let url = if host.contains(':') {
        format!("https://[{host}]/\n")
    } else {
        format!("https://{host}/\n")
    };
    write(&paths.workspace_url, &url)?;
    Ok(outcome)
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEY: &str = "ea4a6c63e29c520abef5507b132ec5f9954776aebebe7b92421eea691446d22c";

    fn bench(release_env: Option<&str>) -> (Paths, PathBuf) {
        use std::sync::atomic::{AtomicU32, Ordering};
        static N: AtomicU32 = AtomicU32::new(0);
        let dir = std::env::temp_dir().join(format!(
            "ocinye-link-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::SeqCst)
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        if let Some(text) = release_env {
            fs::write(dir.join("release.env"), text).unwrap();
        }
        fs::write(
            dir.join("hosts"),
            "127.0.0.1 localhost\n::1 ip6-localhost\n",
        )
        .unwrap();
        let paths = Paths {
            release_env: dir.join("release.env"),
            key: dir.join("instance-session.pub"),
            workspace_url: dir.join("workspace-url"),
            hosts: dir.join("hosts"),
        };
        (paths, dir)
    }

    const INSTALLED: &str =
        "OCINYE_RELEASE_SHA=abc\nOCINYE_PROXY_HEALTH_HOST=ocinye.empresa.example\n";

    #[test]
    fn com_o_core_a_responder_ficam_a_chave_o_endereco_e_o_nome_local() {
        let (p, _) = bench(Some(INSTALLED));
        assert_eq!(link(&p, || Some(KEY.to_owned())), Ok(Linked::Fresh));
        assert_eq!(fs::read_to_string(&p.key).unwrap(), format!("{KEY}\n"));
        assert_eq!(
            fs::read_to_string(&p.workspace_url).unwrap(),
            "https://ocinye.empresa.example/\n"
        );
        assert_eq!(
            fs::read_to_string(&p.hosts).unwrap(),
            "127.0.0.1 localhost\n::1 ip6-localhost\n127.0.0.1 ocinye.empresa.example # ocinye-seat\n"
        );
    }

    #[test]
    fn ligar_outra_vez_nao_acumula_linhas_e_um_nome_novo_substitui_o_antigo() {
        let (p, _) = bench(Some(INSTALLED));
        link(&p, || Some(KEY.to_owned())).unwrap();
        link(&p, || Some(KEY.to_owned())).unwrap();
        fs::write(&p.release_env, "OCINYE_PROXY_HEALTH_HOST=novo.example\n").unwrap();
        link(&p, || Some(KEY.to_owned())).unwrap();
        let hosts = fs::read_to_string(&p.hosts).unwrap();
        assert_eq!(hosts.matches(MARK).count(), 1, "{hosts}");
        assert!(hosts.contains("127.0.0.1 novo.example # ocinye-seat"));
        assert!(hosts.starts_with("127.0.0.1 localhost\n::1 ip6-localhost\n"));
    }

    #[test]
    fn sem_core_fica_a_chave_de_antes_e_sem_chave_nenhuma_nao_ha_ligacao() {
        let (p, _) = bench(Some(INSTALLED));
        assert_eq!(link(&p, || None), Err(LinkError::CoreUnreachable));
        assert!(!p.key.exists() && !p.workspace_url.exists());
        link(&p, || Some(KEY.to_owned())).unwrap();
        assert_eq!(link(&p, || None), Ok(Linked::Kept));
        assert_eq!(fs::read_to_string(&p.key).unwrap(), format!("{KEY}\n"));
        // What the Core says is a key or it is nothing.
        assert_eq!(link(&p, || Some("not-a-key".to_owned())), Ok(Linked::Kept));
        assert_eq!(fs::read_to_string(&p.key).unwrap(), format!("{KEY}\n"));
    }

    #[test]
    fn sem_instancia_instalada_ou_com_um_nome_que_nao_e_um_nome_nada_e_escrito() {
        for recorded in [
            None,
            Some("OCINYE_RELEASE_SHA=abc\n"),
            Some("OCINYE_PROXY_HEALTH_HOST=\n"),
            Some("OCINYE_PROXY_HEALTH_HOST=a b\n"),
            Some("OCINYE_PROXY_HEALTH_HOST=evil.example # x\n"),
            Some("OCINYE_PROXY_HEALTH_HOST=-bad.example\n"),
            Some("OCINYE_PROXY_HEALTH_HOST=a..b\n"),
        ] {
            let (p, _) = bench(recorded);
            assert_eq!(
                link(&p, || Some(KEY.to_owned())),
                Err(LinkError::NoInstance),
                "{recorded:?}"
            );
            assert!(!p.key.exists() && !p.workspace_url.exists());
            assert_eq!(
                fs::read_to_string(&p.hosts).unwrap(),
                "127.0.0.1 localhost\n::1 ip6-localhost\n"
            );
        }
    }

    #[test]
    fn um_endereco_nao_entra_na_tabela_de_nomes() {
        let (p, _) = bench(Some("OCINYE_PROXY_HEALTH_HOST=192.168.1.84\n"));
        assert_eq!(link(&p, || Some(KEY.to_owned())), Ok(Linked::Fresh));
        assert_eq!(
            fs::read_to_string(&p.workspace_url).unwrap(),
            "https://192.168.1.84/\n"
        );
        assert!(!fs::read_to_string(&p.hosts).unwrap().contains(MARK));
    }
}
