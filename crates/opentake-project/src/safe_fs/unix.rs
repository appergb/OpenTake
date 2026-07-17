use super::capability::*;
use super::component::ComponentName;
use super::error::*;
use rustix::fd::{AsFd, OwnedFd};
use rustix::fs::{AtFlags, Dir, FileType, Mode, OFlags, RenameFlags};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::os::fd::AsRawFd;
use std::os::unix::ffi::OsStrExt;
use std::path::{Component, Path};
use std::sync::Arc;

const DIR_FLAGS: OFlags = OFlags::RDONLY
    .union(OFlags::DIRECTORY)
    .union(OFlags::NOFOLLOW)
    .union(OFlags::CLOEXEC);
const FILE_READ_FLAGS: OFlags = OFlags::RDONLY
    .union(OFlags::NOFOLLOW)
    .union(OFlags::CLOEXEC);
const FILE_RW_FLAGS: OFlags = OFlags::RDWR.union(OFlags::NOFOLLOW).union(OFlags::CLOEXEC);
#[cfg(test)]
const OWNER_DIR_MODE: Mode = Mode::RUSR.union(Mode::WUSR).union(Mode::XUSR);
const OWNER_FILE_MODE: Mode = Mode::RUSR.union(Mode::WUSR);
#[cfg(test)]
const INHERIT_DIR_MODE: Mode = Mode::RUSR
    .union(Mode::WUSR)
    .union(Mode::XUSR)
    .union(Mode::RGRP)
    .union(Mode::WGRP)
    .union(Mode::XGRP)
    .union(Mode::ROTH)
    .union(Mode::WOTH)
    .union(Mode::XOTH);
const INHERIT_FILE_MODE: Mode = Mode::RUSR
    .union(Mode::WUSR)
    .union(Mode::RGRP)
    .union(Mode::WGRP)
    .union(Mode::ROTH)
    .union(Mode::WOTH);
#[cfg(target_os = "linux")]
const EXT_MAGIC: i64 = 0x0000_ef53;
#[cfg(target_os = "linux")]
const XFS_MAGIC: i64 = 0x5846_5342;
#[cfg(target_os = "linux")]
const BTRFS_MAGIC: i64 = 0x9123_683e;
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

pub(super) struct NativeNamespaceAnchor {
    root: OwnedFd,
}
pub(super) struct NativeDirectory {
    fd: OwnedFd,
}
pub(super) enum NativeFile {
    Open(File),
    NameOnly {
        name: ComponentName,
        expected: StableIdentity,
        kind: EntryKind,
    },
}

fn io(operation: SafeFsOperation, error: rustix::io::Errno) -> SafeFsError {
    SafeFsError::io(
        operation,
        std::io::Error::from_raw_os_error(error.raw_os_error()),
    )
}
#[cfg(target_os = "linux")]
fn stat_device(stat: &rustix::fs::Stat) -> u64 {
    stat.st_dev
}
#[cfg(target_os = "macos")]
fn stat_device(stat: &rustix::fs::Stat) -> u64 {
    stat.st_dev as u64
}
#[cfg(target_os = "linux")]
fn stat_link_count(stat: &rustix::fs::Stat) -> u64 {
    stat.st_nlink
}
#[cfg(target_os = "macos")]
fn stat_link_count(stat: &rustix::fs::Stat) -> u64 {
    stat.st_nlink as u64
}
fn identity(stat: &rustix::fs::Stat) -> StableIdentity {
    StableIdentity::Unix {
        device: stat_device(stat),
        inode: stat.st_ino,
    }
}
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
fn linux_filesystem_from_raw(
    magic: i64,
    fsid: u64,
    device: u64,
    operation: SafeFsOperation,
) -> Result<LocalFilesystemSnapshot> {
    let family = match magic {
        EXT_MAGIC => LinuxFilesystem::Ext,
        XFS_MAGIC => LinuxFilesystem::Xfs,
        BTRFS_MAGIC => LinuxFilesystem::Btrfs,
        NFS_MAGIC | CIFS_MAGIC | SMB2_MAGIC => {
            return Err(SafeFsError::UnsupportedSecureFilesystem {
                operation,
                reason: SecureFilesystemReason::RemoteFilesystem,
            })
        }
        _ => {
            return Err(SafeFsError::UnsupportedSecureFilesystem {
                operation,
                reason: SecureFilesystemReason::UnknownFilesystem,
            })
        }
    };
    Ok(LocalFilesystemSnapshot::Linux {
        family,
        fsid,
        device,
    })
}

#[cfg(target_os = "linux")]
fn probe_local(
    fd: impl AsFd,
    stat: &rustix::fs::Stat,
    operation: SafeFsOperation,
) -> Result<LocalFilesystemSnapshot> {
    #[cfg(test)]
    if let Some(sample) = super::test_seam::unix_probe_sample() {
        return match sample {
            super::test_seam::UnixProbeSample::Linux {
                magic,
                fsid,
                device,
                ..
            } => linux_filesystem_from_raw(magic, fsid, device, operation),
            super::test_seam::UnixProbeSample::Failure(reason) => {
                Err(SafeFsError::UnsupportedSecureFilesystem { operation, reason })
            }
            super::test_seam::UnixProbeSample::MacOs { .. } => {
                Err(SafeFsError::UnsupportedSecureFilesystem {
                    operation,
                    reason: SecureFilesystemReason::FilesystemProbeUnavailable,
                })
            }
        };
    }
    let fs = rustix::fs::fstatfs(&fd).map_err(|error| io(operation, error))?;
    let vfs = rustix::fs::fstatvfs(fd).map_err(|error| io(operation, error))?;
    linux_filesystem_from_raw(fs.f_type as i64, vfs.f_fsid, stat_device(stat), operation)
}

#[cfg(target_os = "macos")]
fn macos_filesystem_from_raw(
    flags: u32,
    type_name: [u8; 16],
    fsid: u64,
    device: u64,
    operation: SafeFsOperation,
) -> Result<LocalFilesystemSnapshot> {
    if flags & MNT_LOCAL == 0 {
        return Err(SafeFsError::UnsupportedSecureFilesystem {
            operation,
            reason: SecureFilesystemReason::RemoteFilesystem,
        });
    }
    Ok(LocalFilesystemSnapshot::MacOs {
        type_name,
        fsid,
        device,
    })
}

