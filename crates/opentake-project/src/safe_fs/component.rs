use super::error::{ComponentViolation, RelativePathViolation, Result, SafeFsError};
use std::ffi::{OsStr, OsString};
use std::path::{Component, Path};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct ComponentName(OsString);

impl ComponentName {
    pub(crate) fn new(value: impl AsRef<OsStr>) -> Result<Self> {
        let value = value.as_ref();
        let mut parts = Path::new(value).components();
        let normal = match parts.next() {
            None => return Err(SafeFsError::InvalidComponent(ComponentViolation::Empty)),
            Some(Component::CurDir) => {
                return Err(SafeFsError::InvalidComponent(
                    ComponentViolation::CurrentDirectory,
                ));
            }
            Some(Component::ParentDir) => {
                return Err(SafeFsError::InvalidComponent(
                    ComponentViolation::ParentDirectory,
                ));
            }
            Some(Component::RootDir | Component::Prefix(_)) => {
                return Err(SafeFsError::InvalidComponent(
                    ComponentViolation::AbsoluteOrPrefix,
                ));
            }
            Some(Component::Normal(normal)) => normal,
        };
        if parts.next().is_some() {
            return Err(SafeFsError::InvalidComponent(
                ComponentViolation::MultipleComponents,
            ));
        }
        validate_os_component(normal)?;
        Ok(Self(normal.to_os_string()))
    }

    pub(crate) fn as_os_str(&self) -> &OsStr {
        &self.0
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RelativeComponents(Vec<ComponentName>);

impl RelativeComponents {
    pub(crate) fn new(path: &Path) -> Result<Self> {
        let mut values = Vec::new();
        for part in path.components() {
            match part {
                Component::Normal(value) => {
                    values.push(ComponentName::new(value).map_err(|error| match error {
                        SafeFsError::InvalidComponent(reason) => SafeFsError::InvalidRelativePath(
                            RelativePathViolation::InvalidComponent(reason),
                        ),
                        other => other,
                    })?)
                }
                Component::CurDir => {
                    return Err(SafeFsError::InvalidRelativePath(
                        RelativePathViolation::CurrentDirectory,
                    ));
                }
                Component::ParentDir => {
                    return Err(SafeFsError::InvalidRelativePath(
                        RelativePathViolation::ParentDirectory,
                    ));
                }
                Component::RootDir | Component::Prefix(_) => {
                    return Err(SafeFsError::InvalidRelativePath(
                        RelativePathViolation::AbsoluteOrPrefix,
                    ));
                }
            }
        }
        if values.is_empty() {
            return Err(SafeFsError::InvalidRelativePath(
                RelativePathViolation::Empty,
            ));
        }
        Ok(Self(values))
    }

    pub(crate) fn iter(&self) -> impl ExactSizeIterator<Item = &ComponentName> {
        self.0.iter()
    }
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
    let numbered = stem.len() == 4
        && (stem[..3] == [67, 79, 77] || stem[..3] == [76, 80, 84])
        && (b'1' as u16..=b'9' as u16).contains(&stem[3]);
    if reserved.contains(&stem.as_slice()) || numbered {
        return Err(SafeFsError::InvalidComponent(
            ComponentViolation::WindowsDeviceName,
        ));
    }
    Ok(())
}

#[cfg(not(any(unix, windows)))]
fn validate_os_component(value: &OsStr) -> Result<()> {
    if value.is_empty() {
        Err(SafeFsError::InvalidComponent(ComponentViolation::Empty))
    } else {
        Ok(())
    }
}
