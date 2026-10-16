//! What the desktop knows about which product a connected device is (ADR-0014): the identity the
//! device reported (or the legacy default for one that reports nothing), its resolution to a
//! catalog entry, and the decisions that depend on it (the flashing guard and the recovery pick).
//!
//! Numbers resolve only through [`Catalog::by_wire`]; an unknown number is shown as such and never
//! guessed into a known product.

use kivori_model::Capabilities;
use kivori_protocol::{Identity, IdentitySource as WireSource};

use crate::catalog::{Board, Catalog, Product, UsbId};
use crate::firmware_marker::ImageIdentity;

/// The product a device that sends no `Identity` is (every unit shipped before `PRODUCT_IDENTITY_V1`).
const LEGACY_PRODUCT: &str = "pala";
/// The board a device that sends no `Identity` is.
const LEGACY_BOARD: &str = "c3-supermini";

/// Where a device's identity came from. `Legacy` exists only on the desktop: it is the answer for
/// a device that did not negotiate `PRODUCT_IDENTITY_V1`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdentitySource {
    /// The device sent no identity; assumed the original Kivori Pip on the C3 SuperMini.
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
    /// The identity of a device that reported none: `pala` on `c3-supermini`, revision unknown.
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

/// Every capability name the catalog may require, with its flag.
pub const CAPABILITY_NAMES: [(&str, Capabilities); 11] = [
    ("MASCOT_INTERACTION", Capabilities::MASCOT_INTERACTION),
    ("PHYSICAL_INPUT_V1", Capabilities::PHYSICAL_INPUT_V1),
    ("PRESENTATION_V1", Capabilities::PRESENTATION_V1),
    ("BUTTON_INPUT_V1", Capabilities::BUTTON_INPUT_V1),
    ("DESK_STATUS_V1", Capabilities::DESK_STATUS_V1),
    ("ACTION_FEEDBACK_V1", Capabilities::ACTION_FEEDBACK_V1),
    ("DOUBLE_PRESS_V1", Capabilities::DOUBLE_PRESS_V1),
    ("MEDIA_INFO_V1", Capabilities::MEDIA_INFO_V1),
    ("CONTROL_LABELS_V1", Capabilities::CONTROL_LABELS_V1),
    ("CONTEXT_BUTTONS_V1", Capabilities::CONTEXT_BUTTONS_V1),
    ("HOST_TAKEOVERS_V1", Capabilities::HOST_TAKEOVERS_V1),
];

/// Whether a device lacks a capability the catalog requires of its product. Such a device cannot
/// work fully with this app, so its firmware update is a required one. A product the catalog does
/// not know requires nothing here (it is never offered firmware anyway).
#[must_use]
pub fn missing_required_capabilities(identity: &DeviceIdentity, negotiated: Capabilities) -> bool {
    let Some((product, _)) = identity.resolve() else {
        return false;
    };
    product.required_capabilities.iter().any(|name| {
        CAPABILITY_NAMES
            .iter()
            .find(|(known, _)| known == name)
            .is_some_and(|(_, flag)| !negotiated.contains(*flag))
    })
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
/// board. The legacy identity counts as `pala` / `c3-supermini`.
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
    /// Recovery cannot establish a supported, matching product/board target.
    Refuse(RecoveryRefusal),
}

/// Why recovery must stop before reserving a port.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecoveryRefusal {
    /// No image identity, or one that the catalog does not know.
    ImageUnavailable,
    /// No catalog candidate for the port, or the chosen product is not one of its candidates.
    NoCandidate,
    /// The selected product has several possible boards; an image cannot prove which one is present.
    BoardAmbiguous,
    /// A single resolved product/board is not the bundled image's pair.
    WrongProduct(WrongProduct),
}

