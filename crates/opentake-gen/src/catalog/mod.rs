//! Model catalog: data-driven capability + pricing matrices shared by managed
//! and BYOK modes (axiom A5).

pub mod builtin;
pub mod cost;
pub mod entry;

pub use builtin::builtin_catalog;
pub use entry::{
    AudioCaps, AudioPricing, CatalogEntry, ImageCaps, ModelKind, ResponseShape, UiCapabilities,
    UpscaleCaps, VideoCaps,
};

/// A loaded catalog. Thin wrapper over `Vec<CatalogEntry>` with the same `?type`
/// filter the upstream agent `list_models` applies
/// (`ToolExecutor+Generate.swift:374-387`).
#[derive(Debug, Clone, Default)]
pub struct Catalog {
    entries: Vec<CatalogEntry>,
}

impl Catalog {
    pub fn new(entries: Vec<CatalogEntry>) -> Self {
        Self { entries }
    }

    /// The built-in static catalog used under BYOK.
    pub fn builtin() -> Self {
        Self::new(builtin_catalog())
    }

    pub fn entries(&self) -> &[CatalogEntry] {
        &self.entries
    }

    pub fn into_entries(self) -> Vec<CatalogEntry> {
        self.entries
    }

    /// Look up an entry by its full `prefix:vendorModel` id.
    pub fn by_id(&self, id: &str) -> Option<&CatalogEntry> {
        self.entries.iter().find(|e| e.id == id)
    }

    /// Entries of a single kind.
    pub fn of_kind(&self, kind: ModelKind) -> Vec<CatalogEntry> {
        self.entries
            .iter()
            .filter(|e| e.kind == kind)
            .cloned()
            .collect()
    }
}

impl From<Vec<CatalogEntry>> for Catalog {
    fn from(entries: Vec<CatalogEntry>) -> Self {
        Self::new(entries)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_wrapper_filters_by_kind() {
        let cat = Catalog::builtin();
        assert!(!cat.of_kind(ModelKind::Image).is_empty());
        assert!(!cat.of_kind(ModelKind::Video).is_empty());
        assert!(cat
            .of_kind(ModelKind::Image)
            .iter()
            .all(|e| e.kind == ModelKind::Image));
    }

    #[test]
    fn by_id_finds_known_entry() {
        let cat = Catalog::builtin();
        assert!(cat.by_id("fal:flux-pro").is_some());
        assert!(cat.by_id("nonexistent:model").is_none());
    }
}
