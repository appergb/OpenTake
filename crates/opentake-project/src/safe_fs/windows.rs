#![deny(unsafe_op_in_unsafe_fn)]

use super::capability::*;
use super::component::ComponentName;
use super::error::*;
use std::io::SeekFrom;
use std::path::Path;

pub(super) struct NativeNamespaceAnchor;
pub(super) struct NativeDirectory;
pub(super) struct NativeFile;

#[cfg(test)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum WindowsCreateFailurePoint {
    Metadata,
    FilesystemProbe,
    TypeValidation,
    CaseProof,
    SnapshotAssembly,
    ParentDuplicate,
}

#[cfg(test)]
static WINDOWS_CREATE_FAILURE: std::sync::OnceLock<
    std::sync::Mutex<Option<WindowsCreateFailurePoint>>,
> = std::sync::OnceLock::new();

#[cfg(test)]
struct WindowsCreateFailureGuard;

#[cfg(test)]
impl Drop for WindowsCreateFailureGuard {
    fn drop(&mut self) {
        *WINDOWS_CREATE_FAILURE
            .get_or_init(Default::default)
            .lock()
            .expect("Windows create-failure mutex poisoned") = None;
    }
}

#[cfg(test)]
fn install_windows_create_failure(point: WindowsCreateFailurePoint) -> WindowsCreateFailureGuard {
    let mut slot = WINDOWS_CREATE_FAILURE
        .get_or_init(Default::default)
        .lock()
        .expect("Windows create-failure mutex poisoned");
    assert!(
        slot.is_none(),
        "Windows create-failure tests require --test-threads=1"
    );
    *slot = Some(point);
    WindowsCreateFailureGuard
}

#[cfg(test)]
static FAIL_NEXT_CREATED_DISPOSITION: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

#[cfg(test)]
struct CreatedDispositionFailureGuard;

#[cfg(test)]
impl Drop for CreatedDispositionFailureGuard {
    fn drop(&mut self) {
        FAIL_NEXT_CREATED_DISPOSITION.store(false, std::sync::atomic::Ordering::SeqCst);
    }
}

