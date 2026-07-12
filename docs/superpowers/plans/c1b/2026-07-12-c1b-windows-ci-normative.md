# C1B Windows, Native CI, and Evidence Normative Appendix

## 0. 绑定范围与来源

本稿只修订 C1B 计划中的 Windows、三平台原生 CI 与 evidence 部分，并给主计划必须同步采用的 common-facade 修正。它不修改 OpenTake 仓库，不恢复 bundle command/UI，也不授权 push、开 PR 或修改远端状态。

绑定版本：

- 批准 design：`31bfd57e40e3a2bd0ca42b331e5aa877db2d6ace`
- attempt-1 plan：`1b3305ac752977301f9af19fe4e7937d628e0100`
- C1B baseline：`e67917260ace36e4db1ede4e36eecbc401825bb1`
- 本机 API：`windows-sys = 0.61.2`
- safety root：`/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake-safety/20260712-wave1bc-filesystem`
- integration repo root：`/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake-full-convergence`

attempt 2 必须删除 attempt 1 的以下决定：

1. 统一使用 common appendix 的递归 `DirectoryAuthority`；每个打开/创建的目录 authority 都能作为下一层 parent。
2. 不再把 file capability 做成只能查 metadata 的死端。它提供受控 `read`、`write_all`、`rewind`、`flush`，raw HANDLE 不外泄。
3. Windows rename 不使用 `SetFileInformationByHandle(FILE_RENAME_INFO)`。唯一允许的调用是 `NtSetInformationFile(FileRenameInformation)` 配 `FILE_RENAME_INFORMATION.RootDirectory`。
4. Windows delete 不使用名称型 `DeleteFileW`/`RemoveDirectoryW`。它对 retained child HANDLE 调 `NtSetInformationFile(FileDispositionInformation)`。
5. `RtlNtStatusToDosError` 只作未识别状态的诊断 fallback；控制流先匹配原始 `NTSTATUS`。
6. 原生 receipt 没有远端发布权限时必须写成 `BLOCKED`；不得自行 push、开 PR 或把 cross-check 冒充原生行为结果。

## 1. Common facade 必须同步改正

### 1.1 Authority 形状

Common facade 以 common/Unix appendix section 2 为唯一来源。attempt 3 的 Windows adapter 只包含下面这些既有 common symbols；名称和参数逐项一致，`windows.rs` 不包含兼容别名或第二 facade：

```text
platform::{NativeNamespaceAnchor, NativeDirectory, NativeFile};

pub(crate) struct DirectoryAuthority { /* native: NativeDirectory; move-only */ }
pub(crate) struct FileCapability { /* native: NativeFile; move-only */ }
pub(crate) struct StageCapability { /* parent + directory; directory HANDLE has DELETE */ }
pub(crate) struct QuarantinedCapability { /* same parent + directory HANDLE */ }
pub(crate) enum CleanupCapability { /* Entry owns NativeFile; Directory owns QuarantinedCapability */ }

platform::capture_absolute_directory(&Path, DirectoryAccess) -> Result<DirectoryAuthority>;
platform::revalidate_namespace(&DirectoryAuthority) -> Result<()>;
platform::query_child_nofollow(&DirectoryAuthority, &ComponentName) -> Result<ChildState>;
platform::open_dir_nofollow(&DirectoryAuthority, &ComponentName, DirectoryAccess) -> Result<DirectoryAuthority>;
platform::open_file_nofollow(&DirectoryAuthority, &ComponentName, FileAccess) -> Result<FileCapability>;
platform::create_dir_new(&DirectoryAuthority, &ComponentName, CreatePermissions, DirectoryAccess) -> Result<DirectoryAuthority>;
platform::create_file_new(&DirectoryAuthority, &ComponentName, CreatePermissions) -> Result<FileCapability>;
platform::create_stage_dir_new(&DirectoryAuthority, &ComponentName, CreatePermissions) -> Result<StageCapability>;
platform::enumerate(&DirectoryAuthority) -> Result<Vec<ComponentName>>;
platform::read_link_component(&DirectoryAuthority, &ComponentName) -> Result<RawLinkTarget>;
platform::metadata_from_file(&FileCapability) -> Result<EntryMetadata>;
platform::read_file(&mut NativeFile, &mut [u8]) -> Result<usize>;
platform::write_file(&mut NativeFile, &[u8]) -> Result<usize>;
platform::seek_file(&mut NativeFile, SeekFrom) -> Result<u64>;
platform::flush_file(&mut NativeFile) -> Result<()>;
platform::sync_file(&NativeFile) -> Result<()>;
platform::quarantine_stage(StageCapability, &DirectoryAuthority, ComponentName) -> Result<QuarantinedCapability>;
platform::publish_stage_noreplace(StageCapability, &DirectoryAuthority, ComponentName) -> Result<()>;
platform::open_cleanup_child_nofollow(&QuarantinedCapability, &ComponentName) -> Result<CleanupCapability>;
platform::delete_quarantined_entry(CleanupCapability) -> Result<()>;
platform::delete_quarantined_empty_directory(QuarantinedCapability) -> Result<()>;
```

`DirectoryAuthority` 是唯一递归目录 authority；absolute capture、nofollow child open、exclusive child create 都返回它，且每个结果都能作为下一层 parent。`FileCapability` 的 `Read`/`Write`/`Seek`/flush/sync 只委托上述 `platform::*_file`，不保存第二个 `std::fs::File`。Windows 的 `NativeDirectory` 以 `Arc<DirectoryNode>` 保留当前 HANDLE 与 parent chain；外层 authority、file/stage/quarantine/cleanup capability 全部不实现 `Clone`，raw HANDLE 不外泄。

Windows 私有 `DirectoryAccess::{Read, MutateChildren}` 只决定目录自身 child rights。`create_stage_dir_new` 创建的 `NativeDirectory` 从 `NtCreateFile(FILE_CREATE)` 起就带 `DELETE`，`quarantine_stage`、`publish_stage_noreplace` 和 directory delete 都消费并移动这一个 directory HANDLE。cleanup leaf/subdir 由 `open_cleanup_child_nofollow` 第一次打开时取得 DELETE，并把同一个 source HANDLE 放进 consuming capability；禁止重开或 duplicate source HANDLE。为了让递归 child capability 拥有 parent authority，adapter 只允许 `DuplicateHandle(..., DUPLICATE_SAME_ACCESS)` 复制 retained parent directory HANDLE（同一 kernel object，不按名称重开）。普通 ancestor/destination parent 不请求 self-delete。所有 retained HANDLE 的 share mask 都是 `FILE_SHARE_READ | FILE_SHARE_WRITE`，刻意省略 `FILE_SHARE_DELETE`。

Windows delete/rename 是 retained-HANDLE identity binding。Unix cleanup 采用 common/Unix appendix 的 consuming capability facade 与 quarantine → nofollow reopen/identity verify → one-shot no-replace restore/fail-leak 协议；final publish 仍在批准 design 的 same-account boundary 内按名称 no-replace，且不声称 Unix handle-identity atomicity。

### 1.2 私有错误必须保留 raw status

错误类型的唯一完整定义是 common/Unix appendix `safe_fs/error.rs`。Windows 必须使用其中带 `operation` 的结构化 variants、`RawOsError::{NtStatus,Win32,Errno}` 与 `NativeBufferReason`；不得维护第二份缩写 enum。`query_child_nofollow` 的 name/path-not-found 是 `Ok(ChildState::Absent)`；open/read/delete 的同一状态是带对应 operation 的 `Err(NotFound { .. })`。其余 operation 不得把 absence、ACL、sharing、delete-pending、collision 混为一类。

## 2. Windows dependency 与模块边界

最终 target dependency 固定为：

```toml
[target.'cfg(windows)'.dependencies]
windows-sys = { version = "=0.61.2", features = [
  "Wdk_Foundation",
  "Wdk_Storage_FileSystem",
  "Win32_Foundation",
  "Win32_Security",
  "Win32_Storage_FileSystem",
  "Win32_System_IO",
  "Win32_System_Ioctl",
  "Win32_System_SystemServices",
  "Win32_System_Threading",
] }
```

`windows.rs` 顶部必须是 `#![deny(unsafe_op_in_unsafe_fn)]`。所有 FFI helper 都是小函数；每个 `unsafe` block 注明 HANDLE validity、buffer alignment/length、pointer lifetime 和 output initialization。RAII wrapper 只接受非空且非 `INVALID_HANDLE_VALUE` 的成功 HANDLE，`Drop` 恰好调用一次 `CloseHandle`，不实现 `Clone`/raw-handle public getter。

### 2.1 `windows.rs` compile-complete 文件骨架

下面是 `windows.rs` 的首段；sections 3–11 的后续 code blocks 按文档顺序紧接其后，所有同名函数均只有一个定义。整份附录没有 stub、`_real` 或 `_impl` 旁路：

```rust
#![deny(unsafe_op_in_unsafe_fn)]

use super::capability::*;
use super::component::ComponentName;
use super::error::*;
use std::ffi::{c_void, OsStr, OsString};
use std::io::{self, SeekFrom};
use std::mem::{align_of, offset_of, size_of};
use std::os::windows::ffi::{OsStrExt, OsStringExt};
use std::path::{Component, Path, PathBuf, Prefix};
use std::ptr::{null, null_mut};
use std::sync::Arc;
use windows_sys::Wdk::Foundation::{OBJECT_ATTRIBUTES, OBJ_CASE_INSENSITIVE};
use windows_sys::Wdk::Storage::FileSystem::*;
use windows_sys::Win32::Foundation::{
    CloseHandle, DuplicateHandle, GetLastError, HANDLE, INVALID_HANDLE_VALUE, NTSTATUS,
    DUPLICATE_SAME_ACCESS, STATUS_ACCESS_DENIED,
    STATUS_BUFFER_OVERFLOW, STATUS_CANNOT_DELETE, STATUS_DELETE_PENDING,
    STATUS_DIRECTORY_NOT_EMPTY, STATUS_END_OF_FILE, STATUS_FILE_IS_A_DIRECTORY,
    STATUS_INFO_LENGTH_MISMATCH, STATUS_INVALID_PARAMETER, STATUS_NO_MORE_FILES,
    STATUS_NOT_A_DIRECTORY, STATUS_NOT_SUPPORTED, STATUS_OBJECT_NAME_COLLISION,
    STATUS_OBJECT_NAME_NOT_FOUND, STATUS_OBJECT_PATH_NOT_FOUND, STATUS_OBJECT_TYPE_MISMATCH,
    STATUS_PENDING, STATUS_REPARSE_POINT_ENCOUNTERED, STATUS_SHARING_VIOLATION,
    RtlNtStatusToDosError,
    UNICODE_STRING,
};
use windows_sys::Win32::Security::*;
use windows_sys::Win32::Storage::FileSystem::{
    CreateFileW, GetDriveTypeW, GetFileInformationByHandleEx, GetVolumeInformationByHandleW,
    GetVolumeNameForVolumeMountPointW, GetVolumePathNameW, FileAttributeTagInfo,
    FileIdInfo, FileRemoteProtocolInfo, FileStandardInfo, DELETE, DRIVE_FIXED, DRIVE_REMOVABLE,
    FILE_ACCESS_RIGHTS, FILE_ATTRIBUTE_NORMAL,
    FILE_ATTRIBUTE_REPARSE_POINT, FILE_FLAGS_AND_ATTRIBUTES, FILE_FLAG_BACKUP_SEMANTICS,
    FILE_FLAG_OPEN_REPARSE_POINT, FILE_ATTRIBUTE_TAG_INFO, FILE_ID_INFO, FILE_REMOTE_PROTOCOL_INFO,
    FILE_SHARE_MODE, FILE_SHARE_READ, FILE_SHARE_WRITE, FILE_STANDARD_INFO,
    GET_FILEEX_INFO_LEVELS, OPEN_EXISTING, SYNCHRONIZE,
};
use windows_sys::Win32::System::IO::IO_STATUS_BLOCK;
use windows_sys::Win32::System::Ioctl::{FSCTL_GET_REPARSE_POINT, MAXIMUM_REPARSE_DATA_BUFFER_SIZE};
use windows_sys::Win32::System::SystemServices::FILE_CS_FLAG_CASE_SENSITIVE_DIR;
use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

const SHARE: FILE_SHARE_MODE = FILE_SHARE_READ | FILE_SHARE_WRITE;
const COMMON_OPTIONS: NTCREATEFILE_CREATE_OPTIONS =
    FILE_OPEN_REPARSE_POINT | FILE_OPEN_FOR_BACKUP_INTENT | FILE_SYNCHRONOUS_IO_NONALERT;
const DIRECTORY_BUFFER_BYTES: usize = 64 * 1024;
const REPARSE_HEADER_BYTES: usize = 8;
const STATUS_SUCCESS: NTSTATUS = 0;

impl CaseMode {
    fn object_attributes(self) -> u32 {
        match self { Self::Sensitive => 0, Self::Insensitive => OBJ_CASE_INSENSITIVE }
    }
}

#[derive(Debug)]
struct OwnedHandle(HANDLE);

impl OwnedHandle {
    fn new(raw: HANDLE, operation: SafeFsOperation) -> Result<Self> {
        if raw.is_null() || raw == INVALID_HANDLE_VALUE {
            return Err(last_win32(operation));
        }
        Ok(Self(raw))
    }
    fn raw(&self) -> HANDLE { self.0 }
}

impl Drop for OwnedHandle {
    fn drop(&mut self) {
        // SAFETY: `OwnedHandle::new` accepted one live owned HANDLE and this is its only Drop.
        let closed = unsafe { CloseHandle(self.0) };
        debug_assert_ne!(closed, 0, "CloseHandle failed");
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct VolumeProof {
    mapping: Vec<u16>,
    guid: Vec<u16>,
    volume_serial32: u32,
    volume_serial: u64,
    root_id: [u8; 16],
}

#[derive(Debug)]
struct DirectoryNode {
    handle: OwnedHandle,
    parent: Option<Arc<DirectoryNode>>,
    name: Option<ComponentName>,
    case_mode: CaseMode,
    metadata: EntryMetadata,
    volume: VolumeProof,
}

pub(super) struct NativeNamespaceAnchor { root: Arc<DirectoryNode>, mapping: VolumeProof, absolute_path: PathBuf, base_components: usize, access: DirectoryAccess }
pub(super) struct NativeDirectory { node: Arc<DirectoryNode>, access: DirectoryAccess, delete_right: bool }
pub(super) struct NativeFile {
    handle: OwnedHandle,
    opened: EntryMetadata,
    access: FileAccess,
    delete_right: bool,
}

pub(super) fn read_file(file: &mut NativeFile, out: &mut [u8]) -> Result<usize> {
    nt_read(file.handle.raw(), out)
}
pub(super) fn write_file(file: &mut NativeFile, input: &[u8]) -> Result<usize> {
    if file.access == FileAccess::Read {
        return Err(SafeFsError::io(SafeFsOperation::WriteFile,
            io::Error::new(io::ErrorKind::PermissionDenied, "read-only capability")));
    }
    nt_write(file.handle.raw(), input)
}
pub(super) fn flush_file(file: &mut NativeFile) -> Result<()> { nt_flush(file.handle.raw()) }
pub(super) fn seek_file(file: &mut NativeFile, position: SeekFrom) -> Result<u64> {
    nt_seek(file.handle.raw(), position)
}
pub(super) fn sync_file(file: &NativeFile) -> Result<()> { nt_flush(file.handle.raw()) }

fn last_win32(operation: SafeFsOperation) -> SafeFsError {
    // SAFETY: GetLastError has no pointer or lifetime preconditions.
    SafeFsError::Os { operation, raw: RawOsError::Win32(unsafe { GetLastError() }) }
}

struct NtName { units: Vec<u16>, unicode: UNICODE_STRING }

impl NtName {
    fn new(name: &ComponentName) -> Result<Self> {
        let units: Vec<u16> = name.as_os_str().encode_wide().collect();
        let byte_len = units.len().checked_mul(size_of::<u16>())
            .and_then(|value| u16::try_from(value).ok())
            .ok_or(SafeFsError::InvalidComponent(ComponentViolation::TooLong))?;
        let unicode = UNICODE_STRING {
            Length: byte_len,
            MaximumLength: byte_len,
            Buffer: units.as_ptr().cast_mut(),
        };
        Ok(Self { units, unicode })
    }
    fn unicode_ptr(&self) -> *const UNICODE_STRING {
        debug_assert_eq!(self.unicode.Buffer, self.units.as_ptr().cast_mut());
        &self.unicode
    }
}

fn object_attributes(parent: HANDLE, name: &NtName, case: CaseMode, security: *const c_void) -> OBJECT_ATTRIBUTES {
    OBJECT_ATTRIBUTES {
        Length: u32::try_from(size_of::<OBJECT_ATTRIBUTES>()).expect("OBJECT_ATTRIBUTES size fits u32"),
        RootDirectory: parent,
        ObjectName: name.unicode_ptr(),
        Attributes: case.object_attributes(),
        SecurityDescriptor: security,
        SecurityQualityOfService: null(),
    }
}

fn iosb_status(iosb: &IO_STATUS_BLOCK) -> NTSTATUS {
    // SAFETY: kernel initialized the Status union field before a completed synchronous call returns.
    unsafe { iosb.Anonymous.Status }
}

fn checked_information(operation: SafeFsOperation, iosb: &IO_STATUS_BLOCK, capacity: usize) -> Result<usize> {
    let used = iosb.Information;
    if used > capacity {
        return Err(SafeFsError::InvalidNativeBuffer {
            operation,
            reason: NativeBufferReason::IoStatusInformationOutOfBounds,
        });
    }
    Ok(used)
}

fn complete_nt(operation: SafeFsOperation, returned: NTSTATUS, iosb: &IO_STATUS_BLOCK) -> Result<()> {
    if returned == STATUS_PENDING {
        return Err(SafeFsError::InvalidNativeBuffer {
            operation,
            reason: NativeBufferReason::PendingOnSynchronousHandle,
        });
    }
    if returned < STATUS_SUCCESS { return Err(nt_error(operation, returned)); }
    let final_status = iosb_status(iosb);
    if final_status < STATUS_SUCCESS { return Err(nt_error(operation, final_status)); }
    Ok(())
}

fn nt_create_relative(
    parent: HANDLE,
    name: &ComponentName,
    case: CaseMode,
    desired: FILE_ACCESS_RIGHTS,
    disposition: NTCREATEFILE_CREATE_DISPOSITION,
    options: NTCREATEFILE_CREATE_OPTIONS,
    attributes: FILE_FLAGS_AND_ATTRIBUTES,
    security: *const c_void,
    operation: SafeFsOperation,
) -> Result<OwnedHandle> {
    let nt_name = NtName::new(name)?;
    let attrs = object_attributes(parent, &nt_name, case, security);
    let mut raw = null_mut();
    let mut iosb = IO_STATUS_BLOCK::default();
    // SAFETY: raw/iosb are writable; attrs/name/security remain live; parent is retained; no EA buffer.
    let status = unsafe {
        NtCreateFile(&mut raw, desired | SYNCHRONIZE, &attrs, &mut iosb, null(), attributes,
            SHARE, disposition, options | COMMON_OPTIONS, null(), 0)
    };
    complete_nt(operation, status, &iosb)?;
    OwnedHandle::new(raw, operation)
}

fn nt_read(handle: HANDLE, output: &mut [u8]) -> Result<usize> {
    if output.is_empty() { return Ok(0); }
    let length = u32::try_from(output.len().min(u32::MAX as usize)).expect("bounded read length");
    let mut iosb = IO_STATUS_BLOCK::default();
    // SAFETY: retained synchronous handle; output is writable for length; null event/APC/offset uses file position.
    let status = unsafe { NtReadFile(handle, null_mut(), None, null(), &mut iosb,
        output.as_mut_ptr().cast(), length, null(), null()) };
    if status == STATUS_END_OF_FILE { return Ok(0); }
    complete_nt(SafeFsOperation::ReadFile, status, &iosb)?;
    checked_information(SafeFsOperation::ReadFile, &iosb, length as usize)
}

fn nt_write(handle: HANDLE, input: &[u8]) -> Result<usize> {
    if input.is_empty() { return Ok(0); }
    let length = u32::try_from(input.len().min(u32::MAX as usize)).expect("bounded write length");
    let mut iosb = IO_STATUS_BLOCK::default();
    // SAFETY: retained synchronous handle; input is readable for length; null event/APC/offset uses file position.
    let status = unsafe { NtWriteFile(handle, null_mut(), None, null(), &mut iosb,
        input.as_ptr().cast(), length, null(), null()) };
    complete_nt(SafeFsOperation::WriteFile, status, &iosb)?;
    let written = checked_information(SafeFsOperation::WriteFile, &iosb, length as usize)?;
    if written == 0 {
        return Err(SafeFsError::InvalidNativeBuffer {
            operation: SafeFsOperation::WriteFile,
            reason: NativeBufferReason::WriteZero,
        });
    }
    Ok(written)
}

fn nt_flush(handle: HANDLE) -> Result<()> {
    let mut iosb = IO_STATUS_BLOCK::default();
    // SAFETY: handle is retained and iosb is writable for the synchronous flush.
    let status = unsafe { NtFlushBuffersFile(handle, &mut iosb) };
    complete_nt(SafeFsOperation::FlushFile, status, &iosb)
}

fn nt_seek(handle: HANDLE, position: SeekFrom) -> Result<u64> {
    let current = query_position(handle)?;
    let end = query_standard(handle)?.EndOfFile;
    let next = match position {
        SeekFrom::Start(value) => Some(i64::try_from(value).map_err(|_| SafeFsError::InvalidNativeBuffer {
            operation: SafeFsOperation::SeekFile, reason: NativeBufferReason::LengthOverflow })?),
        SeekFrom::Current(delta) => current.checked_add(delta),
        SeekFrom::End(delta) => end.checked_add(delta),
    };
    let next = match next {
        Some(value) => value,
        None => return Err(SafeFsError::InvalidNativeBuffer {
            operation: SafeFsOperation::SeekFile, reason: NativeBufferReason::LengthOverflow }),
    };
    if next < 0 { return Err(SafeFsError::io(SafeFsOperation::SeekFile,
        io::Error::new(io::ErrorKind::InvalidInput, "negative seek"))); }
    let info = FILE_POSITION_INFORMATION { CurrentByteOffset: next };
    let mut iosb = IO_STATUS_BLOCK::default();
    // SAFETY: handle retained; fixed-size initialized info and writable iosb stay live.
    let status = unsafe { NtSetInformationFile(handle, &mut iosb, (&info as *const _).cast(),
        u32::try_from(size_of::<FILE_POSITION_INFORMATION>()).expect("position info fits"),
        FilePositionInformation) };
    complete_nt(SafeFsOperation::SeekFile, status, &iosb)?;
    Ok(next as u64)
}

fn query_position(handle: HANDLE) -> Result<i64> {
    let mut info = FILE_POSITION_INFORMATION::default();
    query_fixed(handle, FilePositionInformation, SafeFsOperation::SeekFile, &mut info)?;
    Ok(info.CurrentByteOffset)
}

fn query_standard(handle: HANDLE) -> Result<FILE_STANDARD_INFORMATION> {
    let mut info = FILE_STANDARD_INFORMATION::default();
    query_fixed(handle, FileStandardInformation, SafeFsOperation::QueryMetadata, &mut info)?;
    Ok(info)
}

fn query_fixed<T>(handle: HANDLE, class: FILE_INFORMATION_CLASS, operation: SafeFsOperation, output: &mut T) -> Result<()> {
    let mut iosb = IO_STATUS_BLOCK::default();
    // SAFETY: output is writable for exactly size_of::<T>(); handle retained; iosb writable.
    let status = unsafe { NtQueryInformationFile(handle, &mut iosb, (output as *mut T).cast(),
        u32::try_from(size_of::<T>()).expect("query structure fits u32"), class) };
    complete_nt(operation, status, &iosb)?;
    let used = checked_information(operation, &iosb, size_of::<T>())?;
    if used != size_of::<T>() {
        return Err(SafeFsError::InvalidNativeBuffer { operation, reason: NativeBufferReason::LengthOverflow });
    }
    Ok(())
}

fn query_case_mode(handle: HANDLE) -> Result<CaseMode> {
    let mut info = FILE_CASE_SENSITIVE_INFORMATION::default();
    query_fixed(handle, FileCaseSensitiveInformation, SafeFsOperation::QueryCaseMode, &mut info)?;
    if info.Flags & !FILE_CS_FLAG_CASE_SENSITIVE_DIR != 0 {
        return Err(SafeFsError::InvalidNativeBuffer {
            operation: SafeFsOperation::QueryCaseMode,
            reason: NativeBufferReason::UnknownCaseFlags,
        });
    }
    Ok(if info.Flags & FILE_CS_FLAG_CASE_SENSITIVE_DIR != 0 { CaseMode::Sensitive } else { CaseMode::Insensitive })
}

fn win32_query<T: Default>(handle: HANDLE, class: GET_FILEEX_INFO_LEVELS, operation: SafeFsOperation) -> Result<T> {
    let mut output = T::default();
    // SAFETY: output is writable for the class-specific fixed T and handle retained.
    if unsafe { GetFileInformationByHandleEx(handle, class, (&mut output as *mut T).cast(),
        u32::try_from(size_of::<T>()).expect("query structure fits")) } == 0 { return Err(last_win32(operation)); }
    Ok(output)
}

fn query_entry_metadata(handle: HANDLE, filesystem: &LocalFilesystemSnapshot, operation: SafeFsOperation) -> Result<EntryMetadata> {
    let tag: FILE_ATTRIBUTE_TAG_INFO = win32_query(handle, FileAttributeTagInfo, operation)?;
    let id: FILE_ID_INFO = win32_query(handle, FileIdInfo, operation)?;
    let standard: FILE_STANDARD_INFO = win32_query(handle, FileStandardInfo, operation)?;
    if let LocalFilesystemSnapshot::Windows { serial, .. } = filesystem {
        if *serial != id.VolumeSerialNumber {
            return Err(SafeFsError::UnsupportedSecureFilesystem {
                operation,
                reason: SecureFilesystemReason::VolumeChanged,
            });
        }
    }
    let mut remote = FILE_REMOTE_PROTOCOL_INFO::default();
    // SAFETY: fixed remote-protocol output is writable and handle retained.
    if unsafe { GetFileInformationByHandleEx(handle, FileRemoteProtocolInfo,
        (&mut remote as *mut FILE_REMOTE_PROTOCOL_INFO).cast(), size_of::<FILE_REMOTE_PROTOCOL_INFO>() as u32) } != 0 {
        return Err(SafeFsError::UnsupportedSecureFilesystem { operation, reason: SecureFilesystemReason::RemoteFilesystem });
    }
    let remote_error = unsafe { GetLastError() };
    if remote_error != windows_sys::Win32::Foundation::ERROR_INVALID_PARAMETER &&
       remote_error != windows_sys::Win32::Foundation::ERROR_NOT_SUPPORTED {
        return Err(SafeFsError::Os { operation, raw: RawOsError::Win32(remote_error) });
    }
    let kind = if tag.FileAttributes & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
        EntryKind::SymlinkOrReparse
    } else if standard.Directory != 0 {
        EntryKind::Directory
    } else {
        EntryKind::RegularFile
    };
    Ok(EntryMetadata {
        identity: StableIdentity::Windows { volume_serial: id.VolumeSerialNumber, file_id: id.FileId.Identifier },
        kind,
        len: u64::try_from(standard.EndOfFile).map_err(|_| SafeFsError::InvalidNativeBuffer {
            operation, reason: NativeBufferReason::LengthOverflow })?,
        link_count: u64::from(standard.NumberOfLinks),
        filesystem: Some(filesystem.clone()),
    })
}
```

