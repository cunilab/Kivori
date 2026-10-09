use std::path::Path;
use std::process::Command;

#[path = "src/firmware_marker.rs"]
mod firmware_marker;

fn main() {
    println!("cargo:rerun-if-changed=../../../assets/mascot.svg");
    // Compile the canonical asset blob into OUT_DIR at build time so the binary bundles it. The runtime
    // reads it via `include_bytes!` — no SVG/resvg at runtime (Constitution XI). Build-time only.
    let blob = kivori_asset_compiler::compile_default_blob();
    let out_dir = std::env::var("OUT_DIR").expect("OUT_DIR is set by cargo");
    std::fs::write(Path::new(&out_dir).join("kivori.assets"), &blob)
        .expect("write compiled asset blob");

    // Firmware is built in its isolated workspace first. Opt in explicitly so ordinary host/CI
    // builds do not silently package an old cross-compiled artifact from a previous local build.
    println!("cargo:rerun-if-env-changed=KIVORI_FIRMWARE_PATH");
    let firmware = match std::env::var_os("KIVORI_FIRMWARE_PATH") {
        Some(path) => {
            let path = Path::new(&path);
            println!("cargo:rerun-if-changed={}", path.display());
            let bytes = std::fs::read(path).expect("read requested firmware ELF");
            assert!(
                bytes.len() >= 52 && &bytes[..6] == b"\x7fELF\x01\x01" && bytes[18..20] == [243, 0],
                "firmware must be a little-endian ELF32 RISC-V image"
            );
            bytes
        }
        None => Vec::new(),
    };
    // The bundled firmware's own version, read from its image so it can never drift from what gets
    // flashed. Empty when no firmware is embedded (ordinary host/CI builds).
    let bundled_version = firmware_marker::scan_firmware_version(&firmware).unwrap_or_default();
    println!("cargo:rustc-env=KIVORI_BUNDLED_FIRMWARE_VERSION={bundled_version}");
    std::fs::write(Path::new(&out_dir).join("kivori-firmware.elf"), firmware)
        .expect("write bundled firmware");

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        build_mediaremote_adapter(Path::new(&out_dir));
    }

    tauri_build::build();
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
