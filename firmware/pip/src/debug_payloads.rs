//! Raw-payload tracing, behind the `debug-payloads` **development** feature (T102; FR-032, ADR-0005 §3).
//!
//! Raw wire bytes are the one thing the logging policy forbids on a shipping device: they can contain a
//! host-supplied identity or state history. This module is the only place in the firmware that can turn
//! bytes into text, and the whole module — type, function, and format string — is compiled out unless
//! `debug-payloads` is enabled. The guard is therefore structural, not a runtime `if`.
//!
//! `scripts/check-release-surface.sh` proves both directions: the symbols are absent from a production
//! firmware library and present when the feature is on.

use esp_println::print;

/// Bytes printed per direction, so a large frame cannot flood the serial link.
const MAX_DUMP_BYTES: usize = 32;

/// Hex-dumps up to [`MAX_DUMP_BYTES`] of `bytes`, tagged with a direction (`"rx"` / `"tx"`).
///
/// Only reachable in a `debug-payloads` build. Never call this from a code path that is compiled into a
/// release artifact.
pub fn dump(direction: &str, bytes: &[u8]) {
    if bytes.is_empty() {
        return;
    }
    let shown = bytes.len().min(MAX_DUMP_BYTES);
    print!(
        "KIVORI-DEBUG-PAYLOAD {direction} len={} bytes=",
        bytes.len()
    );
    for byte in &bytes[..shown] {
        print!("{byte:02x}");
    }
    if shown < bytes.len() {
        print!("…");
    }
    print!("\r\n");
}
