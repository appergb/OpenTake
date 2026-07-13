use super::*;

#[test]
fn component_accepts_safe_names_and_rejects_too_long_and_unsafe_names() {
    let safe = ComponentName::new("asset.mov").expect("safe component accepted");
    assert_eq!(safe.as_os_str(), std::ffi::OsStr::new("asset.mov"));
    let too_long = "x".repeat(32_768);
    assert!(matches!(
        ComponentName::new(&too_long),
        Err(SafeFsError::InvalidComponent(ComponentViolation::TooLong))
    ));
    assert!(matches!(
        ComponentName::new("."),
        Err(SafeFsError::InvalidComponent(
            ComponentViolation::CurrentDirectory
        ))
    ));
    assert!(matches!(
        ComponentName::new(".."),
        Err(SafeFsError::InvalidComponent(
            ComponentViolation::ParentDirectory
        ))
    ));
    assert!(matches!(
        ComponentName::new("a/b"),
        Err(SafeFsError::InvalidComponent(
            ComponentViolation::MultipleComponents | ComponentViolation::WindowsSeparator
        ))
    ));
}
