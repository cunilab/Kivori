//! Build-time catalog discovery and firmware validation, also compiled by library tests.

use std::fs;
use std::path::{Path, PathBuf};

use crate::catalog::{crate_version, Catalog, CatalogError};
use crate::firmware_bundle::{FirmwareBundle, FirmwareImage};

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
pub fn generate(products_dir: &Path, out_dir: &Path) -> Result<(Catalog, String), CatalogError> {
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
    for product in catalog.products() {
        if product.display_name.encode_utf16().count() > 96
            || product.model.len() > 96
            || (product.image.is_some() && product.id.len() > 96)
        {
            return Err(CatalogError(format!(
                "product `{}` presentation exceeds the desktop IPC limit of 96 UTF-16 units",
                product.id
            )));
        }
    }
    let mut generated = format!(
        "const REGISTRY: &str = {registry:?};\nconst PRODUCT_FILES: &[(&str, &str)] = &[\n"
    );
    for (id, text) in sources {
        generated.push_str(&format!("    ({id:?}, {text:?}),\n"));
    }
    generated.push_str("];\nconst PRODUCT_ART: &[(&str, &[u8])] = &[\n");
    for product in catalog.products() {
        if let Some(image) = &product.image {
            let bytes = crate::catalog_art::read(&products_dir.join(&product.id), image)?;
            fs::create_dir_all(out_dir).map_err(|e| CatalogError(e.to_string()))?;
            let bundled = out_dir.join(format!("catalog-art-{}.png", product.id));
            fs::write(&bundled, bytes).map_err(|e| CatalogError(e.to_string()))?;
            generated.push_str(&format!("    ({:?}, include_bytes!(concat!(env!(\"OUT_DIR\"), \"/catalog-art-{}.png\"))),\n", product.id, product.id));
        }
    }
    generated.push_str("];\n");
    Ok((catalog, generated))
}

/// Checks a parsed image against its catalog entry and crate version, never catalog order.
/// Returns the crate manifest Cargo must watch.
pub fn check_firmware(
    catalog: &Catalog,
    root: &Path,
    image: &FirmwareImage<'_>,
) -> Result<PathBuf, CatalogError> {
    let record = image.identity();
    let (_, board) = catalog
        .by_wire(record.product, record.board)
        .ok_or_else(|| CatalogError("bundled image names an unknown product/board".into()))?;
    let manifest = root.join(&board.firmware.crate_dir).join("Cargo.toml");
    let expected_version = crate_version(&read(&manifest)?)
        .ok_or_else(|| CatalogError(format!("{} has no package version", manifest.display())))?;
    if record.version != expected_version || record.min_protocol != board.firmware.min_protocol {
        return Err(CatalogError(format!("bundled image differs from {} version or catalog minimum protocol; rebuild the firmware", manifest.display())));
    }
    Ok(manifest)
}

/// Resolves explicit build inputs without reading process-global environment variables.
pub fn firmware_paths(
    legacy: Option<&std::ffi::OsStr>,
    manifest: Option<&std::ffi::OsStr>,
) -> Result<Vec<PathBuf>, CatalogError> {
    match (legacy, manifest) {
        (Some(_), Some(_)) => Err(CatalogError(
            "set only one of KIVORI_FIRMWARE_PATH and KIVORI_FIRMWARE_MANIFEST".into(),
        )),
        (Some(path), None) => Ok(vec![PathBuf::from(path)]),
        (None, Some(path)) => {
            let path = Path::new(path);
            if !path.is_absolute() {
                return Err(CatalogError(
                    "firmware manifest path must be absolute".into(),
                ));
            }
            let text = fs::read_to_string(path)
                .map_err(|error| CatalogError(format!("{}: {error}", path.display())))?;
            let paths: Vec<PathBuf> = serde_json::from_str::<Vec<String>>(&text)
                .map_err(|error| CatalogError(format!("invalid firmware manifest: {error}")))?
                .into_iter()
                .map(PathBuf::from)
                .collect();
            if paths.is_empty() || paths.iter().any(|path| !path.is_absolute()) {
                return Err(CatalogError(
                    "firmware manifest must list one or more absolute image paths".into(),
                ));
            }
            Ok(paths)
        }
        (None, None) => Ok(Vec::new()),
    }
}