#[cfg(target_os = "macos")]
fn probe_local(
    fd: impl AsFd,
    stat: &rustix::fs::Stat,
    operation: SafeFsOperation,
) -> Result<LocalFilesystemSnapshot> {
    #[cfg(test)]
    if let Some(sample) = super::test_seam::unix_probe_sample() {
        return match sample {
            super::test_seam::UnixProbeSample::MacOs {
                mount_flags,
                type_name,
                fsid,
                device,
                ..
            } => macos_filesystem_from_raw(mount_flags, type_name, fsid, device, operation),
            super::test_seam::UnixProbeSample::Failure(reason) => {
                Err(SafeFsError::UnsupportedSecureFilesystem { operation, reason })
            }
            super::test_seam::UnixProbeSample::Linux { .. } => {
                Err(SafeFsError::UnsupportedSecureFilesystem {
                    operation,
                    reason: SecureFilesystemReason::FilesystemProbeUnavailable,
                })
            }
        };
    }
    let fs = rustix::fs::fstatfs(&fd).map_err(|error| io(operation, error))?;
    let vfs = rustix::fs::fstatvfs(fd).map_err(|error| io(operation, error))?;
    macos_filesystem_from_raw(
        fs.f_flags,
        fs.f_fstypename.map(|byte| byte as u8),
        vfs.f_fsid,
        stat_device(stat),
        operation,
    )
}

fn opened_metadata_from_stat(
    fd: impl AsFd,
    stat: &rustix::fs::Stat,
    operation: SafeFsOperation,
) -> Result<EntryMetadata> {
    let filesystem = probe_local(fd, stat, operation)?;
    Ok(EntryMetadata {
        identity: identity(stat),
        kind: kind(stat),
        len: stat.st_size as u64,
        link_count: stat_link_count(stat),
        filesystem: Some(filesystem),
    })
}

fn opened_metadata(fd: impl AsFd, operation: SafeFsOperation) -> Result<EntryMetadata> {
    let stat = rustix::fs::fstat(&fd).map_err(|error| io(operation, error))?;
    opened_metadata_from_stat(fd, &stat, operation)
}

#[cfg(target_os = "linux")]
fn linux_case_from_raw(
    family: LinuxFilesystem,
    ext_flags: std::result::Result<i64, SecureFilesystemReason>,
    operation: SafeFsOperation,
) -> Result<CaseMode> {
    match family {
        LinuxFilesystem::Xfs | LinuxFilesystem::Btrfs => Ok(CaseMode::Sensitive),
        LinuxFilesystem::Ext => {
            let flags = ext_flags
                .map_err(|reason| SafeFsError::UnsupportedSecureFilesystem { operation, reason })?;
            if flags & FS_CASEFOLD_FL == 0 {
                Ok(CaseMode::Sensitive)
            } else {
                Ok(CaseMode::Insensitive)
            }
        }
    }
}

#[cfg(target_os = "macos")]
fn macos_case_from_raw(value: i64, operation: SafeFsOperation) -> Result<CaseMode> {
    match value {
        0 => Ok(CaseMode::Insensitive),
        1 => Ok(CaseMode::Sensitive),
        _ => Err(SafeFsError::UnsupportedSecureFilesystem {
            operation,
            reason: SecureFilesystemReason::CaseSemanticsUnavailable,
        }),
    }
}

#[cfg(target_os = "linux")]
fn probe_case_mode(
    fd: impl AsFd,
    metadata: &EntryMetadata,
    operation: SafeFsOperation,
) -> Result<CaseMode> {
    let family = match metadata.filesystem.as_ref() {
        Some(LocalFilesystemSnapshot::Linux { family, .. }) => *family,
        _ => {
            return Err(SafeFsError::UnsupportedSecureFilesystem {
                operation,
                reason: SecureFilesystemReason::CaseSemanticsUnavailable,
            })
        }
    };
    #[cfg(test)]
    if let Some(sample) = super::test_seam::unix_probe_sample() {
        return match sample {
            super::test_seam::UnixProbeSample::Linux { ext_flags, .. } => {
                linux_case_from_raw(family, ext_flags, operation)
            }
            super::test_seam::UnixProbeSample::Failure(reason) => {
                Err(SafeFsError::UnsupportedSecureFilesystem { operation, reason })
            }
            super::test_seam::UnixProbeSample::MacOs { .. } => {
                Err(SafeFsError::UnsupportedSecureFilesystem {
                    operation,
                    reason: SecureFilesystemReason::CaseSemanticsUnavailable,
                })
            }
        };
    }
    let ext_flags = if family == LinuxFilesystem::Ext {
        let mut flags: libc::c_long = 0;
        // SAFETY: `flags` points to writable storage of the exact kernel ABI type, and the
        // retained descriptor refers to the directory whose ext flags are being queried.
        let result = unsafe { libc::ioctl(fd.as_fd().as_raw_fd(), FS_IOC_GETFLAGS, &mut flags) };
        if result < 0 {
            Err(SecureFilesystemReason::CaseSemanticsUnavailable)
        } else {
            Ok(flags)
        }
    } else {
        Ok(0)
    };
    linux_case_from_raw(family, ext_flags, operation)
}

#[cfg(target_os = "macos")]
fn probe_case_mode(
    fd: impl AsFd,
    _: &EntryMetadata,
    operation: SafeFsOperation,
) -> Result<CaseMode> {
    #[cfg(test)]
    if let Some(sample) = super::test_seam::unix_probe_sample() {
        return match sample {
            super::test_seam::UnixProbeSample::MacOs { case_sensitive, .. } => {
                macos_case_from_raw(case_sensitive, operation)
            }
            super::test_seam::UnixProbeSample::Failure(reason) => {
                Err(SafeFsError::UnsupportedSecureFilesystem { operation, reason })
            }
            super::test_seam::UnixProbeSample::Linux { .. } => {
                Err(SafeFsError::UnsupportedSecureFilesystem {
                    operation,
                    reason: SecureFilesystemReason::CaseSemanticsUnavailable,
                })
            }
        };
    }
    // SAFETY: the retained descriptor is valid for this call and `_PC_CASE_SENSITIVE` does
    // not require an output buffer; `fpathconf` reports the value directly.
    let case_sensitive =
        unsafe { libc::fpathconf(fd.as_fd().as_raw_fd(), libc::_PC_CASE_SENSITIVE) };
    macos_case_from_raw(case_sensitive, operation)
}

fn name_metadata(
    parent: &DirectoryAuthority,
    name: &ComponentName,
    operation: SafeFsOperation,
) -> Result<ChildState> {
    let stat = match rustix::fs::statat(
        &parent.native.fd,
        name.as_os_str(),
        AtFlags::SYMLINK_NOFOLLOW,
    ) {
        Ok(stat) => stat,
        Err(rustix::io::Errno::NOENT) => return Ok(ChildState::Absent),
        Err(error) => return Err(io(operation, error)),
    };
    let same_device = match (&parent.opened.identity, identity(&stat)) {
        (
            StableIdentity::Unix {
                device: parent_device,
                ..
            },
            StableIdentity::Unix { device, .. },
        ) => *parent_device == device,
        _ => false,
    };
    Ok(ChildState::Present(EntryMetadata {
        identity: identity(&stat),
        kind: kind(&stat),
        len: stat.st_size as u64,
        link_count: stat_link_count(&stat),
        filesystem: same_device
            .then(|| parent.opened.filesystem.clone())
            .flatten(),
    }))
}

fn require_parent(parent: &DirectoryAuthority, operation: SafeFsOperation) -> Result<()> {
    if matches!(
        parent.access,
        DirectoryAccess::MutateChildren | DirectoryAccess::Stage
    ) {
        Ok(())
    } else {
        Err(SafeFsError::AccessMismatch { operation })
    }
}

