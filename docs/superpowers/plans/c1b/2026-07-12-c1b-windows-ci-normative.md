# C1B Windows, Native CI, and Evidence Normative Appendix — Attempt 6

## 0. 绑定范围与来源

本稿只修订 C1B 计划中的 Windows、三平台原生 CI 与 evidence 部分，并给主计划必须同步采用的 common-facade 修正。它不修改 OpenTake 仓库，不恢复 bundle command/UI，也不授权 push、开 PR 或修改远端状态。

绑定版本：

- 批准 design：`31bfd57e40e3a2bd0ca42b331e5aa877db2d6ace`
- attempt-1 plan：`1b3305ac752977301f9af19fe4e7937d628e0100`
- C1B baseline：`e67917260ace36e4db1ede4e36eecbc401825bb1`
- 本机 API：`windows-sys = 0.61.2`
- safety root：`/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake-safety/20260712-wave1bc-filesystem`
- integration repo root：`/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake-full-convergence`

Attempt 6 必须删除 Attempt 1–5 的以下决定：

1. 统一使用 common appendix 的递归 `DirectoryAuthority`；每个打开/创建的目录 authority 都能作为下一层 parent。
2. 不再把 file capability 做成只能查 metadata 的死端。它提供受控 `read`、`write_all`、`rewind`、`flush`，raw HANDLE 不外泄。
3. Windows rename 不使用 `SetFileInformationByHandle(FILE_RENAME_INFO)`。唯一允许的调用是 `NtSetInformationFile(FileRenameInformation)` 配 `FILE_RENAME_INFORMATION.RootDirectory`。
4. Windows delete 不使用名称型 `DeleteFileW`/`RemoveDirectoryW`。它对 retained child HANDLE 调 `NtSetInformationFile(FileDispositionInformation)`。
5. `RtlNtStatusToDosError` 只作未识别状态的诊断 fallback；控制流先匹配原始 `NTSTATUS`。
6. 原生 receipt 没有远端发布权限时必须写成 `BLOCKED`；不得自行 push、开 PR 或把 cross-check 冒充原生行为结果。

## 1. Common facade 必须同步改正

### 1.1 Authority 形状

Common facade 以 common/Unix appendix section 2 为唯一来源。Attempt 6 的 Windows adapter 只包含下面这些既有 common symbols；名称和参数逐项一致，`windows.rs` 不包含兼容别名或第二 facade：

