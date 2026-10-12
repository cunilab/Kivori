//! The chip identity record (ADR-0014): the versioned bytes the provisioning tool burns into one-time
//! chip memory (the C3 user eFuse block) and firmware reads back at boot. This step only reads.
//!
//! Layout, 12 bytes, little-endian:
//!
//! | offset | size | field |
//! |--------|------|-------|
//! | 0 | 1 | format version ([`FORMAT_V1`]) |
//! | 1 | 2 | product wire number |
//! | 3 | 2 | board wire number |
//! | 5 | 1 | hardware revision (0 = unspecified) |
//! | 6 | 2 | reserved, zero |
//! | 8 | 4 | CRC-32 (the frame CRC, [`crc32`]) of bytes 0..8 |
//!
//! A region that is all zero is "not provisioned" (fresh eFuses read zero), which is why the format
//! version starts at 1.

use crate::frame::crc32;

/// Format version 1, the only one so far.
pub const FORMAT_V1: u8 = 1;
/// Encoded record length, in bytes.
pub const RECORD_LEN: usize = 12;

/// A decoded chip identity record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChipRecord {
    /// Product wire number.
    pub product: u16,
    /// Board wire number.
    pub board: u16,
    /// Hardware revision, `None` when unspecified (0).
    pub hw_rev: Option<u8>,
}

/// What a region of chip memory holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChipRecordState {
    /// Every byte is zero: nothing was ever burned.
    NotProvisioned,
    /// A valid record.
    Valid(ChipRecord),
    /// Something is there but it is not a record this firmware accepts (unknown format, bad CRC,
    /// non-zero reserved bytes, zero product or board). Never trusted.
    Invalid,
}

impl ChipRecord {
    /// Encodes the record. A `hw_rev` of `None` is stored as 0.
    #[must_use]
    pub fn encode(&self) -> [u8; RECORD_LEN] {
        let mut out = [0u8; RECORD_LEN];
        out[0] = FORMAT_V1;
        out[1..3].copy_from_slice(&self.product.to_le_bytes());
        out[3..5].copy_from_slice(&self.board.to_le_bytes());
        out[5] = self.hw_rev.unwrap_or(0);
        let crc = crc32(&out[..8]);
        out[8..12].copy_from_slice(&crc.to_le_bytes());
        out
    }

    /// Classifies and decodes a chip memory region (at least [`RECORD_LEN`] bytes are read; a
    /// shorter slice is `Invalid` unless empty-or-all-zero).
    #[must_use]
    pub fn decode(bytes: &[u8]) -> ChipRecordState {
        if bytes.iter().all(|&b| b == 0) {
            return ChipRecordState::NotProvisioned;
        }
        if bytes.len() < RECORD_LEN {
            return ChipRecordState::Invalid;
        }
        let b = &bytes[..RECORD_LEN];
        let crc = u32::from_le_bytes([b[8], b[9], b[10], b[11]]);
        let product = u16::from_le_bytes([b[1], b[2]]);
        let board = u16::from_le_bytes([b[3], b[4]]);
        if b[0] != FORMAT_V1 || b[6] != 0 || b[7] != 0 || crc != crc32(&b[..8]) {
            return ChipRecordState::Invalid;
        }
        if product == 0 || board == 0 {
            return ChipRecordState::Invalid;
        }
        ChipRecordState::Valid(ChipRecord {
            product,
            board,
            hw_rev: if b[5] == 0 { None } else { Some(b[5]) },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const REC: ChipRecord = ChipRecord {
        product: 1,
        board: 1,
        hw_rev: Some(2),
    };

    #[test]
    fn round_trips_and_pins_the_layout() {
        let bytes = REC.encode();
        assert_eq!(&bytes[..8], &[1, 1, 0, 1, 0, 2, 0, 0]);
        assert_eq!(
            u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]),
            crc32(&bytes[..8])
        );
        assert_eq!(ChipRecord::decode(&bytes), ChipRecordState::Valid(REC));
    }

    #[test]
    fn unspecified_revision_is_none() {
        let rec = ChipRecord {
            hw_rev: None,
            ..REC
        };
        assert_eq!(
            ChipRecord::decode(&rec.encode()),
            ChipRecordState::Valid(rec)
        );
    }

    #[test]
    fn all_zero_is_not_provisioned() {
        assert_eq!(
            ChipRecord::decode(&[0; 32]),
            ChipRecordState::NotProvisioned
        );
        assert_eq!(ChipRecord::decode(&[]), ChipRecordState::NotProvisioned);
    }

    #[test]
    fn a_larger_region_with_trailing_zeros_decodes() {
        let mut region = [0u8; 32];
        region[..RECORD_LEN].copy_from_slice(&REC.encode());
        assert_eq!(ChipRecord::decode(&region), ChipRecordState::Valid(REC));
    }

    #[test]
    fn every_single_bit_flip_is_rejected() {
        let good = REC.encode();
        for byte in 0..RECORD_LEN {
            for bit in 0..8 {
                let mut bad = good;
                bad[byte] ^= 1 << bit;
                assert_ne!(
                    ChipRecord::decode(&bad),
                    ChipRecordState::Valid(REC),
                    "flip {byte}:{bit}"
                );
                assert_eq!(ChipRecord::decode(&bad), ChipRecordState::Invalid);
            }
        }
    }

    #[test]
    fn unknown_format_short_and_zero_numbers_are_invalid() {
        let mut v2 = REC.encode();
        v2[0] = 2;
        let crc = crc32(&v2[..8]);
        v2[8..12].copy_from_slice(&crc.to_le_bytes());
        assert_eq!(ChipRecord::decode(&v2), ChipRecordState::Invalid);
        assert_eq!(
            ChipRecord::decode(&REC.encode()[..8]),
            ChipRecordState::Invalid
        );
        let zero_product = ChipRecord { product: 0, ..REC };
        assert_eq!(
            ChipRecord::decode(&zero_product.encode()),
            ChipRecordState::Invalid
        );
    }
}
