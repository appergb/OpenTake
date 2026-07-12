# C1B Common and Unix Normative Implementation Appendix

Status: normative, repository-versioned, attempt-4 implementation contract. It is bound to approved design `31bfd57e40e3a2bd0ca42b331e5aa877db2d6ace` and C1A baseline `e67917260ace36e4db1ede4e36eecbc401825bb1`. This appendix contains the complete common facade, unsupported adapter, Unix ownership model, deterministic tests, and executable RED/GREEN evidence protocol. There is no second Windows-only facade.

## 1. Invariants and threat-boundary contract

1. The only opaque platform seams are `NativeNamespaceAnchor`, `NativeDirectory`, and `NativeFile`. No raw descriptor or HANDLE crosses `safe_fs`.
2. `DirectoryAuthority` is recursive. Capture, child open, and child create return the same type. Its `NamespaceSnapshot` contains the global-root row, every absolute anchor component, and every subsequently opened child component. Each row records raw name, stable identity, filesystem snapshot, and case mode. Revalidation rewalks the entire chain from a fresh global root and compares every row.
3. `DirectoryAccess::{Read, MutateChildren, Stage}`, `FileAccess::{Read, ReadWrite}`, and `CleanupAccess::Delete` are explicit common-contract values. Platform adapters reject an operation if the retained native authority lacks its access.
4. `FileCapability` owns `platform::NativeFile`. Read, write, seek, flush, sync, and metadata all dispatch through `platform`; common code never assumes `std::fs::File`. On Windows these calls use the retained synchronous HANDLE and a fresh IOSB for every NT call.
5. `StageCapability`, `QuarantinedCapability`, and `CleanupCapability` are move-only. They retain parent authority, raw component name, opened identity, filesystem/case provenance, and access. Windows keeps the same DELETE-capable HANDLE from create/open through rename/delete. Unix keeps retained parent descriptors and the source name, performs quarantine no-replace, reopens the quarantine component nofollow, verifies identity, and otherwise restores no-replace or fail-leaks.
6. `query_child_nofollow` returns `Ok(ChildState::Present(metadata))` for symlinks, reparse points, FIFOs, sockets, and devices. Unix performs `statat(..., SYMLINK_NOFOLLOW)` only; it never opens a FIFO/device merely to answer a query.
7. Root quarantine and final publish are atomic no-replace name operations. Unix `renameat2(RENAME_NOREPLACE)` or `renameatx_np(RENAME_EXCL)` is the linearization point. There is no check-then-rename substitute.
8. Unix cannot bind `unlinkat` or the source side of rename atomically to an earlier inode handle. After verified root quarantine, recursive cleanup performs a fresh nofollow identity read immediately before each name syscall. That final same-account race is outside the approved threat boundary and is never described as handle-bound. Mismatch, ambiguity, or restore collision fails closed and retains the quarantine.
9. Recursive cleanup never joins an ambient path. It enumerates a retained quarantined directory, opens/records each child relative to that authority, recursively consumes child cleanup capabilities, then consumes the empty root capability.

## 2. Complete common source

The executor adds these files exactly. Compiler errors require a plan correction; they are not authority to invent another facade.

### `safe_fs/error.rs`

```rust
use super::capability::{EntryKind, StableIdentity};
use std::io;

pub(super) type Result<T> = std::result::Result<T, SafeFsError>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ComponentViolation {
    Empty,
    CurrentDirectory,
    ParentDirectory,
    AbsoluteOrPrefix,
    MultipleComponents,
    EmbeddedNul,
    TooLong,
    WindowsSeparator,
    WindowsAlternateDataStream,
    WindowsTrailingDotOrSpace,
    WindowsDeviceName,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RelativePathViolation {
    Empty,
    AbsoluteOrPrefix,
    CurrentDirectory,
    ParentDirectory,
    InvalidComponent(ComponentViolation),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SecureFilesystemReason {
    UnsupportedTarget,
    RemoteFilesystem,
    UnknownFilesystem,
    FilesystemProbeUnavailable,
    UnstableIdentity,
    UnstableMapping,
    CaseSemanticsUnavailable,
    VolumeChanged,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AtomicPublishReason {
    PrimitiveUnavailable,
    FilesystemRejected,
    RemoteFilesystem,
    CrossDeviceInvariant,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum StageIdentityLostReason {
    SourceChangedBeforeQuarantine,
    QuarantinedObjectChanged,
    OriginalNameOccupied,
    QuarantineRestoreFailed,
    QuarantineNameChanged,
    ParentAuthorityChanged,
    AmbiguousNameMutation,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum NativeBufferReason {
    LengthOverflow,
    DirectoryBufferTooSmall,
    DirectoryBufferMalformed,
    ReparseBufferMalformed,
    IoStatusInformationOutOfBounds,
    PendingOnSynchronousHandle,
    WriteZero,
    SecurityDescriptorMalformed,
    RenameLayoutMalformed,
    UnknownCaseFlags,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RawOsError {
    NtStatus { status: i32, dos_error: u32 },
    Win32(u32),
    Errno(i32),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SafeFsOperation {
    CaptureNamespaceRoot,
    OpenAncestor,
    ProbeFilesystem,
    QueryCaseMode,
    RevalidateNamespace,
    QueryChild,
    OpenDirectory,
    OpenFile,
    OpenCleanupEntry,
    QueryMetadata,
    EnumerateDirectory,
    ReadLink,
    CreateDirectory,
    CreateStageDirectory,
    CreateFile,
    ReadFile,
    WriteFile,
    SeekFile,
    FlushFile,
    SyncFile,
    CopyRead,
    CopyWrite,
    QuarantineNoReplace,
    VerifyQuarantine,
    RestoreQuarantine,
    DeleteQuarantinedEntry,
    DeleteQuarantinedEmptyDirectory,
    RenameNoReplaceSameParent,
    PublishNoReplace,
    ProbeVolume,
    QueryReparsePoint,
    VerifySecurityDescriptor,
    ParseDirectoryBuffer,
    ParseReparseBuffer,
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum SafeFsError {
    #[error("invalid path component: {0:?}")]
    InvalidComponent(ComponentViolation),
    #[error("invalid relative path: {0:?}")]
    InvalidRelativePath(RelativePathViolation),
    #[error("entry not found during {operation:?}")]
    NotFound { operation: SafeFsOperation },
    #[error("entry already exists during {operation:?}")]
    AlreadyExists { operation: SafeFsOperation },
    #[error("symlink or reparse point during {operation:?}")]
    SymlinkOrReparsePoint { operation: SafeFsOperation },
    #[error("unsupported entry type {kind:?} during {operation:?}")]
    UnsupportedEntryType { operation: SafeFsOperation, kind: EntryKind },
    #[error("identity changed during {operation:?}: expected {expected:?}, actual {actual:?}")]
    IdentityChanged { operation: SafeFsOperation, expected: StableIdentity, actual: StableIdentity },
    #[error("namespace changed during {operation:?}")]
    NamespaceChanged { operation: SafeFsOperation },
    #[error("stage identity lost during {operation:?}: {reason:?}")]
    StageIdentityLost { operation: SafeFsOperation, reason: StageIdentityLostReason },
    #[error("retained authority does not permit {operation:?}")]
    AccessMismatch { operation: SafeFsOperation },
    #[error("copy exceeded byte limit {limit}")]
    CopyLimitExceeded { limit: u64 },
    #[error("source ended before its retained size")]
    UnexpectedCopyEof,
    #[error("secure filesystem unavailable during {operation:?}: {reason:?}")]
    UnsupportedSecureFilesystem { operation: SafeFsOperation, reason: SecureFilesystemReason },
    #[error("atomic publish unavailable during {operation:?}: {reason:?}")]
    UnsupportedAtomicPublish { operation: SafeFsOperation, reason: AtomicPublishReason },
    #[error("filesystem I/O failed during {operation:?}: {source}")]
    Io { operation: SafeFsOperation, #[source] source: io::Error },
    #[error("native call failed during {operation:?}: {raw:?}")]
    Os { operation: SafeFsOperation, raw: RawOsError },
    #[error("invalid native buffer during {operation:?}: {reason:?}")]
    InvalidNativeBuffer { operation: SafeFsOperation, reason: NativeBufferReason },
}

impl SafeFsError {
    pub(super) fn io(operation: SafeFsOperation, source: impl Into<io::Error>) -> Self {
        Self::Io { operation, source: source.into() }
    }
}
```

### `safe_fs/component.rs`

```rust
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
            Some(Component::CurDir) => return Err(SafeFsError::InvalidComponent(ComponentViolation::CurrentDirectory)),
            Some(Component::ParentDir) => return Err(SafeFsError::InvalidComponent(ComponentViolation::ParentDirectory)),
            Some(Component::RootDir | Component::Prefix(_)) => return Err(SafeFsError::InvalidComponent(ComponentViolation::AbsoluteOrPrefix)),
            Some(Component::Normal(normal)) => normal,
        };
        if parts.next().is_some() {
            return Err(SafeFsError::InvalidComponent(ComponentViolation::MultipleComponents));
        }
        validate_os_component(normal)?;
        Ok(Self(normal.to_os_string()))
    }

    pub(crate) fn as_os_str(&self) -> &OsStr { &self.0 }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RelativeComponents(Vec<ComponentName>);

impl RelativeComponents {
    pub(crate) fn new(path: &Path) -> Result<Self> {
        let mut values = Vec::new();
        for part in path.components() {
            match part {
                Component::Normal(value) => values.push(ComponentName::new(value).map_err(|error| match error {
                    SafeFsError::InvalidComponent(reason) => SafeFsError::InvalidRelativePath(RelativePathViolation::InvalidComponent(reason)),
                    other => other,
                })?),
                Component::CurDir => return Err(SafeFsError::InvalidRelativePath(RelativePathViolation::CurrentDirectory)),
                Component::ParentDir => return Err(SafeFsError::InvalidRelativePath(RelativePathViolation::ParentDirectory)),
                Component::RootDir | Component::Prefix(_) => return Err(SafeFsError::InvalidRelativePath(RelativePathViolation::AbsoluteOrPrefix)),
            }
        }
        if values.is_empty() { return Err(SafeFsError::InvalidRelativePath(RelativePathViolation::Empty)); }
        Ok(Self(values))
    }

    pub(crate) fn iter(&self) -> impl ExactSizeIterator<Item = &ComponentName> { self.0.iter() }
}

#[cfg(unix)]
fn validate_os_component(value: &OsStr) -> Result<()> {
    use std::os::unix::ffi::OsStrExt;
    if value.as_bytes().contains(&0) { return Err(SafeFsError::InvalidComponent(ComponentViolation::EmbeddedNul)); }
    if value.as_bytes().len() > 255 { return Err(SafeFsError::InvalidComponent(ComponentViolation::TooLong)); }
    Ok(())
}

#[cfg(windows)]
fn validate_os_component(value: &OsStr) -> Result<()> {
    use std::os::windows::ffi::OsStrExt;
    let units: Vec<u16> = value.encode_wide().collect();
    if units.is_empty() { return Err(SafeFsError::InvalidComponent(ComponentViolation::Empty)); }
    if units.len().checked_mul(2).and_then(|n| u16::try_from(n).ok()).is_none() { return Err(SafeFsError::InvalidComponent(ComponentViolation::TooLong)); }
    if units.contains(&0) { return Err(SafeFsError::InvalidComponent(ComponentViolation::EmbeddedNul)); }
    if units.iter().any(|unit| *unit == b'/' as u16 || *unit == b'\\' as u16) { return Err(SafeFsError::InvalidComponent(ComponentViolation::WindowsSeparator)); }
    if units.contains(&(b':' as u16)) { return Err(SafeFsError::InvalidComponent(ComponentViolation::WindowsAlternateDataStream)); }
    if units.last().is_some_and(|unit| *unit == b'.' as u16 || *unit == b' ' as u16) { return Err(SafeFsError::InvalidComponent(ComponentViolation::WindowsTrailingDotOrSpace)); }
    let stem: Vec<u16> = units.iter().copied().take_while(|unit| *unit != b'.' as u16).map(|unit| if (b'a' as u16..=b'z' as u16).contains(&unit) { unit - 32 } else { unit }).collect();
    let reserved: &[&[u16]] = &[&[67,79,78], &[80,82,78], &[65,85,88], &[78,85,76]];
    let numbered = stem.len() == 4 && (&stem[..3] == [67,79,77] || &stem[..3] == [76,80,84]) && (b'1' as u16..=b'9' as u16).contains(&stem[3]);
    if reserved.contains(&stem.as_slice()) || numbered { return Err(SafeFsError::InvalidComponent(ComponentViolation::WindowsDeviceName)); }
    Ok(())
}

#[cfg(not(any(unix, windows)))]
fn validate_os_component(value: &OsStr) -> Result<()> {
    if value.is_empty() { Err(SafeFsError::InvalidComponent(ComponentViolation::Empty)) } else { Ok(()) }
}
```

### `safe_fs/capability.rs`

