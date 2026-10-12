//! Writing a system file so that nobody ever reads half of it.
//!
//! Every file this service writes for another program — a netplan
//! configuration, an iwd network, a udev rule, the ALSA default — is written
//! next to its destination under a temporary name, with its final mode from
//! the first byte, synced, and then renamed over the old one. A reader sees
//! the old file or the new one, never a mixture; a crash leaves at most a
//! temporary file that no program reads.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;

use ocinye_system_contracts::reason::{AdapterFailure, Reason};

fn refused() -> Reason {
    Reason::AdapterFailed {
        cause: AdapterFailure::Refused,
    }
}

/// Replace `path` with `bytes`, mode `mode`, atomically.
///
/// # Errors
///
/// [`AdapterFailure::Refused`] when the directory cannot be written.
pub fn write_atomic(path: &Path, bytes: &[u8], mode: u32) -> Result<(), Reason> {
    let dir = path.parent().ok_or_else(refused)?;
    fs::create_dir_all(dir).map_err(|_| refused())?;
    let name = path.file_name().ok_or_else(refused)?.to_string_lossy();
    // A dot first: no program that reads `*.yaml`, `*.rules` or `*.psk` in
    // this directory picks the temporary file up.
    let tmp = dir.join(format!(".{name}.ocinye-new"));
    let _ = fs::remove_file(&tmp);
    let written = (|| {
        let mut f = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(mode)
            .open(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
        fs::rename(&tmp, path)
    })();
    if written.is_err() {
        let _ = fs::remove_file(&tmp);
        return Err(refused());
    }
    // The rename is durable once the directory is.
    if let Ok(d) = fs::File::open(dir) {
        let _ = d.sync_all();
    }
    Ok(())
}

/// Remove `path`; a file that is already gone is not an error.
///
/// # Errors
///
/// [`AdapterFailure::Refused`] when it exists and cannot be removed.
pub fn remove(path: &Path) -> Result<(), Reason> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(_) => Err(refused()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn escreve_com_o_modo_final_e_substitui_sem_deixar_temporarios() {
        let dir = std::env::temp_dir().join(format!("ocinye-files-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let p = dir.join("sub/90-ocinye-enp1s0.yaml");
        write_atomic(&p, b"one", 0o600).unwrap();
        write_atomic(&p, b"two", 0o600).unwrap();
        assert_eq!(fs::read(&p).unwrap(), b"two");
        assert_eq!(
            fs::metadata(&p).unwrap().permissions().mode() & 0o777,
            0o600
        );
        let names: Vec<_> = fs::read_dir(dir.join("sub"))
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(names.len(), 1, "{names:?}");
        remove(&p).unwrap();
        remove(&p).unwrap();
        assert!(!p.exists());
        let _ = fs::remove_dir_all(&dir);
    }
}