`windows.rs` 后半部的函数定义顺序固定为：`query_case_mode`、`query_entry_metadata`、`query_reparse`、`parse_reparse`、`enumerate`/`parse_directory_batch`、`OwnerOnlySecurity`、`probe_volume`、`capture_absolute_directory`、common dispatch opens/creates、consuming stage transitions、`nt_error`。sections 3–11 给出的每个 code body 都属于同一文件；implementation commit 不能漏任一函数，也不能以 panic/unsupported/空集合作为 Windows body。

## 3. UNICODE_STRING / OBJECT_ATTRIBUTES / IO_STATUS_BLOCK 同步规则

### 3.1 `UNICODE_STRING`

single component 先保留为 `Vec<u16>`，不追加 NUL。构造时：

```text
let byte_len = units.len()
    .checked_mul(size_of::<u16>())
    .and_then(|n| u16::try_from(n).ok())
    .ok_or(SafeFsError::InvalidComponent(ComponentViolation::TooLong))?;
let mut name = UNICODE_STRING {
    Length: byte_len,
    MaximumLength: byte_len,
    Buffer: units.as_ptr().cast_mut(),
};
```

`units`、`name`、引用它的 `OBJECT_ATTRIBUTES` 必须在整个 NT call 返回前保持原地址；不得在建立 pointer 后 push/reallocate。component 已拒绝 NUL、`/`、`\`、冒号、trailing dot/space、`.`/`..` 和 DOS device stem；不经 `String`/lossy conversion。

### 3.2 `OBJECT_ATTRIBUTES`

每次调用都完全初始化：

```text
let attrs = OBJECT_ATTRIBUTES {
    Length: u32::try_from(size_of::<OBJECT_ATTRIBUTES>()).unwrap(),
    RootDirectory: parent.handle(),
    ObjectName: &name,
    Attributes: parent.case_mode().object_attributes(),
    SecurityDescriptor: security
        .map_or(std::ptr::null(), OwnerOnlySecurity::descriptor_ptr),
    SecurityQualityOfService: std::ptr::null(),
};
```

`CaseMode::Sensitive -> 0`；`CaseMode::Insensitive -> OBJ_CASE_INSENSITIVE`。每个打开/创建的 directory 在返回 capability 前用 `NtQueryInformationFile(FileCaseSensitiveInformation)` 查询并把结果存入自己的 node；query 不支持或 flag 含未知 bit 时 `UnsupportedSecureFilesystem(CaseSemanticsUnavailable)`，不得继承/猜测 host-wide case mode。

### 3.3 同步 I/O 与 `IO_STATUS_BLOCK`

所有 `NtCreateFile` handle 同时包含 `SYNCHRONIZE` desired access 和 `FILE_SYNCHRONOUS_IO_NONALERT` create option；event/APC/context 均为空。每次调用新建零初始化 `IO_STATUS_BLOCK`，不得跨 call 复用。

`complete_nt_io(operation, returned_status, &iosb)` 的规则：

1. `returned_status == STATUS_PENDING` 在同步 handle 上属于 invariant failure；记录 raw status 并返回 `Os`，不得读取未完成 output，不另建 event，也不偷偷异步等待。
2. `returned_status < 0` 先走本稿第 7 节的 raw-NTSTATUS 分类；失败时不信任 `iosb.Information` 或 output bytes。
3. 成功/警告状态只在该 operation 明确允许时接受；随后读取 `iosb.Anonymous.Status`，若它是失败状态，以它为最终 raw status。
4. `NtReadFile`/`NtWriteFile`/`NtQueryDirectoryFile`/`NtFsControlFile` 的有效 byte count 只取 `iosb.Information`，先 checked-convert 到 `usize` 并验证不超过 caller buffer。
5. `NtReadFile` 的 `STATUS_END_OF_FILE` 转 `Ok(0)`；每次最大 length 为 `u32::MAX`，更大 slice 分块。
6. `NtWriteFile` 若成功但 `Information == 0` 且 input 非空，返回 `WriteZero`；`write_all` 循环直到完成。
7. `rewind` 调 `NtSetInformationFile(FilePositionInformation)`，offset 精确为 0；`flush` 调 `NtFlushBuffersFile` 并检查 returned status 与 IOSB final status。

## 4. Windows 每 operation 的 NT contract

共同 share mask：`FILE_SHARE_READ | FILE_SHARE_WRITE`，刻意省略 `FILE_SHARE_DELETE`。共同 file attributes 为 0（create file 时为 `FILE_ATTRIBUTE_NORMAL`）。共同 options 包含 `FILE_OPEN_REPARSE_POINT | FILE_OPEN_FOR_BACKUP_INTENT | FILE_SYNCHRONOUS_IO_NONALERT`。

| operation | DesiredAccess | ShareAccess | CreateDisposition | CreateOptions | OBJ case | post-open query / 额外约束 |
|---|---|---|---|---|---|---|
| absolute volume/root capture（仅首次 `CreateFileW`） | `FILE_LIST_DIRECTORY | FILE_TRAVERSE | FILE_READ_ATTRIBUTES | SYNCHRONIZE`，mutable parent 再加 `FILE_ADD_FILE | FILE_ADD_SUBDIRECTORY | FILE_DELETE_CHILD` | R\|W，无 DELETE | Win32 `OPEN_EXISTING` | `FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT` | Win32 首次 open 无 OBJ；随后每层用 stored case | Tag、Id、Standard、case、volume/remote；任何 reparse 拒绝 |
| `query_child_nofollow` transient | `FILE_READ_ATTRIBUTES | SYNCHRONIZE` | R\|W，无 DELETE | `FILE_OPEN` | common（不加 DIRECTORY/NON_DIRECTORY） | parent stored mode | Tag、Id、Standard；name/path not found 唯一转 Absent；handle call 后即 drop |
| `open_dir(Read)` | `FILE_LIST_DIRECTORY | FILE_TRAVERSE | FILE_READ_ATTRIBUTES | SYNCHRONIZE` | R\|W，无 DELETE | `FILE_OPEN` | common + `FILE_DIRECTORY_FILE` | parent mode | Tag 必须非 reparse；Standard.Directory=true；Id；child case；volume/remote 与 parent 相同 |
| `open_dir(MutateChildren)` | 上行 + `FILE_ADD_FILE | FILE_ADD_SUBDIRECTORY | FILE_DELETE_CHILD` | R\|W，无 DELETE | `FILE_OPEN` | common + DIRECTORY | parent mode | 同上 |
| `open_file(Read)` | `FILE_READ_DATA | FILE_READ_ATTRIBUTES | SYNCHRONIZE` | R\|W，无 DELETE | `FILE_OPEN` | common + `FILE_NON_DIRECTORY_FILE` | parent mode | Tag 非 reparse；Standard.Directory=false；Id；remote/volume 相同 |
| `open_file(ReadWrite)` | 上行 + `FILE_WRITE_DATA` | R\|W，无 DELETE | `FILE_OPEN` | common + NON_DIRECTORY | parent mode | 同上；FileCapability access=ReadWrite |
| `read_link_component` | `FILE_READ_ATTRIBUTES | SYNCHRONIZE` | R\|W，无 DELETE | `FILE_OPEN` | common；不加 type constraint | parent mode | Tag 必须是 reparse，随后 `NtFsControlFile(FSCTL_GET_REPARSE_POINT)` |
| `enumerate` | 使用 retained directory handle | N/A | N/A | N/A | N/A | `NtQueryDirectoryFile(FileDirectoryInformation)`；每个 name 再经 component validator + relative nofollow query，不信 dirent attributes |
| `create_file_new(OwnerOnly)` | `FILE_READ_DATA | FILE_WRITE_DATA | FILE_READ_ATTRIBUTES | DELETE | SYNCHRONIZE` | R\|W，无 DELETE | `FILE_CREATE` | common + NON_DIRECTORY | parent mode | DELETE 仅供 post-create DACL rollback，成功 capability 不暴露 delete；protected file DACL；Tag/Id/Standard；verify |
| `create_dir_new(MutateChildren, OwnerOnly)` | `FILE_LIST_DIRECTORY | FILE_TRAVERSE | FILE_READ_ATTRIBUTES | FILE_ADD_FILE | FILE_ADD_SUBDIRECTORY | FILE_DELETE_CHILD | DELETE | SYNCHRONIZE` | R\|W，无 DELETE | `FILE_CREATE` | common + DIRECTORY | parent mode | DELETE 仅供 post-create DACL rollback；protected inheritable dir DACL；Tag/Id/Standard/case/volume；verify |
| `create_dir_new(Stage, OwnerOnly)` | 上行 + `DELETE` | R\|W，无 DELETE | `FILE_CREATE` | common + DIRECTORY | parent mode | 同一 returned HANDLE retained 到 cleanup/publish；不得按名称重开 DELETE handle |
| `delete_file_handle` | source capability 在 open/create 时已含 `DELETE`；若普通 leaf 需要 cleanup，首次 open 就用 private Delete access variant | 原 retained mask | N/A | N/A | N/A | identity/type 已由 handle 固定；`FILE_DISPOSITION_INFORMATION { DeleteFile:true }` + class `FileDispositionInformation`，成功后 drop 触发 delete |
| `delete_empty_dir_handle` | Stage/cleanup dir 已含 `DELETE` | 原 retained mask | N/A | N/A | N/A | 同一 handle 上 FileDispositionInformation；非空 `STATUS_DIRECTORY_NOT_EMPTY` 保持 typed OS error |
| `rename_noreplace_same_parent` | source 是创建时已含 `DELETE` 的 retained Stage HANDLE；parent 含 child mutation rights | 原 retained mask | N/A | N/A | target name按 parent mode | `NtSetInformationFile(FileRenameInformation)`；`ReplaceIfExists=false`、`RootDirectory=parent`；成功后 source capability 被消费 |

关键 lifetime：普通 ancestor/parent handle 无 `FILE_SHARE_DELETE`，因此阻止其在授权期间被 rename/delete；stage 自己在 `FILE_CREATE` 时持 `DELETE`，final rename 不需要再开一个与自身 share policy 冲突的 handle。

表格必须编码为 production 常量，所有 open/create dispatch 只能索引该表，不能现场拼 rights：

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct OpenContract {
    desired: FILE_ACCESS_RIGHTS,
    disposition: NTCREATEFILE_CREATE_DISPOSITION,
    options: NTCREATEFILE_CREATE_OPTIONS,
    attributes: FILE_FLAGS_AND_ATTRIBUTES,
    delete_right: bool,
}

const QUERY_CONTRACT: OpenContract = OpenContract {
    desired: FILE_READ_ATTRIBUTES | SYNCHRONIZE,
    disposition: FILE_OPEN,
    options: COMMON_OPTIONS,
    attributes: 0,
    delete_right: false,
};
const DIR_READ_CONTRACT: OpenContract = OpenContract {
    desired: FILE_LIST_DIRECTORY | FILE_TRAVERSE | FILE_READ_ATTRIBUTES | SYNCHRONIZE,
    disposition: FILE_OPEN,
    options: COMMON_OPTIONS | FILE_DIRECTORY_FILE,
    attributes: 0,
    delete_right: false,
};
const DIR_MUTATE_CONTRACT: OpenContract = OpenContract {
    desired: FILE_LIST_DIRECTORY | FILE_TRAVERSE | FILE_READ_ATTRIBUTES |
        FILE_ADD_FILE | FILE_ADD_SUBDIRECTORY | FILE_DELETE_CHILD | SYNCHRONIZE,
    disposition: FILE_OPEN,
    options: COMMON_OPTIONS | FILE_DIRECTORY_FILE,
    attributes: 0,
    delete_right: false,
};
const FILE_READ_CONTRACT: OpenContract = OpenContract {
    desired: FILE_READ_DATA | FILE_READ_ATTRIBUTES | SYNCHRONIZE,
    disposition: FILE_OPEN,
    options: COMMON_OPTIONS | FILE_NON_DIRECTORY_FILE,
    attributes: 0,
    delete_right: false,
};
const FILE_WRITE_CONTRACT: OpenContract = OpenContract {
    desired: FILE_READ_DATA | FILE_WRITE_DATA | FILE_READ_ATTRIBUTES | SYNCHRONIZE,
    disposition: FILE_OPEN,
    options: COMMON_OPTIONS | FILE_NON_DIRECTORY_FILE,
    attributes: 0,
    delete_right: false,
};
const CREATE_FILE_CONTRACT: OpenContract = OpenContract {
    desired: FILE_READ_DATA | FILE_WRITE_DATA | FILE_READ_ATTRIBUTES | DELETE | SYNCHRONIZE,
    disposition: FILE_CREATE,
    options: COMMON_OPTIONS | FILE_NON_DIRECTORY_FILE,
    attributes: FILE_ATTRIBUTE_NORMAL,
    delete_right: true,
};
const CREATE_DIR_CONTRACT: OpenContract = OpenContract {
    desired: FILE_LIST_DIRECTORY | FILE_TRAVERSE | FILE_READ_ATTRIBUTES |
        FILE_ADD_FILE | FILE_ADD_SUBDIRECTORY | FILE_DELETE_CHILD | DELETE | SYNCHRONIZE,
    disposition: FILE_CREATE,
    options: COMMON_OPTIONS | FILE_DIRECTORY_FILE,
    attributes: 0,
    delete_right: true,
};
const CREATE_STAGE_CONTRACT: OpenContract = OpenContract {
    desired: CREATE_DIR_CONTRACT.desired | DELETE,
    disposition: FILE_CREATE,
    options: COMMON_OPTIONS | FILE_DIRECTORY_FILE,
    attributes: 0,
    delete_right: true,
};
const CLEANUP_FILE_CONTRACT: OpenContract = OpenContract {
    desired: FILE_READ_ATTRIBUTES | DELETE | SYNCHRONIZE,
    disposition: FILE_OPEN,
    options: COMMON_OPTIONS | FILE_NON_DIRECTORY_FILE,
    attributes: 0,
    delete_right: true,
};
const CLEANUP_DIR_CONTRACT: OpenContract = OpenContract {
    desired: FILE_LIST_DIRECTORY | FILE_TRAVERSE | FILE_READ_ATTRIBUTES | DELETE | SYNCHRONIZE,
    disposition: FILE_OPEN,
    options: COMMON_OPTIONS | FILE_DIRECTORY_FILE,
    attributes: 0,
    delete_right: true,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum OpenOperation { Query, DirRead, DirMutate, FileRead, FileWrite, CreateFile, CreateDir, CreateStage, CleanupFile, CleanupDir }

fn contract_for_operation(operation: OpenOperation) -> OpenContract {
    match operation {
        OpenOperation::Query => QUERY_CONTRACT,
        OpenOperation::DirRead => DIR_READ_CONTRACT,
        OpenOperation::DirMutate => DIR_MUTATE_CONTRACT,
        OpenOperation::FileRead => FILE_READ_CONTRACT,
        OpenOperation::FileWrite => FILE_WRITE_CONTRACT,
        OpenOperation::CreateFile => CREATE_FILE_CONTRACT,
        OpenOperation::CreateDir => CREATE_DIR_CONTRACT,
        OpenOperation::CreateStage => CREATE_STAGE_CONTRACT,
        OpenOperation::CleanupFile => CLEANUP_FILE_CONTRACT,
        OpenOperation::CleanupDir => CLEANUP_DIR_CONTRACT,
    }
}
```