```rust
use super::component::ComponentName;
use super::error::{Result, SafeFsError, SafeFsOperation};
use super::platform;
use std::fmt;
use std::io::SeekFrom;
use std::sync::Arc;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum StableIdentity { Unix { device: u64, inode: u64 }, Windows { volume_serial: u64, file_id: [u8; 16] } }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum EntryKind { RegularFile, Directory, SymlinkOrReparse, Fifo, Socket, BlockDevice, CharacterDevice, Other }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DirectoryAccess { Read, MutateChildren, Stage }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FileAccess { Read, ReadWrite }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CleanupAccess { Delete }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CreatePermissions { Inherit, OwnerOnly }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CaseMode { Sensitive, Insensitive }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LinuxFilesystem { Ext, Xfs, Btrfs, Tmpfs }

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum LocalFilesystemSnapshot {
    Linux { family: LinuxFilesystem, fsid: u64, device: u64 },
    MacOs { type_name: [u8; 16], fsid: u64, device: u64 },
    Windows { volume_guid: Vec<u16>, serial: u64 },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct EntryMetadata {
    pub(crate) identity: StableIdentity,
    pub(crate) kind: EntryKind,
    pub(crate) len: u64,
    pub(crate) link_count: u64,
    pub(crate) filesystem: Option<LocalFilesystemSnapshot>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct NamespaceComponent {
    pub(crate) name: ComponentName,
    pub(crate) identity: StableIdentity,
    pub(crate) filesystem: LocalFilesystemSnapshot,
    pub(crate) case_mode: CaseMode,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct NamespaceSnapshot {
    pub(crate) root_identity: StableIdentity,
    pub(crate) root_filesystem: LocalFilesystemSnapshot,
    pub(crate) root_case_mode: CaseMode,
    pub(crate) components: Vec<NamespaceComponent>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ChildState { Absent, Present(EntryMetadata) }
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum RawLinkTarget { Unix(Vec<u8>), Windows { tag: u32, bytes: Vec<u8> } }

pub(super) struct NamespaceAnchor { pub(super) native: platform::NativeNamespaceAnchor }

pub(crate) struct DirectoryAuthority {
    pub(super) anchor: Arc<NamespaceAnchor>,
    pub(super) native: platform::NativeDirectory,
    pub(super) access: DirectoryAccess,
    pub(super) opened: EntryMetadata,
    pub(super) case_mode: CaseMode,
    pub(super) snapshot: NamespaceSnapshot,
}

pub(crate) struct FileCapability {
    pub(super) native: platform::NativeFile,
    pub(super) access: FileAccess,
    pub(super) opened: EntryMetadata,
}

pub(crate) struct StageCapability {
    pub(super) parent: DirectoryAuthority,
    pub(super) directory: DirectoryAuthority,
    pub(super) original_name: ComponentName,
    pub(super) opened: EntryMetadata,
}

pub(crate) struct QuarantinedCapability {
    pub(super) parent: DirectoryAuthority,
    pub(super) directory: DirectoryAuthority,
    pub(super) original_name: ComponentName,
    pub(super) quarantine_name: ComponentName,
    pub(super) opened: EntryMetadata,
}

pub(crate) enum CleanupCapability {
    Entry {
        parent: DirectoryAuthority,
        native: platform::NativeFile,
        name: ComponentName,
        opened: EntryMetadata,
        access: CleanupAccess,
    },
    Directory(QuarantinedCapability),
}

impl fmt::Debug for DirectoryAuthority { fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { f.write_str("DirectoryAuthority(<redacted>)") } }
impl fmt::Debug for FileCapability { fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { f.write_str("FileCapability(<redacted>)") } }
impl fmt::Debug for StageCapability { fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { f.write_str("StageCapability(<redacted>)") } }
impl fmt::Debug for QuarantinedCapability { fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { f.write_str("QuarantinedCapability(<redacted>)") } }
impl fmt::Debug for CleanupCapability { fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { f.write_str("CleanupCapability(<redacted>)") } }

impl DirectoryAuthority {
    pub(crate) fn access(&self) -> DirectoryAccess { self.access }
    pub(crate) fn opened_metadata(&self) -> &EntryMetadata { &self.opened }
    pub(crate) fn namespace_snapshot(&self) -> &NamespaceSnapshot { &self.snapshot }
}

impl StageCapability {
    pub(crate) fn directory(&self) -> &DirectoryAuthority { &self.directory }
    pub(crate) fn opened_metadata(&self) -> &EntryMetadata { &self.opened }
}

impl QuarantinedCapability {
    pub(crate) fn directory(&self) -> &DirectoryAuthority { &self.directory }
    pub(crate) fn opened_metadata(&self) -> &EntryMetadata { &self.opened }
}

impl FileCapability {
    pub(crate) fn opened_metadata(&self) -> &EntryMetadata { &self.opened }
    pub(crate) fn metadata(&self) -> Result<EntryMetadata> { platform::metadata_from_file(&self.native) }
    pub(crate) fn read(&mut self, buffer: &mut [u8]) -> Result<usize> { platform::read_file(&mut self.native, buffer) }
    pub(crate) fn write_all(&mut self, mut buffer: &[u8]) -> Result<()> {
        if self.access != FileAccess::ReadWrite { return Err(SafeFsError::AccessMismatch { operation: SafeFsOperation::WriteFile }); }
        while !buffer.is_empty() {
            let written = platform::write_file(&mut self.native, buffer)?;
            if written == 0 { return Err(SafeFsError::InvalidNativeBuffer { operation: SafeFsOperation::WriteFile, reason: super::error::NativeBufferReason::WriteZero }); }
            buffer = &buffer[written..];
        }
        Ok(())
    }
    pub(crate) fn seek(&mut self, position: SeekFrom) -> Result<u64> { platform::seek_file(&mut self.native, position) }
    pub(crate) fn flush(&mut self) -> Result<()> { platform::flush_file(&mut self.native) }
    pub(crate) fn sync_all(&self) -> Result<()> { platform::sync_file(&self.native) }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CopyOutcome { pub(crate) bytes_copied: u64, pub(crate) source_after: EntryMetadata, pub(crate) destination_after: EntryMetadata }

pub(crate) fn stream_copy_file(source: &mut FileCapability, destination: &mut FileCapability, expected_source: &StableIdentity, byte_limit: u64) -> Result<CopyOutcome> {
    let source_before = source.metadata()?;
    let destination_before = destination.metadata()?;
    if source_before.kind != EntryKind::RegularFile { return Err(SafeFsError::UnsupportedEntryType { operation: SafeFsOperation::CopyRead, kind: source_before.kind }); }
    if destination_before.kind != EntryKind::RegularFile { return Err(SafeFsError::UnsupportedEntryType { operation: SafeFsOperation::CopyWrite, kind: destination_before.kind }); }
    if &source_before.identity != expected_source { return Err(SafeFsError::IdentityChanged { operation: SafeFsOperation::CopyRead, expected: expected_source.clone(), actual: source_before.identity }); }
    if source_before.len > byte_limit { return Err(SafeFsError::CopyLimitExceeded { limit: byte_limit }); }
    source.seek(SeekFrom::Start(0))?;
    destination.seek(SeekFrom::Start(0))?;
    let mut remaining = source_before.len;
    let mut copied = 0_u64;
    let mut bytes = [0_u8; 65_536];
    while remaining != 0 {
        let request = usize::try_from(remaining.min(bytes.len() as u64)).expect("request is bounded by buffer length");
        let read = source.read(&mut bytes[..request])?;
        if read == 0 { return Err(SafeFsError::UnexpectedCopyEof); }
        destination.write_all(&bytes[..read])?;
        copied += read as u64;
        remaining -= read as u64;
    }
    let mut extra = [0_u8; 1];
    if source.read(&mut extra)? != 0 { return Err(SafeFsError::CopyLimitExceeded { limit: byte_limit }); }
    destination.flush()?;
    destination.sync_all()?;
    let source_after = source.metadata()?;
    let destination_after = destination.metadata()?;
    if source_after.identity != source_before.identity { return Err(SafeFsError::IdentityChanged { operation: SafeFsOperation::CopyRead, expected: source_before.identity, actual: source_after.identity }); }
    if destination_after.identity != destination_before.identity { return Err(SafeFsError::IdentityChanged { operation: SafeFsOperation::CopyWrite, expected: destination_before.identity, actual: destination_after.identity }); }
    if source_after.kind != EntryKind::RegularFile || source_after.len != source_before.len || source_after.link_count != source_before.link_count { return Err(SafeFsError::io(SafeFsOperation::CopyRead, std::io::Error::new(std::io::ErrorKind::InvalidData, "source metadata changed during copy"))); }
    if destination_after.kind != EntryKind::RegularFile || destination_after.len != copied { return Err(SafeFsError::io(SafeFsOperation::CopyWrite, std::io::Error::new(std::io::ErrorKind::InvalidData, "destination metadata does not match copied bytes"))); }
    Ok(CopyOutcome { bytes_copied: copied, source_after, destination_after })
}
```

### `safe_fs/ops.rs`

```rust
use super::capability::*;
use super::component::ComponentName;
use super::error::Result;
use super::platform;
use std::path::Path;

pub(crate) fn capture_absolute_directory(path: &Path, access: DirectoryAccess) -> Result<DirectoryAuthority> { platform::capture_absolute_directory(path, access) }
pub(crate) fn revalidate_namespace(directory: &DirectoryAuthority) -> Result<()> { platform::revalidate_namespace(directory) }
pub(crate) fn query_child_nofollow(parent: &DirectoryAuthority, name: &ComponentName) -> Result<ChildState> { platform::query_child_nofollow(parent, name) }
pub(crate) fn open_dir_nofollow(parent: &DirectoryAuthority, name: &ComponentName, access: DirectoryAccess) -> Result<DirectoryAuthority> { platform::open_dir_nofollow(parent, name, access) }
pub(crate) fn open_file_nofollow(parent: &DirectoryAuthority, name: &ComponentName, access: FileAccess) -> Result<FileCapability> { platform::open_file_nofollow(parent, name, access) }
pub(crate) fn create_dir_new(parent: &DirectoryAuthority, name: &ComponentName, permissions: CreatePermissions, access: DirectoryAccess) -> Result<DirectoryAuthority> { platform::create_dir_new(parent, name, permissions, access) }
pub(crate) fn create_stage_dir_new(parent: &DirectoryAuthority, name: &ComponentName, permissions: CreatePermissions) -> Result<StageCapability> { platform::create_stage_dir_new(parent, name, permissions) }
pub(crate) fn create_file_new(parent: &DirectoryAuthority, name: &ComponentName, permissions: CreatePermissions) -> Result<FileCapability> { platform::create_file_new(parent, name, permissions) }
pub(crate) fn enumerate(directory: &DirectoryAuthority) -> Result<Vec<ComponentName>> { platform::enumerate(directory) }
pub(crate) fn read_link_component(parent: &DirectoryAuthority, name: &ComponentName) -> Result<RawLinkTarget> { platform::read_link_component(parent, name) }
pub(crate) fn quarantine_stage(stage: StageCapability, parent: &DirectoryAuthority, quarantine_name: ComponentName) -> Result<QuarantinedCapability> { platform::quarantine_stage(stage, parent, quarantine_name) }
pub(crate) fn publish_stage_noreplace(stage: StageCapability, parent: &DirectoryAuthority, destination: ComponentName) -> Result<()> { platform::publish_stage_noreplace(stage, parent, destination) }
pub(crate) fn open_cleanup_child_nofollow(parent: &QuarantinedCapability, name: &ComponentName) -> Result<CleanupCapability> { platform::open_cleanup_child_nofollow(parent, name) }
pub(crate) fn delete_quarantined_entry(entry: CleanupCapability) -> Result<()> { platform::delete_quarantined_entry(entry) }
pub(crate) fn delete_quarantined_empty_directory(directory: QuarantinedCapability) -> Result<()> { platform::delete_quarantined_empty_directory(directory) }

pub(crate) fn cleanup_quarantined_tree(root: QuarantinedCapability) -> Result<()> {
    let names = enumerate(root.directory())?;
    for name in names {
        match open_cleanup_child_nofollow(&root, &name)? {
            CleanupCapability::Directory(child) => cleanup_quarantined_tree(child)?,
            entry @ CleanupCapability::Entry { .. } => delete_quarantined_entry(entry)?,
        }
    }
    delete_quarantined_empty_directory(root)
}
```

### `safe_fs/mod.rs`

```rust
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

pub(crate) use capability::{CaseMode, ChildState, CleanupAccess, CleanupCapability, CopyOutcome, CreatePermissions, DirectoryAccess, DirectoryAuthority, EntryKind, EntryMetadata, FileAccess, FileCapability, LocalFilesystemSnapshot, QuarantinedCapability, RawLinkTarget, StableIdentity, StageCapability, stream_copy_file};
pub(crate) use component::{ComponentName, RelativeComponents};
pub(crate) use error::{AtomicPublishReason, ComponentViolation, NativeBufferReason, RawOsError, SafeFsError, SafeFsOperation, SecureFilesystemReason, StageIdentityLostReason};
pub(crate) use ops::*;
```

## 3. Complete unsupported adapter

`safe_fs/unsupported.rs` is compile-complete and deliberately refuses acquisition. The mutation/I/O functions remain unreachable because no capability can be constructed; their bodies still return the unique typed refusal rather than panic.

```rust
use super::capability::*;
use super::component::ComponentName;
use super::error::{Result, SafeFsError, SafeFsOperation, SecureFilesystemReason};
use std::io::SeekFrom;
use std::path::Path;

pub(super) struct NativeNamespaceAnchor;
pub(super) struct NativeDirectory;
pub(super) struct NativeFile;

fn unsupported<T>(operation: SafeFsOperation) -> Result<T> {
    Err(SafeFsError::UnsupportedSecureFilesystem { operation, reason: SecureFilesystemReason::UnsupportedTarget })
}

pub(super) fn capture_absolute_directory(_: &Path, _: DirectoryAccess) -> Result<DirectoryAuthority> { unsupported(SafeFsOperation::CaptureNamespaceRoot) }
pub(super) fn revalidate_namespace(_: &DirectoryAuthority) -> Result<()> { unsupported(SafeFsOperation::RevalidateNamespace) }
pub(super) fn query_child_nofollow(_: &DirectoryAuthority, _: &ComponentName) -> Result<ChildState> { unsupported(SafeFsOperation::QueryChild) }
pub(super) fn open_dir_nofollow(_: &DirectoryAuthority, _: &ComponentName, _: DirectoryAccess) -> Result<DirectoryAuthority> { unsupported(SafeFsOperation::OpenDirectory) }
pub(super) fn open_file_nofollow(_: &DirectoryAuthority, _: &ComponentName, _: FileAccess) -> Result<FileCapability> { unsupported(SafeFsOperation::OpenFile) }
pub(super) fn create_dir_new(_: &DirectoryAuthority, _: &ComponentName, _: CreatePermissions, _: DirectoryAccess) -> Result<DirectoryAuthority> { unsupported(SafeFsOperation::CreateDirectory) }
pub(super) fn create_stage_dir_new(_: &DirectoryAuthority, _: &ComponentName, _: CreatePermissions) -> Result<StageCapability> { unsupported(SafeFsOperation::CreateStageDirectory) }
pub(super) fn create_file_new(_: &DirectoryAuthority, _: &ComponentName, _: CreatePermissions) -> Result<FileCapability> { unsupported(SafeFsOperation::CreateFile) }
pub(super) fn enumerate(_: &DirectoryAuthority) -> Result<Vec<ComponentName>> { unsupported(SafeFsOperation::EnumerateDirectory) }
pub(super) fn read_link_component(_: &DirectoryAuthority, _: &ComponentName) -> Result<RawLinkTarget> { unsupported(SafeFsOperation::ReadLink) }
pub(super) fn metadata_from_file(_: &NativeFile) -> Result<EntryMetadata> { unsupported(SafeFsOperation::QueryMetadata) }
pub(super) fn read_file(_: &mut NativeFile, _: &mut [u8]) -> Result<usize> { unsupported(SafeFsOperation::ReadFile) }
pub(super) fn write_file(_: &mut NativeFile, _: &[u8]) -> Result<usize> { unsupported(SafeFsOperation::WriteFile) }
pub(super) fn seek_file(_: &mut NativeFile, _: SeekFrom) -> Result<u64> { unsupported(SafeFsOperation::SeekFile) }
pub(super) fn flush_file(_: &mut NativeFile) -> Result<()> { unsupported(SafeFsOperation::FlushFile) }
pub(super) fn sync_file(_: &NativeFile) -> Result<()> { unsupported(SafeFsOperation::SyncFile) }
pub(super) fn quarantine_stage(_: StageCapability, _: &DirectoryAuthority, _: ComponentName) -> Result<QuarantinedCapability> { unsupported(SafeFsOperation::QuarantineNoReplace) }
pub(super) fn publish_stage_noreplace(_: StageCapability, _: &DirectoryAuthority, _: ComponentName) -> Result<()> { unsupported(SafeFsOperation::PublishNoReplace) }
pub(super) fn open_cleanup_child_nofollow(_: &QuarantinedCapability, _: &ComponentName) -> Result<CleanupCapability> { unsupported(SafeFsOperation::OpenCleanupEntry) }
pub(super) fn delete_quarantined_entry(_: CleanupCapability) -> Result<()> { unsupported(SafeFsOperation::DeleteQuarantinedEntry) }
pub(super) fn delete_quarantined_empty_directory(_: QuarantinedCapability) -> Result<()> { unsupported(SafeFsOperation::DeleteQuarantinedEmptyDirectory) }
```