/// Validated image-set output and the manifests Cargo must watch.
pub struct GeneratedFirmware {
    pub source: String,
    pub manifests: Vec<PathBuf>,
}

/// Manifest builds must contain exactly the active catalog pairs; single-image inputs may be partial.
pub fn require_complete(
    catalog: &Catalog,
    bundle: &FirmwareBundle<'_>,
) -> Result<(), CatalogError> {
    let expected: std::collections::BTreeSet<_> = catalog
        .products()
        .iter()
        .flat_map(|product| {
            product
                .boards
                .iter()
                .map(move |board| (product.number, board.number))
        })
        .collect();
    let actual: std::collections::BTreeSet<_> = bundle
        .images()
        .iter()
        .map(|image| (image.identity().product, image.identity().board))
        .collect();
    if actual != expected {
        return Err(CatalogError(
            "firmware manifest must contain exactly the complete active catalog set".into(),
        ));
    }
    Ok(())
}

/// Validates and emits deterministic image sources, independent of input/catalog order.
/// Manifest (`complete`) mode requires exactly the active catalog identity set.
pub fn generate_firmware(
    catalog: &Catalog,
    root: &Path,
    paths: &[PathBuf],
    out: &Path,
    complete: bool,
) -> Result<GeneratedFirmware, CatalogError> {
    let bytes = paths
        .iter()
        .map(|path| {
            fs::read(path).map_err(|error| CatalogError(format!("{}: {error}", path.display())))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let refs: Vec<&[u8]> = bytes.iter().map(Vec::as_slice).collect();
    let bundle = FirmwareBundle::parse(&refs).map_err(|message| CatalogError(message.into()))?;
    if complete {
        require_complete(catalog, &bundle)?;
    }
    let mut manifests = Vec::new();
    // Validate every entry before emitting any image, so an invalid set never gets a partial index.
    for image in bundle.images() {
        let manifest = check_firmware(catalog, root, image)?;
        manifests.push(manifest);
    }
    manifests.sort();
    manifests.dedup();
    fs::create_dir_all(out).map_err(|error| CatalogError(error.to_string()))?;
    let mut source = String::from("const FIRMWARE_IMAGES: &[&[u8]] = &[\n");
    for image in bundle.images() {
        let record = image.identity();
        let file = format!("firmware-{}-{}.elf", record.product, record.board);
        fs::write(out.join(&file), image.bytes())
            .map_err(|error| CatalogError(error.to_string()))?;
        source.push_str(&format!(
            "    include_bytes!(concat!(env!(\"OUT_DIR\"), \"/{file}\")),\n"
        ));
    }
    source.push_str("];\n");
    Ok(GeneratedFirmware { source, manifests })
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
        let (one, _) = generate(&root.path().join("products"), &root.path().join("out")).unwrap();
        assert_eq!(one.products().len(), 1);
        add_product(root.path(), "lada");
        let mut art_assertions = String::new();
        for (index, id) in ["lada", "pala"].iter().enumerate() {
            let bytes = crate::platform::png::encode_rgba(1, 1, &[index as u8, 1, 2, 255]).unwrap();
            let directory = root.path().join("products").join(id);
            fs::write(directory.join("unit.png"), &bytes).unwrap();
            fs::write(
                directory.join("product.toml"),
                format!("image = \"unit.png\"\n{}", product(id)),
            )
            .unwrap();
            art_assertions.push_str(&format!("assert_eq!(PRODUCT_ART[{index}].0, {id:?}); assert_eq!(PRODUCT_ART[{index}].1, &{bytes:?});"));
        }
        let expected_pala =
            fs::read_to_string(root.path().join("products/pala/product.toml")).unwrap();
        let (catalog, source) =
            generate(&root.path().join("products"), &root.path().join("out")).unwrap();
        assert_eq!(
            catalog
                .products()
                .iter()
                .map(|p| p.id.as_str())
                .collect::<Vec<_>>(),
            ["lada", "pala"]
        );
        assert!(catalog.product("reserved").is_none());
        assert_eq!(
            source,
            generate(&root.path().join("products"), &root.path().join("out"))
                .unwrap()
                .1
        );

        // Snapshot bytes must survive source art removal before compilation.
        for id in ["lada", "pala"] {
            fs::remove_file(root.path().join("products").join(id).join("unit.png")).unwrap();
        }
        // Compile the actual generated constants, including quoted/newline metadata, outside
        // shipping discovery. This checks emission rather than a second implementation of it.
        let harness = root.path().join("embedded.rs");
        fs::write(&harness, format!("{source}\nfn main() {{ assert!(REGISTRY.contains(\"reserved\")); assert_eq!(PRODUCT_FILES.len(), 2); assert_eq!(PRODUCT_FILES[0].0, \"lada\"); assert_eq!(PRODUCT_FILES[1], (\"pala\", {:?})); assert_eq!(PRODUCT_ART.len(), 2); {art_assertions} }}", expected_pala)).unwrap();
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
                .env("OUT_DIR", root.path().join("out"))
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
            assert!(
                generate(&products, &root.path().join("out")).is_err(),
                "{case} must be refused"
            );
        }
    }

    #[test]
    fn root_dotfiles_are_ignored_but_dot_directories_are_not_product_sources() {
        let root = fixture();
        let products = root.path().join("products");
        let (_, expected) = generate(&products, &root.path().join("out")).unwrap();
        for name in [".DS_Store", ".registry.toml.swp"] {
            fs::write(products.join(name), "not catalog data").unwrap();
        }
        assert_eq!(
            generate(&products, &root.path().join("out")).unwrap().1,
            expected
        );
        fs::create_dir(products.join(".fixtures")).unwrap();
        assert!(generate(&products, &root.path().join("out")).is_err());
    }

    #[test]
    fn presentation_must_fit_the_desktop_ipc_limit() {
        for (old, replacement) in [
            ("Test pala", "X".repeat(97)),
            ("Test pala", "😀".repeat(49)),
            ("KV-T1", format!("KV-{}", "X".repeat(94))),
        ] {
            let root = fixture();
            let path = root.path().join("products/pala/product.toml");
            fs::write(path, product("pala").replace(old, &replacement)).unwrap();
            let error =
                generate(&root.path().join("products"), &root.path().join("out")).unwrap_err();
            assert!(error.0.contains("IPC limit"), "{error}");
        }
    }

    #[test]
    fn missing_declared_art_is_refused() {
        let root = fixture();
        let path = root.path().join("products/pala/product.toml");
        fs::write(
            path,
            product("pala").replace(
                "required_capabilities = []",
                "image = \"art.png\"\nrequired_capabilities = []",
            ),
        )
        .unwrap();
        let error = generate(&root.path().join("products"), &root.path().join("out")).unwrap_err();
        assert!(error.0.contains("product art `art.png`"), "{error}");
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
                generate(&products, &root.path().join("out")).is_err(),
                "{case} symlink must be refused"
            );
        }
    }

    fn image(product: u16, board: u16, version: &str, protocol: &str) -> Vec<u8> {
        let mut bytes = vec![0; 52];
        bytes[..6].copy_from_slice(b"\x7fELF\x01\x01");
        bytes[18] = 243;
        bytes.extend_from_slice(format!("KIVORI-FW-VERSION:{version}\0KIVORI-FW-IDENTITY:p={product};b={board};v={version};m={protocol}\0").as_bytes());
        bytes
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
        let (catalog, _) =
            generate(&root.path().join("products"), &root.path().join("out")).unwrap();
        // pala is second in catalog order, but each image still selects its own crate.
        for (product, version, protocol, crate_id) in
            [(1, "0.1.0", "0.1", "pala"), (2, "0.2.0", "0.2", "lada")]
        {
            let bytes = image(product, 1, version, protocol);
            let parsed = FirmwareImage::parse(&bytes).unwrap();
            assert_eq!(
                check_firmware(&catalog, root.path(), &parsed).unwrap(),
                root.path().join(format!("firmware/{crate_id}/Cargo.toml"))
            );
        }
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
            let result = FirmwareImage::parse(&bad)
                .map_err(|message| CatalogError(message.into()))
                .and_then(|image| check_firmware(&catalog, root.path(), &image));
            assert!(result.is_err());
        }
    }

    #[test]
    fn firmware_inputs_are_explicit_and_exclusive() {
        assert!(firmware_paths(None, None).unwrap().is_empty());
        let root = tempfile::tempdir().unwrap();
        let legacy = root.path().join("legacy.elf");
        let manifest = root.path().join("images.json");
        assert_eq!(
            firmware_paths(Some(legacy.as_os_str()), None)
                .unwrap()
                .as_slice(),
            std::slice::from_ref(&legacy)
        );
        assert!(firmware_paths(Some(legacy.as_os_str()), Some(manifest.as_os_str())).is_err());
        assert!(firmware_paths(None, Some(manifest.as_os_str())).is_err());
        assert!(firmware_paths(None, Some(root.path().as_os_str())).is_err());
        assert!(firmware_paths(None, Some(std::ffi::OsStr::new("relative.json"))).is_err());
        for text in [
            "not json",
            "{}",
            "[]",
            "[1]",
            "[\"relative.elf\"]",
            "[\"\"]",
        ] {
            fs::write(&manifest, text).unwrap();
            assert!(
                firmware_paths(None, Some(manifest.as_os_str())).is_err(),
                "{text}"
            );
        }
        fs::write(&manifest, serde_json::to_string(&[&legacy]).unwrap()).unwrap();
        assert_eq!(
            firmware_paths(None, Some(manifest.as_os_str())).unwrap(),
            [legacy]
        );
    }

    #[test]
    fn firmware_set_generation_is_deterministic_and_compiles_with_exact_lookup() {
        let root = fixture();
        add_product(root.path(), "lada");
        fs::write(
            root.path().join("firmware/lada/Cargo.toml"),
            "[package]\nname = 'test'\nversion = '0.2.0'\n",
        )
        .unwrap();
        fs::write(
            root.path().join("products/lada/product.toml"),
            product("lada").replace("min_protocol = \"0.1\"", "min_protocol = \"0.2\""),
        )
        .unwrap();
        let out = root.path().join("out");
        let (catalog, _) = generate(&root.path().join("products"), &out).unwrap();
        let first = image(1, 1, "0.1.0", "0.1");
        let second = image(2, 1, "0.2.0", "0.2");
        let a = root.path().join("first.elf");
        let b = root.path().join("second.elf");
        fs::write(&a, &first).unwrap();
        fs::write(&b, &second).unwrap();
        let incomplete = root.path().join("incomplete");
        assert!(generate_firmware(
            &catalog,
            root.path(),
            std::slice::from_ref(&a),
            &incomplete,
            true
        )
        .is_err());
        assert!(!incomplete.exists(), "incomplete manifests emit nothing");
        assert!(require_complete(&catalog, &FirmwareBundle::parse(&[]).unwrap()).is_err());
        let extra = image(99, 1, "0.1.0", "0.1");
        assert!(require_complete(
            &catalog,
            &FirmwareBundle::parse(&[&first, &second, &extra]).unwrap()
        )
        .is_err());
        assert!(
            require_complete(&catalog, &FirmwareBundle::parse(&[&first, &extra]).unwrap()).is_err()
        );
        // The explicit legacy image path still supports a subset for development/provisioning.
        generate_firmware(
            &catalog,
            root.path(),
            std::slice::from_ref(&a),
            &incomplete,
            false,
        )
        .unwrap();
        let generated =
            generate_firmware(&catalog, root.path(), &[b.clone(), a.clone()], &out, true).unwrap();
        assert!(
            generated.source.find("firmware-1-1.elf").unwrap()
                < generated.source.find("firmware-2-1.elf").unwrap()
        );
        assert_eq!(generated.manifests.len(), 2);
        let forward =
            generate_firmware(&catalog, root.path(), &[a.clone(), b.clone()], &out, true).unwrap();
        assert_eq!(generated.source, forward.source);
        let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let harness = format!("#[path = {:?}] mod firmware_marker;\n#[path = {:?}] mod firmware_bundle;\n{}\nfn main() {{ let bundle = firmware_bundle::FirmwareBundle::parse(FIRMWARE_IMAGES).unwrap(); assert_eq!(bundle.get(1,1).unwrap().bytes(), &{:?}); assert_eq!(bundle.get(2,1).unwrap().bytes(), &{:?}); assert!(bundle.get(9,1).is_none()); assert!(bundle.get(1,9).is_none()); assert!(bundle.single().is_none()); }}", src.join("firmware_marker.rs"), src.join("firmware_bundle.rs"), generated.source, first, second);
        let source = root.path().join("bundle_check.rs");
        fs::write(&source, harness).unwrap();
        let executable = root
            .path()
            .join(format!("bundle_check{}", std::env::consts::EXE_SUFFIX));
        let compiler = std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into());
        let result = std::process::Command::new(compiler)
            .args(["--edition=2021", "-A", "dead_code"])
            .arg(&source)
            .arg("-o")
            .arg(&executable)
            .env("OUT_DIR", &out)
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert!(std::process::Command::new(&executable)
            .status()
            .unwrap()
            .success());
        assert!(
            generate_firmware(&catalog, root.path(), &[a.clone(), a.clone()], &out, false).is_err()
        );
        fs::write(&b, &first).unwrap();
        assert!(generate_firmware(&catalog, root.path(), &[a, b], &out, false).is_err());
    }

    #[test]
    fn invalid_or_missing_firmware_never_emits_a_partial_index() {
        let root = fixture();
        let (catalog, _) = generate(
            &root.path().join("products"),
            &root.path().join("catalog-out"),
        )
        .unwrap();
        let path = root.path().join("image.elf");
        let out = root.path().join("firmware-out");
        assert!(generate_firmware(
            &catalog,
            root.path(),
            std::slice::from_ref(&path),
            &out,
            false
        )
        .is_err());
        for bytes in [
            Vec::new(),
            b"not ELF".to_vec(),
            image(9, 1, "0.1.0", "0.1"),
            image(1, 1, "0.2.0", "0.1"),
            image(1, 1, "0.1.0", "0.2"),
        ] {
            fs::write(&path, bytes).unwrap();
            assert!(generate_firmware(
                &catalog,
                root.path(),
                std::slice::from_ref(&path),
                &out,
                false
            )
            .is_err());
            assert!(!out.exists());
        }
        let valid = root.path().join("valid.elf");
        fs::write(&valid, image(1, 1, "0.1.0", "0.1")).unwrap();
        fs::write(&path, image(9, 1, "0.1.0", "0.1")).unwrap();
        assert!(
            generate_firmware(&catalog, root.path(), &[valid, path.clone()], &out, false).is_err()
        );
        assert!(!out.exists());
        fs::write(&path, image(1, 1, "0.1.0", "0.1")).unwrap();
        let one = generate_firmware(
            &catalog,
            root.path(),
            std::slice::from_ref(&path),
            &out,
            false,
        )
        .unwrap();
        assert!(one.source.contains("firmware-1-1.elf"));
        assert_eq!(
            fs::read(out.join("firmware-1-1.elf")).unwrap(),
            image(1, 1, "0.1.0", "0.1")
        );
        let empty = generate_firmware(&catalog, root.path(), &[], &out, false).unwrap();
        assert_eq!(empty.source, "const FIRMWARE_IMAGES: &[&[u8]] = &[\n];\n");
    }
}
