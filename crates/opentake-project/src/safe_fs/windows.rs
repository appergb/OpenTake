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
use windows_sys::Wdk::Foundation::OBJECT_ATTRIBUTES;
use windows_sys::Wdk::Storage::FileSystem::*;
use windows_sys::Win32::Foundation::{
    CloseHandle, DuplicateHandle, GetLastError, RtlNtStatusToDosError, DUPLICATE_SAME_ACCESS,
    HANDLE, INVALID_HANDLE_VALUE, NTSTATUS, OBJ_CASE_INSENSITIVE, STATUS_ACCESS_DENIED,
    STATUS_BUFFER_OVERFLOW, STATUS_BUFFER_TOO_SMALL, STATUS_CANNOT_DELETE, STATUS_DELETE_PENDING,
    STATUS_DIRECTORY_NOT_EMPTY, STATUS_END_OF_FILE, STATUS_FILE_IS_A_DIRECTORY,
    STATUS_INFO_LENGTH_MISMATCH, STATUS_INVALID_PARAMETER, STATUS_NOT_A_DIRECTORY,
    STATUS_NOT_SUPPORTED, STATUS_NO_MORE_FILES, STATUS_OBJECT_NAME_COLLISION,
    STATUS_OBJECT_NAME_NOT_FOUND, STATUS_OBJECT_PATH_NOT_FOUND, STATUS_OBJECT_TYPE_MISMATCH,
    STATUS_PENDING, STATUS_REPARSE_POINT_ENCOUNTERED, STATUS_SHARING_VIOLATION, UNICODE_STRING,
};
use windows_sys::Win32::Security::*;
use windows_sys::Win32::Storage::FileSystem::{
    CreateFileW, FileAttributeTagInfo, FileIdInfo, FileRemoteProtocolInfo, FileStandardInfo,
    GetDriveTypeW, GetFileInformationByHandleEx, GetVolumeInformationByHandleW,
    GetVolumeNameForVolumeMountPointW, GetVolumePathNameW, DELETE, FILE_ACCESS_RIGHTS,
    FILE_ADD_FILE, FILE_ADD_SUBDIRECTORY, FILE_ALL_ACCESS, FILE_ATTRIBUTE_NORMAL,
    FILE_ATTRIBUTE_REPARSE_POINT, FILE_ATTRIBUTE_TAG_INFO, FILE_DELETE_CHILD,
    FILE_FLAGS_AND_ATTRIBUTES, FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT,
    FILE_ID_INFO, FILE_LIST_DIRECTORY, FILE_READ_ATTRIBUTES, FILE_READ_DATA,
    FILE_REMOTE_PROTOCOL_INFO, FILE_SHARE_MODE, FILE_SHARE_READ, FILE_SHARE_WRITE,
    FILE_STANDARD_INFO, FILE_TRAVERSE, FILE_WRITE_DATA, GET_FILEEX_INFO_LEVELS,
    MAXIMUM_REPARSE_DATA_BUFFER_SIZE, OPEN_EXISTING, READ_CONTROL, SYNCHRONIZE,
};
use windows_sys::Win32::System::Ioctl::FSCTL_GET_REPARSE_POINT;
use windows_sys::Win32::System::SystemServices::{
    ACCESS_ALLOWED_ACE_TYPE, FILE_CS_FLAG_CASE_SENSITIVE_DIR, SECURITY_DESCRIPTOR_REVISION,
};
use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
use windows_sys::Win32::System::IO::IO_STATUS_BLOCK;

const SHARE: FILE_SHARE_MODE = FILE_SHARE_READ | FILE_SHARE_WRITE;
const COMMON_OPTIONS: NTCREATEFILE_CREATE_OPTIONS =
    FILE_OPEN_REPARSE_POINT | FILE_OPEN_FOR_BACKUP_INTENT | FILE_SYNCHRONOUS_IO_NONALERT;
const DIRECTORY_BUFFER_BYTES: usize = 64 * 1024;
const REPARSE_HEADER_BYTES: usize = 8;
const STATUS_SUCCESS: NTSTATUS = 0;
const BOOL_FALSE: BOOL = 0;
const BOOL_TRUE: BOOL = 1;
const DRIVE_REMOVABLE: u32 = 2;
const DRIVE_FIXED: u32 = 3;

impl CaseMode {
    fn object_attributes(self) -> u32 {
        match self {
            Self::Sensitive => 0,
            Self::Insensitive => OBJ_CASE_INSENSITIVE,
        }
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

    fn raw(&self) -> HANDLE {
        self.0
    }
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

pub(super) struct NativeNamespaceAnchor {
    root: Arc<DirectoryNode>,
    mapping: VolumeProof,
    absolute_path: PathBuf,
    base_components: usize,
    access: DirectoryAccess,
}

pub(super) struct NativeDirectory {
    node: Arc<DirectoryNode>,
    access: DirectoryAccess,
    delete_right: bool,
}

pub(super) struct NativeFile {
    handle: OwnedHandle,
    opened: EntryMetadata,
    access: FileAccess,
    delete_right: bool,
}

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
    desired: FILE_LIST_DIRECTORY
        | FILE_TRAVERSE
        | FILE_READ_ATTRIBUTES
        | FILE_ADD_FILE
        | FILE_ADD_SUBDIRECTORY
        | FILE_DELETE_CHILD
        | SYNCHRONIZE,
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
    desired: FILE_READ_DATA
        | FILE_WRITE_DATA
        | FILE_READ_ATTRIBUTES
        | READ_CONTROL
        | DELETE
        | SYNCHRONIZE,
    disposition: FILE_CREATE,
    options: COMMON_OPTIONS | FILE_NON_DIRECTORY_FILE,
    attributes: FILE_ATTRIBUTE_NORMAL,
    delete_right: true,
};
const CREATE_DIR_CONTRACT: OpenContract = OpenContract {
    desired: FILE_LIST_DIRECTORY
        | FILE_TRAVERSE
        | FILE_READ_ATTRIBUTES
        | READ_CONTROL
        | FILE_ADD_FILE
        | FILE_ADD_SUBDIRECTORY
        | FILE_DELETE_CHILD
        | DELETE
        | SYNCHRONIZE,
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
    options: COMMON_OPTIONS,
    attributes: 0,
    delete_right: true,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum OpenOperation {
    Query,
    DirRead,
    DirMutate,
    FileRead,
    FileWrite,
    CreateFile,
    CreateDir,
    CreateStage,
    CleanupFile,
    CleanupDir,
    CleanupReparse,
}

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

struct NtName {
    units: Vec<u16>,
    unicode: UNICODE_STRING,
}

impl NtName {
    fn new(name: &ComponentName) -> Result<Self> {
        let units: Vec<u16> = name.as_os_str().encode_wide().collect();
        let byte_len = units
            .len()
            .checked_mul(size_of::<u16>())
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
        Length: u32::try_from(size_of::<OBJECT_ATTRIBUTES>())
            .expect("OBJECT_ATTRIBUTES size fits u32"),
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

fn checked_information(
    operation: SafeFsOperation,
    iosb: &IO_STATUS_BLOCK,
    capacity: usize,
) -> Result<usize> {
    let used = iosb.Information;
    if used > capacity {
        return Err(SafeFsError::InvalidNativeBuffer {
            operation,
            reason: NativeBufferReason::IoStatusInformationOutOfBounds,
        });
    }
    Ok(used)
}

fn complete_nt(
    operation: SafeFsOperation,
    returned: NTSTATUS,
    iosb: &IO_STATUS_BLOCK,
) -> Result<()> {
    if returned != STATUS_SUCCESS {
        return Err(nt_error(operation, returned));
    }
    let final_status = iosb_status(iosb);
    if final_status != STATUS_SUCCESS {
        return Err(nt_error(operation, final_status));
    }
    Ok(())
}

#[cfg(test)]
pub(super) fn synchronous_pending_contract_for_test() -> Result<()> {
    let mut iosb = IO_STATUS_BLOCK::default();
    // Initialize the Status member even though `complete_nt` must reject the
    // returned STATUS_PENDING before reading it.
    iosb.Anonymous.Status = STATUS_SUCCESS;
    iosb.Information = usize::MAX;
    complete_nt(SafeFsOperation::ReadFile, STATUS_PENDING, &iosb)
}

#[allow(clippy::too_many_arguments)] // Mirrors the fixed NtCreateFile operation contract.
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
        NtCreateFile(
            &mut raw,
            desired | SYNCHRONIZE,
            &attrs,
            &mut iosb,
            null(),
            attributes,
            SHARE,
            disposition,
            options | COMMON_OPTIONS,
            null(),
            0,
        )
    };
    complete_nt(operation, status, &iosb)?;
    OwnedHandle::new(raw, operation)
}

pub(super) fn read_file(file: &mut NativeFile, out: &mut [u8]) -> Result<usize> {
    nt_read(file.handle.raw(), out)
}

pub(super) fn write_file(file: &mut NativeFile, input: &[u8]) -> Result<usize> {
    if file.access == FileAccess::Read {
        return Err(SafeFsError::io(
            SafeFsOperation::WriteFile,
            io::Error::new(io::ErrorKind::PermissionDenied, "read-only capability"),
        ));
    }
    nt_write(file.handle.raw(), input)
}

pub(super) fn flush_file(file: &mut NativeFile) -> Result<()> {
    nt_flush(file.handle.raw())
}

pub(super) fn seek_file(file: &mut NativeFile, position: SeekFrom) -> Result<u64> {
    nt_seek(file.handle.raw(), position)
}

pub(super) fn sync_file(file: &NativeFile) -> Result<()> {
    nt_flush(file.handle.raw())
}

fn nt_read(handle: HANDLE, output: &mut [u8]) -> Result<usize> {
    if output.is_empty() {
        return Ok(0);
    }
    let length = u32::try_from(output.len().min(u32::MAX as usize)).expect("bounded read length");
    let mut iosb = IO_STATUS_BLOCK::default();
    // SAFETY: retained synchronous handle; output is writable for length; null event/APC/offset uses file position.
    let status = unsafe {
        NtReadFile(
            handle,
            null_mut(),
            None,
            null(),
            &mut iosb,
            output.as_mut_ptr().cast(),
            length,
            null(),
            null(),
        )
    };
    if status == STATUS_END_OF_FILE {
        return Ok(0);
    }
    complete_nt(SafeFsOperation::ReadFile, status, &iosb)?;
    checked_information(SafeFsOperation::ReadFile, &iosb, length as usize)
}

fn nt_write(handle: HANDLE, input: &[u8]) -> Result<usize> {
    if input.is_empty() {
        return Ok(0);
    }
    let length = u32::try_from(input.len().min(u32::MAX as usize)).expect("bounded write length");
    let mut iosb = IO_STATUS_BLOCK::default();
    // SAFETY: retained synchronous handle; input is readable for length; null event/APC/offset uses file position.
    let status = unsafe {
        NtWriteFile(
            handle,
            null_mut(),
            None,
            null(),
            &mut iosb,
            input.as_ptr().cast(),
            length,
            null(),
            null(),
        )
    };
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
        SeekFrom::Start(value) => {
            Some(
                i64::try_from(value).map_err(|_| SafeFsError::InvalidNativeBuffer {
                    operation: SafeFsOperation::SeekFile,
                    reason: NativeBufferReason::LengthOverflow,
                })?,
            )
        }
        SeekFrom::Current(delta) => current.checked_add(delta),
        SeekFrom::End(delta) => end.checked_add(delta),
    };
    let next = next.ok_or(SafeFsError::InvalidNativeBuffer {
        operation: SafeFsOperation::SeekFile,
        reason: NativeBufferReason::LengthOverflow,
    })?;
    if next < 0 {
        return Err(SafeFsError::io(
            SafeFsOperation::SeekFile,
            io::Error::new(io::ErrorKind::InvalidInput, "negative seek"),
        ));
    }
    let info = FILE_POSITION_INFORMATION {
        CurrentByteOffset: next,
    };
    let mut iosb = IO_STATUS_BLOCK::default();
    // SAFETY: handle retained; fixed-size initialized info and writable iosb stay live.
    let status = unsafe {
        NtSetInformationFile(
            handle,
            &mut iosb,
            (&info as *const FILE_POSITION_INFORMATION).cast(),
            u32::try_from(size_of::<FILE_POSITION_INFORMATION>()).expect("position info fits"),
            FilePositionInformation,
        )
    };
    complete_nt(SafeFsOperation::SeekFile, status, &iosb)?;
    Ok(next as u64)
}

fn query_position(handle: HANDLE) -> Result<i64> {
    let mut info = FILE_POSITION_INFORMATION::default();
    query_fixed(
        handle,
        FilePositionInformation,
        SafeFsOperation::SeekFile,
        &mut info,
    )?;
    Ok(info.CurrentByteOffset)
}

fn query_standard(handle: HANDLE) -> Result<FILE_STANDARD_INFORMATION> {
    let mut info = FILE_STANDARD_INFORMATION::default();
    query_fixed(
        handle,
        FileStandardInformation,
        SafeFsOperation::QueryMetadata,
        &mut info,
    )?;
    Ok(info)
}

fn query_fixed<T>(
    handle: HANDLE,
    class: FILE_INFORMATION_CLASS,
    operation: SafeFsOperation,
    output: &mut T,
) -> Result<()> {
    let mut iosb = IO_STATUS_BLOCK::default();
    // SAFETY: output is writable for exactly size_of::<T>(); handle retained; iosb writable.
    let status = unsafe {
        NtQueryInformationFile(
            handle,
            &mut iosb,
            (output as *mut T).cast(),
            u32::try_from(size_of::<T>()).expect("query structure fits u32"),
            class,
        )
    };
    complete_nt(operation, status, &iosb)?;
    let used = checked_information(operation, &iosb, size_of::<T>())?;
    if used != size_of::<T>() {
        return Err(SafeFsError::InvalidNativeBuffer {
            operation,
            reason: NativeBufferReason::LengthOverflow,
        });
    }
    Ok(())
}

fn query_case_mode(handle: HANDLE) -> Result<CaseMode> {
    let mut info = FILE_CASE_SENSITIVE_INFORMATION::default();
    query_fixed(
        handle,
        FileCaseSensitiveInformation,
        SafeFsOperation::QueryCaseMode,
        &mut info,
    )?;
    if info.Flags & !FILE_CS_FLAG_CASE_SENSITIVE_DIR != 0 {
        return Err(SafeFsError::InvalidNativeBuffer {
            operation: SafeFsOperation::QueryCaseMode,
            reason: NativeBufferReason::UnknownCaseFlags,
        });
    }
    Ok(if info.Flags & FILE_CS_FLAG_CASE_SENSITIVE_DIR != 0 {
        CaseMode::Sensitive
    } else {
        CaseMode::Insensitive
    })
}

fn win32_query<T: Default>(
    handle: HANDLE,
    class: GET_FILEEX_INFO_LEVELS,
    operation: SafeFsOperation,
) -> Result<T> {
    let mut output = T::default();
    // SAFETY: output is writable for the class-specific fixed T and handle retained.
    if unsafe {
        GetFileInformationByHandleEx(
            handle,
            class,
            (&mut output as *mut T).cast(),
            u32::try_from(size_of::<T>()).expect("query structure fits"),
        )
    } == 0
    {
        return Err(last_win32(operation));
    }
    Ok(output)
}

// The native FileRemoteProtocolInfo query requires an 8-byte-aligned output buffer.
// windows-sys models its 116-byte payload with alignment 4, so align the storage without
// reporting the wrapper's trailing padding through StructureSize or dwBufferSize.
#[repr(C, align(8))]
struct RemoteProtocolQueryBuffer {
    info: FILE_REMOTE_PROTOCOL_INFO,
}

const _: () = {
    assert!(size_of::<FILE_REMOTE_PROTOCOL_INFO>() == 116);
    assert!(align_of::<FILE_REMOTE_PROTOCOL_INFO>() == 4);
    assert!(offset_of!(RemoteProtocolQueryBuffer, info) == 0);
    assert!(size_of::<RemoteProtocolQueryBuffer>() == 120);
    assert!(align_of::<RemoteProtocolQueryBuffer>() == 8);
};

fn remote_protocol_query_buffer(operation: SafeFsOperation) -> Result<RemoteProtocolQueryBuffer> {
    let structure_size = u16::try_from(size_of::<FILE_REMOTE_PROTOCOL_INFO>()).map_err(|_| {
        SafeFsError::InvalidNativeBuffer {
            operation,
            reason: NativeBufferReason::LengthOverflow,
        }
    })?;
    Ok(RemoteProtocolQueryBuffer {
        info: FILE_REMOTE_PROTOCOL_INFO {
            StructureVersion: 2,
            StructureSize: structure_size,
            ..FILE_REMOTE_PROTOCOL_INFO::default()
        },
    })
}

fn query_entry_metadata(
    handle: HANDLE,
    filesystem: &LocalFilesystemSnapshot,
    operation: SafeFsOperation,
) -> Result<EntryMetadata> {
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
    let mut remote = remote_protocol_query_buffer(operation)?;
    // SAFETY: the fixed remote-protocol output is 8-byte aligned, writable, and handle retained.
    if unsafe {
        GetFileInformationByHandleEx(
            handle,
            FileRemoteProtocolInfo,
            (&mut remote.info as *mut FILE_REMOTE_PROTOCOL_INFO).cast(),
            size_of::<FILE_REMOTE_PROTOCOL_INFO>() as u32,
        )
    } != 0
    {
        return Err(SafeFsError::UnsupportedSecureFilesystem {
            operation,
            reason: SecureFilesystemReason::RemoteFilesystem,
        });
    }
    // SAFETY: GetLastError has no pointer or lifetime preconditions.
    let remote_error = unsafe { GetLastError() };
    if remote_error != windows_sys::Win32::Foundation::ERROR_INVALID_PARAMETER
        && remote_error != windows_sys::Win32::Foundation::ERROR_NOT_SUPPORTED
    {
        return Err(SafeFsError::Os {
            operation,
            raw: RawOsError::Win32(remote_error),
        });
    }
    let kind = if tag.FileAttributes & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
        EntryKind::SymlinkOrReparse
    } else if standard.Directory {
        EntryKind::Directory
    } else {
        EntryKind::RegularFile
    };
    Ok(EntryMetadata {
        identity: StableIdentity::Windows {
            volume_serial: id.VolumeSerialNumber,
            file_id: id.FileId.Identifier,
        },
        kind,
        len: u64::try_from(standard.EndOfFile).map_err(|_| SafeFsError::InvalidNativeBuffer {
            operation,
            reason: NativeBufferReason::LengthOverflow,
        })?,
        link_count: u64::from(standard.NumberOfLinks),
        filesystem: Some(filesystem.clone()),
    })
}