Task 2A target adapters are exact one-line source files, so every configured target compiles against this same seam before native behavior lands:

```rust
// safe_fs/unix.rs during Task 2A and Task 2B only
include!("unsupported.rs");
```

```rust
// safe_fs/windows.rs during Task 2A and Task 2B only
include!("unsupported.rs");
```

Task 4 replaces the Unix file with section 4. The Windows task replaces the Windows file with the Windows normative appendix; neither adapter may retain a second facade.

## 4. Unix adapter: exact ownership and complete algorithms

`Cargo.toml` adds `rustix = { version = "=1.1.4", features = ["fs"] }` and `libc = "=0.2.186"` under `cfg(any(target_os = "linux", target_os = "macos"))`. The latter is already locked and is used only for per-directory case proof (`FS_IOC_GETFLAGS` on ext filesystems and `_PC_CASE_SENSITIVE` on macOS). The implementation uses the exact rustix 1.1.4 names below; no aggregate permission aliases are used.

```rust
use super::capability::*;
use super::component::ComponentName;
use super::error::*;
use rustix::fd::{AsFd, OwnedFd};
use rustix::fs::{AtFlags, Dir, FileType, Mode, OFlags, RenameFlags};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::os::fd::{AsRawFd, FromRawFd, IntoRawFd};
use std::os::unix::ffi::OsStrExt;
use std::path::{Component, Path};
use std::sync::Arc;

const DIR_FLAGS: OFlags = OFlags::RDONLY.union(OFlags::DIRECTORY).union(OFlags::NOFOLLOW).union(OFlags::CLOEXEC);
const FILE_READ_FLAGS: OFlags = OFlags::RDONLY.union(OFlags::NOFOLLOW).union(OFlags::CLOEXEC);
const FILE_RW_FLAGS: OFlags = OFlags::RDWR.union(OFlags::NOFOLLOW).union(OFlags::CLOEXEC);
const OWNER_DIR_MODE: Mode = Mode::RUSR.union(Mode::WUSR).union(Mode::XUSR);
const OWNER_FILE_MODE: Mode = Mode::RUSR.union(Mode::WUSR);
const INHERIT_DIR_MODE: Mode = Mode::RUSR.union(Mode::WUSR).union(Mode::XUSR).union(Mode::RGRP).union(Mode::WGRP).union(Mode::XGRP).union(Mode::ROTH).union(Mode::WOTH).union(Mode::XOTH);
const INHERIT_FILE_MODE: Mode = Mode::RUSR.union(Mode::WUSR).union(Mode::RGRP).union(Mode::WGRP).union(Mode::ROTH).union(Mode::WOTH);
#[cfg(target_os = "linux")]
const EXT_MAGIC: i64 = 0x0000_ef53;
#[cfg(target_os = "linux")]
const XFS_MAGIC: i64 = 0x5846_5342;
#[cfg(target_os = "linux")]
const BTRFS_MAGIC: i64 = 0x9123_683e;
#[cfg(target_os = "linux")]
const TMPFS_MAGIC: i64 = 0x0102_1994;
#[cfg(target_os = "linux")]
const NFS_MAGIC: i64 = 0x0000_6969;
#[cfg(target_os = "linux")]
const CIFS_MAGIC: i64 = 0xff53_4d42;
#[cfg(target_os = "linux")]
const SMB2_MAGIC: i64 = 0xfe53_4d42;
#[cfg(target_os = "linux")]
const FS_IOC_GETFLAGS: libc::c_ulong = 0x8008_6601;
#[cfg(target_os = "linux")]
const FS_CASEFOLD_FL: libc::c_long = 0x4000_0000;
#[cfg(target_os = "macos")]
const MNT_LOCAL: u32 = 0x0000_1000;

pub(super) struct NativeNamespaceAnchor { root: OwnedFd }
pub(super) struct NativeDirectory { fd: OwnedFd }
pub(super) enum NativeFile {
    Open(File),
    NameOnly { name: ComponentName, expected: StableIdentity, kind: EntryKind },
}

fn io(operation: SafeFsOperation, error: rustix::io::Errno) -> SafeFsError { SafeFsError::io(operation, std::io::Error::from_raw_os_error(error.raw_os_error())) }
fn identity(stat: &rustix::fs::Stat) -> StableIdentity { StableIdentity::Unix { device: stat.st_dev as u64, inode: stat.st_ino as u64 } }
fn kind(stat: &rustix::fs::Stat) -> EntryKind {
    match FileType::from_raw_mode(stat.st_mode) {
        FileType::RegularFile => EntryKind::RegularFile,
        FileType::Directory => EntryKind::Directory,
        FileType::Symlink => EntryKind::SymlinkOrReparse,
        FileType::Fifo => EntryKind::Fifo,
        FileType::Socket => EntryKind::Socket,
        FileType::BlockDevice => EntryKind::BlockDevice,
        FileType::CharacterDevice => EntryKind::CharacterDevice,
        _ => EntryKind::Other,
    }
}

#[cfg(target_os = "linux")]
fn linux_filesystem_from_raw(magic: i64, fsid: u64, device: u64, operation: SafeFsOperation) -> Result<LocalFilesystemSnapshot> {
    let family = match magic {
        EXT_MAGIC => LinuxFilesystem::Ext,
        XFS_MAGIC => LinuxFilesystem::Xfs,
        BTRFS_MAGIC => LinuxFilesystem::Btrfs,
        TMPFS_MAGIC => LinuxFilesystem::Tmpfs,
        NFS_MAGIC | CIFS_MAGIC | SMB2_MAGIC => return Err(SafeFsError::UnsupportedSecureFilesystem { operation, reason: SecureFilesystemReason::RemoteFilesystem }),
        _ => return Err(SafeFsError::UnsupportedSecureFilesystem { operation, reason: SecureFilesystemReason::UnknownFilesystem }),
    };
    Ok(LocalFilesystemSnapshot::Linux { family, fsid, device })
}

#[cfg(target_os = "linux")]
fn probe_local(fd: impl AsFd, stat: &rustix::fs::Stat, operation: SafeFsOperation) -> Result<LocalFilesystemSnapshot> {
    #[cfg(test)]
    if let Some(sample) = super::test_seam::unix_probe_sample() {
        return match sample {
            super::test_seam::UnixProbeSample::Linux { magic, fsid, device, .. } => linux_filesystem_from_raw(magic, fsid, device, operation),
            super::test_seam::UnixProbeSample::Failure(reason) => Err(SafeFsError::UnsupportedSecureFilesystem { operation, reason }),
            super::test_seam::UnixProbeSample::MacOs { .. } => Err(SafeFsError::UnsupportedSecureFilesystem { operation, reason: SecureFilesystemReason::FilesystemProbeUnavailable }),
        };
    }
    let fs = rustix::fs::fstatfs(&fd).map_err(|error| io(operation, error))?;
    let vfs = rustix::fs::fstatvfs(fd).map_err(|error| io(operation, error))?;
    linux_filesystem_from_raw(fs.f_type as i64, vfs.f_fsid, stat.st_dev as u64, operation)
}

#[cfg(target_os = "macos")]
fn macos_filesystem_from_raw(flags: u32, type_name: [u8; 16], fsid: u64, device: u64, operation: SafeFsOperation) -> Result<LocalFilesystemSnapshot> {
    if flags & MNT_LOCAL == 0 { return Err(SafeFsError::UnsupportedSecureFilesystem { operation, reason: SecureFilesystemReason::RemoteFilesystem }); }
    Ok(LocalFilesystemSnapshot::MacOs { type_name, fsid, device })
}

#[cfg(target_os = "macos")]
fn probe_local(fd: impl AsFd, stat: &rustix::fs::Stat, operation: SafeFsOperation) -> Result<LocalFilesystemSnapshot> {
    #[cfg(test)]
    if let Some(sample) = super::test_seam::unix_probe_sample() {
        return match sample {
            super::test_seam::UnixProbeSample::MacOs { mount_flags, type_name, fsid, device, .. } => macos_filesystem_from_raw(mount_flags, type_name, fsid, device, operation),
            super::test_seam::UnixProbeSample::Failure(reason) => Err(SafeFsError::UnsupportedSecureFilesystem { operation, reason }),
            super::test_seam::UnixProbeSample::Linux { .. } => Err(SafeFsError::UnsupportedSecureFilesystem { operation, reason: SecureFilesystemReason::FilesystemProbeUnavailable }),
        };
    }
    let fs = rustix::fs::fstatfs(&fd).map_err(|error| io(operation, error))?;
    let vfs = rustix::fs::fstatvfs(fd).map_err(|error| io(operation, error))?;
    macos_filesystem_from_raw(fs.f_flags, fs.f_fstypename.map(|byte| byte as u8), vfs.f_fsid, stat.st_dev as u64, operation)
}

fn opened_metadata(fd: impl AsFd, operation: SafeFsOperation) -> Result<EntryMetadata> {
    let stat = rustix::fs::fstat(&fd).map_err(|error| io(operation, error))?;
    let filesystem = probe_local(fd, &stat, operation)?;
    Ok(EntryMetadata { identity: identity(&stat), kind: kind(&stat), len: stat.st_size as u64, link_count: stat.st_nlink as u64, filesystem: Some(filesystem) })
}

#[cfg(target_os = "linux")]
fn linux_case_from_raw(family: LinuxFilesystem, ext_flags: std::result::Result<i64, SecureFilesystemReason>, operation: SafeFsOperation) -> Result<CaseMode> {
    match family {
        LinuxFilesystem::Xfs | LinuxFilesystem::Btrfs | LinuxFilesystem::Tmpfs => Ok(CaseMode::Sensitive),
        LinuxFilesystem::Ext => {
            let flags = ext_flags.map_err(|reason| SafeFsError::UnsupportedSecureFilesystem { operation, reason })?;
            if flags & FS_CASEFOLD_FL as i64 == 0 { Ok(CaseMode::Sensitive) } else { Ok(CaseMode::Insensitive) }
        }
    }
}

#[cfg(target_os = "macos")]
fn macos_case_from_raw(value: i64, operation: SafeFsOperation) -> Result<CaseMode> {
    match value {
        0 => Ok(CaseMode::Insensitive),
        1 => Ok(CaseMode::Sensitive),
        _ => Err(SafeFsError::UnsupportedSecureFilesystem { operation, reason: SecureFilesystemReason::CaseSemanticsUnavailable }),
    }
}

#[cfg(target_os = "linux")]
fn probe_case_mode(fd: impl AsFd, metadata: &EntryMetadata, operation: SafeFsOperation) -> Result<CaseMode> {
    let family = match metadata.filesystem.as_ref() {
        Some(LocalFilesystemSnapshot::Linux { family, .. }) => *family,
        _ => return Err(SafeFsError::UnsupportedSecureFilesystem { operation, reason: SecureFilesystemReason::CaseSemanticsUnavailable }),
    };
    #[cfg(test)]
    if let Some(sample) = super::test_seam::unix_probe_sample() {
        return match sample {
            super::test_seam::UnixProbeSample::Linux { ext_flags, .. } => linux_case_from_raw(family, ext_flags, operation),
            super::test_seam::UnixProbeSample::Failure(reason) => Err(SafeFsError::UnsupportedSecureFilesystem { operation, reason }),
            super::test_seam::UnixProbeSample::MacOs { .. } => Err(SafeFsError::UnsupportedSecureFilesystem { operation, reason: SecureFilesystemReason::CaseSemanticsUnavailable }),
        };
    }
    let ext_flags = if family == LinuxFilesystem::Ext {
        let mut flags: libc::c_long = 0;
        let result = unsafe { libc::ioctl(fd.as_fd().as_raw_fd(), FS_IOC_GETFLAGS, &mut flags) };
        if result < 0 { Err(SecureFilesystemReason::CaseSemanticsUnavailable) } else { Ok(flags as i64) }
    } else {
        Ok(0)
    };
    linux_case_from_raw(family, ext_flags, operation)
}

#[cfg(target_os = "macos")]
fn probe_case_mode(fd: impl AsFd, _: &EntryMetadata, operation: SafeFsOperation) -> Result<CaseMode> {
    #[cfg(test)]
    if let Some(sample) = super::test_seam::unix_probe_sample() {
        return match sample {
            super::test_seam::UnixProbeSample::MacOs { case_sensitive, .. } => macos_case_from_raw(case_sensitive, operation),
            super::test_seam::UnixProbeSample::Failure(reason) => Err(SafeFsError::UnsupportedSecureFilesystem { operation, reason }),
            super::test_seam::UnixProbeSample::Linux { .. } => Err(SafeFsError::UnsupportedSecureFilesystem { operation, reason: SecureFilesystemReason::CaseSemanticsUnavailable }),
        };
    }
    macos_case_from_raw(unsafe { libc::fpathconf(fd.as_fd().as_raw_fd(), libc::_PC_CASE_SENSITIVE) }, operation)
}

fn name_metadata(parent: &DirectoryAuthority, name: &ComponentName, operation: SafeFsOperation) -> Result<ChildState> {
    let stat = match rustix::fs::statat(&parent.native.fd, name.as_os_str(), AtFlags::SYMLINK_NOFOLLOW) {
        Ok(stat) => stat,
        Err(rustix::io::Errno::NOENT) => return Ok(ChildState::Absent),
        Err(error) => return Err(io(operation, error)),
    };
    let same_device = match (&parent.opened.identity, identity(&stat)) {
        (StableIdentity::Unix { device: parent_device, .. }, StableIdentity::Unix { device, .. }) => *parent_device == device,
        _ => false,
    };
    Ok(ChildState::Present(EntryMetadata {
        identity: identity(&stat),
        kind: kind(&stat),
        len: stat.st_size as u64,
        link_count: stat.st_nlink as u64,
        filesystem: same_device.then(|| parent.opened.filesystem.clone()).flatten(),
    }))
}

fn require_parent(parent: &DirectoryAuthority, operation: SafeFsOperation) -> Result<()> {
    if matches!(parent.access, DirectoryAccess::MutateChildren | DirectoryAccess::Stage) { Ok(()) } else { Err(SafeFsError::AccessMismatch { operation }) }
}

fn duplicate_directory(directory: &DirectoryAuthority, access: DirectoryAccess) -> Result<DirectoryAuthority> {
    let fd = rustix::io::fcntl_dupfd_cloexec(&directory.native.fd, 0).map_err(|error| io(SafeFsOperation::OpenDirectory, error))?;
    Ok(DirectoryAuthority { anchor: Arc::clone(&directory.anchor), native: NativeDirectory { fd }, access, opened: directory.opened.clone(), case_mode: directory.case_mode, snapshot: directory.snapshot.clone() })
}

fn open_dir_fd(parent: impl AsFd, name: &ComponentName, operation: SafeFsOperation) -> Result<(OwnedFd, EntryMetadata)> {
    let fd = rustix::fs::openat(parent, name.as_os_str(), DIR_FLAGS, Mode::empty()).map_err(|error| match error {
        rustix::io::Errno::NOENT => SafeFsError::NotFound { operation },
        rustix::io::Errno::LOOP => SafeFsError::SymlinkOrReparsePoint { operation },
        other => io(operation, other),
    })?;
    let metadata = opened_metadata(&fd, operation)?;
    if metadata.kind != EntryKind::Directory { return Err(SafeFsError::UnsupportedEntryType { operation, kind: metadata.kind }); }
    Ok((fd, metadata))
}

fn absolute_names(path: &Path) -> Result<Vec<ComponentName>> {
    let mut rooted = false;
    let mut names = Vec::new();
    for part in path.components() {
        match part {
            Component::RootDir if !rooted => rooted = true,
            Component::Normal(value) if rooted => names.push(ComponentName::new(value)?),
            _ => return Err(SafeFsError::InvalidRelativePath(RelativePathViolation::AbsoluteOrPrefix)),
        }
    }
    if !rooted { return Err(SafeFsError::InvalidRelativePath(RelativePathViolation::AbsoluteOrPrefix)); }
    Ok(names)
}

fn snapshot_from_root(root: OwnedFd, names: &[ComponentName], operation: SafeFsOperation) -> Result<(OwnedFd, EntryMetadata, NamespaceSnapshot)> {
    let root_metadata = opened_metadata(&root, operation)?;
    let root_case_mode = probe_case_mode(&root, &root_metadata, operation)?;
    let mut current = root;
    let mut components = Vec::with_capacity(names.len());
    for name in names {
        let (next, metadata) = open_dir_fd(&current, name, operation)?;
        let case_mode = probe_case_mode(&next, &metadata, operation)?;
        components.push(NamespaceComponent { name: name.clone(), identity: metadata.identity.clone(), filesystem: metadata.filesystem.clone().expect("opened directories have filesystem proof"), case_mode });
        current = next;
    }
    let snapshot = NamespaceSnapshot { root_identity: root_metadata.identity.clone(), root_filesystem: root_metadata.filesystem.clone().expect("opened root has filesystem proof"), root_case_mode, components };
    Ok((current, root_metadata, snapshot))
}

pub(super) fn capture_absolute_directory(path: &Path, access: DirectoryAccess) -> Result<DirectoryAuthority> {
    if access == DirectoryAccess::Stage { return Err(SafeFsError::AccessMismatch { operation: SafeFsOperation::OpenDirectory }); }
    let names = absolute_names(path)?;
    let anchor_root = rustix::fs::open("/", DIR_FLAGS, Mode::empty()).map_err(|error| io(SafeFsOperation::CaptureNamespaceRoot, error))?;
    let walk_root = rustix::io::fcntl_dupfd_cloexec(&anchor_root, 0).map_err(|error| io(SafeFsOperation::CaptureNamespaceRoot, error))?;
    let (tail, _, snapshot) = snapshot_from_root(walk_root, &names, SafeFsOperation::OpenAncestor)?;
    let opened = opened_metadata(&tail, SafeFsOperation::OpenAncestor)?;
    let case_mode = snapshot.components.last().map_or(snapshot.root_case_mode, |row| row.case_mode);
    Ok(DirectoryAuthority { anchor: Arc::new(NamespaceAnchor { native: NativeNamespaceAnchor { root: anchor_root } }), native: NativeDirectory { fd: tail }, access, opened, case_mode, snapshot })
}

pub(super) fn revalidate_namespace(directory: &DirectoryAuthority) -> Result<()> {
    #[cfg(test)] super::test_seam::hit(super::test_seam::HookPoint::BeforeMappingRewalk);
    let root = rustix::fs::open("/", DIR_FLAGS, Mode::empty()).map_err(|error| io(SafeFsOperation::RevalidateNamespace, error))?;
    let names: Vec<ComponentName> = directory.snapshot.components.iter().map(|row| row.name.clone()).collect();
    let (_, _, actual) = snapshot_from_root(root, &names, SafeFsOperation::RevalidateNamespace).map_err(|error| match error {
        SafeFsError::NotFound { .. } | SafeFsError::SymlinkOrReparsePoint { .. } | SafeFsError::UnsupportedEntryType { .. } | SafeFsError::UnsupportedSecureFilesystem { .. } => SafeFsError::NamespaceChanged { operation: SafeFsOperation::RevalidateNamespace },
        other => other,
    })?;
    if actual == directory.snapshot { Ok(()) } else { Err(SafeFsError::NamespaceChanged { operation: SafeFsOperation::RevalidateNamespace }) }
}

pub(super) fn query_child_nofollow(parent: &DirectoryAuthority, name: &ComponentName) -> Result<ChildState> { name_metadata(parent, name, SafeFsOperation::QueryChild) }

fn opened_child_directory(parent: &DirectoryAuthority, name: &ComponentName, access: DirectoryAccess, operation: SafeFsOperation) -> Result<DirectoryAuthority> {
    let (fd, opened) = open_dir_fd(&parent.native.fd, name, operation)?;
    let case_mode = probe_case_mode(&fd, &opened, operation)?;
    let mut snapshot = parent.snapshot.clone();
    snapshot.components.push(NamespaceComponent { name: name.clone(), identity: opened.identity.clone(), filesystem: opened.filesystem.clone().expect("opened directory has filesystem proof"), case_mode });
    Ok(DirectoryAuthority { anchor: Arc::clone(&parent.anchor), native: NativeDirectory { fd }, access, opened, case_mode, snapshot })
}

pub(super) fn open_dir_nofollow(parent: &DirectoryAuthority, name: &ComponentName, access: DirectoryAccess) -> Result<DirectoryAuthority> {
    if access == DirectoryAccess::Stage { return Err(SafeFsError::AccessMismatch { operation: SafeFsOperation::OpenDirectory }); }
    opened_child_directory(parent, name, access, SafeFsOperation::OpenDirectory)
}

fn open_regular(parent: &DirectoryAuthority, name: &ComponentName, access: FileAccess, operation: SafeFsOperation) -> Result<FileCapability> {
    let flags = if access == FileAccess::Read { FILE_READ_FLAGS } else { FILE_RW_FLAGS };
    let fd = rustix::fs::openat(&parent.native.fd, name.as_os_str(), flags, Mode::empty()).map_err(|error| match error {
        rustix::io::Errno::NOENT => SafeFsError::NotFound { operation },
        rustix::io::Errno::LOOP => SafeFsError::SymlinkOrReparsePoint { operation },
        other => io(operation, other),
    })?;
    let opened = opened_metadata(&fd, operation)?;
    if opened.kind != EntryKind::RegularFile { return Err(SafeFsError::UnsupportedEntryType { operation, kind: opened.kind }); }
    let raw = fd.into_raw_fd();
    let file = unsafe { File::from_raw_fd(raw) };
    Ok(FileCapability { native: NativeFile::Open(file), access, opened })
}

pub(super) fn open_file_nofollow(parent: &DirectoryAuthority, name: &ComponentName, access: FileAccess) -> Result<FileCapability> { open_regular(parent, name, access, SafeFsOperation::OpenFile) }

pub(super) fn create_dir_new(parent: &DirectoryAuthority, name: &ComponentName, permissions: CreatePermissions, access: DirectoryAccess) -> Result<DirectoryAuthority> {
    require_parent(parent, SafeFsOperation::CreateDirectory)?;
    if access == DirectoryAccess::Stage { return Err(SafeFsError::AccessMismatch { operation: SafeFsOperation::CreateDirectory }); }
    let mode = if permissions == CreatePermissions::OwnerOnly { OWNER_DIR_MODE } else { INHERIT_DIR_MODE };
    rustix::fs::mkdirat(&parent.native.fd, name.as_os_str(), mode).map_err(|error| match error { rustix::io::Errno::EXIST => SafeFsError::AlreadyExists { operation: SafeFsOperation::CreateDirectory }, other => io(SafeFsOperation::CreateDirectory, other) })?;
    open_dir_nofollow(parent, name, access)
}

pub(super) fn create_stage_dir_new(parent: &DirectoryAuthority, name: &ComponentName, permissions: CreatePermissions) -> Result<StageCapability> {
    require_parent(parent, SafeFsOperation::CreateStageDirectory)?;
    let mode = if permissions == CreatePermissions::OwnerOnly { OWNER_DIR_MODE } else { INHERIT_DIR_MODE };
    rustix::fs::mkdirat(&parent.native.fd, name.as_os_str(), mode).map_err(|error| match error { rustix::io::Errno::EXIST => SafeFsError::AlreadyExists { operation: SafeFsOperation::CreateStageDirectory }, other => io(SafeFsOperation::CreateStageDirectory, other) })?;
    let directory = opened_child_directory(parent, name, DirectoryAccess::Stage, SafeFsOperation::CreateStageDirectory)?;
    let opened = directory.opened.clone();
    Ok(StageCapability { parent: duplicate_directory(parent, DirectoryAccess::MutateChildren)?, directory, original_name: name.clone(), opened })
}

pub(super) fn create_file_new(parent: &DirectoryAuthority, name: &ComponentName, permissions: CreatePermissions) -> Result<FileCapability> {
    require_parent(parent, SafeFsOperation::CreateFile)?;
    let mode = if permissions == CreatePermissions::OwnerOnly { OWNER_FILE_MODE } else { INHERIT_FILE_MODE };
    let flags = FILE_RW_FLAGS.union(OFlags::CREATE).union(OFlags::EXCL);
    let fd = rustix::fs::openat(&parent.native.fd, name.as_os_str(), flags, mode).map_err(|error| match error { rustix::io::Errno::EXIST => SafeFsError::AlreadyExists { operation: SafeFsOperation::CreateFile }, other => io(SafeFsOperation::CreateFile, other) })?;
    let opened = opened_metadata(&fd, SafeFsOperation::CreateFile)?;
    if opened.kind != EntryKind::RegularFile { return Err(SafeFsError::UnsupportedEntryType { operation: SafeFsOperation::CreateFile, kind: opened.kind }); }
    let file = unsafe { File::from_raw_fd(fd.into_raw_fd()) };
    Ok(FileCapability { native: NativeFile::Open(file), access: FileAccess::ReadWrite, opened })
}

pub(super) fn metadata_from_file(native: &NativeFile) -> Result<EntryMetadata> {
    match native {
        NativeFile::Open(file) => { use std::os::fd::AsFd as _; opened_metadata(file.as_fd(), SafeFsOperation::QueryMetadata) }
        NativeFile::NameOnly { .. } => Err(SafeFsError::AccessMismatch { operation: SafeFsOperation::QueryMetadata }),
    }
}

pub(super) fn read_file(native: &mut NativeFile, buffer: &mut [u8]) -> Result<usize> { match native { NativeFile::Open(file) => file.read(buffer).map_err(|error| SafeFsError::io(SafeFsOperation::ReadFile, error)), NativeFile::NameOnly { .. } => Err(SafeFsError::AccessMismatch { operation: SafeFsOperation::ReadFile }) } }
pub(super) fn write_file(native: &mut NativeFile, buffer: &[u8]) -> Result<usize> { match native { NativeFile::Open(file) => file.write(buffer).map_err(|error| SafeFsError::io(SafeFsOperation::WriteFile, error)), NativeFile::NameOnly { .. } => Err(SafeFsError::AccessMismatch { operation: SafeFsOperation::WriteFile }) } }
pub(super) fn seek_file(native: &mut NativeFile, position: SeekFrom) -> Result<u64> { match native { NativeFile::Open(file) => file.seek(position).map_err(|error| SafeFsError::io(SafeFsOperation::SeekFile, error)), NativeFile::NameOnly { .. } => Err(SafeFsError::AccessMismatch { operation: SafeFsOperation::SeekFile }) } }
pub(super) fn flush_file(native: &mut NativeFile) -> Result<()> { match native { NativeFile::Open(file) => file.flush().map_err(|error| SafeFsError::io(SafeFsOperation::FlushFile, error)), NativeFile::NameOnly { .. } => Err(SafeFsError::AccessMismatch { operation: SafeFsOperation::FlushFile }) } }
pub(super) fn sync_file(native: &NativeFile) -> Result<()> { match native { NativeFile::Open(file) => file.sync_all().map_err(|error| SafeFsError::io(SafeFsOperation::SyncFile, error)), NativeFile::NameOnly { .. } => Err(SafeFsError::AccessMismatch { operation: SafeFsOperation::SyncFile }) } }

pub(super) fn enumerate(directory: &DirectoryAuthority) -> Result<Vec<ComponentName>> {
    let mut names = Vec::new();
    let stream = Dir::read_from(&directory.native.fd).map_err(|error| io(SafeFsOperation::EnumerateDirectory, error))?;
    for item in stream {
        let item = item.map_err(|error| io(SafeFsOperation::EnumerateDirectory, error))?;
        let bytes = item.file_name().to_bytes();
        if bytes == b"." || bytes == b".." { continue; }
        let name = ComponentName::new(std::ffi::OsStr::from_bytes(bytes))?;
        if matches!(query_child_nofollow(directory, &name)?, ChildState::Absent) { return Err(SafeFsError::NotFound { operation: SafeFsOperation::EnumerateDirectory }); }
        names.push(name);
    }
    names.sort_by(|left, right| left.as_os_str().as_bytes().cmp(right.as_os_str().as_bytes()));
    Ok(names)
}

pub(super) fn read_link_component(parent: &DirectoryAuthority, name: &ComponentName) -> Result<RawLinkTarget> {
    rustix::fs::readlinkat(&parent.native.fd, name.as_os_str(), Vec::new()).map(|value| RawLinkTarget::Unix(value.into_bytes())).map_err(|error| match error { rustix::io::Errno::NOENT => SafeFsError::NotFound { operation: SafeFsOperation::ReadLink }, other => io(SafeFsOperation::ReadLink, other) })
}

fn rename_noreplace(parent: &DirectoryAuthority, from: &ComponentName, to: &ComponentName, operation: SafeFsOperation) -> Result<()> {
    rustix::fs::renameat_with(&parent.native.fd, from.as_os_str(), &parent.native.fd, to.as_os_str(), RenameFlags::NOREPLACE).map_err(|error| match error {
        rustix::io::Errno::NOENT => SafeFsError::NotFound { operation },
        rustix::io::Errno::EXIST | rustix::io::Errno::NOTEMPTY => SafeFsError::AlreadyExists { operation },
        rustix::io::Errno::NOSYS | rustix::io::Errno::NOTSUP | rustix::io::Errno::OPNOTSUPP => SafeFsError::UnsupportedAtomicPublish { operation, reason: AtomicPublishReason::PrimitiveUnavailable },
        rustix::io::Errno::XDEV => SafeFsError::UnsupportedAtomicPublish { operation, reason: AtomicPublishReason::CrossDeviceInvariant },
        other => io(operation, other),
    })
}

fn parent_matches(retained: &DirectoryAuthority, supplied: &DirectoryAuthority) -> bool {
    retained.opened.identity == supplied.opened.identity
        && retained.opened.filesystem == supplied.opened.filesystem
        && retained.case_mode == supplied.case_mode
        && retained.snapshot == supplied.snapshot
        && matches!(supplied.access, DirectoryAccess::MutateChildren | DirectoryAccess::Stage)
}

fn restore_or_fail_leak(parent: &DirectoryAuthority, original: &ComponentName, quarantine: &ComponentName, reason: StageIdentityLostReason) -> SafeFsError {
    #[cfg(test)] super::test_seam::hit(super::test_seam::HookPoint::BeforeQuarantineRestore);
    let mapped = match name_metadata(parent, original, SafeFsOperation::RestoreQuarantine) {
        Ok(ChildState::Absent) => match rename_noreplace(parent, quarantine, original, SafeFsOperation::RestoreQuarantine) {
            Ok(()) => reason,
            Err(SafeFsError::AlreadyExists { .. }) => StageIdentityLostReason::OriginalNameOccupied,
            Err(_) => StageIdentityLostReason::QuarantineRestoreFailed,
        },
        Ok(ChildState::Present(_)) => StageIdentityLostReason::OriginalNameOccupied,
        Err(_) => StageIdentityLostReason::QuarantineRestoreFailed,
    };
    SafeFsError::StageIdentityLost { operation: SafeFsOperation::RestoreQuarantine, reason: mapped }
}

pub(super) fn quarantine_stage(stage: StageCapability, supplied_parent: &DirectoryAuthority, quarantine_name: ComponentName) -> Result<QuarantinedCapability> {
    if !parent_matches(&stage.parent, supplied_parent) { return Err(SafeFsError::StageIdentityLost { operation: SafeFsOperation::QuarantineNoReplace, reason: StageIdentityLostReason::ParentAuthorityChanged }); }
    #[cfg(test)] super::test_seam::hit(super::test_seam::HookPoint::BeforeQuarantineRename);
    rename_noreplace(&stage.parent, &stage.original_name, &quarantine_name, SafeFsOperation::QuarantineNoReplace)?;
    let reopened = match opened_child_directory(&stage.parent, &quarantine_name, DirectoryAccess::Stage, SafeFsOperation::VerifyQuarantine) {
        Ok(directory) if directory.opened.identity == stage.opened.identity && directory.opened.filesystem == stage.opened.filesystem => directory,
        Ok(_) => return Err(restore_or_fail_leak(&stage.parent, &stage.original_name, &quarantine_name, StageIdentityLostReason::QuarantinedObjectChanged)),
        Err(_) => return Err(restore_or_fail_leak(&stage.parent, &stage.original_name, &quarantine_name, StageIdentityLostReason::AmbiguousNameMutation)),
    };
    Ok(QuarantinedCapability { parent: stage.parent, directory: reopened, original_name: stage.original_name, quarantine_name, opened: stage.opened })
}

pub(super) fn publish_stage_noreplace(stage: StageCapability, supplied_parent: &DirectoryAuthority, destination: ComponentName) -> Result<()> {
    if !parent_matches(&stage.parent, supplied_parent) { return Err(SafeFsError::StageIdentityLost { operation: SafeFsOperation::PublishNoReplace, reason: StageIdentityLostReason::ParentAuthorityChanged }); }
    revalidate_namespace(&stage.directory)?;
    let current = name_metadata(&stage.parent, &stage.original_name, SafeFsOperation::PublishNoReplace)?;
    if !matches!(current, ChildState::Present(ref metadata) if metadata.identity == stage.opened.identity) { return Err(SafeFsError::StageIdentityLost { operation: SafeFsOperation::PublishNoReplace, reason: StageIdentityLostReason::SourceChangedBeforeQuarantine }); }
    rename_noreplace(&stage.parent, &stage.original_name, &destination, SafeFsOperation::PublishNoReplace)
}

fn nested_quarantined(parent: &QuarantinedCapability, name: &ComponentName, directory: DirectoryAuthority) -> Result<QuarantinedCapability> {
    let opened = directory.opened.clone();
    Ok(QuarantinedCapability { parent: duplicate_directory(parent.directory(), DirectoryAccess::MutateChildren)?, directory, original_name: name.clone(), quarantine_name: name.clone(), opened })
}

pub(super) fn open_cleanup_child_nofollow(parent: &QuarantinedCapability, name: &ComponentName) -> Result<CleanupCapability> {
    let metadata = match name_metadata(parent.directory(), name, SafeFsOperation::OpenCleanupEntry)? { ChildState::Absent => return Err(SafeFsError::NotFound { operation: SafeFsOperation::OpenCleanupEntry }), ChildState::Present(metadata) => metadata };
    if metadata.kind == EntryKind::Directory {
        let directory = opened_child_directory(parent.directory(), name, DirectoryAccess::Stage, SafeFsOperation::OpenCleanupEntry)?;
        if directory.opened.identity != metadata.identity { return Err(SafeFsError::IdentityChanged { operation: SafeFsOperation::OpenCleanupEntry, expected: metadata.identity, actual: directory.opened.identity }); }
        return Ok(CleanupCapability::Directory(nested_quarantined(parent, name, directory)?));
    }
    let parent_copy = duplicate_directory(parent.directory(), DirectoryAccess::MutateChildren)?;
    Ok(CleanupCapability::Entry { parent: parent_copy, native: NativeFile::NameOnly { name: name.clone(), expected: metadata.identity.clone(), kind: metadata.kind }, name: name.clone(), opened: metadata, access: CleanupAccess::Delete })
}

pub(super) fn delete_quarantined_entry(entry: CleanupCapability) -> Result<()> {
    let CleanupCapability::Entry { parent, native, name, opened, access: CleanupAccess::Delete } = entry else { return Err(SafeFsError::AccessMismatch { operation: SafeFsOperation::DeleteQuarantinedEntry }); };
    let NativeFile::NameOnly { name: native_name, expected, kind } = native else { return Err(SafeFsError::AccessMismatch { operation: SafeFsOperation::DeleteQuarantinedEntry }); };
    if name != native_name || opened.identity != expected || opened.kind != kind { return Err(SafeFsError::StageIdentityLost { operation: SafeFsOperation::DeleteQuarantinedEntry, reason: StageIdentityLostReason::QuarantineNameChanged }); }
    let retained_parent = rustix::fs::fstat(&parent.native.fd).map_err(|error| io(SafeFsOperation::DeleteQuarantinedEntry, error))?;
    if identity(&retained_parent) != parent.opened.identity { return Err(SafeFsError::StageIdentityLost { operation: SafeFsOperation::DeleteQuarantinedEntry, reason: StageIdentityLostReason::ParentAuthorityChanged }); }
    let stat = rustix::fs::statat(&parent.native.fd, native_name.as_os_str(), AtFlags::SYMLINK_NOFOLLOW).map_err(|error| match error { rustix::io::Errno::NOENT => SafeFsError::StageIdentityLost { operation: SafeFsOperation::DeleteQuarantinedEntry, reason: StageIdentityLostReason::QuarantineNameChanged }, other => io(SafeFsOperation::DeleteQuarantinedEntry, other) })?;
    let actual = identity(&stat);
    if actual != expected { return Err(SafeFsError::IdentityChanged { operation: SafeFsOperation::DeleteQuarantinedEntry, expected, actual }); }
    #[cfg(test)] super::test_seam::hit(super::test_seam::HookPoint::AfterFinalIdentityReadBeforeNameSyscall);
    rustix::fs::unlinkat(&parent.native.fd, native_name.as_os_str(), AtFlags::empty()).map_err(|error| match error { rustix::io::Errno::NOENT => SafeFsError::StageIdentityLost { operation: SafeFsOperation::DeleteQuarantinedEntry, reason: StageIdentityLostReason::QuarantineNameChanged }, other => io(SafeFsOperation::DeleteQuarantinedEntry, other) })
}

pub(super) fn delete_quarantined_empty_directory(directory: QuarantinedCapability) -> Result<()> {
    let stat = rustix::fs::statat(&directory.parent.native.fd, directory.quarantine_name.as_os_str(), AtFlags::SYMLINK_NOFOLLOW).map_err(|error| match error { rustix::io::Errno::NOENT => SafeFsError::StageIdentityLost { operation: SafeFsOperation::DeleteQuarantinedEmptyDirectory, reason: StageIdentityLostReason::QuarantineNameChanged }, other => io(SafeFsOperation::DeleteQuarantinedEmptyDirectory, other) })?;
    let actual = identity(&stat);
    if actual != directory.opened.identity { return Err(SafeFsError::IdentityChanged { operation: SafeFsOperation::DeleteQuarantinedEmptyDirectory, expected: directory.opened.identity, actual }); }
    #[cfg(test)] super::test_seam::hit(super::test_seam::HookPoint::AfterFinalIdentityReadBeforeNameSyscall);
    rustix::fs::unlinkat(&directory.parent.native.fd, directory.quarantine_name.as_os_str(), AtFlags::REMOVEDIR).map_err(|error| match error { rustix::io::Errno::NOENT => SafeFsError::StageIdentityLost { operation: SafeFsOperation::DeleteQuarantinedEmptyDirectory, reason: StageIdentityLostReason::QuarantineNameChanged }, other => io(SafeFsOperation::DeleteQuarantinedEmptyDirectory, other) })
}
```

