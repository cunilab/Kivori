//! Connection lifecycle state and its pure transition rules (data-model §2; plan "connection state
//! machine"; FR-005).
//!
//! This is the transport/link axis, distinct from the companion-state axis. The rules here are a pure
//! function of `(state, event)`; the runtime actor (discovery, timers, backoff, serial I/O) is built
//! on top of this in the desktop crate's `device` module.

/// Desktop-side connection lifecycle status — exactly the five statuses surfaced to the UI (FR-005).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ConnectionState {
    /// No device connected. Internal scanning is surfaced as this state.
    #[default]
    Disconnected,
    /// A candidate port is open and the handshake is in progress.
    Connecting,
    /// Handshake succeeded with a compatible device.
    Connected,
    /// A device was identified but its protocol major version is unsupported. Left when that device
    /// is removed, or when the runtime closes its link and tries another candidate port.
    Incompatible,
    /// A recoverable error (I/O failure, handshake timeout, port busy, heartbeat timeout).
    Error,
}

/// Events that drive [`ConnectionState`] transitions (data-model §2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ConnectionEvent {
    /// A candidate port was opened and the handshake started (also from `Incompatible`, once the
    /// runtime has moved on to a different candidate).
    PortOpened,
    /// The handshake completed successfully with a compatible device.
    HandshakeOk,
    /// The handshake revealed an unsupported protocol major version.
    HandshakeIncompatible,
    /// The handshake did not complete within its deadline.
    HandshakeTimeout,
    /// A serial read/write failure occurred.
    IoError,
    /// The heartbeat missed its threshold of consecutive replies.
    HeartbeatTimeout,
    /// The device/port was removed.
    PortRemoved,
    /// The reconnect backoff interval elapsed.
    BackoffElapsed,
}

impl ConnectionState {
    /// Applies `event`, returning the next state for a valid transition, or `None` if the event is
    /// not legal in the current state (the caller rejects/ignores it).
    #[must_use]
    pub const fn transition(self, event: ConnectionEvent) -> Option<ConnectionState> {
        use ConnectionEvent as E;
        use ConnectionState as S;
        match (self, event) {
            (S::Disconnected, E::PortOpened) => Some(S::Connecting),
            (S::Connecting, E::HandshakeOk) => Some(S::Connected),
            (S::Connecting, E::HandshakeIncompatible) => Some(S::Incompatible),
            (S::Connecting, E::HandshakeTimeout | E::IoError) => Some(S::Error),
            (S::Connecting, E::PortRemoved) => Some(S::Disconnected),
            (S::Connected, E::IoError | E::HeartbeatTimeout) => Some(S::Error),
            (S::Connected, E::PortRemoved) => Some(S::Disconnected),
            (S::Error, E::BackoffElapsed | E::PortRemoved) => Some(S::Disconnected),
            (S::Incompatible, E::PortRemoved) => Some(S::Disconnected),
            // The runtime closed the incompatible device's link and opened another candidate port.
            (S::Incompatible, E::PortOpened) => Some(S::Connecting),
            _ => None,
        }
    }

    /// Returns `true` only in the `Connected` state — the sole state in which the desktop may drive
    /// companion states to the device.
    #[must_use]
    pub const fn can_drive_device(self) -> bool {
        matches!(self, ConnectionState::Connected)
    }
}
