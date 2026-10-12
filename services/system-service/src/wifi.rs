//! Wi-Fi, through iwd over D-Bus (`net.connman.iwd`).
//!
//! iwd associates and authenticates; systemd-networkd gives the address
//! (`EnableNetworkConfiguration=false` on the image). This adapter only asks
//! iwd, and writes one kind of file: a network's passphrase, in iwd's own
//! provisioning format, under `/var/lib/iwd`, which iwd watches.
//!
//! The passphrase is written once, mode 0600, atomically, and the buffer
//! that held it is zeroed. It is never logged, never part of an error and
//! never read back. When a connection with a new passphrase fails, the file
//! is put back as it was: a credential the network refused is not kept.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use ocinye_system_contracts::reason::{AdapterFailure, Reason, Unavailable};
use ocinye_system_contracts::state::{Percent, WifiNetwork, WifiScan, WifiSecurity};
use zeroize::Zeroizing;

use crate::dbus::{self, Arg, Bus, BusError, Objects};
use crate::files;
use crate::linux::Timing;

/// The service on the bus.
pub const IWD: &str = "net.connman.iwd";
const DEVICE: &str = "net.connman.iwd.Device";
const STATION: &str = "net.connman.iwd.Station";
const NETWORK: &str = "net.connman.iwd.Network";
const KNOWN: &str = "net.connman.iwd.KnownNetwork";

/// The radio iwd manages: its device, and its station when it is one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Radio {
    /// The device's object path.
    pub path: String,
    /// Whether iwd has it powered.
    pub powered: bool,
    /// Whether it is in station mode (the only mode v1 uses).
    pub station: bool,
    /// Whether a scan is running.
    pub scanning: bool,
    /// The name of the network it is connected to.
    pub connected: Option<String>,
}

/// The first iwd device, by path order.
#[must_use]
pub fn radio(objects: &Objects) -> Option<Radio> {
    let (path, device) = dbus::with(objects, DEVICE).into_iter().next()?;
    let station = objects.get(path).and_then(|i| i.get(STATION));
    let connected = station
        .filter(|s| dbus::text(s, "State") == Some("connected"))
        .and_then(|s| dbus::path(s, "ConnectedNetwork"))
        .and_then(|n| dbus::prop(objects, n, NETWORK, "Name"))
        .and_then(|v| match v {
            dbus::Value::Str(s) => Some(s.clone()),
            _ => None,
        });
    Some(Radio {
        path: path.to_owned(),
        powered: dbus::flag(device, "Powered"),
        station: station.is_some(),
        scanning: station.is_some_and(|s| dbus::flag(s, "Scanning")),
        connected,
    })
}

/// Signal strength from iwd (hundredths of a dBm) as a percentage:
/// -90 dBm and below is nothing, -30 dBm and above is everything.
#[must_use]
pub fn signal(centi_dbm: i16) -> Percent {
    let dbm = i32::from(centi_dbm) / 100;
    let pct = u8::try_from(((dbm + 90) * 100 / 60).clamp(0, 100)).unwrap_or(0);
    Percent::saturating(pct)
}

fn security(kind: &str) -> Option<WifiSecurity> {
    match kind {
        "open" => Some(WifiSecurity::Open),
        "psk" => Some(WifiSecurity::Passphrase),
        "8021x" => Some(WifiSecurity::Enterprise),
        // WEP and anything newer than this build: not offered.
        _ => None,
    }
}

/// The networks in range, from iwd's ordered list and its objects: strongest
/// first, one row per name.
#[must_use]
pub fn networks(objects: &Objects, ordered: &[(String, i16)]) -> Vec<WifiNetwork> {
    let mut out: Vec<WifiNetwork> = Vec::new();
    for (path, strength) in ordered {
        let Some(n) = objects.get(path).and_then(|i| i.get(NETWORK)) else {
            continue;
        };
        let (Some(name), Some(kind)) = (dbus::text(n, "Name"), dbus::text(n, "Type")) else {
            continue;
        };
        let Some(security) = security(kind) else {
            continue;
        };
        if name.is_empty() || out.iter().any(|w| w.ssid == name) {
            continue;
        }
        out.push(WifiNetwork {
            ssid: name.to_owned(),
            security,
            signal: signal(*strength),
            known: dbus::path(n, "KnownNetwork").is_some(),
            connected: dbus::flag(n, "Connected"),
        });
    }
    out
}

