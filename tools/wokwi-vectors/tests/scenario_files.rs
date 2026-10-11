//! Structural gate over the on-disk Wokwi scenarios.
//!
//! `wokwi-cli` refuses to validate a scenario without a CI token, so scenario mistakes would otherwise
//! only surface in an authenticated run. These tests catch the two classes that actually bite:
//!
//! 1. **An unsupported step kind.** `wokwi-cli` 0.26 supports `wait-serial`, `write-serial`, `delay`,
//!    `set-control`, and `expect-pin`. A `wait-pin`, `screenshot`, or `wait-text` step is rejected at
//!    run time — and only then, after a token is spent.
//! 2. **Marker drift.** A scenario that waits on a marker the firmware never emits hangs until timeout.
//!    That is exactly the T135 defect, so the SPI probe's markers are checked against the firmware source.
//!
//! Both checks carry a negative control, so neither can pass vacuously.

use std::fs;
use std::path::{Path, PathBuf};

/// Step keys `wokwi-cli` 0.26.1 accepts.
const SUPPORTED_STEPS: [&str; 5] = [
    "wait-serial",
    "write-serial",
    "delay",
    "set-control",
    "expect-pin",
];

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("repo root is two levels above tools/wokwi-vectors")
        .to_path_buf()
}

/// Every scenario YAML the gate runs, as `(relative path, contents)`.
fn scenarios() -> Vec<(String, String)> {
    let root = repo_root();
    let mut out = Vec::new();
    for dir in ["sim/wokwi/scenarios", "sim/wokwi/generated"] {
        let path = root.join(dir);
        let entries =
            fs::read_dir(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
        for entry in entries {
            let entry = entry.expect("dir entry");
            let file = entry.path();
            if file.extension().and_then(|e| e.to_str()) != Some("yaml") {
                continue;
            }
            // `vectors.yaml` is the wire-vector catalogue, not a scenario — it has no `steps:`.
            if file.file_name().is_some_and(|n| n == "vectors.yaml") {
                continue;
            }
            let name = format!("{dir}/{}", file.file_name().unwrap().to_string_lossy());
            out.push((name, fs::read_to_string(&file).expect("read scenario")));
        }
    }
    out.sort();
    assert!(
        out.len() >= 7,
        "expected at least the seven gated scenarios, found {}",
        out.len()
    );
    out
}

/// Step keys used by a scenario, in order (`- <key>: …`).
fn step_keys(yaml: &str) -> Vec<String> {
    yaml.lines()
        .filter_map(|line| {
            let trimmed = line.trim_start();
            let rest = trimmed.strip_prefix("- ")?;
            let key = rest.split(':').next()?.trim();
            (!key.is_empty()).then(|| key.to_string())
        })
        .collect()
}

#[test]
fn every_scenario_declares_the_required_top_level_keys() {
    for (name, yaml) in scenarios() {
        for key in ["name:", "version:", "author:", "steps:"] {
            assert!(
                yaml.lines().any(|l| l.starts_with(key)),
                "{name} is missing the top-level `{key}`"
            );
        }
        assert!(
            yaml.lines().any(|l| l.trim() == "version: 1"),
            "{name} must declare `version: 1`"
        );
    }
}

#[test]
fn every_scenario_uses_only_supported_step_kinds() {
    for (name, yaml) in scenarios() {
        let keys = step_keys(&yaml);
        assert!(!keys.is_empty(), "{name} declares no steps");
        for key in keys {
            assert!(
                SUPPORTED_STEPS.contains(&key.as_str()),
                "{name} uses step `{key}`, which wokwi-cli 0.26 does not support \
                 (supported: {SUPPORTED_STEPS:?})"
            );
        }
    }
}

#[test]
fn the_unsupported_step_detector_is_not_vacuous() {
    // Negative control: the three step kinds that look plausible but are rejected at run time.
    let bogus = "\
name: bogus
version: 1
author: t
steps:
  - wait-pin: { part: esp, pin: '4', value: high }
  - screenshot: {}
  - wait-text: hello
";
    let keys = step_keys(bogus);
    assert_eq!(keys, vec!["wait-pin", "screenshot", "wait-text"]);
    assert!(
        keys.iter().all(|k| !SUPPORTED_STEPS.contains(&k.as_str())),
        "the detector must reject every unsupported step kind"
    );
}

/// `wait-serial` payloads a scenario asserts on.
fn awaited_markers(yaml: &str) -> Vec<String> {
    yaml.lines()
        .filter_map(|line| {
            line.trim()
                .strip_prefix("- wait-serial: ")
                .map(|rest| rest.trim().trim_matches('\'').to_string())
        })
        .collect()
}

#[test]
fn the_spi_probe_scenario_matches_the_firmware_markers() {
    let root = repo_root();
    let yaml = fs::read_to_string(root.join("sim/wokwi/scenarios/spi-display-probe.yaml"))
        .expect("spi-display-probe.yaml");
    let source =
        fs::read_to_string(root.join("firmware/desk-buddy/src/spi_probe.rs")).expect("spi_probe.rs");

    let markers = awaited_markers(&yaml);
    assert!(
        markers.len() >= 11,
        "the probe scenario must assert every stage, found {}",
        markers.len()
    );

    // Each `PASS <stage>` marker must correspond to a stage name the firmware actually emits.
    let mut stages = 0;
    for marker in &markers {
        if let Some(stage) = marker.strip_prefix("KIVORI-SPI PASS ") {
            assert!(
                source.contains(&format!("\"{stage}\"")),
                "scenario waits on stage `{stage}`, which firmware/desk-buddy/src/spi_probe.rs never emits"
            );
            stages += 1;
        }
    }
    assert_eq!(stages, 10, "all ten probe stages must be asserted");

    // Required stages, in dependency order.
    let expected = [
        "bus-init",
        "reset-sequence",
        "rgb-red",
        "rgb-green",
        "rgb-blue",
        "checkerboard",
        "tile-count-36",
        "tile-bytes",
        "unchanged-no-reflush",
        "changed-reflush",
    ];
    let asserted: Vec<&str> = markers
        .iter()
        .filter_map(|m| m.strip_prefix("KIVORI-SPI PASS "))
        .collect();
    assert_eq!(asserted, expected, "stage order must follow dependencies");

    // The completion marker must be last, so simulator exit alone cannot look like success.
    assert_eq!(
        markers.last().map(String::as_str),
        Some("KIVORI-SPI ALL PASS")
    );
    // And readiness must be awaited before anything else.
    assert!(
        markers[0].starts_with("KIVORI-SPI BOOT"),
        "the probe must wait for readiness first, got `{}`",
        markers[0]
    );
}

#[test]
fn the_marker_coupling_detector_is_not_vacuous() {
    // Negative control: a fabricated stage name must NOT be found in the firmware source.
    let source = fs::read_to_string(repo_root().join("firmware/desk-buddy/src/spi_probe.rs"))
        .expect("spi_probe.rs");
    assert!(
        !source.contains("\"stage-that-does-not-exist\""),
        "the coupling check would pass for any string, so it proves nothing"
    );
}

#[test]
fn the_spi_probe_scenario_documents_its_scope_limits() {
    // The scope boundary is part of the artifact: a future reader must not be able to mistake this
    // scenario for controller validation.
    let yaml = fs::read_to_string(repo_root().join("sim/wokwi/scenarios/spi-display-probe.yaml"))
        .expect("spi-display-probe.yaml");
    for required in ["NOT IN SCOPE", "unconfirmed", "ILI9341", "ST7789", "GC9A01"] {
        assert!(
            yaml.contains(required),
            "the probe scenario must state its limits (missing `{required}`)"
        );
    }
}

#[test]
fn the_production_runtime_scenario_matches_the_firmware_markers() {
    // Same marker-drift guard as the SPI probe, for the production-runtime mode: a scenario waiting on a
    // marker the firmware never emits hangs until timeout (the T135 defect class).
    let root = repo_root();
    let yaml = fs::read_to_string(root.join("sim/wokwi/generated/production-runtime.yaml"))
        .expect("production-runtime.yaml");
    let source = fs::read_to_string(root.join("firmware/desk-buddy/src/wokwi_runtime.rs"))
        .expect("wokwi_runtime.rs");

    let markers = awaited_markers(&yaml);
    let stages: Vec<&str> = markers
        .iter()
        .filter_map(|m| m.strip_prefix("KIVORI-RUN PASS "))
        .collect();
    assert_eq!(
        stages,
        vec![
            "lifecycle-booting-to-offline",
            "first-frame-36-tiles",
            "health-report",
            "unchanged-frame-no-reflush",
            "hello-ack",
            "state-idle",
            "state-happy",
            "diagnostic label=frame-rejected-framing",
            "pong",
            "recovery-after-reject",
        ],
        "stage order must follow the runtime's dependencies"
    );
    // Marker text lives in the runtime mode; diagnostic LABELS live in the allowlist in health.rs. Both
    // must genuinely contain what the scenario waits for.
    let health =
        fs::read_to_string(root.join("firmware/desk-buddy/src/health.rs")).expect("health.rs");
    for stage in &stages {
        if let Some(label) = stage.strip_prefix("diagnostic label=") {
            assert!(
                health.contains(&format!("\"{label}\"")),
                "scenario waits on diagnostic label `{label}`, which the DeviceDiagnostic allowlist in \
                 health.rs never produces"
            );
        } else {
            assert!(
                source.contains(&format!("\"PASS {stage}\"")),
                "scenario waits on `{stage}`, which wokwi_runtime.rs never emits"
            );
        }
    }
    assert_eq!(
        markers.last().map(String::as_str),
        Some("KIVORI-RUN ALL PASS production-runtime"),
        "the completion marker must be the final assertion"
    );
}
