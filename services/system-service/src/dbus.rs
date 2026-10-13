//! The system bus, as far as the Wi-Fi and Bluetooth adapters use it.
//!
//! ADR-0031 puts D-Bus before a subprocess: iwd and BlueZ are services with
//! typed interfaces, and asking them directly gives error *names*, not a
//! tool's prose. [`Bus`] is the narrow surface the adapters need — read every
//! object of a service, call a method with at most one object-path argument,
//! set a boolean property, and iwd's one method with a reply — so that the
//! decisions above it are tested against canned objects, and only
//! [`SystemBus`] touches a real bus.
//!
//! Nothing here takes a destination, path or method from a client: the
//! adapters pass constants, and object paths that the service itself read
//! from the bus.

use std::collections::{BTreeMap, HashMap};
use std::time::Duration;

use zbus::zvariant::{ObjectPath, OwnedObjectPath, OwnedValue, Value as Variant};

/// A property value, as far as the adapters read one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Value {
    /// A boolean.
    Bool(bool),
    /// A string.
    Str(String),
    /// An object path.
    Path(String),
    /// Any integer type.
    Int(i64),
    /// Anything else: present, not read.
    Other,
}

/// The properties of one interface.
pub type Properties = BTreeMap<String, Value>;
/// The interfaces of one object, with their properties.
pub type Interfaces = BTreeMap<String, Properties>;
/// Every object of a service, by path.
pub type Objects = BTreeMap<String, Interfaces>;

/// The one argument a call may carry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Arg<'a> {
    /// No argument.
    None,
    /// An object path the adapter read from the bus.
    Path(&'a str),
}

/// Why a bus operation did not complete.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BusError {
    /// No system bus, or the service is not on it.
    ServiceDown,
    /// No answer before the deadline.
    Timeout,
    /// The service answered with an error, named as on the bus
    /// (`net.connman.iwd.Failed`). The description that comes with it is
    /// never kept.
    Method(String),
    /// The answer was not of the expected shape.
    UnexpectedReply,
}

/// The bus, narrowed to what the adapters do with it.
pub trait Bus: Send + Sync {
    /// `org.freedesktop.DBus.ObjectManager.GetManagedObjects` of `service`
    /// at `/`.
    ///
    /// # Errors
    ///
    /// A [`BusError`].
    fn managed_objects(&self, service: &str) -> Result<Objects, BusError>;

    /// A method whose reply carries nothing the adapter reads.
    ///
    /// # Errors
    ///
    /// A [`BusError`].
    fn call(
        &self,
        service: &str,
        path: &str,
        interface: &str,
        method: &str,
        arg: Arg<'_>,
        timeout: Duration,
    ) -> Result<(), BusError>;

    /// `org.freedesktop.DBus.Properties.Set` of a boolean.
    ///
    /// # Errors
    ///
    /// A [`BusError`].
    fn set_bool(
        &self,
        service: &str,
        path: &str,
        interface: &str,
        property: &str,
        value: bool,
    ) -> Result<(), BusError>;

    /// iwd's `Station.GetOrderedNetworks`: network paths and signal strength
    /// in hundredths of a dBm, strongest first.
    ///
    /// # Errors
    ///
    /// A [`BusError`].
    fn ordered_networks(&self, station: &str) -> Result<Vec<(String, i16)>, BusError>;
}

/// The system bus, through `zbus`.
#[derive(Debug, Default, Clone, Copy)]
pub struct SystemBus;

/// How long a read of a service's objects may take.
const READ_TIMEOUT: Duration = Duration::from_secs(5);

const SERVICE_DOWN: &[&str] = &[
    "org.freedesktop.DBus.Error.ServiceUnknown",
    "org.freedesktop.DBus.Error.NameHasNoOwner",
    "org.freedesktop.DBus.Error.NoServer",
    "org.freedesktop.DBus.Error.Disconnected",
];

fn classify(e: zbus::Error) -> BusError {
    match e {
        zbus::Error::MethodError(name, _, _) => {
            let name = name.as_str();
            if SERVICE_DOWN.contains(&name) {
                BusError::ServiceDown
            } else if name == "org.freedesktop.DBus.Error.NoReply"
                || name == "org.freedesktop.DBus.Error.Timeout"
            {
                BusError::Timeout
            } else {
                BusError::Method(name.to_owned())
            }
        }
        zbus::Error::FDO(e) => classify(zbus::Error::from(*e)),
        zbus::Error::InputOutput(io) if io.kind() == std::io::ErrorKind::TimedOut => {
            BusError::Timeout
        }
        zbus::Error::InputOutput(_) | zbus::Error::Handshake(_) => BusError::ServiceDown,
        _ => BusError::UnexpectedReply,
    }
}

fn connection(timeout: Duration) -> Result<zbus::blocking::Connection, BusError> {
    zbus::blocking::connection::Builder::system()
        .map_err(|_| BusError::ServiceDown)?
        .method_timeout(timeout)
        .build()
        .map_err(|_| BusError::ServiceDown)
}