fn duplicate_directory(
    directory: &DirectoryAuthority,
    access: DirectoryAccess,
) -> Result<DirectoryAuthority> {
    let fd = rustix::io::fcntl_dupfd_cloexec(&directory.native.fd, 0)
        .map_err(|error| io(SafeFsOperation::OpenDirectory, error))?;
    Ok(DirectoryAuthority {
        anchor: Arc::clone(&directory.anchor),
        native: NativeDirectory { fd },
        access,
        opened: directory.opened.clone(),
        case_mode: directory.case_mode,
        snapshot: directory.snapshot.clone(),
    })
}

fn open_dir_fd(
    parent: impl AsFd,
    name: &ComponentName,
    operation: SafeFsOperation,
) -> Result<(OwnedFd, EntryMetadata)> {
    let fd = rustix::fs::openat(parent, name.as_os_str(), DIR_FLAGS, Mode::empty()).map_err(
        |error| match error {
            rustix::io::Errno::NOENT => SafeFsError::NotFound { operation },
            rustix::io::Errno::LOOP => SafeFsError::SymlinkOrReparsePoint { operation },
            other => io(operation, other),
        },
    )?;
    let metadata = opened_metadata(&fd, operation)?;
    if metadata.kind != EntryKind::Directory {
        return Err(SafeFsError::UnsupportedEntryType {
            operation,
            kind: metadata.kind,
        });
    }
    Ok((fd, metadata))
}

fn absolute_names(path: &Path) -> Result<Vec<ComponentName>> {
    let mut rooted = false;
    let mut names = Vec::new();
    for part in path.components() {
        match part {
            Component::RootDir if !rooted => rooted = true,
            Component::Normal(value) if rooted => names.push(ComponentName::new(value)?),
            _ => {
                return Err(SafeFsError::InvalidRelativePath(
                    RelativePathViolation::AbsoluteOrPrefix,
                ))
            }
        }
    }
    if !rooted {
        return Err(SafeFsError::InvalidRelativePath(
            RelativePathViolation::AbsoluteOrPrefix,
        ));
    }
    Ok(names)
}

fn snapshot_from_root(
    root: OwnedFd,
    names: &[ComponentName],
    operation: SafeFsOperation,
) -> Result<(OwnedFd, EntryMetadata, NamespaceSnapshot)> {
    let root_metadata = opened_metadata(&root, operation)?;
    let root_case_mode = probe_case_mode(&root, &root_metadata, operation)?;
    let mut current = root;
    let mut components = Vec::with_capacity(names.len());
    for name in names {
        let (next, metadata) = open_dir_fd(&current, name, operation)?;
        let case_mode = probe_case_mode(&next, &metadata, operation)?;
        components.push(NamespaceComponent {
            name: name.clone(),
            identity: metadata.identity.clone(),
            filesystem: metadata
                .filesystem
                .clone()
                .expect("opened directories have filesystem proof"),
            case_mode,
        });
        current = next;
    }
    let snapshot = NamespaceSnapshot {
        root_identity: root_metadata.identity.clone(),
        root_filesystem: root_metadata
            .filesystem
            .clone()
            .expect("opened root has filesystem proof"),
        root_case_mode,
        components,
    };
    Ok((current, root_metadata, snapshot))
}

pub(super) fn capture_absolute_directory(
    path: &Path,
    access: DirectoryAccess,
) -> Result<DirectoryAuthority> {
    if access == DirectoryAccess::Stage {
        return Err(SafeFsError::AccessMismatch {
            operation: SafeFsOperation::OpenDirectory,
        });
    }
    let names = absolute_names(path)?;
    let anchor_root = rustix::fs::open("/", DIR_FLAGS, Mode::empty())
        .map_err(|error| io(SafeFsOperation::CaptureNamespaceRoot, error))?;
    let walk_root = rustix::io::fcntl_dupfd_cloexec(&anchor_root, 0)
        .map_err(|error| io(SafeFsOperation::CaptureNamespaceRoot, error))?;
    let (tail, _, snapshot) = snapshot_from_root(walk_root, &names, SafeFsOperation::OpenAncestor)?;
    let opened = opened_metadata(&tail, SafeFsOperation::OpenAncestor)?;
    let case_mode = snapshot
        .components
        .last()
        .map_or(snapshot.root_case_mode, |row| row.case_mode);
    Ok(DirectoryAuthority {
        anchor: Arc::new(NamespaceAnchor {
            native: NativeNamespaceAnchor { root: anchor_root },
        }),
        native: NativeDirectory { fd: tail },
        access,
        opened,
        case_mode,
        snapshot,
    })
}

pub(super) fn revalidate_namespace(directory: &DirectoryAuthority) -> Result<()> {
    #[cfg(test)]
    super::test_seam::hit(super::test_seam::HookPoint::BeforeMappingRewalk);
    let root = rustix::fs::open("/", DIR_FLAGS, Mode::empty())
        .map_err(|error| io(SafeFsOperation::RevalidateNamespace, error))?;
    let names: Vec<ComponentName> = directory
        .snapshot
        .components
        .iter()
        .map(|row| row.name.clone())
        .collect();
    let (_, _, actual) = snapshot_from_root(root, &names, SafeFsOperation::RevalidateNamespace)
        .map_err(|error| match error {
            SafeFsError::NotFound { .. }
            | SafeFsError::SymlinkOrReparsePoint { .. }
            | SafeFsError::UnsupportedEntryType { .. }
            | SafeFsError::UnsupportedSecureFilesystem { .. } => SafeFsError::NamespaceChanged {
                operation: SafeFsOperation::RevalidateNamespace,
            },
            other => other,
        })?;
    if actual == directory.snapshot {
        Ok(())
    } else {
        Err(SafeFsError::NamespaceChanged {
            operation: SafeFsOperation::RevalidateNamespace,
        })
    }
}

pub(super) fn query_child_nofollow(
    parent: &DirectoryAuthority,
    name: &ComponentName,
) -> Result<ChildState> {
    name_metadata(parent, name, SafeFsOperation::QueryChild)
}

fn opened_child_directory(
    parent: &DirectoryAuthority,
    name: &ComponentName,
    access: DirectoryAccess,
    operation: SafeFsOperation,
) -> Result<DirectoryAuthority> {
    let (fd, opened) = open_dir_fd(&parent.native.fd, name, operation)?;
    let case_mode = probe_case_mode(&fd, &opened, operation)?;
    let mut snapshot = parent.snapshot.clone();
    snapshot.components.push(NamespaceComponent {
        name: name.clone(),
        identity: opened.identity.clone(),
        filesystem: opened
            .filesystem
            .clone()
            .expect("opened directory has filesystem proof"),
        case_mode,
    });
    Ok(DirectoryAuthority {
        anchor: Arc::clone(&parent.anchor),
        native: NativeDirectory { fd },
        access,
        opened,
        case_mode,
        snapshot,
    })
}

