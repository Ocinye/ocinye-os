//! Server facts (read-only): what the Installer shows after «Testar ligação».

use std::fs;
use std::path::Path;

use ocinye_installer_contracts::manifest::Arch;
use ocinye_installer_contracts::preflight::Privilege;
use ocinye_installer_contracts::protocol::{OsRelease, ServerFacts};
use serde::Deserialize;

use crate::exec;

/// `/etc/os-release` → id, version, pretty name.
#[must_use]
pub fn parse_os_release(text: &str) -> OsRelease {
    let get = |key: &str| {
        text.lines()
            .find_map(|l| l.strip_prefix(&format!("{key}=")))
            .map(|v| v.trim().trim_matches('"').to_owned())
            .unwrap_or_default()
    };
    OsRelease {
        id: get("ID"),
        version_id: get("VERSION_ID"),
        pretty: get("PRETTY_NAME"),
    }
}

/// The effective uid, from `/proc/self/status`.
#[must_use]
pub fn effective_uid() -> Option<u32> {
    let status = fs::read_to_string("/proc/self/status").ok()?;
    status
        .lines()
        .find_map(|l| l.strip_prefix("Uid:"))
        .and_then(|v| v.split_whitespace().nth(1))
        .and_then(|v| v.parse().ok())
}

#[derive(Deserialize)]
struct IpAddr {
    family: String,
    local: String,
    scope: String,
}

#[derive(Deserialize)]
struct IpLink {
    #[serde(default)]
    addr_info: Vec<IpAddr>,
}

/// `ip -j addr` → global (IPv4, IPv6) addresses.
#[must_use]
pub fn parse_ip_json(json: &str) -> (Vec<String>, Vec<String>) {
    let links: Vec<IpLink> = serde_json::from_str(json).unwrap_or_default();
    let mut v4 = Vec::new();
    let mut v6 = Vec::new();
    for a in links.iter().flat_map(|l| &l.addr_info) {
        if a.scope != "global" {
            continue;
        }
        match a.family.as_str() {
            "inet" => v4.push(a.local.clone()),
            "inet6" => v6.push(a.local.clone()),
            _ => {}
        }
    }
    (v4, v6)
}

/// `uname -m`.
#[must_use]
pub fn machine() -> String {
    exec::run("/usr/bin/uname", &["-m"], exec::secs(5))
        .stdout
        .trim()
        .to_owned()
}

/// Read the facts.
#[must_use]
pub fn read() -> ServerFacts {
    let os = parse_os_release(&fs::read_to_string("/etc/os-release").unwrap_or_default());
    let machine = machine();
    let (ipv4, ipv6) =
        parse_ip_json(&exec::run("/usr/sbin/ip", &["-j", "addr"], exec::secs(5)).stdout);
    ServerFacts {
        hostname: fs::read_to_string("/proc/sys/kernel/hostname")
            .unwrap_or_default()
            .trim()
            .to_owned(),
        os,
        kernel_name: fs::read_to_string("/proc/sys/kernel/ostype")
            .unwrap_or_default()
            .trim()
            .to_owned(),
        kernel_release: fs::read_to_string("/proc/sys/kernel/osrelease")
            .unwrap_or_default()
            .trim()
            .to_owned(),
        arch: Arch::from_uname(&machine),
        machine,
        privilege: if effective_uid() == Some(0) {
            Privilege::Root
        } else {
            Privilege::None
        },
        systemd: Path::new("/run/systemd/system").is_dir(),
        ipv4,
        ipv6,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn o_os_release_do_ubuntu_le_se() {
        let t = "PRETTY_NAME=\"Ubuntu 24.04.1 LTS\"\nNAME=\"Ubuntu\"\nVERSION_ID=\"24.04\"\nID=ubuntu\nID_LIKE=debian\n";
        let o = parse_os_release(t);
        assert_eq!(o.id, "ubuntu");
        assert_eq!(o.version_id, "24.04");
        assert_eq!(o.pretty, "Ubuntu 24.04.1 LTS");
    }

    #[test]
    fn so_os_enderecos_globais_contam() {
        let j = r#"[{"ifname":"lo","addr_info":[{"family":"inet","local":"127.0.0.1","scope":"host"}]},
                   {"ifname":"eth0","addr_info":[{"family":"inet","local":"192.0.2.10","scope":"global"},
                   {"family":"inet6","local":"2001:db8::10","scope":"global"},
                   {"family":"inet6","local":"fe80::1","scope":"link"}]}]"#;
        let (v4, v6) = parse_ip_json(j);
        assert_eq!(v4, ["192.0.2.10"]);
        assert_eq!(v6, ["2001:db8::10"]);
        assert_eq!(parse_ip_json("not json"), (vec![], vec![]));
    }
}
