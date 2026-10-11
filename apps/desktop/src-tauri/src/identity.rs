//! What the desktop knows about which product a connected device is (ADR-0014): the identity the
//! device reported (or the legacy default for one that reports nothing), its resolution to a
//! catalog entry, and the decisions that depend on it (the flashing guard and the recovery pick).
//!
//! Numbers resolve only through [`Catalog::by_wire`]; an unknown number is shown as such and never
//! guessed into a known product.

use kivori_protocol::{Identity, IdentitySource as WireSource};

use crate::catalog::{Board, Catalog, Product, UsbId};
use crate::firmware_marker::ImageIdentity;

/// The product a device that sends no `Identity` is (every unit shipped before `PRODUCT_IDENTITY_V1`).
const LEGACY_PRODUCT: &str = "desk-buddy";
/// The board a device that sends no `Identity` is.
const LEGACY_BOARD: &str = "c3-supermini";

/// Where a device's identity came from. `Legacy` exists only on the desktop: it is the answer for
/// a device that did not negotiate `PRODUCT_IDENTITY_V1`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdentitySource {
    /// The device sent no identity; assumed the original Desk Buddy on the C3 SuperMini.
    Legacy,
    /// Compiled into the device's firmware.
    Build,
    /// Read from the one-time identity record burned into the chip.
    Chip,
}

impl IdentitySource {
    /// The token the webview receives.
    #[must_use]
    pub const fn token(self) -> &'static str {
        match self {
            Self::Legacy => "legacy",
            Self::Build => "build",
            Self::Chip => "chip",
        }
    }

    /// Every source, in declaration order.
    pub const ALL: [Self; 3] = [Self::Legacy, Self::Build, Self::Chip];
}

/// The identity of a connected device. Connection scoped: it lives and dies with the session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeviceIdentity {
    /// Product wire number.
    pub product: u16,
    /// Board wire number.
    pub board: u16,
    /// Hardware revision, `None` until provisioning burned one.
    pub hw_rev: Option<u8>,
    /// Where the numbers came from.
    pub source: IdentitySource,
}

impl DeviceIdentity {
    /// The identity of a device that reported none: `desk-buddy` on `c3-supermini`, revision unknown.
    ///
    /// # Panics
    /// If the embedded catalog lacks the legacy product or board (the catalog tests rule that out).
    #[must_use]
    pub fn legacy() -> Self {
        let catalog = Catalog::embedded();
        let product = catalog
            .product(LEGACY_PRODUCT)
            .expect("catalog has the legacy product");
        let board = product
            .boards
            .iter()
            .find(|b| b.id == LEGACY_BOARD)
            .expect("catalog has the legacy board");
        Self {
            product: product.number,
            board: board.number,
            hw_rev: None,
            source: IdentitySource::Legacy,
        }
    }

    /// The identity carried by an `Identity` message.
    #[must_use]
    pub fn from_wire(message: &Identity) -> Self {
        Self {
            product: message.product,
            board: message.board,
            hw_rev: message.hw_rev,
            source: match message.source {
                WireSource::Build => IdentitySource::Build,
                WireSource::Chip => IdentitySource::Chip,
            },
        }
    }

    /// The catalog entries for this identity, if both numbers are known.
    #[must_use]
    pub fn resolve(&self) -> Option<(&'static Product, &'static Board)> {
        Catalog::embedded().by_wire(self.product, self.board)
    }

    /// Product name for the UI; an unknown number is shown as such.
    #[must_use]
    pub fn product_name(&self) -> String {
        names(Catalog::embedded(), self.product, self.board).0
    }

    /// Board name for the UI; an unknown number is shown as such.
    #[must_use]
    pub fn board_name(&self) -> String {
        names(Catalog::embedded(), self.product, self.board).1
    }

    /// "Product on Board", as shown beside a flashing refusal.
    #[must_use]
    pub fn label(&self) -> String {
        label(Catalog::embedded(), self.product, self.board)
    }
}

/// The product and board names for a pair of wire numbers, each shown as unknown when the catalog
/// lacks it.
fn names(catalog: &Catalog, product: u16, board: u16) -> (String, String) {
    let product_name = catalog
        .products()
        .iter()
        .find(|p| p.number == product)
        .map_or_else(
            || format!("Unknown product ({product})"),
            |p| p.display_name.clone(),
        );
    let board_name = catalog.by_wire(product, board).map_or_else(
        || format!("Unknown board ({board})"),
        |(_, b)| b.display_name.clone(),
    );
    (product_name, board_name)
}