## 5. `NtQueryDirectoryFile` buffer parser

固定 information class 为 `FileDirectoryInformation`，buffer 为 64 KiB、至少 `align_of::<FILE_DIRECTORY_INFORMATION>()` 对齐的 storage。第一次 `restartscan=true`，后续 false；`returnsingleentry=false`，`filename=null`。

循环规则：

- `STATUS_NO_MORE_FILES`：正常结束。
- `STATUS_BUFFER_OVERFLOW`：只有 `iosb.Information > 0` 才解析已返回完整 records；0 bytes 是 `InvalidNativeBuffer(DirectoryBufferTooSmall)`。
- 其他失败：走 raw-status mapping。
- success：要求 `0 < used <= buffer.len()`；0 success 视为 malformed，避免无限循环。

每批 parser 使用：

```text
const NAME_OFFSET: usize = offset_of!(FILE_DIRECTORY_INFORMATION, FileName);
```

对 cursor 处每个 record 依次验证：

1. `remaining >= size_of::<FILE_DIRECTORY_INFORMATION>()`；读取 fixed fields 用 `read_unaligned`，不把任意 byte slice 转成 Rust reference。
2. `FileNameLength` 必须为偶数且非零；`NAME_OFFSET + FileNameLength <= remaining`，所有加法 checked。
3. UTF-16 slice 长度为 `FileNameLength / 2`，用 `OsStringExt::from_wide` 保留 unpaired surrogate，再交 `ComponentName`。`.`/`..` 只允许作为被显式跳过的 directory control record。
4. `NextEntryOffset == 0` 表示本批最后一项；该 record 的 name end 仍须在 `used` 内。
5. 非零 `NextEntryOffset` 必须 `>= NAME_OFFSET + FileNameLength`、是 8 的倍数、`cursor + offset <= used` 且严格前进。
6. 整批最多 `used / NAME_OFFSET + 1` 次迭代；超限即 malformed，防止 offset loop。
7. 收集的每个 name 必须再调用 retained parent 的 `query_child_nofollow`；查询失败、消失或 reparse 均使 enumeration fail closed，不输出仅凭 directory buffer 得到的 authority。

native parser tests 直接喂：单项、多项、unpaired UTF-16、odd byte length、name overrun、zero-progress、misaligned next、next beyond used、truncated header、warning-with-zero-bytes、valid last record with trailing capacity。

实现 body 固定为下列纯 parser；native syscall loop 只能把 `iosb.Information` 验证后的 slice 交给它：

```rust
fn parse_directory_batch(bytes: &[u8]) -> Result<Vec<ComponentName>> {
    const NAME_OFFSET: usize = offset_of!(FILE_DIRECTORY_INFORMATION, FileName);
    if bytes.is_empty() {
        return Err(SafeFsError::InvalidNativeBuffer {
            operation: SafeFsOperation::ParseDirectoryBuffer,
            reason: NativeBufferReason::DirectoryBufferMalformed,
        });
    }
    let mut names = Vec::new();
    let mut cursor = 0usize;
    let mut iterations = 0usize;
    let maximum = bytes.len() / NAME_OFFSET.max(1) + 1;
    loop {
        iterations += 1;
        if iterations > maximum || cursor > bytes.len() || bytes.len() - cursor < size_of::<FILE_DIRECTORY_INFORMATION>() {
            return Err(SafeFsError::InvalidNativeBuffer {
                operation: SafeFsOperation::ParseDirectoryBuffer,
                reason: NativeBufferReason::DirectoryBufferMalformed,
            });
        }
        let base = bytes[cursor..].as_ptr();
        // SAFETY: fixed header availability checked; unaligned kernel buffer is read by value.
        let record = unsafe { std::ptr::read_unaligned(base.cast::<FILE_DIRECTORY_INFORMATION>()) };
        let name_bytes = usize::try_from(record.FileNameLength).map_err(|_| SafeFsError::InvalidNativeBuffer {
            operation: SafeFsOperation::ParseDirectoryBuffer,
            reason: NativeBufferReason::LengthOverflow,
        })?;
        let name_end = NAME_OFFSET.checked_add(name_bytes).and_then(|value| cursor.checked_add(value))
            .ok_or(SafeFsError::InvalidNativeBuffer {
                operation: SafeFsOperation::ParseDirectoryBuffer,
                reason: NativeBufferReason::DirectoryBufferMalformed,
            })?;
        if name_bytes == 0 || name_bytes % 2 != 0 || name_end > bytes.len() {
            return Err(SafeFsError::InvalidNativeBuffer {
                operation: SafeFsOperation::ParseDirectoryBuffer,
                reason: NativeBufferReason::DirectoryBufferMalformed,
            });
        }
        let units_len = name_bytes / 2;
        let mut units = Vec::with_capacity(units_len);
        for index in 0..units_len {
            let offset = cursor + NAME_OFFSET + index * 2;
            units.push(u16::from_le_bytes([bytes[offset], bytes[offset + 1]]));
        }
        let os = OsString::from_wide(&units);
        if os != OsStr::new(".") && os != OsStr::new("..") { names.push(ComponentName::new(os)?); }

        let next = usize::try_from(record.NextEntryOffset).map_err(|_| SafeFsError::InvalidNativeBuffer {
            operation: SafeFsOperation::ParseDirectoryBuffer,
            reason: NativeBufferReason::DirectoryBufferMalformed,
        })?;
        if next == 0 { break; }
        if next % 8 != 0 || next < NAME_OFFSET + name_bytes || cursor.checked_add(next).is_none()
            || cursor + next > bytes.len() || cursor + next <= cursor
        {
            return Err(SafeFsError::InvalidNativeBuffer {
                operation: SafeFsOperation::ParseDirectoryBuffer,
                reason: NativeBufferReason::DirectoryBufferMalformed,
            });
        }
        cursor += next;
    }
    Ok(names)
}

pub(super) fn enumerate(directory: &DirectoryAuthority) -> Result<Vec<ComponentName>> {
    #[repr(align(8))]
    struct Aligned([u8; DIRECTORY_BUFFER_BYTES]);
    let mut output = Vec::new();
    let mut first = true;
    loop {
        let mut buffer = Aligned([0; DIRECTORY_BUFFER_BYTES]);
        let mut iosb = IO_STATUS_BLOCK::default();
        // SAFETY: retained directory HANDLE; aligned writable buffer; synchronous call; null filter.
        let status = unsafe { NtQueryDirectoryFile(directory.native.node.handle.raw(), null_mut(), None,
            null(), &mut iosb, buffer.0.as_mut_ptr().cast(), DIRECTORY_BUFFER_BYTES as u32,
            FileDirectoryInformation, false, null(), first) };
        first = false;
        if status == STATUS_NO_MORE_FILES { break; }
        if status < STATUS_SUCCESS && status != STATUS_BUFFER_OVERFLOW {
            return Err(nt_error(SafeFsOperation::EnumerateDirectory, status));
        }
        let used = checked_information(SafeFsOperation::EnumerateDirectory, &iosb, DIRECTORY_BUFFER_BYTES)?;
        if used == 0 {
            return Err(SafeFsError::InvalidNativeBuffer {
                operation: SafeFsOperation::EnumerateDirectory,
                reason: if status == STATUS_BUFFER_OVERFLOW {
                    NativeBufferReason::DirectoryBufferTooSmall
                } else {
                    NativeBufferReason::DirectoryBufferMalformed
                },
            });
        }
        for name in parse_directory_batch(&buffer.0[..used])? {
            match query_child_nofollow(directory, &name)? {
                ChildState::Present(metadata) if metadata.kind != EntryKind::SymlinkOrReparse => output.push(name),
                ChildState::Present(_) => return Err(SafeFsError::SymlinkOrReparsePoint {
                    operation: SafeFsOperation::EnumerateDirectory,
                }),
                ChildState::Absent => return Err(SafeFsError::NotFound {
                    operation: SafeFsOperation::EnumerateDirectory,
                }),
            }
        }
    }
    output.sort_by(|left, right| left.as_os_str().encode_wide().cmp(right.as_os_str().encode_wide()));
    Ok(output)
}
```

## 6. `FSCTL_GET_REPARSE_POINT` bounds

调用 `NtFsControlFile`（不是把 FSCTL 错送给 `NtDeviceIoControlFile`），输出 storage 长度精确为 `MAXIMUM_REPARSE_DATA_BUFFER_SIZE`（16,384），且按 `REPARSE_DATA_BUFFER` 对齐。输入 buffer null/0。

返回后：

1. `8 <= iosb.Information <= 16_384`。
2. 从前 8 bytes 读取 `ReparseTag:u32`、`ReparseDataLength:u16`、reserved；`total = 8 + data_len` checked，要求 `total <= returned` 且 `total <= 16_384`。
3. 只把 `raw[..total]` 返回 C1C；padding 不外泄。
4. 对 `IO_REPARSE_TAG_MOUNT_POINT`，payload 至少 8 bytes；`SubstituteNameOffset/Length`、`PrintNameOffset/Length` 均为偶数，且每个 `offset + length <= payload_len - 8`。
5. 对 `IO_REPARSE_TAG_SYMLINK`，payload 至少 12 bytes；同样验证两个 name range 相对 PathBuffer；flags 只允许 0 或 `SYMLINK_FLAG_RELATIVE`。
6. unknown tag 仍可作为 bounded raw data 返回给 C1C 的 allowlist 拒绝；malformed known tag 在 C1B 直接 `InvalidNativeBuffer`。

测试覆盖 header 7 bytes、length over returned、length over max、odd path range、offset overflow、mount/symlink minimum、unknown bounded tag、kernel returned byte count 大于 caller buffer。

`query_child_nofollow` 不调用 `query_reparse` 来决定是否存在。它用 `FILE_OPEN_REPARSE_POINT` 得到 transient HANDLE，查询 tag/id/standard 后：tag 含 reparse 时返回 `Ok(ChildState::Present(metadata_with_kind_SymlinkOrReparse))`，不得把“发现 reparse”转换成 error；`open_dir_nofollow`、`open_file_nofollow` 和 `enumerate` 才拒绝该 `Present`。`read_link_component` 要求先得到这个 Present metadata，再对同一个 retained transient HANDLE 调：

```rust
fn query_reparse(handle: HANDLE) -> Result<RawLinkTarget> {
    #[repr(align(8))]
    struct Aligned([u8; MAXIMUM_REPARSE_DATA_BUFFER_SIZE as usize]);
    let mut storage = Aligned([0; MAXIMUM_REPARSE_DATA_BUFFER_SIZE as usize]);
    let mut iosb = IO_STATUS_BLOCK::default();
    // SAFETY: retained reparse HANDLE; aligned output buffer is writable for declared length; synchronous call.
    let status = unsafe { NtFsControlFile(handle, null_mut(), None, null(), &mut iosb,
        FSCTL_GET_REPARSE_POINT, null(), 0, storage.0.as_mut_ptr().cast(),
        MAXIMUM_REPARSE_DATA_BUFFER_SIZE) };
    complete_nt(SafeFsOperation::QueryReparsePoint, status, &iosb)?;
    let used = checked_information(SafeFsOperation::QueryReparsePoint, &iosb, storage.0.len())?;
    let (tag, bounded) = parse_reparse(&storage.0[..used])?;
    Ok(RawLinkTarget::Windows { tag, bytes: bounded })
}

fn parse_reparse(bytes: &[u8]) -> Result<(u32, Vec<u8>)> {
    let malformed = || SafeFsError::InvalidNativeBuffer {
        operation: SafeFsOperation::ParseReparseBuffer,
        reason: NativeBufferReason::ReparseBufferMalformed,
    };
    if bytes.len() < REPARSE_HEADER_BYTES || bytes.len() > MAXIMUM_REPARSE_DATA_BUFFER_SIZE as usize {
        return Err(malformed());
    }
    let tag = u32::from_le_bytes(bytes[0..4].try_into().expect("four-byte tag"));
    let payload_len = usize::from(u16::from_le_bytes([bytes[4], bytes[5]]));
    let total = REPARSE_HEADER_BYTES.checked_add(payload_len).ok_or_else(malformed)?;
    if total > bytes.len() || total > MAXIMUM_REPARSE_DATA_BUFFER_SIZE as usize { return Err(malformed()); }
    let payload = &bytes[REPARSE_HEADER_BYTES..total];
    const MOUNT: u32 = 0xA000_0003;
    const SYMLINK: u32 = 0xA000_000C;
    let validate_range = |base: usize, offset: u16, length: u16| -> Result<()> {
        if offset % 2 != 0 || length % 2 != 0 { return Err(malformed()); }
        let end = base.checked_add(usize::from(offset)).and_then(|start| start.checked_add(usize::from(length)))
            .ok_or_else(malformed)?;
        if end > payload.len() { return Err(malformed()); }
        Ok(())
    };
    match tag {
        MOUNT => {
            if payload.len() < 8 { return Err(malformed()); }
            validate_range(8, u16::from_le_bytes([payload[0], payload[1]]), u16::from_le_bytes([payload[2], payload[3]]))?;
            validate_range(8, u16::from_le_bytes([payload[4], payload[5]]), u16::from_le_bytes([payload[6], payload[7]]))?;
        }
        SYMLINK => {
            if payload.len() < 12 { return Err(malformed()); }
            validate_range(12, u16::from_le_bytes([payload[0], payload[1]]), u16::from_le_bytes([payload[2], payload[3]]))?;
            validate_range(12, u16::from_le_bytes([payload[4], payload[5]]), u16::from_le_bytes([payload[6], payload[7]]))?;
            let flags = u32::from_le_bytes(payload[8..12].try_into().expect("four-byte flags"));
            if flags != 0 && flags != 1 { return Err(malformed()); }
        }
        _ => {}
    }
    Ok((tag, bytes[..total].to_vec()))
}
```

