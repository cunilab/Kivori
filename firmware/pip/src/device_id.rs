//! Per-unit device identity derivation (hardware-neutral; issue #24).
//!
//! The raw flash unique ID never leaves the chip: it is passed through a domain-separated SHA-256
//! and truncated to the 16-byte wire `device_id`. The physical runtime reads the 64-bit ID of the
//! flash chip (the RP2040 itself has none) and calls [`derive_device_id`]; this module touches no
//! peripheral so it is host-testable.

use core::fmt::Write as _;

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

/// The USB serial number the host shows: the derived device id as 32 uppercase hex digits, never the
/// raw flash id.
pub fn usb_serial_number(id: &DeviceId) -> heapless::String<32> {
    const _: () = assert!(core::mem::size_of::<DeviceId>() * 2 <= 32);
    let mut text = heapless::String::new();
    for byte in id {
        let _ = write!(text, "{byte:02X}");
    }
    text
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

    /// `sha256("kivori-device-id-v1" "F" uid)[..16]`, worked out outside the firmware. A change
    /// here renames every unit in the field (its desktop pairing, its idle seed, its USB serial).
    const ID_A: DeviceId = [
        0xE0, 0xAB, 0xD0, 0x21, 0x3E, 0x05, 0xB4, 0x5E, 0x4E, 0xD9, 0x00, 0xD2, 0x8A, 0xE4, 0x87,
        0xDE,
    ];

    #[test]
    fn the_derivation_is_pinned_so_a_unit_keeps_its_id_across_firmware_updates() {
        assert_eq!(derive_device_id(&UID_A), ID_A);
        assert_eq!(idle_seed(&ID_A), 0x73E3_931A);
        let zero = derive_device_id(&[0u8; 8]);
        assert_eq!(
            usb_serial_number(&zero).as_str(),
            "AD4D6261BE25F4B973DA74E7B56F2CB3"
        );
    }

    #[test]
    fn the_domain_prefix_keeps_the_id_apart_from_a_plain_hash_of_the_unique_id() {
        let plain = Sha256::digest(UID_A);
        assert_ne!(derive_device_id(&UID_A)[..], plain[..16]);
    }

    #[test]
    fn flipping_any_one_bit_of_the_unique_id_changes_the_id() {
        let base = derive_device_id(&UID_A);
        for bit in 0..64 {
            let mut uid = UID_A;
            uid[bit / 8] ^= 1 << (bit % 8);
            let id = derive_device_id(&uid);
            assert_ne!(id, base, "bit {bit}");
            // Not merely a different last byte: the hash spreads the change.
            let differing = id.iter().zip(&base).filter(|(a, b)| a != b).count();
            assert!(differing >= 8, "bit {bit} changed only {differing} bytes");
        }
    }

    #[test]
    fn the_raw_unique_id_appears_nowhere_in_the_id_or_the_usb_serial() {
        for uid in [UID_A, UID_B, [0u8; 8], [0xFF; 8]] {
            let id = derive_device_id(&uid);
            assert!(
                !id.windows(8).any(|w| w == uid),
                "raw id inside the device id"
            );
            let serial = usb_serial_number(&id);
            let mut raw_hex = heapless::String::<16>::new();
            for byte in uid {
                let _ = write!(raw_hex, "{byte:02X}");
            }
            assert!(
                !serial.contains(raw_hex.as_str()),
                "raw id inside the serial"
            );
        }
    }

    #[test]
    fn the_usb_serial_is_the_whole_id_as_uppercase_hex_and_fills_its_buffer_exactly() {
        let serial = usb_serial_number(&ID_A);
        assert_eq!(serial.as_str(), "E0ABD0213E05B45E4ED900D28AE487DE");
        assert_eq!(
            serial.len(),
            serial.capacity(),
            "32 digits, nothing truncated"
        );
        // Every id gives 32 hex digits, including ones with leading zero nibbles.
        for id in [[0u8; 16], [0x0F; 16], [0xA0; 16], [0xFF; 16]] {
            let serial = usb_serial_number(&id);
            assert_eq!(serial.len(), 32);
            assert!(serial
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'A'..=b'F').contains(&b)));
        }
        assert_eq!(
            usb_serial_number(&[0x0F; 16]).as_str(),
            "0F0F0F0F0F0F0F0F0F0F0F0F0F0F0F0F"
        );
    }

    #[test]
    fn different_units_show_different_serials() {
        let a = usb_serial_number(&derive_device_id(&UID_A));
        let b = usb_serial_number(&derive_device_id(&UID_B));
        assert_ne!(a, b);
    }
}