pub(super) fn open_dir_nofollow(
    parent: &DirectoryAuthority,
    name: &ComponentName,
    access: DirectoryAccess,
) -> Result<DirectoryAuthority> {
    if access == DirectoryAccess::Stage
        || (parent.access == DirectoryAccess::Read && access != DirectoryAccess::Read)
    {
        return Err(SafeFsError::AccessMismatch {
            operation: SafeFsOperation::OpenDirectory,
        });
    }
    opened_child_directory(parent, name, access, SafeFsOperation::OpenDirectory)
}

fn open_regular(
    parent: &DirectoryAuthority,
    name: &ComponentName,
    access: FileAccess,
    operation: SafeFsOperation,
) -> Result<FileCapability> {
    let flags = if access == FileAccess::Read {
        FILE_READ_FLAGS
    } else {
        FILE_RW_FLAGS
    };
    let fd = rustix::fs::openat(&parent.native.fd, name.as_os_str(), flags, Mode::empty())
        .map_err(|error| match error {
            rustix::io::Errno::NOENT => SafeFsError::NotFound { operation },
            rustix::io::Errno::LOOP => SafeFsError::SymlinkOrReparsePoint { operation },
            other => io(operation, other),
        })?;
    let opened = opened_metadata(&fd, operation)?;
    if opened.kind != EntryKind::RegularFile {
        return Err(SafeFsError::UnsupportedEntryType {
            operation,
            kind: opened.kind,
        });
    }
    let file = File::from(fd);
    Ok(FileCapability {
        native: NativeFile::Open(file),
        access,
        opened,
    })
}

pub(super) fn open_file_nofollow(
    parent: &DirectoryAuthority,
    name: &ComponentName,
    access: FileAccess,
) -> Result<FileCapability> {
    if parent.access == DirectoryAccess::Read && access != FileAccess::Read {
        return Err(SafeFsError::AccessMismatch {
            operation: SafeFsOperation::OpenFile,
        });
    }
    open_regular(parent, name, access, SafeFsOperation::OpenFile)
}

#[cfg(test)]
fn injected_create_failure(
    operation: SafeFsOperation,
    point: super::test_seam::CreateFailurePoint,
) -> Result<()> {
    if super::test_seam::take_create_failure(point) {
        return Err(SafeFsError::io(
            operation,
            std::io::Error::other(format!("injected post-create {point:?} failure")),
        ));
    }
    Ok(())
}

#[cfg(not(test))]
fn created_stat(fd: &OwnedFd, operation: SafeFsOperation) -> Result<rustix::fs::Stat> {
    rustix::fs::fstat(fd).map_err(|error| io(operation, error))
}

#[cfg(test)]
fn created_stat(fd: &OwnedFd, operation: SafeFsOperation) -> Result<rustix::fs::Stat> {
    injected_create_failure(operation, super::test_seam::CreateFailurePoint::Metadata)?;
    rustix::fs::fstat(fd).map_err(|error| io(operation, error))
}

fn created_metadata(
    fd: &OwnedFd,
    stat: &rustix::fs::Stat,
    operation: SafeFsOperation,
) -> Result<EntryMetadata> {
    #[cfg(test)]
    injected_create_failure(
        operation,
        super::test_seam::CreateFailurePoint::FilesystemProbe,
    )?;
    opened_metadata_from_stat(fd, stat, operation)
}

#[cfg(test)]
fn created_case_mode(
    fd: &OwnedFd,
    metadata: &EntryMetadata,
    operation: SafeFsOperation,
) -> Result<CaseMode> {
    #[cfg(test)]
    injected_create_failure(operation, super::test_seam::CreateFailurePoint::CaseProof)?;
    probe_case_mode(fd, metadata, operation)
}

fn random_create_rollback_name() -> Result<ComponentName> {
    let mut random = [0_u8; 16];
    // SAFETY: `random` is writable for exactly the supplied length; getentropy either fills
    // the complete buffer or fails on both supported Unix targets.
    if unsafe { libc::getentropy(random.as_mut_ptr().cast(), random.len()) } != 0 {
        return Err(SafeFsError::io(
            SafeFsOperation::RollbackCreatedEntry,
            std::io::Error::last_os_error(),
        ));
    }
    let mut suffix = String::with_capacity(32);
    for byte in random {
        use std::fmt::Write as _;
        write!(&mut suffix, "{byte:02x}").expect("writing to String cannot fail");
    }
    ComponentName::new(format!(".opentake-create-rollback-{suffix}"))
}

fn created_fail_leak(reason: StageIdentityLostReason) -> SafeFsError {
    SafeFsError::StageIdentityLost {
        operation: SafeFsOperation::RollbackCreatedEntry,
        reason,
    }
}

fn inject_created_identity_unavailable() -> bool {
    #[cfg(test)]
    {
        super::test_seam::take_rollback_failure(
            super::test_seam::RollbackFailurePoint::RetainedIdentity,
        )
    }
    #[cfg(not(test))]
    {
        false
    }
}

fn inject_created_quarantine_failure() -> bool {
    #[cfg(test)]
    {
        super::test_seam::take_rollback_failure(
            super::test_seam::RollbackFailurePoint::QuarantineMove,
        )
    }
    #[cfg(not(test))]
    {
        false
    }
}

fn inject_created_delete_failure() -> bool {
    #[cfg(test)]
    {
        super::test_seam::take_rollback_failure(super::test_seam::RollbackFailurePoint::Delete)
    }
    #[cfg(not(test))]
    {
        false
    }
}