## 7. NTSTATUS-first 私有 mapping

`nt_error(operation, status)` 先匹配下表，再调用 `RtlNtStatusToDosError(status)` 填诊断字段。不得先转 DOS code 再决定控制流。

| raw NTSTATUS | query | open/read | create | delete | rename no-replace | enumerate/control |
|---|---|---|---|---|---|---|
| `STATUS_OBJECT_NAME_NOT_FOUND`, `STATUS_OBJECT_PATH_NOT_FOUND` | `Absent` | `NotFound` | parent/path error | `NotFound` | source missing error | error |
| `STATUS_OBJECT_NAME_COLLISION` | Present/race retry | error | `AlreadyExists` | error | `AlreadyExists` | error |
| `STATUS_REPARSE_POINT_ENCOUNTERED` | `SymlinkOrReparsePoint` | same | same | same | same | same |
| `STATUS_NOT_A_DIRECTORY`, `STATUS_FILE_IS_A_DIRECTORY`, `STATUS_OBJECT_TYPE_MISMATCH` | typed EntryKind/type error | typed type error | typed type error | typed type error | typed type error | malformed/type error |
| `STATUS_ACCESS_DENIED` | `Os` | `Os` | `Os` | `Os` | 仅在 preflight target absent、source HANDLE 已有 DELETE、post-query target Present 三项均成立时 `AlreadyExists`；否则 `Os` | `Os` |
| `STATUS_SHARING_VIOLATION`, `STATUS_DELETE_PENDING`, `STATUS_CANNOT_DELETE` | `Os` | `Os` | `Os` | `Os` | 不自动 collision；同上三条件满足才 collision | `Os` |
| `STATUS_DIRECTORY_NOT_EMPTY` | `Os` | `Os` | `Os` | `Os` | post-query 证实 target present 时 collision，否则 `Os` | `Os` |
| `STATUS_NOT_SUPPORTED` | filesystem proof时 unsupported FS | `Os` | `Os` | `Os` | `UnsupportedAtomicPublish(PrimitiveUnavailable)` | FSCTL/case query 按 operation typed unsupported |
| `STATUS_INVALID_PARAMETER`, `STATUS_INFO_LENGTH_MISMATCH`, `STATUS_BUFFER_TOO_SMALL` | native contract defect | native contract defect | native contract defect | native contract defect | native layout defect，不称 unsupported | parser/buffer defect |
| `STATUS_NO_MORE_FILES` | N/A | N/A | N/A | N/A | N/A | enumerate 正常终止 |
| `STATUS_BUFFER_OVERFLOW` | N/A | N/A | N/A | N/A | N/A | 仅按第 5 节接受 partial bytes |
| `STATUS_END_OF_FILE` | N/A | read `Ok(0)` | N/A | N/A | N/A | N/A |
| `STATUS_PENDING` | invariant error | invariant error | invariant error | invariant error | invariant error | invariant error |

rename 的 ambiguous-status collision upgrade 前必须：rename 前 relative query 得到 Absent；source capability 类型固定且创建时已授予 DELETE；失败后 relative query 成功得到 Present。post-query 自身若 AccessDenied/Sharing 等，保留原 rename raw status，绝不报 collision。注入测试分别覆盖“target absent + AccessDenied”和“target present + preflight/access proof 不完整”，两者都必须保留 `Os`。

唯一 general mapper body 如下；query 的 Absent、read EOF、enumeration 的 terminal/warning、rename 的三证据 collision upgrade 必须在调用 general mapper 前处理：

```rust
fn raw_nt(operation: SafeFsOperation, status: NTSTATUS) -> SafeFsError {
    // SAFETY: pure ntdll status conversion; raw NTSTATUS remains the primary diagnostic.
    let dos_error = unsafe { RtlNtStatusToDosError(status) };
    SafeFsError::Os { operation, raw: RawOsError::NtStatus { status, dos_error } }
}

fn nt_error(operation: SafeFsOperation, status: NTSTATUS) -> SafeFsError {
    match status {
        STATUS_OBJECT_NAME_NOT_FOUND | STATUS_OBJECT_PATH_NOT_FOUND =>
            SafeFsError::NotFound { operation },
        STATUS_OBJECT_NAME_COLLISION if matches!(operation,
            SafeFsOperation::CreateDirectory | SafeFsOperation::CreateFile |
            SafeFsOperation::RenameNoReplaceSameParent) =>
            SafeFsError::AlreadyExists { operation },
        STATUS_REPARSE_POINT_ENCOUNTERED =>
            SafeFsError::SymlinkOrReparsePoint { operation },
        STATUS_NOT_SUPPORTED if operation == SafeFsOperation::QueryCaseMode =>
            SafeFsError::UnsupportedSecureFilesystem {
                operation,
                reason: SecureFilesystemReason::CaseSemanticsUnavailable,
            },
        STATUS_NOT_SUPPORTED if operation == SafeFsOperation::RenameNoReplaceSameParent =>
            SafeFsError::UnsupportedAtomicPublish {
                operation,
                reason: AtomicPublishReason::PrimitiveUnavailable,
            },
        STATUS_INVALID_PARAMETER | STATUS_INFO_LENGTH_MISMATCH =>
            SafeFsError::InvalidNativeBuffer {
                operation,
                reason: if operation == SafeFsOperation::RenameNoReplaceSameParent {
                    NativeBufferReason::RenameLayoutMalformed
                } else {
                    NativeBufferReason::LengthOverflow
                },
            },
        STATUS_NOT_A_DIRECTORY | STATUS_FILE_IS_A_DIRECTORY | STATUS_OBJECT_TYPE_MISMATCH =>
            SafeFsError::UnsupportedEntryType { operation, kind: EntryKind::Other },
        STATUS_ACCESS_DENIED | STATUS_SHARING_VIOLATION | STATUS_DELETE_PENDING |
        STATUS_CANNOT_DELETE | STATUS_DIRECTORY_NOT_EMPTY | STATUS_BUFFER_OVERFLOW |
        STATUS_NO_MORE_FILES | STATUS_END_OF_FILE | STATUS_PENDING | STATUS_NOT_SUPPORTED =>
            raw_nt(operation, status),
        _ => raw_nt(operation, status),
    }
}

fn map_rename_failure(
    status: NTSTATUS,
    preflight_absent: bool,
    source_had_delete: bool,
    postflight: Result<ChildState>,
) -> SafeFsError {
    if status == STATUS_OBJECT_NAME_COLLISION {
        return SafeFsError::AlreadyExists { operation: SafeFsOperation::RenameNoReplaceSameParent };
    }
    let ambiguous = matches!(status,
        STATUS_ACCESS_DENIED | STATUS_SHARING_VIOLATION | STATUS_DELETE_PENDING |
        STATUS_CANNOT_DELETE | STATUS_DIRECTORY_NOT_EMPTY);
    if ambiguous && preflight_absent && source_had_delete &&
        matches!(postflight, Ok(ChildState::Present(_)))
    {
        SafeFsError::AlreadyExists { operation: SafeFsOperation::RenameNoReplaceSameParent }
    } else {
        raw_nt(SafeFsOperation::RenameNoReplaceSameParent, status)
    }
}
```

## 8. Owner SID + protected DACL exact lifetime

`OwnerOnlySecurity::new(kind)` 执行以下固定序列：

1. `OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY)` 得到 RAII token。
2. 第一次 `GetTokenInformation(TokenOwner, null, 0, &needed)` 必须失败且 `GetLastError()==ERROR_INSUFFICIENT_BUFFER`；`needed >= size_of::<TOKEN_OWNER>()`。
3. 分配 aligned token buffer，第二次 query 成功；检查 `Owner` non-null、`IsValidSid`；`GetLengthSid` 非零并 checked-convert。
4. 把 SID copy 到独立 aligned `sid_storage`；token buffer/token HANDLE 随后可 drop。ACL construction 期间 SID storage 保持存活。
5. `acl_size = size_of::<ACL>() + size_of::<ACCESS_ALLOWED_ACE>() - size_of::<u32>() + sid_len`，所有运算 checked，要求可转 u32 且不超过 `u16::MAX`。
6. `InitializeAcl(..., ACL_REVISION)`；`AddAccessAllowedAceEx` 的 mask 为 `FILE_ALL_ACCESS`。file ACE flags=0；directory ACE flags=`OBJECT_INHERIT_ACE | CONTAINER_INHERIT_ACE`。
7. `InitializeSecurityDescriptor(..., SECURITY_DESCRIPTOR_REVISION)`；`SetSecurityDescriptorDacl(TRUE, acl, FALSE)`；`SetSecurityDescriptorControl(SE_DACL_PROTECTED, SE_DACL_PROTECTED)`。
8. `OwnerOnlySecurity` 最终拥有 `acl_storage` 与稳定地址的 `Box<SECURITY_DESCRIPTOR>`；`OBJECT_ATTRIBUTES.SecurityDescriptor` 指向 descriptor，而 descriptor 的 DACL pointer 指向 acl storage。两者必须活到 `NtCreateFile` 返回并完成 post-open metadata；之后 kernel 已捕获 descriptor，可 drop。

post-create 用 `GetKernelObjectSecurity(OWNER_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION)` two-call 读取 descriptor，并验证：control 含 `SE_DACL_PROTECTED`；owner `EqualSid` 当前 owner；DACL present/non-defaulted；ACL 恰好一个 ACCESS_ALLOWED ACE；mask=`FILE_ALL_ACCESS`；SID 相等；ACE flags 与 file/dir kind 精确相符。任一不符立即对刚创建 retained HANDLE 设置 FileDispositionInformation 并 drop，返回 typed security verification error；不得把不合格对象交给调用方。

持有期实现固定为：

```rust
struct OwnerOnlySecurity {
    sid: Vec<usize>,
    acl: Vec<usize>,
    descriptor: Box<SECURITY_DESCRIPTOR>,
    ace_flags: u8,
}

impl OwnerOnlySecurity {
    fn new(directory: bool) -> Result<Self> {
        let op = SafeFsOperation::VerifySecurityDescriptor;
        let mut token_raw = null_mut();
        // SAFETY: output pointer valid; current process pseudo-handle valid.
        if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token_raw) } == 0 {
            return Err(last_win32(op));
        }
        let token = OwnedHandle::new(token_raw, op)?;
        let mut needed = 0u32;
        // SAFETY: documented sizing call with null output.
        let first = unsafe { GetTokenInformation(token.raw(), TokenOwner, null_mut(), 0, &mut needed) };
        if first != 0 || unsafe { GetLastError() } != windows_sys::Win32::Foundation::ERROR_INSUFFICIENT_BUFFER ||
            needed < size_of::<TOKEN_OWNER>() as u32 { return Err(last_win32(op)); }
        let mut token_words = vec![0usize; (needed as usize).div_ceil(size_of::<usize>())];
        // SAFETY: aligned storage is writable for needed bytes.
        if unsafe { GetTokenInformation(token.raw(), TokenOwner, token_words.as_mut_ptr().cast(), needed, &mut needed) } == 0 {
            return Err(last_win32(op));
        }
        // SAFETY: successful TokenOwner query initialized TOKEN_OWNER.
        let owner = unsafe { (*(token_words.as_ptr().cast::<TOKEN_OWNER>())).Owner };
        if owner.is_null() || unsafe { IsValidSid(owner) } == 0 { return Err(last_win32(op)); }
        // SAFETY: validated SID pointer.
        let sid_len = usize::try_from(unsafe { GetLengthSid(owner) }).map_err(|_| SafeFsError::InvalidNativeBuffer {
            operation: op, reason: NativeBufferReason::SecurityDescriptorMalformed })?;
        let mut sid = vec![0usize; sid_len.div_ceil(size_of::<usize>())];
        // SAFETY: destination capacity >= sid_len and source is validated SID.
        if unsafe { CopySid(sid_len as u32, sid.as_mut_ptr().cast(), owner) } == 0 { return Err(last_win32(op)); }
        drop(token_words); drop(token);

        let acl_bytes = size_of::<ACL>().checked_add(size_of::<ACCESS_ALLOWED_ACE>() - size_of::<u32>())
            .and_then(|value| value.checked_add(sid_len)).ok_or(SafeFsError::InvalidNativeBuffer {
                operation: op, reason: NativeBufferReason::LengthOverflow })?;
        let acl_len = u32::try_from(acl_bytes).map_err(|_| SafeFsError::InvalidNativeBuffer {
            operation: op, reason: NativeBufferReason::LengthOverflow })?;
        if acl_bytes > u16::MAX as usize { return Err(SafeFsError::InvalidNativeBuffer {
            operation: op, reason: NativeBufferReason::LengthOverflow }); }
        let mut acl = vec![0usize; acl_bytes.div_ceil(size_of::<usize>())];
        let ace_flags = if directory { OBJECT_INHERIT_ACE | CONTAINER_INHERIT_ACE } else { 0 };
        // SAFETY: aligned ACL storage and copied SID stay live inside Self.
        if unsafe { InitializeAcl(acl.as_mut_ptr().cast(), acl_len, ACL_REVISION) } == 0 ||
            unsafe { AddAccessAllowedAceEx(acl.as_mut_ptr().cast(), ACL_REVISION, ace_flags,
                FILE_ALL_ACCESS, sid.as_ptr().cast()) } == 0 { return Err(last_win32(op)); }
        let mut descriptor = Box::<SECURITY_DESCRIPTOR>::new(unsafe { std::mem::zeroed() });
        // SAFETY: boxed descriptor stable; ACL storage remains owned by returned Self.
        if unsafe { InitializeSecurityDescriptor((&mut *descriptor as *mut SECURITY_DESCRIPTOR).cast(), SECURITY_DESCRIPTOR_REVISION) } == 0 ||
            unsafe { SetSecurityDescriptorDacl((&mut *descriptor as *mut SECURITY_DESCRIPTOR).cast(), true,
                acl.as_mut_ptr().cast(), false) } == 0 ||
            unsafe { SetSecurityDescriptorControl((&mut *descriptor as *mut SECURITY_DESCRIPTOR).cast(),
                SE_DACL_PROTECTED, SE_DACL_PROTECTED) } == 0 { return Err(last_win32(op)); }
        Ok(Self { sid, acl, descriptor, ace_flags })
    }
    fn descriptor_ptr(&self) -> *const c_void { (&*self.descriptor as *const SECURITY_DESCRIPTOR).cast() }
}

fn verify_owner_only(handle: HANDLE, expected: &OwnerOnlySecurity) -> Result<()> {
    let op = SafeFsOperation::VerifySecurityDescriptor;
    let information = OWNER_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION;
    let mut needed = 0u32;
    // SAFETY: sizing call; handle retained.
    unsafe { GetKernelObjectSecurity(handle, information, null_mut(), 0, &mut needed) };
    if unsafe { GetLastError() } != windows_sys::Win32::Foundation::ERROR_INSUFFICIENT_BUFFER {
        return Err(last_win32(op));
    }
    let mut words = vec![0usize; (needed as usize).div_ceil(size_of::<usize>())];
    // SAFETY: aligned buffer writable for needed bytes.
    if unsafe { GetKernelObjectSecurity(handle, information, words.as_mut_ptr().cast(), needed, &mut needed) } == 0 {
        return Err(last_win32(op));
    }
    let descriptor = words.as_mut_ptr().cast::<SECURITY_DESCRIPTOR>();
    let mut control = 0u16; let mut revision = 0u32;
    let mut owner = null_mut(); let mut owner_defaulted = false;
    let mut dacl = null_mut(); let mut present = false; let mut defaulted = false;
    // SAFETY: kernel returned a self-relative security descriptor in words.
    if unsafe { GetSecurityDescriptorControl(descriptor.cast(), &mut control, &mut revision) } == 0 ||
        unsafe { GetSecurityDescriptorOwner(descriptor.cast(), &mut owner, &mut owner_defaulted) } == 0 ||
        unsafe { GetSecurityDescriptorDacl(descriptor.cast(), &mut present, &mut dacl, &mut defaulted) } == 0 ||
        control & SE_DACL_PROTECTED == 0 || owner_defaulted || !present || defaulted || dacl.is_null() ||
        unsafe { EqualSid(owner, expected.sid.as_ptr().cast()) } == 0 {
        return Err(SafeFsError::InvalidNativeBuffer { operation: op, reason: NativeBufferReason::SecurityDescriptorMalformed });
    }
    let mut acl_info = ACL_SIZE_INFORMATION::default();
    // SAFETY: dacl validated non-null; output fixed and writable.
    if unsafe { GetAclInformation(dacl, (&mut acl_info as *mut ACL_SIZE_INFORMATION).cast(),
        size_of::<ACL_SIZE_INFORMATION>() as u32, AclSizeInformation) } == 0 || acl_info.AceCount != 1 {
        return Err(SafeFsError::InvalidNativeBuffer { operation: op, reason: NativeBufferReason::SecurityDescriptorMalformed });
    }
    let mut ace = null_mut();
    // SAFETY: validated one-entry ACL.
    if unsafe { GetAce(dacl, 0, &mut ace) } == 0 || ace.is_null() { return Err(last_win32(op)); }
    // SAFETY: one ACCESS_ALLOWED ACE was required by creation contract; header checked before fields.
    let allowed = unsafe { &*(ace.cast::<ACCESS_ALLOWED_ACE>()) };
    let sid_ptr = (&allowed.SidStart as *const u32).cast();
    if allowed.Header.AceType != ACCESS_ALLOWED_ACE_TYPE || allowed.Header.AceFlags != expected.ace_flags ||
        allowed.Mask != FILE_ALL_ACCESS || unsafe { EqualSid(sid_ptr, expected.sid.as_ptr().cast()) } == 0 {
        return Err(SafeFsError::InvalidNativeBuffer { operation: op, reason: NativeBufferReason::SecurityDescriptorMalformed });
    }
    Ok(())
}
```

## 9. Handle-relative delete

结构固定为 WDK `FILE_DISPOSITION_INFORMATION { DeleteFile: true }`，information class 固定 `FileDispositionInformation`（13）：

