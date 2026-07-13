use super::error::RelativePathViolation;
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
    for unsafe_name in ["asset.mov/", "asset.mov//", "asset.mov/."] {
        assert!(matches!(
            ComponentName::new(unsafe_name),
            Err(SafeFsError::InvalidComponent(
                ComponentViolation::MultipleComponents | ComponentViolation::WindowsSeparator
            ))
        ));
    }
    for unsafe_path in ["asset.mov/", "asset.mov//"] {
        assert!(matches!(
            RelativeComponents::new(std::path::Path::new(unsafe_path)),
            Err(SafeFsError::InvalidRelativePath(
                RelativePathViolation::InvalidComponent(ComponentViolation::Empty)
            ))
        ));
    }
    assert!(matches!(
        RelativeComponents::new(std::path::Path::new("asset.mov/.")),
        Err(SafeFsError::InvalidRelativePath(
            RelativePathViolation::CurrentDirectory
        ))
    ));
    assert!(matches!(
        RelativeComponents::new(std::path::Path::new("a/./b")),
        Err(SafeFsError::InvalidRelativePath(
            RelativePathViolation::CurrentDirectory
        ))
    ));

    #[cfg(windows)]
    {
        for unsafe_name in ["asset.mov\\", "asset.mov\\\\", "asset.mov\\."] {
            assert!(matches!(
                ComponentName::new(unsafe_name),
                Err(SafeFsError::InvalidComponent(
                    ComponentViolation::WindowsSeparator
                ))
            ));
        }
        for unsafe_path in ["asset.mov\\", "asset.mov\\\\"] {
            assert!(matches!(
                RelativeComponents::new(std::path::Path::new(unsafe_path)),
                Err(SafeFsError::InvalidRelativePath(
                    RelativePathViolation::InvalidComponent(ComponentViolation::Empty)
                ))
            ));
        }
        assert!(matches!(
            RelativeComponents::new(std::path::Path::new("a\\.\\b")),
            Err(SafeFsError::InvalidRelativePath(
                RelativePathViolation::CurrentDirectory
            ))
        ));
        assert!(matches!(
            RelativeComponents::new(std::path::Path::new("C:\\asset.mov")),
            Err(SafeFsError::InvalidRelativePath(
                RelativePathViolation::AbsoluteOrPrefix
            ))
        ));
        assert!(matches!(
            ComponentName::new("C:asset.mov"),
            Err(SafeFsError::InvalidComponent(
                ComponentViolation::AbsoluteOrPrefix
            ))
        ));
        for prefix in ["COM", "LPT"] {
            for digit in ['1', '9', '¹', '²', '³'] {
                for extension in ["", ".txt"] {
                    let name = format!("{prefix}{digit}{extension}");
                    assert!(matches!(
                        ComponentName::new(&name),
                        Err(SafeFsError::InvalidComponent(
                            ComponentViolation::WindowsDeviceName
                        ))
                    ));
                }
            }
        }
    }
}
