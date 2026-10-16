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