```rust
fn duplicate_directory(source: &DirectoryAuthority) -> Result<DirectoryAuthority> {
    let mut duplicated = null_mut();
    // SAFETY: source HANDLE retained; current-process source/target; output writable; same access only.
    if unsafe { DuplicateHandle(GetCurrentProcess(), source.native.node.handle.raw(), GetCurrentProcess(),
        &mut duplicated, 0, false, DUPLICATE_SAME_ACCESS) } == 0 { return Err(last_win32(SafeFsOperation::OpenDirectory)); }
    let handle = OwnedHandle::new(duplicated, SafeFsOperation::OpenDirectory)?;
    let old = &source.native.node;
    let node = Arc::new(DirectoryNode {
        handle,
        parent: old.parent.as_ref().map(Arc::clone),
        name: old.name.clone(),
        case_mode: old.case_mode,
        metadata: old.metadata.clone(),
        volume: old.volume.clone(),
    });
    Ok(DirectoryAuthority {
        anchor: Arc::clone(&source.anchor),
        native: NativeDirectory { node, access: source.native.access, delete_right: source.native.delete_right },
        access: source.access,
        opened: source.opened.clone(),
        case_mode: source.case_mode,
        snapshot: source.snapshot.clone(),
    })
}

pub(super) fn open_cleanup_child_nofollow(
    quarantined: &QuarantinedCapability,
    name: &ComponentName,
) -> Result<CleanupCapability> {
    let parent = &quarantined.directory;
    let state = query_child_nofollow(parent, name)?;
    let metadata = match state {
        ChildState::Absent => return Err(SafeFsError::NotFound { operation: SafeFsOperation::OpenCleanupEntry }),
        ChildState::Present(metadata) => metadata,
    };
    if metadata.kind == EntryKind::SymlinkOrReparse {
        return Err(SafeFsError::SymlinkOrReparsePoint { operation: SafeFsOperation::OpenCleanupEntry });
    }
    let contract = contract_for_operation(if metadata.kind == EntryKind::Directory {
        OpenOperation::CleanupDir
    } else {
        OpenOperation::CleanupFile
    });
    let handle = nt_create_relative(parent.native.node.handle.raw(), name, parent.native.node.case_mode,
        contract.desired, contract.disposition, contract.options, contract.attributes, null(),
        SafeFsOperation::OpenCleanupEntry)?;
    let filesystem = parent.opened.filesystem.as_ref().ok_or(SafeFsError::UnsupportedSecureFilesystem {
        operation: SafeFsOperation::ProbeFilesystem,
        reason: SecureFilesystemReason::FilesystemProbeUnavailable,
    })?;
    let opened = query_entry_metadata(handle.raw(), filesystem, SafeFsOperation::QueryMetadata)?;
    if opened.identity != metadata.identity {
        return Err(SafeFsError::IdentityChanged {
            operation: SafeFsOperation::QueryMetadata,
            expected: metadata.identity,
            actual: opened.identity,
        });
    }
    if opened.kind == EntryKind::Directory {
        let duplicated_parent = duplicate_directory(parent)?;
        let child_case = query_case_mode(handle.raw())?;
        let child_snapshot = append_snapshot(&parent.snapshot, name.clone(), &opened, child_case)?;
        let node = Arc::new(DirectoryNode {
            handle,
            parent: Some(Arc::clone(&parent.native.node)),
            name: Some(name.clone()),
            case_mode: child_case,
            metadata: opened.clone(),
            volume: parent.native.node.volume.clone(),
        });
        let directory = DirectoryAuthority {
            anchor: Arc::clone(&parent.anchor),
            native: NativeDirectory { node, access: DirectoryAccess::MutateChildren, delete_right: true },
            access: DirectoryAccess::MutateChildren,
            opened: opened.clone(),
            case_mode: child_case,
            snapshot: child_snapshot,
        };
        Ok(CleanupCapability::Directory(QuarantinedCapability {
            parent: duplicated_parent,
            directory,
            original_name: name.clone(),
            quarantine_name: name.clone(),
            opened,
        }))
    } else {
        Ok(CleanupCapability::Entry {
            parent: duplicate_directory(parent)?,
            native: NativeFile { handle, opened: opened.clone(), access: FileAccess::Read, delete_right: true },
            name: name.clone(),
            opened,
            access: CleanupAccess::Delete,
        })
    }
}

fn mark_delete_handle(handle: HANDLE, operation: SafeFsOperation) -> Result<()> {
    let info = FILE_DISPOSITION_INFORMATION { DeleteFile: true };
    let mut iosb = IO_STATUS_BLOCK::default();
    // SAFETY: caller owns a live DELETE handle; initialized fixed info and writable iosb stay live.
    let status = unsafe { NtSetInformationFile(handle, &mut iosb,
        (&info as *const FILE_DISPOSITION_INFORMATION).cast(),
        u32::try_from(size_of::<FILE_DISPOSITION_INFORMATION>()).expect("disposition info fits"),
        FileDispositionInformation) };
    complete_nt(operation, status, &iosb)
}

fn dispose_retained(mut native: NativeFile, expected_kind: EntryKind, operation: SafeFsOperation) -> Result<()> {
    if !native.delete_right {
        return Err(SafeFsError::Os {
            operation,
            raw: RawOsError::Win32(windows_sys::Win32::Foundation::ERROR_ACCESS_DENIED),
        });
    }
    if native.opened.kind != expected_kind {
        return Err(SafeFsError::UnsupportedEntryType { operation, kind: native.opened.kind });
    }
    mark_delete_handle(native.handle.raw(), operation)?;
    native.delete_right = false;
    drop(native);
    Ok(())
}

pub(super) fn delete_quarantined_entry(cleanup: CleanupCapability) -> Result<()> {
    match cleanup {
        CleanupCapability::Entry { native, opened, access: CleanupAccess::Delete, .. } => {
            if native.opened.identity != opened.identity {
                return Err(SafeFsError::IdentityChanged {
                    operation: SafeFsOperation::DeleteQuarantinedEntry,
                    expected: opened.identity,
                    actual: native.opened.identity,
                });
            }
            dispose_retained(native, opened.kind, SafeFsOperation::DeleteQuarantinedEntry)
        }
        CleanupCapability::Directory(_) => Err(SafeFsError::UnsupportedEntryType {
            operation: SafeFsOperation::DeleteQuarantinedEntry,
            kind: EntryKind::Directory,
        }),
    }
}

pub(super) fn delete_quarantined_empty_directory(quarantined: QuarantinedCapability) -> Result<()> {
    let QuarantinedCapability { directory, opened, .. } = quarantined;
    if directory.opened.identity != opened.identity || !directory.native.delete_right {
        return Err(SafeFsError::IdentityChanged {
            operation: SafeFsOperation::DeleteQuarantinedEmptyDirectory,
            expected: opened.identity,
            actual: directory.opened.identity,
        });
    }
    let native = NativeFile {
        handle: Arc::try_unwrap(directory.native.node).map_err(|node| SafeFsError::io(
            SafeFsOperation::DeleteQuarantinedEmptyDirectory,
            io::Error::new(io::ErrorKind::Other, format!("directory handle still shared: {}", Arc::strong_count(&node))),
        ))?.handle,
        opened: directory.opened,
        access: FileAccess::Read,
        delete_right: true,
    };
    dispose_retained(native, EntryKind::Directory, SafeFsOperation::DeleteQuarantinedEmptyDirectory)
}
```

不用 `FileDispositionInformationEx`/POSIX flags，不用 name-based delete。source handle 必须在首次 open/create 时已有 DELETE；设置 disposition 后 capability 被消费并 drop。Windows native tests 证明 name rebound 后删除的是 retained original object，replacement 的 identity/bytes/tree 不变。

## 10. `FILE_RENAME_INFORMATION` variable layout

唯一 builder `RenameInformationBuffer::new(parent_handle, target_units)`：

```text
let name_bytes = target_units.len()
    .checked_mul(size_of::<u16>())
    .and_then(|n| u32::try_from(n).ok())
    .ok_or(SafeFsError::InvalidComponent(ComponentViolation::TooLong))?;
let name_offset = offset_of!(FILE_RENAME_INFORMATION, FileName);
let total = name_offset
    .checked_add(name_bytes as usize)
    .ok_or(SafeFsError::InvalidNativeBuffer {
        operation: SafeFsOperation::RenameNoReplaceSameParent,
        reason: NativeBufferReason::LengthOverflow,
    })?;
let slots = total.div_ceil(size_of::<usize>());
let mut storage = vec![0usize; slots.max(1)];
let base = storage.as_mut_ptr().cast::<FILE_RENAME_INFORMATION>();
unsafe {
    (*base).Anonymous.ReplaceIfExists = false;
    (*base).RootDirectory = parent_handle;
    (*base).FileNameLength = name_bytes;
    std::ptr::copy_nonoverlapping(
        target_units.as_ptr().cast::<u8>(),
        base.cast::<u8>().add(name_offset),
        name_bytes as usize,
    );
}
```

完整 builder 与 consuming facade bodies：

```rust
struct RenameInformationBuffer { storage: Vec<usize>, used: u32 }

impl RenameInformationBuffer {
    fn new(parent: HANDLE, target: &ComponentName) -> Result<Self> {
        let units: Vec<u16> = target.as_os_str().encode_wide().collect();
        let name_bytes = units.len().checked_mul(size_of::<u16>())
            .and_then(|value| u32::try_from(value).ok())
            .ok_or(SafeFsError::InvalidComponent(ComponentViolation::TooLong))?;
        let name_offset = offset_of!(FILE_RENAME_INFORMATION, FileName);
        let total = name_offset.checked_add(name_bytes as usize).ok_or(SafeFsError::InvalidNativeBuffer {
            operation: SafeFsOperation::RenameNoReplaceSameParent,
            reason: NativeBufferReason::RenameLayoutMalformed,
        })?;
        let mut storage = vec![0usize; total.div_ceil(size_of::<usize>()).max(1)];
        let base = storage.as_mut_ptr().cast::<FILE_RENAME_INFORMATION>();
        debug_assert_eq!((base as usize) % align_of::<FILE_RENAME_INFORMATION>(), 0);
        // SAFETY: usize storage is sufficiently aligned and has at least total bytes; source units live for copy.
        unsafe {
            (*base).Anonymous.ReplaceIfExists = false;
            (*base).RootDirectory = parent;
            (*base).FileNameLength = name_bytes;
            std::ptr::copy_nonoverlapping(units.as_ptr().cast::<u8>(),
                base.cast::<u8>().add(name_offset), name_bytes as usize);
        }
        Ok(Self { storage, used: u32::try_from(total).map_err(|_| SafeFsError::InvalidNativeBuffer {
            operation: SafeFsOperation::RenameNoReplaceSameParent,
            reason: NativeBufferReason::RenameLayoutMalformed,
        })? })
    }
    fn as_ptr(&self) -> *const c_void { self.storage.as_ptr().cast() }
}

fn rename_retained_noreplace(
    native: &NativeDirectory,
    opened: &EntryMetadata,
    parent: &DirectoryAuthority,
    target: &ComponentName,
) -> Result<()> {
    require_mutation(parent, SafeFsOperation::RenameNoReplaceSameParent)?;
    let preflight = query_child_nofollow(parent, target)?;
    if matches!(preflight, ChildState::Present(_)) {
        return Err(SafeFsError::AlreadyExists {
            operation: SafeFsOperation::RenameNoReplaceSameParent,
        });
    }
    if !native.delete_right {
        return Err(raw_nt(SafeFsOperation::RenameNoReplaceSameParent, STATUS_ACCESS_DENIED));
    }
    let buffer = RenameInformationBuffer::new(parent.native.node.handle.raw(), target)?;
    let mut iosb = IO_STATUS_BLOCK::default();
    // SAFETY: retained DELETE source and parent handles plus aligned initialized variable buffer live through call.
    let status = unsafe { NtSetInformationFile(native.node.handle.raw(), &mut iosb, buffer.as_ptr(),
        buffer.used, FileRenameInformation) };
    if status < STATUS_SUCCESS {
        return Err(map_rename_failure(status, true, native.delete_right,
            query_child_nofollow(parent, target)));
    }
    complete_nt(SafeFsOperation::RenameNoReplaceSameParent, status, &iosb)?;
    match query_child_nofollow(parent, target)? {
        ChildState::Present(metadata) if metadata.identity == opened.identity => Ok(()),
        ChildState::Present(metadata) => Err(SafeFsError::IdentityChanged {
            operation: SafeFsOperation::RenameNoReplaceSameParent,
            expected: opened.identity.clone(),
            actual: metadata.identity,
        }),
        ChildState::Absent => Err(SafeFsError::NamespaceChanged {
            operation: SafeFsOperation::RenameNoReplaceSameParent,
        }),
    }
}

fn verify_same_parent(expected: &DirectoryAuthority, actual: &DirectoryAuthority) -> Result<()> {
    if expected.opened.identity == actual.opened.identity && expected.snapshot == actual.snapshot {
        Ok(())
    } else {
        Err(SafeFsError::NamespaceChanged { operation: SafeFsOperation::RenameNoReplaceSameParent })
    }
}

pub(super) fn quarantine_stage(
    stage: StageCapability,
    parent: &DirectoryAuthority,
    quarantine: ComponentName,
) -> Result<QuarantinedCapability> {
    let StageCapability { parent: owned_parent, directory, original_name, opened } = stage;
    verify_same_parent(&owned_parent, parent)?;
    rename_retained_noreplace(&directory.native, &opened, parent, &quarantine)?;
    Ok(QuarantinedCapability {
        parent: owned_parent,
        directory,
        original_name,
        quarantine_name: quarantine,
        opened,
    })
}

pub(super) fn publish_stage_noreplace(
    stage: StageCapability,
    parent: &DirectoryAuthority,
    destination: ComponentName,
) -> Result<()> {
    let StageCapability { parent: owned_parent, directory, opened, .. } = stage;
    verify_same_parent(&owned_parent, parent)?;
    if directory.opened.identity != opened.identity {
        return Err(SafeFsError::IdentityChanged {
            operation: SafeFsOperation::RenameNoReplaceSameParent,
            expected: opened.identity,
            actual: directory.opened.identity,
        });
    }
    rename_retained_noreplace(&directory.native, &directory.opened, parent, &destination)?;
    drop(directory);
    Ok(())
}
```

调用 length 精确为 `total`（转 u32 checked），不是 storage capacity；alignment 由 `Vec<FILE_RENAME_INFORMATION>` 保证。compile/native assertions：`name_offset == offset_of!(..., FileName)`、`name_offset % align_of::<u16>() == 0`、`storage byte capacity >= total`，逐字段 round-trip，UTF-16 length 是 bytes，不含 NUL。target 是 validated single component，不含终止 NUL。

调用：

```text
NtSetInformationFile(
    retained_stage_handle,
    &mut iosb,
    buffer.as_ptr().cast(),
    buffer.len_u32(),
    FileRenameInformation,
)
```

source HANDLE 与 parent capability、buffer storage 在 call 完成前都存活。成功后检查 parent-relative target identity 等于 source retained identity；不相等是 invariant/namespace failure。collision tests 覆盖 file、empty dir、non-empty dir、reparse point，目标 identity/bytes/tree 均不变；没有 `MoveFileExW`、`SetFileInformationByHandle(FileRenameInfo)` 或 joined path fallback。

## 11. Volume / remote mapping proof

initial absolute input 只在 capability capture 边界使用。固定序列：

1. Path prefix 拒绝 UNC、VerbatimUNC、device namespace；只接受 drive/mounted local absolute path。
2. `GetVolumePathNameW` 得到当前 mount mapping；`GetDriveTypeW` 只接受 `DRIVE_FIXED` 或 `DRIVE_REMOVABLE`，其余 remote/unknown/no-root/cdrom/ramdisk fail closed。
3. `GetVolumeNameForVolumeMountPointW` 得到 canonical `\\?\Volume{GUID}\` UTF-16 units，保留原 units。
4. 打开 mapping root，查询 `GetVolumeInformationByHandleW` 的 u32 serial、`FileIdInfo` 的 u64 volume serial + 128-bit root ID。
5. `GetFileInformationByHandleEx(FileRemoteProtocolInfo)` 成功即 remote -> reject。仅当它以 `ERROR_INVALID_PARAMETER`/`ERROR_NOT_SUPPORTED` 失败、drive type 已证明 local、volume GUID/两种 serial/root ID 全部可取时接受；其他错误 fail closed。
6. 从 mapping root 开始用 `NtCreateFile(RootDirectory=previous)` 逐 component 打开；记录 component raw name、FILE_ID_128、volume serial、case mode，并在 capability chain 保留 handle（share 无 DELETE）。每层 remote/volume proof 必须与 root 相同。
7. `revalidate_namespace` 重新执行 2–6，比较 mapping units、GUID、u32/u64 serial、root ID、每层 name/ID/case mode；任一变化 `NamespaceChanged`。原 local mapping 变成 remote/unsupported 也返回 NamespaceChanged，并附 raw probe 诊断。

privilege-free seam 必须能注入 mapping、GUID、serial、root ID、component ID、case 和 remote probe 的每一项变化；real UNC/junction fixture 是 additive，不可用时 seam test 仍为 blocking，不能 silent skip。

完整 capture/dispatch bodies 如下；这是 production code，不是 test fixture：

```rust
fn wide_z(value: &OsStr) -> Result<Vec<u16>> {
    let mut units: Vec<u16> = value.encode_wide().collect();
    if units.contains(&0) { return Err(SafeFsError::InvalidComponent(ComponentViolation::EmbeddedNul)); }
    units.push(0); Ok(units)
}

fn fixed_wide(buffer: &[u16]) -> Result<Vec<u16>> {
    let end = buffer.iter().position(|unit| *unit == 0).ok_or(SafeFsError::InvalidNativeBuffer {
        operation: SafeFsOperation::ProbeVolume, reason: NativeBufferReason::LengthOverflow })?;
    Ok(buffer[..end].to_vec())
}

fn absolute_component_names(path: &Path) -> Result<Vec<ComponentName>> {
    let mut parts = path.components();
    match parts.next() {
        Some(Component::Prefix(prefix)) if matches!(prefix.kind(), Prefix::Disk(_) | Prefix::VerbatimDisk(_)) => {}
        _ => return Err(SafeFsError::UnsupportedSecureFilesystem {
            operation: SafeFsOperation::CaptureNamespaceRoot,
            reason: SecureFilesystemReason::UnstableMapping,
        }),
    }
    if !matches!(parts.next(), Some(Component::RootDir)) {
        return Err(SafeFsError::InvalidRelativePath(RelativePathViolation::AbsoluteOrPrefix));
    }
    parts.map(|part| match part {
        Component::Normal(value) => ComponentName::new(value),
        _ => Err(SafeFsError::InvalidRelativePath(RelativePathViolation::AbsoluteOrPrefix)),
    }).collect()
}

