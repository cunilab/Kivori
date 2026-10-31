use std::path::Path;
use std::process::Command;

#[path = "src/catalog.rs"]
#[allow(dead_code)]
mod catalog;
mod catalog_art;
mod catalog_build;
#[path = "src/firmware_bundle.rs"]
#[allow(dead_code)]
mod firmware_bundle;
#[path = "src/firmware_marker.rs"]
#[allow(dead_code)]
mod firmware_marker;

fn main() {
    println!("cargo:rerun-if-changed=../../../assets/mascot.svg");
    // Compile the canonical asset blob into OUT_DIR at build time so the binary bundles it. The runtime
    // reads it via `include_bytes!` — no SVG/resvg at runtime (Constitution XI). Build-time only.
    let blob = kivori_asset_compiler::compile_default_blob();
    let out_dir = std::env::var("OUT_DIR").expect("OUT_DIR is set by cargo");
    std::fs::write(Path::new(&out_dir).join("kivori.assets"), &blob)
        .expect("write compiled asset blob");

    // Cargo scans this directory recursively: metadata and artwork edits/additions/removals rebuild.
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let products = root.join("products");
    println!("cargo:rerun-if-changed={}", products.display());
    let (catalog, source) =
        catalog_build::generate(&products, Path::new(&out_dir)).expect("valid product catalog");
    std::fs::write(Path::new(&out_dir).join("catalog_sources.rs"), source)
        .expect("write embedded catalog sources");

    // The firmware source id of this checkout (the same hash the firmware stamps into itself, from
    // the input list beside the catalog's firmware crate), compared with the device's in developer
    // mode (`BUILD_INFO_V1`). Only the id is used.
    match catalog
        .products()
        .first()
        .and_then(|product| product.boards.first())
    {
        Some(board) => {
            let crate_dir = board
                .firmware
                .crate_dir
                .to_string_lossy()
                .replace('\\', "/");
            kivori_asset_compiler::build_id::emit(
                "KIVORI_APP",
                &root,
                &format!("{crate_dir}/build-inputs.txt"),
            );
        }
        None => println!("cargo:rustc-env=KIVORI_APP_SOURCE=unknown"),
    }

    println!("cargo:rerun-if-env-changed=KIVORI_FIRMWARE_PATH");
    println!("cargo:rerun-if-env-changed=KIVORI_FIRMWARE_MANIFEST");
    let legacy = std::env::var_os("KIVORI_FIRMWARE_PATH");
    let manifest = std::env::var_os("KIVORI_FIRMWARE_MANIFEST");
    let paths = catalog_build::firmware_paths(legacy.as_deref(), manifest.as_deref())
        .expect("explicit firmware input");
    if let Some(path) = &manifest {
        println!("cargo:rerun-if-changed={}", Path::new(path).display());
    }
    for path in &paths {
        println!("cargo:rerun-if-changed={}", path.display());
    }
    let generated = catalog_build::generate_firmware(
        &catalog,
        &root,
        &paths,
        Path::new(&out_dir),
        manifest.is_some(),
    )
    .expect("bundled firmware matches its catalog identity");
    for manifest in generated.manifests {
        println!("cargo:rerun-if-changed={}", manifest.display());
    }
    std::fs::write(
        Path::new(&out_dir).join("firmware_sources.rs"),
        generated.source,
    )
    .expect("write embedded firmware sources");

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        build_mediaremote_adapter(Path::new(&out_dir));
    }

    tauri_build::build();

    // Tauri embeds its Windows activation manifest in bins only; the dev GUI is an example.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows")
        && std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc")
        && std::env::var_os("CARGO_FEATURE_VIRTUAL_DEVICE").is_some()
    {
        let resource = Path::new(&out_dir).join("resource.lib");
        assert!(resource.is_file(), "Tauri Windows resource is missing");
        println!("cargo:rustc-link-arg-examples={}", resource.display());
    }
}