/// The file iwd reads a network's settings from: the name itself when it is
/// only letters, digits, `-` and `_`, otherwise `=` and the name in hex
/// (`iwd.network(5)`), then the type.
#[must_use]
pub fn file_name(ssid: &str, kind: &str) -> String {
    let plain = !ssid.is_empty()
        && ssid
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_');
    if plain {
        format!("{ssid}.{kind}")
    } else {
        let hex: String = ssid.bytes().map(|b| format!("{b:02x}")).collect();
        format!("={hex}.{kind}")
    }
}

/// What a bus error means for Wi-Fi.
fn reason(e: BusError) -> Reason {
    match e {
        BusError::ServiceDown => Reason::HardwareUnavailable {
            cause: Unavailable::ServiceDown,
        },
        BusError::Timeout => Reason::AdapterFailed {
            cause: AdapterFailure::Timeout,
        },
        BusError::UnexpectedReply => Reason::AdapterFailed {
            cause: AdapterFailure::UnexpectedReply,
        },
        BusError::Method(name) => match name.as_str() {
            "net.connman.iwd.Busy" | "net.connman.iwd.InProgress" => Reason::OperationInProgress,
            "net.connman.iwd.NotFound" => Reason::NotFound,
            _ => Reason::AdapterFailed {
                cause: AdapterFailure::Refused,
            },
        },
    }
}

/// What a failed `Network.Connect` means, knowing whether credentials were
/// involved.
fn connect_reason(e: BusError, with_credentials: bool) -> Reason {
    match e {
        BusError::Timeout => Reason::WifiTimeout,
        BusError::Method(ref name) => match name.as_str() {
            "net.connman.iwd.Aborted" | "net.connman.iwd.Timeout" => Reason::WifiTimeout,
            "net.connman.iwd.Failed"
            | "net.connman.iwd.InvalidFormat"
            | "net.connman.iwd.NoAgent"
            | "net.connman.iwd.NotConfigured"
                if with_credentials =>
            {
                Reason::WifiAuthenticationFailed
            }
            _ => reason(e),
        },
        other => reason(other),
    }
}

/// The Wi-Fi adapter.
pub struct Wifi<'a> {
    /// The bus.
    pub bus: &'a dyn Bus,
    /// iwd's directory (`/var/lib/iwd`).
    pub store: &'a Path,
    /// How long to wait for things.
    pub timing: &'a Timing,
}

