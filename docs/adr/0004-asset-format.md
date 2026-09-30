# ADR-0004: Canonical visual model — scene schema + compiled asset format

**Status**: Accepted · **Date**: 2026-07-19 · **Feature**: 001-device-connection-foundation

## Context

**2026-09-08 amendment — mascot animation (format v2):** The approved mascot work supersedes
decision 4's rejection of interpolation. The shared `MascotAnimator` uses Q8 integer transforms
and cubic smoothstep, retaining the current resolved pose when interrupted. SVG remains a build
input; no runtime vector parser, floating-point animation, heap allocation, or extra device
framebuffer is introduced. Generic static layers retain the original step-held keyframes.

Format v2 adds an optional alpha-pool reference to each bitmap (two 4-bit coverage samples per
byte, even pixel in the low nibble), plus `Static`, `Body`, `Eyes`, `Mouth` and (appended later,
keeping postcard indices stable) `Cap` layer roles. The keycap mascot draws `Body` (fixed base),
`Cap`, both `Eyes`, then `Mouth`; `Cap`, eyes and mouth also move by the pose's press offset.
All mascot scenes share matching facial layer geometry and order. The compositor mixes bounded
state weights before alpha composition, clips to each tile, and blinks around each eye's centre.
Readers reject v1 blobs; desktop and firmware rebuild their embedded blobs together. USB wire
messages and protocol versions are unchanged. See [mascot verification](../features/003-mascot-animation/evidence/mascot-animation.md).

Principle II ("one canonical visual model") requires that Device Studio and the firmware render from
the **same** scene definitions, assets, timing, colors, and renderer. Principle XI requires that SVG/PNG
are authoring inputs only and that the asset compiler emits the canonical runtime representation.
Principle IV requires `no_std`, no runtime `alloc`, and no full-frame framebuffer on the device. This
ADR fixes the scene schema and the compiled asset binary format. The high-level model was chosen
interactively by the maintainer (2026-07-19); this ADR records those choices and the supporting design.

## Decisions

**Steered choices (maintainer):**

1. **Layered composition with integer keyframe transforms.** A scene is a background color plus an
   ordered list of layers; each layer has a keyframe timeline. Motion is expressed as integer
   transforms and sprite-frame selection — not baked full frames — to conserve ESP32-C3 flash and fit
   the tile/partial-refresh renderer.
2. **`embedded-graphics` as the drawing stack.** The renderer draws through an `embedded-graphics`
   `DrawTarget` implemented on the tile framebuffer, using `embedded-graphics` for solid shapes and
   bitmap text and custom RGB565 blits for sprites. This aligns with `mipidsi` (Phase 8) and is
   integer/deterministic (Principle III). `embedded-graphics` is a `no_std` shared-crate dependency
   (permitted by the direct-dependency firewall — it is not a desktop/OS crate).
3. **Content types this slice: sprites + solid rects + bitmap text.**

**Design details (this ADR):**

4. **Animation is step-held (no interpolation).** A layer's transform at `elapsed_ms` is the last
   keyframe whose `at_ms <= elapsed_ms` (holding). Smooth motion is authored as denser keyframes or
   sprite-frame animation. This keeps rendering integer-only and byte-identical across host/device.
5. **Compiled blob layout** (`kivori-assets` reads it, produced by `tools/asset-compiler`):
   - A fixed header: magic, `format_version`, section offsets/lengths, and a content hash.
   - A **structured section** (scene/layer/keyframe records + bitmap/font/string tables) encoded with
     `postcard` and deserialized on load into **bounded `heapless`** structures (the scene graph is
     tiny — 6 scenes, a handful of layers/keyframes each — so materializing the active scene costs a
     few KB of stack/static RAM, no heap).
   - A **data pool**: raw RGB565 sprite pixels, glyph bitmaps, and string bytes, referenced by
     `(offset, len)` and read **zero-copy** as borrowed `&[u8]` (the large data never leaves flash on
     the device beyond the current tile).
6. **Bitmap fonts** are compiled into the blob (glyph atlas + per-glyph metrics), keeping text
   deterministic and independent of host fonts (Principle II/XI).
7. **Deterministic compilation**: pinned `resvg`/`tiny-skia`, fixed rounding, stable record ordering,
   no timestamps → byte-reproducible blob; the blob hash is committed and CI re-compiles and diffs it.

**Crate split:** `kivori-model` owns the leaf value types (`Keyframe`, `LayerKind`, asset ids,
`ResolvedTransform`, and the pure `resolve_transform`); `kivori-assets` owns the blob layout, the
zero-copy/`heapless` reader, and the scene/layer views the renderer consumes; `kivori-renderer` owns the
`DrawTarget` + layer compositor; `tools/asset-compiler` (host) owns SVG/font → blob.

## Alternatives considered

- **Baked composite frames** — rejected (flash-heavy; fights tile rendering). See steered choice 1.
- **Fully zero-copy scene graph** (no `postcard`/`heapless` for the structured section) — more code and
  fragile byte-layout accessors for negligible RAM savings on a tiny scene graph. Rejected in favor of
  the hybrid in decision 5.
- **Keyframe interpolation / sub-pixel** — rejected for determinism (decision 4).
- **Custom blitter instead of `embedded-graphics`** — rejected per steered choice 2.

## Consequences

- The renderer's Phase-4 `Scene` trait (a provisional `pixel()` seam) is superseded by the layered
  compositor over `kivori-assets` scene views; the code-defined test scene remains available for
  golden-frame determinism tests.
- `embedded-graphics`, `heapless` (already used), and a font representation become part of the shared
  rendering stack, all `no_std` and RISC-V-compiled in CI.
- A blob-hash golden guards asset determinism alongside the frame-hash goldens.