fn probe_volume(path: &Path) -> Result<(OwnedHandle, VolumeProof, LocalFilesystemSnapshot)> {
    let input = wide_z(path.as_os_str())?;
    let mut mapping_buffer = vec![0u16; 32_768];
    // SAFETY: nul input and writable mapped-path buffer with declared capacity.
    if unsafe { GetVolumePathNameW(input.as_ptr(), mapping_buffer.as_mut_ptr(), mapping_buffer.len() as u32) } == 0 {
        return Err(last_win32(SafeFsOperation::ProbeVolume));
    }
    let mapping = fixed_wide(&mapping_buffer)?;
    let mut mapping_z = mapping.clone(); mapping_z.push(0);
    // SAFETY: mapping_z is nul terminated.
    let drive_type = unsafe { GetDriveTypeW(mapping_z.as_ptr()) };
    if drive_type != DRIVE_FIXED && drive_type != DRIVE_REMOVABLE {
        return Err(SafeFsError::UnsupportedSecureFilesystem {
            operation: SafeFsOperation::ProbeVolume,
            reason: SecureFilesystemReason::RemoteFilesystem,
        });
    }
    let mut guid_buffer = vec![0u16; 64];
    // SAFETY: nul mapping and writable GUID buffer.
    if unsafe { GetVolumeNameForVolumeMountPointW(mapping_z.as_ptr(), guid_buffer.as_mut_ptr(), guid_buffer.len() as u32) } == 0 {
        return Err(last_win32(SafeFsOperation::ProbeVolume));
    }
    let guid = fixed_wide(&guid_buffer)?;
    // SAFETY: mapped root path is nul terminated; security/template null; synchronous directory open.
    let raw = unsafe { CreateFileW(mapping_z.as_ptr(), FILE_LIST_DIRECTORY | FILE_TRAVERSE | FILE_READ_ATTRIBUTES,
        SHARE, null(), OPEN_EXISTING, FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT, null_mut()) };
    let root = OwnedHandle::new(raw, SafeFsOperation::ProbeVolume)?;
    let mut volume_serial32 = 0u32;
    // SAFETY: retained volume root and writable serial output; unused output buffers null.
    if unsafe { GetVolumeInformationByHandleW(root.raw(), null_mut(), 0, &mut volume_serial32,
        null_mut(), null_mut(), null_mut(), 0) } == 0 { return Err(last_win32(SafeFsOperation::ProbeVolume)); }
    let id: FILE_ID_INFO = win32_query(root.raw(), FileIdInfo, SafeFsOperation::ProbeVolume)?;
    let mut remote = FILE_REMOTE_PROTOCOL_INFO::default();
    // SAFETY: fixed output is writable and root retained.
    let remote_ok = unsafe { GetFileInformationByHandleEx(root.raw(), FileRemoteProtocolInfo,
        (&mut remote as *mut FILE_REMOTE_PROTOCOL_INFO).cast(), size_of::<FILE_REMOTE_PROTOCOL_INFO>() as u32) };
    if remote_ok != 0 {
        return Err(SafeFsError::UnsupportedSecureFilesystem {
            operation: SafeFsOperation::ProbeVolume,
            reason: SecureFilesystemReason::RemoteFilesystem,
        });
    }
    let remote_error = unsafe { GetLastError() };
    if remote_error != windows_sys::Win32::Foundation::ERROR_INVALID_PARAMETER &&
       remote_error != windows_sys::Win32::Foundation::ERROR_NOT_SUPPORTED {
        return Err(SafeFsError::Os { operation: SafeFsOperation::ProbeVolume, raw: RawOsError::Win32(remote_error) });
    }
    let proof = VolumeProof {
        mapping,
        guid: guid.clone(),
        volume_serial32,
        volume_serial: id.VolumeSerialNumber,
        root_id: id.FileId.Identifier,
    };
    let filesystem = LocalFilesystemSnapshot::Windows { volume_guid: guid, serial: id.VolumeSerialNumber };
    Ok((root, proof, filesystem))
}

fn append_snapshot(parent: &NamespaceSnapshot, name: ComponentName, opened: &EntryMetadata, case_mode: CaseMode) -> Result<NamespaceSnapshot> {
    let filesystem = opened.filesystem.clone().ok_or(SafeFsError::UnsupportedSecureFilesystem {
        operation: SafeFsOperation::ProbeFilesystem,
        reason: SecureFilesystemReason::FilesystemProbeUnavailable,
    })?;
    let mut snapshot = parent.clone();
    snapshot.components.push(NamespaceComponent { name, identity: opened.identity.clone(), filesystem, case_mode });
    Ok(snapshot)
}

fn require_mutation(parent: &DirectoryAuthority, operation: SafeFsOperation) -> Result<()> {
    if parent.access == DirectoryAccess::Read {
        Err(SafeFsError::AccessMismatch { operation })
    } else {
        Ok(())
    }
}

fn open_directory_contract(parent: &DirectoryAuthority, name: &ComponentName, access: DirectoryAccess, contract: OpenContract) -> Result<DirectoryAuthority> {
    let handle = nt_create_relative(parent.native.node.handle.raw(), name, parent.case_mode, contract.desired,
        contract.disposition, contract.options, contract.attributes, null(), SafeFsOperation::OpenDirectory)?;
    let filesystem = parent.opened.filesystem.as_ref().ok_or(SafeFsError::UnsupportedSecureFilesystem {
        operation: SafeFsOperation::ProbeFilesystem, reason: SecureFilesystemReason::FilesystemProbeUnavailable })?;
    let opened = query_entry_metadata(handle.raw(), filesystem, SafeFsOperation::OpenDirectory)?;
    if opened.kind == EntryKind::SymlinkOrReparse { return Err(SafeFsError::SymlinkOrReparsePoint { operation: SafeFsOperation::OpenDirectory }); }
    if opened.kind != EntryKind::Directory { return Err(SafeFsError::UnsupportedEntryType { operation: SafeFsOperation::OpenDirectory, kind: opened.kind }); }
    let case_mode = query_case_mode(handle.raw())?;
    let snapshot = append_snapshot(&parent.snapshot, name.clone(), &opened, case_mode)?;
    let node = Arc::new(DirectoryNode { handle, parent: Some(Arc::clone(&parent.native.node)), name: Some(name.clone()),
        case_mode, metadata: opened.clone(), volume: parent.native.node.volume.clone() });
    Ok(DirectoryAuthority { anchor: Arc::clone(&parent.anchor), native: NativeDirectory { node, access, delete_right: contract.delete_right },
        access, opened, case_mode, snapshot })
}

pub(super) fn capture_absolute_directory(path: &Path, access: DirectoryAccess) -> Result<DirectoryAuthority> {
    if access == DirectoryAccess::Stage {
        return Err(SafeFsError::AccessMismatch { operation: SafeFsOperation::CaptureNamespaceRoot });
    }
    let _validated_absolute = absolute_component_names(path)?;
    let (root_handle, volume, filesystem) = probe_volume(path)?;
    let mapping_path = PathBuf::from(OsString::from_wide(&volume.mapping));
    let relative = path.strip_prefix(&mapping_path).map_err(|_| SafeFsError::UnsupportedSecureFilesystem {
        operation: SafeFsOperation::CaptureNamespaceRoot,
        reason: SecureFilesystemReason::UnstableMapping,
    })?;
    let names: Vec<ComponentName> = relative.components().map(|part| match part {
        Component::Normal(value) => ComponentName::new(value),
        _ => Err(SafeFsError::InvalidRelativePath(RelativePathViolation::AbsoluteOrPrefix)),
    }).collect::<Result<_>>()?;
    let root_opened = query_entry_metadata(root_handle.raw(), &filesystem, SafeFsOperation::CaptureNamespaceRoot)?;
    if root_opened.kind == EntryKind::SymlinkOrReparse { return Err(SafeFsError::SymlinkOrReparsePoint { operation: SafeFsOperation::CaptureNamespaceRoot }); }
    let root_case = query_case_mode(root_handle.raw())?;
    let root_node = Arc::new(DirectoryNode { handle: root_handle, parent: None, name: None, case_mode: root_case,
        metadata: root_opened.clone(), volume: volume.clone() });
    let anchor = Arc::new(NamespaceAnchor { native: NativeNamespaceAnchor { root: Arc::clone(&root_node), mapping: volume,
        absolute_path: path.to_path_buf(), base_components: names.len(), access } });
    let snapshot = NamespaceSnapshot { root_identity: root_opened.identity.clone(), root_filesystem: filesystem,
        root_case_mode: root_case, components: Vec::new() };
    let mut current = DirectoryAuthority { anchor, native: NativeDirectory { node: root_node, access, delete_right: false },
        access, opened: root_opened, case_mode: root_case, snapshot };
    for name in names {
        let operation = if access == DirectoryAccess::Read { OpenOperation::DirRead } else { OpenOperation::DirMutate };
        current = open_directory_contract(&current, &name, access, contract_for_operation(operation))?;
    }
    Ok(current)
}

pub(super) fn revalidate_namespace(directory: &DirectoryAuthority) -> Result<()> {
    let mut path = directory.anchor.native.absolute_path.clone();
    for row in directory.snapshot.components.iter().skip(directory.anchor.native.base_components) {
        path.push(row.name.as_os_str());
    }
    let fresh = capture_absolute_directory(&path, directory.anchor.native.access)
        .map_err(|_| SafeFsError::NamespaceChanged { operation: SafeFsOperation::RevalidateNamespace })?;
    if fresh.snapshot == directory.snapshot && fresh.anchor.native.mapping == directory.anchor.native.mapping {
        Ok(())
    } else {
        Err(SafeFsError::NamespaceChanged { operation: SafeFsOperation::RevalidateNamespace })
    }
}

pub(super) fn query_child_nofollow(parent: &DirectoryAuthority, name: &ComponentName) -> Result<ChildState> {
    let contract = contract_for_operation(OpenOperation::Query);
    let handle = match nt_create_relative(parent.native.node.handle.raw(), name, parent.case_mode, contract.desired,
        contract.disposition, contract.options, 0, null(), SafeFsOperation::QueryChild) {
        Ok(handle) => handle,
        Err(SafeFsError::NotFound { .. }) => return Ok(ChildState::Absent),
        Err(error) => return Err(error),
    };
    let filesystem = parent.opened.filesystem.as_ref().ok_or(SafeFsError::UnsupportedSecureFilesystem {
        operation: SafeFsOperation::ProbeFilesystem, reason: SecureFilesystemReason::FilesystemProbeUnavailable })?;
    Ok(ChildState::Present(query_entry_metadata(handle.raw(), filesystem, SafeFsOperation::QueryChild)?))
}

pub(super) fn open_dir_nofollow(parent: &DirectoryAuthority, name: &ComponentName, access: DirectoryAccess) -> Result<DirectoryAuthority> {
    if access == DirectoryAccess::Stage {
        return Err(SafeFsError::AccessMismatch { operation: SafeFsOperation::OpenDirectory });
    }
    let contract = contract_for_operation(if access == DirectoryAccess::Read { OpenOperation::DirRead } else { OpenOperation::DirMutate });
    open_directory_contract(parent, name, access, contract)
}

pub(super) fn open_file_nofollow(parent: &DirectoryAuthority, name: &ComponentName, access: FileAccess) -> Result<FileCapability> {
    let contract = contract_for_operation(if access == FileAccess::Read { OpenOperation::FileRead } else { OpenOperation::FileWrite });
    let handle = nt_create_relative(parent.native.node.handle.raw(), name, parent.case_mode, contract.desired,
        contract.disposition, contract.options, contract.attributes, null(), SafeFsOperation::OpenFile)?;
    let filesystem = parent.opened.filesystem.as_ref().ok_or(SafeFsError::UnsupportedSecureFilesystem {
        operation: SafeFsOperation::ProbeFilesystem, reason: SecureFilesystemReason::FilesystemProbeUnavailable })?;
    let opened = query_entry_metadata(handle.raw(), filesystem, SafeFsOperation::OpenFile)?;
    if opened.kind == EntryKind::SymlinkOrReparse { return Err(SafeFsError::SymlinkOrReparsePoint { operation: SafeFsOperation::OpenFile }); }
    if opened.kind != EntryKind::RegularFile { return Err(SafeFsError::UnsupportedEntryType { operation: SafeFsOperation::OpenFile, kind: opened.kind }); }
    Ok(FileCapability { native: NativeFile { handle, opened: opened.clone(), access, delete_right: false }, access, opened })
}

pub(super) fn metadata_from_file(file: &NativeFile) -> Result<EntryMetadata> {
    let filesystem = file.opened.filesystem.as_ref().ok_or(SafeFsError::UnsupportedSecureFilesystem {
        operation: SafeFsOperation::ProbeFilesystem, reason: SecureFilesystemReason::FilesystemProbeUnavailable })?;
    query_entry_metadata(file.handle.raw(), filesystem, SafeFsOperation::QueryMetadata)
}

fn create_directory_contract(parent: &DirectoryAuthority, name: &ComponentName, permissions: CreatePermissions,
    access: DirectoryAccess, contract: OpenContract) -> Result<DirectoryAuthority> {
    let operation = if access == DirectoryAccess::Stage { SafeFsOperation::CreateStageDirectory } else { SafeFsOperation::CreateDirectory };
    require_mutation(parent, operation)?;
    let security = match permissions { CreatePermissions::OwnerOnly => Some(OwnerOnlySecurity::new(true)?), CreatePermissions::Inherit => None };
    let pointer = security.as_ref().map_or(null(), OwnerOnlySecurity::descriptor_ptr);
    let handle = nt_create_relative(parent.native.node.handle.raw(), name, parent.case_mode, contract.desired,
        contract.disposition, contract.options, contract.attributes, pointer, operation)?;
    let filesystem = parent.opened.filesystem.as_ref().ok_or(SafeFsError::UnsupportedSecureFilesystem {
        operation: SafeFsOperation::ProbeFilesystem, reason: SecureFilesystemReason::FilesystemProbeUnavailable })?;
    let opened = query_entry_metadata(handle.raw(), filesystem, operation)?;
    if opened.kind != EntryKind::Directory { return Err(SafeFsError::UnsupportedEntryType { operation, kind: opened.kind }); }
    if let Some(expected) = &security {
        if let Err(error) = verify_owner_only(handle.raw(), expected) {
            mark_delete_handle(handle.raw(), SafeFsOperation::DeleteQuarantinedEmptyDirectory)?;
            drop(handle);
            return Err(error);
        }
    }
    let case_mode = query_case_mode(handle.raw())?;
    let snapshot = append_snapshot(&parent.snapshot, name.clone(), &opened, case_mode)?;
    let node = Arc::new(DirectoryNode { handle, parent: Some(Arc::clone(&parent.native.node)), name: Some(name.clone()), case_mode,
        metadata: opened.clone(), volume: parent.native.node.volume.clone() });
    Ok(DirectoryAuthority { anchor: Arc::clone(&parent.anchor), native: NativeDirectory { node, access, delete_right: contract.delete_right },
        access, opened, case_mode, snapshot })
}

pub(super) fn create_dir_new(parent: &DirectoryAuthority, name: &ComponentName, permissions: CreatePermissions,
    access: DirectoryAccess) -> Result<DirectoryAuthority> {
    if access == DirectoryAccess::Stage {
        return Err(SafeFsError::AccessMismatch { operation: SafeFsOperation::CreateDirectory });
    }
    create_directory_contract(parent, name, permissions, access, contract_for_operation(OpenOperation::CreateDir))
}

pub(super) fn create_stage_dir_new(parent: &DirectoryAuthority, name: &ComponentName,
    permissions: CreatePermissions) -> Result<StageCapability> {
    let owned_parent = duplicate_directory(parent)?;
    let directory = create_directory_contract(parent, name, permissions, DirectoryAccess::Stage,
        contract_for_operation(OpenOperation::CreateStage))?;
    let opened = directory.opened.clone();
    Ok(StageCapability { parent: owned_parent, directory, original_name: name.clone(), opened })
}

pub(super) fn create_file_new(parent: &DirectoryAuthority, name: &ComponentName,
    permissions: CreatePermissions) -> Result<FileCapability> {
    require_mutation(parent, SafeFsOperation::CreateFile)?;
    let security = match permissions { CreatePermissions::OwnerOnly => Some(OwnerOnlySecurity::new(false)?), CreatePermissions::Inherit => None };
    let pointer = security.as_ref().map_or(null(), OwnerOnlySecurity::descriptor_ptr);
    let contract = contract_for_operation(OpenOperation::CreateFile);
    let handle = nt_create_relative(parent.native.node.handle.raw(), name, parent.case_mode, contract.desired,
        contract.disposition, contract.options, contract.attributes, pointer,
        SafeFsOperation::CreateFile)?;
    let filesystem = parent.opened.filesystem.as_ref().ok_or(SafeFsError::UnsupportedSecureFilesystem {
        operation: SafeFsOperation::ProbeFilesystem, reason: SecureFilesystemReason::FilesystemProbeUnavailable })?;
    let opened = query_entry_metadata(handle.raw(), filesystem, SafeFsOperation::CreateFile)?;
    if opened.kind != EntryKind::RegularFile { return Err(SafeFsError::UnsupportedEntryType { operation: SafeFsOperation::CreateFile, kind: opened.kind }); }
    if let Some(expected) = &security {
        if let Err(error) = verify_owner_only(handle.raw(), expected) {
            mark_delete_handle(handle.raw(), SafeFsOperation::DeleteQuarantinedEntry)?;
            drop(handle);
            return Err(error);
        }
    }
    Ok(FileCapability { native: NativeFile { handle, opened: opened.clone(), access: FileAccess::ReadWrite, delete_right: false },
        access: FileAccess::ReadWrite, opened })
}