impl Wifi<'_> {
    fn objects(&self) -> Result<Objects, Reason> {
        self.bus.managed_objects(IWD).map_err(reason)
    }

    /// The radio, powered and a station, or why not.
    fn station(&self) -> Result<(Objects, Radio), Reason> {
        let objects = self.objects()?;
        let r = radio(&objects).ok_or(Reason::HardwareAbsent)?;
        if !r.powered {
            return Err(Reason::HardwareUnavailable {
                cause: Unavailable::SoftBlocked,
            });
        }
        if !r.station {
            return Err(Reason::AdapterFailed {
                cause: AdapterFailure::Refused,
            });
        }
        Ok((objects, r))
    }

    /// Whether iwd has the radio powered, and the network it is connected
    /// to. `None` when iwd does not answer or has no device.
    #[must_use]
    pub fn state(&self) -> Option<Radio> {
        radio(&self.bus.managed_objects(IWD).ok()?)
    }

    fn wait(&self, done: impl Fn(&Objects) -> bool, limit: Duration) -> Option<Objects> {
        let started = Instant::now();
        loop {
            let objects = self.bus.managed_objects(IWD).ok()?;
            if done(&objects) {
                return Some(objects);
            }
            if started.elapsed() >= limit {
                return None;
            }
            std::thread::sleep(self.timing.poll);
        }
    }

    fn scan_now(&self, radio: &Radio) -> Result<Objects, Reason> {
        match self.bus.call(
            IWD,
            &radio.path,
            STATION,
            "Scan",
            Arg::None,
            Duration::from_secs(15),
        ) {
            // A scan already running is the scan this one wanted.
            Ok(()) => {}
            Err(BusError::Method(n))
                if n == "net.connman.iwd.Busy" || n == "net.connman.iwd.InProgress" => {}
            Err(e) => return Err(reason(e)),
        }
        let path = radio.path.clone();
        let finished = move |o: &Objects| {
            !o.get(&path)
                .and_then(|i| i.get(STATION))
                .is_some_and(|s| dbus::flag(s, "Scanning"))
        };
        // A scan that does not finish in time still leaves what it found.
        match self.wait(finished, self.timing.scan_wait) {
            Some(objects) => Ok(objects),
            None => self.objects(),
        }
    }

    /// `wifi.scan`: scan, then list.
    ///
    /// # Errors
    ///
    /// A typed reason.
    pub fn scan(&self) -> Result<WifiScan, Reason> {
        let (_, r) = self.station()?;
        let objects = self.scan_now(&r)?;
        let ordered = self.bus.ordered_networks(&r.path).map_err(reason)?;
        Ok(WifiScan {
            networks: networks(&objects, &ordered),
        })
    }

    fn find<'o>(objects: &'o Objects, ssid: &str) -> Option<(&'o str, &'o dbus::Properties)> {
        dbus::with(objects, NETWORK)
            .into_iter()
            .find(|(_, n)| dbus::text(n, "Name") == Some(ssid))
    }

    /// `wifi.connect`.
    ///
    /// # Errors
    ///
    /// A typed reason; `WIFI_AUTHENTICATION_FAILED` when the network refused
    /// the passphrase, `WIFI_TIMEOUT` when it did not answer in time.
    pub fn connect(&self, ssid: &str, passphrase: Option<&str>) -> Result<(), Reason> {
        let (mut objects, r) = self.station()?;
        if Self::find(&objects, ssid).is_none() {
            objects = self.scan_now(&r)?;
        }
        let (path, network) = Self::find(&objects, ssid).ok_or(Reason::NotFound)?;
        let path = path.to_owned();
        let kind = dbus::text(network, "Type").unwrap_or("").to_owned();
        let known = dbus::path(network, "KnownNetwork").is_some();
        let mut provisioned: Option<Provisioned> = None;
        match (kind.as_str(), passphrase) {
            ("open", None) => {}
            ("psk", Some(p)) => {
                provisioned = Some(self.provision(ssid, p)?);
                // iwd reads the directory on its own; connect only once it
                // has, or it would ask for an agent this service never runs.
                let p2 = path.clone();
                if self
                    .wait(
                        move |o| dbus::prop(o, &p2, NETWORK, "KnownNetwork").is_some(),
                        self.timing.provision_wait,
                    )
                    .is_none()
                {
                    if let Some(prev) = provisioned.take() {
                        prev.undo();
                    }
                    return Err(Reason::AdapterFailed {
                        cause: AdapterFailure::Timeout,
                    });
                }
            }
            ("psk", None) if known => {}
            // A protected network nobody gave a passphrase for, or a
            // passphrase for an open one: nothing to attempt.
            ("psk" | "open", _) => return Err(Reason::ValueOutOfRange),
            // Enterprise authentication is not implemented in v1.
            _ => return Err(Reason::NotImplemented),
        }
        let with_credentials = kind == "psk";
        if let Err(e) = self.bus.call(
            IWD,
            &path,
            NETWORK,
            "Connect",
            Arg::None,
            self.timing.connect,
        ) {
            if let Some(prev) = provisioned {
                prev.undo();
            }
            return Err(connect_reason(e, with_credentials));
        }
        if let Some(prev) = provisioned {
            prev.keep();
        }
        Ok(())
    }

    /// `wifi.disconnect`. Not connected is already done.
    ///
    /// # Errors
    ///
    /// A typed reason.
    pub fn disconnect(&self) -> Result<(), Reason> {
        let (_, r) = self.station()?;
        if r.connected.is_none() {
            return Ok(());
        }
        match self.bus.call(
            IWD,
            &r.path,
            STATION,
            "Disconnect",
            Arg::None,
            Duration::from_secs(15),
        ) {
            Ok(()) => Ok(()),
            Err(BusError::Method(n)) if n == "net.connman.iwd.NotConnected" => Ok(()),
            Err(e) => Err(reason(e)),
        }
    }

    /// `wifi.forget`.
    ///
    /// # Errors
    ///
    /// `NOT_FOUND` when no stored network has that name.
    pub fn forget(&self, ssid: &str) -> Result<(), Reason> {
        let objects = self.objects()?;
        let (path, _) = dbus::with(&objects, KNOWN)
            .into_iter()
            .find(|(_, k)| dbus::text(k, "Name") == Some(ssid))
            .ok_or(Reason::NotFound)?;
        self.bus
            .call(
                IWD,
                path,
                KNOWN,
                "Forget",
                Arg::None,
                Duration::from_secs(10),
            )
            .map_err(reason)
    }

    /// `wifi.set_radio`, on iwd's side: the device's `Powered`.
    ///
    /// # Errors
    ///
    /// A typed reason.
    pub fn set_powered(&self, on: bool) -> Result<(), Reason> {
        let objects = self.objects()?;
        let r = radio(&objects).ok_or(Reason::HardwareAbsent)?;
        self.bus
            .set_bool(IWD, &r.path, DEVICE, "Powered", on)
            .map_err(reason)
    }

    /// Wait for iwd to show a device (after a radio switch was released).
    #[must_use]
    pub fn wait_for_device(&self) -> bool {
        self.wait(|o| radio(o).is_some(), self.timing.provision_wait)
            .is_some()
    }

    fn provision(&self, ssid: &str, passphrase: &str) -> Result<Provisioned, Reason> {
        let path = self.store.join(file_name(ssid, "psk"));
        let previous = fs::read(&path).ok().map(Zeroizing::new);
        let mut body = Zeroizing::new(String::with_capacity(32 + passphrase.len()));
        body.push_str("[Security]\nPassphrase=");
        body.push_str(passphrase);
        body.push('\n');
        files::write_atomic(&path, body.as_bytes(), 0o600)?;
        Ok(Provisioned { path, previous })
    }
}

