use kivori_desktop::platform::{
    ActionAvailability, BackendError, ConfirmationClass, FakeVolumeBackend, VolumeBackend,
};

/// Restores the endpoint's original volume when dropped, including on panic.
///
/// A test that changes a real person's system volume MUST put it back. `Drop` runs
/// during unwind, so an assertion failure mid-test still restores.
pub struct VolumeGuard<'a> {
    backend: &'a dyn VolumeBackend,
    original: u8,
}

impl<'a> VolumeGuard<'a> {
    pub fn capture(backend: &'a dyn VolumeBackend) -> Option<Self> {
        let original = backend.read().ok()?;
        Some(Self { backend, original })
    }
    pub const fn original(&self) -> u8 {
        self.original
    }
}

impl Drop for VolumeGuard<'_> {
    fn drop(&mut self) {
        let _ = self.backend.set(self.original);
    }
}

/// NON-DESTRUCTIVE contract. Safe against a real endpoint: it captures the original
/// value, nudges it by a small amount, verifies read-back, and restores.
///
/// Exact-boundary behaviour (0 and 100) is NOT exercised here — slamming a real
/// user's speakers to silent or full is not an acceptable test side effect. Those
/// assertions live in `assert_boundary_contract`, which runs on fakes only.
pub fn assert_backend_contract(backend: &dyn VolumeBackend) {
    let ActionAvailability::Available { confirmation } = backend.availability() else {
        panic!("contract suite requires an available backend");
    };
    assert_eq!(
        confirmation,
        ConfirmationClass::StateConfirmed,
        "a backend with read-back must report StateConfirmed"
    );

    let guard = VolumeGuard::capture(backend).expect("read original volume");
    let original = guard.original();

    // A small, safe nudge that stays well inside the range from any starting point.
    let target = if original >= 50 {
        original - 5
    } else {
        original + 5
    };

    // set() returns the READ-BACK value, never the requested one.
    let observed = backend.set(target).expect("set");
    assert_eq!(backend.read().expect("read"), observed);
    assert!(
        observed.abs_diff(target) <= 2,
        "observed {observed} should track requested {target} within quantisation"
    );

    // `guard` restores the original volume here, or on unwind if an assert above failed.
}

/// DESTRUCTIVE exact-boundary contract. Fakes only — never a real endpoint.
pub fn assert_boundary_contract(backend: &dyn VolumeBackend) {
    assert_eq!(backend.set(0).expect("set 0"), 0);
    assert_eq!(backend.set(100).expect("set 100"), 100);
}

#[test]
fn fake_backend_satisfies_the_contract() {
    assert_backend_contract(&FakeVolumeBackend::new(25));
}

#[test]
fn fake_backend_satisfies_the_exact_boundary_contract() {
    assert_boundary_contract(&FakeVolumeBackend::new(25));
}

#[test]
fn the_volume_guard_restores_the_original_value_even_on_panic() {
    let backend = FakeVolumeBackend::new(42);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _guard = VolumeGuard::capture(&backend).expect("capture");
        backend.set(7).expect("set");
        panic!("simulated test failure");
    }));
    assert!(result.is_err());
    assert_eq!(
        backend.read().expect("read"),
        42,
        "the guard must restore the original volume during unwind"
    );
}

#[test]
fn set_returns_read_back_not_the_request() {
    // A backend that quantises must report what it actually achieved.
    let backend = FakeVolumeBackend::quantised(25, /* step */ 5);
    assert_eq!(backend.set(52).expect("set"), 50);
    assert_eq!(backend.read().expect("read"), 50);
}

#[test]
fn an_unavailable_backend_reports_runtime_unavailable_and_refuses_reads() {
    let backend = FakeVolumeBackend::with_availability(ActionAvailability::RuntimeUnavailable {
        reason: "no default render endpoint".to_string(),
    });
    assert!(matches!(
        backend.availability(),
        ActionAvailability::RuntimeUnavailable { .. }
    ));
    assert_eq!(backend.read(), Err(BackendError::NoEndpoint));
}

#[test]
fn a_write_without_readback_is_reported_distinctly() {
    let backend = FakeVolumeBackend::unreadable_after_write(30);
    assert_eq!(backend.set(40), Err(BackendError::ReadBackUnavailable));
}

#[test]
fn external_change_is_observable_without_a_kivori_write() {
    let backend = FakeVolumeBackend::new(10);
    backend.external_change(77);
    assert_eq!(backend.read().expect("read"), 77);
}

#[test]
fn the_unimplemented_backend_says_not_implemented_yet_not_unsupported() {
    // A platform Kivori has not been built for must NOT claim the OS cannot do it.
    let backend = kivori_desktop::platform::unimplemented::UnimplementedVolumeBackend::new("macos");
    match backend.availability() {
        ActionAvailability::NotImplementedYet { target } => assert_eq!(target, "macos"),
        other => panic!("expected NotImplementedYet, got {other:?}"),
    }
}

#[cfg(windows)]
mod windows_backend {
    use super::assert_backend_contract;
    use kivori_desktop::platform::windows::WindowsVolumeBackend;
    use kivori_desktop::platform::{ActionAvailability, VolumeBackend};

    /// GitHub-hosted Windows runners generally expose NO audio endpoint. This test
    /// therefore SKIPS loudly rather than passing silently — a skip is evidence of
    /// absence, not evidence of correctness.
    ///
    /// Runs ONLY the non-destructive contract: it captures the user's current volume,
    /// nudges it by 5 points, verifies read-back, and restores via `VolumeGuard`,
    /// including on unwind. `assert_boundary_contract` is NEVER called here — a test
    /// must not slam a real person's output to silent or full.
    #[test]
    fn real_backend_satisfies_the_non_destructive_contract_when_an_endpoint_exists() {
        let backend = WindowsVolumeBackend::new();
        match backend.availability() {
            ActionAvailability::Available { .. } => assert_backend_contract(&backend),
            other => {
                eprintln!(
                    "SKIPPED: no default render endpoint on this machine ({other:?}). \
                     Real Core Audio behaviour is PHYSICAL WINDOWS EVIDENCE, recorded in \
                     docs/features/002-rotary-volume-control/validation-checklist.md"
                );
            }
        }
    }

    /// Belt and braces: prove the endpoint is back where the user left it.
    #[test]
    fn the_real_endpoint_volume_is_unchanged_after_the_contract_run() {
        let backend = WindowsVolumeBackend::new();
        let ActionAvailability::Available { .. } = backend.availability() else {
            eprintln!("SKIPPED: no default render endpoint on this machine.");
            return;
        };
        let before = backend.read().expect("read before");
        assert_backend_contract(&backend);
        assert_eq!(
            backend.read().expect("read after"),
            before,
            "the contract run must leave the user's volume exactly as it found it"
        );
    }

    #[test]
    fn construction_without_an_endpoint_reports_runtime_unavailable_not_a_panic() {
        let backend = WindowsVolumeBackend::new();
        assert!(
            !matches!(
                backend.availability(),
                ActionAvailability::NotImplementedYet { .. }
            ),
            "Windows IS implemented; absence of an endpoint is RuntimeUnavailable"
        );
    }
}