```text
platform::{NativeNamespaceAnchor, NativeDirectory, NativeFile};

pub(crate) struct DirectoryAuthority { /* native: NativeDirectory; move-only */ }
pub(crate) struct FileCapability { /* native: NativeFile; move-only */ }
pub(crate) struct StageCapability { /* parent + directory; directory HANDLE has DELETE */ }
pub(crate) struct QuarantinedCapability { /* same parent + directory HANDLE */ }
pub(crate) enum CleanupCapability { /* Entry owns Box<CleanupEntry>; Directory owns Box<QuarantinedCapability> */ }

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
platform::metadata_from_file(&NativeFile) -> Result<EntryMetadata>;
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

Windows 私有 `DirectoryAccess::{Read, MutateChildren, Stage}` 决定目录自身 child rights；`Stage` 只允许 exclusive stage create 的 returned authority，absolute capture/open child拒绝该值。`create_stage_dir_new` 创建的 `NativeDirectory` 从 `NtCreateFile(FILE_CREATE)` 起就带 `DELETE`，`quarantine_stage`、`publish_stage_noreplace` 和 directory delete 都消费并移动这一个 directory HANDLE。cleanup leaf/subdir 由 `open_cleanup_child_nofollow` 第一次打开时取得 DELETE，并把同一个 source HANDLE 放进 consuming capability；禁止重开或 duplicate source HANDLE。为了让递归 child capability 拥有 parent authority，adapter 只允许 `DuplicateHandle(..., DUPLICATE_SAME_ACCESS)` 复制 retained parent directory HANDLE（同一 kernel object，不按名称重开）。普通 ancestor/destination parent 不请求 self-delete。所有 retained HANDLE 的 share mask 都是 `FILE_SHARE_READ | FILE_SHARE_WRITE`，刻意省略 `FILE_SHARE_DELETE`。

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
use windows_sys::core::BOOL;
use windows_sys::Wdk::Foundation::{OBJECT_ATTRIBUTES, OBJ_CASE_INSENSITIVE};
use windows_sys::Wdk::Storage::FileSystem::*;
use windows_sys::Win32::Foundation::{
    CloseHandle, DuplicateHandle, GetLastError, HANDLE, INVALID_HANDLE_VALUE, NTSTATUS,
    DUPLICATE_SAME_ACCESS, STATUS_ACCESS_DENIED,
    STATUS_BUFFER_OVERFLOW, STATUS_BUFFER_TOO_SMALL, STATUS_CANNOT_DELETE, STATUS_DELETE_PENDING,
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
    FILE_ACCESS_RIGHTS, FILE_ATTRIBUTE_NORMAL, READ_CONTROL,
    FILE_ATTRIBUTE_REPARSE_POINT, FILE_FLAGS_AND_ATTRIBUTES, FILE_FLAG_BACKUP_SEMANTICS,
    FILE_FLAG_OPEN_REPARSE_POINT, FILE_ATTRIBUTE_TAG_INFO, FILE_ID_INFO, FILE_REMOTE_PROTOCOL_INFO,
    FILE_SHARE_MODE, FILE_SHARE_READ, FILE_SHARE_WRITE, FILE_STANDARD_INFO,
    GET_FILEEX_INFO_LEVELS, OPEN_EXISTING, SYNCHRONIZE,
};
use windows_sys::Win32::System::IO::IO_STATUS_BLOCK;
use windows_sys::Win32::System::Ioctl::{FSCTL_GET_REPARSE_POINT, MAXIMUM_REPARSE_DATA_BUFFER_SIZE};
use windows_sys::Win32::System::SystemServices::{
    ACCESS_ALLOWED_ACE_TYPE, FILE_CS_FLAG_CASE_SENSITIVE_DIR, SECURITY_DESCRIPTOR_REVISION,
};
use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

const SHARE: FILE_SHARE_MODE = FILE_SHARE_READ | FILE_SHARE_WRITE;
const COMMON_OPTIONS: NTCREATEFILE_CREATE_OPTIONS =
    FILE_OPEN_REPARSE_POINT | FILE_OPEN_FOR_BACKUP_INTENT | FILE_SYNCHRONOUS_IO_NONALERT;
const DIRECTORY_BUFFER_BYTES: usize = 64 * 1024;
const REPARSE_HEADER_BYTES: usize = 8;
const STATUS_SUCCESS: NTSTATUS = 0;
const BOOL_FALSE: BOOL = 0;
const BOOL_TRUE: BOOL = 1;

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

fn object_attributes(
    parent: HANDLE,
    name: &NtName,
    case: CaseMode,
    security: *const SECURITY_DESCRIPTOR,
) -> OBJECT_ATTRIBUTES {
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
    // Every caller handles its explicitly accepted terminal/warning status before this helper.
    // The generic synchronous completion contract accepts STATUS_SUCCESS only. In particular,
    // STATUS_PENDING preserves its raw NTSTATUS as Os and no output/IOSB byte count is trusted.
    if returned != STATUS_SUCCESS { return Err(nt_error(operation, returned)); }
    let final_status = iosb_status(iosb);
    if final_status != STATUS_SUCCESS { return Err(nt_error(operation, final_status)); }
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
    security: *const SECURITY_DESCRIPTOR,
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

`complete_nt(operation, returned_status, &iosb)` 的规则：

1. `returned_status == STATUS_PENDING` 在同步 handle 上属于 invariant failure；记录 raw status 并返回 `Os`，不得读取未完成 output，不另建 event，也不偷偷异步等待。
2. `returned_status < 0` 先走本稿第 7 节的 raw-NTSTATUS 分类；失败时不信任 `iosb.Information` 或 output bytes。
3. 通用 helper 只接受 `STATUS_SUCCESS`。`STATUS_END_OF_FILE`/`STATUS_NO_MORE_FILES`/`STATUS_BUFFER_OVERFLOW` 只由 read/enumerate 在调用 helper 前按本附录明文处理；任何其他非零 success/informational/warning status 都保留 raw NTSTATUS 返回 `Os`。只有 returned status 是 `STATUS_SUCCESS` 后才读 `iosb.Anonymous.Status`，且 final status 也必须是 `STATUS_SUCCESS`。
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
| `enumerate` | 使用 retained directory handle | N/A | N/A | N/A | N/A | `NtQueryDirectoryFile(FileDirectoryInformation)`；每个 name 再经 component validator + relative nofollow metadata query，不信 dirent attributes；包括 reparse name，但不打开/授予它 |
| `create_file_new(OwnerOnly)` | `FILE_READ_DATA | FILE_WRITE_DATA | FILE_READ_ATTRIBUTES | READ_CONTROL | DELETE | SYNCHRONIZE` | R\|W，无 DELETE | `FILE_CREATE` | common + NON_DIRECTORY | parent mode | `READ_CONTROL` 供 owner/DACL post-verify；DELETE 仅供失败 rollback；protected file DACL；Tag/Id/Standard；verify |
| `create_dir_new(MutateChildren, OwnerOnly)` | `FILE_LIST_DIRECTORY | FILE_TRAVERSE | FILE_READ_ATTRIBUTES | READ_CONTROL | FILE_ADD_FILE | FILE_ADD_SUBDIRECTORY | FILE_DELETE_CHILD | DELETE | SYNCHRONIZE` | R\|W，无 DELETE | `FILE_CREATE` | common + DIRECTORY | parent mode | `READ_CONTROL` 供 owner/DACL post-verify；DELETE 仅供失败 rollback；protected inheritable dir DACL；Tag/Id/Standard/case/volume；verify |
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
    desired: FILE_READ_DATA | FILE_WRITE_DATA | FILE_READ_ATTRIBUTES |
        READ_CONTROL | DELETE | SYNCHRONIZE,
    disposition: FILE_CREATE,
    options: COMMON_OPTIONS | FILE_NON_DIRECTORY_FILE,
    attributes: FILE_ATTRIBUTE_NORMAL,
    delete_right: true,
};
const CREATE_DIR_CONTRACT: OpenContract = OpenContract {
    desired: FILE_LIST_DIRECTORY | FILE_TRAVERSE | FILE_READ_ATTRIBUTES |
        READ_CONTROL | FILE_ADD_FILE | FILE_ADD_SUBDIRECTORY |
        FILE_DELETE_CHILD | DELETE | SYNCHRONIZE,
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
const CLEANUP_REPARSE_CONTRACT: OpenContract = OpenContract {
    desired: FILE_READ_ATTRIBUTES | DELETE | SYNCHRONIZE,
    disposition: FILE_OPEN,
    // No FILE_DIRECTORY_FILE/NON_DIRECTORY_FILE: open the reparse entry itself, never its target.
    options: COMMON_OPTIONS,
    attributes: 0,
    delete_right: true,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum OpenOperation { Query, DirRead, DirMutate, FileRead, FileWrite, CreateFile, CreateDir, CreateStage, CleanupFile, CleanupDir, CleanupReparse }

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
        OpenOperation::CleanupReparse => CLEANUP_REPARSE_CONTRACT,
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

1. `remaining >= NAME_OFFSET`；只按各字段的 `offset_of!` 位置逐字段读取 `NextEntryOffset` 与 `FileNameLength`，不读取或构造含 padded variable tail 的整个 `FILE_DIRECTORY_INFORMATION`。
2. `FileNameLength` 必须为偶数且非零；`NAME_OFFSET + FileNameLength <= remaining`，所有加法 checked。
3. UTF-16 slice 长度为 `FileNameLength / 2`，用 `OsStringExt::from_wide` 保留 unpaired surrogate，再交 `ComponentName`。`.`/`..` 只允许作为被显式跳过的 directory control record。
4. `NextEntryOffset == 0` 表示本批最后一项；该 record 的 name end 仍须在 `used` 内。
5. 非零 `NextEntryOffset` 必须 `>= NAME_OFFSET + FileNameLength`、是 8 的倍数、`cursor + offset <= used` 且严格前进。
6. 整批最多 `used / NAME_OFFSET + 1` 次迭代；超限即 malformed，防止 offset loop。
7. 收集的每个 name 必须再调用 retained parent 的 `query_child_nofollow`；查询失败或消失使 enumeration fail closed。regular file、directory、symlink/reparse 的 name 都输出；这只是 validated component list，不输出 authority、不跟随 link、不授予 open/delete 权限。common `cleanup_quarantined_tree` 随后用 `open_cleanup_child_nofollow` 为每个 name 建立类型化 retained capability。

native parser tests 直接喂：单项、多项、unpaired UTF-16、odd byte length、name overrun、zero-progress、misaligned next、next beyond used、truncated header、warning-with-zero-bytes、valid last record with trailing capacity。

实现 body 固定为下列纯 parser；native syscall loop 只能把 `iosb.Information` 验证后的 slice 交给它：

```rust
fn parse_directory_batch(bytes: &[u8]) -> Result<Vec<ComponentName>> {
    const NAME_OFFSET: usize = offset_of!(FILE_DIRECTORY_INFORMATION, FileName);
    const NEXT_OFFSET: usize = offset_of!(FILE_DIRECTORY_INFORMATION, NextEntryOffset);
    const NAME_LENGTH_OFFSET: usize = offset_of!(FILE_DIRECTORY_INFORMATION, FileNameLength);
    const U32_BYTES: usize = size_of::<u32>();
    fn field_u32(bytes: &[u8], record: usize, field: usize) -> Option<u32> {
        let start = record.checked_add(field)?;
        let end = start.checked_add(U32_BYTES)?;
        Some(u32::from_le_bytes(bytes.get(start..end)?.try_into().ok()?))
    }
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
        if iterations > maximum || cursor > bytes.len() || bytes.len() - cursor < NAME_OFFSET {
            return Err(SafeFsError::InvalidNativeBuffer {
                operation: SafeFsOperation::ParseDirectoryBuffer,
                reason: NativeBufferReason::DirectoryBufferMalformed,
            });
        }
        let next_raw = field_u32(bytes, cursor, NEXT_OFFSET).ok_or(SafeFsError::InvalidNativeBuffer {
            operation: SafeFsOperation::ParseDirectoryBuffer,
            reason: NativeBufferReason::DirectoryBufferMalformed,
        })?;
        let name_raw = field_u32(bytes, cursor, NAME_LENGTH_OFFSET).ok_or(SafeFsError::InvalidNativeBuffer {
            operation: SafeFsOperation::ParseDirectoryBuffer,
            reason: NativeBufferReason::DirectoryBufferMalformed,
        })?;
        let name_bytes = usize::try_from(name_raw).map_err(|_| SafeFsError::InvalidNativeBuffer {
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

        let next = usize::try_from(next_raw).map_err(|_| SafeFsError::InvalidNativeBuffer {
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

fn validated_directory_used(status: NTSTATUS, iosb: &IO_STATUS_BLOCK) -> Result<usize> {
    let used = checked_information(SafeFsOperation::EnumerateDirectory, iosb, DIRECTORY_BUFFER_BYTES)?;
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
    Ok(used)
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
        let used = validated_directory_used(status, &iosb)?;
        for name in parse_directory_batch(&buffer.0[..used])? {
            match query_child_nofollow(directory, &name)? {
                ChildState::Present(_) => output.push(name),
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
            SafeFsOperation::CreateDirectory | SafeFsOperation::CreateStageDirectory |
            SafeFsOperation::CreateFile |
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
        STATUS_INVALID_PARAMETER | STATUS_INFO_LENGTH_MISMATCH | STATUS_BUFFER_TOO_SMALL =>
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

#[allow(dead_code)] // Task 6B parent symbol; Task 7B removes this when public rename calls it.
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
    _acl: Vec<usize>,
    descriptor: Box<SECURITY_DESCRIPTOR>,
    ace_flags: ACE_FLAGS,
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
            unsafe { SetSecurityDescriptorDacl((&mut *descriptor as *mut SECURITY_DESCRIPTOR).cast(), BOOL_TRUE,
                acl.as_mut_ptr().cast(), BOOL_FALSE) } == 0 ||
            unsafe { SetSecurityDescriptorControl((&mut *descriptor as *mut SECURITY_DESCRIPTOR).cast(),
                SE_DACL_PROTECTED, SE_DACL_PROTECTED) } == 0 { return Err(last_win32(op)); }
        Ok(Self { sid, _acl: acl, descriptor, ace_flags })
    }
    fn descriptor_ptr(&self) -> *const SECURITY_DESCRIPTOR {
        &*self.descriptor as *const SECURITY_DESCRIPTOR
    }
}

fn malformed_security() -> SafeFsError {
    SafeFsError::InvalidNativeBuffer {
        operation: SafeFsOperation::VerifySecurityDescriptor,
        reason: NativeBufferReason::SecurityDescriptorMalformed,
    }
}

fn checked_subslice(base: usize, length: usize, pointer: usize, needed: usize) -> Result<std::ops::Range<usize>> {
    let end = base.checked_add(length).ok_or_else(malformed_security)?;
    let pointer_end = pointer.checked_add(needed).ok_or_else(malformed_security)?;
    if pointer < base || pointer_end > end { return Err(malformed_security()); }
    Ok(pointer - base..pointer_end - base)
}

fn checked_sid_length(buffer: &[u8], sid: *const c_void) -> Result<usize> {
    const SID_PREFIX: usize = 8; // revision + sub-authority count + identifier authority
    let range = checked_subslice(buffer.as_ptr() as usize, buffer.len(), sid as usize, SID_PREFIX)?;
    let count = usize::from(buffer[range.start + 1]);
    let length = SID_PREFIX.checked_add(count.checked_mul(size_of::<u32>()).ok_or_else(malformed_security)?)
        .ok_or_else(malformed_security)?;
    checked_subslice(buffer.as_ptr() as usize, buffer.len(), sid as usize, length)?;
    // SAFETY: the SID prefix and every declared sub-authority are inside `buffer`.
    if unsafe { IsValidSid(sid.cast_mut()) } == 0 { return Err(malformed_security()); }
    // SAFETY: IsValidSid accepted the fully bounded SID.
    if usize::try_from(unsafe { GetLengthSid(sid.cast_mut()) }).map_err(|_| malformed_security())? != length {
        return Err(malformed_security());
    }
    Ok(length)
}

fn verify_single_owner_ace(
    descriptor_bytes: &[u8],
    dacl: *mut ACL,
    acl_bytes_in_use: usize,
    ace: *mut c_void,
    expected: &OwnerOnlySecurity,
) -> Result<()> {
    let descriptor_base = descriptor_bytes.as_ptr() as usize;
    let dacl_start = dacl as usize;
    let dacl_range = checked_subslice(descriptor_base, descriptor_bytes.len(), dacl_start,
        acl_bytes_in_use.max(size_of::<ACL>()))?;
    if acl_bytes_in_use < size_of::<ACL>() || dacl_range.len() != acl_bytes_in_use {
        return Err(malformed_security());
    }
    let ace_start = ace as usize;
    checked_subslice(dacl_start, acl_bytes_in_use, ace_start, size_of::<ACE_HEADER>())?;
    // SAFETY: only the fixed ACE_HEADER bytes were bounds-checked; use unaligned read before
    // assuming an ACCESS_ALLOWED_ACE layout.
    let header = unsafe { std::ptr::read_unaligned(ace.cast::<ACE_HEADER>()) };
    if u32::from(header.AceType) != ACCESS_ALLOWED_ACE_TYPE { return Err(malformed_security()); }
    let ace_size = usize::from(header.AceSize);
    let sid_offset = offset_of!(ACCESS_ALLOWED_ACE, SidStart);
    let minimum = sid_offset.checked_add(8).ok_or_else(malformed_security)?;
    if ace_size < minimum { return Err(malformed_security()); }
    checked_subslice(dacl_start, acl_bytes_in_use, ace_start, ace_size)?;
    let sid_ptr = ace_start.checked_add(sid_offset).ok_or_else(malformed_security)? as *const c_void;
    let sid_length = checked_sid_length(descriptor_bytes, sid_ptr)?;
    if sid_offset.checked_add(sid_length).ok_or_else(malformed_security)? != ace_size {
        return Err(malformed_security());
    }
    // SAFETY: type, complete fixed ACCESS_ALLOWED_ACE prefix, AceSize, ACL bounds, SID range,
    // and IsValidSid were all established above.
    let allowed = unsafe { std::ptr::read_unaligned(ace.cast::<ACCESS_ALLOWED_ACE>()) };
    let expected_header_flags = u8::try_from(expected.ace_flags).map_err(|_| malformed_security())?;
    if allowed.Header.AceFlags != expected_header_flags || allowed.Mask != FILE_ALL_ACCESS ||
        unsafe { EqualSid(sid_ptr.cast_mut(), expected.sid.as_ptr().cast()) } == 0
    {
        return Err(malformed_security());
    }
    Ok(())
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
    let descriptor_bytes = unsafe {
        // SAFETY: the second successful query initialized exactly `needed` bytes in aligned storage.
        std::slice::from_raw_parts_mut(words.as_mut_ptr().cast::<u8>(), needed as usize)
    };
    if descriptor_bytes.len() < size_of::<SECURITY_DESCRIPTOR>() { return Err(malformed_security()); }
    let descriptor = descriptor_bytes.as_mut_ptr().cast::<SECURITY_DESCRIPTOR>();
    let mut control = 0u16; let mut revision = 0u32;
    let mut owner = null_mut(); let mut owner_defaulted: BOOL = BOOL_FALSE;
    let mut dacl = null_mut(); let mut present: BOOL = BOOL_FALSE;
    let mut defaulted: BOOL = BOOL_FALSE;
    // SAFETY: kernel returned a self-relative security descriptor in words.
    if unsafe { GetSecurityDescriptorControl(descriptor.cast(), &mut control, &mut revision) } == 0 ||
        unsafe { GetSecurityDescriptorOwner(descriptor.cast(), &mut owner, &mut owner_defaulted) } == 0 ||
        unsafe { GetSecurityDescriptorDacl(descriptor.cast(), &mut present, &mut dacl, &mut defaulted) } == 0 ||
        control & SE_DACL_PROTECTED == 0 || owner_defaulted != BOOL_FALSE ||
        present == BOOL_FALSE || defaulted != BOOL_FALSE || dacl.is_null() || owner.is_null() {
        return Err(malformed_security());
    }
    #[cfg(test)]
    let descriptor_fixture = take_owner_descriptor_fixture();
    #[cfg(test)]
    if descriptor_fixture == Some(OwnerDescriptorFixture::NullOwner) { owner = null_mut(); }
    #[cfg(test)]
    if descriptor_fixture == Some(OwnerDescriptorFixture::InvalidOwner) {
        owner = descriptor_bytes.as_mut_ptr().wrapping_add(descriptor_bytes.len() - 1).cast();
    }
    if owner.is_null() { return Err(malformed_security()); }
    checked_sid_length(descriptor_bytes, owner.cast_const())?;
    // SAFETY: owner SID has been range-checked inside the returned descriptor and IsValidSid passed.
    if unsafe { EqualSid(owner, expected.sid.as_ptr().cast()) } == 0 { return Err(malformed_security()); }
    let mut acl_info = ACL_SIZE_INFORMATION::default();
    #[cfg(test)]
    if descriptor_fixture == Some(OwnerDescriptorFixture::DaclOutOfRange) {
        dacl = descriptor_bytes.as_mut_ptr().wrapping_add(descriptor_bytes.len() + 1).cast();
    }
    checked_subslice(descriptor_bytes.as_ptr() as usize, descriptor_bytes.len(), dacl as usize, size_of::<ACL>())?;
    // SAFETY: DACL fixed header is inside descriptor bytes; output fixed and writable.
    if unsafe { GetAclInformation(dacl, (&mut acl_info as *mut ACL_SIZE_INFORMATION).cast(),
        size_of::<ACL_SIZE_INFORMATION>() as u32, AclSizeInformation) } == 0 {
        return Err(malformed_security());
    }
    #[cfg(test)]
    if descriptor_fixture == Some(OwnerDescriptorFixture::WrongAceCount) { acl_info.AceCount = 2; }
    if acl_info.AceCount != 1 { return Err(malformed_security()); }
    #[cfg(test)]
    if descriptor_fixture == Some(OwnerDescriptorFixture::AclBytesOutOfRange) {
        acl_info.AclBytesInUse = u32::MAX;
    }
    let acl_bytes_in_use = usize::try_from(acl_info.AclBytesInUse).map_err(|_| malformed_security())?;
    checked_subslice(descriptor_bytes.as_ptr() as usize, descriptor_bytes.len(), dacl as usize,
        acl_bytes_in_use.max(size_of::<ACL>()))?;
    let mut ace = null_mut();
    // SAFETY: ACL header and AclBytesInUse are bounded inside descriptor storage.
    if unsafe { GetAce(dacl, 0, &mut ace) } == 0 || ace.is_null() { return Err(last_win32(op)); }
    #[cfg(test)]
    if descriptor_fixture == Some(OwnerDescriptorFixture::AceOutOfRange) {
        ace = descriptor_bytes.as_mut_ptr().wrapping_add(descriptor_bytes.len() + 1).cast();
    }
    #[cfg(test)]
    if let Some(fixture) = descriptor_fixture {
        // SAFETY: GetAce returned a pointer inside the already bounded one-entry ACL. Each
        // mutation stays inside that allocation and is consumed immediately by the release
        // bounds-first verifier; it never reaches a kernel call.
        unsafe {
            let header = ace.cast::<ACE_HEADER>();
            match fixture {
                OwnerDescriptorFixture::WrongAceType => (*header).AceType = 0x7f,
                OwnerDescriptorFixture::UndersizedAce => (*header).AceSize = size_of::<ACE_HEADER>() as u16,
                OwnerDescriptorFixture::OversizedSid => {
                    let sid = (ace as *mut u8).add(offset_of!(ACCESS_ALLOWED_ACE, SidStart));
                    *sid.add(1) = u8::MAX;
                }
                OwnerDescriptorFixture::InvalidSid => {
                    let sid = (ace as *mut u8).add(offset_of!(ACCESS_ALLOWED_ACE, SidStart));
                    *sid = 0;
                }
                OwnerDescriptorFixture::NullOwner | OwnerDescriptorFixture::InvalidOwner |
                OwnerDescriptorFixture::DaclOutOfRange | OwnerDescriptorFixture::AclBytesOutOfRange |
                OwnerDescriptorFixture::WrongAceCount | OwnerDescriptorFixture::AceOutOfRange => {}
            }
        }
    }
    verify_single_owner_ace(descriptor_bytes, dacl, acl_bytes_in_use, ace, expected)
}
```

## 9. Handle-relative delete

结构固定为 WDK `FILE_DISPOSITION_INFORMATION { DeleteFile: true }`，information class 固定 `FileDispositionInformation`（13）：

```rust
fn duplicate_directory(source: &DirectoryAuthority) -> Result<DirectoryAuthority> {
    let mut duplicated = null_mut();
    // SAFETY: source HANDLE retained; current-process source/target; output writable; same access only.
    if unsafe { DuplicateHandle(GetCurrentProcess(), source.native.node.handle.raw(), GetCurrentProcess(),
        &mut duplicated, 0, BOOL_FALSE, DUPLICATE_SAME_ACCESS) } == 0 { return Err(last_win32(SafeFsOperation::OpenDirectory)); }
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
    let operation = match metadata.kind {
        EntryKind::Directory => OpenOperation::CleanupDir,
        EntryKind::SymlinkOrReparse => OpenOperation::CleanupReparse,
        _ => OpenOperation::CleanupFile,
    };
    let contract = contract_for_operation(operation);
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
    if opened.kind != metadata.kind {
        return Err(SafeFsError::UnsupportedEntryType {
            operation: SafeFsOperation::OpenCleanupEntry,
            kind: opened.kind,
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
        Ok(CleanupCapability::Directory(Box::new(QuarantinedCapability {
            parent: duplicated_parent,
            directory,
            original_name: name.clone(),
            quarantine_name: name.clone(),
            opened,
        })))
    } else {
        Ok(CleanupCapability::Entry(Box::new(CleanupEntry {
            parent: duplicate_directory(parent)?,
            native: NativeFile { handle, opened: opened.clone(), access: FileAccess::Read, delete_right: true },
            name: name.clone(),
            opened,
            access: CleanupAccess::Delete,
        })))
    }
}

// Installed by Task 6B because every FILE_CREATE contract already requests DELETE and every
// post-NtCreateFile failure must roll back before a capability can be returned. Tasks 7A/7C
// reuse this exact body; neither redefines it.
fn mark_delete_handle(handle: HANDLE, operation: SafeFsOperation) -> Result<()> {
    #[cfg(test)]
    if FAIL_NEXT_CREATED_DISPOSITION.swap(false, std::sync::atomic::Ordering::SeqCst) {
        return Err(SafeFsError::io(operation,
            io::Error::new(io::ErrorKind::Other, "injected created disposition failure")));
    }
    let info = FILE_DISPOSITION_INFORMATION { DeleteFile: true };
    let mut iosb = IO_STATUS_BLOCK::default();
    // SAFETY: caller owns a live DELETE handle; initialized fixed info and writable iosb stay live.
    let status = unsafe { NtSetInformationFile(handle, &mut iosb,
        (&info as *const FILE_DISPOSITION_INFORMATION).cast(),
        u32::try_from(size_of::<FILE_DISPOSITION_INFORMATION>()).expect("disposition info fits"),
        FileDispositionInformation) };
    complete_nt(operation, status, &iosb)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum WindowsCreateFailurePoint {
    Metadata,
    FilesystemProbe,
    TypeValidation,
    CaseProof,
    SnapshotAssembly,
    ParentDuplicate,
    #[allow(dead_code)] // Task 7A test-only removes this attribute when it constructs the variant.
    SecurityVerification,
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
        *WINDOWS_CREATE_FAILURE.get_or_init(Default::default).lock()
            .expect("Windows create-failure mutex poisoned") = None;
    }
}

#[cfg(test)]
fn install_windows_create_failure(point: WindowsCreateFailurePoint) -> WindowsCreateFailureGuard {
    let mut slot = WINDOWS_CREATE_FAILURE.get_or_init(Default::default).lock()
        .expect("Windows create-failure mutex poisoned");
    assert!(slot.is_none(), "Windows create-failure tests require --test-threads=1");
    *slot = Some(point);
    WindowsCreateFailureGuard
}

fn inject_windows_create_failure(point: WindowsCreateFailurePoint, operation: SafeFsOperation) -> Result<()> {
    #[cfg(test)]
    {
        let mut slot = WINDOWS_CREATE_FAILURE.get_or_init(Default::default).lock()
            .expect("Windows create-failure mutex poisoned");
        if *slot == Some(point) {
            *slot = None;
            return Err(SafeFsError::io(operation,
                io::Error::new(io::ErrorKind::Other, format!("injected Windows {point:?} failure"))));
        }
    }
    let _ = (point, operation);
    Ok(())
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
    assert!(!FAIL_NEXT_CREATED_DISPOSITION.swap(true, std::sync::atomic::Ordering::SeqCst),
        "created disposition tests require --test-threads=1");
    CreatedDispositionFailureGuard
}

fn rollback_created_handle<T>(handle: OwnedHandle, original: SafeFsError) -> Result<T> {
    let rollback = mark_delete_handle(handle.raw(), SafeFsOperation::RollbackCreatedEntry);
    drop(handle);
    match rollback {
        Ok(()) => Err(original),
        Err(_) => Err(SafeFsError::StageIdentityLost {
            operation: SafeFsOperation::RollbackCreatedEntry,
            reason: StageIdentityLostReason::CreatedRollbackDeleteFailed,
        }),
    }
}

fn rollback_created_directory<T>(directory: DirectoryAuthority, original: SafeFsError) -> Result<T> {
    let node = Arc::try_unwrap(directory.native.node).map_err(|_| SafeFsError::StageIdentityLost {
        operation: SafeFsOperation::RollbackCreatedEntry,
        reason: StageIdentityLostReason::CreatedRollbackDeleteFailed,
    })?;
    let rollback = mark_delete_handle(node.handle.raw(), SafeFsOperation::RollbackCreatedEntry);
    drop(node.handle);
    match rollback {
        Ok(()) => Err(original),
        Err(_) => Err(SafeFsError::StageIdentityLost {
            operation: SafeFsOperation::RollbackCreatedEntry,
            reason: StageIdentityLostReason::CreatedRollbackDeleteFailed,
        }),
    }
}

fn dispose_retained(
    mut native: NativeFile,
    parent: &DirectoryAuthority,
    name: &ComponentName,
    expected_kind: EntryKind,
    operation: SafeFsOperation,
) -> Result<()> {
    if !native.delete_right {
        return Err(SafeFsError::Os {
            operation,
            raw: RawOsError::Win32(windows_sys::Win32::Foundation::ERROR_ACCESS_DENIED),
        });
    }
    if native.opened.kind != expected_kind {
        return Err(SafeFsError::UnsupportedEntryType { operation, kind: native.opened.kind });
    }
    run_before_retained_delete_hook(native.handle.raw(), parent, name)?;
    mark_delete_handle(native.handle.raw(), operation)?;
    native.delete_right = false;
    drop(native);
    Ok(())
}

pub(super) fn delete_quarantined_entry(cleanup: CleanupCapability) -> Result<()> {
    match cleanup {
        CleanupCapability::Entry(entry) => {
            let CleanupEntry { parent, native, name, opened, access: CleanupAccess::Delete } = *entry;
            if native.opened.identity != opened.identity {
                return Err(SafeFsError::IdentityChanged {
                    operation: SafeFsOperation::DeleteQuarantinedEntry,
                    expected: opened.identity,
                    actual: native.opened.identity,
                });
            }
            dispose_retained(native, &parent, &name, opened.kind, SafeFsOperation::DeleteQuarantinedEntry)
        }
        CleanupCapability::Directory(_) => Err(SafeFsError::UnsupportedEntryType {
            operation: SafeFsOperation::DeleteQuarantinedEntry,
            kind: EntryKind::Directory,
        }),
    }
}

pub(super) fn delete_quarantined_empty_directory(quarantined: QuarantinedCapability) -> Result<()> {
    let QuarantinedCapability { parent, directory, quarantine_name, opened, .. } = quarantined;
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
    // Directory cleanup follows the same deterministic retained-delete hook policy as leaf cleanup:
    // the hook may rebind the old name, but disposition always consumes this original DELETE handle.
    dispose_retained(native, &parent, &quarantine_name, EntryKind::Directory,
        SafeFsOperation::DeleteQuarantinedEmptyDirectory)
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
#[allow(dead_code)] // Task 6B parent symbol; Task 7B removes this when public rename calls it.
struct RenameInformationBuffer { storage: Vec<usize>, used: u32 }

const _: () = assert!(align_of::<usize>() >= align_of::<FILE_RENAME_INFORMATION>());

#[allow(dead_code)] // Builder is first called by Task 7B test-only bodies.
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
        debug_assert!(storage.len() * size_of::<usize>() >= total);
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
    _opened: &EntryMetadata,
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
    // NtSetInformationFile success is the linearization point. Do not reopen `target` while the
    // consumed source HANDLE still has DELETE and deliberately omits FILE_SHARE_DELETE: that
    // second open would conflict with our own share contract and could turn success into error.
    Ok(())
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
    revalidate_namespace(parent)?;
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
    revalidate_namespace(parent)?;
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

调用 length 精确为 `total`（转 u32 checked），不是 storage capacity；实际 storage 是 `Vec<usize>`，必须以 `const`/compile assertion 与 native test 证明 `align_of::<usize>() >= align_of::<FILE_RENAME_INFORMATION>()`，并证明 `storage.len() * size_of::<usize>() >= total`。其余 assertions：`name_offset == offset_of!(..., FileName)`、`name_offset % align_of::<u16>() == 0`、逐字段 round-trip，UTF-16 length 是 bytes，不含 NUL。target 是 validated single component，不含终止 NUL。

调用：

```text
NtSetInformationFile(
    retained_stage_handle,
    &mut iosb,
    buffer.as_ptr().cast(),
    buffer.used,
    FileRenameInformation,
)
```

source HANDLE 与 parent capability、buffer storage 在 call 完成前都存活。成功的 `NtSetInformationFile` 本身是 no-replace rename 的线性化点；在 retained DELETE HANDLE drop 前禁止按 target name 重开。若需要诊断新名称，只能在同一 HANDLE 上使用 bounds-checked `NtQueryInformationFile(FileNameInformation)`，且诊断失败不得把已完成的 rename 改报失败；C1B 默认不做这个非必要查询。collision tests 覆盖 file、empty dir、non-empty dir、reparse point，目标 identity/bytes/tree 均不变；另有 quarantine 与 publish 成功 native tests 证明返回 `Ok` 和最终 namespace。没有 `MoveFileExW`、`SetFileInformationByHandle(FileRenameInfo)` 或 joined path fallback。

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

fn root_capture_desired(access: DirectoryAccess) -> Result<FILE_ACCESS_RIGHTS> {
    let base = FILE_LIST_DIRECTORY | FILE_TRAVERSE | FILE_READ_ATTRIBUTES | SYNCHRONIZE;
    match access {
        DirectoryAccess::Read => Ok(base),
        DirectoryAccess::MutateChildren => Ok(base | FILE_ADD_FILE | FILE_ADD_SUBDIRECTORY | FILE_DELETE_CHILD),
        DirectoryAccess::Stage => Err(SafeFsError::AccessMismatch {
            operation: SafeFsOperation::CaptureNamespaceRoot,
        }),
    }
}

fn probe_volume(
    path: &Path,
    access: DirectoryAccess,
) -> Result<(OwnedHandle, VolumeProof, LocalFilesystemSnapshot)> {
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
    let desired = root_capture_desired(access)?;
    // SAFETY: mapped root path is nul terminated; security/template null; synchronous directory open.
    let raw = unsafe { CreateFileW(mapping_z.as_ptr(), desired,
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
    let (root_handle, volume, filesystem) = probe_volume(path, access)?;
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
    let validated = (|| -> Result<(EntryMetadata, CaseMode, NamespaceSnapshot)> {
        inject_windows_create_failure(WindowsCreateFailurePoint::FilesystemProbe, operation)?;
        let filesystem = parent.opened.filesystem.as_ref().ok_or(SafeFsError::UnsupportedSecureFilesystem {
            operation: SafeFsOperation::ProbeFilesystem,
            reason: SecureFilesystemReason::FilesystemProbeUnavailable,
        })?;
        inject_windows_create_failure(WindowsCreateFailurePoint::Metadata, operation)?;
        let opened = query_entry_metadata(handle.raw(), filesystem, operation)?;
        inject_windows_create_failure(WindowsCreateFailurePoint::TypeValidation, operation)?;
        if opened.kind != EntryKind::Directory {
            return Err(SafeFsError::UnsupportedEntryType { operation, kind: opened.kind });
        }
        if let Some(expected) = &security {
            verify_created_owner_only(handle.raw(), expected)?;
        }
        inject_windows_create_failure(WindowsCreateFailurePoint::CaseProof, operation)?;
        let case_mode = query_case_mode(handle.raw())?;
        inject_windows_create_failure(WindowsCreateFailurePoint::SnapshotAssembly, operation)?;
        let snapshot = append_snapshot(&parent.snapshot, name.clone(), &opened, case_mode)?;
        Ok((opened, case_mode, snapshot))
    })();
    let (opened, case_mode, snapshot) = match validated {
        Ok(value) => value,
        Err(error) => return rollback_created_handle(handle, error),
    };
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
    let directory = create_directory_contract(parent, name, permissions, DirectoryAccess::Stage,
        contract_for_operation(OpenOperation::CreateStage))?;
    if let Err(error) = inject_windows_create_failure(WindowsCreateFailurePoint::ParentDuplicate,
        SafeFsOperation::CreateStageDirectory)
    {
        return rollback_created_directory(directory, error);
    }
    let owned_parent = match duplicate_directory(parent) {
        Ok(parent) => parent,
        Err(error) => return rollback_created_directory(directory, error),
    };
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
    let validated = (|| -> Result<EntryMetadata> {
        inject_windows_create_failure(WindowsCreateFailurePoint::FilesystemProbe, SafeFsOperation::CreateFile)?;
        let filesystem = parent.opened.filesystem.as_ref().ok_or(SafeFsError::UnsupportedSecureFilesystem {
            operation: SafeFsOperation::ProbeFilesystem,
            reason: SecureFilesystemReason::FilesystemProbeUnavailable,
        })?;
        inject_windows_create_failure(WindowsCreateFailurePoint::Metadata, SafeFsOperation::CreateFile)?;
        let opened = query_entry_metadata(handle.raw(), filesystem, SafeFsOperation::CreateFile)?;
        inject_windows_create_failure(WindowsCreateFailurePoint::TypeValidation, SafeFsOperation::CreateFile)?;
        if opened.kind != EntryKind::RegularFile {
            return Err(SafeFsError::UnsupportedEntryType {
                operation: SafeFsOperation::CreateFile,
                kind: opened.kind,
            });
        }
        if let Some(expected) = &security {
            verify_created_owner_only(handle.raw(), expected)?;
        }
        Ok(opened)
    })();
    let opened = match validated {
        Ok(value) => value,
        Err(error) => return rollback_created_handle(handle, error),
    };
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

以下代码是最终 `windows.rs` test module 的 body catalog，但不是一次性加入。下表是唯一 add-set ownership：每个 test 只在一个 test-only commit 出现，且其 parent SHA 已定义所有被引用 symbol。任务不得从 catalog 提前复制后续 test。

| owner | exact test-only add set | parent-symbol rule |
|---|---|---|
| Task 6B | `nested_retained_io_roundtrip`; `windows_post_create_metadata_failure_rolls_back_same_handle`; `windows_post_create_filesystem_failure_rolls_back_same_handle`; `windows_post_create_type_failure_rolls_back_same_handle`; `windows_post_create_case_failure_rolls_back_same_handle`; `windows_post_create_snapshot_failure_rolls_back_same_handle`; `windows_post_create_parent_duplicate_failure_rolls_back_same_handle` | The test-only commit adds `TestDir`/`name`/`root`, `assert_file_create_failure_rolls_back`, and compile-only create/disposition failure guards beside the Task 6A refusal scaffold, so all seven bodies compile without Task 6B production. `nested_retained_io_roundtrip` and `windows_post_create_metadata_failure_rolls_back_same_handle` are two separate exact behavioral RED runs, each `running 1 test` and failing at the refusal. GREEN replaces the compile-only seams with the final Task 6B seams; all seven pass, and metadata additionally proves disposition failure returns typed fail-leak with the name retained. |
| Task 7A | `component_utf16_and_rejections`; `unicode_and_object_attribute_lifetimes`; `operation_contract_spy_all_rows`; `volume_root_contract_is_access_dependent`; `synchronous_nt_completion_rejects_pending_buffer_small_and_warnings`; `query_reports_reparse_as_present_and_open_rejects`; `reparse_parser_bounds_every_field`; `directory_parser_bounds_and_requery`; `metadata_types_and_hardlinks`; `ten_thousand_handles_return_to_baseline`; `ancestor_mapping_cannot_rebind`; `every_volume_field_is_bound`; `create_new_preserves_every_existing_kind`; `ntstatus_mapping_is_operation_specific`; `production_capabilities_own_drop_resources`; `owner_only_file_directory_stage_succeed_and_rollback`; `owner_only_dacl_rejects_wrong_ace_type`; `owner_only_dacl_rejects_undersized_ace`; `owner_only_dacl_rejects_oversized_sid`; `owner_only_dacl_rejects_invalid_sid`; `owner_only_dacl_rejects_null_or_invalid_owner`; `windows_post_create_security_failure_rolls_back_same_handle` | Task 6B GREEN parent exposes every parser/contract symbol plus compiling OwnerOnly refusal. Task 7A test-only adds its DACL fixture setter/guards and removes the temporary `SecurityVerification` variant dead-code allowance, so all 22 compile without Task 7A production. The first 15 pure/regression tests pass on the test-only SHA; only `owner_only_file_directory_stage_succeed_and_rollback` is the focused RED. No name references Task 7A production types. |
| Task 7B | `every_revalidation_field_is_bound_before_mutation`; `quarantine_and_publish_refuse_changed_probe_without_mutation`; `quarantine_and_publish_success_do_not_self_conflict`; `rename_never_replaces_any_target_kind`; `create_stage_collision_is_typed_and_preserves_original` | Task 6B parent owns production revalidation/hook plus pure `RenameInformationBuffer` and `map_rename_failure`; Task 7A parent exposes compiling rename refusals. These exact five bodies compile before GREEN while embedding buffer-layout and ambiguous-status assertions. |
| Task 7C | `cleanup_quarantined_tree_deletes_nested_reparse_without_traversal`; `retained_delete_survives_real_name_rebound` | Task 6B parent owns pure `RenameInformationBuffer { used }`; Task 7B parent owns the retained rename execution path. Task 7C test-only adds its own hook type/guard/installer beside the two bodies, while GREEN adds only the runner/call site. Therefore the exact test-only SHA compiles before reaching the cleanup refusal without Task 6B future-symbol dead code. |

Task 6B test-only 加入 `TestDir`/`name`/`root`、普通 helper `assert_file_create_failure_rolls_back`、七个 exact bodies 与 compile-only `#[cfg(test)]` create/disposition-failure seams。helper 与所需 imports 同属 test-only patch，不得延迟到 GREEN。Task 6B GREEN 把 seams替换为 section 9 final body，并加入剩余 catalog helpers与 production revalidation hook symbols；Owner/DACL 与 retained-delete future seams分别延迟到 Task 7A/7C test-only commit，避免 Task 6B `clippy -D warnings` 的 future-symbol dead code。

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
    fn write_u32(bytes: &mut [u8], offset: usize, value: u32) {
        let end = offset.checked_add(size_of::<u32>()).expect("fixture offset overflow");
        bytes.get_mut(offset..end).expect("fixture field in bounds").copy_from_slice(&value.to_le_bytes());
    }
    fn directory_record_units(units: &[u16], next: u32) -> Vec<u8> {
        let name_offset = offset_of!(FILE_DIRECTORY_INFORMATION, FileName);
        let name_bytes = units.len().checked_mul(size_of::<u16>()).expect("fixture length overflow");
        let mut bytes = vec![0u8; name_offset.checked_add(name_bytes).expect("fixture allocation overflow")];
        write_u32(&mut bytes, offset_of!(FILE_DIRECTORY_INFORMATION, NextEntryOffset), next);
        write_u32(&mut bytes, offset_of!(FILE_DIRECTORY_INFORMATION, FileNameLength),
            u32::try_from(name_bytes).expect("fixture name length fits u32"));
        for (index, unit) in units.iter().enumerate() {
            let start = name_offset + index * size_of::<u16>();
            bytes[start..start + 2].copy_from_slice(&unit.to_le_bytes());
        }
        bytes
    }
    fn directory_record(value: &str) -> Vec<u8> {
        directory_record_units(&OsStr::new(value).encode_wide().collect::<Vec<_>>(), 0)
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
        let security = 9usize as *const SECURITY_DESCRIPTOR;
        let attrs = object_attributes(7usize as HANDLE, &nt_name, CaseMode::Insensitive, security);
        assert_eq!(nt_name.unicode.Length, 8);
        assert_eq!(attrs.RootDirectory, 7usize as HANDLE);
        assert_eq!(attrs.Attributes, OBJ_CASE_INSENSITIVE);
        assert_eq!(attrs.SecurityDescriptor, security);
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
            (OpenOperation::CleanupReparse, CLEANUP_REPARSE_CONTRACT),
        ];
        assert_eq!(rows.len(), 11);
        for (operation, expected) in rows {
            let recorded = contract_for_operation(operation);
            assert_eq!(recorded, expected);
            assert_eq!(SHARE & FILE_SHARE_DELETE, 0);
            assert_ne!(recorded.options & FILE_OPEN_REPARSE_POINT, 0);
        }
        assert_eq!(QUERY_CONTRACT.disposition, FILE_OPEN);
        assert_eq!(CREATE_FILE_CONTRACT.disposition, FILE_CREATE);
        assert_eq!(CREATE_DIR_CONTRACT.disposition, FILE_CREATE);
        assert_ne!(CREATE_FILE_CONTRACT.desired & READ_CONTROL, 0);
        assert_ne!(CREATE_DIR_CONTRACT.desired & READ_CONTROL, 0);
        assert_ne!(CREATE_STAGE_CONTRACT.desired & READ_CONTROL, 0);
        assert_ne!(CREATE_STAGE_CONTRACT.desired & DELETE, 0);
        assert_ne!(CLEANUP_FILE_CONTRACT.desired & DELETE, 0);
        assert_ne!(CLEANUP_DIR_CONTRACT.desired & DELETE, 0);
        assert_ne!(CLEANUP_REPARSE_CONTRACT.desired & DELETE, 0);
        assert_eq!(CLEANUP_REPARSE_CONTRACT.options & (FILE_DIRECTORY_FILE | FILE_NON_DIRECTORY_FILE), 0);
    }

    #[test]
    fn volume_root_contract_is_access_dependent() {
        let read = root_capture_desired(DirectoryAccess::Read).unwrap();
        let mutate = root_capture_desired(DirectoryAccess::MutateChildren).unwrap();
        assert_ne!(read & SYNCHRONIZE, 0);
        assert_eq!(read & (FILE_ADD_FILE | FILE_ADD_SUBDIRECTORY | FILE_DELETE_CHILD), 0);
        assert_ne!(mutate & SYNCHRONIZE, 0);
        assert_ne!(mutate & FILE_ADD_FILE, 0);
        assert_ne!(mutate & FILE_ADD_SUBDIRECTORY, 0);
        assert_ne!(mutate & FILE_DELETE_CHILD, 0);
        assert!(matches!(root_capture_desired(DirectoryAccess::Stage),
            Err(SafeFsError::AccessMismatch {
                operation: SafeFsOperation::CaptureNamespaceRoot,
            })));
    }

    #[test]
    fn synchronous_nt_completion_rejects_pending_buffer_small_and_warnings() {
        assert!(matches!(complete_nt(SafeFsOperation::ReadFile, STATUS_PENDING, &test_iosb(0, 0)),
            Err(SafeFsError::Os { raw: RawOsError::NtStatus { status: STATUS_PENDING, .. }, .. })));
        assert!(matches!(nt_error(SafeFsOperation::EnumerateDirectory, STATUS_BUFFER_TOO_SMALL),
            SafeFsError::InvalidNativeBuffer { .. }));
        const UNEXPECTED_INFORMATIONAL_STATUS: NTSTATUS = 1;
        assert!(matches!(complete_nt(SafeFsOperation::ReadFile, UNEXPECTED_INFORMATIONAL_STATUS,
            &test_iosb(STATUS_SUCCESS, 0)),
            Err(SafeFsError::Os { raw: RawOsError::NtStatus { status: UNEXPECTED_INFORMATIONAL_STATUS, .. }, .. })));
        assert!(matches!(complete_nt(SafeFsOperation::ReadFile, STATUS_SUCCESS,
            &test_iosb(UNEXPECTED_INFORMATIONAL_STATUS, 0)),
            Err(SafeFsError::Os { raw: RawOsError::NtStatus { status: UNEXPECTED_INFORMATIONAL_STATUS, .. }, .. })));
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
        const MOUNT: u32 = 0xA000_0003;
        const SYMLINK: u32 = 0xA000_000C;
        let packet = |tag: u32, payload: &[u8]| {
            let mut bytes = Vec::with_capacity(8 + payload.len());
            bytes.extend_from_slice(&tag.to_le_bytes());
            bytes.extend_from_slice(&u16::try_from(payload.len()).unwrap().to_le_bytes());
            bytes.extend_from_slice(&0u16.to_le_bytes());
            bytes.extend_from_slice(payload);
            bytes
        };
        let invalid = [
            vec![], vec![0; 7],
            vec![1, 0, 0, 0, 10, 0, 0, 0],
            { let mut value = vec![0; MAXIMUM_REPARSE_DATA_BUFFER_SIZE as usize + 1]; value[4..6].copy_from_slice(&0u16.to_le_bytes()); value },
            packet(MOUNT, &[0; 7]),
            packet(SYMLINK, &[0; 11]),
            packet(MOUNT, &[1, 0, 2, 0, 0, 0, 0, 0, 0, 0]),
            packet(MOUNT, &[0, 0, 4, 0, 0, 0, 0, 0, 0, 0]),
            packet(SYMLINK, &[0, 0, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0]),
        ];
        for bytes in invalid { assert!(matches!(parse_reparse(&bytes),
            Err(SafeFsError::InvalidNativeBuffer { reason: NativeBufferReason::ReparseBufferMalformed, .. })),
            "accepted malformed reparse packet: {bytes:?}"); }
        let mount = packet(MOUNT, &[0, 0, 0, 0, 0, 0, 0, 0]);
        assert_eq!(parse_reparse(&mount).unwrap(), (MOUNT, mount.clone()));
        for flags in [0u32, 1u32] {
            let mut payload = vec![0; 12];
            payload[8..12].copy_from_slice(&flags.to_le_bytes());
            let link = packet(SYMLINK, &payload);
            assert_eq!(parse_reparse(&link).unwrap(), (SYMLINK, link));
        }
        let unknown = vec![0x34, 0x12, 0, 0, 2, 0, 0, 0, 0xAA, 0xBB];
        assert_eq!(parse_reparse(&unknown).unwrap(), (0x1234, unknown.clone()));
    }

    #[test]
    fn directory_parser_bounds_and_requery() {
        for value in ["a", "ab", "abc"] {
            assert_eq!(parse_directory_batch(&directory_record(value)).unwrap(), vec![name(value)]);
        }
        let first = directory_record_units(&[b'a' as u16], 72);
        let mut multi = first;
        multi.resize(72, 0);
        multi.extend(directory_record_units(&[b'b' as u16, b'c' as u16], 0));
        assert_eq!(parse_directory_batch(&multi).unwrap(), vec![name("a"), name("bc")]);
        let unpaired = directory_record_units(&[0xD800], 0);
        assert_eq!(parse_directory_batch(&unpaired).unwrap()[0].as_os_str().encode_wide().collect::<Vec<_>>(), vec![0xD800]);

        let malformed = |bytes: Vec<u8>| assert!(matches!(parse_directory_batch(&bytes),
            Err(SafeFsError::InvalidNativeBuffer { reason: NativeBufferReason::DirectoryBufferMalformed, .. })));
        malformed(Vec::new());
        malformed(vec![0; offset_of!(FILE_DIRECTORY_INFORMATION, FileName) - 1]);
        let mut odd = directory_record("leaf");
        write_u32(&mut odd, offset_of!(FILE_DIRECTORY_INFORMATION, FileNameLength), 3);
        malformed(odd);
        let mut overrun = directory_record("a");
        write_u32(&mut overrun, offset_of!(FILE_DIRECTORY_INFORMATION, FileNameLength), 4);
        malformed(overrun);
        let mut zero_progress = directory_record("a");
        write_u32(&mut zero_progress, offset_of!(FILE_DIRECTORY_INFORMATION, NextEntryOffset), 1);
        malformed(zero_progress);
        let mut misaligned = directory_record("a");
        write_u32(&mut misaligned, offset_of!(FILE_DIRECTORY_INFORMATION, NextEntryOffset), 70);
        malformed(misaligned);
        let mut beyond = directory_record("a");
        write_u32(&mut beyond, offset_of!(FILE_DIRECTORY_INFORMATION, NextEntryOffset), 72);
        malformed(beyond);
        let mut trailing = directory_record("a");
        trailing.extend_from_slice(&[0xA5; 16]);
        assert_eq!(parse_directory_batch(&trailing).unwrap(), vec![name("a")]);
        assert!(matches!(validated_directory_used(STATUS_BUFFER_OVERFLOW,
            &test_iosb(STATUS_BUFFER_OVERFLOW, 0)),
            Err(SafeFsError::InvalidNativeBuffer { reason: NativeBufferReason::DirectoryBufferTooSmall, .. })));
        assert!(matches!(validated_directory_used(STATUS_SUCCESS, &test_iosb(STATUS_SUCCESS, 0)),
            Err(SafeFsError::InvalidNativeBuffer { reason: NativeBufferReason::DirectoryBufferMalformed, .. })));
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
        let a = create_dir_new(&authority, &name("a"), CreatePermissions::Inherit, DirectoryAccess::MutateChildren).unwrap();
        let b = create_dir_new(&a, &name("b"), CreatePermissions::Inherit, DirectoryAccess::MutateChildren).unwrap();
        let mut file = create_file_new(&b, &name("data"), CreatePermissions::Inherit).unwrap();
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
    fn create_new_preserves_every_existing_kind() {
        let temp = TestDir::new("exclusive"); fs::write(temp.path().join("file"), b"before").unwrap(); fs::create_dir(temp.path().join("dir")).unwrap();
        let authority = root(&temp);
        assert!(matches!(create_file_new(&authority, &name("file"), CreatePermissions::Inherit), Err(SafeFsError::AlreadyExists { .. })));
        assert_eq!(fs::read(temp.path().join("file")).unwrap(), b"before");
        assert!(matches!(create_dir_new(&authority, &name("dir"), CreatePermissions::Inherit, DirectoryAccess::Read), Err(SafeFsError::AlreadyExists { .. })));
    }

    fn assert_file_create_failure_rolls_back(point: WindowsCreateFailurePoint, label: &str) {
        let temp = TestDir::new(label);
        let authority = root(&temp);
        let _failure = install_windows_create_failure(point);
        assert!(create_file_new(&authority, &name("created"), CreatePermissions::Inherit).is_err());
        assert!(matches!(query_child_nofollow(&authority, &name("created")).unwrap(), ChildState::Absent));
    }

    #[test]
    fn windows_post_create_metadata_failure_rolls_back_same_handle() {
        assert_file_create_failure_rolls_back(WindowsCreateFailurePoint::Metadata, "rollback-metadata");

        let temp = TestDir::new("rollback-disposition-failure");
        let authority = root(&temp);
        let _failure = install_windows_create_failure(WindowsCreateFailurePoint::Metadata);
        let _disposition = install_created_disposition_failure();
        let error = match create_file_new(&authority, &name("created"), CreatePermissions::Inherit) {
            Ok(_) => panic!("injected disposition failure must reject the created file"),
            Err(error) => error,
        };
        assert!(matches!(&error,
            SafeFsError::StageIdentityLost {
                operation: SafeFsOperation::RollbackCreatedEntry,
                reason: StageIdentityLostReason::CreatedRollbackDeleteFailed,
            }), "unexpected error: {error:?}");
        assert!(matches!(query_child_nofollow(&authority, &name("created")).unwrap(),
            ChildState::Present(_)), "failed retained-HANDLE disposition must fail-leak the created entry");
    }

    #[test]
    fn windows_post_create_filesystem_failure_rolls_back_same_handle() {
        assert_file_create_failure_rolls_back(WindowsCreateFailurePoint::FilesystemProbe, "rollback-filesystem");
    }

    #[test]
    fn windows_post_create_type_failure_rolls_back_same_handle() {
        assert_file_create_failure_rolls_back(WindowsCreateFailurePoint::TypeValidation, "rollback-type");
    }

    #[test]
    fn windows_post_create_case_failure_rolls_back_same_handle() {
        let temp = TestDir::new("rollback-case");
        let authority = root(&temp);
        let _failure = install_windows_create_failure(WindowsCreateFailurePoint::CaseProof);
        assert!(create_dir_new(&authority, &name("created"), CreatePermissions::Inherit,
            DirectoryAccess::MutateChildren).is_err());
        assert!(matches!(query_child_nofollow(&authority, &name("created")).unwrap(), ChildState::Absent));
    }

    #[test]
    fn windows_post_create_snapshot_failure_rolls_back_same_handle() {
        let temp = TestDir::new("rollback-snapshot");
        let authority = root(&temp);
        let _failure = install_windows_create_failure(WindowsCreateFailurePoint::SnapshotAssembly);
        assert!(create_dir_new(&authority, &name("created"), CreatePermissions::Inherit,
            DirectoryAccess::MutateChildren).is_err());
        assert!(matches!(query_child_nofollow(&authority, &name("created")).unwrap(), ChildState::Absent));
    }

    #[test]
    fn windows_post_create_parent_duplicate_failure_rolls_back_same_handle() {
        let temp = TestDir::new("rollback-duplicate");
        let authority = root(&temp);
        let _failure = install_windows_create_failure(WindowsCreateFailurePoint::ParentDuplicate);
        assert!(create_stage_dir_new(&authority, &name("created"), CreatePermissions::Inherit).is_err());
        assert!(matches!(query_child_nofollow(&authority, &name("created")).unwrap(), ChildState::Absent));
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
        assert!(matches!(nt_error(SafeFsOperation::CreateStageDirectory, STATUS_OBJECT_NAME_COLLISION),
            SafeFsError::AlreadyExists { operation: SafeFsOperation::CreateStageDirectory }));
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

Attempt 6 用以下完整 `.github/workflows/ci.yml`，保留现有 rust/web jobs并加入 exact-SHA native matrix。`workflow_dispatch` 只有该 workflow 已存在 default branch 时才可调用；在它首次进入 default branch 前，中间 SHA 只能通过授权 PR 的 head-SHA path 取得 receipt。

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
      red_task:
        description: Focused Windows expected-RED slice; none runs normal native gates
        required: true
        default: none
        type: choice
        options: [none, 6b, 7a, 7b, 7c]
      red_parent_sha:
        description: Exact parent SHA for a focused Windows expected-RED slice
        required: false
        default: ''
        type: string
      red_nonce:
        description: Unique 16-lower-hex correlation nonce for expected-RED evidence
        required: false
        default: ''
        type: string

permissions:
  contents: read

concurrency:
  group: ${{ github.workflow }}-${{ github.event_name }}-${{ github.ref }}-${{ inputs.commit_sha || github.sha }}-${{ inputs.red_task || 'normal' }}-${{ inputs.red_nonce || 'none' }}
  cancel-in-progress: true

jobs:
  rust:
    name: Rust (fmt / clippy / test)
    if: github.event_name != 'workflow_dispatch' || inputs.red_task == 'none'
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
    if: github.event_name != 'workflow_dispatch' || inputs.red_task == 'none'
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
    if: github.event_name != 'workflow_dispatch' || inputs.red_task == 'none'
    strategy:
      fail-fast: false
      matrix:
        include:
          - receipt_id: linux-x86_64
            runner: ubuntu-24.04
            expected_os: Linux
            expected_arch: X64
          - receipt_id: macos-native
            runner: macos-14
            expected_os: macOS
            expected_arch: ARM64
          - receipt_id: windows-x86_64
            runner: windows-2022
            expected_os: Windows
            expected_arch: X64
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
      - name: Parse Windows expected-RED harness
        if: runner.os == 'Windows'
        shell: pwsh
        run: |
          $tokens = $null
          $errors = $null
          [void][System.Management.Automation.Language.Parser]::ParseFile(
            (Resolve-Path 'scripts/run-c1b-windows-red.ps1'), [ref]$tokens, [ref]$errors)
          if ($errors.Count -ne 0) { throw ($errors | Out-String) }
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
          EXPECTED_RUNNER_OS: ${{ matrix.expected_os }}
          EXPECTED_RUNNER_ARCH: ${{ matrix.expected_arch }}
        run: |
          if ('${{ runner.os }}' -ne $env:EXPECTED_RUNNER_OS) { throw 'runner OS does not match receipt id' }
          if ('${{ runner.arch }}' -ne $env:EXPECTED_RUNNER_ARCH) { throw 'runner architecture does not match receipt id' }
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

  windows-red-evidence:
    name: Windows expected RED (${{ inputs.red_task }})
    if: github.event_name == 'workflow_dispatch' && inputs.red_task != 'none'
    runs-on: windows-2022
    timeout-minutes: 35
    env:
      TARGET_SHA: ${{ inputs.commit_sha }}
      PARENT_SHA: ${{ inputs.red_parent_sha }}
      RED_TASK: ${{ inputs.red_task }}
      RED_NONCE: ${{ inputs.red_nonce }}
    steps:
      - name: Validate immutable RED inputs
        shell: pwsh
        run: |
          if ($env:TARGET_SHA -cnotmatch '^[0-9a-f]{40}$') { throw 'commit_sha must be lower 40-hex' }
          if ($env:PARENT_SHA -cnotmatch '^[0-9a-f]{40}$') { throw 'red_parent_sha must be lower 40-hex' }
          if ($env:RED_NONCE -cnotmatch '^[0-9a-f]{16}$') { throw 'red_nonce must be unique lower 16-hex' }
      - uses: actions/checkout@v4
        with:
          ref: ${{ env.TARGET_SHA }}
          fetch-depth: 2
          persist-credentials: false
      - name: Assert exact RED commit and parent
        id: bind-red
        shell: pwsh
        run: |
          $actual = (git rev-parse HEAD).Trim().ToLowerInvariant()
          $parent = (git rev-parse 'HEAD^').Trim().ToLowerInvariant()
          $commitRow = @((git rev-list --parents -n 1 HEAD).Trim().Split(' '))
          $changedPaths = @(git diff-tree --no-commit-id --name-only -r HEAD)
          if ($actual -cne $env:TARGET_SHA) { throw 'checked-out RED SHA mismatch' }
          if ($parent -cne $env:PARENT_SHA) { throw 'RED parent SHA mismatch' }
          if ($commitRow.Count -ne 2) { throw 'RED commit must have exactly one parent' }
          if ($changedPaths.Count -ne 1 -or $changedPaths[0] -cne 'crates/opentake-project/src/safe_fs/windows.rs') {
            throw 'RED commit changed paths outside windows.rs'
          }
          "sha=$actual" >> $env:GITHUB_OUTPUT
      - name: Run focused expected-RED contract
        shell: pwsh
        run: |
          ./scripts/run-c1b-windows-red.ps1 `
            -Task $env:RED_TASK -TestSha $env:TARGET_SHA -ParentSha $env:PARENT_SHA `
            -Nonce $env:RED_NONCE -EvidenceRoot (Join-Path $env:RUNNER_TEMP 'c1b-red')
      - name: Upload immutable Windows RED receipt
        if: always()
        uses: actions/upload-artifact@v4
        with:
          name: c1b-red-${{ inputs.red_task }}-${{ steps.bind-red.outputs.sha }}-${{ inputs.red_nonce }}
          path: ${{ runner.temp }}/c1b-red/c1b-task-${{ inputs.red_task }}-${{ steps.bind-red.outputs.sha }}-${{ inputs.red_nonce }}/
          if-no-files-found: error
          retention-days: 30
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
expected = {
  "linux-x86_64" => ["ubuntu-24.04", "Linux", "X64"],
  "macos-native" => ["macos-14", "macOS", "ARM64"],
  "windows-x86_64" => ["windows-2022", "Windows", "X64"],
}
rows.each { |r| raise "runner provenance" unless r.values_at("runner", "expected_os", "expected_arch") == expected.fetch(r.fetch("receipt_id")) }
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
gh auth status --hostname github.com
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

### 14.1 Windows expected-RED 的 runner-portable 证据回传

Windows test-only commit 不在 macOS 上直接执行下文 PowerShell 片段，也不向 GitHub runner 传入 `/Users/...`。在已有明确 publication/dispatch 授权、Task 3 workflow/harness 已存在 default branch、且 test-only SHA 已存在 `appergb/OpenTake` 时，唯一路径是 section 13 `windows-red-evidence` job：它在 `$RUNNER_TEMP` exclusive-create 目录，执行 repository-versioned `scripts/run-c1b-windows-red.ps1`，然后上传 nonce/SHA-bound artifact。任一前置不满足则按 section 14 记录 `BLOCKED`，不得在本机伪造 RED receipt。

每个 test-only commit 使用下列固定调度与 intake，`TASK` 只能是 `6b|7a|7b|7c`，`TEST_SHA=$(git rev-parse HEAD)`，`PARENT_SHA=$(git rev-parse HEAD^)`，`PARENT_PROOF` 必须指向紧邻前一个已验证 branch gate（Task 6B 也使用 Task 6A 的三平台 gate）：

```bash
set -euo pipefail
test "$TEST_SHA" != "$PARENT_SHA"
[[ "$TEST_SHA" =~ ^[0-9a-f]{40}$ ]]
[[ "$PARENT_SHA" =~ ^[0-9a-f]{40}$ ]]
case "$TASK" in 6b|7a|7b|7c) ;; *) exit 64 ;; esac
test "$(git rev-parse HEAD)" = "$TEST_SHA"
test "$(git rev-parse HEAD^)" = "$PARENT_SHA"
test "$(git rev-list --parents -n 1 HEAD | wc -w | tr -d ' ')" = 2
test "$(git diff-tree --no-commit-id --name-only -r HEAD)" = \
  'crates/opentake-project/src/safe_fs/windows.rs'
case "$TASK" in
  6b) PARENT_TASK=6a; PROOF_KIND=gate; TEST_SUBJECT='test(project): specify Windows capability acquisition and io'; PARENT_SUBJECT='feat(project): add fail-closed Windows platform scaffold' ;;
  7a) PARENT_TASK=6b; PROOF_KIND=gate; TEST_SUBJECT='test(project): specify Windows owner-only creation and rollback'; PARENT_SUBJECT='feat(project): capture Windows filesystem capabilities' ;;
  7b) PARENT_TASK=7a; PROOF_KIND=gate; TEST_SUBJECT='test(project): specify Windows retained quarantine and publish'; PARENT_SUBJECT='feat(project): enforce Windows owner-only creation' ;;
  7c) PARENT_TASK=7b; PROOF_KIND=gate; TEST_SUBJECT='test(project): specify Windows retained cleanup and delete'; PARENT_SUBJECT='feat(project): add Windows capability-relative rename' ;;
esac
test "$(git show -s --format=%s "$TEST_SHA")" = "$TEST_SUBJECT"
test "$(git show -s --format=%s "$PARENT_SHA")" = "$PARENT_SUBJECT"
ruby -rjson -rpathname -e '
  safety, proof, kind, parent_task, parent_sha = ARGV.map { |value| value }
  safety = Pathname.new(safety).realpath
  proof = Pathname.new(proof).realpath
  raise "Windows RED parent proof escapes safety root" unless proof.to_s.start_with?(safety.to_s + File::SEPARATOR)
  read_regular = lambda do |relative|
    path = proof.join(relative)
    raise "parent proof file is not confined regular data" unless path.lstat.file? &&
      path.realpath.to_s.start_with?(proof.to_s + File::SEPARATOR)
    path.read
  end
  approve = lambda do |relative, role|
    body = read_regular.call(relative)
    raise "parent report identity mismatch" unless body.match?(/^Role:\s*.*#{Regexp.escape(role)}/i) &&
      body.match?(/^Task:\s*#{Regexp.escape(parent_task)}\s*$/i) &&
      body.match?(/^Commit:\s*`?#{parent_sha}`?\s*$/i) && body.match?(/^Verdict:\s*(\*\*)?APPROVE(\*\*)?\s*$/i) &&
      %w[Critical Important Minor].all? { |severity| body.match?(/^#{severity}:\s*(\*\*)?0(\*\*)?\s*$/i) }
  end
  if kind == "review"
    raise "RED parent review directory mismatch" unless
      proof.basename.to_s.match?(/\Ac1b-task-#{Regexp.escape(parent_task)}-#{Regexp.escape(parent_sha)}-attempt-[1-9][0-9]*\z/)
    manifest = JSON.parse(read_regular.call("gate-manifest.json"))
    raise "RED parent manifest mismatch" unless manifest == {
      "schema" => "opentake-c1b-reviewed-stage-v1", "task" => parent_task, "sha" => parent_sha,
      "baseline_sha" => "e67917260ace36e4db1ede4e36eecbc401825bb1" }
    approve.call("spec-security-review.md", "spec-security")
    approve.call("implementation-review.md", "implementation")
  else
    raise "RED parent gate mismatch" unless
      proof.basename.to_s.match?(/\Ac1b-task-#{Regexp.escape(parent_task)}-#{Regexp.escape(parent_sha)}-[0-9a-f]{16}\z/)
    raise "RED parent gate validator failed" unless read_regular.call("results-validation.raw-exit").strip == "0"
    validation_log = read_regular.call("results-validation.log")
    raise "RED parent validator success identity mismatch" unless
      validation_log.match?(/^c1b-evidence-validation=ok task=#{Regexp.escape(parent_task)} predecessor=[0-9a-f]{40} sha=#{parent_sha}$/)
    results = read_regular.call("results.md")
    raise "RED parent results mismatch" unless results.match?(/^Task:\s*#{Regexp.escape(parent_task)}$/) &&
      results.match?(/^Final SHA:\s*#{parent_sha}$/) && results.match?(/^Aggregate:\s*0$/)
    approve.call("reviews/spec-security-review.md", "spec-security")
    approve.call("reviews/implementation-review.md", "implementation")
  end
' "$SAFETY_ROOT" "$PARENT_PROOF" "$PROOF_KIND" "$PARENT_TASK" "$PARENT_SHA"
if test "$PROOF_KIND" = gate; then
  PARENT_REVALIDATION_OUTPUT=$(ruby -rjson -rtmpdir -rfileutils -ropen3 -rrbconfig -e '
    proof, parent_task, parent_sha, repo, validator = ARGV
    binding = JSON.parse(File.read(File.join(proof, "predecessor-binding.json")))
    raise "RED parent binding task mismatch" unless binding.fetch("task") == parent_task
    Dir.mktmpdir("c1b-red-parent-revalidate") do |directory|
      clone = File.join(directory, "repo")
      _out, err, cloned = Open3.capture3("git", "clone", "--quiet", "--shared", "--no-checkout", repo, clone)
      raise "cannot clone RED parent repository: #{err}" unless cloned.success?
      _out, err, checked = Open3.capture3("git", "-C", clone, "checkout", "--detach", parent_sha)
      raise "cannot checkout RED parent SHA: #{err}" unless checked.success?
      stdout, stderr, status = Open3.capture3(
        RbConfig.ruby, validator, proof, parent_task, parent_sha,
        binding.fetch("predecessor_sha"), binding.fetch("predecessor_proof"),
        "reviews/spec-security-review.md", "reviews/implementation-review.md", clone
      )
      raise "live RED parent gate revalidation failed: #{stdout}#{stderr}" unless status.success?
      puts stdout.strip
    end
  ' "$PARENT_PROOF" "$PARENT_TASK" "$PARENT_SHA" "$(pwd -P)" \
    "$(ruby -e 'puts File.realpath("scripts/validate-c1b-evidence.rb")')")
else
  PARENT_REVALIDATION_OUTPUT="review-manifest-validated task=$PARENT_TASK sha=$PARENT_SHA"
fi
NONCE=$(openssl rand -hex 8)
ARTIFACT_NAME="c1b-red-$TASK-$TEST_SHA-$NONCE"
gh workflow run ci.yml --repo appergb/OpenTake --ref main \
  -f commit_sha="$TEST_SHA" -f red_task="$TASK" \
  -f red_parent_sha="$PARENT_SHA" -f red_nonce="$NONCE"

INTAKE=$(mktemp -d "$SAFETY_ROOT/red-intake-$TASK-$TEST_SHA-$NONCE.XXXXXX")
trap 'rm -rf "$INTAKE"' EXIT
GH_API_VERSION=2026-03-10
FOUND=false
for _ in $(seq 1 90); do
  gh api --hostname github.com -H "X-GitHub-Api-Version: $GH_API_VERSION" \
    "/repos/appergb/OpenTake/actions/artifacts?name=$ARTIFACT_NAME&per_page=100" \
    >"$INTAKE/artifacts.json"
  if ruby -rjson -e '
    doc, name = JSON.parse(File.read(ARGV[0])), ARGV[1]
    rows = doc.fetch("artifacts")
    exit 75 unless doc.fetch("total_count") == 1 && rows.length == 1
    row = rows.fetch(0)
    exit 75 unless row.fetch("name") == name && !row.fetch("expired")
  ' "$INTAKE/artifacts.json" "$ARTIFACT_NAME"; then FOUND=true; break; fi
  sleep 10
done
test "$FOUND" = true
ruby -rjson -e '
  row = JSON.parse(File.read(ARGV[0])).fetch("artifacts").fetch(0)
  File.write(ARGV[1], JSON.pretty_generate(row) + "\n")
' "$INTAKE/artifacts.json" "$INTAKE/artifact.json"
ARTIFACT_ID=$(ruby -rjson -e 'puts JSON.parse(File.read(ARGV[0])).fetch("id")' "$INTAKE/artifact.json")
RUN_ID=$(ruby -rjson -e 'puts JSON.parse(File.read(ARGV[0])).fetch("workflow_run").fetch("id")' "$INTAKE/artifact.json")
RUN_COMPLETE=false
for _ in $(seq 1 90); do
  gh api --hostname github.com -H "X-GitHub-Api-Version: $GH_API_VERSION" \
    "/repos/appergb/OpenTake/actions/runs/$RUN_ID" >"$INTAKE/run.json"
  STATUS=$(ruby -rjson -e 'puts JSON.parse(File.read(ARGV[0])).fetch("status")' "$INTAKE/run.json")
  if test "$STATUS" = completed; then RUN_COMPLETE=true; break; fi
  sleep 10
done
test "$RUN_COMPLETE" = true
RUN_HEAD_SHA=$(ruby -rjson -e 'puts JSON.parse(File.read(ARGV[0])).fetch("head_sha")' "$INTAKE/run.json")
[[ "$RUN_HEAD_SHA" =~ ^[0-9a-f]{40}$ ]]
gh api --hostname github.com -H "X-GitHub-Api-Version: $GH_API_VERSION" \
  "/repos/appergb/OpenTake/contents/.github/workflows/ci.yml?ref=$RUN_HEAD_SHA" \
  >"$INTAKE/workflow-content.json"
gh api --hostname github.com -H "X-GitHub-Api-Version: $GH_API_VERSION" \
  "/repos/appergb/OpenTake/contents/scripts/run-c1b-windows-red.ps1?ref=$RUN_HEAD_SHA" \
  >"$INTAKE/harness-content.json"
git show "$TEST_SHA:.github/workflows/ci.yml" >"$INTAKE/expected-workflow.yml"
git show "$TEST_SHA:scripts/run-c1b-windows-red.ps1" >"$INTAKE/expected-harness.ps1"
ruby -rjson -rbase64 -e '
  workflow, expected_workflow, harness, expected_harness =
    JSON.parse(File.read(ARGV[0])), File.binread(ARGV[1]), JSON.parse(File.read(ARGV[2])), File.binread(ARGV[3])
  raise "contents API encoding mismatch" unless
    workflow.fetch("encoding") == "base64" && harness.fetch("encoding") == "base64"
  raise "Windows RED workflow bytes differ from reviewed test-SHA workflow" unless
    Base64.decode64(workflow.fetch("content")) == expected_workflow
  raise "Windows RED harness bytes differ from reviewed main workflow version" unless
    Base64.decode64(harness.fetch("content")) == expected_harness
' "$INTAKE/workflow-content.json" "$INTAKE/expected-workflow.yml" \
  "$INTAKE/harness-content.json" "$INTAKE/expected-harness.ps1"
gh api --hostname github.com -H "X-GitHub-Api-Version: $GH_API_VERSION" \
  "/repos/appergb/OpenTake/actions/runs/$RUN_ID/jobs?per_page=100" >"$INTAKE/jobs.json"
gh api --hostname github.com -H "X-GitHub-Api-Version: $GH_API_VERSION" \
  "/repos/appergb/OpenTake/actions/artifacts/$ARTIFACT_ID/zip" >"$INTAKE/artifact.zip"
test -s "$INTAKE/artifact.zip"
ruby -rjson -rdigest -e '
  expected = JSON.parse(File.read(ARGV[0])).fetch("digest")
  actual = "sha256:#{Digest::SHA256.file(ARGV[1]).hexdigest}"
  raise "Windows RED artifact digest mismatch" unless expected == actual
' "$INTAKE/artifact.json" "$INTAKE/artifact.zip"
unzip -Z1 "$INTAKE/artifact.zip" | ruby -e '
  task = ARGV.fetch(0)
  red_logs = {
    "6b" => %w[windows-io-red.log windows-create-rollback-red.log],
    "7a" => %w[windows-owner-only-red.log],
    "7b" => %w[windows-rename-red.log],
    "7c" => %w[windows-cleanup-red.log],
  }.fetch(task)
  parent_logs = task == "7a" ? %w[
    component_utf16_and_rejections.pass.log unicode_and_object_attribute_lifetimes.pass.log
    operation_contract_spy_all_rows.pass.log volume_root_contract_is_access_dependent.pass.log
    synchronous_nt_completion_rejects_pending_buffer_small_and_warnings.pass.log
    query_reports_reparse_as_present_and_open_rejects.pass.log reparse_parser_bounds_every_field.pass.log
    directory_parser_bounds_and_requery.pass.log metadata_types_and_hardlinks.pass.log
    ten_thousand_handles_return_to_baseline.pass.log ancestor_mapping_cannot_rebind.pass.log
    every_volume_field_is_bound.pass.log create_new_preserves_every_existing_kind.pass.log
    ntstatus_mapping_is_operation_specific.pass.log production_capabilities_own_drop_resources.pass.log
  ] : []
  allowed = ["red-receipt.json"] + red_logs + parent_logs
  names = STDIN.each_line.map(&:strip)
  raise "Windows RED archive entry set mismatch" unless names.length == names.uniq.length && names.sort == allowed.sort
  raise "unsafe RED archive" if names.any? { |name| name.start_with?("/") || name.include?("/") || name.include?("\\") }
' "$TASK"
unzip -q "$INTAKE/artifact.zip" -d "$INTAKE/unpacked"
ruby -rpathname -e '
  root = Pathname.new(ARGV.fetch(0)).realpath
  entries = Dir.children(root)
  raise "RED archive entry set changed after extraction" if entries.empty?
  entries.each do |name|
    path = root.join(name)
    raise "RED archive extracted non-regular file" unless path.lstat.file? && path.realpath.dirname == root
  end
' "$INTAKE/unpacked"
ruby -rjson -rtime -e '
  receipt, run, jobs, artifact, unpacked, task, sha, parent, nonce =
    JSON.parse(File.read(ARGV[0])), JSON.parse(File.read(ARGV[1])), JSON.parse(File.read(ARGV[2])),
    JSON.parse(File.read(ARGV[3])), ARGV[4], *ARGV[5..]
  expected_red = {
    "6b" => [
      ["safe_fs::windows::tests::nested_retained_io_roundtrip", "windows-io-red.log",
        "UnsupportedSecureFilesystem|UnsupportedTarget"],
      ["safe_fs::windows::tests::windows_post_create_metadata_failure_rolls_back_same_handle",
        "windows-create-rollback-red.log", "UnsupportedSecureFilesystem|UnsupportedTarget"],
    ],
    "7a" => [["safe_fs::windows::tests::owner_only_file_directory_stage_succeed_and_rollback",
      "windows-owner-only-red.log", "VerifySecurityDescriptor|UnsupportedSecureFilesystem|UnsupportedTarget"]],
    "7b" => [["safe_fs::windows::tests::quarantine_and_publish_success_do_not_self_conflict",
      "windows-rename-red.log", "QuarantineNoReplace|UnsupportedAtomicPublish|PrimitiveUnavailable"]],
    "7c" => [["safe_fs::windows::tests::cleanup_quarantined_tree_deletes_nested_reparse_without_traversal",
      "windows-cleanup-red.log", "OpenCleanupEntry|UnsupportedSecureFilesystem|UnsupportedTarget"]],
  }.fetch(task)
  expected_parent = task == "7a" ? %w[
    component_utf16_and_rejections unicode_and_object_attribute_lifetimes operation_contract_spy_all_rows
    volume_root_contract_is_access_dependent synchronous_nt_completion_rejects_pending_buffer_small_and_warnings
    query_reports_reparse_as_present_and_open_rejects reparse_parser_bounds_every_field
    directory_parser_bounds_and_requery metadata_types_and_hardlinks ten_thousand_handles_return_to_baseline
    ancestor_mapping_cannot_rebind every_volume_field_is_bound create_new_preserves_every_existing_kind
    ntstatus_mapping_is_operation_specific production_capabilities_own_drop_resources
  ] : []
  raise "RED receipt schema" unless receipt.fetch("schema") == "opentake-c1b-windows-red-v1"
  raise "RED identity mismatch" unless receipt.values_at("task", "test_sha", "parent_sha", "nonce") ==
    [task, sha, parent, nonce]
  raise "RED changed-path contract mismatch" unless receipt.fetch("changed_paths") ==
    ["crates/opentake-project/src/safe_fs/windows.rs"]
  raise "RED provenance mismatch" unless receipt.fetch("repository") == "appergb/OpenTake" &&
    receipt.fetch("workflow") == "CI" && receipt.fetch("job_id") == "windows-red-evidence" &&
    receipt.fetch("event_name") == "workflow_dispatch" && receipt.fetch("runner_os") == "Windows" &&
    receipt.fetch("runner_arch") == "X64" && receipt.fetch("run_id").to_s == run.fetch("id").to_s &&
    receipt.fetch("run_attempt").to_s == run.fetch("run_attempt").to_s &&
    run.fetch("status") == "completed" && run.fetch("conclusion") == "success" &&
    run.fetch("event") == "workflow_dispatch" && run.fetch("path").split("@", 2).first == ".github/workflows/ci.yml" &&
    run.fetch("head_branch") == "main" && run.dig("repository", "full_name") == "appergb/OpenTake" &&
    Time.parse(artifact.fetch("created_at")) >= Time.parse(run.fetch("created_at")) &&
    artifact.fetch("name") == "c1b-red-#{task}-#{sha}-#{nonce}" && !artifact.fetch("expired") &&
    artifact.dig("workflow_run", "id").to_s == run.fetch("id").to_s &&
    artifact.dig("workflow_run", "head_sha") == run.fetch("head_sha")
  rows = jobs.fetch("jobs")
  raise "jobs pagination incomplete" unless jobs.fetch("total_count") == rows.length && rows.length <= 100
  matches = rows.select { |job| job.fetch("name") == "Windows expected RED (#{task})" && job.fetch("conclusion") == "success" }
  raise "expected one successful RED job" unless matches.length == 1
  job = matches.fetch(0)
  raise "RED job/run identity mismatch" unless job.fetch("run_id").to_s == run.fetch("id").to_s &&
    job.fetch("run_attempt").to_s == run.fetch("run_attempt").to_s &&
    job.fetch("status") == "completed" && job.fetch("head_sha") == run.fetch("head_sha") &&
    job.fetch("labels").include?("windows-2022")
  red_rows = receipt.fetch("red")
  raise "RED row count mismatch" unless red_rows.length == expected_red.length
  red_rows.zip(expected_red).each do |row, (name, log, pattern)|
    command = "cargo test -p opentake-project --lib #{name} -- --exact --test-threads=1"
    raise "RED row schema mismatch" unless row.keys.sort == %w[command exit expected log name].sort
    raise "RED row identity mismatch" unless row.values_at("name", "command", "expected", "log") ==
      [name, command, pattern, log]
    raise "RED row exit must be nonzero" unless row.fetch("exit").is_a?(Integer) && row.fetch("exit") != 0
    body = File.binread(File.join(unpacked, log))
    raise "RED log did not run exactly one test" unless body.scan(/^running 1 test\r?$/).length == 1
    raise "RED log result mismatch" unless body.scan(/^test result: FAILED\. 0 passed; 1 failed;/).length == 1
    raise "RED log selected-test mismatch" unless
      body.scan(/^test #{Regexp.escape(name)} \.\.\. FAILED\r?$/).length == 1
    raise "RED log typed refusal missing" unless body.match?(Regexp.new(pattern))
  end
  raise "parent-contract identity mismatch" unless receipt.fetch("parent_pass_tests") == expected_parent &&
    receipt.fetch("parent_pass_count") == expected_parent.length
  expected_parent.each do |short_name|
    name = "safe_fs::windows::tests::#{short_name}"
    body = File.binread(File.join(unpacked, "#{short_name}.pass.log"))
    raise "parent PASS did not run exactly one test" unless body.scan(/^running 1 test\r?$/).length == 1
    raise "parent PASS result mismatch" unless body.scan(/^test result: ok\. 1 passed; 0 failed;/).length == 1
    raise "parent PASS selected-test mismatch" unless
      body.scan(/^test #{Regexp.escape(name)} \.\.\. ok\r?$/).length == 1
  end
' "$INTAKE/unpacked/red-receipt.json" "$INTAKE/run.json" "$INTAKE/jobs.json" "$INTAKE/artifact.json" \
  "$INTAKE/unpacked" \
  "$TASK" "$TEST_SHA" "$PARENT_SHA" "$NONCE"
RED_DIR="$SAFETY_ROOT/red/c1b-task-$TASK-$TEST_SHA-$NONCE"
mkdir "$RED_DIR"
cp "$INTAKE/"{artifact.json,artifact.zip,run.json,jobs.json,workflow-content.json,expected-workflow.yml,harness-content.json,expected-harness.ps1} "$RED_DIR/"
cp "$INTAKE/unpacked/"* "$RED_DIR/"
ruby -rjson -rdigest -rpathname -e '
  output, task, parent_task, parent_sha, kind, proof, revalidation = ARGV
  proof = Pathname.new(proof).realpath
  relatives = kind == "review" ?
    %w[gate-manifest.json spec-security-review.md implementation-review.md] :
    %w[results.md results-validation.log results-validation.raw-exit
      reviews/spec-security-review.md reviews/implementation-review.md]
  digests = relatives.each_with_object({}) do |relative, values|
    path = proof.join(relative)
    values[relative] = "sha256:#{Digest::SHA256.file(path).hexdigest}"
  end
  value = { "schema" => "opentake-c1b-red-parent-proof-v1", "task" => task,
    "parent_task" => parent_task, "parent_sha" => parent_sha, "proof_kind" => kind,
    "proof_path" => proof.to_s, "revalidation" => revalidation, "file_digests" => digests }
  File.open(output, File::WRONLY | File::CREAT | File::EXCL, 0o600) do |file|
    file.write(JSON.pretty_generate(value) + "\n")
  end
' "$RED_DIR/parent-proof.json" "$TASK" "$PARENT_TASK" "$PARENT_SHA" "$PROOF_KIND" \
  "$PARENT_PROOF" "$PARENT_REVALIDATION_OUTPUT"
test -f "$RED_DIR/red-receipt.json"
test -f "$RED_DIR/parent-proof.json"
trap - EXIT
rm -rf "$INTAKE"
```

review report 引用该 nonce-bound `RED_DIR`。artifact API 未出现、run/job 未成功、digest/安全解压/身份/精确 RED 数量或 7A 的 15 项 parent-contract 任一不合约，当次 RED 无效；修正 test/harness 后生成新 test-only SHA 和新 nonce，不覆盖旧证据。

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

创建必须 exclusive：先 `mkdir "$SAFETY_ROOT/logs/c1b-task-$TASK-$SHA-attempt-$ATTEMPT"`，目录已存在即退出，不使用 `mkdir -p`、不覆盖旧 attempt。两份 report 必须含精确 `Task`、`Role`、完整 `Commit`、`Verdict: APPROVE`、`Critical: 0`、`Important: 0`、`Minor: 0`；任一 finding 产生使用该 task exact GREEN subject 的新 correction commit 和新 attempt directory，两角色全部重审。同 task 可有连续同 subject GREEN correction，不可插入 unrelated 或 merge commit。

中间 task gate 与最终 branch gate 分别为：

```text
/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake-safety/20260712-wave1bc-filesystem/branch-gates/c1b-task-<TASK>-<SHA>-<NONCE>/
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

任一 per-task gate 都不得用 `gh run download` 解包后丢失 archive，也不得手写三份 receipt JSON。对每个 receipt id，必须用已认证的 `gh api --hostname github.com` 从 `appergb/OpenTake` 读取 run、jobs 和 run-artifacts REST metadata，再调 `/repos/appergb/OpenTake/actions/artifacts/<artifact-id>/zip` 下载并保留 `artifact.zip`。目录固定为 `native-receipts/<run-id>/<receipt-id>/`，内含 `run.json`、`jobs.json`、`artifact.json`、`artifact.zip`、从 ZIP 解出的 `receipt.json` 与 logs/raw-exit。REST artifact `digest` 必须是 `sha256:<64-lower-hex>` 且等于保留 ZIP 的 SHA-256；artifact `workflow_run.head_sha`、run `head_sha`、job `head_sha` 和 receipt requested/checked-out SHA 都必须等于该 gate 的当前 immutable SHA。

三份 receipt 必须属于同一 workflow run/attempt；receipt id、job id 和 artifact id 各自唯一。每份 `results.md` 必须列 exact task、固定 baseline SHA、predecessor SHA、final SHA、pre/post status、每个 local gate exit、三份 run id/attempt/job id/artifact id/name/digest/SHA、两份 gate-local audit 相对路径、aggregate。`command-ledger.json`、`results.md`、review reports、REST metadata、receipt/log/raw-exit 和 `artifact.zip` 全部必须以 gate-relative path 通过 `confined_file!`；任何 absolute/越界 path 或解析到 gate 外的 symlink 必须拒绝。review reports 先复制到 gate 内的 `reviews/`，validator 不接受外部 report path。validation 脚本必须拒绝：`gh` 缺失/未认证/API 失败、repo/run/job/artifact/workflow/head SHA/digest 不合约、predecessor proof/task-chain 不合约、任一本地自造 JSON 无法被 live API 证实、以及 SHA/receipt/command/audit/clean-status 任一不合约。

## 16. Task slicing 与 RED/GREEN/receipt gate

每个行为 slice 必须是两个 commit：test-only RED，随后 production GREEN。禁止同一 commit 同时加入 test 和实现。Windows runner 上验证单个 RED 的固定函数如下；它同时断言 exit 非零、filter 实际运行恰好一个 test、失败恰好一个，因 absent module/0 tests/compile error 失败都不会通过：

```powershell
function Invoke-ExpectedRed {
  param(
    [Parameter(Mandatory = $true)][string]$Name,
    [Parameter(Mandatory = $true)][string]$Log,
    [Parameter(Mandatory = $true)][string]$ExpectedPattern
  )
  & cargo test -p opentake-project --lib $Name -- --exact --test-threads=1 2>&1 |
    Tee-Object -FilePath $Log | Out-Host
  $code = $LASTEXITCODE
  if ($code -eq 0) { throw "RED unexpectedly passed: $Name" }
  $text = Get-Content -Raw -Path $Log
  if ([regex]::Matches($text, '(?m)^running 1 test\r?$').Count -ne 1) {
    throw "RED did not execute exactly one test: $Name"
  }
  if ([regex]::Matches($text, '(?m)^test result: FAILED\. 0 passed; 1 failed;').Count -ne 1) {
    throw "RED did not report exactly one failed test: $Name"
  }
  $escaped = [regex]::Escape($Name)
  if ([regex]::Matches($text, "(?m)^test $escaped \.\.\. FAILED\r?$").Count -ne 1) {
    throw "RED failure was not the selected test: $Name"
  }
  if (-not (Select-String -Quiet -Path $Log -Pattern $ExpectedPattern)) {
    throw "RED did not fail for the required typed refusal: $Name / $ExpectedPattern"
  }
  return $code
}

function Invoke-ExpectedPass {
  param(
    [Parameter(Mandatory = $true)][string]$Name,
    [Parameter(Mandatory = $true)][string]$Log
  )
  & cargo test -p opentake-project --lib $Name -- --exact --test-threads=1 2>&1 |
    Tee-Object -FilePath $Log | Out-Host
  if ($LASTEXITCODE -ne 0) { throw "required parent-contract test failed: $Name" }
  $text = Get-Content -Raw -Path $Log
  if ([regex]::Matches($text, '(?m)^running 1 test\r?$').Count -ne 1 -or
      [regex]::Matches($text, '(?m)^test result: ok\. 1 passed; 0 failed;').Count -ne 1) {
    throw "PASS did not execute exactly one successful test: $Name"
  }
}
```

Task 3 GREEN 同时提交 `scripts/run-c1b-windows-red.ps1`。上述两个 function 原样放在下列参数/调度代码之后；脚本是四个 Windows test-only RED 的唯一执行入口，不接受任意 test name 或 command：

```powershell
param(
  [Parameter(Mandatory = $true)][ValidateSet('6b', '7a', '7b', '7c')][string]$Task,
  [Parameter(Mandatory = $true)][ValidatePattern('^[0-9a-f]{40}$')][string]$TestSha,
  [Parameter(Mandatory = $true)][ValidatePattern('^[0-9a-f]{40}$')][string]$ParentSha,
  [Parameter(Mandatory = $true)][ValidatePattern('^[0-9a-f]{16}$')][string]$Nonce,
  [Parameter(Mandatory = $true)][string]$EvidenceRoot
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$actual = (git rev-parse HEAD).Trim().ToLowerInvariant()
$actualParent = (git rev-parse 'HEAD^').Trim().ToLowerInvariant()
$commitRow = @((git rev-list --parents -n 1 HEAD).Trim().Split(' '))
$changedPaths = @(git diff-tree --no-commit-id --name-only -r HEAD)
if ($actual -cne $TestSha) { throw 'expected-RED checkout does not match TestSha' }
if ($actualParent -cne $ParentSha) { throw 'expected-RED parent does not match ParentSha' }
if ($commitRow.Count -ne 2) { throw 'expected-RED commit must have exactly one parent' }
if ($changedPaths.Count -ne 1 -or $changedPaths[0] -cne 'crates/opentake-project/src/safe_fs/windows.rs') {
  throw 'expected-RED commit changed paths outside windows.rs'
}
New-Item -ItemType Directory -Path $EvidenceRoot -ErrorAction Stop | Out-Null
$Evidence = Join-Path $EvidenceRoot "c1b-task-$Task-$TestSha-$Nonce"
New-Item -ItemType Directory -Path $Evidence -ErrorAction Stop | Out-Null
$startedAt = (Get-Date).ToUniversalTime().ToString('o')
$redRows = [System.Collections.Generic.List[object]]::new()
$parentPassTests = @()

function Invoke-ExpectedRed {
  param(
    [Parameter(Mandatory = $true)][string]$Name,
    [Parameter(Mandatory = $true)][string]$Log,
    [Parameter(Mandatory = $true)][string]$ExpectedPattern
  )
  & cargo test -p opentake-project --lib $Name -- --exact --test-threads=1 2>&1 |
    Tee-Object -FilePath $Log | Out-Host
  $code = $LASTEXITCODE
  if ($code -eq 0) { throw "RED unexpectedly passed: $Name" }
  $text = Get-Content -Raw -Path $Log
  if ([regex]::Matches($text, '(?m)^running 1 test\r?$').Count -ne 1) {
    throw "RED did not execute exactly one test: $Name"
  }
  if ([regex]::Matches($text, '(?m)^test result: FAILED\. 0 passed; 1 failed;').Count -ne 1) {
    throw "RED did not report exactly one failed test: $Name"
  }
  $escaped = [regex]::Escape($Name)
  if ([regex]::Matches($text, "(?m)^test $escaped \.\.\. FAILED\r?$").Count -ne 1) {
    throw "RED failure was not the selected test: $Name"
  }
  if (-not (Select-String -Quiet -Path $Log -Pattern $ExpectedPattern)) {
    throw "RED did not fail for the required typed refusal: $Name / $ExpectedPattern"
  }
  return $code
}

function Invoke-ExpectedPass {
  param(
    [Parameter(Mandatory = $true)][string]$Name,
    [Parameter(Mandatory = $true)][string]$Log
  )
  & cargo test -p opentake-project --lib $Name -- --exact --test-threads=1 2>&1 |
    Tee-Object -FilePath $Log | Out-Host
  if ($LASTEXITCODE -ne 0) { throw "required parent-contract test failed: $Name" }
  $text = Get-Content -Raw -Path $Log
  if ([regex]::Matches($text, '(?m)^running 1 test\r?$').Count -ne 1 -or
      [regex]::Matches($text, '(?m)^test result: ok\. 1 passed; 0 failed;').Count -ne 1) {
    throw "PASS did not execute exactly one successful test: $Name"
  }
}
function Invoke-RedCase {
  param([string]$Name, [string]$LogName, [string]$ExpectedPattern)
  $exit = Invoke-ExpectedRed -Name $Name -Log (Join-Path $Evidence $LogName) `
    -ExpectedPattern $ExpectedPattern
  $redRows.Add([ordered]@{
    name = $Name
    command = "cargo test -p opentake-project --lib $Name -- --exact --test-threads=1"
    exit = $exit
    expected = $ExpectedPattern
    log = $LogName
  })
}

switch ($Task) {
  '6b' {
    Invoke-RedCase 'safe_fs::windows::tests::nested_retained_io_roundtrip' `
      'windows-io-red.log' 'UnsupportedSecureFilesystem|UnsupportedTarget'
    Invoke-RedCase 'safe_fs::windows::tests::windows_post_create_metadata_failure_rolls_back_same_handle' `
      'windows-create-rollback-red.log' 'UnsupportedSecureFilesystem|UnsupportedTarget'
  }
  '7a' {
    $parentPassTests = @(
      'component_utf16_and_rejections', 'unicode_and_object_attribute_lifetimes',
      'operation_contract_spy_all_rows', 'volume_root_contract_is_access_dependent',
      'synchronous_nt_completion_rejects_pending_buffer_small_and_warnings',
      'query_reports_reparse_as_present_and_open_rejects', 'reparse_parser_bounds_every_field',
      'directory_parser_bounds_and_requery', 'metadata_types_and_hardlinks',
      'ten_thousand_handles_return_to_baseline', 'ancestor_mapping_cannot_rebind',
      'every_volume_field_is_bound', 'create_new_preserves_every_existing_kind',
      'ntstatus_mapping_is_operation_specific', 'production_capabilities_own_drop_resources'
    )
    foreach ($shortName in $parentPassTests) {
      $fullName = "safe_fs::windows::tests::$shortName"
      Invoke-ExpectedPass -Name $fullName -Log (Join-Path $Evidence "$shortName.pass.log")
    }
    Invoke-RedCase 'safe_fs::windows::tests::owner_only_file_directory_stage_succeed_and_rollback' `
      'windows-owner-only-red.log' 'VerifySecurityDescriptor|UnsupportedSecureFilesystem|UnsupportedTarget'
  }
  '7b' {
    Invoke-RedCase 'safe_fs::windows::tests::quarantine_and_publish_success_do_not_self_conflict' `
      'windows-rename-red.log' 'QuarantineNoReplace|UnsupportedAtomicPublish|PrimitiveUnavailable'
  }
  '7c' {
    Invoke-RedCase 'safe_fs::windows::tests::cleanup_quarantined_tree_deletes_nested_reparse_without_traversal' `
      'windows-cleanup-red.log' 'OpenCleanupEntry|UnsupportedSecureFilesystem|UnsupportedTarget'
  }
}

[ordered]@{
  schema = 'opentake-c1b-windows-red-v1'
  repository = $env:GITHUB_REPOSITORY
  workflow = $env:GITHUB_WORKFLOW
  run_id = $env:GITHUB_RUN_ID
  run_attempt = $env:GITHUB_RUN_ATTEMPT
  job_id = $env:GITHUB_JOB
  event_name = $env:GITHUB_EVENT_NAME
  runner_os = $env:RUNNER_OS
  runner_arch = $env:RUNNER_ARCH
  task = $Task
  test_sha = $TestSha
  parent_sha = $ParentSha
  nonce = $Nonce
  changed_paths = @($changedPaths)
  red = @($redRows)
  parent_pass_tests = @($parentPassTests)
  parent_pass_count = $parentPassTests.Count
  started_at_utc = $startedAt
  finished_at_utc = (Get-Date).ToUniversalTime().ToString('o')
} | ConvertTo-Json -Depth 8 | Set-Content -Encoding utf8NoBOM `
  (Join-Path $Evidence 'red-receipt.json')
```

Task 3 GREEN 的 Windows matrix 先用 PowerShell parser 解析该文件并要求零 syntax error。expected-RED job 只在授权的 `workflow_dispatch` 且 `red_task != none` 时运行，使用 runner-local `$RUNNER_TEMP` 不读取 macOS `SAFETY_ROOT`；常规 rust/web/native jobs 在该模式下不运行。

固定 commit/gate sequence 只有一套：Task 3 CI + evidence validators RED/GREEN 使用 sections 18.7–18.8；Task 6A compile-complete refusal scaffold 使用 section 18.1；Task 6B 精确七个 test-only bodies（acquisition/I/O 与 metadata rollback 是两个独立 behavior RED）与 sections 2–7/9/11（含 section 18.4 production revalidation body/hook）GREEN 使用 section 18.2；Task 7A OwnerOnly/DACL/rollback、Task 7B quarantine/publish rename、Task 7C retained cleanup/delete/reparse 各自独立 RED→GREEN，使用 sections 18.3–18.6；Task 8 只创建实际 convergence gate并调用已提交 validators。不存在总括 Task 7 commit；证据目录逐项为 `c1b-task-7a-*`、`c1b-task-7b-*`、`c1b-task-7c-*`。

每个 commit 前 `git diff --cached --name-only` 必须等于该步骤列出的 paths；每个 GREEN 前保存对应 RED commit SHA/log/receipt。任何 RED 在 `running 1 test` 前失败都要先修 test harness 并产生新的 test-only commit；不得把 harness compile failure记录成行为 RED。本稿不授权 push/PR；到首次 native receipt gate 若无远端 authority，按 section 14 写 BLOCKED 并停止。

## 17. 仍未消除、必须如实保留的风险

1. 本机不是 Windows，所有 NT/DACL/layout 行为只有 Windows native receipt 才能升级为已验证；本稿只依据本机 `windows-sys 0.61.2` declaration冻结调用形状。
2. `workflow_dispatch` workflow 首次进入 default branch 前不可作为手工 trigger；无 authorized PR 时原生 gate 必然 BLOCKED。
3. Windows filesystem/filter driver 可能对 ambiguous rename collision 返回不同 NTSTATUS；本稿通过 pre/post relative query 收窄 classification，但 native tests必须在实际 runner上锁定 file/dir/reparse cases。
4. per-directory case-sensitive query、removable volume 和 remote protocol probe 在旧 Windows/filesystem上可能 unsupported；行为是 typed fail closed，不承诺所有卷都支持 export。
5. Unix 无通用“按 fd unlink name”原语；主计划必须采用 quarantine/fail-leak 或明确缩窄 threat contract，不能恢复 attempt 1 的原子 identity-bound 声称。

以上风险都不是允许 pathname fallback、ordinary rename、跳过 native receipt 或擅自远端 publication 的理由。

## 18. Attempt 6 executable patches

Sections 2–15 freeze the final platform algorithms and evidence contracts. This section supplies the exact compile-scaffold, task-specific source/test patches, and repository-versioned validators used by the single task sequence in section 16; it does not define a second facade or an alternate task order.

本节关闭 Attempt 3–4 的全部 Windows/CI finding，并给 sections 2–15 的最终算法配置唯一的 task-specific patch sequence。全计划只使用主计划 Task 6A、6B、7A、7B、7C、8 编号，report/receipt path 也只使用这些编号。

### 18.1 Task 6A：先提交 compile-complete fail-closed Windows scaffold

Task 6A commit `feat(project): add fail-closed Windows platform scaffold` 发生在任何 Windows behavior test 之前。它加入 Windows target dependency，并把 Task 2A 的 `include!("unsupported.rs")` 替换为下面这份完整 `windows.rs`。该 scaffold 的唯一职责是让 common facade 在 `x86_64-pc-windows-msvc` 上完整编译；所有 acquisition、I/O、DACL、quarantine、publish、cleanup 都以结构化错误拒绝。它没有 `todo!`、`unimplemented!`、panic 或缺失 symbol。

```rust
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

pub(super) fn capture_absolute_directory(_: &Path, _: DirectoryAccess) -> Result<DirectoryAuthority> {
    filesystem_refusal(SafeFsOperation::CaptureNamespaceRoot)
}
pub(super) fn revalidate_namespace(_: &DirectoryAuthority) -> Result<()> {
    filesystem_refusal(SafeFsOperation::RevalidateNamespace)
}
pub(super) fn query_child_nofollow(_: &DirectoryAuthority, _: &ComponentName) -> Result<ChildState> {
    filesystem_refusal(SafeFsOperation::QueryChild)
}
pub(super) fn open_dir_nofollow(_: &DirectoryAuthority, _: &ComponentName, _: DirectoryAccess) -> Result<DirectoryAuthority> {
    filesystem_refusal(SafeFsOperation::OpenDirectory)
}
pub(super) fn open_file_nofollow(_: &DirectoryAuthority, _: &ComponentName, _: FileAccess) -> Result<FileCapability> {
    filesystem_refusal(SafeFsOperation::OpenFile)
}
pub(super) fn create_dir_new(_: &DirectoryAuthority, _: &ComponentName, _: CreatePermissions, _: DirectoryAccess) -> Result<DirectoryAuthority> {
    filesystem_refusal(SafeFsOperation::CreateDirectory)
}
pub(super) fn create_stage_dir_new(_: &DirectoryAuthority, _: &ComponentName, _: CreatePermissions) -> Result<StageCapability> {
    mutation_refusal(SafeFsOperation::CreateStageDirectory)
}
pub(super) fn create_file_new(_: &DirectoryAuthority, _: &ComponentName, _: CreatePermissions) -> Result<FileCapability> {
    filesystem_refusal(SafeFsOperation::CreateFile)
}
pub(super) fn enumerate(_: &DirectoryAuthority) -> Result<Vec<ComponentName>> {
    filesystem_refusal(SafeFsOperation::EnumerateDirectory)
}
pub(super) fn read_link_component(_: &DirectoryAuthority, _: &ComponentName) -> Result<RawLinkTarget> {
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
pub(super) fn quarantine_stage(_: StageCapability, _: &DirectoryAuthority, _: ComponentName) -> Result<QuarantinedCapability> {
    mutation_refusal(SafeFsOperation::QuarantineNoReplace)
}
pub(super) fn publish_stage_noreplace(_: StageCapability, _: &DirectoryAuthority, _: ComponentName) -> Result<()> {
    mutation_refusal(SafeFsOperation::PublishNoReplace)
}
pub(super) fn open_cleanup_child_nofollow(_: &QuarantinedCapability, _: &ComponentName) -> Result<CleanupCapability> {
    filesystem_refusal(SafeFsOperation::OpenCleanupEntry)
}
pub(super) fn delete_quarantined_entry(_: CleanupCapability) -> Result<()> {
    filesystem_refusal(SafeFsOperation::DeleteQuarantinedEntry)
}
pub(super) fn delete_quarantined_empty_directory(_: QuarantinedCapability) -> Result<()> {
    filesystem_refusal(SafeFsOperation::DeleteQuarantinedEmptyDirectory)
}
```

Task 6A 固定 gate：

```powershell
cargo fmt --all --check
cargo clippy -p opentake-project --lib --tests --target x86_64-pc-windows-msvc -- -D warnings
cargo check -p opentake-project --lib --tests --target x86_64-pc-windows-msvc
git diff --check
```

提交 `feat(project): add fail-closed Windows platform scaffold` 后记录 `TASK6A_GREEN_SHA=$(git rev-parse HEAD)`，为每次审查 exclusive-create `$SAFETY_ROOT/logs/c1b-task-6a-$TASK6A_GREEN_SHA-attempt-$REVIEW_ATTEMPT/`。两名 fresh reviewer 都必须写 `Task: 6a`、完整 `Commit`、`Verdict: APPROVE` 和三个零 finding。通过后不创建 review-only manifest；立即按 section 18.8 创建 `$SAFETY_ROOT/branch-gates/c1b-task-6a-$TASK6A_GREEN_SHA-<16-lower-hex-NONCE>/`，传 `TASK=6a`、`PREDECESSOR_SHA=Task 5 GREEN SHA`、`PREDECESSOR_PROOF=Task 5 branch gate`，收集三份 authenticated native receipt 并使同一 validator 返回零。Task 6B 的 `PREDECESSOR_PROOF` 必须是这一 Task 6A branch gate；不得只传 Task 6A SHA 或手写 review manifest。

### 18.2 Task 6B：七个 test-only bodies、两个 behavior RED，随后实现 acquisition/I/O/create rollback

Task 6B test-only commit `test(project): specify Windows capability acquisition and io` 加入 section 12 Task 6B 行的精确七个 bodies、`TestDir`/`name`/`root`、普通 helper `assert_file_create_failure_rolls_back`，以及下面 compile-only create/disposition-failure seams。该 helper 完整 body与所需 imports必须在同一 test-only commit，不能等到 GREEN。它不引用任何 Task 6B production-private symbol，因而在 Task 6A parent 上 compile-complete。GREEN 必须删除 compile-only seams并用 section 9 final seams原位替换，不得同时保留两份定义：

```rust
#[cfg(test)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum WindowsCreateFailurePoint {
    Metadata, FilesystemProbe, TypeValidation, CaseProof, SnapshotAssembly, ParentDuplicate,
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
        *WINDOWS_CREATE_FAILURE.get_or_init(Default::default).lock()
            .expect("Windows create-failure mutex poisoned") = None;
    }
}
#[cfg(test)]
fn install_windows_create_failure(point: WindowsCreateFailurePoint) -> WindowsCreateFailureGuard {
    let mut slot = WINDOWS_CREATE_FAILURE.get_or_init(Default::default).lock()
        .expect("Windows create-failure mutex poisoned");
    assert!(slot.is_none(), "Windows create-failure tests require --test-threads=1");
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
    assert!(!FAIL_NEXT_CREATED_DISPOSITION.swap(true, std::sync::atomic::Ordering::SeqCst),
        "created disposition tests require --test-threads=1");
    CreatedDispositionFailureGuard
}
```

两个独立聚焦 RED 都由 section 16 harness 的 `Task='6b'` 固定分支执行：各自必须显示一次 `running 1 test`、一次 FAILED，并因 Task 6A `capture_absolute_directory` 的 `UnsupportedSecureFilesystem/UnsupportedTarget` 拒绝而非 compile/0-tests 失败。调度与回传只使用 section 14.1，不存在第二份本地 PowerShell receipt 协议。

Task 6B GREEN `feat(project): capture Windows filesystem capabilities` 完整加入 sections 2–7 与 11 的 real production bodies、parser/reparse/NTSTATUS pure helpers、read/create/open/enumerate I/O，以及 section 18.4 的完整 `collect_revalidation_proof`/`revalidate_namespace` production body 和 `cfg(test)` hook storage。它不加入 section 8 的 DACL 或 section 10 的 `NtSetInformationFile`/public rename mutation，但必须安装 section 10 的纯 `RenameInformationBuffer` 与 `map_rename_failure`，使 Task 7B test-only body 在其父提交编译并可断言 layout/mapping；这两个无调用父符号带精确 item-level `#[allow(dead_code)]`，Task 7B test-only commit一加入真实调用就删除属性。Task 6B 还必须安装 section 9 的 `mark_delete_handle`、`WindowsCreateFailurePoint`、`install_windows_create_failure`、`rollback_created_handle` 和 `rollback_created_directory`。所有 create contracts 从 `NtCreateFile(FILE_CREATE)` 开始已持有 `DELETE`，因此 metadata、filesystem、type、case、snapshot 或 post-create parent duplication 任一失败都在返回前用同一 HANDLE disposition rollback。Task 7A 只把 security verification 接入这一已存在的 rollback path，Task 7B 只接线 retained rename execution，Task 7C 只复用 `mark_delete_handle`，三者均不重定义 Task 6B helpers。Task 6B GREEN 只保留下列 final-signature refusal bodies；后续 task 的 test seams在各自 test-only commit加入，避免 future-symbol dead code：

```rust
fn owner_only_refusal<T>() -> Result<T> {
    Err(SafeFsError::UnsupportedSecureFilesystem {
        operation: SafeFsOperation::VerifySecurityDescriptor,
        reason: SecureFilesystemReason::UnsupportedTarget,
    })
}

fn require_inherited_permissions(permissions: CreatePermissions) -> Result<()> {
    match permissions {
        CreatePermissions::Inherit => Ok(()),
        CreatePermissions::OwnerOnly => owner_only_refusal(),
    }
}

pub(super) fn quarantine_stage(_: StageCapability, _: &DirectoryAuthority, _: ComponentName) -> Result<QuarantinedCapability> {
    Err(SafeFsError::UnsupportedAtomicPublish {
        operation: SafeFsOperation::QuarantineNoReplace,
        reason: AtomicPublishReason::PrimitiveUnavailable,
    })
}
pub(super) fn publish_stage_noreplace(_: StageCapability, _: &DirectoryAuthority, _: ComponentName) -> Result<()> {
    Err(SafeFsError::UnsupportedAtomicPublish {
        operation: SafeFsOperation::PublishNoReplace,
        reason: AtomicPublishReason::PrimitiveUnavailable,
    })
}
pub(super) fn open_cleanup_child_nofollow(_: &QuarantinedCapability, _: &ComponentName) -> Result<CleanupCapability> {
    Err(SafeFsError::UnsupportedSecureFilesystem {
        operation: SafeFsOperation::OpenCleanupEntry,
        reason: SecureFilesystemReason::UnsupportedTarget,
    })
}
pub(super) fn delete_quarantined_entry(_: CleanupCapability) -> Result<()> {
    Err(SafeFsError::UnsupportedSecureFilesystem {
        operation: SafeFsOperation::DeleteQuarantinedEntry,
        reason: SecureFilesystemReason::UnsupportedTarget,
    })
}
pub(super) fn delete_quarantined_empty_directory(_: QuarantinedCapability) -> Result<()> {
    Err(SafeFsError::UnsupportedSecureFilesystem {
        operation: SafeFsOperation::DeleteQuarantinedEmptyDirectory,
        reason: SecureFilesystemReason::UnsupportedTarget,
    })
}
```

Task 7A test-only commit 与 22 个 owned bodies 同时加入下面 DACL refusal/fixture seam；每个 symbol在该提交已有 test call site，不属于 Task 6B GREEN：

```rust

#[cfg(test)]
static FORCE_DACL_VERIFY_FAILURE: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

#[cfg(test)]
fn force_next_owner_verification_failure() {
    FORCE_DACL_VERIFY_FAILURE.store(true, std::sync::atomic::Ordering::SeqCst);
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum OwnerDescriptorFixture {
    WrongAceType,
    UndersizedAce,
    OversizedSid,
    InvalidSid,
    NullOwner,
    InvalidOwner,
    DaclOutOfRange,
    AclBytesOutOfRange,
    WrongAceCount,
    AceOutOfRange,
}

#[cfg(test)]
static OWNER_DESCRIPTOR_FIXTURE: std::sync::OnceLock<
    std::sync::Mutex<Option<OwnerDescriptorFixture>>,
> = std::sync::OnceLock::new();

#[cfg(test)]
struct OwnerDescriptorFixtureGuard;

#[cfg(test)]
impl Drop for OwnerDescriptorFixtureGuard {
    fn drop(&mut self) {
        *OWNER_DESCRIPTOR_FIXTURE.get_or_init(Default::default).lock()
            .expect("owner descriptor fixture mutex poisoned") = None;
    }
}

#[cfg(test)]
fn install_owner_descriptor_fixture(value: OwnerDescriptorFixture) -> OwnerDescriptorFixtureGuard {
    let mut slot = OWNER_DESCRIPTOR_FIXTURE.get_or_init(Default::default).lock()
        .expect("owner descriptor fixture mutex poisoned");
    assert!(slot.is_none(), "owner descriptor tests require --test-threads=1");
    *slot = Some(value);
    OwnerDescriptorFixtureGuard
}

```

Task 7A GREEN 在安装 release verifier时才加入消费 fixture 的 helper，因此它不会在 test-only parent 中形成 dead code：

```rust

#[cfg(test)]
fn take_owner_descriptor_fixture() -> Option<OwnerDescriptorFixture> {
    OWNER_DESCRIPTOR_FIXTURE.get_or_init(Default::default).lock()
        .expect("owner descriptor fixture mutex poisoned").take()
}

```

Task 7C test-only commit 与两个 owned bodies 同时加入 retained-delete hook type/guard/installer；它们在该提交已有 test call site：

```rust

#[cfg(test)]
type BeforeRetainedDeleteHook =
    Arc<dyn Fn(HANDLE, &DirectoryAuthority, &ComponentName) -> Result<()> + Send + Sync>;
#[cfg(test)]
static BEFORE_RETAINED_DELETE_HOOK:
    std::sync::OnceLock<std::sync::Mutex<Option<BeforeRetainedDeleteHook>>> =
    std::sync::OnceLock::new();

#[cfg(test)]
struct BeforeRetainedDeleteHookGuard;
#[cfg(test)]
impl Drop for BeforeRetainedDeleteHookGuard {
    fn drop(&mut self) {
        *BEFORE_RETAINED_DELETE_HOOK.get_or_init(Default::default)
            .lock().expect("retained-delete hook mutex poisoned") = None;
    }
}

#[cfg(test)]
fn install_before_retained_delete_hook(
    hook: BeforeRetainedDeleteHook,
) -> BeforeRetainedDeleteHookGuard {
    let mut slot = BEFORE_RETAINED_DELETE_HOOK.get_or_init(Default::default)
        .lock().expect("retained-delete hook mutex poisoned");
    assert!(slot.is_none(), "retained-delete tests require --test-threads=1");
    *slot = Some(hook);
    BeforeRetainedDeleteHookGuard
}

```

Task 7C GREEN 接线 cleanup implementation 时才加入 hook runner：

```rust

fn run_before_retained_delete_hook(
    handle: HANDLE,
    parent: &DirectoryAuthority,
    name: &ComponentName,
) -> Result<()> {
    #[cfg(test)]
    {
        let hook = BEFORE_RETAINED_DELETE_HOOK.get_or_init(Default::default)
            .lock().expect("retained-delete hook mutex poisoned").clone();
        if let Some(hook) = hook { return hook(handle, parent, name); }
    }
    let _ = (handle, parent, name);
    Ok(())
}
```

`create_file_new`、`create_dir_new` 与 `create_stage_dir_new` 在 Task 6B 中先调用 `require_inherited_permissions(permissions)?`；`Inherit` 继续执行 real create，`OwnerOnly` 必须在任何 namespace mutation 前拒绝。Task 7A 删除该 refusal helper 并由 section 8 的 `OwnerOnlySecurity` 取代。Task 6B GREEN 后七个 Task 6B-owned tests（I/O 加六个 create rollback regressions）全部通过；pure parser/contract 的 production symbols 此时 compile-complete，其 15 个 bodies 由 Task 7A test-only commit 加入并必须在 OwnerOnly RED 前通过。section 18.4 的 production body 与 hook storage 必须已在本 GREEN；后续 test-only commits只增加对应 `#[test]` bodies。

Task 6B 对 section 11 create/parent-duplication 的 compile-complete bodies 固定如下；它们替换前文引用 section 8/9 symbols 的版本：

```rust
fn duplicate_directory(source: &DirectoryAuthority) -> Result<DirectoryAuthority> {
    let mut duplicated = null_mut();
    // SAFETY: retained source HANDLE; current-process source/target; output writable; same access only.
    if unsafe { DuplicateHandle(GetCurrentProcess(), source.native.node.handle.raw(), GetCurrentProcess(),
        &mut duplicated, 0, BOOL_FALSE, DUPLICATE_SAME_ACCESS) } == 0 {
        return Err(last_win32(SafeFsOperation::OpenDirectory));
    }
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

fn create_directory_contract(
    parent: &DirectoryAuthority,
    name: &ComponentName,
    permissions: CreatePermissions,
    access: DirectoryAccess,
    contract: OpenContract,
) -> Result<DirectoryAuthority> {
    let operation = if access == DirectoryAccess::Stage {
        SafeFsOperation::CreateStageDirectory
    } else {
        SafeFsOperation::CreateDirectory
    };
    require_mutation(parent, operation)?;
    require_inherited_permissions(permissions)?;
    let handle = nt_create_relative(parent.native.node.handle.raw(), name, parent.case_mode,
        contract.desired, contract.disposition, contract.options, contract.attributes, null(), operation)?;
    let validated = (|| -> Result<(EntryMetadata, CaseMode, NamespaceSnapshot)> {
        inject_windows_create_failure(WindowsCreateFailurePoint::FilesystemProbe, operation)?;
        let filesystem = parent.opened.filesystem.as_ref().ok_or(SafeFsError::UnsupportedSecureFilesystem {
            operation: SafeFsOperation::ProbeFilesystem,
            reason: SecureFilesystemReason::FilesystemProbeUnavailable,
        })?;
        inject_windows_create_failure(WindowsCreateFailurePoint::Metadata, operation)?;
        let opened = query_entry_metadata(handle.raw(), filesystem, operation)?;
        inject_windows_create_failure(WindowsCreateFailurePoint::TypeValidation, operation)?;
        if opened.kind != EntryKind::Directory {
            return Err(SafeFsError::UnsupportedEntryType { operation, kind: opened.kind });
        }
        inject_windows_create_failure(WindowsCreateFailurePoint::CaseProof, operation)?;
        let case_mode = query_case_mode(handle.raw())?;
        inject_windows_create_failure(WindowsCreateFailurePoint::SnapshotAssembly, operation)?;
        let snapshot = append_snapshot(&parent.snapshot, name.clone(), &opened, case_mode)?;
        Ok((opened, case_mode, snapshot))
    })();
    let (opened, case_mode, snapshot) = match validated {
        Ok(value) => value,
        Err(error) => return rollback_created_handle(handle, error),
    };
    let node = Arc::new(DirectoryNode {
        handle,
        parent: Some(Arc::clone(&parent.native.node)),
        name: Some(name.clone()),
        case_mode,
        metadata: opened.clone(),
        volume: parent.native.node.volume.clone(),
    });
    Ok(DirectoryAuthority {
        anchor: Arc::clone(&parent.anchor),
        native: NativeDirectory { node, access, delete_right: contract.delete_right },
        access,
        opened,
        case_mode,
        snapshot,
    })
}

pub(super) fn create_dir_new(parent: &DirectoryAuthority, name: &ComponentName,
    permissions: CreatePermissions, access: DirectoryAccess) -> Result<DirectoryAuthority> {
    if access == DirectoryAccess::Stage {
        return Err(SafeFsError::AccessMismatch { operation: SafeFsOperation::CreateDirectory });
    }
    create_directory_contract(parent, name, permissions, access, contract_for_operation(OpenOperation::CreateDir))
}

pub(super) fn create_stage_dir_new(parent: &DirectoryAuthority, name: &ComponentName,
    permissions: CreatePermissions) -> Result<StageCapability> {
    require_inherited_permissions(permissions)?;
    let directory = create_directory_contract(parent, name, CreatePermissions::Inherit,
        DirectoryAccess::Stage, contract_for_operation(OpenOperation::CreateStage))?;
    if let Err(error) = inject_windows_create_failure(WindowsCreateFailurePoint::ParentDuplicate,
        SafeFsOperation::CreateStageDirectory)
    {
        return rollback_created_directory(directory, error);
    }
    let owned_parent = match duplicate_directory(parent) {
        Ok(value) => value,
        Err(error) => return rollback_created_directory(directory, error),
    };
    let opened = directory.opened.clone();
    Ok(StageCapability { parent: owned_parent, directory, original_name: name.clone(), opened })
}

pub(super) fn create_file_new(parent: &DirectoryAuthority, name: &ComponentName,
    permissions: CreatePermissions) -> Result<FileCapability> {
    require_mutation(parent, SafeFsOperation::CreateFile)?;
    require_inherited_permissions(permissions)?;
    let contract = contract_for_operation(OpenOperation::CreateFile);
    let handle = nt_create_relative(parent.native.node.handle.raw(), name, parent.case_mode,
        contract.desired, contract.disposition, contract.options, contract.attributes, null(),
        SafeFsOperation::CreateFile)?;
    let validated = (|| -> Result<EntryMetadata> {
        inject_windows_create_failure(WindowsCreateFailurePoint::FilesystemProbe, SafeFsOperation::CreateFile)?;
        let filesystem = parent.opened.filesystem.as_ref().ok_or(SafeFsError::UnsupportedSecureFilesystem {
            operation: SafeFsOperation::ProbeFilesystem,
            reason: SecureFilesystemReason::FilesystemProbeUnavailable,
        })?;
        inject_windows_create_failure(WindowsCreateFailurePoint::Metadata, SafeFsOperation::CreateFile)?;
        let opened = query_entry_metadata(handle.raw(), filesystem, SafeFsOperation::CreateFile)?;
        inject_windows_create_failure(WindowsCreateFailurePoint::TypeValidation, SafeFsOperation::CreateFile)?;
        if opened.kind != EntryKind::RegularFile {
            return Err(SafeFsError::UnsupportedEntryType {
                operation: SafeFsOperation::CreateFile,
                kind: opened.kind,
            });
        }
        Ok(opened)
    })();
    let opened = match validated {
        Ok(value) => value,
        Err(error) => return rollback_created_handle(handle, error),
    };
    Ok(FileCapability {
        native: NativeFile { handle, opened: opened.clone(), access: FileAccess::ReadWrite, delete_right: false },
        access: FileAccess::ReadWrite,
        opened,
    })
}
```

Task 7A 保留 `duplicate_directory`，只替换上述两个 create implementation 的 permission/security branch；不得重复定义 helper。

### 18.3 Task 7A：OwnerOnly/DACL 与 create rollback

Task 7A test-only commit `test(project): specify Windows owner-only creation and rollback` 向 `crates/opentake-project/src/safe_fs/windows.rs` 加入 section 12 Task 7A 行冻结的精确 22 个 tests（15 个 Task 6B pure bodies加七个 OwnerOnly/DACL/security bodies），并在同一 test-only patch加入 section 18.2 冻结的 `force_next_owner_verification_failure`/owner-descriptor fixture guard+installer，同时删除 `SecurityVerification` variant的 item-level dead-code allowance。前 15 个在 test-only SHA 必须逐项通过，聚焦 RED 仅运行 `owner_only_file_directory_stage_succeed_and_rollback`。这些 test-only symbols不实现 `OwnerOnlySecurity`、`verify_owner_only` 或 delete/rename production：

```rust
#[test]
fn owner_only_file_directory_stage_succeed_and_rollback() {
    let temp = TestDir::new("owner-only");
    let authority = root(&temp);

    let file = create_file_new(&authority, &name("file"), CreatePermissions::OwnerOnly)
        .expect("owner-only file creation succeeds");
    drop(file);
    let directory = create_dir_new(&authority, &name("directory"),
        CreatePermissions::OwnerOnly, DirectoryAccess::MutateChildren)
        .expect("owner-only directory creation succeeds");
    drop(directory);
    let stage = create_stage_dir_new(&authority, &name("stage"), CreatePermissions::OwnerOnly)
        .expect("owner-only stage creation succeeds");
    drop(stage);
    for value in ["file", "directory", "stage"] {
        assert!(matches!(query_child_nofollow(&authority, &name(value)).unwrap(), ChildState::Present(_)));
    }

    force_next_owner_verification_failure();
    assert!(matches!(create_file_new(&authority, &name("rollback-file"), CreatePermissions::OwnerOnly),
        Err(SafeFsError::InvalidNativeBuffer {
            operation: SafeFsOperation::VerifySecurityDescriptor,
            reason: NativeBufferReason::SecurityDescriptorMalformed,
        })));
    force_next_owner_verification_failure();
    assert!(matches!(create_dir_new(&authority, &name("rollback-directory"),
        CreatePermissions::OwnerOnly, DirectoryAccess::MutateChildren),
        Err(SafeFsError::InvalidNativeBuffer {
            operation: SafeFsOperation::VerifySecurityDescriptor,
            reason: NativeBufferReason::SecurityDescriptorMalformed,
        })));
    force_next_owner_verification_failure();
    assert!(matches!(create_stage_dir_new(&authority, &name("rollback-stage"),
        CreatePermissions::OwnerOnly),
        Err(SafeFsError::InvalidNativeBuffer {
            operation: SafeFsOperation::VerifySecurityDescriptor,
            reason: NativeBufferReason::SecurityDescriptorMalformed,
        })));
    for value in ["rollback-file", "rollback-directory", "rollback-stage"] {
        assert!(matches!(query_child_nofollow(&authority, &name(value)).unwrap(), ChildState::Absent));
    }
}

fn assert_owner_descriptor_fixture_rejected(fixture: OwnerDescriptorFixture, leaf: &str) {
    let temp = TestDir::new(leaf);
    let authority = root(&temp);
    let _fixture = install_owner_descriptor_fixture(fixture);
    assert!(matches!(create_file_new(&authority, &name(leaf), CreatePermissions::OwnerOnly),
        Err(SafeFsError::InvalidNativeBuffer {
            operation: SafeFsOperation::VerifySecurityDescriptor,
            reason: NativeBufferReason::SecurityDescriptorMalformed,
        })));
    assert!(matches!(query_child_nofollow(&authority, &name(leaf)).unwrap(), ChildState::Absent));
}

#[test]
fn owner_only_dacl_rejects_wrong_ace_type() {
    assert_owner_descriptor_fixture_rejected(OwnerDescriptorFixture::WrongAceType, "wrong-ace-type");
}

#[test]
fn owner_only_dacl_rejects_undersized_ace() {
    assert_owner_descriptor_fixture_rejected(OwnerDescriptorFixture::UndersizedAce, "undersized-ace");
    assert_owner_descriptor_fixture_rejected(OwnerDescriptorFixture::DaclOutOfRange, "dacl-out-of-range");
    assert_owner_descriptor_fixture_rejected(OwnerDescriptorFixture::AclBytesOutOfRange, "acl-bytes-out-of-range");
    assert_owner_descriptor_fixture_rejected(OwnerDescriptorFixture::WrongAceCount, "wrong-ace-count");
    assert_owner_descriptor_fixture_rejected(OwnerDescriptorFixture::AceOutOfRange, "ace-out-of-range");
}

#[test]
fn owner_only_dacl_rejects_oversized_sid() {
    assert_owner_descriptor_fixture_rejected(OwnerDescriptorFixture::OversizedSid, "oversized-sid");
}

#[test]
fn owner_only_dacl_rejects_invalid_sid() {
    assert_owner_descriptor_fixture_rejected(OwnerDescriptorFixture::InvalidSid, "invalid-sid");
}

#[test]
fn owner_only_dacl_rejects_null_or_invalid_owner() {
    assert_owner_descriptor_fixture_rejected(OwnerDescriptorFixture::NullOwner, "null-owner");
    assert_owner_descriptor_fixture_rejected(OwnerDescriptorFixture::InvalidOwner, "invalid-owner");
}

#[test]
fn windows_post_create_security_failure_rolls_back_same_handle() {
    let temp = TestDir::new("security-rollback");
    let authority = root(&temp);
    let _failure = install_windows_create_failure(WindowsCreateFailurePoint::SecurityVerification);
    assert!(matches!(create_file_new(&authority, &name("leaf"), CreatePermissions::OwnerOnly),
        Err(SafeFsError::Io { operation: SafeFsOperation::VerifySecurityDescriptor, .. })));
    assert!(matches!(query_child_nofollow(&authority, &name("leaf")).unwrap(), ChildState::Absent));
}
```

上述 malformed bodies 覆盖 `WrongAceType`、`UndersizedAce`、`OversizedSid`、ACE `InvalidSid`、`NullOwner`、非空越界 `InvalidOwner`、`DaclOutOfRange`、`AclBytesOutOfRange`、`WrongAceCount` 与 `AceOutOfRange`；security-failure body另安装 `SecurityVerification`。Task 7A GREEN 在第二次 `GetKernelObjectSecurity` 成功后、任何 typed ACE/SID dereference 前消费 descriptor fixture：它只在 returned descriptor byte buffer 或已取回的本地 pointer/info 变量内产生指定 malformed layout，然后必须经过与 release 代码相同的 `checked_subslice`/`checked_sid_length`/`verify_single_owner_ace` 路径被拒绝。

RED protocol：cached path 必须恰为 `crates/opentake-project/src/safe_fs/windows.rs`；commit `test(project): specify Windows owner-only creation and rollback`。section 16 harness 的 `Task='7a'` 固定分支先运行列出的 15 个 parent-contract PASS，再运行唯一 owner-only expected RED；section 14.1 将 nonce-bound receipt/logs 回传到 `$SAFETY_ROOT/red/c1b-task-7a-<RED_SHA>-<NONCE>/`。

必须出现 `running 1 test`、仅一项 FAILED，且 panic 的 source error 是 Task 6B 的 `VerifySecurityDescriptor/UnsupportedTarget`；compile/name-resolution failure 不合格。

Task 7A GREEN commit `feat(project): enforce Windows owner-only creation` 只修改同一 `windows.rs`：删除 `owner_only_refusal`/`require_inherited_permissions`，安装 section 8 的 pinned-ABI exact `OwnerOnlySecurity`、bounds-first `verify_owner_only`、`verify_created_owner_only`，并把 section 11 三个 create bodies切到 security descriptor branch。Task 6B-owned `mark_delete_handle` 和 create rollback helpers/seams 不得重定义。`OBJECT_ATTRIBUTES.SecurityDescriptor` 全程是 `*const SECURITY_DESCRIPTOR`；所有 BOOL storage/arguments（含 `SetSecurityDescriptorDacl`）是 `windows_sys::core::BOOL=i32`；`ace_flags` 保持 `ACE_FLAGS=u32`，仅在与 `ACE_HEADER.AceFlags:u8` 比较前 `u8::try_from`。`CREATE_FILE_CONTRACT`、`CREATE_DIR_CONTRACT`、`CREATE_STAGE_CONTRACT` 必须都含 `READ_CONTROL | DELETE`。file、directory、stage 成功 post-verify；security 失败通过同一 Task 6B rollback path 对刚创建的同一 DELETE HANDLE 设置 disposition、drop、证明 name absent。

Task 7A 的 wrapper 只能是下面 final body；同一 Task 7A test-only commit已安装的 atomic flag/setter不得重复定义：

```rust
fn verify_created_owner_only(handle: HANDLE, expected: &OwnerOnlySecurity) -> Result<()> {
    inject_windows_create_failure(WindowsCreateFailurePoint::SecurityVerification,
        SafeFsOperation::VerifySecurityDescriptor)?;
    #[cfg(test)]
    if FORCE_DACL_VERIFY_FAILURE.swap(false, std::sync::atomic::Ordering::SeqCst) {
        return Err(SafeFsError::InvalidNativeBuffer {
            operation: SafeFsOperation::VerifySecurityDescriptor,
            reason: NativeBufferReason::SecurityDescriptorMalformed,
        });
    }
    verify_owner_only(handle, expected)
}
```

Task 7A GREEN 不实现 quarantine/publish/public cleanup。GREEN SHA 的 exact Windows test、target check、`git diff --check` 和双 reviewer 都写入 exclusive `$SAFETY_ROOT/logs/c1b-task-7a-<GREEN_SHA>-attempt-<M>`；两份 review 必须明确检查三种成功、三种 forced-verification rollback、五种 malformed descriptor refusal、security seam rollback、bounds-first ABI signature 和 `READ_CONTROL | DELETE`，均 `APPROVE/0/0/0` 才进入 7B。

### 18.4 Task 6B-owned fresh revalidation production body 与 Task 7B tests

Task 6B GREEN 必须用下面代码替换 section 11 的旧 `revalidate_namespace` body，并同时加入全部 `cfg(test)` hook storage/guard/install symbols。Task 7B GREEN 的 `quarantine_stage` 与 `publish_stage_noreplace` 只调用这里已存在的函数，不得重新定义或 override 本 body；mapping probe failure 因而发生在 namespace mutation 前。Task 7B test-only commit 加入本节末尾两个 mapping bodies 和 section 18.5 的三个 bodies，合计 section 12 冻结的精确五个 tests；绝不修改 production 或 hook code。

```rust
#[derive(Clone)]
struct RevalidationProof {
    volume: VolumeProof,
    snapshot: NamespaceSnapshot,
    remote: bool,
}

#[cfg(test)]
type RevalidationHook = Arc<dyn Fn(&DirectoryAuthority) -> Result<RevalidationProof> + Send + Sync>;
#[cfg(test)]
static REVALIDATION_HOOK: std::sync::OnceLock<std::sync::Mutex<Option<RevalidationHook>>> =
    std::sync::OnceLock::new();

#[cfg(test)]
#[allow(dead_code)] // Task 6B parent seam; Task 7B test-only removes this.
struct RevalidationHookGuard;
#[cfg(test)]
#[allow(dead_code)]
impl Drop for RevalidationHookGuard {
    fn drop(&mut self) {
        *REVALIDATION_HOOK.get_or_init(Default::default).lock().expect("revalidation hook mutex poisoned") = None;
    }
}

#[cfg(test)]
#[allow(dead_code)] // First call site is in Task 7B test-only bodies.
fn install_revalidation_hook(hook: RevalidationHook) -> RevalidationHookGuard {
    let mut slot = REVALIDATION_HOOK.get_or_init(Default::default).lock().expect("revalidation hook mutex poisoned");
    assert!(slot.is_none(), "revalidation tests require --test-threads=1");
    *slot = Some(hook);
    RevalidationHookGuard
}

fn collect_revalidation_proof(directory: &DirectoryAuthority) -> Result<RevalidationProof> {
    #[cfg(test)]
    {
        let hook = REVALIDATION_HOOK.get_or_init(Default::default)
            .lock().expect("revalidation hook mutex poisoned").clone();
        if let Some(hook) = hook { return hook(directory); }
    }
    let mut path = directory.anchor.native.absolute_path.clone();
    for row in directory.snapshot.components.iter().skip(directory.anchor.native.base_components) {
        path.push(row.name.as_os_str());
    }
    let fresh = capture_absolute_directory(&path, directory.anchor.native.access)
        .map_err(|_| SafeFsError::NamespaceChanged { operation: SafeFsOperation::RevalidateNamespace })?;
    Ok(RevalidationProof {
        volume: fresh.anchor.native.mapping.clone(),
        snapshot: fresh.snapshot,
        remote: false,
    })
}

pub(super) fn revalidate_namespace(directory: &DirectoryAuthority) -> Result<()> {
    let fresh = collect_revalidation_proof(directory)?;
    let expected = &directory.anchor.native.mapping;
    let exact = !fresh.remote
        && fresh.volume.mapping == expected.mapping
        && fresh.volume.guid == expected.guid
        && fresh.volume.volume_serial32 == expected.volume_serial32
        && fresh.volume.volume_serial == expected.volume_serial
        && fresh.volume.root_id == expected.root_id
        && fresh.snapshot.root_identity == directory.snapshot.root_identity
        && fresh.snapshot.root_filesystem == directory.snapshot.root_filesystem
        && fresh.snapshot.root_case_mode == directory.snapshot.root_case_mode
        && fresh.snapshot.components == directory.snapshot.components;
    if exact { Ok(()) } else {
        Err(SafeFsError::NamespaceChanged { operation: SafeFsOperation::RevalidateNamespace })
    }
}
```

完整 field-by-field blocking tests 如下；没有 real mount privilege 依赖，并且只在 Task 7B test-only commit 加入：

```rust
#[test]
fn every_revalidation_field_is_bound_before_mutation() {
    let temp = TestDir::new("probe-fields");
    let authority = root(&temp);
    let baseline = collect_revalidation_proof(&authority).unwrap();
    let reject = |mutate: fn(&mut RevalidationProof)| {
        let mut changed = baseline.clone();
        mutate(&mut changed);
        let _guard = install_revalidation_hook(Arc::new(move |_| Ok(changed.clone())));
        assert!(matches!(revalidate_namespace(&authority), Err(SafeFsError::NamespaceChanged { .. })));
    };
    reject(|p| p.volume.mapping.push(b'x' as u16));
    reject(|p| p.volume.guid.push(b'x' as u16));
    reject(|p| p.volume.volume_serial32 ^= 1);
    reject(|p| p.volume.volume_serial ^= 1);
    reject(|p| p.volume.root_id[0] ^= 1);
    reject(|p| p.snapshot.root_identity = StableIdentity::Windows { volume_serial: 1, file_id: [1; 16] });
    reject(|p| p.snapshot.root_case_mode = match p.snapshot.root_case_mode {
        CaseMode::Sensitive => CaseMode::Insensitive,
        CaseMode::Insensitive => CaseMode::Sensitive,
    });
    reject(|p| {
        let first = p.snapshot.components.first_mut().expect("fixture path has a component");
        first.identity = StableIdentity::Windows { volume_serial: 2, file_id: [2; 16] };
    });
    reject(|p| {
        let first = p.snapshot.components.first_mut().expect("fixture path has a component");
        first.case_mode = match first.case_mode {
            CaseMode::Sensitive => CaseMode::Insensitive,
            CaseMode::Insensitive => CaseMode::Sensitive,
        };
    });
    reject(|p| p.remote = true);
}

#[test]
fn quarantine_and_publish_refuse_changed_probe_without_mutation() {
    for publish in [false, true] {
        let temp = TestDir::new(if publish { "publish-probe" } else { "quarantine-probe" });
        let authority = root(&temp);
        let stage = create_stage_dir_new(&authority, &name("stage"), CreatePermissions::Inherit).unwrap();
        let mut changed = collect_revalidation_proof(&authority).unwrap();
        changed.volume.guid.push(b'x' as u16);
        let _guard = install_revalidation_hook(Arc::new(move |_| Ok(changed.clone())));
        let result = if publish {
            publish_stage_noreplace(stage, &authority, name("destination"))
                .map(|_| ())
        } else {
            quarantine_stage(stage, &authority, name("quarantine"))
                .map(|_| ())
        };
        assert!(matches!(result, Err(SafeFsError::NamespaceChanged { .. })));
        assert!(temp.path().join("stage").is_dir());
        assert!(!temp.path().join(if publish { "destination" } else { "quarantine" }).exists());
    }
}
```

### 18.5 Task 7B：quarantine/publish retained rename

Task 7B test-only commit `test(project): specify Windows retained quarantine and publish` 只加入精确五个 tests：`every_revalidation_field_is_bound_before_mutation`、`quarantine_and_publish_refuse_changed_probe_without_mutation`、`quarantine_and_publish_success_do_not_self_conflict`、`rename_never_replaces_any_target_kind`、`create_stage_collision_is_typed_and_preserves_original`。所有 stage 都用 `CreatePermissions::Inherit`，因此 RED 必须越过 Task 7A DACL 并命中 Task 6B 的 `UnsupportedAtomicPublish` refusal。不存在第六个 standalone rename-layout test；布局和 ambiguous-status assertions 集成在下面两个 behavior bodies：

```rust
#[test]
fn quarantine_and_publish_success_do_not_self_conflict() {
    let quarantine_temp = TestDir::new("quarantine-success");
    let authority = root(&quarantine_temp);
    let layout = RenameInformationBuffer::new(authority.native.node.handle.raw(),
        &ComponentName::new(OsString::from_wide(&[0x61, 0xD800])).unwrap()).unwrap();
    assert_eq!(layout.used as usize, offset_of!(FILE_RENAME_INFORMATION, FileName) + 4);
    assert_eq!((layout.as_ptr() as usize) % align_of::<FILE_RENAME_INFORMATION>(), 0);
    // SAFETY: builder returned an aligned initialized header whose `used` bytes stay live.
    assert_eq!(unsafe { (*(layout.as_ptr().cast::<FILE_RENAME_INFORMATION>())).RootDirectory },
        authority.native.node.handle.raw());
    let stage = create_stage_dir_new(&authority, &name("stage"), CreatePermissions::Inherit).unwrap();
    let quarantined = quarantine_stage(stage, &authority, name("quarantine")).expect("retained rename succeeds");
    drop(quarantined);
    assert!(!quarantine_temp.path().join("stage").exists());
    assert!(quarantine_temp.path().join("quarantine").is_dir());

    let publish_temp = TestDir::new("publish-success");
    let authority = root(&publish_temp);
    let stage = create_stage_dir_new(&authority, &name("stage"), CreatePermissions::Inherit).unwrap();
    publish_stage_noreplace(stage, &authority, name("destination")).expect("retained publish succeeds");
    assert!(!publish_temp.path().join("stage").exists());
    assert!(publish_temp.path().join("destination").is_dir());
}

#[test]
fn rename_never_replaces_any_target_kind() {
    assert!(matches!(map_rename_failure(STATUS_ACCESS_DENIED, true, true, Ok(present())),
        SafeFsError::AlreadyExists { .. }));
    assert!(matches!(map_rename_failure(STATUS_ACCESS_DENIED, true, true, Ok(ChildState::Absent)),
        SafeFsError::Os { raw: RawOsError::NtStatus { status: STATUS_ACCESS_DENIED, .. }, .. }));
    assert!(matches!(map_rename_failure(STATUS_ACCESS_DENIED, false, true, Ok(present())),
        SafeFsError::Os { .. }));
    for kind in ["file", "empty-dir", "nonempty-dir", "reparse"] {
        let temp = TestDir::new(kind);
        let target = temp.path().join("target");
        let external = temp.path().join("external");
        match kind {
            "file" => fs::write(&target, b"keep-file").unwrap(),
            "empty-dir" => fs::create_dir(&target).unwrap(),
            "nonempty-dir" => { fs::create_dir(&target).unwrap(); fs::write(target.join("keep"), b"tree").unwrap(); }
            "reparse" => {
                fs::create_dir(&external).unwrap();
                fs::write(external.join("keep"), b"outside").unwrap();
                let output = Command::new("cmd").args(["/C", "mklink", "/J"]).arg(&target).arg(&external).output().unwrap();
                assert!(output.status.success(), "mklink failed: {}", String::from_utf8_lossy(&output.stderr));
            }
            _ => unreachable!(),
        }
        let authority = root(&temp);
        let before = match query_child_nofollow(&authority, &name("target")).unwrap() {
            ChildState::Present(value) => value,
            ChildState::Absent => panic!("target fixture absent"),
        };
        let stage = create_stage_dir_new(&authority, &name("stage"), CreatePermissions::Inherit).unwrap();
        assert!(matches!(publish_stage_noreplace(stage, &authority, name("target")),
            Err(SafeFsError::AlreadyExists { .. })));
        let after = match query_child_nofollow(&authority, &name("target")).unwrap() {
            ChildState::Present(value) => value,
            ChildState::Absent => panic!("collision target removed"),
        };
        assert_eq!(after.identity, before.identity);
        match kind {
            "file" => assert_eq!(fs::read(&target).unwrap(), b"keep-file"),
            "nonempty-dir" => assert_eq!(fs::read(target.join("keep")).unwrap(), b"tree"),
            "reparse" => assert_eq!(fs::read(external.join("keep")).unwrap(), b"outside"),
            _ => assert!(target.is_dir()),
        }
    }
}

#[test]
fn create_stage_collision_is_typed_and_preserves_original() {
    let temp = TestDir::new("stage-collision");
    let authority = root(&temp);
    let original = create_stage_dir_new(&authority, &name("stage"), CreatePermissions::Inherit).unwrap();
    let before = original.opened.clone();
    drop(original);
    assert!(matches!(create_stage_dir_new(&authority, &name("stage"), CreatePermissions::Inherit),
        Err(SafeFsError::AlreadyExists {
            operation: SafeFsOperation::CreateStageDirectory,
        })));
    let after = match query_child_nofollow(&authority, &name("stage")).unwrap() {
        ChildState::Present(value) => value,
        ChildState::Absent => panic!("original stage disappeared"),
    };
    assert_eq!(after.identity, before.identity);
}
```

RED cached path 只能是 `crates/opentake-project/src/safe_fs/windows.rs`。使用 section 16 harness 的 `Task='7b'` 固定分支与 section 14.1 回传，exclusive evidence 为 `$SAFETY_ROOT/red/c1b-task-7b-<RED_SHA>-<NONCE>/`。

必须恰好一个 executed FAILED，且由 `quarantine_stage` 的 typed refusal 触发。Task 7B test-only commit加入 pure helper/hook调用时删除 Task 6B 在 rename builder/mapping 与 revalidation guard/installer上的六个 item-level `#[allow(dead_code)]`，不得新增更宽 lint suppression。Task 7B GREEN commit `feat(project): add Windows capability-relative rename` 复用已由 test-only bodies 编译检查的 `RenameInformationBuffer`、`map_rename_failure` 与 `revalidate_namespace`，只安装 `NtSetInformationFile(FileRenameInformation)` retained execution 以及 `quarantine_stage`/`publish_stage_noreplace`；它不替换 section 18.4 body/hook，也不实现 cleanup/delete。`CreateStageDirectory + STATUS_OBJECT_NAME_COLLISION` 必须映射 `AlreadyExists`，native test另建同名 stage并验证原 identity/tree不变。

GREEN 运行本节全部 rename/collision tests、18.4 两个 mapping tests、target check、`git diff --check`；exclusive review directory `$SAFETY_ROOT/logs/c1b-task-7b-<GREEN_SHA>-attempt-<M>`。两名 fresh reviewer 必须检查 Inherit fixture确实越过 DACL、rename唯一使用 retained source HANDLE与 parent `RootDirectory`、无 post-success reopen、revalidation production body所有权无 override，均 `APPROVE/0/0/0` 后进入 7C。

### 18.6 Task 7C：retained cleanup/delete/reparse 与真实 name rebound

Task 7C test-only commit `test(project): specify Windows retained cleanup and delete` 只加入下面两个 tests。它们只引用 Task 6B/7A/7B 已存在的 final-signature symbols；stage/file/directory 都用 `CreatePermissions::Inherit`，因此 RED 必须命中 `open_cleanup_child_nofollow` 的 `OpenCleanupEntry/UnsupportedTarget` refusal，而不是 DACL 或 rename：

```rust
#[test]
fn cleanup_quarantined_tree_deletes_nested_reparse_without_traversal() {
    let temp = TestDir::new("cleanup-tree");
    let external = temp.path().join("external");
    fs::create_dir(&external).unwrap();
    fs::write(external.join("keep"), b"outside-bytes").unwrap();
    let authority = root(&temp);
    let stage = create_stage_dir_new(&authority, &name("stage"), CreatePermissions::Inherit).unwrap();
    let nested = create_dir_new(stage.directory(), &name("nested"), CreatePermissions::Inherit,
        DirectoryAccess::MutateChildren).unwrap();
    let mut file = create_file_new(&nested, &name("data"), CreatePermissions::Inherit).unwrap();
    file.write_all(b"inside").unwrap();
    drop(file);
    drop(nested);
    let output = Command::new("cmd").args(["/C", "mklink", "/J"])
        .arg(temp.path().join("stage/nested/link"))
        .arg(&external).output().unwrap();
    assert!(output.status.success(), "mklink failed: {}", String::from_utf8_lossy(&output.stderr));
    let quarantine = quarantine_stage(stage, &authority, name("quarantine")).unwrap();
    super::super::cleanup_quarantined_tree(quarantine).expect("common recursive cleanup succeeds");
    assert!(matches!(query_child_nofollow(&authority, &name("quarantine")).unwrap(), ChildState::Absent));
    assert_eq!(fs::read(external.join("keep")).unwrap(), b"outside-bytes");
}

#[test]
fn retained_delete_survives_real_name_rebound() {
    let temp = TestDir::new("delete-rebound");
    let authority = root(&temp);
    let stage = create_stage_dir_new(&authority, &name("stage"), CreatePermissions::Inherit).unwrap();
    let mut file = create_file_new(stage.directory(), &name("leaf"), CreatePermissions::Inherit).unwrap();
    file.write_all(b"original").unwrap();
    drop(file);
    let quarantine = quarantine_stage(stage, &authority, name("quarantine")).unwrap();
    let quarantine_path = temp.path().join("quarantine");
    let _guard = install_before_retained_delete_hook(Arc::new(move |source, parent, _old_name| {
        let buffer = RenameInformationBuffer::new(parent.native.node.handle.raw(), &name("moved-original"))?;
        let mut iosb = IO_STATUS_BLOCK::default();
        // SAFETY: source is the retained DELETE handle; parent/buffer/iosb stay live for this synchronous test rename.
        let status = unsafe { NtSetInformationFile(source, &mut iosb, buffer.as_ptr(),
            buffer.used, FileRenameInformation) };
        complete_nt(SafeFsOperation::RenameNoReplaceSameParent, status, &iosb)?;
        fs::write(quarantine_path.join("leaf"), b"replacement")
            .map_err(|error| SafeFsError::io(SafeFsOperation::CreateFile, error))?;
        Ok(())
    }));
    let cleanup = open_cleanup_child_nofollow(&quarantine, &name("leaf")).unwrap();
    delete_quarantined_entry(cleanup).unwrap();
    assert_eq!(fs::read(temp.path().join("quarantine/leaf")).unwrap(), b"replacement");
    assert!(!temp.path().join("quarantine/moved-original").exists());
}
```

Task 7C production `delete_quarantined_entry` 必须在对 retained HANDLE 调 Task 6B-owned `mark_delete_handle` 前调用 `run_before_retained_delete_hook(handle, &parent, &name)`；non-test path恒为 no-op。test hook使用同一 retained source HANDLE把 original rename到 `moved-original`，再在旧 name创建不同 bytes；随后的 disposition 必须只删除 retained original，replacement保持。禁止用“外部 rename 被 share mask 拒绝”代替这项 proof。

`enumerate` 已由 Task 6B 返回每个 validated component，包括 reparse name；它只做 nofollow metadata query，不打开/授予 capability。`open_cleanup_child_nofollow` 对 `SymlinkOrReparse` 必须选择 `CLEANUP_REPARSE_CONTRACT`，验证同一 retained HANDLE 的 tag/identity后构造 `CleanupCapability::Entry`。common `cleanup_quarantined_tree` 的 native test必须真实递归 nested directory、file与junction，目标 `external/keep` byte-identical。

RED cached path 固定为 `windows.rs`。使用 section 16 harness 的 `Task='7c'` 固定分支与 section 14.1 回传，exclusive evidence 为 `$SAFETY_ROOT/red/c1b-task-7c-<RED_SHA>-<NONCE>/`。

Task 7C GREEN commit `feat(project): add Windows retained-handle cleanup` 只安装 section 9 的 cleanup open/disposition/public delete bodies和 hook call；不得重新定义 Task 6B 的 `mark_delete_handle`/revalidation 或 Task 7B rename。GREEN 运行本节两个 tests、完整 Windows safe_fs group、archive security、target check和`git diff --check`。exclusive `$SAFETY_ROOT/logs/c1b-task-7c-<GREEN_SHA>-attempt-<M>` 双 review必须检查实际 common recursion、reparse target未变、真实 retained HANDLE rebound与replacement保留，均 `APPROVE/0/0/0` 后才能请求 final exact-SHA native receipts。

### 18.7 Repository-versioned CI validator 与完整 RED/GREEN test

Task 3（主计划编号保持 Task 3）同时拥有 CI 与 final-evidence validator 的 TDD。下列 `scripts/validate-c1b-ci.rb` body 是 GREEN body；test-only RED 先加入两个 tests和两个 fail-closed scaffold，见本节末尾 exact protocol。

```ruby
#!/usr/bin/env ruby
# frozen_string_literal: true

require "yaml"

module C1bCiValidator
  SHA = /\A[0-9a-f]{40}\z/
  RECEIPTS = %w[linux-x86_64 macos-native windows-x86_64].freeze
  PROVENANCE = {
    "linux-x86_64" => { "runner" => "ubuntu-24.04", "expected_os" => "Linux", "expected_arch" => "X64" },
    "macos-native" => { "runner" => "macos-14", "expected_os" => "macOS", "expected_arch" => "ARM64" },
    "windows-x86_64" => { "runner" => "windows-2022", "expected_os" => "Windows", "expected_arch" => "X64" },
  }.freeze
  TARGET_EXPRESSION = "${{ github.event_name == 'workflow_dispatch' && inputs.commit_sha || github.event_name == 'pull_request' && github.event.pull_request.head.sha || github.sha }}"

  module_function

  def select_target(event_name:, github_sha:, pull_request_head_sha: nil, dispatch_sha: nil)
    value = case event_name
            when "workflow_dispatch" then dispatch_sha
            when "pull_request" then pull_request_head_sha
            when "push" then github_sha
            else raise "unsupported event #{event_name.inspect}"
            end
    normalized = value.to_s.downcase
    raise "selected SHA is not immutable 40-hex" unless SHA.match?(normalized)
    normalized
  end

  def validate(path)
    raw = File.read(path)
    document = YAML.safe_load(raw, aliases: true)
    raise "workflow root must be a mapping" unless document.is_a?(Hash)
    events = document["on"] || document[true]
    raise "missing on mapping" unless events.is_a?(Hash)
    raise "push must be independently bound to main" unless events.dig("push", "branches") == ["main"]
    raise "pull_request trigger missing" unless events.key?("pull_request")
    dispatch = events.dig("workflow_dispatch", "inputs", "commit_sha")
    raise "workflow_dispatch.commit_sha must be required" unless dispatch.is_a?(Hash) && dispatch["required"] == true
    red_task = events.dig("workflow_dispatch", "inputs", "red_task")
    raise "workflow_dispatch.red_task contract mismatch" unless red_task.is_a?(Hash) &&
      red_task.values_at("required", "default", "type") == [true, "none", "choice"] &&
      red_task.fetch("options") == %w[none 6b 7a 7b 7c]
    red_parent = events.dig("workflow_dispatch", "inputs", "red_parent_sha")
    red_nonce = events.dig("workflow_dispatch", "inputs", "red_nonce")
    raise "workflow_dispatch RED identity inputs missing" unless
      [red_parent, red_nonce].all? { |input| input.is_a?(Hash) && input["required"] == false && input["default"] == "" }
    expected_concurrency = "${{ github.workflow }}-${{ github.event_name }}-${{ github.ref }}-${{ inputs.commit_sha || github.sha }}-${{ inputs.red_task || 'normal' }}-${{ inputs.red_nonce || 'none' }}"
    raise "workflow concurrency is not RED nonce-bound" unless
      document.dig("concurrency", "group") == expected_concurrency &&
      document.dig("concurrency", "cancel-in-progress") == true

    job = document.dig("jobs", "safe-filesystem")
    raise "missing safe-filesystem job and immutable SHA binding" unless job.is_a?(Hash)
    normal_condition = "github.event_name != 'workflow_dispatch' || inputs.red_task == 'none'"
    %w[rust web safe-filesystem].each do |job_name|
      raise "#{job_name} must be disabled during expected-RED dispatch" unless
        document.dig("jobs", job_name, "if") == normal_condition
    end
    rows = job.dig("strategy", "matrix", "include")
    raise "safe-filesystem matrix missing" unless rows.is_a?(Array)
    receipt_ids = rows.map { |row| row.fetch("receipt_id") }
    raise "duplicate or missing receipt ids" unless receipt_ids.sort == RECEIPTS.sort && receipt_ids.uniq.length == receipt_ids.length
    rows.each do |row|
      expected = PROVENANCE.fetch(row.fetch("receipt_id"))
      raise "native runner provenance does not match receipt id" unless
        row.values_at("runner", "expected_os", "expected_arch") ==
          expected.values_at("runner", "expected_os", "expected_arch")
    end

    target = job.dig("env", "TARGET_SHA")
    raise "TARGET_SHA must bind push, PR head, and dispatch independently" unless target == TARGET_EXPRESSION
    steps = job.fetch("steps")
    checkout = steps.find { |step| step["uses"] == "actions/checkout@v4" }
    raise "checkout@v4 step missing" unless checkout
    raise "checkout ref is not TARGET_SHA" unless checkout.dig("with", "ref") == "${{ env.TARGET_SHA }}"
    raise "checkout must fetch immutable object history" unless checkout.dig("with", "fetch-depth") == 0
    raise "checkout credentials must not persist" unless checkout.dig("with", "persist-credentials") == false

    bind = steps.find { |step| step["name"] == "Assert exact checked-out SHA" }
    bind_text = bind && bind["run"].to_s
    raise "missing exact git rev-parse assertion" unless bind_text&.include?("git rev-parse HEAD") && bind_text.include?('test "$actual" = "$expected"')
    receipt = steps.find { |step| step["name"] == "Build exclusive JSON receipt" }
    raise "receipt builder must run under always" unless receipt && receipt["if"] == "always()"
    receipt_text = receipt["run"].to_s
    %w[repository workflow workflow_file job_id event_name receipt_id runner_label runner_os runner_arch
       requested_sha checked_out_sha aggregate_exit opentake-c1b-native-receipt-v1].each do |token|
      raise "receipt missing #{token}" unless receipt_text.include?(token)
    end
    {
      "repository" => "repository = '${{ github.repository }}'",
      "workflow" => "workflow = '${{ github.workflow }}'",
      "job_id" => "job_id = '${{ github.job }}'",
      "event_name" => "event_name = '${{ github.event_name }}'",
      "runner_os" => "runner_os = '${{ runner.os }}'",
      "runner_arch" => "runner_arch = '${{ runner.arch }}'",
      "receipt_id" => "receipt_id = $env:RECEIPT_ID",
      "runner_label" => "runner_label = $env:RUNNER_LABEL",
    }.each do |field, binding|
      raise "receipt #{field} is not context-bound" unless receipt_text.include?(binding)
    end
    expected_env = {
      "RECEIPT_ID" => "${{ matrix.receipt_id }}",
      "RUNNER_LABEL" => "${{ matrix.runner }}",
      "EXPECTED_RUNNER_OS" => "${{ matrix.expected_os }}",
      "EXPECTED_RUNNER_ARCH" => "${{ matrix.expected_arch }}",
    }
    raise "receipt environment is not matrix-bound" unless expected_env.all? { |key, value| receipt.dig("env", key) == value }
    [
      "if ('${{ runner.os }}' -ne $env:EXPECTED_RUNNER_OS)",
      "if ('${{ runner.arch }}' -ne $env:EXPECTED_RUNNER_ARCH)",
    ].each do |guard|
      raise "receipt runtime provenance guard missing: #{guard}" unless receipt_text.include?(guard)
    end
    upload = steps.find { |step| step["uses"] == "actions/upload-artifact@v4" }
    raise "receipt artifact upload missing" unless upload && upload["if"] == "always()"
    expected_name = "c1b-native-${{ matrix.receipt_id }}-${{ steps.bind.outputs.sha }}"
    raise "artifact name not SHA-bound" unless upload.dig("with", "name") == expected_name
    enforce = steps.find { |step| step["name"] == "Enforce native aggregate" }
    raise "aggregate enforce step missing" unless enforce && enforce["if"] == "always()" &&
      enforce["run"].to_s.include?("final-aggregate.raw-exit")

    parser = steps.find { |step| step["name"] == "Parse Windows expected-RED harness" }
    raise "Windows RED harness parser missing" unless parser && parser["if"] == "runner.os == 'Windows'" &&
      parser["shell"] == "pwsh" && parser["run"].to_s.include?("run-c1b-windows-red.ps1") &&
      parser["run"].to_s.include?("ParseFile") && parser["run"].to_s.include?("$errors.Count -ne 0")

    red_job = document.dig("jobs", "windows-red-evidence")
    raise "missing dispatch-only Windows RED job" unless red_job.is_a?(Hash) &&
      red_job["if"] == "github.event_name == 'workflow_dispatch' && inputs.red_task != 'none'" &&
      red_job["runs-on"] == "windows-2022"
    expected_red_env = {
      "TARGET_SHA" => "${{ inputs.commit_sha }}", "PARENT_SHA" => "${{ inputs.red_parent_sha }}",
      "RED_TASK" => "${{ inputs.red_task }}", "RED_NONCE" => "${{ inputs.red_nonce }}",
    }
    raise "Windows RED job environment is not context-bound" unless
      expected_red_env.all? { |key, value| red_job.dig("env", key) == value }
    red_steps = red_job.fetch("steps")
    red_input = red_steps.find { |step| step["name"] == "Validate immutable RED inputs" }
    red_input_text = red_input && red_input["run"].to_s
    raise "Windows RED immutable input guards missing" unless red_input && red_input["shell"] == "pwsh" &&
      %w[TARGET_SHA PARENT_SHA RED_NONCE].all? { |token| red_input_text.include?(token) } &&
      red_input_text.include?("^[0-9a-f]{40}$") && red_input_text.include?("^[0-9a-f]{16}$")
    red_checkout = red_steps.find { |step| step["uses"] == "actions/checkout@v4" }
    raise "Windows RED checkout is not immutable" unless red_checkout &&
      red_checkout.dig("with", "ref") == "${{ env.TARGET_SHA }}" &&
      red_checkout.dig("with", "fetch-depth") == 2 &&
      red_checkout.dig("with", "persist-credentials") == false
    red_bind = red_steps.find { |step| step["name"] == "Assert exact RED commit and parent" }
    red_bind_text = red_bind && red_bind["run"].to_s
    %w[git\ rev-parse\ HEAD git\ rev-parse\ 'HEAD^' git\ rev-list git\ diff-tree checked-out\ RED\ SHA\ mismatch RED\ parent\ SHA\ mismatch windows.rs].each do |token|
      raise "Windows RED identity assertion missing #{token}" unless red_bind_text&.include?(token.tr("\\", ""))
    end
    red_run = red_steps.find { |step| step["name"] == "Run focused expected-RED contract" }
    raise "repository Windows RED harness is not invoked" unless red_run && red_run["shell"] == "pwsh" &&
      red_run["run"].to_s.include?("./scripts/run-c1b-windows-red.ps1") &&
      %w[-Task -TestSha -ParentSha -Nonce -EvidenceRoot RUNNER_TEMP].all? { |token| red_run["run"].to_s.include?(token) }
    red_upload = red_steps.find { |step| step["uses"] == "actions/upload-artifact@v4" }
    expected_red_name = "c1b-red-${{ inputs.red_task }}-${{ steps.bind-red.outputs.sha }}-${{ inputs.red_nonce }}"
    expected_red_path = "${{ runner.temp }}/c1b-red/c1b-task-${{ inputs.red_task }}-${{ steps.bind-red.outputs.sha }}-${{ inputs.red_nonce }}/"
    raise "Windows RED artifact is not immutable and nonce-bound" unless red_upload && red_upload["if"] == "always()" &&
      red_upload.dig("with", "name") == expected_red_name &&
      red_upload.dig("with", "path") == expected_red_path &&
      red_upload.dig("with", "if-no-files-found") == "error" && red_upload.dig("with", "retention-days") == 30

    merge = "a" * 40
    head = "b" * 40
    dispatch_sha = "c" * 40
    push = "d" * 40
    raise "push selection failed" unless select_target(event_name: "push", github_sha: push) == push
    selected_pr = select_target(event_name: "pull_request", github_sha: merge, pull_request_head_sha: head)
    raise "PR selected synthetic merge SHA" unless selected_pr == head && selected_pr != merge
    raise "dispatch selection failed" unless select_target(event_name: "workflow_dispatch", github_sha: push,
      dispatch_sha: dispatch_sha) == dispatch_sha
    true
  end
end

if $PROGRAM_NAME == __FILE__
  path = ARGV.fetch(0) { abort "usage: validate-c1b-ci.rb WORKFLOW" }
  C1bCiValidator.validate(path)
  puts "c1b-ci-validation=ok"
end
```

`scripts/tests/validate-c1b-ci-test.rb` 完整内容；fixture 全部由 test 在临时目录从 repository workflow 派生，不增加未列出的 fixture files：

```ruby
#!/usr/bin/env ruby
# frozen_string_literal: true

require "open3"
require "tmpdir"
require_relative "../validate-c1b-ci"

ROOT = File.expand_path("../..", __dir__)
WORKFLOW = File.join(ROOT, ".github/workflows/ci.yml")
VALIDATOR = File.join(ROOT, "scripts/validate-c1b-ci.rb")

def assert(condition, message)
  raise message unless condition
end

def run_validator(path)
  Open3.capture3(RbConfig.ruby, VALIDATOR, path)
end

stdout, stderr, status = run_validator(WORKFLOW)
assert(status.success?, "canonical workflow validation failed: #{stdout}#{stderr}")

merge = "a" * 40
head = "b" * 40
push = "c" * 40
dispatch = "d" * 40
assert(C1bCiValidator.select_target(event_name: "push", github_sha: push) == push, "push SHA selection")
assert(C1bCiValidator.select_target(event_name: "pull_request", github_sha: merge,
  pull_request_head_sha: head) == head, "PR head SHA selection")
assert(C1bCiValidator.select_target(event_name: "pull_request", github_sha: merge,
  pull_request_head_sha: head) != merge, "synthetic merge SHA accepted")
assert(C1bCiValidator.select_target(event_name: "workflow_dispatch", github_sha: push,
  dispatch_sha: dispatch) == dispatch, "dispatch SHA selection")

raw = File.read(WORKFLOW)
mutations = {
  "pr-uses-merge" => ["github.event.pull_request.head.sha", "github.sha"],
  "checkout-not-bound" => ["ref: ${{ env.TARGET_SHA }}", "ref: main"],
  "missing-dispatch-input" => ["required: true", "required: false"],
  "receipt-not-always" => ["name: Build exclusive JSON receipt\n        if: always()",
    "name: Build exclusive JSON receipt\n        if: success()"],
  "receipt-id-wrong-runner" => ["runner: ubuntu-24.04", "runner: windows-2022"],
  "receipt-id-wrong-os" => ["expected_os: Linux", "expected_os: Windows"],
  "receipt-id-wrong-arch" => ["expected_arch: X64", "expected_arch: ARM64"],
  "literal-repository-provenance" => ["repository = '${{ github.repository }}'", "repository = 'appergb/OpenTake'"],
  "literal-runner-os-provenance" => ["runner_os = '${{ runner.os }}'", "runner_os = $env:EXPECTED_RUNNER_OS"],
  "runner-label-not-matrix-bound" => ["RUNNER_LABEL: ${{ matrix.runner }}", "RUNNER_LABEL: ubuntu-24.04"],
  "missing-runner-os-guard" => ["if ('${{ runner.os }}' -ne $env:EXPECTED_RUNNER_OS)", "if ($false)"],
  "missing-runner-arch-guard" => ["if ('${{ runner.arch }}' -ne $env:EXPECTED_RUNNER_ARCH)", "if ($false)"],
  "normal-jobs-run-during-red" => ["if: github.event_name != 'workflow_dispatch' || inputs.red_task == 'none'",
    "if: always()"],
  "red-job-wrong-runner" => ["name: Windows expected RED (${{ inputs.red_task }})\n    if: github.event_name == 'workflow_dispatch' && inputs.red_task != 'none'\n    runs-on: windows-2022",
    "name: Windows expected RED (${{ inputs.red_task }})\n    if: github.event_name == 'workflow_dispatch' && inputs.red_task != 'none'\n    runs-on: ubuntu-24.04"],
  "red-harness-not-repository-versioned" => ["./scripts/run-c1b-windows-red.ps1", "./scripts/unreviewed-red.ps1"],
  "red-artifact-not-nonce-bound" => ["-${{ inputs.red_nonce }}", "-fixed"],
  "red-parser-removed" => ["Parse Windows expected-RED harness", "Skip Windows expected-RED harness"],
  "red-concurrency-not-nonce-bound" => ["-${{ inputs.red_nonce || 'none' }}", "-fixed"],
  "red-upload-path-substituted" => ["path: ${{ runner.temp }}/c1b-red/c1b-task-${{ inputs.red_task }}-${{ steps.bind-red.outputs.sha }}-${{ inputs.red_nonce }}/",
    "path: ${{ runner.temp }}/untrusted/"],
  "red-input-guard-weakened" => ["if ($env:RED_NONCE -cnotmatch '^[0-9a-f]{16}$')",
    "if ($false)"],
  "red-changed-path-guard-removed" => ["git diff-tree --no-commit-id --name-only -r HEAD",
    "@('crates/opentake-project/src/safe_fs/windows.rs')"],
}
Dir.mktmpdir("c1b-ci-validator") do |directory|
  mutations.each do |label, (before, after)|
    assert(raw.include?(before), "fixture mutation token missing: #{label}")
    path = File.join(directory, "#{label}.yml")
    File.write(path, raw.sub(before, after))
    _out, _err, result = run_validator(path)
    assert(!result.success?, "validator accepted malformed fixture #{label}")
  end
end

puts "c1b-ci-validator-tests=ok"
```

Task 3 的 commit/evidence/review protocol 是唯一允许序列，不由执行者补写。RED scaffold 精确内容分别是 `abort "unsupported C1B CI schema"` 与 `abort "unsupported C1B evidence schema"`；不得先放入部分 production validator：

```bash
TASK3_RED_DIR="$SAFETY_ROOT/logs/c1b-task-3-red-$(date -u +%Y%m%dT%H%M%SZ)-$(openssl rand -hex 4)"
mkdir "$TASK3_RED_DIR"
TASK3_RED_STARTED_AT=$(date -u +%Y-%m-%dT%H:%M:%SZ)
git add -- \
  scripts/validate-c1b-ci.rb \
  scripts/validate-c1b-evidence.rb \
  scripts/tests/validate-c1b-ci-test.rb \
  scripts/tests/validate-c1b-evidence-test.rb
test "$(git diff --cached --name-only)" = "$(printf '%s\n' \
  scripts/tests/validate-c1b-ci-test.rb \
  scripts/tests/validate-c1b-evidence-test.rb \
  scripts/validate-c1b-ci.rb \
  scripts/validate-c1b-evidence.rb)"
git commit -m 'test(ci): specify immutable C1B receipts and evidence'
TASK3_RED_SHA=$(git rev-parse HEAD)
printf '%s\n' "$TASK3_RED_SHA" >"$TASK3_RED_DIR/red-commit.sha"
set +e
ruby scripts/tests/validate-c1b-ci-test.rb >"$TASK3_RED_DIR/semantic-red.log" 2>&1
TASK3_RED_EXIT=$?
ruby scripts/tests/validate-c1b-evidence-test.rb >"$TASK3_RED_DIR/evidence-red.log" 2>&1
TASK3_EVIDENCE_RED_EXIT=$?
set -e
printf '%s\n' "$TASK3_RED_EXIT" >"$TASK3_RED_DIR/semantic-red.raw-exit"
printf '%s\n' "$TASK3_EVIDENCE_RED_EXIT" >"$TASK3_RED_DIR/evidence-red.raw-exit"
test "$TASK3_RED_EXIT" -ne 0
test "$TASK3_EVIDENCE_RED_EXIT" -ne 0
test "$(grep -Fc 'unsupported C1B CI schema' "$TASK3_RED_DIR/semantic-red.log")" -eq 1
test "$(grep -Fc 'unsupported C1B evidence schema' "$TASK3_RED_DIR/evidence-red.log")" -eq 1
TASK3_RED_FINISHED_AT=$(date -u +%Y-%m-%dT%H:%M:%SZ)
ruby -rjson -e '
  path, sha, started, finished, cwd, semantic_exit, evidence_exit = ARGV
  receipt = {
    "schema" => "opentake-c1b-task3-red-v1", "task" => "3", "attempt" => 1,
    "red_sha" => sha, "cwd" => cwd, "started_at_utc" => started, "finished_at_utc" => finished,
    "commands" => [
      { "command" => "ruby scripts/tests/validate-c1b-ci-test.rb", "exit" => Integer(semantic_exit),
        "expected" => "unsupported C1B CI schema", "log" => "semantic-red.log",
        "raw_exit" => "semantic-red.raw-exit" },
      { "command" => "ruby scripts/tests/validate-c1b-evidence-test.rb", "exit" => Integer(evidence_exit),
        "expected" => "unsupported C1B evidence schema", "log" => "evidence-red.log",
        "raw_exit" => "evidence-red.raw-exit" },
    ],
  }
  File.open(path, File::WRONLY | File::CREAT | File::EXCL, 0o600) do |file|
    file.write(JSON.pretty_generate(receipt) + "\n")
  end
' "$TASK3_RED_DIR/red-receipt.json" "$TASK3_RED_SHA" "$TASK3_RED_STARTED_AT" \
  "$TASK3_RED_FINISHED_AT" "$(pwd -P)" "$TASK3_RED_EXIT" "$TASK3_EVIDENCE_RED_EXIT"
ruby -rjson -e '
  receipt = JSON.parse(File.read(ARGV[0]))
  raise "Task3 RED receipt SHA mismatch" unless receipt.fetch("red_sha") == ARGV[1]
  raise "Task3 RED receipt command mismatch" unless receipt.fetch("commands").length == 2 &&
    receipt.fetch("commands").all? { |row| row.fetch("exit") != 0 }
' "$TASK3_RED_DIR/red-receipt.json" "$TASK3_RED_SHA"
```

RED receipt `red-receipt.json` 必须 exclusive-create，并固定保存：`task="3"`、`attempt>=1`、`red_sha`、上述两个 exact commands/exits/expected messages、UTC start/finish、repo cwd、relative log/raw-exit。`red_sha` 必须等于当前 `HEAD`；两次 failure 必须分别来自 fail-closed scaffold，Ruby parse/name error 不合格。

GREEN 只把两个 validator scaffold替换为本节 CI body与 section 18.8 evidence body，新增 section 16 exact Windows RED harness，并修改 `.github/workflows/ci.yml` 为 section 13 exact YAML；tests不再改动：

```bash
git add -- .github/workflows/ci.yml scripts/run-c1b-windows-red.ps1 scripts/validate-c1b-ci.rb scripts/validate-c1b-evidence.rb
test "$(git diff --cached --name-only)" = "$(printf '%s\n' \
  .github/workflows/ci.yml \
  scripts/run-c1b-windows-red.ps1 \
  scripts/validate-c1b-ci.rb \
  scripts/validate-c1b-evidence.rb)"
git commit -m 'ci: verify C1B receipts and evidence on exact SHAs'
TASK3_GREEN_SHA=$(git rev-parse HEAD)
test "$TASK3_GREEN_SHA" != "$TASK3_RED_SHA"
TASK3_GREEN_DIR="$SAFETY_ROOT/logs/c1b-task-3-$TASK3_GREEN_SHA-attempt-1"
mkdir "$TASK3_GREEN_DIR"
```

随后在 `TASK3_GREEN_DIR` 分别写 `.log` 与 `.raw-exit`：

```bash
ruby scripts/validate-c1b-ci.rb .github/workflows/ci.yml
ruby scripts/tests/validate-c1b-ci-test.rb
ruby scripts/tests/validate-c1b-evidence-test.rb
actionlint .github/workflows/ci.yml
```

四者均为 0；若 `actionlint` 未安装，只能将该项记录为 tool-unavailable，并仍须运行前三项，不能把 YAML parse 当作替代。

Task 3 GREEN 还必须运行 `git diff --check "$TASK3_RED_SHA^".."$TASK3_GREEN_SHA"` 与 `git status --porcelain=v1`（clean），并将 exact command、UTC timestamps、cwd、exit 与 log paths 写入 `command-ledger.json`。随后在 exclusive `TASK3_GREEN_DIR` 生成 `spec-security-review.md` 与 `implementation-review.md`：两名 fresh reviewer 都写 `Task: 3`，绑定完整 `TASK3_GREEN_SHA`，覆盖 RED receipt、workflow exact-SHA semantics、validator mutation fixtures、YAML/Ruby syntax，并各自 `APPROVE/0/0/0` 才可继续。任一 finding 产生新 GREEN commit 和 `attempt-2+` 目录；禁止覆盖 attempt-1。

两份 review 通过后，在同一 exclusive review directory 内生成 Task 4 将消费的固定 manifest：

```bash
for REPORT in "$TASK3_GREEN_DIR/spec-security-review.md" "$TASK3_GREEN_DIR/implementation-review.md"; do
  grep -Eq '^Task:[[:space:]]*3[[:space:]]*$' "$REPORT"
  grep -Eiq "^Commit:[[:space:]]*\`?$TASK3_GREEN_SHA\`?[[:space:]]*$" "$REPORT"
  grep -Eq '^Verdict:[[:space:]]*(\*\*)?APPROVE(\*\*)?[[:space:]]*$' "$REPORT"
done
ruby -rjson -e '
  path, sha = ARGV
  value = { "schema" => "opentake-c1b-reviewed-stage-v1", "task" => "3", "sha" => sha,
    "baseline_sha" => "e67917260ace36e4db1ede4e36eecbc401825bb1" }
  File.open(path, File::WRONLY | File::CREAT | File::EXCL, 0o600) do |file|
    file.write(JSON.pretty_generate(value) + "\n")
  end
' "$TASK3_GREEN_DIR/gate-manifest.json" "$TASK3_GREEN_SHA"
```

### 18.8 Repository-versioned final evidence validator

Task 3 GREEN 把 fail-closed `scripts/validate-c1b-evidence.rb` scaffold替换为下面完整 body。该 body 从 Task 4 起就是每个 GREEN 门禁的同一参数化 validator：参数顺序固定为 `GATE_DIR TASK GREEN_SHA PREDECESSOR_SHA PREDECESSOR_PROOF SPEC_REPORT_REL IMPLEMENTATION_REPORT_REL REPO`。对 Task 4/5/6A/6B/7A/7B/7C，gate 目录必须是 `$SAFETY_ROOT/branch-gates/c1b-task-<TASK>-<GREEN_SHA>-<16-lower-hex-NONCE>/`，`PREDECESSOR_SHA` 分别是 Task 3/4/5/6A/6B/7A/7B 已审 GREEN SHA，`PREDECESSOR_PROOF` 只有 Task 4 使用 Task 3 的安全根内 review-manifest directory；Task 5 起全部使用紧邻前一 task 已通过 validator 的 branch gate，并在每次消费时再次通过 authenticated GitHub API 验证其三份 native receipt。当前 gate 收集 task-bound 两份 review、十个本地 ledger 行和同一 run/attempt 的三份 REST archive。Task 8 只是最终一次应用：它传 `TASK=8`，`FINAL_SHA=PREDECESSOR_SHA=Task 7C GREEN SHA`，`PREDECESSOR_PROOF=Task 7C branch gate`，不修改 validator/test code。下文出现的 `final` 在前置任务门禁中表示“该任务当前不可变 GREEN SHA”，不限定为 Task 8。

同样，本节后的 REST archive protocol 在每个上述 per-task gate 都执行：将 `FINAL_SHA/FINAL_SPEC_REPORT/FINAL_IMPLEMENTATION_REPORT` 分别绑定为当前 `GREEN_SHA`与当前两份 review。不允许中间 task 改用 `native-receipts/.../results.json`、只收集单一 OS，或等到 Task 8 才回填早期 gate。

```ruby
#!/usr/bin/env ruby
# frozen_string_literal: true

require "json"
require "open3"
require "pathname"
require "digest"
require "base64"
require "time"

SHA = /\A[0-9a-f]{40}\z/
C1B_BASELINE_SHA = "e67917260ace36e4db1ede4e36eecbc401825bb1"
EXPECTED_TASKS = %w[4 5 6a 6b 7a 7b 7c 8].freeze
PREVIOUS_TASK = { "4" => "3", "5" => "4", "6a" => "5", "6b" => "6a", "7a" => "6b",
  "7b" => "7a", "7c" => "7b", "8" => "7c" }.freeze
PREDECESSOR_SUBJECT = {
  "4" => "ci: verify C1B receipts and evidence on exact SHAs",
  "5" => "feat(project): add Unix recursive filesystem authorities",
  "6a" => "feat(project): add Unix consuming quarantine cleanup",
  "6b" => "feat(project): add fail-closed Windows platform scaffold",
  "7a" => "feat(project): capture Windows filesystem capabilities",
  "7b" => "feat(project): enforce Windows owner-only creation",
  "7c" => "feat(project): add Windows capability-relative rename",
  "8" => "feat(project): add Windows retained-handle cleanup",
}.freeze
SINGLE_GREEN_SUBJECTS = {
  "6a" => "feat(project): add fail-closed Windows platform scaffold",
}.freeze
EXPECTED_SLICE_SUBJECTS = {
  "4" => ["test(project): specify Unix recursive filesystem authorities",
    "feat(project): add Unix recursive filesystem authorities"],
  "5" => ["test(project): specify Unix quarantine and recursive cleanup",
    "feat(project): add Unix consuming quarantine cleanup"],
  "6b" => ["test(project): specify Windows capability acquisition and io",
    "feat(project): capture Windows filesystem capabilities"],
  "7a" => ["test(project): specify Windows owner-only creation and rollback",
    "feat(project): enforce Windows owner-only creation"],
  "7b" => ["test(project): specify Windows retained quarantine and publish",
    "feat(project): add Windows capability-relative rename"],
  "7c" => ["test(project): specify Windows retained cleanup and delete",
    "feat(project): add Windows retained-handle cleanup"],
}.freeze
EXPECTED_IDS = %w[linux-x86_64 macos-native windows-x86_64].freeze
EXPECTED_PROVENANCE = {
  "linux-x86_64" => { "runner_os" => "Linux", "runner_label" => "ubuntu-24.04", "runner_arch" => "X64" },
  "macos-native" => { "runner_os" => "macOS", "runner_label" => "macos-14", "runner_arch" => "ARM64" },
  "windows-x86_64" => { "runner_os" => "Windows", "runner_label" => "windows-2022", "runner_arch" => "X64" },
}.freeze
EXPECTED_REPOSITORY = "appergb/OpenTake"
EXPECTED_WORKFLOW = "CI"
EXPECTED_WORKFLOW_FILE = ".github/workflows/ci.yml"
EXPECTED_JOB_ID = "safe-filesystem"
EXPECTED_EVENTS = %w[push pull_request workflow_dispatch].freeze
GITHUB_API_VERSION = "2026-03-10"
LOCAL_COMMANDS = {
  "cargo-fmt" => "cargo fmt --all --check",
  "cargo-clippy" => "cargo clippy --workspace --all-targets -- -D warnings",
  "cargo-test-workspace" => "cargo test --workspace",
  "tauri-no-default-check" => "cargo check -p opentake-tauri --no-default-features --all-targets",
  "bundle-export-surface" => "cargo test -p opentake-tauri --test bundle_export_surface -- --test-threads=1",
  "archive-security" => "cargo test -p opentake-project --test archive_security -- --test-threads=1",
  "check-macos" => "cargo check -p opentake-project --lib --tests --target aarch64-apple-darwin",
  "check-linux" => "cargo check -p opentake-project --lib --tests --target x86_64-unknown-linux-gnu",
  "check-windows" => "cargo check -p opentake-project --lib --tests --target x86_64-pc-windows-msvc",
  "git-diff-check" => "git diff --check",
}.freeze
NATIVE_COMMANDS = {
  "cargo-fmt" => "cargo fmt --all --check",
  "cargo-clippy" => "cargo clippy -p opentake-project --lib --tests -- -D warnings",
  "safe-fs-unit" => "cargo test -p opentake-project --lib safe_fs -- --test-threads=1",
  "archive-security" => "cargo test -p opentake-project --test archive_security -- --test-threads=1",
}.freeze

gate, task, expected_sha, predecessor_sha, predecessor_proof, spec_report_relative,
  implementation_report_relative, repo = ARGV
abort "usage: validate-c1b-evidence.rb GATE_DIR TASK SHA PREDECESSOR_SHA PREDECESSOR_PROOF SPEC_REPORT_REL IMPLEMENTATION_REPORT_REL REPO" unless repo
raise "unsupported C1B gate task" unless EXPECTED_TASKS.include?(task)
expected_sha = expected_sha.downcase
predecessor_sha = predecessor_sha.downcase
raise "final SHA must be lowercase 40-hex" unless SHA.match?(expected_sha)
raise "predecessor SHA must be lowercase 40-hex" unless SHA.match?(predecessor_sha)
raise "gate directory missing" unless Dir.exist?(gate)
gate = File.realpath(gate)
repo = File.realpath(repo)
predecessor_proof = File.realpath(predecessor_proof)
raise "gate must be stored directly below branch-gates" unless File.basename(File.dirname(gate)) == "branch-gates"
safety_root = File.dirname(File.dirname(gate))
safety_prefix = safety_root.end_with?(File::SEPARATOR) ? safety_root : safety_root + File::SEPARATOR
raise "predecessor proof escapes safety root" unless predecessor_proof.start_with?(safety_prefix)
nonce = "[0-9a-f]{16}"
expected_gate = if task == "8"
  /\Ac1b-\d{8}T\d{6}Z-#{Regexp.escape(expected_sha)}-#{nonce}\z/
else
  /\Ac1b-task-#{Regexp.escape(task)}-#{Regexp.escape(expected_sha)}-#{nonce}\z/
end
raise "gate basename does not bind task/SHA/nonce" unless File.basename(gate).match?(expected_gate)

def confined_file!(root, relative, label)
  raise "#{label} path must be relative" if Pathname.new(relative).absolute?
  candidate = File.expand_path(relative, root)
  prefix = root.end_with?(File::SEPARATOR) ? root : root + File::SEPARATOR
  raise "#{label} escapes gate" unless candidate.start_with?(prefix)
  raise "#{label} missing" unless File.file?(candidate)
  raise "#{label} resolves outside gate" unless File.realpath(candidate).start_with?(prefix)
  candidate
end

binding_path = confined_file!(gate, "predecessor-binding.json", "predecessor binding")
binding = JSON.parse(File.read(binding_path))
raise "predecessor binding mismatch" unless binding == {
  "schema" => "opentake-c1b-predecessor-binding-v1", "task" => task,
  "predecessor_task" => PREVIOUS_TASK.fetch(task), "predecessor_sha" => predecessor_sha,
  "predecessor_proof" => predecessor_proof,
}

def gh_json!(endpoint, label)
  stdout, stderr, status = Open3.capture3("gh", "api", "--hostname", "github.com",
    "-H", "X-GitHub-Api-Version: #{GITHUB_API_VERSION}", endpoint)
  raise "#{label} GitHub API failed: #{stderr.strip}" unless status.success?
  JSON.parse(stdout)
rescue Errno::ENOENT
  raise "authenticated gh CLI is required"
rescue JSON::ParserError
  raise "#{label} GitHub API returned invalid JSON"
end

def gh_authenticated!
  _stdout, stderr, status = Open3.capture3("gh", "auth", "status", "--hostname", "github.com")
  raise "gh is not authenticated for github.com: #{stderr.strip}" unless status.success?
rescue Errno::ENOENT
  raise "authenticated gh CLI is required"
end

def timestamp!(value, label)
  parsed = Time.iso8601(value)
  raise "#{label} must be UTC" unless parsed.utc_offset.zero?
  parsed
rescue ArgumentError
  raise "#{label} is not RFC3339"
end

def approved_report!(path, role, task, sha)
  body = File.read(path)
  raise "#{role} report role mismatch" unless body.match?(/^Role:\s*.*#{Regexp.escape(role)}/i)
  raise "#{role} report task mismatch" unless body.match?(/^Task:\s*#{Regexp.escape(task)}\s*$/i)
  raise "#{role} report commit mismatch" unless body.match?(/^Commit:\s*`?#{sha}`?\s*$/i)
  raise "#{role} report not approved" unless body.match?(/^Verdict:\s*\*\*?APPROVE\*\*?\s*$/i) ||
    body.match?(/^Verdict:\s*APPROVE\s*$/i)
  %w[Critical Important Minor].each do |severity|
    raise "#{role} report has #{severity} findings" unless
      body.match?(/^#{severity}:\s*\*\*?0\*\*?\s*$/i) || body.match?(/^#{severity}:\s*0\s*$/i)
  end
end

gh_authenticated!
head, head_status = Open3.capture2("git", "-C", repo, "rev-parse", "HEAD")
raise "cannot read repository HEAD" unless head_status.success?
raise "repository HEAD mismatch" unless head.strip.downcase == expected_sha
[C1B_BASELINE_SHA, predecessor_sha, expected_sha].each do |sha|
  _out, _err, object_status = Open3.capture3("git", "-C", repo, "cat-file", "-e", "#{sha}^{commit}")
  raise "required gate commit is missing: #{sha}" unless object_status.success?
end
_out, _err, baseline_status = Open3.capture3("git", "-C", repo, "merge-base", "--is-ancestor",
  C1B_BASELINE_SHA, predecessor_sha)
raise "predecessor is outside the frozen C1B baseline" unless baseline_status.success?
_out, _err, predecessor_status = Open3.capture3("git", "-C", repo, "merge-base", "--is-ancestor",
  predecessor_sha, expected_sha)
raise "gate SHA does not descend from predecessor" unless predecessor_status.success?
predecessor_subject, predecessor_subject_status = Open3.capture2("git", "-C", repo, "show", "-s",
  "--format=%s", predecessor_sha)
raise "cannot resolve predecessor subject" unless predecessor_subject_status.success?
raise "predecessor is not the required previous stage" unless predecessor_subject.strip == PREDECESSOR_SUBJECT.fetch(task)
if task == "8"
  raise "Task 8 must validate the unchanged Task 7C GREEN SHA" unless predecessor_sha == expected_sha
else
  raise "implementation task gate must advance beyond predecessor" if predecessor_sha == expected_sha
  chain, chain_status = Open3.capture2("git", "-C", repo, "rev-list", "--reverse", "--first-parent",
    "#{predecessor_sha}..#{expected_sha}")
  raise "cannot resolve task commit chain" unless chain_status.success?
  commits = chain.lines.map(&:strip).reject(&:empty?)
  minimum = task == "6a" ? 1 : 2
  raise "task chain is shorter than its contract" unless commits.length >= minimum
  commits.each do |commit|
    row, row_status = Open3.capture2("git", "-C", repo, "rev-list", "--parents", "-n", "1", commit)
    raise "cannot resolve task commit parent" unless row_status.success?
    raise "task chain commit must have exactly one parent" unless row.split.length == 2
  end
  first_parent, first_parent_status = Open3.capture2("git", "-C", repo, "rev-parse", "#{commits.first}^")
  raise "cannot resolve first task parent" unless first_parent_status.success?
  raise "task predecessor must be the exact first task parent" unless first_parent.strip.downcase == predecessor_sha
  subjects = commits.map do |commit|
    subject, subject_status = Open3.capture2("git", "-C", repo, "show", "-s", "--format=%s", commit)
    raise "cannot resolve task commit subject" unless subject_status.success?
    subject.strip
  end
  if task == "6a"
    raise "Task 6A GREEN/correction subjects mismatch" unless
      subjects.all? { |subject| subject == SINGLE_GREEN_SUBJECTS.fetch(task) }
  else
    red_subject, green_subject = EXPECTED_SLICE_SUBJECTS.fetch(task)
    first_green = subjects.index(green_subject)
    raise "task chain lacks GREEN commit" unless first_green && first_green.positive?
    raise "task RED commit subjects mismatch" unless subjects[0...first_green].all? { |subject| subject == red_subject }
    raise "task GREEN/correction subjects mismatch" unless subjects[first_green..].all? { |subject| subject == green_subject }
  end
end

previous_task = PREVIOUS_TASK.fetch(task)
predecessor_gate_to_revalidate = nil
if previous_task == "3"
  expected_name = /\Ac1b-task-#{Regexp.escape(previous_task)}-#{Regexp.escape(predecessor_sha)}-attempt-[1-9][0-9]*\z/
  raise "predecessor review directory identity mismatch" unless File.basename(predecessor_proof).match?(expected_name)
  manifest_path = confined_file!(predecessor_proof, "gate-manifest.json", "predecessor manifest")
  manifest = JSON.parse(File.read(manifest_path))
  raise "predecessor manifest mismatch" unless manifest == {
    "schema" => "opentake-c1b-reviewed-stage-v1", "task" => previous_task,
    "sha" => predecessor_sha, "baseline_sha" => C1B_BASELINE_SHA,
  }
  approved_report!(confined_file!(predecessor_proof, "spec-security-review.md", "predecessor spec report"),
    "spec-security", previous_task, predecessor_sha)
  approved_report!(confined_file!(predecessor_proof, "implementation-review.md", "predecessor implementation report"),
    "implementation", previous_task, predecessor_sha)
else
  expected_name = /\Ac1b-task-#{Regexp.escape(previous_task)}-#{Regexp.escape(predecessor_sha)}-[0-9a-f]{16}\z/
  raise "predecessor gate identity mismatch" unless File.basename(predecessor_proof).match?(expected_name)
  raw = confined_file!(predecessor_proof, "results-validation.raw-exit", "predecessor validator exit")
  raise "predecessor gate validator did not pass" unless File.read(raw).strip == "0"
  validation_log = File.read(confined_file!(predecessor_proof, "results-validation.log", "predecessor validator log"))
  raise "predecessor validator success identity mismatch" unless
    validation_log.match?(/^c1b-evidence-validation=ok task=#{Regexp.escape(previous_task)} predecessor=[0-9a-f]{40} sha=#{predecessor_sha}$/)
  previous_results = File.read(confined_file!(predecessor_proof, "results.md", "predecessor results"))
  raise "predecessor results task mismatch" unless previous_results.match?(/^Task:\s*#{Regexp.escape(previous_task)}$/)
  raise "predecessor results SHA mismatch" unless previous_results.match?(/^Final SHA:\s*#{predecessor_sha}$/)
  raise "predecessor results aggregate mismatch" unless previous_results.match?(/^Aggregate:\s*0$/)
  approved_report!(confined_file!(predecessor_proof, "reviews/spec-security-review.md", "predecessor spec report"),
    "spec-security", previous_task, predecessor_sha)
  approved_report!(confined_file!(predecessor_proof, "reviews/implementation-review.md", "predecessor implementation report"),
    "implementation", previous_task, predecessor_sha)
  predecessor_gate_to_revalidate = predecessor_proof
end
status, status_result = Open3.capture2("git", "-C", repo, "status", "--porcelain=v1")
raise "cannot read repository status" unless status_result.success?
raise "repository is not clean" unless status.empty?

ledger_path = confined_file!(gate, "command-ledger.json", "command ledger")
ledger = JSON.parse(File.read(ledger_path))
raise "command ledger must contain exactly ten rows" unless ledger.is_a?(Array) && ledger.length == LOCAL_COMMANDS.length
ledger_ids = ledger.map { |row| row.fetch("id") }
raise "local command id/order mismatch" unless ledger_ids == LOCAL_COMMANDS.keys
ledger.each do |row|
  id = row.fetch("id")
  raise "local command substituted: #{id}" unless row.fetch("command") == LOCAL_COMMANDS.fetch(id)
  raise "local command exit is nonzero: #{id}" unless row.fetch("exit_code") == 0
  raise "local command cwd mismatch" unless row.fetch("cwd") == repo
  started = timestamp!(row.fetch("started_at_utc"), "#{id} started_at_utc")
  finished = timestamp!(row.fetch("finished_at_utc"), "#{id} finished_at_utc")
  raise "local command timestamps reversed: #{id}" if finished < started
  raise "local log name mismatch: #{id}" unless row.fetch("log") == "#{id}.log"
  raise "local raw-exit name mismatch: #{id}" unless row.fetch("raw_exit") == "#{id}.raw-exit"
  confined_file!(gate, row.fetch("log"), "#{id} log")
  raw_exit = confined_file!(gate, row.fetch("raw_exit"), "#{id} raw exit")
  raise "raw exit mismatch for #{id}" unless File.read(raw_exit).strip == "0"
end

%w[pre-status.txt post-status.txt].each do |status_name|
  status_path = confined_file!(gate, status_name, status_name)
  raise "#{status_name} is not clean" unless File.read(status_path).empty?
end

def validate_native_receipts!(gate, expected_sha, repo)
receipt_paths = Dir.glob(File.join(gate, "native-receipts", "*", "*", "receipt.json"))
raise "expected exactly three native receipts" unless receipt_paths.length == 3
receipts = receipt_paths.map do |path|
  relative = Pathname.new(path).relative_path_from(Pathname.new(gate)).to_s
  confined = confined_file!(gate, relative, "native receipt")
  [confined, JSON.parse(File.read(confined))]
end
ids = receipts.map { |_path, receipt| receipt.fetch("receipt_id") }
raise "duplicate or missing native receipt id" unless ids.sort == EXPECTED_IDS.sort && ids.uniq.length == ids.length
seen_run_ids = []
seen_run_attempts = []
seen_job_ids = []
seen_artifact_ids = []
remote_rows = {}
receipts.each do |path, receipt|
  raise "receipt schema mismatch: #{path}" unless receipt.fetch("schema") == "opentake-c1b-native-receipt-v1"
  receipt_id = receipt.fetch("receipt_id")
  provenance = EXPECTED_PROVENANCE.fetch(receipt_id)
  provenance.each do |field, expected|
    raise "receipt #{field} mismatch for #{receipt_id}: #{path}" unless receipt.fetch(field) == expected
  end
  raise "receipt repository mismatch: #{path}" unless receipt.fetch("repository") == EXPECTED_REPOSITORY
  raise "receipt workflow mismatch: #{path}" unless receipt.fetch("workflow") == EXPECTED_WORKFLOW
  raise "receipt workflow_file mismatch: #{path}" unless receipt.fetch("workflow_file") == EXPECTED_WORKFLOW_FILE
  raise "receipt job_id mismatch: #{path}" unless receipt.fetch("job_id") == EXPECTED_JOB_ID
  raise "receipt event_name mismatch: #{path}" unless EXPECTED_EVENTS.include?(receipt.fetch("event_name"))
  requested = receipt.fetch("requested_sha").downcase
  checked = receipt.fetch("checked_out_sha").downcase
  raise "receipt SHA malformed: #{path}" unless SHA.match?(requested) && SHA.match?(checked)
  raise "receipt SHA mismatch: #{path}" unless requested == checked && checked == expected_sha
  run_id = receipt.fetch("run_id").to_s
  run_attempt = receipt.fetch("run_attempt").to_s
  raise "receipt run_id malformed: #{path}" unless run_id.match?(/\A[1-9][0-9]*\z/)
  raise "receipt run_attempt malformed: #{path}" unless run_attempt.match?(/\A[1-9][0-9]*\z/)
  seen_run_ids << run_id
  seen_run_attempts << run_attempt
  expected_receipt_path = File.join(gate, "native-receipts", run_id, receipt_id, "receipt.json")
  raise "receipt path/run_id mismatch: #{path}" unless File.realpath(path) == File.realpath(expected_receipt_path)
  receipt_root = Pathname.new(File.dirname(path)).relative_path_from(Pathname.new(gate)).to_s
  run_path = confined_file!(gate, File.join(receipt_root, "run.json"), "#{receipt_id} run metadata")
  jobs_path = confined_file!(gate, File.join(receipt_root, "jobs.json"), "#{receipt_id} jobs metadata")
  artifact_path = confined_file!(gate, File.join(receipt_root, "artifact.json"), "#{receipt_id} artifact metadata")
  zip_path = confined_file!(gate, File.join(receipt_root, "artifact.zip"), "#{receipt_id} artifact archive")

  run_endpoint = "/repos/#{EXPECTED_REPOSITORY}/actions/runs/#{run_id}"
  jobs_endpoint = "#{run_endpoint}/jobs?per_page=100"
  artifacts_endpoint = "#{run_endpoint}/artifacts?per_page=100"
  live_run = gh_json!(run_endpoint, "#{receipt_id} run")
  live_jobs = gh_json!(jobs_endpoint, "#{receipt_id} jobs")
  live_artifacts = gh_json!(artifacts_endpoint, "#{receipt_id} artifacts")
  raise "jobs API pagination is incomplete" unless
    live_jobs.fetch("total_count") == live_jobs.fetch("jobs").length && live_jobs.fetch("total_count") <= 100
  raise "artifacts API pagination is incomplete" unless
    live_artifacts.fetch("total_count") == live_artifacts.fetch("artifacts").length &&
      live_artifacts.fetch("total_count") <= 100
  raise "saved run metadata differs from authenticated API" unless JSON.parse(File.read(run_path)) == live_run
  raise "saved jobs metadata differs from authenticated API" unless JSON.parse(File.read(jobs_path)) == live_jobs

  raise "run id mismatch" unless live_run.fetch("id").to_s == run_id
  raise "run attempt mismatch" unless live_run.fetch("run_attempt").to_s == run_attempt
  raise "run repository mismatch" unless live_run.dig("repository", "full_name") == EXPECTED_REPOSITORY
  raise "run workflow mismatch" unless live_run.fetch("name") == EXPECTED_WORKFLOW
  run_path = live_run.fetch("path")
  raise "run workflow path mismatch" unless run_path.split("@", 2).first == EXPECTED_WORKFLOW_FILE
  event_name = live_run.fetch("event")
  raise "run event mismatch" unless event_name == receipt.fetch("event_name")
  raise "run is not successful and completed" unless
    live_run.fetch("status") == "completed" && live_run.fetch("conclusion") == "success"
  run_identity_sha = live_run.fetch("head_sha").downcase
  raise "run identity SHA malformed" unless SHA.match?(run_identity_sha)
  case event_name
  when "push"
    raise "push run head SHA mismatch" unless run_identity_sha == expected_sha
  when "pull_request"
    pull_request_heads = live_run.fetch("pull_requests").map { |pr| pr.dig("head", "sha")&.downcase }.compact
    pr_head_bound = run_identity_sha == expected_sha || pull_request_heads.include?(expected_sha)
    unless pr_head_bound
      merge_commit = gh_json!("/repos/#{EXPECTED_REPOSITORY}/commits/#{run_identity_sha}", "PR merge commit")
      parent_shas = merge_commit.fetch("parents").map { |parent| parent.fetch("sha").downcase }
      pr_head_bound = parent_shas.include?(expected_sha)
    end
    raise "PR run is not bound to the checked-out head SHA" unless pr_head_bound
  when "workflow_dispatch"
    # Dispatch run identity is the workflow ref, while TARGET_SHA is the explicit immutable input.
    # Job/artifact metadata bind to run_identity_sha; the archive receipt binds checkout to expected_sha.
    raise "workflow_dispatch must run from main" unless live_run.fetch("head_branch") == "main"
  else
    raise "unsupported run event #{event_name.inspect}"
  end
  workflow_endpoint = "/repos/#{EXPECTED_REPOSITORY}/contents/#{EXPECTED_WORKFLOW_FILE}?ref=#{run_identity_sha}"
  workflow_content = gh_json!(workflow_endpoint, "run workflow content")
  remote_workflow = Base64.strict_decode64(workflow_content.fetch("content").delete("\n"))
  local_workflow_path = File.join(repo, EXPECTED_WORKFLOW_FILE)
  raise "local final workflow missing" unless File.file?(local_workflow_path)
  raise "run workflow bytes differ from final reviewed workflow" unless
    remote_workflow == File.binread(local_workflow_path)

  expected_job_name = "Safe filesystem (#{receipt_id})"
  matching_jobs = live_jobs.fetch("jobs").select { |job| job.fetch("name") == expected_job_name }
  raise "expected exactly one native job #{expected_job_name}" unless matching_jobs.length == 1
  job = matching_jobs.fetch(0)
  raise "native job run mismatch" unless job.fetch("run_id").to_s == run_id
  raise "native job run-identity SHA mismatch" unless job.fetch("head_sha").downcase == run_identity_sha
  raise "native job not successful and completed" unless
    job.fetch("status") == "completed" && job.fetch("conclusion") == "success"
  seen_job_ids << job.fetch("id").to_s

  expected_artifact_name = "c1b-native-#{receipt_id}-#{expected_sha}"
  matching_artifacts = live_artifacts.fetch("artifacts").select do |artifact|
    artifact.fetch("name") == expected_artifact_name
  end
  raise "expected exactly one SHA-bound artifact #{expected_artifact_name}" unless matching_artifacts.length == 1
  artifact = matching_artifacts.fetch(0)
  raise "saved artifact metadata differs from authenticated API" unless JSON.parse(File.read(artifact_path)) == artifact
  raise "artifact expired" if artifact.fetch("expired")
  raise "artifact workflow run id mismatch" unless artifact.dig("workflow_run", "id").to_s == run_id
  raise "artifact workflow run-identity SHA mismatch" unless
    artifact.dig("workflow_run", "head_sha").downcase == run_identity_sha
  digest = artifact.fetch("digest")
  raise "artifact digest malformed" unless digest.match?(/\Asha256:[0-9a-f]{64}\z/)
  actual_digest = "sha256:#{Digest::SHA256.file(zip_path).hexdigest}"
  raise "artifact archive digest mismatch" unless actual_digest == digest
  seen_artifact_ids << artifact.fetch("id").to_s
  remote_rows[receipt_id] = {
    "job_id" => job.fetch("id").to_s,
    "artifact_id" => artifact.fetch("id").to_s,
    "artifact_name" => artifact.fetch("name"),
    "artifact_digest" => digest,
    "run_identity_sha" => run_identity_sha,
  }

  archived_receipt, unzip_error, unzip_status = Open3.capture3("unzip", "-p", zip_path, "receipt.json")
  raise "cannot read receipt.json from retained artifact archive: #{unzip_error.strip}" unless unzip_status.success?
  raise "extracted receipt differs from retained REST archive" unless JSON.parse(archived_receipt) == receipt
  commands = receipt.fetch("commands")
  command_ids = commands.map { |command| command.fetch("id") }
  raise "receipt command set/order mismatch: #{path}" unless command_ids == NATIVE_COMMANDS.keys
  commands.each do |command|
    id = command.fetch("id")
    raise "native command substituted: #{path}:#{id}" unless command.fetch("command") == NATIVE_COMMANDS.fetch(id)
    raise "native command failed: #{path}:#{id}" unless command.fetch("exit_code") == 0
    receipt_dir = File.dirname(path)
    raise "native log name mismatch: #{id}" unless command.fetch("log") == "#{id}.log"
    raise "native raw-exit name mismatch: #{id}" unless command.fetch("raw_exit") == "#{id}.raw-exit"
    log = confined_file!(gate, File.join(Pathname.new(receipt_dir).relative_path_from(Pathname.new(gate)).to_s,
      command.fetch("log")), "native #{id} log")
    raw_exit = confined_file!(gate, File.join(Pathname.new(receipt_dir).relative_path_from(Pathname.new(gate)).to_s,
      command.fetch("raw_exit")), "native #{id} raw exit")
    raise "native raw exit mismatch: #{raw_exit}" unless File.read(raw_exit).strip == "0"
  end
  raise "native aggregate failed: #{path}" unless receipt.fetch("aggregate_exit") == 0
  aggregate = confined_file!(gate,
    File.join(Pathname.new(File.dirname(path)).relative_path_from(Pathname.new(gate)).to_s,
      "final-aggregate.raw-exit"), "native aggregate")
  raise "native aggregate raw exit mismatch: #{path}" unless File.read(aggregate).strip == "0"
end
raise "native receipts do not belong to one workflow run" unless seen_run_ids.uniq.length == 1
raise "native receipts do not belong to one workflow attempt" unless seen_run_attempts.uniq.length == 1
raise "duplicate workflow job ids" unless seen_job_ids.uniq.length == seen_job_ids.length
raise "duplicate artifact ids" unless seen_artifact_ids.uniq.length == seen_artifact_ids.length
[receipts, remote_rows]
end

validate_native_receipts!(predecessor_gate_to_revalidate, predecessor_sha, repo) if predecessor_gate_to_revalidate
receipts, remote_rows = validate_native_receipts!(gate, expected_sha, repo)

{
  "spec-security" => spec_report_relative,
  "implementation" => implementation_report_relative,
}.each do |role, relative|
  path = confined_file!(gate, relative, "#{role} report")
  body = File.read(path)
  raise "#{role} report role mismatch" unless body.match?(/^Role:\s*.*#{Regexp.escape(role)}/i)
  raise "#{role} report task mismatch" unless body.match?(/^Task:\s*#{Regexp.escape(task)}\s*$/i)
  raise "#{role} report commit mismatch" unless body.match?(/^Commit:\s*`?#{expected_sha}`?\s*$/i)
  raise "#{role} report not approved" unless body.match?(/^Verdict:\s*\*\*?APPROVE\*\*?\s*$/i) || body.match?(/^Verdict:\s*APPROVE\s*$/i)
  %w[Critical Important Minor].each do |severity|
    raise "#{role} report has #{severity} findings" unless body.match?(/^#{severity}:\s*\*\*?0\*\*?\s*$/i) || body.match?(/^#{severity}:\s*0\s*$/i)
  end
end

results_path = confined_file!(gate, "results.md", "results")
results = File.read(results_path)
raise "results task mismatch" unless results.match?(/^Task:\s*#{Regexp.escape(task)}$/)
raise "results baseline SHA mismatch" unless results.match?(/^Baseline SHA:\s*#{C1B_BASELINE_SHA}$/)
raise "results predecessor SHA mismatch" unless results.match?(/^Predecessor SHA:\s*#{predecessor_sha}$/)
raise "results missing final SHA" unless results.match?(/^Final SHA:\s*#{expected_sha}$/)
raise "results missing clean pre-status" unless results.match?(/^Pre-status:\s*clean$/)
raise "results missing clean post-status" unless results.match?(/^Post-status:\s*clean$/)
LOCAL_COMMANDS.each do |id, command|
  row = "| #{id} | #{command} | 0 |"
  raise "results missing exact local row #{id}" unless results.include?(row)
end
receipts.each do |_path, receipt|
  remote = remote_rows.fetch(receipt.fetch("receipt_id"))
  row = "| #{receipt.fetch('receipt_id')} | #{receipt.fetch('run_id')} | #{receipt.fetch('run_attempt')} | " \
    "#{remote.fetch('job_id')} | #{remote.fetch('artifact_id')} | #{remote.fetch('artifact_name')} | " \
    "#{remote.fetch('artifact_digest')} | #{expected_sha} | 0 |"
  raise "results missing exact native row #{receipt.fetch('receipt_id')}" unless results.include?(row)
end
raise "results missing spec report path" unless results.include?(spec_report_relative)
raise "results missing implementation report path" unless results.include?(implementation_report_relative)
raise "results missing aggregate" unless results.match?(/^Aggregate:\s*0$/)

puts "c1b-evidence-validation=ok task=#{task} predecessor=#{predecessor_sha} sha=#{expected_sha}"
```

Task 3 test-only commit 同时加入 `scripts/tests/validate-c1b-evidence-test.rb`。该 test 不依赖 preexisting final gate；它在 `Dir.mktmpdir` 中从当前 clean repository HEAD 构造完整 canonical synthetic gate，再派生每个 mutation：

```ruby
#!/usr/bin/env ruby
# frozen_string_literal: true

require "fileutils"
require "json"
require "open3"
require "rbconfig"
require "digest"
require "base64"
require "tmpdir"

validator = File.expand_path("../validate-c1b-evidence.rb", __dir__)
repo = File.expand_path("../..", __dir__)
sha, sha_status = Open3.capture2("git", "-C", repo, "rev-parse", "HEAD")
raise "cannot resolve test HEAD" unless sha_status.success?
sha = sha.strip.downcase
task = "4"
baseline = "e67917260ace36e4db1ede4e36eecbc401825bb1"
predecessor = sha

LOCAL_COMMANDS = {
  "cargo-fmt" => "cargo fmt --all --check",
  "cargo-clippy" => "cargo clippy --workspace --all-targets -- -D warnings",
  "cargo-test-workspace" => "cargo test --workspace",
  "tauri-no-default-check" => "cargo check -p opentake-tauri --no-default-features --all-targets",
  "bundle-export-surface" => "cargo test -p opentake-tauri --test bundle_export_surface -- --test-threads=1",
  "archive-security" => "cargo test -p opentake-project --test archive_security -- --test-threads=1",
  "check-macos" => "cargo check -p opentake-project --lib --tests --target aarch64-apple-darwin",
  "check-linux" => "cargo check -p opentake-project --lib --tests --target x86_64-unknown-linux-gnu",
  "check-windows" => "cargo check -p opentake-project --lib --tests --target x86_64-pc-windows-msvc",
  "git-diff-check" => "git diff --check",
}.freeze
NATIVE_COMMANDS = {
  "cargo-fmt" => "cargo fmt --all --check",
  "cargo-clippy" => "cargo clippy -p opentake-project --lib --tests -- -D warnings",
  "safe-fs-unit" => "cargo test -p opentake-project --lib safe_fs -- --test-threads=1",
  "archive-security" => "cargo test -p opentake-project --test archive_security -- --test-threads=1",
}.freeze
RECEIPT_IDS = %w[linux-x86_64 macos-native windows-x86_64].freeze
PROVENANCE = {
  "linux-x86_64" => { "runner_os" => "Linux", "runner_label" => "ubuntu-24.04", "runner_arch" => "X64" },
  "macos-native" => { "runner_os" => "macOS", "runner_label" => "macos-14", "runner_arch" => "ARM64" },
  "windows-x86_64" => { "runner_os" => "Windows", "runner_label" => "windows-2022", "runner_arch" => "X64" },
}.freeze

def run_validator(validator, gate, task, sha, predecessor, predecessor_proof,
  spec, implementation, repo, fake_bin:, fixture:)
  env = {
    "PATH" => "#{fake_bin}#{File::PATH_SEPARATOR}#{ENV.fetch('PATH', '')}",
    "C1B_FAKE_GH_FIXTURE_ROOT" => File.dirname(fixture),
  }
  Open3.capture3(env, RbConfig.ruby, validator, gate, task, sha, predecessor, predecessor_proof,
    spec, implementation, repo)
end

def install_fake_gh(root)
  bin = File.join(root, "fake-bin")
  FileUtils.mkdir_p(bin)
  script = File.join(bin, "gh")
  File.write(script, <<~'SH')
    #!/bin/sh
    set -eu
    fixture_root=${C1B_FAKE_GH_FIXTURE_ROOT:?}
    if [ "$1" = auth ] && [ "$2" = status ] && [ "$3" = --hostname ] && [ "$4" = github.com ]; then
      exit 0
    fi
    [ "$1" = api ] && [ "$2" = --hostname ] && [ "$3" = github.com ]
    shift 3
    endpoint=
    while [ "$#" -gt 0 ]; do endpoint=$1; shift; done
    fixture_for_run() {
      wanted=$1
      for candidate in "$fixture_root"/*-gh-fixture; do
        [ -d "$candidate" ] || continue
        [ "$(cat "$candidate/run-id.txt")" = "$wanted" ] && { printf '%s\n' "$candidate"; return 0; }
      done
      return 1
    }
    fixture_for_sha() {
      wanted=$1
      for candidate in "$fixture_root"/*-gh-fixture; do
        [ -d "$candidate" ] || continue
        [ "$(cat "$candidate/run-sha.txt")" = "$wanted" ] && { printf '%s\n' "$candidate"; return 0; }
      done
      return 1
    }
    case "$endpoint" in
      /repos/appergb/OpenTake/actions/runs/*/jobs?per_page=100)
        id=$(printf '%s' "$endpoint" | sed -E 's#.*/runs/([0-9]+)/jobs.*#\1#')
        fixture=$(fixture_for_run "$id") && cat "$fixture/jobs.json"
        ;;
      /repos/appergb/OpenTake/actions/runs/*/artifacts?per_page=100)
        id=$(printf '%s' "$endpoint" | sed -E 's#.*/runs/([0-9]+)/artifacts.*#\1#')
        fixture=$(fixture_for_run "$id") && cat "$fixture/artifacts.json"
        ;;
      /repos/appergb/OpenTake/actions/runs/*)
        id=$(printf '%s' "$endpoint" | sed -E 's#.*/runs/([0-9]+).*#\1#')
        fixture=$(fixture_for_run "$id") && cat "$fixture/run.json"
        ;;
      /repos/appergb/OpenTake/commits/*)
        sha=${endpoint##*/}; fixture=$(fixture_for_sha "$sha") && cat "$fixture/run-commit.json"
        ;;
      /repos/appergb/OpenTake/contents/.github/workflows/ci.yml?ref=*)
        sha=${endpoint##*ref=}; fixture=$(fixture_for_sha "$sha") && cat "$fixture/workflow-content.json"
        ;;
      /repos/appergb/OpenTake/actions/artifacts/*/zip)
        id=$(printf '%s' "$endpoint" | sed -E 's#.*/artifacts/([0-9]+)/zip#\1#')
        for fixture in "$fixture_root"/*-gh-fixture; do
          [ -f "$fixture/artifact-$id.zip" ] && { cat "$fixture/artifact-$id.zip"; exit 0; }
        done
        exit 66
        ;;
      *) printf 'unsupported fake endpoint: %s\n' "$endpoint" >&2; exit 64 ;;
    esac
  SH
  File.chmod(0o755, script)
  bin
end

def rewrite_json(path)
  value = JSON.parse(File.read(path))
  yield value
  File.write(path, JSON.pretty_generate(value) + "\n")
end

def build_review_proof(root, task, sha, baseline)
  directory = File.join(root, "logs", "c1b-task-#{task}-#{sha}-attempt-1")
  FileUtils.mkdir_p(directory)
  report = ->(role) { "Role: #{role}\nTask: #{task}\nCommit: #{sha}\nVerdict: APPROVE\nCritical: 0\nImportant: 0\nMinor: 0\n" }
  File.write(File.join(directory, "spec-security-review.md"), report.call("spec-security"))
  File.write(File.join(directory, "implementation-review.md"), report.call("implementation"))
  manifest = { "schema" => "opentake-c1b-reviewed-stage-v1", "task" => task,
    "sha" => sha, "baseline_sha" => baseline }
  File.write(File.join(directory, "gate-manifest.json"), JSON.pretty_generate(manifest) + "\n")
  directory
end

def build_gate(root, label, task, sha, predecessor, predecessor_proof, baseline, repo)
  repo = File.realpath(repo)
  nonce = Digest::SHA256.hexdigest(label)[0, 16]
  gate_name = task == "8" ? "c1b-20260713T000000Z-#{sha}-#{nonce}" : "c1b-task-#{task}-#{sha}-#{nonce}"
  gate = File.join(root, "branch-gates", gate_name)
  FileUtils.mkdir_p(gate)
  binding = { "schema" => "opentake-c1b-predecessor-binding-v1", "task" => task,
    "predecessor_task" => (task == "4" ? "3" : { "5" => "4", "6a" => "5", "6b" => "6a",
      "7a" => "6b", "7b" => "7a", "7c" => "7b", "8" => "7c" }.fetch(task)),
    "predecessor_sha" => predecessor, "predecessor_proof" => File.realpath(predecessor_proof) }
  File.write(File.join(gate, "predecessor-binding.json"), JSON.pretty_generate(binding) + "\n")
  timestamp = "2026-07-13T00:00:00Z"
  ledger = LOCAL_COMMANDS.map do |id, command|
    File.write(File.join(gate, "#{id}.log"), "synthetic #{id}\n")
    File.write(File.join(gate, "#{id}.raw-exit"), "0\n")
    {
      "id" => id, "command" => command, "cwd" => repo,
      "started_at_utc" => timestamp, "finished_at_utc" => timestamp,
      "exit_code" => 0, "log" => "#{id}.log", "raw_exit" => "#{id}.raw-exit",
    }
  end
  File.write(File.join(gate, "command-ledger.json"), JSON.pretty_generate(ledger) + "\n")
  File.write(File.join(gate, "pre-status.txt"), "")
  File.write(File.join(gate, "post-status.txt"), "")

  run_id = sha[0, 12].to_i(16).to_s
  run_identity_sha = Digest::SHA256.hexdigest("c1b-run-#{sha}")[0, 40]
  fixture = File.join(root, "#{label}-gh-fixture")
  FileUtils.mkdir_p(fixture)
  run = {
    "id" => run_id.to_i, "run_attempt" => 1, "head_sha" => run_identity_sha,
    "event" => "pull_request", "status" => "completed", "conclusion" => "success",
    "name" => "CI", "path" => ".github/workflows/ci.yml@refs/pull/42/merge",
    "pull_requests" => [{ "head" => { "sha" => sha } }],
    "repository" => { "full_name" => "appergb/OpenTake" },
  }
  run_commit = {
    "sha" => run_identity_sha,
    "parents" => [{ "sha" => "a" * 40 }, { "sha" => sha }],
  }
  workflow_content = {
    "encoding" => "base64",
    "content" => Base64.strict_encode64(File.binread(File.join(repo, ".github/workflows/ci.yml"))),
  }
  jobs = RECEIPT_IDS.each_with_index.map do |receipt_id, index|
    {
      "id" => sha[0, 8].to_i(16) * 10 + index, "run_id" => run_id.to_i, "head_sha" => run_identity_sha,
      "name" => "Safe filesystem (#{receipt_id})",
      "status" => "completed", "conclusion" => "success",
    }
  end
  artifacts = []
  RECEIPT_IDS.each_with_index do |receipt_id, index|
    directory = File.join(gate, "native-receipts", run_id, receipt_id)
    FileUtils.mkdir_p(directory)
    commands = NATIVE_COMMANDS.map do |id, command|
      File.write(File.join(directory, "#{id}.log"), "synthetic #{id}\n")
      File.write(File.join(directory, "#{id}.raw-exit"), "0\n")
      { "id" => id, "command" => command, "exit_code" => 0,
        "log" => "#{id}.log", "raw_exit" => "#{id}.raw-exit" }
    end
    File.write(File.join(directory, "final-aggregate.raw-exit"), "0\n")
    receipt = {
      "schema" => "opentake-c1b-native-receipt-v1", "receipt_id" => receipt_id,
      "repository" => "appergb/OpenTake", "workflow" => "CI",
      "workflow_file" => ".github/workflows/ci.yml", "job_id" => "safe-filesystem",
      "event_name" => "pull_request",
      "run_id" => run_id, "run_attempt" => "1", "requested_sha" => sha,
      "checked_out_sha" => sha, "commands" => commands, "aggregate_exit" => 0,
    }
    receipt.merge!(PROVENANCE.fetch(receipt_id))
    File.write(File.join(directory, "receipt.json"), JSON.pretty_generate(receipt) + "\n")
    archive_files = commands.flat_map { |command| [command.fetch("log"), command.fetch("raw_exit")] } +
      %w[final-aggregate.raw-exit receipt.json]
    _zip_out, zip_err, zip_status = Open3.capture3("zip", "-q", "artifact.zip", *archive_files,
      chdir: directory)
    raise "cannot build synthetic artifact archive: #{zip_err}" unless zip_status.success?
    artifact_id = sha[8, 8].to_i(16) * 10 + index
    artifact = {
      "id" => artifact_id,
      "name" => "c1b-native-#{receipt_id}-#{sha}",
      "expired" => false,
      "digest" => "sha256:#{Digest::SHA256.file(File.join(directory, 'artifact.zip')).hexdigest}",
      "workflow_run" => { "id" => run_id.to_i, "head_sha" => run_identity_sha },
    }
    artifacts << artifact
    File.write(File.join(directory, "run.json"), JSON.pretty_generate(run) + "\n")
    File.write(File.join(directory, "jobs.json"),
      JSON.pretty_generate({ "total_count" => jobs.length, "jobs" => jobs }) + "\n")
    File.write(File.join(directory, "artifact.json"), JSON.pretty_generate(artifact) + "\n")
    FileUtils.cp(File.join(directory, "artifact.zip"), File.join(fixture, "artifact-#{artifact_id}.zip"))
  end

  File.write(File.join(fixture, "run.json"), JSON.pretty_generate(run) + "\n")
  File.write(File.join(fixture, "run-id.txt"), "#{run_id}\n")
  File.write(File.join(fixture, "run-sha.txt"), "#{run_identity_sha}\n")
  File.write(File.join(fixture, "run-commit.json"), JSON.pretty_generate(run_commit) + "\n")
  File.write(File.join(fixture, "workflow-content.json"), JSON.pretty_generate(workflow_content) + "\n")
  File.write(File.join(fixture, "jobs.json"),
    JSON.pretty_generate({ "total_count" => jobs.length, "jobs" => jobs }) + "\n")
  File.write(File.join(fixture, "artifacts.json"),
    JSON.pretty_generate({ "total_count" => artifacts.length, "artifacts" => artifacts }) + "\n")

  FileUtils.mkdir_p(File.join(gate, "reviews"))
  spec = File.join("reviews", "spec-security-review.md")
  implementation = File.join("reviews", "implementation-review.md")
  report = ->(role) { "Role: #{role}\nTask: #{task}\nCommit: #{sha}\nVerdict: APPROVE\nCritical: 0\nImportant: 0\nMinor: 0\n" }
  File.write(File.join(gate, spec), report.call("spec-security"))
  File.write(File.join(gate, implementation), report.call("implementation"))
  results = [
    "Task: #{task}", "Baseline SHA: #{baseline}", "Predecessor SHA: #{predecessor}",
    "Final SHA: #{sha}", "Pre-status: clean", "Post-status: clean",
    *LOCAL_COMMANDS.map { |id, command| "| #{id} | #{command} | 0 |" },
    *RECEIPT_IDS.each_with_index.map do |id, index|
      artifact = artifacts.fetch(index)
      "| #{id} | #{run_id} | 1 | #{jobs.fetch(index).fetch('id')} | #{artifact.fetch('id')} | " \
        "#{artifact.fetch('name')} | #{artifact.fetch('digest')} | #{sha} | 0 |"
    end,
    "Spec report: #{spec}", "Implementation report: #{implementation}", "Aggregate: 0",
  ].join("\n") + "\n"
  File.write(File.join(gate, "results.md"), results)
  [gate, spec, implementation, fixture]
end

mutations = {
  "missing-local-row" => lambda { |copy|
    rewrite_json(File.join(copy, "command-ledger.json")) { |rows| rows.pop }
  },
  "renamed-local-id" => lambda { |copy|
    rewrite_json(File.join(copy, "command-ledger.json")) { |rows| rows[0]["id"] = "renamed" }
  },
  "duplicate-local-id" => lambda { |copy|
    rewrite_json(File.join(copy, "command-ledger.json")) { |rows| rows[1]["id"] = rows[0]["id"] }
  },
  "reordered-local-rows" => lambda { |copy|
    rewrite_json(File.join(copy, "command-ledger.json")) { |rows| rows[0], rows[1] = rows[1], rows[0] }
  },
  "substituted-local-command" => lambda { |copy|
    rewrite_json(File.join(copy, "command-ledger.json")) { |rows| rows[0]["command"] = "true" }
  },
  "invalid-timestamp" => lambda { |copy|
    rewrite_json(File.join(copy, "command-ledger.json")) { |rows| rows[0]["started_at_utc"] = "not-time" }
  },
  "escaped-log" => lambda { |copy|
    rewrite_json(File.join(copy, "command-ledger.json")) { |rows| rows[0]["log"] = "../outside.log" }
  },
  "dirty-pre-status" => lambda { |copy| File.write(File.join(copy, "pre-status.txt"), " M changed\n") },
  "dirty-post-status" => lambda { |copy| File.write(File.join(copy, "post-status.txt"), "?? new\n") },
  "substituted-native-command" => lambda { |copy|
    receipt = Dir.glob(File.join(copy, "native-receipts", "*", "*", "receipt.json")).first
    rewrite_json(receipt) { |value| value.fetch("commands")[0]["command"] = "true" }
  },
  "zero-run-attempt" => lambda { |copy|
    receipt = Dir.glob(File.join(copy, "native-receipts", "*", "*", "receipt.json")).first
    rewrite_json(receipt) { |value| value["run_attempt"] = "0" }
  },
  "relabeled-runner-os" => lambda { |copy|
    receipt = Dir.glob(File.join(copy, "native-receipts", "*", "linux-x86_64", "receipt.json")).first
    rewrite_json(receipt) { |value| value["runner_os"] = "Windows" }
  },
  "relabeled-runner-label" => lambda { |copy|
    receipt = Dir.glob(File.join(copy, "native-receipts", "*", "macos-native", "receipt.json")).first
    rewrite_json(receipt) { |value| value["runner_label"] = "ubuntu-24.04" }
  },
  "wrong-runner-architecture" => lambda { |copy|
    receipt = Dir.glob(File.join(copy, "native-receipts", "*", "windows-x86_64", "receipt.json")).first
    rewrite_json(receipt) { |value| value["runner_arch"] = "ARM64" }
  },
  "wrong-repository" => lambda { |copy|
    receipt = Dir.glob(File.join(copy, "native-receipts", "*", "*", "receipt.json")).first
    rewrite_json(receipt) { |value| value["repository"] = "someone/else" }
  },
  "wrong-workflow" => lambda { |copy|
    receipt = Dir.glob(File.join(copy, "native-receipts", "*", "*", "receipt.json")).first
    rewrite_json(receipt) { |value| value["workflow"] = "Other" }
  },
  "wrong-workflow-file" => lambda { |copy|
    receipt = Dir.glob(File.join(copy, "native-receipts", "*", "*", "receipt.json")).first
    rewrite_json(receipt) { |value| value["workflow_file"] = ".github/workflows/other.yml" }
  },
  "wrong-job" => lambda { |copy|
    receipt = Dir.glob(File.join(copy, "native-receipts", "*", "*", "receipt.json")).first
    rewrite_json(receipt) { |value| value["job_id"] = "rust" }
  },
  "wrong-event" => lambda { |copy|
    receipt = Dir.glob(File.join(copy, "native-receipts", "*", "*", "receipt.json")).first
    rewrite_json(receipt) { |value| value["event_name"] = "schedule" }
  },
  "wrong-live-run" => lambda { |copy, fixture|
    rewrite_json(File.join(fixture, "run.json")) { |value| value["head_sha"] = "f" * 40 }
    Dir.glob(File.join(copy, "native-receipts", "*", "*", "run.json")).each do |path|
      rewrite_json(path) { |value| value["head_sha"] = "f" * 40 }
    end
  },
  "wrong-live-pr-head" => lambda { |_copy, fixture|
    rewrite_json(File.join(fixture, "run.json")) { |value| value["pull_requests"] = [] }
    rewrite_json(File.join(fixture, "run-commit.json")) do |value|
      value["parents"] = [{ "sha" => "a" * 40 }, { "sha" => "b" * 40 }]
    end
  },
  "wrong-live-workflow-content" => lambda { |_copy, fixture|
    rewrite_json(File.join(fixture, "workflow-content.json")) do |value|
      value["content"] = Base64.strict_encode64("name: Evil\n")
    end
  },
  "wrong-live-job" => lambda { |copy, fixture|
    rewrite_json(File.join(fixture, "jobs.json")) { |value| value.fetch("jobs")[0]["head_sha"] = "f" * 40 }
    Dir.glob(File.join(copy, "native-receipts", "*", "*", "jobs.json")).each do |path|
      rewrite_json(path) { |value| value.fetch("jobs")[0]["head_sha"] = "f" * 40 }
    end
  },
  "paginated-live-jobs" => lambda { |copy, fixture|
    rewrite_json(File.join(fixture, "jobs.json")) { |value| value["total_count"] = 101 }
    Dir.glob(File.join(copy, "native-receipts", "*", "*", "jobs.json")).each do |path|
      rewrite_json(path) { |value| value["total_count"] = 101 }
    end
  },
  "paginated-live-artifacts" => lambda { |_copy, fixture|
    rewrite_json(File.join(fixture, "artifacts.json")) { |value| value["total_count"] = 101 }
  },
  "wrong-live-artifact" => lambda { |copy, fixture|
    rewrite_json(File.join(fixture, "artifacts.json")) do |value|
      value.fetch("artifacts")[0]["name"] = "forged-artifact"
    end
    artifact = Dir.glob(File.join(copy, "native-receipts", "*", "linux-x86_64", "artifact.json")).first
    rewrite_json(artifact) { |value| value["name"] = "forged-artifact" }
  },
  "wrong-live-digest" => lambda { |copy, fixture|
    bad = "sha256:#{'0' * 64}"
    rewrite_json(File.join(fixture, "artifacts.json")) do |value|
      value.fetch("artifacts")[0]["digest"] = bad
    end
    artifact = Dir.glob(File.join(copy, "native-receipts", "*", "linux-x86_64", "artifact.json")).first
    rewrite_json(artifact) { |value| value["digest"] = bad }
  },
  "locally-forged-run-json" => lambda { |copy|
    run = Dir.glob(File.join(copy, "native-receipts", "*", "*", "run.json")).first
    rewrite_json(run) { |value| value["head_sha"] = "f" * 40 }
  },
  "external-symlink-run-json" => lambda { |copy|
    run = Dir.glob(File.join(copy, "native-receipts", "*", "*", "run.json")).first
    outside = File.join(File.dirname(copy), "outside-run.json")
    File.write(outside, File.read(run))
    File.unlink(run)
    File.symlink(outside, run)
  },
  "missing-results-row" => lambda { |copy|
    path = File.join(copy, "results.md")
    File.write(path, File.read(path).lines.reject { |line| line.include?("| cargo-fmt |") }.join)
  },
  "wrong-review-task" => lambda { |copy|
    path = File.join(copy, "reviews", "spec-security-review.md")
    File.write(path, File.read(path).sub("Task: #{task}", "Task: 5"))
  },
  "wrong-results-task" => lambda { |copy|
    path = File.join(copy, "results.md")
    File.write(path, File.read(path).sub("Task: #{task}", "Task: 5"))
  },
  "wrong-frozen-baseline" => lambda { |copy|
    path = File.join(copy, "results.md")
    File.write(path, File.read(path).sub(/Baseline SHA: [0-9a-f]{40}/, "Baseline SHA: #{'0' * 40}"))
  },
  "wrong-results-predecessor" => lambda { |copy|
    path = File.join(copy, "results.md")
    File.write(path, File.read(path).sub(/Predecessor SHA: [0-9a-f]{40}/,
      "Predecessor SHA: #{'f' * 40}"))
  },
  "wrong-predecessor-binding" => lambda { |copy|
    rewrite_json(File.join(copy, "predecessor-binding.json")) { |value| value["predecessor_task"] = "2b" }
  },
  "wrong-gate-task-name" => lambda { |copy|
    moved = File.join(File.dirname(copy), "wrong-task-#{File.basename(copy)}")
    FileUtils.mv(copy, moved)
    File.symlink(moved, copy)
  },
}

Dir.mktmpdir("c1b-evidence-validator") do |temporary|
  slice_repo = File.join(temporary, "task4-worktree")
  raise "cannot create isolated synthetic Task4 clone" unless
    system("git", "clone", "--quiet", "--shared", "--no-checkout", repo, slice_repo,
      out: File::NULL, err: File::NULL)
  raise "cannot checkout synthetic Task4 base" unless
    system("git", "-C", slice_repo, "checkout", "--detach", predecessor,
      out: File::NULL, err: File::NULL)
  begin
    commit_env = {
      "GIT_AUTHOR_NAME" => "C1B Validator Test", "GIT_AUTHOR_EMAIL" => "c1b@example.invalid",
      "GIT_COMMITTER_NAME" => "C1B Validator Test", "GIT_COMMITTER_EMAIL" => "c1b@example.invalid",
    }
    raise "cannot create synthetic Task3 GREEN predecessor" unless system(commit_env, "git", "-C", slice_repo,
      "commit", "--allow-empty", "-m", "ci: verify C1B receipts and evidence on exact SHAs",
      out: File::NULL, err: File::NULL)
    predecessor, predecessor_status = Open3.capture2("git", "-C", slice_repo, "rev-parse", "HEAD")
    raise "cannot resolve synthetic Task3 GREEN predecessor" unless predecessor_status.success?
    predecessor = predecessor.strip.downcase
    raise "cannot create synthetic Task4 RED" unless system(commit_env, "git", "-C", slice_repo,
      "commit", "--allow-empty", "-m", "test(project): specify Unix recursive filesystem authorities",
      out: File::NULL, err: File::NULL)
    raise "cannot create synthetic Task4 GREEN" unless system(commit_env, "git", "-C", slice_repo,
      "commit", "--allow-empty", "-m", "feat(project): add Unix recursive filesystem authorities",
      out: File::NULL, err: File::NULL)
    sha, slice_sha_status = Open3.capture2("git", "-C", slice_repo, "rev-parse", "HEAD")
    raise "cannot resolve synthetic Task4 GREEN" unless slice_sha_status.success?
    sha = sha.strip.downcase
    predecessor_proof = build_review_proof(temporary, "3", predecessor, baseline)
    fake_bin = install_fake_gh(temporary)
    gate, spec, implementation, fixture =
      build_gate(temporary, "canonical", task, sha, predecessor, predecessor_proof, baseline, slice_repo)
    out, err, status = run_validator(validator, gate, task, sha, predecessor, predecessor_proof,
      spec, implementation, slice_repo, fake_bin: fake_bin, fixture: fixture)
    raise "canonical gate rejected: #{out}#{err}" unless status.success?
    _out, _err, wrong_predecessor = run_validator(
      validator, gate, task, sha, baseline, predecessor_proof, spec, implementation, slice_repo,
      fake_bin: fake_bin, fixture: fixture
    )
    raise "validator accepted wrong Task4 predecessor" if wrong_predecessor.success?
    wrong_proof = File.join(temporary, "logs", "c1b-task-3-#{predecessor}-attempt-2")
    FileUtils.cp_r(predecessor_proof, wrong_proof)
    rewrite_json(File.join(wrong_proof, "gate-manifest.json")) { |value| value["task"] = "2b" }
    _out, _err, wrong_proof_status = run_validator(
      validator, gate, task, sha, predecessor, wrong_proof, spec, implementation, slice_repo,
      fake_bin: fake_bin, fixture: fixture
    )
    raise "validator accepted wrong predecessor proof manifest" if wrong_proof_status.success?

  missing_gh_env = { "PATH" => File.join(temporary, "missing-gh"),
    "C1B_FAKE_GH_FIXTURE_ROOT" => temporary }
    _out, _err, missing_gh = Open3.capture3(missing_gh_env, RbConfig.ruby, validator,
      gate, task, sha, predecessor, predecessor_proof, spec, implementation, slice_repo)
    raise "validator accepted evidence without authenticated gh" if missing_gh.success?

    mutations.each do |label, mutate|
      copy, copy_spec, copy_implementation, copy_fixture =
        build_gate(temporary, label, task, sha, predecessor, predecessor_proof, baseline, slice_repo)
      mutate.arity == 1 ? mutate.call(copy) : mutate.call(copy, copy_fixture)
      _out, _err, result = run_validator(
        validator, copy, task, sha, predecessor, predecessor_proof, copy_spec, copy_implementation,
        slice_repo, fake_bin: fake_bin, fixture: copy_fixture
      )
      raise "validator accepted mutation #{label}" if result.success?
    end
    File.write(File.join(gate, "results-validation.raw-exit"), "0\n")
    File.write(File.join(gate, "results-validation.log"),
      "c1b-evidence-validation=ok task=4 predecessor=#{predecessor} sha=#{sha}\n")
    task4_sha = sha
    raise "cannot create synthetic Task5 RED" unless system(commit_env, "git", "-C", slice_repo,
      "commit", "--allow-empty", "-m", "test(project): specify Unix quarantine and recursive cleanup",
      out: File::NULL, err: File::NULL)
    raise "cannot create synthetic Task5 GREEN" unless system(commit_env, "git", "-C", slice_repo,
      "commit", "--allow-empty", "-m", "feat(project): add Unix consuming quarantine cleanup",
      out: File::NULL, err: File::NULL)
    task5_sha, task5_status = Open3.capture2("git", "-C", slice_repo, "rev-parse", "HEAD")
    raise "cannot resolve synthetic Task5 GREEN" unless task5_status.success?
    task5_sha = task5_sha.strip.downcase
    task5_gate, task5_spec, task5_implementation, task5_fixture =
      build_gate(temporary, "task5-canonical", "5", task5_sha, task4_sha, gate, baseline, slice_repo)
    task5_out, task5_err, task5_result = run_validator(
      validator, task5_gate, "5", task5_sha, task4_sha, gate, task5_spec, task5_implementation,
      slice_repo, fake_bin: fake_bin, fixture: task5_fixture
    )
    raise "canonical Task5 predecessor-gate chain rejected: #{task5_out}#{task5_err}" unless task5_result.success?
    File.write(File.join(task5_gate, "results-validation.raw-exit"), "0\n")
    File.write(File.join(task5_gate, "results-validation.log"),
      "c1b-evidence-validation=ok task=5 predecessor=#{task4_sha} sha=#{task5_sha}\n")
    raise "cannot create synthetic Task6A GREEN" unless system(commit_env, "git", "-C", slice_repo,
      "commit", "--allow-empty", "-m", "feat(project): add fail-closed Windows platform scaffold",
      out: File::NULL, err: File::NULL)
    task6a_sha, task6a_status = Open3.capture2("git", "-C", slice_repo, "rev-parse", "HEAD")
    raise "cannot resolve synthetic Task6A GREEN" unless task6a_status.success?
    task6a_sha = task6a_sha.strip.downcase
    task6a_gate, task6a_spec, task6a_implementation, task6a_fixture =
      build_gate(temporary, "task6a-canonical", "6a", task6a_sha, task5_sha, task5_gate, baseline, slice_repo)
    task6a_out, task6a_err, task6a_result = run_validator(
      validator, task6a_gate, "6a", task6a_sha, task5_sha, task5_gate, task6a_spec,
      task6a_implementation, slice_repo, fake_bin: fake_bin, fixture: task6a_fixture
    )
    raise "canonical Task6A predecessor-gate chain rejected: #{task6a_out}#{task6a_err}" unless task6a_result.success?
  ensure
    FileUtils.rm_rf(slice_repo)
  end
end

puts "c1b-evidence-validator-tests=ok"
```

上述 test与fail-closed scaffold在 Task 3 RED；完整 validator在 Task 3 GREEN。Task 8 不增加、修改或提交任何 validator/test code，只创建最终 SHA 的真实 exclusive gate，并调用已经 committed at Task 3 的两个 validators。因此 validator code SHA 与被验证 final SHA 一致，不存在“先有 final gate、后提交 validator 导致 SHA 改变”的循环。

GitHub 的官方 REST artifact schema 定义 `id`、`name`、`digest`、`archive_download_url` 和 `workflow_run.head_sha`，且 download endpoint 返回 ZIP archive；workflow-run/job endpoints 定义 run `head_sha` 以及 job `run_id`/`head_sha`/`name`/`conclusion`。实施时以 [GitHub REST Actions artifacts](https://docs.github.com/en/rest/actions/artifacts)、[workflow runs](https://docs.github.com/en/rest/actions/workflow-runs) 和 [workflow jobs](https://docs.github.com/en/rest/actions/workflow-jobs) 为字段来源。

每个 per-task GREEN gate（Task 8 含在其中）在调 validator 前必须执行下列 REST archive protocol。`RUN_ID`是已完成的授权 CI run，三个 matrix receipt 必须同属该 run/attempt。所有路径位于当前 exclusive gate；`.artifacts-<id>.json` 是 gate-local temporary，成功选取后立即删除：

```bash
set -euo pipefail
gh auth status --hostname github.com
GH_API_VERSION=2026-03-10
case "$GATE_TASK" in
  4) PREDECESSOR_TASK=3 ;; 5) PREDECESSOR_TASK=4 ;; 6a) PREDECESSOR_TASK=5 ;;
  6b) PREDECESSOR_TASK=6a ;; 7a) PREDECESSOR_TASK=6b ;; 7b) PREDECESSOR_TASK=7a ;;
  7c) PREDECESSOR_TASK=7b ;; 8) PREDECESSOR_TASK=7c ;; *) exit 64 ;;
esac
ruby -rjson -rpathname -e '
  output, task, predecessor_task, predecessor_sha, proof = ARGV
  value = { "schema" => "opentake-c1b-predecessor-binding-v1", "task" => task,
    "predecessor_task" => predecessor_task, "predecessor_sha" => predecessor_sha,
    "predecessor_proof" => Pathname.new(proof).realpath.to_s }
  File.open(output, File::WRONLY | File::CREAT | File::EXCL, 0o600) do |file|
    file.write(JSON.pretty_generate(value) + "\n")
  end
' "$GATE_DIR/predecessor-binding.json" "$GATE_TASK" "$PREDECESSOR_TASK" \
  "$PREDECESSOR_SHA" "$PREDECESSOR_PROOF"
mkdir "$GATE_DIR/reviews"
cp "$FINAL_SPEC_REPORT" "$GATE_DIR/reviews/spec-security-review.md"
cp "$FINAL_IMPLEMENTATION_REPORT" "$GATE_DIR/reviews/implementation-review.md"

for RECEIPT_ID in linux-x86_64 macos-native windows-x86_64; do
  RECEIPT_ROOT="$GATE_DIR/native-receipts/$RUN_ID/$RECEIPT_ID"
  mkdir "$RECEIPT_ROOT"
  RUN_ENDPOINT="/repos/appergb/OpenTake/actions/runs/$RUN_ID"
  gh api --hostname github.com -H "X-GitHub-Api-Version: $GH_API_VERSION" \
    "$RUN_ENDPOINT" >"$RECEIPT_ROOT/run.json"
  gh api --hostname github.com -H "X-GitHub-Api-Version: $GH_API_VERSION" \
    "$RUN_ENDPOINT/jobs?per_page=100" >"$RECEIPT_ROOT/jobs.json"
  ALL_ARTIFACTS="$GATE_DIR/.artifacts-$RECEIPT_ID.json"
  gh api --hostname github.com -H "X-GitHub-Api-Version: $GH_API_VERSION" \
    "$RUN_ENDPOINT/artifacts?per_page=100" >"$ALL_ARTIFACTS"
  ARTIFACT_NAME="c1b-native-$RECEIPT_ID-$FINAL_SHA"
  ruby -rjson -e '
    source, name, destination = ARGV
    document = JSON.parse(File.read(source))
    artifacts = document.fetch("artifacts")
    raise "artifact API pagination is incomplete" unless
      document.fetch("total_count") == artifacts.length && document.fetch("total_count") <= 100
    matches = artifacts.select { |row| row.fetch("name") == name }
    raise "expected exactly one artifact #{name}" unless matches.length == 1
    File.write(destination, JSON.pretty_generate(matches.fetch(0)) + "\n")
  ' "$ALL_ARTIFACTS" "$ARTIFACT_NAME" "$RECEIPT_ROOT/artifact.json"
  rm "$ALL_ARTIFACTS"
  ARTIFACT_ID=$(ruby -rjson -e 'puts JSON.parse(File.read(ARGV.fetch(0))).fetch("id")' \
    "$RECEIPT_ROOT/artifact.json")
  gh api --hostname github.com \
    -H "X-GitHub-Api-Version: $GH_API_VERSION" \
    "/repos/appergb/OpenTake/actions/artifacts/$ARTIFACT_ID/zip" \
    >"$RECEIPT_ROOT/artifact.zip"
  test -s "$RECEIPT_ROOT/artifact.zip"
  unzip -Z1 "$RECEIPT_ROOT/artifact.zip" | ruby -e '
    allowed = %w[cargo-fmt.log cargo-fmt.raw-exit cargo-clippy.log cargo-clippy.raw-exit
      safe-fs-unit.log safe-fs-unit.raw-exit archive-security.log archive-security.raw-exit
      final-aggregate.raw-exit receipt.json]
    names = STDIN.each_line.map(&:strip)
    raise "artifact archive entry set mismatch" unless names.sort == allowed.sort
    raise "unsafe archive entry" if names.any? { |name| name.start_with?("/") || name.split(/[\\\/]/).include?("..") }
  '
  unzip -q "$RECEIPT_ROOT/artifact.zip" -d "$RECEIPT_ROOT"
  ruby -rjson -rdigest -e '
    metadata, archive = ARGV
    expected = JSON.parse(File.read(metadata)).fetch("digest")
    actual = "sha256:#{Digest::SHA256.file(archive).hexdigest}"
    raise "REST artifact digest mismatch" unless expected == actual
  ' "$RECEIPT_ROOT/artifact.json" "$RECEIPT_ROOT/artifact.zip"
done
```

每个 gate 的调用固定为（`GATE_TASK`、`FINAL_SHA`、`PREDECESSOR_SHA`、`PREDECESSOR_PROOF` 按本节参数表赋值）：

```bash
ruby scripts/validate-c1b-evidence.rb \
  "$GATE_DIR" "$GATE_TASK" "$FINAL_SHA" "$PREDECESSOR_SHA" "$PREDECESSOR_PROOF" \
  'reviews/spec-security-review.md' 'reviews/implementation-review.md' \
  '/Users/lvbaiqing/TRUE 开发/PRIMARY-CN/OpenTake-full-convergence' \
  >"$GATE_DIR/results-validation.log" 2>&1
printf '%s\n' "$?" >"$GATE_DIR/results-validation.raw-exit"
test "$(cat "$GATE_DIR/results-validation.raw-exit")" = 0
ruby scripts/tests/validate-c1b-evidence-test.rb \
  >"$GATE_DIR/results-mutation-validation.log" 2>&1
printf '%s\n' "$?" >"$GATE_DIR/results-mutation-validation.raw-exit"
test "$(cat "$GATE_DIR/results-mutation-validation.raw-exit")" = 0
```

该 validator 每次都直接执行 authenticated `gh api --hostname github.com`，覆盖 final SHA、run/attempt/repository/workflow/event、三个成功 job、三个 SHA-bound artifact name/id/digest/`workflow_run.head_sha`、保留 ZIP 的 SHA-256 与 archive 内 receipt，以及原有 command/aggregate/audit/clean-worktree/results 条件。所有证据文件通过 gate-relative `confined_file!`，解析到 gate 外的 symlink 必须失败。任何不满足项都非零退出；不能在 validation 后修改 evidence 或 commit。
