//! Extracting an archive without letting it out of its directory.
//!
//! Only regular files and directories are accepted: no symbolic or hard link,
//! no device, no FIFO. Every path is relative and made of plain components.
//! Sizes are bounded per file, in total and by count, and a file is never
//! allowed more bytes than its header declared. The destination must not exist
//! yet; a partial extraction is removed.

use crate::{err, Error, Result};
use std::collections::BTreeSet;
use std::io::Read;
use std::path::{Component, Path, PathBuf};

/// What an archive may contain.
#[derive(Debug, Clone, Copy)]
pub struct Limits {
    /// Files and directories.
    pub max_entries: usize,
    /// Bytes of one file.
    pub max_file_bytes: u64,
    /// Bytes of all files.
    pub max_total_bytes: u64,
}

impl Limits {
    /// For a native layer: binaries, units, interface.
    pub const NATIVE: Self = Self {
        max_entries: 512,
        max_file_bytes: 512 << 20,
        max_total_bytes: 2 << 30,
    };
    /// For an offline bundle: a whole release with its images.
    pub const OFFLINE: Self = Self {
        max_entries: 4096,
        max_file_bytes: 16 << 30,
        max_total_bytes: 32 << 30,
    };
}

/// What was extracted.
#[derive(Debug, Clone, Default)]
pub struct Extracted {
    /// Relative paths of the regular files.
    pub files: Vec<String>,
    /// Bytes written.
    pub bytes: u64,
}

/// A path component is letters, digits, `.`, `_`, `-`, `+`, `@` and not `.`/`..`.
fn plain(component: &str) -> bool {
    !component.is_empty()
        && component != "."
        && component != ".."
        && component.len() <= 128
        && component
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-' | b'+' | b'@'))
}

fn relative(path: &Path) -> Result<String> {
    let mut parts = Vec::new();
    for c in path.components() {
        match c {
            Component::Normal(s) => {
                let s = s
                    .to_str()
                    .ok_or_else(|| Error("archive path is not UTF-8".into()))?;
                if !plain(s) {
                    return err(format!("archive path component {s:?} is not plain"));
                }
                parts.push(s);
            }
            Component::CurDir => {}
            _ => return err("archive path is absolute or goes up"),
        }
    }
    if parts.is_empty() || parts.len() > 16 {
        return err("archive path is empty or too deep");
    }
    Ok(parts.join("/"))
}

/// Extract `reader` (a tar stream) into `dest`, which must not exist.
pub fn extract(reader: impl Read, dest: &Path, limits: Limits) -> Result<Extracted> {
    if dest.exists() {
        return err(format!("{} already exists", dest.display()));
    }
    std::fs::create_dir_all(dest)?;
    match extract_into(reader, dest, limits) {
        Ok(x) => Ok(x),
        Err(e) => {
            let _ = std::fs::remove_dir_all(dest);
            Err(e)
        }
    }
}

