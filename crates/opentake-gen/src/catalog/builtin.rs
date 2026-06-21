//! Built-in static catalog. Under BYOK, `list_models()` returns this catalog
//! compiled into the binary (no backend required), with the same structure as
//! the managed `/v1/models` response so UI/agent behave identically (axiom A5).
//! Entry ids use the `prefix:vendorModel` convention; pricing is omitted (BYOK
//! does not bill) but capability matrices are filled.

use super::entry::CatalogEntry;

/// The catalog JSON embedded at compile time.
const BUILTIN_CATALOG_JSON: &str = include_str!("builtin_catalog.json");

/// Parse and return the built-in catalog. Panics only on a malformed embedded
/// asset, which is a compile-time-shipped file and thus a programmer error.
pub fn builtin_catalog() -> Vec<CatalogEntry> {
    serde_json::from_str(BUILTIN_CATALOG_JSON).expect("builtin catalog must parse")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::entry::ModelKind;
    use std::collections::HashSet;

    #[test]
    fn builtin_catalog_parses() {
        let cat = builtin_catalog();
        assert!(!cat.is_empty());
    }

    #[test]
    fn all_ids_are_prefixed_and_unique() {
        let cat = builtin_catalog();
        let mut seen = HashSet::new();
        for e in &cat {
            assert!(
                e.id.contains(':'),
                "id {} must be prefix:vendorModel",
                e.id
            );
            assert!(seen.insert(e.id.clone()), "duplicate id {}", e.id);
        }
    }

    #[test]
    fn covers_all_four_kinds() {
        let cat = builtin_catalog();
        let kinds: HashSet<ModelKind> = cat.iter().map(|e| e.kind).collect();
        assert!(kinds.contains(&ModelKind::Image));
        assert!(kinds.contains(&ModelKind::Video));
        assert!(kinds.contains(&ModelKind::Audio));
        assert!(kinds.contains(&ModelKind::Upscale));
    }

    #[test]
    fn covers_all_four_providers() {
        let cat = builtin_catalog();
        let prefixes: HashSet<&str> = cat
            .iter()
            .filter_map(|e| e.id.split(':').next())
            .collect();
        for p in ["fal", "replicate", "openai", "elevenlabs"] {
            assert!(prefixes.contains(p), "missing provider {p}");
        }
    }
}