The Unix implementation has no path fallback. `NativeFile::NameOnly` is intentionally not byte-readable; it exists so special-node cleanup remains nonblocking while retaining parent/name/identity provenance. `StageCapability.directory.native` and Windows `CleanupCapability::Entry.native` remain the originally acquired handles; the common wrapper never replaces them.

## 5. Deterministic seam and single test location

All private tests live in `crates/opentake-project/src/safe_fs/tests.rs`. No duplicate `unix::tests` module is permitted. `test_seam.rs` is complete:

```rust
use std::sync::{Arc, Condvar, Mutex, OnceLock};
use std::time::Duration;

#[cfg(any(target_os = "linux", target_os = "macos"))]
use super::error::SecureFilesystemReason;

#[cfg(any(target_os = "linux", target_os = "macos"))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum UnixProbeSample {
    Linux {
        magic: i64,
        fsid: u64,
        device: u64,
        ext_flags: std::result::Result<i64, SecureFilesystemReason>,
    },
    MacOs {
        mount_flags: u32,
        type_name: [u8; 16],
        fsid: u64,
        device: u64,
        case_sensitive: i64,
    },
    Failure(SecureFilesystemReason),
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
static UNIX_PROBE: OnceLock<Mutex<Option<UnixProbeSample>>> = OnceLock::new();

#[cfg(any(target_os = "linux", target_os = "macos"))]
pub(super) struct UnixProbeGuard;

#[cfg(any(target_os = "linux", target_os = "macos"))]
pub(super) fn install_unix_probe(sample: UnixProbeSample) -> UnixProbeGuard {
    let mut slot = UNIX_PROBE.get_or_init(|| Mutex::new(None)).lock().expect("Unix probe mutex poisoned");
    assert!(slot.is_none(), "safe_fs probe tests require --test-threads=1");
    *slot = Some(sample);
    UnixProbeGuard
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
impl UnixProbeGuard {
    pub(super) fn replace(&self, sample: UnixProbeSample) {
        let mut slot = UNIX_PROBE.get_or_init(|| Mutex::new(None)).lock().expect("Unix probe mutex poisoned");
        assert!(slot.is_some(), "Unix probe guard is not installed");
        *slot = Some(sample);
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
pub(super) fn unix_probe_sample() -> Option<UnixProbeSample> {
    UNIX_PROBE.get_or_init(|| Mutex::new(None)).lock().expect("Unix probe mutex poisoned").clone()
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
impl Drop for UnixProbeGuard {
    fn drop(&mut self) {
        *UNIX_PROBE.get_or_init(|| Mutex::new(None)).lock().expect("Unix probe mutex poisoned") = None;
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum HookPoint { BeforeQuarantineRename, BeforeQuarantineRestore, AfterFinalIdentityReadBeforeNameSyscall, BeforeMappingRewalk }
type Hook = Arc<dyn Fn(HookPoint) + Send + Sync>;
static HOOK: OnceLock<Mutex<Option<Hook>>> = OnceLock::new();
pub(super) struct HookGuard;
pub(super) fn install(hook: Hook) -> HookGuard { let mut slot = HOOK.get_or_init(|| Mutex::new(None)).lock().expect("hook mutex poisoned"); assert!(slot.is_none(), "safe_fs race tests require --test-threads=1"); *slot = Some(hook); HookGuard }
pub(super) fn hit(point: HookPoint) { let hook = HOOK.get_or_init(|| Mutex::new(None)).lock().expect("hook mutex poisoned").clone(); if let Some(hook) = hook { hook(point); } }
impl Drop for HookGuard { fn drop(&mut self) { *HOOK.get_or_init(|| Mutex::new(None)).lock().expect("hook mutex poisoned") = None; } }

pub(super) struct RaceGate { reached: (Mutex<bool>, Condvar), released: (Mutex<bool>, Condvar) }
impl RaceGate {
    pub(super) fn new() -> Arc<Self> { Arc::new(Self { reached: (Mutex::new(false), Condvar::new()), released: (Mutex::new(false), Condvar::new()) }) }
    pub(super) fn hook(self: &Arc<Self>, expected: HookPoint) -> Hook { let gate = Arc::clone(self); Arc::new(move |actual| { if actual != expected { return; } *gate.reached.0.lock().expect("race reached mutex poisoned") = true; gate.reached.1.notify_all(); let released = gate.released.0.lock().expect("race release mutex poisoned"); let (released, timeout) = gate.released.1.wait_timeout_while(released, Duration::from_secs(5), |value| !*value).expect("race release mutex poisoned"); assert!(*released && !timeout.timed_out(), "race release timed out"); }) }
    pub(super) fn wait_reached(&self) { let reached = self.reached.0.lock().expect("race reached mutex poisoned"); let (reached, timeout) = self.reached.1.wait_timeout_while(reached, Duration::from_secs(5), |value| !*value).expect("race reached mutex poisoned"); assert!(*reached && !timeout.timed_out(), "race hook was not reached"); }
    pub(super) fn release(&self) { *self.released.0.lock().expect("race release mutex poisoned") = true; self.released.1.notify_all(); }
}
```

