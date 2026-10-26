use std::{fs, path::PathBuf};

fn main() {
    let out = PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
    fs::write(
        out.join("pip.assets"),
        kivori_asset_compiler::compile_default_blob(),
    )
    .unwrap();
    let manifest = fs::read_to_string("../Cargo.toml").unwrap();
    let version = manifest
        .lines()
        .find_map(|line| line.strip_prefix("version = \""))
        .unwrap()
        .trim_end_matches('"');
    let parts: Vec<_> = version.split('.').collect();
    assert_eq!(parts.len(), 3);
    fs::write(out.join("identity.rs"), format!("const VERSION: FirmwareVersion = FirmwareVersion {{ major: {}, minor: {}, patch: {} }};", parts[0], parts[1], parts[2])).unwrap();
    for path in ["../Cargo.toml", "../../../assets"] {
        println!("cargo:rerun-if-changed={path}");
    }
}
