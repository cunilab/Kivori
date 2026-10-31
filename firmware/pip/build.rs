use std::path::Path;

/// The product and board this firmware crate builds (ADR-0014). Their wire numbers and the image's
/// minimum protocol are read from `products/`, so no number is typed twice. A second product's
/// firmware crate names its own pair here.
const PRODUCT: &str = "pip";
const BOARD: &str = "c3-supermini";

/// Looks up `key = <value>` inside the `[section]` of a (simple, flat) TOML file. Only the shapes
/// `products/` uses are understood: bare keys, integer or quoted-string values, `#` comments.
fn toml_value(text: &str, section: &str, key: &str) -> Option<String> {
    let mut current = "";
    for line in text.lines() {
        let line = line.split('#').next().unwrap_or("").trim();
        if let Some(header) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            current = header.trim();
        } else if current == section {
            if let Some((k, v)) = line.split_once('=') {
                if k.trim() == key {
                    return Some(v.trim().trim_matches('"').to_string());
                }
            }
        }
    }
    None
}

/// Exports the build identity as `KIVORI_*` compile-time environment (read with `env!`).
fn export_identity() {
    let registry = std::fs::read_to_string("../../products/registry.toml")
        .expect("read products/registry.toml");
    let product_toml = std::fs::read_to_string(format!("../../products/{PRODUCT}/product.toml"))
        .expect("read the product's product.toml");
    let product = toml_value(&registry, "products", PRODUCT).expect("product is in the registry");
    let board = toml_value(&registry, "boards", BOARD).expect("board is in the registry");
    let min_protocol = toml_value(
        &product_toml,
        &format!("boards.{BOARD}.firmware"),
        "min_protocol",
    )
    .expect("catalog names a min_protocol for this board");
    for (name, value) in [
        ("KIVORI_PRODUCT_NUM", product),
        ("KIVORI_BOARD_NUM", board),
        ("KIVORI_MIN_PROTOCOL", min_protocol),
    ] {
        println!("cargo:rustc-env={name}={value}");
    }
    println!("cargo:rerun-if-changed=../../products");
}

/// Everything that goes into this firmware image, relative to the repository root. An uncommitted
/// change under one of these makes the build "dirty" (reported on the wire as `BuildInfo`); an edit
/// to the simulator, the tests or the desktop app does not, because it cannot change the image.
/// Extend it with the firmware's build inputs (`docs/release.md`, firmware release guard).
const BUILD_PATHS: &[&str] = &[
    "firmware/pip/src",
    "firmware/pip/build.rs",
    "firmware/pip/Cargo.toml",
    "firmware/pip/Cargo.lock",
    "firmware/pip/rust-toolchain.toml",
    "firmware/pip/.cargo",
    "crates",
    "tools/asset-compiler",
    "assets",
    "products",
    "Cargo.toml",
    "Cargo.lock",
    "rust-toolchain.toml",
];

fn main() {
    export_identity();
    // The commit and dirty flag of this build (`KIVORI_BUILD_COMMIT`, `KIVORI_BUILD_DIRTY`), or
    // "unknown" without git. Never anything that varies between two builds of the same commit.
    kivori_asset_compiler::build_id::emit("KIVORI_BUILD", Path::new("../.."), BUILD_PATHS);
    // The flashable binary (`embedded`, and so `physical-st7789`) renders real scenes, so it needs the
    // canonical compiled asset blob. Compile it here (host-side, build-time only) and hand it to the
    // firmware via OUT_DIR — the device never parses SVG at runtime (Constitution XI). Skipped for every
    // other build so the ordinary RISC-V library build stays fast.
    if std::env::var("CARGO_FEATURE_EMBEDDED").is_ok() {
        let blob = kivori_asset_compiler::compile_default_blob();
        let out_dir = std::env::var("OUT_DIR").expect("OUT_DIR is set by cargo");
        std::fs::write(std::path::Path::new(&out_dir).join("kivori.assets"), &blob)
            .expect("write compiled asset blob");
    }
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=../../assets/mascot.svg");
}