pub(super) fn read_link_component(parent: &DirectoryAuthority, name: &ComponentName) -> Result<RawLinkTarget> {
    let contract = contract_for_operation(OpenOperation::Query);
    let handle = nt_create_relative(parent.native.node.handle.raw(), name, parent.case_mode, contract.desired,
        contract.disposition, contract.options, 0, null(), SafeFsOperation::ReadLink)?;
    let filesystem = parent.opened.filesystem.as_ref().ok_or(SafeFsError::UnsupportedSecureFilesystem {
        operation: SafeFsOperation::ProbeFilesystem, reason: SecureFilesystemReason::FilesystemProbeUnavailable })?;
    let metadata = query_entry_metadata(handle.raw(), filesystem, SafeFsOperation::ReadLink)?;
    if metadata.kind != EntryKind::SymlinkOrReparse {
        return Err(SafeFsError::UnsupportedEntryType { operation: SafeFsOperation::ReadLink, kind: metadata.kind });
    }
    query_reparse(handle.raw())
}
```

## 12. Windows native test 完整清单

以下代码是 `windows.rs` 末尾的完整 test module。它只调用本附录已经定义的 production symbols；本段定义它使用的全部 fixture/helper。

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::process::Command;
    use std::sync::atomic::{AtomicU64, Ordering};
    use windows_sys::Win32::System::Threading::GetProcessHandleCount;

    struct TestDir(PathBuf);
    impl TestDir {
        fn new(label: &str) -> Self {
            static NEXT: AtomicU64 = AtomicU64::new(0);
            let id = NEXT.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!("opentake-c1b-win-{label}-{}-{id}", std::process::id()));
            fs::create_dir(&path).expect("create Windows fixture root");
            Self(path)
        }
        fn path(&self) -> &Path { &self.0 }
    }
    impl Drop for TestDir { fn drop(&mut self) { let _ = fs::remove_dir_all(&self.0); } }

    fn name(value: &str) -> ComponentName { ComponentName::new(value).expect("valid fixture name") }
    fn root(dir: &TestDir) -> DirectoryAuthority {
        capture_absolute_directory(dir.path(), DirectoryAccess::MutateChildren).expect("capture fixture root")
    }
    fn process_handle_count() -> u32 {
        let mut count = 0;
        // SAFETY: current process pseudo-handle is valid and count is writable.
        assert_ne!(unsafe { GetProcessHandleCount(GetCurrentProcess(), &mut count) }, 0);
        count
    }
    fn junction(parent: &Path, link: &str, target: &str) {
        fs::create_dir(parent.join(target)).expect("create junction target");
        let output = Command::new("cmd").args(["/C", "mklink", "/J"])
            .arg(parent.join(link)).arg(parent.join(target)).output().expect("execute mklink");
        assert!(output.status.success(), "mklink failed: {}", String::from_utf8_lossy(&output.stderr));
    }
    fn directory_record(value: &str) -> Vec<u8> {
        let units: Vec<u16> = OsStr::new(value).encode_wide().collect();
        let offset = offset_of!(FILE_DIRECTORY_INFORMATION, FileName);
        let mut bytes = vec![0u8; offset + units.len() * 2];
        let mut header = FILE_DIRECTORY_INFORMATION::default();
        header.FileNameLength = u32::try_from(units.len() * 2).unwrap();
        // SAFETY: buffer has a full header/name and both copies are within bounds.
        unsafe {
            std::ptr::write_unaligned(bytes.as_mut_ptr().cast::<FILE_DIRECTORY_INFORMATION>(), header);
            std::ptr::copy_nonoverlapping(units.as_ptr().cast::<u8>(), bytes.as_mut_ptr().add(offset), units.len() * 2);
        }
        bytes
    }
    fn test_iosb(status: NTSTATUS, information: usize) -> IO_STATUS_BLOCK {
        let mut block = IO_STATUS_BLOCK::default();
        // SAFETY: this test initializes the active Status member before the helper reads it.
        unsafe { block.Anonymous.Status = status; }
        block.Information = information;
        block
    }
    fn present() -> ChildState {
        ChildState::Present(EntryMetadata {
            identity: StableIdentity::Windows { volume_serial: 7, file_id: [3; 16] },
            kind: EntryKind::RegularFile,
            len: 0,
            link_count: 1,
            filesystem: Some(LocalFilesystemSnapshot::Windows { volume_guid: vec![1], serial: 7 }),
        })
    }

    #[test]
    fn component_utf16_and_rejections() {
        let raw = OsString::from_wide(&[0x0061, 0xD800, 0x0062]);
        assert_eq!(ComponentName::new(&raw).unwrap().as_os_str().encode_wide().collect::<Vec<_>>(), vec![0x61, 0xD800, 0x62]);
        for bad in ["a:b", "a\\b", "a/b", "CON", "com1.txt", "trail.", "trail "] {
            assert!(ComponentName::new(bad).is_err(), "accepted {bad:?}");
        }
    }

    #[test]
    fn unicode_and_object_attribute_lifetimes() {
        let nt_name = NtName::new(&name("leaf")).unwrap();
        let attrs = object_attributes(7usize as HANDLE, &nt_name, CaseMode::Insensitive, 9usize as *const c_void);
        assert_eq!(nt_name.unicode.Length, 8);
        assert_eq!(attrs.RootDirectory, 7usize as HANDLE);
        assert_eq!(attrs.Attributes, OBJ_CASE_INSENSITIVE);
        assert_eq!(attrs.SecurityDescriptor, 9usize as *const c_void);
        // SAFETY: NtName owns four UTF-16 units for the assertion lifetime.
        assert_eq!(unsafe { std::slice::from_raw_parts(nt_name.unicode.Buffer, 4) }, &[108, 101, 97, 102]);
    }

    #[test]
    fn operation_contract_spy_all_rows() {
        let rows = [
            (OpenOperation::Query, QUERY_CONTRACT), (OpenOperation::DirRead, DIR_READ_CONTRACT),
            (OpenOperation::DirMutate, DIR_MUTATE_CONTRACT), (OpenOperation::FileRead, FILE_READ_CONTRACT),
            (OpenOperation::FileWrite, FILE_WRITE_CONTRACT), (OpenOperation::CreateFile, CREATE_FILE_CONTRACT),
            (OpenOperation::CreateDir, CREATE_DIR_CONTRACT), (OpenOperation::CreateStage, CREATE_STAGE_CONTRACT),
            (OpenOperation::CleanupFile, CLEANUP_FILE_CONTRACT), (OpenOperation::CleanupDir, CLEANUP_DIR_CONTRACT),
        ];
        assert_eq!(rows.len(), 10);
        for (operation, expected) in rows {
            let recorded = contract_for_operation(operation);
            assert_eq!(recorded, expected);
            assert_eq!(SHARE & FILE_SHARE_DELETE, 0);
            assert_ne!(recorded.options & FILE_OPEN_REPARSE_POINT, 0);
        }
        assert_eq!(QUERY_CONTRACT.disposition, FILE_OPEN);
        assert_eq!(CREATE_FILE_CONTRACT.disposition, FILE_CREATE);
        assert_eq!(CREATE_DIR_CONTRACT.disposition, FILE_CREATE);
        assert_ne!(CREATE_STAGE_CONTRACT.desired & DELETE, 0);
        assert_ne!(CLEANUP_FILE_CONTRACT.desired & DELETE, 0);
        assert_ne!(CLEANUP_DIR_CONTRACT.desired & DELETE, 0);
    }

    #[test]
    fn synchronous_io_status_is_bounded() {
        assert!(matches!(complete_nt(SafeFsOperation::ReadFile, STATUS_PENDING, &test_iosb(0, 0)),
            Err(SafeFsError::InvalidNativeBuffer { reason: NativeBufferReason::PendingOnSynchronousHandle, .. })));
        assert!(checked_information(SafeFsOperation::ReadFile, &test_iosb(0, 9), 8).is_err());
        assert!(complete_nt(SafeFsOperation::ReadFile, STATUS_SUCCESS, &test_iosb(STATUS_ACCESS_DENIED, 0)).is_err());
    }

    #[test]
    fn query_reports_reparse_as_present_and_open_rejects() {
        let temp = TestDir::new("reparse");
        junction(temp.path(), "junction", "target");
        let authority = root(&temp);
        assert!(matches!(query_child_nofollow(&authority, &name("junction")),
            Ok(ChildState::Present(EntryMetadata { kind: EntryKind::SymlinkOrReparse, .. }))));
        assert!(matches!(open_dir_nofollow(&authority, &name("junction"), DirectoryAccess::Read),
            Err(SafeFsError::SymlinkOrReparsePoint { .. })));
    }

    #[test]
    fn reparse_parser_bounds_every_field() {
        let invalid = [vec![], vec![0; 7], vec![1, 0, 0, 0, 10, 0, 0, 0]];
        for bytes in invalid { assert!(matches!(parse_reparse(&bytes),
            Err(SafeFsError::InvalidNativeBuffer { reason: NativeBufferReason::ReparseBufferMalformed, .. }))); }
        let unknown = vec![0x34, 0x12, 0, 0, 2, 0, 0, 0, 0xAA, 0xBB];
        assert_eq!(parse_reparse(&unknown).unwrap(), (0x1234, unknown.clone()));
    }

    #[test]
    fn directory_parser_bounds_and_requery() {
        assert_eq!(parse_directory_batch(&directory_record("leaf")).unwrap(), vec![name("leaf")]);
        assert!(parse_directory_batch(&[]).is_err());
        let mut odd = directory_record("leaf");
        let offset = offset_of!(FILE_DIRECTORY_INFORMATION, FileNameLength);
        odd[offset..offset + 4].copy_from_slice(&3u32.to_le_bytes());
        assert!(parse_directory_batch(&odd).is_err());
    }

    #[test]
    fn metadata_types_and_hardlinks() {
        let temp = TestDir::new("metadata");
        fs::write(temp.path().join("file"), b"abc").unwrap();
        fs::hard_link(temp.path().join("file"), temp.path().join("hard")).unwrap();
        fs::create_dir(temp.path().join("dir")).unwrap();
        let authority = root(&temp);
        let file = match query_child_nofollow(&authority, &name("file")).unwrap() { ChildState::Present(m) => m, ChildState::Absent => panic!() };
        let hard = match query_child_nofollow(&authority, &name("hard")).unwrap() { ChildState::Present(m) => m, ChildState::Absent => panic!() };
        let dir = match query_child_nofollow(&authority, &name("dir")).unwrap() { ChildState::Present(m) => m, ChildState::Absent => panic!() };
        assert_eq!(file.kind, EntryKind::RegularFile); assert_eq!(file.len, 3); assert_eq!(file.link_count, 2);
        assert_eq!(file.identity, hard.identity); assert_eq!(dir.kind, EntryKind::Directory);
    }

    #[test]
    fn nested_retained_io_roundtrip() {
        let temp = TestDir::new("nested");
        let authority = root(&temp);
        let a = create_dir_new(&authority, &name("a"), CreatePermissions::OwnerOnly, DirectoryAccess::MutateChildren).unwrap();
        let b = create_dir_new(&a, &name("b"), CreatePermissions::OwnerOnly, DirectoryAccess::MutateChildren).unwrap();
        let mut file = create_file_new(&b, &name("data"), CreatePermissions::OwnerOnly).unwrap();
        file.write_all(b"retained").unwrap(); file.flush().unwrap(); file.sync_all().unwrap(); file.seek(SeekFrom::Start(0)).unwrap();
        let mut output = [0u8; 8]; assert_eq!(file.read(&mut output).unwrap(), 8); assert_eq!(&output, b"retained");
        assert_eq!(enumerate(&b).unwrap(), vec![name("data")]);
    }

    #[test]
    fn ten_thousand_handles_return_to_baseline() {
        let temp = TestDir::new("handles"); fs::write(temp.path().join("leaf"), b"x").unwrap();
        let authority = root(&temp); let before = process_handle_count();
        for _ in 0..10_000 { drop(open_file_nofollow(&authority, &name("leaf"), FileAccess::Read).unwrap()); }
        assert_eq!(process_handle_count(), before);
    }

    #[test]
    fn ancestor_mapping_cannot_rebind() {
        let temp = TestDir::new("mapping"); let authority = root(&temp);
        let target = temp.path().with_extension("renamed");
        assert!(fs::rename(temp.path(), &target).is_err());
        revalidate_namespace(&authority).unwrap();
    }

    #[test]
    fn every_volume_field_is_bound() {
        let temp = TestDir::new("volume"); let authority = root(&temp);
        match authority.opened.filesystem.as_ref().unwrap() {
            LocalFilesystemSnapshot::Windows { volume_guid, serial } => { assert!(!volume_guid.is_empty()); assert_ne!(*serial, 0); }
            _ => panic!("Windows capture returned non-Windows filesystem"),
        }
        assert!(!authority.snapshot.components.is_empty());
        revalidate_namespace(&authority).unwrap();
        assert!(matches!(capture_absolute_directory(Path::new(r"\\server\share\x"), DirectoryAccess::Read),
            Err(SafeFsError::UnsupportedSecureFilesystem { .. })));
    }

    #[test]
    fn owner_only_dacl_is_exact_and_rollback_is_closed() {
        let temp = TestDir::new("dacl"); let authority = root(&temp);
        let file = create_file_new(&authority, &name("file"), CreatePermissions::OwnerOnly).unwrap();
        let expected = OwnerOnlySecurity::new(false).unwrap();
        verify_owner_only(file.native.handle.raw(), &expected).unwrap();
        let mut wrong = OwnerOnlySecurity::new(false).unwrap(); wrong.ace_flags ^= OBJECT_INHERIT_ACE;
        assert!(verify_owner_only(file.native.handle.raw(), &wrong).is_err());
        mark_delete_handle(file.native.handle.raw(), SafeFsOperation::DeleteQuarantinedEntry).unwrap();
        drop(file);
        assert!(matches!(query_child_nofollow(&authority, &name("file")).unwrap(), ChildState::Absent));
    }

    #[test]
    fn create_new_preserves_every_existing_kind() {
        let temp = TestDir::new("exclusive"); fs::write(temp.path().join("file"), b"before").unwrap(); fs::create_dir(temp.path().join("dir")).unwrap();
        let authority = root(&temp);
        assert!(matches!(create_file_new(&authority, &name("file"), CreatePermissions::OwnerOnly), Err(SafeFsError::AlreadyExists { .. })));
        assert_eq!(fs::read(temp.path().join("file")).unwrap(), b"before");
        assert!(matches!(create_dir_new(&authority, &name("dir"), CreatePermissions::OwnerOnly, DirectoryAccess::Read), Err(SafeFsError::AlreadyExists { .. })));
    }

    #[test]
    fn retained_delete_ignores_name_rebound() {
        let temp = TestDir::new("delete"); let authority = root(&temp);
        let stage = create_stage_dir_new(&authority, &name("stage"), CreatePermissions::OwnerOnly).unwrap();
        let mut leaf = create_file_new(stage.directory(), &name("leaf"), CreatePermissions::OwnerOnly).unwrap(); leaf.write_all(b"original").unwrap(); drop(leaf);
        assert!(fs::rename(temp.path().join("stage"), temp.path().join("replacement")).is_err());
        let quarantine = quarantine_stage(stage, &authority, name("quarantine")).unwrap();
        let leaf = open_cleanup_child_nofollow(&quarantine, &name("leaf")).unwrap(); delete_quarantined_entry(leaf).unwrap();
        delete_quarantined_empty_directory(quarantine).unwrap();
        assert!(matches!(query_child_nofollow(&authority, &name("quarantine")).unwrap(), ChildState::Absent));
    }

    #[test]
    fn rename_layout_is_exact() {
        let temp = TestDir::new("layout"); let authority = root(&temp);
        let buffer = RenameInformationBuffer::new(authority.native.node.handle.raw(), &ComponentName::new(OsString::from_wide(&[0x61, 0xD800])).unwrap()).unwrap();
        assert_eq!(buffer.used as usize, offset_of!(FILE_RENAME_INFORMATION, FileName) + 4);
        assert_eq!((buffer.as_ptr() as usize) % align_of::<FILE_RENAME_INFORMATION>(), 0);
        // SAFETY: builder returned aligned initialized header.
        assert_eq!(unsafe { (*(buffer.as_ptr().cast::<FILE_RENAME_INFORMATION>())).RootDirectory }, authority.native.node.handle.raw());
    }

    #[test]
    fn rename_never_replaces_any_target_kind() {
        let temp = TestDir::new("collision"); fs::write(temp.path().join("target"), b"keep").unwrap();
        let authority = root(&temp); let stage = create_stage_dir_new(&authority, &name("stage"), CreatePermissions::OwnerOnly).unwrap();
        assert!(matches!(publish_stage_noreplace(stage, &authority, name("target")), Err(SafeFsError::AlreadyExists { .. })));
        assert_eq!(fs::read(temp.path().join("target")).unwrap(), b"keep");
    }

    #[test]
    fn ambiguous_rename_requires_all_three_proofs() {
        assert!(matches!(map_rename_failure(STATUS_ACCESS_DENIED, true, true, Ok(present())), SafeFsError::AlreadyExists { .. }));
        assert!(matches!(map_rename_failure(STATUS_ACCESS_DENIED, true, true, Ok(ChildState::Absent)),
            SafeFsError::Os { raw: RawOsError::NtStatus { status: STATUS_ACCESS_DENIED, .. }, .. }));
        assert!(matches!(map_rename_failure(STATUS_ACCESS_DENIED, false, true, Ok(present())), SafeFsError::Os { .. }));
    }

    #[test]
    fn ntstatus_mapping_is_operation_specific() {
        assert!(matches!(nt_error(SafeFsOperation::OpenFile, STATUS_OBJECT_NAME_NOT_FOUND), SafeFsError::NotFound { .. }));
        for status in [STATUS_ACCESS_DENIED, STATUS_SHARING_VIOLATION, STATUS_DELETE_PENDING] {
            assert!(matches!(nt_error(SafeFsOperation::OpenFile, status),
                SafeFsError::Os { raw: RawOsError::NtStatus { status: value, .. }, .. } if value == status));
        }
        assert!(matches!(nt_error(SafeFsOperation::QueryCaseMode, STATUS_NOT_SUPPORTED),
            SafeFsError::UnsupportedSecureFilesystem { reason: SecureFilesystemReason::CaseSemanticsUnavailable, .. }));
    }

    #[test]
    fn production_capabilities_own_drop_resources() {
        assert!(std::mem::needs_drop::<NativeFile>());
        assert!(std::mem::needs_drop::<NativeDirectory>());
        assert!(std::mem::needs_drop::<NativeNamespaceAnchor>());
    }
}
```

Windows task 的 GREEN 命令固定为：

```powershell
cargo fmt --all --check
cargo clippy -p opentake-project --lib --tests -- -D warnings
cargo test -p opentake-project --lib safe_fs::windows::tests -- --test-threads=1
cargo test -p opentake-project --test archive_security -- --test-threads=1
cargo check -p opentake-project --lib --tests --target x86_64-pc-windows-msvc
git diff --check
```

## 13. 完整 GitHub Actions YAML

attempt 2 用以下完整 `.github/workflows/ci.yml`，保留现有 rust/web jobs并加入 exact-SHA native matrix。`workflow_dispatch` 只有该 workflow 已存在 default branch 时才可调用；在它首次进入 default branch 前，中间 SHA 只能通过授权 PR 的 head-SHA path 取得 receipt。

