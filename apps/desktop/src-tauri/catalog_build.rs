//! Build-time catalog discovery and firmware validation, also compiled by library tests.

use std::fs;
use std::path::{Path, PathBuf};

use crate::catalog::{crate_version, Catalog, CatalogError};
use crate::firmware_marker::{scan_firmware_version, scan_image_identity};

fn read(path: &Path) -> Result<String, CatalogError> {
    let metadata =
        fs::symlink_metadata(path).map_err(|e| CatalogError(format!("{}: {e}", path.display())))?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(CatalogError(format!(
            "{} must be a regular file",
            path.display()
        )));
    }
    fs::read_to_string(path).map_err(|e| CatalogError(format!("{}: {e}", path.display())))
}

/// Discovers only direct product directories; validates before emitting deterministic Rust literals.
/// Reserved registry entries without a directory do not create active products.
pub fn generate(products_dir: &Path) -> Result<(Catalog, String), CatalogError> {
    let registry = read(&products_dir.join("registry.toml"))?;
    let entries = fs::read_dir(products_dir)
        .map_err(|e| CatalogError(format!("{}: {e}", products_dir.display())))?;
    let mut products = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|e| CatalogError(e.to_string()))?;
        let kind = entry.file_type().map_err(|e| CatalogError(e.to_string()))?;
        let id = entry
            .file_name()
            .into_string()
            .map_err(|_| CatalogError("product directory names must be UTF-8".into()))?;
        if kind.is_file() && id.starts_with('.') {
            continue;
        }
        if kind.is_dir() {
            products.push((id, read(&entry.path().join("product.toml"))?));
        } else if id != "registry.toml" || !kind.is_file() {
            return Err(CatalogError(format!(
                "unexpected catalog entry {}",
                entry.path().display()
            )));
        }
    }
    products.sort_by(|a, b| a.0.cmp(&b.0));
    if products.is_empty() {
        return Err(CatalogError("catalog has no active products".into()));
    }
    let sources: Vec<_> = products
        .iter()
        .map(|(id, text)| (id.as_str(), text.as_str()))
        .collect();
    let catalog = Catalog::parse(&registry, &sources)?;
    let mut generated = format!(
        "const REGISTRY: &str = {registry:?};\nconst PRODUCT_FILES: &[(&str, &str)] = &[\n"
    );
    for (id, text) in sources {
        generated.push_str(&format!("    ({id:?}, {text:?}),\n"));
    }
    generated.push_str("];\n");
    Ok((catalog, generated))
}