The release build has neither race hooks nor probe injection: `test_seam` and every call to it are guarded by `#[cfg(test)]`. Real `fstatfs`/`fstatvfs`/`FS_IOC_GETFLAGS` and `MNT_LOCAL`/`_PC_CASE_SENSITIVE` probes remain the release path; injection feeds the same raw-value classifiers and the same `snapshot_from_root`/`revalidate_namespace` production path. Probe tests use a scoped in-process value replacement, never sleeps, polling, mounts, or network filesystems.

### Required complete Unix test bodies

```rust
use super::*;

#[test]
fn component_accepts_safe_names_and_rejects_too_long_and_unsafe_names() {
    let safe = ComponentName::new("asset.mov").expect("safe component accepted");
    assert_eq!(safe.as_os_str(), std::ffi::OsStr::new("asset.mov"));
    let too_long = "x".repeat(32_768);
    assert!(matches!(ComponentName::new(&too_long), Err(SafeFsError::InvalidComponent(ComponentViolation::TooLong))));
    assert!(matches!(ComponentName::new("."), Err(SafeFsError::InvalidComponent(ComponentViolation::CurrentDirectory))));
    assert!(matches!(ComponentName::new(".."), Err(SafeFsError::InvalidComponent(ComponentViolation::ParentDirectory))));
    assert!(matches!(ComponentName::new("a/b"), Err(SafeFsError::InvalidComponent(ComponentViolation::MultipleComponents | ComponentViolation::WindowsSeparator))));
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
mod unix_contract {
    use super::super::*;
    use super::super::test_seam::{self, HookPoint, RaceGate};
    use std::path::{Path, PathBuf};
    use std::sync::Arc;

    struct TestDir(PathBuf);
    impl TestDir {
        fn new(label: &str) -> Self { static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0); let id = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed); let requested = std::env::temp_dir().join(format!("opentake-c1b-{label}-{}-{id}", std::process::id())); std::fs::create_dir(&requested).expect("create fixture"); let path = std::fs::canonicalize(&requested).expect("canonical fixture anchor"); Self(path) }
        fn path(&self) -> &Path { &self.0 }
    }
    impl Drop for TestDir { fn drop(&mut self) { let _ = std::fs::remove_dir_all(&self.0); } }
    fn name(value: &str) -> ComponentName { ComponentName::new(value).expect("valid fixture component") }
    fn present_identity(parent: &DirectoryAuthority, value: &str) -> StableIdentity {
        match query_child_nofollow(parent, &name(value)).expect("query fixture child") {
            ChildState::Present(metadata) => metadata.identity,
            ChildState::Absent => panic!("expected present fixture child"),
        }
    }

    fn assert_probe_rejected(sample: test_seam::UnixProbeSample, expected: SecureFilesystemReason) {
        let temp = TestDir::new("probe-rejected");
        let _guard = test_seam::install_unix_probe(sample);
        let error = capture_absolute_directory(temp.path(), DirectoryAccess::Read).expect_err("probe must reject capture");
        assert!(matches!(error, SafeFsError::UnsupportedSecureFilesystem { reason, .. } if reason == expected));
    }

    #[cfg(target_os = "linux")]
    fn linux_sample(magic: i64, fsid: u64, device: u64, ext_flags: std::result::Result<i64, SecureFilesystemReason>) -> test_seam::UnixProbeSample {
        test_seam::UnixProbeSample::Linux { magic, fsid, device, ext_flags }
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_filesystem_and_case_probe_matrix_is_enforced() {
        use super::super::capability::LinuxFilesystem;
        const EXT: i64 = 0x0000_ef53;
        const XFS: i64 = 0x5846_5342;
        const BTRFS: i64 = 0x9123_683e;
        const TMPFS: i64 = 0x0102_1994;
        const NFS: i64 = 0x0000_6969;
        const CASEFOLD: i64 = 0x4000_0000;
        for (magic, expected) in [(EXT, LinuxFilesystem::Ext), (XFS, LinuxFilesystem::Xfs), (BTRFS, LinuxFilesystem::Btrfs), (TMPFS, LinuxFilesystem::Tmpfs)] {
            let temp = TestDir::new("linux-accepted");
            let _guard = test_seam::install_unix_probe(linux_sample(magic, 7, 11, Ok(0)));
            let authority = capture_absolute_directory(temp.path(), DirectoryAccess::Read).expect("accepted Linux family");
            assert!(matches!(&authority.namespace_snapshot().root_filesystem, LocalFilesystemSnapshot::Linux { family, fsid: 7, device: 11 } if *family == expected));
            assert_eq!(authority.namespace_snapshot().root_case_mode, CaseMode::Sensitive);
        }
        {
            let temp = TestDir::new("linux-casefold");
            let _guard = test_seam::install_unix_probe(linux_sample(EXT, 7, 11, Ok(CASEFOLD)));
            let authority = capture_absolute_directory(temp.path(), DirectoryAccess::Read).expect("ext casefold probe");
            assert_eq!(authority.namespace_snapshot().root_case_mode, CaseMode::Insensitive);
        }
        assert_probe_rejected(linux_sample(0x7fff_ffff, 7, 11, Ok(0)), SecureFilesystemReason::UnknownFilesystem);
        assert_probe_rejected(linux_sample(NFS, 7, 11, Ok(0)), SecureFilesystemReason::RemoteFilesystem);
        assert_probe_rejected(test_seam::UnixProbeSample::Failure(SecureFilesystemReason::FilesystemProbeUnavailable), SecureFilesystemReason::FilesystemProbeUnavailable);
        assert_probe_rejected(linux_sample(EXT, 7, 11, Err(SecureFilesystemReason::CaseSemanticsUnavailable)), SecureFilesystemReason::CaseSemanticsUnavailable);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_revalidation_rejects_fsid_device_and_case_changes() {
        const EXT: i64 = 0x0000_ef53;
        const CASEFOLD: i64 = 0x4000_0000;
        let temp = TestDir::new("linux-probe-change");
        let baseline = linux_sample(EXT, 7, 11, Ok(0));
        let guard = test_seam::install_unix_probe(baseline.clone());

        let fsid_authority = capture_absolute_directory(temp.path(), DirectoryAccess::Read).expect("capture fsid baseline");
        guard.replace(linux_sample(EXT, 8, 11, Ok(0)));
        assert!(matches!(revalidate_namespace(&fsid_authority), Err(SafeFsError::NamespaceChanged { .. })));

        guard.replace(baseline.clone());
        let device_authority = capture_absolute_directory(temp.path(), DirectoryAccess::Read).expect("capture device baseline");
        guard.replace(linux_sample(EXT, 7, 12, Ok(0)));
        assert!(matches!(revalidate_namespace(&device_authority), Err(SafeFsError::NamespaceChanged { .. })));

        guard.replace(baseline);
        let case_authority = capture_absolute_directory(temp.path(), DirectoryAccess::Read).expect("capture case baseline");
        guard.replace(linux_sample(EXT, 7, 11, Ok(CASEFOLD)));
        assert!(matches!(revalidate_namespace(&case_authority), Err(SafeFsError::NamespaceChanged { .. })));
    }

    #[cfg(target_os = "macos")]
    fn macos_sample(mount_flags: u32, fsid: u64, device: u64, case_sensitive: i64) -> test_seam::UnixProbeSample {
        test_seam::UnixProbeSample::MacOs { mount_flags, type_name: *b"apfs\0\0\0\0\0\0\0\0\0\0\0\0", fsid, device, case_sensitive }
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_local_and_case_probe_matrix_is_enforced() {
        const LOCAL: u32 = 0x0000_1000;
        for (raw_case, expected) in [(1, CaseMode::Sensitive), (0, CaseMode::Insensitive)] {
            let temp = TestDir::new("macos-local");
            let _guard = test_seam::install_unix_probe(macos_sample(LOCAL, 7, 11, raw_case));
            let authority = capture_absolute_directory(temp.path(), DirectoryAccess::Read).expect("MNT_LOCAL and _PC_CASE_SENSITIVE accepted");
            assert!(matches!(&authority.namespace_snapshot().root_filesystem, LocalFilesystemSnapshot::MacOs { fsid: 7, device: 11, .. }));
            assert_eq!(authority.namespace_snapshot().root_case_mode, expected);
        }
        assert_probe_rejected(macos_sample(0, 7, 11, 1), SecureFilesystemReason::RemoteFilesystem);
        assert_probe_rejected(test_seam::UnixProbeSample::Failure(SecureFilesystemReason::FilesystemProbeUnavailable), SecureFilesystemReason::FilesystemProbeUnavailable);
        assert_probe_rejected(macos_sample(LOCAL, 7, 11, -1), SecureFilesystemReason::CaseSemanticsUnavailable);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_revalidation_rejects_fsid_device_and_case_changes() {
        const LOCAL: u32 = 0x0000_1000;
        let temp = TestDir::new("macos-probe-change");
        let baseline = macos_sample(LOCAL, 7, 11, 1);
        let guard = test_seam::install_unix_probe(baseline.clone());

        let fsid_authority = capture_absolute_directory(temp.path(), DirectoryAccess::Read).expect("capture fsid baseline");
        guard.replace(macos_sample(LOCAL, 8, 11, 1));
        assert!(matches!(revalidate_namespace(&fsid_authority), Err(SafeFsError::NamespaceChanged { .. })));

        guard.replace(baseline.clone());
        let device_authority = capture_absolute_directory(temp.path(), DirectoryAccess::Read).expect("capture device baseline");
        guard.replace(macos_sample(LOCAL, 7, 12, 1));
        assert!(matches!(revalidate_namespace(&device_authority), Err(SafeFsError::NamespaceChanged { .. })));

        guard.replace(baseline);
        let case_authority = capture_absolute_directory(temp.path(), DirectoryAccess::Read).expect("capture case baseline");
        guard.replace(macos_sample(LOCAL, 7, 11, 0));
        assert!(matches!(revalidate_namespace(&case_authority), Err(SafeFsError::NamespaceChanged { .. })));
    }

    #[test]
    fn recursive_authority_revalidates_anchor_and_entire_child_scope() {
        let temp = TestDir::new("scope");
        std::fs::create_dir_all(temp.path().join("a/b")).expect("create tree");
        let root = capture_absolute_directory(temp.path(), DirectoryAccess::MutateChildren).expect("capture");
        let a = open_dir_nofollow(&root, &name("a"), DirectoryAccess::Read).expect("open a");
        let b = open_dir_nofollow(&a, &name("b"), DirectoryAccess::Read).expect("open b");
        assert_eq!(b.namespace_snapshot().components.len(), root.namespace_snapshot().components.len() + 2);
        revalidate_namespace(&b).expect("full rewalk");
        std::fs::rename(temp.path().join("a/b"), temp.path().join("a/b-retained")).expect("move child scope");
        std::fs::create_dir(temp.path().join("a/b")).expect("replace child scope");
        assert!(matches!(revalidate_namespace(&b), Err(SafeFsError::NamespaceChanged { operation: SafeFsOperation::RevalidateNamespace })));
    }

    #[test]
    fn query_symlink_fifo_and_special_entries_is_nonblocking_present_metadata() {
        use std::os::unix::fs::symlink;
        use std::process::Command;
        let temp = TestDir::new("query-special");
        symlink("missing-target", temp.path().join("link")).expect("symlink");
        let status = Command::new("mkfifo").arg(temp.path().join("pipe")).status().expect("run mkfifo");
        assert!(status.success());
        let root = capture_absolute_directory(temp.path(), DirectoryAccess::Read).expect("capture");
        assert!(matches!(query_child_nofollow(&root, &name("link")), Ok(ChildState::Present(EntryMetadata { kind: EntryKind::SymlinkOrReparse, .. }))));
        assert!(matches!(query_child_nofollow(&root, &name("pipe")), Ok(ChildState::Present(EntryMetadata { kind: EntryKind::Fifo, .. }))));
    }

    #[test]
    fn platform_dispatched_file_bytes_copy_seek_flush_and_sync() {
        let temp = TestDir::new("bytes");
        std::fs::write(temp.path().join("source"), b"0123456789").expect("source");
        let root = capture_absolute_directory(temp.path(), DirectoryAccess::MutateChildren).expect("capture");
        let mut source = open_file_nofollow(&root, &name("source"), FileAccess::Read).expect("open source");
        let expected = source.opened_metadata().identity.clone();
        let mut destination = create_file_new(&root, &name("destination"), CreatePermissions::OwnerOnly).expect("create destination");
        let result = stream_copy_file(&mut source, &mut destination, &expected, 10).expect("copy");
        assert_eq!(result.bytes_copied, 10);
        destination.seek(std::io::SeekFrom::Start(0)).expect("rewind");
        let mut bytes = [0_u8; 10];
        assert_eq!(destination.read(&mut bytes).expect("read destination"), 10);
        assert_eq!(&bytes, b"0123456789");
    }

    #[test]
    fn nested_recursive_quarantine_cleanup_removes_files_symlink_fifo_and_directories() {
        use std::os::unix::fs::symlink;
        use std::process::Command;
        let temp = TestDir::new("nested-cleanup");
        let root = capture_absolute_directory(temp.path(), DirectoryAccess::MutateChildren).expect("capture");
        let stage = create_stage_dir_new(&root, &name("stage"), CreatePermissions::OwnerOnly).expect("create stage");
        std::fs::create_dir_all(temp.path().join("stage/a/b")).expect("nested dirs");
        std::fs::write(temp.path().join("stage/a/file"), b"payload").expect("file");
        symlink("file", temp.path().join("stage/a/link")).expect("symlink");
        assert!(Command::new("mkfifo").arg(temp.path().join("stage/a/b/pipe")).status().expect("mkfifo").success());
        let quarantined = quarantine_stage(stage, &root, name(".opentake-quarantine-0123456789abcdef")).expect("quarantine");
        cleanup_quarantined_tree(quarantined).expect("recursive cleanup");
        assert!(!temp.path().join("stage").exists());
        assert!(!temp.path().join(".opentake-quarantine-0123456789abcdef").exists());
    }

    #[test]
    fn destination_collision_preserves_stage_and_every_destination_kind() {
        for kind in ["file", "empty-dir", "non-empty-dir", "symlink"] {
            let temp = TestDir::new(kind);
            let root = capture_absolute_directory(temp.path(), DirectoryAccess::MutateChildren).expect("capture");
            let stage = create_stage_dir_new(&root, &name("stage"), CreatePermissions::OwnerOnly).expect("stage");
            match kind {
                "file" => std::fs::write(temp.path().join("destination"), b"existing").expect("file"),
                "empty-dir" => std::fs::create_dir(temp.path().join("destination")).expect("dir"),
                "non-empty-dir" => { std::fs::create_dir(temp.path().join("destination")).expect("dir"); std::fs::write(temp.path().join("destination/child"), b"existing").expect("child"); }
                "symlink" => std::os::unix::fs::symlink("target", temp.path().join("destination")).expect("symlink"),
                _ => unreachable!(),
            }
            assert!(matches!(publish_stage_noreplace(stage, &root, name("destination")), Err(SafeFsError::AlreadyExists { operation: SafeFsOperation::PublishNoReplace })));
            assert!(temp.path().join("stage").is_dir());
        }
    }

    #[test]
    fn source_swap_before_quarantine_restores_without_deletion() {
        let temp = TestDir::new("source-swap");
        let root = Arc::new(capture_absolute_directory(temp.path(), DirectoryAccess::MutateChildren).expect("capture"));
        let stage = create_stage_dir_new(&root, &name("stage"), CreatePermissions::OwnerOnly).expect("stage");
        std::fs::write(temp.path().join("stage/expected"), b"expected").expect("expected file");
        let gate = RaceGate::new();
        let _guard = test_seam::install(gate.hook(HookPoint::BeforeQuarantineRename));
        let worker_root = Arc::clone(&root);
        let worker = std::thread::spawn(move || quarantine_stage(stage, &worker_root, name(".opentake-quarantine-source-swap")));
        gate.wait_reached();
        std::fs::rename(temp.path().join("stage"), temp.path().join("expected-moved")).expect("move expected stage");
        std::fs::create_dir(temp.path().join("stage")).expect("replacement stage");
        std::fs::write(temp.path().join("stage/replacement"), b"replacement").expect("replacement file");
        gate.release();
        assert!(matches!(worker.join().expect("worker join"), Err(SafeFsError::StageIdentityLost { operation: SafeFsOperation::RestoreQuarantine, .. })));
        assert_eq!(std::fs::read(temp.path().join("expected-moved/expected")).expect("expected preserved"), b"expected");
        assert_eq!(std::fs::read(temp.path().join("stage/replacement")).expect("replacement restored"), b"replacement");
        assert!(!temp.path().join(".opentake-quarantine-source-swap").exists());
    }

    #[test]
    fn restore_collision_fail_leaks_original_and_quarantine() {
        let temp = TestDir::new("restore-collision");
        let root = Arc::new(capture_absolute_directory(temp.path(), DirectoryAccess::MutateChildren).expect("capture"));
        let stage = create_stage_dir_new(&root, &name("stage"), CreatePermissions::OwnerOnly).expect("stage");
        std::fs::write(temp.path().join("stage/expected"), b"expected").expect("expected file");
        std::fs::rename(temp.path().join("stage"), temp.path().join("expected-moved")).expect("move expected stage");
        std::fs::create_dir(temp.path().join("stage")).expect("replacement stage");
        std::fs::write(temp.path().join("stage/replacement"), b"replacement").expect("replacement file");
        let gate = RaceGate::new();
        let _guard = test_seam::install(gate.hook(HookPoint::BeforeQuarantineRestore));
        let worker_root = Arc::clone(&root);
        let worker = std::thread::spawn(move || quarantine_stage(stage, &worker_root, name(".opentake-quarantine-restore-collision")));
        gate.wait_reached();
        std::fs::create_dir(temp.path().join("stage")).expect("occupy original name");
        std::fs::write(temp.path().join("stage/occupant"), b"occupant").expect("occupant file");
        gate.release();
        assert!(matches!(worker.join().expect("worker join"), Err(SafeFsError::StageIdentityLost { operation: SafeFsOperation::RestoreQuarantine, reason: StageIdentityLostReason::OriginalNameOccupied })));
        assert_eq!(std::fs::read(temp.path().join("stage/occupant")).expect("occupant preserved"), b"occupant");
        assert_eq!(std::fs::read(temp.path().join(".opentake-quarantine-restore-collision/replacement")).expect("quarantine preserved"), b"replacement");
        assert_eq!(std::fs::read(temp.path().join("expected-moved/expected")).expect("original retained handle target preserved"), b"expected");
    }

    #[test]
    fn final_unix_name_window_is_explicit_same_account_boundary() {
        let temp = TestDir::new("final-name-window");
        let root = capture_absolute_directory(temp.path(), DirectoryAccess::MutateChildren).expect("capture");
        let stage = create_stage_dir_new(&root, &name("stage"), CreatePermissions::OwnerOnly).expect("stage");
        std::fs::write(temp.path().join("stage/leaf"), b"expected").expect("expected leaf");
        let quarantined = quarantine_stage(stage, &root, name(".opentake-quarantine-final-window")).expect("quarantine");
        let entry = open_cleanup_child_nofollow(&quarantined, &name("leaf")).expect("open cleanup entry");
        let gate = RaceGate::new();
        let _guard = test_seam::install(gate.hook(HookPoint::AfterFinalIdentityReadBeforeNameSyscall));
        let worker = std::thread::spawn(move || delete_quarantined_entry(entry));
        gate.wait_reached();
        let quarantine_path = temp.path().join(".opentake-quarantine-final-window");
        std::fs::rename(quarantine_path.join("leaf"), quarantine_path.join("expected-moved")).expect("move expected leaf");
        std::fs::write(quarantine_path.join("leaf"), b"replacement").expect("replacement leaf");
        gate.release();
        worker.join().expect("worker join").expect("name-linearized deletion");
        assert_eq!(std::fs::read(quarantine_path.join("expected-moved")).expect("expected object preserved"), b"expected");
        assert!(!quarantine_path.join("leaf").exists());
    }

    #[test]
    fn cleanup_capability_records_identity_before_consuming_delete() {
        let temp = TestDir::new("cleanup-identity");
        let root = capture_absolute_directory(temp.path(), DirectoryAccess::MutateChildren).expect("capture");
        let stage = create_stage_dir_new(&root, &name("stage"), CreatePermissions::OwnerOnly).expect("stage");
        std::fs::write(temp.path().join("stage/leaf"), b"leaf").expect("leaf");
        let quarantined = quarantine_stage(stage, &root, name(".opentake-quarantine-identity" )).expect("quarantine");
        let expected = present_identity(quarantined.directory(), "leaf");
        let entry = open_cleanup_child_nofollow(&quarantined, &name("leaf")).expect("cleanup entry");
        match &entry {
            CleanupCapability::Entry { opened, access: CleanupAccess::Delete, .. } => assert_eq!(opened.identity, expected),
            CleanupCapability::Directory(_) => panic!("expected leaf cleanup capability"),
        }
        delete_quarantined_entry(entry).expect("delete retained cleanup entry");
    }
}
```

