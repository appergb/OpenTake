#![allow(dead_code, unused_imports)] // Private C1B substrate; remove each allowance when C1C/C1D consumes the facade.

mod capability;
mod component;
mod error;
mod ops;
#[cfg(any(target_os = "linux", target_os = "macos"))]
mod unix;
#[cfg(any(target_os = "linux", target_os = "macos"))]
use unix as platform;
#[cfg(windows)]
mod windows;
#[cfg(windows)]
use windows as platform;
#[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
mod unsupported;
#[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
use unsupported as platform;
#[cfg(test)]
mod test_seam;
#[cfg(test)]
mod tests;

pub(crate) use capability::{
    stream_copy_file, CaseMode, ChildState, CleanupAccess, CopyOutcome, CreatePermissions,
    DirectoryAccess, DirectoryAuthority, EntryKind, EntryMetadata, FileAccess, FileCapability,
    LocalFilesystemSnapshot, QuarantinedCapability, RawLinkTarget, StableIdentity, StageCapability,
};
pub(crate) use component::{ComponentName, RelativeComponents};
pub(crate) use error::{
    AtomicPublishReason, ComponentViolation, NativeBufferReason, RawOsError, SafeFsError,
    SafeFsOperation, SecureFilesystemReason, StageIdentityLostReason,
};
pub(crate) use ops::*;
