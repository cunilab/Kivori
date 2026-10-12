//! Product identity (ADR-0014, ADR-0016 tier A): what this firmware build is, what the chip says it
//! is, and what the device does when the two disagree. Hardware-neutral; the board adapter only
//! supplies the raw chip record bytes.

use kivori_protocol::identity::{ChipRecord, ChipRecordState};
use kivori_protocol::IdentitySource;

/// Parses a decimal at compile time; a malformed value fails the build instead of shipping wrong.
const fn parse_u16(text: &str) -> u16 {
    let bytes = text.as_bytes();
    assert!(!bytes.is_empty(), "empty number");
    let mut value: u16 = 0;
    let mut i = 0;
    while i < bytes.len() {
        assert!(bytes[i].is_ascii_digit(), "non-numeric identity number");
        value = value * 10 + (bytes[i] - b'0') as u16;
        i += 1;
    }
    value
}

/// The product number of this build, from `products/registry.toml` (via `build.rs`).
pub const BUILD_PRODUCT: u16 = parse_u16(env!("KIVORI_PRODUCT_NUM"));
/// The board number of this build, from `products/registry.toml` (via `build.rs`).
pub const BUILD_BOARD: u16 = parse_u16(env!("KIVORI_BOARD_NUM"));

/// The image identity record: searchable text next to the version marker, so tooling can tell which
/// product and board a built `.elf` is for without running it. Format:
/// `KIVORI-FW-IDENTITY:p=<product>;b=<board>;v=<x.y.z>;m=<major.minor>\0`, where `m` is the oldest
/// wire protocol the image speaks (the catalog's `min_protocol`).
pub const IMAGE_RECORD: &str = concat!(
    "KIVORI-FW-IDENTITY:p=",
    env!("KIVORI_PRODUCT_NUM"),
    ";b=",
    env!("KIVORI_BOARD_NUM"),
    ";v=",
    env!("CARGO_PKG_VERSION"),
    ";m=",
    env!("KIVORI_MIN_PROTOCOL"),
    "\0"
);

/// Which product and board a device is, and where that was learned.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProductIdentity {
    /// Product wire number.
    pub product: u16,
    /// Board wire number.
    pub board: u16,
    /// Hardware revision, `None` until provisioning burned one.
    pub hw_rev: Option<u8>,
    /// Where the numbers came from.
    pub source: IdentitySource,
}

impl ProductIdentity {
    /// The identity compiled into this firmware build.
    pub const BUILD: ProductIdentity = ProductIdentity {
        product: BUILD_PRODUCT,
        board: BUILD_BOARD,
        hw_rev: None,
        source: IdentitySource::Build,
    };
}

/// The boot decision from the build identity and the chip's identity record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BootCheck {
    /// Run the product, reporting this identity.
    Run(ProductIdentity),
    /// The chip was provisioned for something else: run nothing, show the wrong-board screen and
    /// report this (the chip's) identity so the app can offer the right firmware.
    WrongBoard(ProductIdentity),
}

/// Compares the build identity with the chip's record. Only a valid record can contradict the
/// build; an empty or unreadable one never blocks the product (the unit simply is not provisioned).
#[must_use]
pub fn check_boot(build: ProductIdentity, chip: ChipRecordState) -> BootCheck {
    match chip {
        ChipRecordState::Valid(ChipRecord {
            product,
            board,
            hw_rev,
        }) => {
            let reported = ProductIdentity {
                product,
                board,
                hw_rev,
                source: IdentitySource::Chip,
            };
            if product == build.product && board == build.board {
                BootCheck::Run(reported)
            } else {
                BootCheck::WrongBoard(reported)
            }
        }
        ChipRecordState::NotProvisioned | ChipRecordState::Invalid => BootCheck::Run(build),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const REC: ChipRecord = ChipRecord {
        product: BUILD_PRODUCT,
        board: BUILD_BOARD,
        hw_rev: Some(2),
    };

    #[test]
    fn build_identity_matches_the_registry_numbers() {
        // pala 1 on c3-supermini 1 (products/registry.toml); `build.rs` generates them.
        assert_eq!((BUILD_PRODUCT, BUILD_BOARD), (1, 1));
        assert_ne!(BUILD_PRODUCT, 0);
        assert_ne!(BUILD_BOARD, 0);
    }

    #[test]
    fn image_record_names_product_board_version_and_min_protocol() {
        assert_eq!(
            IMAGE_RECORD,
            concat!(
                "KIVORI-FW-IDENTITY:p=1;b=1;v=",
                env!("CARGO_PKG_VERSION"),
                ";m=0.1\0"
            )
        );
    }

    #[test]
    fn min_protocol_is_not_newer_than_the_protocol_the_firmware_speaks() {
        let (major, minor) = env!("KIVORI_MIN_PROTOCOL").split_once('.').unwrap();
        let (major, minor): (u16, u16) = (major.parse().unwrap(), minor.parse().unwrap());
        assert_eq!(major, kivori_protocol::PROTOCOL_MAJOR);
        assert!(minor <= kivori_protocol::PROTOCOL_MINOR);
    }

    #[test]
    fn unprovisioned_or_unreadable_chips_run_as_the_build() {
        for chip in [ChipRecordState::NotProvisioned, ChipRecordState::Invalid] {
            assert_eq!(
                check_boot(ProductIdentity::BUILD, chip),
                BootCheck::Run(ProductIdentity::BUILD)
            );
        }
        assert_eq!(ProductIdentity::BUILD.source, IdentitySource::Build);
        assert_eq!(ProductIdentity::BUILD.hw_rev, None);
    }

    #[test]
    fn a_matching_record_runs_and_reports_the_chip() {
        assert_eq!(
            check_boot(ProductIdentity::BUILD, ChipRecordState::Valid(REC)),
            BootCheck::Run(ProductIdentity {
                product: BUILD_PRODUCT,
                board: BUILD_BOARD,
                hw_rev: Some(2),
                source: IdentitySource::Chip,
            })
        );
    }

    #[test]
    fn a_record_for_another_product_or_board_is_wrong_board() {
        for other in [
            ChipRecord {
                product: BUILD_PRODUCT + 1,
                ..REC
            },
            ChipRecord {
                board: BUILD_BOARD + 1,
                ..REC
            },
        ] {
            let BootCheck::WrongBoard(reported) =
                check_boot(ProductIdentity::BUILD, ChipRecordState::Valid(other))
            else {
                panic!("expected wrong board for {other:?}");
            };
            assert_eq!(
                (reported.product, reported.board, reported.source),
                (other.product, other.board, IdentitySource::Chip)
            );
        }
    }
}
