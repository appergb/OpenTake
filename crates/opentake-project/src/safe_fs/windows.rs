#![deny(unsafe_op_in_unsafe_fn)]

use super::capability::*;
use super::component::ComponentName;
use super::error::*;
use std::io::SeekFrom;
use std::path::Path;

pub(super) struct NativeNamespaceAnchor;
pub(super) struct NativeDirectory;
pub(super) struct NativeFile;

fn filesystem_refusal<T>(operation: SafeFsOperation) -> Result<T> {
    Err(SafeFsError::UnsupportedSecureFilesystem {
        operation,
        reason: SecureFilesystemReason::UnsupportedTarget,
    })
}

fn mutation_refusal<T>(operation: SafeFsOperation) -> Result<T> {
    Err(SafeFsError::UnsupportedAtomicPublish {
        operation,
        reason: AtomicPublishReason::PrimitiveUnavailable,
    })
}

pub(super) fn capture_absolute_directory(
    _: &Path,
    _: DirectoryAccess,
) -> Result<DirectoryAuthority> {
    filesystem_refusal(SafeFsOperation::CaptureNamespaceRoot)
}

pub(super) fn revalidate_namespace(_: &DirectoryAuthority) -> Result<()> {
    filesystem_refusal(SafeFsOperation::RevalidateNamespace)
}

pub(super) fn query_child_nofollow(
    _: &DirectoryAuthority,
    _: &ComponentName,
) -> Result<ChildState> {
    filesystem_refusal(SafeFsOperation::QueryChild)
}

pub(super) fn open_dir_nofollow(
    _: &DirectoryAuthority,
    _: &ComponentName,
    _: DirectoryAccess,
) -> Result<DirectoryAuthority> {
    filesystem_refusal(SafeFsOperation::OpenDirectory)
}

pub(super) fn open_file_nofollow(
    _: &DirectoryAuthority,
    _: &ComponentName,
    _: FileAccess,
) -> Result<FileCapability> {
    filesystem_refusal(SafeFsOperation::OpenFile)
}

pub(super) fn create_dir_new(
    _: &DirectoryAuthority,
    _: &ComponentName,
    _: CreatePermissions,
    _: DirectoryAccess,
) -> Result<DirectoryAuthority> {
    filesystem_refusal(SafeFsOperation::CreateDirectory)
}

pub(super) fn create_stage_dir_new(
    _: &DirectoryAuthority,
    _: &ComponentName,
    _: CreatePermissions,
) -> Result<StageCapability> {
    mutation_refusal(SafeFsOperation::CreateStageDirectory)
}

pub(super) fn create_file_new(
    _: &DirectoryAuthority,
    _: &ComponentName,
    _: CreatePermissions,
) -> Result<FileCapability> {
    filesystem_refusal(SafeFsOperation::CreateFile)
}

pub(super) fn enumerate(_: &DirectoryAuthority) -> Result<Vec<ComponentName>> {
    filesystem_refusal(SafeFsOperation::EnumerateDirectory)
}

pub(super) fn read_link_component(
    _: &DirectoryAuthority,
    _: &ComponentName,
) -> Result<RawLinkTarget> {
    filesystem_refusal(SafeFsOperation::ReadLink)
}

pub(super) fn metadata_from_file(_: &NativeFile) -> Result<EntryMetadata> {
    filesystem_refusal(SafeFsOperation::QueryMetadata)
}

pub(super) fn read_file(_: &mut NativeFile, _: &mut [u8]) -> Result<usize> {
    filesystem_refusal(SafeFsOperation::ReadFile)
}

pub(super) fn write_file(_: &mut NativeFile, _: &[u8]) -> Result<usize> {
    filesystem_refusal(SafeFsOperation::WriteFile)
}

pub(super) fn seek_file(_: &mut NativeFile, _: SeekFrom) -> Result<u64> {
    filesystem_refusal(SafeFsOperation::SeekFile)
}

pub(super) fn flush_file(_: &mut NativeFile) -> Result<()> {
    filesystem_refusal(SafeFsOperation::FlushFile)
}

pub(super) fn sync_file(_: &NativeFile) -> Result<()> {
    filesystem_refusal(SafeFsOperation::SyncFile)
}

pub(super) fn quarantine_stage(
    _: StageCapability,
    _: &DirectoryAuthority,
    _: ComponentName,
) -> Result<QuarantinedCapability> {
    mutation_refusal(SafeFsOperation::QuarantineNoReplace)
}

pub(super) fn publish_stage_noreplace(
    _: StageCapability,
    _: &DirectoryAuthority,
    _: ComponentName,
) -> Result<()> {
    mutation_refusal(SafeFsOperation::PublishNoReplace)
}

pub(super) fn open_cleanup_child_nofollow(
    _: &QuarantinedCapability,
    _: &ComponentName,
) -> Result<CleanupCapability> {
    filesystem_refusal(SafeFsOperation::OpenCleanupEntry)
}

pub(super) fn delete_quarantined_entry(_: CleanupCapability) -> Result<()> {
    filesystem_refusal(SafeFsOperation::DeleteQuarantinedEntry)
}

pub(super) fn delete_quarantined_empty_directory(_: QuarantinedCapability) -> Result<()> {
    filesystem_refusal(SafeFsOperation::DeleteQuarantinedEmptyDirectory)
}