/// Decides recovery for a port with USB ids `usb` (`None` when unknown). Candidates are the
/// products with a board listing those ids. With exactly one, it is used; with several the user
/// must pick (`chosen`, a product id). A pick cannot bypass candidate membership. Exactly one
/// board must remain and match a known image; neither image availability nor order resolves ambiguity.
/// A reported identity for this port takes precedence over USB hints, including an unknown identity.
#[must_use]
pub fn decide_recovery(
    catalog: &Catalog,
    usb: Option<UsbId>,
    chosen: Option<&str>,
    image: Option<&ImageIdentity>,
    reported: Option<&DeviceIdentity>,
) -> RecoveryDecision {
    let Some(image) = image.filter(|image| catalog.by_wire(image.product, image.board).is_some())
    else {
        return RecoveryDecision::Refuse(RecoveryRefusal::ImageUnavailable);
    };
    if let Some(device) = reported {
        if device.product != image.product || device.board != image.board {
            return RecoveryDecision::Refuse(RecoveryRefusal::WrongProduct(WrongProduct {
                device: label(catalog, device.product, device.board),
                image: label(catalog, image.product, image.board),
            }));
        }
        // Matching the known image also resolves this device's product/board in the catalog.
        let (product, _) = catalog
            .by_wire(device.product, device.board)
            .expect("known image pair");
        if chosen.is_some_and(|id| id != product.id) {
            return RecoveryDecision::Refuse(RecoveryRefusal::NoCandidate);
        }
        return RecoveryDecision::Proceed;
    }
    let mut candidates: Vec<(&Product, &Board)> = catalog
        .products()
        .iter()
        .flat_map(|p| p.boards.iter().map(move |b| (p, b)))
        .filter(|(_, b)| usb.is_none_or(|id| b.usb.contains(&id)))
        .collect();
    if let Some(id) = chosen {
        candidates.retain(|(product, _)| product.id == id);
    }
    if candidates.is_empty() {
        return RecoveryDecision::Refuse(RecoveryRefusal::NoCandidate);
    }
    let mut options: Vec<RestoreOption> = Vec::new();
    for (product, _) in &candidates {
        if !options.iter().any(|option| option.id == product.id) {
            options.push(RestoreOption {
                id: product.id.clone(),
                name: product.display_name.clone(),
            });
        }
    }
    if options.len() > 1 {
        return RecoveryDecision::Choose(options);
    }
    let [(product, board)] = candidates.as_slice() else {
        return RecoveryDecision::Refuse(RecoveryRefusal::BoardAmbiguous);
    };
    if image.product == product.number && image.board == board.number {
        return RecoveryDecision::Proceed;
    }
    RecoveryDecision::Refuse(RecoveryRefusal::WrongProduct(WrongProduct {
        device: label(catalog, product.number, board.number),
        image: label(catalog, image.product, image.board),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn image(product: u16, board: u16) -> ImageIdentity {
        ImageIdentity {
            product,
            board,
            version: "0.1.0".into(),
            min_protocol: (0, 1),
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

    /// A two-product catalog: `pala` and a pretend second product sharing the C3 SuperMini USB ids.
    fn two_products() -> Catalog {
        let registry = "[products]\npala = 1\nlamp = 2\n[boards]\nc3-supermini = 1\nrp2350 = 2\n";
        let product = |name: &str, board: &str, usb: &str| {
            format!(
                "display_name = \"{name}\"\nshort_name = \"{name}\"\nmodel = \"KV-X1\"\n\
                 required_capabilities = []\n[boards.{board}]\n\
                 display_name = \"B\"\nflash = \"espflash\"\nusb = [\"{usb}\"]\n\
                 [boards.{board}.firmware]\ncrate = \"fw\"\nartifact = \"a\"\nmin_protocol = \"0.1\"\n"
            )
        };
        Catalog::parse(
            registry,
            &[
                ("pala", &product("Kivori Pip", "c3-supermini", "303A:1001")),
                ("lamp", &product("Lamp", "c3-supermini", "303A:1001")),
            ],
        )
        .expect("valid catalog")
    }

    #[test]
    fn legacy_is_pala_on_the_c3_supermini_with_unknown_revision() {
        let legacy = DeviceIdentity::legacy();
        assert_eq!((legacy.product, legacy.board), (1, 1));
        assert_eq!(legacy.hw_rev, None);
        assert_eq!(legacy.source, IdentitySource::Legacy);
        let (product, board) = legacy.resolve().expect("known");
        assert_eq!(
            (product.id.as_str(), board.id.as_str()),
            ("pala", "c3-supermini")
        );
        assert_eq!(legacy.label(), "Kivori Pip on C3 SuperMini");
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
        assert_eq!(board.product_name(), "Kivori Pip");
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
            (device(1, 2), "Kivori Pip on Unknown board (2)"),
        ] {
            assert_eq!(
                check_image_for_device(&image(1, 1), &other),
                Err(WrongProduct {
                    device: device_label.into(),
                    image: "Kivori Pip on C3 SuperMini".into(),
                })
            );
        }
    }

    #[test]
    fn recovery_uses_the_only_product_for_the_port() {
        let catalog = Catalog::embedded();
        let usb = Some((0x303A, 0x1001));
        assert_eq!(
            decide_recovery(catalog, usb, None, Some(&image(1, 1)), None),
            RecoveryDecision::Proceed
        );
        assert_eq!(
            decide_recovery(
                catalog,
                usb,
                None,
                Some(&image(1, 1)),
                Some(&DeviceIdentity::legacy())
            ),
            RecoveryDecision::Proceed
        );
        // Not knowing the USB ids still leaves only one product in this catalog.
        assert_eq!(
            decide_recovery(catalog, None, None, Some(&image(1, 1)), None),
            RecoveryDecision::Proceed
        );
    }

    #[test]
    fn recovery_asks_when_several_products_could_be_on_the_port() {
        let catalog = two_products();
        let usb = Some((0x303A, 0x1001));
        let decision = decide_recovery(&catalog, usb, None, Some(&image(1, 1)), None);
        assert_eq!(
            decision,
            RecoveryDecision::Choose(vec![
                RestoreOption {
                    id: "pala".into(),
                    name: "Kivori Pip".into()
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
            decide_recovery(&catalog, usb, Some("pala"), Some(&image(1, 1)), None),
            RecoveryDecision::Proceed
        );
        let RecoveryDecision::Refuse(RecoveryRefusal::WrongProduct(refusal)) =
            decide_recovery(&catalog, usb, Some("lamp"), Some(&image(1, 1)), None)
        else {
            panic!("the bundled image is not the lamp's");
        };
        assert_eq!(refusal.device, "Lamp on B");
        assert_eq!(refusal.image, "Kivori Pip on B");
    }
}
