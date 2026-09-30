//! Session-nonce freshness.
//!
//! The handshake nonce doubles as connection-scoped session identity: it is
//! stamped on every `InputEvent` and `Presentation` so stale traffic from a
//! previous connection cannot be mistaken for current traffic.
//!
//! The nonce therefore MUST be distinct across desktop process restarts, not
//! merely within one process. It is a freshness token, NOT a security credential.

/// Why a nonce could not be produced.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NonceError {
    /// The OS entropy source was unavailable.
    Unavailable,
}

/// Supplies a fresh session nonce per handshake attempt.
///
/// Returns a `Result` deliberately: if the OS cannot provide randomness, the
/// correct behaviour is to fail THIS CONNECTION ATTEMPT and back off, not to
/// crash the desktop process out from under the user.
pub trait NonceSource: Send {
    /// Produces the next nonce, or `Err` if the source could not supply one.
    fn next_nonce(&mut self) -> Result<u32, NonceError>;
}

/// Production source: OS-provided randomness.
#[derive(Debug, Default, Clone, Copy)]
pub struct OsNonceSource;

impl NonceSource for OsNonceSource {
    fn next_nonce(&mut self) -> Result<u32, NonceError> {
        let mut buf = [0u8; 4];
        getrandom::getrandom(&mut buf).map_err(|_| NonceError::Unavailable)?;
        Ok(u32::from_le_bytes(buf))
    }
}

/// Deterministic source for tests.
#[derive(Debug, Clone)]
pub struct FixedNonceSource {
    values: Vec<u32>,
    index: usize,
}

impl FixedNonceSource {
    /// Creates a source that yields `values` in order, then holds the last value.
    ///
    /// # Panics
    /// Panics if `values` is empty.
    #[must_use]
    pub fn new(values: Vec<u32>) -> Self {
        assert!(
            !values.is_empty(),
            "FixedNonceSource needs at least one value"
        );
        Self { values, index: 0 }
    }
}

impl NonceSource for FixedNonceSource {
    fn next_nonce(&mut self) -> Result<u32, NonceError> {
        let v = self.values[self.index];
        if self.index + 1 < self.values.len() {
            self.index += 1;
        }
        Ok(v)
    }
}

/// Always fails. Exercises the connection-attempt error path.
#[derive(Debug, Default, Clone, Copy)]
pub struct FailingNonceSource;

impl NonceSource for FailingNonceSource {
    fn next_nonce(&mut self) -> Result<u32, NonceError> {
        Err(NonceError::Unavailable)
    }
}