fn query_reparse(handle: HANDLE) -> Result<RawLinkTarget> {
    #[repr(align(8))]
    struct Aligned([u8; MAXIMUM_REPARSE_DATA_BUFFER_SIZE as usize]);
    let mut storage = Aligned([0; MAXIMUM_REPARSE_DATA_BUFFER_SIZE as usize]);
    let mut iosb = IO_STATUS_BLOCK::default();
    // SAFETY: retained reparse HANDLE; aligned output buffer is writable for declared length; synchronous call.
    let status = unsafe {
        NtFsControlFile(
            handle,
            null_mut(),
            None,
            null(),
            &mut iosb,
            FSCTL_GET_REPARSE_POINT,
            null(),
            0,
            storage.0.as_mut_ptr().cast(),
            MAXIMUM_REPARSE_DATA_BUFFER_SIZE,
        )
    };
    complete_nt(SafeFsOperation::QueryReparsePoint, status, &iosb)?;
    let used = checked_information(SafeFsOperation::QueryReparsePoint, &iosb, storage.0.len())?;
    let (tag, bounded) = parse_reparse(&storage.0[..used])?;
    Ok(RawLinkTarget::Windows {
        tag,
        bytes: bounded,
    })
}

fn parse_reparse(bytes: &[u8]) -> Result<(u32, Vec<u8>)> {
    let malformed = || SafeFsError::InvalidNativeBuffer {
        operation: SafeFsOperation::ParseReparseBuffer,
        reason: NativeBufferReason::ReparseBufferMalformed,
    };
    if bytes.len() < REPARSE_HEADER_BYTES || bytes.len() > MAXIMUM_REPARSE_DATA_BUFFER_SIZE as usize
    {
        return Err(malformed());
    }
    let tag = u32::from_le_bytes(bytes[0..4].try_into().expect("four-byte tag"));
    let payload_len = usize::from(u16::from_le_bytes([bytes[4], bytes[5]]));
    let total = REPARSE_HEADER_BYTES
        .checked_add(payload_len)
        .ok_or_else(malformed)?;
    if total > bytes.len() || total > MAXIMUM_REPARSE_DATA_BUFFER_SIZE as usize {
        return Err(malformed());
    }
    let payload = &bytes[REPARSE_HEADER_BYTES..total];
    const MOUNT: u32 = 0xA000_0003;
    const SYMLINK: u32 = 0xA000_000C;
    let validate_range = |base: usize, offset: u16, length: u16| -> Result<()> {
        if !offset.is_multiple_of(2) || !length.is_multiple_of(2) {
            return Err(malformed());
        }
        let end = base
            .checked_add(usize::from(offset))
            .and_then(|start| start.checked_add(usize::from(length)))
            .ok_or_else(malformed)?;
        if end > payload.len() {
            return Err(malformed());
        }
        Ok(())
    };
    match tag {
        MOUNT => {
            if payload.len() < 8 {
                return Err(malformed());
            }
            validate_range(
                8,
                u16::from_le_bytes([payload[0], payload[1]]),
                u16::from_le_bytes([payload[2], payload[3]]),
            )?;
            validate_range(
                8,
                u16::from_le_bytes([payload[4], payload[5]]),
                u16::from_le_bytes([payload[6], payload[7]]),
            )?;
        }
        SYMLINK => {
            if payload.len() < 12 {
                return Err(malformed());
            }
            validate_range(
                12,
                u16::from_le_bytes([payload[0], payload[1]]),
                u16::from_le_bytes([payload[2], payload[3]]),
            )?;
            validate_range(
                12,
                u16::from_le_bytes([payload[4], payload[5]]),
                u16::from_le_bytes([payload[6], payload[7]]),
            )?;
            let flags = u32::from_le_bytes(payload[8..12].try_into().expect("four-byte flags"));
            if flags != 0 && flags != 1 {
                return Err(malformed());
            }
        }
        _ => {}
    }
    Ok((tag, bytes[..total].to_vec()))
}

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

    let malformed = || SafeFsError::InvalidNativeBuffer {
        operation: SafeFsOperation::ParseDirectoryBuffer,
        reason: NativeBufferReason::DirectoryBufferMalformed,
    };
    if bytes.is_empty() {
        return Err(malformed());
    }
    let mut names = Vec::new();
    let mut cursor = 0usize;
    let mut iterations = 0usize;
    let maximum = bytes.len() / NAME_OFFSET.max(1) + 1;
    loop {
        iterations += 1;
        if iterations > maximum || cursor > bytes.len() || bytes.len() - cursor < NAME_OFFSET {
            return Err(malformed());
        }
        let next_raw = field_u32(bytes, cursor, NEXT_OFFSET).ok_or_else(malformed)?;
        let name_raw = field_u32(bytes, cursor, NAME_LENGTH_OFFSET).ok_or_else(malformed)?;
        let name_bytes =
            usize::try_from(name_raw).map_err(|_| SafeFsError::InvalidNativeBuffer {
                operation: SafeFsOperation::ParseDirectoryBuffer,
                reason: NativeBufferReason::LengthOverflow,
            })?;
        let name_end = NAME_OFFSET
            .checked_add(name_bytes)
            .and_then(|value| cursor.checked_add(value))
            .ok_or_else(malformed)?;
        if name_bytes == 0 || name_bytes % 2 != 0 || name_end > bytes.len() {
            return Err(malformed());
        }
        let units_len = name_bytes / 2;
        let mut units = Vec::with_capacity(units_len);
        for index in 0..units_len {
            let offset = cursor + NAME_OFFSET + index * 2;
            units.push(u16::from_le_bytes([bytes[offset], bytes[offset + 1]]));
        }
        let os = OsString::from_wide(&units);
        if os != OsStr::new(".") && os != OsStr::new("..") {
            names.push(ComponentName::new(os)?);
        }

        let next = usize::try_from(next_raw).map_err(|_| malformed())?;
        if next == 0 {
            break;
        }
        if next % 8 != 0
            || next < NAME_OFFSET + name_bytes
            || cursor.checked_add(next).is_none()
            || cursor + next > bytes.len()
            || cursor + next <= cursor
        {
            return Err(malformed());
        }
        cursor += next;
    }
    Ok(names)
}