```yaml
name: CI

on:
  push:
    branches: [main]
  pull_request:
  workflow_dispatch:
    inputs:
      commit_sha:
        description: Immutable 40-hex commit SHA already present in this repository
        required: true
        type: string

permissions:
  contents: read

concurrency:
  group: ${{ github.workflow }}-${{ github.event_name }}-${{ github.ref }}-${{ inputs.commit_sha || github.sha }}
  cancel-in-progress: true

jobs:
  rust:
    name: Rust (fmt / clippy / test)
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Free disk space (playback-engine deps are heavy)
        run: |
          sudo rm -rf /usr/share/dotnet /opt/ghc /usr/local/lib/android "$AGENT_TOOLSDIRECTORY" /opt/hostedtoolcache/CodeQL || true
          sudo docker image prune --all --force || true
          df -h /
      - name: Install Rust toolchain
        run: rustup component add rustfmt clippy
      - name: Install system deps (ffmpeg + Tauri/GTK)
        run: |
          sudo apt-get update
          sudo apt-get install -y ffmpeg libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev libasound2-dev libglib2.0-dev libsoup-3.0-dev patchelf pkg-config fonts-dejavu-core
      - name: Cache cargo
        uses: actions/cache@v4
        with:
          path: |
            ~/.cargo/registry
            ~/.cargo/git
            target
          key: ${{ runner.os }}-cargo-${{ hashFiles('**/Cargo.toml', 'Cargo.lock') }}
          restore-keys: ${{ runner.os }}-cargo-
      - name: cargo fmt
        run: cargo fmt --all --check
      - name: cargo clippy
        run: cargo clippy --workspace --all-targets -- -D warnings
      - name: cargo test
        run: cargo test --workspace
      - name: live playback transport integration (fail closed)
        run: |
          set -euo pipefail
          cargo test -p opentake-tauri --features playback-engine --test playback_transport_integration -- --test-threads=1
      - name: cargo clippy (minimal, no default features)
        run: cargo clippy -p opentake-tauri --no-default-features --all-targets -- -D warnings

  web:
    name: Web (install / build)
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: pnpm/action-setup@v4
        with:
          version: 10
      - uses: actions/setup-node@v4
        with:
          node-version: 22
          cache: pnpm
          cache-dependency-path: web/pnpm-lock.yaml
      - name: pnpm install
        run: pnpm -C web install
      - name: pnpm build
        run: pnpm -C web build
      - name: pnpm test
        run: pnpm -C web test

  safe-filesystem:
    name: Safe filesystem (${{ matrix.receipt_id }})
    strategy:
      fail-fast: false
      matrix:
        include:
          - receipt_id: linux-x86_64
            runner: ubuntu-24.04
          - receipt_id: macos-native
            runner: macos-14
          - receipt_id: windows-x86_64
            runner: windows-2022
    runs-on: ${{ matrix.runner }}
    timeout-minutes: 35
    env:
      TARGET_SHA: ${{ github.event_name == 'workflow_dispatch' && inputs.commit_sha || github.event_name == 'pull_request' && github.event.pull_request.head.sha || github.sha }}
      RECEIPT_DIR: c1b-native-receipt
    steps:
      - name: Validate immutable SHA input
        shell: bash
        run: |
          set -euo pipefail
          [[ "$TARGET_SHA" =~ ^[0-9a-fA-F]{40}$ ]]
      - uses: actions/checkout@v4
        with:
          ref: ${{ env.TARGET_SHA }}
          fetch-depth: 0
          persist-credentials: false
      - name: Assert exact checked-out SHA
        id: bind
        shell: bash
        run: |
          set -euo pipefail
          actual="$(git rev-parse HEAD | tr '[:upper:]' '[:lower:]')"
          expected="$(printf '%s' "$TARGET_SHA" | tr '[:upper:]' '[:lower:]')"
          test "$actual" = "$expected"
          git cat-file -e "${expected}^{commit}"
          printf 'sha=%s\n' "$actual" >> "$GITHUB_OUTPUT"
      - name: Install Rust components
        shell: bash
        run: rustup component add rustfmt clippy
      - name: Cache cargo
        uses: actions/cache@v4
        with:
          path: |
            ~/.cargo/registry
            ~/.cargo/git
            target
          key: safe-fs-${{ matrix.receipt_id }}-${{ hashFiles('**/Cargo.toml', 'Cargo.lock') }}
          restore-keys: safe-fs-${{ matrix.receipt_id }}-
      - name: Run all native gates and retain every exit
        shell: bash
        run: |
          set -u
          mkdir "$RECEIPT_DIR"
          aggregate=0
          run_gate() {
            id="$1"
            shift
            set +e
            "$@" >"$RECEIPT_DIR/$id.log" 2>&1
            code=$?
            set -e
            printf '%s\n' "$code" >"$RECEIPT_DIR/$id.raw-exit"
            if [ "$code" -ne 0 ]; then aggregate=1; fi
          }
          set -e
          run_gate cargo-fmt cargo fmt --all --check
          run_gate cargo-clippy cargo clippy -p opentake-project --lib --tests -- -D warnings
          run_gate safe-fs-unit cargo test -p opentake-project --lib safe_fs -- --test-threads=1
          run_gate archive-security cargo test -p opentake-project --test archive_security -- --test-threads=1
          printf '%s\n' "$aggregate" >"$RECEIPT_DIR/final-aggregate.raw-exit"
      - name: Build exclusive JSON receipt
        if: always()
        shell: pwsh
        env:
          RECEIPT_SHA: ${{ steps.bind.outputs.sha }}
          RECEIPT_ID: ${{ matrix.receipt_id }}
          RUNNER_LABEL: ${{ matrix.runner }}
        run: |
          $commands = @(
            @{ id = 'cargo-fmt'; command = 'cargo fmt --all --check' },
            @{ id = 'cargo-clippy'; command = 'cargo clippy -p opentake-project --lib --tests -- -D warnings' },
            @{ id = 'safe-fs-unit'; command = 'cargo test -p opentake-project --lib safe_fs -- --test-threads=1' },
            @{ id = 'archive-security'; command = 'cargo test -p opentake-project --test archive_security -- --test-threads=1' }
          ) | ForEach-Object {
            $exitPath = Join-Path $env:RECEIPT_DIR ($_.id + '.raw-exit')
            $_ + @{ exit_code = [int](Get-Content $exitPath); log = ($_.id + '.log'); raw_exit = ($_.id + '.raw-exit') }
          }
          $receipt = [ordered]@{
            schema = 'opentake-c1b-native-receipt-v1'
            repository = '${{ github.repository }}'
            workflow = '${{ github.workflow }}'
            workflow_file = '.github/workflows/ci.yml'
            run_id = '${{ github.run_id }}'
            run_attempt = '${{ github.run_attempt }}'
            job_id = '${{ github.job }}'
            receipt_id = $env:RECEIPT_ID
            runner_label = $env:RUNNER_LABEL
            runner_os = '${{ runner.os }}'
            runner_arch = '${{ runner.arch }}'
            event_name = '${{ github.event_name }}'
            requested_sha = $env:TARGET_SHA.ToLowerInvariant()
            checked_out_sha = $env:RECEIPT_SHA.ToLowerInvariant()
            commands = @($commands)
            aggregate_exit = [int](Get-Content (Join-Path $env:RECEIPT_DIR 'final-aggregate.raw-exit'))
          }
          $receipt | ConvertTo-Json -Depth 8 | Set-Content -Encoding utf8NoBOM (Join-Path $env:RECEIPT_DIR 'receipt.json')
      - name: Upload immutable native receipt
        if: always()
        uses: actions/upload-artifact@v4
        with:
          name: c1b-native-${{ matrix.receipt_id }}-${{ steps.bind.outputs.sha }}
          path: c1b-native-receipt/
          if-no-files-found: error
          retention-days: 30
      - name: Enforce native aggregate
        if: always()
        shell: bash
        run: |
          set -euo pipefail
          test -f "$RECEIPT_DIR/final-aggregate.raw-exit"
          test "$(cat "$RECEIPT_DIR/final-aggregate.raw-exit")" = 0
```

展开后的 blocking job 名精确是：

- `Safe filesystem (linux-x86_64)`
- `Safe filesystem (macos-native)`
- `Safe filesystem (windows-x86_64)`

controller 必须检查三份 `receipt.json` 的 `schema`、`checked_out_sha`、`requested_sha`、四个 command exit 与 aggregate；GitHub UI green 本身不代替 receipt content validation。

CI implementation 的本地 GREEN 先运行 `actionlint .github/workflows/ci.yml`。若本机无 `actionlint`，固定 fallback 是下面的 Ruby assertion，不允许只做 YAML parse：

```bash
ruby -ryaml -e '
p = ".github/workflows/ci.yml"
raw = File.read(p)
y = YAML.safe_load(raw, aliases: true)
events = y["on"] || y[true] or raise "on"
raise "push" unless events.fetch("push").fetch("branches") == ["main"]
raise "pull_request" unless events.key?("pull_request")
raise "dispatch input" unless events.fetch("workflow_dispatch").fetch("inputs").fetch("commit_sha").fetch("required") == true
job = y.fetch("jobs").fetch("safe-filesystem")
rows = job.fetch("strategy").fetch("matrix").fetch("include")
raise "matrix" unless rows.map { |r| r.fetch("receipt_id") }.sort == %w[linux-x86_64 macos-native windows-x86_64]
raise "runners" unless rows.map { |r| r.fetch("runner") }.sort == %w[macos-14 ubuntu-24.04 windows-2022]
target = job.fetch("env").fetch("TARGET_SHA")
%w[workflow_dispatch pull_request github.event.pull_request.head.sha github.sha inputs.commit_sha].each { |token| raise token unless target.include?(token) }
checkout = job.fetch("steps").find { |s| s["uses"] == "actions/checkout@v4" }
raise "checkout ref" unless checkout.fetch("with").fetch("ref") == "${{ env.TARGET_SHA }}"
raise "fetch depth" unless checkout.fetch("with").fetch("fetch-depth") == 0
text = job.fetch("steps").map { |s| s["run"].to_s }.join("\n")
%w[git\ rev-parse\ HEAD requested_sha checked_out_sha aggregate_exit opentake-c1b-native-receipt-v1].each { |token| raise token unless raw.include?(token.gsub("\\ ", " ")) || text.include?(token.gsub("\\ ", " ")) }
'
```

三 event 的静态 assertions 必须分别证明：push 使用 `github.sha`；pull request 使用 `github.event.pull_request.head.sha` 而不是 synthetic merge SHA；workflow_dispatch 使用必填 `inputs.commit_sha`。receipt build 必须在 `if: always()` 下执行，artifact name 包含 receipt id 和 asserted SHA，enforce step 必须读取 raw aggregate。

## 14. 远端权限与可执行 BLOCKED 规则

实施请求本身不自动授权 push/PR。执行者在首次需要 native receipt 前运行只读权限探针：

```bash
gh auth status
gh repo view appergb/OpenTake --json nameWithOwner,viewerPermission,defaultBranchRef
```

只有用户在当前任务明确授权远端 publication，且 `viewerPermission` 为可 push 角色时，才可按隔离 feature branch push/open-or-update PR。若任一条件不成立：

```text
STATUS=BLOCKED
REASON=native_receipts_require_authorized_remote_pr_or_preexisting_default_branch_dispatch_workflow
LAST_VERIFIED_SHA=<40-hex local task SHA>
MISSING_RECEIPTS=linux-x86_64,macos-native,windows-x86_64
NO_REMOTE_MUTATION_PERFORMED=true
```

随后停止在该 task gate。不得 push；不得开 PR；不得继续下一个依赖 native receipt 的 code task；不得用 macOS 本机结果或 Windows/Linux cross-check 填补缺失 receipt。

若已有授权 PR，自动 `pull_request` run 会检出/assert `github.event.pull_request.head.sha`。若 workflow 已经存在 default branch 且有明确授权，可手动运行：

```bash
gh workflow run ci.yml --ref main -f commit_sha="$SHA"
```

dispatch 输入 SHA 必须已存在于远端 repository；不可调度本地未发布 object。

## 15. Exclusive evidence / receipt schema 与绝对路径

固定：

```bash
SAFETY_ROOT='/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake-safety/20260712-wave1bc-filesystem'
REPO_ROOT='/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake-full-convergence'
```

每个 task report directory：

```text
/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake-safety/20260712-wave1bc-filesystem/logs/c1b-task-<N>-<SHA>-attempt-<M>/spec-security-review.md
/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake-safety/20260712-wave1bc-filesystem/logs/c1b-task-<N>-<SHA>-attempt-<M>/implementation-review.md
```

创建必须 exclusive：先 `mkdir "$SAFETY_ROOT/logs/c1b-task-$TASK-$SHA-attempt-$ATTEMPT"`，目录已存在即退出，不使用 `mkdir -p`、不覆盖旧 attempt。两份 report 必须含 `Role`、完整 `Commit`、`Verdict: APPROVE`、`Critical: 0`、`Important: 0`、`Minor: 0`；任一 finding 产生新 commit 与新 attempt directory，两角色全部重审。

最终 branch gate：

```text
/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake-safety/20260712-wave1bc-filesystem/branch-gates/c1b-<UTCSTAMP>-<SHA>-<NONCE>/
```

同样用单次 exclusive `mkdir`。每个本地命令恰好有 `<id>.log`、`<id>.raw-exit`；`command-ledger.json` 每项 schema：

```json
{
  "id": "cargo-fmt",
  "command": "cargo fmt --all --check",
  "cwd": "/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake-full-convergence",
  "started_at_utc": "RFC3339",
  "finished_at_utc": "RFC3339",
  "exit_code": 0,
  "log": "cargo-fmt.log",
  "raw_exit": "cargo-fmt.raw-exit"
}
```

`native-receipts/<run-id>/<receipt-id>/receipt.json` 保存三份下载 artifact；禁止同一 receipt_id 重复。最终 `results.md` 必须列 baseline SHA、final SHA、pre/post status、每个 local gate exit、三份 run id/attempt/receipt SHA、两份 final audit 路径、aggregate。validation 脚本必须拒绝：非 40-hex SHA、三 OS SHA 不同、requested/checked-out 不同、duplicate receipt_id、任一 command/aggregate 非 0、report commit 不等 final SHA、非 clean status、缺 log/raw-exit。

## 16. Task slicing 与 RED/GREEN/receipt gate

每个行为 slice 必须是两个 commit：test-only RED，随后 production GREEN。禁止同一 commit 同时加入 test 和实现。Windows runner 上验证单个 RED 的固定函数如下；它同时断言 exit 非零、filter 实际运行恰好一个 test、失败恰好一个，因 absent module/0 tests/compile error 失败都不会通过：

```bash
expect_red() {
  name="$1"
  log="$2"
  set +e
  cargo test -p opentake-project --lib "$name" -- --exact --test-threads=1 2>&1 | tee "$log"
  code=${PIPESTATUS[0]}
  set -e
  test "$code" -ne 0
  test "$(grep -Ec '^running 1 test$' "$log")" -eq 1
  test "$(grep -Ec '^test result: FAILED\. 0 passed; 1 failed;' "$log")" -eq 1
  test "$(grep -Ec "^test ${name//::/::} \.\.\. FAILED$" "$log")" -eq 1
}
```

固定 commit/gate sequence：

1. test-only `test(ci): specify immutable safe filesystem receipts`；只 add `scripts/validate-c1b-ci.rb` 与 `scripts/tests/validate-c1b-ci-test.rb`。RED：`ruby scripts/tests/validate-c1b-ci-test.rb`，必须因缺 `safe-filesystem`/SHA binding assertion 失败，不能因 YAML syntax 失败。GREEN commit `ci: bind safe filesystem receipts to immutable sha`；只 add `.github/workflows/ci.yml`，先 `actionlint`，不可用时跑 section 13 Ruby，再跑 test script，三者 exit 0 后才审查。
2. test-only `test(project): specify Windows capability opens and io`；只 add `crates/opentake-project/src/safe_fs/windows.rs` 的 `#[cfg(test)]` module、common compile seam 与 Cargo Windows dependency，不加入 production Nt bodies。先在 Windows runner 运行：
   `expect_red 'safe_fs::windows::tests::operation_contract_spy_all_rows' "$EVIDENCE/windows-open-red.log"`、
   `expect_red 'safe_fs::windows::tests::query_reports_reparse_as_present_and_open_rejects' "$EVIDENCE/windows-reparse-red.log"`、
   `expect_red 'safe_fs::windows::tests::nested_retained_io_roundtrip' "$EVIDENCE/windows-io-red.log"`。
   GREEN commit `feat(project): capture Windows filesystem capabilities` 只加入 sections 2–8、11 的 production bodies；三个 exact tests exit 0，随后 section 12 全组、clippy/check/native receipt，两角色 0/0/0。
3. test-only `test(project): specify Windows retained-handle mutations`；只增加 DACL/delete/rename tests。Windows RED 固定：
   `expect_red 'safe_fs::windows::tests::owner_only_dacl_is_exact_and_rollback_is_closed' "$EVIDENCE/windows-dacl-red.log"`、
   `expect_red 'safe_fs::windows::tests::retained_delete_ignores_name_rebound' "$EVIDENCE/windows-delete-red.log"`、
   `expect_red 'safe_fs::windows::tests::rename_never_replaces_any_target_kind' "$EVIDENCE/windows-rename-red.log"`。
   GREEN commit `feat(project): add Windows capability-relative mutations` 只加入 sections 8–10 production bodies；三个 exact tests和全组 exit 0，随后 native receipt + 两角色 0/0/0。
4. convergence SHA 不增加行为；只允许 review-fix commits。必须取得 Linux/macOS/Windows 同一 40-hex SHA receipt，再跑 workspace/no-default/product-closed checks 与 final two-role audit。

每个 commit 前 `git diff --cached --name-only` 必须等于该步骤列出的 paths；每个 GREEN 前保存对应 RED commit SHA/log/receipt。任何 RED 在 `running 1 test` 前失败都要先修 test harness 并产生新的 test-only commit；不得把 harness compile failure记录成行为 RED。本稿不授权 push/PR；到首次 native receipt gate 若无远端 authority，按 section 14 写 BLOCKED 并停止。

## 17. 仍未消除、必须如实保留的风险

1. 本机不是 Windows，所有 NT/DACL/layout 行为只有 Windows native receipt 才能升级为已验证；本稿只依据本机 `windows-sys 0.61.2` declaration冻结调用形状。
2. `workflow_dispatch` workflow 首次进入 default branch 前不可作为手工 trigger；无 authorized PR 时原生 gate 必然 BLOCKED。
3. Windows filesystem/filter driver 可能对 ambiguous rename collision 返回不同 NTSTATUS；本稿通过 pre/post relative query 收窄 classification，但 native tests必须在实际 runner上锁定 file/dir/reparse cases。
4. per-directory case-sensitive query、removable volume 和 remote protocol probe 在旧 Windows/filesystem上可能 unsupported；行为是 typed fail closed，不承诺所有卷都支持 export。
5. Unix 无通用“按 fd unlink name”原语；主计划必须采用 quarantine/fail-leak 或明确缩窄 threat contract，不能恢复 attempt 1 的原子 identity-bound 声称。

以上风险都不是允许 pathname fallback、ordinary rename、跳过 native receipt 或擅自远端 publication 的理由。