fn rollback_created(
    parent: &DirectoryAuthority,
    original_name: &ComponentName,
    retained: &OwnedFd,
    expected: Option<StableIdentity>,
    expected_kind: EntryKind,
    original_error: SafeFsError,
) -> SafeFsError {
    // A retained fd is the sole source of truth. Failure to derive its identity is an
    // explicit fail-leak; rollback never guesses from the pathname.
    if inject_created_identity_unavailable() {
        return created_fail_leak(StageIdentityLostReason::CreatedObjectIdentityUnavailable);
    }
    let retained_stat = match rustix::fs::fstat(retained) {
        Ok(stat) => stat,
        Err(_) => {
            return created_fail_leak(StageIdentityLostReason::CreatedObjectIdentityUnavailable)
        }
    };
    let retained_identity = identity(&retained_stat);
    if expected
        .as_ref()
        .is_some_and(|value| value != &retained_identity)
        || kind(&retained_stat) != expected_kind
    {
        return created_fail_leak(StageIdentityLostReason::CreatedObjectIdentityUnavailable);
    }

    #[cfg(test)]
    super::test_seam::hit(super::test_seam::HookPoint::BeforeCreatedRollbackInitialNameCheck);

    let named = match rustix::fs::statat(
        &parent.native.fd,
        original_name.as_os_str(),
        AtFlags::SYMLINK_NOFOLLOW,
    ) {
        Ok(stat) => stat,
        Err(_) => return created_fail_leak(StageIdentityLostReason::CreatedNameChanged),
    };
    if identity(&named) != retained_identity || kind(&named) != expected_kind {
        return created_fail_leak(StageIdentityLostReason::CreatedNameChanged);
    }

    #[cfg(test)]
    super::test_seam::hit(super::test_seam::HookPoint::BeforeCreatedRollbackQuarantine);

    if inject_created_quarantine_failure() {
        return created_fail_leak(StageIdentityLostReason::CreatedRollbackQuarantineFailed);
    }
    let quarantine_name = {
        let mut selected = None;
        for _ in 0..8 {
            let candidate = match random_create_rollback_name() {
                Ok(candidate) => candidate,
                Err(_) => {
                    return created_fail_leak(
                        StageIdentityLostReason::CreatedRollbackQuarantineFailed,
                    )
                }
            };
            match rename_noreplace(
                parent,
                original_name,
                &candidate,
                SafeFsOperation::RollbackCreatedEntry,
            ) {
                Ok(()) => {
                    selected = Some(candidate);
                    break;
                }
                Err(SafeFsError::AlreadyExists { .. }) => continue,
                Err(SafeFsError::NotFound { .. }) => {
                    return created_fail_leak(StageIdentityLostReason::CreatedNameChanged)
                }
                Err(_) => {
                    return created_fail_leak(
                        StageIdentityLostReason::CreatedRollbackQuarantineFailed,
                    )
                }
            }
        }
        match selected {
            Some(name) => name,
            None => {
                return created_fail_leak(StageIdentityLostReason::CreatedRollbackQuarantineFailed)
            }
        }
    };

    let quarantined = match rustix::fs::statat(
        &parent.native.fd,
        quarantine_name.as_os_str(),
        AtFlags::SYMLINK_NOFOLLOW,
    ) {
        Ok(stat) => stat,
        Err(_) => {
            return created_fail_leak(StageIdentityLostReason::CreatedRollbackQuarantineChanged)
        }
    };
    if identity(&quarantined) != retained_identity || kind(&quarantined) != expected_kind {
        return created_fail_leak(StageIdentityLostReason::CreatedRollbackQuarantineChanged);
    }

    #[cfg(test)]
    super::test_seam::hit(super::test_seam::HookPoint::AfterCreatedRollbackVerifyBeforeDelete);

    // Re-read immediately before the name syscall. The remaining read-to-unlink window is
    // the same documented Unix same-account boundary as recursive cleanup.
    let final_stat = match rustix::fs::statat(
        &parent.native.fd,
        quarantine_name.as_os_str(),
        AtFlags::SYMLINK_NOFOLLOW,
    ) {
        Ok(stat) => stat,
        Err(_) => {
            return created_fail_leak(StageIdentityLostReason::CreatedRollbackQuarantineChanged)
        }
    };
    if identity(&final_stat) != retained_identity || kind(&final_stat) != expected_kind {
        return created_fail_leak(StageIdentityLostReason::CreatedRollbackQuarantineChanged);
    }
    let flags = if expected_kind == EntryKind::Directory {
        AtFlags::REMOVEDIR
    } else {
        AtFlags::empty()
    };
    if inject_created_delete_failure()
        || rustix::fs::unlinkat(&parent.native.fd, quarantine_name.as_os_str(), flags).is_err()
    {
        return created_fail_leak(StageIdentityLostReason::CreatedRollbackDeleteFailed);
    }
    original_error
}

#[cfg(test)]
fn open_created_directory(parent: &DirectoryAuthority, name: &ComponentName) -> Result<OwnedFd> {
    rustix::fs::openat(
        &parent.native.fd,
        name.as_os_str(),
        DIR_FLAGS,
        Mode::empty(),
    )
    .map_err(|_| created_fail_leak(StageIdentityLostReason::CreatedObjectIdentityUnavailable))
}

#[cfg(test)]
fn validate_created_directory(
    parent: &DirectoryAuthority,
    name: &ComponentName,
    fd: &OwnedFd,
    operation: SafeFsOperation,
) -> Result<(EntryMetadata, CaseMode, NamespaceSnapshot)> {
    let stat = created_stat(fd, operation)
        .map_err(|error| rollback_created(parent, name, fd, None, EntryKind::Directory, error))?;
    let expected = identity(&stat);
    if kind(&stat) != EntryKind::Directory {
        let error = SafeFsError::UnsupportedEntryType {
            operation,
            kind: kind(&stat),
        };
        return Err(rollback_created(
            parent,
            name,
            fd,
            Some(expected),
            EntryKind::Directory,
            error,
        ));
    }
    let opened = created_metadata(fd, &stat, operation).map_err(|error| {
        rollback_created(
            parent,
            name,
            fd,
            Some(expected.clone()),
            EntryKind::Directory,
            error,
        )
    })?;
    let case_mode = created_case_mode(fd, &opened, operation).map_err(|error| {
        rollback_created(
            parent,
            name,
            fd,
            Some(expected),
            EntryKind::Directory,
            error,
        )
    })?;
    let mut snapshot = parent.snapshot.clone();
    snapshot.components.push(NamespaceComponent {
        name: name.clone(),
        identity: opened.identity.clone(),
        filesystem: opened
            .filesystem
            .clone()
            .expect("created directories have filesystem proof"),
        case_mode,
    });
    Ok((opened, case_mode, snapshot))
}

pub(super) fn create_dir_new(
    _parent: &DirectoryAuthority,
    _name: &ComponentName,
    _permissions: CreatePermissions,
    _access: DirectoryAccess,
) -> Result<DirectoryAuthority> {
    Err(SafeFsError::UnsupportedAtomicPublish {
        operation: SafeFsOperation::CreateDirectory,
        reason: AtomicPublishReason::PrimitiveUnavailable,
    })
}

#[cfg(test)]
pub(super) fn create_dir_new_trusted_fixture(
    parent: &DirectoryAuthority,
    name: &ComponentName,
    permissions: CreatePermissions,
    access: DirectoryAccess,
) -> Result<DirectoryAuthority> {
    require_parent(parent, SafeFsOperation::CreateDirectory)?;
    if access == DirectoryAccess::Stage {
        return Err(SafeFsError::AccessMismatch {
            operation: SafeFsOperation::CreateDirectory,
        });
    }
    let mode = if permissions == CreatePermissions::OwnerOnly {
        OWNER_DIR_MODE
    } else {
        INHERIT_DIR_MODE
    };
    rustix::fs::mkdirat(&parent.native.fd, name.as_os_str(), mode).map_err(
        |error| match error {
            rustix::io::Errno::EXIST => SafeFsError::AlreadyExists {
                operation: SafeFsOperation::CreateDirectory,
            },
            other => io(SafeFsOperation::CreateDirectory, other),
        },
    )?;
    let fd = open_created_directory(parent, name)?;
    let (opened, case_mode, snapshot) =
        validate_created_directory(parent, name, &fd, SafeFsOperation::CreateDirectory)?;
    Ok(DirectoryAuthority {
        anchor: Arc::clone(&parent.anchor),
        native: NativeDirectory { fd },
        access,
        opened,
        case_mode,
        snapshot,
    })
}

