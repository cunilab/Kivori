//! Physical input: pure, host-testable decoding and gesture formation.
//!
//! Everything here sits ABOVE the `InputSource` port, so it is provable without hardware.

/// Gesture formation on top of validated detents: identity and the inactivity boundary.
pub mod gesture;
pub mod quadrature;