The three race tests use the exact bounded `RaceGate` body above. A sleep, polling loop, ignored result, or alternate test location is not accepted.

## 6. Unique error/absence mapping

| facade operation | absence | collision | reparse/symlink | unsupported primitive | other |
|---|---|---|---|---|---|
| query child | `Ok(Absent)` only here | n/a | `Ok(Present)` | secure-FS typed reason | exact operation error |
| open file/dir/cleanup | `NotFound` | n/a | ordinary open rejects; cleanup records the entry itself | secure-FS typed reason | exact operation error |
| create file/dir/stage | parent disappearance is `Io`; child collision is `AlreadyExists` | `AlreadyExists` | collision remains collision | secure-FS typed reason | exact operation error |
| byte I/O/metadata | never optional | n/a | handle already accepted | access mismatch/typed native reason | exact byte operation |
| quarantine | `NotFound(QuarantineNoReplace)` | quarantine target `AlreadyExists` without mutation | post-rename reopen rejects | `UnsupportedAtomicPublish(PrimitiveUnavailable)` | EXDEV is `CrossDeviceInvariant`; mismatch restore/fail-leak |
| recursive cleanup | `StageIdentityLost(QuarantineNameChanged)` | n/a | symlink is deleted as the entry, never followed | n/a | ambiguity preserves remaining tree |
| publish | `NotFound(PublishNoReplace)` | destination `AlreadyExists` without mutation | source name is not followed | typed primitive reason | EXDEV is cross-device; no fallback |