/// Checks the explicitly supplied single image by its own identity, never catalog order.
/// Returns its crate manifest so Cargo can watch version changes. An absent image is a host build.
pub fn check_firmware(
    catalog: &Catalog,
    root: &Path,
    image: &[u8],
) -> Result<Option<PathBuf>, CatalogError> {
    if image.is_empty() {
        return Ok(None);
    }
    let version = scan_firmware_version(image)
        .ok_or_else(|| CatalogError("bundled image has no firmware version marker".into()))?;
    let record = scan_image_identity(image)
        .ok_or_else(|| CatalogError("bundled image has no KIVORI-FW-IDENTITY record".into()))?;
    let (_, board) = catalog
        .by_wire(record.product, record.board)
        .ok_or_else(|| CatalogError("bundled image names an unknown product/board".into()))?;
    let manifest = root.join(&board.firmware.crate_dir).join("Cargo.toml");
    let expected_version = crate_version(&read(&manifest)?)
        .ok_or_else(|| CatalogError(format!("{} has no package version", manifest.display())))?;
    if version != expected_version
        || record.version != expected_version
        || record.min_protocol != board.firmware.min_protocol
    {
        return Err(CatalogError(format!("bundled image differs from {} version or catalog minimum protocol; rebuild the firmware", manifest.display())));
    }
    Ok(Some(manifest))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn product(id: &str) -> String {
        format!(
            "display_name = \"Test {id}\\nwith \\\"quotes\\\"\"\nshort_name = \"Test\"\nmodel = \"KV-T1\"\nrequired_capabilities = []\n\
             [boards.test-board]\ndisplay_name = \"Test board\"\nflash = \"espflash\"\nusb = [\"1234:5678\"]\n\
             [boards.test-board.firmware]\ncrate = \"firmware/{id}\"\nartifact = \"target/image\"\nmin_protocol = \"0.1\"\n"
        )
    }

    fn fixture() -> tempfile::TempDir {
        let root = tempfile::tempdir().unwrap();
        let products = root.path().join("products");
        fs::create_dir(&products).unwrap();
        fs::write(
            products.join("registry.toml"),
            "[products]\npala = 1\nlada = 2\nreserved = 3\n[boards]\ntest-board = 1\n",
        )
        .unwrap();
        add_product(root.path(), "pala");
        root
    }

    fn add_product(root: &Path, id: &str) {
        let directory = root.join("products").join(id);
        fs::create_dir(&directory).unwrap();
        fs::write(directory.join("product.toml"), product(id)).unwrap();
        let firmware = root.join("firmware").join(id);
        fs::create_dir_all(&firmware).unwrap();
        fs::write(
            firmware.join("Cargo.toml"),
            "[package]\nname = 'test-firmware'\nversion = '0.1.0'\n",
        )
        .unwrap();
    }

    #[test]
    fn second_product_is_generated_without_a_source_list_and_reservations_stay_inactive() {
        let root = fixture();
        let (one, _) = generate(&root.path().join("products")).unwrap();
        assert_eq!(one.products().len(), 1);
        add_product(root.path(), "lada");
        let (catalog, source) = generate(&root.path().join("products")).unwrap();
        assert_eq!(
            catalog
                .products()
                .iter()
                .map(|p| p.id.as_str())
                .collect::<Vec<_>>(),
            ["lada", "pala"]
        );
        assert!(catalog.product("reserved").is_none());
        assert_eq!(source, generate(&root.path().join("products")).unwrap().1);

        // Compile the actual generated constants, including quoted/newline metadata, outside
        // shipping discovery. This checks emission rather than a second implementation of it.
        let harness = root.path().join("embedded.rs");
        fs::write(&harness, format!("{source}\nfn main() {{ assert!(REGISTRY.contains(\"reserved\")); assert_eq!(PRODUCT_FILES.len(), 2); assert_eq!(PRODUCT_FILES[0].0, \"lada\"); assert_eq!(PRODUCT_FILES[1], (\"pala\", {:?})); }}", product("pala"))).unwrap();
        let binary = root
            .path()
            .join(format!("embedded{}", std::env::consts::EXE_SUFFIX));
        let output =
            std::process::Command::new(std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into()))
                .args([
                    "--edition=2021",
                    "--crate-type=bin",
                    "--crate-name",
                    "catalog_fixture",
                ])
                .arg(&harness)
                .arg("-o")
                .arg(&binary)
                .output()
                .expect("catalog fixture needs rustc on PATH or RUSTC set");
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(std::process::Command::new(binary)
            .status()
            .unwrap()
            .success());
    }

    #[test]
    fn malformed_unknown_missing_and_stray_sources_fail_closed() {
        for case in [
            "unknown",
            "malformed",
            "missing",
            "stray",
            "registry",
            "empty",
            "invalid-id",
        ] {
            let root = fixture();
            let products = root.path().join("products");
            match case {
                "unknown" => add_product(root.path(), "unregistered"),
                "malformed" => {
                    fs::write(products.join("pala/product.toml"), "not valid = [").unwrap()
                }
                "missing" => fs::remove_file(products.join("pala/product.toml")).unwrap(),
                "stray" => fs::write(products.join("typo.toml"), product("lada")).unwrap(),
                "registry" => fs::remove_file(products.join("registry.toml")).unwrap(),
                "empty" => fs::remove_dir_all(products.join("pala")).unwrap(),
                "invalid-id" => fs::rename(products.join("pala"), products.join("Pala")).unwrap(),
                _ => unreachable!(),
            }
            assert!(generate(&products).is_err(), "{case} must be refused");
        }
    }

    #[test]
    fn root_dotfiles_are_ignored_but_dot_directories_are_not_product_sources() {
        let root = fixture();
        let products = root.path().join("products");
        let (_, expected) = generate(&products).unwrap();
        for name in [".DS_Store", ".registry.toml.swp"] {
            fs::write(products.join(name), "not catalog data").unwrap();
        }
        assert_eq!(generate(&products).unwrap().1, expected);
        fs::create_dir(products.join(".fixtures")).unwrap();
        assert!(generate(&products).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn symlinked_directories_registry_and_product_files_are_refused() {
        use std::os::unix::fs::symlink;
        for case in ["directory", "registry", "product"] {
            let root = fixture();
            let products = root.path().join("products");
            match case {
                "directory" => symlink(products.join("pala"), products.join("lada")).unwrap(),
                "registry" => {
                    fs::rename(
                        products.join("registry.toml"),
                        root.path().join("registry.toml"),
                    )
                    .unwrap();
                    symlink(
                        root.path().join("registry.toml"),
                        products.join("registry.toml"),
                    )
                    .unwrap();
                }
                "product" => {
                    fs::rename(
                        products.join("pala/product.toml"),
                        root.path().join("product.toml"),
                    )
                    .unwrap();
                    symlink(
                        root.path().join("product.toml"),
                        products.join("pala/product.toml"),
                    )
                    .unwrap();
                }
                _ => unreachable!(),
            }
            assert!(
                generate(&products).is_err(),
                "{case} symlink must be refused"
            );
        }
    }

    fn image(product: u16, board: u16, version: &str, protocol: &str) -> Vec<u8> {
        format!("KIVORI-FW-VERSION:{version}\0KIVORI-FW-IDENTITY:p={product};b={board};v={version};m={protocol}\0").into_bytes()
    }

    #[test]
    fn explicit_image_is_checked_by_identity_not_catalog_order() {
        let root = fixture();
        add_product(root.path(), "lada");
        fs::write(
            root.path().join("firmware/lada/Cargo.toml"),
            "[package]\nname = 'test-firmware'\nversion = '0.2.0'\n",
        )
        .unwrap();
        fs::write(
            root.path().join("products/lada/product.toml"),
            product("lada").replace("min_protocol = \"0.1\"", "min_protocol = \"0.2\""),
        )
        .unwrap();
        let (catalog, _) = generate(&root.path().join("products")).unwrap();
        // pala is second in deterministic catalog order, but its image still selects its crate.
        assert_eq!(
            check_firmware(&catalog, root.path(), &image(1, 1, "0.1.0", "0.1")).unwrap(),
            Some(root.path().join("firmware/pala/Cargo.toml"))
        );
        assert_eq!(
            check_firmware(&catalog, root.path(), &image(2, 1, "0.2.0", "0.2")).unwrap(),
            Some(root.path().join("firmware/lada/Cargo.toml"))
        );
        for bad in [
            image(2, 1, "0.1.0", "0.2"),
            image(2, 1, "0.2.0", "0.1"),
            image(9, 1, "0.1.0", "0.1"),
            image(1, 9, "0.1.0", "0.1"),
            image(1, 1, "0.2.0", "0.1"),
            image(1, 1, "0.1.0", "0.2"),
            b"KIVORI-FW-VERSION:0.1.0\0KIVORI-FW-IDENTITY:p=1;b=1;v=0.2.0;m=0.1\0".to_vec(),
            b"no markers".to_vec(),
            b"KIVORI-FW-VERSION:0.1.0\0".to_vec(),
            b"KIVORI-FW-IDENTITY:p=1;b=1;v=0.1.0;m=0.1\0".to_vec(),
        ] {
            assert!(check_firmware(&catalog, root.path(), &bad).is_err());
        }
        assert_eq!(check_firmware(&catalog, root.path(), &[]).unwrap(), None);
    }
}
