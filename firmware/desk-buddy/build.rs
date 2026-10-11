fn main() {
    // The production runtime and every simulation mode render real scenes, so they need the
    // canonical compiled asset blob. Compile it here (host-side, build-time only) and hand it to the
    // firmware via OUT_DIR — the device never parses SVG at runtime (Constitution XI). Skipped for every
    // other build so the ordinary RISC-V library build stays fast.
    // Every artifact that renders needs it: the production runtime (`embedded`) plus each simulation mode.
    if std::env::var("CARGO_FEATURE_EMBEDDED").is_ok() {
        let blob = kivori_asset_compiler::compile_default_blob();
        let out_dir = std::env::var("OUT_DIR").expect("OUT_DIR is set by cargo");
        std::fs::write(std::path::Path::new(&out_dir).join("kivori.assets"), &blob)
            .expect("write compiled asset blob");
    }
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=../../assets/mascot.svg");
}