fn extract_into(reader: impl Read, dest: &Path, limits: Limits) -> Result<Extracted> {
    use std::os::unix::fs::PermissionsExt;
    let mut archive = tar::Archive::new(reader);
    let mut out = Extracted::default();
    let mut seen = BTreeSet::new();
    let mut entries = 0usize;
    for entry in archive.entries()? {
        let mut entry = entry?;
        entries += 1;
        if entries > limits.max_entries {
            return err("archive has too many entries");
        }
        let kind = entry.header().entry_type();
        let rel = relative(&entry.path()?)?;
        if !seen.insert(rel.clone()) {
            return err(format!("archive lists {rel} twice"));
        }
        let target: PathBuf = dest.join(&rel);
        if kind.is_dir() {
            std::fs::create_dir_all(&target)?;
            std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o755))?;
            continue;
        }
        if !kind.is_file() {
            return err(format!("{rel} is a link or a special file"));
        }
        let declared = entry.header().size()?;
        if declared > limits.max_file_bytes {
            return err(format!("{rel} is larger than allowed"));
        }
        out.bytes = out
            .bytes
            .checked_add(declared)
            .filter(|t| *t <= limits.max_total_bytes)
            .ok_or_else(|| Error("archive is larger than allowed".into()))?;
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mode = entry.header().mode().unwrap_or(0o644);
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&target)?;
        let copied = std::io::copy(&mut (&mut entry).take(declared + 1), &mut file)?;
        if copied != declared {
            return err(format!("{rel} does not hold the bytes its header declares"));
        }
        file.sync_all()?;
        let perm = if mode & 0o111 != 0 { 0o755 } else { 0o644 };
        std::fs::set_permissions(&target, std::fs::Permissions::from_mode(perm))?;
        out.files.push(rel);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tar_with(build: impl FnOnce(&mut tar::Builder<Vec<u8>>)) -> Vec<u8> {
        let mut b = tar::Builder::new(Vec::new());
        build(&mut b);
        b.into_inner().unwrap()
    }

    fn file(b: &mut tar::Builder<Vec<u8>>, path: &str, data: &[u8]) {
        let mut h = tar::Header::new_gnu();
        h.set_size(data.len() as u64);
        h.set_mode(0o755);
        h.set_entry_type(tar::EntryType::Regular);
        h.set_cksum();
        b.append_data(&mut h, path, data).unwrap();
    }

    fn raw_path(
        b: &mut tar::Builder<Vec<u8>>,
        path: &str,
        kind: tar::EntryType,
        link: Option<&str>,
    ) {
        let mut h = tar::Header::new_gnu();
        h.set_size(0);
        h.set_entry_type(kind);
        {
            let gnu = h.as_gnu_mut().unwrap();
            gnu.name[..path.len()].copy_from_slice(path.as_bytes());
            if let Some(l) = link {
                gnu.linkname[..l.len()].copy_from_slice(l.as_bytes());
            }
        }
        h.set_cksum();
        b.append(&h, std::io::empty()).unwrap();
    }

    #[test]
    fn extrai_ficheiros_simples() {
        let t = tempfile::tempdir().unwrap();
        let data = tar_with(|b| file(b, "bin/ocinye-system-service", b"\x7fELF"));
        let x = extract(&data[..], &t.path().join("out"), Limits::NATIVE).unwrap();
        assert_eq!(x.files, vec!["bin/ocinye-system-service".to_owned()]);
        assert_eq!(
            std::fs::read(t.path().join("out/bin/ocinye-system-service")).unwrap(),
            b"\x7fELF"
        );
    }

    #[test]
    fn recusa_saidas_ligacoes_e_bombas() {
        let t = tempfile::tempdir().unwrap();
        let cases: Vec<(&str, Vec<u8>)> = vec![
            (
                "dot-dot",
                tar_with(|b| raw_path(b, "../escape", tar::EntryType::Regular, None)),
            ),
            (
                "absolute",
                tar_with(|b| raw_path(b, "/etc/passwd", tar::EntryType::Regular, None)),
            ),
            (
                "symlink",
                tar_with(|b| raw_path(b, "x", tar::EntryType::Symlink, Some("/etc/shadow"))),
            ),
            (
                "hardlink",
                tar_with(|b| raw_path(b, "x", tar::EntryType::Link, Some("/etc/shadow"))),
            ),
            (
                "device",
                tar_with(|b| raw_path(b, "x", tar::EntryType::Char, None)),
            ),
            (
                "twice",
                tar_with(|b| {
                    file(b, "a", b"1");
                    file(b, "a", b"2");
                }),
            ),
            ("odd name", tar_with(|b| file(b, "a b", b"1"))),
        ];
        for (name, data) in cases {
            let dest = t.path().join(name.replace(' ', "_"));
            assert!(
                extract(&data[..], &dest, Limits::NATIVE).is_err(),
                "{name} accepted"
            );
            assert!(!dest.exists(), "{name} left a partial extraction");
        }
        let big = tar_with(|b| file(b, "big", &vec![0u8; 4096]));
        let small = Limits {
            max_entries: 10,
            max_file_bytes: 1024,
            max_total_bytes: 1024,
        };
        assert!(extract(&big[..], &t.path().join("big"), small).is_err());
        let many = tar_with(|b| {
            for i in 0..20 {
                file(b, &format!("f{i}"), b"x")
            }
        });
        let few = Limits {
            max_entries: 10,
            max_file_bytes: 1024,
            max_total_bytes: 1 << 20,
        };
        assert!(extract(&many[..], &t.path().join("many"), few).is_err());
    }

    #[test]
    fn recusa_destino_existente() {
        let t = tempfile::tempdir().unwrap();
        let data = tar_with(|b| file(b, "a", b"1"));
        assert!(extract(&data[..], t.path(), Limits::NATIVE).is_err());
    }
}
