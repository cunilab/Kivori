use kivori_desktop::device::nonce::{
    FailingNonceSource, FixedNonceSource, NonceError, NonceSource, OsNonceSource,
};

// NOTE: there is deliberately NO test asserting that two OS-random nonces differ.
// That assertion is probabilistic and would flake roughly 1 in 2^32 runs while
// proving nothing a deterministic source cannot prove. Session-freshness BEHAVIOUR
// is tested with FixedNonceSource; OsNonceSource gets a smoke and an error-path test.

#[test]
fn os_nonce_source_smoke_produces_a_value() {
    let mut src = OsNonceSource;
    assert!(
        src.next_nonce().is_ok(),
        "OS entropy must be available on a desktop"
    );
}

#[test]
fn os_nonce_source_can_be_called_repeatedly_without_error() {
    let mut src = OsNonceSource;
    for _ in 0..16 {
        src.next_nonce().expect("OS entropy remains available");
    }
}

#[test]
fn fixed_nonce_source_is_deterministic_for_tests() {
    let mut src = FixedNonceSource::new(vec![10, 20, 30]);
    assert_eq!(src.next_nonce(), Ok(10));
    assert_eq!(src.next_nonce(), Ok(20));
    assert_eq!(src.next_nonce(), Ok(30));
    // Exhausted sequences hold the last value rather than panicking mid-test.
    assert_eq!(src.next_nonce(), Ok(30));
}

/// A nonce source that cannot produce a value must fail the CONNECTION ATTEMPT,
/// never crash the desktop process.
#[test]
fn a_failing_nonce_source_reports_an_error_rather_than_panicking() {
    let mut src = FailingNonceSource;
    assert_eq!(src.next_nonce(), Err(NonceError::Unavailable));
}
