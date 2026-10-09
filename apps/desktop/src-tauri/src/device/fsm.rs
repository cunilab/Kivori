//! The desktop connection manager: the runtime actor's state built on the pure model FSM
//! ([`kivori_model::ConnectionState`]). Tracks the surfaced status, the connected-device summary, the
//! consecutive-retry count, and the human-readable incompatible reason (FR-003/005; SC-003).

use crate::device::connection::ConnectedDevice;
use kivori_model::{ConnectionEvent, ConnectionState};
use kivori_protocol::PROTOCOL_MAJOR;

/// A higher-level connection event carrying the data the manager records alongside the transition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ManagerEvent {
    /// A candidate port opened; the handshake begins.
    PortOpened,
    /// The handshake succeeded with a compatible device.
    HandshakeOk(ConnectedDevice),
    /// The handshake revealed an unsupported protocol major version.
    HandshakeIncompatible {
        /// The device's reported major version.
        device_major: u16,
    },
    /// The handshake exceeded its deadline.
    HandshakeTimeout,
    /// A serial read/write failure.
    IoError,
    /// The heartbeat missed its threshold of consecutive replies.
    HeartbeatTimeout,
    /// The device/port was removed.
    PortRemoved,
    /// The reconnect backoff interval elapsed.
    BackoffElapsed,
}

impl ManagerEvent {
    fn as_connection_event(&self) -> ConnectionEvent {
        match self {
            ManagerEvent::PortOpened => ConnectionEvent::PortOpened,
            ManagerEvent::HandshakeOk(_) => ConnectionEvent::HandshakeOk,
            ManagerEvent::HandshakeIncompatible { .. } => ConnectionEvent::HandshakeIncompatible,
            ManagerEvent::HandshakeTimeout => ConnectionEvent::HandshakeTimeout,
            ManagerEvent::IoError => ConnectionEvent::IoError,
            ManagerEvent::HeartbeatTimeout => ConnectionEvent::HeartbeatTimeout,
            ManagerEvent::PortRemoved => ConnectionEvent::PortRemoved,
            ManagerEvent::BackoffElapsed => ConnectionEvent::BackoffElapsed,
        }
    }
}

/// The connection manager's observable state.
#[derive(Debug, Clone, Default)]
pub struct ConnectionManager {
    state: ConnectionState,
    device: Option<ConnectedDevice>,
    retry_count: u32,
    incompatible_reason: Option<String>,
}

impl ConnectionManager {
    /// A fresh manager in the `Disconnected` state.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The current surfaced connection status.
    #[must_use]
    pub fn state(&self) -> ConnectionState {
        self.state
    }

    /// The connected-device summary, if `Connected`.
    #[must_use]
    pub fn device(&self) -> Option<&ConnectedDevice> {
        self.device.as_ref()
    }

    /// Consecutive failed-connection attempts (reset on a successful connect or a fresh/absent port).
    #[must_use]
    pub fn retry_count(&self) -> u32 {
        self.retry_count
    }

    /// A human-readable reason, present only in the `Incompatible` state.
    #[must_use]
    pub fn incompatible_reason(&self) -> Option<&str> {
        self.incompatible_reason.as_deref()
    }

    /// Applies `event`. Returns `true` if it caused a legal transition; `false` if the event was not
    /// legal in the current state (it is then ignored).
    pub fn apply(&mut self, event: ManagerEvent) -> bool {
        let Some(next) = self.state.transition(event.as_connection_event()) else {
            return false;
        };

        match &event {
            ManagerEvent::HandshakeOk(device) => {
                self.device = Some(device.clone());
                self.incompatible_reason = None;
                self.retry_count = 0;
            }
            ManagerEvent::HandshakeIncompatible { device_major } => {
                self.device = None;
                self.incompatible_reason = Some(format!(
                    "device protocol major v{device_major} is unsupported \
                     (host supports v{PROTOCOL_MAJOR})"
                ));
            }
            // Leaving `Incompatible` for another candidate port.
            ManagerEvent::PortOpened => self.incompatible_reason = None,
            _ => {}
        }

        match next {
            ConnectionState::Error => self.retry_count += 1,
            ConnectionState::Disconnected => {
                self.device = None;
                self.incompatible_reason = None;
                // A removed/absent port is a fresh start; a backoff-elapsed retry keeps the count so
                // the next attempt's backoff keeps escalating.
                if matches!(event, ManagerEvent::PortRemoved) {
                    self.retry_count = 0;
                }
            }
            _ => {}
        }

        self.state = next;
        true
    }
}
