//! Physical input: pure, host-testable decoding and gesture formation.
//!
//! Everything here sits ABOVE the `InputSource` port, so it is provable without hardware.

/// Push-switch debounce and Press / Hold / recovery-hold formation.
pub mod button;
/// Gesture formation on top of validated detents: identity and the inactivity boundary.
pub mod gesture;
/// HW-040 pin-to-channel mapping (measured on hardware).
pub mod hw040;
pub mod quadrature;
