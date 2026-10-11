//! Events: what changed without anyone on this connection asking.
//!
//! An event informs an interface that state it shows is stale and carries the
//! new state. It never mutates anything and never carries authority.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::state::{
    AudioState, BluetoothState, DisplayState, NetworkState, PowerState, SystemHealth, UpdateState,
};

/// A change of system state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "event", content = "state", rename_all = "snake_case")]
pub enum Event {
    /// Cable pulled, address gained, Wi-Fi lost.
    NetworkStateChanged(NetworkState),
    /// A device appeared, a default changed, a level changed elsewhere.
    AudioStateChanged(AudioState),
    /// A monitor was connected or the backlight changed.
    DisplayStateChanged(DisplayState),
    /// Mains removed, battery level crossed a step.
    PowerStateChanged(PowerState),
    /// A device connected or left.
    BluetoothStateChanged(BluetoothState),
    /// An update moved to another phase.
    UpdateStateChanged(UpdateState),
    /// A part of the system changed health.
    SystemHealthChanged(SystemHealth),
}
