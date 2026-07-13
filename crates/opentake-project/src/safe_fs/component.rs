use super::error::{ComponentViolation, RelativePathViolation, Result, SafeFsError};
use std::ffi::{OsStr, OsString};
use std::path::Path;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct ComponentName(OsString);

impl ComponentName {
    pub(crate) fn new(value: impl AsRef<OsStr>) -> Result<Self> {
        let value = value.as_ref();
        validate_component_syntax(value)?;
        validate_os_component(value)?;
        Ok(Self(value.to_os_string()))
    }

    pub(crate) fn as_os_str(&self) -> &OsStr {
        &self.0
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RelativeComponents(Vec<ComponentName>);

impl RelativeComponents {
    pub(crate) fn new(path: &Path) -> Result<Self> {
        Ok(Self(parse_relative_components(path)?))
    }

    pub(crate) fn iter(&self) -> impl ExactSizeIterator<Item = &ComponentName> {
        self.0.iter()
    }
}

fn relative_component_error(error: SafeFsError) -> SafeFsError {
    match error {
        SafeFsError::InvalidComponent(reason) => {
            SafeFsError::InvalidRelativePath(RelativePathViolation::InvalidComponent(reason))
        }
        other => other,
    }
}

#[cfg(unix)]
fn validate_component_syntax(value: &OsStr) -> Result<()> {
    use std::os::unix::ffi::OsStrExt;
    let bytes = value.as_bytes();
    if bytes.is_empty() {
        return Err(SafeFsError::InvalidComponent(ComponentViolation::Empty));
    }
    if bytes == b"." {
        return Err(SafeFsError::InvalidComponent(
            ComponentViolation::CurrentDirectory,
        ));
    }
    if bytes == b".." {
        return Err(SafeFsError::InvalidComponent(
            ComponentViolation::ParentDirectory,
        ));
    }
    if bytes.first() == Some(&b'/') {
        return Err(SafeFsError::InvalidComponent(
            ComponentViolation::AbsoluteOrPrefix,
        ));
    }
    if bytes.contains(&b'/') {
        return Err(SafeFsError::InvalidComponent(
            ComponentViolation::MultipleComponents,
        ));
    }
    Ok(())
}

#[cfg(unix)]
fn parse_relative_components(path: &Path) -> Result<Vec<ComponentName>> {
    use std::os::unix::ffi::OsStrExt;
    let bytes = path.as_os_str().as_bytes();
    if bytes.is_empty() {
        return Err(SafeFsError::InvalidRelativePath(
            RelativePathViolation::Empty,
        ));
    }
    if bytes.first() == Some(&b'/') {
        return Err(SafeFsError::InvalidRelativePath(
            RelativePathViolation::AbsoluteOrPrefix,
        ));
    }
    bytes
        .split(|byte| *byte == b'/')
        .map(|part| {
            if part.is_empty() {
                return Err(SafeFsError::InvalidRelativePath(
                    RelativePathViolation::InvalidComponent(ComponentViolation::Empty),
                ));
            }
            if part == b"." {
                return Err(SafeFsError::InvalidRelativePath(
                    RelativePathViolation::CurrentDirectory,
                ));
            }
            if part == b".." {
                return Err(SafeFsError::InvalidRelativePath(
                    RelativePathViolation::ParentDirectory,
                ));
            }
            ComponentName::new(OsStr::from_bytes(part)).map_err(relative_component_error)
        })
        .collect()
}

#[cfg(unix)]
fn validate_os_component(value: &OsStr) -> Result<()> {
    use std::os::unix::ffi::OsStrExt;
    if value.as_bytes().contains(&0) {
        return Err(SafeFsError::InvalidComponent(
            ComponentViolation::EmbeddedNul,
        ));
    }
    if value.as_bytes().len() > 255 {
        return Err(SafeFsError::InvalidComponent(ComponentViolation::TooLong));
    }
    Ok(())
}

#[cfg(windows)]
fn is_windows_separator(unit: u16) -> bool {
    unit == b'/' as u16 || unit == b'\\' as u16
}

#[cfg(windows)]
fn is_windows_drive_prefix(units: &[u16]) -> bool {
    units.len() >= 2
        && ((b'A' as u16..=b'Z' as u16).contains(&units[0])
            || (b'a' as u16..=b'z' as u16).contains(&units[0]))
        && units[1] == b':' as u16
}

#[cfg(windows)]
fn validate_component_syntax(value: &OsStr) -> Result<()> {
    use std::os::windows::ffi::OsStrExt;
    let units: Vec<u16> = value.encode_wide().collect();
    if units.is_empty() {
        return Err(SafeFsError::InvalidComponent(ComponentViolation::Empty));
    }
    if units == [b'.' as u16] {
        return Err(SafeFsError::InvalidComponent(
            ComponentViolation::CurrentDirectory,
        ));
    }
    if units == [b'.' as u16, b'.' as u16] {
        return Err(SafeFsError::InvalidComponent(
            ComponentViolation::ParentDirectory,
        ));
    }
    if units
        .first()
        .is_some_and(|unit| is_windows_separator(*unit))
        || is_windows_drive_prefix(&units)
    {
        return Err(SafeFsError::InvalidComponent(
            ComponentViolation::AbsoluteOrPrefix,
        ));
    }
    if units.iter().any(|unit| is_windows_separator(*unit)) {
        return Err(SafeFsError::InvalidComponent(
            ComponentViolation::WindowsSeparator,
        ));
    }
    Ok(())
}

#[cfg(windows)]
fn parse_relative_components(path: &Path) -> Result<Vec<ComponentName>> {
    use std::os::windows::ffi::{OsStrExt, OsStringExt};
    let units: Vec<u16> = path.as_os_str().encode_wide().collect();
    if units.is_empty() {
        return Err(SafeFsError::InvalidRelativePath(
            RelativePathViolation::Empty,
        ));
    }
    if units
        .first()
        .is_some_and(|unit| is_windows_separator(*unit))
        || is_windows_drive_prefix(&units)
    {
        return Err(SafeFsError::InvalidRelativePath(
            RelativePathViolation::AbsoluteOrPrefix,
        ));
    }
    units
        .split(|unit| is_windows_separator(*unit))
        .map(|part| {
            if part.is_empty() {
                return Err(SafeFsError::InvalidRelativePath(
                    RelativePathViolation::InvalidComponent(ComponentViolation::Empty),
                ));
            }
            if part == [b'.' as u16] {
                return Err(SafeFsError::InvalidRelativePath(
                    RelativePathViolation::CurrentDirectory,
                ));
            }
            if part == [b'.' as u16, b'.' as u16] {
                return Err(SafeFsError::InvalidRelativePath(
                    RelativePathViolation::ParentDirectory,
                ));
            }
            ComponentName::new(OsString::from_wide(part)).map_err(relative_component_error)
        })
        .collect()
}

#[cfg(windows)]
fn validate_os_component(value: &OsStr) -> Result<()> {
    use std::os::windows::ffi::OsStrExt;
    let units: Vec<u16> = value.encode_wide().collect();
    if units.is_empty() {
        return Err(SafeFsError::InvalidComponent(ComponentViolation::Empty));
    }
    if units
        .len()
        .checked_mul(2)
        .and_then(|n| u16::try_from(n).ok())
        .is_none()
    {
        return Err(SafeFsError::InvalidComponent(ComponentViolation::TooLong));
    }
    if units.contains(&0) {
        return Err(SafeFsError::InvalidComponent(
            ComponentViolation::EmbeddedNul,
        ));
    }
    if units
        .iter()
        .any(|unit| *unit == b'/' as u16 || *unit == b'\\' as u16)
    {
        return Err(SafeFsError::InvalidComponent(
            ComponentViolation::WindowsSeparator,
        ));
    }
    if units.contains(&(b':' as u16)) {
        return Err(SafeFsError::InvalidComponent(
            ComponentViolation::WindowsAlternateDataStream,
        ));
    }
    if units
        .last()
        .is_some_and(|unit| *unit == b'.' as u16 || *unit == b' ' as u16)
    {
        return Err(SafeFsError::InvalidComponent(
            ComponentViolation::WindowsTrailingDotOrSpace,
        ));
    }
    let stem: Vec<u16> = units
        .iter()
        .copied()
        .take_while(|unit| *unit != b'.' as u16)
        .map(|unit| {
            if (b'a' as u16..=b'z' as u16).contains(&unit) {
                unit - 32
            } else {
                unit
            }
        })
        .collect();
    let reserved: &[&[u16]] = &[&[67, 79, 78], &[80, 82, 78], &[65, 85, 88], &[78, 85, 76]];
    let device_digit = |unit: u16| {
        (b'1' as u16..=b'9' as u16).contains(&unit) || matches!(unit, 0x00b9 | 0x00b2 | 0x00b3)
    };
    let numbered = stem.len() == 4
        && (stem[..3] == [67, 79, 77] || stem[..3] == [76, 80, 84])
        && device_digit(stem[3]);
    if reserved.contains(&stem.as_slice()) || numbered {
        return Err(SafeFsError::InvalidComponent(
            ComponentViolation::WindowsDeviceName,
        ));
    }
    Ok(())
}

#[cfg(not(any(unix, windows)))]
fn validate_component_syntax(_: &OsStr) -> Result<()> {
    Err(SafeFsError::UnsupportedSecureFilesystem {
        operation: super::error::SafeFsOperation::QueryChild,
        reason: super::error::SecureFilesystemReason::UnsupportedTarget,
    })
}

#[cfg(not(any(unix, windows)))]
fn parse_relative_components(_: &Path) -> Result<Vec<ComponentName>> {
    Err(SafeFsError::UnsupportedSecureFilesystem {
        operation: super::error::SafeFsOperation::QueryChild,
        reason: super::error::SecureFilesystemReason::UnsupportedTarget,
    })
}

#[cfg(not(any(unix, windows)))]
fn validate_os_component(_: &OsStr) -> Result<()> {
    Err(SafeFsError::UnsupportedSecureFilesystem {
        operation: super::error::SafeFsOperation::QueryChild,
        reason: super::error::SecureFilesystemReason::UnsupportedTarget,
    })
}