Windows NTSTATUS mapping, directory/reparse buffer reasons, DACL verification, and IOSB rules use these same enums and operation names. It may not add synonyms such as `CaseModeUnavailable`, `DeleteFileHandle`, or `PublishNoReplaceName`; retained-HANDLE Windows rename uses the unique `RenameNoReplaceSameParent` operation.

## 7. Executable test-first task protocol

All commands run in `/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake-full-convergence`. `SAFETY_ROOT=/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake-safety/20260712-wave1bc-filesystem`. The main-plan numbering is the only numbering used here: Task 2A is the compile scaffold, Task 2B is common validation, Task 4 is Unix read/acquisition, and Task 5 is Unix mutation/cleanup. Task 2A is the sole compile-scaffold exception: its test-only commit adds only the unresolved private module declaration, and its RED must be compiler `E0583`; its separate GREEN commit adds the compile-complete acquisition-refusing skeleton. Every behavioral task beginning with Task 2B commits a named failing test before implementation. Behavioral RED evidence must show `running 1 test`, a nonzero exit, the test-only SHA, and the unchanged implementation parent SHA. A behavioral command that reports `0 tests` invalidates the attempt.

### Task 2A — compile RED, then compile-complete fail-closed common skeleton

The test-only commit modifies exactly `crates/opentake-project/src/lib.rs`, adding one private `mod safe_fs;` declaration beside the existing crate-root module declarations. It must not create `safe_fs.rs`, `safe_fs/mod.rs`, or any other file. Before commit, `git diff --name-only` must print only `crates/opentake-project/src/lib.rs`, and `git diff -- crates/opentake-project/src/lib.rs` must show only that declaration. Commit and record the true compiler RED:

```bash
git diff --name-only | diff -u - <(printf '%s\n' crates/opentake-project/src/lib.rs)
git add crates/opentake-project/src/lib.rs
git diff --cached --name-only | diff -u - <(printf '%s\n' crates/opentake-project/src/lib.rs)
git commit -m "test(project): require C1B safe filesystem module"
TEST_SHA=$(git rev-parse HEAD)
RED_DIR="$SAFETY_ROOT/red/c1b-task-2a-$TEST_SHA"
mkdir "$RED_DIR"
set +e
cargo check -p opentake-project --lib >"$RED_DIR/output.log" 2>&1
STATUS=$?
set -e
test "$STATUS" -ne 0
rg -n "error\[E0583\]|file not found for module .safe_fs.|could not compile" "$RED_DIR/output.log"
printf 'test_sha=%s\nparent_sha=%s\nexit=%s\nkind=compile-module-missing\n' "$TEST_SHA" "$(git rev-parse "$TEST_SHA^")" "$STATUS" >"$RED_DIR/receipt.txt"
```

The GREEN commit then creates exactly `safe_fs/{mod,error,component,capability,ops,unsupported,unix,windows,test_seam,tests}.rs`; `lib.rs` is already committed and is not staged again. `error.rs`, `capability.rs`, `ops.rs`, `mod.rs`, and `unsupported.rs` use sections 2–3. Both selected target adapters are the exact `include!("unsupported.rs")` files from section 3. `tests.rs` is empty, and `test_seam.rs` is the complete section-5 source. The probe seam is test-only infrastructure and grants no authority; the unsupported adapters do not call it. The only temporary source is the following compile-complete, fail-closed `component.rs`; it constructs no component and therefore grants no filesystem authority:

```rust
use super::error::{Result, SafeFsError, SafeFsOperation, SecureFilesystemReason};
use std::ffi::{OsStr, OsString};
use std::path::Path;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct ComponentName(OsString);
impl ComponentName {
    pub(crate) fn new(_: impl AsRef<OsStr>) -> Result<Self> { Err(SafeFsError::UnsupportedSecureFilesystem { operation: SafeFsOperation::QueryChild, reason: SecureFilesystemReason::UnsupportedTarget }) }
    pub(crate) fn as_os_str(&self) -> &OsStr { &self.0 }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RelativeComponents(Vec<ComponentName>);
impl RelativeComponents {
    pub(crate) fn new(_: &Path) -> Result<Self> { Err(SafeFsError::UnsupportedSecureFilesystem { operation: SafeFsOperation::QueryChild, reason: SecureFilesystemReason::UnsupportedTarget }) }
    pub(crate) fn iter(&self) -> impl ExactSizeIterator<Item = &ComponentName> { self.0.iter() }
}
```

