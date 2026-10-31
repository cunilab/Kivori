//! Compiles the canonical asset blob (the same bytes the product firmware embeds) and exposes the
//! linker script to the final link.
use std::{env, fs, path::PathBuf};

fn main() {
    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    fs::write(
        out.join("kivori.assets"),
        kivori_asset_compiler::compile_default_blob(),
    )
    .unwrap();
    // The firmware crate's `.cargo/config.toml` (a parent directory) already passes `-Tlinkall.x` and
    // cargo merges rustflags, so the memory map is provided under that name.
    fs::copy("link.x", out.join("linkall.x")).unwrap();
    println!("cargo:rustc-link-search={}", out.display());
    println!("cargo:rerun-if-changed=link.x");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=../../../../assets/mascot.svg");
}
