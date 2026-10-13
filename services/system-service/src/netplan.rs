//! Addressing of a wired interface, through netplan (`network.configure`).
//!
//! The service writes one file per interface, `/etc/netplan/90-ocinye-IFACE.yaml`
//! (0600, atomically), from the validated request, and runs `netplan apply`.
//! When the apply fails, the file is put back as it was and netplan applied
//! again: the machine keeps the addressing it had.
//!
//! The definition's identifier starts with `00-`. netplan names the
//! systemd-networkd file after it (`10-netplan-ID.network`) and networkd
//! uses the first file, in lexical order, that matches an interface; the
//! image's own `all-en`/`all-eth` DHCP definitions would otherwise win.

use std::net::Ipv4Addr;
use std::path::{Path, PathBuf};

use ocinye_system_contracts::reason::Reason;
use ocinye_system_contracts::request::Addressing;
use zeroize::Zeroizing;

use crate::files;
use crate::tool::{Runner, Tool};

/// Where the service's file for `interface` lives under `netplan`.
#[must_use]
pub fn file(netplan: &Path, interface: &str) -> PathBuf {
    netplan.join(format!("90-ocinye-{interface}.yaml"))
}

fn network(address: Ipv4Addr, prefix: u8) -> u32 {
    let mask = if prefix == 0 {
        0
    } else {
        u32::MAX << (32 - u32::from(prefix))
    };
    u32::from(address) & mask
}

/// The file for an interface. Every value was validated by the contract
/// (an interface name of letters, digits, `-`, `_`, `.`; dotted quads; a
/// prefix of 1 to 32) and is quoted anyway. A static address whose gateway
/// is not on its own network, or is the address itself, is refused.
///
/// # Errors
///
/// [`Reason::ValueOutOfRange`].
pub fn render(interface: &str, addressing: &Addressing) -> Result<String, Reason> {
    let mut y = String::new();
    y.push_str("# Ocinye OS: written by the System Service (network.configure).\n");
    y.push_str("# Replaced on the next change; edit nothing here.\n");
    y.push_str("network:\n  version: 2\n  renderer: networkd\n  ethernets:\n");
    y.push_str(&format!("    \"00-ocinye-{interface}\":\n"));
    y.push_str(&format!("      match:\n        name: \"{interface}\"\n"));
    y.push_str("      optional: true\n      dhcp6: false\n");
    match addressing {
        Addressing::Dhcp => y.push_str("      dhcp4: true\n"),
        Addressing::Static {
            address,
            prefix,
            gateway,
            dns,
        } => {
            let a: Ipv4Addr = address.parse().map_err(|_| Reason::ValueOutOfRange)?;
            let g: Ipv4Addr = gateway.parse().map_err(|_| Reason::ValueOutOfRange)?;
            if !(1..=32).contains(prefix) || a == g || network(a, *prefix) != network(g, *prefix) {
                return Err(Reason::ValueOutOfRange);
            }
            y.push_str("      dhcp4: false\n");
            y.push_str(&format!("      addresses: [\"{a}/{prefix}\"]\n"));
            y.push_str(&format!(
                "      routes:\n        - to: default\n          via: \"{g}\"\n"
            ));
            if !dns.is_empty() {
                let list: Vec<String> = dns
                    .iter()
                    .map(|d| {
                        d.parse::<Ipv4Addr>()
                            .map(|d| format!("\"{d}\""))
                            .map_err(|_| Reason::ValueOutOfRange)
                    })
                    .collect::<Result<_, _>>()?;
                y.push_str(&format!(
                    "      nameservers:\n        addresses: [{}]\n",
                    list.join(", ")
                ));
            }
        }
    }
    Ok(y)
}

/// Write the file and apply it; on failure, restore and apply again.
///
/// # Errors
///
/// A typed reason; the machine is as it was.
pub fn configure(
    runner: &dyn Runner,
    netplan: &Path,
    interface: &str,
    addressing: &Addressing,
) -> Result<(), Reason> {
    let body = render(interface, addressing)?;
    let path = file(netplan, interface);
    let previous = std::fs::read(&path).ok().map(Zeroizing::new);
    files::write_atomic(&path, body.as_bytes(), 0o600)?;
    if let Err(cause) = runner.run(Tool::Netplan, &["apply"]) {
        let _ = match &previous {
            Some(bytes) => files::write_atomic(&path, bytes, 0o600),
            None => files::remove(&path),
        };
        let _ = runner.run(Tool::Netplan, &["apply"]);
        return Err(Reason::AdapterFailed { cause });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tool::fake::Script;
    use ocinye_system_contracts::reason::AdapterFailure;

    fn fixed(gateway: &str) -> Addressing {
        Addressing::Static {
            address: "192.168.1.84".into(),
            prefix: 24,
            gateway: gateway.into(),
            dns: vec!["1.1.1.1".into(), "9.9.9.9".into()],
        }
    }

    #[test]
    fn o_ficheiro_por_dhcp_e_fixo() {
        let d = render("enp3s0", &Addressing::Dhcp).unwrap();
        assert!(d.contains("\"00-ocinye-enp3s0\":\n      match:\n        name: \"enp3s0\"\n"));
        assert!(d.contains("dhcp4: true"));
        let s = render("enp3s0", &fixed("192.168.1.1")).unwrap();
        assert!(s.contains("dhcp4: false"));
        assert!(s.contains("addresses: [\"192.168.1.84/24\"]"));
        assert!(s.contains("via: \"192.168.1.1\""));
        assert!(s.contains("addresses: [\"1.1.1.1\", \"9.9.9.9\"]"));
        assert_eq!(
            render("enp3s0", &fixed("10.0.0.1")),
            Err(Reason::ValueOutOfRange)
        );
        assert_eq!(
            render("enp3s0", &fixed("192.168.1.84")),
            Err(Reason::ValueOutOfRange)
        );
    }

    #[test]
    fn um_apply_que_falha_repoe_o_ficheiro_anterior() {
        let dir = std::env::temp_dir().join(format!("ocinye-netplan-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let s = Script::default();
        configure(&s, &dir, "enp3s0", &Addressing::Dhcp).unwrap();
        let path = file(&dir, "enp3s0");
        let before = std::fs::read_to_string(&path).unwrap();
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        s.fail(Tool::Netplan, &["apply"], AdapterFailure::Refused);
        assert_eq!(
            configure(&s, &dir, "enp3s0", &fixed("192.168.1.1")),
            Err(Reason::AdapterFailed {
                cause: AdapterFailure::Refused
            })
        );
        assert_eq!(std::fs::read_to_string(&path).unwrap(), before);
        // A first file that fails is removed, not left half-applied.
        assert_eq!(
            configure(&s, &dir, "enp4s0", &Addressing::Dhcp),
            Err(Reason::AdapterFailed {
                cause: AdapterFailure::Refused
            })
        );
        assert!(!file(&dir, "enp4s0").exists());
        // Each failure applied again what was restored.
        let applies = s
            .changes()
            .iter()
            .filter(|(t, _)| *t == Tool::Netplan)
            .count();
        assert_eq!(applies, 5);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