fn value(v: &Variant<'_>) -> Value {
    match v {
        Variant::Bool(b) => Value::Bool(*b),
        Variant::Str(s) => Value::Str(s.as_str().to_owned()),
        Variant::ObjectPath(p) => Value::Path(p.as_str().to_owned()),
        Variant::U8(n) => Value::Int(i64::from(*n)),
        Variant::I16(n) => Value::Int(i64::from(*n)),
        Variant::U16(n) => Value::Int(i64::from(*n)),
        Variant::I32(n) => Value::Int(i64::from(*n)),
        Variant::U32(n) => Value::Int(i64::from(*n)),
        Variant::I64(n) => Value::Int(*n),
        Variant::U64(n) => i64::try_from(*n).map_or(Value::Other, Value::Int),
        Variant::Value(inner) => value(inner),
        _ => Value::Other,
    }
}

impl Bus for SystemBus {
    fn managed_objects(&self, service: &str) -> Result<Objects, BusError> {
        let c = connection(READ_TIMEOUT)?;
        let reply = c
            .call_method(
                Some(service),
                "/",
                Some("org.freedesktop.DBus.ObjectManager"),
                "GetManagedObjects",
                &(),
            )
            .map_err(classify)?;
        type Raw = HashMap<OwnedObjectPath, HashMap<String, HashMap<String, OwnedValue>>>;
        let raw: Raw = reply
            .body()
            .deserialize()
            .map_err(|_| BusError::UnexpectedReply)?;
        Ok(raw
            .into_iter()
            .map(|(path, interfaces)| {
                let interfaces = interfaces
                    .into_iter()
                    .map(|(name, props)| {
                        let props = props.iter().map(|(k, v)| (k.clone(), value(v))).collect();
                        (name, props)
                    })
                    .collect();
                (path.as_str().to_owned(), interfaces)
            })
            .collect())
    }

    fn call(
        &self,
        service: &str,
        path: &str,
        interface: &str,
        method: &str,
        arg: Arg<'_>,
        timeout: Duration,
    ) -> Result<(), BusError> {
        let c = connection(timeout)?;
        match arg {
            Arg::None => c.call_method(Some(service), path, Some(interface), method, &()),
            Arg::Path(p) => {
                let p = ObjectPath::try_from(p).map_err(|_| BusError::UnexpectedReply)?;
                c.call_method(Some(service), path, Some(interface), method, &(p,))
            }
        }
        .map(drop)
        .map_err(classify)
    }

    fn set_bool(
        &self,
        service: &str,
        path: &str,
        interface: &str,
        property: &str,
        value: bool,
    ) -> Result<(), BusError> {
        let c = connection(READ_TIMEOUT)?;
        c.call_method(
            Some(service),
            path,
            Some("org.freedesktop.DBus.Properties"),
            "Set",
            &(interface, property, Variant::from(value)),
        )
        .map(drop)
        .map_err(classify)
    }

    fn ordered_networks(&self, station: &str) -> Result<Vec<(String, i16)>, BusError> {
        let c = connection(READ_TIMEOUT)?;
        let reply = c
            .call_method(
                Some("net.connman.iwd"),
                station,
                Some("net.connman.iwd.Station"),
                "GetOrderedNetworks",
                &(),
            )
            .map_err(classify)?;
        let list: Vec<(OwnedObjectPath, i16)> = reply
            .body()
            .deserialize()
            .map_err(|_| BusError::UnexpectedReply)?;
        Ok(list
            .into_iter()
            .map(|(p, s)| (p.as_str().to_owned(), s))
            .collect())
    }
}

/// A property of an interface of an object, when present.
#[must_use]
pub fn prop<'a>(objects: &'a Objects, path: &str, interface: &str, key: &str) -> Option<&'a Value> {
    objects.get(path)?.get(interface)?.get(key)
}

/// A boolean property, `false` when absent or of another type.
#[must_use]
pub fn flag(props: &Properties, key: &str) -> bool {
    matches!(props.get(key), Some(Value::Bool(true)))
}

/// A string property.
#[must_use]
pub fn text<'a>(props: &'a Properties, key: &str) -> Option<&'a str> {
    match props.get(key) {
        Some(Value::Str(s)) => Some(s),
        _ => None,
    }
}

/// An object-path property.
#[must_use]
pub fn path<'a>(props: &'a Properties, key: &str) -> Option<&'a str> {
    match props.get(key) {
        Some(Value::Path(p)) => Some(p),
        _ => None,
    }
}

/// An integer property.
#[must_use]
pub fn int(props: &Properties, key: &str) -> Option<i64> {
    match props.get(key) {
        Some(Value::Int(n)) => Some(*n),
        _ => None,
    }
}

/// Every object carrying `interface`, with its properties, by path order.
#[must_use]
pub fn with<'a>(objects: &'a Objects, interface: &str) -> Vec<(&'a str, &'a Properties)> {
    objects
        .iter()
        .filter_map(|(p, ifaces)| ifaces.get(interface).map(|props| (p.as_str(), props)))
        .collect()
}