fn validated_directory_used(status: NTSTATUS, iosb: &IO_STATUS_BLOCK) -> Result<usize> {
    let used = checked_information(
        SafeFsOperation::EnumerateDirectory,
        iosb,
        DIRECTORY_BUFFER_BYTES,
    )?;
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
        let status = unsafe {
            NtQueryDirectoryFile(
                directory.native.node.handle.raw(),
                null_mut(),
                None,
                null(),
                &mut iosb,
                buffer.0.as_mut_ptr().cast(),
                DIRECTORY_BUFFER_BYTES as u32,
                FileDirectoryInformation,
                false,
                null(),
                first,
            )
        };
        first = false;
        if status == STATUS_NO_MORE_FILES {
            break;
        }
        if status < STATUS_SUCCESS && status != STATUS_BUFFER_OVERFLOW {
            return Err(nt_error(SafeFsOperation::EnumerateDirectory, status));
        }
        let used = validated_directory_used(status, &iosb)?;
        for name in parse_directory_batch(&buffer.0[..used])? {
            match query_child_nofollow(directory, &name)? {
                ChildState::Present(_) => output.push(name),
                ChildState::Absent => {
                    return Err(SafeFsError::NotFound {
                        operation: SafeFsOperation::EnumerateDirectory,
                    })
                }
            }
        }
    }
    output.sort_by(|left, right| {
        left.as_os_str()
            .encode_wide()
            .cmp(right.as_os_str().encode_wide())
    });
    Ok(output)
}

fn last_win32(operation: SafeFsOperation) -> SafeFsError {
    // SAFETY: GetLastError has no pointer or lifetime preconditions.
    SafeFsError::Os {
        operation,
        raw: RawOsError::Win32(unsafe { GetLastError() }),
    }
}

fn raw_nt(operation: SafeFsOperation, status: NTSTATUS) -> SafeFsError {
    // SAFETY: pure ntdll status conversion; raw NTSTATUS remains the primary diagnostic.
    let dos_error = unsafe { RtlNtStatusToDosError(status) };
    SafeFsError::Os {
        operation,
        raw: RawOsError::NtStatus { status, dos_error },
    }
}

fn nt_error(operation: SafeFsOperation, status: NTSTATUS) -> SafeFsError {
    match status {
        STATUS_OBJECT_NAME_NOT_FOUND | STATUS_OBJECT_PATH_NOT_FOUND => {
            SafeFsError::NotFound { operation }
        }
        STATUS_OBJECT_NAME_COLLISION
            if matches!(
                operation,
                SafeFsOperation::CreateDirectory
                    | SafeFsOperation::CreateStageDirectory
                    | SafeFsOperation::CreateFile
                    | SafeFsOperation::RenameNoReplaceSameParent
            ) =>
        {
            SafeFsError::AlreadyExists { operation }
        }
        STATUS_REPARSE_POINT_ENCOUNTERED => SafeFsError::SymlinkOrReparsePoint { operation },
        STATUS_NOT_SUPPORTED if operation == SafeFsOperation::QueryCaseMode => {
            SafeFsError::UnsupportedSecureFilesystem {
                operation,
                reason: SecureFilesystemReason::CaseSemanticsUnavailable,
            }
        }
        STATUS_NOT_SUPPORTED if operation == SafeFsOperation::RenameNoReplaceSameParent => {
            SafeFsError::UnsupportedAtomicPublish {
                operation,
                reason: AtomicPublishReason::PrimitiveUnavailable,
            }
        }
        STATUS_INVALID_PARAMETER | STATUS_INFO_LENGTH_MISMATCH | STATUS_BUFFER_TOO_SMALL => {
            SafeFsError::InvalidNativeBuffer {
                operation,
                reason: if operation == SafeFsOperation::RenameNoReplaceSameParent {
                    NativeBufferReason::RenameLayoutMalformed
                } else {
                    NativeBufferReason::LengthOverflow
                },
            }
        }
        STATUS_NOT_A_DIRECTORY | STATUS_FILE_IS_A_DIRECTORY | STATUS_OBJECT_TYPE_MISMATCH => {
            SafeFsError::UnsupportedEntryType {
                operation,
                kind: EntryKind::Other,
            }
        }
        STATUS_ACCESS_DENIED
        | STATUS_SHARING_VIOLATION
        | STATUS_DELETE_PENDING
        | STATUS_CANNOT_DELETE
        | STATUS_DIRECTORY_NOT_EMPTY
        | STATUS_BUFFER_OVERFLOW
        | STATUS_NO_MORE_FILES
        | STATUS_END_OF_FILE
        | STATUS_PENDING
        | STATUS_NOT_SUPPORTED => raw_nt(operation, status),
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
        return SafeFsError::AlreadyExists {
            operation: SafeFsOperation::RenameNoReplaceSameParent,
        };
    }
    let ambiguous = matches!(
        status,
        STATUS_ACCESS_DENIED
            | STATUS_SHARING_VIOLATION
            | STATUS_DELETE_PENDING
            | STATUS_CANNOT_DELETE
            | STATUS_DIRECTORY_NOT_EMPTY
    );
    if ambiguous
        && preflight_absent
        && source_had_delete
        && matches!(postflight, Ok(ChildState::Present(_)))
    {
        SafeFsError::AlreadyExists {
            operation: SafeFsOperation::RenameNoReplaceSameParent,
        }
    } else {
        raw_nt(SafeFsOperation::RenameNoReplaceSameParent, status)
    }
}

fn wide_z(value: &OsStr) -> Result<Vec<u16>> {
    let mut units: Vec<u16> = value.encode_wide().collect();
    if units.contains(&0) {
        return Err(SafeFsError::InvalidComponent(
            ComponentViolation::EmbeddedNul,
        ));
    }
    units.push(0);
    Ok(units)
}

fn fixed_wide(buffer: &[u16]) -> Result<Vec<u16>> {
    let end =
        buffer
            .iter()
            .position(|unit| *unit == 0)
            .ok_or(SafeFsError::InvalidNativeBuffer {
                operation: SafeFsOperation::ProbeVolume,
                reason: NativeBufferReason::LengthOverflow,
            })?;
    Ok(buffer[..end].to_vec())
}

fn absolute_component_names(path: &Path) -> Result<Vec<ComponentName>> {
    let mut parts = path.components();
    match parts.next() {
        Some(Component::Prefix(prefix))
            if matches!(prefix.kind(), Prefix::Disk(_) | Prefix::VerbatimDisk(_)) => {}
        _ => {
            return Err(SafeFsError::UnsupportedSecureFilesystem {
                operation: SafeFsOperation::CaptureNamespaceRoot,
                reason: SecureFilesystemReason::UnstableMapping,
            })
        }
    }
    if !matches!(parts.next(), Some(Component::RootDir)) {
        return Err(SafeFsError::InvalidRelativePath(
            RelativePathViolation::AbsoluteOrPrefix,
        ));
    }
    parts
        .map(|part| match part {
            Component::Normal(value) => ComponentName::new(value),
            _ => Err(SafeFsError::InvalidRelativePath(
                RelativePathViolation::AbsoluteOrPrefix,
            )),
        })
        .collect()
}

