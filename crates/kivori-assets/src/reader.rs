//! Zero-copy runtime reader for the compiled asset blob (ADR-0004, FR-021).
//!
//! Blob layout (little-endian):
//! ```text
//! [ magic:u32 | format_version:u16 | reserved:u16 | manifest_len:u32 | pool_len:u32 ]  (16-byte header)
//! [ manifest: manifest_len bytes ]   postcard-encoded `Manifest`
//! [ pool: pool_len bytes ]           raw RGB565 pixels + UTF-8 strings, referenced by PoolRef
//! ```
//! The manifest is deserialized into bounded `heapless` collections (no heap); pixel/string bytes are
//! borrowed from the pool zero-copy. Source SVG/PNG is never parsed at runtime.

use crate::manifest::{BitmapEntry, Manifest, SceneDef};
use kivori_model::{AssetId, CompanionState, DeviceProfile, StringId};

/// Blob magic (`"KASS"`, little-endian).
pub const MAGIC: u32 = 0x5353_414B;
/// Supported blob format version.
pub const FORMAT_VERSION: u16 = 2;
/// Fixed header length in bytes.
pub const HEADER_LEN: usize = 16;

/// An error parsing or reading a compiled asset blob. Never panics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssetError {
    /// The blob is shorter than the header (or a declared section).
    TooShort,
    /// The blob magic did not match.
    BadMagic,
    /// The blob's format version is unsupported.
    UnsupportedVersion,
    /// The header's section lengths do not match the blob length.
    LengthMismatch,
    /// The manifest failed to `postcard`-decode (e.g., exceeded a bounded capacity).
    Postcard,
}

/// A parsed compiled asset blob: an owned (bounded) [`Manifest`] plus a borrowed data pool.
#[derive(Debug, Clone)]
pub struct AssetBlob<'a> {
    manifest: Manifest,
    pool: &'a [u8],
}

impl<'a> AssetBlob<'a> {
    /// Parses and validates a blob, deserializing the manifest and borrowing the data pool.
    ///
    /// # Errors
    /// A typed [`AssetError`] on any malformed blob; never panics.
    pub fn parse(bytes: &'a [u8]) -> Result<Self, AssetError> {
        if bytes.len() < HEADER_LEN {
            return Err(AssetError::TooShort);
        }
        if u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) != MAGIC {
            return Err(AssetError::BadMagic);
        }
        if u16::from_le_bytes([bytes[4], bytes[5]]) != FORMAT_VERSION {
            return Err(AssetError::UnsupportedVersion);
        }
        let manifest_len = u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]) as usize;
        let pool_len = u32::from_le_bytes([bytes[12], bytes[13], bytes[14], bytes[15]]) as usize;
        let total = HEADER_LEN
            .checked_add(manifest_len)
            .and_then(|n| n.checked_add(pool_len));
        if total != Some(bytes.len()) {
            return Err(AssetError::LengthMismatch);
        }
        let manifest_bytes = &bytes[HEADER_LEN..HEADER_LEN + manifest_len];
        let pool = &bytes[HEADER_LEN + manifest_len..];
        // Matched rather than `map_err(..)?`: the manifest is tens of KiB, and every extra move of
        // it is another full copy in this stack frame.
        match postcard::from_bytes::<Manifest>(manifest_bytes) {
            Ok(manifest) => Ok(AssetBlob { manifest, pool }),
            Err(_) => Err(AssetError::Postcard),
        }
    }

    /// The device profile this blob was compiled for.
    #[must_use]
    pub fn profile(&self) -> DeviceProfile {
        self.manifest.profile
    }

    /// The parsed manifest.
    #[must_use]
    pub fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    /// The scene for `state`, if present.
    #[must_use]
    pub fn scene(&self, state: CompanionState) -> Option<&SceneDef> {
        self.manifest.scenes.iter().find(|s| s.id == state)
    }

    /// The bitmap table entry for `id`, if present.
    #[must_use]
    pub fn bitmap(&self, id: AssetId) -> Option<&BitmapEntry> {
        self.manifest.bitmaps.get(id as usize)
    }

    /// The RGB565 pixel bytes for a bitmap entry, borrowed zero-copy from the pool.
    #[must_use]
    pub fn bitmap_pixels(&self, entry: &BitmapEntry) -> Option<&[u8]> {
        self.pool_slice(entry.data.offset, entry.data.len)
    }

    /// Packed alpha4 bytes for `entry`, or `None` when it is explicitly opaque or malformed.
    #[must_use]
    pub fn bitmap_alpha(&self, entry: &BitmapEntry) -> Option<&[u8]> {
        let alpha = entry.alpha?;
        self.pool_slice(alpha.offset, alpha.len)
    }

    /// The UTF-8 string for `id`, borrowed zero-copy from the pool.
    #[must_use]
    pub fn string(&self, id: StringId) -> Option<&str> {
        let r = self.manifest.strings.get(id as usize)?;
        let bytes = self.pool_slice(r.offset, r.len)?;
        core::str::from_utf8(bytes).ok()
    }

    fn pool_slice(&self, offset: u32, len: u32) -> Option<&[u8]> {
        let start = offset as usize;
        let end = start.checked_add(len as usize)?;
        self.pool.get(start..end)
    }
}