pub(super) fn create_stage_dir_new(
    _parent: &DirectoryAuthority,
    _name: &ComponentName,
    _permissions: CreatePermissions,
) -> Result<StageCapability> {
    Err(SafeFsError::UnsupportedAtomicPublish {
        operation: SafeFsOperation::CreateStageDirectory,
        reason: AtomicPublishReason::PrimitiveUnavailable,
    })
}

#[cfg(test)]
pub(super) fn create_stage_dir_new_trusted_fixture(
    parent: &DirectoryAuthority,
    name: &ComponentName,
    permissions: CreatePermissions,
) -> Result<StageCapability> {
    require_parent(parent, SafeFsOperation::CreateStageDirectory)?;
    let mode = if permissions == CreatePermissions::OwnerOnly {
        OWNER_DIR_MODE
    } else {
        INHERIT_DIR_MODE
    };
    rustix::fs::mkdirat(&parent.native.fd, name.as_os_str(), mode).map_err(
        |error| match error {
            rustix::io::Errno::EXIST => SafeFsError::AlreadyExists {
                operation: SafeFsOperation::CreateStageDirectory,
            },
            other => io(SafeFsOperation::CreateStageDirectory, other),
        },
    )?;
    let fd = open_created_directory(parent, name)?;
    let (opened, case_mode, snapshot) =
        validate_created_directory(parent, name, &fd, SafeFsOperation::CreateStageDirectory)?;
    let directory = DirectoryAuthority {
        anchor: Arc::clone(&parent.anchor),
        native: NativeDirectory { fd },
        access: DirectoryAccess::Stage,
        opened: opened.clone(),
        case_mode,
        snapshot,
    };
    #[cfg(test)]
    if let Err(error) = injected_create_failure(
        SafeFsOperation::OpenDirectory,
        super::test_seam::CreateFailurePoint::ParentDuplicate,
    ) {
        return Err(rollback_created(
            parent,
            name,
            &directory.native.fd,
            Some(opened.identity.clone()),
            EntryKind::Directory,
            error,
        ));
    }
    let parent_copy =
        duplicate_directory(parent, DirectoryAccess::MutateChildren).map_err(|error| {
            rollback_created(
                parent,
                name,
                &directory.native.fd,
                Some(opened.identity.clone()),
                EntryKind::Directory,
                error,
            )
        })?;
    let opened = directory.opened.clone();
    Ok(StageCapability {
        parent: parent_copy,
        directory,
        original_name: name.clone(),
        opened,
    })
}

pub(super) fn create_file_new(
    parent: &DirectoryAuthority,
    name: &ComponentName,
    permissions: CreatePermissions,
) -> Result<FileCapability> {
    require_parent(parent, SafeFsOperation::CreateFile)?;
    let mode = if permissions == CreatePermissions::OwnerOnly {
        OWNER_FILE_MODE
    } else {
        INHERIT_FILE_MODE
    };
    let flags = FILE_RW_FLAGS.union(OFlags::CREATE).union(OFlags::EXCL);
    let fd =
        rustix::fs::openat(&parent.native.fd, name.as_os_str(), flags, mode).map_err(|error| {
            match error {
                rustix::io::Errno::EXIST => SafeFsError::AlreadyExists {
                    operation: SafeFsOperation::CreateFile,
                },
                other => io(SafeFsOperation::CreateFile, other),
            }
        })?;
    let stat = created_stat(&fd, SafeFsOperation::CreateFile).map_err(|error| {
        rollback_created(parent, name, &fd, None, EntryKind::RegularFile, error)
    })?;
    let expected = identity(&stat);
    if kind(&stat) != EntryKind::RegularFile {
        let error = SafeFsError::UnsupportedEntryType {
            operation: SafeFsOperation::CreateFile,
            kind: kind(&stat),
        };
        return Err(rollback_created(
            parent,
            name,
            &fd,
            Some(expected),
            EntryKind::RegularFile,
            error,
        ));
    }
    let opened = created_metadata(&fd, &stat, SafeFsOperation::CreateFile).map_err(|error| {
        rollback_created(
            parent,
            name,
            &fd,
            Some(expected),
            EntryKind::RegularFile,
            error,
        )
    })?;
    let file = File::from(fd);
    Ok(FileCapability {
        native: NativeFile::Open(file),
        access: FileAccess::ReadWrite,
        opened,
    })
}

pub(super) fn metadata_from_file(native: &NativeFile) -> Result<EntryMetadata> {
    match native {
        NativeFile::Open(file) => {
            use std::os::fd::AsFd as _;
            opened_metadata(file.as_fd(), SafeFsOperation::QueryMetadata)
        }
        NativeFile::NameOnly { .. } => Err(SafeFsError::AccessMismatch {
            operation: SafeFsOperation::QueryMetadata,
        }),
    }
}

pub(super) fn read_file(native: &mut NativeFile, buffer: &mut [u8]) -> Result<usize> {
    match native {
        NativeFile::Open(file) => file
            .read(buffer)
            .map_err(|error| SafeFsError::io(SafeFsOperation::ReadFile, error)),
        NativeFile::NameOnly { .. } => Err(SafeFsError::AccessMismatch {
            operation: SafeFsOperation::ReadFile,
        }),
    }
}
pub(super) fn write_file(native: &mut NativeFile, buffer: &[u8]) -> Result<usize> {
    match native {
        NativeFile::Open(file) => file
            .write(buffer)
            .map_err(|error| SafeFsError::io(SafeFsOperation::WriteFile, error)),
        NativeFile::NameOnly { .. } => Err(SafeFsError::AccessMismatch {
            operation: SafeFsOperation::WriteFile,
        }),
    }
}
pub(super) fn seek_file(native: &mut NativeFile, position: SeekFrom) -> Result<u64> {
    match native {
        NativeFile::Open(file) => file
            .seek(position)
            .map_err(|error| SafeFsError::io(SafeFsOperation::SeekFile, error)),
        NativeFile::NameOnly { .. } => Err(SafeFsError::AccessMismatch {
            operation: SafeFsOperation::SeekFile,
        }),
    }
}
pub(super) fn flush_file(native: &mut NativeFile) -> Result<()> {
    match native {
        NativeFile::Open(file) => file
            .flush()
            .map_err(|error| SafeFsError::io(SafeFsOperation::FlushFile, error)),
        NativeFile::NameOnly { .. } => Err(SafeFsError::AccessMismatch {
            operation: SafeFsOperation::FlushFile,
        }),
    }
}
pub(super) fn sync_file(native: &NativeFile) -> Result<()> {
    match native {
        NativeFile::Open(file) => file
            .sync_all()
            .map_err(|error| SafeFsError::io(SafeFsOperation::SyncFile, error)),
        NativeFile::NameOnly { .. } => Err(SafeFsError::AccessMismatch {
            operation: SafeFsOperation::SyncFile,
        }),
    }
}