/// A passphrase file this service just wrote, and what was there before.
struct Provisioned {
    path: PathBuf,
    previous: Option<Zeroizing<Vec<u8>>>,
}

impl Provisioned {
    /// The network refused it: put the directory back as it was.
    fn undo(self) {
        let _ = match &self.previous {
            Some(bytes) => files::write_atomic(&self.path, bytes, 0o600),
            None => files::remove(&self.path),
        };
    }

    /// It worked: the file stays.
    fn keep(self) {}
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dbus::fake::{obj, p, s, FakeBus};
    use crate::dbus::Value;

    const DEV: &str = "/net/connman/iwd/0/4";
    const LAB: &str = "/net/connman/iwd/0/4/4c6162_psk";
    const CAFE: &str = "/net/connman/iwd/0/4/43616665_open";

    fn timing() -> Timing {
        Timing::instant()
    }

    fn objects(connected: bool, lab_known: bool) -> Objects {
        let mut o = Objects::new();
        obj(
            &mut o,
            DEV,
            DEVICE,
            &[("Name", s("wlan0")), ("Powered", Value::Bool(true))],
        );
        let mut station = vec![
            (
                "State",
                s(if connected {
                    "connected"
                } else {
                    "disconnected"
                }),
            ),
            ("Scanning", Value::Bool(false)),
        ];
        if connected {
            station.push(("ConnectedNetwork", p(LAB)));
        }
        obj(&mut o, DEV, STATION, &station);
        let mut lab = vec![
            ("Name", s("Lab")),
            ("Type", s("psk")),
            ("Connected", Value::Bool(connected)),
            ("Device", p(DEV)),
        ];
        if lab_known {
            lab.push(("KnownNetwork", p("/net/connman/iwd/4c6162_psk")));
            obj(
                &mut o,
                "/net/connman/iwd/4c6162_psk",
                KNOWN,
                &[("Name", s("Lab")), ("Type", s("psk"))],
            );
        }
        obj(&mut o, LAB, NETWORK, &lab);
        obj(
            &mut o,
            CAFE,
            NETWORK,
            &[
                ("Name", s("Café Central")),
                ("Type", s("open")),
                ("Device", p(DEV)),
            ],
        );
        obj(
            &mut o,
            "/net/connman/iwd/0/4/x_wep",
            NETWORK,
            &[("Name", s("Velha")), ("Type", s("wep"))],
        );
        o
    }

    struct Store(PathBuf);
    impl Store {
        fn new() -> Self {
            let d = std::env::temp_dir().join(format!(
                "ocinye-iwd-{}-{}",
                std::process::id(),
                N.fetch_add(1, std::sync::atomic::Ordering::SeqCst)
            ));
            let _ = fs::remove_dir_all(&d);
            fs::create_dir_all(&d).unwrap();
            Self(d)
        }
    }
    impl Drop for Store {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    static N: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