fn label(catalog: &Catalog, product: u16, board: u16) -> String {
    let (product, board) = names(catalog, product, board);
    format!("{product} on {board}")
}

/// Why an image was refused for a device: both names, for the UI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WrongProduct {
    /// What the device is.
    pub device: String,
    /// What the image is for.
    pub image: String,
}

/// Tier A of ADR-0016: the image's identity record must name the connected device's product and
/// board. The legacy identity counts as `desk-buddy` / `c3-supermini`.
///
/// # Errors
/// [`WrongProduct`] naming both sides when they differ (including a device whose numbers the
/// catalog does not know).
pub fn check_image_for_device(
    image: &ImageIdentity,
    device: &DeviceIdentity,
) -> Result<(), WrongProduct> {
    if image.product == device.product && image.board == device.board {
        return Ok(());
    }
    Err(WrongProduct {
        device: device.label(),
        image: label(Catalog::embedded(), image.product, image.board),
    })
}

/// One product the user may pick when a port could be several of them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RestoreOption {
    /// Machine id of the product.
    pub id: String,
    /// Name shown to the user.
    pub name: String,
}

/// What recovery (no handshake possible) does about which product's firmware to write.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecoveryDecision {
    /// Write the bundled image.
    Proceed,
    /// More than one product could be on this port: the user must pick, then ask again.
    Choose(Vec<RestoreOption>),
    /// The product (the only one, or the user's pick) is not the bundled image's.
    Refuse(WrongProduct),
}

