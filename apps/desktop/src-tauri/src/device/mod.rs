//! The desktop device connection manager (Phase 9; US1/US3).
//!
//! Discovery filtering, handshake verification, the connection state machine, heartbeat, and reconnect
//! are all pure, host-testable logic driven by injected events (`cargo test -p kivori-desktop`). The
//! concrete serial adapter (research R-5) and the async run loop that ties these together land with
//! the runtime wiring; nothing in this module performs I/O.

pub mod connection;
pub mod discovery;
pub mod fsm;
pub mod heartbeat;
pub mod nonce;
pub mod reconnect;
pub mod serial;
pub mod session;
pub mod transport;

pub use connection::{
    build_hello, hash_device_id_short, summarize, verify_handshake, ConnectedDevice,
};
pub use discovery::{
    filter_candidates, is_candidate, PortCandidate, UsbId, DEFAULT_ALLOWLIST, KIVORI_PID,
    KIVORI_VID,
};
pub use fsm::{ConnectionManager, ManagerEvent};
pub use heartbeat::{HeartbeatMonitor, DEFAULT_MISS_THRESHOLD};
pub use reconnect::{base_delay_ms, with_jitter, Backoff, BASE_MS, MAX_MS};
pub use session::{Session, SessionConfig, SessionError};
pub use transport::SerialLink;
