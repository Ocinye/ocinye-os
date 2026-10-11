//! The TypeScript bindings are generated from the Rust contracts and checked
//! in. This test regenerates them and fails on any difference, so a contract
//! cannot change without its bindings, and nobody edits them by hand.
//!
//! To update after a deliberate contract change:
//!   OCINYE_UPDATE_BINDINGS=1 cargo test -p ocinye-system-contracts --test bindings

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use ocinye_system_contracts::capability::CapabilitySpec;
use ocinye_system_contracts::wire::{ClientFrame, ServerFrame};
use ts_rs::{Config, TS};

fn tree(dir: &Path) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    let Ok(entries) = fs::read_dir(dir) else {
        return out;
    };
    for e in entries.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        if name.ends_with(".ts") {
            // The generator leaves trailing blanks; the repository does not
            // keep them, so both sides are compared without.
            let text = fs::read_to_string(e.path()).unwrap();
            let clean: Vec<&str> = text.lines().map(str::trim_end).collect();
            out.insert(name, clean.join("\n") + "\n");
        }
    }
    out
}

#[test]
fn as_ligacoes_typescript_sao_as_geradas_dos_contratos() {
    let committed = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("bindings");
    let fresh = std::env::temp_dir().join(format!("ocinye-bindings-{}", std::process::id()));
    let _ = fs::remove_dir_all(&fresh);
    fs::create_dir_all(&fresh).unwrap();
    // Everything a client can send or receive hangs off these three roots.
    // Every integer in the contract fits a JavaScript number.
    let cfg = Config::new().with_out_dir(&fresh).with_large_int("number");
    ClientFrame::export_all(&cfg).unwrap();
    ServerFrame::export_all(&cfg).unwrap();
    CapabilitySpec::export_all(&cfg).unwrap();
    let generated = tree(&fresh);
    assert!(
        generated.len() > 40,
        "the generator produced {} files",
        generated.len()
    );

    if std::env::var_os("OCINYE_UPDATE_BINDINGS").is_some() {
        let _ = fs::remove_dir_all(&committed);
        fs::create_dir_all(&committed).unwrap();
        for (name, text) in &generated {
            fs::write(committed.join(name), text).unwrap();
        }
    }
    let _ = fs::remove_dir_all(&fresh);
    let on_disk = tree(&committed);
    let stale: Vec<&String> = generated
        .iter()
        .filter(|(n, t)| on_disk.get(*n) != Some(*t))
        .map(|(n, _)| n)
        .collect();
    let orphan: Vec<&String> = on_disk
        .keys()
        .filter(|n| !generated.contains_key(*n))
        .collect();
    assert!(
        stale.is_empty() && orphan.is_empty(),
        "bindings differ from the contracts (stale or missing: {stale:?}; no longer generated: {orphan:?}); \
         regenerate with OCINYE_UPDATE_BINDINGS=1"
    );
}
