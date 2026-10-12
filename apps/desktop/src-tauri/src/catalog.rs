//! The product catalog (ADR-0014): which products and boards this app knows, read from
//! `products/registry.toml` (machine ids and wire numbers) and `products/<id>/product.toml`
//! (customer name, short name, model code, flash method, discovery hints, bundled firmware,
//! required capabilities).
//!
//! Both are compiled into the app. This file is also compiled by `build.rs` (via `#[path]`), which
//! validates the catalog and checks the bundled firmware against it, so it stays free of
//! crate-internal dependencies; capability names are resolved to flags in `provision`.

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::Deserialize;

/// A `(vendor, product)` USB id pair.
pub type UsbId = (u16, u16);

/// The registry file, as embedded.
const REGISTRY: &str = include_str!("../../../../products/registry.toml");

/// Every product file, as embedded: `(machine id, file contents)`. A test checks that each
/// directory under `products/` is listed.
const PRODUCT_FILES: &[(&str, &str)] = &[(
    "pala",
    include_str!("../../../../products/pala/product.toml"),
)];

/// How a board is flashed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FlashMethod {
    /// `espflash` over the board's USB serial port.
    Espflash,
}

/// The firmware bundled for one board.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BundledFirmware {
    /// Firmware crate directory, relative to the repository root.
    pub crate_dir: PathBuf,
    /// Release build output, relative to the crate directory.
    pub artifact: PathBuf,
    /// Oldest wire protocol `(major, minor)` this firmware speaks.
    pub min_protocol: (u16, u16),
}

/// One board a product ships on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Board {
    /// Machine id from the registry.
    pub id: String,
    /// Wire number from the registry.
    pub number: u16,
    /// Name shown to the user.
    pub display_name: String,
    /// How the board is flashed.
    pub flash: FlashMethod,
    /// USB ids that nominate a serial port as a candidate. Never identity.
    pub usb: Vec<UsbId>,
    /// The firmware bundled for this board.
    pub firmware: BundledFirmware,
}

/// One product.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Product {
    /// Machine id from the registry.
    pub id: String,
    /// Wire number from the registry.
    pub number: u16,
    /// Customer name shown to the user ("Kivori Pip").
    pub display_name: String,
    /// Short customer name, without the brand ("Pip").
    pub short_name: String,
    /// Model code printed on the unit and its box ("KV-P1").
    pub model: String,
    /// Product picture, relative to the product directory (none yet when empty).
    pub image: Option<String>,
    /// Names of the capabilities every unit must negotiate.
    pub required_capabilities: Vec<String>,
    /// The boards this product ships on.
    pub boards: Vec<Board>,
}

/// The validated catalog.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Catalog {
    products: Vec<Product>,
}

/// Why the catalog failed to parse or validate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogError(pub String);

impl core::fmt::Display for CatalogError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "product catalog: {}", self.0)
    }
}

impl std::error::Error for CatalogError {}