Scaffold GREEN, commit, and review:

```bash
cargo fmt --all --check
cargo check -p opentake-project --lib --tests --target aarch64-apple-darwin
cargo check -p opentake-project --lib --tests --target x86_64-unknown-linux-gnu
cargo check -p opentake-project --lib --tests --target x86_64-pc-windows-msvc
cargo clippy -p opentake-project --lib --tests -- -D warnings
git diff --check
git add crates/opentake-project/src/safe_fs
git diff --cached --name-only | diff -u - <(printf '%s\n' \
  crates/opentake-project/src/safe_fs/capability.rs \
  crates/opentake-project/src/safe_fs/component.rs \
  crates/opentake-project/src/safe_fs/error.rs \
  crates/opentake-project/src/safe_fs/mod.rs \
  crates/opentake-project/src/safe_fs/ops.rs \
  crates/opentake-project/src/safe_fs/test_seam.rs \
  crates/opentake-project/src/safe_fs/tests.rs \
  crates/opentake-project/src/safe_fs/unix.rs \
  crates/opentake-project/src/safe_fs/unsupported.rs \
  crates/opentake-project/src/safe_fs/windows.rs)
git commit -m "feat(project): add fail-closed C1B filesystem skeleton"
SCAFFOLD_SHA=$(git rev-parse HEAD)
```

For the first review set `REVIEW_ATTEMPT=1`; increment it for each rejected replacement without reusing a directory. The exact reports are `$SAFETY_ROOT/logs/c1b-task-2a-$SCAFFOLD_SHA-attempt-$REVIEW_ATTEMPT/{spec-security-review.md,implementation-review.md}`. Both must bind `SCAFFOLD_SHA`, cite `$SAFETY_ROOT/red/c1b-task-2a-$TEST_SHA/{output.log,receipt.txt}`, and approve 0/0/0 before Task 2B. Reviewers confirm the RED commit contains only `mod safe_fs;`, `E0583` is the compiler failure, every GREEN adapter refuses acquisition, and the temporary component constructor always fails.

### Task 2B — common facade, component validation, unsupported adapter

Test-only commit adds `component_accepts_safe_names_and_rejects_too_long_and_unsafe_names` to the sole `safe_fs/tests.rs`. Against the approved fail-closed scaffold it compiles, runs exactly one test, and fails at `safe component accepted`.

```bash
git add crates/opentake-project/src/safe_fs/tests.rs
git commit -m "test(project): specify C1B common capability facade"
TEST_SHA=$(git rev-parse HEAD)
RED_DIR="$SAFETY_ROOT/red/c1b-task-2b-$TEST_SHA"
mkdir "$RED_DIR"
set +e
cargo test -p opentake-project --lib safe_fs::tests::component_accepts_safe_names_and_rejects_too_long_and_unsafe_names -- --exact --test-threads=1 >"$RED_DIR/output.log" 2>&1
STATUS=$?
set -e
test "$STATUS" -ne 0
rg -n "running 1 test|component_accepts_safe_names_and_rejects_too_long_and_unsafe_names|safe component accepted|FAILED" "$RED_DIR/output.log"
printf 'test_sha=%s\nparent_sha=%s\nexit=%s\n' "$TEST_SHA" "$(git rev-parse "$TEST_SHA^")" "$STATUS" >"$RED_DIR/receipt.txt"
```

GREEN replaces only the temporary fail-closed `component.rs` with the complete validator in section 2. All acquisition adapters remain the section-3 refusal implementation, so Task 2B claims component/common compile behavior only.

```bash
cargo test -p opentake-project --lib safe_fs::tests::component_accepts_safe_names_and_rejects_too_long_and_unsafe_names -- --exact --test-threads=1
cargo test -p opentake-project --lib safe_fs::tests -- --test-threads=1
cargo fmt --all --check
cargo clippy -p opentake-project --lib --tests -- -D warnings
cargo check -p opentake-project --lib --tests --target aarch64-apple-darwin
cargo check -p opentake-project --lib --tests --target x86_64-unknown-linux-gnu
cargo check -p opentake-project --lib --tests --target x86_64-pc-windows-msvc
cargo check --workspace --all-targets
git diff --check
git add crates/opentake-project/src/safe_fs/component.rs
git commit -m "feat(project): validate C1B filesystem components"
GREEN_SHA=$(git rev-parse HEAD)
```

Task 2B does not claim file, directory, quarantine, or platform behavior. Its reports are `$SAFETY_ROOT/logs/c1b-task-2b-$GREEN_SHA-attempt-$REVIEW_ATTEMPT/{spec-security-review.md,implementation-review.md}` and its RED receipt is `$SAFETY_ROOT/red/c1b-task-2b-$TEST_SHA/receipt.txt`. Reviews bind `GREEN_SHA` and must be 0/0/0 before Task 4.

### Task 4 — Unix recursive namespace, filesystem/case proof, and platform-dispatched file I/O

The test-only commit adds exactly these named tests to the single `safe_fs/tests.rs`: `recursive_authority_revalidates_anchor_and_entire_child_scope`, `query_symlink_fifo_and_special_entries_is_nonblocking_present_metadata`, `platform_dispatched_file_bytes_copy_seek_flush_and_sync`, Linux-only `linux_filesystem_and_case_probe_matrix_is_enforced` and `linux_revalidation_rejects_fsid_device_and_case_changes`, and macOS-only `macos_local_and_case_probe_matrix_is_enforced` and `macos_revalidation_rejects_fsid_device_and_case_changes`. Against the approved unsupported adapter and already-reviewed test-only seam, the host-specific probe matrix compiles, runs once, and fails only when acquisition returns the typed refusal.

```bash
git add crates/opentake-project/src/safe_fs/tests.rs
git commit -m "test(project): specify Unix recursive filesystem authorities"
TEST_SHA=$(git rev-parse HEAD)
RED_DIR="$SAFETY_ROOT/red/c1b-task-4-$TEST_SHA"
mkdir "$RED_DIR"
case "$(uname -s)" in
  Linux) RED_TEST=linux_filesystem_and_case_probe_matrix_is_enforced ;;
  Darwin) RED_TEST=macos_local_and_case_probe_matrix_is_enforced ;;
  *) printf 'Task 4 RED requires native Linux or macOS\n' >&2; exit 1 ;;
esac
set +e
cargo test -p opentake-project --lib "safe_fs::tests::unix_contract::$RED_TEST" -- --exact --test-threads=1 >"$RED_DIR/output.log" 2>&1
STATUS=$?
set -e
test "$STATUS" -ne 0
rg -n "running 1 test|$RED_TEST|UnsupportedTarget|FAILED" "$RED_DIR/output.log"
printf 'test_sha=%s\nparent_sha=%s\nexit=%s\n' "$TEST_SHA" "$(git rev-parse "$TEST_SHA^")" "$STATUS" >"$RED_DIR/receipt.txt"
```

GREEN adds the Unix acquisition, real and injected filesystem/case probes, full-scope rewalk, query, file byte I/O, and create/open code from section 4. The injected raw samples pass through the same classifiers and snapshot comparison used by release probes and revalidation; real native probes remain additive. It does not add quarantine or cleanup yet. To keep the single platform seam compile-complete, Task 4 ends `unix.rs` with these exact fail-closed bodies; Task 5 replaces them with section 4's consuming implementations:

```rust
pub(super) fn quarantine_stage(_: StageCapability, _: &DirectoryAuthority, _: ComponentName) -> Result<QuarantinedCapability> {
    Err(SafeFsError::UnsupportedAtomicPublish { operation: SafeFsOperation::QuarantineNoReplace, reason: AtomicPublishReason::PrimitiveUnavailable })
}
pub(super) fn publish_stage_noreplace(_: StageCapability, _: &DirectoryAuthority, _: ComponentName) -> Result<()> {
    Err(SafeFsError::UnsupportedAtomicPublish { operation: SafeFsOperation::PublishNoReplace, reason: AtomicPublishReason::PrimitiveUnavailable })
}
pub(super) fn open_cleanup_child_nofollow(_: &QuarantinedCapability, _: &ComponentName) -> Result<CleanupCapability> {
    Err(SafeFsError::UnsupportedSecureFilesystem { operation: SafeFsOperation::OpenCleanupEntry, reason: SecureFilesystemReason::UnsupportedTarget })
}
pub(super) fn delete_quarantined_entry(_: CleanupCapability) -> Result<()> {
    Err(SafeFsError::UnsupportedSecureFilesystem { operation: SafeFsOperation::DeleteQuarantinedEntry, reason: SecureFilesystemReason::UnsupportedTarget })
}
pub(super) fn delete_quarantined_empty_directory(_: QuarantinedCapability) -> Result<()> {
    Err(SafeFsError::UnsupportedSecureFilesystem { operation: SafeFsOperation::DeleteQuarantinedEmptyDirectory, reason: SecureFilesystemReason::UnsupportedTarget })
}
```

```bash
cargo test -p opentake-project --lib safe_fs::tests::unix_contract::recursive_authority_revalidates_anchor_and_entire_child_scope -- --exact --test-threads=1
cargo test -p opentake-project --lib safe_fs::tests::unix_contract::query_symlink_fifo_and_special_entries_is_nonblocking_present_metadata -- --exact --test-threads=1
cargo test -p opentake-project --lib safe_fs::tests::unix_contract::platform_dispatched_file_bytes_copy_seek_flush_and_sync -- --exact --test-threads=1
cargo test -p opentake-project --lib safe_fs::tests::unix_contract -- --test-threads=1
cargo test -p opentake-project --lib safe_fs::tests -- --test-threads=1
cargo fmt --all --check
cargo clippy -p opentake-project --lib --tests -- -D warnings
cargo check -p opentake-project --lib --tests --target aarch64-apple-darwin
cargo check -p opentake-project --lib --tests --target x86_64-unknown-linux-gnu
cargo check -p opentake-project --lib --tests --target x86_64-pc-windows-msvc
cargo check --workspace --all-targets
git diff --check
git add crates/opentake-project/Cargo.toml Cargo.lock crates/opentake-project/src/safe_fs
git commit -m "feat(project): add Unix recursive filesystem authorities"
GREEN_SHA=$(git rev-parse HEAD)
```

Task 4 reports are `$SAFETY_ROOT/logs/c1b-task-4-$GREEN_SHA-attempt-$REVIEW_ATTEMPT/{spec-security-review.md,implementation-review.md}`, the RED receipt is `$SAFETY_ROOT/red/c1b-task-4-$TEST_SHA/receipt.txt`, and native receipts are `$SAFETY_ROOT/native-receipts/c1b-task-4-$GREEN_SHA/{linux,macos}/results.json`. Native Linux and macOS receipts at this exact `GREEN_SHA` are blocking for Task 4 approval; cross-target check is compilation evidence only.

### Task 5 — consuming quarantine, recursive cleanup, and publish

The test-only commit adds named tests `source_swap_before_quarantine_restores_without_deletion`, `restore_collision_fail_leaks_original_and_quarantine`, `final_unix_name_window_is_explicit_same_account_boundary`, `nested_recursive_quarantine_cleanup_removes_files_symlink_fifo_and_directories`, and `destination_collision_preserves_stage_and_every_destination_kind`.

```bash
git add crates/opentake-project/src/safe_fs/tests.rs
git commit -m "test(project): specify Unix quarantine and recursive cleanup"
TEST_SHA=$(git rev-parse HEAD)
RED_DIR="$SAFETY_ROOT/red/c1b-task-5-$TEST_SHA"
mkdir "$RED_DIR"
set +e
cargo test -p opentake-project --lib safe_fs::tests::unix_contract::nested_recursive_quarantine_cleanup_removes_files_symlink_fifo_and_directories -- --exact --test-threads=1 >"$RED_DIR/output.log" 2>&1
STATUS=$?
set -e
test "$STATUS" -ne 0
rg -n "running 1 test|nested_recursive_quarantine_cleanup_removes_files_symlink_fifo_and_directories|FAILED|error" "$RED_DIR/output.log"
printf 'test_sha=%s\nparent_sha=%s\nexit=%s\n' "$TEST_SHA" "$(git rev-parse "$TEST_SHA^")" "$STATUS" >"$RED_DIR/receipt.txt"
```

GREEN adds the complete consuming capability algorithms and their `#[cfg(test)]` bounded race-hook call sites from section 4; the section-5 seam itself was already present in the fail-closed scaffold.

```bash
cargo test -p opentake-project --lib safe_fs::tests::unix_contract -- --test-threads=1
cargo test -p opentake-project --lib safe_fs::tests -- --test-threads=1
cargo fmt --all --check
cargo clippy -p opentake-project --lib --tests -- -D warnings
cargo check -p opentake-project --lib --tests --target aarch64-apple-darwin
cargo check -p opentake-project --lib --tests --target x86_64-unknown-linux-gnu
cargo check -p opentake-project --lib --tests --target x86_64-pc-windows-msvc
cargo test -p opentake-project --all-targets -- --test-threads=1
cargo check --workspace --all-targets
cargo test -p opentake-tauri --test bundle_export_surface -- --test-threads=1
git diff --check
git add crates/opentake-project/src/safe_fs
git commit -m "feat(project): add Unix consuming quarantine cleanup"
GREEN_SHA=$(git rev-parse HEAD)
```

Task 5 reports are `$SAFETY_ROOT/logs/c1b-task-5-$GREEN_SHA-attempt-$REVIEW_ATTEMPT/{spec-security-review.md,implementation-review.md}`, its RED receipt is `$SAFETY_ROOT/red/c1b-task-5-$TEST_SHA/receipt.txt`, and native receipts are `$SAFETY_ROOT/native-receipts/c1b-task-5-$GREEN_SHA/{linux,macos}/results.json`. Both reviews bind the same 40-character SHA and state `APPROVE — Critical 0 / Important 0 / Minor 0`. Missing publication authority is `BLOCKED`; it is not permission to push or dispatch.

## 8. Residual platform limits

1. Linux's allowlist intentionally rejects unknown filesystem magic. A family is removed if native evidence cannot prove stable `f_fsid + st_dev + ordered dev/inode chain` behavior.
2. Unix final unlink/remove and rename source identity are name-linearized, not handle-bound. If a same-account namespace attacker enters the threat model, C1D requires a new journal/quarantine design.
3. Native Linux and macOS behavior receipts and native Windows receipts are required at the exact implementation SHA. Cross-compilation does not replace them.
4. No local instruction in this appendix grants push, PR, or workflow-dispatch authority. The implementation stops at the native receipt gate when those receipts cannot be obtained without new external authority.
