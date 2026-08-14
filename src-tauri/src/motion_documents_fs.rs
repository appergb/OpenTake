//! Capability-relative, crash-safe file primitives for Motion Studio documents.

use std::io::{Read, Write};

use cap_fs_ext::{DirExt, FollowSymlinks, OpenOptionsFollowExt};
use cap_std::fs::{Dir, File, OpenOptions};
use same_file::Handle;

use super::{validate_revision_directory, CATALOG_FILE};

#[derive(Debug)]
pub(super) struct CatalogWriteError {
    pub(super) message: String,
    /// The atomic catalog replacement already happened. Callers must retain
    /// the newly referenced revision and reconcile by reading the catalog.
    pub(super) committed: bool,
}

impl CatalogWriteError {
    fn before_commit(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            committed: false,
        }
    }

    fn after_commit(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            committed: true,
        }
    }
}

pub(super) fn write_new_file(directory: &Dir, name: &str, bytes: &[u8]) -> Result<File, String> {
    let mut options = OpenOptions::new();
    options
        .write(true)
        .create_new(true)
        .follow(FollowSymlinks::No);
    #[cfg(windows)]
    {
        use cap_std::fs::OpenOptionsExt;
        use windows_sys::Win32::Foundation::{GENERIC_READ, GENERIC_WRITE};
        use windows_sys::Win32::Storage::FileSystem::{
            DELETE, FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE,
        };
        options
            .access_mode(GENERIC_READ | GENERIC_WRITE | DELETE)
            .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE);
    }
    let mut file = directory
        .open_with(name, &options)
        .map_err(|error| format!("motion document file could not be created: {error}"))?;
    file.write_all(bytes)
        .map_err(|error| format!("motion document file could not be written: {error}"))?;
    file.sync_all()
        .map_err(|error| format!("motion document file could not be synced: {error}"))?;
    Ok(file)
}

pub(super) fn write_catalog_atomic(
    root: &Dir,
    bytes: &[u8],
    inject_replace_failure: bool,
    inject_sync_failure: bool,
) -> Result<(), CatalogWriteError> {
    let temp_name = format!(".catalog-{}.tmp", uuid::Uuid::new_v4());
    let temp = write_new_file(root, &temp_name, bytes).map_err(CatalogWriteError::before_commit)?;
    let result = (|| {
        match root.symlink_metadata(CATALOG_FILE) {
            Ok(metadata) if !metadata.is_file() || metadata.file_type().is_symlink() => {
                return Err(CatalogWriteError::before_commit(
                    "motion document catalog must be a no-follow regular file",
                ))
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(CatalogWriteError::before_commit(format!(
                    "motion document catalog could not be inspected: {error}"
                )))
            }
        }
        if !file_matches_name(root, &temp_name, &temp).map_err(CatalogWriteError::before_commit)? {
            return Err(CatalogWriteError::before_commit(
                "motion document catalog staging identity changed",
            ));
        }
        if inject_replace_failure {
            return Err(CatalogWriteError::before_commit(
                "injected catalog replace failure",
            ));
        }
        replace_catalog_file(root, &temp, &temp_name, CATALOG_FILE).map_err(|error| {
            CatalogWriteError::before_commit(format!(
                "motion document catalog could not be replaced: {error}"
            ))
        })?;
        if inject_sync_failure {
            return Err(CatalogWriteError::after_commit(
                "injected catalog directory sync failure after commit",
            ));
        }
        sync_directory(root).map_err(CatalogWriteError::after_commit)?;
        Ok(())
    })();
    if result.as_ref().is_err_and(|error| !error.committed)
        && file_matches_name(root, &temp_name, &temp).unwrap_or(false)
    {
        let _ = root.remove_file(&temp_name);
    }
    result
}

fn file_matches_name(root: &Dir, name: &str, expected: &File) -> Result<bool, String> {
    let mut options = OpenOptions::new();
    options.read(true).follow(FollowSymlinks::No);
    #[cfg(unix)]
    {
        use cap_std::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NONBLOCK);
    }
    #[cfg(windows)]
    {
        use cap_std::fs::OpenOptionsExt;
        use windows_sys::Win32::Storage::FileSystem::{
            FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE,
        };
        options.share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE);
    }
    let current = match root.open_with(name, &options) {
        Ok(current) => current,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => {
            return Err(format!(
                "motion document catalog staging could not be inspected: {error}"
            ))
        }
    };
    let expected = expected
        .try_clone()
        .and_then(|file| Handle::from_file(file.into_std()))
        .map_err(|_| "motion document catalog staging identity is unavailable".to_string())?;
    let current = Handle::from_file(current.into_std())
        .map_err(|_| "motion document catalog staging identity is unavailable".to_string())?;
    Ok(expected == current)
}

