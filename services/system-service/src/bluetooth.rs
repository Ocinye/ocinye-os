//! Bluetooth, through BlueZ over D-Bus (`org.bluez`).
//!
//! No pairing agent is registered: BlueZ then pairs with the capability
//! "no input, no output", so a device that pairs without a code ("just
//! works") pairs, and one that needs a PIN or a confirmation is refused by
//! BlueZ — and that refusal is returned as a typed reason. Nothing on the
//! screen is ever asked to type or compare a code.

use std::time::Duration;

use ocinye_system_contracts::reason::{AdapterFailure, Reason, Unavailable};
use ocinye_system_contracts::state::{BluetoothClass, BluetoothDevice, BluetoothState};

use crate::dbus::{self, Arg, Bus, BusError, Objects, Properties};
use crate::linux::Timing;

/// The service on the bus.
pub const BLUEZ: &str = "org.bluez";
const ADAPTER: &str = "org.bluez.Adapter1";
const DEVICE: &str = "org.bluez.Device1";

/// What a device is, from the icon BlueZ derives or the class of device it
/// advertises (Bluetooth Assigned Numbers: major class 4 is audio/video, 5 a
/// peripheral whose minor bits say keyboard or pointer).
#[must_use]
pub fn class(props: &Properties) -> BluetoothClass {
    match dbus::text(props, "Icon") {
        Some("audio-card" | "audio-headset" | "audio-headphones" | "audio-speakers") => {
            return BluetoothClass::Audio
        }
        Some("input-keyboard") => return BluetoothClass::Keyboard,
        Some("input-mouse" | "input-tablet") => return BluetoothClass::Pointer,
        _ => {}
    }
    let Some(cod) = dbus::int(props, "Class") else {
        return BluetoothClass::Other;
    };
    match ((cod >> 8) & 0x1f, (cod >> 6) & 0x03) {
        (0x04, _) => BluetoothClass::Audio,
        (0x05, 1 | 3) => BluetoothClass::Keyboard,
        (0x05, 2) => BluetoothClass::Pointer,
        _ => BluetoothClass::Other,
    }
}

/// The first adapter, by path order.
fn adapter(objects: &Objects) -> Option<(&str, &Properties)> {
    dbus::with(objects, ADAPTER).into_iter().next()
}

/// The state an interface shows: the adapter, and its devices — paired
/// first, then by name.
#[must_use]
pub fn state(objects: &Objects) -> Option<BluetoothState> {
    let (path, a) = adapter(objects)?;
    let mut devices: Vec<BluetoothDevice> = dbus::with(objects, DEVICE)
        .into_iter()
        .filter(|(_, d)| dbus::path(d, "Adapter") == Some(path))
        .filter_map(|(_, d)| {
            let address = dbus::text(d, "Address")?.to_ascii_uppercase();
            let name = dbus::text(d, "Name")
                .or_else(|| dbus::text(d, "Alias"))
                .filter(|n| !n.is_empty())
                .map_or_else(|| address.clone(), str::to_owned);
            Some(BluetoothDevice {
                class: class(d),
                paired: dbus::flag(d, "Paired"),
                connected: dbus::flag(d, "Connected"),
                address,
                name,
            })
        })
        .collect();
    devices.sort_by(|x, y| y.paired.cmp(&x.paired).then_with(|| x.name.cmp(&y.name)));
    Some(BluetoothState {
        powered: dbus::flag(a, "Powered"),
        devices,
    })
}

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
            "org.bluez.Error.NotReady" => Reason::HardwareUnavailable {
                cause: Unavailable::SoftBlocked,
            },
            "org.bluez.Error.InProgress" => Reason::OperationInProgress,
            "org.bluez.Error.DoesNotExist" => Reason::NotFound,
            "org.bluez.Error.AuthenticationTimeout"
            | "org.bluez.Error.ConnectionAttemptFailed"
            | "org.bluez.Error.PageTimeout" => Reason::AdapterFailed {
                cause: AdapterFailure::Timeout,
            },
            // AuthenticationFailed, AuthenticationRejected,
            // AuthenticationCanceled — a device that wanted a code — and
            // every other refusal.
            _ => Reason::AdapterFailed {
                cause: AdapterFailure::Refused,
            },
        },
    }
}

