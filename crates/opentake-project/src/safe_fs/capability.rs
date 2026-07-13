use super::component::ComponentName;
use super::error::{Result, SafeFsError, SafeFsOperation};
use super::platform;
use std::fmt;
use std::io::SeekFrom;
use std::sync::Arc;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum StableIdentity {
    Unix {
        device: u64,
        inode: u64,
    },
    Windows {
        volume_serial: u64,
        file_id: [u8; 16],
    },
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum EntryKind {
    RegularFile,
    Directory,
    SymlinkOrReparse,
    Fifo,
    Socket,
    BlockDevice,
    CharacterDevice,
    Other,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DirectoryAccess {
    Read,
    MutateChildren,
    Stage,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FileAccess {
    Read,
    ReadWrite,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CleanupAccess {
    Delete,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CreatePermissions {
    Inherit,
    OwnerOnly,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CaseMode {
    Sensitive,
    Insensitive,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LinuxFilesystem {
    Ext,
    Xfs,
    Btrfs,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum LocalFilesystemSnapshot {
    Linux {
        family: LinuxFilesystem,
        fsid: u64,
        device: u64,
    },
    MacOs {
        type_name: [u8; 16],
        fsid: u64,
        device: u64,
    },
    Windows {
        volume_guid: Vec<u16>,
        serial: u64,
    },
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
pub(crate) enum ChildState {
    Absent,
    Present(EntryMetadata),
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum RawLinkTarget {
    Unix(Vec<u8>),
    Windows { tag: u32, bytes: Vec<u8> },
}

pub(super) struct NamespaceAnchor {
    pub(super) native: platform::NativeNamespaceAnchor,
}

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

pub(super) struct CleanupEntry {
    pub(super) parent: DirectoryAuthority,
    pub(super) native: platform::NativeFile,
    pub(super) name: ComponentName,
    pub(super) opened: EntryMetadata,
    pub(super) access: CleanupAccess,
}

pub(super) enum CleanupCapability {
    Entry(Box<CleanupEntry>),
    Directory(Box<QuarantinedCapability>),
}

impl fmt::Debug for DirectoryAuthority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("DirectoryAuthority(<redacted>)")
    }
}
impl fmt::Debug for FileCapability {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("FileCapability(<redacted>)")
    }
}
impl fmt::Debug for StageCapability {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("StageCapability(<redacted>)")
    }
}
impl fmt::Debug for QuarantinedCapability {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("QuarantinedCapability(<redacted>)")
    }
}
impl fmt::Debug for CleanupCapability {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("CleanupCapability(<redacted>)")
    }
}

impl DirectoryAuthority {
    pub(crate) fn access(&self) -> DirectoryAccess {
        self.access
    }
    pub(crate) fn opened_metadata(&self) -> &EntryMetadata {
        &self.opened
    }
    pub(crate) fn namespace_snapshot(&self) -> &NamespaceSnapshot {
        &self.snapshot
    }
}

impl StageCapability {
    pub(crate) fn directory(&self) -> &DirectoryAuthority {
        &self.directory
    }
    pub(crate) fn opened_metadata(&self) -> &EntryMetadata {
        &self.opened
    }
}

impl QuarantinedCapability {
    pub(crate) fn directory(&self) -> &DirectoryAuthority {
        &self.directory
    }
    pub(crate) fn opened_metadata(&self) -> &EntryMetadata {
        &self.opened
    }
}

impl FileCapability {
    pub(crate) fn opened_metadata(&self) -> &EntryMetadata {
        &self.opened
    }
    pub(crate) fn metadata(&self) -> Result<EntryMetadata> {
        platform::metadata_from_file(&self.native)
    }
    pub(crate) fn read(&mut self, buffer: &mut [u8]) -> Result<usize> {
        platform::read_file(&mut self.native, buffer)
    }
    pub(crate) fn write_all(&mut self, mut buffer: &[u8]) -> Result<()> {
        if self.access != FileAccess::ReadWrite {
            return Err(SafeFsError::AccessMismatch {
                operation: SafeFsOperation::WriteFile,
            });
        }
        while !buffer.is_empty() {
            let written = platform::write_file(&mut self.native, buffer)?;
            if written == 0 {
                return Err(SafeFsError::InvalidNativeBuffer {
                    operation: SafeFsOperation::WriteFile,
                    reason: super::error::NativeBufferReason::WriteZero,
                });
            }
            buffer = &buffer[written..];
        }
        Ok(())
    }
    pub(crate) fn seek(&mut self, position: SeekFrom) -> Result<u64> {
        platform::seek_file(&mut self.native, position)
    }
    pub(crate) fn flush(&mut self) -> Result<()> {
        platform::flush_file(&mut self.native)
    }
    pub(crate) fn sync_all(&self) -> Result<()> {
        platform::sync_file(&self.native)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CopyOutcome {
    pub(crate) bytes_copied: u64,
    pub(crate) source_after: EntryMetadata,
    pub(crate) destination_after: EntryMetadata,
}

pub(crate) fn stream_copy_file(
    source: &mut FileCapability,
    destination: &mut FileCapability,
    expected_source: &StableIdentity,
    byte_limit: u64,
) -> Result<CopyOutcome> {
    let source_before = source.metadata()?;
    let destination_before = destination.metadata()?;
    if source_before.kind != EntryKind::RegularFile {
        return Err(SafeFsError::UnsupportedEntryType {
            operation: SafeFsOperation::CopyRead,
            kind: source_before.kind,
        });
    }
    if destination_before.kind != EntryKind::RegularFile {
        return Err(SafeFsError::UnsupportedEntryType {
            operation: SafeFsOperation::CopyWrite,
            kind: destination_before.kind,
        });
    }
    if &source_before.identity != expected_source {
        return Err(SafeFsError::IdentityChanged {
            operation: SafeFsOperation::CopyRead,
            expected: expected_source.clone(),
            actual: source_before.identity,
        });
    }
    if source_before.len > byte_limit {
        return Err(SafeFsError::CopyLimitExceeded { limit: byte_limit });
    }
    source.seek(SeekFrom::Start(0))?;
    destination.seek(SeekFrom::Start(0))?;
    let mut remaining = source_before.len;
    let mut copied = 0_u64;
    let mut bytes = [0_u8; 65_536];
    while remaining != 0 {
        let request = usize::try_from(remaining.min(bytes.len() as u64))
            .expect("request is bounded by buffer length");
        let read = source.read(&mut bytes[..request])?;
        if read == 0 {
            return Err(SafeFsError::UnexpectedCopyEof);
        }
        destination.write_all(&bytes[..read])?;
        copied += read as u64;
        remaining -= read as u64;
    }
    let mut extra = [0_u8; 1];
    if source.read(&mut extra)? != 0 {
        return Err(SafeFsError::CopyLimitExceeded { limit: byte_limit });
    }
    destination.flush()?;
    destination.sync_all()?;
    let source_after = source.metadata()?;
    let destination_after = destination.metadata()?;
    if source_after.identity != source_before.identity {
        return Err(SafeFsError::IdentityChanged {
            operation: SafeFsOperation::CopyRead,
            expected: source_before.identity,
            actual: source_after.identity,
        });
    }
    if destination_after.identity != destination_before.identity {
        return Err(SafeFsError::IdentityChanged {
            operation: SafeFsOperation::CopyWrite,
            expected: destination_before.identity,
            actual: destination_after.identity,
        });
    }
    if source_after.kind != EntryKind::RegularFile
        || source_after.len != source_before.len
        || source_after.link_count != source_before.link_count
    {
        return Err(SafeFsError::io(
            SafeFsOperation::CopyRead,
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "source metadata changed during copy",
            ),
        ));
    }
    if destination_after.kind != EntryKind::RegularFile || destination_after.len != copied {
        return Err(SafeFsError::io(
            SafeFsOperation::CopyWrite,
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "destination metadata does not match copied bytes",
            ),
        ));
    }
    Ok(CopyOutcome {
        bytes_copied: copied,
        source_after,
        destination_after,
    })
}
