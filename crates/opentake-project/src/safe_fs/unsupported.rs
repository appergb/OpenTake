use super::capability::*;
use super::component::ComponentName;
use super::error::{Result, SafeFsError, SafeFsOperation, SecureFilesystemReason};
use std::io::SeekFrom;
use std::path::Path;

pub(super) struct NativeNamespaceAnchor;
pub(super) struct NativeDirectory;
pub(super) struct NativeFile;

fn unsupported<T>(operation: SafeFsOperation) -> Result<T> {
    Err(SafeFsError::UnsupportedSecureFilesystem {
        operation,
        reason: SecureFilesystemReason::UnsupportedTarget,
    })
}

pub(super) fn capture_absolute_directory(
    _: &Path,
    _: DirectoryAccess,
) -> Result<DirectoryAuthority> {
    unsupported(SafeFsOperation::CaptureNamespaceRoot)
}
pub(super) fn revalidate_namespace(_: &DirectoryAuthority) -> Result<()> {
    unsupported(SafeFsOperation::RevalidateNamespace)
}
pub(super) fn query_child_nofollow(
    _: &DirectoryAuthority,
    _: &ComponentName,
) -> Result<ChildState> {
    unsupported(SafeFsOperation::QueryChild)
}
pub(super) fn open_dir_nofollow(
    _: &DirectoryAuthority,
    _: &ComponentName,
    _: DirectoryAccess,
) -> Result<DirectoryAuthority> {
    unsupported(SafeFsOperation::OpenDirectory)
}
pub(super) fn open_file_nofollow(
    _: &DirectoryAuthority,
    _: &ComponentName,
    _: FileAccess,
) -> Result<FileCapability> {
    unsupported(SafeFsOperation::OpenFile)
}
pub(super) fn create_dir_new(
    _: &DirectoryAuthority,
    _: &ComponentName,
    _: CreatePermissions,
    _: DirectoryAccess,
) -> Result<DirectoryAuthority> {
    unsupported(SafeFsOperation::CreateDirectory)
}
pub(super) fn create_stage_dir_new(
    _: &DirectoryAuthority,
    _: &ComponentName,
    _: CreatePermissions,
) -> Result<StageCapability> {
    unsupported(SafeFsOperation::CreateStageDirectory)
}
pub(super) fn create_file_new(
    _: &DirectoryAuthority,
    _: &ComponentName,
    _: CreatePermissions,
) -> Result<FileCapability> {
    unsupported(SafeFsOperation::CreateFile)
}
pub(super) fn enumerate(_: &DirectoryAuthority) -> Result<Vec<ComponentName>> {
    unsupported(SafeFsOperation::EnumerateDirectory)
}
pub(super) fn read_link_component(
    _: &DirectoryAuthority,
    _: &ComponentName,
) -> Result<RawLinkTarget> {
    unsupported(SafeFsOperation::ReadLink)
}
pub(super) fn metadata_from_file(_: &NativeFile) -> Result<EntryMetadata> {
    unsupported(SafeFsOperation::QueryMetadata)
}
pub(super) fn read_file(_: &mut NativeFile, _: &mut [u8]) -> Result<usize> {
    unsupported(SafeFsOperation::ReadFile)
}
pub(super) fn write_file(_: &mut NativeFile, _: &[u8]) -> Result<usize> {
    unsupported(SafeFsOperation::WriteFile)
}
pub(super) fn seek_file(_: &mut NativeFile, _: SeekFrom) -> Result<u64> {
    unsupported(SafeFsOperation::SeekFile)
}
pub(super) fn flush_file(_: &mut NativeFile) -> Result<()> {
    unsupported(SafeFsOperation::FlushFile)
}
pub(super) fn sync_file(_: &NativeFile) -> Result<()> {
    unsupported(SafeFsOperation::SyncFile)
}
pub(super) fn quarantine_stage(
    _: StageCapability,
    _: &DirectoryAuthority,
    _: ComponentName,
) -> Result<QuarantinedCapability> {
    unsupported(SafeFsOperation::QuarantineNoReplace)
}
pub(super) fn publish_stage_noreplace(
    _: StageCapability,
    _: &DirectoryAuthority,
    _: ComponentName,
) -> Result<()> {
    unsupported(SafeFsOperation::PublishNoReplace)
}
pub(super) fn open_cleanup_child_nofollow(
    _: &QuarantinedCapability,
    _: &ComponentName,
) -> Result<CleanupCapability> {
    unsupported(SafeFsOperation::OpenCleanupEntry)
}
pub(super) fn delete_quarantined_entry(_: CleanupCapability) -> Result<()> {
    unsupported(SafeFsOperation::DeleteQuarantinedEntry)
}
pub(super) fn delete_quarantined_empty_directory(_: QuarantinedCapability) -> Result<()> {
    unsupported(SafeFsOperation::DeleteQuarantinedEmptyDirectory)
}
