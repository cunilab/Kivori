//! T051 — no runtime SVG/PNG parser is linked into the asset runtime (FR-021, Principle XI).
//!
//! Source art is compiled to an RGB565 blob at build time; the device and the runtime reader only ever see
//! the compiled bytes. That guarantee is architectural, so this test asserts it against the **manifests**
//! rather than at run time: a vector/raster parser must not appear as a dependency of `kivori-assets`, of any
//! other shared `no_std` crate, or of the firmware crate.
//!
//! `scripts/check-crate-boundaries.sh` enforces the same rule in CI via `cargo metadata`. This test is the
//! in-crate counterpart, so the guarantee survives even when the shell guard is not run, and it carries a
//! negative control so it cannot pass vacuously.

use std::path::{Path, PathBuf};

/// Crates that parse SVG or raster images. None may be reachable from the runtime asset path.
const BANNED: [&str; 8] = [
    "resvg",
    "usvg",
    "tiny-skia",
    "image",
    "png",
    "jpeg-decoder",
    "svgtypes",
    "roxmltree",
];

/// Manifests that must stay free of the banned crates: the runtime reader, every shared `no_std` crate, and
/// the firmware. The asset *compiler* is deliberately absent — it is a host build tool and is expected to
/// parse SVG.
const GUARDED: [&str; 6] = [
    "crates/kivori-assets/Cargo.toml",
    "crates/kivori-model/Cargo.toml",
    "crates/kivori-protocol/Cargo.toml",
    "crates/kivori-framebuffer/Cargo.toml",
    "crates/kivori-renderer/Cargo.toml",
    "firmware/pala/Cargo.toml",
];

fn repo_root() -> PathBuf {
    // tests run from the crate directory; the workspace root is two levels up (crates/<name>).
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("workspace root above crates/<name>")
        .to_path_buf()
}

/// Dependency names declared in a Cargo manifest, from every `[dependencies]`-like table.
///
/// Deliberately simple line scanning rather than a TOML dependency: a `key = ...` line inside a dependency
/// table is a dependency name, which is all this guard needs.
fn declared_dependencies(manifest: &str) -> Vec<String> {
    let mut names = Vec::new();
    let mut in_deps = false;
    for line in manifest.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            // Covers [dependencies], [dev-dependencies], [build-dependencies] and their target-specific
            // and per-dependency forms.
            in_deps = trimmed.contains("dependencies");
            // `[dependencies.foo]` declares `foo` itself.
            if let Some(rest) = trimmed.strip_prefix("[dependencies.") {
                names.push(rest.trim_end_matches(']').to_string());
            }
            continue;
        }
        if !in_deps || trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if let Some((name, _)) = trimmed.split_once('=') {
            names.push(name.trim().trim_matches('"').to_string());
        }
    }
    names
}

#[test]
fn no_runtime_svg_or_raster_parser_is_declared() {
    let root = repo_root();
    let mut violations = Vec::new();
    for manifest in GUARDED {
        let path = root.join(manifest);
        let text = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
        for dep in declared_dependencies(&text) {
            if BANNED.contains(&dep.as_str()) {
                violations.push(format!("{manifest} declares '{dep}'"));
            }
        }
    }
    assert!(
        violations.is_empty(),
        "runtime SVG/PNG parser reachable from the asset runtime (FR-021): {violations:?}"
    );
}

#[test]
fn every_guarded_manifest_exists_and_declares_something() {
    // Without this, a renamed or moved manifest would make the guard above silently vacuous.
    let root = repo_root();
    for manifest in GUARDED {
        let path = root.join(manifest);
        assert!(path.is_file(), "guarded manifest missing: {manifest}");
        let deps = declared_dependencies(&std::fs::read_to_string(&path).expect("read manifest"));
        assert!(
            !deps.is_empty(),
            "{manifest} parsed to zero dependencies — the parser or the manifest layout changed"
        );
    }
}

#[test]
fn the_detector_is_not_vacuous() {
    // Negative control: a synthetic manifest that DOES pull a raster parser must be caught, in each of the
    // declaration forms Cargo accepts.
    let inline = "\
[package]\nname = \"x\"\n\n[dependencies]\nserde = \"1\"\nresvg = \"0.40\"\n";
    let table = "\
[package]\nname = \"x\"\n\n[dependencies.image]\nversion = \"0.25\"\n";
    let dev = "\
[package]\nname = \"x\"\n\n[dev-dependencies]\npng = \"0.17\"\n";

    for (label, text, expected) in [
        ("inline", inline, "resvg"),
        ("table", table, "image"),
        ("dev", dev, "png"),
    ] {
        let deps = declared_dependencies(text);
        assert!(
            deps.iter().any(|d| d == expected),
            "{label} form: detector missed '{expected}' (parsed {deps:?})"
        );
        assert!(
            deps.iter().any(|d| BANNED.contains(&d.as_str())),
            "{label} form: banned crate not recognised"
        );
    }

    // And a clean manifest must NOT trip the guard.
    let clean = "[package]\nname = \"x\"\n\n[dependencies]\nserde = \"1\"\npostcard = \"1\"\n";
    assert!(
        !declared_dependencies(clean)
            .iter()
            .any(|d| BANNED.contains(&d.as_str())),
        "a clean manifest must not be reported as a violation"
    );
}