/// Decides recovery for a port with USB ids `usb` (`None` when unknown). Candidates are the
/// products with a board listing those ids. With exactly one, it is used; with several the user
/// must pick (`chosen`, a product id). The chosen product's board must be the bundled image's.
#[must_use]
pub fn decide_recovery(
    catalog: &Catalog,
    usb: Option<UsbId>,
    chosen: Option<&str>,
    image: Option<&ImageIdentity>,
) -> RecoveryDecision {
    let candidates: Vec<(&Product, &Board)> = catalog
        .products()
        .iter()
        .flat_map(|p| p.boards.iter().map(move |b| (p, b)))
        .filter(|(_, b)| usb.is_none_or(|id| b.usb.contains(&id)))
        .collect();
    let target = match (chosen, candidates.as_slice()) {
        (Some(id), _) => candidates.iter().find(|(p, _)| p.id == id).copied(),
        (None, [only]) => Some(*only),
        (None, []) => None,
        (None, _) => {
            let mut options: Vec<RestoreOption> = Vec::new();
            for (product, _) in &candidates {
                if !options.iter().any(|o| o.id == product.id) {
                    options.push(RestoreOption {
                        id: product.id.clone(),
                        name: product.display_name.clone(),
                    });
                }
            }
            // One product on several boards is a board question, not a product one; the bundled
            // image's own check below settles it.
            if options.len() > 1 {
                return RecoveryDecision::Choose(options);
            }
            candidates.first().copied()
        }
    };
    let (Some((product, board)), Some(image)) = (target, image) else {
        return RecoveryDecision::Proceed;
    };
    if image.product == product.number && image.board == board.number {
        return RecoveryDecision::Proceed;
    }
    RecoveryDecision::Refuse(WrongProduct {
        device: label(catalog, product.number, board.number),
        image: label(catalog, image.product, image.board),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn image(product: u16, board: u16) -> ImageIdentity {
        ImageIdentity {
            product,
            board,
            version: "1.4.0".into(),
            min_protocol: (1, 6),
        }
    }

    fn device(product: u16, board: u16) -> DeviceIdentity {
        DeviceIdentity {
            product,
            board,
            hw_rev: None,
            source: IdentitySource::Build,
        }
    }

    /// A two-product catalog: `desk-buddy` and a pretend second product sharing the C3 SuperMini USB ids.
    fn two_products() -> Catalog {
        let registry =
            "[products]\ndesk-buddy = 1\nlamp = 2\n[boards]\nc3-supermini = 1\nrp2350 = 2\n";
        let product = |name: &str, board: &str, usb: &str| {
            format!(
                "display_name = \"{name}\"\nrequired_capabilities = []\n[boards.{board}]\n\
                 display_name = \"B\"\nflash = \"espflash\"\nusb = [\"{usb}\"]\n\
                 [boards.{board}.firmware]\ncrate = \"fw\"\nartifact = \"a\"\nmin_protocol = \"1.6\"\n"
            )
        };
        Catalog::parse(
            registry,
            &[
                (
                    "desk-buddy",
                    &product("Desk Buddy", "c3-supermini", "303A:1001"),
                ),
                ("lamp", &product("Lamp", "c3-supermini", "303A:1001")),
            ],
        )
        .expect("valid catalog")
    }

    #[test]
    fn legacy_is_desk_buddy_on_the_c3_supermini_with_unknown_revision() {
        let legacy = DeviceIdentity::legacy();
        assert_eq!((legacy.product, legacy.board), (1, 1));
        assert_eq!(legacy.hw_rev, None);
        assert_eq!(legacy.source, IdentitySource::Legacy);
        let (product, board) = legacy.resolve().expect("known");
        assert_eq!(
            (product.id.as_str(), board.id.as_str()),
            ("desk-buddy", "c3-supermini")
        );
        assert_eq!(legacy.label(), "Kivori Desk Buddy on C3 SuperMini");
    }

    #[test]
    fn wire_identities_keep_their_source_and_revision() {
        let wire = Identity {
            session: 9,
            product: 1,
            board: 1,
            hw_rev: Some(3),
            source: WireSource::Chip,
        };
        let id = DeviceIdentity::from_wire(&wire);
        assert_eq!((id.hw_rev, id.source), (Some(3), IdentitySource::Chip));
        assert!(id.resolve().is_some());
    }

    #[test]
    fn unknown_numbers_are_named_as_unknown_and_never_resolved() {
        let unknown = device(9, 1);
        assert!(unknown.resolve().is_none());
        assert_eq!(unknown.product_name(), "Unknown product (9)");
        let board = device(1, 7);
        assert!(board.resolve().is_none());
        assert_eq!(board.product_name(), "Kivori Desk Buddy");
        assert_eq!(board.board_name(), "Unknown board (7)");
    }

    #[test]
    fn the_guard_passes_a_matching_device_and_the_legacy_one() {
        assert_eq!(check_image_for_device(&image(1, 1), &device(1, 1)), Ok(()));
        assert_eq!(
            check_image_for_device(&image(1, 1), &DeviceIdentity::legacy()),
            Ok(())
        );
    }

    #[test]
    fn the_guard_refuses_another_product_board_or_unknown_device_naming_both() {
        for (other, device_label) in [
            (device(2, 1), "Unknown product (2) on Unknown board (1)"),
            (device(1, 2), "Kivori Desk Buddy on Unknown board (2)"),
        ] {
            assert_eq!(
                check_image_for_device(&image(1, 1), &other),
                Err(WrongProduct {
                    device: device_label.into(),
                    image: "Kivori Desk Buddy on C3 SuperMini".into(),
                })
            );
        }
    }

    #[test]
    fn recovery_uses_the_only_product_for_the_port() {
        let catalog = Catalog::embedded();
        let usb = Some((0x303A, 0x1001));
        assert_eq!(
            decide_recovery(catalog, usb, None, Some(&image(1, 1))),
            RecoveryDecision::Proceed
        );
        // Not knowing the USB ids still leaves only one product in this catalog.
        assert_eq!(
            decide_recovery(catalog, None, None, Some(&image(1, 1))),
            RecoveryDecision::Proceed
        );
    }

    #[test]
    fn recovery_asks_when_several_products_could_be_on_the_port() {
        let catalog = two_products();
        let usb = Some((0x303A, 0x1001));
        let decision = decide_recovery(&catalog, usb, None, Some(&image(1, 1)));
        assert_eq!(
            decision,
            RecoveryDecision::Choose(vec![
                RestoreOption {
                    id: "desk-buddy".into(),
                    name: "Desk Buddy".into()
                },
                RestoreOption {
                    id: "lamp".into(),
                    name: "Lamp".into()
                },
            ])
        );
    }

    #[test]
    fn a_pick_must_be_the_bundled_image_or_it_is_refused_with_both_names() {
        let catalog = two_products();
        let usb = Some((0x303A, 0x1001));
        assert_eq!(
            decide_recovery(&catalog, usb, Some("desk-buddy"), Some(&image(1, 1))),
            RecoveryDecision::Proceed
        );
        let RecoveryDecision::Refuse(refusal) =
            decide_recovery(&catalog, usb, Some("lamp"), Some(&image(1, 1)))
        else {
            panic!("the bundled image is not the lamp's");
        };
        assert_eq!(refusal.device, "Lamp on B");
        assert_eq!(refusal.image, "Desk Buddy on B");
    }
}
