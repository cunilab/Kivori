//! Runtime-only embedding: the build script uses the shared parser without requiring OUT_DIR.

use crate::catalog::Catalog;

include!(concat!(env!("OUT_DIR"), "/catalog_sources.rs"));

impl Catalog {
    /// The catalog compiled into this build from active product directories.
    ///
    /// # Panics
    /// If generated metadata is invalid; the build script validates it before compilation.
    #[must_use]
    pub fn embedded() -> &'static Self {
        static CATALOG: std::sync::OnceLock<Catalog> = std::sync::OnceLock::new();
        CATALOG.get_or_init(|| {
            Catalog::parse(REGISTRY, PRODUCT_FILES).expect("embedded product catalog is valid")
        })
    }
}

/// Serves only compiled catalog artwork, never paths supplied by the webview.
pub(crate) fn product_art(product: &str) -> Option<String> {
    PRODUCT_ART
        .iter()
        .find(|(id, _)| *id == product)
        .map(|(_, bytes)| crate::platform::png::png_data_url(bytes))
}

#[cfg(test)]
mod tests {
    #[test]
    fn unbundled_keys_never_resolve_to_files() {
        assert!(super::product_art("../art.png").is_none());
        assert!(super::product_art("unknown-product").is_none());
    }
}