fn root_capture_desired(access: DirectoryAccess) -> Result<FILE_ACCESS_RIGHTS> {
    let base = FILE_LIST_DIRECTORY | FILE_TRAVERSE | FILE_READ_ATTRIBUTES | SYNCHRONIZE;
    match access {
        DirectoryAccess::Read => Ok(base),
        DirectoryAccess::MutateChildren => {
            Ok(base | FILE_ADD_FILE | FILE_ADD_SUBDIRECTORY | FILE_DELETE_CHILD)
        }
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
    if unsafe {
        GetVolumePathNameW(
            input.as_ptr(),
            mapping_buffer.as_mut_ptr(),
            mapping_buffer.len() as u32,
        )
    } == 0
    {
        return Err(last_win32(SafeFsOperation::ProbeVolume));
    }
    let mapping = fixed_wide(&mapping_buffer)?;
    let mut mapping_z = mapping.clone();
    mapping_z.push(0);
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
    if unsafe {
        GetVolumeNameForVolumeMountPointW(
            mapping_z.as_ptr(),
            guid_buffer.as_mut_ptr(),
            guid_buffer.len() as u32,
        )
    } == 0
    {
        return Err(last_win32(SafeFsOperation::ProbeVolume));
    }
    let guid = fixed_wide(&guid_buffer)?;
    let desired = root_capture_desired(access)?;
    // SAFETY: mapped root path is nul terminated; security/template null; synchronous directory open.
    let raw = unsafe {
        CreateFileW(
            mapping_z.as_ptr(),
            desired,
            SHARE,
            null(),
            OPEN_EXISTING,
            FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT,
            null_mut(),
        )
    };
    let root = OwnedHandle::new(raw, SafeFsOperation::ProbeVolume)?;
    let mut volume_serial32 = 0u32;
    // SAFETY: retained volume root and writable serial output; unused output buffers null.
    if unsafe {
        GetVolumeInformationByHandleW(
            root.raw(),
            null_mut(),
            0,
            &mut volume_serial32,
            null_mut(),
            null_mut(),
            null_mut(),
            0,
        )
    } == 0
    {
        return Err(last_win32(SafeFsOperation::ProbeVolume));
    }
    let id: FILE_ID_INFO = win32_query(root.raw(), FileIdInfo, SafeFsOperation::ProbeVolume)?;
    let mut remote = remote_protocol_query_buffer(SafeFsOperation::ProbeVolume)?;
    // SAFETY: the fixed output is 8-byte aligned, writable, and root retained.
    let remote_ok = unsafe {
        GetFileInformationByHandleEx(
            root.raw(),
            FileRemoteProtocolInfo,
            (&mut remote.info as *mut FILE_REMOTE_PROTOCOL_INFO).cast(),
            size_of::<FILE_REMOTE_PROTOCOL_INFO>() as u32,
        )
    };
    if remote_ok != 0 {
        return Err(SafeFsError::UnsupportedSecureFilesystem {
            operation: SafeFsOperation::ProbeVolume,
            reason: SecureFilesystemReason::RemoteFilesystem,
        });
    }
    // SAFETY: GetLastError has no pointer or lifetime preconditions.
    let remote_error = unsafe { GetLastError() };
    if remote_error != windows_sys::Win32::Foundation::ERROR_INVALID_PARAMETER
        && remote_error != windows_sys::Win32::Foundation::ERROR_NOT_SUPPORTED
    {
        return Err(SafeFsError::Os {
            operation: SafeFsOperation::ProbeVolume,
            raw: RawOsError::Win32(remote_error),
        });
    }
    let proof = VolumeProof {
        mapping,
        guid: guid.clone(),
        volume_serial32,
        volume_serial: id.VolumeSerialNumber,
        root_id: id.FileId.Identifier,
    };
    let filesystem = LocalFilesystemSnapshot::Windows {
        volume_guid: guid,
        serial: id.VolumeSerialNumber,
    };
    Ok((root, proof, filesystem))
}

fn append_snapshot(
    parent: &NamespaceSnapshot,
    name: ComponentName,
    opened: &EntryMetadata,
    case_mode: CaseMode,
) -> Result<NamespaceSnapshot> {
    let filesystem = opened
        .filesystem
        .clone()
        .ok_or(SafeFsError::UnsupportedSecureFilesystem {
            operation: SafeFsOperation::ProbeFilesystem,
            reason: SecureFilesystemReason::FilesystemProbeUnavailable,
        })?;
    let mut snapshot = parent.clone();
    snapshot.components.push(NamespaceComponent {
        name,
        identity: opened.identity.clone(),
        filesystem,
        case_mode,
    });
    Ok(snapshot)
}

#[allow(clippy::arc_with_non_send_sync)] // Arc retains the HANDLE-bearing parent chain; it is not shared publicly.
fn open_directory_contract(
    parent: &DirectoryAuthority,
    name: &ComponentName,
    access: DirectoryAccess,
    contract: OpenContract,
) -> Result<DirectoryAuthority> {
    let handle = nt_create_relative(
        parent.native.node.handle.raw(),
        name,
        parent.case_mode,
        contract.desired,
        contract.disposition,
        contract.options,
        contract.attributes,
        null(),
        SafeFsOperation::OpenDirectory,
    )?;
    let filesystem =
        parent
            .opened
            .filesystem
            .as_ref()
            .ok_or(SafeFsError::UnsupportedSecureFilesystem {
                operation: SafeFsOperation::ProbeFilesystem,
                reason: SecureFilesystemReason::FilesystemProbeUnavailable,
            })?;
    let opened = query_entry_metadata(handle.raw(), filesystem, SafeFsOperation::OpenDirectory)?;
    if opened.kind == EntryKind::SymlinkOrReparse {
        return Err(SafeFsError::SymlinkOrReparsePoint {
            operation: SafeFsOperation::OpenDirectory,
        });
    }
    if opened.kind != EntryKind::Directory {
        return Err(SafeFsError::UnsupportedEntryType {
            operation: SafeFsOperation::OpenDirectory,
            kind: opened.kind,
        });
    }
    let case_mode = query_case_mode(handle.raw())?;
    let snapshot = append_snapshot(&parent.snapshot, name.clone(), &opened, case_mode)?;
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
        native: NativeDirectory {
            node,
            access,
            delete_right: contract.delete_right,
        },
        access,
        opened,
        case_mode,
        snapshot,
    })
}

fn require_mutation(parent: &DirectoryAuthority, operation: SafeFsOperation) -> Result<()> {
    if parent.access == DirectoryAccess::Read {
        Err(SafeFsError::AccessMismatch { operation })
    } else {
        Ok(())
    }
}

struct OwnerOnlySecurity {
    sid: Vec<usize>,
    _acl: Vec<usize>,
    descriptor: Box<SECURITY_DESCRIPTOR>,
    ace_flags: ACE_FLAGS,
}

impl OwnerOnlySecurity {
    fn new(directory: bool) -> Result<Self> {
        let operation = SafeFsOperation::VerifySecurityDescriptor;
        let mut token_raw = null_mut();
        // SAFETY: the current-process pseudo-handle is valid and the output pointer is writable.
        if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token_raw) } == 0 {
            return Err(last_win32(operation));
        }
        let token = OwnedHandle::new(token_raw, operation)?;
        let mut needed = 0u32;
        // SAFETY: documented sizing call with a null output buffer.
        let first =
            unsafe { GetTokenInformation(token.raw(), TokenOwner, null_mut(), 0, &mut needed) };
        if first != 0
            || unsafe { GetLastError() }
                != windows_sys::Win32::Foundation::ERROR_INSUFFICIENT_BUFFER
            || needed < size_of::<TOKEN_OWNER>() as u32
        {
            return Err(last_win32(operation));
        }
        let mut token_words = vec![0usize; (needed as usize).div_ceil(size_of::<usize>())];
        // SAFETY: aligned storage is writable for exactly `needed` bytes.
        if unsafe {
            GetTokenInformation(
                token.raw(),
                TokenOwner,
                token_words.as_mut_ptr().cast(),
                needed,
                &mut needed,
            )
        } == 0
        {
            return Err(last_win32(operation));
        }
        // SAFETY: the successful TokenOwner query initialized a TOKEN_OWNER value.
        let owner = unsafe { (*(token_words.as_ptr().cast::<TOKEN_OWNER>())).Owner };
        if owner.is_null() || unsafe { IsValidSid(owner) } == 0 {
            return Err(last_win32(operation));
        }
        // SAFETY: `owner` is a validated SID returned in the live token buffer.
        let sid_len = usize::try_from(unsafe { GetLengthSid(owner) }).map_err(|_| {
            SafeFsError::InvalidNativeBuffer {
                operation,
                reason: NativeBufferReason::SecurityDescriptorMalformed,
            }
        })?;
        let mut sid = vec![0usize; sid_len.div_ceil(size_of::<usize>())];
        // SAFETY: destination capacity is at least sid_len and owner is a validated SID.
        if unsafe { CopySid(sid_len as u32, sid.as_mut_ptr().cast(), owner) } == 0 {
            return Err(last_win32(operation));
        }
        drop(token_words);
        drop(token);

        let acl_bytes = size_of::<ACL>()
            .checked_add(size_of::<ACCESS_ALLOWED_ACE>() - size_of::<u32>())
            .and_then(|value| value.checked_add(sid_len))
            .ok_or(SafeFsError::InvalidNativeBuffer {
                operation,
                reason: NativeBufferReason::LengthOverflow,
            })?;
        let acl_len = u32::try_from(acl_bytes).map_err(|_| SafeFsError::InvalidNativeBuffer {
            operation,
            reason: NativeBufferReason::LengthOverflow,
        })?;
        if acl_bytes > u16::MAX as usize {
            return Err(SafeFsError::InvalidNativeBuffer {
                operation,
                reason: NativeBufferReason::LengthOverflow,
            });
        }
        let mut acl = vec![0usize; acl_bytes.div_ceil(size_of::<usize>())];
        let ace_flags = if directory {
            OBJECT_INHERIT_ACE | CONTAINER_INHERIT_ACE
        } else {
            0
        };
        // SAFETY: aligned ACL storage and the copied SID remain live inside Self.
        if unsafe { InitializeAcl(acl.as_mut_ptr().cast(), acl_len, ACL_REVISION) } == 0
            || unsafe {
                AddAccessAllowedAceEx(
                    acl.as_mut_ptr().cast(),
                    ACL_REVISION,
                    ace_flags,
                    FILE_ALL_ACCESS,
                    sid.as_mut_ptr().cast(),
                )
            } == 0
        {
            return Err(last_win32(operation));
        }
        // SAFETY: SECURITY_DESCRIPTOR is a C POD initialized immediately below.
        let mut descriptor = Box::<SECURITY_DESCRIPTOR>::new(unsafe { std::mem::zeroed() });
        // SAFETY: the boxed descriptor has a stable address and ACL storage remains owned by Self.
        if unsafe {
            InitializeSecurityDescriptor(
                (&mut *descriptor as *mut SECURITY_DESCRIPTOR).cast(),
                SECURITY_DESCRIPTOR_REVISION,
            )
        } == 0
            || unsafe {
                SetSecurityDescriptorDacl(
                    (&mut *descriptor as *mut SECURITY_DESCRIPTOR).cast(),
                    BOOL_TRUE,
                    acl.as_mut_ptr().cast(),
                    BOOL_FALSE,
                )
            } == 0
            || unsafe {
                SetSecurityDescriptorControl(
                    (&mut *descriptor as *mut SECURITY_DESCRIPTOR).cast(),
                    SE_DACL_PROTECTED,
                    SE_DACL_PROTECTED,
                )
            } == 0
        {
            return Err(last_win32(operation));
        }
        Ok(Self {
            sid,
            _acl: acl,
            descriptor,
            ace_flags,
        })
    }

    fn descriptor_ptr(&self) -> *const SECURITY_DESCRIPTOR {
        &*self.descriptor
    }
}

fn malformed_security() -> SafeFsError {
    SafeFsError::InvalidNativeBuffer {
        operation: SafeFsOperation::VerifySecurityDescriptor,
        reason: NativeBufferReason::SecurityDescriptorMalformed,
    }
}

fn checked_subslice(
    base: usize,
    length: usize,
    pointer: usize,
    needed: usize,
) -> Result<std::ops::Range<usize>> {
    let end = base.checked_add(length).ok_or_else(malformed_security)?;
    let pointer_end = pointer.checked_add(needed).ok_or_else(malformed_security)?;
    if pointer < base || pointer_end > end {
        return Err(malformed_security());
    }
    Ok(pointer - base..pointer_end - base)
}