#[cfg(test)]
pub mod fake {
    //! A bus that answers from objects held in memory and records calls.

    use super::*;
    use std::sync::Mutex;

    /// One recorded call: `(path, interface, method, argument)`.
    pub type Call = (String, String, String, Option<String>);

    /// A scripted bus.
    #[derive(Default)]
    pub struct FakeBus {
        /// Objects by service. A service absent here is down.
        pub objects: Mutex<BTreeMap<String, Objects>>,
        /// Errors to answer, by method name, once each.
        pub errors: Mutex<Vec<(String, BusError)>>,
        /// Calls made, in order.
        pub calls: Mutex<Vec<Call>>,
        /// `GetOrderedNetworks`.
        pub ordered: Mutex<Vec<(String, i16)>>,
        /// What a call does to the objects, by method name.
        #[allow(clippy::type_complexity)]
        pub effects: Mutex<Vec<(String, Box<dyn Fn(&mut Objects, &str) + Send>)>>,
    }

    impl FakeBus {
        /// A bus where `service` has `objects`.
        pub fn with(service: &str, objects: Objects) -> Self {
            let bus = Self::default();
            bus.objects
                .lock()
                .unwrap()
                .insert(service.to_owned(), objects);
            bus
        }

        /// The next `method` answers `error`.
        pub fn fail(&self, method: &str, error: BusError) {
            self.errors.lock().unwrap().push((method.to_owned(), error));
        }

        /// What `method` does to the objects of the service it was called on.
        pub fn on(&self, method: &str, effect: impl Fn(&mut Objects, &str) + Send + 'static) {
            self.effects
                .lock()
                .unwrap()
                .push((method.to_owned(), Box::new(effect)));
        }

        /// The methods called, in order.
        pub fn methods(&self) -> Vec<String> {
            self.calls
                .lock()
                .unwrap()
                .iter()
                .map(|c| c.2.clone())
                .collect()
        }

        fn answer(&self, service: &str, path: &str, method: &str) -> Result<(), BusError> {
            let mut errors = self.errors.lock().unwrap();
            if let Some(i) = errors.iter().position(|(m, _)| m == method) {
                return Err(errors.remove(i).1);
            }
            drop(errors);
            let mut all = self.objects.lock().unwrap();
            let objects = all.get_mut(service).ok_or(BusError::ServiceDown)?;
            for (m, effect) in self.effects.lock().unwrap().iter() {
                if m == method {
                    effect(objects, path);
                }
            }
            Ok(())
        }
    }

    impl Bus for FakeBus {
        fn managed_objects(&self, service: &str) -> Result<Objects, BusError> {
            self.objects
                .lock()
                .unwrap()
                .get(service)
                .cloned()
                .ok_or(BusError::ServiceDown)
        }

        fn call(
            &self,
            service: &str,
            path: &str,
            interface: &str,
            method: &str,
            arg: Arg<'_>,
            _timeout: Duration,
        ) -> Result<(), BusError> {
            let arg = match arg {
                Arg::None => None,
                Arg::Path(p) => Some(p.to_owned()),
            };
            self.calls.lock().unwrap().push((
                path.to_owned(),
                interface.to_owned(),
                method.to_owned(),
                arg.clone(),
            ));
            self.answer(service, arg.as_deref().unwrap_or(path), method)
        }

        fn set_bool(
            &self,
            service: &str,
            path: &str,
            interface: &str,
            property: &str,
            value: bool,
        ) -> Result<(), BusError> {
            self.calls.lock().unwrap().push((
                path.to_owned(),
                interface.to_owned(),
                format!("Set {property}"),
                Some(value.to_string()),
            ));
            self.answer(service, path, &format!("Set {property}"))?;
            let mut all = self.objects.lock().unwrap();
            if let Some(props) = all
                .get_mut(service)
                .and_then(|o| o.get_mut(path))
                .and_then(|i| i.get_mut(interface))
            {
                props.insert(property.to_owned(), Value::Bool(value));
            }
            Ok(())
        }

        fn ordered_networks(&self, station: &str) -> Result<Vec<(String, i16)>, BusError> {
            self.calls.lock().unwrap().push((
                station.to_owned(),
                "net.connman.iwd.Station".to_owned(),
                "GetOrderedNetworks".to_owned(),
                None,
            ));
            self.answer("net.connman.iwd", station, "GetOrderedNetworks")?;
            Ok(self.ordered.lock().unwrap().clone())
        }
    }

    /// Build an object: `obj(&mut objects, path, interface, [(key, value)])`.
    pub fn obj(objects: &mut Objects, path: &str, interface: &str, props: &[(&str, Value)]) {
        objects.entry(path.to_owned()).or_default().insert(
            interface.to_owned(),
            props
                .iter()
                .map(|(k, v)| ((*k).to_owned(), v.clone()))
                .collect(),
        );
    }

    /// A string value.
    pub fn s(v: &str) -> Value {
        Value::Str(v.to_owned())
    }

    /// A path value.
    pub fn p(v: &str) -> Value {
        Value::Path(v.to_owned())
    }
}
