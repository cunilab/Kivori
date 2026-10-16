//! Immutable firmware image sets. Build validation and native lookup share this parser.
use crate::firmware_marker::{scan_firmware_versions, scan_image_identities, ImageIdentity};

/// A structurally validated image and the identity carried in its bytes.
#[derive(Debug)]
pub struct FirmwareImage<'a> {
    identity: ImageIdentity,
    bytes: &'a [u8],
}

impl<'a> FirmwareImage<'a> {
    /// Parses an ELF32 RISC-V image with exactly one matching version and identity record.
    ///
    /// # Errors
    /// Invalid ELF header, absent/ambiguous markers or differing version records.
    pub fn parse(bytes: &'a [u8]) -> Result<Self, &'static str> {
        if bytes.len() < 52 || !bytes.starts_with(b"\x7fELF\x01\x01") || bytes[18..20] != [243, 0] {
            return Err("firmware must be a little-endian ELF32 RISC-V image");
        }
        let mut identities = scan_image_identities(bytes);
        let identity = identities.next().ok_or("firmware has no identity record")?;
        if identities.next().is_some() {
            return Err("firmware has ambiguous identity records");
        }
        let mut versions = scan_firmware_versions(bytes);
        let version = versions.next().ok_or("firmware has no version marker")?;
        if versions.next().is_some() || version != identity.version {
            return Err("firmware has ambiguous or differing version records");
        }
        Ok(Self { identity, bytes })
    }

    /// The validated identity record.
    #[must_use]
    pub fn identity(&self) -> &ImageIdentity {
        &self.identity
    }

    /// The exact image bytes to program.
    #[must_use]
    pub fn bytes(&self) -> &'a [u8] {
        self.bytes
    }
}

/// Images keyed by exact product/board identity, sorted independently of input/catalog order.
#[derive(Debug, Default)]
pub struct FirmwareBundle<'a> {
    images: Vec<FirmwareImage<'a>>,
}

impl<'a> FirmwareBundle<'a> {
    /// Parses images and rejects duplicate product/board keys.
    ///
    /// # Errors
    /// An invalid image or a duplicate key, even when the bytes are identical.
    pub fn parse(bytes: &[&'a [u8]]) -> Result<Self, &'static str> {
        let mut images = bytes
            .iter()
            .map(|bytes| FirmwareImage::parse(bytes))
            .collect::<Result<Vec<_>, _>>()?;
        images.sort_by_key(|image| (image.identity.product, image.identity.board));
        if images.windows(2).any(|pair| {
            pair[0].identity.product == pair[1].identity.product
                && pair[0].identity.board == pair[1].identity.board
        }) {
            return Err("duplicate firmware product/board pair");
        }
        Ok(Self { images })
    }

    /// All images in product/board order.
    #[must_use]
    pub fn images(&self) -> &[FirmwareImage<'a>] {
        &self.images
    }

    /// Exact lookup only; unknown or absent keys have no fallback.
    #[must_use]
    pub fn get(&self, product: u16, board: u16) -> Option<&FirmwareImage<'a>> {
        self.images
            .iter()
            .find(|image| image.identity.product == product && image.identity.board == board)
    }

    /// Compatibility for explicitly single-image workflows; never picks the first of several.
    #[must_use]
    pub fn single(&self) -> Option<&FirmwareImage<'a>> {
        match self.images.as_slice() {
            [only] => Some(only),
            _ => None,
        }
    }
}
