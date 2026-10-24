//! Windows discovery can fail for one endpoint without hiding the endpoints we can still use.

use super::{MicDevice, MicMute};

type Scan<D> = Result<Vec<Result<(String, D), ()>>, ()>;

/// Reconcile only identities actually observed. An unidentified entry may be an existing
/// endpoint, so retain previous bindings until a complete identity scan proves they are gone.
pub(super) fn reconcile<D, E>(
    endpoints: &mut Vec<E>,
    scan: Scan<D>,
    id: impl Fn(&E) -> &str,
    mut bind: impl FnMut(D, String) -> Option<E>,
) -> bool {
    let Ok(entries) = scan else {
        return false;
    };
    let identities_complete = entries.iter().all(Result::is_ok);
    let mut complete = identities_complete;
    let mut present = Vec::new();
    for entry in entries {
        let Ok((key, device)) = entry else { continue };
        if !endpoints.iter().any(|endpoint| id(endpoint) == key) {
            if let Some(endpoint) = bind(device, key.clone()) {
                endpoints.push(endpoint);
            } else {
                complete = false;
            }
        }
        present.push(key);
    }
    if identities_complete {
        endpoints.retain(|endpoint| present.iter().any(|key| key == id(endpoint)));
    }
    complete
}

/// A single unknown entry is enough to prevent aggregate mute and action confirmation.
pub(super) fn observed(mut devices: Vec<MicDevice>, complete: bool) -> Vec<MicDevice> {
    if !complete {
        devices.push(MicDevice {
            name: "Unverified microphone coverage".to_string(),
            mute: MicMute::Unreadable,
            capturing: None,
        });
    }
    devices
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::mics_muted;

    struct Endpoint {
        id: String,
        mute: MicMute,
        notifications: bool,
    }

    fn scan(endpoints: &mut Vec<Endpoint>, entries: Scan<(bool, bool)>) -> bool {
        reconcile(
            endpoints,
            entries,
            |e| &e.id,
            |(activate, notify), id| {
                activate.then_some(Endpoint {
                    id,
                    mute: MicMute::Live,
                    notifications: notify,
                })
            },
        )
    }

    fn snapshot(endpoints: &[Endpoint], complete: bool) -> Vec<MicDevice> {
        observed(
            endpoints
                .iter()
                .map(|e| MicDevice {
                    name: e.id.clone(),
                    mute: e.mute,
                    capturing: None,
                })
                .collect(),
            complete && endpoints.iter().all(|e| e.notifications),
        )
    }

    fn muted_a() -> Vec<Endpoint> {
        vec![Endpoint {
            id: "a".into(),
            mute: MicMute::Muted,
            notifications: true,
        }]
    }

    #[test]
    fn incomplete_discovery_never_confirms_all_muted_and_keeps_healthy_devices() {
        // Collection Item and GetId failures both enter discovery as an unknown identity.
        // Activation failure has an identity, but no usable volume interface.
        for failed_b in [Err(()), Ok(("b".into(), (false, true)))] {
            let mut endpoints = muted_a();
            let complete = scan(
                &mut endpoints,
                Ok(vec![Ok(("a".into(), (true, true))), failed_b]),
            );
            assert!(!complete);
            assert_eq!(endpoints.len(), 1);
            assert_eq!(mics_muted(&snapshot(&endpoints, complete)), None);
            endpoints[0].mute = MicMute::Live;
            assert_eq!(mics_muted(&snapshot(&endpoints, complete)), Some(false));
        }
    }

    #[test]
    fn failed_scan_invalidates_muted_state_then_success_restores_certainty() {
        let mut endpoints = muted_a();
        assert_eq!(mics_muted(&snapshot(&endpoints, true)), Some(true));
        let complete = scan(&mut endpoints, Err(()));
        assert_eq!(mics_muted(&snapshot(&endpoints, complete)), None);
        assert_eq!(endpoints.len(), 1, "healthy binding remains controllable");
        let complete = scan(&mut endpoints, Ok(vec![Ok(("a".into(), (true, true)))]));
        assert_eq!(mics_muted(&snapshot(&endpoints, complete)), Some(true));
        let complete = scan(&mut endpoints, Ok(vec![]));
        assert!(
            snapshot(&endpoints, complete).is_empty(),
            "known no devices stays empty"
        );
    }

    #[test]
    fn callback_failure_preserves_read_write_but_not_confirmation() {
        let mut endpoints = muted_a();
        let complete = scan(
            &mut endpoints,
            Ok(vec![
                Ok(("a".into(), (true, true))),
                Ok(("b".into(), (true, false))),
            ]),
        );
        assert_eq!(endpoints.len(), 2);
        for endpoint in &mut endpoints {
            endpoint.mute = MicMute::Muted;
        }
        assert_eq!(mics_muted(&snapshot(&endpoints, complete)), None);
        endpoints[1].mute = MicMute::Live;
        assert_eq!(mics_muted(&snapshot(&endpoints, complete)), Some(false));
        endpoints[1].notifications = true;
        endpoints[1].mute = MicMute::Muted;
        assert_eq!(mics_muted(&snapshot(&endpoints, complete)), Some(true));
    }

    #[test]
    fn unknown_identity_does_not_drop_previously_controllable_endpoint() {
        let mut endpoints = muted_a();
        assert!(!scan(&mut endpoints, Ok(vec![Err(())])));
        assert_eq!(endpoints[0].id, "a");
    }
}