/// Compiles the vendored mediaremote-adapter (ADR-0009) with plain clang into
/// `OUT_DIR/mediaremote-adapter/` and copies that directory to `target/<profile>/` for bundling.
/// Never fails the build: without clang/SDK it warns and the app runs with the adapter
/// unavailable (AppleScript fallback). Both architectures, because `/usr/bin/perl` loads it
/// natively on either Mac. Sets `KIVORI_MEDIAREMOTE_ADAPTER_DIR` for dev/test runs.
fn build_mediaremote_adapter(out_dir: &Path) {
    let src = Path::new("vendor/mediaremote-adapter");
    println!("cargo:rerun-if-changed={}", src.display());
    let dir = out_dir.join("mediaremote-adapter");
    let framework = dir.join("MediaRemoteAdapter.framework");
    let lib = framework.join("MediaRemoteAdapter");
    let client = dir.join("MediaRemoteAdapterTestClient");
    let sources = |sub: &str| -> Vec<std::path::PathBuf> {
        std::fs::read_dir(src.join("src").join(sub))
            .map(|entries| {
                entries
                    .filter_map(|e| e.ok().map(|e| e.path()))
                    .filter(|p| p.extension().is_some_and(|x| x == "m"))
                    .collect()
            })
            .unwrap_or_default()
    };
    let min = std::env::var("MACOSX_DEPLOYMENT_TARGET").unwrap_or_else(|_| "11.0".to_string());
    let clang = |out: &Path| {
        let mut cmd = Command::new("xcrun");
        cmd.args([
            "clang",
            "-fobjc-arc",
            "-O2",
            "-arch",
            "arm64",
            "-arch",
            "x86_64",
        ])
        .arg(format!("-mmacosx-version-min={min}"))
        .arg("-o")
        .arg(out);
        cmd
    };
    let lib_sources: Vec<_> = ["adapter", "private", "utility"]
        .into_iter()
        .flat_map(sources)
        .collect();
    let client_sources = sources("test");
    let mut lib_cmd = clang(&lib);
    lib_cmd
        .args(["-dynamiclib", "-fvisibility=default", "-Wno-everything"])
        .arg(format!("-I{}", src.join("include").display()))
        .arg(format!("-I{}", src.join("src").display()))
        .args(&lib_sources)
        .args(["-framework", "Foundation", "-framework", "AppKit"])
        .args(["-framework", "UniformTypeIdentifiers"]);
    let mut client_cmd = clang(&client);
    client_cmd
        .arg("-Wno-everything")
        .arg(format!("-I{}", src.join("src/test").display()))
        .args(&client_sources)
        .args(["-framework", "Foundation", "-framework", "MediaPlayer"]);

    let built = !lib_sources.is_empty()
        && !client_sources.is_empty()
        && std::fs::create_dir_all(&framework).is_ok()
        && [lib_cmd, client_cmd]
            .iter_mut()
            .all(|cmd| cmd.status().is_ok_and(|s| s.success()))
        && std::fs::copy(
            src.join("bin/mediaremote-adapter.pl"),
            dir.join("mediaremote-adapter.pl"),
        )
        .is_ok()
        && std::fs::copy(src.join("LICENSE"), dir.join("LICENSE")).is_ok();
    if !built {
        println!(
            "cargo:warning=mediaremote-adapter not built (needs Xcode Command Line Tools); \
             macOS now-playing falls back to AppleScript"
        );
        return;
    }
    // Ad-hoc signature, as upstream's CMake does (Developer ID signing is a release step).
    for bin in [&lib, &client] {
        let _ = Command::new("codesign")
            .args(["--force", "--sign", "-"])
            .arg(bin)
            .status();
    }
    println!(
        "cargo:rustc-env=KIVORI_MEDIAREMOTE_ADAPTER_DIR={}",
        dir.display()
    );
    // OUT_DIR is target/<profile>/build/<pkg>-<hash>/out; a stable copy lets tauri.conf.json
    // bundle it as a resource.
    if let Some(profile_dir) = out_dir.ancestors().nth(3) {
        let staged = profile_dir.join("mediaremote-adapter");
        let _ = std::fs::create_dir_all(staged.join("MediaRemoteAdapter.framework"));
        for rel in [
            "MediaRemoteAdapter.framework/MediaRemoteAdapter",
            "MediaRemoteAdapterTestClient",
            "mediaremote-adapter.pl",
            "LICENSE",
        ] {
            let _ = std::fs::copy(dir.join(rel), staged.join(rel));
        }
    }
}
