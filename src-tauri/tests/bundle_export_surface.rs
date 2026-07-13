const LIB_RS: &str = include_str!("../src/lib.rs");
const EXPORT_RS: &str = include_str!("../src/export.rs");

fn identifiers(source: &str) -> impl Iterator<Item = &str> {
    source
        .split(|character: char| !(character.is_ascii_alphanumeric() || character == '_'))
        .filter(|token| !token.is_empty())
}

#[test]
fn bundle_export_is_not_registered_or_exposed_as_a_tauri_command() {
    assert!(!identifiers(LIB_RS).any(|token| token == "export_bundle"));
    assert!(!identifiers(EXPORT_RS).any(|token| token == "export_bundle"));
}