#[cfg(not(windows))]
fn replace_catalog_file(
    root: &Dir,
    _temp: &File,
    temp_name: &str,
    target: &str,
) -> std::io::Result<()> {
    root.rename(temp_name, root, target)
}

#[cfg(windows)]
fn replace_catalog_file(
    root: &Dir,
    temp: &File,
    _temp_name: &str,
    target: &str,
) -> std::io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Storage::FileSystem::{
        FileRenameInfo, SetFileInformationByHandle, FILE_RENAME_INFO, FILE_RENAME_INFO_0,
    };

    let wide = std::ffi::OsStr::new(target)
        .encode_wide()
        .collect::<Vec<_>>();
    if wide.is_empty() || wide.contains(&0) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "motion document catalog target is invalid",
        ));
    }
    let file_name_bytes = wide
        .len()
        .checked_mul(std::mem::size_of::<u16>())
        .and_then(|size| u32::try_from(size).ok())
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidInput, "target too long"))?;
    let info_size = std::mem::size_of::<FILE_RENAME_INFO>()
        .checked_add(file_name_bytes as usize)
        .ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "rename allocation overflow",
            )
        })?;
    const _: () =
        assert!(std::mem::align_of::<usize>() >= std::mem::align_of::<FILE_RENAME_INFO>());
    let word_size = std::mem::size_of::<usize>();
    let mut storage = vec![0_usize; info_size.div_ceil(word_size)];
    let info = storage.as_mut_ptr().cast::<FILE_RENAME_INFO>();
    // SAFETY: storage is aligned and sized for the native header plus target;
    // both retained handles remain alive for the synchronous call.
    let replaced = unsafe {
        (*info).Anonymous = FILE_RENAME_INFO_0 {
            ReplaceIfExists: true,
        };
        (*info).RootDirectory = root.as_raw_handle();
        (*info).FileNameLength = file_name_bytes;
        std::ptr::copy_nonoverlapping(
            wide.as_ptr(),
            std::ptr::addr_of_mut!((*info).FileName).cast::<u16>(),
            wide.len(),
        );
        SetFileInformationByHandle(
            temp.as_raw_handle(),
            FileRenameInfo,
            info.cast(),
            u32::try_from(info_size).map_err(|_| {
                std::io::Error::new(std::io::ErrorKind::InvalidInput, "rename buffer too large")
            })?,
        )
    };
    if replaced == 0 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(())
    }
}

pub(super) fn read_bounded_file(
    directory: &Dir,
    name: &str,
    max_bytes: usize,
    label: &str,
) -> Result<Option<Vec<u8>>, String> {
    let mut options = OpenOptions::new();
    options.read(true).follow(FollowSymlinks::No);
    #[cfg(unix)]
    {
        use cap_std::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NONBLOCK);
    }
    let mut file = match directory.open_with(name, &options) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => {
            return Err(format!(
                "motion document {label} must be a no-follow regular file"
            ))
        }
    };
    let metadata = file
        .metadata()
        .map_err(|_| format!("motion document {label} metadata is unavailable"))?;
    if !metadata.is_file() || metadata.len() > max_bytes as u64 {
        return Err(format!("motion document {label} exceeds its byte limit"));
    }
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    Read::by_ref(&mut file)
        .take(max_bytes as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| format!("motion document {label} could not be read"))?;
    if bytes.len() > max_bytes {
        return Err(format!("motion document {label} exceeds its byte limit"));
    }
    Ok(Some(bytes))
}

pub(super) fn read_bounded_utf8(
    directory: &Dir,
    name: &str,
    max_bytes: usize,
) -> Result<String, String> {
    let bytes = read_bounded_file(directory, name, max_bytes, name)?
        .ok_or_else(|| format!("motion document {name} is missing"))?;
    String::from_utf8(bytes).map_err(|_| format!("motion document {name} must be UTF-8"))
}

pub(super) fn sync_directory(directory: &Dir) -> Result<(), String> {
    #[cfg(unix)]
    {
        directory
            .try_clone()
            .and_then(|directory| directory.into_std_file().sync_all())
            .map_err(|error| format!("motion document directory could not be synced: {error}"))
    }
    #[cfg(not(unix))]
    {
        let _ = directory;
        Ok(())
    }
}

pub(super) fn cleanup_revision_directory(root: &Dir, name: &str) {
    if validate_revision_directory(name).is_err() {
        return;
    }
    if let Ok(directory) = root.open_dir_nofollow(name) {
        let _ = directory.remove_open_dir_all();
    }
}
