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
            for digit in ['1', '2', '3', '4', '5', '6', '7', '8', '9', '¹', '²', '³'] {
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

#[cfg(any(target_os = "linux", target_os = "macos"))]
mod unix_contract {
    use super::super::capability::CleanupCapability;
    use super::super::ops::{delete_quarantined_entry, open_cleanup_child_nofollow};
    use super::super::test_seam::{self, HookPoint, RaceGate};
    use super::super::*;
    use std::path::{Path, PathBuf};
    use std::sync::Arc;

    struct TestDir(PathBuf);
    impl TestDir {
        fn new(label: &str) -> Self {
            static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
            let id = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let requested = std::env::temp_dir()
                .join(format!("opentake-c1b-{label}-{}-{id}", std::process::id()));
            std::fs::create_dir(&requested).expect("create fixture");
            let path = std::fs::canonicalize(&requested).expect("canonical fixture anchor");
            Self(path)
        }
        fn path(&self) -> &Path {
            &self.0
        }
    }
    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    fn name(value: &str) -> ComponentName {
        ComponentName::new(value).expect("valid fixture component")
    }
    fn present_identity(parent: &DirectoryAuthority, value: &str) -> StableIdentity {
        match query_child_nofollow(parent, &name(value)).expect("query fixture child") {
            ChildState::Present(metadata) => metadata.identity,
            ChildState::Absent => panic!("expected present fixture child"),
        }
    }
    fn assert_absent(parent: &DirectoryAuthority, value: &str) {
        assert!(
            matches!(
                query_child_nofollow(parent, &name(value)),
                Ok(ChildState::Absent)
            ),
            "{value} must be absent after ordinary post-create rollback"
        );
    }

    fn assert_probe_rejected(sample: test_seam::UnixProbeSample, expected: SecureFilesystemReason) {
        let temp = TestDir::new("probe-rejected");
        let _guard = test_seam::install_unix_probe(sample);
        let error = capture_absolute_directory(temp.path(), DirectoryAccess::Read)
            .expect_err("probe must reject capture");
        assert!(
            matches!(error, SafeFsError::UnsupportedSecureFilesystem { reason, .. } if reason == expected)
        );
    }

    #[cfg(target_os = "linux")]
    fn linux_sample(
        magic: i64,
        fsid: u64,
        device: u64,
        ext_flags: std::result::Result<i64, SecureFilesystemReason>,
    ) -> test_seam::UnixProbeSample {
        test_seam::UnixProbeSample::Linux {
            magic,
            fsid,
            device,
            ext_flags,
        }
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_filesystem_and_case_probe_matrix_is_enforced() {
        let _serial = test_seam::serialize_unix_test();
        use super::super::capability::LinuxFilesystem;
        const EXT: i64 = 0x0000_ef53;
        const XFS: i64 = 0x5846_5342;
        const BTRFS: i64 = 0x9123_683e;
        const TMPFS: i64 = 0x0102_1994;
        const NFS: i64 = 0x0000_6969;
        const CASEFOLD: i64 = 0x4000_0000;
        for (magic, expected) in [
            (EXT, LinuxFilesystem::Ext),
            (XFS, LinuxFilesystem::Xfs),
            (BTRFS, LinuxFilesystem::Btrfs),
        ] {
            let temp = TestDir::new("linux-accepted");
            let _guard = test_seam::install_unix_probe(linux_sample(magic, 7, 11, Ok(0)));
            let authority = capture_absolute_directory(temp.path(), DirectoryAccess::Read)
                .expect("accepted Linux family");
            assert!(
                matches!(&authority.namespace_snapshot().root_filesystem, LocalFilesystemSnapshot::Linux { family, fsid: 7, device: 11 } if *family == expected)
            );
            assert_eq!(
                authority.namespace_snapshot().root_case_mode,
                CaseMode::Sensitive
            );
        }
        // tmpfs is fail-closed until a separately reviewed native case-semantics proof exists.
        assert_probe_rejected(
            linux_sample(TMPFS, 7, 11, Ok(0)),
            SecureFilesystemReason::UnknownFilesystem,
        );
        {
            let temp = TestDir::new("linux-casefold");
            let _guard = test_seam::install_unix_probe(linux_sample(EXT, 7, 11, Ok(CASEFOLD)));
            let authority = capture_absolute_directory(temp.path(), DirectoryAccess::Read)
                .expect("ext casefold probe");
            assert_eq!(
                authority.namespace_snapshot().root_case_mode,
                CaseMode::Insensitive
            );
        }
        assert_probe_rejected(
            linux_sample(0x7fff_ffff, 7, 11, Ok(0)),
            SecureFilesystemReason::UnknownFilesystem,
        );
        assert_probe_rejected(
            linux_sample(NFS, 7, 11, Ok(0)),
            SecureFilesystemReason::RemoteFilesystem,
        );
        assert_probe_rejected(
            test_seam::UnixProbeSample::Failure(SecureFilesystemReason::FilesystemProbeUnavailable),
            SecureFilesystemReason::FilesystemProbeUnavailable,
        );
        assert_probe_rejected(
            linux_sample(
                EXT,
                7,
                11,
                Err(SecureFilesystemReason::CaseSemanticsUnavailable),
            ),
            SecureFilesystemReason::CaseSemanticsUnavailable,
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_revalidation_rejects_fsid_device_and_case_changes() {
        let _serial = test_seam::serialize_unix_test();
        const EXT: i64 = 0x0000_ef53;
        const CASEFOLD: i64 = 0x4000_0000;
        let temp = TestDir::new("linux-probe-change");
        let baseline = linux_sample(EXT, 7, 11, Ok(0));
        let guard = test_seam::install_unix_probe(baseline.clone());

        let fsid_authority = capture_absolute_directory(temp.path(), DirectoryAccess::Read)
            .expect("capture fsid baseline");
        guard.replace(linux_sample(EXT, 8, 11, Ok(0)));
        assert!(matches!(
            revalidate_namespace(&fsid_authority),
            Err(SafeFsError::NamespaceChanged { .. })
        ));

        guard.replace(baseline.clone());
        let device_authority = capture_absolute_directory(temp.path(), DirectoryAccess::Read)
            .expect("capture device baseline");
        guard.replace(linux_sample(EXT, 7, 12, Ok(0)));
        assert!(matches!(
            revalidate_namespace(&device_authority),
            Err(SafeFsError::NamespaceChanged { .. })
        ));

        guard.replace(baseline);
        let case_authority = capture_absolute_directory(temp.path(), DirectoryAccess::Read)
            .expect("capture case baseline");
        guard.replace(linux_sample(EXT, 7, 11, Ok(CASEFOLD)));
        assert!(matches!(
            revalidate_namespace(&case_authority),
            Err(SafeFsError::NamespaceChanged { .. })
        ));
    }

    #[cfg(target_os = "macos")]
    fn macos_sample(
        mount_flags: u32,
        fsid: u64,
        device: u64,
        case_sensitive: i64,
    ) -> test_seam::UnixProbeSample {
        test_seam::UnixProbeSample::MacOs {
            mount_flags,
            type_name: *b"apfs\0\0\0\0\0\0\0\0\0\0\0\0",
            fsid,
            device,
            case_sensitive,
        }
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_local_and_case_probe_matrix_is_enforced() {
        let _serial = test_seam::serialize_unix_test();
        const LOCAL: u32 = 0x0000_1000;
        for (raw_case, expected) in [(1, CaseMode::Sensitive), (0, CaseMode::Insensitive)] {
            let temp = TestDir::new("macos-local");
            let _guard = test_seam::install_unix_probe(macos_sample(LOCAL, 7, 11, raw_case));
            let authority = capture_absolute_directory(temp.path(), DirectoryAccess::Read)
                .expect("MNT_LOCAL and _PC_CASE_SENSITIVE accepted");
            assert!(matches!(
                &authority.namespace_snapshot().root_filesystem,
                LocalFilesystemSnapshot::MacOs {
                    fsid: 7,
                    device: 11,
                    ..
                }
            ));
            assert_eq!(authority.namespace_snapshot().root_case_mode, expected);
        }
        assert_probe_rejected(
            macos_sample(0, 7, 11, 1),
            SecureFilesystemReason::RemoteFilesystem,
        );
        assert_probe_rejected(
            test_seam::UnixProbeSample::Failure(SecureFilesystemReason::FilesystemProbeUnavailable),
            SecureFilesystemReason::FilesystemProbeUnavailable,
        );
        assert_probe_rejected(
            macos_sample(LOCAL, 7, 11, -1),
            SecureFilesystemReason::CaseSemanticsUnavailable,
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_revalidation_rejects_fsid_device_and_case_changes() {
        let _serial = test_seam::serialize_unix_test();
        const LOCAL: u32 = 0x0000_1000;
        let temp = TestDir::new("macos-probe-change");
        let baseline = macos_sample(LOCAL, 7, 11, 1);
        let guard = test_seam::install_unix_probe(baseline.clone());

        let fsid_authority = capture_absolute_directory(temp.path(), DirectoryAccess::Read)
            .expect("capture fsid baseline");
        guard.replace(macos_sample(LOCAL, 8, 11, 1));
        assert!(matches!(
            revalidate_namespace(&fsid_authority),
            Err(SafeFsError::NamespaceChanged { .. })
        ));

        guard.replace(baseline.clone());
        let device_authority = capture_absolute_directory(temp.path(), DirectoryAccess::Read)
            .expect("capture device baseline");
        guard.replace(macos_sample(LOCAL, 7, 12, 1));
        assert!(matches!(
            revalidate_namespace(&device_authority),
            Err(SafeFsError::NamespaceChanged { .. })
        ));

        guard.replace(baseline);
        let case_authority = capture_absolute_directory(temp.path(), DirectoryAccess::Read)
            .expect("capture case baseline");
        guard.replace(macos_sample(LOCAL, 7, 11, 0));
        assert!(matches!(
            revalidate_namespace(&case_authority),
            Err(SafeFsError::NamespaceChanged { .. })
        ));
    }

    #[test]
    fn recursive_authority_revalidates_anchor_and_entire_child_scope() {
        let _serial = test_seam::serialize_unix_test();
        let temp = TestDir::new("scope");
        std::fs::create_dir_all(temp.path().join("a/b")).expect("create tree");
        let root = capture_absolute_directory(temp.path(), DirectoryAccess::MutateChildren)
            .expect("capture");
        let a = open_dir_nofollow(&root, &name("a"), DirectoryAccess::Read).expect("open a");
        let b = open_dir_nofollow(&a, &name("b"), DirectoryAccess::Read).expect("open b");
        assert_eq!(
            b.namespace_snapshot().components.len(),
            root.namespace_snapshot().components.len() + 2
        );
        revalidate_namespace(&b).expect("full rewalk");
        std::fs::rename(temp.path().join("a/b"), temp.path().join("a/b-retained"))
            .expect("move child scope");
        std::fs::create_dir(temp.path().join("a/b")).expect("replace child scope");
        assert!(matches!(
            revalidate_namespace(&b),
            Err(SafeFsError::NamespaceChanged {
                operation: SafeFsOperation::RevalidateNamespace
            })
        ));
    }

    #[test]
    fn query_symlink_fifo_and_special_entries_is_nonblocking_present_metadata() {
        let _serial = test_seam::serialize_unix_test();
        use std::os::unix::fs::symlink;
        use std::process::Command;
        let temp = TestDir::new("query-special");
        symlink("missing-target", temp.path().join("link")).expect("symlink");
        let status = Command::new("mkfifo")
            .arg(temp.path().join("pipe"))
            .status()
            .expect("run mkfifo");
        assert!(status.success());
        let root = capture_absolute_directory(temp.path(), DirectoryAccess::Read).expect("capture");
        assert!(matches!(
            query_child_nofollow(&root, &name("link")),
            Ok(ChildState::Present(EntryMetadata {
                kind: EntryKind::SymlinkOrReparse,
                ..
            }))
        ));
        assert!(matches!(
            query_child_nofollow(&root, &name("pipe")),
            Ok(ChildState::Present(EntryMetadata {
                kind: EntryKind::Fifo,
                ..
            }))
        ));
        let enumerated = enumerate(&root).expect("enumerate every validated component");
        assert_eq!(enumerated, vec![name("link"), name("pipe")]);
    }

    #[test]
    fn platform_dispatched_file_bytes_copy_seek_flush_and_sync() {
        let _serial = test_seam::serialize_unix_test();
        let temp = TestDir::new("bytes");
        std::fs::write(temp.path().join("source"), b"0123456789").expect("source");
        let root = capture_absolute_directory(temp.path(), DirectoryAccess::MutateChildren)
            .expect("capture");
        let mut source =
            open_file_nofollow(&root, &name("source"), FileAccess::Read).expect("open source");
        let expected = source.opened_metadata().identity.clone();
        let mut destination =
            create_file_new(&root, &name("destination"), CreatePermissions::OwnerOnly)
                .expect("create destination");
        let result = stream_copy_file(&mut source, &mut destination, &expected, 10).expect("copy");
        assert_eq!(result.bytes_copied, 10);
        destination
            .seek(std::io::SeekFrom::Start(0))
            .expect("rewind");
        let mut bytes = [0_u8; 10];
        assert_eq!(destination.read(&mut bytes).expect("read destination"), 10);
        assert_eq!(&bytes, b"0123456789");
    }

    #[test]
    fn post_create_metadata_failure_removes_new_file() {
        let _serial = test_seam::serialize_unix_test();
        let temp = TestDir::new("create-metadata-rollback");
        let root = capture_absolute_directory(temp.path(), DirectoryAccess::MutateChildren)
            .expect("capture");
        let _failure = test_seam::install_create_failure(test_seam::CreateFailurePoint::Metadata);
        let error = match create_file_new(&root, &name("created"), CreatePermissions::OwnerOnly) {
            Ok(_) => panic!("metadata failure must reject the created file"),
            Err(error) => error,
        };
        assert!(
            matches!(
                error,
                SafeFsError::Io {
                    operation: SafeFsOperation::CreateFile,
                    ..
                }
            ),
            "unexpected error: {error:?}"
        );
        assert_absent(&root, "created");
    }

    #[test]
    fn post_create_filesystem_failure_removes_new_file() {
        let _serial = test_seam::serialize_unix_test();
        let temp = TestDir::new("create-filesystem-rollback");
        let root = capture_absolute_directory(temp.path(), DirectoryAccess::MutateChildren)
            .expect("capture");
        let _failure =
            test_seam::install_create_failure(test_seam::CreateFailurePoint::FilesystemProbe);
        assert!(matches!(
            create_file_new(&root, &name("created"), CreatePermissions::OwnerOnly),
            Err(SafeFsError::Io {
                operation: SafeFsOperation::CreateFile,
                ..
            })
        ));
        assert_absent(&root, "created");
    }

    #[test]
    fn post_create_case_failure_removes_new_directory() {
        let _serial = test_seam::serialize_unix_test();
        let temp = TestDir::new("create-case-rollback");
        let root = capture_absolute_directory(temp.path(), DirectoryAccess::MutateChildren)
            .expect("capture");
        let _failure = test_seam::install_create_failure(test_seam::CreateFailurePoint::CaseProof);
        assert!(matches!(
            create_dir_new(
                &root,
                &name("created"),
                CreatePermissions::OwnerOnly,
                DirectoryAccess::Read
            ),
            Err(SafeFsError::Io {
                operation: SafeFsOperation::CreateDirectory,
                ..
            })
        ));
        assert_absent(&root, "created");
    }

    #[test]
    fn post_create_parent_duplicate_failure_removes_new_stage() {
        let _serial = test_seam::serialize_unix_test();
        let temp = TestDir::new("create-parent-dup-rollback");
        let root = capture_absolute_directory(temp.path(), DirectoryAccess::MutateChildren)
            .expect("capture");
        let _failure =
            test_seam::install_create_failure(test_seam::CreateFailurePoint::ParentDuplicate);
        assert!(matches!(
            create_stage_dir_new(&root, &name("stage"), CreatePermissions::OwnerOnly),
            Err(SafeFsError::Io {
                operation: SafeFsOperation::OpenDirectory,
                ..
            })
        ));
        assert_absent(&root, "stage");
    }

    #[test]
    fn post_create_retained_identity_failure_returns_typed_fail_leak() {
        let _serial = test_seam::serialize_unix_test();
        let temp = TestDir::new("create-retained-identity-fail-leak");
        let root = capture_absolute_directory(temp.path(), DirectoryAccess::MutateChildren)
            .expect("capture");
        let _failure = test_seam::install_create_failure(test_seam::CreateFailurePoint::Metadata);
        let _rollback =
            test_seam::install_rollback_failure(test_seam::RollbackFailurePoint::RetainedIdentity);
        assert!(matches!(
            create_file_new(&root, &name("created"), CreatePermissions::OwnerOnly),
            Err(SafeFsError::StageIdentityLost {
                operation: SafeFsOperation::RollbackCreatedEntry,
                reason: StageIdentityLostReason::CreatedObjectIdentityUnavailable
            })
        ));
        assert!(
            temp.path().join("created").is_file(),
            "unproven created file must fail-leak"
        );
    }

    #[test]
    fn post_create_original_name_rebound_before_identity_check_is_preserved() {
        let _serial = test_seam::serialize_unix_test();
        let temp = TestDir::new("create-original-rebound");
        let root = Arc::new(
            capture_absolute_directory(temp.path(), DirectoryAccess::MutateChildren)
                .expect("capture"),
        );
        let _failure =
            test_seam::install_create_failure(test_seam::CreateFailurePoint::ParentDuplicate);
        let gate = RaceGate::new();
        let _hook = test_seam::install(gate.hook(HookPoint::BeforeCreatedRollbackInitialNameCheck));
        let worker_root = Arc::clone(&root);
        let worker = std::thread::spawn(move || {
            create_stage_dir_new(&worker_root, &name("stage"), CreatePermissions::OwnerOnly)
        });
        gate.wait_reached();
        let retained = temp.path().join("created-retained");
        std::fs::rename(temp.path().join("stage"), &retained).expect("retain created directory");
        std::fs::create_dir(temp.path().join("stage")).expect("rebind original name");
        std::fs::write(temp.path().join("stage/replacement-marker"), b"replacement")
            .expect("mark replacement");
        gate.release();
        assert!(matches!(
            worker.join().expect("worker join"),
            Err(SafeFsError::StageIdentityLost {
                operation: SafeFsOperation::RollbackCreatedEntry,
                reason: StageIdentityLostReason::CreatedNameChanged
            })
        ));
        assert!(retained.is_dir(), "created object must remain retained");
        assert_eq!(
            std::fs::read(temp.path().join("stage/replacement-marker"))
                .expect("replacement preserved"),
            b"replacement"
        );
    }

    #[test]
    fn post_create_quarantine_move_failure_returns_typed_fail_leak() {
        let _serial = test_seam::serialize_unix_test();
        let temp = TestDir::new("create-quarantine-move-fail-leak");
        let root = capture_absolute_directory(temp.path(), DirectoryAccess::MutateChildren)
            .expect("capture");
        let _failure =
            test_seam::install_create_failure(test_seam::CreateFailurePoint::ParentDuplicate);
        let _rollback =
            test_seam::install_rollback_failure(test_seam::RollbackFailurePoint::QuarantineMove);
        assert!(matches!(
            create_stage_dir_new(&root, &name("stage"), CreatePermissions::OwnerOnly),
            Err(SafeFsError::StageIdentityLost {
                operation: SafeFsOperation::RollbackCreatedEntry,
                reason: StageIdentityLostReason::CreatedRollbackQuarantineFailed
            })
        ));
        assert!(
            temp.path().join("stage").is_dir(),
            "failed quarantine must preserve original name"
        );
    }

    #[test]
    fn post_create_delete_failure_preserves_verified_quarantine() {
        let _serial = test_seam::serialize_unix_test();
        let temp = TestDir::new("create-delete-fail-leak");
        let root = capture_absolute_directory(temp.path(), DirectoryAccess::MutateChildren)
            .expect("capture");
        let _failure =
            test_seam::install_create_failure(test_seam::CreateFailurePoint::ParentDuplicate);
        let _rollback =
            test_seam::install_rollback_failure(test_seam::RollbackFailurePoint::Delete);
        assert!(matches!(
            create_stage_dir_new(&root, &name("stage"), CreatePermissions::OwnerOnly),
            Err(SafeFsError::StageIdentityLost {
                operation: SafeFsOperation::RollbackCreatedEntry,
                reason: StageIdentityLostReason::CreatedRollbackDeleteFailed
            })
        ));
        assert!(
            !temp.path().join("stage").exists(),
            "original name was already quarantined"
        );
        let quarantine = std::fs::read_dir(temp.path())
            .expect("enumerate fixture")
            .map(|entry| entry.expect("directory entry").path())
            .find(|path| {
                path.file_name().is_some_and(|name| {
                    name.to_string_lossy()
                        .starts_with(".opentake-create-rollback-")
                })
            })
            .expect("verified quarantine must fail-leak");
        assert!(quarantine.is_dir());
    }

    #[test]
    fn post_create_rebound_name_returns_typed_fail_leak_without_deletion() {
        let _serial = test_seam::serialize_unix_test();
        let temp = TestDir::new("create-rebound-fail-leak");
        let root = Arc::new(
            capture_absolute_directory(temp.path(), DirectoryAccess::MutateChildren)
                .expect("capture"),
        );
        let _failure =
            test_seam::install_create_failure(test_seam::CreateFailurePoint::ParentDuplicate);
        let gate = RaceGate::new();
        let _hook = test_seam::install(gate.hook(HookPoint::BeforeCreatedRollbackQuarantine));
        let worker_root = Arc::clone(&root);
        let worker = std::thread::spawn(move || {
            create_stage_dir_new(&worker_root, &name("stage"), CreatePermissions::OwnerOnly)
        });
        gate.wait_reached();
        std::fs::rename(
            temp.path().join("stage"),
            temp.path().join("created-retained"),
        )
        .expect("retain created directory");
        std::fs::create_dir(temp.path().join("stage")).expect("rebind original name");
        std::fs::write(temp.path().join("stage/replacement-marker"), b"replacement")
            .expect("mark replacement");
        gate.release();
        assert!(matches!(
            worker.join().expect("worker join"),
            Err(SafeFsError::StageIdentityLost {
                operation: SafeFsOperation::RollbackCreatedEntry,
                reason: StageIdentityLostReason::CreatedRollbackQuarantineChanged
            })
        ));
        assert!(
            temp.path().join("created-retained").is_dir(),
            "retained created object must not be deleted"
        );
        let quarantine = std::fs::read_dir(temp.path())
            .expect("enumerate fixture")
            .map(|entry| entry.expect("directory entry").path())
            .find(|path| {
                path.file_name().is_some_and(|name| {
                    name.to_string_lossy()
                        .starts_with(".opentake-create-rollback-")
                })
            })
            .expect("rebound name must fail-leak in quarantine");
        assert_eq!(
            std::fs::read(quarantine.join("replacement-marker")).expect("replacement preserved"),
            b"replacement"
        );
    }

    #[test]
    fn post_create_quarantine_rebound_after_verification_is_not_deleted() {
        let _serial = test_seam::serialize_unix_test();
        let temp = TestDir::new("create-quarantine-rebound");
        let root = Arc::new(
            capture_absolute_directory(temp.path(), DirectoryAccess::MutateChildren)
                .expect("capture"),
        );
        let _failure =
            test_seam::install_create_failure(test_seam::CreateFailurePoint::ParentDuplicate);
        let gate = RaceGate::new();
        let _hook =
            test_seam::install(gate.hook(HookPoint::AfterCreatedRollbackVerifyBeforeDelete));
        let worker_root = Arc::clone(&root);
        let worker = std::thread::spawn(move || {
            create_stage_dir_new(&worker_root, &name("stage"), CreatePermissions::OwnerOnly)
        });
        gate.wait_reached();
        let quarantine = std::fs::read_dir(temp.path())
            .expect("enumerate fixture")
            .map(|entry| entry.expect("directory entry").path())
            .find(|path| {
                path.file_name().is_some_and(|name| {
                    name.to_string_lossy()
                        .starts_with(".opentake-create-rollback-")
                })
            })
            .expect("rollback quarantine exists at verification hook");
        let retained = temp.path().join("created-quarantine-retained");
        std::fs::rename(&quarantine, &retained).expect("retain verified created object");
        std::fs::create_dir(&quarantine).expect("rebind quarantine name");
        std::fs::write(quarantine.join("replacement-marker"), b"replacement")
            .expect("mark replacement");
        gate.release();
        assert!(matches!(
            worker.join().expect("worker join"),
            Err(SafeFsError::StageIdentityLost {
                operation: SafeFsOperation::RollbackCreatedEntry,
                reason: StageIdentityLostReason::CreatedRollbackQuarantineChanged
            })
        ));
        assert!(
            retained.is_dir(),
            "verified created object must not be deleted after rebound"
        );
        assert_eq!(
            std::fs::read(quarantine.join("replacement-marker")).expect("replacement preserved"),
            b"replacement"
        );
    }

    #[test]
    fn nested_recursive_quarantine_cleanup_removes_files_symlink_fifo_and_directories() {
        let _serial = test_seam::serialize_unix_test();
        use std::os::unix::fs::symlink;
        use std::process::Command;
        let temp = TestDir::new("nested-cleanup");
        let root = capture_absolute_directory(temp.path(), DirectoryAccess::MutateChildren)
            .expect("capture");
        let stage = create_stage_dir_new(&root, &name("stage"), CreatePermissions::OwnerOnly)
            .expect("create stage");
        std::fs::create_dir_all(temp.path().join("stage/a/b")).expect("nested dirs");
        std::fs::write(temp.path().join("stage/a/file"), b"payload").expect("file");
        symlink("file", temp.path().join("stage/a/link")).expect("symlink");
        assert!(Command::new("mkfifo")
            .arg(temp.path().join("stage/a/b/pipe"))
            .status()
            .expect("mkfifo")
            .success());
        let quarantined =
            quarantine_stage(stage, &root, name(".opentake-quarantine-0123456789abcdef"))
                .expect("quarantine");
        cleanup_quarantined_tree(quarantined).expect("recursive cleanup");
        assert!(!temp.path().join("stage").exists());
        assert!(!temp
            .path()
            .join(".opentake-quarantine-0123456789abcdef")
            .exists());
    }

    #[test]
    fn destination_collision_preserves_stage_and_every_destination_kind() {
        let _serial = test_seam::serialize_unix_test();
        for kind in ["file", "empty-dir", "non-empty-dir", "symlink"] {
            let temp = TestDir::new(kind);
            let root = capture_absolute_directory(temp.path(), DirectoryAccess::MutateChildren)
                .expect("capture");
            let stage = create_stage_dir_new(&root, &name("stage"), CreatePermissions::OwnerOnly)
                .expect("stage");
            match kind {
                "file" => {
                    std::fs::write(temp.path().join("destination"), b"existing").expect("file")
                }
                "empty-dir" => std::fs::create_dir(temp.path().join("destination")).expect("dir"),
                "non-empty-dir" => {
                    std::fs::create_dir(temp.path().join("destination")).expect("dir");
                    std::fs::write(temp.path().join("destination/child"), b"existing")
                        .expect("child");
                }
                "symlink" => std::os::unix::fs::symlink("target", temp.path().join("destination"))
                    .expect("symlink"),
                _ => unreachable!(),
            }
            assert!(matches!(
                publish_stage_noreplace(stage, &root, name("destination")),
                Err(SafeFsError::AlreadyExists {
                    operation: SafeFsOperation::PublishNoReplace
                })
            ));
            assert!(temp.path().join("stage").is_dir());
        }
    }

    #[test]
    fn source_swap_before_quarantine_restores_without_deletion() {
        let _serial = test_seam::serialize_unix_test();
        let temp = TestDir::new("source-swap");
        let root = Arc::new(
            capture_absolute_directory(temp.path(), DirectoryAccess::MutateChildren)
                .expect("capture"),
        );
        let stage = create_stage_dir_new(&root, &name("stage"), CreatePermissions::OwnerOnly)
            .expect("stage");
        std::fs::write(temp.path().join("stage/expected"), b"expected").expect("expected file");
        let gate = RaceGate::new();
        let _guard = test_seam::install(gate.hook(HookPoint::BeforeQuarantineRename));
        let worker_root = Arc::clone(&root);
        let worker = std::thread::spawn(move || {
            quarantine_stage(
                stage,
                &worker_root,
                name(".opentake-quarantine-source-swap"),
            )
        });
        gate.wait_reached();
        std::fs::rename(
            temp.path().join("stage"),
            temp.path().join("expected-moved"),
        )
        .expect("move expected stage");
        std::fs::create_dir(temp.path().join("stage")).expect("replacement stage");
        std::fs::write(temp.path().join("stage/replacement"), b"replacement")
            .expect("replacement file");
        gate.release();
        assert!(matches!(
            worker.join().expect("worker join"),
            Err(SafeFsError::StageIdentityLost {
                operation: SafeFsOperation::RestoreQuarantine,
                ..
            })
        ));
        assert_eq!(
            std::fs::read(temp.path().join("expected-moved/expected")).expect("expected preserved"),
            b"expected"
        );
        assert_eq!(
            std::fs::read(temp.path().join("stage/replacement")).expect("replacement restored"),
            b"replacement"
        );
        assert!(!temp
            .path()
            .join(".opentake-quarantine-source-swap")
            .exists());
    }

    #[test]
    fn restore_collision_fail_leaks_original_and_quarantine() {
        let _serial = test_seam::serialize_unix_test();
        let temp = TestDir::new("restore-collision");
        let root = Arc::new(
            capture_absolute_directory(temp.path(), DirectoryAccess::MutateChildren)
                .expect("capture"),
        );
        let stage = create_stage_dir_new(&root, &name("stage"), CreatePermissions::OwnerOnly)
            .expect("stage");
        std::fs::write(temp.path().join("stage/expected"), b"expected").expect("expected file");
        std::fs::rename(
            temp.path().join("stage"),
            temp.path().join("expected-moved"),
        )
        .expect("move expected stage");
        std::fs::create_dir(temp.path().join("stage")).expect("replacement stage");
        std::fs::write(temp.path().join("stage/replacement"), b"replacement")
            .expect("replacement file");
        let gate = RaceGate::new();
        let _guard = test_seam::install(gate.hook(HookPoint::BeforeQuarantineRestore));
        let worker_root = Arc::clone(&root);
        let worker = std::thread::spawn(move || {
            quarantine_stage(
                stage,
                &worker_root,
                name(".opentake-quarantine-restore-collision"),
            )
        });
        gate.wait_reached();
        std::fs::create_dir(temp.path().join("stage")).expect("occupy original name");
        std::fs::write(temp.path().join("stage/occupant"), b"occupant").expect("occupant file");
        gate.release();
        assert!(matches!(
            worker.join().expect("worker join"),
            Err(SafeFsError::StageIdentityLost {
                operation: SafeFsOperation::RestoreQuarantine,
                reason: StageIdentityLostReason::OriginalNameOccupied
            })
        ));
        assert_eq!(
            std::fs::read(temp.path().join("stage/occupant")).expect("occupant preserved"),
            b"occupant"
        );
        assert_eq!(
            std::fs::read(
                temp.path()
                    .join(".opentake-quarantine-restore-collision/replacement")
            )
            .expect("quarantine preserved"),
            b"replacement"
        );
        assert_eq!(
            std::fs::read(temp.path().join("expected-moved/expected"))
                .expect("original retained handle target preserved"),
            b"expected"
        );
    }

    #[test]
    fn final_unix_name_window_is_explicit_same_account_boundary() {
        let _serial = test_seam::serialize_unix_test();
        let temp = TestDir::new("final-name-window");
        let root = capture_absolute_directory(temp.path(), DirectoryAccess::MutateChildren)
            .expect("capture");
        let stage = create_stage_dir_new(&root, &name("stage"), CreatePermissions::OwnerOnly)
            .expect("stage");
        std::fs::write(temp.path().join("stage/leaf"), b"expected").expect("expected leaf");
        let quarantined = quarantine_stage(stage, &root, name(".opentake-quarantine-final-window"))
            .expect("quarantine");
        let entry =
            open_cleanup_child_nofollow(&quarantined, &name("leaf")).expect("open cleanup entry");
        let gate = RaceGate::new();
        let _guard =
            test_seam::install(gate.hook(HookPoint::AfterFinalIdentityReadBeforeNameSyscall));
        let worker = std::thread::spawn(move || delete_quarantined_entry(entry));
        gate.wait_reached();
        let quarantine_path = temp.path().join(".opentake-quarantine-final-window");
        std::fs::rename(
            quarantine_path.join("leaf"),
            quarantine_path.join("expected-moved"),
        )
        .expect("move expected leaf");
        std::fs::write(quarantine_path.join("leaf"), b"replacement").expect("replacement leaf");
        gate.release();
        worker
            .join()
            .expect("worker join")
            .expect("name-linearized deletion");
        assert_eq!(
            std::fs::read(quarantine_path.join("expected-moved"))
                .expect("expected object preserved"),
            b"expected"
        );
        assert!(!quarantine_path.join("leaf").exists());
    }

    #[test]
    fn cleanup_capability_records_identity_before_consuming_delete() {
        let _serial = test_seam::serialize_unix_test();
        let temp = TestDir::new("cleanup-identity");
        let root = capture_absolute_directory(temp.path(), DirectoryAccess::MutateChildren)
            .expect("capture");
        let stage = create_stage_dir_new(&root, &name("stage"), CreatePermissions::OwnerOnly)
            .expect("stage");
        std::fs::write(temp.path().join("stage/leaf"), b"leaf").expect("leaf");
        let quarantined = quarantine_stage(stage, &root, name(".opentake-quarantine-identity"))
            .expect("quarantine");
        let expected = present_identity(quarantined.directory(), "leaf");
        let entry =
            open_cleanup_child_nofollow(&quarantined, &name("leaf")).expect("cleanup entry");
        match &entry {
            CleanupCapability::Entry(entry) => {
                assert_eq!(entry.access, CleanupAccess::Delete);
                assert_eq!(entry.opened.identity, expected);
            }
            CleanupCapability::Directory(_) => panic!("expected leaf cleanup capability"),
        }
        delete_quarantined_entry(entry).expect("delete retained cleanup entry");
    }
}