#[cfg(test)]
fn install_created_disposition_failure() -> CreatedDispositionFailureGuard {
    assert!(
        !FAIL_NEXT_CREATED_DISPOSITION.swap(true, std::sync::atomic::Ordering::SeqCst),
        "created disposition tests require --test-threads=1"
    );
    CreatedDispositionFailureGuard
}

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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    struct TestDir(PathBuf);

    impl TestDir {
        fn new(label: &str) -> Self {
            static NEXT: AtomicU64 = AtomicU64::new(0);
            let id = NEXT.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "opentake-c1b-win-{label}-{}-{id}",
                std::process::id()
            ));
            fs::create_dir(&path).expect("create Windows fixture root");
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn name(value: &str) -> ComponentName {
        ComponentName::new(value).expect("valid fixture name")
    }

    fn root(dir: &TestDir) -> DirectoryAuthority {
        capture_absolute_directory(dir.path(), DirectoryAccess::MutateChildren)
            .expect("capture fixture root")
    }

    #[test]
    fn nested_retained_io_roundtrip() {
        let temp = TestDir::new("nested");
        let authority = root(&temp);
        let a = create_dir_new(
            &authority,
            &name("a"),
            CreatePermissions::Inherit,
            DirectoryAccess::MutateChildren,
        )
        .unwrap();
        let b = create_dir_new(
            &a,
            &name("b"),
            CreatePermissions::Inherit,
            DirectoryAccess::MutateChildren,
        )
        .unwrap();
        let mut file = create_file_new(&b, &name("data"), CreatePermissions::Inherit).unwrap();
        file.write_all(b"retained").unwrap();
        file.flush().unwrap();
        file.sync_all().unwrap();
        file.seek(SeekFrom::Start(0)).unwrap();
        let mut output = [0u8; 8];
        assert_eq!(file.read(&mut output).unwrap(), 8);
        assert_eq!(&output, b"retained");
        assert_eq!(enumerate(&b).unwrap(), vec![name("data")]);
    }

    fn assert_file_create_failure_rolls_back(point: WindowsCreateFailurePoint, label: &str) {
        let temp = TestDir::new(label);
        let authority = root(&temp);
        let _failure = install_windows_create_failure(point);
        assert!(create_file_new(&authority, &name("created"), CreatePermissions::Inherit).is_err());
        assert!(matches!(
            query_child_nofollow(&authority, &name("created")).unwrap(),
            ChildState::Absent
        ));
    }

    #[test]
    fn windows_post_create_metadata_failure_rolls_back_same_handle() {
        assert_file_create_failure_rolls_back(
            WindowsCreateFailurePoint::Metadata,
            "rollback-metadata",
        );

        let temp = TestDir::new("rollback-disposition-failure");
        let authority = root(&temp);
        let _failure = install_windows_create_failure(WindowsCreateFailurePoint::Metadata);
        let _disposition = install_created_disposition_failure();
        let error = match create_file_new(&authority, &name("created"), CreatePermissions::Inherit)
        {
            Ok(_) => panic!("injected disposition failure must reject the created file"),
            Err(error) => error,
        };
        assert!(
            matches!(
                &error,
                SafeFsError::StageIdentityLost {
                    operation: SafeFsOperation::RollbackCreatedEntry,
                    reason: StageIdentityLostReason::CreatedRollbackDeleteFailed,
                }
            ),
            "unexpected error: {error:?}"
        );
        assert!(
            matches!(
                query_child_nofollow(&authority, &name("created")).unwrap(),
                ChildState::Present(_)
            ),
            "failed retained-HANDLE disposition must fail-leak the created entry"
        );
    }

    #[test]
    fn windows_post_create_filesystem_failure_rolls_back_same_handle() {
        assert_file_create_failure_rolls_back(
            WindowsCreateFailurePoint::FilesystemProbe,
            "rollback-filesystem",
        );
    }

    #[test]
    fn windows_post_create_type_failure_rolls_back_same_handle() {
        assert_file_create_failure_rolls_back(
            WindowsCreateFailurePoint::TypeValidation,
            "rollback-type",
        );
    }

    #[test]
    fn windows_post_create_case_failure_rolls_back_same_handle() {
        let temp = TestDir::new("rollback-case");
        let authority = root(&temp);
        let _failure = install_windows_create_failure(WindowsCreateFailurePoint::CaseProof);
        assert!(create_dir_new(
            &authority,
            &name("created"),
            CreatePermissions::Inherit,
            DirectoryAccess::MutateChildren,
        )
        .is_err());
        assert!(matches!(
            query_child_nofollow(&authority, &name("created")).unwrap(),
            ChildState::Absent
        ));
    }

    #[test]
    fn windows_post_create_snapshot_failure_rolls_back_same_handle() {
        let temp = TestDir::new("rollback-snapshot");
        let authority = root(&temp);
        let _failure = install_windows_create_failure(WindowsCreateFailurePoint::SnapshotAssembly);
        assert!(create_dir_new(
            &authority,
            &name("created"),
            CreatePermissions::Inherit,
            DirectoryAccess::MutateChildren,
        )
        .is_err());
        assert!(matches!(
            query_child_nofollow(&authority, &name("created")).unwrap(),
            ChildState::Absent
        ));
    }

    #[test]
    fn windows_post_create_parent_duplicate_failure_rolls_back_same_handle() {
        let temp = TestDir::new("rollback-duplicate");
        let authority = root(&temp);
        let _failure = install_windows_create_failure(WindowsCreateFailurePoint::ParentDuplicate);
        assert!(
            create_stage_dir_new(&authority, &name("created"), CreatePermissions::Inherit).is_err()
        );
        assert!(matches!(
            query_child_nofollow(&authority, &name("created")).unwrap(),
            ChildState::Absent
        ));
    }

    #[test]
    fn read_parent_cannot_escalate_child_directory_access() {
        let temp = TestDir::new("read-child-access");
        fs::create_dir(temp.path().join("child")).expect("create child");
        let authority = capture_absolute_directory(temp.path(), DirectoryAccess::Read)
            .expect("capture read authority");
        open_dir_nofollow(&authority, &name("child"), DirectoryAccess::Read)
            .expect("read parent may open read child");
        assert!(matches!(
            open_dir_nofollow(&authority, &name("child"), DirectoryAccess::MutateChildren),
            Err(SafeFsError::AccessMismatch {
                operation: SafeFsOperation::OpenDirectory
            })
        ));
    }

    #[test]
    fn read_parent_cannot_escalate_file_access() {
        let temp = TestDir::new("read-file-access");
        fs::write(temp.path().join("leaf"), b"payload").expect("create file");
        let authority = capture_absolute_directory(temp.path(), DirectoryAccess::Read)
            .expect("capture read authority");
        let mut read_file = open_file_nofollow(&authority, &name("leaf"), FileAccess::Read)
            .expect("read parent may open read file");
        assert!(matches!(
            read_file.write_all(b"forbidden"),
            Err(SafeFsError::AccessMismatch {
                operation: SafeFsOperation::WriteFile
            })
        ));
        assert_eq!(
            fs::read(temp.path().join("leaf")).expect("read unchanged file"),
            b"payload"
        );
        assert!(matches!(
            open_file_nofollow(&authority, &name("leaf"), FileAccess::ReadWrite),
            Err(SafeFsError::AccessMismatch {
                operation: SafeFsOperation::OpenFile
            })
        ));
    }

    #[test]
    fn read_parent_cannot_create_children() {
        let temp = TestDir::new("read-create-access");
        let authority = capture_absolute_directory(temp.path(), DirectoryAccess::Read)
            .expect("capture read authority");

        assert!(matches!(
            create_dir_new(
                &authority,
                &name("directory"),
                CreatePermissions::Inherit,
                DirectoryAccess::Read,
            ),
            Err(SafeFsError::AccessMismatch {
                operation: SafeFsOperation::CreateDirectory
            })
        ));
        assert!(!temp.path().join("directory").exists());

        assert!(matches!(
            create_stage_dir_new(&authority, &name("stage"), CreatePermissions::Inherit),
            Err(SafeFsError::AccessMismatch {
                operation: SafeFsOperation::CreateStageDirectory
            })
        ));
        assert!(!temp.path().join("stage").exists());

        assert!(matches!(
            create_file_new(&authority, &name("file"), CreatePermissions::Inherit),
            Err(SafeFsError::AccessMismatch {
                operation: SafeFsOperation::CreateFile
            })
        ));
        assert!(!temp.path().join("file").exists());
    }

    #[test]
    fn stage_access_is_internal_only() {
        let temp = TestDir::new("stage-access");
        assert!(matches!(
            capture_absolute_directory(temp.path(), DirectoryAccess::Stage),
            Err(SafeFsError::AccessMismatch {
                operation: SafeFsOperation::CaptureNamespaceRoot
            })
        ));

        fs::create_dir(temp.path().join("child")).expect("create child");
        let authority = root(&temp);
        assert!(matches!(
            open_dir_nofollow(&authority, &name("child"), DirectoryAccess::Stage),
            Err(SafeFsError::AccessMismatch {
                operation: SafeFsOperation::OpenDirectory
            })
        ));
        assert!(matches!(
            create_dir_new(
                &authority,
                &name("created-stage"),
                CreatePermissions::Inherit,
                DirectoryAccess::Stage,
            ),
            Err(SafeFsError::AccessMismatch {
                operation: SafeFsOperation::CreateDirectory
            })
        ));
        assert!(!temp.path().join("created-stage").exists());
    }
}