/// The Bluetooth adapter.
pub struct Bluetooth<'a> {
    /// The bus.
    pub bus: &'a dyn Bus,
    /// How long to wait for things.
    pub timing: &'a Timing,
}

impl Bluetooth<'_> {
    fn objects(&self) -> Result<Objects, Reason> {
        self.bus.managed_objects(BLUEZ).map_err(reason)
    }

    /// `bluetooth.status`.
    ///
    /// # Errors
    ///
    /// `HARDWARE_ABSENT` without an adapter; `HARDWARE_UNAVAILABLE` when
    /// BlueZ is not running.
    pub fn status(&self) -> Result<BluetoothState, Reason> {
        state(&self.objects()?).ok_or(Reason::HardwareAbsent)
    }

    fn powered_adapter(&self) -> Result<(Objects, String), Reason> {
        let objects = self.objects()?;
        let (path, a) = adapter(&objects).ok_or(Reason::HardwareAbsent)?;
        if !dbus::flag(a, "Powered") {
            return Err(Reason::HardwareUnavailable {
                cause: Unavailable::SoftBlocked,
            });
        }
        let path = path.to_owned();
        Ok((objects, path))
    }

    /// `bluetooth.set_power`, on BlueZ's side.
    ///
    /// # Errors
    ///
    /// A typed reason.
    pub fn set_power(&self, on: bool) -> Result<(), Reason> {
        let objects = self.objects()?;
        let (path, _) = adapter(&objects).ok_or(Reason::HardwareAbsent)?;
        self.bus
            .set_bool(BLUEZ, path, ADAPTER, "Powered", on)
            .map_err(reason)
    }

    /// `bluetooth.scan`: discover for a bounded time, then stop.
    ///
    /// # Errors
    ///
    /// A typed reason.
    pub fn scan(&self) -> Result<BluetoothState, Reason> {
        let (_, path) = self.powered_adapter()?;
        let t = Duration::from_secs(10);
        match self
            .bus
            .call(BLUEZ, &path, ADAPTER, "StartDiscovery", Arg::None, t)
        {
            // Someone else's discovery is running: it finds the same devices.
            Ok(()) => {}
            Err(BusError::Method(n)) if n == "org.bluez.Error.InProgress" => {}
            Err(e) => return Err(reason(e)),
        }
        std::thread::sleep(self.timing.discovery);
        // Stopping someone else's discovery is refused; that is fine.
        let _ = self
            .bus
            .call(BLUEZ, &path, ADAPTER, "StopDiscovery", Arg::None, t);
        self.status()
    }

    fn device(objects: &Objects, address: &str) -> Result<String, Reason> {
        dbus::with(objects, DEVICE)
            .into_iter()
            .find(|(_, d)| {
                dbus::text(d, "Address").is_some_and(|a| a.eq_ignore_ascii_case(address))
            })
            .map(|(p, _)| p.to_owned())
            .ok_or(Reason::NotFound)
    }

    /// `bluetooth.pair`: only devices that pair without a code.
    ///
    /// # Errors
    ///
    /// A typed reason; a device that needs a PIN or a confirmation is
    /// `ADAPTER_FAILED { REFUSED }`.
    pub fn pair(&self, address: &str) -> Result<BluetoothState, Reason> {
        let (objects, _) = self.powered_adapter()?;
        let device = Self::device(&objects, address)?;
        if dbus::prop(&objects, &device, DEVICE, "Paired") != Some(&dbus::Value::Bool(true)) {
            match self
                .bus
                .call(BLUEZ, &device, DEVICE, "Pair", Arg::None, self.timing.pair)
            {
                Ok(()) => {}
                Err(BusError::Method(n)) if n == "org.bluez.Error.AlreadyExists" => {}
                Err(e) => return Err(reason(e)),
            }
        }
        // A paired keyboard or headset reconnects by itself only when
        // trusted; trust is what this machine extends to a device it paired.
        let _ = self.bus.set_bool(BLUEZ, &device, DEVICE, "Trusted", true);
        self.status()
    }

    /// `bluetooth.connect` / `bluetooth.disconnect`.
    ///
    /// # Errors
    ///
    /// A typed reason.
    pub fn link(&self, address: &str, connect: bool) -> Result<BluetoothState, Reason> {
        let (objects, _) = self.powered_adapter()?;
        let device = Self::device(&objects, address)?;
        let method = if connect { "Connect" } else { "Disconnect" };
        match self
            .bus
            .call(BLUEZ, &device, DEVICE, method, Arg::None, self.timing.pair)
        {
            Ok(()) => {}
            Err(BusError::Method(n))
                if n == "org.bluez.Error.AlreadyConnected"
                    || n == "org.bluez.Error.NotConnected" => {}
            Err(e) => return Err(reason(e)),
        }
        self.status()
    }

    /// `bluetooth.remove`: forget the pairing and the device.
    ///
    /// # Errors
    ///
    /// A typed reason.
    pub fn remove(&self, address: &str) -> Result<BluetoothState, Reason> {
        let objects = self.objects()?;
        let (adapter, _) = adapter(&objects).ok_or(Reason::HardwareAbsent)?;
        let device = Self::device(&objects, address)?;
        self.bus
            .call(
                BLUEZ,
                adapter,
                ADAPTER,
                "RemoveDevice",
                Arg::Path(&device),
                Duration::from_secs(10),
            )
            .map_err(reason)?;
        self.status()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dbus::fake::{obj, p, s, FakeBus};
    use crate::dbus::Value;

    const HCI: &str = "/org/bluez/hci0";
    const KBD: &str = "/org/bluez/hci0/dev_AA_BB_CC_DD_EE_01";
    const PHONES: &str = "/org/bluez/hci0/dev_AA_BB_CC_DD_EE_02";
    const MOUSE: &str = "/org/bluez/hci0/dev_AA_BB_CC_DD_EE_03";

    fn objects(powered: bool) -> Objects {
        let mut o = Objects::new();
        obj(
            &mut o,
            HCI,
            ADAPTER,
            &[
                ("Powered", Value::Bool(powered)),
                ("Address", s("00:11:22:33:44:55")),
            ],
        );
        obj(
            &mut o,
            KBD,
            DEVICE,
            &[
                ("Address", s("AA:BB:CC:DD:EE:01")),
                ("Name", s("Teclado")),
                ("Icon", s("input-keyboard")),
                ("Paired", Value::Bool(true)),
                ("Connected", Value::Bool(true)),
                ("Adapter", p(HCI)),
            ],
        );
        // Class 0x240404: audio/video, wearable headset.
        obj(
            &mut o,
            PHONES,
            DEVICE,
            &[
                ("Address", s("aa:bb:cc:dd:ee:02")),
                ("Alias", s("Auscultadores")),
                ("Class", Value::Int(0x0024_0404)),
                ("Adapter", p(HCI)),
            ],
        );
        // Class 0x002580: peripheral, pointing device; no name at all.
        obj(
            &mut o,
            MOUSE,
            DEVICE,
            &[
                ("Address", s("AA:BB:CC:DD:EE:03")),
                ("Class", Value::Int(0x2580)),
                ("Adapter", p(HCI)),
            ],
        );
        obj(
            &mut o,
            "/org/bluez/hci1/dev_X",
            DEVICE,
            &[
                ("Address", s("AA:BB:CC:DD:EE:09")),
                ("Adapter", p("/org/bluez/hci1")),
            ],
        );
        o
    }

    fn timing() -> Timing {
        Timing::instant()
    }

    #[test]
    fn o_estado_lista_os_dispositivos_do_adaptador_emparelhados_primeiro() {
        let st = state(&objects(true)).unwrap();
        assert!(st.powered);
        let rows: Vec<_> = st
            .devices
            .iter()
            .map(|d| {
                (
                    d.address.as_str(),
                    d.name.as_str(),
                    d.class,
                    d.paired,
                    d.connected,
                )
            })
            .collect();
        assert_eq!(
            rows,
            [
                (
                    "AA:BB:CC:DD:EE:01",
                    "Teclado",
                    BluetoothClass::Keyboard,
                    true,
                    true
                ),
                (
                    "AA:BB:CC:DD:EE:03",
                    "AA:BB:CC:DD:EE:03",
                    BluetoothClass::Pointer,
                    false,
                    false
                ),
                (
                    "AA:BB:CC:DD:EE:02",
                    "Auscultadores",
                    BluetoothClass::Audio,
                    false,
                    false
                ),
            ]
        );
        assert_eq!(state(&Objects::new()), None);
    }

    #[test]
    fn sem_bluez_e_sem_adaptador_e_desligado() {
        let t = timing();
        let down = FakeBus::default();
        assert_eq!(
            Bluetooth {
                bus: &down,
                timing: &t
            }
            .status(),
            Err(Reason::HardwareUnavailable {
                cause: Unavailable::ServiceDown
            })
        );
        let empty = FakeBus::with(BLUEZ, Objects::new());
        assert_eq!(
            Bluetooth {
                bus: &empty,
                timing: &t
            }
            .status(),
            Err(Reason::HardwareAbsent)
        );
        let off = FakeBus::with(BLUEZ, objects(false));
        let b = Bluetooth {
            bus: &off,
            timing: &t,
        };
        assert_eq!(
            b.scan(),
            Err(Reason::HardwareUnavailable {
                cause: Unavailable::SoftBlocked
            })
        );
        assert_eq!(
            b.pair("AA:BB:CC:DD:EE:02"),
            Err(Reason::HardwareUnavailable {
                cause: Unavailable::SoftBlocked
            })
        );
        b.set_power(true).unwrap();
        assert!(b.status().unwrap().powered);
    }

    #[test]
    fn procurar_comeca_e_para_sempre() {
        let t = timing();
        let bus = FakeBus::with(BLUEZ, objects(true));
        let b = Bluetooth {
            bus: &bus,
            timing: &t,
        };
        b.scan().unwrap();
        assert_eq!(bus.methods(), ["StartDiscovery", "StopDiscovery"]);
    }

    #[test]
    fn emparelhar_so_sem_codigo_e_confiar_no_que_emparelhou() {
        let t = timing();
        let bus = FakeBus::with(BLUEZ, objects(true));
        bus.on("Pair", |o, path| {
            o.get_mut(path)
                .unwrap()
                .get_mut(DEVICE)
                .unwrap()
                .insert("Paired".into(), Value::Bool(true));
        });
        let b = Bluetooth {
            bus: &bus,
            timing: &t,
        };
        let st = b.pair("aa:bb:cc:dd:ee:02").unwrap();
        assert!(st
            .devices
            .iter()
            .any(|d| d.address == "AA:BB:CC:DD:EE:02" && d.paired));
        assert_eq!(bus.methods(), ["Pair", "Set Trusted"]);
        // Already paired: nothing to ask.
        b.pair("AA:BB:CC:DD:EE:01").unwrap();
        assert_eq!(bus.methods().iter().filter(|m| *m == "Pair").count(), 1);
        // A device that wanted a code.
        bus.fail(
            "Pair",
            BusError::Method("org.bluez.Error.AuthenticationRejected".into()),
        );
        assert_eq!(
            b.pair("AA:BB:CC:DD:EE:03"),
            Err(Reason::AdapterFailed {
                cause: AdapterFailure::Refused
            })
        );
        bus.fail(
            "Pair",
            BusError::Method("org.bluez.Error.AuthenticationTimeout".into()),
        );
        assert_eq!(
            b.pair("AA:BB:CC:DD:EE:03"),
            Err(Reason::AdapterFailed {
                cause: AdapterFailure::Timeout
            })
        );
        assert_eq!(b.pair("AA:BB:CC:DD:EE:77"), Err(Reason::NotFound));
    }

    #[test]
    fn ligar_desligar_e_remover() {
        let t = timing();
        let bus = FakeBus::with(BLUEZ, objects(true));
        bus.on("RemoveDevice", |o, path| {
            o.remove(path);
        });
        let b = Bluetooth {
            bus: &bus,
            timing: &t,
        };
        b.link("AA:BB:CC:DD:EE:01", false).unwrap();
        bus.fail(
            "Connect",
            BusError::Method("org.bluez.Error.AlreadyConnected".into()),
        );
        b.link("AA:BB:CC:DD:EE:01", true).unwrap();
        let st = b.remove("AA:BB:CC:DD:EE:01").unwrap();
        assert!(!st.devices.iter().any(|d| d.address == "AA:BB:CC:DD:EE:01"));
        let calls = bus.calls.lock().unwrap().clone();
        assert_eq!(calls.last().unwrap().3.as_deref(), Some(KBD));
        assert_eq!(bus.methods(), ["Disconnect", "Connect", "RemoveDevice"]);
    }
}