// Bounded nofollow name discovery validates and returns every raw child component,
// including symlink/reparse, FIFO, socket, device, and other special-entry names.
// It neither filters by entry kind nor grants authority. Validation callers query and
// reject metadata explicitly; cleanup callers obtain consuming authority separately.

pub(super) fn enumerate(directory: &DirectoryAuthority) -> Result<Vec<ComponentName>> {
    let mut names = Vec::new();
    let stream = Dir::read_from(&directory.native.fd)
        .map_err(|error| io(SafeFsOperation::EnumerateDirectory, error))?;
    for item in stream {
        let item = item.map_err(|error| io(SafeFsOperation::EnumerateDirectory, error))?;
        let bytes = item.file_name().to_bytes();
        if bytes == b"." || bytes == b".." {
            continue;
        }
        let name = ComponentName::new(std::ffi::OsStr::from_bytes(bytes))?;
        if matches!(query_child_nofollow(directory, &name)?, ChildState::Absent) {
            return Err(SafeFsError::NotFound {
                operation: SafeFsOperation::EnumerateDirectory,
            });
        }
        names.push(name);
    }
    names.sort_by(|left, right| {
        left.as_os_str()
            .as_bytes()
            .cmp(right.as_os_str().as_bytes())
    });
    Ok(names)
}

pub(super) fn read_link_component(
    parent: &DirectoryAuthority,
    name: &ComponentName,
) -> Result<RawLinkTarget> {
    rustix::fs::readlinkat(&parent.native.fd, name.as_os_str(), Vec::new())
        .map(|value| RawLinkTarget::Unix(value.into_bytes()))
        .map_err(|error| match error {
            rustix::io::Errno::NOENT => SafeFsError::NotFound {
                operation: SafeFsOperation::ReadLink,
            },
            other => io(SafeFsOperation::ReadLink, other),
        })
}

fn rename_noreplace(
    parent: &DirectoryAuthority,
    from: &ComponentName,
    to: &ComponentName,
    operation: SafeFsOperation,
) -> Result<()> {
    rustix::fs::renameat_with(
        &parent.native.fd,
        from.as_os_str(),
        &parent.native.fd,
        to.as_os_str(),
        RenameFlags::NOREPLACE,
    )
    .map_err(|error| match error {
        rustix::io::Errno::NOENT => SafeFsError::NotFound { operation },
        rustix::io::Errno::EXIST | rustix::io::Errno::NOTEMPTY => {
            SafeFsError::AlreadyExists { operation }
        }
        rustix::io::Errno::NOSYS => SafeFsError::UnsupportedAtomicPublish {
            operation,
            reason: AtomicPublishReason::PrimitiveUnavailable,
        },
        value if value == rustix::io::Errno::NOTSUP || value == rustix::io::Errno::OPNOTSUPP => {
            SafeFsError::UnsupportedAtomicPublish {
                operation,
                reason: AtomicPublishReason::PrimitiveUnavailable,
            }
        }
        rustix::io::Errno::XDEV => SafeFsError::UnsupportedAtomicPublish {
            operation,
            reason: AtomicPublishReason::CrossDeviceInvariant,
        },
        other => io(operation, other),
    })
}

fn parent_matches(retained: &DirectoryAuthority, supplied: &DirectoryAuthority) -> bool {
    retained.opened.identity == supplied.opened.identity
        && retained.opened.filesystem == supplied.opened.filesystem
        && retained.case_mode == supplied.case_mode
        && retained.snapshot == supplied.snapshot
        && matches!(
            supplied.access,
            DirectoryAccess::MutateChildren | DirectoryAccess::Stage
        )
}

fn restore_or_fail_leak(
    parent: &DirectoryAuthority,
    original: &ComponentName,
    quarantine: &ComponentName,
    reason: StageIdentityLostReason,
) -> SafeFsError {
    #[cfg(test)]
    super::test_seam::hit(super::test_seam::HookPoint::BeforeQuarantineRestore);
    let mapped = match name_metadata(parent, original, SafeFsOperation::RestoreQuarantine) {
        Ok(ChildState::Absent) => match rename_noreplace(
            parent,
            quarantine,
            original,
            SafeFsOperation::RestoreQuarantine,
        ) {
            Ok(()) => reason,
            Err(SafeFsError::AlreadyExists { .. }) => StageIdentityLostReason::OriginalNameOccupied,
            Err(_) => StageIdentityLostReason::QuarantineRestoreFailed,
        },
        Ok(ChildState::Present(_)) => StageIdentityLostReason::OriginalNameOccupied,
        Err(_) => StageIdentityLostReason::QuarantineRestoreFailed,
    };
    SafeFsError::StageIdentityLost {
        operation: SafeFsOperation::RestoreQuarantine,
        reason: mapped,
    }
}

pub(super) fn quarantine_stage(
    stage: StageCapability,
    supplied_parent: &DirectoryAuthority,
    quarantine_name: ComponentName,
) -> Result<QuarantinedCapability> {
    if !parent_matches(&stage.parent, supplied_parent) {
        return Err(SafeFsError::StageIdentityLost {
            operation: SafeFsOperation::QuarantineNoReplace,
            reason: StageIdentityLostReason::ParentAuthorityChanged,
        });
    }
    #[cfg(test)]
    super::test_seam::hit(super::test_seam::HookPoint::BeforeQuarantineRename);
    rename_noreplace(
        &stage.parent,
        &stage.original_name,
        &quarantine_name,
        SafeFsOperation::QuarantineNoReplace,
    )?;
    let reopened = match opened_child_directory(
        &stage.parent,
        &quarantine_name,
        DirectoryAccess::Stage,
        SafeFsOperation::VerifyQuarantine,
    ) {
        Ok(directory)
            if directory.opened.identity == stage.opened.identity
                && directory.opened.filesystem == stage.opened.filesystem =>
        {
            directory
        }
        Ok(_) => {
            return Err(restore_or_fail_leak(
                &stage.parent,
                &stage.original_name,
                &quarantine_name,
                StageIdentityLostReason::QuarantinedObjectChanged,
            ))
        }
        Err(_) => {
            return Err(restore_or_fail_leak(
                &stage.parent,
                &stage.original_name,
                &quarantine_name,
                StageIdentityLostReason::AmbiguousNameMutation,
            ))
        }
    };
    Ok(QuarantinedCapability {
        parent: stage.parent,
        directory: reopened,
        original_name: stage.original_name,
        quarantine_name,
        opened: stage.opened,
    })
}

