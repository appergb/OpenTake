//! Portable validation for paths stored inside `.opentake` bundles.

use std::path::{Component, Path};

/// Return whether `value` is a portable, non-empty bundle-relative path made
/// exclusively from ordinary components.
pub fn is_safe_project_asset_relative_path(value: &str) -> bool {
    // Bundle paths move between host platforms. Reject Windows separators,
    // drive prefixes and ADS syntax even when parsing on Unix.
    if value.is_empty() || value.contains(['\\', ':']) {
        return false;
    }
    let path = Path::new(value);
    !path.is_absolute()
        && path
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
        && path.components().next().is_some()
}
