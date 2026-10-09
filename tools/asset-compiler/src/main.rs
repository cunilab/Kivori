//! Kivori asset compiler (host) — compiles the layered mascot SVG into the deterministic RGB565
//! mascot asset blob and writes it to `assets/compiled/kivori.assets` (ADR-0004, Principle XI).

use std::fs;
use std::path::Path;

fn main() {
    let blob = kivori_asset_compiler::compile_default_blob();
    let out = Path::new("assets/compiled/kivori.assets");
    if let Some(dir) = out.parent() {
        fs::create_dir_all(dir).expect("create assets/compiled");
    }
    fs::write(out, &blob).expect("write asset blob");
    eprintln!(
        "kivori-asset-compiler: wrote {} bytes ({:.1}% of 128 KiB) to {}",
        blob.len(),
        blob.len() as f64 / 131_072.0 * 100.0,
        out.display()
    );
}