fn checked_sid_length(buffer: &[u8], sid: *const c_void) -> Result<usize> {
    const SID_PREFIX: usize = 8;
    let range = checked_subslice(
        buffer.as_ptr() as usize,
        buffer.len(),
        sid as usize,
        SID_PREFIX,
    )?;
    let count = usize::from(buffer[range.start + 1]);
    let length = SID_PREFIX
        .checked_add(
            count
                .checked_mul(size_of::<u32>())
                .ok_or_else(malformed_security)?,
        )
        .ok_or_else(malformed_security)?;
    checked_subslice(buffer.as_ptr() as usize, buffer.len(), sid as usize, length)?;
    // SAFETY: the SID prefix and every declared sub-authority are inside buffer.
    if unsafe { IsValidSid(sid.cast_mut()) } == 0 {
        return Err(malformed_security());
    }
    // SAFETY: IsValidSid accepted the fully bounded SID.
    if usize::try_from(unsafe { GetLengthSid(sid.cast_mut()) }).map_err(|_| malformed_security())?
        != length
    {
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
    let dacl_start = dacl as usize;
    let dacl_range = checked_subslice(
        descriptor_bytes.as_ptr() as usize,
        descriptor_bytes.len(),
        dacl_start,
        acl_bytes_in_use.max(size_of::<ACL>()),
    )?;
    if acl_bytes_in_use < size_of::<ACL>() || dacl_range.len() != acl_bytes_in_use {
        return Err(malformed_security());
    }
    let ace_start = ace as usize;
    checked_subslice(
        dacl_start,
        acl_bytes_in_use,
        ace_start,
        size_of::<windows_sys::Win32::Security::ACE_HEADER>(),
    )?;
    // SAFETY: only the fixed ACE header bytes were bounds checked; read unaligned.
    let header =
        unsafe { std::ptr::read_unaligned(ace.cast::<windows_sys::Win32::Security::ACE_HEADER>()) };
    if u32::from(header.AceType) != ACCESS_ALLOWED_ACE_TYPE {
        return Err(malformed_security());
    }
    let ace_size = usize::from(header.AceSize);
    let sid_offset = offset_of!(ACCESS_ALLOWED_ACE, SidStart);
    if ace_size < sid_offset.checked_add(8).ok_or_else(malformed_security)? {
        return Err(malformed_security());
    }
    checked_subslice(dacl_start, acl_bytes_in_use, ace_start, ace_size)?;
    let sid_ptr = ace_start
        .checked_add(sid_offset)
        .ok_or_else(malformed_security)? as *const c_void;
    let sid_length = checked_sid_length(descriptor_bytes, sid_ptr)?;
    if sid_offset
        .checked_add(sid_length)
        .ok_or_else(malformed_security)?
        != ace_size
    {
        return Err(malformed_security());
    }
    // SAFETY: ACE type, size, ACL bounds and SID range were established above.
    let allowed = unsafe { std::ptr::read_unaligned(ace.cast::<ACCESS_ALLOWED_ACE>()) };
    let expected_flags = u8::try_from(expected.ace_flags).map_err(|_| malformed_security())?;
    if allowed.Header.AceFlags != expected_flags
        || allowed.Mask != FILE_ALL_ACCESS
        || unsafe { EqualSid(sid_ptr.cast_mut(), expected.sid.as_ptr().cast_mut().cast()) } == 0
    {
        return Err(malformed_security());
    }
    Ok(())
}

fn verify_owner_only(handle: HANDLE, expected: &OwnerOnlySecurity) -> Result<()> {
    let operation = SafeFsOperation::VerifySecurityDescriptor;
    let information = OWNER_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION;
    let mut needed = 0u32;
    // SAFETY: documented sizing call against a retained handle.
    unsafe { GetKernelObjectSecurity(handle, information, null_mut(), 0, &mut needed) };
    if unsafe { GetLastError() } != windows_sys::Win32::Foundation::ERROR_INSUFFICIENT_BUFFER {
        return Err(last_win32(operation));
    }
    let mut words = vec![0usize; (needed as usize).div_ceil(size_of::<usize>())];
    // SAFETY: aligned storage is writable for exactly needed bytes.
    if unsafe {
        GetKernelObjectSecurity(
            handle,
            information,
            words.as_mut_ptr().cast(),
            needed,
            &mut needed,
        )
    } == 0
    {
        return Err(last_win32(operation));
    }
    // SAFETY: the successful query initialized exactly `needed` bytes.
    let descriptor_bytes =
        unsafe { std::slice::from_raw_parts_mut(words.as_mut_ptr().cast::<u8>(), needed as usize) };
    if descriptor_bytes.len() < size_of::<SECURITY_DESCRIPTOR>() {
        return Err(malformed_security());
    }
    let descriptor = descriptor_bytes.as_mut_ptr().cast::<SECURITY_DESCRIPTOR>();
    let mut control = 0u16;
    let mut revision = 0u32;
    let mut owner = null_mut();
    let mut owner_defaulted = BOOL_FALSE;
    let mut dacl = null_mut();
    let mut present = BOOL_FALSE;
    let mut defaulted = BOOL_FALSE;
    // SAFETY: the kernel returned a self-relative descriptor in aligned storage.
    if unsafe { GetSecurityDescriptorControl(descriptor.cast(), &mut control, &mut revision) } == 0
        || unsafe {
            GetSecurityDescriptorOwner(descriptor.cast(), &mut owner, &mut owner_defaulted)
        } == 0
        || unsafe {
            GetSecurityDescriptorDacl(descriptor.cast(), &mut present, &mut dacl, &mut defaulted)
        } == 0
        || control & SE_DACL_PROTECTED == 0
        || owner_defaulted != BOOL_FALSE
        || present == BOOL_FALSE
        || defaulted != BOOL_FALSE
        || dacl.is_null()
        || owner.is_null()
    {
        return Err(malformed_security());
    }
    checked_sid_length(descriptor_bytes, owner.cast_const())?;
    // SAFETY: owner SID is fully bounded and validated in descriptor_bytes.
    if unsafe { EqualSid(owner, expected.sid.as_ptr().cast_mut().cast()) } == 0 {
        return Err(malformed_security());
    }
    checked_subslice(
        descriptor_bytes.as_ptr() as usize,
        descriptor_bytes.len(),
        dacl as usize,
        size_of::<ACL>(),
    )?;
    let mut acl_info = ACL_SIZE_INFORMATION::default();
    // SAFETY: the DACL fixed header is bounded and output is writable.
    if unsafe {
        GetAclInformation(
            dacl,
            (&mut acl_info as *mut ACL_SIZE_INFORMATION).cast(),
            size_of::<ACL_SIZE_INFORMATION>() as u32,
            AclSizeInformation,
        )
    } == 0
        || acl_info.AceCount != 1
    {
        return Err(malformed_security());
    }
    let acl_bytes_in_use =
        usize::try_from(acl_info.AclBytesInUse).map_err(|_| malformed_security())?;
    checked_subslice(
        descriptor_bytes.as_ptr() as usize,
        descriptor_bytes.len(),
        dacl as usize,
        acl_bytes_in_use.max(size_of::<ACL>()),
    )?;
    let mut ace = null_mut();
    // SAFETY: the ACL and AclBytesInUse are bounded inside descriptor storage.
    if unsafe { GetAce(dacl, 0, &mut ace) } == 0 || ace.is_null() {
        return Err(last_win32(operation));
    }
    verify_single_owner_ace(descriptor_bytes, dacl, acl_bytes_in_use, ace, expected)
}

#[cfg(test)]
static FORCE_DACL_VERIFY_FAILURE: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

#[cfg(test)]
fn force_next_owner_verification_failure() {
    FORCE_DACL_VERIFY_FAILURE.store(true, std::sync::atomic::Ordering::SeqCst);
}

fn verify_created_owner_only(handle: HANDLE, expected: &OwnerOnlySecurity) -> Result<()> {
    inject_windows_create_failure(
        WindowsCreateFailurePoint::SecurityVerification,
        SafeFsOperation::VerifySecurityDescriptor,
    )?;
    #[cfg(test)]
    if FORCE_DACL_VERIFY_FAILURE.swap(false, std::sync::atomic::Ordering::SeqCst) {
        return Err(malformed_security());
    }
    verify_owner_only(handle, expected)
}

#[allow(clippy::arc_with_non_send_sync)] // Arc retains the HANDLE-bearing parent chain; it is not shared publicly.
fn duplicate_directory(source: &DirectoryAuthority) -> Result<DirectoryAuthority> {
    let mut duplicated = null_mut();
    // SAFETY: retained source HANDLE; current-process source/target; output writable; same access only.
    if unsafe {
        DuplicateHandle(
            GetCurrentProcess(),
            source.native.node.handle.raw(),
            GetCurrentProcess(),
            &mut duplicated,
            0,
            BOOL_FALSE,
            DUPLICATE_SAME_ACCESS,
        )
    } == 0
    {
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
        native: NativeDirectory {
            node,
            access: source.native.access,
            delete_right: source.native.delete_right,
        },
        access: source.access,
        opened: source.opened.clone(),
        case_mode: source.case_mode,
        snapshot: source.snapshot.clone(),
    })
}