    #[test]
    fn o_nome_do_ficheiro_e_o_do_iwd() {
        assert_eq!(file_name("Lab_2-G", "psk"), "Lab_2-G.psk");
        assert_eq!(file_name("Café", "psk"), "=436166c3a9.psk");
        assert_eq!(file_name("a b", "open"), "=612062.open");
        assert_eq!(file_name("../x", "psk"), "=2e2e2f78.psk");
    }

    #[test]
    fn o_sinal_vai_de_menos_90_a_menos_30_dbm() {
        assert_eq!(signal(-9000).get(), 0);
        assert_eq!(signal(-12000).get(), 0);
        assert_eq!(signal(-6000).get(), 50);
        assert_eq!(signal(-3000).get(), 100);
        assert_eq!(signal(-1000).get(), 100);
    }

    #[test]
    fn a_lista_vem_por_ordem_sem_repetidos_nem_wep() {
        let o = objects(true, true);
        let list = networks(
            &o,
            &[
                (LAB.to_owned(), -4500),
                (CAFE.to_owned(), -7000),
                ("/net/connman/iwd/0/4/x_wep".to_owned(), -5000),
                ("/gone".to_owned(), -4000),
                (LAB.to_owned(), -8000),
            ],
        );
        assert_eq!(list.len(), 2);
        assert_eq!(
            (
                list[0].ssid.as_str(),
                list[0].security,
                list[0].known,
                list[0].connected,
                list[0].signal.get()
            ),
            ("Lab", WifiSecurity::Passphrase, true, true, 75)
        );
        assert_eq!(
            (list[1].ssid.as_str(), list[1].security, list[1].known),
            ("Café Central", WifiSecurity::Open, false)
        );
        assert_eq!(radio(&o).unwrap().connected.as_deref(), Some("Lab"));
        assert_eq!(radio(&objects(false, true)).unwrap().connected, None);
    }

    #[test]
    fn procurar_espera_pelo_fim_da_procura_e_lista() {
        let bus = FakeBus::with(IWD, objects(false, false));
        *bus.ordered.lock().unwrap() = vec![(CAFE.to_owned(), -6000)];
        let store = Store::new();
        let t = timing();
        let w = Wifi {
            bus: &bus,
            store: &store.0,
            timing: &t,
        };
        let scan = w.scan().unwrap();
        assert_eq!(scan.networks.len(), 1);
        assert_eq!(bus.methods(), ["Scan", "GetOrderedNetworks"]);
        // A scan already running is not a failure.
        bus.fail("Scan", BusError::Method("net.connman.iwd.Busy".into()));
        assert!(w.scan().is_ok());
    }

    #[test]
    fn sem_iwd_e_servico_em_baixo_e_sem_dispositivo_e_ausente_e_desligado_e_bloqueado() {
        let store = Store::new();
        let t = timing();
        let none = FakeBus::default();
        let w = Wifi {
            bus: &none,
            store: &store.0,
            timing: &t,
        };
        assert_eq!(
            w.scan(),
            Err(Reason::HardwareUnavailable {
                cause: Unavailable::ServiceDown
            })
        );
        let empty = FakeBus::with(IWD, Objects::new());
        let w = Wifi {
            bus: &empty,
            store: &store.0,
            timing: &t,
        };
        assert_eq!(w.scan(), Err(Reason::HardwareAbsent));
        let mut o = objects(false, false);
        o.get_mut(DEV)
            .unwrap()
            .get_mut(DEVICE)
            .unwrap()
            .insert("Powered".into(), Value::Bool(false));
        let off = FakeBus::with(IWD, o);
        let w = Wifi {
            bus: &off,
            store: &store.0,
            timing: &t,
        };
        assert_eq!(
            w.connect("Lab", Some("correct horse")),
            Err(Reason::HardwareUnavailable {
                cause: Unavailable::SoftBlocked
            })
        );
        assert!(off.methods().is_empty());
    }

    #[test]
    fn ligar_com_palavra_passe_escreve_o_ficheiro_do_iwd_e_liga() {
        let store = Store::new();
        let t = timing();
        // The fake iwd "notices" the file as soon as it is read again.
        let bus = FakeBus::with(IWD, objects(false, false));
        let dir = store.0.clone();
        bus.on("Connect", |o, path| {
            o.get_mut(path)
                .unwrap()
                .get_mut(NETWORK)
                .unwrap()
                .insert("Connected".into(), Value::Bool(true));
        });
        // iwd has loaded the file by the time it is looked at again.
        {
            let mut all = bus.objects.lock().unwrap();
            let o = all.get_mut(IWD).unwrap();
            o.get_mut(LAB)
                .unwrap()
                .get_mut(NETWORK)
                .unwrap()
                .insert("KnownNetwork".into(), p("/k"));
        }
        let w = Wifi {
            bus: &bus,
            store: &dir,
            timing: &t,
        };
        w.connect("Lab", Some("correct horse")).unwrap();
        let file = dir.join("Lab.psk");
        assert_eq!(
            fs::read_to_string(&file).unwrap(),
            "[Security]\nPassphrase=correct horse\n"
        );
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            fs::metadata(&file).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert_eq!(bus.methods(), ["Connect"]);
    }

