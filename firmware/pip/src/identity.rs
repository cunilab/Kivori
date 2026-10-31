//! Product identity (ADR-0014, ADR-0016 tier A): what this firmware build is, what the chip says it
//! is, and what the device does when the two disagree. Hardware-neutral; the board adapter only
//! supplies the raw chip record bytes.

use kivori_protocol::identity::{ChipRecord, ChipRecordState};
use kivori_protocol::{IdentitySource, SOURCE_ID_LEN};

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

/// Reads the 12 hex digits `build.rs` found (`unknown` without git) into the wire form; anything
/// else fails the build instead of shipping a wrong id.
const fn parse_source_id(text: &str) -> Option<[u8; SOURCE_ID_LEN]> {
    let bytes = text.as_bytes();
    if bytes.len() == 7 && bytes[0] == b'u' {
        return None;
    }
    assert!(
        bytes.len() == 2 * SOURCE_ID_LEN,
        "build source id must be 12 hex digits"
    );
    let mut out = [0u8; SOURCE_ID_LEN];
    let mut i = 0;
    while i < bytes.len() {
        let digit = match bytes[i] {
            b @ b'0'..=b'9' => b - b'0',
            b @ b'a'..=b'f' => b - b'a' + 10,
            _ => panic!("build source id must be lowercase hex"),
        };
        out[i / 2] = (out[i / 2] << 4) | digit;
        i += 1;
    }
    Some(out)
}

/// Which exact firmware source this build is: the content hash of its committed input files (`None`
/// without git) and whether the tree had uncommitted changes in the firmware's files. Reported once per
/// session as `BuildInfo` (`BUILD_INFO_V1`), so a developer can tell a unit runs a build that differs
/// from the repository.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuildId {
    /// The 12-hex-digit source id packed into 6 bytes, or `None`.
    pub source_id: Option<[u8; SOURCE_ID_LEN]>,
    /// Uncommitted changes under the firmware's build paths.
    pub dirty: bool,
}

impl BuildId {
    /// The build id compiled into this firmware (`build.rs`: `KIVORI_BUILD_SOURCE`, `KIVORI_BUILD_DIRTY`).
    pub const BUILD: BuildId = BuildId {
        source_id: parse_source_id(env!("KIVORI_BUILD_SOURCE")),
        dirty: parse_u16(env!("KIVORI_BUILD_DIRTY")) != 0,
    };
}

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
        // pip 1 on c3-supermini 1 (products/registry.toml); `build.rs` generates them.
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
    fn parse_source_id_reads_twelve_hex_digits_or_unknown() {
        assert_eq!(
            parse_source_id("c2ff8c4f68de"),
            Some([0xc2, 0xff, 0x8c, 0x4f, 0x68, 0xde])
        );
        assert_eq!(parse_source_id("000000000001"), Some([0, 0, 0, 0, 0, 1]));
        assert_eq!(parse_source_id("unknown"), None);
    }

    #[test]
    fn the_compiled_build_id_is_either_a_source_id_or_unknown_never_dirty_without_one() {
        let build = BuildId::BUILD;
        if build.source_id.is_none() {
            assert!(!build.dirty, "an unknown source id cannot be dirty");
        }
        // build.rs exports the same facts the const parses.
        let text = env!("KIVORI_BUILD_SOURCE");
        assert_eq!(build.source_id.is_some(), text != "unknown");
        assert!(matches!(env!("KIVORI_BUILD_DIRTY"), "0" | "1"));
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
