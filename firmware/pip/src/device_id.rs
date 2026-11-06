//! Per-unit device identity derivation (hardware-neutral; issue #24).
//!
//! The raw flash unique ID never leaves the chip: it is passed through a domain-separated SHA-256
//! and truncated to the 16-byte wire `device_id`. The physical runtime reads the 64-bit ID of the
//! flash chip (the RP2040 itself has none) and calls [`derive_device_id`]; this module touches no
//! peripheral so it is host-testable.

use kivori_protocol::DeviceId;
use sha2::{Digest, Sha256};

/// Domain-separation prefix; bump the version suffix if the derivation ever changes.
const DOMAIN: &[u8] = b"kivori-device-id-v1";

/// Derives the stable wire `device_id` from the flash chip's 64-bit unique ID. The source kind is
/// hashed in so an ID of another origin can never collide with one that shares its bytes.
pub fn derive_device_id(flash_unique_id: &[u8; 8]) -> DeviceId {
    let mut hasher = Sha256::new();
    hasher.update(DOMAIN);
    hasher.update(b"F");
    hasher.update(flash_unique_id);
    let digest = hasher.finalize();
    let mut id = [0u8; 16];
    id.copy_from_slice(&digest[..16]);
    id
}

/// The seed for the buddy's idle life: the device id folded to 32 bits, so each unit blinks and
/// glances on its own schedule while every run of the same unit repeats exactly.
pub fn idle_seed(id: &DeviceId) -> u32 {
    id.as_chunks::<4>()
        .0
        .iter()
        .fold(0, |seed, word| seed ^ u32::from_le_bytes(*word))
}

#[cfg(test)]
mod tests {
    use super::*;

    const UID_A: [u8; 8] = [0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88];
    const UID_B: [u8; 8] = [0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x89];

    #[test]
    fn derivation_is_deterministic() {
        assert_eq!(derive_device_id(&UID_A), derive_device_id(&UID_A));
    }

    #[test]
    fn different_unique_ids_give_different_ids() {
        assert_ne!(derive_device_id(&UID_A), derive_device_id(&UID_B));
    }

    #[test]
    fn idle_seed_differs_per_unit_and_repeats_per_unit() {
        let a = derive_device_id(&UID_A);
        let b = derive_device_id(&UID_B);
        assert_eq!(idle_seed(&a), idle_seed(&a));
        assert_ne!(idle_seed(&a), idle_seed(&b));
    }

    #[test]
    fn output_is_not_the_raw_input_even_for_an_all_zero_id() {
        let id = derive_device_id(&UID_A);
        assert_ne!(&id[..8], &UID_A[..]);
        assert_ne!(derive_device_id(&[0u8; 8]), [0u8; 16]);
    }
}