    #[test]
    fn uma_palavra_passe_recusada_nao_fica_guardada_e_a_anterior_volta() {
        let store = Store::new();
        let t = timing();
        let mut o = objects(false, false);
        o.get_mut(LAB)
            .unwrap()
            .get_mut(NETWORK)
            .unwrap()
            .insert("KnownNetwork".into(), p("/k"));
        let bus = FakeBus::with(IWD, o);
        let w = Wifi {
            bus: &bus,
            store: &store.0,
            timing: &t,
        };
        // A new network: refused, the file goes.
        bus.fail("Connect", BusError::Method("net.connman.iwd.Failed".into()));
        assert_eq!(
            w.connect("Lab", Some("wrong horse")),
            Err(Reason::WifiAuthenticationFailed)
        );
        assert!(!store.0.join("Lab.psk").exists());
        // A known one: refused, the old file is back, byte for byte.
        fs::write(store.0.join("Lab.psk"), "[Security]\nPreSharedKey=abcd\n").unwrap();
        bus.fail("Connect", BusError::Method("net.connman.iwd.Failed".into()));
        assert_eq!(
            w.connect("Lab", Some("wrong horse")),
            Err(Reason::WifiAuthenticationFailed)
        );
        assert_eq!(
            fs::read_to_string(store.0.join("Lab.psk")).unwrap(),
            "[Security]\nPreSharedKey=abcd\n"
        );
        // No answer in time.
        bus.fail("Connect", BusError::Timeout);
        assert_eq!(w.connect("Lab", None), Err(Reason::WifiTimeout));
    }

    #[test]
    fn so_se_tenta_o_que_faz_sentido() {
        let store = Store::new();
        let t = timing();
        let mut o = objects(false, false);
        obj(
            &mut o,
            "/net/connman/iwd/0/4/e_8021x",
            NETWORK,
            &[("Name", s("Empresa")), ("Type", s("8021x"))],
        );
        let bus = FakeBus::with(IWD, o);
        let w = Wifi {
            bus: &bus,
            store: &store.0,
            timing: &t,
        };
        // Unknown protected network, no passphrase.
        assert_eq!(w.connect("Lab", None), Err(Reason::ValueOutOfRange));
        // A passphrase for an open network.
        assert_eq!(
            w.connect("Café Central", Some("12345678")),
            Err(Reason::ValueOutOfRange)
        );
        assert_eq!(
            w.connect("Empresa", Some("12345678")),
            Err(Reason::NotImplemented)
        );
        // Not in range even after a scan.
        assert_eq!(w.connect("Nenhuma", None), Err(Reason::NotFound));
        assert!(!bus.methods().contains(&"Connect".to_owned()));
        assert!(fs::read_dir(&store.0).unwrap().next().is_none());
        // Open: straight to Connect.
        w.connect("Café Central", None).unwrap();
        assert_eq!(bus.methods().last().unwrap(), "Connect");
    }

    #[test]
    fn desligar_esquecer_e_o_radio() {
        let store = Store::new();
        let t = timing();
        let bus = FakeBus::with(IWD, objects(true, true));
        let w = Wifi {
            bus: &bus,
            store: &store.0,
            timing: &t,
        };
        w.disconnect().unwrap();
        w.forget("Lab").unwrap();
        assert_eq!(w.forget("Outra"), Err(Reason::NotFound));
        w.set_powered(false).unwrap();
        assert_eq!(bus.methods(), ["Disconnect", "Forget", "Set Powered"]);
        assert!(!w.state().unwrap().powered);
        // Disconnecting when not connected asks nothing.
        let idle = FakeBus::with(IWD, objects(false, false));
        let w = Wifi {
            bus: &idle,
            store: &store.0,
            timing: &t,
        };
        w.disconnect().unwrap();
        assert!(idle.methods().is_empty());
    }
}