fn mark_delete_handle(handle: HANDLE, operation: SafeFsOperation) -> Result<()> {
    #[cfg(test)]
    if FAIL_NEXT_CREATED_DISPOSITION.swap(false, std::sync::atomic::Ordering::SeqCst) {
        return Err(SafeFsError::io(
            operation,
            io::Error::other("injected created disposition failure"),
        ));
    }
    let info = FILE_DISPOSITION_INFORMATION { DeleteFile: true };
    let mut iosb = IO_STATUS_BLOCK::default();
    // SAFETY: caller owns a live DELETE handle; initialized fixed info and writable iosb stay live.
    let status = unsafe {
        NtSetInformationFile(
            handle,
            &mut iosb,
            (&info as *const FILE_DISPOSITION_INFORMATION).cast(),
            u32::try_from(size_of::<FILE_DISPOSITION_INFORMATION>())
                .expect("disposition info fits"),
            FileDispositionInformation,
        )
    };
    complete_nt(operation, status, &iosb)
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

fn rollback_created_directory<T>(
    directory: DirectoryAuthority,
    original: SafeFsError,
) -> Result<T> {
    let node =
        Arc::try_unwrap(directory.native.node).map_err(|_| SafeFsError::StageIdentityLost {
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

#[allow(clippy::arc_with_non_send_sync)] // Arc retains the HANDLE-bearing parent chain; it is not shared publicly.
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
    let security = match permissions {
        CreatePermissions::OwnerOnly => Some(OwnerOnlySecurity::new(true)?),
        CreatePermissions::Inherit => None,
    };
    let security_descriptor = security
        .as_ref()
        .map_or(null(), OwnerOnlySecurity::descriptor_ptr);
    let handle = nt_create_relative(
        parent.native.node.handle.raw(),
        name,
        parent.case_mode,
        contract.desired,
        contract.disposition,
        contract.options,
        contract.attributes,
        security_descriptor,
        operation,
    )?;
    let validated =
        (|| -> Result<(EntryMetadata, CaseMode, NamespaceSnapshot)> {
            inject_windows_create_failure(WindowsCreateFailurePoint::FilesystemProbe, operation)?;
            let filesystem = parent.opened.filesystem.as_ref().ok_or(
                SafeFsError::UnsupportedSecureFilesystem {
                    operation: SafeFsOperation::ProbeFilesystem,
                    reason: SecureFilesystemReason::FilesystemProbeUnavailable,
                },
            )?;
            inject_windows_create_failure(WindowsCreateFailurePoint::Metadata, operation)?;
            let opened = query_entry_metadata(handle.raw(), filesystem, operation)?;
            inject_windows_create_failure(WindowsCreateFailurePoint::TypeValidation, operation)?;
            if opened.kind != EntryKind::Directory {
                return Err(SafeFsError::UnsupportedEntryType {
                    operation,
                    kind: opened.kind,
                });
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
        native: NativeDirectory {
            node,
            access,
            delete_right: contract.delete_right,
        },
        access,
        opened,
        case_mode,
        snapshot,
    })
}

#[allow(dead_code)] // Task 6B parent symbol; Task 7B removes this when public rename calls it.
struct RenameInformationBuffer {
    storage: Vec<usize>,
    used: u32,
}

const _: () = assert!(align_of::<usize>() >= align_of::<FILE_RENAME_INFORMATION>());

#[allow(dead_code)] // Builder is first called by Task 7B test-only bodies.
impl RenameInformationBuffer {
    fn new(parent: HANDLE, target: &ComponentName) -> Result<Self> {
        let units: Vec<u16> = target.as_os_str().encode_wide().collect();
        let name_bytes = units
            .len()
            .checked_mul(size_of::<u16>())
            .and_then(|value| u32::try_from(value).ok())
            .ok_or(SafeFsError::InvalidComponent(ComponentViolation::TooLong))?;
        let name_offset = offset_of!(FILE_RENAME_INFORMATION, FileName);
        let total = name_offset.checked_add(name_bytes as usize).ok_or(
            SafeFsError::InvalidNativeBuffer {
                operation: SafeFsOperation::RenameNoReplaceSameParent,
                reason: NativeBufferReason::RenameLayoutMalformed,
            },
        )?;
        let mut storage = vec![0usize; total.div_ceil(size_of::<usize>()).max(1)];
        let base = storage.as_mut_ptr().cast::<FILE_RENAME_INFORMATION>();
        debug_assert_eq!((base as usize) % align_of::<FILE_RENAME_INFORMATION>(), 0);
        debug_assert!(storage.len() * size_of::<usize>() >= total);
        // SAFETY: usize storage is sufficiently aligned and has at least total bytes; source units live for copy.
        unsafe {
            (*base).Anonymous.ReplaceIfExists = false;
            (*base).RootDirectory = parent;
            (*base).FileNameLength = name_bytes;
            std::ptr::copy_nonoverlapping(
                units.as_ptr().cast::<u8>(),
                base.cast::<u8>().add(name_offset),
                name_bytes as usize,
            );
        }
        Ok(Self {
            storage,
            used: u32::try_from(total).map_err(|_| SafeFsError::InvalidNativeBuffer {
                operation: SafeFsOperation::RenameNoReplaceSameParent,
                reason: NativeBufferReason::RenameLayoutMalformed,
            })?,
        })
    }

    fn as_ptr(&self) -> *const c_void {
        self.storage.as_ptr().cast()
    }
}

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
        *REVALIDATION_HOOK
            .get_or_init(Default::default)
            .lock()
            .expect("revalidation hook mutex poisoned") = None;
    }
}

#[cfg(test)]
#[allow(dead_code)] // First call site is in Task 7B test-only bodies.
fn install_revalidation_hook(hook: RevalidationHook) -> RevalidationHookGuard {
    let mut slot = REVALIDATION_HOOK
        .get_or_init(Default::default)
        .lock()
        .expect("revalidation hook mutex poisoned");
    assert!(
        slot.is_none(),
        "revalidation tests require --test-threads=1"
    );
    *slot = Some(hook);
    RevalidationHookGuard
}

fn collect_revalidation_proof(directory: &DirectoryAuthority) -> Result<RevalidationProof> {
    #[cfg(test)]
    {
        let hook = REVALIDATION_HOOK
            .get_or_init(Default::default)
            .lock()
            .expect("revalidation hook mutex poisoned")
            .clone();
        if let Some(hook) = hook {
            return hook(directory);
        }
    }
    let mut path = directory.anchor.native.absolute_path.clone();
    for row in directory
        .snapshot
        .components
        .iter()
        .skip(directory.anchor.native.base_components)
    {
        path.push(row.name.as_os_str());
    }
    let fresh =
        capture_absolute_directory(&path, directory.anchor.native.access).map_err(|_| {
            SafeFsError::NamespaceChanged {
                operation: SafeFsOperation::RevalidateNamespace,
            }
        })?;
    Ok(RevalidationProof {
        volume: fresh.anchor.native.mapping.clone(),
        snapshot: fresh.snapshot,
        remote: false,
    })
}

#[allow(clippy::arc_with_non_send_sync)] // Arc retains the HANDLE-bearing namespace chain; it is not shared publicly.
pub(super) fn capture_absolute_directory(
    path: &Path,
    access: DirectoryAccess,
) -> Result<DirectoryAuthority> {
    if access == DirectoryAccess::Stage {
        return Err(SafeFsError::AccessMismatch {
            operation: SafeFsOperation::CaptureNamespaceRoot,
        });
    }
    let _validated_absolute = absolute_component_names(path)?;
    let (root_handle, volume, filesystem) = probe_volume(path, access)?;
    let mapping_path = PathBuf::from(OsString::from_wide(&volume.mapping));
    let relative =
        path.strip_prefix(&mapping_path)
            .map_err(|_| SafeFsError::UnsupportedSecureFilesystem {
                operation: SafeFsOperation::CaptureNamespaceRoot,
                reason: SecureFilesystemReason::UnstableMapping,
            })?;
    let names: Vec<ComponentName> = relative
        .components()
        .map(|part| match part {
            Component::Normal(value) => ComponentName::new(value),
            _ => Err(SafeFsError::InvalidRelativePath(
                RelativePathViolation::AbsoluteOrPrefix,
            )),
        })
        .collect::<Result<_>>()?;
    let root_opened = query_entry_metadata(
        root_handle.raw(),
        &filesystem,
        SafeFsOperation::CaptureNamespaceRoot,
    )?;
    if root_opened.kind == EntryKind::SymlinkOrReparse {
        return Err(SafeFsError::SymlinkOrReparsePoint {
            operation: SafeFsOperation::CaptureNamespaceRoot,
        });
    }
    if root_opened.kind != EntryKind::Directory {
        return Err(SafeFsError::UnsupportedEntryType {
            operation: SafeFsOperation::CaptureNamespaceRoot,
            kind: root_opened.kind,
        });
    }
    let root_case = query_case_mode(root_handle.raw())?;
    let root_node = Arc::new(DirectoryNode {
        handle: root_handle,
        parent: None,
        name: None,
        case_mode: root_case,
        metadata: root_opened.clone(),
        volume: volume.clone(),
    });
    let anchor = Arc::new(NamespaceAnchor {
        native: NativeNamespaceAnchor {
            root: Arc::clone(&root_node),
            mapping: volume,
            absolute_path: path.to_path_buf(),
            base_components: names.len(),
            access,
        },
    });
    let snapshot = NamespaceSnapshot {
        root_identity: root_opened.identity.clone(),
        root_filesystem: filesystem,
        root_case_mode: root_case,
        components: Vec::new(),
    };
    let mut current = DirectoryAuthority {
        anchor,
        native: NativeDirectory {
            node: root_node,
            access,
            delete_right: false,
        },
        access,
        opened: root_opened,
        case_mode: root_case,
        snapshot,
    };
    for name in names {
        let operation = if access == DirectoryAccess::Read {
            OpenOperation::DirRead
        } else {
            OpenOperation::DirMutate
        };
        current =
            open_directory_contract(&current, &name, access, contract_for_operation(operation))?;
    }
    Ok(current)
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
    if exact {
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
    let contract = contract_for_operation(OpenOperation::Query);
    let handle = match nt_create_relative(
        parent.native.node.handle.raw(),
        name,
        parent.case_mode,
        contract.desired,
        contract.disposition,
        contract.options,
        contract.attributes,
        null(),
        SafeFsOperation::QueryChild,
    ) {
        Ok(handle) => handle,
        Err(SafeFsError::NotFound { .. }) => return Ok(ChildState::Absent),
        Err(error) => return Err(error),
    };
    let filesystem =
        parent
            .opened
            .filesystem
            .as_ref()
            .ok_or(SafeFsError::UnsupportedSecureFilesystem {
                operation: SafeFsOperation::ProbeFilesystem,
                reason: SecureFilesystemReason::FilesystemProbeUnavailable,
            })?;
    Ok(ChildState::Present(query_entry_metadata(
        handle.raw(),
        filesystem,
        SafeFsOperation::QueryChild,
    )?))
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
    let operation = if access == DirectoryAccess::Read {
        OpenOperation::DirRead
    } else {
        OpenOperation::DirMutate
    };
    open_directory_contract(parent, name, access, contract_for_operation(operation))
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
    let contract = contract_for_operation(if access == FileAccess::Read {
        OpenOperation::FileRead
    } else {
        OpenOperation::FileWrite
    });
    let handle = nt_create_relative(
        parent.native.node.handle.raw(),
        name,
        parent.case_mode,
        contract.desired,
        contract.disposition,
        contract.options,
        contract.attributes,
        null(),
        SafeFsOperation::OpenFile,
    )?;
    let filesystem =
        parent
            .opened
            .filesystem
            .as_ref()
            .ok_or(SafeFsError::UnsupportedSecureFilesystem {
                operation: SafeFsOperation::ProbeFilesystem,
                reason: SecureFilesystemReason::FilesystemProbeUnavailable,
            })?;
    let opened = query_entry_metadata(handle.raw(), filesystem, SafeFsOperation::OpenFile)?;
    if opened.kind == EntryKind::SymlinkOrReparse {
        return Err(SafeFsError::SymlinkOrReparsePoint {
            operation: SafeFsOperation::OpenFile,
        });
    }
    if opened.kind != EntryKind::RegularFile {
        return Err(SafeFsError::UnsupportedEntryType {
            operation: SafeFsOperation::OpenFile,
            kind: opened.kind,
        });
    }
    Ok(FileCapability {
        native: NativeFile {
            handle,
            opened: opened.clone(),
            access,
            delete_right: false,
        },
        access,
        opened,
    })
}

pub(super) fn create_dir_new(
    parent: &DirectoryAuthority,
    name: &ComponentName,
    permissions: CreatePermissions,
    access: DirectoryAccess,
) -> Result<DirectoryAuthority> {
    if access == DirectoryAccess::Stage {
        return Err(SafeFsError::AccessMismatch {
            operation: SafeFsOperation::CreateDirectory,
        });
    }
    create_directory_contract(
        parent,
        name,
        permissions,
        access,
        contract_for_operation(OpenOperation::CreateDir),
    )
}

pub(super) fn create_stage_dir_new(
    parent: &DirectoryAuthority,
    name: &ComponentName,
    permissions: CreatePermissions,
) -> Result<StageCapability> {
    let directory = create_directory_contract(
        parent,
        name,
        permissions,
        DirectoryAccess::Stage,
        contract_for_operation(OpenOperation::CreateStage),
    )?;
    if let Err(error) = inject_windows_create_failure(
        WindowsCreateFailurePoint::ParentDuplicate,
        SafeFsOperation::CreateStageDirectory,
    ) {
        return rollback_created_directory(directory, error);
    }
    let owned_parent = match duplicate_directory(parent) {
        Ok(value) => value,
        Err(error) => return rollback_created_directory(directory, error),
    };
    let opened = directory.opened.clone();
    Ok(StageCapability {
        parent: owned_parent,
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
    require_mutation(parent, SafeFsOperation::CreateFile)?;
    let security = match permissions {
        CreatePermissions::OwnerOnly => Some(OwnerOnlySecurity::new(false)?),
        CreatePermissions::Inherit => None,
    };
    let security_descriptor = security
        .as_ref()
        .map_or(null(), OwnerOnlySecurity::descriptor_ptr);
    let contract = contract_for_operation(OpenOperation::CreateFile);
    let handle = nt_create_relative(
        parent.native.node.handle.raw(),
        name,
        parent.case_mode,
        contract.desired,
        contract.disposition,
        contract.options,
        contract.attributes,
        security_descriptor,
        SafeFsOperation::CreateFile,
    )?;
    let validated =
        (|| -> Result<EntryMetadata> {
            inject_windows_create_failure(
                WindowsCreateFailurePoint::FilesystemProbe,
                SafeFsOperation::CreateFile,
            )?;
            let filesystem = parent.opened.filesystem.as_ref().ok_or(
                SafeFsError::UnsupportedSecureFilesystem {
                    operation: SafeFsOperation::ProbeFilesystem,
                    reason: SecureFilesystemReason::FilesystemProbeUnavailable,
                },
            )?;
            inject_windows_create_failure(
                WindowsCreateFailurePoint::Metadata,
                SafeFsOperation::CreateFile,
            )?;
            let opened =
                query_entry_metadata(handle.raw(), filesystem, SafeFsOperation::CreateFile)?;
            inject_windows_create_failure(
                WindowsCreateFailurePoint::TypeValidation,
                SafeFsOperation::CreateFile,
            )?;
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
    Ok(FileCapability {
        native: NativeFile {
            handle,
            opened: opened.clone(),
            access: FileAccess::ReadWrite,
            delete_right: false,
        },
        access: FileAccess::ReadWrite,
        opened,
    })
}

pub(super) fn read_link_component(
    parent: &DirectoryAuthority,
    name: &ComponentName,
) -> Result<RawLinkTarget> {
    let contract = contract_for_operation(OpenOperation::Query);
    let handle = nt_create_relative(
        parent.native.node.handle.raw(),
        name,
        parent.case_mode,
        contract.desired,
        contract.disposition,
        contract.options,
        contract.attributes,
        null(),
        SafeFsOperation::ReadLink,
    )?;
    let filesystem =
        parent
            .opened
            .filesystem
            .as_ref()
            .ok_or(SafeFsError::UnsupportedSecureFilesystem {
                operation: SafeFsOperation::ProbeFilesystem,
                reason: SecureFilesystemReason::FilesystemProbeUnavailable,
            })?;
    let metadata = query_entry_metadata(handle.raw(), filesystem, SafeFsOperation::ReadLink)?;
    if metadata.kind != EntryKind::SymlinkOrReparse {
        return Err(SafeFsError::UnsupportedEntryType {
            operation: SafeFsOperation::ReadLink,
            kind: metadata.kind,
        });
    }
    query_reparse(handle.raw())
}

pub(super) fn metadata_from_file(file: &NativeFile) -> Result<EntryMetadata> {
    let filesystem =
        file.opened
            .filesystem
            .as_ref()
            .ok_or(SafeFsError::UnsupportedSecureFilesystem {
                operation: SafeFsOperation::ProbeFilesystem,
                reason: SecureFilesystemReason::FilesystemProbeUnavailable,
            })?;
    query_entry_metadata(
        file.handle.raw(),
        filesystem,
        SafeFsOperation::QueryMetadata,
    )
}

fn rename_retained_noreplace(
    native: &NativeDirectory,
    parent: &DirectoryAuthority,
    target: &ComponentName,
) -> Result<()> {
    require_mutation(parent, SafeFsOperation::RenameNoReplaceSameParent)?;
    if matches!(
        query_child_nofollow(parent, target)?,
        ChildState::Present(_)
    ) {
        return Err(SafeFsError::AlreadyExists {
            operation: SafeFsOperation::RenameNoReplaceSameParent,
        });
    }
    if !native.delete_right {
        return Err(raw_nt(
            SafeFsOperation::RenameNoReplaceSameParent,
            STATUS_ACCESS_DENIED,
        ));
    }
    let buffer = RenameInformationBuffer::new(parent.native.node.handle.raw(), target)?;
    let mut iosb = IO_STATUS_BLOCK::default();
    // SAFETY: the retained DELETE source and parent handles plus the aligned,
    // initialized variable-length buffer remain live for this synchronous call.
    let status = unsafe {
        NtSetInformationFile(
            native.node.handle.raw(),
            &mut iosb,
            buffer.as_ptr(),
            buffer.used,
            FileRenameInformation,
        )
    };
    if status < STATUS_SUCCESS {
        return Err(map_rename_failure(
            status,
            true,
            native.delete_right,
            query_child_nofollow(parent, target),
        ));
    }
    complete_nt(SafeFsOperation::RenameNoReplaceSameParent, status, &iosb)
}

fn verify_same_parent(expected: &DirectoryAuthority, actual: &DirectoryAuthority) -> Result<()> {
    if expected.opened.identity == actual.opened.identity && expected.snapshot == actual.snapshot {
        Ok(())
    } else {
        Err(SafeFsError::NamespaceChanged {
            operation: SafeFsOperation::RenameNoReplaceSameParent,
        })
    }
}

pub(super) fn quarantine_stage(
    stage: StageCapability,
    parent: &DirectoryAuthority,
    quarantine_name: ComponentName,
) -> Result<QuarantinedCapability> {
    let StageCapability {
        parent: owned_parent,
        directory,
        original_name,
        opened,
    } = stage;
    verify_same_parent(&owned_parent, parent)?;
    revalidate_namespace(parent)?;
    rename_retained_noreplace(&directory.native, parent, &quarantine_name)?;
    Ok(QuarantinedCapability {
        parent: owned_parent,
        directory,
        original_name,
        quarantine_name,
        opened,
    })
}

pub(super) fn publish_stage_noreplace(
    stage: StageCapability,
    parent: &DirectoryAuthority,
    destination: ComponentName,
) -> Result<()> {
    let StageCapability {
        parent: owned_parent,
        directory,
        opened,
        ..
    } = stage;
    verify_same_parent(&owned_parent, parent)?;
    revalidate_namespace(parent)?;
    if directory.opened.identity != opened.identity {
        return Err(SafeFsError::IdentityChanged {
            operation: SafeFsOperation::RenameNoReplaceSameParent,
            expected: opened.identity,
            actual: directory.opened.identity.clone(),
        });
    }
    rename_retained_noreplace(&directory.native, parent, &destination)?;
    drop(directory);
    Ok(())
}

#[allow(clippy::arc_with_non_send_sync)] // Arc retains the HANDLE parent chain; capabilities never cross threads.
pub(super) fn open_cleanup_child_nofollow(
    quarantined: &QuarantinedCapability,
    name: &ComponentName,
) -> Result<CleanupCapability> {
    let parent = &quarantined.directory;
    let metadata = match query_child_nofollow(parent, name)? {
        ChildState::Absent => {
            return Err(SafeFsError::NotFound {
                operation: SafeFsOperation::OpenCleanupEntry,
            })
        }
        ChildState::Present(metadata) => metadata,
    };
    let contract = contract_for_operation(match metadata.kind {
        EntryKind::Directory => OpenOperation::CleanupDir,
        EntryKind::SymlinkOrReparse => OpenOperation::CleanupReparse,
        _ => OpenOperation::CleanupFile,
    });
    let handle = nt_create_relative(
        parent.native.node.handle.raw(),
        name,
        parent.case_mode,
        contract.desired,
        contract.disposition,
        contract.options,
        contract.attributes,
        null(),
        SafeFsOperation::OpenCleanupEntry,
    )?;
    let filesystem =
        parent
            .opened
            .filesystem
            .as_ref()
            .ok_or(SafeFsError::UnsupportedSecureFilesystem {
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
            native: NativeDirectory {
                node,
                access: DirectoryAccess::MutateChildren,
                delete_right: true,
            },
            access: DirectoryAccess::MutateChildren,
            opened: opened.clone(),
            case_mode: child_case,
            snapshot: child_snapshot,
        };
        Ok(CleanupCapability::Directory(Box::new(
            QuarantinedCapability {
                parent: duplicated_parent,
                directory,
                original_name: name.clone(),
                quarantine_name: name.clone(),
                opened,
            },
        )))
    } else {
        Ok(CleanupCapability::Entry(Box::new(CleanupEntry {
            parent: duplicate_directory(parent)?,
            native: NativeFile {
                handle,
                opened: opened.clone(),
                access: FileAccess::Read,
                delete_right: true,
            },
            name: name.clone(),
            opened,
            access: CleanupAccess::Delete,
        })))
    }
}

#[cfg(test)]
type BeforeRetainedDeleteHook =
    Arc<dyn Fn(HANDLE, &DirectoryAuthority, &ComponentName) -> Result<()> + Send + Sync>;

#[cfg(test)]
static BEFORE_RETAINED_DELETE_HOOK: std::sync::OnceLock<
    std::sync::Mutex<Option<BeforeRetainedDeleteHook>>,
> = std::sync::OnceLock::new();

#[cfg(test)]
struct BeforeRetainedDeleteHookGuard;

#[cfg(test)]
impl Drop for BeforeRetainedDeleteHookGuard {
    fn drop(&mut self) {
        *BEFORE_RETAINED_DELETE_HOOK
            .get_or_init(Default::default)
            .lock()
            .expect("retained-delete hook mutex poisoned") = None;
    }
}

#[cfg(test)]
fn install_before_retained_delete_hook(
    hook: BeforeRetainedDeleteHook,
) -> BeforeRetainedDeleteHookGuard {
    let mut slot = BEFORE_RETAINED_DELETE_HOOK
        .get_or_init(Default::default)
        .lock()
        .expect("retained-delete hook mutex poisoned");
    assert!(
        slot.is_none(),
        "retained-delete tests require --test-threads=1"
    );
    *slot = Some(hook);
    BeforeRetainedDeleteHookGuard
}

fn run_before_retained_delete_hook(
    handle: HANDLE,
    parent: &DirectoryAuthority,
    name: &ComponentName,
) -> Result<()> {
    #[cfg(test)]
    {
        let hook = BEFORE_RETAINED_DELETE_HOOK
            .get_or_init(Default::default)
            .lock()
            .expect("retained-delete hook mutex poisoned")
            .clone();
        if let Some(hook) = hook {
            return hook(handle, parent, name);
        }
    }
    let _ = (handle, parent, name);
    Ok(())
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
        return Err(SafeFsError::UnsupportedEntryType {
            operation,
            kind: native.opened.kind,
        });
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
            let CleanupEntry {
                parent,
                native,
                name,
                opened,
                access: CleanupAccess::Delete,
            } = *entry;
            if native.opened.identity != opened.identity {
                return Err(SafeFsError::IdentityChanged {
                    operation: SafeFsOperation::DeleteQuarantinedEntry,
                    expected: opened.identity,
                    actual: native.opened.identity,
                });
            }
            dispose_retained(
                native,
                &parent,
                &name,
                opened.kind,
                SafeFsOperation::DeleteQuarantinedEntry,
            )
        }
        CleanupCapability::Directory(_) => Err(SafeFsError::UnsupportedEntryType {
            operation: SafeFsOperation::DeleteQuarantinedEntry,
            kind: EntryKind::Directory,
        }),
    }
}

pub(super) fn delete_quarantined_empty_directory(quarantined: QuarantinedCapability) -> Result<()> {
    let QuarantinedCapability {
        parent,
        directory,
        quarantine_name,
        opened,
        ..
    } = quarantined;
    if directory.opened.identity != opened.identity || !directory.native.delete_right {
        return Err(SafeFsError::IdentityChanged {
            operation: SafeFsOperation::DeleteQuarantinedEmptyDirectory,
            expected: opened.identity,
            actual: directory.opened.identity,
        });
    }
    let native = NativeFile {
        handle: Arc::try_unwrap(directory.native.node)
            .map_err(|node| {
                SafeFsError::io(
                    SafeFsOperation::DeleteQuarantinedEmptyDirectory,
                    io::Error::other(format!(
                        "directory handle still shared: {}",
                        Arc::strong_count(&node)
                    )),
                )
            })?
            .handle,
        opened: directory.opened,
        access: FileAccess::Read,
        delete_right: true,
    };
    dispose_retained(
        native,
        &parent,
        &quarantine_name,
        EntryKind::Directory,
        SafeFsOperation::DeleteQuarantinedEmptyDirectory,
    )
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum WindowsCreateFailurePoint {
    Metadata,
    FilesystemProbe,
    TypeValidation,
    CaseProof,
    SnapshotAssembly,
    ParentDuplicate,
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

fn inject_windows_create_failure(
    point: WindowsCreateFailurePoint,
    operation: SafeFsOperation,
) -> Result<()> {
    #[cfg(test)]
    {
        let mut slot = WINDOWS_CREATE_FAILURE
            .get_or_init(Default::default)
            .lock()
            .expect("Windows create-failure mutex poisoned");
        if *slot == Some(point) {
            *slot = None;
            return Err(SafeFsError::io(
                operation,
                io::Error::other(format!("injected Windows {point:?} failure")),
            ));
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
    assert!(
        !FAIL_NEXT_CREATED_DISPOSITION.swap(true, std::sync::atomic::Ordering::SeqCst),
        "created disposition tests require --test-threads=1"
    );
    CreatedDispositionFailureGuard
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use std::process::Command;
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

    fn present_for_test() -> ChildState {
        ChildState::Present(EntryMetadata {
            identity: StableIdentity::Windows {
                volume_serial: 7,
                file_id: [3; 16],
            },
            kind: EntryKind::RegularFile,
            len: 0,
            link_count: 1,
            filesystem: Some(LocalFilesystemSnapshot::Windows {
                volume_guid: vec![1],
                serial: 7,
            }),
        })
    }

    #[test]
    fn remote_protocol_query_buffer_initializes_required_header() {
        let buffer = remote_protocol_query_buffer(SafeFsOperation::ProbeVolume).unwrap();

        assert_eq!(size_of::<FILE_REMOTE_PROTOCOL_INFO>(), 116);
        assert_eq!(align_of::<FILE_REMOTE_PROTOCOL_INFO>(), 4);
        assert_eq!(size_of::<RemoteProtocolQueryBuffer>(), 120);
        assert_eq!(align_of::<RemoteProtocolQueryBuffer>(), 8);
        assert_eq!(std::ptr::addr_of!(buffer.info) as usize % 8, 0);
        assert_eq!(buffer.info.StructureVersion, 2);
        assert_eq!(
            buffer.info.StructureSize,
            u16::try_from(size_of::<FILE_REMOTE_PROTOCOL_INFO>()).unwrap()
        );
        assert_ne!(
            usize::from(buffer.info.StructureSize),
            size_of::<RemoteProtocolQueryBuffer>()
        );
        assert_eq!(buffer.info.Protocol, 0);
        assert_eq!(buffer.info.ProtocolMajorVersion, 0);
        assert_eq!(buffer.info.ProtocolMinorVersion, 0);
        assert_eq!(buffer.info.ProtocolRevision, 0);
        assert_eq!(buffer.info.Reserved, 0);
        assert_eq!(buffer.info.Flags, 0);
        assert_eq!(buffer.info.GenericReserved.Reserved, [0; 8]);
        // SAFETY: the helper starts from a zeroed union, so its Reserved view is initialized.
        assert_eq!(unsafe { buffer.info.ProtocolSpecific.Reserved }, [0; 16]);
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

    #[test]
    fn owner_only_file_directory_stage_succeed_and_rollback() {
        let temp = TestDir::new("owner-only");
        let authority = root(&temp);

        let file = create_file_new(&authority, &name("file"), CreatePermissions::OwnerOnly)
            .expect("owner-only file creation succeeds");
        drop(file);
        let directory = create_dir_new(
            &authority,
            &name("directory"),
            CreatePermissions::OwnerOnly,
            DirectoryAccess::MutateChildren,
        )
        .expect("owner-only directory creation succeeds");
        drop(directory);
        let stage = create_stage_dir_new(&authority, &name("stage"), CreatePermissions::OwnerOnly)
            .expect("owner-only stage creation succeeds");
        drop(stage);
        for value in ["file", "directory", "stage"] {
            assert!(matches!(
                query_child_nofollow(&authority, &name(value)).unwrap(),
                ChildState::Present(_)
            ));
        }

        force_next_owner_verification_failure();
        assert!(matches!(
            create_file_new(
                &authority,
                &name("rollback-file"),
                CreatePermissions::OwnerOnly
            ),
            Err(SafeFsError::InvalidNativeBuffer {
                operation: SafeFsOperation::VerifySecurityDescriptor,
                reason: NativeBufferReason::SecurityDescriptorMalformed,
            })
        ));
        force_next_owner_verification_failure();
        assert!(matches!(
            create_dir_new(
                &authority,
                &name("rollback-directory"),
                CreatePermissions::OwnerOnly,
                DirectoryAccess::MutateChildren,
            ),
            Err(SafeFsError::InvalidNativeBuffer {
                operation: SafeFsOperation::VerifySecurityDescriptor,
                reason: NativeBufferReason::SecurityDescriptorMalformed,
            })
        ));
        force_next_owner_verification_failure();
        assert!(matches!(
            create_stage_dir_new(
                &authority,
                &name("rollback-stage"),
                CreatePermissions::OwnerOnly,
            ),
            Err(SafeFsError::InvalidNativeBuffer {
                operation: SafeFsOperation::VerifySecurityDescriptor,
                reason: NativeBufferReason::SecurityDescriptorMalformed,
            })
        ));
        for value in ["rollback-file", "rollback-directory", "rollback-stage"] {
            assert!(matches!(
                query_child_nofollow(&authority, &name(value)).unwrap(),
                ChildState::Absent
            ));
        }
    }

    #[test]
    fn windows_post_create_security_failure_rolls_back_same_handle() {
        let temp = TestDir::new("security-rollback");
        let authority = root(&temp);
        let _failure =
            install_windows_create_failure(WindowsCreateFailurePoint::SecurityVerification);
        assert!(matches!(
            create_file_new(&authority, &name("leaf"), CreatePermissions::OwnerOnly),
            Err(SafeFsError::Io {
                operation: SafeFsOperation::VerifySecurityDescriptor,
                ..
            })
        ));
        assert!(matches!(
            query_child_nofollow(&authority, &name("leaf")).unwrap(),
            ChildState::Absent
        ));
    }

    #[test]
    fn quarantine_and_publish_success_do_not_self_conflict() {
        let quarantine_temp = TestDir::new("quarantine-success");
        let authority = root(&quarantine_temp);
        let stage =
            create_stage_dir_new(&authority, &name("stage"), CreatePermissions::Inherit).unwrap();
        let quarantined = quarantine_stage(stage, &authority, name("quarantine"))
            .expect("retained quarantine rename succeeds");
        drop(quarantined);
        assert!(!quarantine_temp.path().join("stage").exists());
        assert!(quarantine_temp.path().join("quarantine").is_dir());

        let publish_temp = TestDir::new("publish-success");
        let authority = root(&publish_temp);
        let stage =
            create_stage_dir_new(&authority, &name("stage"), CreatePermissions::Inherit).unwrap();
        publish_stage_noreplace(stage, &authority, name("destination"))
            .expect("retained publish rename succeeds");
        assert!(!publish_temp.path().join("stage").exists());
        assert!(publish_temp.path().join("destination").is_dir());
    }

    #[test]
    fn rename_never_replaces_any_target_kind() {
        assert!(matches!(
            map_rename_failure(STATUS_ACCESS_DENIED, true, true, Ok(present_for_test())),
            SafeFsError::AlreadyExists { .. }
        ));
        assert!(matches!(
            map_rename_failure(STATUS_ACCESS_DENIED, true, true, Ok(ChildState::Absent)),
            SafeFsError::Os {
                raw: RawOsError::NtStatus {
                    status: STATUS_ACCESS_DENIED,
                    ..
                },
                ..
            }
        ));
        for kind in ["file", "empty-dir", "nonempty-dir", "reparse"] {
            let temp = TestDir::new(kind);
            let target = temp.path().join("target");
            let external = temp.path().join("external");
            match kind {
                "file" => fs::write(&target, b"keep-file").unwrap(),
                "empty-dir" => fs::create_dir(&target).unwrap(),
                "nonempty-dir" => {
                    fs::create_dir(&target).unwrap();
                    fs::write(target.join("keep"), b"tree").unwrap();
                }
                "reparse" => {
                    fs::create_dir(&external).unwrap();
                    fs::write(external.join("keep"), b"outside").unwrap();
                    let output = Command::new("cmd")
                        .args(["/C", "mklink", "/J"])
                        .arg(&target)
                        .arg(&external)
                        .output()
                        .unwrap();
                    assert!(
                        output.status.success(),
                        "mklink failed: {}",
                        String::from_utf8_lossy(&output.stderr)
                    );
                }
                _ => unreachable!(),
            }
            let authority = root(&temp);
            let before = match query_child_nofollow(&authority, &name("target")).unwrap() {
                ChildState::Present(value) => value,
                ChildState::Absent => panic!("collision target absent"),
            };
            let stage =
                create_stage_dir_new(&authority, &name("stage"), CreatePermissions::Inherit)
                    .unwrap();
            assert!(matches!(
                publish_stage_noreplace(stage, &authority, name("target")),
                Err(SafeFsError::AlreadyExists { .. })
            ));
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
    fn cleanup_quarantined_tree_deletes_nested_reparse_without_traversal() {
        let temp = TestDir::new("cleanup-tree");
        let external = temp.path().join("external");
        fs::create_dir(&external).unwrap();
        fs::write(external.join("keep"), b"outside-bytes").unwrap();
        let authority = root(&temp);
        let stage =
            create_stage_dir_new(&authority, &name("stage"), CreatePermissions::Inherit).unwrap();
        let nested = create_dir_new(
            stage.directory(),
            &name("nested"),
            CreatePermissions::Inherit,
            DirectoryAccess::MutateChildren,
        )
        .unwrap();
        let mut file = create_file_new(&nested, &name("data"), CreatePermissions::Inherit).unwrap();
        file.write_all(b"inside").unwrap();
        drop(file);
        drop(nested);
        let output = Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(temp.path().join("stage").join("nested").join("link"))
            .arg(&external)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "mklink failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let quarantine = quarantine_stage(stage, &authority, name("quarantine")).unwrap();
        super::super::cleanup_quarantined_tree(quarantine)
            .expect("common recursive cleanup succeeds");
        assert!(matches!(
            query_child_nofollow(&authority, &name("quarantine")).unwrap(),
            ChildState::Absent
        ));
        assert_eq!(fs::read(external.join("keep")).unwrap(), b"outside-bytes");
    }

    #[test]
    fn retained_delete_survives_real_name_rebound() {
        let temp = TestDir::new("delete-rebound");
        let authority = root(&temp);
        let stage =
            create_stage_dir_new(&authority, &name("stage"), CreatePermissions::Inherit).unwrap();
        let mut file =
            create_file_new(stage.directory(), &name("leaf"), CreatePermissions::Inherit).unwrap();
        file.write_all(b"original").unwrap();
        drop(file);
        let quarantine = quarantine_stage(stage, &authority, name("quarantine")).unwrap();
        let quarantine_path = temp.path().join("quarantine");
        let _guard =
            install_before_retained_delete_hook(Arc::new(move |source, parent, _old_name| {
                let buffer = RenameInformationBuffer::new(
                    parent.native.node.handle.raw(),
                    &name("moved-original"),
                )?;
                let mut iosb = IO_STATUS_BLOCK::default();
                // SAFETY: source is the retained DELETE handle and all inputs
                // remain live for this synchronous test-only rename.
                let status = unsafe {
                    NtSetInformationFile(
                        source,
                        &mut iosb,
                        buffer.as_ptr(),
                        buffer.used,
                        FileRenameInformation,
                    )
                };
                complete_nt(SafeFsOperation::RenameNoReplaceSameParent, status, &iosb)?;
                fs::write(quarantine_path.join("leaf"), b"replacement")
                    .map_err(|error| SafeFsError::io(SafeFsOperation::CreateFile, error))?;
                Ok(())
            }));
        let cleanup = open_cleanup_child_nofollow(&quarantine, &name("leaf")).unwrap();
        delete_quarantined_entry(cleanup).unwrap();
        assert_eq!(
            fs::read(temp.path().join("quarantine").join("leaf")).unwrap(),
            b"replacement"
        );
        assert!(!temp
            .path()
            .join("quarantine")
            .join("moved-original")
            .exists());
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
