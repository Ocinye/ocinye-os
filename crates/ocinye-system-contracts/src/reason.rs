//! Typed refusals. The service produces a reason code; an interface owns the
//! sentence it shows for it, in `pt`, `en` and `fr`. Tool output is never the
//! message.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::capability::{AuthorizationClass, FreshAuthClass};

/// Why a request was not carried out. One taxonomy for the interface, the
/// headless client, logs and tests. Only the service produces it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "code", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Reason {
    /// The request did not parse against the contract.
    MalformedRequest,
    /// The request is larger than the frame limit.
    RequestTooLarge,
    /// The client speaks a contract this service does not.
    IncompatibleContract {
        /// What the service speaks.
        service: u16,
        /// What the client offered.
        client: u16,
    },
    /// The first message was not the handshake.
    HandshakeRequired,
    /// The caller lacks the authority the capability needs.
    NotAuthorized {
        /// What the capability needs.
        needs: AuthorizationClass,
    },
    /// Fresh proof is needed first.
    FreshAuthRequired {
        /// Which kind.
        class: FreshAuthClass,
    },
    /// The grant presented does not match this request, or was already used.
    GrantInvalid,
    /// The Core must answer and cannot be reached.
    AuthorityUnavailable,
    /// The session is locked and the capability is not offered then.
    SessionLocked,
    /// The machine has no such hardware.
    HardwareAbsent,
    /// The hardware exists and cannot be used.
    HardwareUnavailable {
        /// Why.
        cause: Unavailable,
    },
    /// A value is outside what the capability accepts.
    ValueOutOfRange,
    /// The thing named does not exist (a network, a device, an output).
    NotFound,
    /// The subsystem refused or failed.
    AdapterFailed {
        /// Which class of failure.
        cause: AdapterFailure,
    },
    /// Wi-Fi: the network refused the credentials.
    WifiAuthenticationFailed,
    /// Wi-Fi: the network was not reachable in time.
    WifiTimeout,
    /// Another operation holds the machine (an update, an installation).
    OperationInProgress,
    /// The capability exists in the contract and is not implemented on this
    /// build.
    NotImplemented,
    /// The capability is deliberately not offered through this client.
    NotOfferedHere,
}

/// Why present hardware cannot be used.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Unavailable {
    /// A hardware switch blocks it.
    HardBlocked,
    /// Software switched it off.
    SoftBlocked,
    /// No driver bound.
    NoDriver,
    /// The service that manages it is not running.
    ServiceDown,
}

/// The class of an adapter failure. Never the tool's text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AdapterFailure {
    /// The subsystem did not answer in time.
    Timeout,
    /// It answered with a refusal.
    Refused,
    /// Its answer could not be understood.
    UnexpectedReply,
    /// It is not installed or not running.
    Unreachable,
}

impl Reason {
    /// The stable code, as logged and shown under "details".
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::MalformedRequest => "MALFORMED_REQUEST",
            Self::RequestTooLarge => "REQUEST_TOO_LARGE",
            Self::IncompatibleContract { .. } => "INCOMPATIBLE_CONTRACT",
            Self::HandshakeRequired => "HANDSHAKE_REQUIRED",
            Self::NotAuthorized { .. } => "NOT_AUTHORIZED",
            Self::FreshAuthRequired { .. } => "FRESH_AUTH_REQUIRED",
            Self::GrantInvalid => "GRANT_INVALID",
            Self::AuthorityUnavailable => "AUTHORITY_UNAVAILABLE",
            Self::SessionLocked => "SESSION_LOCKED",
            Self::HardwareAbsent => "HARDWARE_ABSENT",
            Self::HardwareUnavailable { .. } => "HARDWARE_UNAVAILABLE",
            Self::ValueOutOfRange => "VALUE_OUT_OF_RANGE",
            Self::NotFound => "NOT_FOUND",
            Self::AdapterFailed { .. } => "ADAPTER_FAILED",
            Self::WifiAuthenticationFailed => "WIFI_AUTHENTICATION_FAILED",
            Self::WifiTimeout => "WIFI_TIMEOUT",
            Self::OperationInProgress => "OPERATION_IN_PROGRESS",
            Self::NotImplemented => "NOT_IMPLEMENTED",
            Self::NotOfferedHere => "NOT_OFFERED_HERE",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn o_codigo_e_a_etiqueta_serializada() {
        let all = [
            Reason::MalformedRequest,
            Reason::RequestTooLarge,
            Reason::IncompatibleContract {
                service: 1,
                client: 2,
            },
            Reason::HandshakeRequired,
            Reason::NotAuthorized {
                needs: AuthorizationClass::Operator,
            },
            Reason::FreshAuthRequired {
                class: FreshAuthClass::CoreStepUp,
            },
            Reason::GrantInvalid,
            Reason::AuthorityUnavailable,
            Reason::SessionLocked,
            Reason::HardwareAbsent,
            Reason::HardwareUnavailable {
                cause: Unavailable::HardBlocked,
            },
            Reason::ValueOutOfRange,
            Reason::NotFound,
            Reason::AdapterFailed {
                cause: AdapterFailure::Timeout,
            },
            Reason::WifiAuthenticationFailed,
            Reason::WifiTimeout,
            Reason::OperationInProgress,
            Reason::NotImplemented,
            Reason::NotOfferedHere,
        ];
        for r in all {
            let v = serde_json::to_value(&r).unwrap();
            assert_eq!(v["code"], r.code());
            let back: Reason = serde_json::from_value(v).unwrap();
            assert_eq!(back, r);
        }
    }
}
