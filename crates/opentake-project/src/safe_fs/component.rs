use super::error::{Result, SafeFsError, SafeFsOperation, SecureFilesystemReason};
use std::ffi::{OsStr, OsString};
use std::path::Path;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct ComponentName(OsString);
impl ComponentName {
    pub(crate) fn new(_: impl AsRef<OsStr>) -> Result<Self> {
        Err(SafeFsError::UnsupportedSecureFilesystem {
            operation: SafeFsOperation::QueryChild,
            reason: SecureFilesystemReason::UnsupportedTarget,
        })
    }
    pub(crate) fn as_os_str(&self) -> &OsStr {
        &self.0
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RelativeComponents(Vec<ComponentName>);
impl RelativeComponents {
    pub(crate) fn new(_: &Path) -> Result<Self> {
        Err(SafeFsError::UnsupportedSecureFilesystem {
            operation: SafeFsOperation::QueryChild,
            reason: SecureFilesystemReason::UnsupportedTarget,
        })
    }
    pub(crate) fn iter(&self) -> impl ExactSizeIterator<Item = &ComponentName> {
        self.0.iter()
    }
}
