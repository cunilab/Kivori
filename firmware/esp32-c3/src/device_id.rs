//! Per-unit device identity derivation (hardware-neutral; issue #24).
//!
//! The raw factory eFuse bytes never leave the chip: they are passed through a domain-separated
//! SHA-256 and truncated to the 16-byte wire `device_id`. The physical runtime reads the eFuse and
//! calls [`derive_device_id`]; this module touches no peripheral so it is host-testable.

use kivori_protocol::DeviceId;
use sha2::{Digest, Sha256};

/// Domain-separation prefix; bump the version suffix if the derivation ever changes.
const DOMAIN: &[u8] = b"kivori-device-id-v1";

/// Derives the stable wire `device_id` from the factory eFuse data.
///
/// Prefers the 128-bit unique ID; an all-zero unique ID (older chip revisions that do not burn it)
/// falls back to the base MAC. The source kind is hashed in so a MAC can never collide with a
/// unique ID that happens to share its bytes.
pub fn derive_device_id(unique_id: &[u8; 16], base_mac: &[u8; 6]) -> DeviceId {
    let mut hasher = Sha256::new();
    hasher.update(DOMAIN);
    if unique_id.iter().any(|&b| b != 0) {
        hasher.update(b"U");
        hasher.update(unique_id);
    } else {
        hasher.update(b"M");
        hasher.update(base_mac);
    }
    let digest = hasher.finalize();
    let mut id = [0u8; 16];
    id.copy_from_slice(&digest[..16]);
    id
}

#[cfg(test)]
mod tests {
    use super::*;

    const UID_A: [u8; 16] = [
        0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xF0,
        0x01,
    ];
    const UID_B: [u8; 16] = [
        0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xF0,
        0x02,
    ];
    const MAC_A: [u8; 6] = [0x60, 0x55, 0xF9, 0x01, 0x02, 0x03];
    const MAC_B: [u8; 6] = [0x60, 0x55, 0xF9, 0x01, 0x02, 0x04];

    #[test]
    fn derivation_is_deterministic() {
        assert_eq!(
            derive_device_id(&UID_A, &MAC_A),
            derive_device_id(&UID_A, &MAC_A)
        );
    }

    #[test]
    fn different_unique_ids_give_different_ids() {
        assert_ne!(
            derive_device_id(&UID_A, &MAC_A),
            derive_device_id(&UID_B, &MAC_A)
        );
    }

    #[test]
    fn unique_id_takes_precedence_over_mac() {
        assert_eq!(
            derive_device_id(&UID_A, &MAC_A),
            derive_device_id(&UID_A, &MAC_B)
        );
    }

    #[test]
    fn all_zero_unique_id_falls_back_to_mac() {
        let zero = [0u8; 16];
        let a = derive_device_id(&zero, &MAC_A);
        assert_eq!(a, derive_device_id(&zero, &MAC_A));
        assert_ne!(a, derive_device_id(&zero, &MAC_B));
        assert_ne!(a, [0u8; 16]);
    }

    #[test]
    fn output_is_not_the_raw_input() {
        let id = derive_device_id(&UID_A, &MAC_A);
        assert_ne!(id, UID_A);
        let zero_id = derive_device_id(&[0u8; 16], &MAC_A);
        assert_ne!(&zero_id[..6], &MAC_A[..]);
    }
}