pub(super) fn publish_stage_noreplace(
    stage: StageCapability,
    supplied_parent: &DirectoryAuthority,
    destination: ComponentName,
) -> Result<()> {
    if !parent_matches(&stage.parent, supplied_parent) {
        return Err(SafeFsError::StageIdentityLost {
            operation: SafeFsOperation::PublishNoReplace,
            reason: StageIdentityLostReason::ParentAuthorityChanged,
        });
    }
    revalidate_namespace(&stage.directory)?;
    let current = name_metadata(
        &stage.parent,
        &stage.original_name,
        SafeFsOperation::PublishNoReplace,
    )?;
    if !matches!(current, ChildState::Present(ref metadata) if metadata.identity == stage.opened.identity)
    {
        return Err(SafeFsError::StageIdentityLost {
            operation: SafeFsOperation::PublishNoReplace,
            reason: StageIdentityLostReason::SourceChangedBeforeQuarantine,
        });
    }
    rename_noreplace(
        &stage.parent,
        &stage.original_name,
        &destination,
        SafeFsOperation::PublishNoReplace,
    )
}

fn nested_quarantined(
    parent: &QuarantinedCapability,
    name: &ComponentName,
    directory: DirectoryAuthority,
) -> Result<QuarantinedCapability> {
    let opened = directory.opened.clone();
    Ok(QuarantinedCapability {
        parent: duplicate_directory(parent.directory(), DirectoryAccess::MutateChildren)?,
        directory,
        original_name: name.clone(),
        quarantine_name: name.clone(),
        opened,
    })
}

pub(super) fn open_cleanup_child_nofollow(
    parent: &QuarantinedCapability,
    name: &ComponentName,
) -> Result<CleanupCapability> {
    let metadata = match name_metadata(parent.directory(), name, SafeFsOperation::OpenCleanupEntry)?
    {
        ChildState::Absent => {
            return Err(SafeFsError::NotFound {
                operation: SafeFsOperation::OpenCleanupEntry,
            })
        }
        ChildState::Present(metadata) => metadata,
    };
    if metadata.kind == EntryKind::Directory {
        let directory = opened_child_directory(
            parent.directory(),
            name,
            DirectoryAccess::Stage,
            SafeFsOperation::OpenCleanupEntry,
        )?;
        if directory.opened.identity != metadata.identity {
            return Err(SafeFsError::IdentityChanged {
                operation: SafeFsOperation::OpenCleanupEntry,
                expected: metadata.identity,
                actual: directory.opened.identity,
            });
        }
        return Ok(CleanupCapability::Directory(Box::new(nested_quarantined(
            parent, name, directory,
        )?)));
    }
    let parent_copy = duplicate_directory(parent.directory(), DirectoryAccess::MutateChildren)?;
    Ok(CleanupCapability::Entry(Box::new(CleanupEntry {
        parent: parent_copy,
        native: NativeFile::NameOnly {
            name: name.clone(),
            expected: metadata.identity.clone(),
            kind: metadata.kind,
        },
        name: name.clone(),
        opened: metadata,
        access: CleanupAccess::Delete,
    })))
}

pub(super) fn delete_quarantined_entry(entry: CleanupCapability) -> Result<()> {
    let CleanupCapability::Entry(entry) = entry else {
        return Err(SafeFsError::AccessMismatch {
            operation: SafeFsOperation::DeleteQuarantinedEntry,
        });
    };
    let CleanupEntry {
        parent,
        native,
        name,
        opened,
        access: CleanupAccess::Delete,
    } = *entry;
    let NativeFile::NameOnly {
        name: native_name,
        expected,
        kind,
    } = native
    else {
        return Err(SafeFsError::AccessMismatch {
            operation: SafeFsOperation::DeleteQuarantinedEntry,
        });
    };
    if name != native_name || opened.identity != expected || opened.kind != kind {
        return Err(SafeFsError::StageIdentityLost {
            operation: SafeFsOperation::DeleteQuarantinedEntry,
            reason: StageIdentityLostReason::QuarantineNameChanged,
        });
    }
    let retained_parent = rustix::fs::fstat(&parent.native.fd)
        .map_err(|error| io(SafeFsOperation::DeleteQuarantinedEntry, error))?;
    if identity(&retained_parent) != parent.opened.identity {
        return Err(SafeFsError::StageIdentityLost {
            operation: SafeFsOperation::DeleteQuarantinedEntry,
            reason: StageIdentityLostReason::ParentAuthorityChanged,
        });
    }
    let stat = rustix::fs::statat(
        &parent.native.fd,
        native_name.as_os_str(),
        AtFlags::SYMLINK_NOFOLLOW,
    )
    .map_err(|error| match error {
        rustix::io::Errno::NOENT => SafeFsError::StageIdentityLost {
            operation: SafeFsOperation::DeleteQuarantinedEntry,
            reason: StageIdentityLostReason::QuarantineNameChanged,
        },
        other => io(SafeFsOperation::DeleteQuarantinedEntry, other),
    })?;
    let actual = identity(&stat);
    if actual != expected {
        return Err(SafeFsError::IdentityChanged {
            operation: SafeFsOperation::DeleteQuarantinedEntry,
            expected,
            actual,
        });
    }
    #[cfg(test)]
    super::test_seam::hit(super::test_seam::HookPoint::AfterFinalIdentityReadBeforeNameSyscall);
    rustix::fs::unlinkat(&parent.native.fd, native_name.as_os_str(), AtFlags::empty()).map_err(
        |error| match error {
            rustix::io::Errno::NOENT => SafeFsError::StageIdentityLost {
                operation: SafeFsOperation::DeleteQuarantinedEntry,
                reason: StageIdentityLostReason::QuarantineNameChanged,
            },
            other => io(SafeFsOperation::DeleteQuarantinedEntry, other),
        },
    )
}

pub(super) fn delete_quarantined_empty_directory(directory: QuarantinedCapability) -> Result<()> {
    let stat = rustix::fs::statat(
        &directory.parent.native.fd,
        directory.quarantine_name.as_os_str(),
        AtFlags::SYMLINK_NOFOLLOW,
    )
    .map_err(|error| match error {
        rustix::io::Errno::NOENT => SafeFsError::StageIdentityLost {
            operation: SafeFsOperation::DeleteQuarantinedEmptyDirectory,
            reason: StageIdentityLostReason::QuarantineNameChanged,
        },
        other => io(SafeFsOperation::DeleteQuarantinedEmptyDirectory, other),
    })?;
    let actual = identity(&stat);
    if actual != directory.opened.identity {
        return Err(SafeFsError::IdentityChanged {
            operation: SafeFsOperation::DeleteQuarantinedEmptyDirectory,
            expected: directory.opened.identity,
            actual,
        });
    }
    #[cfg(test)]
    super::test_seam::hit(super::test_seam::HookPoint::AfterFinalIdentityReadBeforeNameSyscall);
    rustix::fs::unlinkat(
        &directory.parent.native.fd,
        directory.quarantine_name.as_os_str(),
        AtFlags::REMOVEDIR,
    )
    .map_err(|error| match error {
        rustix::io::Errno::NOENT => SafeFsError::StageIdentityLost {
            operation: SafeFsOperation::DeleteQuarantinedEmptyDirectory,
            reason: StageIdentityLostReason::QuarantineNameChanged,
        },
        other => io(SafeFsOperation::DeleteQuarantinedEmptyDirectory, other),
    })
}