fn err<T>(msg: impl Into<String>) -> Result<T, CatalogError> {
    Err(CatalogError(msg.into()))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawRegistry {
    products: BTreeMap<String, u16>,
    boards: BTreeMap<String, u16>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawProduct {
    display_name: String,
    short_name: String,
    model: String,
    #[serde(default)]
    image: String,
    required_capabilities: Vec<String>,
    boards: BTreeMap<String, RawBoard>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawBoard {
    display_name: String,
    flash: FlashMethod,
    usb: Vec<String>,
    firmware: RawFirmware,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawFirmware {
    #[serde(rename = "crate")]
    crate_dir: String,
    artifact: String,
    min_protocol: String,
}

/// Machine ids are lowercase kebab-case.
fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && !id.starts_with('-')
        && !id.ends_with('-')
        && id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

/// Checks one registry table: valid ids, non-zero numbers, no number used twice.
fn check_table(kind: &str, table: &BTreeMap<String, u16>) -> Result<(), CatalogError> {
    let mut seen = BTreeMap::new();
    for (id, &number) in table {
        if !valid_id(id) {
            return err(format!("{kind} id `{id}` is not lowercase kebab-case"));
        }
        if number == 0 {
            return err(format!(
                "{kind} `{id}` uses 0, which is reserved for unknown"
            ));
        }
        if let Some(other) = seen.insert(number, id) {
            return err(format!(
                "{kind} number {number} is used by `{other}` and `{id}`"
            ));
        }
    }
    Ok(())
}

/// Model codes are two uppercase letters, a dash, then uppercase letters or digits (`KV-P1`).
fn valid_model(model: &str) -> bool {
    model.split_once('-').is_some_and(|(prefix, rest)| {
        prefix.len() == 2
            && prefix.chars().all(|c| c.is_ascii_uppercase())
            && !rest.is_empty()
            && rest
                .chars()
                .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
    })
}

fn parse_usb(text: &str) -> Result<UsbId, CatalogError> {
    let parsed = text.split_once(':').and_then(|(vid, pid)| {
        Some((
            u16::from_str_radix(vid, 16).ok()?,
            u16::from_str_radix(pid, 16).ok()?,
        ))
    });
    parsed.map_or_else(
        || err(format!("USB id `{text}` is not `VID:PID` in hex")),
        Ok,
    )
}

fn parse_protocol(text: &str) -> Result<(u16, u16), CatalogError> {
    let parsed = text
        .split_once('.')
        .and_then(|(major, minor)| Some((major.parse().ok()?, minor.parse().ok()?)));
    parsed.map_or_else(
        || err(format!("protocol `{text}` is not `major.minor`")),
        Ok,
    )
}

impl Catalog {
    /// Parses and validates a registry plus product files given as `(machine id, contents)`.
    ///
    /// # Errors
    /// Malformed TOML, an invalid or reused id or number, a product missing from the registry, a
    /// board missing from the registry, an empty name or short name, a malformed model code, or a
    /// malformed USB id or protocol version.
    pub fn parse(registry: &str, products: &[(&str, &str)]) -> Result<Self, CatalogError> {
        let registry: RawRegistry =
            toml::from_str(registry).map_err(|e| CatalogError(format!("registry.toml: {e}")))?;
        check_table("product", &registry.products)?;
        check_table("board", &registry.boards)?;

        let mut out = Vec::new();
        for &(id, source) in products {
            let Some(&number) = registry.products.get(id) else {
                return err(format!("product `{id}` is not in the registry"));
            };
            let raw: RawProduct = toml::from_str(source)
                .map_err(|e| CatalogError(format!("products/{id}/product.toml: {e}")))?;
            if raw.display_name.trim().is_empty() || raw.short_name.trim().is_empty() {
                return err(format!(
                    "product `{id}` has an empty display_name or short_name"
                ));
            }
            if !valid_model(&raw.model) {
                return err(format!(
                    "product `{id}` model `{}` is not like `KV-P1` (two capitals, a dash, capitals or digits)",
                    raw.model
                ));
            }
            let mut boards = Vec::new();
            for (board_id, board) in raw.boards {
                let Some(&board_number) = registry.boards.get(&board_id) else {
                    return err(format!(
                        "`{id}` names board `{board_id}`, not in the registry"
                    ));
                };
                let usb = board
                    .usb
                    .iter()
                    .map(|u| parse_usb(u))
                    .collect::<Result<Vec<_>, _>>()?;
                boards.push(Board {
                    id: board_id,
                    number: board_number,
                    display_name: board.display_name,
                    flash: board.flash,
                    usb,
                    firmware: BundledFirmware {
                        crate_dir: board.firmware.crate_dir.into(),
                        artifact: board.firmware.artifact.into(),
                        min_protocol: parse_protocol(&board.firmware.min_protocol)?,
                    },
                });
            }
            if boards.is_empty() {
                return err(format!("product `{id}` lists no boards"));
            }
            out.push(Product {
                id: id.to_string(),
                number,
                display_name: raw.display_name,
                short_name: raw.short_name,
                model: raw.model,
                image: (!raw.image.is_empty()).then_some(raw.image),
                required_capabilities: raw.required_capabilities,
                boards,
            });
        }
        Ok(Self { products: out })
    }

    /// The catalog compiled into this build.
    ///
    /// # Panics
    /// If the embedded files are invalid; `build.rs` and the tests reject that before release.
    #[must_use]
    pub fn embedded() -> &'static Self {
        static CATALOG: std::sync::OnceLock<Catalog> = std::sync::OnceLock::new();
        CATALOG.get_or_init(|| {
            Catalog::parse(REGISTRY, PRODUCT_FILES).expect("embedded product catalog is valid")
        })
    }

    /// Every product, in registry file order of embedding.
    #[must_use]
    pub fn products(&self) -> &[Product] {
        &self.products
    }

    /// The product with this machine id.
    #[must_use]
    pub fn product(&self, id: &str) -> Option<&Product> {
        self.products.iter().find(|p| p.id == id)
    }

    /// The product and board for a pair of wire numbers (as an Identity message will carry them).
    #[must_use]
    pub fn by_wire(&self, product: u16, board: u16) -> Option<(&Product, &Board)> {
        let product = self.products.iter().find(|p| p.number == product)?;
        let board = product.boards.iter().find(|b| b.number == board)?;
        Some((product, board))
    }

    /// Every USB id that nominates a port as a candidate, without duplicates.
    #[must_use]
    pub fn usb_ids(&self) -> Vec<UsbId> {
        let mut ids: Vec<UsbId> = Vec::new();
        for id in self
            .products
            .iter()
            .flat_map(|p| &p.boards)
            .flat_map(|b| &b.usb)
        {
            if !ids.contains(id) {
                ids.push(*id);
            }
        }
        ids
    }
}

/// The `version` of a firmware crate, read from its `Cargo.toml` text.
#[must_use]
pub fn crate_version(cargo_toml: &str) -> Option<String> {
    let value: toml::Value = toml::from_str(cargo_toml).ok()?;
    Some(value.get("package")?.get("version")?.as_str()?.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    const REG: &str = "[products]\npala = 1\n[boards]\nc3-supermini = 1\n";

    fn product(board: &str) -> String {
        format!(
            "display_name = \"X\"\nshort_name = \"X\"\nmodel = \"KV-X1\"\nrequired_capabilities = []\n[boards.{board}]\ndisplay_name = \"B\"\n\
             flash = \"espflash\"\nusb = [\"303A:1001\"]\n[boards.{board}.firmware]\ncrate = \"fw\"\n\
             artifact = \"a\"\nmin_protocol = \"0.1\"\n"
        )
    }

    fn parse(reg: &str, product_src: &str) -> Result<Catalog, CatalogError> {
        Catalog::parse(reg, &[("pala", product_src)])
    }

    #[test]
    fn embedded_catalog_is_valid_and_has_pala_on_c3_supermini() {
        let catalog = Catalog::embedded();
        let (product, board) = catalog.by_wire(1, 1).expect("pala on c3-supermini");
        assert_eq!(product.id, "pala");
        assert_eq!(product.display_name, "Kivori Pip");
        assert_eq!(product.short_name, "Pip");
        assert_eq!(product.model, "KV-P1");
        assert_eq!(board.id, "c3-supermini");
        assert_eq!(board.flash, FlashMethod::Espflash);
        // The bundled firmware speaks this app's protocol major, at a minor it has.
        let (major, minor) = board.firmware.min_protocol;
        assert_eq!(major, kivori_protocol::PROTOCOL_MAJOR);
        assert!(minor <= kivori_protocol::PROTOCOL_MINOR);
        assert_eq!(catalog.usb_ids(), vec![(0x303A, 0x1001)]);
        assert_eq!(catalog.by_wire(1, 2), None);
        assert_eq!(catalog.by_wire(0, 0), None);
        assert!(catalog.product("pala").is_some());
        assert!(catalog.product("nope").is_none());
    }

    #[test]
    fn every_product_directory_is_embedded_and_every_artifact_crate_exists() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../");
        let mut on_disk: Vec<String> = std::fs::read_dir(root.join("products"))
            .expect("products dir")
            .filter_map(Result::ok)
            .filter(|e| e.path().is_dir())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        on_disk.sort();
        let mut embedded: Vec<&str> = PRODUCT_FILES.iter().map(|(id, _)| *id).collect();
        embedded.sort_unstable();
        assert_eq!(on_disk, embedded);
        for board in Catalog::embedded()
            .products()
            .iter()
            .flat_map(|p| &p.boards)
        {
            let manifest = root.join(&board.firmware.crate_dir).join("Cargo.toml");
            let text = std::fs::read_to_string(&manifest).expect("firmware crate manifest");
            assert!(crate_version(&text).is_some(), "{}", manifest.display());
        }
    }

    #[test]
    fn rejects_unknown_board_and_unknown_product() {
        assert!(parse(REG, &product("rp2350")).is_err());
        assert!(Catalog::parse(REG, &[("other", &product("c3-supermini"))]).is_err());
    }

    #[test]
    fn rejects_reused_or_zero_numbers_and_bad_ids() {
        let dup = "[products]\na = 1\nb = 1\n[boards]\nc3-supermini = 1\n";
        assert!(Catalog::parse(dup, &[]).is_err());
        let zero = "[products]\npala = 0\n[boards]\nc3-supermini = 1\n";
        assert!(Catalog::parse(zero, &[]).is_err());
        let bad = "[products]\nDesk = 1\n[boards]\nc3-supermini = 1\n";
        assert!(Catalog::parse(bad, &[]).is_err());
    }

    #[test]
    fn rejects_malformed_usb_and_protocol() {
        let usb = product("c3-supermini").replace("303A:1001", "nope");
        assert!(parse(REG, &usb).is_err());
        let proto = product("c3-supermini").replace("0.1", "x");
        assert!(parse(REG, &proto).is_err());
        assert!(parse(REG, &product("c3-supermini")).is_ok());
    }

    #[test]
    fn requires_names_and_a_well_formed_model_code() {
        let ok = product("c3-supermini");
        let empty_short = ok.replace("short_name = \"X\"", "short_name = \" \"");
        assert!(parse(REG, &empty_short).is_err());
        let no_model = ok.replace("model = \"KV-X1\"\n", "");
        assert!(parse(REG, &no_model).is_err());
        for bad in ["kv-p1", "KVP1", "K-P1", "KVX-P1", "KV-", "KV-p1", "KV-P 1"] {
            let src = ok.replace("KV-X1", bad);
            assert!(parse(REG, &src).is_err(), "{bad} should be rejected");
        }
        for good in ["KV-P1", "KV-1", "AB-XYZ9"] {
            let src = ok.replace("KV-X1", good);
            assert!(parse(REG, &src).is_ok(), "{good} should be accepted");
        }
    }

    #[test]
    fn reads_a_crate_version() {
        let text = "[package]\nname = \"x\"\nversion = \"0.1.0\"\n";
        assert_eq!(crate_version(text).as_deref(), Some("0.1.0"));
        assert_eq!(crate_version("[package]\nname = \"x\"\n"), None);
    }
}
